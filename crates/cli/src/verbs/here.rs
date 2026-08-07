use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{env_detect, ui, verbs::skill};

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          8sync .                       seed agents/* context and resume the last omp session (omp --continue)
          8sync . work                  resume the named session bucket 'work' (created by `8sync new work`)

        BEHAVIOR
          · Walks up from cwd to find the project root (.git / Cargo.toml / package.json / pyproject.toml / go.mod / deno.json).
          · Detects stack (rust/node/python/nextjs/tauri/react-native/go) and seeds AGENTS.md + agents/{PROJECT,KNOWLEDGE,DECISIONS,PREFERENCES,STATE,NOTES}.md when missing.
          · Re-injects the dynamic skills block in AGENTS.md so omp sees an up-to-date skill list.
          · Execs `omp --continue` in the project root (resumes the latest session). Pass a NAME to resume that named bucket instead.
          · To start FRESH instead of resuming, use `8sync new` (optionally `8sync new <name>`).
          · If omp is missing, drops into the user shell instead (run `8sync setup` to fix).
    "}
)]
pub struct Args {
    /// Optional session name — resume the named bucket created by `new <name>` (default: the unnamed session).
    pub name: Option<String>,
}

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          8sync new                     seed agents/* context and start a FRESH omp session (does NOT resume)
          8sync new fix-auth            start a fresh, named session bucket 'fix-auth' — return to it later with `8sync . fix-auth`

        BEHAVIOR
          · Same seeding as `8sync .` (project root detection, AGENTS.md + agents/* memory, skills block).
          · Execs `omp` WITHOUT --continue, so a brand-new session starts instead of resuming the last one.
          · With a NAME, the session lives in its own isolated bucket (omp --session-dir ~/.omp/agent/named/<name>),
            so it never collides with the default session and can be resumed via `8sync . <name>`.
          · omp has no custom-title flag; it auto-generates the session title. The NAME is the CLI bucket you reopen by.
    "}
)]
pub struct NewArgs {
    /// Optional session name — an isolated, resumable bucket (reopen with `. <name>`).
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

/// Shared body for `8sync .` (resume) and `8sync new` (fresh): detect the
/// project root, seed context, then exec omp. `fresh` drops `--continue` so a
/// new session starts; `name`, when set, isolates the session in its own
/// `--session-dir` bucket (resumable by the same name).
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

    if which::which("omp").is_ok() {
        let cfg = crate::models::ModelConfig::load();
        let mut cmd = Command::new("omp");
        cmd.arg("--cwd").arg(&root).args(cfg.resume_flags());
        if let Some(n) = name {
            let dir = named_session_dir(&env.home, n);
            let _ = std::fs::create_dir_all(&dir);
            cmd.arg("--session-dir").arg(&dir);
        }
        if !fresh {
            cmd.arg("--continue");
        }
        ui::ok(&format!("→ exec: {}", session_desc(fresh, name)));
        let err = cmd.current_dir(&root).status();
        match err {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(anyhow::anyhow!("omp exited with {}", s)),
            Err(e) => Err(anyhow::anyhow!("could not exec omp: {}", e)),
        }
    } else {
        ui::warn("omp not installed — run `8sync setup` first. Falling back to $SHELL.");
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let _ = Command::new(&shell).current_dir(&root).status();
        Ok(())
    }
}

/// Human-readable description of the omp launch for the status line.
fn session_desc(fresh: bool, name: Option<&str>) -> String {
    match (fresh, name) {
        (true, Some(n)) => format!("omp (new session '{n}')"),
        (true, None) => "omp (new session)".to_string(),
        (false, Some(n)) => format!("omp --continue (session '{n}')"),
        (false, None) => "omp --continue".to_string(),
    }
}

/// Isolated session-storage bucket for a named session: `~/.omp/agent/named/<slug>`.
/// Kept under omp's home session area (never the project tree) so session logs
/// are never at risk of being committed. The slug keeps the dir name filesystem-safe.
fn named_session_dir(home: &Path, name: &str) -> PathBuf {
    let slug: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    home.join(".omp/agent/named").join(slug)
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
