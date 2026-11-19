//! Which model a run uses and how much subscription quota is left.
//!
//! - Model: the last assistant `turn_end` of a run's omp event log carries
//!   `message.{provider, model, requestControls.effort, credentialId}`. Logs grow
//!   to hundreds of MB, so only the tail is read, cached by (path, mtime, len).
//!   Subagents run in child sessions `sessions/<ts>_<session id>/<Name>.jsonl`
//!   whose header holds `model_change` + `thinking_level_change`.
//! - Quota: `omp usage --json` (per provider account: windows with used %,
//!   reset time), refreshed in the background at most once a minute. Accounts are
//!   tied to the `credentialId` of runs through omp's credential store
//!   (`auth_credentials.identity_key`, metadata only, never the secret blob).
//!   Emails never leave this module unredacted.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use axum::{routing::get, Json, Router};
use serde::Serialize;

use super::{active_root, list, omp_cmd, roots, task_dir, ApiRes, Run, Status, Task};

/// Model + effort of one run (or one subagent group).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub(super) struct RunModel {
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
    pub credential_id: Option<i64>,
    /// Distinct subagent models of the run (detail only), most used first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<SubModel>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct SubModel {
    pub model: String,
    pub effort: Option<String>,
    pub count: u32,
}

// ── run model ───────────────────────────────────────────────────────────────

/// `provider/model` split; a bare name keeps an empty provider.
fn split_model(s: &str) -> (String, String) {
    match s.split_once('/') {
        Some((p, m)) => (p.to_string(), m.to_string()),
        None => (String::new(), s.to_string()),
    }
}

/// Model of an assistant `turn_end`/`message_end` event line; None for any other line.
fn parse_turn(line: &[u8]) -> Option<RunModel> {
    let v: serde_json::Value = serde_json::from_slice(line).ok()?;
    let m = v.get("message")?;
    let s = |p: &str| m.pointer(p).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_string);
    let model = s("/model")?;
    Some(RunModel {
        provider: s("/provider").unwrap_or_default(),
        model,
        effort: s("/requestControls/effort/topLevel").or_else(|| s("/requestControls/effort/tail")),
        credential_id: m.get("credentialId").and_then(|x| x.as_i64()),
        subagents: Vec::new(),
    })
}

/// Last assistant turn's model in the log, reading growing tail windows (a single
/// `agent_end` line can be ~1 MB, a long turn many MB of streaming updates).
fn tail_model(path: &FsPath) -> Option<RunModel> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    for window in [1u64 << 18, 1 << 22, 1 << 25] {
        let start = len.saturating_sub(window);
        f.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = Vec::with_capacity((len - start) as usize);
        (&mut f).take(len - start).read_to_end(&mut buf).ok()?;
        let mut lines: Vec<&[u8]> = buf.split(|b| *b == b'\n').collect();
        if start > 0 && !lines.is_empty() {
            lines.remove(0); // partial first line
        }
        let hit = lines
            .iter()
            .rev()
            .filter(|l| l.starts_with(br#"{"type":"turn_end""#) || l.starts_with(br#"{"type":"message_end""#))
            .find_map(|l| parse_turn(l));
        if hit.is_some() || start == 0 {
            return hit;
        }
    }
    None
}

/// Header of an omp session file: (model change, thinking level, session start).
fn session_head(path: &FsPath) -> Option<(String, Option<String>, String)> {
    let mut buf = Vec::new();
    std::fs::File::open(path).ok()?.take(16 << 10).read_to_end(&mut buf).ok()?;
    let (mut model, mut effort, mut at) = (None, None, String::new());
    for l in buf.split(|b| *b == b'\n') {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(l) else { continue };
        match v["type"].as_str() {
            Some("session") => at = v["timestamp"].as_str().unwrap_or_default().to_string(),
            Some("model_change") if model.is_none() => model = v["model"].as_str().map(str::to_string),
            Some("thinking_level_change") if effort.is_none() => effort = v["thinkingLevel"].as_str().map(str::to_string),
            _ => {}
        }
        if model.is_some() && effort.is_some() {
            break;
        }
    }
    model.map(|m| (m, effort, at))
}

/// Session id the run log starts with (`{"type":"session","id":…}`).
pub(super) fn log_session_id(log: &FsPath) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(log).ok()?.take(4096).read_to_end(&mut buf).ok()?;
    let first = buf.split(|b| *b == b'\n').next()?;
    let v: serde_json::Value = serde_json::from_slice(first).ok()?;
    (v["type"] == "session").then(|| v["id"].as_str().map(str::to_string)).flatten()
}

type Stamp = (SystemTime, u64);

fn stamp(path: &FsPath) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn model_cache() -> &'static Mutex<HashMap<PathBuf, (Stamp, Option<RunModel>)>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, (Stamp, Option<RunModel>)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Cached `tail_model`, falling back to the main session header (a run before its
/// first turn ends: model + effort, no credential yet).
fn log_model(dir: &FsPath, log: &FsPath) -> Option<RunModel> {
    let st = stamp(log)?;
    if let Some((s, m)) = model_cache().lock().ok()?.get(log) {
        if *s == st {
            return m.clone();
        }
    }
    let m = tail_model(log).or_else(|| {
        let id = log_session_id(log)?;
        let file = session_entries(dir).into_iter().find(|p| p.is_file() && name_of(p).ends_with(&format!("_{id}.jsonl")))?;
        let (model, effort, _) = session_head(&file)?;
        let (provider, model) = split_model(&model);
        Some(RunModel { provider, model, effort, ..Default::default() })
    });
    if let Ok(mut c) = model_cache().lock() {
        c.insert(log.to_path_buf(), (st, m.clone()));
    }
    m
}

fn name_of(p: &FsPath) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn session_entries(dir: &FsPath) -> Vec<PathBuf> {
    std::fs::read_dir(dir.join("sessions")).into_iter().flatten().flatten().map(|e| e.path()).collect()
}

fn head_cache() -> &'static Mutex<HashMap<PathBuf, (String, Option<String>, String)>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, (String, Option<String>, String)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Child sessions of a run: its session's subagent files started inside the run's
/// time window (resumed runs share a session), as (id = file stem, path, model,
/// effort), start order. The advisor sidecar is skipped.
pub(super) fn subagent_sessions(dir: &FsPath, log: &FsPath, run: &Run) -> Vec<(String, PathBuf, String, Option<String>)> {
    let Some(id) = log_session_id(log) else { return Vec::new() };
    let Some(sdir) = session_entries(dir).into_iter().find(|p| p.is_dir() && name_of(p).ends_with(&format!("_{id}"))) else {
        return Vec::new();
    };
    let (from, to) = (&run.started[..run.started.len().min(19)], run.ended.as_deref().map(|e| &e[..e.len().min(19)]));
    let mut out: Vec<(String, String, PathBuf, String, Option<String>)> = Vec::new();
    for p in std::fs::read_dir(&sdir).into_iter().flatten().flatten().map(|e| e.path()) {
        let name = name_of(&p);
        if !name.ends_with(".jsonl") || name.starts_with("__") || name.starts_with('.') {
            continue;
        }
        let cached = head_cache().lock().ok().and_then(|c| c.get(&p).cloned());
        let Some((model, effort, at)) = cached.or_else(|| {
            let h = session_head(&p)?; // headers never change once written
            head_cache().lock().ok()?.insert(p.clone(), h.clone());
            Some(h)
        }) else {
            continue;
        };
        let at = at[..at.len().min(19)].to_string();
        if at.as_str() < from || to.is_some_and(|t| at.as_str() > t) {
            continue;
        }
        out.push((at, name.trim_end_matches(".jsonl").to_string(), p, split_model(&model).1, effort));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, id, p, m, e)| (id, p, m, e)).collect()
}

/// Subagent models of a run, grouped, most used first.
fn subagents(dir: &FsPath, log: &FsPath, run: &Run) -> Vec<SubModel> {
    let mut groups: Vec<SubModel> = Vec::new();
    for (_, _, model, effort) in subagent_sessions(dir, log, run) {
        match groups.iter_mut().find(|g| g.model == model && g.effort == effort) {
            Some(g) => g.count += 1,
            None => groups.push(SubModel { model, effort, count: 1 }),
        }
    }
    groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.model.cmp(&b.model)));
    groups
}

/// Model of the task's current (or last) run, for the summary.
pub(super) fn run_model(root: &FsPath, t: &Task) -> Option<RunModel> {
    let r = t.runs.last()?;
    let dir = task_dir(root, &t.id);
    log_model(&dir, &dir.join("logs").join(&r.log))
}

/// Model of every run (with its subagents), for the detail's run list.
pub(super) fn run_models(root: &FsPath, t: &Task) -> Vec<Option<RunModel>> {
    let dir = task_dir(root, &t.id);
    t.runs
        .iter()
        .map(|r| {
            let log = dir.join("logs").join(&r.log);
            let mut m = log_model(&dir, &log)?;
            m.subagents = subagents(&dir, &log, r);
            Some(m)
        })
        .collect()
}

// ── quota ───────────────────────────────────────────────────────────────────

const REFRESH: Duration = Duration::from_secs(60);
const OMP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Window {
    id: String,
    label: String,
    used_pct: f64,
    /// epoch ms
    resets_at: Option<i64>,
    status: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Account {
    provider: String,
    /// redacted: `tu…@d…` / `c703b3…`
    label: String,
    org: Option<String>,
    plan: Option<String>,
    credential_ids: Vec<i64>,
    limit_reached: bool,
    fetched_at: Option<i64>,
    windows: Vec<Window>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Disabled {
    provider: String,
    label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
struct Usage {
    generated_at: Option<i64>,
    accounts: Vec<Account>,
    disabled: Vec<Disabled>,
}

/// A row of omp's `auth_credentials` (no secrets).
#[derive(Clone, Debug)]
struct Cred {
    id: i64,
    provider: String,
    identity: String,
}

fn creds() -> Vec<Cred> {
    let Some(db) = dirs::home_dir().map(|h| h.join(".omp/agent/agent.db")) else { return Vec::new() };
    let Ok(c) = rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX) else {
        return Vec::new();
    };
    let Ok(mut q) = c.prepare("SELECT id, provider, COALESCE(identity_key, '') FROM auth_credentials") else { return Vec::new() };
    q.query_map([], |r| Ok(Cred { id: r.get(0)?, provider: r.get(1)?, identity: r.get(2)? }))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// `tungnt@dante.vn` → `tu…@d…`; any email inside a longer string is replaced.
fn redact(s: &str) -> String {
    s.split_inclusive(|c: char| c.is_whitespace() || c == '\'' || c == '"')
        .map(|w| {
            let (word, tail) = w.split_at(w.trim_end_matches(|c: char| c.is_whitespace() || c == '\'' || c == '"').len());
            match word.split_once('@') {
                Some((user, dom)) if !user.is_empty() && dom.contains('.') => {
                    format!("{}…@{}…{tail}", user.chars().take(2).collect::<String>(), dom.chars().next().unwrap_or('?'))
                }
                _ => w.to_string(),
            }
        })
        .collect()
}

fn label_of(meta: &serde_json::Value) -> String {
    if let Some(e) = meta["email"].as_str().filter(|e| e.contains('@')) {
        return redact(e);
    }
    meta["accountId"].as_str().map(|a| format!("{}…", a.chars().take(6).collect::<String>())).unwrap_or_else(|| "tài khoản".into())
}

/// Every `k:v` of the credential's identity key (`email:…|org:…`, `account:…`)
/// matches the report's metadata.
fn identity_matches(identity: &str, meta: &serde_json::Value) -> bool {
    !identity.is_empty()
        && identity.split('|').all(|seg| {
            let Some((k, v)) = seg.split_once(':') else { return true };
            let field = match k {
                "email" => "email",
                "org" => "orgId",
                "account" => "accountId",
                _ => return true,
            };
            meta[field].as_str() == Some(v)
        })
}

fn parse_usage(raw: &serde_json::Value, creds: &[Cred]) -> Usage {
    let accounts = raw["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            let provider = r["provider"].as_str().unwrap_or_default().to_string();
            let meta = &r["metadata"];
            let windows = r["limits"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| {
                    let amt = &l["amount"];
                    let frac = amt["usedFraction"].as_f64().or_else(|| {
                        let (u, lim) = (amt["used"].as_f64()?, amt["limit"].as_f64()?);
                        (lim > 0.0).then(|| u / lim)
                    });
                    Window {
                        id: l.pointer("/window/id").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                        label: l["label"].as_str().or_else(|| l.pointer("/window/label").and_then(|x| x.as_str())).unwrap_or_default().to_string(),
                        used_pct: (frac.unwrap_or(0.0) * 1000.0).round() / 10.0,
                        resets_at: l.pointer("/window/resetsAt").and_then(|x| x.as_i64()),
                        status: l["status"].as_str().unwrap_or_default().to_string(),
                    }
                })
                .collect();
            Account {
                credential_ids: creds.iter().filter(|c| c.provider == provider && identity_matches(&c.identity, meta)).map(|c| c.id).collect(),
                label: label_of(meta),
                org: meta["orgName"].as_str().map(redact),
                plan: meta["planType"].as_str().map(str::to_string),
                limit_reached: meta["limitReached"].as_bool().unwrap_or(false),
                fetched_at: r["fetchedAt"].as_i64(),
                provider,
                windows,
            }
        })
        .collect();
    let disabled = raw["disabledCredentials"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| Disabled { provider: d["provider"].as_str().unwrap_or_default().to_string(), label: label_of(d) })
        .collect();
    Usage { generated_at: raw["generatedAt"].as_i64(), accounts, disabled }
}

#[derive(Default)]
struct State {
    usage: Option<Usage>,
    error: Option<String>,
    at: Option<Instant>,
    refreshing: bool,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// `omp usage --json`, killed after `OMP_TIMEOUT`.
fn fetch() -> Result<serde_json::Value, String> {
    let mut child = omp_cmd()
        .args(["usage", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("không chạy được omp usage: {e}"))?;
    let mut out = child.stdout.take().ok_or("omp usage: không có stdout")?;
    let reader = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let t0 = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t0.elapsed() < OMP_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("omp usage quá 20 giây".into());
            }
        }
    };
    let raw = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("omp usage lỗi (mã {})", status.code().unwrap_or(-1)));
    }
    serde_json::from_slice(&raw).map_err(|_| "omp usage trả về dữ liệu không đọc được".to_string())
}

/// Start a background refresh when the snapshot is older than `REFRESH`.
fn kick() {
    let Ok(mut s) = state().lock() else { return };
    if s.refreshing || s.at.is_some_and(|a| a.elapsed() < REFRESH) {
        return;
    }
    s.refreshing = true;
    drop(s);
    std::thread::spawn(|| {
        let res = fetch().map(|raw| parse_usage(&raw, &creds()));
        if let Ok(mut s) = state().lock() {
            match res {
                Ok(u) => {
                    s.usage = Some(u);
                    s.error = None;
                }
                Err(e) => s.error = Some(e),
            }
            s.at = Some(Instant::now());
            s.refreshing = false;
        }
    });
}

/// A running task and the model/account it is on.
#[derive(Serialize, Clone, Debug, PartialEq)]
struct InUse {
    project: String,
    id: String,
    title: String,
    provider: String,
    model: String,
    effort: Option<String>,
    credential_id: Option<i64>,
}

fn running() -> Vec<InUse> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut all = roots(&home);
    if let Some(c) = active_root() {
        if !all.contains(&c) {
            all.push(c);
        }
    }
    all.iter()
        .flat_map(|r| list(r).into_iter().filter(|t| t.status == Status::Running).map(move |t| (r.clone(), t)))
        .filter_map(|(r, t)| {
            let m = run_model(&r, &t)?;
            Some(InUse {
                project: r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                id: t.id,
                title: t.title,
                provider: m.provider,
                model: m.model,
                effort: m.effort,
                credential_id: m.credential_id,
            })
        })
        .collect()
}

/// Index of the account a running task draws from: its credential, else the
/// provider's only account (a run before its first turn has no credential yet).
fn account_of(accounts: &[Account], u: &InUse) -> Option<usize> {
    if let Some(i) = u.credential_id.and_then(|c| accounts.iter().position(|a| a.credential_ids.contains(&c))) {
        return Some(i);
    }
    let mut same = accounts.iter().enumerate().filter(|(_, a)| a.provider == u.provider);
    match (same.next(), same.next()) {
        (Some((i, _)), None) => Some(i),
        _ => None,
    }
}

fn view(usage: Option<&Usage>, error: Option<&str>, loading: bool, in_use: Vec<InUse>) -> serde_json::Value {
    let accounts: Vec<serde_json::Value> = usage
        .map(|u| {
            u.accounts
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let tasks: Vec<&InUse> = in_use.iter().filter(|t| account_of(&u.accounts, t) == Some(i)).collect();
                    let mut v = serde_json::to_value(a).unwrap_or_default();
                    v["tasks"] = serde_json::json!(tasks);
                    v
                })
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({
        "loading": loading && usage.is_none(),
        "error": error,
        "generated_at": usage.and_then(|u| u.generated_at),
        "accounts": accounts,
        "disabled": usage.map(|u| u.disabled.clone()).unwrap_or_default(),
        "running": in_use,
    })
}

/// `GET /api/board/usage` — never waits on omp: returns the last snapshot and
/// refreshes it in the background.
async fn api_usage() -> ApiRes<serde_json::Value> {
    kick();
    let in_use = tokio::task::spawn_blocking(running).await.unwrap_or_default();
    let s = state().lock().map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "usage lock".to_string()))?;
    Ok(Json(view(s.usage.as_ref(), s.error.as_deref(), s.refreshing, in_use)))
}

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/api/board/usage", get(api_usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ckit-usage-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const TURN: &str = r#"{"type":"turn_end","message":{"role":"assistant","api":"anthropic-messages","provider":"anthropic","model":"claude-opus-5-5","usage":{"input":2,"output":10},"requestControls":{"messageIndex":3,"effort":{"topLevel":"high","tail":"medium"}},"credentialId":6},"toolResults":[]}"#;

    #[test]
    fn turn_end_gives_provider_model_effort_credential() {
        let m = parse_turn(TURN.as_bytes()).unwrap();
        assert_eq!((m.provider.as_str(), m.model.as_str(), m.effort.as_deref(), m.credential_id), ("anthropic", "claude-opus-5-5", Some("high"), Some(6)));
        let tail_only = r#"{"type":"turn_end","message":{"provider":"openai-codex","model":"gpt-5.5","requestControls":{"effort":{"tail":"low"}}}}"#;
        let m = parse_turn(tail_only.as_bytes()).unwrap();
        assert_eq!((m.effort.as_deref(), m.credential_id), (Some("low"), None));
        assert!(parse_turn(br#"{"type":"message_end","message":{"role":"user","content":"hi"}}"#).is_none());
        assert!(parse_turn(b"not json").is_none());
    }

    #[test]
    fn tail_finds_last_turn_behind_huge_lines_and_falls_back_to_session_head() {
        let dir = tmp("tail");
        let logs = dir.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let log = logs.join("dev-1.jsonl");
        let first = TURN.replace("claude-opus-5-5", "old-model");
        let big = format!("{{\"type\":\"agent_end\",\"messages\":\"{}\"}}", "x".repeat(600_000)); // > first tail window
        std::fs::write(&log, format!("{{\"type\":\"session\",\"id\":\"abc\"}}\n{first}\n{TURN}\n{big}\n{{\"type\":\"message_update\"}}\n")).unwrap();
        let run = Run { stage: super::super::Stage::Dev, n: 1, started: "2026-10-15T14:00:00Z".into(), ended: None, exit: None, cost: 0.0, verdict: None, log: "dev-1.jsonl".into(), phase: None, round: None, title: None };
        let m = log_model(&dir, &log).unwrap();
        assert_eq!((m.model.as_str(), m.effort.as_deref()), ("claude-opus-5-5", Some("high")), "last turn wins");

        // A run with no finished turn yet: model + effort from the session header.
        let log2 = logs.join("dev-2.jsonl");
        std::fs::write(&log2, "{\"type\":\"session\",\"id\":\"def\"}\n{\"type\":\"agent_start\"}\n").unwrap();
        let sessions = dir.join("sessions");
        std::fs::create_dir_all(sessions.join("2026-10-15T14-00-00-000Z_abc")).unwrap();
        let head = |model: &str, lvl: &str, at: &str| {
            format!("{{\"type\":\"title\",\"v\":1}}\n{{\"type\":\"session\",\"id\":\"x\",\"timestamp\":\"{at}\"}}\n{{\"type\":\"model_change\",\"model\":\"{model}\"}}\n{{\"type\":\"thinking_level_change\",\"thinkingLevel\":\"{lvl}\"}}\n")
        };
        std::fs::write(sessions.join("2026-10-15T14-00-00-000Z_def.jsonl"), head("anthropic/claude-sonnet-5", "low", "2026-10-15T14:00:00.000Z")).unwrap();
        let m = log_model(&dir, &log2).unwrap();
        assert_eq!((m.provider.as_str(), m.model.as_str(), m.effort.as_deref(), m.credential_id), ("anthropic", "claude-sonnet-5", Some("low"), None));

        // Subagents of run 1: grouped, advisor skipped, other runs' children excluded.
        let child = sessions.join("2026-10-15T14-00-00-000Z_abc");
        std::fs::write(child.join("T1Fix.jsonl"), head("anthropic/claude-opus-5-5", "medium", "2026-10-15T14:10:00.000Z")).unwrap();
        std::fs::write(child.join("T2Fix.jsonl"), head("anthropic/claude-opus-5-5", "medium", "2026-10-15T14:20:00.000Z")).unwrap();
        std::fs::write(child.join("Scout.jsonl"), head("anthropic/claude-haiku-4-5", "low", "2026-10-15T14:30:00.000Z")).unwrap();
        std::fs::write(child.join("Before.jsonl"), head("anthropic/claude-haiku-4-5", "low", "2026-10-15T13:00:00.000Z")).unwrap();
        std::fs::write(child.join("__advisor.jsonl"), head("anthropic/claude-opus-5-5", "high", "2026-10-15T14:10:00.000Z")).unwrap();
        let subs = subagents(&dir, &log, &run);
        assert_eq!(
            subs,
            vec![
                SubModel { model: "claude-opus-5-5".into(), effort: Some("medium".into()), count: 2 },
                SubModel { model: "claude-haiku-4-5".into(), effort: Some("low".into()), count: 1 },
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "generatedAt": 1792147276302i64,
            "reports": [
                { "provider": "anthropic", "fetchedAt": 1792147269197i64,
                  "limits": [
                    { "id": "anthropic:5h", "label": "Claude 5 Hour", "window": { "id": "5h", "label": "5 Hour", "durationMs": 18000000, "resetsAt": 1792152000000i64 },
                      "amount": { "used": 72, "limit": 100, "usedFraction": 0.72, "unit": "percent" }, "status": "ok" },
                    { "id": "anthropic:7d", "label": "Claude 7 Day", "window": { "id": "7d", "resetsAt": 1792692000000i64 },
                      "amount": { "used": 3, "limit": 4 }, "status": "warning" } ],
                  "metadata": { "email": "alice.bob@example.test", "orgId": "org-1", "orgName": "alice.bob@example.test's Organization", "accountId": "acc-1" } },
                { "provider": "anthropic", "limits": [],
                  "metadata": { "email": "carol@example.test", "orgId": "org-2" } },
                { "provider": "xai-oauth", "limits": [], "metadata": { "accountId": "c703b3c1-cc93" } }
            ],
            "disabledCredentials": [ { "id": 3, "provider": "anthropic", "email": "dave@example.test" } ]
        })
    }

    #[test]
    fn usage_json_parses_windows_redacts_and_maps_credentials() {
        let creds = [
            Cred { id: 6, provider: "anthropic".into(), identity: "email:alice.bob@example.test|org:org-1".into() },
            Cred { id: 7, provider: "anthropic".into(), identity: "email:alice.bob@example.test|org:org-OTHER".into() },
            Cred { id: 8, provider: "anthropic".into(), identity: "email:carol@example.test|org:org-2".into() },
            Cred { id: 4, provider: "xai-oauth".into(), identity: "account:c703b3c1-cc93".into() },
        ];
        let u = parse_usage(&fixture(), &creds);
        let s = serde_json::to_string(&u).unwrap();
        assert!(!s.contains("example.test"), "no email leaks: {s}");
        let a = &u.accounts[0];
        assert_eq!((a.label.as_str(), a.org.as_deref(), a.credential_ids.as_slice()), ("al…@e…", Some("al…@e…'s Organization"), &[6][..]));
        assert_eq!(a.windows[0], Window { id: "5h".into(), label: "Claude 5 Hour".into(), used_pct: 72.0, resets_at: Some(1792152000000), status: "ok".into() });
        assert_eq!(a.windows[1].used_pct, 75.0, "used/limit when usedFraction is missing");
        assert_eq!(u.accounts[1].credential_ids, [8]);
        assert_eq!((u.accounts[2].label.as_str(), u.accounts[2].credential_ids.as_slice()), ("c703b3…", &[4][..]));
        assert_eq!(u.disabled, [Disabled { provider: "anthropic".into(), label: "da…@e…".into() }]);
        assert_eq!(u.generated_at, Some(1792147276302));

        // Running tasks land on their credential's account; without one, on the
        // provider's only account; ambiguous providers stay unassigned.
        let t = |p: &str, c: Option<i64>| InUse { project: "p".into(), id: "T-1".into(), title: "t".into(), provider: p.into(), model: "m".into(), effort: None, credential_id: c };
        assert_eq!(account_of(&u.accounts, &t("anthropic", Some(8))), Some(1));
        assert_eq!(account_of(&u.accounts, &t("anthropic", None)), None);
        assert_eq!(account_of(&u.accounts, &t("xai-oauth", None)), Some(2));
        let v = view(Some(&u), None, false, vec![t("anthropic", Some(6))]);
        assert_eq!(v["accounts"][0]["tasks"][0]["id"], "T-1");
        assert_eq!(v["accounts"][1]["tasks"].as_array().unwrap().len(), 0);
        assert_eq!(view(None, Some("x"), true, vec![])["loading"], true);
    }
}
