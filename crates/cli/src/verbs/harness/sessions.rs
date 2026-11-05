//! Resolve a project's omp session directory (`~/.omp/agent/sessions/<slug>`).
//!
//! omp's slug format has changed across versions (`-a-b-c` → `--a-b-c--`), so
//! hard-coding it breaks on every omp update. Each session JSONL carries the
//! ground truth instead: a `{"type":"session","cwd":"..."}` header line. Scan
//! the sessions root and match that `cwd` against the project root; fall back
//! to the legacy single-dash slug for dirs whose files lack a readable header.

use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Session dir for `root`; newest-modified wins when several dirs match.
pub(crate) fn find_sessions_dir(home: &Path, root: &Path) -> Option<PathBuf> {
    let base = home.join(".omp/agent/sessions");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(&base) {
        for ent in rd.flatten() {
            let dir = ent.path();
            if !dir.is_dir() {
                continue;
            }
            if dir_cwd(&dir).is_some_and(|c| c == root) {
                let mtime = ent
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().map_or(true, |(t, _)| mtime > *t) {
                    best = Some((mtime, dir));
                }
            }
        }
    }
    if let Some((_, dir)) = best {
        return Some(dir);
    }

    // Legacy fallback (old omp single-dash slug) for header-less session files.
    let rel = root.strip_prefix(home).unwrap_or(root);
    let legacy = base.join(format!(
        "-{}",
        rel.to_string_lossy().trim_start_matches('/').replace('/', "-")
    ));
    legacy.is_dir().then_some(legacy)
}

/// The `cwd` recorded in any of the dir's session JSONL headers.
pub(crate) fn dir_cwd(dir: &Path) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(cwd) = session_cwd(&p) {
            return Some(cwd);
        }
    }
    None
}

/// `cwd` from the `{"type":"session",...}` header within a file's first lines.
fn session_cwd(path: &Path) -> Option<PathBuf> {
    let f = std::fs::File::open(path).ok()?;
    for line in std::io::BufReader::new(f).lines().take(8) {
        let line = line.ok()?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("session") {
            return v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from);
        }
    }
    None
}
