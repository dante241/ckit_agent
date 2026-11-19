//! Telegram: tell the boss when a task needs them, and act on their answer there.
//!
//! One bot, paired to ONE chat (the boss sends `/start <code>` shown in the
//! dashboard; every other chat is ignored). The worker watches every board task
//! and sends a message when a task newly needs the boss: plan approval,
//! acceptance (= create the MR), a question from the team, intake questions, an
//! investigate answer, a parked task to start/continue, a failure. Buttons act
//! directly; replying to a message answers that gate/question; any other text
//! goes to the project's coordinator. HTTP via `curl` (no HTTP client crate).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path as FsPath, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::{approve, approve_and_code, ask, continue_plan, coord, list, reply, retry, roots, update, ApiRes, RootQ, Stage, Status, Task};

const MAX_TEXT: usize = 3500;
const KEEP_MSGS: usize = 500;

#[derive(Serialize, Deserialize, Default, Clone)]
struct Config {
    token: String,
    #[serde(default)]
    chat_id: Option<i64>,
    #[serde(default)]
    pair_code: Option<String>,
    #[serde(default)]
    bot: String,
}

/// A sent message the boss may reply to / press buttons on.
#[derive(Serialize, Deserialize, Clone)]
struct Sent {
    mid: i64,
    root: PathBuf,
    id: String,
    /// approval · acceptance · intake · investigate · ask · start · resume · failed · done
    kind: String,
    #[serde(default)]
    qid: Option<String>,
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    offset: i64,
    msgs: Vec<Sent>,
    /// `<root>|<task>` → attention key already notified.
    keys: HashMap<String, String>,
    last_root: Option<PathBuf>,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}
fn config_path() -> PathBuf {
    crate::brand::config_dir(&home()).join("telegram.json")
}
fn state_path() -> PathBuf {
    crate::brand::config_dir(&home()).join("telegram-state.json")
}

fn load_config() -> Option<Config> {
    std::fs::read_to_string(config_path()).ok().and_then(|s| serde_json::from_str::<Config>(&s).ok()).filter(|c| !c.token.is_empty())
}

fn save_config(c: &Config) -> std::io::Result<()> {
    let p = config_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&p, serde_json::to_vec_pretty(c)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)); // holds the bot token
    }
    Ok(())
}

/// One lock for state.json: the watcher and the poller both write it.
fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
}
fn load_state() -> State {
    std::fs::read_to_string(state_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_state(s: &State) {
    let _ = std::fs::write(state_path(), serde_json::to_vec(s).unwrap_or_default());
}

/// POST a Bot API method; `Some(result)` when Telegram says ok.
fn call(token: &str, method: &str, body: &serde_json::Value, timeout: u64) -> Option<serde_json::Value> {
    // The URL carries the bot token: hand it to curl in a 0600 config file, never
    // on its command line (visible to every local process via `ps`).
    // `CKIT_TELEGRAM_API`: point at a local stand-in (tests, self-hosted Bot API server).
    let base = std::env::var("CKIT_TELEGRAM_API").unwrap_or_else(|_| "https://api.telegram.org".into());
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cfg = crate::brand::config_dir(&home()).join(format!(".tg-curl-{}-{n}", std::process::id()));
    {
        let _ = std::fs::create_dir_all(crate::brand::config_dir(&home()));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&cfg).ok()?;
        writeln!(f, "url = \"{base}/bot{token}/{method}\"").ok()?;
    }
    let child = Command::new("curl")
        .args(["-sS", "-m", &timeout.to_string(), "-X", "POST", "-H", "content-type: application/json", "--data-binary", "@-", "-K"])
        .arg(&cfg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let out = child.ok().and_then(|mut c| {
        c.stdin.take()?.write_all(body.to_string().as_bytes()).ok()?;
        c.wait_with_output().ok()
    });
    let _ = std::fs::remove_file(&cfg);
    let v: serde_json::Value = serde_json::from_slice(&out?.stdout).ok()?;
    (v.get("ok").and_then(|o| o.as_bool()) == Some(true)).then(|| v.get("result").cloned().unwrap_or_default())
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn project_name(root: &FsPath) -> String {
    root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn last_from(t: &Task, from: &str) -> String {
    t.thread.iter().rev().find(|m| m.from == from).map(|m| m.text.clone()).unwrap_or_default()
}

/// What the boss is needed for on this task, if anything: (key, kind, body, buttons, qid, options).
struct Attention {
    key: String,
    kind: &'static str,
    body: String,
    buttons: Vec<(String, String)>,
    qid: Option<String>,
    options: Vec<String>,
}

fn attention(t: &Task) -> Option<Attention> {
    let base = |kind: &'static str, body: String, buttons: Vec<(&str, &str)>| Attention {
        key: format!("{kind}|{}|{}|{}", t.stage.key(), t.runs.len(), t.thread.len().min(1)),
        kind,
        body,
        buttons: buttons.into_iter().map(|(l, d)| (l.to_string(), d.to_string())).collect(),
        qid: None,
        options: Vec::new(),
    };
    if let Some(a) = t.asks.iter().find(|a| a.status == "boss") {
        let escalated = if a.to == "boss" { "" } else { " (được Leader chuyển lên)" };
        let body = if a.is_ac_change() {
            format!("🔁 {} xin đổi AC, chỉ bạn duyệt được [{}]:\n{}", a.from.label(), a.id, a.question)
        } else {
            format!("❓ {} hỏi bạn{escalated} [{}]:\n{}", a.from.label(), a.id, a.question)
        };
        return Some(Attention {
            key: format!("ask|{}", a.id),
            kind: "ask",
            body,
            buttons: a.options.iter().enumerate().map(|(i, o)| (clip(o, 40), format!("o{i}"))).collect(),
            qid: Some(a.id.clone()),
            options: a.options.clone(),
        });
    }
    match (t.status, t.stage) {
        (Status::Waiting, _) if t.paused && t.runs.is_empty() => Some(base("start", "🆕 Việc mới chờ bạn bấm Bắt đầu.".into(), vec![("▶️ Bắt đầu", "st")])),
        (Status::Waiting, _) if t.paused => Some(base("resume", format!("⏸ Đang tạm dừng ({}).\n{}", t.stage.label(), t.note.clone().unwrap_or_default()), vec![("▶️ Tiếp tục", "st")])),
        (Status::Waiting, Stage::Approval) => {
            // A plan-only task (no Dev): the boss may approve and go on to code it.
            let mut buttons = vec![("✅ Duyệt kế hoạch", "ap")];
            if !t.has(Stage::Dev) {
                buttons.push(("🛠 Duyệt + code", "ac"));
            }
            Some(base("approval", format!("📋 Chờ bạn duyệt kế hoạch.\n\n{}", last_from(t, "leader")), buttons))
        }
        (Status::Waiting, Stage::Acceptance) => Some(base(
            "acceptance",
            format!("📦 Xong, chờ bạn nghiệm thu: tạo merge request từ {}.\n\n{}", t.branch.clone().unwrap_or_default(), last_from(t, "leader")),
            vec![("🔀 Tạo MR (nghiệm thu)", "mg")],
        )),
        (Status::Waiting, Stage::Intake) => Some(base("intake", format!("❓ Leader cần làm rõ yêu cầu:\n\n{}", last_from(t, "leader")), vec![])),
        (Status::Waiting, Stage::Investigate) => {
            let more = t.steps.iter().any(|s| super::catalog_pos(*s) > super::catalog_pos(Stage::Investigate));
            Some(base("investigate", format!("🔎 Trả lời điều tra:\n\n{}", last_from(t, "leader")), vec![if more { ("➡️ Tiếp tục", "ap") } else { ("✔️ Đóng", "ap") }]))
        }
        (Status::Failed, _) => Some(base("failed", format!("⚠️ Lỗi ở bước {}: {}", t.stage.label(), t.note.clone().unwrap_or_default()), vec![("🔁 Chạy lại", "rt")])),
        (Status::Done, _) => {
            // The endpoint re-checks the branch (still there, unmerged) on press.
            let plan_only = t.has(Stage::Solution) && !t.has(Stage::Dev) && t.branch.is_some();
            let buttons = if plan_only { vec![("🛠 Code theo kế hoạch này", "cc")] } else { vec![] };
            Some(base("done", format!("✅ Xong.\n{}", clip(&last_from(t, "system"), 400)), buttons))
        }
        _ => None,
    }
}

const HOW: &str = "\n\n↩️ Trả lời (reply) tin này để góp ý / trả lời.";

/// Worker tick: notify each task that newly needs the boss. Runs on its own
/// thread per project so a slow Telegram never stalls the pipeline worker.
pub(super) fn watch(root: &FsPath) {
    static BUSY: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    let busy = BUSY.get_or_init(Default::default);
    if load_config().and_then(|c| c.chat_id).is_none() || !busy.lock().map(|mut b| b.insert(root.to_path_buf())).unwrap_or(false) {
        return;
    }
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        notify_root(&root);
        busy.lock().map(|mut b| b.remove(&root)).ok();
    });
}

fn notify_root(root: &FsPath) {
    let Some(cfg) = load_config() else { return };
    let Some(chat) = cfg.chat_id else { return };
    for t in list(root) {
        let k = format!("{}|{}", root.display(), t.id);
        let att = attention(&t);
        let new_key = att.as_ref().map(|a| a.key.clone()).unwrap_or_default();
        {
            let _g = state_lock();
            let mut st = load_state();
            let old = st.keys.get(&k).cloned();
            if old.as_deref() == Some(new_key.as_str()) {
                continue;
            }
            st.keys.insert(k.clone(), new_key.clone());
            save_state(&st);
            // First sight of a task that already finished: record, don't announce history.
            if old.is_none() && matches!(t.status, Status::Done | Status::Cancelled) {
                continue;
            }
        }
        let Some(a) = att else { continue };
        let text = clip(
            &format!("[{}] {} · {}\n{}{}", project_name(root), t.id, t.title, a.body, if a.kind == "done" { "" } else { HOW }),
            MAX_TEXT,
        );
        let kb: Vec<Vec<serde_json::Value>> = a.buttons.iter().map(|(l, d)| vec![serde_json::json!({ "text": l, "callback_data": d })]).collect();
        let mut body = serde_json::json!({ "chat_id": chat, "text": text, "disable_web_page_preview": true });
        if !kb.is_empty() {
            body["reply_markup"] = serde_json::json!({ "inline_keyboard": kb });
        }
        if let Some(msg) = call(&cfg.token, "sendMessage", &body, 20) {
            let mid = msg.get("message_id").and_then(|m| m.as_i64()).unwrap_or(0);
            let _g = state_lock();
            let mut st = load_state();
            st.msgs.push(Sent { mid, root: root.to_path_buf(), id: t.id.clone(), kind: a.kind.into(), qid: a.qid, options: a.options });
            let n = st.msgs.len();
            if n > KEEP_MSGS {
                st.msgs.drain(..n - KEEP_MSGS);
            }
            st.last_root = Some(root.to_path_buf());
            save_state(&st);
        }
    }
}

/// Start the long-poll thread (one per dashboard). Idle until a bot is configured.
pub(super) fn start() {
    // A dashboard killed mid-request leaves its curl config (it holds the token);
    // the PMS client's configs (session token) live in the same directory.
    for e in std::fs::read_dir(crate::brand::config_dir(&home())).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(".tg-curl-") || name.starts_with(super::pms::CURL_CFG_PREFIX) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    std::thread::spawn(|| loop {
        let Some(cfg) = load_config() else {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };
        let offset = load_state().offset;
        let body = serde_json::json!({ "offset": offset, "timeout": 25, "allowed_updates": ["message", "callback_query"] });
        let Some(updates) = call(&cfg.token, "getUpdates", &body, 35).and_then(|v| v.as_array().cloned()) else {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };
        for u in updates {
            let uid = u.get("update_id").and_then(|x| x.as_i64()).unwrap_or(0);
            {
                let _g = state_lock();
                let mut st = load_state();
                st.offset = st.offset.max(uid + 1);
                save_state(&st);
            }
            handle(&cfg, &u);
        }
    });
}

/// Send a plain message to the paired chat (e.g. the coordinator's reply).
pub(super) fn tell(text: &str) {
    if let Some((cfg, chat)) = load_config().and_then(|c| c.chat_id.map(|id| (c, id))) {
        send(&cfg, chat, text, None);
    }
}

fn send(cfg: &Config, chat: i64, text: &str, reply_to: Option<i64>) {
    let mut body = serde_json::json!({ "chat_id": chat, "text": clip(text, MAX_TEXT) });
    if let Some(r) = reply_to {
        body["reply_parameters"] = serde_json::json!({ "message_id": r, "allow_sending_without_reply": true });
    }
    let _ = call(&cfg.token, "sendMessage", &body, 20);
}

fn find_sent(mid: i64) -> Option<Sent> {
    load_state().msgs.into_iter().rev().find(|s| s.mid == mid)
}

/// Run a board action on the task behind a sent message.
fn act(s: &Sent, f: impl FnOnce(&FsPath, &mut Task) -> Result<(), String>) -> Result<(), String> {
    let q = RootQ { root: Some(s.root.display().to_string()) };
    update(&q, &s.id, f).map(|_| ()).map_err(|(_, e)| e)
}

fn handle(cfg: &Config, u: &serde_json::Value) {
    if let Some(cb) = u.get("callback_query") {
        let chat = cb.pointer("/message/chat/id").and_then(|x| x.as_i64());
        if chat.is_none() || chat != cfg.chat_id {
            return; // only the paired chat may act
        }
        let chat = chat.unwrap_or_default();
        let id = cb.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let mid = cb.pointer("/message/message_id").and_then(|x| x.as_i64()).unwrap_or(0);
        let data = cb.get("data").and_then(|x| x.as_str()).unwrap_or("");
        let result = match find_sent(mid) {
            None => Err("tin cũ, mở dashboard để thao tác".to_string()),
            Some(s) => on_button(cfg, chat, mid, &s, data),
        };
        let note = match &result {
            Ok(m) => m.clone(),
            Err(e) => format!("Không làm được: {e}"),
        };
        let _ = call(&cfg.token, "answerCallbackQuery", &serde_json::json!({ "callback_query_id": id, "text": clip(&note, 190) }), 15);
        if !note.is_empty() {
            send(cfg, chat, &note, Some(mid));
        }
        return;
    }
    let Some(m) = u.get("message") else { return };
    let chat = m.pointer("/chat/id").and_then(|x| x.as_i64()).unwrap_or(0);
    let text = m.get("text").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    // Pairing: the code shown in the dashboard binds this chat.
    if cfg.chat_id.is_none() {
        if let (Some(code), Some(arg)) = (&cfg.pair_code, text.strip_prefix("/start")) {
            if arg.trim() == code {
                let mut c = cfg.clone();
                c.chat_id = Some(chat);
                c.pair_code = None;
                if save_config(&c).is_ok() {
                    send(&c, chat, "✅ Đã kết nối ckit Board. Bạn sẽ nhận tin khi có việc cần duyệt / trả lời.", None);
                }
            }
        }
        return;
    }
    if Some(chat) != cfg.chat_id || text.is_empty() {
        return;
    }
    let mid = m.get("message_id").and_then(|x| x.as_i64());
    if text == "/status" || text == "/start" {
        send(cfg, chat, &status_text(), mid);
        return;
    }
    let replied = m.pointer("/reply_to_message/message_id").and_then(|x| x.as_i64()).and_then(find_sent);
    let out = match replied {
        Some(s) if s.kind == "ask" => {
            let qid = s.qid.clone().unwrap_or_default();
            act(&s, |root, t| ask::answer_as(root, t, &qid, &text, "boss")).map(|_| format!("Đã gửi câu trả lời {qid} cho {}.", s.id))
        }
        // A finished task: the reply is a chat message to its Leader (answer comes back here).
        Some(s) => act(&s, |root, t| if to_chat(&s.kind, t.status) { super::chat::post(root, t, &text, Some("telegram")) } else { reply(t, &text) })
            .map(|_| format!("Đã chuyển cho {} ({}).", s.id, project_name(&s.root))),
        // Free text → the coordinator of the project last heard from.
        None => match load_state().last_root.or_else(|| roots(&home()).into_iter().next()) {
            Some(root) => coord::say_via(&root, &text, "telegram").map(|_| format!("Đã chuyển cho Điều phối dự án {}, trả lời sẽ gửi về đây.", project_name(&root))),
            None => Err("chưa có dự án nào trên Board".into()),
        },
    };
    send(cfg, chat, &out.unwrap_or_else(|e| format!("Không làm được: {e}")), mid);
}

/// A reply to a task notification (not a question card) goes to the task's Leader
/// chat once the task is done; before that it answers the task's gate.
fn to_chat(kind: &str, status: Status) -> bool {
    kind == "done" || status == Status::Done
}

fn on_button(cfg: &Config, chat: i64, mid: i64, s: &Sent, data: &str) -> Result<String, String> {
    match data {
        "ap" => act(s, |root, t| approve(root, t, false)).map(|_| format!("✓ {} đã duyệt — team làm tiếp.", s.id)),
        "ac" => act(s, approve_and_code).map(|_| format!("🛠 {} đã duyệt và bật code — team bắt đầu code.", s.id)),
        "cc" => act(s, |root, t| continue_plan(root, t, "Telegram")).map(|_| format!("🛠 {} code tiếp theo kế hoạch đã duyệt.", s.id)),
        "st" => act(s, |_, t| super::inbox::resume(t, "", "boss")).map(|_| format!("▶️ {} đã bắt đầu / chạy tiếp.", s.id)),
        "rt" => act(s, |_, t| retry(t)).map(|_| format!("🔁 {} chạy lại.", s.id)),
        // Pushing is not undoable from here: show what goes out, ask once more.
        "mg" => {
            let t = super::load(&s.root, &s.id).ok_or("task không còn")?;
            let text = match &t.mr {
                Some(m) => format!("Nghiệm thu {}: đã có MR {}. Bấm để đánh dấu xong (đẩy thêm commit mới nếu có).", s.id, m.url),
                None => {
                    let mut p = super::action::mr_proposal(&t);
                    super::chat::check(&s.root, &t, &mut p);
                    if let Some(e) = p.check_error {
                        return Err(format!("chưa tạo MR được: {e}"));
                    }
                    let br = t.branch.clone().unwrap_or_default();
                    let mine = p.commits.iter().filter(|c| c.from_task).count();
                    let mut text = format!("Tạo MR {} → {}: {mine} commit của task. Agent dùng skill của dự án để tạo MR (có thể cập nhật PMS theo skill).", if p.foreign.is_empty() { br.clone() } else { format!("{br}-mr") }, p.target.unwrap_or_default());
                    if !p.foreign.is_empty() {
                        let subj: Vec<&str> = p.foreign.iter().map(|c| c.subject.as_str()).collect();
                        text.push_str(&format!(" Bỏ {} commit không phải của task: {}. Muốn đẩy cả chúng thì làm trên Board.", p.foreign.len(), clip(&subj.join("; "), 300)));
                    }
                    text
                }
            };
            let body = serde_json::json!({
                "chat_id": chat,
                "text": text,
                "reply_parameters": { "message_id": mid },
                "reply_markup": { "inline_keyboard": [[{ "text": "Tạo MR ngay", "callback_data": "mg!" }]] },
            });
            if let Some(m) = call(&cfg.token, "sendMessage", &body, 20) {
                let cmid = m.get("message_id").and_then(|x| x.as_i64()).unwrap_or(0);
                let _g = state_lock();
                let mut st = load_state();
                st.msgs.push(Sent { mid: cmid, ..s.clone() });
                save_state(&st);
            }
            Ok(String::new())
        }
        // Defaults: the task's commits only, origin's default branch (or the MR's).
        "mg!" => super::action::accept(&s.root, &s.id, &super::action::mr_proposal(&super::load(&s.root, &s.id).ok_or("task không còn")?), Some("telegram")).map(|t| match (t.stage, &t.mr) {
            (Stage::Done, Some(m)) => format!("✅ {} đã nghiệm thu: MR {}", s.id, m.url),
            (Stage::Done, None) => format!("✅ {} đã nghiệm thu (không có git).", s.id),
            _ => format!("⏳ {}: agent đang tạo MR bằng skill của dự án, xong sẽ báo.", s.id),
        }),
        o if o.starts_with('o') => {
            let i: usize = o[1..].parse().map_err(|_| "nút lạ".to_string())?;
            let answer = s.options.get(i).cloned().ok_or("lựa chọn không còn")?;
            let qid = s.qid.clone().unwrap_or_default();
            act(s, |root, t| ask::answer_as(root, t, &qid, &answer, "boss")).map(|_| format!("Đã trả lời {qid}: {answer}"))
        }
        _ => Err("nút lạ".into()),
    }
}

/// `/status`: everything waiting on the boss, across projects.
fn status_text() -> String {
    let mut lines = Vec::new();
    for root in roots(&home()) {
        for t in list(&root) {
            if let Some(a) = attention(&t).filter(|a| a.kind != "done") {
                lines.push(format!("• [{}] {} {} — {}", project_name(&root), t.id, clip(&t.title, 40), a.kind));
            }
        }
    }
    if lines.is_empty() { "Không có việc nào chờ bạn.".into() } else { format!("Việc chờ bạn:\n{}", lines.join("\n")) }
}

// ── Dashboard API: connect / test / disconnect ──────────────────────────────

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/board/telegram", get(api_get).post(api_connect))
        .route("/api/board/telegram/test", post(api_test))
        .route("/api/board/telegram/off", post(api_off))
}

fn view() -> serde_json::Value {
    match load_config() {
        None => serde_json::json!({ "configured": false }),
        Some(c) => serde_json::json!({
            "configured": true, "bot": c.bot, "paired": c.chat_id.is_some(),
            "pair_code": if c.chat_id.is_none() { c.pair_code } else { None },
        }),
    }
}

async fn api_get() -> Json<serde_json::Value> {
    Json(view())
}

#[derive(Deserialize)]
struct ConnectBody {
    token: String,
}

async fn api_connect(Json(b): Json<ConnectBody>) -> ApiRes<serde_json::Value> {
    let token = b.token.trim().to_string();
    let me = call(&token, "getMe", &serde_json::json!({}), 15).ok_or((StatusCode::BAD_REQUEST, "token không hợp lệ hoặc không gọi được Telegram".to_string()))?;
    let bot = me.get("username").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let code = format!("{:06}", (nanos ^ std::process::id()) % 1_000_000);
    save_config(&Config { token, chat_id: None, pair_code: Some(code), bot }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Skip any backlog sent to the bot before pairing.
    {
        let _g = state_lock();
        let mut st = load_state();
        st.offset = 0;
        save_state(&st);
    }
    Ok(Json(view()))
}

async fn api_test() -> ApiRes<serde_json::Value> {
    let c = load_config().ok_or((StatusCode::BAD_REQUEST, "chưa kết nối bot".to_string()))?;
    let chat = c.chat_id.ok_or((StatusCode::BAD_REQUEST, "chưa ghép đôi — gửi /start <mã> cho bot".to_string()))?;
    call(&c.token, "sendMessage", &serde_json::json!({ "chat_id": chat, "text": format!("🔔 Thử thông báo ckit Board.\n{}", status_text()) }), 20)
        .ok_or((StatusCode::BAD_GATEWAY, "Telegram từ chối tin".to_string()))?;
    Ok(Json(view()))
}

async fn api_off() -> Json<serde_json::Value> {
    let _ = std::fs::remove_file(config_path());
    Json(view())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verbs::harness::board::tests::task;

    #[test]
    fn replies_to_a_done_task_go_to_its_leader_chat() {
        assert!(to_chat("done", Status::Done));
        assert!(to_chat("acceptance", Status::Done), "accepted since the notification was sent");
        assert!(!to_chat("acceptance", Status::Waiting), "still at the gate: the reply answers it");
        assert!(!to_chat("approval", Status::Waiting));
    }

    #[test]
    fn a_parked_task_cannot_pass_its_gate_from_a_button() {
        let root = std::env::temp_dir().join(format!("ckit-tg-gate-{}", std::process::id()));
        let mut t = task(Stage::Investigate, Status::Waiting);
        t.paused = true; // created, never started
        assert!(approve(&root, &mut t, false).is_err());
        assert_eq!((t.stage, t.status), (Stage::Investigate, Status::Waiting), "nothing skipped");
        t.paused = false;
        approve(&root, &mut t, false).unwrap();
        assert_ne!(t.stage, Stage::Investigate);
    }

    #[test]
    fn attention_covers_every_gate_and_changes_key_per_question() {
        let mut t = task(Stage::Approval, Status::Waiting);
        assert_eq!(attention(&t).unwrap().kind, "approval");
        t.set(Stage::Dev, Status::Running);
        assert!(attention(&t).is_none(), "running needs nobody");
        t.paused = true;
        t.status = Status::Waiting;
        assert_eq!(attention(&t).unwrap().kind, "start", "never ran → Bắt đầu");
        t.paused = false;
        t.set(Stage::Acceptance, Status::Waiting);
        let a = attention(&t).unwrap();
        assert_eq!((a.kind, a.buttons[0].1.as_str()), ("acceptance", "mg"), "merge asks for confirmation first");
        t.set(Stage::Dev, Status::Running);
        t.asks.push(ask::Ask { id: "Q-2".into(), from: Stage::Dev, to: "leader".into(), question: "bảng nào?".into(), options: vec!["a".into(), "b".into()], at: String::new(), status: "boss".into(), answer: None, by: None, answered_at: None, kind: None, ac: None, phase: None });
        let a = attention(&t).unwrap();
        assert_eq!((a.kind, a.key.as_str(), a.buttons.len()), ("ask", "ask|Q-2", 2));
        assert!(a.body.contains("được Leader chuyển lên"));
    }
}
