//! `ckit feature run` — unattended phase loop for the active feature.
//!
//! Each step is a fresh `omp -p` process (the `/feature` command body rendered
//! with `--auto` / `ship --auto`), so every step starts at subagent depth 0 with
//! an empty context. Advancing is decided here, not by the model: a phase moves
//! on only when `phase_gate.py --check all` exits 0 with `→ DONE` AND the ship
//! step moved STATE `active_phase` (or archived the feature). Anything else stops
//! the loop and lands in `agents/planning/<slug>/RUN-REPORT.md`.
use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{active_slug, frontmatter, Args, PLANNING_REL};
use crate::{assets, brand, ui};

const REPORT: &str = "RUN-REPORT.md";

struct StepRow {
    phase: String,
    step: &'static str,
    exit: String,
    minutes: u64,
    log: PathBuf,
}

pub(super) fn run(root: &Path, a: &Args) -> Result<()> {
    let planning = root.join(PLANNING_REL);
    let Some(slug) = active_slug(&planning) else {
        bail!("no active feature — `{} feature switch <slug>` first", brand::CMD);
    };
    let feat = planning.join(&slug);
    let to = a.to.as_deref().map(phase_num).transpose()?;
    let deadline = a.until.as_deref().map(until_instant).transpose()?;

    ui::header(&format!("{} feature run — {}", brand::CMD, slug));
    let pending = pending_phases(&feat);
    let (cur_phase, _) = position(&feat);
    ui::info(&format!("active phase: {}", cur_phase.as_deref().unwrap_or("(none)")));
    ui::info(&format!(
        "ROADMAP open: {}",
        if pending.is_empty() { "(none)".to_string() } else { pending.join(" ") }
    ));
    if let Some(n) = to {
        ui::info(&format!("stop after:   M{n}"));
    }

    // Preflight — hard failures abort, soft ones warn.
    let mut hard = Vec::new();
    if which::which("omp").is_err() {
        hard.push("`omp` not on PATH".to_string());
    }
    if which::which("python3").is_err() {
        hard.push("`python3` not on PATH (phase_gate.py)".to_string());
    }
    match std::fs::read_to_string(feat.join("ROADMAP.md")) {
        Err(_) => hard.push(format!("{}/ROADMAP.md missing", feat.display())),
        Ok(r) if r.contains("[mục tiêu]") || r.contains("[tên]") => {
            hard.push("ROADMAP.md still has template placeholders — finish `/feature new` (gate 1) first".into())
        }
        Ok(_) => {}
    }
    if cur_phase.is_none() {
        hard.push("STATE.md has no active_phase (nothing to run)".into());
    }
    let dirty = git(root, &["status", "--porcelain", "--untracked-files=no"]);
    match &dirty {
        None => hard.push("not a git repository".into()),
        Some(s) if !s.trim().is_empty() => {
            hard.push("tracked changes present — commit or stash first (engine commits per task file)".into())
        }
        Some(_) => {}
    }
    if cfg!(target_os = "macos") {
        let batt = Command::new("pmset").args(["-g", "batt"]).output().ok();
        let on_ac = batt.map(|o| String::from_utf8_lossy(&o.stdout).contains("AC Power")).unwrap_or(true);
        if !on_ac {
            ui::warn("on battery — plug in; caffeinate cannot stop a closed lid from sleeping on battery");
        }
        ui::info("closing the lid sleeps the Mac unless it is on AC with an external display");
    }
    for h in &hard {
        ui::warn(h);
    }
    if a.dry_run {
        ui::info(if hard.is_empty() { "preflight OK — dry run, nothing changed" } else { "preflight FAILED" });
        return Ok(());
    }
    if !hard.is_empty() {
        bail!("preflight failed ({} issue(s))", hard.len());
    }

    let stamp = stamp();
    let run_dir = root.join(".cache/ckit/feature-run").join(&stamp);
    std::fs::create_dir_all(&run_dir)?;
    let overlay = run_dir.join("omp-overlay.yml");
    std::fs::write(&overlay, "tools:\n  approvalMode: yolo\n")?;
    let gate = run_dir.join("phase_gate.py");
    std::fs::write(
        &gate,
        assets::read("skills/feature/scripts/phase_gate.py").context("bundled phase_gate.py missing")?,
    )?;
    let command = command_body()?;

    let branch = if a.branch {
        let b = format!("{slug}-run-{stamp}");
        let ok = Command::new("git").args(["checkout", "-b", &b]).current_dir(root).status()?.success();
        if !ok {
            bail!("git checkout -b {b} failed");
        }
        b
    } else {
        git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string()
    };
    let start_head = git(root, &["rev-parse", "HEAD"]).unwrap_or_default().trim().to_string();
    let started = now_hm();
    // Keep the machine awake for exactly as long as this process lives.
    let _caffeinate = cfg!(target_os = "macos")
        .then(|| {
            Command::new("caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        })
        .flatten();
    ui::ok(&format!("branch {branch} · logs {}", run_dir.display()));

    let mut rows: Vec<StepRow> = Vec::new();
    let mut gates: Vec<String> = Vec::new();
    let mut done_phases: Vec<String> = Vec::new();
    let stop: String = 'phases: loop {
        let Some(slug_now) = active_slug(&planning).filter(|s| *s == slug) else {
            break "feature finished (archived)".into();
        };
        let feat = planning.join(&slug_now);
        let (phase, next) = position(&feat);
        let Some(phase) = phase else {
            break "ROADMAP finished (no active_phase)".into();
        };
        if next.as_deref() == Some("done") {
            break "ROADMAP finished (next_action: done)".into();
        }
        if let (Some(limit), Ok(n)) = (to, phase_num(&phase)) {
            if n > limit {
                break format!("reached --to M{limit}");
            }
        }
        let steps: &[(&'static str, &str)] = if matches!(next.as_deref(), Some("ship-phase" | "converge")) {
            &[("ship", "ship --auto")]
        } else {
            &[("auto", "--auto"), ("ship", "ship --auto")]
        };
        for (name, args) in steps {
            let budget = Duration::from_secs(a.step_timeout * 60);
            let budget = match deadline {
                Some(d) if d <= Instant::now() => break 'phases format!("--until reached before {phase} {name}"),
                Some(d) => budget.min(d.saturating_duration_since(Instant::now())),
                None => budget,
            };
            let log = run_dir.join(format!("{phase}-{name}.log"));
            ui::info(&format!("▶ {phase} {name}  (log {})", log.display()));
            let t0 = Instant::now();
            let prompt = format!(
                "Unattended run by `{cmd} feature run` — no human is attached: never call `ask`, \
                 follow `references/auto.md` §Chạy không người trông. Do this ONE step for the active \
                 phase, then end your turn.\n\n{body}",
                cmd = brand::CMD,
                body = command.replace("$ARGUMENTS", args),
            );
            let exit = run_omp(root, &overlay, &prompt, &log, budget)?;
            rows.push(StepRow {
                phase: phase.clone(),
                step: name,
                exit: exit.clone(),
                minutes: t0.elapsed().as_secs() / 60,
                log,
            });
            if exit != "0" {
                break 'phases format!("{phase} {name}: omp exit {exit}");
            }
        }
        let gate_dir = if feat.is_dir() { feat.clone() } else { planning.join("_archive").join(&slug) };
        let out = Command::new("python3")
            .arg(&gate)
            .arg(&gate_dir)
            .args(["--phase", &phase, "--check", "all"])
            .current_dir(root)
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let summary = text.lines().find(|l| l.contains('→')).unwrap_or("(no verdict line)").to_string();
        gates.push(format!("{phase}: exit {} · {summary}", out.status.code().unwrap_or(-1)));
        let gate_done = out.status.success() && summary.ends_with("→ DONE");
        let archived = active_slug(&planning).as_deref() != Some(slug.as_str());
        let moved = archived || position(&planning.join(&slug)).0.as_deref() != Some(phase.as_str());
        if !(gate_done && moved) {
            let why = if gate_done { "gate DONE but active_phase did not move" } else { "gate not DONE" };
            break format!("{phase}: {why} — {summary}");
        }
        done_phases.push(phase);
    };

    let report = report(
        root, &planning, &slug, &branch, &started, &start_head, &stop, &done_phases, &rows, &gates, &run_dir,
    );
    // The final phase's ship archives the feature dir; the report follows it.
    let dir = [planning.join(&slug), planning.join("_archive").join(&slug)]
        .into_iter()
        .find(|d| d.is_dir())
        .unwrap_or_else(|| planning.join(&slug));
    let path = dir.join(REPORT);
    std::fs::write(&path, report)?;
    ui::ok(&format!("stopped: {stop}"));
    ui::ok(&format!("report → {}", path.display()));
    Ok(())
}

/// The `/feature` command body (brand-rendered, frontmatter stripped) — the same
/// instructions an interactive `/feature …` gets, with `$ARGUMENTS` left to fill.
fn command_body() -> Result<String> {
    let raw = assets::read("commands/feature.md").context("bundled commands/feature.md missing")?;
    let raw = brand::render(&raw).into_owned();
    let body = match raw.strip_prefix("---\n").and_then(|r| r.split_once("\n---\n")) {
        Some((_, rest)) => rest.trim_start().to_string(),
        None => raw,
    };
    Ok(body)
}

/// Run one `omp -p` step, output → `log`; kill it after `budget`. Returns the
/// exit code as text, or `timeout`.
fn run_omp(root: &Path, overlay: &Path, prompt: &str, log: &Path, budget: Duration) -> Result<String> {
    let f = std::fs::File::create(log)?;
    let mut child = Command::new("omp")
        .arg("-p")
        .arg("--config")
        .arg(overlay)
        .arg(prompt)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(f.try_clone()?)
        .stderr(f)
        .spawn()
        .context("spawn omp")?;
    let end = Instant::now() + budget;
    loop {
        if let Some(st) = child.try_wait()? {
            return Ok(st.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into()));
        }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            return Ok("timeout".into());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// (active_phase, next_action) from STATE.md; `null`/empty → None.
fn position(feat: &Path) -> (Option<String>, Option<String>) {
    let Ok(s) = std::fs::read_to_string(feat.join("STATE.md")) else {
        return (None, None);
    };
    let fm = frontmatter(&s);
    let get = |k: &str| {
        fm.iter()
            .find(|(kk, _)| kk == k)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty() && v != "null" && v != "~")
    };
    (get("active_phase"), get("next_action"))
}

/// ROADMAP phases not yet `[x]`: `- [ ] **M1 …` / `- [~] **M1 …` → `M1`.
fn pending_phases(feat: &Path) -> Vec<String> {
    let Ok(r) = std::fs::read_to_string(feat.join("ROADMAP.md")) else {
        return Vec::new();
    };
    r.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let rest = l.strip_prefix("- [ ] **").or_else(|| l.strip_prefix("- [~] **"))?;
            let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            (id.starts_with('M') && id.len() > 1).then_some(id)
        })
        .collect()
}

/// `M3` / `m3` / `3` → 3.
fn phase_num(p: &str) -> Result<u32> {
    let digits: String = p.trim_start_matches(['M', 'm']).chars().take_while(char::is_ascii_digit).collect();
    digits.parse().with_context(|| format!("bad phase '{p}' — expected M<n>"))
}

/// Next local occurrence of `HH:MM` as an Instant (tomorrow when already past).
fn until_instant(hm: &str) -> Result<Instant> {
    let parse = |s: &str| -> Option<u64> {
        let mut it = s.trim().split(':').map(|x| x.parse::<u64>().ok());
        let (h, m, sec) = (it.next()??, it.next()??, it.next().flatten().unwrap_or(0));
        (h < 24 && m < 60 && sec < 60).then_some(h * 3600 + m * 60 + sec)
    };
    let target = parse(hm).with_context(|| format!("bad --until '{hm}' — expected HH:MM"))?;
    let now = parse(&date("+%H:%M:%S")).context("cannot read local time via `date`")?;
    let wait = (target + 86_400 - now) % 86_400;
    let wait = if wait == 0 { 86_400 } else { wait };
    Ok(Instant::now() + Duration::from_secs(wait))
}

#[allow(clippy::too_many_arguments)]
fn report(
    root: &Path,
    planning: &Path,
    slug: &str,
    branch: &str,
    started: &str,
    start_head: &str,
    stop: &str,
    done: &[String],
    rows: &[StepRow],
    gates: &[String],
    run_dir: &Path,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Run report — {slug}\n");
    let _ = writeln!(s, "- Run: {started} → {}", now_hm());
    let _ = writeln!(s, "- Branch: `{branch}` (not pushed)");
    let _ = writeln!(s, "- Stopped: **{stop}**");
    let _ = writeln!(
        s,
        "- Phases DONE: {}\n",
        if done.is_empty() { "none".to_string() } else { done.join(", ") }
    );
    let _ = writeln!(s, "## Steps\n\n| Phase | Step | Exit | Minutes | Log |\n|---|---|---|---|---|");
    for r in rows {
        let _ = writeln!(s, "| {} | {} | {} | {} | `{}` |", r.phase, r.step, r.exit, r.minutes, rel(root, &r.log));
    }
    let _ = writeln!(s, "\n## Phase gate\n");
    for g in gates {
        let _ = writeln!(s, "- {g}");
    }
    let log = git(root, &["log", "--oneline", &format!("{start_head}..HEAD")]).unwrap_or_default();
    let _ = writeln!(s, "\n## Commits\n\n```\n{}```", if log.is_empty() { "(none)\n".into() } else { log });
    let state = [planning.join(slug), planning.join("_archive").join(slug)]
        .iter()
        .find_map(|d| std::fs::read_to_string(d.join("STATE.md")).ok())
        .unwrap_or_default();
    let blockers = section(&state, "Blockers");
    let _ = writeln!(s, "\n## Blockers / NEEDS-CONFIRM (STATE.md)\n\n{}", if blockers.is_empty() { "None" } else { &blockers });
    let _ = writeln!(s, "\nLogs: `{}`", rel(root, run_dir));
    s
}

/// Body of the first markdown heading containing `name`, up to the next heading.
fn section(text: &str, name: &str) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    for l in text.lines() {
        if l.starts_with('#') {
            if inside {
                break;
            }
            inside = l.contains(name);
            continue;
        }
        if inside {
            out.push(l);
        }
    }
    out.join("\n").trim().to_string()
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").args(args).current_dir(root).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn date(fmt: &str) -> String {
    Command::new("date")
        .arg(fmt)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn stamp() -> String {
    date("+%Y%m%d-%H%M")
}

fn now_hm() -> String {
    date("+%Y-%m-%d %H:%M")
}
