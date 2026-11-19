//! CloudGO PMS (pms.cloudgo.vn, vtiger CloudWORK) as a source of Board work:
//! the boss searches his PMS project tasks / HelpDesk tickets in "Tạo task"
//! and picks one instead of typing. READ-ONLY: list, retrieve and comment
//! listing; nothing is ever created, updated or deleted in PMS.
//!
//! Same calls as the boss's `mcp-pms-server` (`pms_list_tasks`, `pms_tickets`,
//! `pms_task_detail`, `pms_comments get`): CloudGoAppApi `Login` → token, then
//! CloudWorkApi v2 `getListProjectTask` / `getListTicket` / `retrieveProjectTask`
//! / `retrieveTicket` / `getRelatedListCommentById`. Filter options (`/meta`):
//! CloudWorkApi `getListProject`, `getListUser`; CloudGoAppApi `getCVList`
//! (saved lists → `Params.cvid`) and `getPicklistOption` (status/priority/category).
//! Reports (a third tab in the picker): OpenAPI `auth?username=&access_key_md5=`
//! → `Access-Token`, then `getReportData?report=&page=`. OpenAPI does not take
//! the CloudWORK token, so it needs the boss's PMS Access Key (optional).
//!
//! Credentials: ONLY `~/.config/ckit/pms.json` (0600: `base_url`, `username`,
//! `password`, `display_name`, optional `access_key`), written by the
//! dashboard's PMS panel after a successful login. `~/.omp/agent/mcp.json` is
//! read solely when the boss clicks "Dùng tài khoản từ MCP pms"
//! (`/api/board/pms/import-mcp`). HTTP via `curl`; tokens and the URL (which
//! carries md5(access key)) go in a 0600 `-K` file and the login body on
//! stdin, so no secret reaches `ps` argv. Nothing here logs, and no API
//! response carries the password, access key or a token.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use axum::{extract::Query, http::StatusCode, routing, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ApiErr, ApiRes};

const TIMEOUT_SECS: u64 = 10;
const PAGE_ROWS: u64 = 20;
const COMMENTS: u64 = 5;
/// Leftover curl config files (a dashboard killed mid-request) start with this.
pub(super) const CURL_CFG_PREFIX: &str = ".pms-curl-";

/// Where a Board task came from (stored on the task, shown as a link chip).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Source {
    pub(super) system: String,
    pub(super) kind: String,
    pub(super) id: String,
    #[serde(default)]
    pub(super) no: String,
    #[serde(default)]
    pub(super) url: String,
}

impl Source {
    /// The only accepted shape: `pms` + task|ticket|record (a report row of any
    /// other module) + numeric id + http(s) link.
    pub(super) fn validate(&self) -> Result<(), String> {
        let ok = self.system == "pms"
            && matches!(self.kind.as_str(), "task" | "ticket" | "record")
            && is_id(&self.id)
            && (self.url.is_empty() || self.url.starts_with("https://") || self.url.starts_with("http://"))
            && self.no.chars().count() <= 40;
        ok.then_some(()).ok_or_else(|| "nguồn PMS không hợp lệ".to_string())
    }
}

#[derive(Clone)]
struct Creds {
    base: String,
    user: String,
    pass: String,
    /// OpenAPI Access Key (reports only); "" = not set.
    key: String,
}

impl Creds {
    /// Trimmed, `base` without trailing `/`; None unless http(s) + user + password.
    fn new(base: &str, user: &str, pass: &str) -> Option<Creds> {
        let base = base.trim().trim_end_matches('/').to_string();
        let (user, pass) = (user.trim().to_string(), pass.to_string());
        let valid = (base.starts_with("https://") || base.starts_with("http://")) && !user.is_empty() && !pass.trim().is_empty();
        valid.then_some(Creds { base, user, pass, key: String::new() })
    }

    fn with_key(mut self, key: &str) -> Creds {
        self.key = key.trim().to_string();
        self
    }
}

/// `~/.config/ckit/pms.json`. Holds the password: always 0600.
#[derive(Serialize, Deserialize, Clone)]
struct Config {
    base_url: String,
    username: String,
    password: String,
    #[serde(default)]
    display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    access_key: String,
}

const DEFAULT_BASE: &str = "https://pms.cloudgo.vn";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn config_path(home: &Path) -> PathBuf {
    crate::brand::config_dir(home).join("pms.json")
}

fn load_config(home: &Path) -> Option<Config> {
    let c = serde_json::from_str::<Config>(&std::fs::read_to_string(config_path(home)).ok()?).ok()?;
    Creds::new(&c.base_url, &c.username, &c.password).map(|_| c)
}

/// Write `pms.json` created 0600 (never briefly world-readable), replacing any old file.
fn save_config(home: &Path, c: &Config) -> std::io::Result<()> {
    let p = config_path(home);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = p.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(c)?)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, &p)
}

/// The configured account: `pms.json` only. There is no implicit fallback.
fn creds_in(home: &Path) -> Option<Creds> {
    load_config(home).and_then(|c| Creds::new(&c.base_url, &c.username, &c.password).map(|cr| cr.with_key(&c.access_key)))
}

fn creds() -> Option<Creds> {
    creds_in(&home())
}

/// The `pms` MCP server's account in `~/.omp/agent/mcp.json`; only for the
/// explicit "Dùng tài khoản từ MCP pms" import.
fn mcp_creds(home: &Path) -> Option<Creds> {
    let raw = std::fs::read_to_string(home.join(".omp/agent/mcp.json")).ok()?;
    let mcp: Value = serde_json::from_str(&raw).ok()?;
    let env = mcp.get("mcpServers")?.get("pms")?.get("env")?;
    let s = |k: &str| env.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Creds::new(&s("PMS_BASE_URL"), &s("PMS_USERNAME"), &s("PMS_PASSWORD")).map(|c| c.with_key(&s("PMS_ACCESSKEY")))
}

// ── HTTP ────────────────────────────────────────────────────────────────────

fn err(code: StatusCode, msg: impl Into<String>) -> ApiErr {
    (code, msg.into())
}
fn not_configured() -> ApiErr {
    err(StatusCode::BAD_REQUEST, "Chưa cấu hình PMS")
}
fn login_failed() -> ApiErr {
    err(StatusCode::BAD_GATEWAY, "Đăng nhập PMS thất bại, kiểm tra tên đăng nhập và mật khẩu")
}
fn no_answer() -> ApiErr {
    err(StatusCode::GATEWAY_TIMEOUT, "PMS không phản hồi")
}

/// A value for a curl config line: `"…"` with `\` and `"` escaped.
fn cfg_quote(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], ""))
}

/// POST `body` (JSON, via stdin) to `url`.
fn post(url: &str, headers: &[String], body: &str) -> Result<(u16, Vec<u8>), ApiErr> {
    request(url, headers, Some(body), TIMEOUT_SECS)
}

/// GET (`body` None) or POST `url`. `url` and `headers` may hold secrets: they
/// go to curl in a 0600 config file. Returns (HTTP status, body).
fn request(url: &str, headers: &[String], body: Option<&str>, timeout: u64) -> Result<(u16, Vec<u8>), ApiErr> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = crate::brand::config_dir(&home());
    let cfg = dir.join(format!("{CURL_CFG_PREFIX}{}-{n}", std::process::id()));
    {
        let _ = std::fs::create_dir_all(&dir);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&cfg).map_err(|_| no_answer())?;
        let mut text = format!("url = {}\nheader = \"Content-Type: application/json\"\n", cfg_quote(url));
        for h in headers {
            text.push_str(&format!("header = {}\n", cfg_quote(h)));
        }
        f.write_all(text.as_bytes()).map_err(|_| no_answer())?;
    }
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "-m", &timeout.to_string(), "-w", "\n%{http_code}"]);
    if body.is_some() {
        cmd.args(["-X", "POST", "--data-binary", "@-"]);
    }
    let child = cmd
        .arg("-K")
        .arg(&cfg)
        .stdin(if body.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let out = child.ok().and_then(|mut c| {
        if let Some(b) = body {
            c.stdin.take()?.write_all(b.as_bytes()).ok()?;
        }
        c.wait_with_output().ok()
    });
    let _ = std::fs::remove_file(&cfg);
    let out = out.ok_or_else(no_answer)?;
    if !out.status.success() {
        return Err(no_answer());
    }
    let raw = out.stdout;
    let cut = raw.iter().rposition(|b| *b == b'\n').ok_or_else(no_answer)?;
    let code = std::str::from_utf8(&raw[cut + 1..]).ok().and_then(|s| s.trim().parse().ok()).ok_or_else(no_answer)?;
    Ok((code, raw[..cut].to_vec()))
}

#[derive(Clone)]
struct Session {
    /// Fingerprint of the credentials the token belongs to (config may change under us).
    key: u64,
    token: String,
    uid: String,
    /// `Họ Tên` from the login's `user_info`.
    name: String,
}

fn session_cache() -> &'static Mutex<Option<Session>> {
    static S: Mutex<Option<Session>> = Mutex::new(None);
    &S
}

fn cache_key(c: &Creds) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&c.base, &c.user, &c.pass).hash(&mut h);
    h.finish()
}

/// Forget the cached token (config changed or removed).
fn drop_session() {
    *session_cache().lock().unwrap_or_else(|e| e.into_inner()) = None;
    *openapi_cache().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// CloudGoAppApi `Login` (CloudWorkApi checks the token before its own Login).
fn login(c: &Creds) -> Result<Session, ApiErr> {
    let body = json!({ "RequestAction": "Login", "Credentials": { "username": c.user, "password": c.pass } }).to_string();
    let (code, raw) = post(&format!("{}/api/CloudGoAppApi.php", c.base), &[], &body)?;
    if code != 200 {
        return Err(login_failed());
    }
    let v: Value = serde_json::from_slice(&raw).map_err(|_| login_failed())?;
    let l = parse_login(&v).ok_or_else(login_failed)?;
    Ok(Session { key: cache_key(c), token: l.token, uid: l.uid, name: if l.name.is_empty() { c.user.clone() } else { l.name } })
}

/// Cached CloudWORK session; `fresh` drops the cached token first (after a 401).
fn session(c: &Creds, fresh: bool) -> Result<Session, ApiErr> {
    let key = cache_key(c);
    let mut g = session_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = g.as_ref().filter(|s| !fresh && s.key == key) {
        return Ok(s.clone());
    }
    *g = None;
    let s = login(c)?;
    *g = Some(s.clone());
    Ok(s)
}

#[derive(Debug, PartialEq)]
struct LoginInfo {
    token: String,
    uid: String,
    name: String,
}

/// `{status, data: {token, user_info: {id, last_name, first_name}}}`.
fn parse_login(v: &Value) -> Option<LoginInfo> {
    let p = v.get("data").filter(|d| d.is_object()).unwrap_or(v);
    let token = p.get("token")?.as_str().filter(|t| !t.is_empty())?.to_string();
    let u = p.get("user_info").cloned().unwrap_or(Value::Null);
    let uid = u.get("id").map(text).unwrap_or_default();
    let name = [field(&u, "last_name"), field(&u, "first_name")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    Some(LoginInfo { token, uid, name })
}

/// One CloudWorkApi v2 call → its `data` object. Re-logs in once on 401.
fn cloudwork(c: &Creds, action: &str, params: Value) -> Result<Value, ApiErr> {
    call(c, "CloudWorkApi.php", &json!({ "RequestAction": action, "Params": params }), true).and_then(check)
}

/// One CloudGoAppApi read (`getCVList`, `getPicklistOption`) → its `data`.
fn app(c: &Creds, action: &str, module: &str, data: Value) -> Result<Value, ApiErr> {
    call(c, "CloudGoAppApi.php", &json!({ "RequestAction": action, "Module": module, "Data": data }), false)
}

/// POST `body` to `/api/<endpoint>` with the session token; `{status, data}` → data.
fn call(c: &Creds, endpoint: &str, body: &Value, v2: bool) -> Result<Value, ApiErr> {
    let body = body.to_string();
    for attempt in 0..2 {
        let s = session(c, attempt > 0)?;
        let mut headers = vec![format!("Token: {}", s.token)];
        if v2 {
            headers.push("X-Api-Version: 2".to_string());
        }
        let (code, raw) = post(&format!("{}/api/{endpoint}", c.base), &headers, &body)?;
        let v: Option<Value> = serde_json::from_slice(&raw).ok();
        let envelope = v.as_ref().and_then(|v| v.get("status")).and_then(|s| s.as_i64());
        if code == 401 || envelope == Some(401) {
            if attempt == 0 {
                continue;
            }
            return Err(login_failed());
        }
        if code != 200 {
            return Err(err(StatusCode::BAD_GATEWAY, format!("PMS lỗi HTTP {code}")));
        }
        return v.and_then(|v| v.get("data").cloned()).ok_or_else(|| err(StatusCode::BAD_GATEWAY, "PMS trả về dữ liệu không đọc được"));
    }
    Err(login_failed())
}

/// `success: "0"` → the PMS message as an error.
fn check(data: Value) -> Result<Value, ApiErr> {
    if data.get("success").map(text).as_deref() == Some("0") {
        let msg = data.get("message").map(text).unwrap_or_default();
        let code = if msg.to_ascii_lowercase().contains("not found") { StatusCode::NOT_FOUND } else { StatusCode::BAD_GATEWAY };
        return Err(err(code, if msg.is_empty() { "PMS từ chối yêu cầu".to_string() } else { format!("PMS: {msg}") }));
    }
    Ok(data)
}

// ── OpenAPI (reports) ───────────────────────────────────────────────────────

const REPORT_TIMEOUT_SECS: u64 = 30;

fn no_key() -> ApiErr {
    err(StatusCode::BAD_REQUEST, "Chưa có Access Key PMS. Thêm Access Key ở nút PMS để xem report.")
}
fn key_failed() -> ApiErr {
    err(StatusCode::BAD_REQUEST, "Access Key PMS không đúng hoặc đã đổi. Cập nhật lại ở nút PMS.")
}

/// (fingerprint of base + user + key, OpenAPI access token).
fn openapi_cache() -> &'static Mutex<Option<(u64, String)>> {
    static S: Mutex<Option<(u64, String)>> = Mutex::new(None);
    &S
}

fn md5_hex(s: &str) -> String {
    use md5::{Digest, Md5};
    Md5::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// OpenAPI `auth`: md5(access key) → access token. A wrong key is a 401.
fn openapi_login(c: &Creds) -> Result<String, ApiErr> {
    if c.key.is_empty() {
        return Err(no_key());
    }
    let url = format!("{}/api/OpenAPI/auth?username={}&access_key_md5={}", c.base, urlencoding::encode(&c.user), md5_hex(&c.key));
    let (code, raw) = request(&url, &[], None, TIMEOUT_SECS)?;
    if code == 401 || code == 403 {
        return Err(key_failed());
    }
    if code != 200 {
        return Err(err(StatusCode::BAD_GATEWAY, format!("PMS lỗi HTTP {code}")));
    }
    serde_json::from_slice::<Value>(&raw).ok().map(|v| field(&v, "access_token")).filter(|t| !t.is_empty()).ok_or_else(key_failed)
}

fn openapi_token(c: &Creds, fresh: bool) -> Result<String, ApiErr> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&c.base, &c.user, &c.key).hash(&mut h);
    let key = h.finish();
    let mut g = openapi_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, t)) = g.as_ref().filter(|(k, _)| !fresh && *k == key) {
        return Ok(t.clone());
    }
    *g = None;
    let t = openapi_login(c)?;
    *g = Some((key, t.clone()));
    Ok(t)
}

/// GET `/api/OpenAPI/<path>` → raw JSON body. An expired token gets a 401 or a
/// plain-text "restricted file" page instead of JSON: log in again once.
fn openapi_get(c: &Creds, path: &str) -> Result<Vec<u8>, ApiErr> {
    for attempt in 0..2 {
        let t = openapi_token(c, attempt > 0)?;
        let (code, raw) = request(&format!("{}/api/OpenAPI/{path}", c.base), &[format!("Access-Token: {t}")], None, REPORT_TIMEOUT_SECS)?;
        let is_json = raw.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{');
        if code == 401 || (code == 200 && !is_json) {
            continue;
        }
        if code != 200 {
            return Err(err(StatusCode::BAD_GATEWAY, format!("PMS lỗi HTTP {code}")));
        }
        return Ok(raw);
    }
    Err(key_failed())
}

// ── Parsing (pure) ──────────────────────────────────────────────────────────

/// Scalar → string (PMS sends numbers as strings, but not always).
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn field(v: &Value, k: &str) -> String {
    v.get(k).map(text).unwrap_or_default().trim().to_string()
}

/// First non-empty of several fields.
fn first(v: &Value, keys: &[&str]) -> String {
    keys.iter().map(|k| field(v, k)).find(|s| !s.is_empty()).unwrap_or_default()
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit())
}

fn module(kind: &str) -> &'static str {
    if kind == "ticket" { "HelpDesk" } else { "ProjectTask" }
}

fn record_url(base: &str, kind: &str, id: &str) -> String {
    module_url(base, module(kind), id)
}

fn module_url(base: &str, module: &str, id: &str) -> String {
    format!("{base}/index.php?module={module}&view=Detail&record={id}")
}

/// Main owner's name (`Tên (email)` → `Tên`), else the first owner.
fn assignee(v: &Value) -> String {
    let owners = v.get("assigned_owners").and_then(|o| o.as_array()).cloned().unwrap_or_default();
    let main = format!("Users:{}", field(v, "main_owner_id"));
    let pick = owners.iter().find(|o| field(o, "id") == main).or_else(|| owners.first());
    pick.map(|o| field(o, "name")).map(|n| n.split(" (").next().unwrap_or("").trim().to_string()).unwrap_or_default()
}

#[derive(Serialize, Debug, Default, PartialEq)]
pub(super) struct Item {
    kind: &'static str,
    id: String,
    /// `PT12345` / `#67890`; task lists carry no number (only the detail does).
    no: String,
    title: String,
    status: String,
    priority: String,
    project: String,
    assignee: String,
    due: String,
    /// Last modified; empty in lists that do not return it.
    updated: String,
    created: String,
    /// Task type (`Coding`) / ticket category (`Bug`).
    category: String,
    /// Task progress (`40%`); empty for tickets.
    progress: String,
    /// Planned start (tasks).
    start: String,
    /// Logged hours (`2.50`); `0` reads as none.
    hours: String,
    url: String,
}

fn item(kind: &'static str, base: &str, v: &Value) -> Item {
    let id = first(v, &["id", "projecttaskid", "ticketid", "record_id"]);
    let task = kind == "task";
    Item {
        kind,
        url: record_url(base, kind, &id),
        no: if task { field(v, "projecttask_no") } else { field(v, "ticket_no") },
        title: if task { first(v, &["projecttaskname", "label"]) } else { first(v, &["ticket_title", "label"]) },
        status: if task { field(v, "projecttaskstatus") } else { field(v, "ticketstatus") },
        priority: if task { field(v, "projecttaskpriority") } else { field(v, "ticketpriorities") },
        project: if task { field(v, "projectid_label") } else { field(v, "related_project_name") },
        assignee: assignee(v),
        due: if task { first(v, &["end_date_plan", "enddate"]) } else { field(v, "expected_date") },
        updated: field(v, "modifiedtime"),
        created: field(v, "createdtime"),
        category: if task { field(v, "projecttasktype") } else { field(v, "ticketcategories") },
        progress: if task { field(v, "projecttaskprogress") } else { String::new() },
        start: if task { first(v, &["start_date_plan", "startdate"]) } else { String::new() },
        hours: Some(if task { field(v, "projecttaskhours") } else { field(v, "hours") })
            .filter(|h| h.parse::<f64>().map_or(!h.is_empty(), |n| n > 0.0))
            .unwrap_or_default(),
        id,
    }
}

/// `data` of getListProjectTask / getListTicket → (items, has_more).
pub(super) fn parse_list(kind: &'static str, base: &str, data: &Value) -> (Vec<Item>, bool) {
    let items: Vec<Item> = data.get("entry_list").and_then(|e| e.as_array()).into_iter().flatten().map(|v| item(kind, base, v)).collect();
    let paging = data.get("paging").cloned().unwrap_or(Value::Null);
    let num = |k: &str| field(&paging, k).parse::<u64>().ok();
    let has_more = match (num("next_offset"), num("total_count")) {
        (Some(next), Some(total)) => next < total,
        (Some(_), None) => true,
        _ => false,
    };
    (items, has_more)
}

/// What the boss filters a PMS search by. Values are checked before they reach
/// a CloudWORK filter (`safe_value`, numeric ids): PMS splices some of them into SQL.
#[derive(Default, Debug)]
pub(super) struct Filters {
    pub(super) q: String,
    /// Main owner's user id ("Của tôi" = the boss's own id).
    pub(super) owner: Option<String>,
    /// Preset: `open` (default) | `done` | `all`; ignored when `statuses` is set.
    pub(super) status: String,
    /// Exact picklist statuses (multi-select).
    pub(super) statuses: Vec<String>,
    pub(super) project_id: Option<String>,
    /// Tickets filter the related project by its name (CloudWORK compares the label).
    pub(super) project_name: Option<String>,
    /// vtiger custom view (saved list) id → `Params.cvid`.
    pub(super) view: Option<String>,
    pub(super) priority: Option<String>,
    /// Ticket category / task type.
    pub(super) category: Option<String>,
}

/// A picklist value safe to put in a CloudWORK filter: short, no list separator,
/// quote or backslash (PMS joins `e`/`n` values on commas).
pub(super) fn safe_value(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 60 && !s.chars().any(|c| matches!(c, ',' | '\'' | '"' | '\\' | ';') || c.is_control())
}

/// CloudWORK `Params` for a search.
pub(super) fn search_params(kind: &str, f: &Filters, page: u64) -> Value {
    let cond = |name: &str, value: &str, op: &str| json!({ "name": name, "value": value, "operator": op });
    let task = kind == "task";
    let mut filters = Vec::new();
    if let Some(uid) = f.owner.as_deref().filter(|u| is_id(u)) {
        filters.push(cond("main_owner_id", uid, "e"));
    }
    let col = if task { "projecttaskstatus" } else { "ticketstatus" };
    let statuses: Vec<&str> = f.statuses.iter().map(String::as_str).filter(|s| safe_value(s)).collect();
    if !statuses.is_empty() {
        filters.push(cond(col, &statuses.join(","), "e"));
    } else {
        match f.status.as_str() {
            "done" => filters.push(cond(col, if task { "Completed" } else { "Closed" }, "e")),
            "all" => {}
            // "Đang mở" = work still to do: a rejected ticket ("Từ chối") is as finished as a cancelled one.
            _ => filters.push(cond(col, if task { "Completed,Canceled" } else { "Closed,Cancel,Reject" }, "n")),
        }
    }
    if task {
        if let Some(id) = f.project_id.as_deref().filter(|p| is_id(p)) {
            filters.push(cond("projectid", id, "e"));
        }
    } else if let Some(name) = f.project_name.as_deref().filter(|n| !n.trim().is_empty()) {
        filters.push(cond("related_project", name, "e"));
    }
    if let Some(p) = f.priority.as_deref().filter(|p| safe_value(p)) {
        filters.push(cond(if task { "projecttaskpriority" } else { "ticketpriorities" }, p, "e"));
    }
    if let Some(cat) = f.category.as_deref().filter(|c| safe_value(c)) {
        filters.push(cond(if task { "projecttasktype" } else { "ticketcategories" }, cat, "e"));
    }
    let q: String = f.q.trim().chars().take(100).collect();
    if !q.is_empty() {
        let upper = q.to_uppercase();
        let digits = upper.trim_start_matches(if task { "PT" } else { "#" });
        if is_id(digits) {
            let no = if task { format!("PT{digits}") } else { format!("#{digits}") };
            filters.push(cond(if task { "projecttask_no" } else { "ticket_no" }, &no, "e"));
        } else {
            // `label` is CloudWORK's alias for the task name; tickets filter their title.
            filters.push(cond(if task { "label" } else { "ticket_title" }, &q, "c"));
        }
    }
    let mut p = json!({
        "filters": filters,
        "paging": { "offset": page * PAGE_ROWS, "max_rows": PAGE_ROWS, "order_by": "modifiedtime", "sort_order": "DESC" },
    });
    if let Some(cv) = f.view.as_deref().filter(|v| is_id(v)) {
        p["cvid"] = json!(cv);
    }
    p
}

// ── Filter metadata (saved lists, picklists, projects, people) ─────────────

#[derive(Serialize, Debug, Clone, PartialEq)]
pub(super) struct Opt {
    value: String,
    label: String,
    /// Saved lists: "Danh sách của tôi" / "Danh sách chia sẻ"; picklists: PMS colour.
    #[serde(skip_serializing_if = "String::is_empty")]
    group: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    color: String,
}

#[derive(Serialize, Debug, Clone, Default)]
pub(super) struct Meta {
    projects: Vec<Opt>,
    views: Vec<Opt>,
    statuses: Vec<Opt>,
    priorities: Vec<Opt>,
    categories: Vec<Opt>,
    users: Vec<Opt>,
}

/// `getCVList` data: `{Mine: {group_label, <id>: {id, display_name}}, Shared: {…}}`.
pub(super) fn parse_views(data: &Value) -> Vec<Opt> {
    let mut out = Vec::new();
    for key in ["Mine", "Shared"] {
        let Some(g) = data.get(key).and_then(|g| g.as_object()) else { continue };
        let group = g.get("group_label").map(text).unwrap_or_default();
        let mut items: Vec<Opt> = g
            .values()
            .filter(|v| v.is_object())
            .map(|v| Opt { value: field(v, "id"), label: field(v, "display_name"), group: group.clone(), color: String::new() })
            .filter(|o| is_id(&o.value) && !o.label.is_empty())
            .collect();
        items.sort_by(|a, b| a.label.cmp(&b.label));
        out.extend(items);
    }
    out
}

/// `getPicklistOption` data: `{options: [{value, label, color, assigned}]}` (role-assigned only).
pub(super) fn parse_picklist(data: &Value) -> Vec<Opt> {
    data.get("options")
        .and_then(|o| o.as_array())
        .into_iter()
        .flatten()
        .filter(|o| field(o, "assigned") != "0")
        .filter_map(|o| {
            // Raw value, untrimmed: PMS has `Canceled ` with a trailing space.
            let value = o.get("value").map(text).unwrap_or_default();
            let label = field(o, "label");
            safe_value(&value).then(|| Opt { label: if label.is_empty() { value.trim().to_string() } else { label }, value, group: String::new(), color: field(o, "color") })
        })
        .collect()
}

/// `getListProject` data → projects (newest-modified first, as asked).
pub(super) fn parse_projects(data: &Value) -> Vec<Opt> {
    data.get("entry_list")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .map(|p| Opt { value: first(p, &["projectid", "id"]), label: field(p, "projectname"), group: field(p, "projectstatus"), color: String::new() })
        .filter(|o| is_id(&o.value) && !o.label.is_empty())
        .collect()
}

/// `getListUser` data: `entry_list` is an object keyed by user id.
pub(super) fn parse_users(data: &Value) -> Vec<Opt> {
    let mut out: Vec<Opt> = match data.get("entry_list") {
        Some(Value::Object(m)) => m.values().cloned().collect::<Vec<_>>(),
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
    .iter()
    .map(|u| {
        let full = field(u, "full_name");
        let name = if full.is_empty() { [field(u, "last_name"), field(u, "first_name")].join(" ").trim().to_string() } else { full };
        Opt { value: field(u, "id"), label: name, group: String::new(), color: String::new() }
    })
    .filter(|o| is_id(&o.value) && !o.label.is_empty())
    .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

const META_TTL: std::time::Duration = std::time::Duration::from_secs(600);

type MetaCache = std::collections::HashMap<(u64, &'static str), (std::time::Instant, Meta)>;

fn meta_cache() -> &'static Mutex<MetaCache> {
    static M: std::sync::OnceLock<Mutex<MetaCache>> = std::sync::OnceLock::new();
    M.get_or_init(|| Mutex::new(MetaCache::new()))
}

/// Filter options for `kind`, cached 10 min per account. A failing piece is
/// left empty (the picker still works without it); login failures propagate.
fn meta(c: &Creds, kind: &'static str) -> Result<Meta, ApiErr> {
    let key = (cache_key(c), kind);
    if let Some((at, m)) = meta_cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        if at.elapsed() < META_TTL {
            return Ok(m.clone());
        }
    }
    session(c, false)?;
    let task = kind == "task";
    let module = module(kind);
    let pick = |f: &str| app(c, "getPicklistOption", module, json!({ "field": f })).map(|d| parse_picklist(&d)).unwrap_or_default();
    let m = Meta {
        views: app(c, "getCVList", module, json!({})).map(|d| parse_views(&d)).unwrap_or_default(),
        statuses: pick(if task { "projecttaskstatus" } else { "ticketstatus" }),
        priorities: pick(if task { "projecttaskpriority" } else { "ticketpriorities" }),
        categories: pick(if task { "projecttasktype" } else { "ticketcategories" }),
        projects: cloudwork(
            c,
            "getListProject",
            json!({ "filters": [{ "name": "projectstatus", "value": "10 - Closed", "operator": "n" }],
                    "paging": { "offset": 0, "max_rows": 100, "order_by": "modifiedtime", "sort_order": "DESC" } }),
        )
        .map(|d| parse_projects(&d))
        .unwrap_or_default(),
        users: all_users(c),
    };
    meta_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, (std::time::Instant::now(), m.clone()));
    Ok(m)
}

/// Every active user: `getListUser` pages by 100 (more → `success 0`).
fn all_users(c: &Creds) -> Vec<Opt> {
    let mut out = Vec::new();
    for page in 0..10u64 {
        let Ok(d) = cloudwork(c, "getListUser", json!({ "paging": { "offset": page * 100, "max_rows": 100 } })) else { break };
        let got = parse_users(&d);
        let n = got.len();
        out.extend(got);
        if n < 100 {
            break;
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out.dedup_by(|a, b| a.value == b.value);
    out
}

const FIND_ROWS: u64 = 30;

/// A typed search for projects/people: 1–60 chars, none of `' " \ ; ,` or controls
/// (PMS builds `LIKE` clauses from it).
pub(super) fn find_query(q: &str) -> Result<String, ApiErr> {
    let q = q.trim();
    let ok = !q.is_empty() && q.chars().count() <= 60 && !q.chars().any(|c| matches!(c, ',' | '\'' | '"' | '\\' | ';' | '%' | '_') || c.is_control());
    ok.then(|| q.to_string()).ok_or_else(|| err(StatusCode::BAD_REQUEST, "Từ khoá tìm kiếm không hợp lệ"))
}

/// Lowercase, Vietnamese diacritics and `đ` folded away: "Phường" → "phuong".
pub(super) fn fold_vi(s: &str) -> String {
    // Lowercase first, so only the lower-case forms need a row.
    const MAP: [(&str, char); 7] = [
        ("àáạảãâầấậẩẫăằắặẳẵ", 'a'),
        ("èéẹẻẽêềếệểễ", 'e'),
        ("ìíịỉĩ", 'i'),
        ("òóọỏõôồốộổỗơờớợởỡ", 'o'),
        ("ùúụủũưừứựửữ", 'u'),
        ("ỳýỵỷỹ", 'y'),
        ("đ", 'd'),
    ];
    s.to_lowercase()
        .chars()
        .map(|c| MAP.iter().find(|(set, _)| set.contains(c)).map(|(_, b)| *b).unwrap_or(c))
        .collect()
}

/// Options whose label contains `q`, diacritic- and case-insensitive; at most `max`.
pub(super) fn match_opts(list: &[Opt], q: &str, max: usize) -> Vec<Opt> {
    let q = fold_vi(q);
    list.iter().filter(|o| fold_vi(&o.label).contains(&q)).take(max).cloned().collect()
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Check {
    name: String,
    done: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Sub {
    title: String,
    status: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Comment {
    author: String,
    text: String,
    at: String,
}

#[derive(Serialize, Debug)]
pub(super) struct Detail {
    #[serde(flatten)]
    item: Item,
    description: String,
    checklist: Vec<Check>,
    subtasks: Vec<Sub>,
    comments: Vec<Comment>,
}

/// `data` of retrieveProjectTask / retrieveTicket (record under `data`).
pub(super) fn parse_detail(kind: &'static str, base: &str, data: &Value) -> Detail {
    let rec = data.get("data").cloned().unwrap_or(Value::Null);
    let mut description = html_to_text(&field(&rec, "description"));
    if kind == "ticket" {
        for (k, label) in [("steps_to_reproduce", "Các bước tái hiện"), ("expected_result", "Kết quả mong muốn"), ("actual_result", "Kết quả thực tế")] {
            let t = html_to_text(&field(&rec, k));
            if !t.is_empty() {
                description = format!("{description}\n\n{label}:\n{t}").trim().to_string();
            }
        }
    }
    let checklist = rec
        .get("checklist")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let name = html_to_text(&first(c, &["name", "title"]));
            (!name.is_empty()).then(|| Check { name, done: field(c, "status") == "1" })
        })
        .collect();
    let subtasks = rec
        .get("projecttask_related")
        .and_then(|r| r.get("entry_list"))
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .map(|s| Sub { title: first(s, &["projecttaskname", "label"]), status: field(s, "projecttaskstatus") })
        .filter(|s| !s.title.is_empty())
        .collect();
    Detail { item: item(kind, base, &rec), description, checklist, subtasks, comments: Vec::new() }
}

/// `data` of getRelatedListCommentById (newest first) → plain-text comments.
pub(super) fn parse_comments(data: &Value) -> Vec<Comment> {
    data.get("entry_list")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .map(|c| {
            let raw = field(c, "rawcontent");
            let text = if raw.is_empty() { html_to_text(&field(c, "commentcontent")) } else { html_to_text(&raw) };
            Comment { author: assignee(c), text, at: field(c, "createdtime") }
        })
        .filter(|c| !c.text.is_empty())
        .take(COMMENTS as usize)
        .collect()
}

// ── Reports (pure) ──────────────────────────────────────────────────────────

/// A JSON object's entries in document order (serde_json's `Map` sorts keys,
/// which would shuffle the report's columns). PHP sends `[]` for an empty one.
#[derive(Default)]
struct Ordered(Vec<(String, RawField)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some(e) = m.next_entry()? {
                    out.push(e);
                }
                Ok(Ordered(out))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<Ordered, A::Error> {
                while s.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Ordered::default())
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Deserialize)]
struct RawField {
    #[serde(default)]
    value: Value,
    #[serde(default, rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct RawRow {
    #[serde(default)]
    record_id: Value,
    #[serde(default)]
    fields: Ordered,
}

#[derive(Deserialize)]
struct RawReport {
    #[serde(default)]
    report_name: String,
    #[serde(default)]
    report_type: String,
    #[serde(default)]
    primary_module: String,
    #[serde(default)]
    has_next_page: bool,
    #[serde(default)]
    rows: Vec<RawRow>,
}

#[derive(Deserialize)]
struct RawEnvelope {
    #[serde(default)]
    success: Value,
    #[serde(default)]
    message: Value,
    data: Option<RawReport>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Cell {
    label: String,
    value: String,
}

#[derive(Serialize, Debug)]
pub(super) struct ReportRow {
    id: String,
    title: String,
    /// Non-empty columns in the report's order.
    cells: Vec<Cell>,
    url: String,
}

#[derive(Serialize, Debug)]
pub(super) struct Report {
    id: String,
    name: String,
    module: String,
    /// `task` / `ticket` when the rows are ProjectTask / HelpDesk records (the
    /// picker then opens them like a search result); None for any other module.
    kind: Option<&'static str>,
    /// A chart report: PMS returns no rows for it.
    chart: bool,
    rows: Vec<ReportRow>,
    has_more: bool,
    url: String,
}

const CELL_CHARS: usize = 2000;
const TITLE_CHARS: usize = 160;

/// Report column key → label: `CPTimeSheets_CPTimeSheets_LBL_ASSIGNED_TO` → `Phụ trách`,
/// `ProjectTask_projecttask_no` → `Mã task`, unknown `X_some_field` → `Some field`.
/// The module prefix appears once or twice; a known field name wins at the first match.
pub(super) fn column_label(module: &str, key: &str) -> String {
    let known = |k: &str| match k.strip_prefix("LBL_").unwrap_or(k).to_ascii_lowercase().as_str() {
        "title" | "ticket_title" => Some("Tiêu đề"),
        "projecttaskname" => Some("Tên task"),
        "projecttask_no" => Some("Mã task"),
        "ticket_no" => Some("Mã ticket"),
        "description" => Some("Mô tả"),
        "assigned_to" | "main_owner_id" | "assigned_user_id" => Some("Phụ trách"),
        "projecttaskstatus" | "ticketstatus" => Some("Tình trạng"),
        "projecttaskpriority" | "ticketpriorities" => Some("Ưu tiên"),
        "projecttasktype" => Some("Loại công việc"),
        "ticketcategories" => Some("Phân loại"),
        "projecttaskprogress" => Some("Tiến độ"),
        "projecttaskhours" | "total_hours" => Some("Số giờ"),
        "startdate" => Some("Bắt đầu"),
        "enddate" => Some("Kết thúc"),
        "project" => Some("Dự án"),
        "apply_date" => Some("Ngày"),
        "createdtime" => Some("Ngày tạo"),
        "modifiedtime" => Some("Cập nhật"),
        _ => None,
    };
    fn strip<'a>(k: &'a str, module: &str) -> Option<&'a str> {
        let p = k.get(..module.len()).filter(|p| !module.is_empty() && p.eq_ignore_ascii_case(module))?;
        k.get(p.len()..)?.strip_prefix('_')
    }
    let mut k = key;
    for _ in 0..3 {
        if let Some(l) = known(k) {
            return l.to_string();
        }
        match strip(k, module) {
            Some(rest) => k = rest,
            None => break,
        }
    }
    let words = k.strip_prefix("LBL_").unwrap_or(k).replace('_', " ");
    let words = words.trim();
    let mut cs = words.chars();
    cs.next().map(|c| c.to_uppercase().chain(cs).collect()).unwrap_or_else(|| key.to_string())
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max - 1).collect::<String>())
}

/// Display value: HTML stripped, an owner without its `(email)`, capped.
fn cell_text(f: &RawField) -> String {
    let t = html_to_text(&text(&f.value));
    let t = if f.kind == "owner" { t.split(" (").next().unwrap_or("").trim().to_string() } else { t };
    clip(&t, CELL_CHARS)
}

/// The row's name: a title/name/subject column, else the description, else the first value.
fn row_title(cells: &[(String, Cell)]) -> String {
    let titled = |k: &str| {
        let k = k.to_ascii_lowercase();
        ["title", "name", "subject"].iter().any(|s| k.ends_with(s))
    };
    let pick = cells
        .iter()
        .find(|(k, _)| titled(k))
        .or_else(|| cells.iter().find(|(k, _)| k.to_ascii_lowercase().ends_with("description")))
        .or_else(|| cells.first());
    pick.map(|(_, c)| clip(&c.value.split_whitespace().collect::<Vec<_>>().join(" "), TITLE_CHARS)).unwrap_or_default()
}

/// Body of OpenAPI `getReportData` → the picker's report (rows with ids only).
pub(super) fn parse_report(base: &str, id: &str, raw: &[u8]) -> Result<Report, ApiErr> {
    let env: RawEnvelope = serde_json::from_slice(raw).map_err(|_| err(StatusCode::BAD_GATEWAY, "PMS trả về report không đọc được"))?;
    if text(&env.success) != "1" {
        let msg = text(&env.message);
        if msg.to_ascii_lowercase().contains("not found") {
            return Err(err(StatusCode::NOT_FOUND, format!("Không tìm thấy report {id}, hoặc bạn không có quyền xem report này")));
        }
        return Err(err(StatusCode::BAD_GATEWAY, if msg.is_empty() { "PMS từ chối yêu cầu".to_string() } else { format!("PMS: {msg}") }));
    }
    let r = env.data.ok_or_else(|| err(StatusCode::BAD_GATEWAY, "PMS trả về report không đọc được"))?;
    // Goes into links: letters and digits only.
    let module = Some(r.primary_module.trim()).filter(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_alphanumeric())).unwrap_or("").to_string();
    let kind = match module.as_str() {
        "ProjectTask" => Some("task"),
        "HelpDesk" => Some("ticket"),
        _ => None,
    };
    let rows = r
        .rows
        .into_iter()
        .filter_map(|row| {
            let rid = text(&row.record_id);
            if !is_id(&rid) {
                return None;
            }
            let cells: Vec<(String, Cell)> = row
                .fields
                .0
                .into_iter()
                .map(|(k, f)| {
                    let c = Cell { label: column_label(&module, &k), value: cell_text(&f) };
                    (k, c)
                })
                .filter(|(_, c)| !c.value.is_empty())
                .collect();
            let title = row_title(&cells);
            let url = if module.is_empty() { String::new() } else { module_url(base, &module, &rid) };
            Some(ReportRow { title: if title.is_empty() { format!("#{rid}") } else { title }, cells: cells.into_iter().map(|(_, c)| c).collect(), url, id: rid })
        })
        .collect();
    Ok(Report {
        id: id.to_string(),
        name: r.report_name.trim().to_string(),
        kind,
        chart: !r.report_type.is_empty() && r.report_type != "tabular",
        rows,
        has_more: r.has_next_page,
        url: module_url(base, "Reports", id),
        module,
    })
}

/// PMS rich text (vtiger CKEditor HTML, sometimes plain text) → plain text.
pub(super) fn html_to_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        let Some(j) = after.find('>') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let tag = after[1..j].trim();
        let closing = tag.starts_with('/');
        let name = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        rest = &after[j + 1..];
        match name.as_str() {
            // Skip the element's content entirely.
            "head" | "style" | "script" | "title" if !closing => {
                let end = rest.to_ascii_lowercase().find(&format!("</{name}"));
                rest = match end {
                    Some(e) => rest[e..].find('>').map(|k| &rest[e + k + 1..]).unwrap_or(""),
                    None => "",
                };
            }
            "br" | "p" | "div" | "tr" | "ul" | "ol" | "table" | "blockquote" | "pre" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => out.push('\n'),
            "li" if !closing => out.push_str("\n- "),
            "td" | "th" if closing => out.push(' '),
            _ => {}
        }
    }
    out.push_str(rest);
    let decoded = decode_entities(&out).replace('\u{a0}', " ").replace("\r\n", "\n");
    let mut lines: Vec<&str> = Vec::new();
    for l in decoded.lines().map(str::trim) {
        if !(l.is_empty() && lines.last().is_none_or(|p| p.is_empty())) {
            lines.push(l);
        }
    }
    lines.join("\n").trim().to_string()
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        let end = after.char_indices().take(12).find(|(_, c)| *c == ';').map(|(k, _)| k);
        let ent = end.map(|e| &after[1..e]);
        let ch = ent.and_then(|e| match e {
            "nbsp" => Some('\u{a0}'),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => e
                .strip_prefix("#x")
                .or_else(|| e.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        });
        match (ch, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ── Dashboard API ───────────────────────────────────────────────────────────

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/board/pms", routing::post(api_connect))
        .route("/api/board/pms/status", get(api_status))
        .route("/api/board/pms/test", routing::post(api_test))
        .route("/api/board/pms/off", routing::post(api_off))
        .route("/api/board/pms/import-mcp", get(api_mcp_preview).post(api_mcp_import))
        .route("/api/board/pms/search", get(api_search))
        .route("/api/board/pms/item", get(api_item))
        .route("/api/board/pms/meta", get(api_meta))
        .route("/api/board/pms/projects", get(api_projects))
        .route("/api/board/pms/users", get(api_users))
        .route("/api/board/pms/key", routing::post(api_key))
        .route("/api/board/pms/report", get(api_report))
}

/// Status for the panel. Never the password, access key or token.
fn view_in(home: &Path) -> Value {
    match load_config(home) {
        Some(c) => {
            let name = if c.display_name.is_empty() { c.username.clone() } else { c.display_name };
            json!({ "configured": true, "base_url": c.base_url, "user": c.username, "display_name": name, "report": !c.access_key.trim().is_empty() })
        }
        None => json!({ "configured": false, "base_url": null, "user": null, "display_name": null, "report": false }),
    }
}

async fn api_status() -> Json<Value> {
    Json(view_in(&home()))
}

/// Log in with `c` (and check its access key, when given); only on success save
/// it as the account and seed the cache.
fn connect_in(home: &Path, c: &Creds) -> Result<Value, ApiErr> {
    let s = login(c).map_err(|e| if e.0 == StatusCode::BAD_GATEWAY { err(StatusCode::BAD_REQUEST, e.1) } else { e })?;
    if !c.key.is_empty() {
        openapi_token(c, true)?;
    }
    let cfg = Config { base_url: c.base.clone(), username: c.user.clone(), password: c.pass.clone(), display_name: s.name.clone(), access_key: c.key.clone() };
    save_config(home, &cfg).map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cấu hình PMS"))?;
    *session_cache().lock().unwrap_or_else(|e| e.into_inner()) = Some(s);
    Ok(view_in(home))
}

#[derive(Deserialize)]
struct ConnectBody {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    /// Optional: OpenAPI Access Key, for reports.
    #[serde(default)]
    access_key: String,
}

async fn api_connect(Json(b): Json<ConnectBody>) -> ApiRes<Value> {
    let base = if b.base_url.trim().is_empty() { DEFAULT_BASE } else { b.base_url.trim() };
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err(err(StatusCode::BAD_REQUEST, "Địa chỉ PMS phải bắt đầu bằng https://"));
    }
    if b.username.trim().is_empty() || b.password.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Nhập tên đăng nhập và mật khẩu PMS"));
    }
    let c = Creds::new(base, &b.username, &b.password).ok_or_else(|| err(StatusCode::BAD_REQUEST, "Thông tin PMS không hợp lệ"))?.with_key(&b.access_key);
    blocking(move || connect_in(&home(), &c)).await
}

/// Re-test the saved account with a fresh login; refreshes the display name.
async fn api_test() -> ApiRes<Value> {
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || {
        let s = session(&c, true)?;
        let h = home();
        if let Some(mut cfg) = load_config(&h).filter(|cfg| cfg.display_name != s.name) {
            cfg.display_name = s.name;
            let _ = save_config(&h, &cfg);
        }
        Ok(view_in(&h))
    })
    .await
}

#[derive(Deserialize)]
struct KeyBody {
    #[serde(default)]
    access_key: String,
}

/// Set (checked against OpenAPI first) or clear (`""`) the saved account's access key.
fn set_key_in(home: &Path, key: &str) -> Result<Value, ApiErr> {
    let mut cfg = load_config(home).ok_or_else(not_configured)?;
    let key = key.trim();
    if !key.is_empty() {
        let c = Creds::new(&cfg.base_url, &cfg.username, &cfg.password).ok_or_else(not_configured)?.with_key(key);
        openapi_token(&c, true)?;
    } else {
        *openapi_cache().lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    cfg.access_key = key.to_string();
    save_config(home, &cfg).map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cấu hình PMS"))?;
    Ok(view_in(home))
}

async fn api_key(Json(b): Json<KeyBody>) -> ApiRes<Value> {
    blocking(move || set_key_in(&home(), &b.access_key)).await
}

/// Remove `pms.json` and the cached token.
fn disconnect_in(home: &Path) -> Value {
    let _ = std::fs::remove_file(config_path(home));
    drop_session();
    view_in(home)
}

async fn api_off() -> Json<Value> {
    Json(disconnect_in(&home()))
}

/// What the import would use (no password), shown for the boss to confirm.
async fn api_mcp_preview() -> Json<Value> {
    Json(match mcp_creds(&home()) {
        Some(c) => json!({ "found": true, "base_url": c.base, "user": c.user, "report": !c.key.is_empty() }),
        None => json!({ "found": false, "base_url": null, "user": null, "report": false }),
    })
}

async fn api_mcp_import() -> ApiRes<Value> {
    let c = mcp_creds(&home()).ok_or_else(|| err(StatusCode::BAD_REQUEST, "Không tìm thấy tài khoản PMS trong MCP pms (~/.omp/agent/mcp.json)"))?;
    blocking(move || connect_in(&home(), &c)).await
}

#[derive(Deserialize)]
struct SearchQ {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    q: String,
    mine: Option<String>,
    #[serde(default)]
    status: String,
    /// Exact statuses, comma-separated (picklist values never contain commas).
    #[serde(default)]
    statuses: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    view: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    category: String,
    /// Main owner's user id; overrides "Của tôi".
    #[serde(default)]
    owner: String,
    #[serde(default)]
    page: u64,
}

impl SearchQ {
    fn filters(&self) -> Result<Filters, ApiErr> {
        let opt = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        let id = |s: &str, what: &str| match opt(s) {
            Some(v) if !is_id(&v) => Err(err(StatusCode::BAD_REQUEST, format!("{what} không hợp lệ"))),
            v => Ok(v),
        };
        let pick = |s: &str, what: &str| match s.split(',').find(|v| !v.trim().is_empty() && !safe_value(v)) {
            Some(_) => Err(err(StatusCode::BAD_REQUEST, format!("{what} không hợp lệ"))),
            None => Ok(()),
        };
        pick(&self.statuses, "Tình trạng")?;
        pick(&self.priority, "Mức ưu tiên")?;
        pick(&self.category, "Phân loại")?;
        Ok(Filters {
            q: self.q.clone(),
            owner: id(&self.owner, "Người phụ trách")?,
            status: self.status.clone(),
            statuses: self.statuses.split(',').filter(|s| !s.trim().is_empty()).map(String::from).collect(),
            project_id: id(&self.project, "Dự án")?,
            project_name: None,
            view: id(&self.view, "Bộ lọc")?,
            priority: opt(&self.priority),
            category: opt(&self.category),
        })
    }
}

#[derive(Deserialize)]
struct MetaQ {
    #[serde(default)]
    kind: String,
}

async fn api_meta(Query(q): Query<MetaQ>) -> ApiRes<Meta> {
    let kind = kind_of(&q.kind)?;
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || meta(&c, kind)).await
}

#[derive(Deserialize)]
struct FindQ {
    #[serde(default)]
    q: String,
}

/// Any PMS project by name (closed ones too): the cached filter list holds only 100.
async fn api_projects(Query(q): Query<FindQ>) -> ApiRes<Value> {
    let q = find_query(&q.q)?;
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || {
        let p = json!({ "filters": [{ "name": "projectname", "value": q, "operator": "c" }],
                        "paging": { "offset": 0, "max_rows": FIND_ROWS, "order_by": "modifiedtime", "sort_order": "DESC" } });
        Ok(json!({ "items": parse_projects(&cloudwork(&c, "getListProject", p)?) }))
    })
    .await
}

/// People by name. `getListUser` cannot search, so this filters the complete list.
async fn api_users(Query(q): Query<FindQ>) -> ApiRes<Value> {
    let q = find_query(&q.q)?;
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || {
        let users = meta(&c, "ticket")?.users;
        Ok(json!({ "items": match_opts(&users, &q, FIND_ROWS as usize) }))
    })
    .await
}

fn kind_of(k: &str) -> Result<&'static str, ApiErr> {
    match k {
        "task" | "" => Ok("task"),
        "ticket" => Ok("ticket"),
        _ => Err(err(StatusCode::BAD_REQUEST, "kind phải là task hoặc ticket")),
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, ApiErr> + Send + 'static) -> ApiRes<T> {
    tokio::task::spawn_blocking(f).await.map_err(|_| no_answer())?.map(Json)
}

async fn api_search(Query(q): Query<SearchQ>) -> ApiRes<Value> {
    let kind = kind_of(&q.kind)?;
    let mut f = q.filters()?;
    let c = creds().ok_or_else(not_configured)?;
    let mine = q.mine.as_deref() != Some("0");
    let page = q.page.min(500);
    blocking(move || {
        if f.owner.is_none() && mine {
            f.owner = Some(session(&c, false)?.uid);
        }
        // Tickets match their project by name: resolve the id from our own list,
        // never from the browser.
        if kind == "ticket" {
            if let Some(id) = &f.project_id {
                let cached = meta(&c, kind)?.projects.into_iter().find(|p| &p.value == id).map(|p| p.label);
                // Found through the project search (not in the cached 100): ask PMS.
                let name = match cached {
                    Some(n) => Some(n),
                    None => cloudwork(&c, "retrieveProject", json!({ "id": id.parse::<u64>().unwrap_or(0) }))
                        .ok()
                        .map(|d| field(d.get("data").unwrap_or(&d), "projectname"))
                        .filter(|n| !n.is_empty()),
                };
                f.project_name = Some(name.ok_or_else(|| err(StatusCode::BAD_REQUEST, "Không tìm thấy dự án trên PMS"))?);
            }
        }
        let action = if kind == "task" { "getListProjectTask" } else { "getListTicket" };
        let data = cloudwork(&c, action, search_params(kind, &f, page))?;
        let (items, has_more) = parse_list(kind, &c.base, &data);
        Ok(json!({ "items": items, "has_more": has_more }))
    })
    .await
}

#[derive(Deserialize)]
struct ItemQ {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    id: String,
}

async fn api_item(Query(q): Query<ItemQ>) -> ApiRes<Detail> {
    let kind = kind_of(&q.kind)?;
    if !is_id(&q.id) {
        return Err(err(StatusCode::BAD_REQUEST, "id không hợp lệ"));
    }
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || {
        let id: u64 = q.id.parse().map_err(|_| err(StatusCode::BAD_REQUEST, "id không hợp lệ"))?;
        let action = if kind == "task" { "retrieveProjectTask" } else { "retrieveTicket" };
        let data = cloudwork(&c, action, json!({ "id": id }))?;
        let mut d = parse_detail(kind, &c.base, &data);
        // Comments are extra context: a failure there must not lose the item.
        if let Ok(cm) = cloudwork(&c, "getRelatedListCommentById", json!({ "id": id, "paging": { "offset": 0, "max_rows": COMMENTS } })) {
            d.comments = parse_comments(&cm);
        }
        Ok(d)
    })
    .await
}

#[derive(Deserialize)]
struct ReportQ {
    #[serde(default)]
    id: String,
    /// 1-based.
    #[serde(default)]
    page: u64,
}

/// One page (≤500 rows) of a PMS report. Read-only, like everything here.
async fn api_report(Query(q): Query<ReportQ>) -> ApiRes<Report> {
    if !is_id(&q.id) {
        return Err(err(StatusCode::BAD_REQUEST, "ID report không hợp lệ"));
    }
    let c = creds().ok_or_else(not_configured)?;
    if c.key.is_empty() {
        return Err(no_key());
    }
    let page = q.page.clamp(1, 1000);
    blocking(move || {
        let raw = openapi_get(&c, &format!("getReportData?report={}&page={page}", q.id))?;
        parse_report(&c.base, &q.id, &raw)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://pms.example";

    fn fixture(name: &str) -> Value {
        let raw = match name {
            "tasks" => include_str!("pms_fixtures/tasks.json"),
            "tickets" => include_str!("pms_fixtures/tickets.json"),
            "task_detail" => include_str!("pms_fixtures/task_detail.json"),
            "ticket_detail" => include_str!("pms_fixtures/ticket_detail.json"),
            _ => include_str!("pms_fixtures/comments.json"),
        };
        serde_json::from_str::<Value>(raw).unwrap()["data"].clone()
    }

    #[test]
    fn parses_task_and_ticket_lists() {
        let (tasks, more) = parse_list("task", BASE, &fixture("tasks"));
        assert_eq!(tasks.len(), 2);
        assert!(more, "next_offset 2 < total 40");
        let t = &tasks[0];
        assert_eq!(t.id, "1001");
        assert_eq!(t.no, "", "task lists carry no number");
        assert_eq!(t.title, "Task mẫu 1");
        assert_eq!((t.status.as_str(), t.priority.as_str(), t.project.as_str()), ("Open", "normal", "Dự án mẫu 1"));
        assert_eq!(t.assignee, "Nhân viên A");
        assert_eq!(t.url, "https://pms.example/index.php?module=ProjectTask&view=Detail&record=1001");
        assert_eq!(tasks[1].due, "2026-01-20", "end_date_plan wins over enddate");
        assert_eq!(tasks[1].assignee, "Nhân viên A", "main owner, not the first owner");
        assert_eq!((t.category.as_str(), t.progress.as_str(), t.start.as_str(), t.hours.as_str()), ("Coding", "0%", "2026-01-05", ""));

        let (tickets, more) = parse_list("ticket", BASE, &fixture("tickets"));
        assert!(!more, "no next_offset");
        let k = &tickets[0];
        assert_eq!((k.no.as_str(), k.status.as_str(), k.priority.as_str()), ("#90001", "Wait For Verifying", "High"));
        assert_eq!(k.created, "2026-01-10 09:00:00");
        assert_eq!((k.category.as_str(), k.hours.as_str()), ("Bug", ""), "0.00 hours reads as none");
        assert_eq!(k.assignee, "Nhân viên C", "main owner, not the first owner");
        assert!(k.url.ends_with("module=HelpDesk&view=Detail&record=2001"));
    }

    #[test]
    fn parses_task_detail() {
        let d = parse_detail("task", BASE, &fixture("task_detail"));
        assert_eq!(d.item.no, "PT90001");
        assert_eq!(d.item.updated, "2026-01-05 10:15:00");
        assert_eq!(d.item.due, "2026-01-06");
        assert_eq!(d.description, "Cần thống kê chi phí mẫu & báo cáo.\n\n- Theo ngày\n- Theo loại");
        assert_eq!(d.checklist, vec![Check { name: "Bước mẫu 1".into(), done: true }, Check { name: "Bước mẫu 2".into(), done: false }]);
        assert_eq!(d.subtasks, vec![Sub { title: "Việc con mẫu".into(), status: "Open".into() }]);
    }

    #[test]
    fn parses_ticket_detail_and_comments() {
        let d = parse_detail("ticket", BASE, &fixture("ticket_detail"));
        assert_eq!((d.item.no.as_str(), d.item.project.as_str(), d.item.due.as_str()), ("#90003", "Dự án mẫu", "2026-01-30"));
        assert!(d.description.starts_with("Khách hàng A báo lỗi mẫu"), "{}", d.description);
        assert!(d.description.ends_with("Các bước tái hiện:\n1. Mở màn hình mẫu\n2. Gửi tệp"), "{}", d.description);
        let c = parse_comments(&fixture("comments"));
        assert_eq!(c, vec![Comment { author: "Nhân viên E".into(), text: "Chào @Nhân viên F, kiểm tra lại dữ liệu mẫu & phản hồi nhé".into(), at: "2026-01-09 13:40:00".into() }]);
    }

    #[test]
    fn login_and_failure_envelopes() {
        let ok = json!({ "status": 200, "data": { "token": "abc", "user_info": { "id": "11", "first_name": "Tùng", "last_name": "Nguyễn" } } });
        assert_eq!(parse_login(&ok), Some(LoginInfo { token: "abc".into(), uid: "11".into(), name: "Nguyễn Tùng".into() }));
        assert_eq!(parse_login(&json!({ "data": { "token": "t" } })).map(|l| l.name), Some(String::new()));
        assert_eq!(parse_login(&json!({ "status": 401, "data": "" })), None);
        let e = check(json!({ "success": "0", "message": "Parent record not found!" })).unwrap_err();
        assert_eq!(e.0, StatusCode::NOT_FOUND);
        assert!(check(json!({ "success": "1", "entry_list": [] })).is_ok());
    }

    #[test]
    fn html_to_text_strips_markup() {
        let empty = "<html>\n<head>\n\t<title>x</title>\n</head>\n<body></body>\n</html>";
        assert_eq!(html_to_text(empty), "");
        assert_eq!(html_to_text("a<br/>b<p>c&nbsp;&lt;d&gt; &#39;e&#x27; &copy;</p>"), "a\nb\nc <d> 'e' &copy;");
        assert_eq!(html_to_text("plain\n\n\n\nline  "), "plain\n\nline");
        assert_eq!(html_to_text("x < y"), "x < y");
        assert_eq!(html_to_text("<style>p{}</style><ul><li>one</li><li>two</li></ul>"), "- one\n- two");
    }

    #[test]
    fn search_params_map_filters() {
        let f = |q: &str, owner: Option<&str>, status: &str| Filters { q: q.into(), owner: owner.map(String::from), status: status.into(), ..Filters::default() };
        let p = search_params("task", &f("", Some("11"), ""), 1);
        assert_eq!(p["filters"], json!([
            { "name": "main_owner_id", "value": "11", "operator": "e" },
            { "name": "projecttaskstatus", "value": "Completed,Canceled", "operator": "n" },
        ]));
        assert_eq!(p["paging"], json!({ "offset": 20, "max_rows": 20, "order_by": "modifiedtime", "sort_order": "DESC" }));
        assert!(p.get("cvid").is_none());
        let p = search_params("task", &f("pt12345", None, "all"), 0);
        assert_eq!(p["filters"], json!([{ "name": "projecttask_no", "value": "PT12345", "operator": "e" }]));
        let p = search_params("ticket", &f("#67890", None, "done"), 0);
        assert_eq!(p["filters"][0], json!({ "name": "ticketstatus", "value": "Closed", "operator": "e" }));
        assert_eq!(p["filters"][1], json!({ "name": "ticket_no", "value": "#67890", "operator": "e" }));
        let p = search_params("ticket", &f(" mẫu ", None, "open"), 0);
        assert_eq!(p["filters"][0], json!({ "name": "ticketstatus", "value": "Closed,Cancel,Reject", "operator": "n" }));
        assert_eq!(p["filters"][1], json!({ "name": "ticket_title", "value": "mẫu", "operator": "c" }));
    }

    #[test]
    fn search_params_extra_filters() {
        let ticket = Filters {
            statuses: vec!["Open".into(), "Testing".into()],
            status: "done".into(),
            project_id: Some("5001".into()),
            project_name: Some("Dự án mẫu".into()),
            view: Some("77".into()),
            priority: Some("High".into()),
            category: Some("Bug".into()),
            owner: Some("12".into()),
            ..Filters::default()
        };
        let p = search_params("ticket", &ticket, 0);
        assert_eq!(p["cvid"], json!("77"));
        assert_eq!(p["filters"], json!([
            { "name": "main_owner_id", "value": "12", "operator": "e" },
            { "name": "ticketstatus", "value": "Open,Testing", "operator": "e" },
            { "name": "related_project", "value": "Dự án mẫu", "operator": "e" },
            { "name": "ticketpriorities", "value": "High", "operator": "e" },
            { "name": "ticketcategories", "value": "Bug", "operator": "e" },
        ]), "statuses win over the preset; tickets match the project by name");
        let task = Filters { project_id: Some("5001".into()), priority: Some("high".into()), category: Some("Coding".into()), status: "all".into(), ..Filters::default() };
        let p = search_params("task", &task, 0);
        assert_eq!(p["filters"], json!([
            { "name": "projectid", "value": "5001", "operator": "e" },
            { "name": "projecttaskpriority", "value": "high", "operator": "e" },
            { "name": "projecttasktype", "value": "Coding", "operator": "e" },
        ]));
        // Unsafe ids / values never reach a filter.
        let bad = Filters { project_id: Some("1 OR 1=1".into()), view: Some("x".into()), owner: Some("1;".into()), statuses: vec!["a,b".into(), "x'y".into()], status: "all".into(), ..Filters::default() };
        let p = search_params("task", &bad, 0);
        assert_eq!(p["filters"], json!([]));
        assert!(p.get("cvid").is_none());
    }

    #[test]
    fn search_query_validation() {
        let q = |s: &str| serde_json::from_value::<SearchQ>(serde_json::from_str(s).unwrap()).unwrap();
        let f = q(r#"{"kind":"ticket","statuses":"Open,Testing","project":"5001","view":"77","priority":"High","owner":"12"}"#).filters().unwrap();
        assert_eq!((f.statuses.len(), f.project_id.as_deref(), f.view.as_deref(), f.owner.as_deref()), (2, Some("5001"), Some("77"), Some("12")));
        assert!(q(r#"{"project":"5001 OR 1"}"#).filters().is_err());
        assert!(q(r#"{"view":"abc"}"#).filters().is_err());
        assert!(q(r#"{"statuses":"Open,x'y"}"#).filters().is_err());
        assert!(q(r#"{"category":"a\"b"}"#).filters().is_err());
    }

    #[test]
    fn parses_filter_meta() {
        let views = parse_views(&json!({
            "Mine": { "group_label": "Danh sách của tôi", "21": { "id": "21", "display_name": "Ticket mẫu của tôi" } },
            "Shared": { "group_label": "Danh sách chia sẻ", "13": { "id": "13", "display_name": "Tất cả" }, "9": { "id": "9", "display_name": "Bộ lọc mẫu" } },
        }));
        let names: Vec<(&str, &str, &str)> = views.iter().map(|o| (o.value.as_str(), o.label.as_str(), o.group.as_str())).collect();
        assert_eq!(names, vec![("21", "Ticket mẫu của tôi", "Danh sách của tôi"), ("9", "Bộ lọc mẫu", "Danh sách chia sẻ"), ("13", "Tất cả", "Danh sách chia sẻ")]);
        assert!(parse_views(&json!({ "Mine": { "group_label": "Danh sách của tôi" } })).is_empty(), "group with no lists");

        let st = parse_picklist(&json!({ "options": [
            { "value": "Open", "label": "Mới", "color": "#aaaaaa", "assigned": "1" },
            { "value": "Canceled ", "label": "Đã hủy", "color": "", "assigned": "1" },
            { "value": "Hidden", "label": "Ẩn", "assigned": "0" },
            { "value": "a,b", "label": "Lỗi", "assigned": "1" },
        ] }));
        assert_eq!(st.iter().map(|o| (o.value.as_str(), o.label.as_str())).collect::<Vec<_>>(), vec![("Open", "Mới"), ("Canceled ", "Đã hủy")], "raw value kept, unassigned and unsafe dropped");
        assert_eq!(st[0].color, "#aaaaaa");

        let pr = parse_projects(&json!({ "entry_list": [
            { "projectid": "5001", "id": "5001", "projectname": "Dự án mẫu 1", "projectstatus": "3- In Progress" },
            { "projectid": "", "projectname": "Không id" },
        ] }));
        assert_eq!(pr, vec![Opt { value: "5001".into(), label: "Dự án mẫu 1".into(), group: "3- In Progress".into(), color: String::new() }]);

        let us = parse_users(&json!({ "entry_list": {
            "12": { "id": "12", "full_name": "Nhân viên B" },
            "11": { "id": "11", "full_name": "", "first_name": "A", "last_name": "Nhân viên" },
        } }));
        assert_eq!(us.iter().map(|o| (o.value.as_str(), o.label.as_str())).collect::<Vec<_>>(), vec![("11", "Nhân viên A"), ("12", "Nhân viên B")]);
    }

    #[test]
    fn find_query_validation() {
        assert_eq!(find_query("  Phường số ").unwrap(), "Phường số");
        for bad in ["", "   ", "a'b", "x\"y", "a;b", "a,b", "100%", "a_b", "a\\b", &"x".repeat(61)] {
            assert!(find_query(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn diacritic_insensitive_match() {
        assert_eq!(fold_vi("Phường Số ĐẠI"), "phuong so dai");
        assert_eq!(fold_vi("Nguyễn Thị Ý"), "nguyen thi y");
        let o = |v: &str, l: &str| Opt { value: v.into(), label: l.into(), group: String::new(), color: String::new() };
        let list = vec![o("1", "Dự án Phường mẫu"), o("2", "Nhân viên Đức"), o("3", "Khác")];
        assert_eq!(match_opts(&list, "phuong", 10), vec![o("1", "Dự án Phường mẫu")]);
        assert_eq!(match_opts(&list, "DUC", 10), vec![o("2", "Nhân viên Đức")]);
        assert_eq!(match_opts(&list, "a", 1).len(), 1, "capped");
    }

    #[test]
    fn source_validation() {
        let s = |kind: &str, id: &str, url: &str| Source { system: "pms".into(), kind: kind.into(), id: id.into(), no: "PT1".into(), url: url.into() };
        assert!(s("task", "123", "https://pms.example/index.php").validate().is_ok());
        assert!(s("task", "12a", "").validate().is_err());
        assert!(s("note", "1", "").validate().is_err());
        assert!(s("record", "1568427", "https://pms.example/index.php?module=CPTimeSheets&view=Detail&record=1568427").validate().is_ok());
        assert!(s("ticket", "1", "javascript:alert(1)").validate().is_err());
    }

    const OWNER_HTML: &str = "\n<span class=\"owners\"><span class=\"stand-owner\"><a href=\"index.php?module=Users&record=5768\">Huỳnh Quang Thịnh (thinh.huynh@cloudgo.vn)</a></span></span>";

    #[test]
    fn parses_report_rows_in_column_order() {
        let empty_html = "<html>\n<head>\n\t<title></title>\n</head>\n<body></body>\n</html>";
        // Keys deliberately out of alphabetical order: the report's column order must survive.
        let raw = format!(
            r#"{{"success":1,"data":{{"report_id":1047,"report_name":" Việc trong tuần ","report_type":"tabular","primary_module":"CPTimeSheets","page":1,"has_next_page":true,"rows":[
              {{"record_id":1568427,"fields":{{
                "CPTimeSheets_CPTimeSheets_Project":{{"value":"Khải Đông 2025","type":"reference"}},
                "CPTimeSheets_description":{{"value":{empty},"type":"text"}},
                "CPTimeSheets_apply_date":{{"value":"19-10-2026","type":"date"}},
                "CPTimeSheets_cptimesheets_task":{{"value":"Lập trình","type":"picklist"}},
                "CPTimeSheets_total_hours":{{"value":"1.5","type":"double"}},
                "CPTimeSheets_CPTimeSheets_LBL_ASSIGNED_TO":{{"value":{owner},"type":"owner"}}}}}},
              {{"record_id":"1568460","fields":{{
                "CPTimeSheets_CPTimeSheets_Project":{{"value":"[OCS] Tân Đồng Phúc","type":"reference"}},
                "CPTimeSheets_description":{{"value":"Cải tiến &amp; auto fill\nngười liên hệ","type":"text"}}}}}},
              {{"record_id":"x","fields":[]}},
              {{"record_id":7,"fields":[]}}]}}}}"#,
            empty = serde_json::to_string(empty_html).unwrap(),
            owner = serde_json::to_string(OWNER_HTML).unwrap(),
        );
        let r = parse_report(BASE, "1047", raw.as_bytes()).unwrap();
        assert_eq!((r.name.as_str(), r.module.as_str(), r.kind, r.chart, r.has_more), ("Việc trong tuần", "CPTimeSheets", None, false, true));
        assert_eq!(r.url, "https://pms.example/index.php?module=Reports&view=Detail&record=1047");
        assert_eq!(r.rows.len(), 3, "a non-numeric record id is dropped; `fields: []` is an empty row");
        let a = &r.rows[0];
        let cells: Vec<(&str, &str)> = a.cells.iter().map(|c| (c.label.as_str(), c.value.as_str())).collect();
        assert_eq!(
            cells,
            vec![("Dự án", "Khải Đông 2025"), ("Ngày", "19-10-2026"), ("Task", "Lập trình"), ("Số giờ", "1.5"), ("Phụ trách", "Huỳnh Quang Thịnh")],
            "report order kept, empty HTML description dropped, owner without email"
        );
        assert_eq!(a.title, "Khải Đông 2025", "no title/name column and no description: first value");
        assert_eq!(a.url, "https://pms.example/index.php?module=CPTimeSheets&view=Detail&record=1568427");
        assert_eq!(r.rows[1].title, "Cải tiến & auto fill người liên hệ", "description, on one line");
        assert_eq!(r.rows[2].title, "#7");
    }

    #[test]
    fn report_of_tasks_and_tickets_opens_as_items() {
        let raw = r#"{"success":1,"data":{"report_name":"Tác vụ","report_type":"tabular","primary_module":"ProjectTask","has_next_page":false,"rows":[
            {"record_id":215076,"fields":{"ProjectTask_projecttask_no":{"value":"PT544","type":"string"},"ProjectTask_projecttaskname":{"value":"Tích hợp MIFI","type":"string"}}}]}}"#;
        let r = parse_report(BASE, "1189", raw.as_bytes()).unwrap();
        assert_eq!(r.kind, Some("task"));
        assert_eq!((r.rows[0].id.as_str(), r.rows[0].title.as_str()), ("215076", "Tích hợp MIFI"));
        assert_eq!(r.rows[0].cells[0], Cell { label: "Mã task".into(), value: "PT544".into() });
        let t = br#"{"success":1,"data":{"report_type":"chart","primary_module":"HelpDesk","rows":[]}}"#;
        let r = parse_report(BASE, "1029", t).unwrap();
        assert_eq!((r.kind, r.chart, r.rows.len()), (Some("ticket"), true, 0));
        // A module name is only ever letters/digits before it goes into a link.
        let bad = br#"{"success":1,"data":{"primary_module":"Help\"Desk","rows":[{"record_id":1,"fields":{"x_title":{"value":"a"}}}]}}"#;
        let r = parse_report(BASE, "1", bad).unwrap();
        assert_eq!((r.module.as_str(), r.rows[0].url.as_str()), ("", ""));
    }

    #[test]
    fn report_errors() {
        let e = parse_report(BASE, "99", br#"{"success":0,"message":"Report not found!"}"#).unwrap_err();
        assert_eq!(e.0, StatusCode::NOT_FOUND);
        assert!(e.1.contains("99"));
        assert_eq!(parse_report(BASE, "1", b"Sorry! Attempt to access restricted file.").unwrap_err().0, StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn curl_config_quotes() {
        assert_eq!(cfg_quote("Token: a\"b\\c\n"), "\"Token: a\\\"b\\\\c\"");
    }

    /// A throwaway HOME under the temp dir.
    fn temp_home(tag: &str) -> PathBuf {
        let h = std::env::temp_dir().join(format!("ckit-pms-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&h);
        std::fs::create_dir_all(&h).unwrap();
        h
    }

    const SECRET: &str = "s3cr3t-Pa55";

    fn sample() -> Config {
        Config { base_url: "https://pms.example".into(), username: "boss".into(), password: SECRET.into(), display_name: "Nguyễn Tùng".into(), access_key: String::new() }
    }

    #[test]
    fn config_roundtrip_is_0600() {
        let h = temp_home("rw");
        assert!(load_config(&h).is_none());
        save_config(&h, &sample()).unwrap();
        // Overwrite keeps 0600 too.
        save_config(&h, &sample()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(config_path(&h)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let c = creds_in(&h).unwrap();
        assert_eq!((c.base.as_str(), c.user.as_str(), c.pass.as_str()), ("https://pms.example", "boss", SECRET));
        assert!(!config_path(&h).with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn status_never_leaks_password_or_key() {
        let h = temp_home("view");
        save_config(&h, &sample()).unwrap();
        let v = view_in(&h);
        assert_eq!(v, json!({ "configured": true, "base_url": "https://pms.example", "user": "boss", "display_name": "Nguyễn Tùng", "report": false }));
        save_config(&h, &Config { access_key: "AK-9z".into(), ..sample() }).unwrap();
        let v = view_in(&h);
        assert_eq!(v["report"], json!(true));
        assert!(!v.to_string().contains(SECRET) && !v.to_string().contains("AK-9z"));
        assert_eq!(creds_in(&h).unwrap().key, "AK-9z");
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn off_removes_file_and_cached_token() {
        let h = temp_home("off");
        save_config(&h, &sample()).unwrap();
        *session_cache().lock().unwrap() = Some(Session { key: 1, token: "tok".into(), uid: "11".into(), name: String::new() });
        let v = disconnect_in(&h);
        assert_eq!(v["configured"], json!(false));
        assert!(!config_path(&h).exists());
        assert!(session_cache().lock().unwrap().is_none());
        assert!(creds_in(&h).is_none());
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn mcp_json_is_never_an_implicit_source() {
        let h = temp_home("mcp");
        std::fs::create_dir_all(h.join(".omp/agent")).unwrap();
        let mcp = json!({ "mcpServers": { "pms": { "env": { "PMS_BASE_URL": "https://pms.example/", "PMS_USERNAME": "boss", "PMS_PASSWORD": SECRET } } } });
        std::fs::write(h.join(".omp/agent/mcp.json"), mcp.to_string()).unwrap();
        assert!(creds_in(&h).is_none(), "mcp.json must not configure PMS");
        assert_eq!(view_in(&h)["configured"], json!(false));
        // Only the explicit import reads it.
        let c = mcp_creds(&h).unwrap();
        assert_eq!((c.base.as_str(), c.user.as_str()), ("https://pms.example", "boss"));
        let _ = std::fs::remove_dir_all(&h);
    }
}
