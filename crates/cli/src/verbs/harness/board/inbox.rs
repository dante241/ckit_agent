//! Interjections into a running step: a note / redirect, or a pause.
//!
//! The boss (or the coordinator) posts into the task's inbox; the engine
//! extension inside the running omp polls it and injects each message into the
//! live session as a `steer` (delivered at the next tool boundary — no restart,
//! no lost context). A pause asks the agent to stop at a safe point and write
//! `{"verdict":"paused"}`; if it has not stopped after `PAUSE_GRACE` the board
//! kills the run. Resume re-runs the step with `omp --continue` (same session),
//! and the engine's task states let Dev pick up at the next sub-task.

use std::path::{Path as FsPath, PathBuf};
use std::time::Duration;

use super::{ask, iso_now, Status, Task};

pub(super) const PAUSE_GRACE: Duration = Duration::from_secs(5 * 60);

pub(super) fn inbox_dir(root: &FsPath, id: &str) -> PathBuf {
    ask::ask_dir(root, id).join("inbox")
}

/// Marker the worker watches: a pause was requested for the running step.
pub(super) fn pause_marker(root: &FsPath, id: &str) -> PathBuf {
    inbox_dir(root, id).join("PAUSE")
}

/// Who posted, as shown in the thread.
fn label(from: &str) -> &'static str {
    match from {
        "coordinator" => "coordinator",
        _ => "user",
    }
}

/// Post a note or a pause to the task. Running → delivered into the live session;
/// queued → a pause parks it right away, a note waits in the thread for the next run.
pub(super) fn post(root: &FsPath, t: &mut Task, kind: &str, text: &str, from: &str) -> Result<(), String> {
    let text = text.trim();
    if !matches!(kind, "note" | "pause") {
        return Err("kind phải là note hoặc pause".into());
    }
    if kind == "note" && text.is_empty() {
        return Err("nội dung trống".into());
    }
    let tag = if kind == "pause" { "[Tạm dừng] " } else { "" };
    match t.status {
        Status::Running => {
            let dir = inbox_dir(root, &t.id);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let body = serde_json::json!({ "kind": kind, "text": text, "from": from, "at": iso_now() }).to_string();
            let mut n = 1;
            loop {
                let path = dir.join(format!("M-{n}.json"));
                match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(mut f) => {
                        use std::io::Write;
                        f.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                    Err(e) => return Err(e.to_string()),
                }
            }
            if kind == "pause" {
                t.paused_by = from.into();
                std::fs::write(pause_marker(root, &t.id), iso_now()).map_err(|e| e.to_string())?;
                t.note = Some("đang xin tạm dừng…".into());
            }
            t.say(&format!("{} → {}", label(from), t.stage.key()), &format!("{tag}{text}"));
        }
        Status::Queued if kind == "pause" => {
            t.paused = true;
            t.paused_by = from.into();
            t.status = Status::Waiting;
            t.note = Some("Tạm dừng".into());
            t.say(label(from), &format!("{tag}{text}"));
        }
        Status::Queued | Status::Waiting => t.say(label(from), text),
        _ => return Err("task không chạy — không có gì để nhắn".into()),
    }
    Ok(())
}

/// Continue a paused step in the same omp session.
pub(super) fn resume(t: &mut Task, text: &str, from: &str) -> Result<(), String> {
    if !t.paused {
        return Err("task không ở trạng thái tạm dừng".into());
    }
    // The boss decides when work starts: the coordinator may only resume a step it paused.
    if from == "coordinator" && t.paused_by != "coordinator" {
        return Err("chỉ sếp được bắt đầu / tiếp tục việc này".into());
    }
    let text = text.trim();
    let verb = if t.runs.is_empty() { "[Bắt đầu]" } else { "[Tiếp tục]" };
    t.say(label(from), &format!("{verb} {}", if text.is_empty() { "làm tiếp từ chỗ đã dừng" } else { text }).replace("[Bắt đầu] làm tiếp từ chỗ đã dừng", "[Bắt đầu]"));
    t.paused_by.clear();
    t.paused = false;
    // Only a step that actually ran and stopped (pause, or a dashboard restart that
    // already flagged `resume`) can be continued in its session.
    t.resume = t.resume || t.runs.last().is_some_and(|r| r.stage == t.stage && r.verdict.as_deref() == Some("paused"));
    if !text.is_empty() {
        t.resume_note = if t.resume_note.is_empty() { text.to_string() } else { format!("{} Sếp dặn thêm: {text}", t.resume_note) };
    }
    t.status = Status::Queued;
    t.note = None;
    Ok(())
}

/// Age of the pause request, if one is pending.
pub(super) fn pause_age(root: &FsPath, id: &str) -> Option<Duration> {
    std::fs::metadata(pause_marker(root, id)).ok()?.modified().ok()?.elapsed().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verbs::harness::board::tests::task;
    use crate::verbs::harness::board::{after_run, Stage};

    #[test]
    fn running_pause_is_delivered_then_paused_step_resumes_in_its_session() {
        let root = std::env::temp_dir().join(format!("ckit-inbox-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = task(Stage::Dev, Status::Running);
        post(&root, &mut t, "note", "dùng bảng X", "boss").unwrap();
        post(&root, &mut t, "pause", "", "coordinator").unwrap();
        let dir = inbox_dir(&root, &t.id);
        assert!(dir.join("M-1.json").exists() && dir.join("M-2.json").exists(), "each message its own file");
        assert!(pause_age(&root, &t.id).is_some());

        t.runs.push(crate::verbs::harness::board::Run {
            stage: Stage::Dev, n: 1, started: String::new(), ended: None, exit: None, cost: 0.0, verdict: Some("paused".into()), log: String::new(),
        });
        after_run(&mut t, "paused", None);
        assert_eq!((t.stage, t.status, t.paused), (Stage::Dev, Status::Waiting, true));
        assert!(post(&root, &mut t, "pause", "", "boss").is_ok(), "pause while waiting is just a note");
        resume(&mut t, "", "boss").unwrap();
        assert_eq!((t.status, t.paused, t.resume), (Status::Queued, false, true));
        assert!(resume(&mut t, "", "boss").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn queued_pause_parks_without_a_session_to_continue() {
        let root = std::env::temp_dir().join(format!("ckit-inbox-q-{}", std::process::id()));
        let mut t = task(Stage::Test, Status::Queued);
        post(&root, &mut t, "pause", "chờ tôi", "boss").unwrap();
        assert_eq!((t.status, t.paused), (Status::Waiting, true));
        resume(&mut t, "", "boss").unwrap();
        assert!(!t.resume, "never ran → fresh run, not --continue");
        assert!(post(&root, &mut task(Stage::Done, Status::Done), "note", "x", "boss").is_err());
    }
}
