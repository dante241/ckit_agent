//! Domain skill+rule packs — `assets/packs/<name>/{skills/<sub>,rules/*.md}`.
//!
//! A pack bundles a project-type's SKILL.md set (e.g. VTiger/PHP: `action`,
//! `field`, `database`, …) together with its `.omp/rules/*.md` convention
//! files, so a new project of that type gets both in one `skill add pack:<name>`.
//! Always project-local (never deployed to `~/.omp/skills`) — domain rules only
//! make sense scoped to a matching project, unlike generic always-on skills.
use anyhow::Result;
use std::path::Path;

use super::meta::audit_skill_layout;
use crate::{assets, ui};

/// Every sub-skill name bundled under `assets/packs/<pack>/skills/`.
fn pack_skill_names(pack: &str) -> Vec<String> {
    let prefix = format!("packs/{}/skills/", pack);
    let mut names: Vec<String> = assets::iter_under(&prefix)
        .iter()
        .filter_map(|p| p.strip_prefix(&prefix)?.split('/').next().map(String::from))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// One discoverable pack: `(name, description)` from `assets/packs/<name>/pack.toml`.
pub(crate) struct PackInfo {
    pub(crate) name: String,
    pub(crate) description: String,
}

/// Every pack bundled under `assets/packs/<name>/pack.toml`, sorted by name.
pub(crate) fn discover_packs() -> Vec<PackInfo> {
    let mut names: Vec<String> = assets::iter_under("packs/")
        .iter()
        .filter_map(|p| p.strip_prefix("packs/")?.split('/').next().map(String::from))
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let description = assets::read(&format!("packs/{}/pack.toml", name))
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("description = ").map(|v| v.trim_matches('"').to_string()))
                })
                .unwrap_or_default();
            PackInfo { name, description }
        })
        .collect()
}

/// A pack counts as installed once it has a `[<name>]` entry with
/// `src = "pack:<name>"` in the project manifest (`agents/skills.toml`) —
/// the same registry every other source (`git`/`builtin`/`path`) is tracked
/// by; `skill update vtiger-php` re-heals a hand-deleted skill/rule file.
pub(crate) fn is_pack_installed(root: &Path, name: &str) -> bool {
    let manifest = root.join("agents/skills.toml");
    let reg = super::discover::read_registry(&manifest);
    reg.get(name).is_some_and(|e| e.src == format!("pack:{}", name))
}

/// Deploy every skill in pack `name` into `<root>/.omp/skills/<sub>` and every
/// rule file into `<root>/.omp/rules/`. Project-local only — packs carry
/// domain convention (PHP/VTiger, …) that doesn't belong in `~/.omp/skills`.
/// Returns `(skills installed, rule files written)`. Errors only if the pack
/// doesn't exist under `assets/packs/`.
pub(crate) fn install_pack(root: &Path, name: &str, force: bool) -> Result<(usize, usize)> {
    let skill_names = pack_skill_names(name);
    let rules_prefix = format!("packs/{}/rules/", name);
    let has_rules = !assets::iter_under(&rules_prefix).is_empty();
    if skill_names.is_empty() && !has_rules {
        anyhow::bail!("no bundled pack `{}` (assets/packs/{}/ not found)", name, name);
    }

    let mut skills_written = 0usize;
    for sub in &skill_names {
        let target = root.join(".omp/skills").join(sub);
        if target.exists() && !force {
            ui::skip(sub, "already installed (--force to overwrite)");
            continue;
        }
        let asset_prefix = format!("packs/{}/skills/{}", name, sub);
        let (w, _) = assets::install_tree(&asset_prefix, &target)?;
        audit_skill_layout(&target);
        if w > 0 {
            skills_written += 1;
        }
    }

    let mut rules_written = 0usize;
    if has_rules {
        let rules_dir = root.join(".omp/rules");
        let (w, _) = assets::install_tree(&rules_prefix, &rules_dir)?;
        rules_written = w;
    }

    ui::ok(&format!(
        "pack `{}` → {} skill(s), {} rule file(s) → {}",
        name,
        skill_names.len(),
        rules_written,
        root.display()
    ));
    Ok((skills_written, rules_written))
}
