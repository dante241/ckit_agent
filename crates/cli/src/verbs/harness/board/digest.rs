//! What one run did, from its omp `--mode json` event log: tool calls, real vs
//! harness errors, files edited, the boss's notes it received (and its one-line
//! acknowledgement), its closing words and cost.
//!
//! Logs reach hundreds of MB, nearly all streaming deltas. They are streamed once:
//! only `message_end` lines (omp session files: `message` lines) are buffered and
//! parsed, every other line is skipped without being held in memory. A run is
//! digested when it ends; older runs on first view. Cached at
//! `<task>/digest/<log stem>.json`, keyed by the log size.
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

use super::*;

/// A kept line longer than this is dropped (still streamed past, never buffered).
const MAX_LINE: usize = 32 << 20;
/// Bytes needed to tell a line's kind (`{"type":"message_end"` is 21).
const HEAD: usize = 24;

/// Tool errors raised by the harness, not by the work (a blocked grep, a skipped
/// wait, an eval timeout, a malformed edit header): shown apart from real errors.
const HARNESS_MARKERS: [&str; 7] = [
    "code-intel-first",
    "Skipped due to a queued background completion",
    "eval cell timed out",
    "Input header must be [PATH]",
    "Validation failed for tool",
    "Invalid args for",
    "Tool call blocked",
];

pub(super) fn is_harness_error(text: &str) -> bool {
    HARNESS_MARKERS.iter().any(|m| text.contains(m))
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub(super) struct Facts {
    pub tools: usize,
    /// Real tool errors (harness ones excluded).
    pub errors: usize,
    pub harness_errors: usize,
    /// Paths given to `edit` / `write`, first-seen order (internal URLs and board files skipped).
    pub files: Vec<String>,
    pub last_error: Option<String>,
    /// The run's last assistant text block (its closing words).
    pub last_text: Option<String>,
    /// Notes the boss / coordinator steered into the run.
    pub steers: Vec<Steer>,
    pub cost: f64,
    /// Latest intent or text (`bash: Running tests`), for the PM's status line.
    #[serde(skip)]
    pub last: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub(super) struct Steer {
    /// The note as posted (`[Tạm dừng] …` for a pause).
    pub text: String,
    /// The agent's first words after receiving it.
    pub reply: Option<String>,
}

/// Calls `f` with every line of `r` (newline excluded) whose first `head` bytes
/// satisfy `keep`. Other lines are skipped without buffering, so memory is bounded
/// by the longest kept line (≤ `cap`; longer kept lines are dropped). Returns the
/// largest line buffer used.
pub(super) fn scan_lines<R: Read>(r: R, head: usize, keep: impl Fn(&[u8]) -> bool, cap: usize, mut f: impl FnMut(&[u8])) -> std::io::Result<usize> {
    #[derive(PartialEq)]
    enum Mode {
        Head,
        Keep,
        Skip,
    }
    let mut br = BufReader::with_capacity(1 << 20, r);
    let mut line: Vec<u8> = Vec::new();
    let mut mode = Mode::Head;
    let mut max_kept = 0;
    loop {
        let buf = br.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        let nl = memchr::memchr(b'\n', buf);
        let end = nl.unwrap_or(buf.len());
        let mut i = 0;
        if mode == Mode::Head {
            let take = (head - line.len()).min(end);
            line.extend_from_slice(&buf[..take]);
            i = take;
            if line.len() >= head || nl.is_some() {
                mode = if keep(&line) { Mode::Keep } else { Mode::Skip };
            }
        }
        if mode == Mode::Keep {
            if line.len() + (end - i) > cap {
                mode = Mode::Skip;
                line = Vec::new();
            } else {
                line.extend_from_slice(&buf[i..end]);
            }
        }
        let used = match nl {
            Some(n) => {
                if mode == Mode::Keep {
                    max_kept = max_kept.max(line.capacity());
                    f(&line);
                }
                line.clear();
                if line.capacity() > 1 << 20 {
                    line = Vec::new(); // one huge kept line must not pin its buffer
                }
                mode = Mode::Head;
                n + 1
            }
            None => buf.len(),
        };
        br.consume(used);
    }
    if mode == Mode::Keep || (mode == Mode::Head && !line.is_empty() && keep(&line)) {
        max_kept = max_kept.max(line.capacity());
        f(&line);
    }
    Ok(max_kept)
}

/// `message_end` (run log) or `message` (session file) line.
pub(super) fn is_message(head: &[u8]) -> bool {
    head.starts_with(b"{\"type\":\"message_end\"") || head.starts_with(b"{\"type\":\"message\",")
}

/// The `message` object of a kept message line.
pub(super) fn message_of(line: &[u8]) -> Option<serde_json::Value> {
    let mut v: serde_json::Value = serde_json::from_slice(line).ok()?;
    Some(v.get_mut("message")?.take())
}

pub(super) fn text_blocks(m: &serde_json::Value) -> String {
    match m.get("content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|b| b.get("text").and_then(|x| x.as_str())).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

/// Cost of one assistant message (`usage.cost.total`, else the sum of its parts).
pub(super) fn message_cost(m: &serde_json::Value) -> f64 {
    match m.pointer("/usage/cost") {
        Some(c) => match c.get("total").and_then(|x| x.as_f64()) {
            Some(t) => t,
            None => c.as_object().map(|o| o.values().filter_map(|x| x.as_f64()).sum()).unwrap_or(0.0),
        },
        None => 0.0,
    }
}

/// The boss's note inside the steer the engine injected (see `8sync-engine.ts`).
fn steer_note(text: &str) -> Option<String> {
    const NOTE: &str = " while you work (it overrides the plan where they conflict): ";
    if text.starts_with("Message from ") {
        let i = text.find(NOTE)?;
        let rest = &text[i + NOTE.len()..];
        let end = rest.find("\nAcknowledge it in one line").unwrap_or(rest.len());
        return Some(rest[..end].trim().to_string());
    }
    let rest = text.strip_prefix("PAUSE requested by ")?;
    let head = rest.split("\nStop at a safe point").next().unwrap_or("");
    let note = head.split_once(": ").map(|(_, n)| n.trim()).unwrap_or("");
    Some(format!("[Tạm dừng] {note}").trim().to_string())
}

fn on_message(m: &serde_json::Value, f: &mut Facts, pending: &mut Option<usize>) {
    let blocks = m.get("content").and_then(|c| c.as_array());
    match m.get("role").and_then(|r| r.as_str()) {
        Some("assistant") => {
            f.cost += message_cost(m);
            for b in blocks.into_iter().flatten() {
                match b.get("type").and_then(|x| x.as_str()) {
                    Some("toolCall") => {
                        f.tools += 1;
                        let name = b.get("name").and_then(|x| x.as_str()).unwrap_or("tool");
                        let args = b.get("arguments");
                        f.last = match args.and_then(|a| a.get("i")).and_then(|x| x.as_str()) {
                            Some(i) => format!("{name}: {i}"),
                            None => format!("{name} {}", clip(&args.map(|a| a.to_string()).unwrap_or_default(), 80)),
                        };
                        if matches!(name, "edit" | "write") {
                            let path = args.and_then(|a| a.get("path")).and_then(|x| x.as_str()).unwrap_or("");
                            if !path.is_empty() && !path.contains("://") && !path.contains(BOARD_REL) && !f.files.iter().any(|p| p == path) {
                                f.files.push(path.to_string());
                            }
                        }
                    }
                    Some("text") => {
                        let s = b.get("text").and_then(|x| x.as_str()).unwrap_or("").trim();
                        if !s.is_empty() {
                            f.last = clip(s, 120);
                            f.last_text = Some(clip(s, 4000));
                            if let Some(i) = pending.take() {
                                f.steers[i].reply = Some(clip(s, 300));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Some("toolResult") if m.get("isError").and_then(|x| x.as_bool()) == Some(true) => {
            let text = text_blocks(m);
            if is_harness_error(&text) {
                f.harness_errors += 1;
            } else {
                f.errors += 1;
                let text = text.trim();
                if !text.is_empty() {
                    let line = match m.get("toolName").and_then(|x| x.as_str()) {
                        Some(name) => format!("{name}: {text}"),
                        None => text.to_string(),
                    };
                    f.last_error = Some(clip(&line, 200));
                }
            }
        }
        Some("user") => {
            if let Some(note) = steer_note(text_blocks(m).trim_start()) {
                f.steers.push(Steer { text: note, reply: None });
                *pending = Some(f.steers.len() - 1);
            }
        }
        _ => {}
    }
}

pub(super) fn scan(r: impl Read) -> Facts {
    let mut f = Facts::default();
    let mut pending = None;
    let _ = scan_lines(r, HEAD, is_message, MAX_LINE, |l| {
        if let Some(m) = message_of(l) {
            on_message(&m, &mut f, &mut pending);
        }
    });
    f
}

pub(super) fn scan_file(path: &FsPath) -> Facts {
    std::fs::File::open(path).map(scan).unwrap_or_default()
}

#[derive(Serialize, Deserialize)]
struct Cached {
    len: u64,
    facts: Facts,
}

fn cache_path(dir: &FsPath, log: &str) -> PathBuf {
    dir.join("digest").join(format!("{}.json", log.trim_end_matches(".jsonl")))
}

/// Digest of a finished run's log: cached, else streamed now and cached.
pub(super) fn of_log(dir: &FsPath, log: &str) -> Facts {
    let path = dir.join("logs").join(log);
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let cache = cache_path(dir, log);
    if let Some(c) = std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice::<Cached>(&b).ok()).filter(|c| c.len == len) {
        return c.facts;
    }
    let facts = scan_file(&path);
    store(dir, log, len, &facts);
    facts
}

/// Cached digest only (None when the log was never digested or grew since).
pub(super) fn cached(dir: &FsPath, log: &str) -> Option<Facts> {
    let len = std::fs::metadata(dir.join("logs").join(log)).map(|m| m.len()).ok()?;
    std::fs::read(cache_path(dir, log)).ok().and_then(|b| serde_json::from_slice::<Cached>(&b).ok()).filter(|c| c.len == len).map(|c| c.facts)
}

/// Write-then-rename: a reader never sees half a cache file.
pub(super) fn store(dir: &FsPath, log: &str, len: u64, facts: &Facts) {
    let path = cache_path(dir, log);
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let tmp = path.with_extension(format!("json.{}", std::process::id()));
    let body = serde_json::to_vec(&Cached { len, facts: facts.clone() }).unwrap_or_default();
    if std::fs::write(&tmp, body).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// One thing a running step just did, readable by the boss (`bash: Running tests`).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub(super) struct Action {
    /// When omp stamped it (assistant text); tool starts carry no time, except the
    /// newest action, which gets the log's last write.
    pub at: Option<String>,
    /// Tool name; `null` for the agent's own words.
    pub tool: Option<String>,
    pub text: String,
    /// Path an `edit`/`write` touched.
    #[serde(skip)]
    pub file: Option<String>,
    /// Tool call it belongs to: a call's start and its many updates are one action.
    #[serde(skip)]
    pub call: Option<String>,
    /// An update of a call (merged into its start), not a start.
    #[serde(skip)]
    pub update: bool,
}

/// What a `task` call's subagents are doing now, from its progress update:
/// `PlanReviewM2 (reviewer): Checking addIndexIfMissing` (running ones; else all).
fn task_progress(v: &serde_json::Value) -> Option<String> {
    let prog = v.pointer("/partialResult/details/progress")?.as_array()?;
    let line = |p: &serde_json::Value| {
        let id = p["id"].as_str().unwrap_or("?");
        let who = p["agent"].as_str().map(|a| format!(" ({a})")).unwrap_or_default();
        match p["lastIntent"].as_str().filter(|s| !s.is_empty()) {
            Some(i) => format!("{id}{who}: {i}"),
            None => format!("{id}{who}"),
        }
    };
    let running: Vec<String> = prog.iter().filter(|p| matches!(p["status"].as_str(), Some("running" | "pending"))).map(line).collect();
    let all = if running.is_empty() { prog.iter().map(line).collect() } else { running };
    (!all.is_empty()).then(|| clip(&format!("đang chờ subagent: {}", all.join("; ")), 240))
}

/// Action of one log line: a tool call's start or update (its intent, else a short
/// arg; a `task` update says what its subagents are doing), or an assistant
/// `message_end` with text. Anything else (deltas, results) is none.
fn action_of(line: &[u8]) -> Option<Action> {
    let start = line.starts_with(b"{\"type\":\"tool_execution_start\"");
    let update = line.starts_with(b"{\"type\":\"tool_execution_update\"") || line.starts_with(b"{\"type\":\"tool_stream_update\"");
    if start || update {
        let v: serde_json::Value = serde_json::from_slice(line).ok()?;
        let tool = v["toolName"].as_str().unwrap_or("tool").to_string();
        let args = &v["args"];
        let s = |k: &str| args.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty());
        let intent = v["intent"].as_str().map(str::trim).filter(|x| !x.is_empty()).or_else(|| s("i"));
        let path = s("path").filter(|p| !p.contains("://") && !p.contains(BOARD_REL)).map(str::to_string);
        let tasks = || {
            let names: Vec<String> = args["tasks"].as_array()?.iter().filter_map(|t| t["name"].as_str().or(t["agent"].as_str()).map(str::to_string)).collect();
            (!names.is_empty()).then(|| names.join(", "))
        };
        let text = (tool == "task").then(|| task_progress(&v)).flatten().or_else(|| intent.map(str::to_string)).or_else(|| (tool == "task").then(tasks).flatten()).unwrap_or_else(|| {
            clip(&s("path").or_else(|| s("command")).or_else(|| s("pattern")).or_else(|| s("query")).or_else(|| s("description")).unwrap_or("").replace('\n', " "), 100)
        });
        let file = path.filter(|_| matches!(tool.as_str(), "edit" | "write"));
        return Some(Action { at: None, tool: Some(tool), text, file, call: v["toolCallId"].as_str().map(str::to_string), update });
    }
    if !line.starts_with(b"{\"type\":\"message_end\"") {
        return None;
    }
    let m = message_of(line)?;
    if m.get("role").and_then(|r| r.as_str()) != Some("assistant") {
        return None;
    }
    let text = m.get("content").and_then(|c| c.as_array())?.iter().filter(|b| b.get("type").and_then(|x| x.as_str()) == Some("text")).filter_map(|b| b.get("text").and_then(|x| x.as_str())).collect::<Vec<_>>().join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    let at = m.get("timestamp").and_then(|x| x.as_u64()).map(|ms| iso_at(UNIX_EPOCH + Duration::from_millis(ms)));
    Some(Action { at, tool: None, text: clip(&text, 200), file: None, call: None, update: false })
}

/// Lines → actions, one per tool call: its updates refresh the text (a bare
/// `tool_stream_update` keeps what the start said), the call stays where it began.
fn collapse(lines: impl Iterator<Item = Action>) -> Vec<Action> {
    let mut out: Vec<Action> = Vec::new();
    for a in lines {
        match out.iter().rposition(|p| a.update && a.call.is_some() && p.call == a.call) {
            Some(i) => {
                if !a.text.is_empty() {
                    out[i].text = a.text;
                }
                if out[i].file.is_none() {
                    out[i].file = a.file;
                }
            }
            None => out.push(a),
        }
    }
    // A bare stream update whose start is out of the window says nothing; keep it
    // only when it is all there is.
    if out.iter().any(|a| !a.text.is_empty()) {
        out.retain(|a| !a.text.is_empty());
    }
    for a in out.iter_mut().filter(|a| a.text.is_empty()) {
        a.text = "đang chạy".into();
    }
    out
}

/// The last `n` actions of a (running) log, read from its tail only: 64 KB, or
/// 1 MB when the last 64 KB are all streaming deltas. Never the whole log.
pub(super) fn tail_actions(path: &FsPath, n: usize) -> Vec<Action> {
    let Ok(mut f) = std::fs::File::open(path) else { return Vec::new() };
    let meta = f.metadata().ok();
    let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta.and_then(|m| m.modified().ok()).map(iso_at);
    for window in [64u64 << 10, 1 << 20] {
        let start = len.saturating_sub(window);
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(start)).is_err() || (&mut f).take(window).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        // A cut first line is not JSON; skip it (a cut last line fails to parse too).
        let body = if start > 0 { memchr::memchr(b'\n', &buf).map(|i| &buf[i + 1..]).unwrap_or(&[]) } else { &buf[..] };
        let lines = || body.split(|b| *b == b'\n');
        let mut out: Vec<Action> = collapse(lines().filter_map(action_of));
        if !out.is_empty() || start == 0 {
            let mut out = out.split_off(out.len().saturating_sub(n));
            if let Some(last) = out.last_mut().filter(|a| a.at.is_none()) {
                last.at = mtime.clone();
            }
            if out.is_empty() {
                out.extend(last_event(lines()).map(|a| Action { at: mtime.clone(), ..a }));
            }
            return out;
        }
        if window == 1 << 20 {
            return last_event(lines()).map(|a| Action { at: mtime.clone(), ..a }).into_iter().collect();
        }
    }
    Vec::new()
}

/// Last resort for a tail with no readable action: the newest event says at least
/// that the step is alive (`đang viết trả lời`, `đang chạy bash`).
fn last_event<'a>(lines: impl DoubleEndedIterator<Item = &'a [u8]>) -> Option<Action> {
    lines.rev().find_map(|l| {
        let v: serde_json::Value = serde_json::from_slice(l).ok()?;
        let kind = v["type"].as_str()?;
        let tool = v["toolName"].as_str().map(str::to_string);
        let text = match (kind, &tool) {
            (_, Some(t)) => format!("đang chạy {t}"),
            ("message_update" | "message_start", _) => "đang viết trả lời".to_string(),
            ("turn_start" | "turn_end", _) => "đang suy nghĩ".to_string(),
            (k, None) => k.replace('_', " "),
        };
        Some(Action { at: None, tool, text, file: None, call: None, update: true })
    })
}

/// Digest the run's log now (it just ended) and cache it.
pub(super) fn at_run_end(dir: &FsPath, log: &str) -> Facts {
    let path = dir.join("logs").join(log);
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let facts = scan_file(&path);
    store(dir, log, len, &facts);
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `message_end` line as omp writes it (`type` first: lines are told apart by prefix).
    fn end(msg: serde_json::Value) -> String {
        format!("{{\"type\":\"message_end\",\"message\":{msg}}}\n")
    }

    fn sample() -> String {
        let mut s = String::new();
        s.push_str("{\"type\":\"session\",\"id\":\"abc\"}\n");
        s.push_str(&end(serde_json::json!({ "role": "assistant", "content": [
            { "type": "toolCall", "name": "grep", "arguments": { "i": "Finding x" } },
            { "type": "toolCall", "name": "edit", "arguments": { "path": "src/a.rs" } },
            { "type": "toolCall", "name": "write", "arguments": { "path": "xd://engine_plan" } },
        ], "usage": { "cost": { "total": 0.5 } } })));
        s.push_str(&end(serde_json::json!({ "role": "toolResult", "toolName": "grep", "isError": true, "content": [{ "type": "text", "text": "code-intel-first: run `write` …" }] })));
        s.push_str(&end(serde_json::json!({ "role": "toolResult", "toolName": "bash", "isError": true, "content": [{ "type": "text", "text": "Command exited with code 1" }] })));
        s.push_str("stderr noise\n");
        s.push_str(&end(serde_json::json!({ "role": "user", "content": [{ "type": "text", "text": "Message from the boss while you work (it overrides the plan where they conflict): dùng subagent\nAcknowledge it in one line, adjust, and continue." }] })));
        s.push_str(&end(serde_json::json!({ "role": "assistant", "content": [{ "type": "text", "text": "Đã nhận: sẽ gọi subagent." }], "usage": { "cost": { "input": 0.25, "output": 0.25 } } })));
        s.push_str(&end(serde_json::json!({ "role": "user", "content": "PAUSE requested by the boss.\nStop at a safe point NOW: …" })));
        s.push_str(&end(serde_json::json!({ "role": "assistant", "content": [{ "type": "text", "text": "Xong M1.\n- 3 file" }] })));
        s
    }

    #[test]
    fn digest_counts_real_vs_harness_errors_files_steers_cost() {
        let f = scan(sample().as_bytes());
        assert_eq!((f.tools, f.errors, f.harness_errors), (3, 1, 1));
        assert_eq!(f.files, ["src/a.rs"]);
        assert_eq!(f.last_error.as_deref(), Some("bash: Command exited with code 1"));
        assert_eq!(f.last_text.as_deref(), Some("Xong M1.\n- 3 file"));
        assert!((f.cost - 1.0).abs() < 1e-9);
        assert_eq!(
            f.steers,
            [Steer { text: "dùng subagent".into(), reply: Some("Đã nhận: sẽ gọi subagent.".into()) }, Steer { text: "[Tạm dừng]".into(), reply: Some("Xong M1.\n- 3 file".into()) }]
        );
    }

    /// A procedurally generated log: each chunk is a byte pattern repeated n times
    /// (streaming deltas, one 64 MB line, a few message_end lines). O(1) memory.
    struct Synthetic {
        chunks: Vec<(Vec<u8>, u64)>,
        idx: usize,
        pos: u64,
    }

    impl Read for Synthetic {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            while let Some((pat, times)) = self.chunks.get(self.idx) {
                let total = pat.len() as u64 * times;
                if self.pos >= total {
                    self.idx += 1;
                    self.pos = 0;
                    continue;
                }
                let mut n = 0;
                while n < out.len() && self.pos < total {
                    let k = (self.pos % pat.len() as u64) as usize;
                    let m = (pat.len() - k).min(out.len() - n);
                    out[n..n + m].copy_from_slice(&pat[k..k + m]);
                    n += m;
                    self.pos += m as u64;
                }
                return Ok(n);
            }
            Ok(0)
        }
    }

    #[test]
    fn digest_streams_a_huge_log_in_bounded_memory() {
        let delta = b"{\"type\":\"message_update\",\"assistantMessageEvent\":{\"delta\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}}\n".to_vec();
        let reps = (256u64 << 20) / delta.len() as u64; // ≈ 256 MB of deltas
        let call = end(serde_json::json!({ "role": "assistant", "content": [{ "type": "toolCall", "name": "bash", "arguments": { "i": "x" } }] })).into_bytes();
        let fin = end(serde_json::json!({ "role": "assistant", "content": [{ "type": "text", "text": "done" }] })).into_bytes();
        let chunks = vec![
            (call.clone(), 1),
            (delta.clone(), reps / 2),
            (b"{\"type\":\"tool_execution_update\",\"partial\":\"".to_vec(), 1),
            (vec![b'y'; 4096], 16 << 10), // one 64 MB line, never a message
            (b"\"}\n".to_vec(), 1),
            (call, 1),
            (delta, reps / 2),
            (fin, 1),
        ];
        let mut f = Facts::default();
        let mut pending = None;
        let max = scan_lines(Synthetic { chunks, idx: 0, pos: 0 }, HEAD, is_message, MAX_LINE, |l| {
            if let Some(m) = message_of(l) {
                on_message(&m, &mut f, &mut pending);
            }
        })
        .unwrap();
        assert_eq!((f.tools, f.last_text.as_deref()), (2, Some("done")));
        assert!(max < 4096, "only message lines are buffered, got {max} bytes");
    }

    #[test]
    fn kept_line_over_cap_is_dropped_and_last_line_without_newline_kept() {
        let mut seen = Vec::new();
        let data = b"{\"type\":\"message_end\",\"x\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}\nshort\n{\"type\":\"message_end\"}";
        scan_lines(&data[..], HEAD, is_message, 40, |l| seen.push(String::from_utf8_lossy(l).into_owned())).unwrap();
        assert_eq!(seen, ["{\"type\":\"message_end\"}"]);
    }

    /// T-006 live case: one long `task` call whose start is MBs back; the tail is only
    /// its progress updates (and a bare stream update of an edit).
    #[test]
    fn tail_of_a_long_task_call_names_its_running_subagents() {
        let path = std::env::temp_dir().join(format!("ckit-tail-task-{}.jsonl", std::process::id()));
        let upd = |intent: &str| {
            format!(
                "{{\"type\":\"tool_execution_update\",\"toolCallId\":\"t1\",\"toolName\":\"task\",\"args\":{{\"context\":\"{}\",\"tasks\":[{{\"name\":\"PlanReview3\",\"agent\":\"reviewer\"}}]}},\"partialResult\":{{\"content\":[{{\"type\":\"text\",\"text\":\"Running agent PlanReview3...\"}}],\"details\":{{\"progress\":[{{\"id\":\"BaM2\",\"agent\":\"ba\",\"status\":\"completed\",\"lastIntent\":\"done\"}},{{\"id\":\"PlanReview3\",\"agent\":\"reviewer\",\"status\":\"running\",\"lastIntent\":\"{intent}\"}}]}}}}}}\n",
                "c".repeat(2000)
            )
        };
        let mut log = String::from("{\"type\":\"tool_execution_start\",\"toolCallId\":\"w\",\"toolName\":\"wait\",\"args\":{},\"intent\":\"Waiting for plan reviewer\"}\n");
        for i in 0..200 {
            log += &upd(&format!("Checking file {i}"));
        }
        log += "{\"type\":\"tool_stream_update\",\"toolCallId\":\"e9\",\"toolName\":\"edit\"}\n";
        std::fs::write(&path, log).unwrap();
        let got: Vec<(Option<String>, String)> = tail_actions(&path, 5).into_iter().map(|a| (a.tool, a.text)).collect();
        assert_eq!(got, [(Some("task".into()), "đang chờ subagent: PlanReview3 (reviewer): Checking file 199".into())], "200 updates of one call are one action, the newest progress wins; a bare stream update with no start in view says nothing");
        // Only streaming deltas in the last 1 MB: the newest event still says the step is alive.
        let delta = "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"delta\":\"".to_string() + &"x".repeat(500) + "\"}}\n";
        std::fs::write(&path, delta.repeat(2600)).unwrap();
        let got = tail_actions(&path, 5);
        assert_eq!((got.len(), got[0].text.as_str(), got[0].at.is_some()), (1, "đang viết trả lời", true));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn digest_cache_keyed_by_log_size() {
        let dir = std::env::temp_dir().join(format!("ckit-digest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/dev-1.jsonl"), sample()).unwrap();
        assert!(cached(&dir, "dev-1.jsonl").is_none());
        let f = of_log(&dir, "dev-1.jsonl");
        assert_eq!(cached(&dir, "dev-1.jsonl"), Some(Facts { last: String::new(), ..f.clone() }), "the PM-only `last` is not cached");
        std::fs::write(dir.join("logs/dev-1.jsonl"), format!("{}{}", sample(), end(serde_json::json!({ "role": "assistant", "content": [{ "type": "toolCall", "name": "bash" }] })))).unwrap();
        assert!(cached(&dir, "dev-1.jsonl").is_none(), "a grown log is re-digested");
        assert_eq!(of_log(&dir, "dev-1.jsonl").tools, f.tools + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
