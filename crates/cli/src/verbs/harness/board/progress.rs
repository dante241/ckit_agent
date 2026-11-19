//! Task progress for the drawer/card: the `/feature` phases of a board task and
//! the sub-tasks of its active phase. Read-only; three sources:
//!   - `ROADMAP.md`   phase list + `[ ]/[~]/[x]` state,
//!   - `M<x>-NN-PLAN.md` the phase's tasks (`- [ ] T1: …`, `- [x] T1 (sha): …`,
//!     `- [ ] T1 — …`, or a `| T1 | … |` table row; tags `[file: …] [depends: …] [AC: …]`),
//!   - engine `state.json` live status (pending/in_progress/done/blocked, retries) —
//!     only while THIS task owns the tree and a dev/test run of the phase is current.
//! PLAN/ROADMAP are AI-written: parsing is lenient, unknown lines are skipped.
use super::*;

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Phase {
    pub(super) id: String,
    name: String,
    /// done | active | todo
    state: &'static str,
    ac_pass: usize,
    ac_total: usize,
    /// ACs graded with a note (`PASS (AC chấm lại)`) or not passed: the boss should look.
    ac_notes: Vec<AcNote>,
}

/// One AC row of a VERIFICATION matrix worth showing: re-graded / noted / not passed.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub(super) struct AcNote {
    pub id: String,
    /// PASS | FAIL | NEEDS-CONFIRM
    pub status: &'static str,
    pub note: String,
    /// The row's source cell (`REQUIREMENTS + PO Q-4`), for who re-graded it.
    #[serde(skip)]
    pub source: String,
    /// The cell after the verdict (command / output / file:line).
    #[serde(skip)]
    pub evidence: String,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub(super) struct SubTask {
    pub(super) key: String,
    /// First sentence of the task text, ≤ 120 chars.
    pub(super) title: String,
    /// The whole task text (description + tags).
    pub(super) detail: String,
    files: String,
    depends: String,
    pub(super) ac: String,
    /// todo | in_progress | done | blocked
    status: &'static str,
    retries: u64,
    note: String,
    /// Commit the PLAN line names (`T1 (8d2143768e): …`).
    #[serde(skip)]
    pub(super) sha: Option<String>,
}

#[derive(Serialize, Debug)]
pub(super) struct Progress {
    phases: Vec<Phase>,
    phase: Option<String>,
    tasks: Vec<SubTask>,
    done: usize,
    total: usize,
    /// Task statuses come from the running engine (vs PLAN checkboxes).
    live: bool,
}

impl Progress {
    pub(super) fn counts(&self) -> serde_json::Value {
        serde_json::json!({ "done": self.done, "total": self.total })
    }
}

pub(super) fn progress(root: &FsPath, t: &Task) -> Option<Progress> {
    let fdir = feature_dir(root, t)?;
    let phase = active_phase(root, t);
    let roadmap = std::fs::read_to_string(fdir.join("ROADMAP.md")).unwrap_or_default();
    let mut phases = parse_roadmap(&roadmap);
    for p in &mut phases {
        if p.state == "todo" && phase.as_deref() == Some(p.id.as_str()) {
            p.state = "active";
        }
        if let Some(d) = phase_dir(&fdir, &p.id) {
            grade(p, &d);
        }
    }
    let mut tasks = phase
        .as_deref()
        .and_then(|id| phase_dir(&fdir, id).map(|d| parse_plan(&read_glob(&d, "-PLAN"))))
        .unwrap_or_default();
    let phase_done = phases.iter().any(|p| Some(&p.id) == phase.as_ref() && p.state == "done");
    let live = engine_applies(root, t) && merge_engine(&mut tasks, &read_engine(root));
    if phase_done {
        tasks.iter_mut().for_each(|s| s.status = "done");
    }
    let done = tasks.iter().filter(|s| s.status == "done").count();
    let total = tasks.len();
    Some(Progress { phases, phase, tasks, done, total, live })
}

/// The engine state is per project: trust it only while this task owns the tree
/// and its latest dev/test run (the phase being executed) is the newest one.
fn engine_applies(root: &FsPath, t: &Task) -> bool {
    if !t.holds_tree() || !matches!(t.stage, Stage::Dev | Stage::Test | Stage::Security | Stage::Report) {
        return false;
    }
    let Some(run) = t.runs.iter().rev().find(|r| r.stage == Stage::Dev) else { return false };
    let updated = read_engine(root).get("updatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    // ISO-8601 UTC strings compare chronologically (engine adds ms; compare to seconds).
    updated.get(..19).unwrap_or("") >= run.started.get(..19).unwrap_or("~")
}

fn read_engine(root: &FsPath) -> serde_json::Value {
    std::fs::read_to_string(root.join(".cache/ckit/engine/state.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Overlay engine statuses onto PLAN tasks (by key, else by title). When PLAN
/// parsing found nothing, the engine's own tasks are the list. Returns whether
/// the engine contributed anything.
fn merge_engine(tasks: &mut Vec<SubTask>, engine: &serde_json::Value) -> bool {
    let eng: Vec<&serde_json::Value> = engine
        .get("slices")
        .and_then(|s| s.as_array())
        .into_iter()
        .flatten()
        .flat_map(|s| s.get("tasks").and_then(|t| t.as_array()).into_iter().flatten())
        .collect();
    if eng.is_empty() {
        return false;
    }
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let status = |v: &serde_json::Value| match v.get("status").and_then(|x| x.as_str()) {
        Some("in_progress") => "in_progress",
        Some("done") => "done",
        Some("blocked") => "blocked",
        _ => "todo",
    };
    for e in &eng {
        let (key, title) = (s(e, "key"), s(e, "title"));
        let hit = tasks.iter_mut().find(|t| (!key.is_empty() && t.key.eq_ignore_ascii_case(&key)) || (!title.is_empty() && (t.title == title || t.detail.starts_with(&title))));
        let retries = e.get("retries").and_then(|x| x.as_u64()).unwrap_or(0);
        match hit {
            Some(t) => {
                t.status = status(e);
                t.retries = retries;
                t.note = s(e, "note");
            }
            None => tasks.push(SubTask {
                key: if key.is_empty() { s(e, "id") } else { key },
                title: short_title(&title),
                detail: title,
                files: e.get("files").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|f| f.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                depends: e.get("depends").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|f| f.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                ac: String::new(),
                status: status(e),
                retries,
                note: s(e, "note"),
                sha: None,
            }),
        }
    }
    true
}

pub(super) fn phase_dir(fdir: &FsPath, id: &str) -> Option<PathBuf> {
    std::fs::read_dir(fdir.join("phases")).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.is_dir() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == id || n.starts_with(&format!("{id}-")))
    })
}

/// Concatenated `*.md` files in `dir` whose stem contains `needle`, sorted.
pub(super) fn read_glob(dir: &FsPath, needle: &str) -> String {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md") && p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.contains(needle)))
        .collect();
    files.sort();
    files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect::<Vec<_>>().join("\n")
}

/// Text after the bold phase title: `— goal · cần: M0` → "goal · cần: M0".
fn roadmap_goal(md: &str, id: &str) -> String {
    md.lines()
        .filter_map(|l| checkbox(l).map(|(_, r)| r))
        .find(|r| r.trim_start().strip_prefix("**").is_some_and(|b| b.split_whitespace().next().map(|w| w.trim_end_matches(':')) == Some(id)))
        .and_then(|r| r.trim_start().strip_prefix("**").and_then(|b| b.split_once("**")).map(|(_, after)| after))
        .map(|g| g.trim().trim_start_matches(['—', '-', ':']).trim().to_string())
        .unwrap_or_default()
}

#[derive(Serialize, Debug)]
pub(super) struct StepChip {
    key: &'static str,
    /// done | now | todo
    state: &'static str,
}

#[derive(Serialize, Debug)]
pub(super) struct PhaseBoard {
    #[serde(flatten)]
    phase: Phase,
    goal: String,
    /// The phase has a PLAN (tasks exist); future phases get theirs when planned.
    planned: bool,
    steps: Vec<StepChip>,
    tasks: Vec<SubTaskRow>,
    /// Indexes into `task.runs` of the runs that worked on this phase.
    runs: Vec<usize>,
    /// First run start / last run end (null while one still runs).
    started: Option<String>,
    ended: Option<String>,
    cost: f64,
    /// Plan reviews sent back (reviewer `fail`).
    rounds_back: usize,
    /// Before/after rows of a VERIFICATION or MEASURE table (`| Chỉ số | Trước | Sau |`).
    result: Vec<BeforeAfter>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct BeforeAfter {
    label: String,
    before: String,
    after: String,
}

#[derive(Serialize, Debug)]
pub(super) struct SubTaskRow {
    #[serde(flatten)]
    task: SubTask,
    /// Short hash of the commit the PLAN line names, else of a branch commit whose
    /// subject names this task key.
    commit: Option<String>,
}

/// The whole job as a hierarchy: phases (ROADMAP) → each phase's tasks (PLAN,
/// live from the engine for the phase being executed). A task without the
/// `/feature` tree (quick presets) has no phases: `flat` holds the engine's tasks.
pub(super) fn board(root: &FsPath, t: &Task) -> serde_json::Value {
    let commits = timeline::task_commits(root, t);
    // PLAN keys restart per phase (every phase has a T1) and the branch log holds
    // every phase's commits: with more than one phase a commit must also name the
    // phase (`M1`) to count. No match beats a wrong one. Best first: the oldest
    // commit scoped to the task (`feat(M1/T2): …`), then the oldest non-docs commit
    // naming it (a `docs: log T2-T5, T7` sweep names many tasks).
    let rows = |tasks: Vec<SubTask>, other_phase: Option<&str>| -> Vec<SubTaskRow> {
        tasks
            .into_iter()
            .map(|task| {
                let names = |c: &&timeline::Commit| names_key(&c.subject, &task.key) && other_phase.is_none_or(|ph| names_key(&c.subject, ph));
                let scoped = other_phase.map(|ph| format!("{ph}/{}", task.key));
                let oldest = || commits.iter().rev();
                let commit = task.sha.clone().or_else(|| {
                    oldest()
                        .find(|c| scoped.as_ref().is_some_and(|s| names_key(&c.subject, s)))
                        .or_else(|| oldest().filter(names).find(|c| !c.subject.starts_with("docs")))
                        .or_else(|| oldest().find(names))
                        .map(|c| c.sha.clone())
                });
                SubTaskRow { task, commit }
            })
            .collect()
    };
    let engine_live = engine_applies(root, t);
    let Some(fdir) = feature_dir(root, t) else {
        let mut flat = Vec::new();
        if engine_live {
            merge_engine(&mut flat, &read_engine(root));
        }
        return serde_json::json!({ "phases": [], "flat": rows(flat, None), "live": engine_live });
    };
    let active = active_phase(root, t);
    let run_phase = timeline::run_phases(root, t);
    let roadmap = std::fs::read_to_string(fdir.join("ROADMAP.md")).unwrap_or_default();
    let per_phase = [Stage::Solution, Stage::Review, Stage::Approval, Stage::Dev, Stage::Test, Stage::Security];
    let cur = catalog_pos(t.stage);
    let parsed = parse_roadmap(&roadmap);
    let multi_phase = parsed.len() > 1;
    let phases: Vec<PhaseBoard> = parsed
        .into_iter()
        .map(|mut p| {
            if p.state == "todo" && active.as_deref() == Some(p.id.as_str()) {
                p.state = "active";
            }
            let dir = phase_dir(&fdir, &p.id);
            if let Some(d) = &dir {
                grade(&mut p, d);
            }
            let plan = dir.as_ref().map(|d| read_glob(d, "-PLAN")).unwrap_or_default();
            let mut tasks = parse_plan(&plan);
            if p.state == "active" && engine_live {
                merge_engine(&mut tasks, &read_engine(root));
            }
            if p.state == "done" {
                tasks.iter_mut().for_each(|s| s.status = "done");
            }
            let steps = per_phase
                .iter()
                .filter(|s| t.has(**s))
                .map(|s| StepChip {
                    key: s.key(),
                    state: match p.state {
                        "done" => "done",
                        "active" if catalog_pos(*s) < cur => "done",
                        "active" if catalog_pos(*s) == cur => "now",
                        _ => "todo",
                    },
                })
                .collect();
            let other = multi_phase.then_some(p.id.as_str());
            let runs: Vec<usize> = run_phase.iter().enumerate().filter(|(_, ph)| ph.as_deref() == Some(p.id.as_str())).map(|(i, _)| i).collect();
            let of = |i: &usize| &t.runs[*i];
            let started = runs.iter().map(|i| of(i).started.clone()).min();
            let ended = if runs.iter().all(|i| of(i).ended.is_some()) { runs.iter().filter_map(|i| of(i).ended.clone()).max() } else { None };
            let cost = runs.iter().map(|i| of(i).cost).sum();
            let rounds_back = runs.iter().filter(|i| of(i).stage == Stage::Review && of(i).verdict.as_deref() == Some("fail")).count();
            let result = dir.as_ref().map(|d| before_after(&format!("{}\n{}", read_glob(d, "-VERIFICATION"), read_glob(d, "-MEASURE")))).unwrap_or_default();
            PhaseBoard {
                goal: roadmap_goal(&roadmap, &p.id),
                planned: !plan.trim().is_empty(),
                steps,
                tasks: rows(tasks, other),
                phase: p,
                runs,
                started,
                ended,
                cost,
                rounds_back,
                result,
            }
        })
        .collect();
    serde_json::json!({ "phases": phases, "flat": [], "live": engine_live })
}

/// Whether a commit subject names task `key` (`T3` as a whole word, not `T30`/`T-003`).
fn names_key(subject: &str, key: &str) -> bool {
    subject.match_indices(key).any(|(i, _)| {
        let before = subject[..i].chars().next_back();
        let after = subject[i + key.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '-') && !after.is_some_and(|c| c.is_alphanumeric())
    })
}

/// `- [x] **M0 Foundation** — …` → (M0, "Foundation", done).
pub(super) fn parse_roadmap(md: &str) -> Vec<Phase> {
    md.lines()
        .filter_map(|l| {
            let (mark, rest) = checkbox(l)?;
            let bold = rest.trim_start().strip_prefix("**").and_then(|r| r.split_once("**")).map(|(b, _)| b).unwrap_or(rest);
            let mut words = bold.split_whitespace();
            let id = words.next()?.trim_end_matches(':').to_string();
            let is_phase = id.len() > 1 && id.starts_with('M') && id[1..].chars().all(|c| c.is_ascii_alphanumeric());
            is_phase.then(|| Phase {
                name: words.collect::<Vec<_>>().join(" ").trim_start_matches(['—', '-', ':', ' ']).to_string(),
                id,
                state: match mark {
                    'x' | 'X' => "done",
                    '~' => "active",
                    _ => "todo",
                },
                ac_pass: 0,
                ac_total: 0,
                ac_notes: Vec::new(),
            })
        })
        .collect()
}

/// PLAN tasks, leniently: checkbox lines (`- [ ] T1: …`, `- [x] T1 (8d2143768e): …`,
/// `- [ ] **T2** — …`) and table rows (`| T1 | việc | file | … |`, keyed by the
/// header when there is one). Coverage checklist lines (`- [x] Mọi UC có AC`) are
/// not tasks: a task starts with its `T<n>` key.
pub(super) fn parse_plan(md: &str) -> Vec<SubTask> {
    let mut out: Vec<SubTask> = Vec::new();
    let mut header: Vec<String> = Vec::new();
    let mut prev_row: Vec<String> = Vec::new();
    for l in md.lines() {
        if let Some(t) = plan_line(l) {
            out.push(t);
            continue;
        }
        let t = l.trim();
        if !t.starts_with('|') {
            prev_row.clear();
            header.clear();
            continue;
        }
        let cells = table_cells(t);
        if is_rule(&cells) {
            header = std::mem::take(&mut prev_row); // `|---|` → the row above was the header
            continue;
        }
        if let Some(task) = plan_row(&cells, &header).filter(|r| !out.iter().any(|o| o.key == r.key)) {
            out.push(task);
        }
        prev_row = cells;
    }
    out
}

/// `T12` (optionally bold) at the start of `s` → (key, rest).
fn task_key(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start().trim_start_matches('*');
    let end = s.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(s.len());
    let key = &s[..end];
    (key.len() > 1 && key.starts_with('T') && key[1..].chars().all(|c| c.is_ascii_digit())).then(|| (key, &s[end..]))
}

fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// `(sha)` right after the key → (sha, rest).
fn key_sha(rest: &str) -> (Option<String>, &str) {
    let r = rest.trim_start_matches(['*', ' ']);
    if let Some((inside, tail)) = r.strip_prefix('(').and_then(|p| p.split_once(')')) {
        let inside = inside.trim().trim_matches('`');
        if is_sha(inside) {
            return (Some(inside.to_string()), tail);
        }
    }
    (None, rest)
}

fn plan_line(l: &str) -> Option<SubTask> {
    let (mark, rest) = checkbox(l)?;
    let (key, after) = task_key(rest)?;
    let (sha, after) = key_sha(after);
    let after = after.trim_start_matches(['*', ' ']);
    let body = after.strip_prefix(':').or_else(|| after.strip_prefix('—')).or_else(|| after.strip_prefix('–')).or_else(|| after.strip_prefix('-'))?;
    let body = body.trim_start_matches(['*', ' ']).trim();
    let desc = &body[..tags_start(body)];
    Some(SubTask {
        key: key.to_string(),
        title: short_title(desc),
        detail: body.to_string(),
        files: tag(body, "file"),
        depends: tag(body, "depends"),
        ac: tag(body, "AC"),
        status: match mark {
            'x' | 'X' => "done",
            '~' => "in_progress",
            _ => "todo",
        },
        retries: 0,
        note: String::new(),
        sha,
    })
}

fn table_cells(row: &str) -> Vec<String> {
    let inner = row.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// `|---|:--:|` separator row.
fn is_rule(cells: &[String]) -> bool {
    cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// Column of the header whose name (lowercase) is one of `names`.
fn col(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|h| {
        let h = h.trim_matches('*').trim().to_lowercase();
        names.iter().any(|n| h == *n || h.starts_with(&format!("{n} ")))
    })
}

/// A PLAN table row: the key is the first cell, or the second after a status cell.
fn plan_row(cells: &[String], header: &[String]) -> Option<SubTask> {
    let is_status = |c: &str| c.is_empty() || (c.len() <= 3 && (c.starts_with('[') || c.eq_ignore_ascii_case("x"))) || c.starts_with('✅') || c.starts_with('⬜');
    let ki = match cells.first() {
        Some(c) if task_key(c).is_some_and(|(_, r)| key_sha(r).1.trim_matches(['*', ' ']).is_empty()) => 0,
        Some(c) if is_status(c) && cells.get(1).is_some_and(|c| task_key(c).is_some()) => 1,
        _ => return None,
    };
    let (key, rest) = task_key(&cells[ki])?;
    let (sha, _) = key_sha(rest);
    let at = |names: &[&str]| col(header, names).and_then(|i| cells.get(i)).map(|s| s.trim().to_string()).filter(|s| s != "—" && s != "-");
    let row = cells.join(" | ");
    let desc = at(&["việc", "task", "title", "mô tả", "nội dung", "description", "tên"])
        .filter(|_| col(header, &["task"]) != Some(ki))
        .or_else(|| cells.get(ki + 1).cloned())
        .unwrap_or_default();
    let state = at(&["status", "trạng thái", "tt", "done", "xong"]).or_else(|| (ki == 1).then(|| cells[0].clone())).unwrap_or_default().to_lowercase();
    let done = state.contains("[x]") || state.starts_with('✅') || state == "x" || state.starts_with("done") || state.starts_with("xong");
    let doing = state.contains("[~]") || state.contains("progress") || state.starts_with("đang");
    Some(SubTask {
        key: key.to_string(),
        title: short_title(&desc[..tags_start(&desc)]),
        detail: desc.clone(),
        files: at(&["file", "files", "tệp"]).unwrap_or_else(|| tag(&row, "file")),
        depends: at(&["depends", "phụ thuộc"]).unwrap_or_else(|| tag(&row, "depends")),
        ac: at(&["ac"]).unwrap_or_else(|| tag(&row, "AC")),
        status: if done {
            "done"
        } else if doing {
            "in_progress"
        } else {
            "todo"
        },
        retries: 0,
        note: String::new(),
        sha,
    })
}

/// Byte offset of the first `[name:` tag (`[file:`, `[depends:`, `[AC:`…), else the end.
fn tags_start(s: &str) -> usize {
    s.match_indices('[')
        .find(|(i, _)| {
            let after = &s[i + 1..];
            let name = after.split(':').next().unwrap_or("");
            after.contains(':') && (1..=12).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphabetic())
        })
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// First sentence of a task text (up to `. `, `; ` or a newline outside brackets and
/// code spans; or `: ` / ` — ` once the head is long enough to name the task on its
/// own, so `` `baseline.php`: thêm … `` keeps going), ≤ 120 chars.
pub(super) fn short_title(s: &str) -> String {
    const MAX: usize = 120;
    let s = s.trim();
    let (mut depth, mut code) = (0i32, false);
    let mut end = s.len();
    let mut chars = s.char_indices().peekable();
    let head_len = |i: usize| s[..i].trim().chars().count();
    while let Some((i, c)) = chars.next() {
        let next_ws = chars.peek().is_none_or(|(_, n)| n.is_whitespace());
        let free = !code && depth <= 0;
        match c {
            '`' => code = !code,
            '(' | '[' | '{' if !code => depth += 1,
            ')' | ']' | '}' if !code => depth -= 1,
            '\n' => {
                end = i;
                break;
            }
            '.' | ';' if free && next_ws => {
                end = i;
                break;
            }
            ':' if free && next_ws && head_len(i) >= 25 => {
                end = i;
                break;
            }
            '—' | '–' if free && s[..i].ends_with(' ') && next_ws && head_len(i) >= 15 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    clip(s[..end].trim(), MAX)
}

/// `- [c] rest` → (c, rest).
fn checkbox(line: &str) -> Option<(char, &str)> {
    let l = line.trim_start().strip_prefix(['-', '*'])?.trim_start().strip_prefix('[')?;
    let mut chars = l.chars();
    let mark = chars.next()?;
    let rest = chars.as_str().strip_prefix(']')?;
    Some((mark, rest))
}

/// Value of `[name: value]` in `s` (case-insensitive name), "" if absent or `—`.
pub(super) fn tag(s: &str, name: &str) -> String {
    // ASCII lowercasing keeps byte offsets valid for `s`.
    let low = s.to_ascii_lowercase();
    let pat = format!("[{}:", name.to_ascii_lowercase());
    let Some(i) = low.find(&pat) else { return String::new() };
    let after = &s[i + pat.len()..];
    let v = after[..after.find(']').unwrap_or(after.len())].trim();
    if v == "—" || v == "-" { String::new() } else { v.to_string() }
}

fn grade(p: &mut Phase, dir: &FsPath) {
    let (pass, total, notes) = ac_counts(&read_glob(dir, &format!("{}-VERIFICATION", p.id)));
    (p.ac_pass, p.ac_total, p.ac_notes) = (pass, total, notes);
}

/// Noted ACs of every phase (ROADMAP order) — the re-graded ones are decisions.
pub(super) fn phase_notes(root: &FsPath, t: &Task) -> Vec<(String, Vec<AcNote>)> {
    let Some(fdir) = feature_dir(root, t) else { return Vec::new() };
    let roadmap = std::fs::read_to_string(fdir.join("ROADMAP.md")).unwrap_or_default();
    parse_roadmap(&roadmap)
        .into_iter()
        .filter_map(|p| {
            let d = phase_dir(&fdir, &p.id)?;
            let (_, _, notes) = ac_counts(&read_glob(&d, &format!("{}-VERIFICATION", p.id)));
            (!notes.is_empty()).then_some((p.id, notes))
        })
        .collect()
}

/// Graded ACs of a VERIFICATION file: (PASS count, AC count, noted ACs). A row has
/// an AC id cell (`AC-07`) and a verdict cell starting with PASS / FAIL /
/// NEEDS-CONFIRM (`PASS (AC chấm lại)` counts as PASS and is noted). An AC listed
/// twice (a re-run table) counts once, its last verdict wins.
pub(super) fn ac_counts(md: &str) -> (usize, usize, Vec<AcNote>) {
    let mut rows = ac_rows(md);
    let pass = rows.iter().filter(|r| r.status == "PASS").count();
    let total = rows.len();
    rows.retain(|r| r.status != "PASS" || !r.note.is_empty());
    (pass, total, rows)
}

/// Every graded AC row of a VERIFICATION file (last verdict per AC wins).
pub(super) fn ac_rows(md: &str) -> Vec<AcNote> {
    let mut rows: Vec<AcNote> = Vec::new();
    for l in md.lines().map(str::trim).filter(|l| l.starts_with('|')) {
        let cells = table_cells(l);
        let Some((idx, id)) = cells.iter().enumerate().find_map(|(i, c)| lead_ac_id(c).map(|id| (i, id))) else { continue };
        let Some(vi) = cells.iter().enumerate().skip(idx + 1).find(|(_, c)| verdict(c).is_some()).map(|(i, _)| i) else { continue };
        let (status, note) = verdict(&cells[vi]).unwrap_or(("PASS", String::new()));
        let source = cells.get(idx + 1).filter(|c| verdict(c).is_none()).cloned().unwrap_or_default();
        let evidence = cells[vi + 1..].iter().find(|c| !c.is_empty()).cloned().unwrap_or_default();
        let row = AcNote { id, status, note, source, evidence };
        match rows.iter_mut().find(|r| r.id == row.id) {
            Some(r) => *r = row,
            None => rows.push(row),
        }
    }
    rows
}

fn is_ac_id(c: &str) -> bool {
    c.trim_matches('*').strip_prefix("AC-").is_some_and(|n| n.starts_with(|d: char| d.is_ascii_alphanumeric()) && n.contains(|d: char| d.is_ascii_digit()) && n.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-'))
}

/// The AC a matrix cell names: the whole cell (`AC-07`, `**AC-07**`, `AC-M5-2`) or
/// its leading id before a label (`AC-11 khoá hub…`, `**AC-2** — body…`). A cell
/// listing several ACs (`AC-07, AC-13: …`, a rounds table) names none.
fn lead_ac_id(c: &str) -> Option<String> {
    let t = c.trim().trim_start_matches('*');
    let end = t.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-')).unwrap_or(t.len());
    let id = t[..end].trim_end_matches('-');
    if !is_ac_id(id) {
        return None;
    }
    let rest = t[id.len()..].trim_start_matches('*').trim_start();
    let label = rest.is_empty() || rest.starts_with(['—', '–', ':']) || (t[id.len()..].starts_with(|ch: char| ch.is_whitespace() || ch == '*') && !rest.starts_with(','));
    (label && !rest.trim_start_matches(['—', '–', ':', ' ']).starts_with("AC-")).then(|| id.to_string())
}

/// `✅ PASS (AC chấm lại)` → (PASS, "AC chấm lại"). A cell that passes only in part
/// (`PASS (BE) / NEEDS-CONFIRM (FE E2E)`, `PASS (PHP) + ⚠️ E2E NEEDS-CONFIRM`) is
/// graded by its worst part.
fn verdict(cell: &str) -> Option<(&'static str, String)> {
    let c = cell.trim_start_matches(|ch: char| !ch.is_ascii_alphabetic());
    let up = c.to_ascii_uppercase();
    let (status, len) = [("NEEDS-CONFIRM", "NEEDS-CONFIRM"), ("NEEDS CONFIRM", "NEEDS-CONFIRM"), ("PASS", "PASS"), ("FAIL", "FAIL")]
        .iter()
        .find(|(k, _)| up.starts_with(k) && !up[k.len()..].starts_with(|ch: char| ch.is_ascii_alphabetic()))
        .map(|(k, s)| (*s, k.len()))?;
    let rest = c[len..].trim().trim_start_matches('*').trim();
    let rest = rest.trim_start_matches([':', '—', '-', '–']).trim();
    // `(…)` around the whole note only: `(cấu trúc) + E2E …` keeps its text.
    let wraps = |r: &str| {
        let mut depth = 0i32;
        r.char_indices().all(|(i, ch)| {
            depth += match ch {
                '(' => 1,
                ')' => -1,
                _ => 0,
            };
            depth > 0 || i + ch.len_utf8() == r.len()
        })
    };
    let rest = if rest.starts_with('(') && rest.ends_with(')') && wraps(rest) { rest[1..rest.len() - 1].trim() } else { rest };
    let worse = |w: &str| up[len..].match_indices(w).any(|(i, _)| {
        let tail = &up[len..][i + w.len()..];
        !tail.starts_with(|ch: char| ch.is_ascii_alphabetic())
    });
    let status = if status != "PASS" {
        status
    } else if worse("NEEDS-CONFIRM") || worse("NEEDS CONFIRM") {
        "NEEDS-CONFIRM"
    } else if worse("FAIL") {
        "FAIL"
    } else {
        status
    };
    Some((status, rest.to_string()))
}

/// Rows of a table whose header has a `Trước`/`Before` and a `Sau`/`After` column.
fn before_after(md: &str) -> Vec<BeforeAfter> {
    let mut out = Vec::new();
    let mut cols: Option<(usize, usize)> = None;
    let mut prev: Vec<String> = Vec::new();
    for l in md.lines().map(str::trim) {
        if !l.starts_with('|') {
            cols = None;
            prev.clear();
            continue;
        }
        let cells = table_cells(l);
        if is_rule(&cells) {
            let find = |names: &[&str]| prev.iter().position(|h| names.iter().any(|n| h.trim_matches('*').trim().to_lowercase().starts_with(n)));
            cols = find(&["trước", "before"]).zip(find(&["sau", "after"])).filter(|(b, a)| *b > 0 && *a > 0);
            continue;
        }
        if let Some((b, a)) = cols {
            if let (Some(label), Some(before), Some(after)) = (cells.first(), cells.get(b), cells.get(a)) {
                out.push(BeforeAfter { label: label.clone(), before: before.clone(), after: after.clone() });
            }
        }
        prev = cells;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roadmap_phases_with_state() {
        let md = "## Phases\n- [x] **M0 Foundation** — schema\n- [~] **M1 Webhook nhận tin** — x · cần: M0\n- [ ] **M2 Gửi tin** — y\n- [ ] something else\nStatus: `[ ]` chưa";
        let p = parse_roadmap(md);
        let got: Vec<(&str, &str, &str)> = p.iter().map(|p| (p.id.as_str(), p.name.as_str(), p.state)).collect();
        assert_eq!(got, [("M0", "Foundation", "done"), ("M1", "Webhook nhận tin", "active"), ("M2", "Gửi tin", "todo")]);
    }

    #[test]
    fn plan_tasks_tags_and_checkboxes() {
        let md = "## Tasks\n- [x] T1: Thêm divide(a, b)   [file: calc.py]   [depends: —]  [verify: python3 -m unittest]  [AC: AC-01,AC-02]\n- [ ] **T2**: Test chia 0 [file: test_calc.py] [depends: T1] [ac: AC-03]\n- [ ] Mọi UC có ≥1 AC.\n";
        let t = parse_plan(md);
        assert_eq!(t.len(), 2, "coverage checklist lines are not tasks");
        assert_eq!((t[0].key.as_str(), t[0].title.as_str(), t[0].files.as_str(), t[0].depends.as_str(), t[0].ac.as_str(), t[0].status), ("T1", "Thêm divide(a, b)", "calc.py", "", "AC-01,AC-02", "done"));
        assert_eq!((t[1].key.as_str(), t[1].depends.as_str(), t[1].ac.as_str(), t[1].status), ("T2", "T1", "AC-03", "todo"));
    }

    #[test]
    fn roadmap_goal_and_commit_key_match() {
        let md = "- [~] **M1 Webhook nhận tin** — nhận + lưu tin · cần: M0\n";
        assert_eq!(roadmap_goal(md, "M1"), "nhận + lưu tin · cần: M0");
        assert!(names_key("feat: T3 lưu tin", "T3"));
        assert!(names_key("engine(T3): done", "T3"));
        assert!(!names_key("feat: T30 x", "T3"));
        assert!(!names_key("board T-003: x", "T3"), "task id is not a PLAN key");
    }

    #[test]
    fn engine_overlays_plan_by_key_and_adds_unknown() {
        let mut t = parse_plan("- [ ] T1: a [file: x]\n- [ ] T2: b [file: y]\n");
        let eng = serde_json::json!({ "slices": [{ "tasks": [
            { "id": "s1-t1", "key": "T1", "title": "a", "status": "done", "retries": 0, "note": "" },
            { "id": "s1-t2", "key": "T2", "title": "b", "status": "in_progress", "retries": 2, "note": "verify fail" },
            { "id": "s2-t1", "key": "T9", "title": "converge fix", "status": "pending", "retries": 0, "note": "", "files": ["z"] }
        ]}]});
        assert!(merge_engine(&mut t, &eng));
        let got: Vec<(&str, &str, u64)> = t.iter().map(|s| (s.key.as_str(), s.status, s.retries)).collect();
        assert_eq!(got, [("T1", "done", 0), ("T2", "in_progress", 2), ("T9", "todo", 0)]);
        assert!(!merge_engine(&mut t, &serde_json::Value::Null));
    }

    #[test]
    fn verification_ac_counts() {
        let md = "| UC | AC | Nguồn | Verdict | Bằng chứng |\n|---|---|---|---|---|\n| UC1 | AC-01 | REQ | PASS | ok |\n| UC2 | AC-02 | REQ | FAIL | x |\n| UC3 | AC-03 | REQ | NEEDS-CONFIRM | y |\n";
        let (pass, total, notes) = ac_counts(md);
        assert_eq!((pass, total), (1, 3));
        let got: Vec<(&str, &str)> = notes.iter().map(|n| (n.id.as_str(), n.status)).collect();
        assert_eq!(got, [("AC-02", "FAIL"), ("AC-03", "NEEDS-CONFIRM")], "not-passed ACs are always noted");
    }

    /// Shapes of T-004's M1-VERIFICATION: verdict cells carrying a re-grade note, a
    /// rounds table whose cell lists AC ids in prose, a blocked table without verdicts.
    #[test]
    fn ac_cells_with_notes_count_and_are_listed() {
        let md = "| UC | AC | Nguồn | Verdict | Bằng chứng |\n|----|----|----|----|----|\n\
| UC2 | AC-06 | REQUIREMENTS | PASS | ok |\n\
| UC3 | AC-07 | REQUIREMENTS + PO Q-4 | PASS (AC chấm lại) | nq 63 |\n\
| UC3, UC4 | AC-13 | REQUIREMENTS + PO Q-4 | PASS (Loại có số, lệch chữ AC) | 3,37 ms |\n\
| UC7 | AC-16 | REQUIREMENTS + PO Q-4 | ✅ PASS (AC chấm lại ở (d)) | x |\n\
| NFR1 | AC-17 | REQUIREMENTS | **PASS** | y |\n\
| UC1 | AC-18 | REQUIREMENTS | Passed | not a verdict word |\n\n\
| AC | Chặn bởi gì |\n|---|---|\n| — | Không AC nào bị chặn. |\n\n\
| Round | AC/finding vào | Task | Kết quả |\n|---|---|---|---|\n\
| — | AC-07, AC-13, AC-16(d): câu chữ AC không khớp | không sinh task | PO Q-4: chấm lại |\n";
        let (pass, total, notes) = ac_counts(md);
        assert_eq!((pass, total), (5, 5), "PASS (…) counts; prose AC lists and non-verdict cells do not");
        let got: Vec<(&str, &str, &str, &str)> = notes.iter().map(|n| (n.id.as_str(), n.status, n.note.as_str(), n.source.as_str())).collect();
        assert_eq!(
            got,
            [
                ("AC-07", "PASS", "AC chấm lại", "REQUIREMENTS + PO Q-4"),
                ("AC-13", "PASS", "Loại có số, lệch chữ AC", "REQUIREMENTS + PO Q-4"),
                ("AC-16", "PASS", "AC chấm lại ở (d)", "REQUIREMENTS + PO Q-4"),
            ]
        );
        // A re-run table grading the same AC again: counted once, last verdict wins.
        let rerun = "| AC | Verdict |\n|---|---|\n| AC-01 | FAIL |\n| AC-02 | PASS |\n\n| AC | Verdict |\n|---|---|\n| AC-01 | PASS |\n";
        assert_eq!((ac_counts(rerun).0, ac_counts(rerun).1), (2, 2));
    }

    /// T-004's M0 PLAN: `T<n> (sha): …` lines, long multi-sentence task text, a
    /// coverage checklist in between, a converge section after it.
    #[test]
    fn plan_lines_with_commit_sha_dash_and_long_text() {
        let md = "## Tasks\n\n\
- [x] T1 (8d2143768e): `render.php` chạy Action + save rollback. (a) env `KIND=action`: class `Vtiger_Loader::getComponentClassName('Action', <tên>, <module>)`; (b) env `WRITE=1`.   [file: tests/Perf/render.php]   [depends: —]   [verify: `php -l`]   [AC: AC-05, AC-07]\n\
- [x] T2 (57181a0090): `render.php` TRACE. Env `TRACE=<name>` (chỉ `^[A-Za-z0-9_-]+$`).   [file: tests/Perf/render.php]   [depends: T1]\n\
- [x] **T3** (`d4bbf067ef`) — #5 spike (D1, sau #1–#4; chạy khi KHÔNG task khác) giữ code nếu warm giảm\n\
- [~] T4 – đang làm [file: a.php]\n\
- [x] UC1 → AC-01…AC-14\n\
- [x] AC → task: AC-01 T3 · AC-02 T3\n\
- [x] `[depends:]`: T2←T1 (cùng file)\n\
\n## Converge round 1\n\n\
- [x] T11 (99650c5da1): `render.php` an toàn WRITE.\n";
        let t = parse_plan(md);
        let keys: Vec<&str> = t.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["T1", "T2", "T3", "T4", "T11"]);
        assert_eq!((t[0].title.as_str(), t[0].sha.as_deref(), t[0].files.as_str(), t[0].ac.as_str(), t[0].status), ("`render.php` chạy Action + save rollback", Some("8d2143768e"), "tests/Perf/render.php", "AC-05, AC-07", "done"));
        assert!(t[0].detail.starts_with("`render.php` chạy Action") && t[0].detail.ends_with("[AC: AC-05, AC-07]"), "detail keeps the whole text");
        assert_eq!(t[1].title, "`render.php` TRACE", "a `.` inside code or brackets does not end the title");
        assert_eq!((t[2].sha.as_deref(), t[2].title.as_str()), (Some("d4bbf067ef"), "#5 spike (D1, sau #1–#4; chạy khi KHÔNG task khác) giữ code nếu warm giảm"));
        assert_eq!((t[3].title.as_str(), t[3].status), ("đang làm", "in_progress"));
        let long = format!("- [ ] T5: {}\n", "chữ ".repeat(60));
        assert!(parse_plan(&long)[0].title.chars().count() <= 121, "title clipped to 120 chars + …");
        assert_eq!(short_title("#25(b) + thuế individual phần `getAssociatedProducts` (D7b, PO Q-6): trong `x` …"), "#25(b) + thuế individual phần `getAssociatedProducts` (D7b, PO Q-6)");
        assert_eq!(short_title("`baseline.php`: thêm 11 kịch bản D3 (key `kind`)"), "`baseline.php`: thêm 11 kịch bản D3 (key `kind`)", "a short head before `:` is not a title");
        assert_eq!(short_title("#3 (phần block) + #20: `Vtiger_Block` — khi hụt, tra index"), "#3 (phần block) + #20: `Vtiger_Block`");
    }

    #[test]
    fn plan_table_rows_by_header() {
        let md = "| # | Việc | File | Phụ thuộc | AC | Trạng thái |\n|---|---|---|---|---|---|\n\
| T1 | Thêm API. Chi tiết dài | api.go | — | AC-01 | done |\n\
| **T2** (abc1234) | Test API | api_test.go | T1 | AC-02 | todo |\n\n\
| [x] | T3 | Không header |\n\
| AC-01 | T1 | coverage matrix, not a task |\n";
        let t = parse_plan(md);
        let got: Vec<(&str, &str, &str, &str, &str, &str)> = t.iter().map(|s| (s.key.as_str(), s.title.as_str(), s.files.as_str(), s.depends.as_str(), s.ac.as_str(), s.status)).collect();
        assert_eq!(got, [("T1", "Thêm API", "api.go", "", "AC-01", "done"), ("T2", "Test API", "api_test.go", "T1", "AC-02", "todo"), ("T3", "Không header", "", "", "", "done")]);
        assert_eq!(t[1].sha.as_deref(), Some("abc1234"));
    }

    #[test]
    fn before_after_table_rows() {
        let md = "## Kết quả\n| Kịch bản | Trước | Sau | Ghi chú |\n|---|---|---|---|\n| invoice_edit nq | 106 | 68 | |\n| save | 64 | 56 | x |\n\nText\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(
            before_after(md),
            [BeforeAfter { label: "invoice_edit nq".into(), before: "106".into(), after: "68".into() }, BeforeAfter { label: "save".into(), before: "64".into(), after: "56".into() }]
        );
    }
}
