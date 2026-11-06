//! Domain packs — `assets/packs/<name>/{skills/<sub>,rules,commands,agents}`.
//!
//! A pack bundles a project type's skills, convention rules, slash commands and
//! subagents (e.g. VTiger/PHP). Every pack file lives once in a shared store
//! (`~/.omp/shared/<group>/pack/`, `group` from `pack.toml` `shared`) that omp
//! loads as an extension package; next to it `~/.omp/shared/<group>/personal/` holds
//! the user's own skills for that repo family (ckit creates it, never writes in it).
//! A project opts in through `extensions` in its `.omp/settings.json`, which lists
//! both. omp reads `skills/`, `commands/`, `rules/` and `agents/` itself, so `8sync up`
//! refreshing the store updates every project at once. A real file of the same
//! name under a project's `.omp/` overrides the pack copy. Packs are never global:
//! their always-on rules belong only to the repos that list them.
use anyhow::{anyhow, bail, Result};
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
/// the same registry every other source (`git`/`builtin`/`path`) is tracked by.
pub(crate) fn is_pack_installed(root: &Path, name: &str) -> bool {
    let manifest = root.join("agents/skills.toml");
    let reg = super::discover::read_registry(&manifest);
    reg.get(name).is_some_and(|e| e.src == format!("pack:{}", name))
}

/// Install pack `name` into a project: refresh the shared store, retire any
/// older per-project copy or symlinks of it under `.omp/`, then list the store
/// and the user's `personal` folder in `extensions` of `<root>/.omp/settings.json`
/// (other keys and entries kept; the v0.1.48 store entry is dropped).
/// Errors only if the pack doesn't exist or `settings.json` isn't a JSON object.
pub(crate) fn install_pack(home: &Path, root: &Path, name: &str, force: bool) -> Result<()> {
    let store = refresh_store(home, name, force)?;
    retire_project_copy(root, &store, name, force)?;
    let settings = root.join(".omp/settings.json");
    set_extension(&settings, home, &legacy_store_dir(home, name), false)?;
    set_extension(&settings, home, &store, true)?;
    let personal = personal_dir(home, name);
    std::fs::create_dir_all(personal.join("skills"))?;
    set_extension(&settings, home, &personal, true)?;
    exclude_settings_from_git(root, name);
    ui::ok(&format!(
        "pack `{}` → omp extensions {} + {} in {}",
        name,
        extension_entry(home, &store),
        extension_entry(home, &personal),
        settings.display()
    ));
    Ok(())
}

/// Install every pack this project lists in `agents/skills.toml` that is not an
/// omp extension of it yet (fresh clone, a pre-extension copy/link install, or a
/// v0.1.48 store path). Once installed it only re-asserts the git exclude, so
/// `8sync .` can run it on each launch. Local edits are kept.
pub(crate) fn ensure_listed_packs(home: &Path, root: &Path) {
    for pk in discover_packs() {
        if !is_pack_installed(root, &pk.name) {
            continue;
        }
        let store = store_dir(home, &pk.name);
        let settings = root.join(".omp/settings.json");
        if store.is_dir()
            && has_extension(&settings, home, &store)
            && has_extension(&settings, home, &personal_dir(home, &pk.name))
            && !has_extension(&settings, home, &legacy_store_dir(home, &pk.name))
            && !has_legacy_install(root, &store, &pk.name)
        {
            exclude_settings_from_git(root, &pk.name);
            continue;
        }
        if let Err(e) = install_pack(home, root, &pk.name, false) {
            ui::warn(&format!("pack `{}` install failed: {}", pk.name, e));
        }
    }
}

/// `8sync up` / harness passes: refresh the store of every pack this machine
/// uses. Projects that v0.1.47 recorded for re-linking are migrated to the
/// extension once, then the record is dropped.
pub(crate) fn refresh_stores(home: &Path) {
    for pk in discover_packs() {
        move_legacy_store(home, &pk.name);
        let record = legacy_projects_file(home, &pk.name);
        if let Ok(body) = std::fs::read_to_string(&record) {
            for root in body.lines().filter(|l| !l.trim().is_empty()).map(Path::new) {
                if is_pack_installed(root, &pk.name) {
                    if let Err(e) = install_pack(home, root, &pk.name, false) {
                        ui::warn(&format!("pack `{}` migration failed in {}: {}", pk.name, root.display(), e));
                    }
                }
            }
            let _ = std::fs::remove_file(&record);
        }
        if !store_dir(home, &pk.name).is_dir() {
            continue;
        }
        if let Err(e) = refresh_store(home, &pk.name, false) {
            ui::warn(&format!("pack `{}` store refresh failed: {}", pk.name, e));
        }
    }
}

/// Move a v0.1.48 store (`<config>/packs/<name>` + its lock) to `<shared>/pack`,
/// keeping local edits. Projects still listing the old path are re-pointed by
/// `ensure_listed_packs` on their next `8sync .` / harness run.
fn move_legacy_store(home: &Path, name: &str) {
    let old = legacy_store_dir(home, name);
    if !old.is_dir() {
        return;
    }
    let new = store_dir(home, name);
    let old_lock = old.with_file_name(format!("{}.lock", name));
    let moved = !new.exists()
        && new.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok())
        && std::fs::rename(&old, &new).is_ok();
    if moved {
        let _ = std::fs::rename(&old_lock, store_lock(&new));
        ui::ok(&format!("pack `{}` store moved {} → {}", name, old.display(), new.display()));
    }
    else {
        // Both exist (an older binary recreated the old store): drop only files
        // still matching their install hash; edited ones stay and are reported.
        let lock = read_lock(&old_lock);
        let mut edited: Vec<String> = Vec::new();
        for (rel, hash) in &lock {
            let f = old.join(rel);
            match std::fs::read_to_string(&f) {
                Ok(cur) if fnv1a(&cur) == *hash => {
                    let _ = std::fs::remove_file(&f);
                }
                Ok(_) => edited.push(rel.clone()),
                Err(_) => {}
            }
        }
        if edited.is_empty() {
            let _ = std::fs::remove_dir_all(&old);
            let _ = std::fs::remove_file(&old_lock);
            return;
        }
        ui::warn(&format!(
            "old pack `{}` store {} has {} locally edited file(s), kept and no longer loaded; move them into {} or delete them:",
            name,
            old.display(),
            edited.len(),
            new.display()
        ));
        for rel in &edited {
            ui::info(&format!("  {}", old.join(rel).display()));
        }
    }
}

/// `~/.omp/shared/<group>`: one folder per repo family. `group` is the pack's
/// `shared = "…"` key in `pack.toml` (default: the pack name).
fn shared_dir(home: &Path, name: &str) -> PathBuf {
    let group = assets::read(&format!("packs/{}/pack.toml", name))
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("shared = ").map(|v| v.trim().trim_matches('"').to_string())))
        .filter(|g| !g.is_empty() && !g.contains(['/', '\\', '.']))
        .unwrap_or_else(|| name.to_string());
    home.join(".omp/shared").join(group)
}

/// The ckit-managed store of the pack: `<shared>/pack`.
fn store_dir(home: &Path, name: &str) -> PathBuf {
    shared_dir(home, name).join("pack")
}

/// The user's own skills for this repo family: `<shared>/personal`. ckit only
/// creates it and lists it as an extension; it never writes inside.
fn personal_dir(home: &Path, name: &str) -> PathBuf {
    shared_dir(home, name).join("personal")
}

/// Install hashes of the store files, beside the store so the extension root
/// holds only omp's content directories.
fn store_lock(store: &Path) -> PathBuf {
    store.with_file_name(".pack.lock")
}

/// v0.1.48 store location (`<config>/packs/<name>` + `<name>.lock` beside it).
fn legacy_store_dir(home: &Path, name: &str) -> PathBuf {
    crate::brand::config_dir(home).join("packs").join(name)
}

/// v0.1.47 list of linked projects (`<packs dir>/<name>.projects`).
fn legacy_projects_file(home: &Path, name: &str) -> PathBuf {
    crate::brand::config_dir(home).join("packs").join(format!("{}.projects", name))
}

/// `extensions` entry for the store: `~/…` when under home, so the line reads
/// the same on every machine.
fn extension_entry(home: &Path, store: &Path) -> String {
    match store.strip_prefix(home) {
        Ok(rel) => format!("~/{}", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => store.to_string_lossy().into_owned(),
    }
}

fn points_to(home: &Path, entry: &serde_json::Value, store: &Path) -> bool {
    entry.as_str().is_some_and(|s| match s.strip_prefix("~/") {
        Some(rel) => home.join(rel) == store,
        None => Path::new(s) == store,
    })
}

fn has_extension(settings: &Path, home: &Path, store: &Path) -> bool {
    std::fs::read_to_string(settings)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("extensions")?.as_array().map(|a| a.iter().any(|e| points_to(home, e, store))))
        .unwrap_or(false)
}

/// Add (`present`) or remove the store from `extensions` in an omp
/// `settings.json`, keeping every other key and entry. A file left as `{}` by a
/// removal is deleted. Never rewrites a file it cannot parse.
fn set_extension(settings: &Path, home: &Path, store: &Path, present: bool) -> Result<()> {
    let text = std::fs::read_to_string(settings).ok();
    if text.is_none() && !present {
        return Ok(());
    }
    let entry = extension_entry(home, store);
    let mut value: serde_json::Value = match text.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => serde_json::from_str(t).map_err(|e| {
            anyhow!("{} is not valid JSON ({}); add \"{}\" to its `extensions` by hand", settings.display(), e, entry)
        })?,
        _ => serde_json::json!({}),
    };
    let Some(obj) = value.as_object_mut() else {
        bail!("{} is not a JSON object", settings.display());
    };
    let list = obj.entry("extensions").or_insert_with(|| serde_json::json!([]));
    let Some(list) = list.as_array_mut() else {
        bail!("`extensions` in {} is not an array", settings.display());
    };
    if list.iter().any(|e| points_to(home, e, store)) == present {
        return Ok(());
    }
    if present {
        list.push(entry.into());
    } else {
        list.retain(|e| !points_to(home, e, store));
    }
    if list.is_empty() {
        obj.remove("extensions");
    }
    if obj.is_empty() {
        std::fs::remove_file(settings)?;
        return Ok(());
    }
    if let Some(p) = settings.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(settings, serde_json::to_string_pretty(&value)? + "\n")?;
    Ok(())
}

/// Pack files the store holds, relative to the pack root.
fn pack_rels(name: &str) -> Vec<String> {
    let prefix = format!("packs/{}/", name);
    assets::iter_under(&prefix)
        .iter()
        .filter_map(|p| p.strip_prefix(&prefix).map(String::from))
        .filter(|rel| ["skills/", "rules/", "commands/", "agents/"].iter().any(|d| rel.starts_with(d)))
        .collect()
}

/// The top-level `.omp/` entry an older install created for pack file `rel`:
/// `<kind>/<file>` or `skills/<dir>`.
fn top_entry(rel: &str) -> String {
    let mut parts = rel.splitn(3, '/');
    match (parts.next(), parts.next()) {
        (Some(kind), Some(item)) => format!("{}/{}", kind, item),
        _ => rel.to_string(),
    }
}

/// Top-level entries an older install put under `.omp/`: `RULES.md`, every
/// entry of the current pack, and every entry its copy lock recorded — so a
/// file the pack has since dropped is retired too.
fn legacy_entries(name: &str, lock: &BTreeMap<String, String>) -> Vec<String> {
    let mut entries: Vec<String> = pack_rels(name).iter().chain(lock.keys()).map(|rel| top_entry(rel)).collect();
    entries.push("RULES.md".to_string());
    entries.sort();
    entries.dedup();
    entries
}

/// Symlinks under `.omp/` (and its `rules|commands|agents|skills`) that point into `store`.
fn store_links(omp: &Path, store: &Path) -> Vec<PathBuf> {
    ["", "rules", "commands", "agents", "skills"]
        .iter()
        .filter_map(|d| std::fs::read_dir(omp.join(d)).ok())
        .flat_map(|read| read.flatten().map(|e| e.path()))
        .filter(|p| std::fs::read_link(p).is_ok_and(|t| t.starts_with(store)))
        .collect()
}

/// A copy lock or a link into the store is left from a pre-extension install.
fn has_legacy_install(root: &Path, store: &Path, name: &str) -> bool {
    let omp = root.join(".omp");
    omp.join(format!("pack-{}.lock", name)).exists() || !store_links(&omp, store).is_empty()
}

/// Remove what a pre-extension install left under `<root>/.omp/`: every link into
/// the store (v0.1.47) and every copied entry still equal to the store copy or to
/// the hash in `.omp/pack-<name>.lock`. A locally edited copy keeps overriding the
/// pack unless `force`, which moves it to `.omp/removed/`.
fn retire_project_copy(root: &Path, store: &Path, name: &str, force: bool) -> Result<()> {
    let omp = root.join(".omp");
    let mut retired = 0usize;
    for link in store_links(&omp, store) {
        std::fs::remove_file(&link)?;
        retired += 1;
    }

    let lock_path = omp.join(format!("pack-{}.lock", name));
    let lock = read_lock(&lock_path);
    let backup_dir = omp.join("removed").join(format!(
        "pack-{}-{}",
        name,
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    ));
    let (mut kept, mut moved): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for entry in legacy_entries(name, &lock) {
        let dst = omp.join(&entry);
        let Ok(meta) = std::fs::symlink_metadata(&dst) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if is_pristine(&dst, &store.join(&entry), &entry, &lock) {
            if meta.is_dir() {
                std::fs::remove_dir_all(&dst)?;
            } else {
                std::fs::remove_file(&dst)?;
            }
            retired += 1;
        } else if force {
            let backup = backup_dir.join(&entry);
            if let Some(p) = backup.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::rename(&dst, &backup)?;
            moved.push(entry);
        } else {
            kept.push(entry);
        }
    }
    let _ = std::fs::remove_file(&lock_path);

    if retired > 0 {
        ui::info(&format!("pack `{}`: {} old link(s)/copied entr(ies) removed from {}", name, retired, omp.display()));
    }
    if !moved.is_empty() {
        ui::warn(&format!("{} locally edited entr(ies) moved to {}:", moved.len(), backup_dir.display()));
        for entry in &moved {
            ui::info(&format!("  {}", entry));
        }
    }
    if !kept.is_empty() {
        ui::warn(&format!(
            "{} locally edited entr(ies) kept; they override the pack copy (rerun with --force to move them to .omp/removed/):",
            kept.len()
        ));
        for entry in &kept {
            ui::info(&format!("  {}", omp.join(entry).display()));
        }
    }
    Ok(())
}

/// A real file/dir may go when every file in it equals the store copy or still
/// has the hash recorded at copy-install time.
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

/// Keep the machine-local `.omp/settings.json` (it names this user's store) out
/// of git: a managed block in the clone's `info/exclude`, never committed. A
/// per-file pattern is fine here — omp reads `settings.json` directly, it only
/// filters the files it globs (which is why v0.1.47's per-entry block hid the
/// pack's commands and rules; that block is replaced).
fn exclude_settings_from_git(root: &Path, name: &str) {
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
    let begin = format!("# >>> ckit pack {} (machine-local) >>>", name);
    let legacy_begin = format!("# >>> ckit pack {} (machine-local symlinks) >>>", name);
    if let Ok(cur) = std::fs::read_to_string(&exclude) {
        if cur.contains(&legacy_begin) {
            let _ = std::fs::write(&exclude, cur.replace(&legacy_begin, &begin));
        }
    }
    let _ = crate::verbs::harness::memory::upsert_block(
        &exclude,
        &begin,
        &format!("# <<< ckit pack {} <<<", name),
        &format!("/{}.omp/settings.json", prefix),
    );
}

/// Registry `src` prefix older versions wrote for a `--global` pack.
pub(crate) const GLOBAL_SRC_PREFIX: &str = "pack-global:";

fn global_settings(home: &Path) -> PathBuf {
    home.join(".omp/agent/settings.json")
}

/// Undo every global pack install of older versions: the `~/.omp` copies, every
/// `pack-global:` row of `registry` (the global `skills.toml`) and every pack
/// store in `~/.omp/agent/settings.json` `extensions`. Run by `harness`
/// init/up/global, so `8sync up` migrates each machine.
pub(crate) fn retire_global_packs(home: &Path, registry: &Path) {
    remove_legacy_global_copies(home);
    let mut reg = super::discover::read_registry(registry);
    let mut dropped: Vec<String> = Vec::new();
    reg.retain(|_, e| match e.src.strip_prefix(GLOBAL_SRC_PREFIX) {
        Some(name) => {
            dropped.push(name.to_string());
            false
        }
        None => true,
    });
    if !dropped.is_empty() {
        match super::discover::write_registry(registry, &reg) {
            Ok(()) => ui::warn(&format!(
                "global pack {} removed from {}: packs now load only in repos that list them",
                dropped.join(", "),
                registry.display()
            )),
            Err(e) => ui::warn(&format!("could not update {}: {}", registry.display(), e)),
        }
    }
    for pk in discover_packs() {
        for store in [store_dir(home, &pk.name), legacy_store_dir(home, &pk.name)] {
            if let Err(e) = set_extension(&global_settings(home), home, &store, false) {
                ui::warn(&format!("global pack `{}` removal failed: {}", pk.name, e));
            }
        }
    }
}

/// Where a pre-extension `--global` install copied pack file `rel`.
fn legacy_global_target(home: &Path, rel: &str) -> PathBuf {
    match rel.strip_prefix("skills/") {
        Some(skill) => home.join(".omp/skills").join(skill),
        None => home.join(".omp/agent").join(rel),
    }
}

/// Undo every `~/.omp` copy of a pack (lock `~/.omp/agent/pack-<name>.lock`).
/// Only files still byte-identical to the lock are deleted. Edited files stay in the
/// lock, so every run reports them until the user moves or deletes them.
fn remove_legacy_global_copies(home: &Path) {
    let Ok(entries) = std::fs::read_dir(home.join(".omp/agent")) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_prefix("pack-").and_then(|f| f.strip_suffix(".lock")) else {
            continue;
        };
        let mut removed = 0usize;
        let mut kept: BTreeMap<String, String> = BTreeMap::new();
        for (rel, hash) in read_lock(&entry.path()) {
            let target = legacy_global_target(home, &rel);
            match std::fs::read_to_string(&target) {
                Ok(cur) if fnv1a(&cur) == hash => {
                    if std::fs::remove_file(&target).is_ok() {
                        removed += 1;
                        prune_empty_parents(&target, home);
                    }
                }
                Ok(_) => {
                    kept.insert(rel, hash);
                }
                Err(_) => {}
            }
        }
        if removed > 0 {
            ui::warn(&format!("old ~/.omp copy of pack `{}` removed ({} file(s)); packs now load as omp extensions", name, removed));
        }
        if kept.is_empty() {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        let _ = write_lock(&entry.path(), &kept);
        ui::warn(&format!("{} locally edited file(s) from the old ~/.omp copy of pack `{}` still load in every repo; move or delete them:", kept.len(), name));
        for rel in kept.keys() {
            ui::info(&format!("  {}", legacy_global_target(home, rel).display()));
        }
    }
}

/// Remove now-empty directories above `path`, stopping at `~/.omp/skills` / `~/.omp/agent`.
/// Empty subdirectories (the `references/` + `scripts/` a skill layout pass adds) don't
/// keep a directory alive.
fn prune_empty_parents(path: &Path, home: &Path) {
    let stops = [home.join(".omp/skills"), home.join(".omp/agent"), home.join(".omp")];
    let mut dir = path.parent();
    while let Some(d) = dir {
        if stops.iter().any(|s| s == d) {
            break;
        }
        for sub in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let _ = std::fs::remove_dir(sub.path());
        }
        if std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

/// Refresh the shared store from the embedded assets (store edits kept unless
/// `force`, which saves the edit as `.bak`), then delete store files the pack no
/// longer ships while they are still unedited.
fn refresh_store(home: &Path, name: &str, force: bool) -> Result<PathBuf> {
    move_legacy_store(home, name);
    let dir = store_dir(home, name);
    let files = pack_rels(name);
    if files.is_empty() {
        bail!("no bundled pack `{}` (assets/packs/{}/ not found)", name, name);
    }
    std::fs::create_dir_all(&dir)?;
    let lock_path = store_lock(&dir);
    let mut lock = read_lock(&lock_path);
    let (mut written, mut unchanged) = (0usize, 0usize);
    let mut kept: Vec<String> = Vec::new();
    for rel in &files {
        let Some(body) = assets::read(&format!("packs/{}/{}", name, rel)) else {
            continue;
        };
        let body = if rel.ends_with(".md") || rel.ends_with(".txt") { crate::brand::render(&body).into_owned() } else { body };
        let target = dir.join(rel);
        match std::fs::read_to_string(&target).ok() {
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

    let shipped: std::collections::BTreeSet<&String> = files.iter().collect();
    let mut orphaned: Vec<String> = Vec::new();
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
            Ok(_) => {
                orphaned.push(rel.clone());
                true
            }
            Err(_) => false,
        }
    });
    write_lock(&lock_path, &lock)?;
    for sub in pack_skill_names(name) {
        audit_skill_layout(&dir.join("skills").join(sub));
    }

    ui::ok(&format!("pack `{}` → {} file(s) written, {} unchanged → store {}", name, written, unchanged, dir.display()));
    if !kept.is_empty() {
        ui::warn(&format!(
            "{} locally edited store file(s) kept (rerun with --force to overwrite, old copy saved as .bak):",
            kept.len()
        ));
        for rel in &kept {
            ui::info(&format!("  {}", dir.join(rel).display()));
        }
    }
    if !orphaned.is_empty() {
        ui::warn(&format!(
            "{} store file(s) no longer shipped by pack `{}` but edited locally — kept, still loaded; move or delete them:",
            orphaned.len(),
            name
        ));
        for rel in &orphaned {
            ui::info(&format!("  {}", dir.join(rel).display()));
        }
    }
    Ok(dir)
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

/// Lock format: one `<hash>\t<path relative to the pack root>` per line.
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
        dir.canonicalize().unwrap()
    }

    fn extensions(settings: &Path) -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(settings).unwrap()).unwrap();
        v["extensions"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect()
    }

    fn list_pack(root: &Path) {
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::write(root.join("agents/skills.toml"), "[vtiger-php]\nsrc = \"pack:vtiger-php\"\n").unwrap();
    }

    #[test]
    fn project_install_adds_the_store_extension_once_and_keeps_other_settings() {
        let home = temp_home("ext");
        let root = home.join("repo");
        std::fs::create_dir_all(root.join(".omp")).unwrap();
        let settings = root.join(".omp/settings.json");
        std::fs::write(&settings, "{\"extensions\": [\"/opt/other\"], \"theme\": \"dark\"}").unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        install_pack(&home, &root, "vtiger-php", false).unwrap();
        let entry = |p: PathBuf| format!("~/{}", p.display());
        assert_eq!(
            extensions(&settings),
            vec!["/opt/other".to_string(), entry(store_dir(Path::new(""), "vtiger-php")), entry(personal_dir(Path::new(""), "vtiger-php"))]
        );
        assert!(personal_dir(&home, "vtiger-php").join("skills").is_dir());
        assert!(store_dir(&home, "vtiger-php").ends_with(".omp/shared/vtiger/pack"));
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");

        let store = store_dir(&home, "vtiger-php");
        for f in ["commands/review-pr.md", "rules/php-conventions.md", "agents/code-reviewer.md", "skills/review-pr/SKILL.md"] {
            assert!(store.join(f).is_file(), "{}", f);
        }
        // omp would run a top-level .ts/.js or `*/index.*` of an extension as code
        assert!(!store.join("RULES.md").exists());
        assert!(std::fs::read_dir(&store).unwrap().flatten().all(|e| e.path().is_dir()));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn invalid_settings_json_is_reported_and_left_untouched() {
        let home = temp_home("badjson");
        let root = home.join("repo");
        std::fs::create_dir_all(root.join(".omp")).unwrap();
        let settings = root.join(".omp/settings.json");
        std::fs::write(&settings, "{ // jsonc\n}").unwrap();
        assert!(install_pack(&home, &root, "vtiger-php", false).is_err());
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), "{ // jsonc\n}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn old_links_and_pristine_copies_are_retired_but_edits_stay_unless_forced() {
        let home = temp_home("retire");
        let root = home.join("repo");
        let store = refresh_store(&home, "vtiger-php", false).unwrap();
        let omp = root.join(".omp");
        std::fs::create_dir_all(omp.join("rules")).unwrap();
        std::fs::create_dir_all(omp.join("skills")).unwrap();
        std::os::unix::fs::symlink(store.join("commands/review-pr.md"), omp.join("commands-link.md")).unwrap();
        std::os::unix::fs::symlink(store.join("skills/field"), omp.join("skills/field")).unwrap();
        std::os::unix::fs::symlink(store.join("RULES.md"), omp.join("RULES.md")).unwrap(); // dangling
        std::fs::copy(store.join("rules/php83.md"), omp.join("rules/php83.md")).unwrap();
        std::fs::write(omp.join("rules/migration-rules.md"), "local edit").unwrap();
        std::fs::write(omp.join("rules/team-own.md"), "not a pack file").unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        for gone in ["commands-link.md", "skills/field", "RULES.md", "rules/php83.md"] {
            assert!(std::fs::symlink_metadata(omp.join(gone)).is_err(), "{}", gone);
        }
        assert_eq!(std::fs::read_to_string(omp.join("rules/migration-rules.md")).unwrap(), "local edit");
        assert_eq!(std::fs::read_to_string(omp.join("rules/team-own.md")).unwrap(), "not a pack file");

        install_pack(&home, &root, "vtiger-php", true).unwrap();
        assert!(!omp.join("rules/migration-rules.md").exists());
        let backup = std::fs::read_dir(omp.join("removed")).unwrap().flatten().next().unwrap().path();
        assert_eq!(std::fs::read_to_string(backup.join("rules/migration-rules.md")).unwrap(), "local edit");
        assert!(omp.join("rules/team-own.md").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn copies_of_files_the_pack_dropped_are_retired_by_their_lock_hash() {
        let home = temp_home("dropped");
        let root = home.join("repo");
        let omp = root.join(".omp");
        std::fs::create_dir_all(omp.join("rules")).unwrap();
        std::fs::write(omp.join("rules/primary-workflow.md"), "old shipped body").unwrap();
        std::fs::write(omp.join("rules/error-patterns-pending.md"), "shipped\n+ local draft").unwrap();
        let lock = format!(
            "{}\trules/primary-workflow.md\n{}\trules/error-patterns-pending.md\n",
            fnv1a("old shipped body"),
            fnv1a("shipped")
        );
        std::fs::write(omp.join("pack-vtiger-php.lock"), lock).unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        assert!(!omp.join("rules/primary-workflow.md").exists());
        assert_eq!(std::fs::read_to_string(omp.join("rules/error-patterns-pending.md")).unwrap(), "shipped\n+ local draft");
        assert!(!omp.join("pack-vtiger-php.lock").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn settings_stay_out_of_git_and_the_old_per_entry_block_is_replaced() {
        let home = temp_home("gitx");
        let repo = home.join("repo");
        let root = repo.join("app");
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&repo).args(args).output().unwrap();
        assert!(git(&["init", "-q"]).status.success());
        std::fs::write(root.join("keep.txt"), "x").unwrap();
        let exclude = repo.join(".git/info/exclude");
        std::fs::write(
            &exclude,
            "# >>> ckit pack vtiger-php (machine-local symlinks) >>>\n/app/.omp/commands/review-pr.md\n# <<< ckit pack vtiger-php <<<\n*.log\n",
        )
        .unwrap();

        install_pack(&home, &root, "vtiger-php", false).unwrap();
        let status = String::from_utf8(git(&["status", "--porcelain", "--untracked-files=all"]).stdout).unwrap();
        assert_eq!(status.trim(), "?? app/keep.txt");
        let body = std::fs::read_to_string(&exclude).unwrap();
        assert_eq!(body.matches("# >>> ckit pack vtiger-php").count(), 1);
        assert!(body.contains("/app/.omp/settings.json\n"));
        assert!(!body.contains("review-pr.md"));
        assert!(body.contains("*.log"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn launch_installs_a_listed_pack_and_migrates_an_old_link_install_only() {
        let home = temp_home("launch");
        let (clone, linked, plain) = (home.join("clone"), home.join("linked"), home.join("plain"));
        list_pack(&clone);
        list_pack(&linked);
        std::fs::create_dir_all(&plain).unwrap();
        let store = refresh_store(&home, "vtiger-php", false).unwrap();
        std::fs::create_dir_all(linked.join(".omp/commands")).unwrap();
        std::os::unix::fs::symlink(store.join("commands/review-pr.md"), linked.join(".omp/commands/review-pr.md")).unwrap();
        std::fs::write(linked.join(".omp/settings.json"), format!("{{\"extensions\":[\"{}\"]}}", store.display())).unwrap();

        for root in [&clone, &linked, &plain] {
            ensure_listed_packs(&home, root);
        }
        assert!(has_extension(&clone.join(".omp/settings.json"), &home, &store));
        assert!(std::fs::symlink_metadata(linked.join(".omp/commands/review-pr.md")).is_err());
        assert_eq!(extensions(&linked.join(".omp/settings.json")).len(), 2);
        assert!(!plain.join(".omp").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn up_migrates_recorded_projects_and_drops_the_record() {
        let home = temp_home("record");
        let (listed, dropped) = (home.join("listed"), home.join("dropped"));
        list_pack(&listed);
        std::fs::create_dir_all(&dropped).unwrap();
        let record = legacy_projects_file(&home, "vtiger-php");
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(&record, format!("{}\n{}\n", listed.display(), dropped.display())).unwrap();

        refresh_stores(&home);
        let store = store_dir(&home, "vtiger-php");
        assert!(has_extension(&listed.join(".omp/settings.json"), &home, &store));
        assert!(!dropped.join(".omp").exists());
        assert!(!record.exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn recreated_old_store_beside_the_new_one_loses_only_unedited_files() {
        let home = temp_home("both");
        refresh_store(&home, "vtiger-php", false).unwrap();
        let old = legacy_store_dir(&home, "vtiger-php");
        std::fs::create_dir_all(old.join("rules")).unwrap();
        std::fs::write(old.join("rules/a.md"), "pristine").unwrap();
        std::fs::write(old.join("rules/b.md"), "my edit").unwrap();
        let mut lock = BTreeMap::new();
        lock.insert("rules/a.md".to_string(), fnv1a("pristine"));
        lock.insert("rules/b.md".to_string(), fnv1a("original"));
        write_lock(&old.with_file_name("vtiger-php.lock"), &lock).unwrap();

        refresh_stores(&home);
        assert!(!old.join("rules/a.md").exists());
        assert_eq!(std::fs::read_to_string(old.join("rules/b.md")).unwrap(), "my edit");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn old_config_store_moves_to_the_shared_folder_and_projects_are_repointed() {
        let home = temp_home("move");
        let root = home.join("repo");
        list_pack(&root);
        let old = legacy_store_dir(&home, "vtiger-php");
        std::fs::create_dir_all(old.join("rules")).unwrap();
        std::fs::write(old.join("rules/php-conventions.md"), "my edit").unwrap();
        std::fs::write(old.with_file_name("vtiger-php.lock"), "").unwrap();
        std::fs::create_dir_all(root.join(".omp")).unwrap();
        let settings = root.join(".omp/settings.json");
        let old_entry = extension_entry(&home, &old);
        std::fs::write(&settings, format!("{{\"extensions\":[\"{}\",\"/opt/other\"]}}", old_entry)).unwrap();

        refresh_stores(&home);
        let store = store_dir(&home, "vtiger-php");
        assert!(!old.exists() && !old.with_file_name("vtiger-php.lock").exists());
        assert_eq!(std::fs::read_to_string(store.join("rules/php-conventions.md")).unwrap(), "my edit");
        assert!(store.join("skills/review-pr/SKILL.md").is_file());

        ensure_listed_packs(&home, &root);
        let ext = extensions(&settings);
        assert!(!ext.iter().any(|e| e.contains(".config")), "{:?}", ext);
        assert!(has_extension(&settings, &home, &store) && has_extension(&settings, &home, &personal_dir(&home, "vtiger-php")));
        assert!(ext.contains(&"/opt/other".to_string()));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn old_global_copies_go_but_edited_ones_stay_reported() {
        let home = temp_home("global");
        let registry = home.join("skills.toml");
        // an old `--global` copy: one pristine file, one edited
        let lock = home.join(".omp/agent/pack-vtiger-php.lock");
        std::fs::create_dir_all(home.join(".omp/skills/field/scripts")).unwrap();
        std::fs::write(home.join(".omp/skills/field/SKILL.md"), "pack body").unwrap();
        std::fs::create_dir_all(home.join(".omp/agent/rules")).unwrap();
        std::fs::write(home.join(".omp/agent/rules/php83.md"), "local edit").unwrap();
        std::fs::write(&lock, format!("{}\tskills/field/SKILL.md\n{}\trules/php83.md\n", fnv1a("pack body"), fnv1a("pack"))).unwrap();

        retire_global_packs(&home, &registry);
        assert!(!home.join(".omp/skills/field").exists());
        assert!(home.join(".omp/skills").is_dir());
        assert_eq!(std::fs::read_to_string(home.join(".omp/agent/rules/php83.md")).unwrap(), "local edit");
        // the edited copy stays tracked, so later runs keep reporting it
        assert_eq!(read_lock(&lock).into_keys().collect::<Vec<_>>(), ["rules/php83.md"]);
        retire_global_packs(&home, &registry);
        assert_eq!(read_lock(&lock).len(), 1);
        std::fs::remove_file(home.join(".omp/agent/rules/php83.md")).unwrap();
        retire_global_packs(&home, &registry);
        assert!(!lock.exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn every_pack_global_row_and_global_extension_is_dropped_even_without_a_copy() {
        let home = temp_home("globalrow");
        let registry = home.join("skills.toml");
        let settings = global_settings(&home);
        std::fs::create_dir_all(home.join(".omp/agent")).unwrap();
        std::fs::write(&settings, "{\"theme\": \"dark\"}").unwrap();
        set_extension(&settings, &home, &store_dir(&home, "vtiger-php"), true).unwrap();
        assert_eq!(extensions(&settings).len(), 1);
        let rows = "[vtiger-php]\nsrc = \"pack-global:vtiger-php\"\n\n[karpathy]\nsrc = \"builtin:karpathy\"\nwhen = \"always\"\n";
        std::fs::write(&registry, rows).unwrap();

        retire_global_packs(&home, &registry);
        let reg = super::super::discover::read_registry(&registry);
        assert!(!reg.contains_key("vtiger-php"));
        assert_eq!(reg["karpathy"].src, "builtin:karpathy");
        let body = std::fs::read_to_string(&settings).unwrap();
        assert!(!body.contains("extensions"));
        assert!(body.contains("dark"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn store_refresh_drops_retired_files_but_keeps_edited_ones() {
        let home = temp_home("prune");
        let dir = refresh_store(&home, "vtiger-php", false).unwrap();
        let lock_path = store_lock(&dir);
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
}
