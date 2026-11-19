//! Run the project's own `/review-pr` on an MR, the way the boss would type it
//! in omp: a normal omp session (full tools, the project's skills, MCP, `glab`),
//! so every step of it happens (agents, validators, comment, labels, merge, PMS
//! update). The command is `.omp/commands/review-pr.md`, or the skill
//! `.omp/skills/review-pr/SKILL.md` alone when a project has no command file.
//!
//! It runs in a temporary detached checkout of the MR's target branch with the
//! boss's `.omp` linked in (usually gitignored, so only the boss's copy has it;
//! the skill may write `.omp/plans/*` there, as when typed by hand). The boss's
//! working tree and Board tasks are left alone.
//!
//! Where the command asks the boss before posting (blocking findings), the run
//! stops with `verdict: "ask"` and a draft; the Board shows it, and the boss's
//! answer resumes the same omp session (`answer`).

use crate::models::{ModelConfig, TaskClass};
use crate::verbs::review;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The command reads the MR, spawns ~7 agents and validators, posts and merges.
const TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The project's review command (or its skill when there is no command file).
pub(super) fn command_path(root: &Path) -> Option<PathBuf> {
    [".omp/commands/review-pr.md", ".omp/skills/review-pr/SKILL.md"].iter().map(|p| root.join(p)).find(|p| p.is_file())
}

/// Flags the boss picked, passed to the command as `--no-merge` / `--dry-run`.
#[derive(Deserialize, Serialize, Clone, Copy, Default, Debug)]
pub(super) struct Flags {
    #[serde(default)]
    pub(super) no_merge: bool,
    #[serde(default)]
    pub(super) dry_run: bool,
}

/// The last `/review-pr` run on MR `n`: `.cache/ckit/reviews/<n>.cmd.json`.
#[derive(Serialize, Deserialize, Clone)]
pub(super) struct CmdRun {
    head_sha: String,
    /// Unix seconds.
    at: u64,
    ended: u64,
    #[serde(default)]
    flags: Flags,
    /// `pass` · `fail` · `ask` (stopped at the command's question; see `draft`).
    pub(super) verdict: String,
    merged: bool,
    posted: bool,
    blocking: u64,
    message: String,
    /// `ask`: the comment the command would post.
    #[serde(default)]
    draft: String,
    cost: f64,
    /// The run's omp log (`<n>.cmd.jsonl`; an answered run appends to it).
    log: String,
}

#[derive(Deserialize, Default)]
struct RunResult {
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    posted: bool,
    #[serde(default)]
    blocking: u64,
    #[serde(default)]
    message: String,
    #[serde(default)]
    draft: String,
}

fn record_path(root: &Path, n: u64) -> PathBuf {
    review::reviews_dir(root).join(format!("{n}.cmd.json"))
}

/// The run's omp session, kept so the boss's answer continues it.
fn session_dir(root: &Path, n: u64) -> PathBuf {
    review::reviews_dir(root).join(format!("{n}.session"))
}

pub(super) fn load(root: &Path, n: u64) -> Option<CmdRun> {
    serde_json::from_str(&std::fs::read_to_string(record_path(root, n)).ok()?).ok()
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .context("git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Temporary detached checkout of `origin/<target>` with the boss's `.omp`
/// linked in; removed on drop (the link first: it points at the only copy).
struct Checkout {
    root: PathBuf,
    tmp: PathBuf,
    wt: PathBuf,
    refname: String,
}

impl Checkout {
    fn open(root: &Path, n: u64, target: &str) -> Result<Self> {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = format!("{n}-{}-{}", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let tmp = std::env::temp_dir().join(format!("ckit-review-pr-{id}"));
        let co = Self { root: root.to_path_buf(), wt: tmp.join("wt"), tmp, refname: format!("refs/ckit-review/cmd-{id}") };
        git(root, &["fetch", "--quiet", "--no-tags", "origin", &format!("+refs/heads/{target}:{}", co.refname)]).with_context(|| format!("git fetch origin {target}"))?;
        std::fs::create_dir_all(&co.tmp)?;
        git(root, &["worktree", "add", "--quiet", "--detach", &co.wt.display().to_string(), &co.refname])?;
        // A tracked `.omp` on the target wins; otherwise the boss's own.
        let omp = co.wt.join(".omp");
        if !omp.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(root.join(".omp"), &omp).context("link .omp")?;
            #[cfg(not(unix))]
            bail!(".omp không có trên nhánh {target}");
        }
        Ok(co)
    }
}

impl Drop for Checkout {
    fn drop(&mut self) {
        let link = self.wt.join(".omp");
        if link.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            let _ = std::fs::remove_file(&link);
        }
        if self.wt.exists() {
            let _ = git(&self.root, &["worktree", "remove", "--force", &self.wt.display().to_string()]);
        }
        let _ = std::fs::remove_dir_all(&self.tmp);
        let _ = git(&self.root, &["update-ref", "-d", &self.refname]);
    }
}

fn args_of(n: u64, f: Flags) -> String {
    let mut a = n.to_string();
    if f.no_merge {
        a.push_str(" --no-merge");
    }
    if f.dry_run {
        a.push_str(" --dry-run");
    }
    a
}

/// Run `/review-pr <n> [flags]` until it ends or stops at its question.
pub(super) fn run(root: &Path, n: u64, cmd_path: &Path, flags: Flags) -> Result<CmdRun> {
    let session = session_dir(root, n);
    let _ = std::fs::remove_dir_all(&session);
    let tmpl = crate::assets::read("board/review-pr.md").ok_or_else(|| anyhow!("asset board/review-pr.md thiếu"))?;
    exec(root, n, cmd_path, flags, &session, false, |co, target| {
        let rel = cmd_path.strip_prefix(root).unwrap_or(cmd_path);
        tmpl.replace("{{NUMBER}}", &n.to_string())
            .replace("{{ARGS}}", &args_of(n, flags))
            .replace("{{TARGET}}", target)
            .replace("{{CMD_PATH}}", &co.wt.join(rel).display().to_string())
    })
}

/// The boss answered the command's question: continue its session.
pub(super) fn answer(root: &Path, n: u64, cmd_path: &Path, post: bool, note: &str) -> Result<CmdRun> {
    let prev = load(root, n).filter(|r| r.verdict == "ask").ok_or_else(|| anyhow!("MR {n} không có câu hỏi nào đang chờ"))?;
    let session = session_dir(root, n);
    if !session.is_dir() {
        bail!("phiên /review-pr của MR {n} không còn; chạy lại");
    }
    // The draft is a review of `prev.head_sha`: never post it on newer commits.
    let (head, _) = review::head_and_target(root, n)?;
    if head != prev.head_sha {
        let _ = std::fs::remove_dir_all(&session);
        let mut stale = prev.clone();
        stale.verdict = "fail".into();
        stale.message = format!("MR có commit mới ({}) sau lần review ({}); bản nháp đã bỏ, chạy lại /review-pr", &head[..head.len().min(8)], &prev.head_sha[..prev.head_sha.len().min(8)]);
        std::fs::write(record_path(root, n), serde_json::to_vec_pretty(&stale)?)?;
        bail!("{}", stale.message);
    }
    let tmpl = crate::assets::read("board/review-pr-answer.md").ok_or_else(|| anyhow!("asset board/review-pr-answer.md thiếu"))?;
    let choice = match (post, note.trim()) {
        (true, "") => "Đồng ý post comment lên GitLab".to_string(),
        (true, n) => format!("Chỉnh sửa trước khi post: {n}"),
        (false, _) => "Không post, chỉ xem".to_string(),
    };
    exec(root, n, cmd_path, prev.flags, &session, true, |_, target| {
        tmpl.replace("{{NUMBER}}", &n.to_string()).replace("{{TARGET}}", target).replace("{{CHOICE}}", &choice)
    })
}

fn exec(root: &Path, n: u64, cmd_path: &Path, flags: Flags, session: &Path, resume: bool, prompt: impl FnOnce(&Checkout, &str) -> String) -> Result<CmdRun> {
    let started = now();
    let (head_sha, target) = review::head_and_target(root, n)?;
    let co = Checkout::open(root, n, &target)?;
    let rel = cmd_path.strip_prefix(root).unwrap_or(cmd_path);
    if !co.wt.join(rel).is_file() {
        bail!("không thấy {} trong bản checkout của {target}", rel.display());
    }
    let dir = review::reviews_dir(root);
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(session)?;
    let log = dir.join(format!("{n}.cmd.jsonl"));
    let result = co.tmp.join("result.json");
    let text = prompt(&co, &target).replace("{{RESULT}}", &result.display().to_string());
    let f = if resume { std::fs::OpenOptions::new().create(true).append(true).open(&log)? } else { std::fs::File::create(&log)? };
    let mut cmd = super::super::omp_cmd();
    cmd.current_dir(&co.wt)
        .args(["--cwd", &co.wt.display().to_string(), "--mode", "json", "--approval-mode", "yolo", "--no-title", "--session-dir", &session.display().to_string()]);
    if resume {
        cmd.arg("--continue");
    }
    cmd.args(ModelConfig::load().class_flags(TaskClass::Review))
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-p")
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::from(f.try_clone()?))
        .stderr(Stdio::from(f));
    let mut child = cmd.spawn().context("không chạy được omp")?;
    let t = Instant::now();
    let exit = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if t.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            bail!("/review-pr quá {} phút", TIMEOUT.as_secs() / 60);
        }
        std::thread::sleep(Duration::from_millis(1000));
    };
    let res: Option<RunResult> = std::fs::read_to_string(&result).ok().and_then(|s| serde_json::from_str(&s).ok());
    let r = res.unwrap_or_else(|| RunResult { verdict: "fail".into(), message: format!("omp kết thúc ({exit}) mà không ghi file kết quả; xem log {}", log.display()), ..Default::default() });
    let verdict = match r.verdict.trim() {
        v @ ("pass" | "ask") => v.to_string(),
        _ => "fail".to_string(),
    };
    let rec = CmdRun {
        head_sha,
        at: started,
        ended: now(),
        flags,
        verdict,
        merged: r.merged,
        posted: r.posted,
        blocking: r.blocking,
        message: r.message.trim().to_string(),
        draft: r.draft.trim().to_string(),
        cost: super::super::log_cost(&log),
        log: log.display().to_string(),
    };
    std::fs::write(record_path(root, n), serde_json::to_vec_pretty(&rec)?)?;
    if rec.verdict != "ask" {
        let _ = std::fs::remove_dir_all(session);
    }
    if rec.verdict == "fail" {
        bail!("{}", rec.message);
    }
    Ok(rec)
}
