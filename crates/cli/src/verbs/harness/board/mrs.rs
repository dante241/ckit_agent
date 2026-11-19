//! Board view "Review MR": the project's open PRs/MRs with their last review.
//! Projects with their own `/review-pr` command (`.omp/commands/review-pr.md`)
//! get exactly that command, run headless (`mrs/cmd.rs`): it posts, labels,
//! merges and updates PMS itself. Others get `ckit review`: read-only omp in a
//! detached worktree, posting is a separate click; storage is the same
//! `.cache/ckit/reviews/<n>.json` the CLI writes. Neither touches the working
//! tree, so reviews never wait for or block Board tasks.

use super::{project, ApiErr, ApiRes, RootQ};
use crate::verbs::review;

mod cmd;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, OnceLock};

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/board/mrs", get(api_list))
        .route("/api/board/mrs/:n", get(api_get))
        .route("/api/board/mrs/:n/comments", get(api_comments))
        .route("/api/board/mrs/:n/review", post(api_review))
        .route("/api/board/mrs/:n/answer", post(api_answer))
        .route("/api/board/mrs/:n/post", post(api_post))
}

/// What the MR's comments said about reviews when they were last loaded: the
/// commit ckit's summary covers (posted from here, CI or another machine) and
/// who posted it, plus a review written another way. The list stays one API
/// call; this fills in as MRs are opened.
#[derive(Clone, Default)]
struct Remote {
    head: Option<String>,
    by: Option<String>,
    other: Option<review::OtherReview>,
}

fn remote() -> std::sync::MutexGuard<'static, HashMap<(PathBuf, u64), Remote>> {
    static R: OnceLock<Mutex<HashMap<(PathBuf, u64), Remote>>> = OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Review state per (project, number): waiting for a slot, running, or the
/// last run's error.
#[derive(Clone)]
enum Run {
    Queued,
    Running,
    Failed(String),
}

/// Reviews running at once, across projects. Each is a full omp session; the
/// boss may tick 20 MRs, so the rest wait their turn instead of all starting.
const MAX_PARALLEL: usize = 5;

fn runs() -> std::sync::MutexGuard<'static, HashMap<(PathBuf, u64), Run>> {
    static R: OnceLock<Mutex<HashMap<(PathBuf, u64), Run>>> = OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn slots() -> &'static (Mutex<usize>, Condvar) {
    static S: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    S.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

/// Holds one of the MAX_PARALLEL slots; frees it (and wakes a waiter) on drop.
struct Slot;

impl Slot {
    fn take() -> Self {
        let (m, cv) = slots();
        let mut used = m.lock().unwrap_or_else(|e| e.into_inner());
        while *used >= MAX_PARALLEL {
            used = cv.wait(used).unwrap_or_else(|e| e.into_inner());
        }
        *used += 1;
        Slot
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let (m, cv) = slots();
        *m.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        cv.notify_one();
    }
}

fn state(root: &PathBuf, n: u64) -> Value {
    let (queued, running, error) = match runs().get(&(root.clone(), n)) {
        Some(Run::Queued) => (true, false, None),
        Some(Run::Running) => (false, true, None),
        Some(Run::Failed(e)) => (false, false, Some(e.clone())),
        None => (false, false, None),
    };
    let rm = remote().get(&(root.clone(), n)).cloned().unwrap_or_default();
    json!({
        "queued": queued, "running": running, "error": error, "review": review::load(root, n),
        "posted_head": rm.head, "posted_by": rm.by, "other_review": rm.other, "command_run": cmd::load(root, n),
    })
}

fn bad_gateway(e: anyhow::Error) -> ApiErr {
    (StatusCode::BAD_GATEWAY, format!("{e:#}"))
}

/// gh/glab and git are blocking subprocesses: keep them off the async workers.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, ApiErr> + Send + 'static) -> Result<T, ApiErr> {
    tokio::task::spawn_blocking(f).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
}

/// The pid in a review run id `[cmd-]<n>-<pid>-<seq>`.
fn run_pid(id: &str) -> Option<u32> {
    let parts: Vec<&str> = id.trim_start_matches("cmd-").split('-').collect();
    (parts.len() == 3).then(|| parts[1].parse().ok()).flatten()
}

fn alive(pid: u32) -> bool {
    pid == std::process::id() || std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Reviews run in threads of the dashboard: a restart kills them mid-run and
/// their temp worktrees, refs (and `.omp` links) stay behind. Once per project
/// and process, remove those whose process is gone.
fn sweep(root: &PathBuf) {
    static DONE: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    if !DONE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).insert(root.clone()) {
        return;
    }
    let git = |args: &[&str]| std::process::Command::new("git").current_dir(root).args(args).env("GIT_TERMINAL_PROMPT", "0").stdin(std::process::Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    for wt in git(&["worktree", "list", "--porcelain"]).lines().filter_map(|l| l.strip_prefix("worktree ")).map(PathBuf::from) {
        let Some(dir) = wt.parent().filter(|_| wt.ends_with("wt")) else { continue };
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let Some(id) = name.strip_prefix("ckit-review-pr-").or_else(|| name.strip_prefix("ckit-review-")) else { continue };
        if run_pid(id).is_none_or(alive) {
            continue;
        }
        let link = wt.join(".omp");
        if link.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            let _ = std::fs::remove_file(&link);
        }
        git(&["worktree", "remove", "--force", &wt.display().to_string()]);
        let _ = std::fs::remove_dir_all(dir);
    }
    for r in git(&["for-each-ref", "--format=%(refname)", "refs/ckit-review"]).lines() {
        let id = r.trim_start_matches("refs/ckit-review/").split('/').next().unwrap_or("");
        if run_pid(id).is_some_and(|p| !alive(p)) {
            git(&["update-ref", "-d", r]);
        }
    }
    git(&["worktree", "prune"]);
}

async fn api_list(Query(rq): Query<RootQ>) -> ApiRes<Value> {
    let root = project(&rq)?;
    blocking(move || {
        sweep(&root);
        let (host, rows) = review::list_open(&root).map_err(bad_gateway)?;
        let items: Vec<Value> = rows
            .into_iter()
            .map(|mut r| {
                let n = r["number"].as_u64().unwrap_or(0);
                if let (Some(o), Value::Object(st)) = (r.as_object_mut(), state(&root, n)) {
                    o.extend(st);
                }
                r
            })
            .collect();
        let command = cmd::command_path(&root).map(|p| p.display().to_string());
        Ok(Json(json!({ "host": host, "items": items, "command": command })))
    })
    .await
}

async fn api_get(Path(n): Path<u64>, Query(rq): Query<RootQ>) -> ApiRes<Value> {
    let root = project(&rq)?;
    Ok(Json(state(&root, n)))
}

#[derive(Deserialize, Default)]
struct ReviewBody {
    model: Option<String>,
    /// Project `/review-pr` only: `--no-merge` / `--dry-run`.
    #[serde(flatten)]
    flags: cmd::Flags,
}

/// Queue `job` for MR `n` (one at a time per MR) and run it in a review slot.
fn start(root: PathBuf, n: u64, job: impl FnOnce(&PathBuf) -> anyhow::Result<()> + Send + 'static) -> ApiRes<Value> {
    {
        let mut r = runs();
        if matches!(r.get(&(root.clone(), n)), Some(Run::Running | Run::Queued)) {
            return Err((StatusCode::CONFLICT, "MR này đang review hoặc đang chờ review".into()));
        }
        r.insert((root.clone(), n), Run::Queued);
    }
    let bg = root.clone();
    std::thread::spawn(move || {
        let _slot = Slot::take();
        runs().insert((bg.clone(), n), Run::Running);
        let out = job(&bg);
        let mut r = runs();
        match out {
            Ok(_) => r.remove(&(bg, n)),
            Err(e) => r.insert((bg, n), Run::Failed(format!("{e:#}"))),
        };
    });
    Ok(Json(state(&root, n)))
}

/// Reviews run the project's own `/review-pr` when it has one (command or
/// skill); otherwise `ckit review`.
async fn api_review(Path(n): Path<u64>, Query(rq): Query<RootQ>, body: Option<Json<ReviewBody>>) -> ApiRes<Value> {
    let root = project(&rq)?;
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let model = body.model.filter(|m| !m.trim().is_empty());
    let flags = body.flags;
    start(root, n, move |root| match cmd::command_path(root) {
        Some(path) => cmd::run(root, n, &path, flags).map(|_| ()),
        None => review::review(root, n, model.as_deref()).map(|_| ()),
    })
}

#[derive(Deserialize)]
struct AnswerBody {
    /// true: post (optionally with the boss's edit in `note`); false: don't post.
    post: bool,
    #[serde(default)]
    note: String,
}

/// The boss's answer to `/review-pr`'s question: resumes the paused run.
async fn api_answer(Path(n): Path<u64>, Query(rq): Query<RootQ>, Json(b): Json<AnswerBody>) -> ApiRes<Value> {
    let root = project(&rq)?;
    let path = cmd::command_path(&root).ok_or((StatusCode::BAD_REQUEST, "dự án không có /review-pr".to_string()))?;
    if cmd::load(&root, n).is_none_or(|r| r.verdict != "ask") {
        return Err((StatusCode::CONFLICT, "MR này không có câu hỏi nào đang chờ".into()));
    }
    start(root, n, move |root| cmd::answer(root, n, &path, b.post, &b.note).map(|_| ()))
}

/// MRs being posted right now: two posts racing (double click, a second tab)
/// would both see no summary yet and each create one.
fn posting() -> std::sync::MutexGuard<'static, std::collections::HashSet<(PathBuf, u64)>> {
    static P: OnceLock<Mutex<std::collections::HashSet<(PathBuf, u64)>>> = OnceLock::new();
    P.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

async fn api_post(Path(n): Path<u64>, Query(rq): Query<RootQ>) -> ApiRes<Value> {
    let root = project(&rq)?;
    if matches!(runs().get(&(root.clone(), n)), Some(Run::Running | Run::Queued)) {
        return Err((StatusCode::CONFLICT, "MR này đang review, chờ xong rồi đăng".into()));
    }
    if !posting().insert((root.clone(), n)) {
        return Err((StatusCode::CONFLICT, "MR này đang được đăng".into()));
    }
    blocking(move || {
        let out = review::post(&root, n);
        posting().remove(&(root.clone(), n));
        let s = out.map_err(|e| (StatusCode::CONFLICT, format!("{e:#}")))?;
        let mut r = remote();
        let e = r.entry((root.clone(), n)).or_default();
        e.head = Some(s.head_sha);
        e.by = None;
        drop(r);
        Ok(Json(state(&root, n)))
    })
    .await
}

/// The MR's comments (everyone's, oldest first); also refreshes what they say
/// about reviews (`Remote`).
async fn api_comments(Path(n): Path<u64>, Query(rq): Query<RootQ>) -> ApiRes<Value> {
    let root = project(&rq)?;
    blocking(move || {
        let (items, seen) = review::comments(&root, n).map_err(bad_gateway)?;
        remote().insert((root.clone(), n), Remote { head: seen.ckit_head, by: seen.ckit_by, other: seen.other });
        Ok(Json(json!({ "items": items, "state": state(&root, n) })))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn reviews_beyond_the_cap_wait_and_every_waiter_gets_a_turn() {
        let (now, max) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let hs: Vec<_> = (0..MAX_PARALLEL * 2 + 1)
            .map(|_| {
                let (now, max) = (now.clone(), max.clone());
                std::thread::spawn(move || {
                    let _s = Slot::take();
                    let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                    max.fetch_max(n, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    now.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(max.load(Ordering::SeqCst), MAX_PARALLEL);
    }
}
