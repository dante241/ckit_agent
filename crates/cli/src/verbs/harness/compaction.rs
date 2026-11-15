//! `8sync harness compaction [pct|tokens]` — view or set omp's auto-compaction threshold
//! in `~/.omp/agent/config.yml`. omp honours `compaction.thresholdTokens` over
//! `compaction.thresholdPercent`, so both are shown. `N` (1..=95, `%` optional) sets the
//! percent and drops `thresholdTokens` (else the percent has no effect); `300k` or a
//! number above 95 sets `thresholdTokens`. No value prints the effective threshold.
use anyhow::{anyhow, Result};
use std::path::Path;

use crate::ui;

pub(crate) fn harness_compaction(home: &Path, value: Option<&str>) -> Result<()> {
    ui::header("8sync harness compaction");
    let cfg = home.join(".omp/agent/config.yml");

    let Some(raw) = value else {
        match (current(&cfg, TOKENS), current(&cfg, PERCENT)) {
            (Some(t), _) => ui::ok(&format!(
                "compaction.thresholdTokens = {t} — omp auto-compacts at {t} tokens (overrides thresholdPercent)"
            )),
            (None, Some(p)) => ui::ok(&format!(
                "compaction.thresholdPercent = {p} — omp auto-compacts at {p}% of the context window"
            )),
            (None, None) => ui::info(
                "no compaction threshold set — omp uses its default; set it with `8sync harness compaction 50` or `300k`",
            ),
        }
        return Ok(());
    };

    let s = raw.trim().to_ascii_lowercase();
    let (num, kilo) = match s.strip_suffix('k') {
        Some(n) => (n, true),
        None => (s.trim_end_matches('%'), false),
    };
    let n: u64 = num
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid threshold `{raw}` — use a percent 1..95 or tokens like 300k"))?;
    if kilo || n > 95 {
        let tokens = if kilo { n * 1000 } else { n };
        if tokens < 10_000 {
            return Err(anyhow!("{tokens} tokens is too low to compact usefully — use ≥ 10k"));
        }
        set_key(&cfg, TOKENS, tokens)?;
        ui::ok(&format!("compaction.thresholdTokens = {tokens} → {}", cfg.display()));
    } else {
        if n == 0 {
            return Err(anyhow!("percent 0 out of range — use 1..95"));
        }
        set_key(&cfg, PERCENT, n)?;
        remove_key(&cfg, TOKENS)?;
        ui::ok(&format!("compaction.thresholdPercent = {n} → {} (thresholdTokens removed)", cfg.display()));
    }
    ui::info("takes effect on the next omp session (subagents inherit it)");
    Ok(())
}

const PERCENT: &str = "thresholdPercent";
const TOKENS: &str = "thresholdTokens";

/// Current `<key>:` value from config.yml, if present and a positive number.
fn current(cfg: &Path, key: &str) -> Option<u64> {
    let s = std::fs::read_to_string(cfg).ok()?;
    let prefix = format!("{key}:");
    s.lines()
        .find_map(|l| l.trim().strip_prefix(prefix.as_str()).map(|r| r.trim().parse().ok()))
        .flatten()
        .filter(|v| *v > 0)
}

/// Ensure `compaction.thresholdPercent` HAS a value — write `pct` only when the
/// key is absent (never override a user's explicit setting). Used by
/// `8sync harness global` as the Anthropic token-optimizer default (50%).
pub(crate) fn ensure_threshold_default(home: &Path, pct: u64) -> Result<()> {
    let cfg = home.join(".omp/agent/config.yml");
    if current(&cfg, PERCENT).is_some() || current(&cfg, TOKENS).is_some() {
        return Ok(());
    }
    set_key(&cfg, PERCENT, pct)?;
    ui::ok(&format!("compaction.thresholdPercent = {pct} (default) → {}", cfg.display()));
    Ok(())
}

/// Set `compaction.<key>` in config.yml. Updates the line in place if present
/// (indentation preserved), inserts under an existing `compaction:` block, or
/// appends a fresh block. omp normalizes config.yml, so a line-level edit is safe.
fn set_key(cfg: &Path, key: &str, val: u64) -> Result<()> {
    if let Some(p) = cfg.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut s = std::fs::read_to_string(cfg).unwrap_or_default();

    let prefix = format!("{key}:");
    if s.lines().any(|l| l.trim().starts_with(prefix.as_str())) {
        s = s
            .lines()
            .map(|l| {
                if l.trim().starts_with(prefix.as_str()) {
                    let indent: String = l.chars().take_while(|c| c.is_whitespace()).collect();
                    format!("{indent}{key}: {val}")
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else if s.lines().any(|l| l.starts_with("compaction:")) {
        s = s
            .lines()
            .map(|l| {
                if l.starts_with("compaction:") {
                    format!("{l}\n  {key}: {val}")
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else {
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(&format!("\ncompaction:\n  {key}: {val}\n  idleEnabled: true\n"));
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
    std::fs::write(cfg, s)?;
    Ok(())
}

/// Drop the `compaction.<key>` line, if any.
fn remove_key(cfg: &Path, key: &str) -> Result<()> {
    let Ok(s) = std::fs::read_to_string(cfg) else { return Ok(()) };
    let prefix = format!("{key}:");
    if !s.lines().any(|l| l.trim().starts_with(prefix.as_str())) {
        return Ok(());
    }
    let mut out: String = s
        .lines()
        .filter(|l| !l.trim().starts_with(prefix.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    std::fs::write(cfg, out)?;
    Ok(())
}
