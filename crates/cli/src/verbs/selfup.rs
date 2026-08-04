// Self-update: pull the prebuilt binary from the latest GitHub Release of the
// PUBLIC repo `dante241/ckit_agent`. Public assets download straight from the
// `browser_download_url`, so no token is needed. A token (CKIT_GITHUB_TOKEN /
// GITHUB_TOKEN) is used only when present, to dodge the 60-req/hour anonymous
// API rate limit. Also exposes a rate-limited auto-check used from main().

use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::ui;

const REPO_OWNER: &str = "dante241";
const REPO_NAME: &str = "ckit_agent";
/// Release asset name prefix (`ckit-<tag><platform suffix>`).
const ASSET_PREFIX: &str = "ckit-";
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600); // 6h

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("8sync")
}

fn last_check_file() -> PathBuf { cache_dir().join("last_check") }
fn last_seen_tag_file() -> PathBuf { cache_dir().join("last_seen_tag") }

fn build_version() -> &'static str { env!("CARGO_PKG_VERSION") }

fn should_check() -> bool {
    let p = last_check_file();
    if !p.exists() { return true; }
    let modified = match p.metadata().and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return true,
    };
    SystemTime::now().duration_since(modified).map(|d| d > CHECK_INTERVAL).unwrap_or(true)
}

fn touch_check() {
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = std::fs::write(last_check_file(), "");
}

/// Strip leading `v` from a release tag so it can be compared to CARGO_PKG_VERSION.
fn strip_v(s: &str) -> &str { s.strip_prefix('v').unwrap_or(s) }

/// Map the running platform to the release asset suffix produced by
/// `.github/workflows/release.yml` and consumed by `install.sh`.
fn asset_suffix_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64" | "amd64") => Some("-linux-x86_64"),
        ("linux", "aarch64" | "arm64") => Some("-linux-aarch64"),
        ("macos" | "darwin", "x86_64" | "amd64") => Some("-darwin-x86_64"),
        ("macos" | "darwin", "aarch64" | "arm64") => Some("-darwin-arm64"),
        ("windows", "x86_64" | "amd64") => Some("-windows-x86_64.exe"),
        _ => None,
    }
}

fn current_asset_suffix() -> Result<&'static str> {
    asset_suffix_for(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        anyhow!(
            "no prebuilt ckit asset for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })
}

fn wanted_asset_name(tag: &str) -> Result<String> {
    Ok(format!("{}{}{}", ASSET_PREFIX, tag, current_asset_suffix()?))
}

fn verify_downloaded_binary(bin: &std::path::Path, expected_version: &str) -> Result<()> {
    let out = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|e| anyhow!("downloaded asset is not executable on this machine: {e}"))?;
    if !out.status.success() {
        bail!("downloaded asset failed `--version` with status {}", out.status);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let got = stdout.trim();
    let want = format!("ckit {expected_version}");
    if got != want {
        bail!("downloaded asset version mismatch: expected `{want}`, got `{got}`");
    }
    Ok(())
}

/// Optional GitHub token. Public repo needs none; when present (CKIT_GITHUB_TOKEN
/// or GITHUB_TOKEN) it's added to API calls to dodge the 60-req/hour anon limit.
fn github_token() -> Option<String> {
    std::env::var("CKIT_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .filter(|s| !s.is_empty())
}

/// Build the base curl args for a GitHub API GET, adding the token header only
/// when a token is available.
fn api_curl_args<'a>(url: &'a str, auth: &'a Option<String>) -> Vec<&'a str> {
    let mut a = vec![
        "-fsSL", "--max-time", "5",
        "-H", "Accept: application/vnd.github+json",
        "-H", "User-Agent: ckit-selfup",
    ];
    if let Some(h) = auth {
        a.push("-H");
        a.push(h.as_str());
    }
    a.push(url);
    a
}

/// Query GitHub for the latest release. Returns `(tag_name, browser_download_url)`.
/// Public repo → the browser URL downloads without auth. Short timeout so it never
/// blocks. Silent on offline / no-curl.
fn fetch_latest_release() -> Result<Option<(String, String)>> {
    if which::which("curl").is_err() { return Ok(None); }
    let auth = github_token().map(|t| format!("Authorization: Bearer {}", t));
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );
    let out = Command::new("curl").args(api_curl_args(&url, &auth)).output().ok();
    let Some(out) = out else { return Ok(None); };
    if !out.status.success() { return Ok(None); }
    let Some(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout).ok() else {
        return Ok(None);
    };
    let Some(tag) = v.get("tag_name").and_then(|t| t.as_str()).map(str::to_owned) else {
        return Ok(None);
    };
    let want_name = wanted_asset_name(&tag)?;
    let Some(assets) = v.get("assets").and_then(|a| a.as_array()) else {
        return Ok(None);
    };
    for a in assets {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name == want_name {
            let Some(dl) = a.get("browser_download_url").and_then(|u| u.as_str()) else {
                return Ok(None);
            };
            return Ok(Some((tag, dl.to_string())));
        }
    }
    Ok(None)
}

/// Cheap auto-check called from main(). Prints a 1-line notice if a newer
/// release exists. Never blocks for long (5s timeout). Silent on offline.
pub fn auto_check_notice() {
    if std::env::var("SUSYNC_NO_AUTO_CHECK").is_ok() { return; }
    if !should_check() { return; }
    touch_check();
    let local = build_version();
    let Ok(Some((tag, _url))) = fetch_latest_release() else { return; };
    let remote = strip_v(&tag);
    if remote == local { return; }
    let _ = std::fs::write(last_seen_tag_file(), &tag);
    eprintln!(
        "\x1b[33m! ckit update available: v{} → {} — run `ckit up` to install\x1b[0m",
        local, tag
    );
}

/// Force self-update: download the latest release asset and install it.
/// Returns Ok(true) when a new binary was written, Ok(false) when already up-to-date.
pub fn run_self_update(force: bool) -> Result<bool> {
    ui::step("Self-update — GitHub Releases");
    let local = build_version();

    let (tag, asset_url) = fetch_latest_release()?
        .ok_or_else(|| anyhow!("could not query latest release from github.com/{}/{}", REPO_OWNER, REPO_NAME))?;
    let remote = strip_v(&tag);
    if !force && remote == local {
        ui::skip("ckit", &format!("up to date (v{})", local));
        return Ok(false);
    }

    ui::info(&format!("local v{} → {} ({})", local, tag, asset_url));
    let bin_dst = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join(".local/bin/ckit");
    std::fs::create_dir_all(bin_dst.parent().unwrap())?;

    // Atomic-ish replace: download to a sibling temp file then rename. Public
    // repo → the browser_download_url needs no auth.
    let tmp = bin_dst.with_extension(format!("new.{}", std::process::id()));
    let status = Command::new("curl")
        .args([
            "-fsSL", "--max-time", "120",
            "-H", "User-Agent: ckit-selfup",
            "-o", tmp.to_str().unwrap(), &asset_url,
        ])
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("download failed: {}", asset_url);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    verify_downloaded_binary(&tmp, remote)?;
    std::fs::rename(&tmp, &bin_dst)?;

    let _ = std::fs::write(last_seen_tag_file(), &tag);
    ui::ok(&format!("installed {} → {}", tag, bin_dst.display()));
    Ok(true)
}

/// Resolve the public browser download url for a specific tag's asset.
fn fetch_asset_url_for_tag(tag: &str) -> Result<Option<String>> {
    let auth = github_token().map(|t| format!("Authorization: Bearer {}", t));
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/tags/{}",
        REPO_OWNER, REPO_NAME, tag
    );
    let mut args = api_curl_args(&url, &auth);
    if let Some(pos) = args.iter().position(|a| *a == "5") { args[pos] = "10"; }
    let out = Command::new("curl").args(args).output().ok();
    let Some(out) = out else { return Ok(None); };
    if !out.status.success() { return Ok(None); }
    let Some(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout).ok() else {
        return Ok(None);
    };
    let want_name = wanted_asset_name(tag)?;
    let Some(assets) = v.get("assets").and_then(|a| a.as_array()) else {
        return Ok(None);
    };
    for a in assets {
        if a.get("name").and_then(|n| n.as_str()) == Some(want_name.as_str()) {
            let Some(dl) = a.get("browser_download_url").and_then(|u| u.as_str()) else {
                return Ok(None);
            };
            return Ok(Some(dl.to_string()));
        }
    }
    Ok(None)
}

/// Install a specific tag (e.g. `v0.6.10`). Used by `ckit up --to <tag>`
/// for reproducibility / explicit downgrade.
pub fn install_tag(tag: &str) -> Result<bool> {
    ui::step(&format!("Self-update → pinned tag {}", tag));
    let tag = tag.strip_prefix('v').map(|t| format!("v{}", t)).unwrap_or_else(|| format!("v{}", tag));
    let want_name = wanted_asset_name(&tag)?;
    let asset_url = fetch_asset_url_for_tag(&tag)?
        .ok_or_else(|| anyhow!("release {} has no asset {}", tag, want_name))?;
    let bin_dst = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join(".local/bin/ckit");
    std::fs::create_dir_all(bin_dst.parent().unwrap())?;
    let tmp = bin_dst.with_extension(format!("new.{}", std::process::id()));
    ui::info(&format!("$ curl -fsSL --max-time 120 -o {} {}", tmp.display(), asset_url));
    let status = Command::new("curl")
        .args([
            "-fsSL", "--max-time", "120",
            "-H", "User-Agent: ckit-selfup",
            "-o", tmp.to_str().unwrap(), &asset_url,
        ])
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("download failed: {}", asset_url);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    verify_downloaded_binary(&tmp, strip_v(&tag))?;
    std::fs::rename(&tmp, &bin_dst)?;
    let _ = std::fs::write(last_seen_tag_file(), &tag);
    ui::ok(&format!("installed {} → {}", tag, bin_dst.display()));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{asset_suffix_for, verify_downloaded_binary};

    #[test]
    fn maps_release_asset_suffix_by_platform() {
        assert_eq!(asset_suffix_for("linux", "x86_64"), Some("-linux-x86_64"));
        assert_eq!(asset_suffix_for("linux", "aarch64"), Some("-linux-aarch64"));
        assert_eq!(asset_suffix_for("linux", "arm64"), Some("-linux-aarch64"));
        assert_eq!(asset_suffix_for("macos", "aarch64"), Some("-darwin-arm64"));
        assert_eq!(asset_suffix_for("darwin", "arm64"), Some("-darwin-arm64"));
        assert_eq!(asset_suffix_for("macos", "x86_64"), Some("-darwin-x86_64"));
        assert_eq!(asset_suffix_for("windows", "amd64"), Some("-windows-x86_64.exe"));
        assert_eq!(asset_suffix_for("freebsd", "x86_64"), None);
    }

    #[cfg(unix)]
    #[test]
    fn verifies_downloaded_binary_version_before_install() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "ckit-version-guard-{}",
            std::process::id()
        ));
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        writeln!(file, "echo 'ckit 9.9.9'").unwrap();
        drop(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        verify_downloaded_binary(&path, "9.9.9").unwrap();
        let err = verify_downloaded_binary(&path, "1.2.3").unwrap_err().to_string();
        assert!(err.contains("version mismatch"));

        let _ = std::fs::remove_file(path);
    }
}
