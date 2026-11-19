//! `board_ask` — team members ask each other mid-step.
//!
//! A running step calls the `board_ask` tool (engine extension), which writes
//! `ask/Q-<n>.json` and waits for `ask/A-<n>.json`. The worker routes each new
//! question: to the boss (dashboard), or to the Leader, who answers or escalates
//! to the boss. The Leader answering is the Leader who planned the phase: its
//! Kế hoạch session is copied to a temp dir and continued read-only, so the
//! answer comes from the one who knows the plan best (no plan session → a fresh
//! read-only Leader session). Those sessions bypass the project's single run
//! slot — the asking run holds it while it waits, so queueing the answer behind
//! it would deadlock.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{guard, iso_now, list, load, save, task_dir, Msg, Stage, Status, Task};
use crate::models::{ModelConfig, TaskClass};

const ANSWER_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Ask {
    pub id: String,
    pub from: Stage,
    pub to: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
    pub at: String,
    /// answering (Leader session) · boss · answered · timeout · closed (run ended first)
    pub status: String,
    pub answer: Option<String>,
    pub by: Option<String>,
    pub answered_at: Option<String>,
    /// `ac_change`: a request to change an acceptance criterion — only the boss
    /// decides it, it never times out and a run's end does not close it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ac: Option<AcChange>,
    /// `/feature` phase the question belongs to (AC ids restart per phase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// What an AC change request proposes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(super) struct AcChange {
    pub ac: String,
    #[serde(default)]
    pub old: String,
    #[serde(default)]
    pub new: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub evidence: String,
}

pub(super) const AC_CHANGE: &str = "ac_change";
/// Options of an AC change card; the answer's first word decides it.
pub(super) const AC_OPTIONS: [&str; 2] = ["Duyệt", "Không duyệt"];

impl Ask {
    pub(super) fn is_ac_change(&self) -> bool {
        self.kind.as_deref() == Some(AC_CHANGE)
    }

    /// Some(true) approved, Some(false) rejected, None not decided (yet).
    pub(super) fn approved(&self) -> Option<bool> {
        if !self.is_ac_change() || self.status != "answered" || self.by.as_deref() != Some("boss") {
            return None;
        }
        Some(approves(self.answer.as_deref().unwrap_or("")))
    }
}

/// `Duyệt …` / `Đồng ý` / `ok` / `approve` = yes; `Không duyệt`, anything else = no.
pub(super) fn approves(answer: &str) -> bool {
    let a = answer.trim().to_lowercase();
    ["duyệt", "đồng ý", "ok", "approve", "yes", "có"].iter().any(|w| a.starts_with(w)) && !a.starts_with("không")
}

#[derive(Deserialize)]
struct QFile {
    id: String,
    to: String,
    #[serde(default)]
    question: String,
    #[serde(default)]
    options: Vec<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    ac: Option<AcChange>,
}

#[derive(Serialize, Deserialize)]
struct AFile {
    answer: String,
    by: String,
}

pub(super) fn ask_dir(root: &FsPath, id: &str) -> PathBuf {
    task_dir(root, id).join("ask")
}

/// A question is open while its answer file does not exist yet.
pub(super) fn has_open_question(dir: &FsPath) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return false };
    rd.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().into_owned();
        name.strip_prefix("Q-").and_then(|r| r.strip_suffix(".json")).is_some_and(|n| !dir.join(format!("A-{n}.json")).exists())
    })
}

/// Publish the answer file the tool polls — complete (temp + link) and exclusive:
/// the tool's timeout, the Leader session and the boss may race; the first one to
/// land wins and `absorb` records exactly that one (`AlreadyExists` for losers).
fn write_answer(dir: &FsPath, qid: &str, answer: &str, by: &str) -> std::io::Result<()> {
    let n = qid.trim_start_matches("Q-");
    let tmp = dir.join(format!(".A-{n}-{by}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(&AFile { answer: answer.into(), by: by.into() })?)?;
    let linked = std::fs::hard_link(&tmp, dir.join(format!("A-{n}.json")));
    let _ = std::fs::remove_file(&tmp);
    linked
}

fn in_flight() -> &'static Mutex<HashSet<String>> {
    static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Pull new questions and answers from the ask dir into the task. Returns the
/// questions that need a Leader session. Caller holds the board lock and saves.
fn absorb(root: &FsPath, t: &mut Task) -> (bool, Vec<Ask>) {
    let dir = ask_dir(root, &t.id);
    let mut changed = false;
    let mut route = Vec::new();
    let running = t.status == Status::Running;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut qs: Vec<QFile> = rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("Q-"))
            .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
            .collect();
        qs.sort_by_key(|q| q.id.trim_start_matches("Q-").parse::<u32>().unwrap_or(0));
        for q in qs {
            if t.asks.iter().any(|a| a.id == q.id) {
                continue;
            }
            let ac_change = q.kind.as_deref() == Some(AC_CHANGE);
            // AC changes go to the boss whoever the run addressed.
            let to = if ac_change { "boss" } else { route_to(&q.to) };
            let (question, options) = match (&q.ac, ac_change) {
                (Some(c), true) => (ac_question(c, &q.question), AC_OPTIONS.map(String::from).to_vec()),
                _ => (q.question, q.options),
            };
            let ask = Ask {
                id: q.id,
                from: t.stage,
                to: to.into(),
                question,
                options,
                at: iso_now(),
                status: if ac_change || (running && to == "boss") { "boss" } else if !running { "closed" } else { "answering" }.into(),
                answer: None,
                by: None,
                answered_at: None,
                kind: ac_change.then(|| AC_CHANGE.to_string()),
                ac: q.ac.filter(|_| ac_change),
                phase: t.runs.last().and_then(|r| r.phase.clone()).or_else(|| super::active_phase(root, t)),
            };
            let opts = if ask.options.is_empty() { String::new() } else { format!("\n\nLựa chọn: {}", ask.options.join(" · ")) };
            t.say(&format!("{} → {}", t.stage.key(), to), &format!("[{}] {}{opts}", ask.id, ask.question));
            if ask.status == "answering" {
                route.push(ask.clone());
            }
            t.asks.push(ask);
            changed = true;
        }
    }
    let mut msgs = Vec::new();
    for a in t.asks.iter_mut().filter(|a| matches!(a.status.as_str(), "answering" | "boss")) {
        let n = a.id.trim_start_matches("Q-");
        let Some(f) = std::fs::read_to_string(dir.join(format!("A-{n}.json"))).ok().and_then(|s| serde_json::from_str::<AFile>(&s).ok()) else {
            continue;
        };
        if f.by == "timeout" && a.is_ac_change() {
            // Only the boss decides an AC change: a timeout (an older engine) is not
            // an answer; clear it so the boss's answer can land.
            let _ = std::fs::remove_file(dir.join(format!("A-{n}.json")));
            continue;
        }
        a.answered_at = Some(iso_now());
        a.by = Some(f.by.clone());
        if f.by == "timeout" {
            a.status = "timeout".into();
            msgs.push(("system".to_string(), format!("[{}] hết giờ chờ trả lời — {} tự quyết (auto-decided).", a.id, a.from.key())));
        } else {
            a.status = "answered".into();
            msgs.push((f.by.clone(), format!("[{}] {}", a.id, f.answer)));
            a.answer = Some(f.answer);
        }
        changed = true;
    }
    for (from, text) in msgs {
        t.say(&from, &text);
    }
    (changed, route)
}

/// The run ended (done, failed, cancelled): unanswered questions are closed —
/// except AC change requests, which wait for the boss (the phase-close gate uses them).
pub(super) fn close_open(root: &FsPath, t: &mut Task) {
    let _ = absorb(root, t);
    let dir = ask_dir(root, &t.id);
    for a in t.asks.iter_mut().filter(|a| matches!(a.status.as_str(), "answering" | "boss") && !a.is_ac_change()) {
        a.status = "closed".into();
        // Publish an answer file so the question no longer counts as open (the
        // timeout clock and later runs check `has_open_question`).
        let _ = write_answer(&dir, &a.id, "", "closed");
    }
}

/// The card text of an AC change request.
pub(super) fn ac_question(c: &AcChange, extra: &str) -> String {
    let mut s = format!("[Đổi AC] {}", c.ac);
    if !c.old.trim().is_empty() {
        s.push_str(&format!("\nĐang là: {}", c.old.trim()));
    }
    if !c.new.trim().is_empty() {
        s.push_str(&format!("\nĐề xuất: {}", c.new.trim()));
    }
    if !c.reason.trim().is_empty() {
        s.push_str(&format!("\nLý do: {}", c.reason.trim()));
    }
    if !c.evidence.trim().is_empty() {
        s.push_str(&format!("\nBằng chứng: {}", c.evidence.trim()));
    }
    if !extra.trim().is_empty() {
        s.push_str(&format!("\n{}", extra.trim()));
    }
    s
}

/// Open an AC change card for the boss outside a run (the phase-close gate):
/// writes the question file (so ids never collide with the runs') and records it.
pub(super) fn open_ac_change(root: &FsPath, t: &mut Task, c: AcChange, phase: Option<String>) -> std::io::Result<String> {
    let dir = ask_dir(root, &t.id);
    std::fs::create_dir_all(&dir)?;
    let mut n = 1;
    let id = loop {
        let id = format!("Q-{n}");
        let body = serde_json::json!({ "id": id, "from": t.stage.key(), "to": "boss", "kind": AC_CHANGE, "ac": c, "question": "", "at": iso_now() });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(format!("{id}.json"))) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(body.to_string().as_bytes())?;
                break id;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    };
    let ask = Ask {
        id: id.clone(),
        from: t.stage,
        to: "boss".into(),
        question: ac_question(&c, ""),
        options: AC_OPTIONS.map(String::from).to_vec(),
        at: iso_now(),
        status: "boss".into(),
        answer: None,
        by: None,
        answered_at: None,
        kind: Some(AC_CHANGE.into()),
        ac: Some(c),
        phase,
    };
    t.say(&format!("{} → boss", t.stage.key()), &format!("[{}] {}\n\nLựa chọn: {}", ask.id, ask.question, AC_OPTIONS.join(" · ")));
    t.asks.push(ask);
    Ok(id)
}

/// Who answers: the Leader (also the legacy `ba` / `po` targets) or the boss.
fn route_to(to: &str) -> &'static str {
    if matches!(to, "leader" | "ba" | "po") {
        "leader"
    } else {
        "boss"
    }
}

/// Worker tick: route new questions, record answers. Every question for the
/// Leader gets its own answering session; nothing goes to the coordinator.
pub(super) fn sync(root: &FsPath) {
    for (id, ask) in collect(root) {
        let key = format!("{}|{id}|{}", root.display(), ask.id);
        if !in_flight().lock().map(|mut s| s.insert(key.clone())).unwrap_or(false) {
            continue;
        }
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            answer(&root, &id, &ask);
            in_flight().lock().map(|mut s| s.remove(&key)).ok();
        });
    }
}

/// Absorb every running task's ask dir; the questions the Leader must answer:
/// new ones, plus open ones without an answer (a restarted dashboard adopted the
/// run and lost the answering threads; `in_flight` keeps one answer per question).
fn collect(root: &FsPath) -> Vec<(String, Ask)> {
    let _g = guard();
    let mut out = Vec::new();
    for mut t in list(root).into_iter().filter(|t| t.status == Status::Running) {
        let (changed, mut route) = absorb(root, &mut t);
        if changed {
            let _ = save(root, &mut t);
        }
        let dir = ask_dir(root, &t.id);
        let open: Vec<Ask> = t
            .asks
            .iter()
            .filter(|a| a.status == "answering" && !route.iter().any(|r| r.id == a.id))
            .filter(|a| !dir.join(format!("A-{}.json", a.id.trim_start_matches("Q-"))).exists())
            .cloned()
            .collect();
        route.extend(open);
        out.extend(route.into_iter().map(|a| (t.id.clone(), a)));
    }
    out
}

/// The boss answers from the dashboard / Telegram; pre-empts the Leader's answer.
pub(super) fn answer_as(root: &FsPath, t: &mut Task, qid: &str, answer: &str, by: &str) -> Result<(), String> {
    let answer = answer.trim();
    if answer.is_empty() {
        return Err("nội dung trống".into());
    }
    let open = t.asks.iter().any(|a| a.id == qid && matches!(a.status.as_str(), "answering" | "boss"));
    if !open {
        return Err(format!("{qid} không còn chờ trả lời"));
    }
    write_answer(&ask_dir(root, &t.id), qid, answer, by).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => format!("{qid} vừa được trả lời trước bạn — xem khung trao đổi"),
        _ => e.to_string(),
    })?;
    let _ = absorb(root, t);
    // The last open AC change card of a held phase close decides where it goes.
    super::goals::resolve_gate(root, t);
    Ok(())
}

#[derive(Deserialize)]
struct Verdict {
    verdict: String,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    reason: String,
}

/// Run the Leader session for one question; answer, or escalate to the boss.
fn answer(root: &FsPath, id: &str, ask: &Ask) {
    let Some(t) = load(root, id) else { return };
    let dir = ask_dir(root, id);
    let n = ask.id.trim_start_matches("Q-");
    let log = dir.join(format!("leader-{n}.jsonl"));
    let verdict = run_answerer(root, &t, ask, &log);
    let _g = guard();
    let Some(mut cur) = load(root, id) else { return };
    cur.recost(root);
    let still_open = cur.asks.iter().any(|a| a.id == ask.id && a.status == "answering");
    if still_open {
        match verdict {
            Some(v) if v.verdict == "answer" && !v.answer.trim().is_empty() => {
                let _ = write_answer(&dir, &ask.id, v.answer.trim(), "leader");
                let _ = absorb(root, &mut cur);
            }
            v => {
                let reason = v.map(|v| v.reason).filter(|r| !r.trim().is_empty()).unwrap_or_else(|| "không trả lời được".into());
                if let Some(a) = cur.asks.iter_mut().find(|a| a.id == ask.id) {
                    a.status = "boss".into();
                }
                cur.say("leader", &format!("[{}] chuyển cho sếp: {reason}", ask.id));
            }
        }
    }
    let _ = save(root, &mut cur);
}

/// The Leader's planning session: the session file written by the task's latest
/// finished Kế hoạch run.
fn plan_session(dir: &FsPath, t: &Task) -> Option<PathBuf> {
    run_session(dir, t.runs.iter().rev().find(|r| r.stage == Stage::Solution)?)
}

/// The session file a finished run wrote (`sessions/` is shared by every run of
/// the task): its log names the session id; else the first session created inside
/// the run's [started, ended] window (the next run may start in the same second).
pub(super) fn run_session(dir: &FsPath, run: &super::Run) -> Option<PathBuf> {
    let ended = run.ended.as_deref()?;
    let files: Vec<(String, PathBuf)> = std::fs::read_dir(dir.join("sessions"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|p| Some((p.file_name()?.to_str()?.to_string(), p)))
        .filter(|(name, _)| !name.starts_with('.'))
        .collect();
    if let Some(id) = super::usage::log_session_id(&dir.join("logs").join(&run.log)) {
        if let Some((_, p)) = files.iter().find(|(name, _)| name.contains(&id)) {
            return Some(p.clone());
        }
    }
    let (from, to) = (run.started.get(..19)?, ended.get(..19)?);
    files
        .into_iter()
        .filter_map(|(name, p)| Some((session_stamp(&name)?, p)))
        .filter(|(at, _)| at.as_str() >= from && at.as_str() <= to)
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, p)| p)
}

/// `2026-10-15T16-44-28-390Z_<id>.jsonl` → `2026-10-15T16:44:28` (comparable
/// with the first 19 chars of `iso_now()`).
fn session_stamp(name: &str) -> Option<String> {
    let s = name.get(..19)?;
    let b = s.as_bytes();
    (b[10] == b'T' && b[13] == b'-' && b[16] == b'-').then(|| format!("{}:{}:{}", &s[..13], &s[14..16], &s[17..19]))
}

/// A private copy of the plan session for one question; removed on drop. The
/// original is never opened for writing: `omp --continue` appends to the copy.
pub(super) struct Fork(pub(super) PathBuf);

impl Drop for Fork {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fork_session(plan: &FsPath, task: &str, qid: &str) -> Option<Fork> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let fork = Fork(std::env::temp_dir().join(format!("ckit-leader-{}-{task}-{qid}-{n}", std::process::id())));
    let _ = std::fs::remove_dir_all(&fork.0);
    std::fs::create_dir_all(&fork.0).ok()?;
    std::fs::copy(plan, fork.0.join(plan.file_name()?)).ok()?;
    Some(fork)
}

/// omp arguments of the Leader answering, the prompt excluded. Read-only and
/// isolated: no write/edit/bash tools, no extensions (the engine tools commit),
/// so it can never touch the tree the asking run is editing. `skills`: the
/// project's skills are listed (the task chat's Leader names the one an action
/// should use); a team question's answerer runs without them. `fork` = continue
/// the copied plan session; None = a fresh session-less Leader.
pub(super) fn answerer_args(root: &FsPath, fork: Option<&FsPath>, skills: bool) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["--cwd".into(), root.into()];
    a.extend(["--mode", "json", "--no-extensions", "--tools=read,grep,glob"].map(Into::into));
    if !skills {
        a.push("--no-skills".into());
    }
    match fork {
        Some(dir) => a.extend(["--continue".into(), "--session-dir".into(), dir.into()]),
        None => a.push("--no-session".into()),
    }
    a.extend(ModelConfig::load().class_flags(TaskClass::Plan).into_iter().map(Into::into));
    a
}

/// Thread for the Leader: every message from the boss and every Leader
/// decision/answer (legacy po/ba too), plus the last `OTHERS` other messages.
pub(super) fn thread_context(thread: &[Msg]) -> String {
    const OTHERS: usize = 15;
    let decides = |m: &Msg| matches!(m.from.as_str(), "user" | "boss" | "leader" | "po" | "ba");
    let mut skip = thread.iter().filter(|m| !decides(m)).count().saturating_sub(OTHERS);
    thread
        .iter()
        .filter(|m| {
            if decides(m) || skip == 0 {
                return true;
            }
            skip -= 1;
            false
        })
        .map(|m| format!("[{}] {}", m.from, m.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn answer_prompt(root: &FsPath, t: &Task, ask: &Ask, forked: bool) -> Option<String> {
    let tpl = crate::assets::read("board/answer.md")?;
    let who = if forked {
        "You are the Leader (team lead, also the BA) who planned this phase: the conversation above is your own planning session. Planning is over; you now answer a teammate's question about it, consistent with the decisions you recorded."
    } else {
        "You are the Leader (team lead, also the BA) of an AI delivery team."
    };
    let fdir = super::feature_dir(root, t).map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into());
    let digest = t
        .runs
        .last()
        .map(|r| task_dir(root, &t.id).join("logs").join(&r.log))
        .filter(|p| p.exists())
        .map(|p| super::pm::run_digest_of(t, &p))
        .unwrap_or_else(|| "(none)".into());
    Some(
        tpl.replace("{{WHO}}", who)
            .replace("{{FROM}}", ask.from.key())
            .replace("{{ID}}", &t.id)
            .replace("{{TITLE}}", &t.title)
            .replace("{{FEATURE_DIR}}", &fdir)
            .replace("{{DIR}}", &task_dir(root, &t.id).display().to_string())
            .replace("{{THREAD}}", &thread_context(&t.thread))
            .replace("{{RUN}}", &digest)
            .replace("{{OPTIONS}}", &if ask.options.is_empty() { "(none)".into() } else { ask.options.join(" · ") })
            // Free text last: a request/question containing `{{…}}` is never expanded.
            .replace("{{REQUEST}}", &t.request)
            .replace("{{QUESTION}}", &ask.question),
    )
}

fn run_answerer(root: &FsPath, t: &Task, ask: &Ask, log: &FsPath) -> Option<Verdict> {
    let fork = plan_session(&task_dir(root, &t.id), t).and_then(|p| fork_session(&p, &t.id, &ask.id));
    let prompt = answer_prompt(root, t, ask, fork.is_some())?;
    let f = std::fs::File::create(log).ok()?;
    let err = f.try_clone().ok()?;
    let mut child = super::omp_cmd()
        .current_dir(root)
        .args(answerer_args(root, fork.as_ref().map(|f| f.0.as_path()), false))
        .arg("-p")
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::from(f))
        .stderr(Stdio::from(err))
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > ANSWER_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(_) => return None,
        }
    }
    drop(fork);
    let text = super::pm::answer_text(&std::fs::read_to_string(log).ok()?)?;
    parse_verdict(&text)
}

/// The answerer's final message is one JSON object, possibly fenced.
fn parse_verdict(text: &str) -> Option<Verdict> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verbs::harness::board::tests::task;
    use std::io::Write;

    fn tmp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ckit-ask-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn write_q(dir: &FsPath, n: u32, to: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let q = serde_json::json!({ "id": format!("Q-{n}"), "to": to, "question": "dùng bảng nào?", "options": ["a", "b"] });
        std::fs::write(dir.join(format!("Q-{n}.json")), q.to_string()).unwrap();
    }

    #[test]
    fn questions_route_and_answers_land_in_the_thread() {
        let root = tmp_root("route");
        let mut t = task(Stage::Dev, Status::Running);
        let dir = ask_dir(&root, &t.id);
        write_q(&dir, 1, "ba");
        write_q(&dir, 2, "boss");
        assert!(has_open_question(&dir));
        let (changed, route) = absorb(&root, &mut t);
        assert!(changed);
        assert_eq!(route.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["Q-1"], "only Leader questions spawn a session");
        assert_eq!(t.asks[0].to, "leader", "legacy ba → leader");
        assert_eq!(t.asks[1].status, "boss");
        assert!(absorb(&root, &mut t).1.is_empty(), "a question is routed once");

        answer_as(&root, &mut t, "Q-2", "bảng messages", "boss").unwrap();
        assert_eq!((t.asks[1].status.as_str(), t.asks[1].by.as_deref()), ("answered", Some("boss")));
        assert!(answer_as(&root, &mut t, "Q-2", "lại", "boss").is_err(), "answered once");
        assert!(t.thread.iter().any(|m| m.from == "boss" && m.text.contains("bảng messages")));

        write_answer(&dir, "Q-1", "", "timeout").unwrap();
        absorb(&root, &mut t);
        assert_eq!(t.asks[0].status, "timeout");
        assert!(!has_open_question(&dir));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn run_end_closes_unanswered_questions() {
        let root = tmp_root("close");
        let mut t = task(Stage::Test, Status::Running);
        write_q(&ask_dir(&root, &t.id), 1, "po");
        absorb(&root, &mut t);
        t.status = Status::Failed;
        close_open(&root, &mut t);
        assert_eq!(t.asks[0].status, "closed");
        assert!(answer_as(&root, &mut t, "Q-1", "x", "boss").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn verdict_parses_from_fenced_text() {
        let v = parse_verdict("ok\n```json\n{\"verdict\":\"escalate\",\"reason\":\"tiền\"}\n```").unwrap();
        assert_eq!((v.verdict.as_str(), v.reason.as_str()), ("escalate", "tiền"));
    }

    #[test]
    fn legacy_ba_po_route_to_the_leader_everything_else_to_the_boss() {
        let root = tmp_root("legacy");
        let mut t = task(Stage::Dev, Status::Running);
        let dir = ask_dir(&root, &t.id);
        for (n, to) in [(1, "ba"), (2, "po"), (3, "leader"), (4, "boss"), (5, "dev")] {
            write_q(&dir, n, to);
        }
        let (_, route) = absorb(&root, &mut t);
        assert_eq!(t.asks.iter().map(|a| a.to.as_str()).collect::<Vec<_>>(), ["leader", "leader", "leader", "boss", "boss"]);
        assert_eq!(route.len(), 3);
        assert!(t.thread.iter().any(|m| m.from == "dev → leader"));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn run(stage: Stage, n: u32, started: &str, ended: Option<&str>) -> super::super::Run {
        super::super::Run { stage, n, started: started.into(), ended: ended.map(Into::into), exit: None, cost: 0.0, verdict: None, log: format!("{}-{n}.jsonl", stage.key()), phase: None, round: None, title: None }
    }

    /// sessions/ of a task: intake, solution, then review starting in the second the solution ended.
    fn sessions_fixture(dir: &FsPath) -> [PathBuf; 3] {
        let s = dir.join("sessions");
        std::fs::create_dir_all(s.join("2026-10-15T16-44-28-390Z_bbb")).unwrap(); // subagent sessions
        std::fs::write(s.join(".2026-10-15T16-44-28-390Z_bbb.jsonl.lock.os"), "").unwrap();
        let files = [
            s.join("2026-10-15T16-41-41-873Z_aaa.jsonl"),
            s.join("2026-10-15T16-44-28-390Z_bbb.jsonl"),
            s.join("2026-10-15T17-27-28-595Z_ccc.jsonl"),
        ];
        for f in &files {
            std::fs::write(f, "{\"type\":\"session\"}\n{\"type\":\"plan\"}\n").unwrap();
        }
        files
    }

    #[test]
    fn fork_picks_the_plan_session_and_never_changes_it() {
        let root = tmp_root("fork");
        let mut t = task(Stage::Dev, Status::Running);
        let dir = task_dir(&root, &t.id);
        let [_, plan, _] = sessions_fixture(&dir);
        t.runs = vec![
            run(Stage::Intake, 1, "2026-10-15T16:41:41Z", Some("2026-10-15T16:44:27Z")),
            run(Stage::Solution, 1, "2026-10-15T16:44:28Z", Some("2026-10-15T17:27:28Z")),
            run(Stage::Review, 1, "2026-10-15T17:27:28Z", Some("2026-10-15T17:30:59Z")),
            run(Stage::Dev, 1, "2026-10-15T17:43:11Z", None),
        ];
        // No run log: the first session inside the Kế hoạch window (not Review's, same second).
        assert_eq!(plan_session(&dir, &t).as_deref(), Some(plan.as_path()));
        // The run log names its session: that file wins.
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/solution-1.jsonl"), "{\"type\":\"session\",\"id\":\"bbb\"}\n").unwrap();
        assert_eq!(plan_session(&dir, &t).as_deref(), Some(plan.as_path()));

        let meta = |p: &FsPath| std::fs::metadata(p).map(|m| (m.len(), m.modified().unwrap())).unwrap();
        let before = meta(&plan);
        let (a, b) = (fork_session(&plan, &t.id, "Q-1").unwrap(), fork_session(&plan, &t.id, "Q-2").unwrap());
        assert_ne!(a.0, b.0, "concurrent questions get their own copy");
        let copy = a.0.join(plan.file_name().unwrap());
        assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&plan).unwrap());
        // What `omp --continue` does to the copy.
        std::fs::OpenOptions::new().append(true).open(&copy).unwrap().write_all(b"{\"type\":\"answer\"}\n").unwrap();
        let args = answerer_args(&root, Some(&a.0), false);
        let s: Vec<String> = args.iter().map(|x| x.to_string_lossy().into_owned()).collect();
        let at = s.iter().position(|x| x == "--session-dir").unwrap();
        assert_eq!((s.contains(&"--continue".to_string()), s[at + 1].as_str()), (true, a.0.to_str().unwrap()));
        assert!(s.contains(&"--tools=read,grep,glob".to_string()) && !s.contains(&"--no-session".to_string()));
        let tmp = a.0.clone();
        drop(a);
        drop(b);
        assert!(!tmp.exists(), "temp session dir deleted");
        assert_eq!(meta(&plan), before, "the Leader's plan session is unchanged");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_a_finished_plan_run_the_leader_answers_fresh() {
        let root = tmp_root("fresh");
        let mut t = task(Stage::Dev, Status::Running);
        let dir = task_dir(&root, &t.id);
        sessions_fixture(&dir);
        // Quick task: no Kế hoạch run at all.
        t.runs = vec![run(Stage::Dev, 1, "2026-10-15T16:41:41Z", None)];
        assert!(plan_session(&dir, &t).is_none());
        // Kế hoạch still running (it asked itself): no fork of a half-written session.
        t.runs = vec![run(Stage::Solution, 1, "2026-10-15T16:44:28Z", None)];
        assert!(plan_session(&dir, &t).is_none());
        let s: Vec<String> = answerer_args(&root, None, false).iter().map(|x| x.to_string_lossy().into_owned()).collect();
        assert!(s.contains(&"--no-session".to_string()) && !s.contains(&"--continue".to_string()) && s.contains(&"--tools=read,grep,glob".to_string()));
        let ask = Ask { id: "Q-1".into(), from: Stage::Dev, to: "leader".into(), question: "tên hàm {{ID}}?".into(), options: vec![], at: String::new(), status: "answering".into(), answer: None, by: None, answered_at: None, kind: None, ac: None, phase: None };
        let p = answer_prompt(&root, &t, &ask, false).unwrap();
        assert!(p.starts_with("You are the Leader") && p.contains("tên hàm {{ID}}?"), "question is data, never expanded");
        assert!(!p.contains("{{WHO}}") && !p.contains("{{RUN}}") && !p.contains("{{THREAD}}"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn context_keeps_every_decision_and_the_last_15_others() {
        let msg = |from: &str, text: String| Msg { from: from.into(), text, at: String::new() };
        let mut thread = vec![msg("user", "yêu cầu gốc".into()), msg("leader", "quyết định D1".into()), msg("po", "trả lời cũ".into())];
        thread.extend((0..40).map(|i| msg("system", format!("note {i}"))));
        thread.push(msg("boss", "chốt: dùng bảng messages".into()));
        thread.extend((40..45).map(|i| msg("dev → leader", format!("note {i}"))));
        let c = thread_context(&thread);
        for d in ["yêu cầu gốc", "quyết định D1", "trả lời cũ", "chốt: dùng bảng messages"] {
            assert!(c.contains(d), "{d} kept");
        }
        assert_eq!(c.matches("note ").count(), 15);
        assert!(!c.contains("note 29\n") && c.contains("note 30") && c.contains("note 44"));
        assert!(c.find("quyết định D1").unwrap() < c.find("note 30").unwrap(), "oldest first");
    }
}

#[cfg(test)]
mod coord_tests {
    use super::*;
    use crate::verbs::harness::board::{coord, tests::task};

    #[test]
    fn the_coordinator_receives_no_team_question_events() {
        let root = std::env::temp_dir().join(format!("ckit-ask-coord-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        coord::set_port(1);
        coord::set_enabled(&root, true).unwrap();
        assert!(coord::enabled(&root));
        let mut t = task(Stage::Dev, Status::Running);
        save(&root, &mut t).unwrap();
        let dir = ask_dir(&root, &t.id);
        std::fs::create_dir_all(&dir).unwrap();
        for (n, to) in [(1, "po"), (2, "leader")] {
            std::fs::write(dir.join(format!("Q-{n}.json")), serde_json::json!({ "id": format!("Q-{n}"), "to": to, "question": "bảng nào?" }).to_string()).unwrap();
        }
        let routed = collect(&root);
        assert_eq!(routed.iter().map(|(_, a)| a.to.as_str()).collect::<Vec<_>>(), ["leader", "leader"]);
        assert!(coord::chat(&root, 50).is_empty() && !coord::busy(&root), "no event reached the coordinator");
        let _ = std::fs::remove_dir_all(&root);
    }
}
