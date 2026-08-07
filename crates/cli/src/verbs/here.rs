use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::HashSet;

use crate::{env_detect, ui, verbs::skill};

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          8sync .                       seed agents/* context and resume the last omp session (omp --continue)
          8sync . work                  reopen the session you named with `8sync new work`

        BEHAVIOR
          · Walks up from cwd to find the project root (.git / Cargo.toml / package.json / pyproject.toml / go.mod / deno.json).
          · Detects stack (rust/node/python/nextjs/tauri/react-native/go) and seeds AGENTS.md + agents/{PROJECT,KNOWLEDGE,DECISIONS,PREFERENCES,STATE,NOTES}.md when missing.
          · Re-injects the dynamic skills block in AGENTS.md so omp sees an up-to-date skill list.
          · Execs `omp --continue` in the project root (resumes the latest session). Pass a NAME to reopen that named session (omp --resume <its file>).
          · To start FRESH instead of resuming, use `8sync new` (optionally `8sync new <name>`).
          · If omp is missing, drops into the user shell instead (run `8sync setup` to fix).
    "}
)]
pub struct Args {
    /// Optional session name — reopen the session you saved with `new <name>` (default: the latest session).
    pub name: Option<String>,
}

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          8sync new                     seed agents/* context and start a FRESH omp session (does NOT resume)
          8sync new fix-auth            start a fresh session and remember it as 'fix-auth' — reopen later with `8sync . fix-auth`

        BEHAVIOR
          · Same seeding as `8sync .` (project root detection, AGENTS.md + agents/* memory, skills block).
          · Execs `omp` WITHOUT --continue, so a brand-new session starts instead of resuming the last one.
          · With a NAME, ckit remembers which session file omp created (in ~/.cache/ckit/named-sessions.json) so `8sync . <name>` reopens exactly it.
          · Sessions always live in omp's DEFAULT session dir, so they also appear in omp's `/resume` picker (listed by omp's auto-title).
          · omp has no custom-title flag; the NAME is a ckit-side label to reopen by, not the title shown inside omp.
    "}
)]
pub struct NewArgs {
    /// Optional session name — remembered so `. <name>` reopens this exact session later.
    pub name: Option<String>,
}

/// `8sync .` — seed + resume (the latest session, or a named bucket).
pub fn run(a: Args) -> Result<()> {
    enter(false, a.name.as_deref())
}

/// `8sync new` — seed + start a FRESH session (optionally in a named bucket).
pub fn run_new(a: NewArgs) -> Result<()> {
    enter(true, a.name.as_deref())
}

/// Shared body for `8sync .` (resume) and `8sync new` (fresh). Always uses omp's
/// DEFAULT session dir so every session — including `8sync new` — shows up in
/// omp's own `/resume` picker. `fresh` drops `--continue`; `name` is a ckit-side
/// label: a fresh named launch records which session file omp created, and
/// `8sync . <name>` reopens exactly that one (`omp --resume <path>`).
fn enter(fresh: bool, name: Option<&str>) -> Result<()> {
    let env = env_detect::Env::detect()?;
    let cwd = std::env::current_dir().context("no cwd")?;
    let root = detect_project_root(&cwd).unwrap_or(cwd.clone());

    ui::header(&format!("{} {}", crate::brand::CMD, if fresh { "new" } else { "." }));
    ui::info(&format!("project: {}", root.display()));

    let stack = detect_stack(&root);
    if !stack.is_empty() {
        ui::ok(&format!("stack: {}", stack.join(", ")));
    }

    seed_project_context(&env, &root, &stack)?;

    if which::which("omp").is_err() {
        ui::warn("omp not installed — run `8sync setup` first. Falling back to $SHELL.");
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let _ = Command::new(&shell).current_dir(&root).status();
        return Ok(());
    }

    let cfg = crate::models::ModelConfig::load();
    let sessions_root = env.home.join(".omp/agent/sessions");
    let mut cmd = Command::new("omp");
    cmd.arg("--cwd").arg(&root).args(cfg.resume_flags());

    // Snapshot so a fresh NAMED launch can learn which file omp creates.
    let before = if fresh && name.is_some() { session_files(&sessions_root) } else { Vec::new() };

    let desc = match (fresh, name) {
        // Reopen a previously-named session by its recorded path.
        (false, Some(n)) => match lookup_named(&env.home, &root, n) {
            Some(p) if p.exists() => {
                cmd.arg("--resume").arg(&p);
                format!("omp --resume (named '{n}')")
            }
            _ => {
                ui::warn(&format!(
                    "no named session '{n}' yet — resuming the latest instead (create one with `{} new {n}`)",
                    crate::brand::CMD
                ));
                cmd.arg("--continue");
                "omp --continue".to_string()
            }
        },
        (false, None) => {
            cmd.arg("--continue");
            "omp --continue".to_string()
        }
        (true, Some(n)) => format!("omp (new session, will save as '{n}')"),
        (true, None) => "omp (new session)".to_string(),
    };
    ui::ok(&format!("→ exec: {desc}"));

    let status = cmd.current_dir(&root).status();

    // Fresh + named: record name → the session file omp just created, so a later
    // `8sync . <name>` reopens exactly it (and it's already visible in /resume).
    if fresh {
        if let Some(n) = name {
            if let Some(p) = newest_added(&sessions_root, &before) {
                if record_named(&env.home, &root, n, &p).is_ok() {
                    ui::ok(&format!("named '{n}' saved — reopen with `{} . {n}`", crate::brand::CMD));
                }
            }
        }
    }

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(anyhow::anyhow!("omp exited with {}", s)),
        Err(e) => Err(anyhow::anyhow!("could not exec omp: {}", e)),
    }
}

/// All `*.jsonl` session files under `root` (recursive, best-effort).
fn session_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_jsonl(root, &mut out);
    out
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_jsonl(&p, out);
            } else if p.extension().is_some_and(|x| x == "jsonl") {
                out.push(p);
            }
        }
    }
}

/// The newest `*.jsonl` under `root` absent from `before` — the session omp
/// created during this launch. `None` if the user started none.
fn newest_added(root: &Path, before: &[PathBuf]) -> Option<PathBuf> {
    let prev: HashSet<&Path> = before.iter().map(|p| p.as_path()).collect();
    session_files(root)
        .into_iter()
        .filter(|p| !prev.contains(p.as_path()))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// ckit's name→session-path map: `~/.cache/ckit/named-sessions.json` (flat JSON
/// object keyed by "<project-root>\n<name>"). Lives in the ckit cache, never the
/// project tree. Best-effort — a missing/corrupt file reads as empty.
fn named_map_path(home: &Path) -> PathBuf {
    home.join(".cache/ckit/named-sessions.json")
}

fn named_key(root: &Path, name: &str) -> String {
    format!("{}\n{}", root.display(), name)
}

fn load_named_map(home: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(named_map_path(home))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn lookup_named(home: &Path, root: &Path, name: &str) -> Option<PathBuf> {
    load_named_map(home)
        .get(&named_key(root, name))
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

fn record_named(home: &Path, root: &Path, name: &str, path: &Path) -> Result<()> {
    let mut map = load_named_map(home);
    map.insert(named_key(root, name), serde_json::Value::String(path.display().to_string()));
    let p = named_map_path(home);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(&serde_json::Value::Object(map))?)?;
    Ok(())
}

/// Scaffold a brand-new project directory headlessly (no omp exec): create the
/// dir, `git init` (so sweep + project detection recognize it), then seed the
/// full 8sync context (AGENTS.md + agents memory + injected skills block).
/// Used by the dashboard `POST /api/projects/create`. Idempotent on an existing dir.
pub(crate) fn scaffold_project(env: &env_detect::Env, root: &Path) -> Result<()> {
    std::fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    if !root.join(".git").exists() {
        let _ = Command::new("git").arg("-C").arg(root).arg("init").arg("-q").status();
    }
    let stack = detect_stack(root);
    seed_project_context(env, root, &stack)
}

// ═════════════════════════════════════════════════════════════════
// helpers
// ═════════════════════════════════════════════════════════════════

pub(crate) fn detect_project_root(start: &Path) -> Option<PathBuf> {
    let markers = [".git", "Cargo.toml", "package.json", "pyproject.toml", "deno.json", "go.mod"];
    let mut p = start.to_path_buf();
    loop {
        for m in &markers {
            if p.join(m).exists() {
                return Some(p);
            }
        }
        if !p.pop() {
            return None;
        }
    }
}

fn detect_stack(root: &Path) -> Vec<String> {
    let mut s = Vec::new();
    if root.join("Cargo.toml").exists() { s.push("rust".into()); }
    if root.join("package.json").exists() { s.push("node".into()); }
    if root.join("next.config.js").exists()
        || root.join("next.config.ts").exists()
        || root.join("next.config.mjs").exists()
    {
        s.push("nextjs".into());
    }
    if root.join("pyproject.toml").exists() { s.push("python".into()); }
    if root.join("src-tauri").exists() || root.join("tauri.conf.json").exists() {
        s.push("tauri".into());
    }
    if root.join("app.json").exists() && root.join("metro.config.js").exists() {
        s.push("react-native".into());
    }
    if root.join("go.mod").exists() { s.push("go".into()); }
    s
}

fn seed_project_context(env: &env_detect::Env, root: &Path, stack: &[String]) -> Result<()> {
    let agents = root.join("AGENTS.md");
    if !agents.exists() {
        let name = root.file_name().and_then(|s| s.to_str()).unwrap_or("project");
        let stack_lines = if stack.is_empty() {
            "- (auto-detect failed, please fill in)".to_string()
        } else {
            stack.iter().map(|s| format!("- {}", s)).collect::<Vec<_>>().join("\n")
        };
        let content = format!(
            r#"# AGENTS.md — guidance for AI working in `{name}`

> Managed by **8sync**. AI tooling (omp, claude-code, cursor, opencode) MUST
> read this file at the start of every session.

<!-- 8sync:skills:begin -->
<!-- 8sync:skills:end -->

## Stack (auto-detected)
{stack_lines}

## Project memory (đọc TRƯỚC khi bắt đầu bất kỳ task)

| File | Mục đích |
|---|---|
| `agents/PROJECT.md`     | facts cố định (stack, entrypoint, conventions) |
| `agents/KNOWLEDGE.md`   | append-only: AI học được gì về codebase |
| `agents/DECISIONS.md`   | append-only: quyết định kiến trúc |
| `agents/PREFERENCES.md` | append-only: user style preferences |
| `agents/STATE.md`       | việc đang dở, next-step concrete |
| `agents/NOTES.md`       | quick notes appended via `8sync note` |

Session memory được omp tự quản (retain/recall/auto-compact). Không cần capture tay.

## Conventions

- Cite code dạng `path/to/file.rs:23-58` hoặc `file.rs:23`.
- Commit + push + PR qua `8sync ship "msg"` (không git push thô).
- Screenshot UI / PDF / diff: ưu tiên `8sync shot|pdf-img|diff-img` thay vì
  dump text (tiết kiệm token 3-10×).
- Tìm symbol/file: `8sync find <kw>` (không gọi `rg`/`fd` thô).
- Ghi nhớ ý tưởng nhanh: `8sync note "..."` (append vào `agents/NOTES.md`).
"#
        );
        std::fs::write(&agents, crate::brand::render(&content).as_ref())?;
        ui::ok(&format!("seeded {}", agents.display()));
    }

    let agents_dir = root.join("agents");
    std::fs::create_dir_all(&agents_dir)?;
    let project_md = agents_dir.join("PROJECT.md");
    if !project_md.exists() {
        std::fs::write(
            &project_md,
            format!(
                "# Project facts\n\n- name: {}\n- stack: {}\n- created_by: 8sync .\n",
                root.file_name().and_then(|s| s.to_str()).unwrap_or("project"),
                stack.join(", ")
            ),
        )?;
        ui::ok(&format!("seeded {}", project_md.display()));
    }
    for f in ["KNOWLEDGE.md", "DECISIONS.md", "PREFERENCES.md", "STATE.md", "NOTES.md"] {
        let p = agents_dir.join(f);
        if !p.exists() {
            std::fs::write(
                &p,
                format!("# {} (8sync managed — append-only)\n\n_empty_\n", f.trim_end_matches(".md")),
            )?;
        }
    }

    // Re-inject the dynamic skills block (root) + a compact index into every
    // significant sub-folder, so an agent opening any sub-tree still sees the
    // skill rules (progressive disclosure: nearest AGENTS.md wins).
    if let Err(e) = skill::inject_agents_md(&env.home, root) {
        ui::warn(&format!("could not inject AGENTS.md skills block: {}", e));
    }
    if let Err(e) = skill::inject_subfolder_indexes(root) {
        ui::warn(&format!("could not inject sub-folder skill indexes: {}", e));
    }
    Ok(())
}
