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
        ui::info("done — re-run any ckit command to pick up the new binary");
    }
    ui::info("note: `ckit up` only updates ckit. For omp run `omp update`; for system pkgs run `paru -Syu`.");
    Ok(())
}
