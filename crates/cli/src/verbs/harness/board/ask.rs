//! `board_ask` — team members ask each other mid-step.
//!
//! A running step calls the `board_ask` tool (engine extension), which writes
//! `ask/Q-<n>.json` and waits for `ask/A-<n>.json`. The worker routes each new
//! question: to the boss (dashboard), or to a short read-only BA / PO session
//! that answers or escalates to the boss. Those sessions bypass the project's
//! single run slot — the asking run holds it while it waits, so queueing the
//! answer behind it would deadlock.

use std::collections::HashSet;
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{guard, iso_now, list, load, save, task_dir, Stage, Status, Task};
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
    /// answering (BA/PO session) · boss · answered · timeout · closed (run ended first)
    pub status: String,
    pub answer: Option<String>,
    pub by: Option<String>,
    pub answered_at: Option<String>,
}

#[derive(Deserialize)]
struct QFile {
    id: String,
    to: String,
    question: String,
    #[serde(default)]
    options: Vec<String>,
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
/// the tool's timeout, the BA/PO session and the boss may race; the first one to
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
/// questions that need a BA/PO session. Caller holds the board lock and saves.
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
            let to = if matches!(q.to.as_str(), "ba" | "po") { q.to.clone() } else { "boss".to_string() };
            let ask = Ask {
                id: q.id,
                from: t.stage,
                to: to.clone(),
                question: q.question,
                options: q.options,
                at: iso_now(),
                status: if !running { "closed" } else if to == "boss" { "boss" } else { "answering" }.into(),
                answer: None,
                by: None,
                answered_at: None,
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

/// The run ended (done, failed, cancelled): unanswered questions are closed.
pub(super) fn close_open(root: &FsPath, t: &mut Task) {
    let _ = absorb(root, t);
    let dir = ask_dir(root, &t.id);
    for a in t.asks.iter_mut().filter(|a| matches!(a.status.as_str(), "answering" | "boss")) {
        a.status = "closed".into();
        // Publish an answer file so the question no longer counts as open (the
        // timeout clock and later runs check `has_open_question`).
        let _ = write_answer(&dir, &a.id, "", "closed");
    }
}

/// Worker tick: route new questions, record answers.
pub(super) fn sync(root: &FsPath) {
    let routes: Vec<(String, Ask)> = {
        let _g = guard();
        let mut out = Vec::new();
        for mut t in list(root).into_iter().filter(|t| t.status == Status::Running) {
            let (changed, route) = absorb(root, &mut t);
            if changed {
                let _ = save(root, &mut t);
            }
            out.extend(route.into_iter().map(|a| (t.id.clone(), a)));
        }
        out
    };
    for (id, ask) in routes {
        // With the coordinator on, PO questions go to it: it already holds every
        // task's context. BA questions keep their own short session.
        if ask.to == "po" && super::coord::enabled(root) {
            let opts = if ask.options.is_empty() { String::new() } else { format!(" Lựa chọn: {}.", ask.options.join(" · ")) };
            super::coord::event(
                root,
                format!(
                    "{id} {}: bước {} hỏi PO (đang chờ, tối đa ~20 phút): \"{}\".{opts} Trả lời bằng board_answer, hoặc board_escalate nếu chỉ sếp được quyết.",
                    ask.id,
                    ask.from.key(),
                    ask.question
                ),
            );
            continue;
        }
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

/// The coordinator hands a question to the boss.
pub(super) fn escalate(t: &mut Task, qid: &str, reason: &str, from: &str) -> Result<(), String> {
    let a = t
        .asks
        .iter_mut()
        .find(|a| a.id == qid && a.status == "answering")
        .ok_or_else(|| format!("{qid} không còn chờ trả lời"))?;
    a.status = "boss".into();
    t.say(from, &format!("[{qid}] chuyển cho sếp: {}", reason.trim()));
    Ok(())
}

/// Answer from the dashboard (boss) or the coordinator (as po/ba); pre-empts a BA/PO session.
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

/// Run the BA / PO session for one question; answer, or escalate to the boss.
fn answer(root: &FsPath, id: &str, ask: &Ask) {
    let Some(t) = load(root, id) else { return };
    let dir = ask_dir(root, id);
    let n = ask.id.trim_start_matches("Q-");
    let log = dir.join(format!("{}-{n}.jsonl", ask.to));
    let verdict = run_answerer(root, &t, ask, &log);
    let _g = guard();
    let Some(mut cur) = load(root, id) else { return };
    cur.recost(root);
    let still_open = cur.asks.iter().any(|a| a.id == ask.id && a.status == "answering");
    if still_open {
        match verdict {
            Some(v) if v.verdict == "answer" && !v.answer.trim().is_empty() => {
                let _ = write_answer(&dir, &ask.id, v.answer.trim(), &ask.to);
                let _ = absorb(root, &mut cur);
            }
            v => {
                let reason = v.map(|v| v.reason).filter(|r| !r.trim().is_empty()).unwrap_or_else(|| "không trả lời được".into());
                if let Some(a) = cur.asks.iter_mut().find(|a| a.id == ask.id) {
                    a.status = "boss".into();
                }
                cur.say(&ask.to, &format!("[{}] chuyển cho sếp: {reason}", ask.id));
            }
        }
    }
    let _ = save(root, &mut cur);
}

fn run_answerer(root: &FsPath, t: &Task, ask: &Ask, log: &FsPath) -> Option<Verdict> {
    let tpl = crate::assets::read("board/answer.md")?;
    let role = if ask.to == "ba" { "BA (business analyst)" } else { "PO (the Leader, product owner)" };
    let thread = t.thread.iter().rev().take(30).rev().map(|m| format!("[{}] {}", m.from, m.text)).collect::<Vec<_>>().join("\n\n");
    let fdir = super::feature_dir(root, t).map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into());
    let prompt = tpl
        .replace("{{ROLE}}", role)
        .replace("{{FROM}}", ask.from.key())
        .replace("{{QUESTION}}", &ask.question)
        .replace("{{OPTIONS}}", &if ask.options.is_empty() { "(none)".into() } else { ask.options.join(" · ") })
        .replace("{{ID}}", &t.id)
        .replace("{{TITLE}}", &t.title)
        .replace("{{REQUEST}}", &t.request)
        .replace("{{THREAD}}", &thread)
        .replace("{{FEATURE_DIR}}", &fdir)
        .replace("{{DIR}}", &task_dir(root, &t.id).display().to_string());
    let f = std::fs::File::create(log).ok()?;
    let err = f.try_clone().ok()?;
    // Read-only and isolated: no write/edit/bash tools, no extensions (the engine
    // tools commit), so it can never touch the tree the asking run is editing.
    let mut child = super::omp_cmd()
        .current_dir(root)
        .arg("--cwd")
        .arg(root)
        .args(["--mode", "json", "--no-session", "--no-extensions", "--no-skills", "--tools=read,grep,glob"])
        .args(ModelConfig::load().class_flags(TaskClass::Plan))
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
        assert_eq!(route.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["Q-1"], "only BA/PO questions spawn a session");
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
}
