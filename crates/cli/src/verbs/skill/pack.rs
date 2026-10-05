//! Domain skill+rule packs — `assets/packs/<name>/{skills/<sub>,rules/*.md}`.
//!
//! A pack bundles a project-type's SKILL.md set (e.g. VTiger/PHP: `action`,
//! `field`, `database`, …) together with its `.omp/rules/*.md` convention
//! files, so a new project of that type gets both in one `skill add pack:<name>`.
//! Always project-local (never deployed to `~/.omp/skills`) — domain rules only
//! make sense scoped to a matching project, unlike generic always-on skills.
use anyhow::Result;
use std::collections::BTreeMap;
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
///
/// `.omp` is usually untracked, so a local edit must never be lost silently:
/// `.omp/pack-<name>.lock` records the hash of every file as installed. A file
/// is overwritten only while it still has that hash; an edited (or never
/// recorded) file is kept unless `force`, which saves it as `<file>.bak` first.
/// Errors only if the pack doesn't exist under `assets/packs/`.
pub(crate) fn install_pack(root: &Path, name: &str, force: bool) -> Result<()> {
    let pack_prefix = format!("packs/{}/", name);
    let files: Vec<String> = assets::iter_under(&pack_prefix)
        .into_iter()
        .filter(|p| p[pack_prefix.len()..].starts_with("skills/") || p[pack_prefix.len()..].starts_with("rules/"))
        .collect();
    if files.is_empty() {
        anyhow::bail!("no bundled pack `{}` (assets/packs/{}/ not found)", name, name);
    }

    let omp = root.join(".omp");
    let lock_path = omp.join(format!("pack-{}.lock", name));
    let mut lock = read_lock(&lock_path);
    let (mut written, mut unchanged) = (0usize, 0usize);
    let mut kept: Vec<String> = Vec::new();
    for asset in &files {
        let rel = &asset[pack_prefix.len()..];
        let Some(body) = assets::read(asset) else {
            continue;
        };
        let body = if rel.ends_with(".md") || rel.ends_with(".txt") { crate::brand::render(&body).into_owned() } else { body };
        let target = omp.join(rel);
        let current = std::fs::read_to_string(&target).ok();
        match current {
            Some(cur) if cur == body => unchanged += 1,
            Some(cur) if lock.get(rel) != Some(&fnv1a(&cur)) => {
                if !force {
                    kept.push(rel.to_string());
                    continue;
                }
                std::fs::rename(&target, omp.join(format!("{}.bak", rel)))?;
                write_file(&target, &body)?;
                written += 1;
            }
            _ => {
                write_file(&target, &body)?;
                written += 1;
            }
        }
        lock.insert(rel.to_string(), fnv1a(&body));
    }
    write_lock(&lock_path, &lock)?;
    for sub in pack_skill_names(name) {
        audit_skill_layout(&omp.join("skills").join(sub));
    }

    // Pack-root `RULES.md` → project sticky always-apply rule (`.omp/RULES.md`).
    // Backup-on-diff (assets::install) so a user's local edits are never clobbered.
    let rules_md = format!("packs/{}/RULES.md", name);
    if assets::read(&rules_md).is_some() {
        assets::install(&rules_md, &omp.join("RULES.md"), force)?;
    }

    ui::ok(&format!("pack `{}` → {} file(s) written, {} unchanged → {}", name, written, unchanged, omp.display()));
    if !kept.is_empty() {
        ui::warn(&format!(
            "{} locally edited file(s) kept (rerun with --force to overwrite, old copy saved as .bak):",
            kept.len()
        ));
        for rel in &kept {
            ui::info(&format!("  .omp/{}", rel));
        }
    }
    Ok(())
}

fn write_file(target: &Path, body: &str) -> Result<()> {
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(target, body)?;
    #[cfg(unix)]
    if target.extension().is_some_and(|e| e == "sh") {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// FNV-1a 64 — stable across builds (std's hasher is not), enough to detect edits.
fn fnv1a(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", h)
}

/// Lock format: one `<hash>\t<path relative to .omp>` per line.
fn read_lock(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(h, p)| (p.to_string(), h.to_string())))
        .collect()
}

fn write_lock(path: &Path, lock: &BTreeMap<String, String>) -> Result<()> {
    let body: String = lock.iter().map(|(p, h)| format!("{}\t{}\n", h, p)).collect();
    std::fs::write(path, body)?;
    Ok(())
}
