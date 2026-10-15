//! Task progress for the drawer/card: the `/feature` phases of a board task and
//! the sub-tasks of its active phase. Read-only; three sources:
//!   - `ROADMAP.md`   phase list + `[ ]/[~]/[x]` state,
//!   - `M<x>-NN-PLAN.md` the phase's tasks (`- [ ] T1: … [file: …] [depends: …] [AC: …]`),
//!   - engine `state.json` live status (pending/in_progress/done/blocked, retries) —
//!     only while THIS task owns the tree and a dev/test run of the phase is current.
//! PLAN/ROADMAP are AI-written: parsing is lenient, unknown lines are skipped.
use super::*;

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Phase {
    id: String,
    name: String,
    /// done | active | todo
    state: &'static str,
    ac_pass: usize,
    ac_total: usize,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub(super) struct SubTask {
    key: String,
    title: String,
    files: String,
    depends: String,
    ac: String,
    /// todo | in_progress | done | blocked
    status: &'static str,
    retries: u64,
    note: String,
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
            let (pass, total) = ac_counts(&read_glob(&d, &format!("{}-VERIFICATION", p.id)));
            (p.ac_pass, p.ac_total) = (pass, total);
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
        let hit = tasks.iter_mut().find(|t| (!key.is_empty() && t.key.eq_ignore_ascii_case(&key)) || (!title.is_empty() && t.title == title));
        let retries = e.get("retries").and_then(|x| x.as_u64()).unwrap_or(0);
        match hit {
            Some(t) => {
                t.status = status(e);
                t.retries = retries;
                t.note = s(e, "note");
            }
            None => tasks.push(SubTask {
                key: if key.is_empty() { s(e, "id") } else { key },
                title,
                files: e.get("files").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|f| f.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                depends: e.get("depends").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|f| f.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                ac: String::new(),
                status: status(e),
                retries,
                note: s(e, "note"),
            }),
        }
    }
    true
}

fn phase_dir(fdir: &FsPath, id: &str) -> Option<PathBuf> {
    std::fs::read_dir(fdir.join("phases")).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.is_dir() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == id || n.starts_with(&format!("{id}-")))
    })
}

/// Concatenated `*.md` files in `dir` whose stem contains `needle`, sorted.
fn read_glob(dir: &FsPath, needle: &str) -> String {
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
}

#[derive(Serialize, Debug)]
pub(super) struct SubTaskRow {
    #[serde(flatten)]
    task: SubTask,
    /// Short hash of a branch commit whose subject names this task key.
    commit: Option<String>,
}

/// The whole job as a hierarchy: phases (ROADMAP) → each phase's tasks (PLAN,
/// live from the engine for the phase being executed). A task without the
/// `/feature` tree (quick presets) has no phases: `flat` holds the engine's tasks.
pub(super) fn board(root: &FsPath, t: &Task) -> serde_json::Value {
    let commits = branch_commits(root, t);
    // PLAN keys restart per phase (every phase has a T1) and the branch log holds
    // every phase's commits: with more than one phase a commit must also name the
    // phase (`M1`) to count. No match beats a wrong one.
    let rows = |tasks: Vec<SubTask>, other_phase: Option<&str>| -> Vec<SubTaskRow> {
        tasks
            .into_iter()
            .map(|task| {
                let commit = commits
                    .iter()
                    .find(|(_, subj)| names_key(subj, &task.key) && other_phase.is_none_or(|ph| names_key(subj, ph)))
                    .map(|(h, _)| h.clone());
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
                (p.ac_pass, p.ac_total) = ac_counts(&read_glob(d, &format!("{}-VERIFICATION", p.id)));
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
            PhaseBoard { goal: roadmap_goal(&roadmap, &p.id), planned: !plan.trim().is_empty(), steps, tasks: rows(tasks, other), phase: p }
        })
        .collect();
    serde_json::json!({ "phases": phases, "flat": [], "live": engine_live })
}

/// `(short hash, subject)` of the commits on the task branch since its base.
fn branch_commits(root: &FsPath, t: &Task) -> Vec<(String, String)> {
    let (Some(br), Some(base)) = (&t.branch, &t.base) else { return Vec::new() };
    git(root, &["log", "--format=%h %s", &format!("{base}..{br}")])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' ').map(|(h, s)| (h.to_string(), s.to_string())))
        .collect()
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
            })
        })
        .collect()
}

/// `- [ ] T1: <việc> [file: a] [depends: —] [AC: AC-01]` → SubTask.
pub(super) fn parse_plan(md: &str) -> Vec<SubTask> {
    md.lines()
        .filter_map(|l| {
            let (mark, rest) = checkbox(l)?;
            let (key, body) = rest.trim().split_once(':')?;
            let key = key.trim().trim_matches('*').trim();
            if !(key.len() > 1 && key.starts_with('T') && key[1..].chars().all(|c| c.is_ascii_digit())) {
                return None;
            }
            let title_end = body.find('[').unwrap_or(body.len());
            Some(SubTask {
                key: key.to_string(),
                title: body[..title_end].trim().to_string(),
                files: tag(body, "file"),
                depends: tag(body, "depends"),
                ac: tag(body, "AC"),
                status: if matches!(mark, 'x' | 'X') { "done" } else { "todo" },
                retries: 0,
                note: String::new(),
            })
        })
        .collect()
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
fn tag(s: &str, name: &str) -> String {
    let low = s.to_lowercase();
    let pat = format!("[{}:", name.to_lowercase());
    let Some(i) = low.find(&pat) else { return String::new() };
    let after = &s[i + pat.len()..];
    let v = after[..after.find(']').unwrap_or(after.len())].trim();
    if v == "—" || v == "-" { String::new() } else { v.to_string() }
}

/// AC matrix rows of a VERIFICATION file: (PASS count, AC rows).
fn ac_counts(md: &str) -> (usize, usize) {
    md.lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| {
            let cells: Vec<String> = l.split('|').map(|c| c.trim().to_uppercase()).collect();
            if !cells.iter().any(|c| c.starts_with("AC-")) {
                return None;
            }
            cells.iter().find_map(|c| match c.as_str() {
                "PASS" | "✅ PASS" => Some(true),
                "FAIL" | "NEEDS-CONFIRM" | "❌ FAIL" => Some(false),
                _ => None,
            })
        })
        .fold((0, 0), |(p, n), pass| (p + usize::from(pass), n + 1))
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
        assert_eq!(ac_counts(md), (1, 3));
    }
}
