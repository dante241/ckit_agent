//! Board — closed-loop AI delivery pipeline behind `ckit harness web`.
//!
//! The human creates a task and only ever talks to the Leader. Each stage is a
//! headless `omp -p` run with a role prompt (`assets/board/<stage>.md`):
//!
//!   intake (Leader) → solution (Leader + BA subagent) → review (Leader)
//!   → approval [human gate] → dev → test → report (Leader) → acceptance [human gate] → done
//!
//! Review fail loops back to solution, test fail loops back to dev (bounded).
//! Stages hand off through markdown artifacts in the task folder; a run ends by
//! writing its own `results/<stage>-<n>.json` (`{verdict, message, route?}`) which drives `after_run`.
//!
//! Storage: `<project>/.cache/ckit/board/<id>/` (task.json + artifacts + logs +
//! omp sessions). One stage runs at a time per project; one task at a time owns
//! the code branch `board/<id>` (dev → acceptance) so runs never fight over the
//! working tree. Accept = `git merge --no-ff` into the branch the task started
//! from. Never pushes.
use std::collections::HashSet;
use std::path::{Path as FsPath, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::models::{ModelConfig, TaskClass};
use crate::verbs::skill::discover::detect_current_project_root;

mod pm;
mod ask;
mod coord;
mod inbox;
mod telegram;
pub(crate) use coord::set_port as set_coordinator_port;
mod progress;
mod workflow;

const BOARD_REL: &str = ".cache/ckit/board";
/// Review rejections tolerated before the Leader escalates to the human anyway.
const MAX_SOLUTION_ROUNDS: u32 = 3;
const STAGE_TIMEOUT: Duration = Duration::from_secs(2 * 3600);
const PLANNING_REL: &str = "agents/planning";
const LOG_TAIL: usize = 400;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum Stage {
    Investigate,
    Intake,
    Solution,
    Review,
    Approval,
    Dev,
    Test,
    Security,
    Report,
    Acceptance,
    Done,
    Cancelled,
}

/// Every step a task may enable, in the only order they run.
const CATALOG: [Stage; 10] = [
    Stage::Investigate,
    Stage::Intake,
    Stage::Solution,
    Stage::Review,
    Stage::Approval,
    Stage::Dev,
    Stage::Test,
    Stage::Security,
    Stage::Report,
    Stage::Acceptance,
];

/// Steps of tasks created before steps were selectable (and the default preset).
fn full_feature() -> Vec<Stage> {
    use Stage::*;
    vec![Intake, Solution, Review, Approval, Dev, Test, Report, Acceptance]
}

impl Stage {
    fn key(self) -> &'static str {
        match self {
            Stage::Investigate => "investigate",
            Stage::Intake => "intake",
            Stage::Solution => "solution",
            Stage::Review => "review",
            Stage::Approval => "approval",
            Stage::Dev => "dev",
            Stage::Test => "test",
            Stage::Security => "security",
            Stage::Report => "report",
            Stage::Acceptance => "acceptance",
            Stage::Done => "done",
            Stage::Cancelled => "cancelled",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Stage::Investigate => "Điều tra / Hỏi đáp",
            Stage::Intake => "Tiếp nhận",
            Stage::Solution => "Kế hoạch",
            Stage::Review => "PO duyệt",
            Stage::Approval => "Bạn duyệt kế hoạch",
            Stage::Dev => "Phát triển",
            Stage::Test => "Kiểm thử",
            Stage::Security => "Review bảo mật",
            Stage::Report => "Báo cáo",
            Stage::Acceptance => "Bạn nghiệm thu",
            Stage::Done => "Xong",
            Stage::Cancelled => "Đã huỷ",
        }
    }

    /// Steps that only make sense when another step is enabled too.
    fn requires(self) -> &'static [Stage] {
        match self {
            Stage::Review | Stage::Approval => &[Stage::Solution],
            Stage::Test | Stage::Security => &[Stage::Dev],
            _ => &[],
        }
    }

    /// Human gates: the task waits there instead of running omp.
    fn is_gate(self) -> bool {
        matches!(self, Stage::Approval | Stage::Acceptance)
    }

    /// The team seat doing this step.
    fn who(self) -> &'static str {
        match self {
            Stage::Solution => "Leader + BA",
            Stage::Review => "Leader (PO)",
            Stage::Approval | Stage::Acceptance => "Bạn",
            Stage::Dev => "Dev",
            Stage::Test => "Tester",
            Stage::Security => "Security reviewer",
            _ => "Leader",
        }
    }

    fn model_class(self) -> TaskClass {
        match self {
            Stage::Review | Stage::Test | Stage::Security => TaskClass::Review,
            Stage::Dev => TaskClass::Code,
            _ => TaskClass::Plan,
        }
    }
}

fn catalog_pos(s: Stage) -> usize {
    CATALOG.iter().position(|c| *c == s).unwrap_or(CATALOG.len())
}

/// Normalize a requested step list: catalog order, no duplicates, dependencies met.
fn validate_steps(steps: &[Stage]) -> Result<Vec<Stage>, String> {
    let out: Vec<Stage> = CATALOG.iter().copied().filter(|c| steps.contains(c)).collect();
    if steps.iter().any(|s| !CATALOG.contains(s)) {
        return Err("bước không hợp lệ".into());
    }
    if out.iter().all(|s| s.is_gate()) {
        return Err("cần ít nhất một bước cho team AI".into());
    }
    for s in &out {
        if let Some(r) = s.requires().iter().find(|r| !out.contains(r)) {
            return Err(format!("{} cần bật {}", s.label(), r.label()));
        }
    }
    Ok(out)
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum Status {
    Queued,
    Running,
    /// Needs the human: intake questions, approval gate, acceptance gate.
    Waiting,
    Failed,
    Done,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Msg {
    from: String,
    text: String,
    at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Run {
    stage: Stage,
    n: u32,
    started: String,
    ended: Option<String>,
    exit: Option<i32>,
    cost: f64,
    verdict: Option<String>,
    log: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Rounds {
    solution: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Task {
    id: String,
    title: String,
    request: String,
    stage: Stage,
    status: Status,
    note: Option<String>,
    branch: Option<String>,
    base: Option<String>,
    /// `/feature` slug — the task's planning tree is `agents/planning/<feature>/`.
    #[serde(default)]
    feature: Option<String>,
    /// Untracked files under the /feature paths that existed before the task
    /// started (the user's own drafts) — never committed by the board.
    #[serde(default)]
    preexisting: Vec<String>,
    cost: f64,
    created: String,
    updated: String,
    thread: Vec<Msg>,
    runs: Vec<Run>,
    rounds: Rounds,
    /// The steps the boss chose for this task (catalog order); disabled ones are skipped.
    #[serde(default = "full_feature")]
    steps: Vec<Stage>,
    /// `board_ask` questions raised by this task's runs (mid-step team chat).
    #[serde(default)]
    asks: Vec<ask::Ask>,
    /// Stopped at a safe point by a pause request (Waiting, same stage).
    #[serde(default)]
    paused: bool,
    /// Next run continues the paused step's omp session (`--continue`).
    #[serde(default)]
    resume: bool,
    #[serde(default)]
    resume_note: String,
    /// Who parked it: new (never started) · restart · boss · coordinator. The
    /// coordinator may only resume what it paused itself.
    #[serde(default)]
    paused_by: String,
    /// The boss's own slash command (`/cook`) this task runs instead of `/feature`:
    /// fixed steps Solution (its plan) → Approval → Dev (its run) → Acceptance.
    #[serde(default)]
    workflow: Option<workflow::Workflow>,
}

impl Task {
    fn summary(&self, root: &FsPath) -> serde_json::Value {
        serde_json::json!({
            "id": self.id, "title": self.title, "stage": self.stage, "status": self.status,
            "note": self.note, "branch": self.branch, "cost": self.cost, "steps": self.steps,
            "workflow": self.workflow.as_ref().map(|w| w.name.clone()),
            "phase": active_phase(root, self),
            "progress": progress::progress(root, self).map(|p| p.counts()),
            "asking": self.asks.iter().filter(|a| a.status == "boss").count(),
            "paused": self.paused,
            "started": !self.runs.is_empty(),
            "run_started": self.runs.last().filter(|_| self.status == Status::Running).map(|r| r.started.clone()),
            "created": self.created, "updated": self.updated,
        })
    }

    fn say(&mut self, from: &str, text: &str) {
        let text = text.trim();
        if !text.is_empty() {
            self.thread.push(Msg { from: from.into(), text: text.into(), at: iso_now() });
        }
    }

    /// Billed total, re-summed (never accumulated, so a run is billed exactly once):
    /// every run plus the BA/PO answer sessions, each priced from its own log.
    fn recost(&mut self, root: &FsPath) {
        let asks: f64 = std::fs::read_dir(ask::ask_dir(root, &self.id))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .map(|p| log_cost(&p))
            .sum();
        self.cost = self.runs.iter().map(|r| r.cost).sum::<f64>() + asks;
    }

    fn set(&mut self, stage: Stage, status: Status) {
        self.stage = stage;
        self.status = status;
    }

    /// A running step is waiting on the boss's answer (`board_ask`).
    fn needs_answer(&self) -> bool {
        self.asks.iter().any(|a| a.status == "boss")
    }

    fn has(&self, s: Stage) -> bool {
        self.steps.contains(&s)
    }

    /// Move to the first enabled step after `from`: gates wait, others queue;
    /// past the last step the task is done (branch left unmerged when the boss
    /// turned acceptance off — the worker parks it).
    fn advance(&mut self, from: Stage) {
        let next = CATALOG.iter().copied().skip(catalog_pos(from) + 1).find(|s| self.has(*s));
        match next {
            Some(s) if s.is_gate() => self.set(s, Status::Waiting),
            Some(s) => self.set(s, Status::Queued),
            None => self.set(Stage::Done, Status::Done),
        }
    }

    /// Owns the working tree (and the single `/feature` ACTIVE slot) from its first
    /// code-touching run until done/cancelled — `ACTIVE.md` can point at only one
    /// feature. Read-only investigate runs never take it.
    fn holds_tree(&self) -> bool {
        (self.branch.is_some() || self.runs.iter().any(|r| r.stage != Stage::Investigate))
            && !matches!(self.stage, Stage::Done | Stage::Cancelled)
    }

    /// The plan run of a workflow task: read-only outside its plan dirs.
    fn is_plan_run(&self) -> bool {
        self.workflow.is_some() && self.stage == Stage::Solution
    }
}

// ── State machine (pure) ────────────────────────────────────────────────────

/// Apply a finished stage run's result verdict.
fn after_run(t: &mut Task, verdict: &str, route: Option<&str>) {
    use Stage::*;
    use Status::*;
    match (t.stage, verdict) {
        // The answer goes to the boss, who asks more or moves on (approve).
        // Stopped on a pause request: same stage, waits for Tiếp tục.
        (_, "paused") => {
            t.paused = true;
            t.status = Waiting;
            t.note = Some("Tạm dừng".into());
        }
        (Investigate, "pass" | "need_user") => t.set(Investigate, Waiting),
        (Intake, "need_user") => t.set(Intake, Waiting),
        // The human only asked/agreed after delivery: back to their accept gate, no rework.
        (Intake, "pass") if route == Some("acceptance") && t.branch.is_some() && t.has(Acceptance) => t.set(Acceptance, Waiting),
        (Review, "fail") => {
            t.rounds.solution += 1;
            if t.rounds.solution >= MAX_SOLUTION_ROUNDS {
                // A plan the PO keeps rejecting never reaches Dev unseen: without the
                // boss's approval step the task stops for a human instead.
                if t.has(Approval) {
                    t.advance(Review);
                } else {
                    t.note = Some(format!(
                        "PO từ chối kế hoạch {MAX_SOLUTION_ROUNDS} lần — xem REVIEW. Bật 'Bạn duyệt kế hoạch' rồi Chạy lại để tự quyết, hoặc Chạy lại để lập kế hoạch lại từ đầu"
                    ));
                    t.status = Failed;
                }
            } else {
                t.set(Solution, Queued);
            }
        }
        // `/feature ship` closed the phase and ROADMAP has another → plan it (new gate 2).
        (Test, "pass") if route == Some("next_phase") && t.has(Solution) => {
            t.rounds.solution = 0;
            t.set(Solution, Queued);
        }
        (Intake | Solution | Review | Dev | Report, "pass") => {
            let s = t.stage;
            t.advance(s);
        }
        // Test/security ran their own converge rounds; the Leader reports what is left.
        (Test | Security, "pass" | "fail") => {
            let s = t.stage;
            t.advance(s);
        }
        (s, v) => {
            t.note = Some(format!("{} kết thúc với verdict '{}'", s.key(), v));
            t.status = Failed;
        }
    }
}

/// Human reply while the task waits on them.
fn on_reply(t: &mut Task, text: &str) -> Result<(), String> {
    if t.status != Status::Waiting {
        return Err("task không chờ phản hồi của bạn".into());
    }
    if text.trim().is_empty() {
        return Err("nội dung trống".into());
    }
    t.say("user", text);
    let stage = match t.stage {
        Stage::Investigate => Stage::Investigate,
        Stage::Intake => Stage::Intake,
        Stage::Approval => {
            t.rounds.solution = 0;
            Stage::Solution
        }
        // Change request after delivery: the Leader re-triages (fix vs re-design);
        // without an intake step Dev takes the change straight from the thread.
        Stage::Acceptance if t.has(Stage::Intake) => Stage::Intake,
        Stage::Acceptance if t.has(Stage::Dev) => Stage::Dev,
        s => return Err(format!("không thể trả lời ở stage {}", s.key())),
    };
    t.set(stage, Status::Queued);
    t.note = None;
    Ok(())
}

// ── Storage ─────────────────────────────────────────────────────────────────

/// Serializes every task.json read-modify-write (API handlers + worker threads).
static LOCK: Mutex<()> = Mutex::new(());
/// Project roots with a stage currently running.
static RUNNING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
/// `<root>|<id>` of running tasks the human cancelled.
static CANCEL: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn running() -> &'static Mutex<HashSet<PathBuf>> {
    RUNNING.get_or_init(Default::default)
}
fn cancels() -> &'static Mutex<HashSet<String>> {
    CANCEL.get_or_init(Default::default)
}
fn guard() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn board_dir(root: &FsPath) -> PathBuf {
    root.join(BOARD_REL)
}
fn task_dir(root: &FsPath, id: &str) -> PathBuf {
    board_dir(root).join(id)
}

fn load(root: &FsPath, id: &str) -> Option<Task> {
    let raw = std::fs::read_to_string(task_dir(root, id).join("task.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save(root: &FsPath, t: &mut Task) -> std::io::Result<()> {
    t.updated = iso_now();
    let dir = task_dir(root, &t.id);
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join("task.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(t).unwrap_or_default())?;
    std::fs::rename(tmp, dir.join("task.json"))
}

fn list(root: &FsPath) -> Vec<Task> {
    let mut out: Vec<Task> = std::fs::read_dir(board_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| load(root, e.file_name().to_str()?))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn next_id(root: &FsPath) -> String {
    let max = list(root)
        .iter()
        .filter_map(|t| t.id.strip_prefix("T-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("T-{:03}", max + 1)
}

/// Every project that ever had a board task — the worker serves all of them,
/// not just the one the dashboard currently shows.
fn roots_file(home: &FsPath) -> PathBuf {
    crate::brand::config_dir(home).join("board-roots.json")
}
fn roots(home: &FsPath) -> Vec<PathBuf> {
    std::fs::read_to_string(roots_file(home))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}
fn register_root(home: &FsPath, root: &FsPath) {
    let mut all = roots(home);
    if !all.iter().any(|r| r == root) {
        all.push(root.to_path_buf());
        let f = roots_file(home);
        if let Some(p) = f.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(f, serde_json::to_string_pretty(&all).unwrap_or_default());
    }
    exclude_board_from_git(root);
}

/// Board state and code-intel caches live inside the working tree that dev
/// commits from. Projects without the managed `.gitignore` would otherwise commit
/// `.cache/ckit/board/**` onto the task branch (lost on the next `git switch`), and
/// `.serena/` / `.codegraph/` would look like edits to the read-only Investigate
/// check. Local-only (`.git/info/exclude`), idempotent, never touches a tracked file.
fn exclude_board_from_git(root: &FsPath) {
    let Ok(rel) = git(root, &["rev-parse", "--git-path", "info/exclude"]) else { return };
    let path = root.join(rel);
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    let missing: Vec<&str> = [".cache/ckit/", ".serena/", ".codegraph/"].into_iter().filter(|e| !cur.lines().any(|l| l.trim() == *e)).collect();
    if missing.is_empty() {
        return;
    }
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let sep = if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" };
    let _ = std::fs::write(&path, format!("{cur}{sep}# ckit board state + code-intel caches (local)\n{}\n", missing.join("\n")));
}

// ── Git ─────────────────────────────────────────────────────────────────────

fn git(root: &FsPath, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn is_git(root: &FsPath) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

fn clean(root: &FsPath) -> bool {
    git(root, &["status", "--porcelain", "--untracked-files=no"]).map(|s| s.is_empty()).unwrap_or(false)
}

fn current_branch(root: &FsPath) -> Option<String> {
    git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()
}

/// Put the working tree on the task's branch (created from the current branch on
/// the task's first run): every stage writes the `/feature` planning tree there.
fn checkout_task_branch(root: &FsPath, t: &mut Task) -> Result<(), String> {
    if !is_git(root) {
        return Ok(()); // no VCS: work in place, accept = mark done
    }
    let want = format!("board/{}", t.id.to_lowercase());
    if current_branch(root).as_deref() == Some(want.as_str()) {
        return Ok(());
    }
    if !clean(root) {
        return Err("working tree có thay đổi chưa commit — commit/stash để team bắt đầu".into());
    }
    if t.branch.is_some() {
        git(root, &["switch", &want])?;
    } else {
        t.base = current_branch(root);
        t.preexisting = git(root, &["ls-files", "--others", "--exclude-standard", "--", "agents", "docs", "CHANGELOG.md"])
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect();
        git(root, &["switch", "-c", &want])?;
        t.branch = Some(want);
    }
    Ok(())
}

/// Commit what `/feature` steps left uncommitted on the task branch — only the
/// paths /feature writes (planning tree, `agents/*.md` memory, `docs/`, CHANGELOG).
/// Anything else still dirty is NOT swept in: the caller refuses with the list.
fn commit_leftovers(root: &FsPath, t: &Task, why: &str) -> Result<(), String> {
    let on_branch = t.branch.is_some() && current_branch(root) == t.branch;
    if !on_branch {
        return Ok(());
    }
    // One pathspec per call: a missing path (no docs/ yet) fails the whole `git add`.
    for p in ["agents", "docs", "CHANGELOG.md"].iter().filter(|p| root.join(p).exists()) {
        git(root, &["add", "-A", "--", p])?;
    }
    // Keep the user's own pre-task drafts out of the board's commit (and the merge).
    for p in &t.preexisting {
        let _ = git(root, &["reset", "-q", "--", p]);
    }
    if git(root, &["diff", "--cached", "--quiet"]).is_ok() {
        return Ok(()); // nothing staged
    }
    git(root, &["commit", "-m", &format!("docs: board {} {why}", t.id)]).map(|_| ())
}

fn dirty_files(root: &FsPath) -> String {
    git(root, &["diff", "--name-only", "HEAD"]).unwrap_or_default().lines().collect::<Vec<_>>().join(", ")
}

/// Park the task branch (commit leftovers) and return to the base branch. Best-effort.
fn leave_task_branch(root: &FsPath, t: &Task) {
    if let (Some(br), Some(base)) = (&t.branch, &t.base) {
        let _ = commit_leftovers(root, t, "parked");
        if current_branch(root).as_deref() == Some(br.as_str()) && clean(root) {
            let _ = git(root, &["switch", base]);
        }
    }
}

/// `/feature` keeps ONE active feature in `ACTIVE.md` + `config.json`; the task
/// branch rewrote both for its own slug. Put base's versions back on the branch
/// before merging, so accepting a task never changes the feature the user has
/// active on base (absent on base → removed from the branch too).
fn restore_base_pointers(root: &FsPath, t: &Task, base: &str) -> Result<(), String> {
    for f in [format!("{PLANNING_REL}/ACTIVE.md"), format!("{PLANNING_REL}/config.json")] {
        if git(root, &["cat-file", "-e", &format!("{base}:{f}")]).is_ok() {
            git(root, &["checkout", base, "--", &f])?;
        } else {
            git(root, &["rm", "-q", "--ignore-unmatch", "--", &f])?;
        }
    }
    if git(root, &["diff", "--cached", "--quiet"]).is_ok() {
        return Ok(());
    }
    git(root, &["commit", "-m", &format!("docs: board {} restore base /feature pointers", t.id)]).map(|_| ())
}

fn merge_task_branch(root: &FsPath, t: &Task) -> Result<String, String> {
    let (Some(br), Some(base)) = (&t.branch, &t.base) else {
        return Ok("không có nhánh git — đánh dấu hoàn tất".into());
    };
    if current_branch(root).as_deref() != Some(br.as_str()) {
        git(root, &["switch", br])?;
    }
    commit_leftovers(root, t, "planning")?;
    if !clean(root) {
        return Err(format!("còn file sửa dở ngoài phần /feature, không merge được: {}", dirty_files(root)));
    }
    restore_base_pointers(root, t, base)?;
    git(root, &["switch", base])?;
    let msg = format!("board {}: {}", t.id, t.title);
    if let Err(e) = git(root, &["merge", "--no-ff", br, "-m", &msg]) {
        let _ = git(root, &["merge", "--abort"]);
        return Err(format!("merge {br} → {base} lỗi: {e}"));
    }
    Ok(format!("Đã merge {br} vào {base} (chưa push)."))
}

// ── /feature planning tree ──────────────────────────────────────────────────

fn feature_slug(t: &Task) -> String {
    format!("board-{}", t.id.to_lowercase())
}

/// The task's planning dir — live, or archived by `/feature ship` on the last phase.
fn feature_dir(root: &FsPath, t: &Task) -> Option<PathBuf> {
    let slug = t.feature.as_ref()?;
    let planning = root.join(PLANNING_REL);
    [planning.join(slug), planning.join("_archive").join(slug)].into_iter().find(|p| p.is_dir())
}

/// `active_phase` from the feature's STATE.md frontmatter (`null` once all shipped).
fn active_phase(root: &FsPath, t: &Task) -> Option<String> {
    let state = std::fs::read_to_string(feature_dir(root, t)?.join("STATE.md")).ok()?;
    let v = state.lines().find_map(|l| l.trim().strip_prefix("active_phase:"))?;
    let v = v.trim().trim_matches('"').trim_matches('\'');
    (!v.is_empty() && v != "null").then(|| v.to_string())
}

/// Scaffold + activate the task's feature on its first run (`ckit feature new`).
/// A live tree whose ACTIVE slot was cleared or moved (a finished plan-only task
/// continued into code) is re-activated.
fn ensure_feature(root: &FsPath, t: &mut Task) -> Result<(), String> {
    if let Some(slug) = t.feature.clone().filter(|_| feature_dir(root, t).is_some()) {
        if root.join(PLANNING_REL).join(&slug).is_dir() {
            crate::verbs::feature::ensure_active(root, &slug).map_err(|e| format!("feature switch: {e}"))?;
        }
        return Ok(());
    }
    let slug = feature_slug(t);
    if !root.join(PLANNING_REL).join(&slug).is_dir() {
        crate::verbs::feature::new(root, &slug).map_err(|e| format!("feature new: {e}"))?;
    }
    t.feature = Some(slug);
    Ok(())
}

// ── Worker ──────────────────────────────────────────────────────────────────

/// Start the background pipeline worker (one per dashboard process).
pub(crate) fn start_worker(home: PathBuf) {
    {
        // Opening the dashboard never starts work by itself: whatever was running
        // or queued when it stopped (Ctrl+C, crash, reboot) is parked as paused and
        // runs only when the boss presses Tiếp tục. An interrupted step continues in
        // its own omp session (work, commits and engine state are kept).
        let _g = guard();
        for root in roots(&home) {
            for mut t in list(&root) {
                if matches!(t.status, Status::Running | Status::Queued) {
                    if t.status == Status::Running {
                        kill_orphan(&task_dir(&root, &t.id));
                    }
                    park_on_open(&root, &mut t);
                    let _ = save(&root, &mut t);
                } else if repair_runs(&root, &mut t) {
                    let _ = save(&root, &mut t);
                }
            }
        }
    }
    telegram::start();
    std::thread::spawn(move || loop {
        for root in roots(&home) {
            ask::sync(&root);
            coord::tick(&root);
            telegram::watch(&root);
            pm::observe(&root);
            if running().lock().map(|r| r.contains(&root)).unwrap_or(true) {
                continue;
            }
            if let Some(t) = claim_next(&root) {
                running().lock().map(|mut r| r.insert(root.clone())).ok();
                let root2 = root.clone();
                std::thread::spawn(move || {
                    run_stage(&root2, t);
                    running().lock().map(|mut r| r.remove(&root2)).ok();
                });
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    });
}

/// Park a task the previous dashboard left running or queued: paused until the
/// boss presses Tiếp tục. An interrupted run is closed (cost from its log, open
/// questions closed) and flagged to continue in its omp session if one exists.
fn park_on_open(root: &FsPath, t: &mut Task) {
    if t.status == Status::Running {
        let dir = task_dir(root, &t.id);
        let _ = std::fs::remove_file(inbox::pause_marker(root, &t.id));
        let resumable = t.runs.last().is_some_and(|r| session_since(&dir.join("sessions"), &r.started));
        repair_runs(root, t);
        let open: Vec<String> = t.asks.iter().filter(|a| matches!(a.status.as_str(), "answering" | "boss")).map(|a| a.id.clone()).collect();
        ask::close_open(root, t);
        t.resume = resumable;
        t.resume_note = format!(
            "Dashboard bị tắt giữa bước này. Kiểm tra lại trạng thái (git status, engine_status) rồi làm tiếp từ chỗ dở — không làm lại phần đã commit.{}",
            if open.is_empty() { String::new() } else { format!(" Câu hỏi {} đã bị đóng do khởi động lại — tự quyết theo đề xuất của bạn hoặc hỏi lại bằng board_ask.", open.join(", ")) }
        );
    }
    t.paused = true;
    t.status = Status::Waiting;
    t.paused_by = "restart".into();
    t.note = Some("Tạm dừng — dashboard vừa mở lại, bấm Tiếp tục để chạy".into());
}

/// Close runs no process owns any more (the dashboard died mid-run, or another
/// dashboard raced this task) and re-bill every run from its own log: a racing
/// run's end may have been written onto another run. Startup only, nothing runs.
fn repair_runs(root: &FsPath, t: &mut Task) -> bool {
    if t.runs.iter().all(|r| r.ended.is_some()) {
        return false;
    }
    let logs = task_dir(root, &t.id).join("logs");
    for r in &mut t.runs {
        if r.ended.is_none() {
            r.ended = Some(iso_now());
            r.verdict = Some("interrupted".into());
        }
        r.cost = log_cost(&logs.join(&r.log));
    }
    t.recost(root);
    true
}

/// Close the run this worker started, found by its log: not "the last run", which
/// is someone else's when a racing dashboard claimed a newer run meanwhile.
fn close_run(root: &FsPath, t: &mut Task, log: &str, exit: Option<i32>, verdict: Option<String>, cost: f64) {
    if let Some(r) = t.runs.iter_mut().rev().find(|r| r.log == log) {
        r.ended = Some(iso_now());
        r.exit = exit;
        r.cost = cost;
        r.verdict = verdict;
    }
    t.recost(root);
}

/// A run that is no longer the task's current one (a newer run was claimed after
/// it, or the task left that step) is only closed and billed: its verdict belongs
/// to a superseded attempt and must not move the task, nor touch the current
/// run's cancel / pause requests.
fn close_if_stale(root: &FsPath, t: &mut Task, ran: Stage, log: &str, exit: Option<i32>, verdict: Option<String>, cost: f64) -> bool {
    let current = t.runs.last().is_some_and(|r| r.log == log) && (t.stage, t.status) == (ran, Status::Running);
    if current {
        return false;
    }
    close_run(root, t, log, exit, verdict, cost);
    t.say("system", &format!("Bỏ qua kết quả của {} (đã có lượt chạy mới hơn)", log.trim_end_matches(".jsonl")));
    true
}

/// Pick the oldest runnable queued task, mark it running, append its Run.
fn claim_next(root: &FsPath) -> Option<Task> {
    let _g = guard();
    let tasks = list(root);
    // Two holders only after a finished plan-only task was continued into code while
    // another task held the tree: the one checked out keeps it, the other waits.
    let holders: Vec<&Task> = tasks.iter().filter(|t| t.holds_tree()).collect();
    let checked_out = if holders.len() > 1 { current_branch(root) } else { None };
    let holder = holders.iter().find(|t| t.branch.is_some() && t.branch == checked_out).or(holders.first()).map(|t| t.id.clone());
    let mut queued: Vec<Task> = tasks.into_iter().filter(|t| t.status == Status::Queued).collect();
    queued.sort_by(|a, b| a.updated.cmp(&b.updated));
    for mut t in queued {
        // Read-only investigate runs may slip in while another task holds the tree
        // (the per-project RUNNING slot still keeps runs one at a time).
        if let Some(h) = holder.as_ref().filter(|h| **h != t.id && t.stage != Stage::Investigate) {
            let note = format!("chờ {h} xong (team đang làm {h})");
            if t.note.as_deref() != Some(note.as_str()) {
                t.note = Some(note);
                let _ = save(root, &mut t);
            }
            continue;
        }
        let prep = if t.stage == Stage::Investigate {
            Ok(())
        } else {
            checkout_task_branch(root, &mut t).and_then(|_| {
                // `/feature` tree only when the task plans with it; quick tasks code straight
                // from the request, workflow tasks plan with their own command.
                if t.workflow.is_none() && (t.has(Stage::Intake) || t.has(Stage::Solution)) { ensure_feature(root, &mut t) } else { Ok(()) }
            })
        };
        if let Err(e) = prep {
            if t.note.as_deref() != Some(e.as_str()) {
                t.note = Some(e);
                let _ = save(root, &mut t);
            }
            continue;
        }
        let n = t.runs.iter().filter(|r| r.stage == t.stage).count() as u32 + 1;
        t.runs.push(Run {
            stage: t.stage,
            n,
            started: iso_now(),
            ended: None,
            exit: None,
            cost: 0.0,
            verdict: None,
            log: format!("{}-{}.jsonl", t.stage.key(), n),
        });
        t.status = Status::Running;
        t.note = None;
        save(root, &mut t).ok()?;
        return Some(t);
    }
    None
}

#[derive(Deserialize)]
struct StageResult {
    verdict: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    route: Option<String>,
    /// Workflow plan step: the plan file it wrote (`null` when the command needs none).
    #[serde(default)]
    plan: Option<String>,
}

/// `git status` around a run that must not change the tree. The workflow plan
/// lists every untracked file so its allowed plan dirs are matched by path.
/// Untrimmed: `git()` would eat the leading status column of the first line.
fn tree_status(root: &FsPath, plan: bool) -> String {
    let args: &[&str] = if plan { &["status", "--porcelain", "--untracked-files=all"] } else { &["status", "--porcelain"] };
    Command::new("git").current_dir(root).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// Fail a read-only run (Investigate; a workflow's plan outside its plan dirs)
/// that changed the tree. The boss's files are left as they are, for them to see.
fn guard_tree(t: &mut Task, plan: bool, before: &str, after: &str) {
    if plan {
        let bad = workflow::plan_violations(before, after);
        if !bad.is_empty() {
            // Back to the plan step (it may have advanced to Approval): Chạy lại re-plans.
            t.set(Stage::Solution, Status::Failed);
            t.note = Some(format!("Bước kế hoạch đã sửa code: {}", bad.join(", ")));
        }
        return;
    }
    let touched: Vec<&str> = after.lines().filter(|l| !before.lines().any(|b| b == *l)).collect();
    if !touched.is_empty() {
        t.status = Status::Failed;
        t.note = Some(format!("bước Điều tra đã sửa file (chỉ được đọc): {}", touched.join(", ")));
    }
}

/// HEAD and branch around a workflow run (`/cook` creates its own branch, syncs,
/// commits: none of that shows in `git status`).
struct GitPos {
    head: Option<String>,
}

impl GitPos {
    fn take(root: &FsPath) -> Self {
        Self { head: git(root, &["rev-parse", "HEAD"]).ok() }
    }

    /// (branch now, HEAD moved, commits added since `self`).
    fn since(&self, root: &FsPath) -> (Option<String>, bool, u32) {
        let head = git(root, &["rev-parse", "HEAD"]).ok();
        let n = match (&self.head, &head) {
            (Some(a), Some(b)) if a != b => git(root, &["rev-list", "--count", &format!("{a}..{b}")]).ok().and_then(|s| s.parse().ok()).unwrap_or(0),
            _ => 0,
        };
        (current_branch(root), head != self.head, n)
    }
}

/// A workflow run must stay on the task branch the board owns (Acceptance merges
/// it); its plan run must not commit or move HEAD either. Fails at the step that
/// ran (after_run may have advanced past it), keeping an earlier failure note.
fn guard_git(t: &mut Task, ran: Stage, branch_now: Option<&str>, head_moved: bool, new_commits: u32) {
    let Some(want) = t.branch.clone() else { return };
    let step = if ran == Stage::Solution { "Bước kế hoạch" } else { "Bước thực hiện" };
    let mut bad = Vec::new();
    if branch_now != Some(want.as_str()) {
        bad.push(format!("{step} đã đổi nhánh sang {} (phải ở {want})", branch_now.unwrap_or("?")));
    }
    if ran == Stage::Solution && new_commits > 0 {
        bad.push(format!("{step} đã commit {new_commits} commit"));
    } else if ran == Stage::Solution && head_moved {
        bad.push(format!("{step} đã đổi HEAD"));
    }
    if bad.is_empty() {
        return;
    }
    let prev = t.note.take().filter(|_| (t.stage, t.status) == (ran, Status::Failed));
    t.set(ran, Status::Failed);
    t.note = Some(prev.into_iter().chain(bad).collect::<Vec<_>>().join("; "));
}

fn run_stage(root: &FsPath, t: Task) {
    let dir = task_dir(root, &t.id);
    let logs = dir.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let log_name = t.runs.last().map(|r| r.log.clone()).unwrap_or_default();
    let log_path = logs.join(&log_name);
    let _ = std::fs::remove_file(result_file(&dir, &log_name));
    let since = std::time::SystemTime::now();

    // Investigate must leave the tree untouched, a workflow's plan touches only its
    // plan dirs: compare git status around the run.
    let plan_run = t.is_plan_run();
    let snapshot = (t.stage == Stage::Investigate || plan_run).then(|| tree_status(root, plan_run));
    // A workflow's plan and run steps must leave git where the board put it.
    let git_pos = (t.workflow.is_some() && matches!(t.stage, Stage::Solution | Stage::Dev)).then(|| GitPos::take(root));
    let outcome = spawn_and_wait(root, &t, &dir, &log_path);
    let cost = log_cost(&log_path);
    let result = read_result(&dir, &log_name, since);

    let _g = guard();
    let Some(mut cur) = load(root, &t.id) else { return };
    let exit = outcome.as_ref().ok().copied().flatten();
    if close_if_stale(root, &mut cur, t.stage, &log_name, exit, result.as_ref().map(|x| x.verdict.clone()), cost) {
        let _ = save(root, &mut cur);
        return;
    }
    let cancelled = cancels().lock().map(|mut c| c.remove(&cancel_key(root, &t.id))).unwrap_or(false);
    // A pause was requested during the run: the marker outlives the run (the agent
    // may not have reached a safe point before the grace kill).
    let pause_req = std::fs::remove_file(inbox::pause_marker(root, &t.id)).is_ok();
    let paused_hard = pause_req && result.is_none();
    let verdict = if cancelled { Some("cancelled".into()) } else { result.as_ref().map(|x| x.verdict.clone()) };
    close_run(root, &mut cur, &log_name, exit, verdict, cost);
    if cancelled {
        cur.set(Stage::Cancelled, Status::Cancelled);
        cur.note = None;
        cur.say("system", "Đã huỷ theo yêu cầu.");
        leave_task_branch(root, &cur);
    } else {
        match (outcome, result) {
            (Err(e), _) => {
                cur.status = Status::Failed;
                cur.note = Some(e);
            }
            (Ok(_), None) if paused_hard => {
                after_run(&mut cur, "paused", None);
                cur.say("system", "Đã dừng cứng sau 5 phút (agent chưa tự dừng). Phần đang sửa dở vẫn nằm trong cây làm việc.");
            }
            (Ok(_), None) => {
                cur.status = Status::Failed;
                cur.note = Some(format!("{} không ghi file kết quả ({}) — xem log", cur.stage.key(), result_file(&dir, &log_name).display()));
            }
            (Ok(_), Some(r)) => {
                cur.say("leader", &r.message);
                if cur.is_plan_run() && r.verdict.trim() == "pass" {
                    record_plan(root, &mut cur, r.plan.as_deref());
                }
                after_run(&mut cur, r.verdict.trim(), r.route.as_deref());
            }
        }
    }
    if let Some(before) = snapshot.filter(|_| !cancelled) {
        guard_tree(&mut cur, plan_run, &before, &tree_status(root, plan_run));
    }
    if let Some(pos) = git_pos.filter(|_| !cancelled) {
        let (branch, moved, n) = pos.since(root);
        guard_git(&mut cur, t.stage, branch.as_deref(), moved, n);
    }
    ask::close_open(root, &mut cur);
    cur.resume = false;
    cur.resume_note.clear();
    if (cur.stage, cur.status) == (Stage::Approval, Status::Waiting) {
        coord::event(root, format!("{} \"{}\" chờ duyệt kế hoạch. Đọc REVIEW + PLAN (board_task) và quyết theo luật uỷ quyền: board_approve, hoặc để lại cho sếp kèm tóm tắt 3 dòng.", cur.id, cur.title));
    }
    park_if_finished(root, &mut cur);
    let _ = save(root, &mut cur);
}

/// Keep the plan file a workflow's plan run reported (shown at Approval).
fn record_plan(root: &FsPath, t: &mut Task, reported: Option<&str>) {
    let plan = reported.and_then(|p| workflow::plan_path(root, p));
    if let (Some(p), None) = (reported, &plan) {
        t.say("system", &format!("Không đọc được file kế hoạch {p} (phải là một file trong dự án)."));
    }
    if let Some(w) = t.workflow.as_mut() {
        w.plan = plan;
    }
}

/// Done without the acceptance step: commit /feature leftovers, return to base,
/// leave the task branch unmerged for the boss.
fn park_if_finished(root: &FsPath, t: &mut Task) {
    if (t.stage, t.status) != (Stage::Done, Status::Done) {
        return;
    }
    leave_task_branch(root, t);
    if let (Some(br), Some(base)) = (t.branch.clone(), t.base.clone()) {
        t.say("system", &format!("Xong (task không có bước nghiệm thu): nhánh {br} để nguyên, chưa merge vào {base}."));
    }
}

fn cancel_key(root: &FsPath, id: &str) -> String {
    format!("{}|{}", root.display(), id)
}

/// `omp` for a board run. Drops the omp daemon variables a dashboard inherits
/// when it was started from inside an omp session: with them, every child omp
/// takes THAT session's repo as its project and loads its `.omp/extensions`
/// (e.g. a stale engine without `board_ask`) instead of the task's project.
fn omp_cmd() -> Command {
    let mut cmd = Command::new("omp");
    for v in ["OMP_DAEMON_PROJECT_DIR", "OMP_DAEMON_RUNTIME_DIR", "OMP_DAEMON_IDLE_GRACE_MS"] {
        cmd.env_remove(v);
    }
    cmd
}

/// The run continues an omp session: a paused/interrupted step, or a workflow's
/// Dev picking up the plan run's session (every run of a task shares `sessions/`,
/// and `--continue` takes the newest one = the plan the boss just approved).
fn continues_session(t: &Task, dir: &FsPath) -> bool {
    t.resume || (t.workflow.is_some() && t.stage == Stage::Dev && !walk_files(&dir.join("sessions")).is_empty())
}

/// omp arguments for the task's current stage, the prompt excluded.
fn stage_args(root: &FsPath, t: &Task, dir: &FsPath) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec!["--cwd".into(), root.into()];
    args.extend(["--mode", "json", "--approval-mode", "yolo", "--session-dir"].map(Into::into));
    args.push(dir.join("sessions").into());
    args.extend(ModelConfig::load().class_flags(t.stage.model_class()).into_iter().map(Into::into));
    if continues_session(t, dir) {
        args.push("--continue".into());
    }
    args
}

/// Run omp for the task's current stage. Ok(exit code) when it ran to the end.
fn spawn_and_wait(root: &FsPath, t: &Task, dir: &FsPath, log_path: &FsPath) -> Result<Option<i32>, String> {
    let prompt = if t.resume { resume_prompt(t, dir) } else { build_prompt(root, t, dir)? };
    let log_name = log_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let result = result_file(dir, &log_name);
    if let Some(p) = result.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let log = std::fs::File::create(log_path).map_err(|e| format!("log: {e}"))?;
    let log_err = log.try_clone().map_err(|e| format!("log: {e}"))?;
    let mut cmd = omp_cmd();
    cmd.current_dir(root)
        .args(stage_args(root, t, dir))
        // `board_ask` (engine extension) is registered only when this is set.
        .env("CKIT_BOARD_ASK_DIR", ask::ask_dir(root, &t.id))
        .env("CKIT_BOARD_STAGE", t.stage.key())
        .env("CKIT_BOARD_RESULT", &result);
    cmd
        .arg("-p")
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    let mut child = cmd.spawn().map_err(|e| format!("không chạy được omp: {e}"))?;
    // Lets a restarted dashboard stop this omp if the dashboard dies first.
    let pid = child.id();
    let _ = std::fs::write(dir.join("run.pid"), pid.to_string());
    let out = wait_child(root, t, &mut child);
    release_pid(dir, pid);
    out
}

/// Drop `run.pid` only while it still names this run's omp: a racing run of the
/// task may have written its own pid since, and a restarted dashboard needs it.
fn release_pid(dir: &FsPath, pid: u32) {
    let path = dir.join("run.pid");
    if std::fs::read_to_string(&path).is_ok_and(|s| s.trim() == pid.to_string()) {
        let _ = std::fs::remove_file(path);
    }
}

/// This run's verdict file (`<task>/results/<stage>-<n>.json`): runs never share
/// one, so a superseded run writing late cannot hand its verdict to the current run.
fn result_file(dir: &FsPath, log: &str) -> PathBuf {
    dir.join("results").join(format!("{}.json", log.trim_end_matches(".jsonl")))
}

/// The run's own result; else the legacy shared `result.json` (an agent following
/// an older prompt in a `--continue`d session), only if written during this run.
fn read_result(dir: &FsPath, log: &str, since: std::time::SystemTime) -> Option<StageResult> {
    let own = result_file(dir, log);
    let legacy = dir.join("result.json");
    let path = if own.exists() {
        own
    } else if std::fs::metadata(&legacy).and_then(|m| m.modified()).is_ok_and(|m| m >= since) {
        legacy
    } else {
        return None;
    };
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Wait for a run's omp: stops it on cancel, on an overdue pause, or after the
/// working-time limit.
fn wait_child(root: &FsPath, t: &Task, child: &mut std::process::Child) -> Result<Option<i32>, String> {
    // The timeout counts working time only: a run waiting on a teammate's or the
    // boss's answer (`board_ask`) is not stuck.
    let mut worked = Duration::ZERO;
    let mut tick = Instant::now();
    let ask_dir = ask::ask_dir(root, &t.id);
    let key = cancel_key(root, &t.id);
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(st.code());
        }
        let cancelled = cancels().lock().map(|c| c.contains(&key)).unwrap_or(false);
        let dt = tick.elapsed();
        tick = Instant::now();
        if !ask::has_open_question(&ask_dir) {
            worked += dt;
        }
        let pause_overdue = inbox::pause_age(root, &t.id).is_some_and(|a| a > inbox::PAUSE_GRACE);
        if pause_overdue {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        if cancelled || worked > STAGE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return if cancelled { Ok(None) } else { Err(format!("{} quá thời gian {}h", t.stage.key(), STAGE_TIMEOUT.as_secs() / 3600)) };
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn build_prompt(root: &FsPath, t: &Task, dir: &FsPath) -> Result<String, String> {
    let asset = |name: &str| crate::assets::read(name).ok_or_else(|| format!("asset {name} thiếu"));
    // A workflow task follows the boss's own command, not the `/feature` team prompts.
    let template = if let Some(w) = &t.workflow {
        let name = if t.stage == Stage::Dev { "board/cmd-run.md" } else { "board/cmd-plan.md" };
        asset(name)?.replace("{{CMD}}", &w.name).replace("{{CMD_PATH}}", &w.path).replace("{{PLAN}}", w.plan.as_deref().unwrap_or("(none)"))
    } else {
        // Without a planning step Dev/Test work straight from the request (no PLAN, no AC matrix).
        let variant = if matches!(t.stage, Stage::Dev | Stage::Test) && !t.has(Stage::Solution) { "-auto" } else { "" };
        format!("{}\n\n{}", asset("board/common.md")?, asset(&format!("board/{}{variant}.md", t.stage.key()))?)
    };
    let steps = t.steps.iter().map(|s| s.key()).collect::<Vec<_>>().join(" → ");
    let thread = if t.thread.is_empty() {
        "(empty)".to_string()
    } else {
        t.thread.iter().map(|m| format!("[{} · {}] {}", m.from, m.at, m.text)).collect::<Vec<_>>().join("\n\n")
    };
    let round = match t.stage {
        Stage::Solution => t.rounds.solution + 1,
        _ => 1,
    };
    let none = "(none)".to_string();
    let feature_dir = feature_dir(root, t).map(|p| p.display().to_string()).unwrap_or_else(|| none.clone());
    Ok(template
        .replace("{{ROOT}}", &root.display().to_string())
        .replace("{{DIR}}", &dir.display().to_string())
        .replace("{{RESULT}}", &result_file(dir, &t.runs.last().map(|r| r.log.clone()).unwrap_or_default()).display().to_string())
        .replace("{{ID}}", &t.id)
        .replace("{{TITLE}}", &t.title)
        .replace("{{REQUEST}}", &t.request)
        .replace("{{THREAD}}", &thread)
        .replace("{{BRANCH}}", t.branch.as_ref().unwrap_or(&none))
        .replace("{{BASE}}", t.base.as_ref().unwrap_or(&none))
        .replace("{{FEATURE}}", t.feature.as_ref().unwrap_or(&none))
        .replace("{{FEATURE_DIR}}", &feature_dir)
        .replace("{{PHASE}}", &active_phase(root, t).unwrap_or_else(|| none.clone()))
        .replace("{{ROUND}}", &round.to_string())
        .replace("{{STEPS}}", &steps)
        .replace(
            "{{PREEXISTING}}",
            &if t.preexisting.is_empty() { none.clone() } else { t.preexisting.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ") },
        ))
}

/// Documents the drawer shows, in reading order: the `/feature` tree (PROJECT,
/// REQUIREMENTS, ROADMAP, per-phase CONTEXT/PLAN/VERIFICATION, STATE) plus the
/// board's own REVIEW (PO) and REPORT. Second value = the doc to open first.
fn task_docs(root: &FsPath, t: &Task) -> (Vec<(String, String)>, Option<String>) {
    let mut docs: Vec<(String, String)> = Vec::new();
    let mut add = |name: String, path: PathBuf| {
        if let Ok(body) = std::fs::read_to_string(path) {
            docs.push((name, body));
        }
    };
    let fdir = feature_dir(root, t);
    if let Some(f) = &fdir {
        for n in ["PROJECT", "REQUIREMENTS", "ROADMAP", "COVERAGE"] {
            add(n.into(), f.join(format!("{n}.md")));
        }
        let mut phase_files: Vec<PathBuf> = std::fs::read_dir(f.join("phases"))
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        phase_files.sort();
        for p in phase_files {
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            add(stem, p);
        }
    }
    let dir = task_dir(root, &t.id);
    let plan_doc = t.workflow.as_ref().and_then(|w| Some((format!("PLAN (/{})", w.name), w.plan.as_ref()?)));
    if let Some((name, rel)) = &plan_doc {
        add(name.clone(), root.join(rel));
    }
    add("REVIEW".into(), dir.join("REVIEW.md"));
    add("TEST".into(), dir.join("TEST.md"));
    add("SECURITY".into(), dir.join("SECURITY.md"));
    add("REPORT".into(), dir.join("REPORT.md"));
    if let Some(f) = &fdir {
        add("STATE".into(), f.join("STATE.md"));
    }
    let last = |suffix: &str| docs.iter().rev().find(|(n, _)| n.ends_with(suffix)).map(|(n, _)| n.clone());
    let focus = match t.stage {
        Stage::Solution | Stage::Approval | Stage::Dev if plan_doc.as_ref().is_some_and(|(n, _)| docs.iter().any(|(d, _)| d == n)) => plan_doc.map(|(n, _)| n),
        Stage::Intake => last("ROADMAP"),
        Stage::Solution | Stage::Review | Stage::Approval => last("-CONTEXT"),
        Stage::Dev => last("-PLAN"),
        Stage::Test => last("-VERIFICATION").or_else(|| last("-PLAN")),
        _ => last("REPORT"),
    }
    .or_else(|| docs.last().map(|(n, _)| n.clone()));
    (docs, focus)
}

// ── Log digest ──────────────────────────────────────────────────────────────

/// Only `message_end` events carry full assistant/tool messages; skip the
/// (many) streaming deltas without parsing them.
fn message_ends(log: &str) -> impl Iterator<Item = serde_json::Value> + '_ {
    log.lines()
        .filter(|l| l.starts_with("{\"type\":\"message_end\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("message").cloned())
}

fn log_cost(path: &FsPath) -> f64 {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    message_ends(&raw)
        .filter_map(|m| m.pointer("/usage/cost").cloned())
        .map(|c| match c.get("total").and_then(|x| x.as_f64()) {
            Some(t) => t,
            None => c.as_object().map(|o| o.values().filter_map(|x| x.as_f64()).sum()).unwrap_or(0.0),
        })
        .sum()
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn log_lines(path: &FsPath) -> Vec<serde_json::Value> {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    let line = |kind: &str, text: String| serde_json::json!({ "kind": kind, "text": text });
    for l in raw.lines() {
        if !l.starts_with('{') {
            if !l.trim().is_empty() {
                out.push(line("error", clip(l, 500))); // stderr interleaved in the log
            }
            continue;
        }
        if !l.starts_with("{\"type\":\"message_end\"") {
            continue;
        }
        let Some(m) = serde_json::from_str::<serde_json::Value>(l).ok().and_then(|v| v.get("message").cloned()) else {
            continue;
        };
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let blocks = m.get("content").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        match role {
            "assistant" => {
                for b in blocks {
                    match b.get("type").and_then(|x| x.as_str()) {
                        Some("text") => {
                            let s = b.get("text").and_then(|x| x.as_str()).unwrap_or("").trim();
                            if !s.is_empty() {
                                out.push(line("text", clip(s, 2000)));
                            }
                        }
                        Some("toolCall") => {
                            let name = b.get("name").and_then(|x| x.as_str()).unwrap_or("tool");
                            let args = b.get("arguments").map(|a| a.to_string()).unwrap_or_default();
                            out.push(line("tool", format!("{name} {}", clip(&args, 240))));
                        }
                        _ => {}
                    }
                }
            }
            "toolResult" if m.get("isError").and_then(|x| x.as_bool()) == Some(true) => {
                let text = blocks.iter().filter_map(|b| b.get("text").and_then(|x| x.as_str())).collect::<Vec<_>>().join(" ");
                let name = m.get("toolName").and_then(|x| x.as_str()).unwrap_or("tool");
                out.push(line("error", format!("{name}: {}", clip(&text, 400))));
            }
            _ => {}
        }
    }
    let skip = out.len().saturating_sub(LOG_TAIL);
    out.split_off(skip)
}

// ── HTTP API ────────────────────────────────────────────────────────────────

type ApiErr = (StatusCode, String);
type ApiRes<T> = Result<Json<T>, ApiErr>;

pub(crate) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/board", get(api_list).post(api_create))
        .route("/api/board/all", get(api_all))
        .route("/api/board/:id", get(api_get))
        .route("/api/board/:id/log", get(api_log))
        .route("/api/board/:id/reply", post(api_reply))
        .route("/api/board/:id/approve", post(api_approve))
        .route("/api/board/:id/continue", post(api_continue))
        .route("/api/board/:id/retry", post(api_retry))
        .route("/api/board/:id/cancel", post(api_cancel))
        .route("/api/board/pm", get(pm::api_feed))
        .route("/api/board/pm/report", post(pm::api_report))
        .route("/api/board/:id/progress", get(api_progress))
        .route("/api/board/:id/phases", get(api_phases))
        .merge(telegram::routes())
        .route("/api/board/steps", get(api_step_catalog))
        .route("/api/board/workflows", get(workflow::api_list).post(workflow::api_toggle))
        .route("/api/board/:id/steps", post(api_steps))
        .route("/api/board/:id/asks/:qid", post(api_ask_answer))
        .route("/api/board/:id/asks/:qid/escalate", post(api_ask_escalate))
        .route("/api/board/:id/inbox", post(api_inbox))
        .route("/api/board/:id/resume", post(api_resume))
        .route("/api/board/coord", get(api_coord).post(api_coord_say))
        .route("/api/board/coord/enabled", post(api_coord_enabled))
}

/// Prompt for continuing a paused step inside its own omp session.
fn resume_prompt(t: &Task, dir: &FsPath) -> String {
    let note = if t.resume_note.is_empty() { "Làm tiếp từ chỗ đã dừng." } else { t.resume_note.as_str() };
    format!(
        "This step continues in the same session. {note}\nSame task and rules as before: if you use the engine, `engine_status` first and continue with the next pending task; finish by writing {} as your step requires (this run's result file; not the path of an earlier run).",
        result_file(dir, &t.runs.last().map(|r| r.log.clone()).unwrap_or_default()).display()
    )
}

/// Stop the omp a dead dashboard left running for this task (it would otherwise
/// race the resumed run on the same tree). Kills only a process whose command
/// line carries THIS task's session dir — pids get reused, and the boss may run
/// their own omp sessions.
fn kill_orphan(dir: &FsPath) {
    let Some(pid) = std::fs::read_to_string(dir.join("run.pid")).ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return };
    let _ = std::fs::remove_file(dir.join("run.pid"));
    let argv = Command::new("ps").args(["-ww", "-o", "command=", "-p", &pid.to_string()]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    if !argv.contains(&dir.join("sessions").display().to_string()) {
        return;
    }
    let _ = Command::new("kill").arg(pid.to_string()).status();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(250));
        let alive = Command::new("kill").args(["-0", &pid.to_string()]).status().is_ok_and(|s| s.success());
        if !alive {
            return;
        }
    }
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

/// The interrupted run got as far as an omp session (written after `started`).
fn session_since(sessions: &FsPath, started: &str) -> bool {
    let run_age = pm::secs_since(started);
    walk_files(sessions).iter().any(|p| {
        std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() <= run_age)
    })
}

fn walk_files(dir: &FsPath) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_files(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[derive(Deserialize)]
struct AnswerBody {
    answer: String,
    /// boss (default) · po / ba (the coordinator answering for the team)
    #[serde(default)]
    by: Option<String>,
}

/// The boss (or the coordinator as PO) answers a teammate's mid-step question.
async fn api_ask_answer(Path((id, qid)): Path<(String, String)>, Query(rq): Query<RootQ>, Json(b): Json<AnswerBody>) -> ApiRes<Task> {
    let by = b.by.clone().unwrap_or_else(|| "boss".into());
    if !matches!(by.as_str(), "boss" | "po" | "ba") {
        return Err((StatusCode::BAD_REQUEST, "by phải là boss, po hoặc ba".into()));
    }
    update(&rq, &id, |root, t| ask::answer_as(root, t, &qid, &b.answer, &by))
}

#[derive(Deserialize)]
struct EscalateBody {
    reason: String,
}

async fn api_ask_escalate(Path((id, qid)): Path<(String, String)>, Query(rq): Query<RootQ>, Json(b): Json<EscalateBody>) -> ApiRes<Task> {
    update(&rq, &id, |_, t| ask::escalate(t, &qid, &b.reason, "coordinator"))
}

#[derive(Deserialize)]
struct InboxBody {
    kind: String,
    #[serde(default)]
    text: String,
    /// boss (default) · coordinator
    #[serde(default)]
    from: Option<String>,
}

/// Note / pause into a running step (delivered live by the engine extension).
async fn api_inbox(Path(id): Path<String>, Query(rq): Query<RootQ>, Json(b): Json<InboxBody>) -> ApiRes<Task> {
    let from = if b.from.as_deref() == Some("coordinator") { "coordinator" } else { "boss" };
    update(&rq, &id, |root, t| inbox::post(root, t, &b.kind, &b.text, from))
}

async fn api_resume(Path(id): Path<String>, Query(rq): Query<RootQ>, Json(b): Json<InboxBody>) -> ApiRes<Task> {
    let from = if b.from.as_deref() == Some("coordinator") { "coordinator" } else { "boss" };
    update(&rq, &id, |_, t| inbox::resume(t, &b.text, from))
}

async fn api_coord(Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    Ok(Json(serde_json::json!({ "enabled": coord::enabled(&root), "busy": coord::busy(&root), "chat": coord::chat(&root, 200) })))
}

#[derive(Deserialize)]
struct SayBody {
    text: String,
}

async fn api_coord_say(Query(rq): Query<RootQ>, Json(b): Json<SayBody>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    coord::say(&root, &b.text).map_err(bad)?;
    // The worker serves registered projects only — the coordinator must run here.
    register_root(&dirs::home_dir().unwrap_or_default(), &root);
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

async fn api_coord_enabled(Query(rq): Query<RootQ>, Json(b): Json<EnabledBody>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    coord::set_enabled(&root, b.enabled).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "enabled": coord::enabled(&root) })))
}

/// The step catalog + presets the create form offers.
async fn api_step_catalog() -> Json<serde_json::Value> {
    Json(step_catalog_json())
}

/// Catalog JSON, shared with `/api/models` (its per-step model table). `class`
/// = the models.toml task class the step runs with; null for human gates.
pub(crate) fn step_catalog_json() -> serde_json::Value {
    use Stage::*;
    let catalog: Vec<serde_json::Value> = CATALOG
        .iter()
        .map(|s| {
            let class = (!s.is_gate()).then(|| s.model_class().key());
            serde_json::json!({ "key": s, "label": s.label(), "who": s.who(), "gate": s.is_gate(), "requires": s.requires(), "class": class })
        })
        .collect();
    let preset = |name: &str, steps: Vec<Stage>| serde_json::json!({ "name": name, "steps": steps });
    serde_json::json!({
        "catalog": catalog,
        "presets": [
            preset("Feature đầy đủ", full_feature()),
            preset("Sửa nhanh", vec![Dev, Test, Report, Acceptance]),
            preset("Chỉ kế hoạch", vec![Intake, Solution, Review, Approval]),
            preset("Điều tra", vec![Investigate]),
            preset("Điều tra rồi sửa", vec![Investigate, Dev, Test, Report, Acceptance]),
        ],
    })
}

#[derive(Deserialize)]
struct StepsBody {
    steps: Vec<Stage>,
}

/// The caller as shown in the thread: first word of its User-Agent (`curl/8`, `Mozilla/5.0`).
fn client_of(headers: &HeaderMap) -> String {
    headers.get("user-agent").and_then(|v| v.to_str().ok()).map(|ua| ua.split_whitespace().next().unwrap_or(ua).to_string()).unwrap_or_else(|| "?".into())
}

fn step_keys(s: &[Stage]) -> String {
    s.iter().map(|x| x.key()).collect::<Vec<_>>().join(" → ")
}

/// Change the steps still ahead of the task; the current and earlier ones are history.
async fn api_steps(Path(id): Path<String>, Query(rq): Query<RootQ>, headers: HeaderMap, Json(b): Json<StepsBody>) -> ApiRes<Task> {
    // Who changed the plan of steps is otherwise invisible: say it in the thread.
    let client = client_of(&headers);
    update(&rq, &id, |_, t| change_steps(t, &b.steps, &client))
}

fn change_steps(t: &mut Task, steps: &[Stage], client: &str) -> Result<(), String> {
    if matches!(t.status, Status::Done | Status::Cancelled) {
        return Err("task đã kết thúc".into());
    }
    if let Some(w) = &t.workflow {
        return Err(format!("việc chạy theo /{}: các bước cố định, không đổi được", w.name));
    }
    let new = validate_steps(steps)?;
    let cur = catalog_pos(t.stage);
    if let Some(s) = CATALOG.iter().take(cur + 1).find(|s| t.has(**s) != new.contains(s)) {
        return Err(format!("{} đã chạy hoặc đang chạy — không đổi được", s.label()));
    }
    if new != t.steps {
        t.say("system", &format!("Các bước đổi: {} ⇒ {} (từ {client})", step_keys(&t.steps), step_keys(&new)));
    }
    t.steps = new;
    Ok(())
}

/// The job as a hierarchy for the 3-level kanban: phases → tasks (or flat engine tasks).
async fn api_phases(Path(id): Path<String>, Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let t = load(&root, &id).ok_or((StatusCode::NOT_FOUND, format!("task {id} không tồn tại")))?;
    Ok(Json(progress::board(&root, &t)))
}

async fn api_progress(Path(id): Path<String>, Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let t = load(&root, &id).ok_or((StatusCode::NOT_FOUND, format!("task {id} không tồn tại")))?;
    Ok(Json(serde_json::to_value(progress::progress(&root, &t)).unwrap_or_default()))
}

/// Board project for a request. The FE echoes the `project` it got from
/// `/api/board` as `?root=`, so an action always hits the task it was shown:
/// task ids repeat across repos (every repo has a T-001), and the dashboard's
/// active project (process cwd) can change while a drawer is open. `?root=` must
/// be a registered board project; without it, the active project.
#[derive(Deserialize, Default)]
pub(super) struct RootQ {
    root: Option<String>,
}

fn project(q: &RootQ) -> Result<PathBuf, ApiErr> {
    if let Some(r) = q.root.as_deref().filter(|s| !s.is_empty()) {
        let r = canonical(PathBuf::from(r));
        let home = dirs::home_dir().unwrap_or_default();
        if roots(&home).contains(&r) || active_root().as_ref() == Some(&r) {
            return Ok(r);
        }
        return Err((StatusCode::BAD_REQUEST, format!("{} không phải dự án có board", r.display())));
    }
    active_root().ok_or((StatusCode::NOT_FOUND, "not in a project".into()))
}

/// `/tmp/x` and `/private/tmp/x` must be ONE root, or the worker's
/// one-stage-per-project lock could run two stages on one checkout. Unix only:
/// Windows `canonicalize` yields verbatim `\\?\C:\…` paths that git and Node mishandle.
fn canonical(p: PathBuf) -> PathBuf {
    #[cfg(unix)]
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    p
}

fn active_root() -> Option<PathBuf> {
    detect_current_project_root().map(canonical)
}

fn bad(msg: impl Into<String>) -> ApiErr {
    (StatusCode::CONFLICT, msg.into())
}

/// Load → mutate → save under the lock; returns the saved task.
fn update(q: &RootQ, id: &str, f: impl FnOnce(&FsPath, &mut Task) -> Result<(), String>) -> ApiRes<Task> {
    let root = project(q)?;
    let _g = guard();
    let mut t = load(&root, id).ok_or((StatusCode::NOT_FOUND, format!("task {id} không tồn tại")))?;
    f(&root, &mut t).map_err(bad)?;
    save(&root, &mut t).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(t))
}

async fn api_list(Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let mut tasks = list(&root);
    tasks.reverse();
    // The worker serves every board project in the background; the dashboard
    // shows one. Surface the others so their gates/runs are never invisible.
    let home = dirs::home_dir().unwrap_or_default();
    let others: Vec<serde_json::Value> = roots(&home)
        .into_iter()
        .filter(|r| *r != root)
        .filter_map(|r| {
            let ts = list(&r);
            let active = ts.iter().filter(|t| matches!(t.status, Status::Running | Status::Queued)).count();
            let waiting = ts.iter().filter(|t| matches!(t.status, Status::Waiting | Status::Failed) || t.needs_answer()).count();
            (active + waiting > 0).then(|| serde_json::json!({ "project": r.display().to_string(), "active": active, "waiting": waiting }))
        })
        .collect();
    Ok(Json(serde_json::json!({
        "project": root.display().to_string(),
        "tasks": tasks.iter().map(|t| t.summary(&root)).collect::<Vec<_>>(),
        "others": others,
    })))
}

/// Every board project at once (the multi-project view): each with its tasks and
/// counts. Includes the dashboard's active project even with no tasks yet.
async fn api_all() -> ApiRes<serde_json::Value> {
    let home = dirs::home_dir().unwrap_or_default();
    let current = active_root();
    let mut all = roots(&home);
    if let Some(c) = &current {
        if !all.contains(c) {
            all.push(c.clone());
        }
    }
    let count = |ts: &[Task], s: Status| ts.iter().filter(|t| t.status == s).count();
    let mut projects: Vec<serde_json::Value> = all
        .iter()
        .map(|r| {
            let mut ts = list(r);
            ts.reverse();
            serde_json::json!({
                "project": r.display().to_string(),
                "name": r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "current": current.as_ref() == Some(r),
                "waiting": ts.iter().filter(|t| t.status == Status::Waiting || t.needs_answer()).count(),
                "failed": count(&ts, Status::Failed),
                "running": count(&ts, Status::Running),
                "queued": count(&ts, Status::Queued),
                "cost": ts.iter().map(|t| t.cost).sum::<f64>(),
                "tasks": ts.iter().map(|t| t.summary(r)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let n = |v: &serde_json::Value, k: &str| v[k].as_u64().unwrap_or(0);
    projects.sort_by(|a, b| {
        (n(b, "waiting") + n(b, "failed"))
            .cmp(&(n(a, "waiting") + n(a, "failed")))
            .then((n(b, "running") + n(b, "queued")).cmp(&(n(a, "running") + n(a, "queued"))))
            .then(a["name"].as_str().cmp(&b["name"].as_str()))
    });
    Ok(Json(serde_json::json!({
        "current": current.map(|c| c.display().to_string()).unwrap_or_default(),
        "projects": projects,
    })))
}

#[derive(Deserialize)]
struct CreateBody {
    title: String,
    request: String,
    /// Omitted → the full `/feature` pipeline.
    #[serde(default)]
    steps: Option<Vec<Stage>>,
    /// The boss's slash command to run (`cook`) instead of `/feature`; must be
    /// enabled for the project. Replaces `steps` with the workflow's fixed steps.
    #[serde(default)]
    workflow: Option<String>,
}

/// Project for creating a task (or picking its workflow): any git repo may start
/// a board (picked in the multi-project view); other routes still require an
/// already-registered project.
fn create_root(rq: &RootQ) -> Result<PathBuf, ApiErr> {
    match rq.root.as_deref().filter(|s| !s.is_empty()) {
        Some(r) => {
            let r = canonical(PathBuf::from(r));
            if !(r.is_dir() && is_git(&r)) {
                return Err((StatusCode::BAD_REQUEST, format!("{} không phải git repo", r.display())));
            }
            Ok(r)
        }
        None => project(rq),
    }
}

async fn api_create(Query(rq): Query<RootQ>, Json(b): Json<CreateBody>) -> ApiRes<Task> {
    let root = create_root(&rq)?;
    if b.title.trim().is_empty() || b.request.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "cần tiêu đề và mô tả".into()));
    }
    let home = dirs::home_dir().unwrap_or_default();
    let wf = match b.workflow.as_deref().filter(|w| !w.trim().is_empty()) {
        Some(name) => Some(workflow::resolve(&home, &root, name).map_err(|e| (StatusCode::BAD_REQUEST, e))?),
        None => None,
    };
    let steps = match (&wf, &b.steps) {
        (Some(_), _) => workflow::steps(),
        (None, Some(s)) => validate_steps(s).map_err(|e| (StatusCode::BAD_REQUEST, e))?,
        (None, None) => full_feature(),
    };
    let _g = guard();
    let now = iso_now();
    let mut t = Task {
        id: next_id(&root),
        title: b.title.trim().into(),
        request: b.request.trim().into(),
        stage: steps[0],
        // Created tasks wait for the boss's Bắt đầu; nothing runs on its own.
        status: Status::Waiting,
        note: Some("Chưa bắt đầu — bấm Bắt đầu để chạy".into()),
        branch: None,
        base: None,
        feature: None,
        preexisting: Vec::new(),
        cost: 0.0,
        created: now.clone(),
        updated: now,
        thread: Vec::new(),
        runs: Vec::new(),
        rounds: Rounds::default(),
        steps,
        asks: Vec::new(),
        paused: false,
        resume: false,
        resume_note: String::new(),
        paused_by: "new".into(),
        workflow: wf,
    };
    t.paused = true;
    save(&root, &mut t).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    register_root(&home, &root);
    Ok(Json(t))
}

async fn api_get(Path(id): Path<String>, Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let t = load(&root, &id).ok_or((StatusCode::NOT_FOUND, format!("task {id} không tồn tại")))?;
    let (docs, focus) = task_docs(&root, &t);
    let docs: Vec<serde_json::Value> = docs.into_iter().map(|(name, content)| serde_json::json!({ "name": name, "content": content })).collect();
    let prog = progress::progress(&root, &t).map(|p| p.counts());
    // "Code theo kế hoạch này" is offered only when the endpoint would accept it.
    let can_continue = continue_blocker(&root, &t).is_none();
    Ok(Json(serde_json::json!({ "task": t, "docs": docs, "focus": focus, "phase": active_phase(&root, &t), "progress": prog, "can_continue": can_continue })))
}

#[derive(Deserialize)]
struct LogQuery {
    run: Option<usize>,
}

async fn api_log(Path(id): Path<String>, Query(q): Query<LogQuery>, Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = project(&rq)?;
    let t = load(&root, &id).ok_or((StatusCode::NOT_FOUND, format!("task {id} không tồn tại")))?;
    let last = t.runs.len().checked_sub(1);
    let idx = q.run.or(last);
    let run = idx.and_then(|i| t.runs.get(i));
    let lines = run.map(|r| log_lines(&task_dir(&root, &id).join("logs").join(&r.log))).unwrap_or_default();
    let running = t.status == Status::Running && idx == last;
    Ok(Json(serde_json::json!({ "run": run, "running": running, "lines": lines })))
}

#[derive(Deserialize)]
struct ReplyBody {
    text: String,
}

async fn api_reply(Path(id): Path<String>, Query(rq): Query<RootQ>, Json(b): Json<ReplyBody>) -> ApiRes<Task> {
    // Replying to a paused step = continue it with that instruction.
    update(&rq, &id, |_, t| reply(t, &b.text))
}

/// The boss's free-text reply: continues a parked step, else answers its gate.
fn reply(t: &mut Task, text: &str) -> Result<(), String> {
    if t.paused { inbox::resume(t, text, "boss") } else { on_reply(t, text) }
}

#[derive(Deserialize)]
struct ApproveQ {
    #[serde(default)]
    by: Option<String>,
    /// `code`: a plan-only task approved at its gate goes on to code it.
    #[serde(default)]
    then: Option<String>,
}

async fn api_approve(Path(id): Path<String>, Query(rq): Query<RootQ>, Query(aq): Query<ApproveQ>, body: Option<Json<ApproveQ>>) -> ApiRes<Task> {
    // The coordinator may approve plans for the boss — never accept/merge.
    let coordinator = aq.by.as_deref() == Some("coordinator");
    let then_code = aq.then.as_deref() == Some("code") || body.is_some_and(|Json(b)| b.then.as_deref() == Some("code"));
    if then_code && coordinator {
        return Err(bad("điều phối chỉ được duyệt kế hoạch, không bật thêm bước code"));
    }
    update(&rq, &id, |root, t| if then_code { approve_and_code(root, t) } else { approve(root, t, coordinator) })
}

/// Add Dev, Test, Report, Acceptance to a plan-only task (other enabled steps
/// stay); returns `old ⇒ new` for the thread.
fn add_code_steps(t: &mut Task) -> Result<String, String> {
    let mut steps = t.steps.clone();
    steps.extend([Stage::Dev, Stage::Test, Stage::Report, Stage::Acceptance].into_iter().filter(|s| !t.has(*s)));
    let new = validate_steps(&steps)?;
    let change = format!("{} ⇒ {}", step_keys(&t.steps), step_keys(&new));
    t.steps = new;
    Ok(change)
}

/// "Duyệt và code luôn": approve the plan of a task without Dev and turn on the
/// coding steps in one go (→ Dev queued).
fn approve_and_code(root: &FsPath, t: &mut Task) -> Result<(), String> {
    if t.paused {
        return Err("việc đang chờ Bắt đầu / Tiếp tục".into());
    }
    if (t.stage, t.status) != (Stage::Approval, Status::Waiting) {
        return Err("task không chờ duyệt kế hoạch".into());
    }
    if t.has(Stage::Dev) {
        return Err("task đã có bước Phát triển, chỉ cần bấm Duyệt".into());
    }
    let change = add_code_steps(t)?;
    t.say("user", &format!("Bạn duyệt kế hoạch và bật code: {change}"));
    approve(root, t, false)
}

/// Why a finished plan-only task cannot be continued into code (None = it can).
fn continue_blocker(root: &FsPath, t: &Task) -> Option<String> {
    if (t.stage, t.status) != (Stage::Done, Status::Done) {
        return Some("chỉ code tiếp được task đã xong".into());
    }
    if t.has(Stage::Dev) {
        return Some("task đã có bước Phát triển".into());
    }
    if !t.has(Stage::Solution) {
        return Some("task không có kế hoạch (bước Giải pháp) để code theo".into());
    }
    let (Some(br), Some(base)) = (&t.branch, &t.base) else {
        return Some("task không có nhánh git chứa kế hoạch".into());
    };
    if git(root, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{br}")]).is_err() {
        return Some(format!("nhánh {br} không còn"));
    }
    if git(root, &["merge-base", "--is-ancestor", br, base]).is_ok() {
        return Some(format!("nhánh {br} đã merge vào {base}"));
    }
    None
}

/// "Code theo kế hoạch này": a finished plan-only task goes back to Dev on its own
/// branch. The worker re-claims the tree like any run (checkout + ACTIVE slot,
/// or waits while another task holds it).
fn continue_plan(root: &FsPath, t: &mut Task, client: &str) -> Result<(), String> {
    if let Some(e) = continue_blocker(root, t) {
        return Err(e);
    }
    // The working tree may now belong to another task (mid-flow on its own
    // branch): reopening would make the worker switch branches under it.
    if let Some(o) = list(root).into_iter().find(|o| o.id != t.id && o.holds_tree()) {
        return Err(format!("{} đang giữ cây làm việc; đợi {} xong rồi bấm lại", o.id, o.id));
    }
    let change = add_code_steps(t)?;
    t.set(Stage::Dev, Status::Queued);
    t.paused = false;
    t.note = None;
    t.say("user", &format!("Code tiếp theo kế hoạch đã duyệt (từ {client})"));
    t.say("system", &format!("Các bước đổi: {change}"));
    Ok(())
}

async fn api_continue(Path(id): Path<String>, Query(rq): Query<RootQ>, headers: HeaderMap) -> ApiRes<Task> {
    let client = client_of(&headers);
    update(&rq, &id, |root, t| continue_plan(root, t, &client))
}

/// Pass the gate the task waits at (plan approval, investigate continue/close,
/// acceptance = merge). The coordinator may only approve plans.
fn approve(root: &FsPath, t: &mut Task, coordinator: bool) -> Result<(), String> {
    // A parked task waits for Bắt đầu / Tiếp tục first: passing its gate now would
    // skip work that never ran (e.g. close a never-started investigate).
    if t.paused {
        return Err("việc đang chờ Bắt đầu / Tiếp tục".into());
    }
    match (t.stage, t.status) {
        (Stage::Approval, Status::Waiting) if coordinator => {
            t.say("coordinator", "Duyệt kế hoạch thay sếp (theo luật uỷ quyền).");
            t.advance(Stage::Approval);
            park_if_finished(root, t);
            Ok(())
        }
        _ if coordinator => Err("điều phối chỉ được duyệt kế hoạch".into()),
        (Stage::Approval, Status::Waiting) => {
            t.say("user", "Duyệt giải pháp.");
            t.advance(Stage::Approval);
            park_if_finished(root, t);
            Ok(())
        }
        (Stage::Investigate, Status::Waiting) => {
            t.advance(Stage::Investigate);
            t.say("user", if t.stage == Stage::Done { "Đóng." } else { "Tiếp tục." });
            park_if_finished(root, t);
            Ok(())
        }
        (Stage::Acceptance, Status::Waiting) => {
            let msg = merge_task_branch(root, t)?;
            t.say("user", "Nghiệm thu.");
            t.say("system", &msg);
            t.set(Stage::Done, Status::Done);
            Ok(())
        }
        _ => Err("task không ở cổng duyệt".into()),
    }
}

async fn api_retry(Path(id): Path<String>, Query(rq): Query<RootQ>, headers: HeaderMap) -> ApiRes<Task> {
    // A retry starts paid work: record who asked for it.
    let client = client_of(&headers);
    update(&rq, &id, |_, t| {
        retry(t)?;
        t.say("system", &format!("Chạy lại bước {} (từ {client}).", t.stage.label()));
        Ok(())
    })
}

fn retry(t: &mut Task) -> Result<(), String> {
    if t.status != Status::Failed {
        return Err("chỉ chạy lại được task lỗi".into());
    }
    t.note = None;
    // Stopped after the PO's last rejection: re-running Review would just reject
    // (and bill) again. Go to the boss if they switched the gate on, else re-plan.
    if t.stage == Stage::Review && t.rounds.solution >= MAX_SOLUTION_ROUNDS {
        if t.has(Stage::Approval) {
            t.set(Stage::Approval, Status::Waiting);
        } else {
            t.rounds.solution = 0;
            t.set(Stage::Solution, Status::Queued);
        }
        return Ok(());
    }
    t.status = Status::Queued;
    Ok(())
}

async fn api_cancel(Path(id): Path<String>, Query(rq): Query<RootQ>) -> ApiRes<Task> {
    update(&rq, &id, |root, t| match t.status {
        Status::Done | Status::Cancelled => Err("task đã kết thúc".into()),
        // The worker kills omp and records the cancellation when the run ends.
        Status::Running => {
            cancels().lock().map(|mut c| c.insert(cancel_key(root, &t.id))).ok();
            t.note = Some("đang huỷ…".into());
            Ok(())
        }
        _ => {
            t.set(Stage::Cancelled, Status::Cancelled);
            t.say("system", "Đã huỷ theo yêu cầu.");
            leave_task_branch(root, t);
            Ok(())
        }
    })
}

// ── Time ────────────────────────────────────────────────────────────────────

/// UTC ISO-8601 (`2026-10-14T15:25:40Z`) without a date crate.
fn iso_now() -> String {
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn task(stage: Stage, status: Status) -> Task {
        Task {
            id: "T-001".into(),
            title: "t".into(),
            request: "r".into(),
            stage,
            status,
            note: None,
            branch: None,
            base: None,
            feature: None,
            preexisting: Vec::new(),
            cost: 0.0,
            created: String::new(),
            updated: String::new(),
            thread: Vec::new(),
            runs: Vec::new(),
            rounds: Rounds::default(),
            steps: full_feature(),
            asks: Vec::new(),
            paused: false,
            resume: false,
            resume_note: String::new(),
            paused_by: String::new(),
            workflow: None,
        }
    }

    fn cook_task(stage: Stage, status: Status) -> Task {
        let mut t = task(stage, status);
        t.steps = workflow::steps();
        t.workflow = Some(workflow::Workflow { name: "cook".into(), path: "/home/u/.omp/agent/commands/cook.md".into(), plan: None });
        t
    }

    #[test]
    fn workflow_task_plans_waits_for_the_boss_runs_then_accepts() {
        use Stage::*;
        let root = std::env::temp_dir().join(format!("ckit-board-wf-{}", std::process::id()));
        let mut t = cook_task(Solution, Status::Running);
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Approval, Status::Waiting), "plan → the boss's gate, always");
        on_reply(&mut t, "bỏ phần export").unwrap();
        assert_eq!((t.stage, t.status), (Solution, Status::Queued), "Yêu cầu sửa re-runs the plan");
        t.set(Approval, Status::Waiting);
        approve(&root, &mut t, false).unwrap();
        assert_eq!((t.stage, t.status), (Dev, Status::Queued));
        let mut failed = t.clone();
        after_run(&mut failed, "fail", None);
        assert_eq!((failed.stage, failed.status), (Dev, Status::Failed));
        retry(&mut failed).unwrap();
        assert_eq!((failed.stage, failed.status), (Dev, Status::Queued), "retry re-runs the run step");
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Acceptance, Status::Waiting));
        assert!(approve_and_code(&root, &mut cook_task(Approval, Status::Waiting)).is_err());
    }

    #[test]
    fn workflow_steps_cannot_be_changed() {
        let mut t = cook_task(Stage::Approval, Status::Waiting);
        let err = change_steps(&mut t, &[Stage::Solution, Stage::Approval, Stage::Dev], "curl/8").unwrap_err();
        assert!(err.contains("/cook") && err.contains("cố định"), "{err}");
        assert_eq!(t.steps, workflow::steps());
        let mut plain = task(Stage::Intake, Status::Waiting);
        change_steps(&mut plain, &[Stage::Intake, Stage::Solution, Stage::Dev], "curl/8").unwrap();
    }

    #[test]
    fn plan_step_that_edits_code_fails_back_to_the_plan_step() {
        let root = std::env::temp_dir().join(format!("ckit-board-wfguard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let g = |args: &[&str]| git(&root, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(root.join("app.php"), "v1\n").unwrap();
        std::fs::write(root.join("draft.txt"), "boss\n").unwrap(); // untracked before the run
        g(&["add", "app.php"]);
        g(&["commit", "-qm", "base"]);
        let before = tree_status(&root, true);
        // Plan dirs are fine (an untracked .omp/ is listed file by file, not as `.omp/`).
        std::fs::create_dir_all(root.join(".omp/plans")).unwrap();
        std::fs::write(root.join(".omp/plans/cook-1.md"), "# plan\n").unwrap();
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::write(root.join("agents/STATE.md"), "x\n").unwrap();
        let mut ok = cook_task(Stage::Solution, Status::Running);
        after_run(&mut ok, "pass", None);
        guard_tree(&mut ok, true, &before, &tree_status(&root, true));
        assert_eq!((ok.stage, ok.status, ok.note.clone()), (Stage::Approval, Status::Waiting, None));

        std::fs::write(root.join("app.php"), "v2\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/new.php"), "<?php\n").unwrap();
        let mut bad = cook_task(Stage::Solution, Status::Running);
        after_run(&mut bad, "pass", None);
        guard_tree(&mut bad, true, &before, &tree_status(&root, true));
        assert_eq!((bad.stage, bad.status), (Stage::Solution, Status::Failed));
        assert_eq!(bad.note.as_deref(), Some("Bước kế hoạch đã sửa code: app.php, src/new.php"));
        assert_eq!(std::fs::read_to_string(root.join("app.php")).unwrap(), "v2\n", "the change is left for the boss to see");
        retry(&mut bad).unwrap();
        assert_eq!((bad.stage, bad.status), (Stage::Solution, Status::Queued));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn run_step_continues_the_plan_session_with_the_run_prompt() {
        let root = std::env::temp_dir().join(format!("ckit-board-wfargs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = task_dir(&root, "T-001");
        let has = |args: &[std::ffi::OsString], a: &str| args.iter().any(|x| x == a);
        let mut t = cook_task(Stage::Dev, Status::Running);
        assert!(!has(&stage_args(&root, &t, &dir), "--continue"), "no plan session yet → fresh run");
        std::fs::create_dir_all(dir.join("sessions/x")).unwrap();
        std::fs::write(dir.join("sessions/x/1.jsonl"), "{}").unwrap();
        let args = stage_args(&root, &t, &dir);
        assert!(has(&args, "--continue"), "{args:?}");
        let sd = args.iter().position(|a| a == "--session-dir").unwrap();
        assert_eq!(args[sd + 1], dir.join("sessions").as_os_str(), "the plan step's session dir");
        t.stage = Stage::Solution;
        assert!(!has(&stage_args(&root, &t, &dir), "--continue"), "the plan starts fresh");
        assert!(!has(&stage_args(&root, &task(Stage::Dev, Status::Running), &dir), "--continue"), "/feature Dev unchanged");

        t.request = "#123 sửa form".into();
        let plan = build_prompt(&root, &t, &dir).unwrap();
        assert!(plan.contains("/home/u/.omp/agent/commands/cook.md") && plan.contains("#123 sửa form") && plan.contains("ONLY its planning part"));
        assert!(!plan.contains("{{") && !plan.contains("feature/SKILL.md"), "no /feature team prompt");
        t.stage = Stage::Dev;
        t.workflow.as_mut().unwrap().plan = Some(".omp/plans/cook-1.md".into());
        let run = build_prompt(&root, &t, &dir).unwrap();
        assert!(run.contains("approved the plan") && run.contains(".omp/plans/cook-1.md") && !run.contains("{{"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_runs_must_stay_on_the_task_branch_and_the_plan_must_not_commit() {
        let root = std::env::temp_dir().join(format!("ckit-board-wfgit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let g = |args: &[&str]| git(&root, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(root.join("app.txt"), "v1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-qm", "base"]);
        let mut t = cook_task(Stage::Solution, Status::Running);
        checkout_task_branch(&root, &mut t).unwrap();
        let commit = |line: &str| {
            std::fs::write(root.join("app.txt"), line).unwrap();
            g(&["commit", "-qam", line]);
        };
        let check = |t: &mut Task, ran: Stage, pos: &GitPos| {
            let (b, moved, n) = pos.since(&root);
            guard_git(t, ran, b.as_deref(), moved, n);
        };

        // Clean plan run: passes to Approval.
        let pos = GitPos::take(&root);
        let mut ok = t.clone();
        after_run(&mut ok, "pass", None);
        check(&mut ok, Stage::Solution, &pos);
        assert_eq!((ok.stage, ok.status), (Stage::Approval, Status::Waiting));

        // Plan run that committed on the task branch.
        let pos = GitPos::take(&root);
        commit("plan-commit\n");
        let mut c = t.clone();
        after_run(&mut c, "pass", None);
        check(&mut c, Stage::Solution, &pos);
        assert_eq!((c.stage, c.status), (Stage::Solution, Status::Failed));
        assert_eq!(c.note.as_deref(), Some("Bước kế hoạch đã commit 1 commit"));

        // Plan run that made its own branch (`/cook` branch step) and committed there.
        let pos = GitPos::take(&root);
        g(&["checkout", "-qb", "feature/cook-1"]);
        commit("on-feature\n");
        let mut b = t.clone();
        b.note = Some("Bước kế hoạch đã sửa code: src/a.php".into());
        b.status = Status::Failed;
        check(&mut b, Stage::Solution, &pos);
        assert_eq!((b.stage, b.status), (Stage::Solution, Status::Failed));
        assert_eq!(
            b.note.as_deref(),
            Some("Bước kế hoạch đã sửa code: src/a.php; Bước kế hoạch đã đổi nhánh sang feature/cook-1 (phải ở board/t-001); Bước kế hoạch đã commit 1 commit")
        );
        g(&["checkout", "-q", "board/t-001"]);

        // Run step: commits on the task branch are its job…
        let pos = GitPos::take(&root);
        commit("run-commit\n");
        let mut run = t.clone();
        run.set(Stage::Dev, Status::Running);
        after_run(&mut run, "pass", None);
        check(&mut run, Stage::Dev, &pos);
        assert_eq!((run.stage, run.status), (Stage::Acceptance, Status::Waiting));
        // …switching branches is not: back to Dev (retrying the Acceptance gate would spawn omp).
        let pos = GitPos::take(&root);
        g(&["checkout", "-qb", "feature/x"]);
        let mut sw = t.clone();
        sw.set(Stage::Dev, Status::Running);
        after_run(&mut sw, "pass", None);
        check(&mut sw, Stage::Dev, &pos);
        assert_eq!((sw.stage, sw.status), (Stage::Dev, Status::Failed));
        assert_eq!(sw.note.as_deref(), Some("Bước thực hiện đã đổi nhánh sang feature/x (phải ở board/t-001)"));
        retry(&mut sw).unwrap();
        assert_eq!((sw.stage, sw.status), (Stage::Dev, Status::Queued));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn approval_shows_the_reported_plan_file() {
        let root = std::env::temp_dir().join(format!("ckit-board-wfdocs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".omp/plans")).unwrap();
        std::fs::write(root.join(".omp/plans/cook-1.md"), "# Kế hoạch\n").unwrap();
        let mut t = cook_task(Stage::Solution, Status::Running);
        record_plan(&root, &mut t, Some(".omp/plans/cook-1.md"));
        after_run(&mut t, "pass", None);
        let (docs, focus) = task_docs(&root, &t);
        assert_eq!(docs, [("PLAN (/cook)".to_string(), "# Kế hoạch\n".to_string())]);
        assert_eq!(focus.as_deref(), Some("PLAN (/cook)"));
        record_plan(&root, &mut t, Some("../x.md"));
        assert_eq!(t.workflow.as_ref().unwrap().plan, None);
        assert!(t.thread.last().unwrap().text.contains("../x.md"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn happy_path_reaches_both_human_gates() {
        let mut t = task(Stage::Intake, Status::Running);
        for (verdict, stage, status) in [
            ("pass", Stage::Solution, Status::Queued),
            ("pass", Stage::Review, Status::Queued),
            ("pass", Stage::Approval, Status::Waiting),
        ] {
            after_run(&mut t, verdict, None);
            assert_eq!((t.stage, t.status), (stage, status));
        }
        t.set(Stage::Dev, Status::Running);
        for (stage, status) in [(Stage::Test, Status::Queued), (Stage::Report, Status::Queued), (Stage::Acceptance, Status::Waiting)] {
            after_run(&mut t, "pass", None);
            assert_eq!((t.stage, t.status), (stage, status));
        }
    }

    #[test]
    fn intake_questions_wait_then_reply_requeues_intake() {
        let mut t = task(Stage::Intake, Status::Running);
        after_run(&mut t, "need_user", None);
        assert_eq!((t.stage, t.status), (Stage::Intake, Status::Waiting));
        on_reply(&mut t, "dùng MySQL").unwrap();
        assert_eq!((t.stage, t.status), (Stage::Intake, Status::Queued));
        assert!(on_reply(&mut t, "again").is_err(), "reply only while waiting");
    }

    #[test]
    fn review_rejections_are_bounded() {
        let mut t = task(Stage::Review, Status::Running);
        for _ in 1..MAX_SOLUTION_ROUNDS {
            after_run(&mut t, "fail", None);
            assert_eq!(t.stage, Stage::Solution);
            t.set(Stage::Review, Status::Running);
        }
        after_run(&mut t, "fail", None);
        assert_eq!((t.stage, t.status), (Stage::Approval, Status::Waiting));
    }

    #[test]
    fn shipped_phase_loops_to_next_phase_plan_else_reports() {
        let mut t = task(Stage::Test, Status::Running);
        t.rounds.solution = 2;
        after_run(&mut t, "pass", Some("next_phase"));
        assert_eq!((t.stage, t.status, t.rounds.solution), (Stage::Solution, Status::Queued, 0));
        let mut t = task(Stage::Test, Status::Running);
        after_run(&mut t, "pass", Some("done"));
        assert_eq!(t.stage, Stage::Report);
        let mut t = task(Stage::Test, Status::Running);
        after_run(&mut t, "fail", None);
        assert_eq!(t.stage, Stage::Report, "ship converged already; Leader reports the leftover FAILs");
    }

    #[test]
    fn question_after_delivery_returns_to_accept_gate() {
        let mut t = task(Stage::Intake, Status::Running);
        t.branch = Some("board/t-001".into());
        after_run(&mut t, "pass", Some("acceptance"));
        assert_eq!((t.stage, t.status), (Stage::Acceptance, Status::Waiting));
        let mut t = task(Stage::Intake, Status::Running);
        after_run(&mut t, "pass", Some("acceptance"));
        assert_eq!(t.stage, Stage::Solution, "nothing delivered yet → no accept gate to return to");
    }

    #[test]
    fn gate_replies_route_back() {
        let mut t = task(Stage::Approval, Status::Waiting);
        t.rounds.solution = 2;
        on_reply(&mut t, "đổi cách làm").unwrap();
        assert_eq!((t.stage, t.status, t.rounds.solution), (Stage::Solution, Status::Queued, 0));
        let mut t = task(Stage::Acceptance, Status::Waiting);
        on_reply(&mut t, "sửa nút").unwrap();
        assert_eq!(t.stage, Stage::Intake);
    }

    #[test]
    fn disabled_steps_are_skipped_and_last_step_finishes() {
        use Stage::*;
        let mut t = task(Dev, Status::Running);
        t.steps = vec![Dev, Test, Report];
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Test, Status::Queued));
        after_run(&mut t, "pass", Some("next_phase"));
        assert_eq!(t.stage, Report, "no planning step → no phase loop");
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Done, Status::Done), "no acceptance → done, unmerged");

        let mut t = task(Review, Status::Running);
        t.steps = vec![Intake, Solution, Review, Dev];
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Dev, Status::Queued), "approval gate off");
    }

    #[test]
    fn investigate_answers_then_waits_for_more_or_moves_on() {
        use Stage::*;
        let mut t = task(Investigate, Status::Running);
        t.steps = vec![Investigate, Dev, Acceptance];
        after_run(&mut t, "pass", None);
        assert_eq!((t.stage, t.status), (Investigate, Status::Waiting));
        on_reply(&mut t, "còn chỗ nào gọi hàm này?").unwrap();
        assert_eq!((t.stage, t.status), (Investigate, Status::Queued));
        t.status = Status::Waiting;
        t.advance(Investigate);
        assert_eq!((t.stage, t.status), (Dev, Status::Queued));
        t.set(Acceptance, Status::Waiting);
        on_reply(&mut t, "đổi màu nút").unwrap();
        assert_eq!(t.stage, Dev, "no intake step → change request goes to Dev");
        assert!(!task(Investigate, Status::Waiting).holds_tree(), "read-only task never takes the tree");
    }

    #[test]
    fn step_sets_are_normalized_and_dependencies_enforced() {
        use Stage::*;
        assert_eq!(validate_steps(&[Acceptance, Dev, Dev]).unwrap(), vec![Dev, Acceptance]);
        assert!(validate_steps(&[Review, Dev]).unwrap_err().contains("Kế hoạch"));
        assert!(validate_steps(&[Test]).is_err());
        assert!(validate_steps(&[Acceptance]).is_err(), "gates alone do nothing");
        assert!(validate_steps(&[]).is_err());
        assert!(validate_steps(&[Done]).is_err());
    }

    #[test]
    fn rejected_plan_never_reaches_dev_without_the_boss() {
        use Stage::*;
        let mut t = task(Review, Status::Running);
        t.steps = vec![Solution, Review, Dev];
        t.rounds.solution = MAX_SOLUTION_ROUNDS - 1;
        after_run(&mut t, "fail", None);
        assert_eq!((t.stage, t.status), (Review, Status::Failed));
        let mut again = t.clone();
        retry(&mut again).unwrap();
        assert_eq!((again.stage, again.status, again.rounds.solution), (Solution, Status::Queued, 0), "re-plan, not re-reject");
        t.steps.push(Approval);
        t.steps = validate_steps(&t.steps).unwrap();
        retry(&mut t).unwrap();
        assert_eq!((t.stage, t.status), (Approval, Status::Waiting), "gate switched on → boss decides");
    }

    #[test]
    fn opening_the_dashboard_parks_unfinished_work() {
        let root = std::env::temp_dir().join(format!("ckit-park-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = task(Stage::Dev, Status::Running);
        t.runs.push(Run { stage: Stage::Dev, n: 1, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: "dev-1.jsonl".into() });
        park_on_open(&root, &mut t);
        assert_eq!((t.status, t.paused, t.paused_by.as_str()), (Status::Waiting, true, "restart"));
        assert_eq!(t.runs[0].verdict.as_deref(), Some("interrupted"));
        assert!(!t.resume, "no omp session was written → nothing to continue");
        assert!(inbox::resume(&mut t, "", "coordinator").is_err(), "only the boss starts parked work");
        inbox::resume(&mut t, "", "boss").unwrap();
        assert_eq!(t.status, Status::Queued);

        let mut q = task(Stage::Intake, Status::Queued);
        park_on_open(&root, &mut q);
        assert_eq!((q.status, q.paused, q.resume), (Status::Waiting, true, false));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_verdict_fails_stage_in_place() {
        let mut t = task(Stage::Solution, Status::Running);
        after_run(&mut t, "fail", None);
        assert_eq!((t.stage, t.status), (Stage::Solution, Status::Failed));
    }

    #[test]
    fn one_task_owns_the_tree_and_gets_its_feature_scaffolded() {
        let root = std::env::temp_dir().join(format!("ckit-board-slot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Not a git repo → checkout is a no-op, so only the tree lock decides.
        let mut holder = task(Stage::Approval, Status::Waiting);
        holder.runs.push(Run { stage: Stage::Intake, n: 1, started: String::new(), ended: None, exit: None, cost: 0.0, verdict: None, log: String::new() });
        save(&root, &mut holder).unwrap();
        let mut next = task(Stage::Intake, Status::Queued);
        next.id = "T-002".into();
        save(&root, &mut next).unwrap();
        assert!(claim_next(&root).is_none(), "every stage waits: ACTIVE.md points at one feature");
        assert!(load(&root, "T-002").unwrap().note.unwrap().contains("T-001"));
        // Holder finished → the next task starts and gets its own /feature tree.
        holder.set(Stage::Done, Status::Done);
        save(&root, &mut holder).unwrap();
        let claimed = claim_next(&root).unwrap();
        assert_eq!((claimed.id.as_str(), claimed.status), ("T-002", Status::Running));
        assert_eq!(claimed.feature.as_deref(), Some("board-t-002"));
        assert!(root.join("agents/planning/board-t-002/ROADMAP.md").is_file());
        let active = std::fs::read_to_string(root.join("agents/planning/ACTIVE.md")).unwrap();
        assert!(active.lines().any(|l| l.trim() == "board-t-002"), "{active}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn accept_keeps_users_active_feature_and_refuses_unrelated_wip() {
        let root = std::env::temp_dir().join(format!("ckit-board-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("agents/planning")).unwrap();
        let g = |args: &[&str]| git(&root, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(root.join("agents/planning/ACTIVE.md"), "user-feat\n").unwrap();
        std::fs::write(root.join("app.txt"), "v1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-qm", "base"]);
        // The user's own untracked draft under docs/ before the task starts.
        std::fs::create_dir_all(root.join("docs/knowledge")).unwrap();
        std::fs::write(root.join("docs/knowledge/draft.md"), "mine\n").unwrap();
        let mut t = task(Stage::Acceptance, Status::Waiting);
        checkout_task_branch(&root, &mut t).unwrap();
        // What a /feature run leaves on the branch: its own ACTIVE slug + config + a code commit.
        std::fs::write(root.join("agents/planning/ACTIVE.md"), "board-t-001\n").unwrap();
        std::fs::write(root.join("agents/planning/config.json"), "{\"active_feature\":\"board-t-001\"}\n").unwrap();
        std::fs::write(root.join("app.txt"), "v2\n").unwrap();
        g(&["add", "app.txt"]);
        g(&["commit", "-qm", "feat"]);
        // User WIP outside /feature paths blocks the merge instead of being swept in.
        std::fs::write(root.join("app.txt"), "user wip\n").unwrap();
        let err = merge_task_branch(&root, &t).unwrap_err();
        assert!(err.contains("app.txt"), "{err}");
        g(&["checkout", "--", "app.txt"]);
        merge_task_branch(&root, &t).unwrap();
        assert_eq!(current_branch(&root).as_deref(), Some("main"));
        assert_eq!(std::fs::read_to_string(root.join("agents/planning/ACTIVE.md")).unwrap(), "user-feat\n");
        assert!(!root.join("agents/planning/config.json").exists(), "absent on base → stays absent");
        let tracked = git(&root, &["ls-files", "docs/knowledge/draft.md"]).unwrap();
        assert!(tracked.is_empty(), "user's pre-task draft must not be committed/merged");
        assert_eq!(std::fs::read_to_string(root.join("docs/knowledge/draft.md")).unwrap(), "mine\n", "draft left untouched");
        assert_eq!(std::fs::read_to_string(root.join("app.txt")).unwrap(), "v2\n", "the delivered code is merged");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn board_state_is_git_excluded_without_any_gitignore() {
        let root = std::env::temp_dir().join(format!("ckit-board-excl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".cache/ckit/board/T-001")).unwrap();
        std::fs::write(root.join(".cache/ckit/board/T-001/task.json"), "{}").unwrap();
        git(&root, &["init", "-q"]).unwrap();
        exclude_board_from_git(&root);
        exclude_board_from_git(&root); // idempotent
        let excl = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert_eq!(excl.matches(".cache/ckit/").count(), 1);
        // Ignore the machine's global excludes so only our entry is measured.
        let st = git(&root, &["-c", "core.excludesFile=/dev/null", "status", "--porcelain", "--untracked-files=all"]).unwrap();
        assert!(!st.contains(".cache"), "board state visible to git: {st}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn iso_now_shape() {
        let s = iso_now();
        assert_eq!(s.len(), 20);
        assert!(s.starts_with("20") && s.ends_with('Z'));
    }

    #[test]
    fn log_digest_extracts_text_tools_and_cost() {
        let dir = std::env::temp_dir().join(format!("ckit-board-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.jsonl");
        std::fs::write(
            &p,
            concat!(
                "{\"type\":\"message_update\",\"x\":1}\n",
                "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"hi\"},{\"type\":\"toolCall\",\"name\":\"read\",\"arguments\":{\"path\":\"a\"}}],\"usage\":{\"cost\":{\"input\":0.5,\"output\":0.25,\"total\":0.75}}}}\n",
                "{\"type\":\"message_end\",\"message\":{\"role\":\"toolResult\",\"toolName\":\"bash\",\"isError\":true,\"content\":[{\"type\":\"text\",\"text\":\"boom\"}]}}\n",
                "panic: stderr line\n",
            ),
        )
        .unwrap();
        let lines = log_lines(&p);
        let kinds: Vec<&str> = lines.iter().map(|l| l["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["text", "tool", "error", "error"]);
        assert!((log_cost(&p) - 0.75).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn each_run_is_billed_from_its_own_log_once() {
        let root = std::env::temp_dir().join(format!("ckit-board-cost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = task(Stage::Dev, Status::Running);
        let logs = task_dir(&root, &t.id).join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::create_dir_all(ask::ask_dir(&root, &t.id)).unwrap();
        let end = |c: f64| format!("{{\"type\":\"message_end\",\"message\":{{\"role\":\"assistant\",\"usage\":{{\"cost\":{{\"input\":0.01,\"total\":{c}}}}}}}}}\n");
        let header = "{\"type\":\"session\",\"id\":\"s-1\"}\n";
        std::fs::write(logs.join("dev-1.jsonl"), format!("{header}{}{}", end(0.5), end(0.25))).unwrap();
        // `--continue` resumes session s-1: the new log carries only its own turns
        // (agent_end lists the whole history, priced turns included: not billed again).
        let history = "{\"type\":\"agent_end\",\"messages\":[{\"role\":\"assistant\",\"usage\":{\"cost\":{\"total\":0.75}}}]}\n";
        std::fs::write(logs.join("dev-2.jsonl"), format!("{header}{}{history}", end(0.1))).unwrap();
        std::fs::write(ask::ask_dir(&root, &t.id).join("po-1.jsonl"), end(0.05)).unwrap();
        let run = |n: u32| Run { stage: Stage::Dev, n, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: format!("dev-{n}.jsonl") };
        t.runs = vec![run(1), run(2)];

        // dev-1's worker finishes after dev-2 was claimed: it closes dev-1, not the last run.
        let c1 = log_cost(&logs.join("dev-1.jsonl"));
        close_run(&root, &mut t, "dev-1.jsonl", Some(0), Some("pass".into()), c1);
        assert!((t.runs[0].cost - 0.75).abs() < 1e-9 && t.runs[0].ended.is_some());
        assert_eq!((t.runs[1].cost, t.runs[1].ended.as_deref()), (0.0, None));
        assert!((t.cost - 0.8).abs() < 1e-9, "runs + PO answer, got {}", t.cost);

        // Restart: the orphaned dev-2 is closed and billed; a run billed from another log is re-priced.
        t.runs[0].cost = 9.0;
        assert!(repair_runs(&root, &mut t));
        assert_eq!(t.runs[1].verdict.as_deref(), Some("interrupted"));
        assert!((t.runs[0].cost - 0.75).abs() < 1e-9 && (t.runs[1].cost - 0.1).abs() < 1e-9);
        assert!((t.cost - 0.9).abs() < 1e-9, "total = 0.75 + 0.1 + 0.05, got {}", t.cost);
        assert!(!repair_runs(&root, &mut t), "nothing left open");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stale_run_is_billed_but_never_moves_the_task() {
        let root = std::env::temp_dir().join(format!("ckit-board-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut t = task(Stage::Dev, Status::Running);
        let run = |n: u32| Run { stage: Stage::Dev, n, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: format!("dev-{n}.jsonl") };
        t.runs = vec![run(1), run(2)];
        let said = t.thread.len();

        // dev-1 (superseded by dev-2) finishes with a pass: closed + billed only.
        assert!(close_if_stale(&root, &mut t, Stage::Dev, "dev-1.jsonl", Some(0), Some("pass".into()), 0.75));
        assert_eq!((t.stage, t.status), (Stage::Dev, Status::Running), "dev-2 keeps the step");
        assert_eq!((t.runs[0].verdict.as_deref(), t.runs[0].cost), (Some("pass"), 0.75));
        assert!(t.runs[1].ended.is_none() && (t.cost - 0.75).abs() < 1e-9);
        assert_eq!(t.thread.len(), said + 1);
        assert_eq!(t.thread.last().unwrap().text, "Bỏ qua kết quả của dev-1 (đã có lượt chạy mới hơn)");

        // The current run goes through; the same run after the task left its step does not.
        assert!(!close_if_stale(&root, &mut t, Stage::Dev, "dev-2.jsonl", Some(0), None, 0.1));
        t.set(Stage::Test, Status::Queued);
        assert!(close_if_stale(&root, &mut t, Stage::Dev, "dev-2.jsonl", Some(0), Some("pass".into()), 0.1));
        assert_eq!((t.stage, t.status), (Stage::Test, Status::Queued));
        assert!((t.cost - 0.85).abs() < 1e-9);
    }

    #[test]
    fn a_finished_run_releases_only_its_own_pid() {
        let dir = std::env::temp_dir().join(format!("ckit-board-pid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("run.pid"), "222").unwrap();
        release_pid(&dir, 111);
        assert_eq!(std::fs::read_to_string(dir.join("run.pid")).unwrap(), "222", "the newer run's pid stays for kill_orphan");
        release_pid(&dir, 222);
        assert!(!dir.join("run.pid").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn each_run_reads_its_own_result_file() {
        let dir = std::env::temp_dir().join(format!("ckit-board-result-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("results")).unwrap();
        assert_eq!(result_file(&dir, "dev-2.jsonl"), dir.join("results").join("dev-2.json"));
        let before = std::time::SystemTime::now() - Duration::from_secs(60);
        std::fs::write(result_file(&dir, "dev-1.jsonl"), r#"{"verdict":"pass","message":"old"}"#).unwrap();
        assert!(read_result(&dir, "dev-2.jsonl", before).is_none(), "another run's result is never read");
        std::fs::write(result_file(&dir, "dev-2.jsonl"), r#"{"verdict":"fail","message":"mine"}"#).unwrap();
        std::fs::write(dir.join("result.json"), r#"{"verdict":"pass","message":"legacy"}"#).unwrap();
        assert_eq!(read_result(&dir, "dev-2.jsonl", before).unwrap().message, "mine", "own file wins");

        // Legacy shared file: only when the run's own file is missing and it was written during the run.
        assert_eq!(read_result(&dir, "dev-3.jsonl", before).unwrap().message, "legacy");
        let later = std::time::SystemTime::now() + Duration::from_secs(60);
        assert!(read_result(&dir, "dev-3.jsonl", later).is_none(), "a legacy file older than the run is stale");
        let _ = std::fs::remove_dir_all(dir);
    }

    fn plan_only(stage: Stage, status: Status) -> Task {
        let mut t = task(stage, status);
        t.steps = vec![Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval];
        t
    }

    #[test]
    fn approve_then_code_adds_the_coding_steps_and_queues_dev() {
        let root = std::env::temp_dir().join(format!("ckit-board-apcode-{}", std::process::id()));
        let mut t = plan_only(Stage::Approval, Status::Waiting);
        approve_and_code(&root, &mut t).unwrap();
        assert_eq!((t.stage, t.status), (Stage::Dev, Status::Queued));
        assert_eq!(t.steps, [Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval, Stage::Dev, Stage::Test, Stage::Report, Stage::Acceptance]);
        assert!(t.thread.iter().any(|m| m.text.starts_with("Bạn duyệt kế hoạch và bật code: intake → solution → review → approval ⇒")));
        // Other enabled steps stay (an investigate before the plan).
        let mut t = plan_only(Stage::Approval, Status::Waiting);
        t.steps.insert(0, Stage::Investigate);
        approve_and_code(&root, &mut t).unwrap();
        assert_eq!((t.steps[0], t.steps.len(), t.stage), (Stage::Investigate, 9, Stage::Dev));
        let mut has_dev = task(Stage::Approval, Status::Waiting);
        assert!(approve_and_code(&root, &mut has_dev).is_err(), "a task with Dev uses plain approve");
        let mut not_gate = plan_only(Stage::Solution, Status::Running);
        assert!(approve_and_code(&root, &mut not_gate).is_err());
        assert_eq!(not_gate.steps.len(), 4, "a refused approve changes nothing");
        let mut parked = plan_only(Stage::Approval, Status::Waiting);
        parked.paused = true;
        assert!(approve_and_code(&root, &mut parked).is_err());
        assert_eq!(parked.steps.len(), 4);
    }

    #[test]
    fn continue_refuses_tasks_it_cannot_code() {
        let root = std::env::temp_dir().join(format!("ckit-board-cont-refuse-{}", std::process::id()));
        let mut running = plan_only(Stage::Review, Status::Running);
        assert!(continue_plan(&root, &mut running, "t").unwrap_err().contains("đã xong"));
        let mut has_dev = task(Stage::Done, Status::Done);
        assert!(continue_plan(&root, &mut has_dev, "t").unwrap_err().contains("Phát triển"));
        let mut no_plan = task(Stage::Done, Status::Done);
        no_plan.steps = vec![Stage::Investigate];
        assert!(continue_plan(&root, &mut no_plan, "t").unwrap_err().contains("kế hoạch"));
        let mut no_branch = plan_only(Stage::Done, Status::Done);
        assert!(continue_plan(&root, &mut no_branch, "t").unwrap_err().contains("nhánh"));
        assert_eq!((no_branch.stage, no_branch.steps.len()), (Stage::Done, 4), "nothing changed");
    }

    #[test]
    fn continue_reclaims_the_plan_branch_and_its_active_slot() {
        let root = std::env::temp_dir().join(format!("ckit-board-cont-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let g = |args: &[&str]| git(&root, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(root.join("app.txt"), "v1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-qm", "base"]);
        exclude_board_from_git(&root);
        // A plan-only task planned on its branch, ACTIVE cleared, then finished (parked on main).
        let mut t = plan_only(Stage::Approval, Status::Waiting);
        checkout_task_branch(&root, &mut t).unwrap();
        ensure_feature(&root, &mut t).unwrap();
        std::fs::write(root.join("agents/planning/ACTIVE.md"), "\n").unwrap();
        commit_leftovers(&root, &t, "planning").unwrap();
        approve(&root, &mut t, false).unwrap();
        assert_eq!((t.stage, t.status), (Stage::Done, Status::Done));
        assert_eq!(current_branch(&root).as_deref(), Some("main"), "parked on base");
        assert!(continue_blocker(&root, &t).is_none());
        // Another task mid-flow on its own branch owns the tree: no reopen.
        let mut other = plan_only(Stage::Approval, Status::Waiting);
        other.id = "T-002".into();
        other.branch = Some("board/t-002".into());
        save(&root, &mut other).unwrap();
        let err = continue_plan(&root, &mut t.clone(), "t").unwrap_err();
        assert!(err.contains("T-002"), "{err}");
        std::fs::remove_dir_all(board_dir(&root).join("T-002")).unwrap();

        continue_plan(&root, &mut t, "curl/8").unwrap();
        assert_eq!((t.stage, t.status, t.paused), (Stage::Dev, Status::Queued, false));
        assert_eq!(t.steps, [Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval, Stage::Dev, Stage::Test, Stage::Report, Stage::Acceptance]);
        assert!(t.thread.iter().any(|m| m.text == "Code tiếp theo kế hoạch đã duyệt (từ curl/8)"));
        assert!(t.holds_tree(), "leaves the Xong column and owns the tree again");
        save(&root, &mut t).unwrap();
        let claimed = claim_next(&root).unwrap();
        assert_eq!((claimed.stage, claimed.status), (Stage::Dev, Status::Running));
        assert_eq!(current_branch(&root).as_deref(), Some("board/t-001"));
        let active = std::fs::read_to_string(root.join("agents/planning/ACTIVE.md")).unwrap();
        assert!(active.lines().any(|l| l.trim() == "board-t-001"), "{active}");
        // Dev sees the approved plan: same feature tree, non-auto prompt variant.
        assert!(feature_dir(&root, &claimed).is_some_and(|d| d.ends_with("agents/planning/board-t-001")));

        // Merged into base → nothing left to continue from.
        let mut done = claimed.clone();
        done.set(Stage::Done, Status::Done);
        done.steps = vec![Stage::Intake, Stage::Solution, Stage::Review, Stage::Approval];
        commit_leftovers(&root, &done, "planning").unwrap();
        g(&["switch", "main"]);
        g(&["merge", "-q", "--no-ff", "board/t-001", "-m", "m"]);
        assert!(continue_plan(&root, &mut done, "t").unwrap_err().contains("đã merge"));
        g(&["branch", "-D", "board/t-001"]);
        assert!(continue_plan(&root, &mut done, "t").unwrap_err().contains("không còn"));
        let _ = std::fs::remove_dir_all(root);
    }
}
