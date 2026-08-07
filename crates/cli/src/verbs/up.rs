// `ckit up` — update the ckit binary from a GitHub Release.
//
// Default: pull the latest tag. With `--to <tag>` pin a specific release
// for reproducibility / downgrade (e.g. `ckit up --to v0.6.10` to roll
// every machine to a known-stable baseline).
//
// Decoupled from omp on purpose: omp self-updates via `omp update`. System
// pkgs (pacman/AUR) untouched — user runs `paru -Syu` on their own schedule.

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
    let updated = match a.to {
        Some(tag) => selfup::install_tag(&tag)?,
        None      => selfup::run_self_update(true)?,
    };
    if updated {
        ui::ok("ckit binary updated");
        // ckit up ONLY swaps the binary. Skills / 00-force-load.md /
        // APPEND_SYSTEM.md / MCP under ~/.omp do NOT auto-sync, and omp +
        // system pkgs update on their own tracks. Spell the chain out so the
        // user does not run a stale harness against a new binary.
        ui::header("next steps");
        ui::step("omp update        update the AI engine (ckit up does NOT touch omp)");
        ui::step("ckit harness global   re-deploy skills + 00-force-load + APPEND_SYSTEM + MCP globally (they do NOT auto-sync)");
        ui::step("ckit doctor       verify everything is in place");
        ui::step("ckit .            back to work — resume your omp session");
        ui::info("small update (verb logic only)? `ckit .` alone is enough — skip the `ckit harness global` re-deploy.");
    } else {
        ui::info("already up to date — nothing to do.");
    }
    ui::info("scope: `ckit up` updates ckit only. omp → `omp update` · system pkgs → `paru -Syu`.");
    Ok(())
}
