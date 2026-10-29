//! Domain skill+rule packs — `assets/packs/<name>/{skills/<sub>,rules/*.md}`.
//!
//! A pack bundles a project-type's SKILL.md set (e.g. VTiger/PHP: `action`,
//! `field`, `database`, …) together with its `.omp/rules/*.md` convention
//! files, so a new project of that type gets both in one `skill add pack:<name>`.
//! Project-local by default; `--global` installs it into `~/.omp` for machines
//! that work on that project type almost exclusively (its always-on rules then
//! load in every omp session, in every repo).
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// Where a pack is installed.
pub(crate) enum PackScope<'a> {
    /// `<root>/.omp/{skills,rules,commands,agents,RULES.md}`
    Project(&'a Path),
    /// `~/.omp/skills/<sub>` + `~/.omp/agent/{rules,commands,agents,RULES.md}`
    Global(&'a Path),
}

impl PackScope<'_> {
    /// Installed path of pack file `rel` (relative to the pack root).
    fn target(&self, rel: &str) -> PathBuf {
        match self {
            PackScope::Project(root) => root.join(".omp").join(rel),
            PackScope::Global(home) => match rel.strip_prefix("skills/") {
                Some(skill) => home.join(".omp/skills").join(skill),
                None => home.join(".omp/agent").join(rel),
            },
        }
    }

    fn lock_path(&self, name: &str) -> PathBuf {
        match self {
            PackScope::Project(root) => root.join(".omp").join(format!("pack-{}.lock", name)),
            PackScope::Global(home) => home.join(".omp/agent").join(format!("pack-{}.lock", name)),
        }
    }
}

/// Deploy pack `name` into the project's `.omp/`: skills (`skills/<sub>`), rule
/// files (`rules/`), slash commands (`commands/`), agents (`agents/`) and the always-apply `RULES.md`.
/// Domain convention (PHP/VTiger, …) is scoped to a matching project by default.
///
/// `.omp` is usually untracked, so a local edit must never be lost silently:
/// `.omp/pack-<name>.lock` records the hash of every file as installed. A file
/// is overwritten only while it still has that hash; an edited (or never
/// recorded) file is kept unless `force`, which saves it as `<file>.bak` first.
/// Errors only if the pack doesn't exist under `assets/packs/`.
pub(crate) fn install_pack(root: &Path, name: &str, force: bool) -> Result<()> {
    install_pack_in(&PackScope::Project(root), name, force)
}

/// Registry `src` prefix of a `--global` pack. Distinct from the project-manifest
/// `pack:` so a stale `pack:` row in the global registry never installs globally.
pub(crate) const GLOBAL_SRC_PREFIX: &str = "pack-global:";

/// Same as [`install_pack`] into `~/.omp` (lock: `~/.omp/agent/pack-<name>.lock`).
pub(crate) fn install_pack_global(home: &Path, name: &str, force: bool) -> Result<()> {
    install_pack_in(&PackScope::Global(home), name, force)
}

/// Refresh every pack registered with `--global` in `registry` (the global
/// `skills.toml`) from the embedded assets — offline, local edits kept. Run by
/// `harness` init/up/global, so `8sync up` applies a new release's pack too.
pub(crate) fn refresh_global_packs(home: &Path, registry: &Path) {
    for (key, entry) in super::discover::read_registry(registry) {
        let Some(pack) = entry.src.strip_prefix(GLOBAL_SRC_PREFIX) else {
            continue;
        };
        if let Err(e) = install_pack_global(home, pack, false) {
            ui::warn(&format!("global pack `{}` refresh failed: {}", key, e));
        }
    }
}

fn install_pack_in(scope: &PackScope, name: &str, force: bool) -> Result<()> {
    let pack_prefix = format!("packs/{}/", name);
    let files: Vec<String> = assets::iter_under(&pack_prefix)
        .into_iter()
        .filter(|p| {
            let rel = &p[pack_prefix.len()..];
            rel == "RULES.md" || ["skills/", "rules/", "commands/", "agents/"].iter().any(|d| rel.starts_with(d))
        })
        .collect();
    if files.is_empty() {
        anyhow::bail!("no bundled pack `{}` (assets/packs/{}/ not found)", name, name);
    }

    let lock_path = scope.lock_path(name);
    let mut lock = read_lock(&lock_path);
    let (mut written, mut unchanged) = (0usize, 0usize);
    let mut kept: Vec<String> = Vec::new();
    for asset in &files {
        let rel = &asset[pack_prefix.len()..];
        let Some(body) = assets::read(asset) else {
            continue;
        };
        let body = if rel.ends_with(".md") || rel.ends_with(".txt") { crate::brand::render(&body).into_owned() } else { body };
        let target = scope.target(rel);
        let current = std::fs::read_to_string(&target).ok();
        match current {
            Some(cur) if cur == body => unchanged += 1,
            Some(cur) if lock.get(rel) != Some(&fnv1a(&cur)) => {
                if !force {
                    kept.push(rel.to_string());
                    continue;
                }
                let mut backup = target.clone().into_os_string();
                backup.push(".bak");
                std::fs::rename(&target, PathBuf::from(backup))?;
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
        audit_skill_layout(&scope.target(&format!("skills/{}", sub)));
    }

    let place = match scope {
        PackScope::Project(root) => root.join(".omp").display().to_string(),
        PackScope::Global(home) => format!("{} + {}", home.join(".omp/skills").display(), home.join(".omp/agent").display()),
    };
    ui::ok(&format!("pack `{}` → {} file(s) written, {} unchanged → {}", name, written, unchanged, place));
    if !kept.is_empty() {
        ui::warn(&format!(
            "{} locally edited file(s) kept (rerun with --force to overwrite, old copy saved as .bak):",
            kept.len()
        ));
        for rel in &kept {
            ui::info(&format!("  {}", scope.target(rel).display()));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ckit-pack-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn global_scope_maps_skills_to_omp_skills_and_the_rest_to_agent_dir() {
        let home = Path::new("/h");
        let scope = PackScope::Global(home);
        assert_eq!(scope.target("skills/field/SKILL.md"), Path::new("/h/.omp/skills/field/SKILL.md"));
        assert_eq!(scope.target("rules/php83.md"), Path::new("/h/.omp/agent/rules/php83.md"));
        assert_eq!(scope.target("commands/review-pr.md"), Path::new("/h/.omp/agent/commands/review-pr.md"));
        assert_eq!(scope.target("RULES.md"), Path::new("/h/.omp/agent/RULES.md"));
        assert_eq!(scope.lock_path("vtiger-php"), Path::new("/h/.omp/agent/pack-vtiger-php.lock"));
    }

    #[test]
    fn global_install_is_idempotent_and_keeps_a_local_edit() {
        let home = temp_home("global");
        install_pack_global(&home, "vtiger-php", false).unwrap();
        let rule = home.join(".omp/agent/rules/php-conventions.md");
        let installed = std::fs::read(&rule).unwrap();
        assert!(home.join(".omp/skills/review-pr/SKILL.md").is_file());
        assert!(home.join(".omp/agent/commands/review-pr.md").is_file());

        // Rerun writes nothing; an edited file survives a normal update.
        std::fs::write(&rule, "local edit").unwrap();
        install_pack_global(&home, "vtiger-php", false).unwrap();
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), "local edit");

        // --force restores the pack copy and keeps the edit as .bak.
        install_pack_global(&home, "vtiger-php", true).unwrap();
        assert_eq!(std::fs::read(&rule).unwrap(), installed);
        assert_eq!(std::fs::read_to_string(home.join(".omp/agent/rules/php-conventions.md.bak")).unwrap(), "local edit");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn harness_refresh_installs_only_packs_registered_globally() {
        let home = temp_home("refresh");
        let registry = home.join("skills.toml");
        refresh_global_packs(&home, &registry);
        assert!(!home.join(".omp/agent/rules").exists());

        // a legacy project-style `pack:` row must not opt the machine in
        std::fs::write(&registry, "[vtiger-php]\nsrc = \"pack:vtiger-php\"\nwhen = \"on-demand\"\n").unwrap();
        refresh_global_packs(&home, &registry);
        assert!(!home.join(".omp/agent/rules").exists());

        std::fs::write(&registry, "[vtiger-php]\nsrc = \"pack-global:vtiger-php\"\nwhen = \"on-demand\"\n").unwrap();
        refresh_global_packs(&home, &registry);
        assert!(home.join(".omp/agent/rules/php-conventions.md").is_file());
        assert!(home.join(".omp/agent/pack-vtiger-php.lock").is_file());
        let _ = std::fs::remove_dir_all(&home);
    }
}
