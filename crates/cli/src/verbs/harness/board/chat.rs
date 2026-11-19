//! "Trao đổi với Leader": the boss chats with the task's Leader on every task page,
//! done tasks included (Telegram replies to a done task land here too).
//!
//! The Leader is a read-only fork of the task's latest finished working session
//! (Dev / workflow run / Test / Kế hoạch): the session file is copied to a temp dir
//! and continued with read tools only, so it answers from what it did and the
//! original session is never touched. Asked for an action it only proposes one
//! (`{"propose": {...}}`); the boss confirms in the dashboard and the board runs it:
//! - `action`: anything to do ("Tạo merge request", update the PMS ticket, …): a
//!   normal omp run that picks the project's own skill (see `action.rs`); the
//!   Leader names the skill it expects, whether it pushes, and the external writes.
//!   At the acceptance gate a pushing action IS the acceptance (Done on an MR URL);
//! - `revise`: a new round ("Vòng N") on the same branch: Dev → Test → Report →
//!   Acceptance (workflow tasks: run → acceptance), Dev continuing its session.
//! The Leader runs with the project's skills listed (still read-only tools) so it
//! can name the right one.
//!
//! Chat turns are serialized per task; logs and cost under `<task>/chat/`.
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Mutex, OnceLock};

use super::*;

const TURN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub(super) const PUSH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct ChatMsg {
    pub id: String,
    /// boss | leader | system
    pub from: String,
    pub text: String,
    pub at: String,
    #[serde(default)]
    pub cost: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<Proposal>,
    /// `telegram` when the boss wrote from Telegram: the answer goes back there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(super) struct Proposal {
    /// action | revise
    pub kind: String,
    /// action: target branch on origin when it pushes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// action: short name ("Tạo merge request").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// action: the skill/command the Leader expects the agent to use (`gitlab-mr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// action: it pushes a branch (git rails + commit list apply).
    #[serde(default)]
    pub pushes: bool,
    /// action: writes outside the repo the boss should see first ("PMS ticket #… ").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes_external: Vec<String>,
    /// open | running | done | cancelled | failed
    #[serde(default)]
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ProposalResult>,
    /// merge_request: commits `origin/<target>..<branch>`, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<CommitInfo>,
    /// merge_request: those not made by this task (e.g. the boss's unpushed base commits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foreign: Vec<CommitInfo>,
    /// merge_request: branches on origin (target choices).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_default: Option<String>,
    /// merge_request: why it cannot run as proposed (target missing on origin…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_error: Option<String>,
    /// merge_request: `task` (default) = only the task's commits, replayed on
    /// `origin/<target>` as `<branch>-mr`; `all` = push the task branch as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(super) struct CommitInfo {
    pub sha: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub at: String,
    /// Made by this task's runs (not reachable from its base at task start).
    pub from_task: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(super) struct ProposalResult {
    /// The MR/PR link among `urls`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    /// The agent's report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The merge/pull request opened for the task branch.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Mr {
    pub url: String,
    /// gitlab | github
    pub provider: String,
    pub target: String,
    pub at: String,
    /// Branch the MR/PR is from (`board/t-007`, or `board/t-007-mr` when only the
    /// task's commits were pushed).
    #[serde(default)]
    pub source: String,
    /// The task branch's commit the MR's branch was last pushed from; a later round
    /// moves the branch and the next MR confirm / acceptance updates the MR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

fn busy_set() -> &'static Mutex<HashSet<String>> {
    static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn key(root: &FsPath, id: &str) -> String {
    format!("{}|{id}", root.display())
}

pub(super) fn busy(root: &FsPath, id: &str) -> bool {
    busy_set().lock().map(|s| s.contains(&key(root, id))).unwrap_or(false)
}

/// One chat turn or action at a time per task.
pub(super) fn claim(root: &FsPath, id: &str) -> bool {
    busy_set().lock().map(|mut s| s.insert(key(root, id))).unwrap_or(false)
}

pub(super) fn release(root: &FsPath, id: &str) {
    busy_set().lock().map(|mut s| s.remove(&key(root, id))).ok();
}

fn next_id(t: &Task) -> String {
    format!("C-{}", t.chat.iter().filter_map(|m| m.id.strip_prefix("C-")?.parse::<u32>().ok()).max().unwrap_or(0) + 1)
}

fn push_msg(t: &mut Task, from: &str, text: &str, cost: f64, proposal: Option<Proposal>, via: Option<String>) -> String {
    let id = next_id(t);
    t.chat.push(ChatMsg { id: id.clone(), from: from.into(), text: text.trim().into(), at: iso_now(), cost, proposal, via });
    id
}

// ── a chat turn ─────────────────────────────────────────────────────────────

/// The boss writes to the Leader: recorded now, answered in the background (one
/// turn at a time per task). Caller holds the board lock and saves.
pub(super) fn post(root: &FsPath, t: &mut Task, text: &str, via: Option<&str>) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("nội dung trống".into());
    }
    if t.status == Status::Cancelled {
        return Err("việc đã huỷ, hãy tạo task mới".into());
    }
    if !claim(root, &t.id) {
        return Err("Leader đang trả lời tin trước (hoặc một thao tác đang chạy), chờ chút rồi gửi tiếp".into());
    }
    let msg = push_msg(t, "boss", text, 0.0, None, via.map(String::from));
    let (root, id) = (root.to_path_buf(), t.id.clone());
    std::thread::spawn(move || {
        answer(&root, &id, &msg);
        release(&root, &id);
    });
    Ok(())
}

/// Session the Leader continues: the latest finished Dev / Test / Kế hoạch run's.
pub(super) fn leader_session(dir: &FsPath, t: &Task) -> Option<PathBuf> {
    t.runs.iter().rev().filter(|r| matches!(r.stage, Stage::Dev | Stage::Test | Stage::Solution) && r.ended.is_some()).find_map(|r| ask::run_session(dir, r))
}

fn answer(root: &FsPath, id: &str, msg: &str) {
    // The poster's lock is released only after it saved the boss's message.
    let Some(t) = ({
        let _g = guard();
        load(root, id)
    }) else {
        return;
    };
    let dir = task_dir(root, id);
    let _ = std::fs::create_dir_all(dir.join("chat"));
    let log = dir.join("chat").join(format!("{msg}.jsonl"));
    let fork = leader_session(&dir, &t).and_then(|p| ask::fork_session(&p, id, msg));
    let reply = prompt(root, &t, msg, fork.is_some()).and_then(|p| run_turn(root, fork.as_ref().map(|f| f.0.as_path()), &p, &log));
    drop(fork);
    let cost = if log.exists() { log_cost(&log) } else { 0.0 };
    // Parse and check a proposal before taking the lock: checking reads origin.
    let reply = reply.map(|text| {
        let (text, proposal) = split_proposal(&text);
        let proposal = proposal.map(|p| {
            let mut p = fill(&t, p);
            if p.kind == "action" && p.pushes {
                check(root, &t, &mut p);
            }
            p
        });
        (text, proposal)
    });
    let _g = guard();
    let Some(mut cur) = load(root, id) else { return };
    let via = cur.chat.iter().find(|m| m.id == msg).and_then(|m| m.via.clone());
    let told = match reply {
        Some((text, proposal)) => {
            let shown = if text.is_empty() { proposal_line(proposal.as_ref()) } else { text };
            push_msg(&mut cur, "leader", &shown, cost, proposal, None);
            shown
        }
        None => {
            let e = "Leader không trả lời được (lỗi model hoặc quá thời gian). Gửi lại sau.".to_string();
            push_msg(&mut cur, "system", &e, cost, None, None);
            e
        }
    };
    cur.recost(root);
    let _ = save(root, &mut cur);
    if via.as_deref() == Some("telegram") {
        telegram::tell(&format!("💬 {} · Leader: {}", cur.id, clip(&told, 3500)));
    }
}

fn proposal_line(p: Option<&Proposal>) -> String {
    match p {
        Some(p) if p.kind == "action" => format!("Đề xuất: {}.", p.title.as_deref().unwrap_or("thực hiện")),
        Some(p) if p.kind == "revise" => "Đề xuất sửa thêm (vòng mới).".into(),
        _ => String::new(),
    }
}

/// Older kinds: `merge_request` / `accept` are the action "create the MR" (the
/// agent picks the project's skill for it).
pub(super) fn normalize(p: &mut Proposal) {
    if matches!(p.kind.as_str(), "merge_request" | "accept") {
        p.kind = "action".into();
        p.pushes = true;
        if p.title.as_deref().is_none_or(|s| s.trim().is_empty()) || p.title.as_deref().is_some_and(|s| !s.starts_with("Tạo")) {
            p.title = Some("Tạo merge request".into());
        }
        if p.instruction.as_deref().is_none_or(|s| s.trim().is_empty()) {
            p.instruction = Some(action::mr_instruction(p.target.as_deref()));
        }
    }
}

fn prompt(root: &FsPath, t: &Task, msg: &str, forked: bool) -> Option<String> {
    let tpl = crate::assets::read("board/chat.md")?;
    let who = if forked {
        "You are the Leader of this task: the conversation above is the team's own latest working session on it. That work is over; you now talk with the boss about the task."
    } else {
        "You are the Leader (team lead) of this task in an AI delivery team."
    };
    let dir = task_dir(root, &t.id);
    let none = "(none)".to_string();
    let message = t.chat.iter().find(|m| m.id == msg).map(|m| m.text.clone()).unwrap_or_default();
    let before: Vec<&ChatMsg> = t.chat.iter().take_while(|m| m.id != msg).collect();
    let earlier: Vec<String> = before[before.len().saturating_sub(12)..].iter().map(|m| format!("[{}] {}", m.from, m.text)).collect();
    let commits = timeline::task_commits(root, t).iter().map(|c| format!("{} {}", c.sha, c.subject)).collect::<Vec<_>>().join("\n");
    let report = std::fs::read_to_string(dir.join("REPORT.md")).map(|r| clip(&r, 6000)).unwrap_or_default();
    let mr = t.mr.as_ref().map(|m| format!("{} ({} → {})", m.url, m.provider, m.target)).unwrap_or_else(|| none.clone());
    let acceptable = if (t.stage, t.status) == (Stage::Acceptance, Status::Waiting) { "it does now" } else { "it does not now" };
    Some(
        tpl.replace("{{WHO}}", who)
            .replace("{{ID}}", &t.id)
            .replace("{{TITLE}}", &t.title)
            .replace("{{STATUS}}", &format!("{:?}", t.status).to_lowercase())
            .replace("{{STAGE}}", t.stage.key())
            .replace("{{ROUND}}", &t.round.to_string())
            .replace("{{BRANCH}}", t.branch.as_ref().unwrap_or(&none))
            .replace("{{BASE}}", t.base.as_ref().unwrap_or(&none))
            .replace("{{MR}}", &mr)
            .replace("{{ACCEPTABLE}}", acceptable)
            .replace("{{FEATURE_DIR}}", &feature_dir(root, t).map(|p| p.display().to_string()).unwrap_or_else(|| none.clone()))
            .replace("{{DIR}}", &dir.display().to_string())
            .replace("{{COMMITS}}", if commits.is_empty() { "(none)" } else { &commits })
            .replace("{{THREAD}}", &ask::thread_context(&t.thread))
            // Free text last: `{{…}}` inside it is never expanded.
            .replace("{{CHAT}}", &if earlier.is_empty() { none.clone() } else { earlier.join("\n\n") })
            .replace("{{REPORT}}", &report)
            .replace("{{REQUEST}}", &t.request)
            .replace("{{MESSAGE}}", &message),
    )
}

fn run_turn(root: &FsPath, fork: Option<&FsPath>, prompt: &str, log: &FsPath) -> Option<String> {
    let f = std::fs::File::create(log).ok()?;
    let err = f.try_clone().ok()?;
    let mut child = omp_cmd().current_dir(root).args(ask::answerer_args(root, fork, true)).arg("-p").arg(prompt).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(err)).spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > TURN_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(_) => return None,
        }
    }
    pm::answer_text(&std::fs::read_to_string(log).ok()?)
}

/// The Leader's reply → (text without the proposal JSON, proposal). The proposal is
/// the last JSON object holding a `"propose"` key (fenced or not).
pub(super) fn split_proposal(text: &str) -> (String, Option<Proposal>) {
    let Some(k) = text.rfind("\"propose\"") else { return (text.trim().to_string(), None) };
    let Some(start) = text[..k].rfind('{') else { return (text.trim().to_string(), None) };
    // Matching close brace, strings respected.
    let (mut depth, mut in_str, mut esc, mut end) = (0i32, false, false, None);
    for (i, c) in text[start..].char_indices() {
        match c {
            _ if esc => esc = false,
            '\\' if in_str => esc = true,
            '"' => in_str = !in_str,
            '{' if !in_str => depth += 1,
            '}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else { return (text.trim().to_string(), None) };
    let v: Option<serde_json::Value> = serde_json::from_str(&text[start..end]).ok();
    let Some(mut p) = v.and_then(|v| v.get("propose").cloned()).and_then(|p| serde_json::from_value::<Proposal>(p).ok()) else {
        return (text.trim().to_string(), None);
    };
    normalize(&mut p);
    if !matches!(p.kind.as_str(), "action" | "revise") {
        return (text.trim().to_string(), None);
    }
    p.state = "open".into();
    p.result = None;
    let mut rest = format!("{}{}", &text[..start], &text[end..]);
    for fence in ["```json", "```"] {
        rest = rest.replace(fence, "");
    }
    (rest.trim().to_string(), Some(p))
}

/// Defaults the Leader may have left out.
fn fill(t: &Task, mut p: Proposal) -> Proposal {
    let empty = |s: &Option<String>| s.as_deref().is_none_or(|s| s.trim().is_empty());
    if p.kind == "action" {
        if empty(&p.instruction) {
            p.instruction = t.chat.iter().rev().find(|m| m.from == "boss").map(|m| m.text.clone());
        }
        if empty(&p.title) {
            p.title = Some(clip(p.instruction.as_deref().unwrap_or("Thực hiện"), 60));
        }
        if p.skill.as_deref().is_some_and(|s| s.trim().is_empty() || s.trim() == "none") {
            p.skill = None;
        }
    }
    if p.kind == "revise" && p.instruction.as_deref().is_none_or(|s| s.trim().is_empty()) {
        p.instruction = t.chat.iter().rev().find(|m| m.from == "boss").map(|m| m.text.clone());
    }
    p
}

// ── API helpers ─────────────────────────────────────────────────────────────

/// The chat for the API: action proposals that push carry the remote facts the
/// card shows (provider, host, source branch, target choices, commits).
pub(super) fn view(root: &FsPath, t: &Task) -> serde_json::Value {
    let remote = origin(root);
    let messages: Vec<serde_json::Value> = t
        .chat
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(m).unwrap_or_default();
            if let Some(p) = &m.proposal {
                let mut p = p.clone();
                normalize(&mut p);
                v["proposal"] = if p.kind == "action" && p.pushes { proposal_json(t, &p, remote.as_ref()) } else { serde_json::to_value(&p).unwrap_or_default() };
            }
            v
        })
        .collect();
    serde_json::json!({ "messages": messages, "busy": busy(root, &t.id), "mr": t.mr })
}

/// A pushing action for the API, with the remote facts the card shows.
pub(super) fn proposal_json(t: &Task, p: &Proposal, remote: Option<&(&'static str, String)>) -> serde_json::Value {
    let mut v = serde_json::to_value(p).unwrap_or_default();
    v["source"] = serde_json::json!(t.branch);
    v["provider"] = serde_json::json!(remote.map(|r| r.0));
    v["host"] = serde_json::json!(remote.map(|r| r.1.clone()));
    for k in ["commits", "foreign", "targets", "writes_external"] {
        if v.get(k).is_none() {
            v[k] = serde_json::json!([]);
        }
    }
    v
}

/// The acceptance card (`GET /mr/draft`): the action "create the MR", remote facts
/// checked; the agent's skill writes the MR itself.
pub(super) fn draft(root: &FsPath, t: &Task) -> serde_json::Value {
    let mut p = action::mr_proposal(t);
    check(root, t, &mut p);
    p.instruction = Some(action::mr_instruction(p.target.as_deref()));
    let mut v = proposal_json(t, &p, origin(root).as_ref());
    v["mr"] = serde_json::json!(t.mr);
    v
}

/// Branches on origin (board/* excluded).
fn remote_heads(root: &FsPath) -> Result<Vec<String>, String> {
    let out = run_cmd(root, "git", &["ls-remote", "--heads", "origin"], PUSH_TIMEOUT)?;
    Ok(out.lines().filter_map(|l| l.split('\t').nth(1)?.strip_prefix("refs/heads/").map(str::to_string)).filter(|b| !b.starts_with("board/")).collect())
}

/// origin's default branch (its HEAD).
pub(super) fn remote_default(root: &FsPath) -> Option<String> {
    let out = run_cmd(root, "git", &["ls-remote", "--symref", "origin", "HEAD"], PUSH_TIMEOUT).ok()?;
    out.lines().find_map(|l| l.strip_prefix("ref: refs/heads/")?.split('\t').next().map(str::to_string))
}

/// The task's base commit at task start: recorded when the branch was created;
/// for older tasks the merge-base of the branch and the base branch as it was when
/// the first run started.
fn task_base_sha(root: &FsPath, t: &Task) -> Option<String> {
    if let Some(s) = &t.base_sha {
        return Some(s.clone());
    }
    let (br, base) = (t.branch.as_ref()?, t.base.as_ref()?);
    let first = t.runs.iter().find(|r| r.stage != Stage::Investigate)?;
    let then = git(root, &["rev-list", "-1", &format!("--before={}", first.started), base]).ok().filter(|s| !s.trim().is_empty())?;
    git(root, &["merge-base", br, then.trim()]).ok().map(|s| s.trim().to_string())
}

fn is_ancestor(root: &FsPath, a: &str, b: &str) -> bool {
    Command::new("git").current_dir(root).args(["merge-base", "--is-ancestor", a, b]).status().is_ok_and(|s| s.success())
}

/// Commits `origin/<target>..<branch>` (oldest first), each marked whether this
/// task made it: not reachable from the task's base, and committed during a run
/// or by the board for it (`docs: board <id> …` leftovers / pointer restore).
pub(super) fn mr_commits(root: &FsPath, t: &Task, target: &str) -> Vec<CommitInfo> {
    let Some(br) = &t.branch else { return Vec::new() };
    let out = git(root, &["log", "--no-merges", "--reverse", "--format=%H%x09%h%x09%an%x09%ct%x09%s", &format!("refs/remotes/origin/{target}..refs/heads/{br}")]).unwrap_or_default();
    let base = task_base_sha(root, t);
    let windows: Vec<(i64, i64)> = t
        .runs
        .iter()
        .filter_map(|r| {
            let s = pm::iso_secs(&r.started)?;
            let e = r.ended.as_deref().and_then(pm::iso_secs).unwrap_or(i64::MAX / 2);
            Some((s, e + 60))
        })
        .collect();
    out.lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.splitn(5, '\t').collect();
            let (sha, short, author, ct, subject) = (p.first()?, p.get(1)?, p.get(2)?, p.get(3)?.parse::<i64>().ok()?, p.get(4).copied().unwrap_or(""));
            let in_base = base.as_deref().is_some_and(|b| is_ancestor(root, sha, b));
            let in_run = windows.iter().any(|(s, e)| ct >= *s && ct <= *e) || subject.starts_with(&format!("docs: board {} ", t.id));
            Some(CommitInfo {
                sha: sha.to_string(),
                short: short.to_string(),
                subject: subject.to_string(),
                author: author.to_string(),
                at: iso_at(UNIX_EPOCH + Duration::from_secs(ct.max(0) as u64)),
                from_task: !in_base && in_run,
            })
        })
        .collect()
}

/// The target exists on origin (refuse before any push), fetched; then the commits
/// an MR from the task branch would carry.
pub(super) fn prepare(root: &FsPath, t: &Task, target: &str) -> Result<Vec<CommitInfo>, String> {
    if target.trim().is_empty() {
        return Err("chưa chọn nhánh đích".into());
    }
    let heads = run_cmd(root, "git", &["ls-remote", "--heads", "origin", target], PUSH_TIMEOUT).map_err(|e| format!("không đọc được origin: {e}"))?;
    if !heads.lines().any(|l| l.ends_with(&format!("refs/heads/{target}"))) {
        return Err(format!("nhánh đích {target} không có trên origin"));
    }
    run_cmd(root, "git", &["fetch", "--no-tags", "origin", &format!("+refs/heads/{target}:refs/remotes/origin/{target}")], PUSH_TIMEOUT).map_err(|e| format!("fetch origin/{target} lỗi: {e}"))?;
    Ok(mr_commits(root, t, target))
}

/// Fill a merge_request proposal with the remote facts: target choices, origin's
/// default branch (the default target), the commits it would carry, the foreign ones.
pub(super) fn check(root: &FsPath, t: &Task, p: &mut Proposal) {
    p.check_error = None;
    match remote_heads(root) {
        Ok(h) => p.targets = h,
        Err(e) => {
            p.check_error = Some(format!("không đọc được nhánh trên origin: {e}"));
            return;
        }
    }
    p.remote_default = remote_default(root);
    if p.target.as_deref().is_none_or(|s| s.trim().is_empty()) {
        p.target = p.remote_default.clone().or_else(|| t.base.clone());
    }
    match prepare(root, t, p.target.as_deref().unwrap_or("")) {
        Ok(c) => {
            p.foreign = c.iter().filter(|c| !c.from_task).cloned().collect();
            p.commits = c;
        }
        Err(e) => p.check_error = Some(e),
    }
}

/// `origin` → (provider, host): github.com → github, any other host → gitlab.
pub(super) fn origin(root: &FsPath) -> Option<(&'static str, String)> {
    // The configured URL (an `insteadOf` rewrite is transport, not the provider).
    let url = git(root, &["config", "--get", "remote.origin.url"]).ok()?;
    let host = remote_host(&url)?;
    Some((if host == "github.com" { "github" } else { "gitlab" }, host))
}

/// `https://user@host:8080/a/b.git` / `git@host:a/b.git` / `ssh://git@host/a` → host.
pub(super) fn remote_host(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = match u.split_once("://") {
        Some((_, r)) => r,
        None if u.contains('@') && u.contains(':') => u,
        None => return None, // a local path: no provider
    };
    let rest = rest.rsplit_once('@').map(|(_, h)| h).unwrap_or(rest);
    let host = rest.split(['/', ':']).next()?.to_string();
    (!host.is_empty()).then_some(host)
}

/// Run a program in the project with a hard timeout; Ok(stdout+stderr) on exit 0.
pub(super) fn run_cmd(root: &FsPath, prog: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(prog)
        .current_dir(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GLAB_NO_PROMPT", "1")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("không chạy được {prog}: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{prog} quá {} giây", timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut out);
    }
    if status.success() { Ok(out) } else { Err(format!("{prog} {}: {}", args.first().unwrap_or(&""), clip(out.trim(), 400))) }
}

/// The MR/PR URL in a CLI's output: `web_url` / `url` of JSON, else the first link
/// that looks like an MR/PR, else the first link.
pub(super) fn find_url(out: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(out.trim()) {
        if let Some(u) = v.get("web_url").or_else(|| v.get("url")).and_then(|u| u.as_str()) {
            return Some(u.to_string());
        }
    }
    let links: Vec<String> = out
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')' | ','))
        .filter(|w| w.starts_with("https://") || w.starts_with("http://"))
        .map(|w| w.trim_end_matches(['.', ';']).to_string())
        .collect();
    links.iter().find(|l| l.contains("/merge_requests/") || l.contains("/pull/")).or(links.first()).cloned()
}

pub(super) fn branch_exists(root: &FsPath, br: &str) -> bool {
    git(root, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{br}")]).is_ok()
}

/// Replay `shas` onto `origin/<target>` in a temporary worktree (the boss's working
/// tree is never touched) and point `refs/heads/<branch>` at the result.
pub(super) fn replay(root: &FsPath, target: &str, shas: &[String], branch: &str) -> Result<(), String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let wt = std::env::temp_dir().join(format!("ckit-mr-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wt);
    let wts = wt.display().to_string();
    run_cmd(root, "git", &["worktree", "add", "--detach", &wts, &format!("refs/remotes/origin/{target}")], PUSH_TIMEOUT).map_err(|e| format!("không tạo được worktree tạm: {e}"))?;
    let picked = (|| {
        let mut args: Vec<&str> = vec!["cherry-pick", "--allow-empty"];
        args.extend(shas.iter().map(String::as_str));
        run_cmd(&wt, "git", &args, PUSH_TIMEOUT).map_err(|e| {
            let _ = run_cmd(&wt, "git", &["cherry-pick", "--abort"], PUSH_TIMEOUT);
            format!("cherry-pick commit của task lên origin/{target} bị xung đột ({}). Chọn \"Đẩy cả các commit đó\" hoặc gỡ xung đột bằng tay.", clip(&e, 200))
        })?;
        git(&wt, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string())
    })();
    let _ = run_cmd(root, "git", &["worktree", "remove", "--force", &wts], PUSH_TIMEOUT);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = run_cmd(root, "git", &["worktree", "prune"], PUSH_TIMEOUT);
    let head = picked?;
    git(root, &["update-ref", &format!("refs/heads/{branch}"), &head]).map(|_| ())
}

/// A new round on the task's branch, Dev first. Caller holds the board lock.
pub(super) fn revise(root: &FsPath, t: &mut Task, instruction: &str) -> Result<(), String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err("chưa có nội dung cần sửa".into());
    }
    let done_or_accepting = t.status == Status::Done || (t.stage, t.status) == (Stage::Acceptance, Status::Waiting);
    if !done_or_accepting {
        return Err("chỉ mở vòng sửa mới khi việc đã xong hoặc đang chờ nghiệm thu".into());
    }
    let Some(br) = t.branch.clone() else { return Err("việc không có nhánh git".into()) };
    // Nothing merges locally: the branch (and its MR) carries the new round.
    if is_git(root) && !branch_exists(root, &br) {
        return Err(format!("nhánh {br} không còn"));
    }
    if let Some(h) = list(root).into_iter().find(|o| o.id != t.id && o.holds_tree()) {
        return Err(format!("{} đang giữ cây làm việc — chờ việc đó xong", h.id));
    }
    let mut steps = t.steps.clone();
    let add: &[Stage] = if t.workflow.is_some() { &[Stage::Dev, Stage::Acceptance] } else { &[Stage::Dev, Stage::Test, Stage::Report, Stage::Acceptance] };
    for s in add {
        if !steps.contains(s) {
            steps.push(*s);
        }
    }
    t.steps = validate_steps(&steps)?;
    t.round += 1;
    t.revise = Some(instruction.to_string());
    t.paused = false;
    t.resume = false;
    t.note = None;
    t.set(Stage::Dev, Status::Queued);
    t.say("system", &format!("Vòng {}: {instruction}", t.round));
    Ok(())
}

/// Close an open proposal without running it.
pub(super) fn cancel(t: &mut Task, msg: &str) -> Result<(), String> {
    let p = open_proposal(t, msg)?;
    p.state = "cancelled".into();
    Ok(())
}

pub(super) fn open_proposal<'a>(t: &'a mut Task, msg: &str) -> Result<&'a mut Proposal, String> {
    let m = t.chat.iter_mut().find(|m| m.id == msg).ok_or_else(|| format!("không có tin {msg}"))?;
    let p = m.proposal.as_mut().ok_or("tin này không có đề xuất")?;
    if p.state != "open" {
        return Err(format!("đề xuất đã {}", p.state));
    }
    Ok(p)
}

/// Boss's edits of a proposal before confirming (also the acceptance body).
#[derive(Deserialize, Default)]
pub(super) struct Fields {
    target: Option<String>,
    title: Option<String>,
    instruction: Option<String>,
    /// pushing action: `task` | `all`.
    push: Option<String>,
    /// pushing action: `true` = `push: "all"`, `false` = `push: "task"`.
    include_foreign: Option<bool>,
}

impl Fields {
    /// Any choice given (an empty acceptance body gets the draft back).
    pub(super) fn has_choice(&self) -> bool {
        self.target.is_some() || self.title.is_some() || self.instruction.is_some() || self.push.is_some() || self.include_foreign.is_some()
    }
}

pub(super) fn apply_fields(p: &mut Proposal, f: Fields) {
    let set = |dst: &mut Option<String>, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.trim().is_empty()) {
            *dst = Some(v);
        }
    };
    set(&mut p.target, f.target);
    set(&mut p.title, f.title);
    set(&mut p.instruction, f.instruction);
    set(&mut p.push, f.push.filter(|v| v == "task" || v == "all"));
    if let Some(all) = f.include_foreign {
        p.push = Some(if all { "all" } else { "task" }.into());
    }
}

/// The task got its MR: open MR proposals (pushing actions, older merge_request /
/// accept kinds) are settled with that link so no second MR can be confirmed.
pub(super) fn close_mr_proposals(t: &mut Task, url: &str) {
    for p in t.chat.iter_mut().filter_map(|m| m.proposal.as_mut()) {
        let mut n = p.clone();
        normalize(&mut n);
        if p.state == "open" && n.kind == "action" && n.pushes {
            p.state = "done".into();
            p.result = Some(ProposalResult { url: Some(url.to_string()), message: Some("MR đã có (tạo khi nghiệm thu hoặc từ đề xuất khác)".into()), ..Default::default() });
        }
    }
}

/// Close an open proposal that ran synchronously (revise).
pub(super) fn finish(t: &mut Task, msg: &str, res: Result<(), String>) {
    if let Ok(p) = open_proposal(t, msg) {
        match res {
            Ok(()) => p.state = "done".into(),
            Err(e) => {
                p.state = "failed".into();
                p.result = Some(ProposalResult { error: Some(e), ..Default::default() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_json_is_split_from_the_reply() {
        let (text, p) = split_proposal("Em sẽ tạo MR bằng skill gitlab-mr.\n```json\n{\"propose\": {\"kind\": \"action\", \"title\": \"Tạo merge request\", \"instruction\": \"MR {x} vào master, \\\"PT1\\\" }\", \"skill\": \"gitlab-mr\", \"pushes\": true, \"target\": \"master\", \"writes_external\": [\"PMS ticket #1: solution\"]}}\n```");
        let p = p.unwrap();
        assert_eq!(text, "Em sẽ tạo MR bằng skill gitlab-mr.");
        assert_eq!((p.kind.as_str(), p.title.as_deref(), p.instruction.as_deref(), p.skill.as_deref(), p.pushes, p.target.as_deref(), p.state.as_str()), ("action", Some("Tạo merge request"), Some("MR {x} vào master, \"PT1\" }"), Some("gitlab-mr"), true, Some("master"), "open"));
        assert_eq!(p.writes_external, ["PMS ticket #1: solution"]);
        // Older kinds are the MR action.
        let (_, old) = split_proposal("{\"propose\": {\"kind\": \"merge_request\", \"target\": \"dev\"}}");
        let old = old.unwrap();
        assert_eq!((old.kind.as_str(), old.pushes, old.title.as_deref(), old.instruction.as_deref()), ("action", true, Some("Tạo merge request"), Some("Tạo merge request cho tính năng này vào dev")));
        assert_eq!(split_proposal("Chỉ trả lời thôi, không đề xuất.").1, None);
        assert_eq!(split_proposal("x {\"propose\": {\"kind\": \"delete_repo\"}}").1, None, "unknown kinds are not proposals");
        let (t2, p2) = split_proposal("Ok.\n{\"propose\":{\"kind\":\"revise\",\"instruction\":\"đổi màu nút\"}}");
        assert_eq!((t2.as_str(), p2.unwrap().instruction.as_deref()), ("Ok.", Some("đổi màu nút")));
    }

    #[test]
    fn remote_hosts_and_urls() {
        assert_eq!(remote_host("http://git.onlinecrm.vn/cloudgo/vtiger.git").as_deref(), Some("git.onlinecrm.vn"));
        assert_eq!(remote_host("git@github.com:a/b.git").as_deref(), Some("github.com"));
        assert_eq!(remote_host("ssh://git@host.x:2222/a/b").as_deref(), Some("host.x"));
        assert_eq!(remote_host("/tmp/origin.git"), None);
        assert_eq!(find_url("Creating merge request…\n!12 Fix (board/t-001)\nhttps://git.x/a/b/-/merge_requests/12\n").as_deref(), Some("https://git.x/a/b/-/merge_requests/12"));
        assert_eq!(find_url("{\"web_url\":\"https://git.x/a/-/merge_requests/3\",\"author\":{\"avatar_url\":\"https://a\"}}").as_deref(), Some("https://git.x/a/-/merge_requests/3"));
        assert_eq!(find_url("https://github.com/a/b/pull/7").as_deref(), Some("https://github.com/a/b/pull/7"));
    }

    fn sh(dir: &FsPath, args: &[&str]) {
        let ok = Command::new("git").current_dir(dir).args(args).env("GIT_CONFIG_GLOBAL", "/dev/null").status().unwrap().success();
        assert!(ok, "git {args:?}");
    }

    fn commit(dir: &FsPath, file: &str, content: &str, msg: &str, date: &str) {
        std::fs::write(dir.join(file), content).unwrap();
        sh(dir, &["add", file]);
        let ok = Command::new("git")
            .current_dir(dir)
            .args(["commit", "-q", "-m", msg])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .status()
            .unwrap()
            .success();
        assert!(ok, "commit {msg}");
    }

    /// The T-007 shape: origin has `init`; the boss's local master is one unpushed
    /// commit ahead ("Update .gitignore", 10:00, before the task); `board/t-001`
    /// branched from that local master carries the task's commit (12:00, during its
    /// 11:00–13:00 run). Origin's URL looks like GitLab/GitHub and is rewritten to a
    /// local bare repo (`insteadOf`). `bin` is for a fake `omp` (see `fake_omp`).
    fn fixture(tag: &str, host_url: &str) -> (PathBuf, Task, PathBuf) {
        let base = std::env::temp_dir().join(format!("ckit-chat-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, bare, bin) = (base.join("repo"), base.join("origin.git"), base.join("bin"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        sh(&base, &["init", "-q", "--bare", "-b", "master", bare.to_str().unwrap()]);
        sh(&root, &["init", "-q", "-b", "master"]);
        sh(&root, &["config", "user.email", "t@t"]);
        sh(&root, &["config", "user.name", "t"]);
        commit(&root, "a.txt", "a\n", "init", "2026-10-18T09:00:00Z");
        sh(&root, &["remote", "add", "origin", host_url]);
        sh(&root, &["config", &format!("url.{}.insteadOf", bare.display()), host_url]);
        sh(&root, &["push", "-q", "origin", "master"]);
        commit(&root, ".gitignore", "x\n", "Update .gitignore", "2026-10-18T10:00:00Z");
        let base_sha = git(&root, &["rev-parse", "HEAD"]).unwrap().trim().to_string();
        sh(&root, &["switch", "-q", "-c", "board/t-001"]);
        commit(&root, "calc.py", "def divide(a, b):\n    return a / b\n", "feat: chia", "2026-10-18T12:00:00Z");
        sh(&root, &["switch", "-q", "master"]);
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        t.branch = Some("board/t-001".into());
        t.base = Some("master".into());
        t.base_sha = Some(base_sha);
        t.runs = vec![Run {
            stage: Stage::Dev,
            n: 1,
            started: "2026-10-18T11:00:00Z".into(),
            ended: Some("2026-10-18T13:00:00Z".into()),
            exit: Some(0),
            cost: 0.0,
            verdict: Some("pass".into()),
            log: String::new(),
            phase: None,
            round: None,
            title: None,
        }];
        (root, t, bin)
    }

    fn bare_log(root: &FsPath, range: &str) -> String {
        let out = Command::new("git").current_dir(root.parent().unwrap().join("origin.git")).args(["log", "--format=%s", range]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn bare_has(root: &FsPath, br: &str) -> bool {
        Command::new("git").current_dir(root.parent().unwrap().join("origin.git")).args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{br}")]).status().unwrap().success()
    }

    fn with_path<T>(bin: &FsPath, f: impl FnOnce() -> T) -> T {
        static ENV: Mutex<()> = Mutex::new(());
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{}", bin.display(), old.to_string_lossy()));
        let r = f();
        std::env::set_var("PATH", old);
        r
    }

    #[test]
    fn revise_opens_a_round_or_refuses() {
        let (root, mut t, _) = fixture("rev", "http://git.example.vn/team/repo.git");
        t.steps = vec![Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval, Stage::Dev, Stage::Acceptance];
        // Another task holds the tree.
        let mut other = super::super::tests::task(Stage::Dev, Status::Running);
        other.id = "T-002".into();
        other.branch = Some("board/t-002".into());
        save(&root, &mut other).unwrap();
        assert_eq!(revise(&root, &mut t, "đổi màu nút").unwrap_err(), "T-002 đang giữ cây làm việc — chờ việc đó xong");
        std::fs::remove_dir_all(task_dir(&root, "T-002")).unwrap();
        revise(&root, &mut t, "đổi màu nút").unwrap();
        assert_eq!((t.stage, t.status, t.round), (Stage::Dev, Status::Queued, 2));
        assert_eq!(t.steps, [Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval, Stage::Dev, Stage::Test, Stage::Report, Stage::Acceptance]);
        assert_eq!(t.thread.last().unwrap().text, "Vòng 2: đổi màu nút");
        // Workflow task: run → acceptance only.
        let (root2, mut w, _) = fixture("rev2", "http://git.example.vn/team/repo.git");
        w.workflow = Some(workflow::Workflow { name: "cook".into(), path: "x".into(), plan: None });
        w.steps = vec![Stage::Solution, Stage::Approval, Stage::Dev];
        revise(&root2, &mut w, "thêm test").unwrap();
        assert_eq!(w.steps, [Stage::Solution, Stage::Approval, Stage::Dev, Stage::Acceptance]);
        // Nothing merges locally any more: a branch already in base still gets a
        // round (its MR carries it); only a gone branch is refused.
        let (root3, mut m, _) = fixture("rev3", "http://git.example.vn/team/repo.git");
        sh(&root3, &["-c", "user.email=t@t", "-c", "user.name=t", "merge", "-q", "--no-ff", "board/t-001", "-m", "merge"]);
        revise(&root3, &mut m, "x").unwrap();
        let mut gone = super::super::tests::task(Stage::Done, Status::Done);
        gone.branch = Some("board/t-009".into());
        assert_eq!(revise(&root3, &mut gone, "x").unwrap_err(), "nhánh board/t-009 không còn");
        t.status = Status::Running;
        assert!(revise(&root, &mut t, "x").is_err(), "not while working");
        for r in [root, root2, root3] {
            let _ = std::fs::remove_dir_all(r.parent().unwrap());
        }
    }

    /// A fake `omp`: records its args and cwd, cd's into `--cwd`, then runs `body`
    /// (with `$R` = the result file) the way an agent following a skill would.
    fn fake_omp(bin: &FsPath, body: &str) {
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> '{0}/omp.args'\nwhile [ $# -gt 0 ]; do [ \"$1\" = --cwd ] && cd \"$2\"; shift; done\npwd > '{0}/omp.cwd'\nR=\"$CKIT_BOARD_RESULT\"\n{body}\n",
            bin.display()
        );
        std::fs::write(bin.join("omp"), script).unwrap();
        Command::new("chmod").arg("+x").arg(bin.join("omp")).status().unwrap();
    }

    /// What a skill does once the board pushed: create the MR (API), report it.
    const PUSH_AND_URL: &str = "echo \"$TARGET_REF\" > \"$(dirname \"$0\")/target.ref\"; printf '{\"verdict\":\"pass\",\"message\":\"Đã tạo MR theo skill fake-mr\",\"urls\":[\"https://x/-/merge_requests/5\"]}' > \"$R\"";

    fn mr_action() -> Proposal {
        Proposal { kind: "action".into(), title: Some("Tạo merge request".into()), instruction: Some("Tạo MR vào master".into()), skill: Some("fake-mr".into()), pushes: true, target: Some("master".into()), state: "open".into(), ..Default::default() }
    }

    fn run_action(root: &FsPath, bin: &FsPath, id: &str, p: &Proposal, accept: bool) -> Result<Task, String> {
        with_path(bin, || {
            let (_, h) = action::start(root, id, p, action::Mode { msg: None, accept, via: None })?;
            h.join().unwrap();
            Ok(load(root, id).unwrap())
        })
    }

    #[test]
    fn pushing_action_lists_target_choices_commits_and_foreign_ones() {
        let (root, t, _) = fixture("chk", "http://git.example.vn/team/repo.git");
        let mut p = mr_action();
        p.target = None;
        check(&root, &t, &mut p);
        assert_eq!(p.check_error, None);
        assert_eq!((p.targets.clone(), p.remote_default.as_deref(), p.target.as_deref()), (vec!["master".to_string()], Some("master"), Some("master")));
        let marks: Vec<(&str, bool)> = p.commits.iter().map(|c| (c.subject.as_str(), c.from_task)).collect();
        assert_eq!(marks, [("Update .gitignore", false), ("feat: chia", true)]);
        assert_eq!(p.foreign.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["Update .gitignore"]);
        // Older task without a recorded base: derived from the base as of the first run.
        let mut old = t.clone();
        old.base_sha = None;
        assert_eq!(mr_commits(&root, &old, "master").iter().map(|c| c.from_task).collect::<Vec<_>>(), [false, true]);
        p.target = Some("develop".into());
        check(&root, &t, &mut p);
        assert_eq!(p.check_error.as_deref(), Some("nhánh đích develop không có trên origin"));
        // The chat Leader lists the project's skills (still read-only); a team
        // question's answerer does not; the action run is a normal session.
        let chat_args: Vec<String> = ask::answerer_args(&root, None, true).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!chat_args.iter().any(|a| a == "--no-skills") && chat_args.iter().any(|a| a == "--tools=read,grep,glob"), "{chat_args:?}");
        assert!(ask::answerer_args(&root, None, false).iter().any(|a| a == "--no-skills"));
        let act: Vec<String> = action::action_args(FsPath::new("/tmp/wt"), FsPath::new("/tmp/s"), true).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(act.windows(2).any(|w| w == ["--cwd", "/tmp/wt"]) && act.iter().any(|a| a == "--continue") && act.iter().any(|a| a == "yolo"), "{act:?}");
        assert!(!act.iter().any(|a| a.starts_with("--no-") || a.starts_with("--tools")), "full tools, skills, extensions: {act:?}");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    /// The agent runs in a temp worktree on the board-built `-mr` branch (task's
    /// commits only), with skills/extensions; its URL becomes `task.mr`.
    #[test]
    fn action_runs_the_agent_in_a_temp_worktree_and_stores_the_mr() {
        let (root, mut t, bin) = fixture("act", "http://git.example.vn/team/repo.git");
        save(&root, &mut t).unwrap();
        fake_omp(&bin, PUSH_AND_URL);
        let done = run_action(&root, &bin, &t.id, &mr_action(), false).unwrap();
        let cwd = std::fs::read_to_string(bin.join("omp.cwd")).unwrap();
        assert!(cwd.trim().ends_with("/wt") && !cwd.contains("/repo"), "temp worktree, not the boss's tree: {cwd}");
        let args = std::fs::read_to_string(bin.join("omp.args")).unwrap();
        assert!(args.contains("--approval-mode yolo") && !args.contains("--no-skills") && !args.contains("--no-extensions"), "{args}");
        assert!(args.contains("Board đã push nhánh `board/t-001-mr` lên origin; không push, chỉ tạo MR/PR") && args.contains("So sánh với `origin/master` (env `TARGET_REF`), không dùng `master` local"), "{args}");
        assert_eq!(std::fs::read_to_string(bin.join("target.ref")).unwrap().trim(), "origin/master");
        assert_eq!(bare_log(&root, "master..board/t-001-mr"), "feat: chia", "only the task's commit");
        assert_eq!(bare_log(&root, "master"), "init", "base never pushed");
        assert!(!bare_has(&root, "board/t-001"));
        let r = done.runs.last().unwrap();
        assert_eq!((r.stage, r.verdict.as_deref(), r.title.as_deref(), r.log.as_str()), (Stage::Action, Some("pass"), Some("Tạo merge request"), "action-1.jsonl"));
        let mr = done.mr.clone().unwrap();
        assert_eq!((mr.url.as_str(), mr.source.as_str(), mr.target.as_str()), ("https://x/-/merge_requests/5", "board/t-001-mr", "master"));
        assert!(done.thread.last().unwrap().text.starts_with("Thực hiện: Tạo merge request: Đã tạo MR theo skill fake-mr"));
        assert_eq!((done.stage, done.note.as_deref()), (Stage::Done, None));
        assert_eq!(git(&root, &["worktree", "list"]).unwrap().lines().count(), 1, "worktree removed");
        assert!(clean(&root) && current_branch(&root).as_deref() == Some("master"));
        // `push: "all"`: the task branch as it is (foreign commit included).
        let (root2, mut t2, bin2) = fixture("actall", "http://git.example.vn/team/repo.git");
        save(&root2, &mut t2).unwrap();
        fake_omp(&bin2, PUSH_AND_URL);
        let all = run_action(&root2, &bin2, &t2.id, &Proposal { push: Some("all".into()), ..mr_action() }, false).unwrap();
        assert_eq!(all.mr.unwrap().source, "board/t-001");
        assert_eq!(bare_log(&root2, "master..board/t-001"), "feat: chia\nUpdate .gitignore");
        // A replay conflict refuses before anything runs.
        let (root3, mut t3, bin3) = fixture("actconf", "http://git.example.vn/team/repo.git");
        sh(&root3, &["switch", "-q", "board/t-001"]);
        commit(&root3, ".gitignore", "x\ny\n", "chore: ignore y", "2026-10-18T12:30:00Z");
        sh(&root3, &["switch", "-q", "master"]);
        save(&root3, &mut t3).unwrap();
        fake_omp(&bin3, PUSH_AND_URL);
        let e = run_action(&root3, &bin3, &t3.id, &mr_action(), false).unwrap_err();
        assert!(e.contains("xung đột"), "{e}");
        assert!(!bin3.join("omp.args").exists() && !bare_has(&root3, "board/t-001-mr"));
        assert!(!busy(&root3, &t3.id), "claim released");
        for r in [&root, &root2, &root3] {
            let _ = std::fs::remove_dir_all(r.parent().unwrap());
        }
    }

    /// First layer: the worktree's own push URL is unusable, so an agent push
    /// fails even with `--no-verify` (origin unchanged, shared config untouched).
    /// Second layer, for a repo with its own `remote.origin.pushurl` (the block
    /// cannot cover it): the pre-push hook refuses non-board refs and ls-remote
    /// catches a `--no-verify` push afterwards. No MR is recorded then.
    #[test]
    fn agent_pushes_are_blocked_and_bypasses_are_caught() {
        let url = "printf '{\"verdict\":\"pass\",\"message\":\"xong\",\"urls\":[\"https://x/-/merge_requests/6\"]}' > \"$R\"";
        let (root, mut t, bin) = fixture("noverify", "http://git.example.vn/team/repo.git");
        save(&root, &mut t).unwrap();
        fake_omp(&bin, &format!("git push -q --no-verify origin HEAD:refs/heads/master; echo $? > '{0}/push.exit'; git push -q --no-verify origin HEAD:refs/heads/board/t-001-mr; echo $? >> '{0}/push.exit'; {url}", bin.display()));
        let out = run_action(&root, &bin, &t.id, &mr_action(), false).unwrap();
        let exits = std::fs::read_to_string(bin.join("push.exit")).unwrap();
        assert!(!exits.lines().any(|l| l.trim() == "0"), "every agent push fails: {exits}");
        assert_eq!(bare_log(&root, "master"), "init", "master untouched");
        let after = Command::new("git").current_dir(root.parent().unwrap().join("origin.git")).args(["for-each-ref", "--format=%(refname)"]).output().unwrap().stdout;
        assert!(String::from_utf8_lossy(&after).contains("refs/heads/board/t-001-mr") && !String::from_utf8_lossy(&after).contains("refs/heads/evil"));
        assert_eq!(out.runs.last().unwrap().verdict.as_deref(), Some("pass"), "nothing reached origin: the run itself is fine");
        assert!(git(&root, &["config", "--get", "remote.origin.pushurl"]).is_err(), "no push URL in the shared config");
        assert!(!out.thread.last().unwrap().text.contains("chưa chặn được"));
        // A repo with its own pushurl: the hook and ls-remote are what is left.
        let (root1, mut t1, bin1) = fixture("evil", "http://git.example.vn/team/repo.git");
        sh(&root1, &["config", "remote.origin.pushurl", root1.parent().unwrap().join("origin.git").to_str().unwrap()]);
        save(&root1, &mut t1).unwrap();
        fake_omp(&bin1, &format!("git push -q origin HEAD:refs/heads/master; {url}"));
        let out1 = run_action(&root1, &bin1, &t1.id, &mr_action(), false).unwrap();
        assert_eq!(bare_log(&root1, "master"), "init", "master untouched");
        assert_eq!(out1.runs.last().unwrap().verdict.as_deref(), Some("blocked"));
        assert!(out1.mr.is_none());
        let line = &out1.thread.last().unwrap().text;
        assert!(line.contains("refs/heads/master (bị chặn)") && line.contains("worktree giữ lại") && line.contains("chưa chặn được"), "{line}");
        let (root2, mut t2, bin2) = fixture("bypass", "http://git.example.vn/team/repo.git");
        sh(&root2, &["config", "remote.origin.pushurl", root2.parent().unwrap().join("origin.git").to_str().unwrap()]);
        save(&root2, &mut t2).unwrap();
        fake_omp(&bin2, &format!("git push -q --no-verify origin HEAD:refs/heads/evil; {url}"));
        let out2 = run_action(&root2, &bin2, &t2.id, &mr_action(), false).unwrap();
        assert!(bare_has(&root2, "evil"));
        assert_eq!(out2.runs.last().unwrap().verdict.as_deref(), Some("blocked"));
        assert!(out2.mr.is_none() && out2.thread.last().unwrap().text.contains("refs/heads/evil"));
        assert_eq!(action::foreign_ref_changes(&[("refs/heads/board/x".into(), "1".into()), ("refs/heads/master".into(), "a".into())], &[("refs/heads/board/x".into(), "2".into())]), ["refs/heads/master (xoá)"]);
        assert_eq!(action::mr_url(&["https://x/docs".into(), "https://g/a/-/merge_requests/12".into()], ""), Some("https://g/a/-/merge_requests/12".into()));
        assert_eq!(action::mr_url(&[], "PR: https://github.com/a/b/pull/3."), Some("https://github.com/a/b/pull/3".into()));
        assert_eq!(action::mr_url(&["https://g/a/-/merge_requests/new".into()], ""), None);
        for r in [&root, &root1, &root2] {
            let _ = std::fs::remove_dir_all(r.parent().unwrap());
        }
    }

    /// "Nghiệm thu" = the MR action: Done only with an MR URL; no URL → stays at the
    /// gate with the agent's message; WIP / missing target refuse before running; an
    /// MR recorded before heads (the real T-007 !434) is only linked.
    #[test]
    fn acceptance_is_done_only_with_an_mr_url() {
        let (root, mut t, bin) = fixture("acc", "http://git.example.vn/team/repo.git");
        t.set(Stage::Acceptance, Status::Waiting);
        sh(&root, &["switch", "-q", "board/t-001"]);
        std::fs::create_dir_all(root.join("agents/planning")).unwrap();
        std::fs::write(root.join("agents/planning/ACTIVE.md"), "board-t-001\n").unwrap();
        save(&root, &mut t).unwrap();
        let p = action::mr_proposal(&t);
        std::fs::write(root.join("calc.py"), "wip\n").unwrap();
        let e = with_path(&bin, || action::accept(&root, &t.id, &p, None)).unwrap_err();
        assert!(e.contains("calc.py"), "{e}");
        sh(&root, &["checkout", "--", "calc.py"]);
        let e = with_path(&bin, || action::accept(&root, &t.id, &Proposal { target: Some("develop".into()), ..p.clone() }, None)).unwrap_err();
        assert_eq!(e, "nhánh đích develop không có trên origin");
        // The agent answers without a link: still at the gate.
        fake_omp(&bin, "printf '{\"verdict\":\"pass\",\"message\":\"đã đẩy nhánh, chưa tạo được MR\",\"urls\":[]}' > \"$R\"");
        let mut p2 = p.clone();
        p2.instruction = Some(action::mr_instruction(Some("master")));
        let waiting = run_action(&root, &bin, &t.id, &p2, true).unwrap();
        assert_eq!((waiting.stage, waiting.status, waiting.mr.is_none()), (Stage::Acceptance, Status::Waiting, true));
        assert_eq!(waiting.thread.last().unwrap().text, "Nghiệm thu chưa xong (chưa có MR): agent không trả về link MR: đã đẩy nhánh, chưa tạo được MR");
        // With a link: Done, base's /feature pointers restored on the branch, the
        // chat's open MR proposal (and an older merge_request one) settled with it.
        let mut cur = load(&root, &t.id).unwrap();
        push_msg(&mut cur, "leader", "Đề xuất MR", 0.0, Some(mr_action()), None);
        push_msg(&mut cur, "leader", "MR cũ", 0.0, Some(Proposal { kind: "merge_request".into(), state: "open".into(), ..Default::default() }), None);
        push_msg(&mut cur, "leader", "Sửa", 0.0, Some(Proposal { kind: "revise".into(), instruction: Some("x".into()), state: "open".into(), ..Default::default() }), None);
        save(&root, &mut cur).unwrap();
        fake_omp(&bin, PUSH_AND_URL);
        let done = run_action(&root, &bin, &t.id, &p2, true).unwrap();
        assert_eq!((done.stage, done.status), (Stage::Done, Status::Done));
        assert_eq!(done.thread.last().unwrap().text, "Nghiệm thu: đã tạo MR https://x/-/merge_requests/5");
        let bare = root.parent().unwrap().join("origin.git");
        assert!(!Command::new("git").current_dir(&bare).args(["cat-file", "-e", "board/t-001-mr:agents/planning/ACTIVE.md"]).status().unwrap().success());
        assert_eq!(done.runs.iter().filter(|r| r.stage == Stage::Action).count(), 2);
        let states: Vec<(&str, Option<&str>)> = done.chat.iter().filter_map(|m| m.proposal.as_ref()).map(|p| (p.state.as_str(), p.result.as_ref().and_then(|r| r.url.as_deref()))).collect();
        assert_eq!(states, [("done", Some("https://x/-/merge_requests/5")), ("done", Some("https://x/-/merge_requests/5")), ("open", None)]);
        // Legacy MR (no source/head): acceptance links it, nothing pushed, no agent.
        let (root2, mut old, bin2) = fixture("acc2", "http://git.example.vn/team/repo.git");
        old.set(Stage::Acceptance, Status::Waiting);
        old.mr = Some(Mr { url: "https://git.example.vn/team/repo/-/merge_requests/434".into(), provider: "gitlab".into(), target: "master".into(), at: String::new(), source: String::new(), head: None });
        save(&root2, &mut old).unwrap();
        let linked = with_path(&bin2, || action::accept(&root2, &old.id, &action::mr_proposal(&old), None)).unwrap();
        assert_eq!((linked.stage, linked.thread.last().unwrap().text.as_str()), (Stage::Done, "Nghiệm thu: đã có MR https://git.example.vn/team/repo/-/merge_requests/434"));
        assert!(!bare_has(&root2, "board/t-001") && !bare_has(&root2, "board/t-001-mr") && !bin2.join("omp.args").exists());
        for r in [root, root2] {
            let _ = std::fs::remove_dir_all(r.parent().unwrap());
        }
    }

    /// The Leader continues a copy of the latest finished working session; the
    /// original file is left byte-for-byte unchanged.
    #[test]
    fn leader_forks_the_latest_finished_working_session() {
        let root = std::env::temp_dir().join(format!("ckit-chat-fork-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        let dir = task_dir(&root, &t.id);
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        let run = |stage: Stage, n: u32, stamp: &str, started: &str, ended: &str, id: &str| {
            std::fs::write(dir.join("logs").join(format!("{}-{n}.jsonl", stage.key())), format!("{{\"type\":\"session\",\"id\":\"{id}\"}}\n")).unwrap();
            std::fs::write(dir.join("sessions").join(format!("{stamp}_{id}.jsonl")), format!("session {id}\n")).unwrap();
            Run { stage, n, started: started.into(), ended: Some(ended.into()), exit: Some(0), cost: 0.0, verdict: Some("pass".into()), log: format!("{}-{n}.jsonl", stage.key()), phase: None, round: None, title: None }
        };
        t.runs = vec![
            run(Stage::Solution, 1, "2026-10-15T10-00-00-000Z", "2026-10-15T10:00:00Z", "2026-10-15T10:10:00Z", "aaa"),
            run(Stage::Dev, 1, "2026-10-15T11-00-00-000Z", "2026-10-15T11:00:00Z", "2026-10-15T11:30:00Z", "bbb"),
            run(Stage::Report, 1, "2026-10-15T12-00-00-000Z", "2026-10-15T12:00:00Z", "2026-10-15T12:05:00Z", "ccc"),
        ];
        let s = leader_session(&dir, &t).unwrap();
        assert!(s.to_string_lossy().ends_with("_bbb.jsonl"), "Dev is the latest working run (Report is not one): {s:?}");
        let before = std::fs::read(&s).unwrap();
        let fork = ask::fork_session(&s, &t.id, "C-1").unwrap();
        let copy = fork.0.join(s.file_name().unwrap());
        std::fs::write(&copy, "appended by omp --continue\n").unwrap();
        assert_eq!(std::fs::read(&s).unwrap(), before);
        drop(fork);
        assert!(!copy.exists());
        t.runs[1].ended = None; // still running → previous finished one
        assert!(leader_session(&dir, &t).unwrap().to_string_lossy().ends_with("_aaa.jsonl"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn chat_cost_counts_in_the_task_and_turns_are_serialized() {
        let root = std::env::temp_dir().join(format!("ckit-chat-cost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        let dir = task_dir(&root, &t.id);
        std::fs::create_dir_all(dir.join("chat")).unwrap();
        std::fs::write(dir.join("chat/C-2.jsonl"), "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[],\"usage\":{\"cost\":{\"total\":0.25}}}}\n").unwrap();
        t.recost(&root);
        assert!((t.cost - 0.25).abs() < 1e-9);
        // A turn in flight refuses the next message; cancelled tasks refuse all.
        busy_set().lock().unwrap().insert(key(&root, &t.id));
        assert!(post(&root, &mut t, "còn gì?", None).unwrap_err().contains("đang trả lời"));
        busy_set().lock().unwrap().remove(&key(&root, &t.id));
        t.status = Status::Cancelled;
        assert!(post(&root, &mut t, "còn gì?", None).is_err());
        assert!(t.chat.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
