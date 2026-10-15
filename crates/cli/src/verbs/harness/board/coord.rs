//! The coordinator ("Điều phối viên"): one standing agent per project that the
//! boss chats with and that the board wakes on events (a teammate's question to
//! the PO, a plan waiting for approval, a stalled or straying run, periodic
//! oversight). It is ONE omp session kept on disk (`omp --continue`), run a turn
//! at a time — standing without a resident process. It acts only through the
//! board's own API (tools in `assets/extensions/ckit-board.ts`) plus read-only
//! code tools: it never edits code and never merges.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{board_dir, iso_now, list, log_cost, Stage, Status};
use crate::models::{ModelConfig, TaskClass};

const TURN_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// How often a running Dev/Test step is reviewed against its plan (only if it moved).
const OVERSIGHT: Duration = Duration::from_secs(10 * 60);
const TOOLS: &str = "read,grep,glob,board_status,board_task,board_create,board_message,board_resume,board_answer,board_escalate,board_approve";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct ChatMsg {
    /// boss · coordinator · event
    pub from: String,
    pub text: String,
    pub at: String,
    #[serde(default)]
    pub cost: f64,
    /// `telegram` when the boss wrote from Telegram: the reply goes back there too.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub via: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    #[serde(default = "yes")]
    enabled: bool,
}

/// Opt-in per project: it spends money and acts (approves plans, pauses runs).
fn yes() -> bool {
    false
}

#[derive(Default)]
struct State {
    queue: HashMap<PathBuf, Vec<ChatMsg>>,
    busy: HashMap<PathBuf, bool>,
    /// `<root>|<task>|<run log>` → (last review, log size then)
    watched: HashMap<String, (Instant, u64)>,
    /// Roots whose chat log was re-queued after (re)start.
    recovered: std::collections::HashSet<PathBuf>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

static API_PORT: OnceLock<u16> = OnceLock::new();

/// The dashboard's port — the coordinator's tools call the board API there.
pub(crate) fn set_port(port: u16) {
    let _ = API_PORT.set(port);
}

fn dir(root: &FsPath) -> PathBuf {
    board_dir(root).join("_coord")
}

pub(super) fn enabled(root: &FsPath) -> bool {
    std::fs::read_to_string(dir(root).join("config.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .map(|c| c.enabled)
        .unwrap_or(false)
        && API_PORT.get().is_some()
}

pub(super) fn set_enabled(root: &FsPath, on: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(root))?;
    std::fs::write(dir(root).join("config.json"), serde_json::to_vec(&serde_json::json!({ "enabled": on }))?)
}

fn append_chat(root: &FsPath, m: &ChatMsg) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(dir(root));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir(root).join("chat.jsonl")) {
        let _ = writeln!(f, "{}", serde_json::to_string(m).unwrap_or_default());
    }
}

pub(super) fn chat(root: &FsPath, limit: usize) -> Vec<ChatMsg> {
    let raw = std::fs::read_to_string(dir(root).join("chat.jsonl")).unwrap_or_default();
    let all: Vec<ChatMsg> = raw.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    all[all.len().saturating_sub(limit)..].to_vec()
}

pub(super) fn busy(root: &FsPath) -> bool {
    let st = state();
    st.busy.get(root).copied().unwrap_or(false) || st.queue.get(root).is_some_and(|q| !q.is_empty())
}

/// The boss writes to the coordinator.
pub(super) fn say(root: &FsPath, text: &str) -> Result<(), String> {
    say_via(root, text, "")
}

pub(super) fn say_via(root: &FsPath, text: &str, via: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("nội dung trống".into());
    }
    if API_PORT.get().is_none() {
        return Err("điều phối chưa sẵn sàng".into());
    }
    let m = ChatMsg { from: "boss".into(), text: text.into(), at: iso_now(), cost: 0.0, via: via.into() };
    // Talking to it is the opt-in for this project.
    if !enabled(root) {
        set_enabled(root, true).map_err(|e| e.to_string())?;
    }
    append_chat(root, &m);
    state().queue.entry(root.to_path_buf()).or_default().push(m);
    Ok(())
}

/// Wake the coordinator on a board event (no-op when it is switched off).
pub(super) fn event(root: &FsPath, text: String) {
    if !enabled(root) {
        return;
    }
    let m = ChatMsg { from: "event".into(), text, at: iso_now(), cost: 0.0, via: String::new() };
    append_chat(root, &m);
    state().queue.entry(root.to_path_buf()).or_default().push(m);
}

/// After a dashboard restart the in-memory queue is gone: re-queue the inputs
/// in the chat log that came after the coordinator's last reply. Once per root.
fn recover(root: &FsPath) {
    {
        let mut st = state();
        if !st.recovered.insert(root.to_path_buf()) {
            return;
        }
    }
    let chat = chat(root, 200);
    let start = chat.iter().rposition(|m| m.from == "coordinator").map_or(0, |i| i + 1);
    let pending: Vec<ChatMsg> = chat[start..].to_vec();
    if !pending.is_empty() {
        state().queue.entry(root.to_path_buf()).or_default().splice(0..0, pending);
    }
}

/// Worker tick: oversight events, then run one queued turn if idle.
pub(super) fn tick(root: &FsPath) {
    if !enabled(root) {
        state().queue.remove(root);
        return;
    }
    recover(root);
    oversee(root);
    let batch = {
        let mut st = state();
        if st.busy.get(root).copied().unwrap_or(false) {
            return;
        }
        let q = st.queue.remove(root).unwrap_or_default();
        if q.is_empty() {
            return;
        }
        st.busy.insert(root.to_path_buf(), true);
        q
    };
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        run_turn(&root, &batch);
        state().busy.insert(root, false);
    });
}

/// Every `OVERSIGHT`, ask the coordinator to look at each running Dev/Test step
/// that produced new output since the last look.
fn oversee(root: &FsPath) {
    for t in list(root).into_iter().filter(|t| t.status == Status::Running && matches!(t.stage, Stage::Dev | Stage::Test)) {
        let Some(run) = t.runs.last() else { continue };
        let size = std::fs::metadata(super::task_dir(root, &t.id).join("logs").join(&run.log)).map(|m| m.len()).unwrap_or(0);
        let key = format!("{}|{}|{}", root.display(), t.id, run.log);
        let due = {
            let mut st = state();
            let (last, seen) = *st.watched.entry(key.clone()).or_insert((Instant::now(), size));
            let due = last.elapsed() >= OVERSIGHT && size > seen;
            if due {
                st.watched.insert(key, (Instant::now(), size));
            }
            due
        };
        if due {
            event(root, format!("Giám sát định kỳ: {} \"{}\" đang ở bước {}. Xem tiến độ/log (board_task) và đối chiếu kế hoạch; chỉ can thiệp (board_message / pause) nếu thật sự lệch hướng, lặp lỗi hoặc đốt tiền vô ích.", t.id, t.title, t.stage.key()));
        }
    }
}

/// The tools extension, materialized next to the board state (never in an omp
/// extensions dir — it must not load into the boss's own sessions).
fn tools_ext(root: &FsPath) -> Option<PathBuf> {
    let body = crate::assets::read("extensions/ckit-board.ts")?;
    let path = dir(root).join("ckit-board.ts");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
        std::fs::create_dir_all(dir(root)).ok()?;
        std::fs::write(&path, body).ok()?;
    }
    Some(path)
}

fn run_turn(root: &FsPath, batch: &[ChatMsg]) {
    let d = dir(root);
    let sessions = d.join("sessions");
    let fresh = std::fs::read_dir(&sessions).map(|mut r| r.next().is_none()).unwrap_or(true);
    let inputs = batch
        .iter()
        .map(|m| format!("[{} · {}] {}", if m.from == "boss" { "Sếp" } else { "Sự kiện board" }, m.at, m.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let snapshot = list(root)
        .iter()
        .rev()
        .take(15)
        .map(|t| format!("- {} [{} · {}{}] {}", t.id, t.stage.key(), status_key(t.status), if t.paused { " · tạm dừng" } else { "" }, t.title))
        .collect::<Vec<_>>()
        .join("\n");
    let mut prompt = String::new();
    if fresh {
        prompt.push_str(&crate::assets::read("board/coordinator.md").unwrap_or_default());
        let policy = dirs::home_dir().map(|h| h.join(".config/ckit/board-policy.md")).and_then(|p| std::fs::read_to_string(p).ok());
        if let Some(p) = policy {
            prompt.push_str(&format!("\n\n## Luật uỷ quyền riêng của sếp (ưu tiên hơn mặc định)\n{p}"));
        }
        prompt.push_str("\n\n---\n");
    }
    prompt.push_str(&format!(
        "Bảng hiện tại (dự án {}):\n{}\n\nTin mới:\n{}\n\nXử lý bằng tool board_* nếu cần, rồi trả lời sếp ngắn gọn bằng tiếng Việt (câu trả lời cuối của bạn hiện trong khung chat điều phối). Sự kiện không cần sếp biết thì trả lời 1 dòng.",
        root.display(),
        if snapshot.is_empty() { "(chưa có task)".into() } else { snapshot },
        inputs
    ));
    let Some(ext) = tools_ext(root) else { return };
    let logs = d.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let log = logs.join(format!("turn-{}.jsonl", iso_now().replace(':', "")));
    let reply = spawn(root, &sessions, fresh, &ext, &prompt, &log);
    let cost = log_cost(&log);
    let text = reply.unwrap_or_else(|| "(điều phối không trả lời được — xem log _coord/logs)".into());
    if batch.iter().any(|m| m.via == "telegram") {
        super::telegram::tell(&format!("🧭 Điều phối [{}]:\n{text}", root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    }
    append_chat(root, &ChatMsg { from: "coordinator".into(), text, at: iso_now(), cost, via: String::new() });
}

fn status_key(s: Status) -> &'static str {
    match s {
        Status::Queued => "chờ chạy",
        Status::Running => "đang chạy",
        Status::Waiting => "chờ sếp",
        Status::Failed => "lỗi",
        Status::Done => "xong",
        Status::Cancelled => "huỷ",
    }
}

fn spawn(root: &FsPath, sessions: &FsPath, fresh: bool, ext: &FsPath, prompt: &str, log: &FsPath) -> Option<String> {
    let port = *API_PORT.get()?;
    let f = std::fs::File::create(log).ok()?;
    let err = f.try_clone().ok()?;
    let mut cmd = super::omp_cmd();
    cmd.current_dir(root)
        .arg("--cwd")
        .arg(root)
        .args(["--mode", "json", "--approval-mode", "yolo", "--no-extensions", "--no-skills", "--session-dir"])
        .arg(sessions)
        .arg("-e")
        .arg(ext)
        .arg(format!("--tools={TOOLS}"))
        .args(ModelConfig::load().class_flags(TaskClass::Plan))
        .env("CKIT_BOARD_API", format!("http://127.0.0.1:{port}"))
        .env("CKIT_BOARD_ROOT", root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(f))
        .stderr(Stdio::from(err));
    if !fresh {
        cmd.arg("--continue");
    }
    let mut child = cmd.arg("-p").arg(prompt).spawn().ok()?;
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
    super::pm::answer_text(&std::fs::read_to_string(log).ok()?)
}
