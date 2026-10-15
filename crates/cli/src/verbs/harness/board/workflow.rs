//! The boss's own omp slash-command workflows (`/cook`, …) as a task's pipeline.
//!
//! Discovery: `~/.omp/agent/commands/*.md` and `<project>/.omp/commands/*.md`
//! (the project's file wins on a name clash). A command is offered for a project
//! only when enabled there: its frontmatter says `board: true`, or the boss put it
//! in the project's opt-in list `.cache/ckit/board/workflows.json`. Never
//! auto-enabled. `feature` is the board's built-in pipeline and is never listed.
//!
//! A workflow task always runs Solution (the command's plan, read-only outside
//! its plan dirs) → Approval (the boss) → Dev (the same omp session continues
//! the command to completion) → Acceptance.

use std::path::{Path as FsPath, PathBuf};

use axum::{extract::Query, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use super::{board_dir, exclude_board_from_git, ApiErr, ApiRes, RootQ, Stage};

/// The steps every workflow task runs, in order.
pub(super) fn steps() -> Vec<Stage> {
    vec![Stage::Solution, Stage::Approval, Stage::Dev, Stage::Acceptance]
}

/// A task's workflow: the command it runs and, once planned, its plan file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Workflow {
    pub(super) name: String,
    /// Absolute path of the command file the runs read and follow.
    pub(super) path: String,
    /// Plan file the plan step wrote (relative to the project root).
    #[serde(default)]
    pub(super) plan: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct Command {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) description: String,
    pub(super) argument_hint: String,
    /// `board: true` in the frontmatter (enabled everywhere, no toggle needed).
    pub(super) board: bool,
    pub(super) enabled: bool,
}

/// Flat `key: value` pairs of the leading `---` frontmatter block (quotes stripped).
fn frontmatter(raw: &str) -> Vec<(String, String)> {
    let mut lines = raw.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    lines
        .take_while(|l| l.trim() != "---")
        .filter(|l| !l.starts_with([' ', '\t', '#']))
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            let v = v.trim();
            let v = ['"', '\''].iter().find_map(|q| v.strip_prefix(*q).and_then(|s| s.strip_suffix(*q))).unwrap_or(v);
            Some((k.trim().to_string(), v.to_string()))
        })
        .collect()
}

fn read_dir_cmds(dir: &FsPath) -> Vec<Command> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let fm = frontmatter(&std::fs::read_to_string(&p).ok()?);
            let get = |k: &str| fm.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone()).unwrap_or_default();
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            let name = Some(get("name")).filter(|n| !n.is_empty()).unwrap_or(stem);
            Some(Command {
                name,
                path: p.display().to_string(),
                description: get("description"),
                argument_hint: get("argument-hint"),
                board: get("board") == "true",
                enabled: false,
            })
        })
        .collect()
}

fn optin_file(root: &FsPath) -> PathBuf {
    board_dir(root).join("workflows.json")
}

fn optin(root: &FsPath) -> Vec<String> {
    std::fs::read_to_string(optin_file(root)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Every command the project can see (global + project, project wins), sorted by
/// name, with `enabled` for this project. `feature` is the built-in pipeline.
pub(super) fn discover(home: &FsPath, root: &FsPath) -> Vec<Command> {
    let mut all = read_dir_cmds(&home.join(".omp/agent/commands"));
    for c in read_dir_cmds(&root.join(".omp/commands")) {
        all.retain(|g| g.name != c.name);
        all.push(c);
    }
    all.retain(|c| c.name != "feature");
    let on = optin(root);
    for c in &mut all {
        c.enabled = c.board || on.contains(&c.name);
    }
    all.sort_by(|a, b| a.name.cmp(&b.name));
    all
}

/// The workflow a new task runs; refused unless the command is enabled here.
pub(super) fn resolve(home: &FsPath, root: &FsPath, name: &str) -> Result<Workflow, String> {
    let name = name.trim().trim_start_matches('/');
    match discover(home, root).into_iter().find(|c| c.name == name) {
        Some(c) if c.enabled => Ok(Workflow { name: c.name, path: c.path, plan: None }),
        Some(_) => Err(format!("/{name} chưa bật cho dự án này (Quản lý quy trình)")),
        None => Err(format!("không tìm thấy lệnh /{name}")),
    }
}

/// Add/remove a command in the project's opt-in list.
pub(super) fn set_enabled(root: &FsPath, name: &str, on: bool) -> std::io::Result<()> {
    let mut list = optin(root);
    list.retain(|n| n != name);
    if on {
        list.push(name.to_string());
        list.sort();
    }
    std::fs::create_dir_all(board_dir(root))?;
    std::fs::write(optin_file(root), serde_json::to_string_pretty(&list).unwrap_or_default())
}

/// The plan path a plan run reported, kept only when it is a file inside the project.
pub(super) fn plan_path(root: &FsPath, reported: &str) -> Option<String> {
    let p = FsPath::new(reported.trim());
    let rel = if p.is_absolute() { p.strip_prefix(root).ok()? } else { p };
    if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    root.join(rel).is_file().then(|| rel.display().to_string())
}

/// Paths the plan step may write (porcelain paths, `/`-separated): its plan, the
/// project's memory, the board's own state.
const PLAN_WRITABLE: [&str; 3] = [".omp/plans/", "agents/", ".cache/ckit/"];

/// Files the plan run changed outside its writable dirs, from `git status
/// --porcelain --untracked-files=all` before/after.
pub(super) fn plan_violations(before: &str, after: &str) -> Vec<String> {
    after
        .lines()
        .filter(|l| !before.lines().any(|b| b == *l))
        .filter_map(|l| l.get(3..))
        .map(|p| p.rsplit(" -> ").next().unwrap_or(p).trim_matches('"').to_string())
        .filter(|p| !PLAN_WRITABLE.iter().any(|w| p.starts_with(w)))
        .collect()
}

#[derive(Deserialize)]
pub(super) struct ToggleBody {
    name: String,
    enabled: bool,
}

/// `GET /api/board/workflows?root=` — every command with its per-project switch.
pub(super) async fn api_list(Query(rq): Query<RootQ>) -> ApiRes<serde_json::Value> {
    let root = super::create_root(&rq)?;
    let home = dirs::home_dir().unwrap_or_default();
    Ok(Json(serde_json::json!({ "commands": discover(&home, &root) })))
}

/// `POST /api/board/workflows?root=` `{name, enabled}` — edit the opt-in list.
pub(super) async fn api_toggle(Query(rq): Query<RootQ>, Json(b): Json<ToggleBody>) -> ApiRes<serde_json::Value> {
    let root = super::create_root(&rq)?;
    let home = dirs::home_dir().unwrap_or_default();
    if !discover(&home, &root).iter().any(|c| c.name == b.name) {
        return Err((StatusCode::NOT_FOUND, format!("không tìm thấy lệnh /{}", b.name)));
    }
    set_enabled(&root, &b.name, b.enabled).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) as ApiErr)?;
    exclude_board_from_git(&root);
    Ok(Json(serde_json::json!({ "commands": discover(&home, &root) })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ckit-wf-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn cmd(dir: &FsPath, file: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file), body).unwrap();
    }

    #[test]
    fn discovery_parses_frontmatter_project_wins_and_needs_opt_in() {
        let home = tmp("home");
        let root = tmp("root");
        let g = home.join(".omp/agent/commands");
        cmd(&g, "cook.md", "---\nname: cook\nargument-hint: '<#ticket | \"desc\">'\ndescription: Global cook\n---\n# body\nname: not-frontmatter\n");
        cmd(&g, "feature.md", "---\nname: feature\ndescription: built-in\n---\n");
        cmd(&g, "ship.md", "---\ndescription: \"Ship it\"\nboard: true\n---\n");
        cmd(&g, "notes.txt", "---\nname: notes\n---\n");
        let p = root.join(".omp/commands");
        cmd(&p, "cook.md", "---\nname: cook\ndescription: Project cook\n---\n");
        cmd(&p, "demo.md", "no frontmatter at all\n");

        let all = discover(&home, &root);
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["cook", "demo", "ship"], "feature excluded, non-md skipped, sorted");
        let cook = &all[0];
        assert_eq!(cook.description, "Project cook", "project overrides global");
        assert!(cook.path.ends_with(".omp/commands/cook.md") && cook.path.starts_with(&root.display().to_string()));
        assert!(!cook.enabled, "never auto-enabled");
        assert_eq!((all[1].description.as_str(), all[1].enabled), ("", false));
        let ship = &all[2];
        assert_eq!((ship.description.as_str(), ship.board, ship.enabled), ("Ship it", true, true), "`board: true` opts in");

        assert!(resolve(&home, &root, "cook").unwrap_err().contains("chưa bật"));
        assert!(resolve(&home, &root, "nope").unwrap_err().contains("không tìm thấy"));
        assert!(resolve(&home, &root, "feature").is_err(), "the built-in pipeline is not a workflow");
        set_enabled(&root, "cook", true).unwrap();
        let wf = resolve(&home, &root, "/cook").unwrap();
        assert_eq!((wf.name.as_str(), wf.plan.as_ref()), ("cook", None));
        assert!(wf.path.ends_with(".omp/commands/cook.md"));
        set_enabled(&root, "cook", false).unwrap();
        assert!(resolve(&home, &root, "cook").is_err());
        assert_eq!(resolve(&home, &root, "ship").unwrap().name, "ship");
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plan_step_may_only_touch_its_plan_dirs() {
        let before = "?? notes.txt\n";
        let after = "?? notes.txt\n?? .omp/plans/cook-1.md\n M agents/STATE.md\n?? .cache/ckit/x\n M src/app.php\nR  old.rs -> src/new.rs\n";
        assert_eq!(plan_violations(before, after), ["src/app.php", "src/new.rs"]);
        assert!(plan_violations(before, before).is_empty());
    }

    #[test]
    fn reported_plan_path_must_be_a_file_inside_the_project() {
        let root = tmp("plan");
        cmd(&root.join(".omp/plans"), "p.md", "# plan");
        assert_eq!(plan_path(&root, ".omp/plans/p.md").as_deref(), Some(".omp/plans/p.md"));
        assert_eq!(plan_path(&root, &root.join(".omp/plans/p.md").display().to_string()).as_deref(), Some(".omp/plans/p.md"));
        assert!(plan_path(&root, "../etc/passwd").is_none());
        assert!(plan_path(&root, "/etc/hosts").is_none());
        assert!(plan_path(&root, ".omp/plans/missing.md").is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
