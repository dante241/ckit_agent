//! Follow-up actions on a task ("Tạo merge request", update a PMS ticket, …) run
//! like a normal omp session: the agent picks the project's own skill/command
//! (e.g. vtiger `.omp/skills/gitlab-mr`), with full tools, extensions, skills and
//! MCP, continuing a copy of the task's latest working session. The board hard-codes
//! nothing about HOW the action is done; it keeps only the git safety rails:
//! - pushes: target must exist on origin; the commits `origin/<target>..<branch>`
//!   are listed; commits the task did not make are left out by default (the task's
//!   commits are cherry-picked onto `origin/<target>` as the board-owned
//!   `<branch>-mr`, which only the board force-pushes);
//! - the agent works in a TEMP worktree on the source branch, never the boss's tree;
//! - a `pre-push` hook (via `core.hooksPath` in the run's environment only) refuses
//!   any push to a ref outside `refs/heads/board/*`; afterwards `git ls-remote`
//!   proves only board branches changed (a `--no-verify` push still fails the run).
//!
//! Acceptance ("Nghiệm thu") is the action "create the merge request": Done only
//! when an MR/PR URL comes back, else the task stays at the gate.
use super::chat::{CommitInfo, Mr, Proposal, ProposalResult};
use super::*;

const ACTION_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Where the action comes from: a chat proposal, or the acceptance gate
/// (dashboard / Telegram). `accept` = finishing the task needs an MR URL.
#[derive(Clone)]
pub(super) struct Mode {
    pub msg: Option<String>,
    pub accept: bool,
    pub via: Option<String>,
}

#[derive(Deserialize, Default)]
struct ActionResult {
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    urls: Vec<String>,
}

/// Git state prepared for the run.
struct Prepared {
    /// Directory the agent works in (the temp worktree, or the project without git).
    cwd: PathBuf,
    /// Temp dir holding the worktree, hooks and session copy (removed on success).
    tmp: Option<PathBuf>,
    hooks: Option<PathBuf>,
    source: Option<String>,
    target: Option<String>,
    commits: Vec<CommitInfo>,
    /// `git ls-remote origin` heads/tags before the run.
    refs: Option<Vec<(String, String)>>,
    /// Task branch head when the action started (`Mr.head`).
    head: Option<String>,
    /// Every agent push from the worktree fails (its own pushurl is unusable).
    push_blocked: bool,
}

/// The acceptance's action: create the MR for the feature into `target`.
pub(super) fn mr_instruction(target: Option<&str>) -> String {
    match target {
        Some(t) => format!("Tạo merge request cho tính năng này vào {t}"),
        None => "Tạo merge request cho tính năng này".into(),
    }
}

/// The MR proposal of the acceptance gate (card / `GET /mr/draft` / Telegram).
pub(super) fn mr_proposal(t: &Task) -> Proposal {
    Proposal { kind: "action".into(), title: Some("Tạo merge request".into()), pushes: true, state: "open".into(), target: t.mr.as_ref().map(|m| m.target.clone()), ..Default::default() }
}

/// "Nghiệm thu": a task with an MR already is linked (its branch re-pushed by the
/// board when a later round moved it); otherwise the MR action starts. Returns the
/// saved task right away (the run continues in the background).
pub(super) fn accept(root: &FsPath, id: &str, p: &Proposal, via: Option<&str>) -> Result<Task, String> {
    let t = load(root, id).ok_or_else(|| format!("task {id} không tồn tại"))?;
    if t.branch.is_none() || !is_git(root) {
        // No git: nothing to push, the acceptance just marks it done.
        let _g = guard();
        let mut cur = load(root, id).ok_or_else(|| format!("task {id} không tồn tại"))?;
        if (cur.stage, cur.status) != (Stage::Acceptance, Status::Waiting) || cur.paused {
            return Err("task không chờ nghiệm thu".into());
        }
        cur.say("user", "Nghiệm thu.");
        cur.say("system", "Không có nhánh git — đánh dấu hoàn tất.");
        cur.set(Stage::Done, Status::Done);
        save(root, &mut cur).map_err(|e| e.to_string())?;
        return Ok(cur);
    }
    if let Some(m) = t.mr.clone() {
        if p.target.as_deref().is_none_or(|x| x == m.target) {
            let m = update_mr(root, &t, &m)?;
            let _g = guard();
            let mut cur = load(root, id).ok_or_else(|| format!("task {id} không tồn tại"))?;
            if (cur.stage, cur.status) != (Stage::Acceptance, Status::Waiting) {
                return Err("task không chờ nghiệm thu".into());
            }
            cur.say("user", "Nghiệm thu.");
            cur.say("system", &format!("Nghiệm thu: đã có MR {}", m.url));
            chat::close_mr_proposals(&mut cur, &m.url);
            cur.mr = Some(m);
            cur.set(Stage::Done, Status::Done);
            leave_task_branch(root, &cur);
            save(root, &mut cur).map_err(|e| e.to_string())?;
            return Ok(cur);
        }
    }
    let mut p = p.clone();
    p.kind = "action".into();
    p.pushes = true;
    if p.title.as_deref().is_none_or(|s| s.trim().is_empty()) {
        p.title = Some("Tạo merge request".into());
    }
    if p.instruction.as_deref().is_none_or(|s| s.trim().is_empty()) {
        p.instruction = Some(mr_instruction(p.target.as_deref()));
    }
    start(root, id, &p, Mode { msg: None, accept: true, via: via.map(String::from) }).map(|(t, _)| t)
}

/// An MR recorded before heads were stored (the real T-007 !434) or with nothing
/// new: linked as it is. A later round moved the branch: the board pushes the MR's
/// board-owned branch again (re-replaying `-mr`); the MR follows its branch.
fn update_mr(root: &FsPath, t: &Task, m: &Mr) -> Result<Mr, String> {
    let Some(br) = t.branch.clone().filter(|b| chat::branch_exists(root, b)) else { return Ok(m.clone()) };
    let head = git(root, &["rev-parse", &format!("refs/heads/{br}")]).map(|s| s.trim().to_string()).ok();
    if m.source.is_empty() || m.head.is_none() || m.head == head {
        return Ok(m.clone());
    }
    let commits = chat::prepare(root, t, &m.target)?;
    let spec = if m.source == br {
        format!("refs/heads/{br}:refs/heads/{br}")
    } else {
        let mine: Vec<String> = commits.iter().filter(|c| c.from_task).map(|c| c.sha.clone()).collect();
        chat::replay(root, &m.target, &mine, &m.source)?;
        format!("+refs/heads/{0}:refs/heads/{0}", m.source)
    };
    chat::run_cmd(root, "git", &["push", "origin", &spec], chat::PUSH_TIMEOUT).map_err(|e| format!("push lỗi: {e}"))?;
    Ok(Mr { head, at: iso_now(), ..m.clone() })
}

/// Validate, prepare git, record the run, then run the agent in the background.
/// Errors here are preconditions (nothing ran, nothing pushed).
pub(super) fn start(root: &FsPath, id: &str, p: &Proposal, mode: Mode) -> Result<(Task, std::thread::JoinHandle<()>), String> {
    if !chat::claim(root, id) {
        return Err("đang có việc khác cho task này (Leader trả lời / một thao tác đang chạy) — chờ xong".into());
    }
    let res = prepare_and_record(root, id, p, &mode);
    let (t, prep) = match res {
        Ok(x) => x,
        Err(e) => {
            chat::release(root, id);
            return Err(e);
        }
    };
    let (root2, id2, p2) = (root.to_path_buf(), id.to_string(), p.clone());
    let log = t.runs.last().map(|r| r.log.clone()).unwrap_or_default();
    let h = std::thread::spawn(move || {
        execute(&root2, &id2, &p2, &mode, prep, &log);
        chat::release(&root2, &id2);
    });
    Ok((t, h))
}

fn prepare_and_record(root: &FsPath, id: &str, p: &Proposal, mode: &Mode) -> Result<(Task, Prepared), String> {
    // 1. Under the lock: state checks, leftovers committed, boss's tree off the task branch.
    let t = {
        let _g = guard();
        let t = load(root, id).ok_or_else(|| format!("task {id} không tồn tại"))?;
        match t.status {
            Status::Cancelled => return Err("việc đã huỷ".into()),
            Status::Running | Status::Queued => return Err("việc đang chạy — chờ bước hiện tại xong".into()),
            _ => {}
        }
        if mode.accept {
            if t.paused {
                return Err("việc đang chờ Bắt đầu / Tiếp tục".into());
            }
            if (t.stage, t.status) != (Stage::Acceptance, Status::Waiting) {
                return Err("task không chờ nghiệm thu".into());
            }
        }
        if let (Some(br), true) = (t.branch.clone(), is_git(root)) {
            if !chat::branch_exists(root, &br) {
                return Err(format!("nhánh {br} không còn"));
            }
            if current_branch(root).as_deref() == Some(br.as_str()) {
                commit_leftovers(root, &t, "planning")?;
                if !clean(root) {
                    return Err(format!("còn file sửa dở ngoài phần /feature: {}", dirty_files(root)));
                }
                let base = t.base.clone().ok_or("việc không có nhánh gốc")?;
                if mode.accept {
                    restore_base_pointers(root, &t, &base)?;
                }
                git(root, &["switch", &base]).map_err(|e| format!("không về được nhánh {base}: {e}"))?;
            }
        }
        t
    };
    // 2. Without the lock: network + worktree.
    let prep = prepare_git(root, &t, p)?;
    // 3. Record the run.
    let _g = guard();
    let mut cur = load(root, id).ok_or_else(|| format!("task {id} không tồn tại"))?;
    let n = cur.runs.iter().filter(|r| r.stage == Stage::Action).count() as u32 + 1;
    let title = p.title.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Thực hiện".into());
    cur.runs.push(Run {
        stage: Stage::Action,
        n,
        started: iso_now(),
        ended: None,
        exit: None,
        cost: 0.0,
        verdict: None,
        log: format!("action-{n}.jsonl"),
        phase: None,
        round: (cur.round > 1).then_some(cur.round),
        title: Some(title.clone()),
    });
    cur.note = Some(format!("Đang thực hiện: {title}…"));
    if let Some(msg) = &mode.msg {
        if let Some(pp) = cur.chat.iter_mut().find(|m| &m.id == msg).and_then(|m| m.proposal.as_mut()) {
            pp.state = "running".into();
        }
    }
    save(root, &mut cur).map_err(|e| e.to_string())?;
    Ok((cur, prep))
}

/// Target, commits, `-mr` replay (pushed by the board), temp worktree, push guard.
fn prepare_git(root: &FsPath, t: &Task, p: &Proposal) -> Result<Prepared, String> {
    let Some(br) = t.branch.clone().filter(|_| is_git(root)) else {
        return Ok(Prepared { cwd: root.to_path_buf(), tmp: None, hooks: None, source: None, target: None, commits: Vec::new(), refs: None, head: None, push_blocked: false });
    };
    let head = git(root, &["rev-parse", &format!("refs/heads/{br}")]).map(|s| s.trim().to_string()).ok();
    let has_origin = git(root, &["config", "--get", "remote.origin.url"]).is_ok();
    let (mut source, mut target, mut commits) = (br.clone(), None, Vec::new());
    if p.pushes {
        if !has_origin {
            return Err("dự án không có remote origin".into());
        }
        let tg = p
            .target
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| t.mr.as_ref().map(|m| m.target.clone()))
            .or_else(|| chat::remote_default(root))
            .or_else(|| t.base.clone())
            .ok_or("chưa chọn nhánh đích")?;
        if tg == br {
            return Err("nhánh đích trùng nhánh của việc".into());
        }
        commits = chat::prepare(root, t, &tg)?;
        if commits.is_empty() {
            return Err(format!("{br} không có commit nào mới so với origin/{tg}"));
        }
        let mine: Vec<String> = commits.iter().filter(|c| c.from_task).map(|c| c.sha.clone()).collect();
        if mine.len() < commits.len() && p.push.as_deref() != Some("all") {
            if mine.is_empty() {
                return Err(format!("không có commit nào của việc trong {br} so với origin/{tg}"));
            }
            source = format!("{br}-mr");
            chat::replay(root, &tg, &mine, &source)?;
        }
        // The board pushes the MR's source itself, before the run: the agent never
        // needs to push (its pushes are blocked below). `-mr` is board-owned and
        // rebuilt by every replay: the only branch ever force-pushed.
        let spec = if source == br { format!("refs/heads/{br}:refs/heads/{br}") } else { format!("+refs/heads/{source}:refs/heads/{source}") };
        chat::run_cmd(root, "git", &["push", "origin", &spec], chat::PUSH_TIMEOUT).map_err(|e| format!("push {source} lỗi: {e}"))?;
        target = Some(tg);
    }
    let refs = if has_origin {
        match remote_refs(root) {
            Ok(r) => Some(r),
            Err(e) if p.pushes => return Err(format!("không đọc được origin: {e}")),
            Err(_) => None,
        }
    } else {
        None
    };
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = std::env::temp_dir().join(format!("ckit-action-{}-{}-{}", std::process::id(), t.id.to_lowercase(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let wt = tmp.join("wt");
    if let Err(e) = chat::run_cmd(root, "git", &["worktree", "add", &wt.display().to_string(), &source], chat::PUSH_TIMEOUT) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("không tạo được worktree tạm cho {source}: {e}"));
    }
    let hooks = tmp.join("hooks");
    install_hooks(root, &hooks)?;
    let push_blocked = block_pushes(root, &wt);
    Ok(Prepared { cwd: wt, tmp: Some(tmp), hooks: Some(hooks), source: Some(source), target, commits, refs, head, push_blocked })
}

/// URL every push from the action's worktree goes to: unusable, so `git push`
/// fails (with or without `--no-verify`) while `glab` / `gh` API calls still work.
pub(super) const BLOCKED_PUSH_URL: &str = "file:///dev/null/ckit-push-blocked";

/// Point the worktree's own `remote.origin.pushurl` at `BLOCKED_PUSH_URL`
/// (`git config --worktree`; needs `extensions.worktreeConfig`, a repo flag that
/// is switched on once and changes nothing else; the shared config gets no push
/// URL). True when that is origin's only push URL there: a repo that already
/// defines its own `remote.origin.pushurl` keeps pushing to it as well, so then
/// only the hook and the ls-remote check guard the run.
fn block_pushes(root: &FsPath, wt: &FsPath) -> bool {
    if git(root, &["config", "--bool", "extensions.worktreeConfig"]).map(|s| s.trim() != "true").unwrap_or(true) && git(root, &["config", "extensions.worktreeConfig", "true"]).is_err() {
        return false;
    }
    if git(wt, &["config", "--worktree", "remote.origin.pushurl", BLOCKED_PUSH_URL]).is_err() {
        return false;
    }
    git(wt, &["remote", "get-url", "--push", "--all", "origin"]).is_ok_and(|s| s.lines().all(|l| l.trim() == BLOCKED_PUSH_URL))
}

/// `refs/heads/*` and `refs/tags/*` on origin (server-made refs such as GitLab's
/// `refs/merge-requests/*` are not pushes).
fn remote_refs(root: &FsPath) -> Result<Vec<(String, String)>, String> {
    let out = chat::run_cmd(root, "git", &["ls-remote", "origin"], chat::PUSH_TIMEOUT)?;
    let mut v: Vec<(String, String)> = out
        .lines()
        .filter_map(|l| {
            let (sha, r) = l.split_once('\t')?;
            (r.starts_with("refs/heads/") || (r.starts_with("refs/tags/") && !r.ends_with("^{}"))).then(|| (r.to_string(), sha.to_string()))
        })
        .collect();
    v.sort();
    Ok(v)
}

/// Refs that changed between two `remote_refs` snapshots outside `refs/heads/board/`.
pub(super) fn foreign_ref_changes(before: &[(String, String)], after: &[(String, String)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (r, sha) in after {
        if before.iter().find(|(b, _)| b == r).map(|(_, s)| s) != Some(sha) && !r.starts_with("refs/heads/board/") {
            out.push(r.clone());
        }
    }
    for (r, _) in before {
        if !after.iter().any(|(a, _)| a == r) && !r.starts_with("refs/heads/board/") {
            out.push(format!("{r} (xoá)"));
        }
    }
    out
}

/// The run's hooks dir: a `pre-push` guard (only `refs/heads/board/*`; refusals
/// logged to `blocked.log`) chaining the project's own pre-push, and pass-through
/// wrappers for the project's other hooks (`core.hooksPath` replaces them all).
fn install_hooks(root: &FsPath, hooks: &FsPath) -> Result<(), String> {
    std::fs::create_dir_all(hooks).map_err(|e| e.to_string())?;
    let orig = git(root, &["rev-parse", "--path-format=absolute", "--git-path", "hooks"]).map(|s| s.trim().to_string()).unwrap_or_default();
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let blocked = hooks.join("blocked.log").display().to_string();
    let pre_push = format!(
        "#!/bin/sh\n# ckit board action: only board/* branches may be pushed.\ninput=$(cat)\nbad=0\nfor r in $(printf '%s\\n' \"$input\" | awk '{{print $3}}'); do\n  case \"$r\" in\n    refs/heads/board/*) ;;\n    *) echo \"ckit board: chặn push lên $r (chỉ được đẩy nhánh board/*)\" >&2; echo \"$r\" >> {blocked}; bad=1;;\n  esac\ndone\n[ $bad = 0 ] || exit 1\nif [ -x {orig}/pre-push ]; then printf '%s\\n' \"$input\" | {orig}/pre-push \"$@\" || exit $?; fi\nexit 0\n",
        blocked = q(&blocked),
        orig = q(&orig)
    );
    let write = |name: &str, body: &str| -> Result<(), String> {
        let p = hooks.join(name);
        std::fs::write(&p, body).map_err(|e| e.to_string())?;
        Command::new("chmod").arg("+x").arg(&p).status().map_err(|e| e.to_string()).map(|_| ())
    };
    write("pre-push", &pre_push)?;
    if let Ok(rd) = std::fs::read_dir(&orig) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "pre-push" || name.ends_with(".sample") || name.starts_with('.') {
                continue;
            }
            write(&name, &format!("#!/bin/sh\nexec {} \"$@\"\n", q(&e.path().display().to_string())))?;
        }
    }
    Ok(())
}

/// omp arguments of an action run, the prompt excluded: a normal session (full
/// tools, extensions, skills, MCP) in the temp worktree, continuing a copy of the
/// task's latest working session when there is one.
pub(super) fn action_args(cwd: &FsPath, session_dir: &FsPath, forked: bool) -> Vec<std::ffi::OsString> {
    let mut a: Vec<std::ffi::OsString> = vec!["--cwd".into(), cwd.into()];
    a.extend(["--mode", "json", "--approval-mode", "yolo", "--session-dir"].map(Into::into));
    a.push(session_dir.into());
    if forked {
        a.push("--continue".into());
    }
    a.extend(ModelConfig::load().class_flags(TaskClass::Code).into_iter().map(Into::into));
    a
}

fn prompt(t: &Task, p: &Proposal, prep: &Prepared, result: &FsPath) -> Option<String> {
    let none = "(none)".to_string();
    let commits = if prep.commits.is_empty() {
        none.clone()
    } else {
        prep.commits.iter().filter(|c| c.from_task || prep.source.as_deref() == t.branch.as_deref()).map(|c| format!("{} {}", c.short, c.subject)).collect::<Vec<_>>().join("\n")
    };
    let target = prep.target.as_ref().or(t.base.as_ref()).unwrap_or(&none);
    let target_ref = format!("origin/{target}");
    let push = if p.pushes {
        let src = prep.source.as_deref().unwrap_or("board/*");
        format!(
            "Board đã push nhánh `{src}` lên origin; không push, chỉ tạo MR/PR (source `{src}`, target `{target}`) và các bước khác của skill. Every `git push` from this checkout fails on purpose (the board does all pushing); never force-push, delete or merge branches.\n- So sánh với `{target_ref}` (env `TARGET_REF`), không dùng `{target}` local: the local `{target}` may hold unpushed commits that are not this task's. Wherever the skill says `git log {target}..HEAD` / `git diff {target}...HEAD`, use `{target_ref}` instead."
        )
    } else {
        "This action does not push: every `git push` from this checkout fails on purpose; never merge.".into()
    };
    Some(
        crate::assets::read("board/action.md")?
            .replace("{{ID}}", &t.id)
            .replace("{{TITLE}}", &t.title)
            .replace("{{ACTION}}", p.title.as_deref().unwrap_or("Thực hiện"))
            .replace("{{SKILL}}", p.skill.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("(none: pick it yourself)"))
            .replace("{{CWD}}", &prep.cwd.display().to_string())
            .replace("{{SOURCE}}", prep.source.as_ref().unwrap_or(&none))
            .replace("{{BRANCH}}", t.branch.as_ref().unwrap_or(&none))
            .replace("{{TARGET}}", target)
            .replace("{{PUSH}}", &push)
            .replace("{{COMMITS}}", &commits)
            .replace("{{WRITES}}", &if p.writes_external.is_empty() { none.clone() } else { p.writes_external.join("; ") })
            .replace("{{RESULT}}", &result.display().to_string())
            // Free text last: `{{…}}` inside it is never expanded.
            .replace("{{REQUEST}}", &clip(&t.request, 3000))
            .replace("{{INSTRUCTION}}", p.instruction.as_deref().unwrap_or("")),
    )
}

/// The first MR/PR link among the run's URLs (then its message).
pub(super) fn mr_url(urls: &[String], message: &str) -> Option<String> {
    let is_mr = |u: &str| {
        let after = |k: &str| u.split_once(k).is_some_and(|(_, n)| n.chars().next().is_some_and(|c| c.is_ascii_digit()));
        after("/merge_requests/") || after("/pull/")
    };
    urls.iter().find(|u| is_mr(u)).cloned().or_else(|| chat::find_url(message).filter(|u| is_mr(u)))
}

fn execute(root: &FsPath, id: &str, p: &Proposal, mode: &Mode, prep: Prepared, log_name: &str) {
    let Some(t) = load(root, id) else { return };
    let dir = task_dir(root, id);
    let logs = dir.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let log = logs.join(log_name);
    let result = result_file(&dir, log_name);
    if let Some(d) = result.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::remove_file(&result);
    // A copy of the task's latest working session: the agent remembers the work.
    let session = prep.tmp.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("ckit-action-{}-{id}", std::process::id()))).join("session");
    let _ = std::fs::create_dir_all(&session);
    let forked = chat::leader_session(&dir, &t).and_then(|s| Some(std::fs::copy(&s, session.join(s.file_name()?)).is_ok())).unwrap_or(false);
    let outcome = match prompt(&t, p, &prep, &result) {
        None => Err("asset board/action.md thiếu".to_string()),
        Some(pr) => spawn(&prep, &session, forked, &pr, &log, &result),
    };
    let cost = digest::at_run_end(&dir, log_name).cost;
    let res: Option<ActionResult> = std::fs::read_to_string(&result).ok().and_then(|s| serde_json::from_str(&s).ok());
    // Rails: refused pushes (hook) and ref changes outside board/* (ls-remote).
    let mut violations: Vec<String> = prep.hooks.as_ref().and_then(|h| std::fs::read_to_string(h.join("blocked.log")).ok()).map(|s| s.lines().map(|l| format!("{l} (bị chặn)")).collect()).unwrap_or_default();
    if let Some(before) = &prep.refs {
        match remote_refs(root) {
            Ok(after) => violations.extend(foreign_ref_changes(before, &after)),
            Err(e) => violations.push(format!("không kiểm tra được origin sau khi chạy: {e}")),
        }
    }
    let ok = outcome.is_ok() && violations.is_empty() && res.as_ref().is_some_and(|r| r.verdict.trim() == "pass");
    let message = res.as_ref().map(|r| r.message.trim().to_string()).unwrap_or_default();
    let urls = res.as_ref().map(|r| r.urls.clone()).unwrap_or_default();
    let url = if violations.is_empty() { mr_url(&urls, &message) } else { None };
    // Keep the worktree of a failed run for inspection.
    let kept = match (&prep.tmp, ok) {
        (Some(tmp), true) => {
            let _ = chat::run_cmd(root, "git", &["worktree", "remove", "--force", &prep.cwd.display().to_string()], chat::PUSH_TIMEOUT);
            let _ = std::fs::remove_dir_all(tmp);
            let _ = chat::run_cmd(root, "git", &["worktree", "prune"], chat::PUSH_TIMEOUT);
            None
        }
        (Some(_), false) => Some(prep.cwd.display().to_string()),
        (None, _) => {
            let _ = std::fs::remove_dir_all(session.parent().unwrap_or(&session));
            None
        }
    };
    let title = p.title.clone().unwrap_or_else(|| "Thực hiện".into());
    let error = match (&outcome, &res) {
        _ if !violations.is_empty() => Some(format!("đẩy lên nhánh ngoài board/*: {}", violations.join(", "))),
        (Err(e), _) => Some(e.clone()),
        (Ok(_), None) => Some(format!("agent không ghi file kết quả ({})", result.display())),
        (Ok(_), Some(r)) if r.verdict.trim() != "pass" => Some(if message.is_empty() { "agent báo không làm được".into() } else { message.clone() }),
        _ => None,
    };
    let _g = guard();
    let Some(mut cur) = load(root, id) else { return };
    if let Some(r) = cur.runs.iter_mut().rev().find(|r| r.log == log_name) {
        r.ended = Some(iso_now());
        r.exit = outcome.as_ref().ok().copied().flatten();
        r.cost = cost;
        r.verdict = Some(if !violations.is_empty() { "blocked".into() } else if ok { "pass".into() } else { "fail".into() });
    }
    if cur.note.as_deref().is_some_and(|n| n.starts_with("Đang thực hiện")) {
        cur.note = None;
    }
    let mut line = format!("Thực hiện: {title}: {}", if message.is_empty() { error.clone().unwrap_or_default() } else { message.clone() });
    if let Some(e) = error.as_ref().filter(|e| !message.is_empty() && **e != message) {
        line.push_str(&format!("\n⚠️ {e}"));
    }
    if prep.tmp.is_some() && !prep.push_blocked {
        line.push_str("\n(push từ worktree chưa chặn được trước: repo có remote.origin.pushurl riêng; chỉ hook + kiểm tra ls-remote canh)");
    }
    if let Some(k) = &kept {
        line.push_str(&format!("\n(worktree giữ lại để xem: {k})"));
    }
    cur.say("leader", &line);
    if let Some(u) = url.clone().filter(|_| ok) {
        let provider = chat::origin(root).map(|o| o.0.to_string()).unwrap_or_else(|| "gitlab".into());
        let target = prep.target.clone().or_else(|| cur.base.clone()).unwrap_or_default();
        chat::close_mr_proposals(&mut cur, &u);
        cur.mr = Some(Mr { url: u, provider, target, at: iso_now(), source: prep.source.clone().unwrap_or_default(), head: prep.head.clone() });
    }
    let told = if mode.accept {
        match url.clone().filter(|_| ok) {
            Some(u) if (cur.stage, cur.status) == (Stage::Acceptance, Status::Waiting) => {
                cur.say("user", "Nghiệm thu.");
                cur.say("system", &format!("Nghiệm thu: đã tạo MR {u}"));
                cur.set(Stage::Done, Status::Done);
                leave_task_branch(root, &cur);
                format!("✅ {} đã nghiệm thu: MR {u}", cur.id)
            }
            _ => {
                let why = error.clone().unwrap_or_else(|| if message.is_empty() { "agent không trả về link MR".into() } else { format!("agent không trả về link MR: {message}") });
                cur.say("system", &format!("Nghiệm thu chưa xong (chưa có MR): {why}"));
                format!("⚠️ {}: nghiệm thu chưa xong (chưa có MR): {why}", cur.id)
            }
        }
    } else {
        format!("{} {}: {title}: {}", if ok { "✅" } else { "⚠️" }, cur.id, if message.is_empty() { error.clone().unwrap_or_default() } else { message.clone() })
    };
    if let Some(msg) = &mode.msg {
        if let Some(pp) = cur.chat.iter_mut().find(|m| &m.id == msg).and_then(|m| m.proposal.as_mut()) {
            pp.state = if ok { "done".into() } else { "failed".into() };
            pp.result = Some(ProposalResult { url: url.clone(), urls: urls.clone(), message: (!message.is_empty()).then(|| message.clone()), error: error.clone() });
        }
    }
    cur.recost(root);
    let _ = save(root, &mut cur);
    if mode.via.as_deref() == Some("telegram") {
        telegram::tell(&clip(&told, 3500));
    }
}

/// Run omp for the action in the prepared directory, push guard in its environment.
fn spawn(prep: &Prepared, session: &FsPath, forked: bool, prompt: &str, log: &FsPath, result: &FsPath) -> Result<Option<i32>, String> {
    let f = std::fs::File::create(log).map_err(|e| format!("log: {e}"))?;
    let err = f.try_clone().map_err(|e| format!("log: {e}"))?;
    let mut cmd = omp_cmd();
    cmd.current_dir(&prep.cwd)
        .args(action_args(&prep.cwd, session, forked))
        .env("CKIT_BOARD_STAGE", "action")
        .env("CKIT_BOARD_RESULT", result)
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(tg) = &prep.target {
        // Skills diff against this, not the local `<target>` (ahead with the boss's commits).
        cmd.env("TARGET_REF", format!("origin/{tg}"));
    }
    if let Some(h) = &prep.hooks {
        // Only this run's git processes see the guard; the repo config is untouched.
        cmd.env("GIT_CONFIG_COUNT", "1").env("GIT_CONFIG_KEY_0", "core.hooksPath").env("GIT_CONFIG_VALUE_0", h);
    }
    let mut child = cmd.arg("-p").arg(prompt).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(err)).spawn().map_err(|e| format!("không chạy được omp: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) => return Ok(s.code()),
            Ok(None) if started.elapsed() > ACTION_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("quá thời gian {} phút", ACTION_TIMEOUT.as_secs() / 60));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(e) => return Err(e.to_string()),
        }
    }
}
