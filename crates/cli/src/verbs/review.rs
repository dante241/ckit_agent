//! `8sync review` — automated PR/MR code review, the way Claude Code reviews
//! GitHub PRs: fetch the PR/MR, let omp review its diff read-only, then post one
//! summary comment (edited in place on every new push) plus inline comments for
//! blocking findings. Runs in CI (GitHub Actions / GitLab CI templates via
//! `8sync review init`) or locally. GitHub through `gh`, GitLab through `glab`.
//!
//! Safety: the diff and PR text are untrusted. omp runs in a dedicated profile
//! (`~/.omp/profiles/ckit-review`: no MCP servers, no xd:// devices, no memory,
//! no auto-learn) with only `read,grep,glob`, inside a detached worktree. The
//! review only ever comments: it never approves, blocks a merge, or pushes.

use crate::{assets, ui};
use anyhow::{anyhow, bail, Context, Result};
use clap::Args as ClapArgs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// First line of the summary comment; carries the reviewed head commit.
const SUMMARY_MARK: &str = "<!-- ckit-review head=";
/// First line of every inline comment; carries the finding's key (dedup).
const FINDING_MARK: &str = "<!-- ckit-review-finding ";
const SKIP_LABEL: &str = "no-ai-review";
const PROFILE: &str = "ckit-review";
const OMP_TIMEOUT: Duration = Duration::from_secs(25 * 60);
const MAX_FINDINGS: usize = 15;
/// GitHub rejects comment bodies over 65536 chars.
const MAX_BODY: usize = 60_000;

#[derive(ClapArgs, Debug)]
#[command(after_help = indoc::indoc! {"
    EXAMPLES
      8sync review 42                 review PR/MR 42, print the result (posts nothing)
      8sync review 42 --post          review and post: summary comment + inline comments
      8sync review --post             in CI: the PR/MR of the current job
      8sync review 42 --model opus    pick the omp model
      8sync review init               write the CI workflow (GitHub Actions or GitLab CI)

    HOW IT WORKS
      · GitHub via `gh`, GitLab via `glab` (host from CI env, else from `origin`).
      · One summary comment, edited in place on every push; inline comments only
        for blocking findings on added lines, never posted twice.
      · Skips drafts, the `no-ai-review` label and an already-reviewed head commit
        (--force reviews anyway).
      · omp runs read-only (read/grep/glob) in profile `ckit-review` with no MCP
        servers; models come from ~/.omp/agent/models.yml or provider env keys.
"})]
pub struct Args {
    /// PR/MR number, or `init` to write the CI workflow. Default (in CI): the job's PR/MR.
    pub target: Option<String>,
    /// Post to GitHub/GitLab (default: print only).
    #[arg(long)]
    pub post: bool,
    /// Review drafts, `no-ai-review` and already-reviewed commits too; `init`: overwrite.
    #[arg(long)]
    pub force: bool,
    /// omp model (default: omp's default role).
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Host {
    Github,
    Gitlab,
}

struct Pr {
    number: u64,
    title: String,
    body: String,
    draft: bool,
    labels: Vec<String>,
    head_sha: String,
    /// GitLab `diff_refs` (inline-comment positions); GitHub: unused.
    base_sha: String,
    start_sha: String,
    target: String,
    url: String,
}

#[derive(Deserialize, Default)]
struct Review {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    findings: Vec<Finding>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Finding {
    path: String,
    #[serde(default)]
    line: u64,
    #[serde(default)]
    severity: String,
    title: String,
    #[serde(default)]
    body: String,
    /// Set by ckit, not the model: blocking and on a line the diff adds.
    #[serde(default)]
    inline: bool,
    /// Path before a rename (GitLab inline positions).
    #[serde(default)]
    old_path: String,
}

impl Finding {
    fn blocking(&self) -> bool {
        self.severity.eq_ignore_ascii_case("blocking")
    }
    /// Stable across pushes (no line number): the same issue is never re-posted.
    fn key(&self) -> String {
        format!("{:016x}", fnv(&format!("{}\0{}", self.path, self.title.trim().to_lowercase())))
    }
}

/// The last review of a PR/MR, kept in `<repo>/.cache/ckit/reviews/<n>.json`
/// (CLI and Board share it): reviewed commit, result, and whether it was posted.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Stored {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) head_sha: String,
    /// Unix seconds.
    pub(crate) at: u64,
    pub(crate) model: Option<String>,
    pub(crate) summary: String,
    pub(crate) findings: Vec<Finding>,
    #[serde(default)]
    pub(crate) posted_at: Option<u64>,
}

struct Note {
    id: u64,
    body: String,
    /// GitHub login / GitLab user id: markers count only from our own identity.
    author: String,
}

/// Lines a diff adds, per new path, plus the old path (renames) for GitLab positions.
#[derive(Default, Debug)]
struct Added {
    old_path: String,
    lines: HashSet<u64>,
}

pub fn run(a: Args) -> Result<()> {
    let root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"]).context("not inside a git repository")?);
    let host = detect_host(&root);
    if a.target.as_deref() == Some("init") {
        return init(&root, host, a.force);
    }
    let number = match a.target.as_deref() {
        Some(t) => t.trim_start_matches(['#', '!']).parse().map_err(|_| anyhow!("`{t}` is not a PR/MR number"))?,
        None => ci_number(host)?,
    };
    let pr = fetch_pr(&root, host, number)?;
    ui::header(&format!("8sync review — {} {}", pr_label(host, number), pr.title));
    if !a.force && pr.draft {
        ui::skip("review", "draft");
        return Ok(());
    }
    if !a.force && pr.labels.iter().any(|l| l == SKIP_LABEL) {
        ui::skip("review", &format!("label `{SKIP_LABEL}`"));
        return Ok(());
    }
    let notes = if a.post { list_notes(&root, host, number)? } else { Vec::new() };
    if !a.force && notes.iter().any(|n| reviewed_head(&n.body) == Some(pr.head_sha.as_str())) {
        ui::skip("review", &format!("{} already reviewed", short(&pr.head_sha)));
        return Ok(());
    }
    let Some(mut s) = review_pr(&root, host, &pr, a.model.as_deref())? else {
        ui::skip("review", "empty diff");
        return Ok(());
    };
    println!("\n{}\n", render_summary(&s));
    if !a.post {
        for f in s.findings.iter().filter(|f| f.inline) {
            ui::info(&format!("inline {}:{} — {}", f.path, f.line, f.title));
        }
        ui::info("dry run — add --post to comment on the PR/MR");
        return Ok(());
    }
    publish(&root, host, &pr, &mut s)
}

/// Board: review PR/MR `n` now (no skip rules) and store the result.
pub(crate) fn review(root: &Path, n: u64, model: Option<&str>) -> Result<Stored> {
    let host = detect_host(root);
    let pr = fetch_pr(root, host, n)?;
    review_pr(root, host, &pr, model)?.ok_or_else(|| anyhow!("{} không có thay đổi nào để review", pr_label(host, n)))
}

/// Board: the PR/MR's current head commit and target branch.
pub(crate) fn head_and_target(root: &Path, n: u64) -> Result<(String, String)> {
    let pr = fetch_pr(root, detect_host(root), n)?;
    Ok((pr.head_sha, pr.target))
}

/// Board: post the stored review of `n`. Refuses when the PR/MR has commits the
/// review has not seen.
pub(crate) fn post(root: &Path, n: u64) -> Result<Stored> {
    let host = detect_host(root);
    let mut s = load(root, n).ok_or_else(|| anyhow!("{} chưa được review", pr_label(host, n)))?;
    let pr = fetch_pr(root, host, n)?;
    if s.head_sha != pr.head_sha {
        bail!("{} có commit mới ({}) sau lần review ({}): review lại trước khi đăng", pr_label(host, n), short(&pr.head_sha), short(&s.head_sha));
    }
    publish(root, host, &pr, &mut s)?;
    Ok(s)
}

/// Open PRs/MRs of the repo, newest created first (≤50), with their labels
/// (`{name, color}`, color as `#rrggbb` or empty).
pub(crate) fn list_open(root: &Path) -> Result<(&'static str, Vec<Value>)> {
    let host = detect_host(root);
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let (path, name) = match host {
        Host::Github => ("repos/{owner}/{repo}/pulls?state=open&per_page=50&sort=created&direction=desc", "github"),
        Host::Gitlab => ("projects/:id/merge_requests?state=opened&per_page=50&order_by=created_at&sort=desc&with_labels_details=true", "gitlab"),
    };
    let raw: Value = serde_json::from_str(&api(root, host, "GET", path, None)?)?;
    let labels = |v: &Value| -> Vec<Value> {
        v["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| match l {
                // GitLab without details: plain names.
                Value::String(n) => json!({ "name": n, "color": "" }),
                l => {
                    let c = s(&l["color"]);
                    let color = if c.is_empty() || c.starts_with('#') { c } else { format!("#{c}") };
                    json!({ "name": s(&l["name"]), "color": color })
                }
            })
            .collect()
    };
    let rows = raw
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| match host {
            Host::Github => json!({
                "number": v["number"], "title": s(&v["title"]), "author": s(&v["user"]["login"]), "url": s(&v["html_url"]),
                "draft": v["draft"].as_bool().unwrap_or(false), "head_sha": s(&v["head"]["sha"]), "labels": labels(v),
                "source": s(&v["head"]["ref"]), "target": s(&v["base"]["ref"]), "created": s(&v["created_at"]), "updated": s(&v["updated_at"]),
            }),
            Host::Gitlab => json!({
                "number": v["iid"], "title": s(&v["title"]), "author": s(&v["author"]["name"]), "url": s(&v["web_url"]),
                "draft": v["draft"].as_bool().unwrap_or(false), "head_sha": s(&v["sha"]), "labels": labels(v),
                "source": s(&v["source_branch"]), "target": s(&v["target_branch"]), "created": s(&v["created_at"]), "updated": s(&v["updated_at"]),
            }),
        })
        .collect();
    Ok((name, rows))
}

/// What the MR's comments say about reviews: the commit ckit's latest summary
/// covers, and whether someone reviewed it another way (a comment headed
/// "Code Review", e.g. a team review skill) — with the commit it cites, if any.
#[derive(Default)]
pub(crate) struct Seen {
    pub(crate) ckit_head: Option<String>,
    pub(crate) ckit_by: Option<String>,
    pub(crate) other: Option<OtherReview>,
}

#[derive(Clone, Serialize)]
pub(crate) struct OtherReview {
    pub(crate) author: String,
    pub(crate) at: String,
    /// Short or full SHA quoted in the review; None when it names no commit.
    pub(crate) sha: Option<String>,
}

/// A review someone wrote by hand or with another tool: first non-empty line
/// is a heading or bold line containing "code review".
fn other_review(body: &str) -> Option<Option<String>> {
    let first = body.lines().map(str::trim).find(|l| !l.is_empty())?;
    let head = first.trim_start_matches(['#', '*', ' ']).to_lowercase();
    if !(first.starts_with('#') || first.starts_with("**")) || !head.contains("code review") {
        return None;
    }
    // The commit it covers: the first 7-40 hex run in backticks or after "commit".
    let sha = body
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|w| (7..=40).contains(&w.len()) && w.chars().all(|c| c.is_ascii_hexdigit()) && w.chars().any(|c| c.is_ascii_digit()) && w.chars().any(|c| c.is_ascii_alphabetic()))
        .map(str::to_lowercase);
    Some(sha)
}

/// Every human/bot comment on PR/MR `n`, oldest first (system notes skipped),
/// plus what they say about reviews (`Seen`). ckit summaries count only from
/// the identities ckit posts as: this machine's gh/glab account, GitLab project
/// bots (the CI token's user) and `github-actions[bot]`.
pub(crate) fn comments(root: &Path, n: u64) -> Result<(Vec<Value>, Seen)> {
    let host = detect_host(root);
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let me = whoami(root, host)?;
    let mut out: Vec<Value> = Vec::new();
    let mut seen = Seen::default();
    let mut push = |id: &Value, author: String, trusted: bool, body: String, at: String, path: String, line: &Value, resolved: &Value| {
        // Oldest first: a later summary of ours (normally the same, edited in place) wins.
        if let Some(h) = reviewed_head(&body).filter(|_| trusted) {
            seen.ckit_head = Some(h.to_string());
            seen.ckit_by = Some(author.clone());
        }
        let ckit = trusted && (body.starts_with(SUMMARY_MARK) || body.starts_with(FINDING_MARK));
        if !ckit && path.is_empty() {
            if let Some(sha) = other_review(&body) {
                seen.other = Some(OtherReview { author: author.clone(), at: at.clone(), sha });
            }
        }
        // Our marker line is for machines; the reader gets the text.
        let body = if ckit { body.split_once("-->\n").map_or(body.clone(), |(_, b)| b.to_string()) } else { body };
        out.push(json!({ "id": id, "author": author, "body": body, "at": at, "path": path, "line": line, "resolved": resolved, "ckit": ckit }));
    };
    match host {
        Host::Github => {
            for (p, inline) in [(format!("repos/{{owner}}/{{repo}}/issues/{n}/comments"), false), (format!("repos/{{owner}}/{{repo}}/pulls/{n}/comments"), true)] {
                let raw = sh(root, "gh", &["api", "--paginate", &p, "--jq", ".[]"], None)?;
                for v in raw.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
                    let login = s(&v["user"]["login"]);
                    let trusted = login == me || login == "github-actions[bot]";
                    let line = if !inline { Value::Null } else if v["line"].is_null() { v["original_line"].clone() } else { v["line"].clone() };
                    push(&v["id"], login, trusted, s(&v["body"]), s(&v["created_at"]), if inline { s(&v["path"]) } else { String::new() }, &line, &Value::Null);
                }
            }
        }
        Host::Gitlab => {
            let p = format!("projects/:id/merge_requests/{n}/notes?per_page=100&sort=asc&order_by=created_at");
            let raw = sh(root, "glab", &["api", "--paginate", "--output", "ndjson", &p], None)?;
            for v in raw.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v["system"] != true) {
                let a = &v["author"];
                let id = a["id"].as_u64().map(|i| i.to_string()).unwrap_or_default();
                // A project access token's user (what the CI template posts with).
                let bot = s(&a["username"]).starts_with("project_") && s(&a["username"]).contains("_bot");
                let pos = &v["position"];
                let line = if pos["new_line"].is_null() { pos["old_line"].clone() } else { pos["new_line"].clone() };
                let path = if pos["new_path"].is_string() { s(&pos["new_path"]) } else { String::new() };
                let resolved = if v["resolvable"] == true { v["resolved"].clone() } else { Value::Null };
                push(&v["id"], s(&a["name"]), id == me || bot, s(&v["body"]), s(&v["created_at"]), path, &line, &resolved);
            }
        }
    }
    out.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    Ok((out, seen))
}

pub(crate) fn reviews_dir(root: &Path) -> PathBuf {
    root.join(".cache/ckit/reviews")
}

pub(crate) fn load(root: &Path, n: u64) -> Option<Stored> {
    serde_json::from_str(&std::fs::read_to_string(reviews_dir(root).join(format!("{n}.json"))).ok()?).ok()
}

/// Two reviews of one MR may finish together (Board + CLI/CI on this machine):
/// each writes its own temp file, the last rename wins, nobody reads half a file.
fn save(root: &Path, s: &Stored) -> Result<()> {
    let dir = reviews_dir(root);
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join(format!("{}.json", s.number)), &serde_json::to_vec_pretty(s)?)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Review the PR/MR head and store the result. None: the diff is empty.
fn review_pr(root: &Path, host: Host, pr: &Pr, model: Option<&str>) -> Result<Option<Stored>> {
    let ws = Workspace::open(root, host, pr)?;
    ui::info(&format!("{} file(s) changed, {} added line(s)", ws.added.len(), ws.added.values().map(|a| a.lines.len()).sum::<usize>()));
    if ws.diff.trim().is_empty() {
        return Ok(None);
    }
    ui::step("omp review (read-only)");
    let mut r = run_omp(&ws, pr, model)?;
    for f in &mut r.findings {
        f.inline = f.blocking() && on_added_line(&ws.added, f);
        f.old_path = ws.added.get(&f.path).map(|a| a.old_path.clone()).unwrap_or_else(|| f.path.clone());
    }
    let s = Stored { number: pr.number, title: pr.title.clone(), url: pr.url.clone(), head_sha: pr.head_sha.clone(), at: now(), model: model.map(String::from), summary: r.summary, findings: r.findings, posted_at: None };
    save(root, &s)?;
    Ok(Some(s))
}

/// Lock on `.cache/ckit/reviews/<n>.lock` while one process publishes MR `n`:
/// Board and CLI posting together would each see no summary yet and both create
/// one. Covers processes of this checkout only (CI runs in its own checkout).
struct PublishLock(PathBuf);

impl PublishLock {
    const WAIT: Duration = Duration::from_secs(20);
    /// A crashed publisher's lock; publishing takes seconds.
    const STALE: Duration = Duration::from_secs(10 * 60);

    fn take(root: &Path, n: u64) -> Result<Self> {
        let dir = reviews_dir(root);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{n}.lock"));
        let started = Instant::now();
        loop {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    let _ = write!(f, "{}", std::process::id());
                    return Ok(Self(path));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let age = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
                    if age.is_some_and(|a| a > Self::STALE) {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if started.elapsed() > Self::WAIT {
                        bail!("MR {n} đang được đăng bởi tiến trình khác ({}): thử lại sau", path.display());
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Drop for PublishLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Inline comments not posted before, then the summary (created or edited in place).
/// Reads the MR's comments under the lock, so a publish that just finished is seen.
fn publish(root: &Path, host: Host, pr: &Pr, s: &mut Stored) -> Result<()> {
    let _lock = PublishLock::take(root, pr.number)?;
    let notes = list_notes(root, host, pr.number)?;
    let summary_note = notes.iter().find(|n| n.body.starts_with(SUMMARY_MARK));
    let posted: HashSet<String> = notes.iter().filter_map(|n| finding_key(&n.body).map(String::from)).collect();
    let inline: Vec<&Finding> = s.findings.iter().filter(|f| f.inline && !posted.contains(&f.key())).collect();
    if !inline.is_empty() {
        match post_inline(root, host, pr, &inline) {
            Ok(n) => ui::ok(&format!("{n} inline comment(s) posted")),
            // Every finding is in the summary too: a refused inline comment loses nothing.
            Err(e) => ui::warn(&format!("inline comments not posted: {e}")),
        }
    }
    upsert_summary(root, host, pr.number, summary_note.map(|n| n.id), &render_summary(s))?;
    ui::ok(&format!("summary {} on {}", if summary_note.is_some() { "updated" } else { "posted" }, pr.url));
    s.posted_at = Some(now());
    save(root, s)
}

// ── host / CI detection ─────────────────────────────────────────────

fn detect_host(root: &Path) -> Host {
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        return Host::Github;
    }
    if std::env::var_os("GITLAB_CI").is_some() {
        return Host::Gitlab;
    }
    match git(root, &["remote", "get-url", "origin"]) {
        Ok(u) if u.contains("github.com") => Host::Github,
        _ => Host::Gitlab,
    }
}

fn ci_number(host: Host) -> Result<u64> {
    let n = match host {
        Host::Github => std::env::var("GITHUB_EVENT_PATH")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["pull_request"]["number"].as_u64()),
        Host::Gitlab => std::env::var("CI_MERGE_REQUEST_IID").ok().and_then(|s| s.parse().ok()),
    };
    n.ok_or_else(|| anyhow!("no PR/MR number: pass one (`8sync review 42`) or run in a pull_request / merge_request pipeline"))
}

fn pr_label(host: Host, n: u64) -> String {
    match host {
        Host::Github => format!("PR #{n}"),
        Host::Gitlab => format!("MR !{n}"),
    }
}

fn head_ref(host: Host, n: u64) -> String {
    match host {
        Host::Github => format!("refs/pull/{n}/head"),
        Host::Gitlab => format!("refs/merge-requests/{n}/head"),
    }
}

// ── GitHub / GitLab API (gh / glab) ─────────────────────────────────

fn fetch_pr(root: &Path, host: Host, n: u64) -> Result<Pr> {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    Ok(match host {
        Host::Github => {
            let v: Value = serde_json::from_str(&api(root, host, "GET", &format!("repos/{{owner}}/{{repo}}/pulls/{n}"), None)?)?;
            Pr {
                number: n,
                title: s(&v["title"]),
                body: s(&v["body"]),
                draft: v["draft"].as_bool().unwrap_or(false),
                labels: v["labels"].as_array().into_iter().flatten().map(|l| s(&l["name"])).collect(),
                head_sha: s(&v["head"]["sha"]),
                base_sha: s(&v["base"]["sha"]),
                start_sha: s(&v["base"]["sha"]),
                target: s(&v["base"]["ref"]),
                url: s(&v["html_url"]),
            }
        }
        Host::Gitlab => {
            let v: Value = serde_json::from_str(&api(root, host, "GET", &format!("projects/:id/merge_requests/{n}"), None)?)?;
            let refs = &v["diff_refs"];
            Pr {
                number: n,
                title: s(&v["title"]),
                body: s(&v["description"]),
                draft: v["draft"].as_bool().or(v["work_in_progress"].as_bool()).unwrap_or(false),
                labels: v["labels"].as_array().into_iter().flatten().map(s).collect(),
                head_sha: if refs["head_sha"].is_string() { s(&refs["head_sha"]) } else { s(&v["sha"]) },
                base_sha: s(&refs["base_sha"]),
                start_sha: s(&refs["start_sha"]),
                target: s(&v["target_branch"]),
                url: s(&v["web_url"]),
            }
        }
    })
    .and_then(|p: Pr| if p.head_sha.is_empty() || p.target.is_empty() { Err(anyhow!("{} has no head commit / target branch", pr_label(host, n))) } else { Ok(p) })
}

/// Every comment on the PR/MR (GitHub: conversation + review comments).
fn list_notes(root: &Path, host: Host, n: u64) -> Result<Vec<Note>> {
    let paths = match host {
        Host::Github => vec![format!("repos/{{owner}}/{{repo}}/issues/{n}/comments"), format!("repos/{{owner}}/{{repo}}/pulls/{n}/comments")],
        Host::Gitlab => vec![format!("projects/:id/merge_requests/{n}/notes?per_page=100")],
    };
    let mut out = Vec::new();
    for p in paths {
        let raw = match host {
            Host::Github => sh(root, "gh", &["api", "--paginate", &p, "--jq", ".[] | {id, body, author: .user.login}"], None)?,
            Host::Gitlab => sh(root, "glab", &["api", "--paginate", "--output", "ndjson", &p], None)?,
        };
        out.extend(parse_notes(&raw));
    }
    // Anyone can paste a marker; only our own comments may skip a review,
    // suppress a finding or be edited.
    let me = whoami(root, host)?;
    out.retain(|n| n.author == me);
    Ok(out)
}

/// The identity the post token acts as. GitHub Actions' GITHUB_TOKEN cannot read
/// `/user` (403); it always comments as `github-actions[bot]`.
fn whoami(root: &Path, host: Host) -> Result<String> {
    match host {
        Host::Github => Ok(sh(root, "gh", &["api", "user", "--jq", ".login"], None).map(|s| s.trim().to_string()).unwrap_or_else(|_| "github-actions[bot]".into())),
        Host::Gitlab => {
            let v: Value = serde_json::from_str(&sh(root, "glab", &["api", "user"], None)?)?;
            v["id"].as_u64().map(|id| id.to_string()).ok_or_else(|| anyhow!("glab api user: no id"))
        }
    }
}

fn parse_notes(ndjson: &str) -> Vec<Note> {
    ndjson
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            let author = match &v["author"] {
                Value::String(s) => s.clone(),
                a => a["id"].as_u64().map(|i| i.to_string()).unwrap_or_default(),
            };
            Some(Note { id: v["id"].as_u64()?, body: v["body"].as_str().unwrap_or_default().to_string(), author })
        })
        .collect()
}

fn api(root: &Path, host: Host, method: &str, path: &str, body: Option<&Value>) -> Result<String> {
    let bin = match host {
        Host::Github => "gh",
        Host::Gitlab => "glab",
    };
    let mut args = vec!["api", "-X", method, path];
    let input = body.map(|b| b.to_string());
    if input.is_some() {
        args.extend(["-H", "Content-Type: application/json", "--input", "-"]);
    }
    sh(root, bin, &args, input.as_deref())
}

/// One GitHub review (event COMMENT: never approves or blocks) / one GitLab
/// discussion per finding. Returns how many were posted.
fn post_inline(root: &Path, host: Host, pr: &Pr, fs: &[&Finding]) -> Result<usize> {
    match host {
        Host::Github => {
            let comments: Vec<Value> = fs.iter().map(|f| json!({"path": f.path, "line": f.line, "side": "RIGHT", "body": inline_body(f)})).collect();
            let body = json!({"commit_id": pr.head_sha, "event": "COMMENT", "body": "", "comments": comments});
            api(root, host, "POST", &format!("repos/{{owner}}/{{repo}}/pulls/{}/reviews", pr.number), Some(&body))?;
            Ok(fs.len())
        }
        Host::Gitlab => {
            let mut n = 0;
            for f in fs {
                let old = if f.old_path.is_empty() { &f.path } else { &f.old_path };
                let body = json!({"body": inline_body(f), "position": {
                    "position_type": "text", "base_sha": pr.base_sha, "start_sha": pr.start_sha, "head_sha": pr.head_sha,
                    "new_path": f.path, "old_path": old, "new_line": f.line,
                }});
                match api(root, host, "POST", &format!("projects/:id/merge_requests/{}/discussions", pr.number), Some(&body)) {
                    Ok(_) => n += 1,
                    Err(e) => ui::warn(&format!("{}:{} not posted inline: {e}", f.path, f.line)),
                }
            }
            Ok(n)
        }
    }
}

fn upsert_summary(root: &Path, host: Host, n: u64, existing: Option<u64>, body: &str) -> Result<()> {
    let b = json!({ "body": body });
    let (method, path) = match (host, existing) {
        (Host::Github, Some(id)) => ("PATCH", format!("repos/{{owner}}/{{repo}}/issues/comments/{id}")),
        (Host::Github, None) => ("POST", format!("repos/{{owner}}/{{repo}}/issues/{n}/comments")),
        (Host::Gitlab, Some(id)) => ("PUT", format!("projects/:id/merge_requests/{n}/notes/{id}")),
        (Host::Gitlab, None) => ("POST", format!("projects/:id/merge_requests/{n}/notes")),
    };
    api(root, host, method, &path, Some(&b)).map(|_| ())
}

// ── workspace: detached worktree at the PR head + the diff ──────────

struct Workspace {
    root: PathBuf,
    tmp: PathBuf,
    wt: PathBuf,
    /// Private fetch refs of this run (head, base): concurrent reviews in one repo
    /// (Board, CLI) never read or delete each other's.
    refs: [String; 2],
    diff_file: PathBuf,
    meta_file: PathBuf,
    diff: String,
    added: HashMap<String, Added>,
}

/// `<number>-<pid>-<seq>`: unique per review run on this machine.
fn run_id(n: u64) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("{n}-{}-{}", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

impl Workspace {
    fn open(root: &Path, host: Host, pr: &Pr) -> Result<Self> {
        let id = run_id(pr.number);
        let tmp = std::env::temp_dir().join(format!("ckit-review-{id}"));
        let mut ws = Self {
            root: root.to_path_buf(),
            diff_file: tmp.join("pr.diff"),
            meta_file: tmp.join("pr.md"),
            wt: tmp.join("wt"),
            refs: [format!("refs/ckit-review/{id}/head"), format!("refs/ckit-review/{id}/base")],
            tmp,
            diff: String::new(),
            added: HashMap::new(),
        };
        let head = head_ref(host, pr.number);
        git(root, &["fetch", "--quiet", "--no-tags", "origin", &format!("+{head}:{}", ws.refs[0]), &format!("+refs/heads/{}:{}", pr.target, ws.refs[1])])
            .with_context(|| format!("git fetch {head} + {}", pr.target))?;
        let fetched = git(root, &["rev-parse", &ws.refs[0]])?;
        if fetched != pr.head_sha {
            bail!("{} moved to {} while reviewing — rerun", pr_label(host, pr.number), short(&fetched));
        }
        let mb = git(root, &["merge-base", &ws.refs[1], &pr.head_sha])
            .context("no merge base with the target branch (shallow clone? use fetch-depth: 0 / GIT_DEPTH: 0)")?;
        ws.diff = git_raw(root, &["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "-M", &mb, &pr.head_sha])?;
        ws.added = added_lines(&ws.diff);
        std::fs::create_dir_all(&ws.tmp)?;
        git(root, &["worktree", "add", "--quiet", "--detach", &ws.wt.display().to_string(), &pr.head_sha])?;
        std::fs::write(&ws.diff_file, &ws.diff)?;
        let untag = |s: &str| s.replace("</pr_source", "‹/pr_source").replace("<pr_source", "‹pr_source");
        std::fs::write(&ws.meta_file, format!("<pr_source>\n# {}\n\n{}\n</pr_source>\n", untag(&pr.title), untag(&pr.body)))?;
        Ok(ws)
    }
}

/// Removes the worktree, temp files and the private fetch refs.
impl Drop for Workspace {
    fn drop(&mut self) {
        if self.wt.exists() {
            let _ = git(&self.root, &["worktree", "remove", "--force", &self.wt.display().to_string()]);
        }
        let _ = std::fs::remove_dir_all(&self.tmp);
        for r in &self.refs {
            let _ = git(&self.root, &["update-ref", "-d", r]);
        }
    }
}

/// Unified diff → added line numbers (new side) per new path.
fn added_lines(diff: &str) -> HashMap<String, Added> {
    let mut out: HashMap<String, Added> = HashMap::new();
    let (mut old, mut cur): (Option<String>, Option<String>) = (None, None);
    let mut line = 0u64;
    for l in diff.lines() {
        if l.starts_with("diff --git ") {
            (old, cur) = (None, None);
        } else if let Some(p) = l.strip_prefix("--- ") {
            old = p.strip_prefix("a/").map(String::from);
        } else if let Some(p) = l.strip_prefix("+++ ") {
            cur = p.strip_prefix("b/").map(String::from); // `/dev/null` = deleted file
            if let Some(c) = &cur {
                out.entry(c.clone()).or_default().old_path = old.clone().unwrap_or_else(|| c.clone());
            }
        } else if let Some(h) = l.strip_prefix("@@ ") {
            // `@@ -a,b +c,d @@`: the new side starts at c.
            line = h.split(' ').find_map(|t| t.strip_prefix('+')).and_then(|t| t.split(',').next()).and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if let Some(c) = &cur {
            match l.as_bytes().first() {
                Some(b'+') => {
                    out.entry(c.clone()).or_default().lines.insert(line);
                    line += 1;
                }
                Some(b' ') => line += 1,
                _ => {} // `-` and `\ No newline at end of file`
            }
        }
    }
    out
}

fn on_added_line(added: &HashMap<String, Added>, f: &Finding) -> bool {
    added.get(&f.path).is_some_and(|a| a.lines.contains(&f.line))
}

// ── omp ─────────────────────────────────────────────────────────────

/// Profile `ckit-review`: same models as the user's omp (models.yml + model
/// roles), but no MCP servers, no xd:// devices, no memory, no auto-learn. With
/// `--tools read,grep,glob` the model can read the repo and nothing else.
/// (`--no-tools` alone still exposes every MCP tool and `learn`/`manage_skill`.)
fn prepare_profile() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    let dir = home.join(".omp/profiles").join(PROFILE).join("agent");
    std::fs::create_dir_all(&dir)?;
    let models = home.join(".omp/agent/models.yml");
    let dst = dir.join("models.yml");
    if models.exists() {
        write_atomic(&dst, &std::fs::read(&models)?)?;
    } else {
        let _ = std::fs::remove_file(&dst);
    }
    // JSON is YAML: the user's model roles go in verbatim.
    let roles = Command::new("omp")
        .args(["config", "get", "modelRoles", "--json"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .map(|v| v["value"].clone())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}));
    let cfg = format!("# managed by `ckit review` — rewritten every run\ntools:\n  xdev: false\nautolearn:\n  enabled: false\nmemory:\n  backend: off\nmcp:\n  enableProjectConfig: false\nmodelRoles: {roles}\n");
    write_atomic(&dir.join("config.yml"), cfg.as_bytes())?;
    Ok(())
}

/// Reviews may start concurrently (Board): a reader never sees a half-written
/// file. 0600: models.yml holds API keys.
fn write_atomic(dst: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = dst.with_extension(format!("tmp-{}", run_id(0)));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(&tmp, dst)?;
    Ok(())
}

fn run_omp(ws: &Workspace, pr: &Pr, model: Option<&str>) -> Result<Review> {
    prepare_profile()?;
    let tmpl = assets::read("review/prompt.md").ok_or_else(|| anyhow!("embedded review/prompt.md missing"))?;
    let lines: usize = ws.added.values().map(|a| a.lines.len()).sum();
    let prompt = tmpl
        .replace("{{NUMBER}}", &pr.number.to_string())
        .replace("{{META}}", &ws.meta_file.display().to_string())
        .replace("{{DIFF}}", &ws.diff_file.display().to_string())
        .replace("{{FILES}}", &ws.added.len().to_string())
        .replace("{{LINES}}", &lines.to_string())
        .replace("{{MAX}}", &MAX_FINDINGS.to_string());
    let log = ws.tmp.join("omp.jsonl");
    let mut cmd = Command::new("omp");
    // Started from inside an omp session these point every child at THAT project.
    for v in ["OMP_DAEMON_PROJECT_DIR", "OMP_DAEMON_RUNTIME_DIR", "OMP_DAEMON_IDLE_GRACE_MS"] {
        cmd.env_remove(v);
    }
    // The post token (and any other token/password) never reaches the model's process.
    for (name, _) in std::env::vars_os() {
        let k = name.to_string_lossy().to_uppercase();
        if k.contains("TOKEN") || k.contains("PASSWORD") || ["GH_", "GLAB_", "GITLAB_", "GITHUB_"].iter().any(|p| k.starts_with(p)) {
            cmd.env_remove(&name);
        }
    }
    cmd.current_dir(&ws.wt)
        .args(["--profile", PROFILE, "--no-session", "--no-extensions", "--no-title", "--tools", "read,grep,glob", "--mode", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(std::fs::File::create(&log)?))
        // A file, not a pipe: a chatty omp would fill a pipe nobody reads until exit and hang.
        .stderr(Stdio::from(std::fs::File::create(ws.tmp.join("omp.err"))?));
    if let Some(m) = model {
        cmd.args(["--model", m]);
    }
    cmd.arg("-p").arg(prompt);
    let mut child = cmd.spawn().context("omp not found — install it (`8sync setup`, or curl -fsSL https://omp.sh/install | sh)")?;
    let started = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if started.elapsed() > OMP_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            bail!("omp review timed out after {} min", OMP_TIMEOUT.as_secs() / 60);
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    let raw = std::fs::read_to_string(&log).unwrap_or_default();
    let text = last_answer(&raw);
    if !status.success() || text.is_err() {
        let err = std::fs::read_to_string(ws.tmp.join("omp.err")).unwrap_or_default();
        let why = text.err().map(|e| e.to_string()).unwrap_or_default();
        bail!("omp review failed ({status}): {why} {}", err.lines().rev().take(3).collect::<Vec<_>>().join(" | "));
    }
    let mut r = parse_review(&text?)?;
    // `read` can reach files outside the repo (env, credential files): whatever the
    // model quotes is published, so secrets are masked before anything is posted.
    let secrets = secret_values();
    r.summary = redact(&r.summary, &secrets);
    for f in &mut r.findings {
        f.title = redact(&f.title, &secrets);
        f.body = redact(&f.body, &secrets);
    }
    r.findings.retain(|f| !f.path.starts_with('/') && !f.path.split('/').any(|s| s == ".."));
    Ok(r)
}

/// Values of env vars that look like credentials (model keys, tokens, passwords).
fn secret_values() -> Vec<String> {
    let mut v: Vec<String> = std::env::vars()
        .filter(|(k, val)| {
            let k = k.to_uppercase();
            val.len() >= 8 && ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASS", "AUTH"].iter().any(|w| k.contains(w))
        })
        .map(|(_, val)| val)
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.len()));
    v
}

/// Mask exact secret values and well-known token shapes (gitleaks-style prefixes).
fn redact(s: &str, secrets: &[String]) -> String {
    let mut out = s.to_string();
    for sec in secrets {
        out = out.replace(sec.as_str(), "[REDACTED]");
    }
    const PREFIXES: [&str; 10] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "sk-ant-", "sk-", "xox"];
    let mut res = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some((i, p)) = PREFIXES.iter().filter_map(|p| rest.find(p).map(|i| (i, *p))).min_by_key(|(i, _)| *i) {
        let tail = &rest[i + p.len()..];
        let n = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(tail.len());
        res.push_str(&rest[..i]);
        if n >= 16 {
            res.push_str("[REDACTED]");
        } else {
            res.push_str(&rest[i..i + p.len() + n]);
        }
        rest = &tail[n..];
    }
    res.push_str(rest);
    res
}

/// Text of the last assistant message in an omp `--mode json` stream.
fn last_answer(log: &str) -> Result<String> {
    let last = log
        .lines()
        .filter(|l| l.starts_with("{\"type\":\"message_end\""))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|v| v["message"].clone())
        .filter(|m| m["role"] == "assistant")
        .last()
        .ok_or_else(|| anyhow!("no answer"))?;
    if last["stopReason"] == "error" || last.get("errorMessage").is_some() {
        bail!("{}", last["errorMessage"].as_str().unwrap_or("model error"));
    }
    let text: Vec<&str> = last["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
    Ok(text.join("\n"))
}

/// The answer's JSON object (tolerates a code fence or stray prose around it).
fn parse_review(text: &str) -> Result<Review> {
    let (s, e) = (text.find('{'), text.rfind('}'));
    let json = match (s, e) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => bail!("model answer has no JSON object: {}", text.chars().take(200).collect::<String>()),
    };
    let mut r: Review = serde_json::from_str(json).with_context(|| format!("model answer is not the review JSON: {}", json.chars().take(200).collect::<String>()))?;
    r.findings.retain(|f| !f.path.trim().is_empty() && !f.title.trim().is_empty());
    r.findings.sort_by_key(|f| !f.blocking()); // stable: blocking first, model order kept
    r.findings.truncate(MAX_FINDINGS);
    Ok(r)
}

// ── rendering ───────────────────────────────────────────────────────

/// Model text must not forge our markers (or close an HTML comment).
fn clean(s: &str) -> String {
    s.replace("<!--", "&lt;!--").replace("-->", "--&gt;").trim().to_string()
}

fn reviewed_head(body: &str) -> Option<&str> {
    body.strip_prefix(SUMMARY_MARK)?.split_whitespace().next()
}

fn finding_key(body: &str) -> Option<&str> {
    body.strip_prefix(FINDING_MARK)?.split_whitespace().next()
}

fn inline_body(f: &Finding) -> String {
    format!("{FINDING_MARK}{} -->\n**🔴 {}**\n\n{}", f.key(), clean(&f.title), clean(&f.body))
}

fn render_summary(r: &Stored) -> String {
    let mut s = format!("{SUMMARY_MARK}{} -->\n### 🤖 ckit review · `{}`\n\n{}\n", r.head_sha, short(&r.head_sha), clean(&r.summary));
    let (block, sugg): (Vec<&Finding>, Vec<&Finding>) = r.findings.iter().partition(|f| f.blocking());
    if r.findings.is_empty() {
        s.push_str("\nKhông thấy vấn đề nào cần sửa.\n");
    }
    for (head, list) in [("🔴 Cần sửa", block), ("🟡 Gợi ý", sugg)] {
        if list.is_empty() {
            continue;
        }
        s.push_str(&format!("\n**{head} ({})**\n", list.len()));
        for f in list {
            let at = if f.line > 0 { format!("{}:{}", f.path, f.line) } else { f.path.clone() };
            let body = clean(&f.body).replace('\n', " ");
            s.push_str(&format!("- `{}` **{}**{}\n", at, clean(&f.title), if body.is_empty() { String::new() } else { format!(" — {body}") }));
        }
    }
    s.push_str(&format!("\n<sub>Review tự động bởi ckit, chỉ góp ý, không chặn merge. Gắn nhãn `{SKIP_LABEL}` để tắt.</sub>\n"));
    if s.len() > MAX_BODY {
        let mut cut = MAX_BODY;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n…\n");
    }
    s
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

// ── init: CI templates ──────────────────────────────────────────────

fn init(root: &Path, host: Host, force: bool) -> Result<()> {
    let (asset, rel) = match host {
        Host::Github => ("review/github-workflow.yml", ".github/workflows/ckit-review.yml"),
        Host::Gitlab => ("review/gitlab-ci.yml", ".gitlab/ckit-review.yml"),
    };
    let dst = root.join(rel);
    if dst.exists() && !force {
        ui::skip(rel, "exists — --force to overwrite");
    } else {
        let body = assets::read(asset).ok_or_else(|| anyhow!("embedded {asset} missing"))?;
        std::fs::create_dir_all(dst.parent().unwrap_or(root))?;
        std::fs::write(&dst, body)?;
        ui::ok(&format!("wrote {rel}"));
    }
    match host {
        Host::Github => {
            ui::info("Set repo secret ANTHROPIC_API_KEY (or OPENAI_API_KEY…); optional repo variable CKIT_REVIEW_MODEL.");
        }
        Host::Gitlab => {
            ui::info("Add to .gitlab-ci.yml:  include: { local: .gitlab/ckit-review.yml }");
            ui::info("CI/CD variables (masked): GITLAB_TOKEN = bot/project access token with `api` scope (CI_JOB_TOKEN cannot comment), ANTHROPIC_API_KEY; optional CKIT_REVIEW_MODEL.");
        }
    }
    Ok(())
}

// ── process helpers ─────────────────────────────────────────────────

fn sh(cwd: &Path, bin: &str, args: &[&str], stdin: Option<&str>) -> Result<String> {
    let mut c = Command::new(bin);
    // Runs unattended (CI, Board): a credential prompt would hang forever.
    c.current_dir(cwd).args(args).env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never");
    c.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = c.spawn().with_context(|| format!("`{bin}` not found"))?;
    if let (Some(input), Some(mut w)) = (stdin, child.stdin.take()) {
        w.write_all(input.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() { String::from_utf8_lossy(&out.stdout).to_string() } else { err.to_string() };
        bail!("{bin} {}: {}", args.iter().take(4).copied().collect::<Vec<_>>().join(" "), msg.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn git_raw(root: &Path, args: &[&str]) -> Result<String> {
    sh(root, "git", args, None)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    git_raw(root, args).map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,4 +1,5 @@\n keep\n-gone\n+new2\n+new3\n same\n@@ -20,2 +21,2 @@ fn x()\n ctx\n+new22\n\\ No newline at end of file\ndiff --git a/old.rs b/renamed.rs\nsimilarity index 90%\nrename from old.rs\nrename to renamed.rs\n--- a/old.rs\n+++ b/renamed.rs\n@@ -3 +3 @@\n-x\n+y\ndiff --git a/del.rs b/del.rs\n--- a/del.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\ndiff --git a/n.rs b/n.rs\nnew file mode 100644\n--- /dev/null\n+++ b/n.rs\n@@ -0,0 +1,2 @@\n+a\n+b\n";

    #[test]
    fn added_lines_track_new_side_numbers_across_hunks_renames_and_new_files() {
        let a = added_lines(DIFF);
        let mut l: Vec<u64> = a["src/a.rs"].lines.iter().copied().collect();
        l.sort();
        assert_eq!(l, vec![2, 3, 22]);
        assert_eq!(a["renamed.rs"].old_path, "old.rs");
        assert_eq!(a["renamed.rs"].lines, HashSet::from([3]));
        assert_eq!(a["n.rs"].old_path, "n.rs");
        assert_eq!(a["n.rs"].lines, HashSet::from([1, 2]));
        assert!(!a.contains_key("del.rs"), "a deleted file has no new side");
    }

    #[test]
    fn only_blocking_findings_on_added_lines_go_inline() {
        let a = added_lines(DIFF);
        let f = |path: &str, line, sev: &str| Finding { path: path.into(), line, severity: sev.into(), title: "t".into(), body: String::new(), inline: false, old_path: String::new() };
        assert!(on_added_line(&a, &f("src/a.rs", 22, "blocking")));
        assert!(!on_added_line(&a, &f("src/a.rs", 21, "blocking")), "context line");
        assert!(!on_added_line(&a, &f("other.rs", 2, "blocking")));
    }

    #[test]
    fn review_json_is_found_in_fenced_or_chatty_answers_and_blocking_sorts_first() {
        let r = parse_review("Here:\n```json\n{\"summary\":\"ok\",\"findings\":[{\"path\":\"a\",\"line\":1,\"severity\":\"suggestion\",\"title\":\"s\"},{\"path\":\"b\",\"line\":2,\"severity\":\"Blocking\",\"title\":\"b\"},{\"path\":\"\",\"title\":\"no path\"}]}\n```").unwrap();
        assert_eq!(r.findings.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        assert!(parse_review("no json here").is_err());
    }

    #[test]
    fn secrets_are_masked_before_posting() {
        let s = redact("key=sk-ant-api03-AAAABBBBCCCCDDDD tok glpat-abcdefghijklmnopqrst short sk-x my-secret-value-123 end", &["my-secret-value-123".into()]);
        assert_eq!(s, "key=[REDACTED] tok [REDACTED] short sk-x [REDACTED] end");
    }

    #[test]
    fn a_second_publisher_waits_for_the_first_and_the_lock_is_freed() {
        let root = std::env::temp_dir().join(format!("ckit-lock-test-{}", std::process::id()));
        let held = PublishLock::take(&root, 7).unwrap();
        let r2 = root.clone();
        let t = Instant::now();
        let waiter = std::thread::spawn(move || PublishLock::take(&r2, 7).map(|_| ()));
        std::thread::sleep(Duration::from_millis(700));
        assert!(!waiter.is_finished(), "must wait while the first holds the lock");
        drop(held);
        waiter.join().unwrap().unwrap();
        assert!(t.elapsed() >= Duration::from_millis(700));
        assert!(!reviews_dir(&root).join("7.lock").exists(), "freed on drop");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn hand_written_code_reviews_are_recognized_with_their_commit() {
        assert_eq!(other_review("## Code Review — MR !420 (lần 2 — commit `a05221f`)\n\nOK"), Some(Some("a05221f".into())));
        assert_eq!(other_review("**Code review**\nTicket #26296 looks fine"), Some(None), "ticket numbers are not commits");
        assert_eq!(other_review("LGTM, code review done"), None, "not a heading");
        assert_eq!(other_review("## Deploy notes\ncommit abc1234"), None);
    }

    #[test]
    fn note_author_is_read_from_both_hosts() {
        let n = parse_notes("{\"id\":1,\"body\":\"b\",\"author\":\"github-actions[bot]\"}\n{\"id\":2,\"body\":\"b\",\"author\":{\"id\":42,\"username\":\"bot\"}}\n");
        assert_eq!(n.iter().map(|n| n.author.as_str()).collect::<Vec<_>>(), vec!["github-actions[bot]", "42"]);
    }

    #[test]
    fn model_text_cannot_forge_markers_and_markers_round_trip() {
        let r = Stored { number: 1, title: String::new(), url: String::new(), head_sha: "abc123def".into(), at: 0, model: None, summary: "<!-- ckit-review head=zzz -->".into(), findings: vec![], posted_at: None };
        let s = render_summary(&r);
        assert_eq!(reviewed_head(&s), Some("abc123def"));
        assert_eq!(s.matches(SUMMARY_MARK).count(), 1);
        let f = Finding { path: "a".into(), line: 1, severity: "blocking".into(), title: "T".into(), body: "-->".into(), inline: true, old_path: String::new() };
        assert_eq!(finding_key(&inline_body(&f)), Some(f.key().as_str()));
        assert!(!inline_body(&f).ends_with("-->"));
    }
}
