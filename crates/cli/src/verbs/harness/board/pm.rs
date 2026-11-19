//! PM — the board's project manager. Every 5 minutes it posts a status line for
//! each running stage, built from the run log, so it is free and exact. It calls
//! an AI only when something looks wrong (running too long, stalled, repeated tool
//! errors) and to write a board report when the board goes idle after activity
//! (or on demand). Feed: `<project>/.cache/ckit/board/pm.jsonl`.
use std::collections::{HashMap, HashSet};
use std::io::Write as _;

use super::*;

const TICK: Duration = Duration::from_secs(5 * 60);
const LONG_RUN: Duration = Duration::from_secs(30 * 60);
const ALERT_COOLDOWN: Duration = Duration::from_secs(15 * 60);
const ERROR_BURST: usize = 3;
const AI_TIMEOUT: Duration = Duration::from_secs(4 * 60);
const AI_BACKOFF: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Entry {
    at: String,
    task: Option<String>,
    stage: Option<Stage>,
    kind: String,
    text: String,
}

/// What a run has done so far, digested from its log.
#[derive(Default, Debug, PartialEq)]
struct Activity {
    tools: usize,
    errors: usize,
    files: Vec<String>,
    last: String,
    last_error: String,
}

#[derive(Default)]
struct Watch {
    long_alerted: bool,
    size: u64,
    errors: usize,
    alerted: Option<Instant>,
}

#[derive(Default)]
struct PmState {
    last_tick: HashMap<PathBuf, Instant>,
    last_tick_iso: HashMap<PathBuf, String>,
    runs: HashMap<String, Watch>,
    /// Task ids already seen done/cancelled/failed per root (baseline at first sight).
    ended: HashMap<PathBuf, HashSet<String>>,
    reporting: HashSet<PathBuf>,
    /// Last time every AI model failed for a root — PM stays deterministic until AI_BACKOFF passes.
    ai_failed: HashMap<PathBuf, Instant>,
}

static STATE: OnceLock<Mutex<PmState>> = OnceLock::new();
static FEED_LOCK: Mutex<()> = Mutex::new(());

fn state() -> std::sync::MutexGuard<'static, PmState> {
    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn feed_path(root: &FsPath) -> PathBuf {
    board_dir(root).join("pm.jsonl")
}

fn append(root: &FsPath, task: Option<&Task>, kind: &str, text: &str) {
    let e = Entry {
        at: iso_now(),
        task: task.map(|t| t.id.clone()),
        stage: task.map(|t| t.stage),
        kind: kind.into(),
        text: text.trim().into(),
    };
    let _g = FEED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = std::fs::create_dir_all(board_dir(root));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(feed_path(root)) {
        let _ = writeln!(f, "{}", serde_json::to_string(&e).unwrap_or_default());
    }
}

fn feed(root: &FsPath, limit: usize) -> Vec<Entry> {
    let raw = std::fs::read_to_string(feed_path(root)).unwrap_or_default();
    let all: Vec<Entry> = raw.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let skip = all.len().saturating_sub(limit);
    all.into_iter().skip(skip).collect()
}

fn who(stage: Stage) -> &'static str {
    stage.who()
}

#[cfg(test)]
fn activity(log: &str) -> Activity {
    from_facts(digest::scan(log.as_bytes()))
}

/// Streamed: a running log is 100s of MB by the time the PM looks at it.
fn activity_of(path: &FsPath) -> Activity {
    from_facts(digest::scan_file(path))
}

fn from_facts(f: digest::Facts) -> Activity {
    let mut files: Vec<String> = Vec::new();
    for p in &f.files {
        let short = p.rsplit('/').next().unwrap_or(p).to_string();
        if !files.contains(&short) {
            files.push(short);
        }
    }
    // Harness errors (a blocked grep, a skipped wait) are not the work going wrong.
    Activity { tools: f.tools, errors: f.errors, files, last: f.last, last_error: f.last_error.unwrap_or_default() }
}

fn status_line(t: &Task, phase: Option<&str>, a: &Activity, mins: u64) -> String {
    let phase = phase.map(|p| format!(" {p}")).unwrap_or_default();
    let mut s = format!("{}{phase} đang {} — {} phút, {} lệnh", who(t.stage), t.stage.key(), mins, a.tools);
    if !a.files.is_empty() {
        s.push_str(&format!(", sửa {}", a.files.join(", ")));
    }
    if a.errors > 0 {
        s.push_str(&format!(", {} lỗi tool", a.errors));
    }
    if !a.last.is_empty() {
        s.push_str(&format!(". Gần nhất: {}", a.last));
    }
    s
}

/// ≤5-line digest of what a run has done so far (files edited, last error, last
/// intent), for the Leader answering that run's question.
#[cfg(test)]
pub(super) fn run_digest(t: &Task, log: &str) -> String {
    digest_text(t, activity(log))
}

/// `run_digest` of a log file, streamed.
pub(super) fn run_digest_of(t: &Task, path: &FsPath) -> String {
    digest_text(t, activity_of(path))
}

fn digest_text(t: &Task, a: Activity) -> String {
    let or = |s: &str, none: &str| if s.is_empty() { none.to_string() } else { s.to_string() };
    [
        format!("Bước {} ({}), {} lệnh, {} lỗi tool", t.stage.key(), who(t.stage), a.tools, a.errors),
        format!("File đã sửa: {}", or(&a.files.join(", "), "chưa có")),
        format!("Lỗi gần nhất: {}", or(&a.last_error, "không có")),
        format!("Việc gần nhất: {}", or(&a.last, "chưa có")),
    ]
    .join("\n")
}

/// Called by the worker every loop for each project root.
pub(super) fn observe(root: &FsPath) {
    let tasks = list(root);
    // Auto-report only when a task newly ends (done/cancelled/failed) — gates already
    // show as "Chờ sếp" in the room, and the manual button covers the rest.
    let ended: HashSet<String> = tasks
        .iter()
        .filter(|t| matches!(t.status, Status::Done | Status::Cancelled | Status::Failed))
        .map(|t| t.id.clone())
        .collect();
    let newly_ended = {
        let mut st = state();
        match st.ended.insert(root.to_path_buf(), ended.clone()) {
            Some(prev) => ended.iter().any(|id| !prev.contains(id)),
            None => false, // first sight after (re)start: baseline, not news
        }
    };
    if newly_ended {
        start_report(root);
    }

    let due = {
        let mut st = state();
        let last = *st.last_tick.entry(root.to_path_buf()).or_insert_with(Instant::now);
        if last.elapsed() >= TICK {
            st.last_tick.insert(root.to_path_buf(), Instant::now());
            st.last_tick_iso.insert(root.to_path_buf(), iso_now());
            true
        } else {
            false
        }
    };
    if !due {
        return;
    }

    for t in tasks.iter().filter(|t| t.status == Status::Running) {
        let Some(run) = t.runs.last() else { continue };
        let log_path = task_dir(root, &t.id).join("logs").join(&run.log);
        let size = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        let a = activity_of(&log_path);
        let elapsed = secs_since(&run.started);
        let (mins, reason) = {
            let mut st = state();
            let w = st.runs.entry(format!("{}|{}|{}", root.display(), t.id, run.log)).or_default();
            let stalled = w.size > 0 && w.size == size;
            let new_errors = a.errors.saturating_sub(w.errors);
            // Long-run alert fires once per run; stalls/error bursts may repeat after the cooldown.
            let long = !w.long_alerted && elapsed >= LONG_RUN.as_secs();
            w.size = size;
            w.errors = a.errors;
            let cool = w.alerted.map(|x| x.elapsed() >= ALERT_COOLDOWN).unwrap_or(true);
            let reason = if long {
                w.long_alerted = true;
                Some(format!("đã chạy hơn {} phút", LONG_RUN.as_secs() / 60))
            } else if !cool {
                None
            } else if stalled {
                Some(format!("không có hoạt động mới trong {} phút", TICK.as_secs() / 60))
            } else if new_errors >= ERROR_BURST {
                Some(format!("{new_errors} lỗi tool mới trong {} phút", TICK.as_secs() / 60))
            } else {
                None
            };
            if reason.is_some() {
                w.alerted = Some(Instant::now());
            }
            (elapsed / 60, reason)
        };
        // A step waiting on `board_ask` writes nothing — that is not a stall.
        let waiting = t.asks.iter().find(|a| matches!(a.status.as_str(), "answering" | "boss"));
        let line = match waiting {
            Some(q) => format!("{} · đang chờ {} trả lời {}", status_line(t, active_phase(root, t).as_deref(), &a, mins), q.to, q.id),
            None => status_line(t, active_phase(root, t).as_deref(), &a, mins),
        };
        append(root, Some(t), "status", &line);
        if let Some(reason) = reason.filter(|_| waiting.is_none()) {
            // The coordinator can act on it (message / pause); the tool-less PM can only report.
            if super::coord::enabled(root) {
                super::coord::event(root, format!("PM cảnh báo: {} \"{}\" ở bước {} — {reason}. Xem log (board_task) và quyết có cần nhắn/tạm dừng không.", t.id, t.title, t.stage.key()));
            } else {
                let (root, t, tail) = (root.to_path_buf(), t.clone(), log_tail(&log_path, 40));
                std::thread::spawn(move || alert(&root, &t, &reason, &tail));
            }
        }
    }
}

/// Seconds since an `iso_now()` timestamp (`YYYY-MM-DDTHH:MM:SSZ`); 0 if unparsable.
pub(super) fn secs_since(iso: &str) -> u64 {
    let Some(then) = iso_secs(iso) else { return 0 };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    (now - then).max(0) as u64
}

/// Unix seconds of a UTC `YYYY-MM-DDTHH:MM:SS…` timestamp (fraction/`Z` ignored).
pub(super) fn iso_secs(iso: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| iso.get(r).and_then(|s| s.parse::<i64>().ok());
    let (y, m, d, hh, mm, ss) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    // Howard Hinnant's days_from_civil (inverse of iso_now's civil_from_days).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// `(at, text)` of the PM status ticks posted for task `id`, oldest first.
pub(super) fn task_ticks(root: &FsPath, id: &str) -> Vec<(String, String)> {
    let raw = std::fs::read_to_string(feed_path(root)).unwrap_or_default();
    raw.lines()
        .filter(|l| l.contains(id))
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .filter(|e| e.kind == "status" && e.task.as_deref() == Some(id))
        .map(|e| (e.at, e.text))
        .collect()
}

/// Phase a status tick names: `Tester M1 đang test — …` → M1 (the word before `đang`).
pub(super) fn tick_phase(text: &str) -> Option<String> {
    let head = text.split(" đang ").next()?;
    let w = head.split_whitespace().last()?;
    (w.len() > 1 && w.starts_with('M') && w[1..].chars().all(|c| c.is_ascii_alphanumeric()) && w[1..].starts_with(|c: char| c.is_ascii_digit())).then(|| w.to_string())
}

fn log_tail(path: &FsPath, n: usize) -> String {
    log_lines(path)
        .iter()
        .rev()
        .take(n)
        .rev()
        .map(|l| format!("[{}] {}", l["kind"].as_str().unwrap_or(""), l["text"].as_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn alert(root: &FsPath, t: &Task, reason: &str, tail: &str) {
    let fallback = format!("⚠ {} · {}: {}", t.id, who(t.stage), reason);
    let prompt = format!(
        "You are the PM of an AI delivery team. Task {} \"{}\", stage {} (worked by {}). Anomaly: {}.\n\
         Recent activity of that stage (oldest first):\n{}\n\n\
         In Vietnamese, 2-3 plain sentences for the boss: what is happening, whether it looks stuck, and what to do (wait / cancel / retry). No markdown, no preamble. Do not use any tools.",
        t.id, t.title, t.stage.key(), who(t.stage), reason, tail
    );
    let text = ask_ai(root, &prompt).map(|s| format!("⚠ {}: {}", t.id, s)).unwrap_or(fallback);
    append(root, Some(t), "alert", &text);
}

fn start_report(root: &FsPath) {
    if !state().reporting.insert(root.to_path_buf()) {
        return; // one at a time
    }
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        report(&root);
        state().reporting.remove(&root);
    });
}

fn report(root: &FsPath) {
    let tasks = list(root);
    if tasks.is_empty() {
        return;
    }
    let rows: Vec<String> = tasks
        .iter()
        .map(|t| format!("- {} | {} | stage={} status={} | ${:.2} | {}", t.id, t.title, t.stage.key(), serde_json::to_string(&t.status).unwrap_or_default(), t.cost, t.note.clone().unwrap_or_default()))
        .collect();
    let recent: Vec<String> = feed(root, 30).iter().filter(|e| e.kind != "report").map(|e| format!("{} {} {}", e.at, e.task.clone().unwrap_or_default(), e.text)).collect();
    let total: f64 = tasks.iter().map(|t| t.cost).sum();
    let fallback = format!(
        "## Báo cáo PM\n\n{}\n\nTổng chi phí: ${:.2}",
        tasks.iter().map(|t| format!("- **{}** {} — {} ({})", t.id, t.title, t.stage.key(), serde_json::to_string(&t.status).unwrap_or_default().trim_matches('"'))).collect::<Vec<_>>().join("\n"),
        total
    );
    let prompt = format!(
        "You are the PM of an AI delivery team. Write the board report for the boss in Vietnamese markdown, at most 25 lines, sections: \
         \"Đã xong\", \"Đang làm\", \"Chờ sếp\" (approval/acceptance gates or questions), \"Rủi ro / vướng\" (failed, alerts, long runs), \"Chi phí\" (total ${:.2}). \
         Only use the facts below. Do not use any tools.\n\nTasks:\n{}\n\nRecent PM log:\n{}",
        total,
        rows.join("\n"),
        recent.join("\n")
    );
    let text = ask_ai(root, &prompt).unwrap_or(fallback);
    append(root, None, "report", &text);
}

/// One-shot text answer: the cheap `smol` role first, then omp's default model.
/// After both fail, stay deterministic for AI_BACKOFF instead of re-calling every tick.
fn ask_ai(root: &FsPath, prompt: &str) -> Option<String> {
    if state().ai_failed.get(root).is_some_and(|t| t.elapsed() < AI_BACKOFF) {
        return None;
    }
    for model in [Some("@smol"), None] {
        if let Some(s) = run_text(root, prompt, model) {
            state().ai_failed.remove(root);
            return Some(s);
        }
    }
    state().ai_failed.insert(root.to_path_buf(), Instant::now());
    None
}

/// Summaries need no tools: the prompt embeds agent log tails (shell output, file
/// text), so a tool-enabled run could act on injected instructions. Runs with no
/// tools/skills/extensions/rules — also far cheaper — and reads the answer from
/// the json event stream so provider errors never pass as the PM's words.
fn run_text(root: &FsPath, prompt: &str, model: Option<&str>) -> Option<String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = std::env::temp_dir().join(format!("ckit-pm-{}-{n}.jsonl", std::process::id()));
    let f = std::fs::File::create(&out).ok()?;
    let mut cmd = super::omp_cmd();
    cmd.current_dir(root)
        .args(["--no-session", "--no-tools", "--no-skills", "--no-extensions", "--no-rules", "--mode", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(f))
        .stderr(Stdio::null());
    if let Some(m) = model {
        cmd.args(["--model", m]);
    }
    cmd.arg("-p").arg(prompt);
    let mut child = cmd.spawn().ok()?;
    let started = Instant::now();
    let ok = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.success(),
            Ok(None) if started.elapsed() > AI_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(_) => break false,
        }
    };
    let raw = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    if !ok {
        return None;
    }
    answer_text(&raw)
}

/// Text of the last assistant message; None if it errored or said nothing.
pub(super) fn answer_text(log: &str) -> Option<String> {
    let last = message_ends(log).filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant")).last()?;
    let errored = last.get("stopReason").and_then(|s| s.as_str()) == Some("error") || last.get("errorMessage").is_some();
    let text: String = last
        .get("content")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let text = text.trim();
    (!errored && !text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod answer_tests {
    use super::answer_text;

    #[test]
    fn provider_error_is_not_an_answer() {
        // Shapes captured from `omp --mode json -p` (broken @smol → 400; default model → ok).
        let bad = "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[],\"stopReason\":\"error\",\"errorMessage\":\"400 not supported\"}}\n";
        let good = "{\"type\":\"message_end\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"Reply: ok\"}]}}\n{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\" ok \"}],\"stopReason\":\"stop\"}}\n";
        assert_eq!(answer_text(bad), None);
        assert_eq!(answer_text(good).as_deref(), Some("ok"));
        assert_eq!(answer_text(""), None);
    }
}

// ── HTTP ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct FeedQuery {
    limit: Option<usize>,
}

pub(super) async fn api_feed(Query(q): Query<FeedQuery>, Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let st = state();
    Ok(Json(serde_json::json!({
        "entries": feed(&root, q.limit.unwrap_or(100)),
        "reporting": st.reporting.contains(&root),
        "interval_secs": TICK.as_secs(),
        "last_tick": st.last_tick_iso.get(&root),
    })))
}

pub(super) async fn api_report(Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    start_report(&root);
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_counts_tools_errors_and_project_files_only() {
        let log = concat!(
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[",
            "{\"type\":\"toolCall\",\"name\":\"edit\",\"arguments\":{\"path\":\"src/calc.py\"}},",
            "{\"type\":\"toolCall\",\"name\":\"write\",\"arguments\":{\"path\":\"/r/.cache/ckit/board/T-001/DEV.md\"}},",
            "{\"type\":\"toolCall\",\"name\":\"write\",\"arguments\":{\"path\":\"xd://mcp__codegraph_explore\"}},",
            "{\"type\":\"toolCall\",\"name\":\"bash\",\"arguments\":{\"command\":\"pytest\",\"i\":\"Running tests\"}}]}}\n",
            "{\"type\":\"message_end\",\"message\":{\"role\":\"toolResult\",\"isError\":true,\"content\":[]}}\n",
        );
        let a = activity(log);
        assert_eq!(a.tools, 4);
        assert_eq!(a.errors, 1);
        assert_eq!(a.files, vec!["calc.py".to_string()], "board artifacts and xd:// devices are not project edits");
        assert_eq!(a.last, "bash: Running tests");
        assert_eq!(a.last_error, "");
    }

    #[test]
    fn digest_names_files_last_error_and_last_intent() {
        let log = concat!(
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"toolCall\",\"name\":\"edit\",\"arguments\":{\"path\":\"app.py\",\"i\":\"Adding sum\"}}]}}\n",
            "{\"type\":\"message_end\",\"message\":{\"role\":\"toolResult\",\"isError\":true,\"content\":[{\"type\":\"text\",\"text\":\"SyntaxError: bad\"}]}}\n",
        );
        let t = super::super::tests::task(Stage::Dev, Status::Running);
        let d = run_digest(&t, log);
        assert!(d.lines().count() <= 5, "{d}");
        assert!(d.contains("File đã sửa: app.py") && d.contains("Lỗi gần nhất: SyntaxError: bad") && d.contains("Việc gần nhất: edit: Adding sum"), "{d}");
    }

    #[test]
    fn tick_posts_status_for_running_stage() {
        let root = std::env::temp_dir().join(format!("ckit-pm-tick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = super::super::tests::task(Stage::Dev, Status::Running);
        t.runs.push(Run { stage: Stage::Dev, n: 1, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: "dev-1.jsonl".into(), phase: None, round: None, title: None });
        save(&root, &mut t).unwrap();
        let logs = task_dir(&root, &t.id).join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            logs.join("dev-1.jsonl"),
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"toolCall\",\"name\":\"edit\",\"arguments\":{\"path\":\"calc.py\",\"i\":\"Adding divide\"}}]}}\n",
        )
        .unwrap();
        observe(&root); // first sight starts the 5-minute clock: no post yet
        assert!(feed(&root, 10).is_empty());
        state().last_tick.insert(root.clone(), Instant::now().checked_sub(TICK).unwrap_or_else(Instant::now));
        observe(&root);
        let f = feed(&root, 10);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].kind.as_str(), f[0].task.as_deref()), ("status", Some("T-001")));
        assert!(f[0].text.starts_with("Dev đang dev") && f[0].text.contains("sửa calc.py"), "{}", f[0].text);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod time_tests {
    use super::*;

    #[test]
    fn secs_since_inverts_iso_now() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(secs_since(&iso_now()) <= 1);
        assert!(secs_since("1970-01-01T00:00:00Z").abs_diff(now) <= 1);
        assert!(secs_since("2000-03-01T00:00:00Z").abs_diff(now - 951_868_800) <= 1, "leap-year March boundary");
        assert_eq!(secs_since("garbage"), 0);
    }
}
