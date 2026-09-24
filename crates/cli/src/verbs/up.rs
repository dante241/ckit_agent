// `ckit up` — update the ckit binary from a GitHub Release, then apply it.
//
// Default: pull the latest tag. With `--to <tag>` pin a specific release
// for reproducibility / downgrade (e.g. `ckit up --to v0.6.10` to roll
// every machine to a known-stable baseline).
//
// After the binary swap it runs `<new binary> harness global`, so the bundled
// files embedded in the NEW binary (skills, APPEND_SYSTEM, omp extensions like
// gw-quota.ts, MCP) land in ~/.omp without a second command. It must spawn the
// new binary: this process still carries the OLD embedded assets.
//
// Decoupled from omp on purpose: omp self-updates via `omp update`. System
// pkgs (pacman/AUR) untouched — user runs `paru -Syu` on their own schedule.

use std::process::Command;

use anyhow::Result;
use clap::Args as ClapArgs;

use crate::{ui, verbs::selfup};

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          ckit up                       update to the latest GitHub Release
          ckit up --to v0.6.10          pin/downgrade to a specific tag
    "}
)]
pub struct Args {
    /// Pin to a specific release tag (e.g. `v0.6.10`). Default: latest.
    #[arg(long, value_name = "TAG")]
    pub to: Option<String>,
}

pub fn run(a: Args) -> Result<()> {
    ui::header("ckit up");
    let installed = match a.to {
        Some(tag) => selfup::install_tag(&tag)?,
        None      => selfup::run_self_update(true)?,
    };
    let Some(bin) = installed else {
        ui::info("already up to date — nothing to do.");
        return Ok(());
    };
    ui::ok("ckit binary updated");

    // Apply the new release's managed files (idempotent; byte-identical files
    // are skipped, user-owned files are never overwritten).
    ui::step("applying new release → ckit harness global");
    let applied = Command::new(&bin)
        .args(["harness", "global"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    ui::header("next steps");
    if applied {
        ui::step("restart omp (quit, then `ckit .`) — omp loads skills/extensions only at session start");
    } else {
        ui::warn("apply step failed — run it manually: `ckit harness global`, then restart omp");
    }
    ui::step("omp update        update the AI engine (ckit up does NOT touch omp)");
    ui::info("scope: `ckit up` updates ckit only. omp → `omp update` · system pkgs → `paru -Syu`.");
    Ok(())
}
