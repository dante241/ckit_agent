//! `8sync harness eval` — quality probe for the engineering loop. Runs a fixed
//! task-suite through omp non-interactively, scores each task with a
//! deterministic self-check, and writes a JSON scorecard (+ optional baseline
//! diff). Model + network, non-deterministic — a periodic quality SIGNAL, not a
//! CI gate; `--runs N` measures how much one task's outcome varies.
//!
//! The grader exists only AFTER omp exits, in a separate grading dir (see
//! `grade`): the agent can neither read nor edit `verify.sh` or the `hidden/`
//! tree, and `hidden/` overwrites its copies of those paths (restores the
//! original tests, Cargo.toml), so deleting or weakening a test cannot pass.
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::assets;
use crate::{env_detect, ui};

/// Per-task omp wall-clock cap (seconds). A stuck task fails on its verifier.
const MAX_TIME_SECS: &str = "300";

#[derive(Serialize, Deserialize)]
struct TaskResult {
    name: String,
    /// Every run passed.
    pass: bool,
    /// Mean wall time of one run.
    secs: u64,
    #[serde(default)]
    passes: u32,
    #[serde(default)]
    runs: u32,
}

#[derive(Serialize, Deserialize)]
struct EvalReport {
    stamp: String,
    /// Passing task-runs.
    passed: usize,
    /// Task-runs attempted (tasks × runs).
    total: usize,
    #[serde(default)]
    runs: u32,
    results: Vec<TaskResult>,
}

/// Unique fixture names = first path segment under the embedded `eval/` tree.
fn fixture_names() -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for p in assets::iter_under("eval/") {
        if let Some(rest) = p.strip_prefix("eval/") {
            if let Some(name) = rest.split('/').next() {
                if !name.is_empty() {
                    set.insert(name.to_string());
                }
            }
        }
    }
    set.into_iter().collect()
}

/// One fresh attempt at `name`: setup → omp → grade. Anything the agent breaks
/// while grading (its dir gone, a file where a dir should be) fails this task
/// only, never the whole suite.
fn run_once(name: &str, dir: &Path, task: &str) -> Result<(bool, u64)> {
    let _ = std::fs::remove_dir_all(dir.with_extension("grade")); // a crashed earlier grading
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    assets::install_tree(&format!("eval/{}/setup", name), dir)?;

    let t0 = Instant::now();
    let out = Command::new("omp")
        .args(["-p", "--no-session", "--auto-approve", "--max-time", MAX_TIME_SECS])
        .arg(task)
        .current_dir(dir)
        .output();
    let secs = t0.elapsed().as_secs();
    let Ok(o) = out else { return Ok((false, secs)) };
    let pass = grade(name, dir, &o.stdout).unwrap_or(false);
    // It holds the hidden tests and verify.sh: never leave it for a later run's agent to read.
    let _ = std::fs::remove_dir_all(dir.with_extension("grade"));
    Ok((pass, secs))
}

/// Grades in a fresh sibling dir, so nothing the agent left in its own dir can
/// rewrite the grader: only its regular files and directories are copied (no
/// symlinks, no dot-directories such as `.cargo`/`.git`, no `target/`), then
/// `hidden/` overwrites them and `verify.sh` runs. The hidden `Cargo.toml`s set
/// `build = false`, so an agent `build.rs` never runs either.
fn grade(name: &str, work: &Path, stdout: &[u8]) -> Result<bool> {
    let g = work.with_extension("grade");
    let _ = std::fs::remove_dir_all(&g);
    std::fs::create_dir_all(&g)?;
    copy_agent_tree(work, &g)?;
    assets::install_tree(&format!("eval/{}/hidden", name), &g)?;
    let Some(v) = assets::read(&format!("eval/{}/verify.sh", name)) else { return Ok(false) };
    std::fs::write(g.join(".omp-out.txt"), stdout)?;
    std::fs::write(g.join(".eval-verify.sh"), v)?;
    Ok(Command::new("sh")
        .arg(".eval-verify.sh")
        .arg(".omp-out.txt")
        .current_dir(&g)
        .status()
        .map(|s| s.success())
        .unwrap_or(false))
}

fn copy_agent_tree(src: &Path, dst: &Path) -> Result<()> {
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ft = e.file_type()?; // does not follow symlinks
        let name = e.file_name();
        let to = dst.join(&name);
        if ft.is_dir() {
            if name.to_string_lossy().starts_with('.') || name == "target" {
                continue;
            }
            std::fs::create_dir_all(&to)?;
            copy_agent_tree(&e.path(), &to)?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

fn pct(passed: usize, total: usize) -> usize {
    (passed * 100).checked_div(total).unwrap_or(0)
}

pub(crate) fn harness_eval(env: &env_detect::Env, baseline: bool, runs: u32) -> Result<()> {
    ui::header("8sync harness eval — loop quality probe");
    if which::which("omp").is_err() {
        ui::warn("omp not found — eval needs omp on PATH");
        return Ok(());
    }
    let names = fixture_names();
    if names.is_empty() {
        ui::warn("no eval fixtures bundled");
        return Ok(());
    }
    let runs = runs.max(1);
    let cache = env.home.join(".cache/ckit/eval");
    ui::info(&format!(
        "running {} task(s) × {} run(s) through omp (model + network; non-deterministic)…",
        names.len(),
        runs
    ));
    println!();

    let mut results: Vec<TaskResult> = Vec::new();
    for name in &names {
        let task = assets::read(&format!("eval/{}/task.md", name)).unwrap_or_default();
        let dir = cache.join(name);
        let (mut passes, mut secs) = (0u32, 0u64);
        for _ in 0..runs {
            let (pass, s) = run_once(name, &dir, &task)?;
            passes += pass as u32;
            secs += s;
        }
        let mean = secs / runs as u64;
        let (mark, flaky) = match passes {
            p if p == runs => ("✓", ""),
            0 => ("✗", ""),
            _ => ("~", "  flaky"),
        };
        println!("   {} {:<26} {}/{} {:>5}s{}", mark, name, passes, runs, mean, flaky);
        results.push(TaskResult { name: name.clone(), pass: passes == runs, secs: mean, passes, runs });
    }

    let passed: usize = results.iter().map(|r| r.passes as usize).sum();
    let total = names.len() * runs as usize;
    let flaky = results.iter().filter(|r| r.passes > 0 && r.passes < r.runs).count();
    println!();
    ui::info(&format!("score: {}/{} task-runs passed ({}%)", passed, total, pct(passed, total)));
    if runs > 1 {
        ui::info(&format!("flaky: {} of {} task(s) passed in some runs and failed in others", flaky, names.len()));
    }

    let report = EvalReport { stamp: super::memory::now_stamp(), passed, total, runs, results };
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();

    // Scorecards live in the gitignored cache (machine-local + model-dependent;
    // not committed repo state). The baseline is the reference future runs diff.
    let out_dir = cache.clone();
    let _ = std::fs::create_dir_all(&out_dir);
    let run_path = out_dir.join(format!("eval-{}.json", report.stamp.replace(':', "-")));
    if std::fs::write(&run_path, &json).is_ok() {
        ui::ok(&format!("scorecard → {}", run_path.display()));
    }

    let baseline_path = out_dir.join("eval-baseline.json");
    if baseline {
        if std::fs::write(&baseline_path, &json).is_ok() {
            ui::ok(&format!("baseline saved → {}", baseline_path.display()));
        }
    } else if let Ok(prev) = std::fs::read_to_string(&baseline_path) {
        if let Ok(base) = serde_json::from_str::<EvalReport>(&prev) {
            // Pass RATE, not counts: the suite size and --runs may differ between runs.
            let (was, now) = (pct(base.passed, base.total) as i64, pct(passed, total) as i64);
            ui::info(&format!(
                "vs baseline ({}): {}% ({}/{}) → {}% ({}/{}) ({}{} pts)",
                base.stamp,
                was,
                base.passed,
                base.total,
                now,
                passed,
                total,
                if now >= was { "+" } else { "" },
                now - was
            ));
        }
    }
    Ok(())
}

/// `8sync harness eval --project` — agent-team READINESS scorecard for the
/// current repo: per-role capability coverage (%), computed from real checks
/// (engines on PATH, skills present, memory spine, stack signals). This is
/// "what is the team equipped with HERE", NOT output quality — that is the
/// model+network `harness eval` loop probe. Honest + deterministic + offline.
#[derive(serde::Serialize)]
pub(crate) struct RoleScore {
    pub role: String,
    pub pct: usize,
    pub detail: Vec<String>,
}
#[derive(serde::Serialize)]
pub(crate) struct EvalData {
    pub overall: usize,
    pub total: usize,
    pub present: usize,
    pub roles: Vec<RoleScore>,
}

/// Compute the agent-team READINESS scorecard for the current repo (per-role
/// capability coverage %). Deterministic, offline. None if not in a project.
/// Shared by the CLI (`harness eval --project`) and the web dashboard `/api/eval`.
pub(crate) fn eval_project_data(home: &std::path::Path) -> Option<EvalData> {
    use crate::verbs::skill::discover::detect_current_project_root;
    let root = detect_current_project_root()?;
    let skill = |n: &str| home.join(".omp/skills").join(n).exists() || root.join(".omp/skills").join(n).exists();
    let bin = |n: &str| which::which(n).is_ok();
    let has = |p: &str| root.join(p).exists();
    let pkg = std::fs::read_to_string(root.join("package.json")).unwrap_or_default();
    let cfg = std::fs::read_to_string(home.join(".omp/agent/config.yml")).unwrap_or_default();
    let dep = |k: &str| pkg.contains(k);
    let frontend = dep("react") || dep("vue") || dep("next") || dep("svelte") || dep("\"vite\"");
    let backend = dep("encore.dev") || dep("express") || dep("fastify") || dep("@nestjs")
        || has("Cargo.toml") || has("go.mod") || has("requirements.txt") || has("pyproject.toml");
    let build_cmd = pkg.contains("\"build\"") || has("Cargo.toml") || has("Makefile") || has("go.mod");
    let test_cmd = pkg.contains("\"test\"") || has("Cargo.toml") || pkg.contains("vitest") || pkg.contains("jest");
    let cbm = bin("codebase-memory-mcp");
    let roles_raw: Vec<(&str, Vec<(&str, bool)>)> = vec![
        ("dev", vec![("codegraph", has(".codegraph")), ("cbm-graph", cbm), ("build", build_cmd), ("karpathy+ponytail", skill("karpathy-guidelines") && skill("ponytail"))]),
        ("qa/testing", vec![("test", test_cmd), ("full-flow", skill("full-flow")), ("browser-testing", skill("browser-testing-with-devtools"))]),
        ("research", vec![("omp/web_search", bin("omp")), ("agent-reach|deep-research", skill("agent-reach") || skill("deep-research")), ("last30days", skill("last30days"))]),
        ("ba/po", vec![("planning", skill("planning-and-task-breakdown")), ("spec-driven", skill("spec-driven-development")), ("STATE+DECISIONS", has("agents/STATE.md") && has("agents/DECISIONS.md"))]),
        ("fe", vec![("frontend-stack", frontend), ("impeccable+taste", skill("impeccable") && skill("taste-skill")), ("senior-frontend", skill("senior-frontend"))]),
        ("be", vec![("backend-stack", backend), ("api-design", skill("api-and-interface-design")), ("security", skill("senior-security") || skill("security-and-hardening"))]),
        ("docs", vec![("docs-skill", skill("documentation-and-adrs")), ("AGENTS.md", has("AGENTS.md")), ("CHANGELOG", has("CHANGELOG.md"))]),
        ("memory/learn", vec![("Mnemopi-ON", cfg.contains("backend: mnemopi")), ("KNOWLEDGE+PLAYBOOKS", has("agents/KNOWLEDGE.md") && has("agents/PLAYBOOKS.md")), ("cbm-graph", cbm)]),
        ("token-opt", vec![("codegraph", bin("codegraph")), ("cbm", cbm)]),
    ];
    let mut roles_out = Vec::new();
    let (mut tp, mut tn) = (0usize, 0usize);
    for (role, checks) in &roles_raw {
        let p = checks.iter().filter(|(_, ok)| *ok).count();
        let n = checks.len();
        tp += p; tn += n;
        let pct = 100 * p / n;
        let detail: Vec<String> = checks.iter().map(|(l, ok)| format!("{}{}", if *ok { "✓" } else { "·" }, l)).collect();
        roles_out.push(RoleScore { role: role.to_string(), pct, detail });
    }
    let overall = if tn > 0 { 100 * tp / tn } else { 0 };
    Some(EvalData { overall, total: tn, present: tp, roles: roles_out })
}

/// `8sync harness eval --project` — print the readiness scorecard (CLI view).
pub(crate) fn harness_eval_project(env: &env_detect::Env) -> Result<()> {
    ui::header("8sync harness eval --project — agent-team readiness scorecard");
    let Some(data) = eval_project_data(&env.home) else {
        ui::warn("not inside a project — cd into a repo first");
        return Ok(());
    };
    ui::info("readiness = capabilities the team HAS here (not output quality — that's `harness eval`)");
    println!();
    for r in &data.roles {
        let line = format!("  {:<13} {:>3}%  {}", r.role, r.pct, r.detail.join("  "));
        if r.pct == 100 { ui::ok(&line); } else if r.pct >= 50 { ui::info(&line); } else { ui::warn(&line); }
    }
    println!();
    ui::info(&format!("OVERALL team readiness: {}%  ({}/{} capabilities present)", data.overall, data.present, data.total));
    ui::info("close gaps (·): `8sync harness` (engines+skills) · enable Mnemopi · add a stack skill · seed agents/*");
    Ok(())
}
