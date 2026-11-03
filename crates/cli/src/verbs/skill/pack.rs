//! Domain skill+rule packs — `assets/packs/<name>/{skills/<sub>,rules/*.md}`.
//!
//! A pack bundles a project-type's SKILL.md set (e.g. VTiger/PHP: `action`,
//! `field`, `database`, …) together with its `.omp/rules/*.md` convention
//! files, so a new project of that type gets both in one `skill add pack:<name>`.
//! Project-local by default: every pack file lives once in a shared store
//! (`~/.config/<ns>/packs/<name>/`) and each project's `.omp/` holds symlinks
//! into it, so `8sync up` refreshes every linked project at once. `--global`
//! installs a copy into `~/.omp` for machines that work on that project type
//! almost exclusively (its always-on rules then load in every omp session).
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
    /// Shared store `<packs dir>/<name>/`, laid out like the pack root.
    Store(&'a Path),
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
            PackScope::Store(dir) => dir.join(rel),
        }
    }

    fn lock_path(&self, name: &str) -> PathBuf {
        match self {
            PackScope::Project(root) => root.join(".omp").join(format!("pack-{}.lock", name)),
            PackScope::Global(home) => home.join(".omp/agent").join(format!("pack-{}.lock", name)),
            PackScope::Store(dir) => dir.with_file_name(format!("{}.lock", name)),
        }
    }
}

/// Install pack `name` into a project: refresh the shared store, then make every
/// top-level pack entry under `<root>/.omp/` (`RULES.md`, `rules/<f>`,
/// `commands/<f>`, `agents/<f>`, `skills/<dir>`) a symlink into it, and record
/// the project so `8sync up` re-links it after a release.
///
/// `.omp` is usually untracked, so a local edit must never be lost silently: a
/// real file/dir (copy-mode install) is replaced only while every file in it
/// equals the store copy or still has the hash in `.omp/pack-<name>.lock`; an
/// edited one is kept unless `force`, which moves it to `.omp/removed/` first.
/// Edits made through a link land in the store and are kept by later refreshes
/// unless `force`. Errors only if the pack doesn't exist under `assets/packs/`.
pub(crate) fn install_pack(home: &Path, root: &Path, name: &str, force: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let store = refresh_store(home, name, force)?;
        link_pack(root, &store, name, force)?;
        record_project(home, name, root)
    }
    #[cfg(not(unix))]
    {
        let _ = home;
        install_pack_in(&PackScope::Project(root), name, force)
    }
}

/// Link every pack this project lists in `agents/skills.toml` that is not linked
/// yet: a fresh clone (row, no `.omp/`) or a copy-mode install. Once linked it
/// only re-asserts the git exclude block (the repo may have been `git init`ed
/// after linking), so `8sync .` can run it on each launch. Local edits are kept.
pub(crate) fn link_listed_packs(home: &Path, root: &Path) {
    for pk in discover_packs() {
        if !is_pack_installed(root, &pk.name) {
            continue;
        }
        if is_linked(home, root, &pk.name) {
            #[cfg(unix)]
            exclude_links_from_git(root, &pk.name);
            continue;
        }
        if let Err(e) = install_pack(home, root, &pk.name, false) {
            ui::warn(&format!("pack `{}` install failed: {}", pk.name, e));
        }
    }
}

/// Recorded for `name` and every pack entry present under `.omp/`: a real file
/// (kept local edit) counts, a link counts only while it points into the store.
fn is_linked(home: &Path, root: &Path, name: &str) -> bool {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if !read_projects(home, name).contains(&canonical) {
        return false;
    }
    let (omp, store) = (root.join(".omp"), store_dir(home, name));
    pack_entries(name).iter().all(|e| {
        let path = omp.join(e);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => {
                std::fs::read_link(&path).is_ok_and(|t| t == store.join(e)) && path.exists()
            }
            Ok(_) => true,
            Err(_) => false,
        }
    })
}

/// `8sync up` / harness passes: refresh each linked pack's store once, then
/// re-link every recorded project (new pack entries get a link, removed ones
/// lose theirs). A recorded project that is gone or no longer lists the pack in
/// `agents/skills.toml` is dropped from the record.
pub(crate) fn refresh_linked_packs(home: &Path) {
    #[cfg(unix)]
    for pk in discover_packs() {
        let recorded = read_projects(home, &pk.name);
        if recorded.is_empty() {
            continue;
        }
        let live: Vec<PathBuf> = recorded.into_iter().filter(|r| is_pack_installed(r, &pk.name)).collect();
        if !live.is_empty() {
            match refresh_store(home, &pk.name, false) {
                Ok(store) => {
                    for root in &live {
                        if let Err(e) = link_pack(root, &store, &pk.name, false) {
                            ui::warn(&format!("pack `{}` link failed in {}: {}", pk.name, root.display(), e));
                        }
                    }
                }
                Err(e) => ui::warn(&format!("pack `{}` store refresh failed: {}", pk.name, e)),
            }
        }
        if let Err(e) = write_projects(home, &pk.name, &live) {
            ui::warn(&format!("pack `{}` project record not saved: {}", pk.name, e));
        }
    }
    #[cfg(not(unix))]
    let _ = home;
}

fn store_dir(home: &Path, name: &str) -> PathBuf {
    crate::brand::config_dir(home).join("packs").join(name)
}

fn projects_file(home: &Path, name: &str) -> PathBuf {
    crate::brand::config_dir(home).join("packs").join(format!("{}.projects", name))
}

fn read_projects(home: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_to_string(projects_file(home, name))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn write_projects(home: &Path, name: &str, roots: &[PathBuf]) -> Result<()> {
    let path = projects_file(home, name);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let body: String = roots.iter().map(|r| format!("{}\n", r.display())).collect();
    std::fs::write(path, body)?;
    Ok(())
}

fn record_project(home: &Path, name: &str, root: &Path) -> Result<()> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut roots = read_projects(home, name);
    if !roots.contains(&root) {
        roots.push(root);
        roots.sort();
    }
    write_projects(home, name, &roots)
}

/// Pack files an install deploys, relative to the pack root.
fn pack_rels(name: &str) -> Vec<String> {
    let prefix = format!("packs/{}/", name);
    assets::iter_under(&prefix)
        .iter()
        .filter_map(|p| p.strip_prefix(&prefix).map(String::from))
        .filter(|rel| rel == "RULES.md" || ["skills/", "rules/", "commands/", "agents/"].iter().any(|d| rel.starts_with(d)))
        .collect()
}

/// Top-level entries a project links: `RULES.md`, `<kind>/<file>` or `skills/<dir>`.
fn pack_entries(name: &str) -> Vec<String> {
    let mut entries: Vec<String> = pack_rels(name)
        .iter()
        .map(|rel| {
            let mut parts = rel.splitn(3, '/');
            match (parts.next(), parts.next()) {
                (Some(kind), Some(item)) => format!("{}/{}", kind, item),
                _ => rel.clone(),
            }
        })
        .collect();
    entries.sort();
    entries.dedup();
    entries
}

/// Refresh the shared store (store edits kept unless `force`), then delete store
/// files the pack no longer ships while they are still unedited.
fn refresh_store(home: &Path, name: &str, force: bool) -> Result<PathBuf> {
    let dir = store_dir(home, name);
    let scope = PackScope::Store(&dir);
    install_pack_in(&scope, name, force)?;
    let shipped: std::collections::BTreeSet<String> = pack_rels(name).into_iter().collect();
    let lock_path = scope.lock_path(name);
    let mut lock = read_lock(&lock_path);
    lock.retain(|rel, hash| {
        if shipped.contains(rel) {
            return true;
        }
        let target = dir.join(rel);
        match std::fs::read_to_string(&target) {
            Ok(cur) if fnv1a(&cur) == *hash => {
                let _ = std::fs::remove_file(&target);
                let mut parent = target.parent();
                while let Some(p) = parent {
                    if p == dir || std::fs::remove_dir(p).is_err() {
                        break;
                    }
                    parent = p.parent();
                }
                false
            }
            Ok(_) => true,
            Err(_) => false,
        }
    });
    write_lock(&lock_path, &lock)?;
    Ok(dir)
}

/// Point every pack entry under `<root>/.omp/` at the store; drop links whose
/// store target is gone.
#[cfg(unix)]
fn link_pack(root: &Path, store: &Path, name: &str, force: bool) -> Result<()> {
    let omp = root.join(".omp");
    let lock_path = PackScope::Project(root).lock_path(name);
    let lock = read_lock(&lock_path);
    let backup_dir = omp.join("removed").join(format!(
        "pack-{}-{}",
        name,
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    ));
    let (mut linked, mut unchanged) = (0usize, 0usize);
    let (mut kept, mut moved): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for entry in pack_entries(name) {
        let src = store.join(&entry);
        let dst = omp.join(&entry);
        match std::fs::symlink_metadata(&dst) {
            Ok(m) if m.file_type().is_symlink() => {
                if std::fs::read_link(&dst).is_ok_and(|t| t == src) {
                    unchanged += 1;
                    continue;
                }
                std::fs::remove_file(&dst)?;
            }
            Ok(m) => {
                if !is_pristine(&dst, &src, &entry, &lock) {
                    if !force {
                        kept.push(entry);
                        continue;
                    }
                    let backup = backup_dir.join(&entry);
                    if let Some(p) = backup.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    std::fs::rename(&dst, &backup)?;
                    moved.push(entry.clone());
                } else if m.is_dir() {
                    std::fs::remove_dir_all(&dst)?;
                } else {
                    std::fs::remove_file(&dst)?;
                }
            }
            Err(_) => {}
        }
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::os::unix::fs::symlink(&src, &dst)?;
        linked += 1;
    }
    let stale = remove_stale_links(&omp, store);
    let _ = std::fs::remove_file(&lock_path);
    exclude_links_from_git(root, name);

    ui::ok(&format!(
        "pack `{}` → {} link(s) created, {} unchanged, {} stale removed → {}",
        name, linked, unchanged, stale, omp.display()
    ));
    if !moved.is_empty() {
        ui::warn(&format!("{} locally edited entr(ies) moved to {}:", moved.len(), backup_dir.display()));
        for entry in &moved {
            ui::info(&format!("  {}", entry));
        }
    }
    if !kept.is_empty() {
        ui::warn(&format!(
            "{} locally edited entr(ies) kept as real files, not linked (rerun with --force to move them to .omp/removed/ and link):",
            kept.len()
        ));
        for entry in &kept {
            ui::info(&format!("  {}", omp.join(entry).display()));
        }
    }
    Ok(())
}

/// A real file/dir may be replaced by a link when every file in it equals the
/// store copy or still has the hash recorded at copy-install time.
#[cfg(unix)]
fn is_pristine(dst: &Path, src: &Path, entry: &str, lock: &BTreeMap<String, String>) -> bool {
    let file_ok = |path: &Path, store_copy: &Path, rel: &str| -> bool {
        let Ok(cur) = std::fs::read_to_string(path) else {
            return false;
        };
        std::fs::read_to_string(store_copy).is_ok_and(|s| s == cur) || lock.get(rel).is_some_and(|h| *h == fnv1a(&cur))
    };
    if !dst.is_dir() {
        return file_ok(dst, src, entry);
    }
    let mut stack = vec![dst.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            return false;
        };
        for item in read.flatten() {
            let path = item.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if item.file_name() == ".DS_Store" {
                continue;
            }
            let Ok(sub) = path.strip_prefix(dst) else {
                return false;
            };
            if !file_ok(&path, &src.join(sub), &format!("{}/{}", entry, sub.to_string_lossy())) {
                return false;
            }
        }
    }
    true
}

/// Remove links into `store` whose target no longer exists. Returns the count.
#[cfg(unix)]
fn remove_stale_links(omp: &Path, store: &Path) -> usize {
    let mut removed = 0usize;
    for dir in [omp.to_path_buf(), omp.join("rules"), omp.join("commands"), omp.join("agents"), omp.join("skills")] {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let path = item.path();
            let Ok(target) = std::fs::read_link(&path) else {
                continue;
            };
            if target.starts_with(store) && !target.exists() && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Keep the machine-local links out of git: a managed block in the clone's
/// `info/exclude` (never committed) lists every pack entry under `.omp/`.
#[cfg(unix)]
fn exclude_links_from_git(root: &Path, name: &str) {
    let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-prefix", "--git-path", "info/exclude"])
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let (prefix, exclude) = (lines.next().unwrap_or("").trim(), lines.next().unwrap_or("").trim());
    if exclude.is_empty() {
        return;
    }
    let exclude = PathBuf::from(exclude);
    let exclude = if exclude.is_absolute() { exclude } else { root.join(exclude) };
    let body: Vec<String> = pack_entries(name).iter().map(|e| format!("/{}.omp/{}", prefix, e)).collect();
    let _ = crate::verbs::harness::memory::upsert_block(
        &exclude,
        &format!("# >>> ckit pack {} (machine-local symlinks) >>>", name),
        &format!("# <<< ckit pack {} <<<", name),
        &body.join("\n"),
    );
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
    let reg = super::discover::read_registry(registry);
    let wanted: Vec<&str> = reg.values().filter_map(|e| e.src.strip_prefix(GLOBAL_SRC_PREFIX)).collect();
    remove_unregistered_global_packs(home, &wanted);
    for pack in wanted {
        if let Err(e) = install_pack_global(home, pack, false) {
            ui::warn(&format!("global pack `{}` refresh failed: {}", pack, e));
        }
    }
}

/// Undo a global pack whose lock exists but which has no `pack-global:` registry
/// row (v0.1.33 installed one from a legacy `pack:` row). Only files still
/// byte-identical to the lock are deleted; locally edited files stay.
fn remove_unregistered_global_packs(home: &Path, wanted: &[&str]) {
    let scope = PackScope::Global(home);
    let Ok(entries) = std::fs::read_dir(home.join(".omp/agent")) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_prefix("pack-").and_then(|f| f.strip_suffix(".lock")) else {
            continue;
        };
        if wanted.contains(&name) {
            continue;
        }
        let (mut removed, mut kept) = (0usize, Vec::new());
        for (rel, hash) in read_lock(&entry.path()) {
            let target = scope.target(&rel);
            match std::fs::read_to_string(&target) {
                Ok(cur) if fnv1a(&cur) == hash => {
                    if std::fs::remove_file(&target).is_ok() {
                        removed += 1;
                        prune_empty_parents(&target, home);
                    }
                }
                Ok(_) => kept.push(rel),
                Err(_) => {}
            }
        }
        let _ = std::fs::remove_file(entry.path());
        ui::warn(&format!(
            "global pack `{}` removed ({} file(s)): it was never added with `skill add pack:{} --global`",
            name, removed, name
        ));
        for rel in &kept {
            ui::info(&format!("  kept (locally edited): {}", scope.target(rel).display()));
        }
    }
}

/// Remove now-empty directories above `path`, stopping at `~/.omp/skills` / `~/.omp/agent`.
fn prune_empty_parents(path: &Path, home: &Path) {
    let stops = [home.join(".omp/skills"), home.join(".omp/agent"), home.join(".omp")];
    let mut dir = path.parent();
    while let Some(d) = dir {
        if stops.iter().any(|s| s == d) || std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

fn install_pack_in(scope: &PackScope, name: &str, force: bool) -> Result<()> {
    let files = pack_rels(name);
    if files.is_empty() {
        anyhow::bail!("no bundled pack `{}` (assets/packs/{}/ not found)", name, name);
    }

    let lock_path = scope.lock_path(name);
    if let Some(p) = lock_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut lock = read_lock(&lock_path);
    let (mut written, mut unchanged) = (0usize, 0usize);
    let mut kept: Vec<String> = Vec::new();
    for rel in &files {
        let Some(body) = assets::read(&format!("packs/{}/{}", name, rel)) else {
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
        PackScope::Store(dir) => format!("store {}", dir.display()),
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

    #[test]
    fn harness_refresh_removes_an_unregistered_global_pack_but_keeps_edits() {
        let home = temp_home("orphan");
        let registry = home.join("skills.toml");
        install_pack_global(&home, "vtiger-php", false).unwrap();
        let edited = home.join(".omp/agent/rules/php83.md");
        std::fs::write(&edited, "local edit").unwrap();

        // legacy `pack:` row = not an opt-in -> the orphan install is undone
        std::fs::write(&registry, "[vtiger-php]\nsrc = \"pack:vtiger-php\"\n").unwrap();
        refresh_global_packs(&home, &registry);
        assert!(!home.join(".omp/agent/pack-vtiger-php.lock").exists());
        assert!(!home.join(".omp/agent/RULES.md").exists());
        assert!(!home.join(".omp/skills/field").exists());
        assert_eq!(std::fs::read_to_string(&edited).unwrap(), "local edit");
        assert!(home.join(".omp/skills").is_dir());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn project_install_links_every_entry_into_the_shared_store() {
        let home = temp_home("link");
        let root = home.join("repo");
        std::fs::create_dir_all(&root).unwrap();
        install_pack(&home, &root, "vtiger-php", false).unwrap();

        let store = store_dir(&home, "vtiger-php");
        for entry in ["rules/php-conventions.md", "skills/review-pr", "commands/review-pr.md", "RULES.md"] {
            assert_eq!(std::fs::read_link(root.join(".omp").join(entry)).unwrap(), store.join(entry), "{}", entry);
        }
        assert!(root.join(".omp/skills/review-pr/SKILL.md").is_file());
        assert!(!root.join(".omp/pack-vtiger-php.lock").exists());
        assert_eq!(read_projects(&home, "vtiger-php"), vec![root.canonicalize().unwrap()]);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn pack_links_stay_out_of_git_even_in_a_subdirectory_project() {
        let home = temp_home("gitx");
        let repo = home.join("repo");
        let root = repo.join("app");
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&repo).args(args).output().unwrap();
        assert!(git(&["init", "-q"]).status.success());
        std::fs::write(root.join("keep.txt"), "x").unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        install_pack(&home, &root, "vtiger-php", false).unwrap();
        let status = String::from_utf8(git(&["status", "--porcelain", "--untracked-files=all"]).stdout).unwrap();
        assert_eq!(status.trim(), "?? app/keep.txt");
        let exclude = std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude.matches("# >>> ckit pack vtiger-php").count(), 1);
        assert!(exclude.contains("/app/.omp/skills/field\n"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn repo_initialised_after_linking_gets_its_exclude_on_the_next_launch() {
        let home = temp_home("gitlate");
        let root = home.join("repo");
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::write(root.join("agents/skills.toml"), "[vtiger-php]\nsrc = \"pack:vtiger-php\"\n").unwrap();
        install_pack(&home, &root, "vtiger-php", false).unwrap();

        let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&root).args(args).output().unwrap();
        assert!(git(&["init", "-q"]).status.success());
        link_listed_packs(&home, &root);
        let status = String::from_utf8(git(&["status", "--porcelain", "--untracked-files=all"]).stdout).unwrap();
        assert_eq!(status.trim(), "?? agents/skills.toml");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn copy_install_becomes_links_but_local_edits_stay_unless_forced() {
        let home = temp_home("convert");
        let root = home.join("repo");
        install_pack_in(&PackScope::Project(&root), "vtiger-php", false).unwrap();
        let rule = root.join(".omp/rules/php83.md");
        std::fs::write(&rule, "local edit").unwrap();
        std::fs::write(root.join(".omp/skills/field/notes.md"), "mine").unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        assert!(std::fs::read_link(root.join(".omp/rules/php-conventions.md")).is_ok());
        assert!(std::fs::symlink_metadata(&rule).unwrap().is_file());
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), "local edit");
        assert!(std::fs::symlink_metadata(root.join(".omp/skills/field")).unwrap().is_dir());

        install_pack(&home, &root, "vtiger-php", true).unwrap();
        assert!(std::fs::read_link(&rule).is_ok());
        assert!(std::fs::read_link(root.join(".omp/skills/field")).is_ok());
        let backup = std::fs::read_dir(root.join(".omp/removed")).unwrap().flatten().next().unwrap().path();
        assert_eq!(std::fs::read_to_string(backup.join("rules/php83.md")).unwrap(), "local edit");
        assert_eq!(std::fs::read_to_string(backup.join("skills/field/notes.md")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn refresh_relinks_listed_projects_and_drops_stale_links_and_unlisted_projects() {
        let home = temp_home("relink");
        let (root, other) = (home.join("repo"), home.join("other"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        install_pack(&home, &root, "vtiger-php", false).unwrap();
        install_pack(&home, &other, "vtiger-php", false).unwrap();
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::write(root.join("agents/skills.toml"), "[vtiger-php]\nsrc = \"pack:vtiger-php\"\n").unwrap();

        let store = store_dir(&home, "vtiger-php");
        let rule = root.join(".omp/rules/php83.md");
        std::fs::remove_file(&rule).unwrap();
        let retired = root.join(".omp/rules/retired.md");
        std::os::unix::fs::symlink(store.join("rules/retired.md"), &retired).unwrap();

        refresh_linked_packs(&home);
        assert_eq!(std::fs::read_link(&rule).unwrap(), store.join("rules/php83.md"));
        assert!(std::fs::symlink_metadata(&retired).is_err());
        assert_eq!(read_projects(&home, "vtiger-php"), vec![root.canonicalize().unwrap()]);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn store_refresh_drops_retired_files_but_keeps_edited_ones() {
        let home = temp_home("prune");
        let dir = refresh_store(&home, "vtiger-php", false).unwrap();
        let lock_path = PackScope::Store(&dir).lock_path("vtiger-php");
        std::fs::write(dir.join("rules/retired.md"), "old").unwrap();
        std::fs::write(dir.join("rules/edited-retired.md"), "edited").unwrap();
        let mut lock = read_lock(&lock_path);
        lock.insert("rules/retired.md".to_string(), fnv1a("old"));
        lock.insert("rules/edited-retired.md".to_string(), fnv1a("original"));
        write_lock(&lock_path, &lock).unwrap();

        refresh_store(&home, "vtiger-php", false).unwrap();
        assert!(!dir.join("rules/retired.md").exists());
        assert_eq!(std::fs::read_to_string(dir.join("rules/edited-retired.md")).unwrap(), "edited");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn listed_pack_in_a_fresh_clone_gets_linked_and_unlisted_repo_stays_untouched() {
        let home = temp_home("clone");
        let (clone, plain) = (home.join("clone"), home.join("plain"));
        std::fs::create_dir_all(clone.join("agents")).unwrap();
        std::fs::create_dir_all(&plain).unwrap();
        std::fs::write(clone.join("agents/skills.toml"), "[vtiger-php]\nsrc = \"pack:vtiger-php\"\n").unwrap();

        link_listed_packs(&home, &clone);
        link_listed_packs(&home, &plain);
        assert!(std::fs::read_link(clone.join(".omp/rules/php83.md")).is_ok());
        assert!(is_linked(&home, &clone, "vtiger-php"));
        assert!(!plain.join(".omp").exists());
        assert_eq!(read_projects(&home, "vtiger-php"), vec![clone.canonicalize().unwrap()]);
        let _ = std::fs::remove_dir_all(&home);
    }
}
