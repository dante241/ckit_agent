//! Bundled-skill deployment (embedded assets → ~/.omp/skills) and codegraph
//! bootstrap. The building blocks `8sync harness init` composes.
use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::{assets, env_detect, ui};

/// Deploy every bundled skill tree under `assets/skills/<name>/` into
/// `~/.omp/skills/<name>/`. Each tree is deployed verbatim including any
/// `references/` or `scripts/` subdirs. Shell scripts get mode 0755.
pub(crate) fn install_bundled_global(env: &env_detect::Env) -> Result<()> {
    let skills_dir = env.home.join(".omp/skills");
    // (asset prefix, target subdir name). always-on first (read order), then
    // on-demand specialists. Encore/full-flow are on-demand + tech-gated.
    let bundled: [(&str, &str); 17] = [
        ("skills/codegraph",               "codegraph"),
        ("skills/karpathy",                "karpathy-guidelines"),
        ("skills/ponytail",                "ponytail"),
        ("skills/assp-skill",              "assp-skill"),
        ("skills/impeccable",              "impeccable"),
        ("skills/taste-skill",             "taste-skill"),
        ("skills/8sync-cli",               "8sync-cli"),
        ("skills/image-routing",           "image-routing"),
        ("skills/locate-anything",         "locate-anything"),
        ("skills/code-review-and-quality", "code-review-and-quality"),
        ("skills/senior-security",         "senior-security"),
        ("skills/senior-frontend",         "senior-frontend"),
        ("skills/full-flow",               "full-flow"),
        ("skills/encore-deploy",           "encore-deploy"),
        ("skills/last30days",              "last30days"),
        ("skills/token-bench",             "token-bench"),
        ("skills/feature",                 "feature"),
    ];
    for (asset_prefix, name) in bundled {
        let target_dir = skills_dir.join(name);
        std::fs::create_dir_all(&target_dir)?;
        let (written, _unchanged) = assets::install_tree(asset_prefix, &target_dir)?;
        if written > 0 {
            ui::ok(&format!("synced {} ({} file(s) written) → {}", name, written, target_dir.display()));
        }
    }
    Ok(())
}

/// Clean cutover for machines that installed an earlier 8sync: remove the retired
/// `/gs` command + skill (global + project). Idempotent no-op when absent — `/auto`
/// is the single automation entry now.
pub(crate) fn cleanup_legacy_gs(home: &Path, root: Option<&Path>) {
    let _ = std::fs::remove_file(home.join(".omp/agent/commands/gs.md"));
    let _ = std::fs::remove_dir_all(home.join(".omp/skills/gs"));
    if let Some(r) = root {
        let _ = std::fs::remove_file(r.join(".omp/commands/gs.md"));
        let _ = std::fs::remove_dir_all(r.join(".omp/skills/gs"));
        let _ = std::fs::remove_dir_all(r.join("su-code/skills/gs")); // pre-rename legacy location
    }
}

/// Ensure a skill directory follows the Agent Skills 3-folder layout:
///   <name>/ ├── SKILL.md  ├── scripts/  └── references/
/// Idempotent. Empty dirs are intentional.
pub(crate) fn ensure_skill_layout(dir: &Path) {
    for sub in ["scripts", "references"] {
        let p = dir.join(sub);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }
}

/// Recursively copy `src` into `dst`. Skips `.git/` (vendor copies should not
/// carry the git history of an unrelated repo). Overwrites existing files.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" { continue; }
        let from = entry.path();
        let to = dst.join(&name);
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_symlink() {
            // Resolve and copy the target as a regular file (keeps vendor copy self-contained).
            if let Ok(target) = std::fs::read_link(&from) {
                let resolved = if target.is_absolute() { target } else { from.parent().map(|p| p.join(&target)).unwrap_or(target) };
                if resolved.is_file() {
                    std::fs::copy(&resolved, &to)?;
                }
            }
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Make sure the `codegraph` binary is installed (upstream curl installer) and
/// registered in the skills.toml registry. The SKILL.md tree is deployed
/// separately from embedded assets.
pub(crate) fn ensure_codegraph(env: &env_detect::Env) -> Result<()> {
    if which::which("codegraph").is_err() {
        ui::step("codegraph (binary missing — running upstream installer)");
        // Windows has no POSIX `sh` for the curl|sh bundle installer; codegraph
        // ships on npm (`@colbymchenry/codegraph`), so install it via bun/npm.
        if crate::platform::os() == crate::platform::Os::Windows {
            match install_node_pkg("codegraph", "@colbymchenry/codegraph") {
                Ok(()) => ui::ok("codegraph installed"),
                Err(e) => ui::warn(&format!("{} — skill SKILL.md was still deployed", e)),
            }
        } else {
            let url = "https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.sh";
            let st = Command::new("sh")
                .arg("-c")
                .arg(format!("curl -fsSL {} | sh", url))
                .status();
            match st {
                Ok(s) if s.success() => ui::ok("codegraph installed"),
                Ok(s) => ui::warn(&format!("codegraph installer exited {} — skill SKILL.md was still deployed", s)),
                Err(e) => ui::warn(&format!("could not run installer: {} — continuing", e)),
            }
        }
    } else {
        let v = env_detect::cmd_version("codegraph", &["--version"]).unwrap_or_default();
        ui::skip("codegraph", &format!("present ({})", v));
    }

    let toml_path = crate::brand::config_dir(&env.home).join("skills.toml");
    if let Some(parent) = toml_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = std::fs::read_to_string(&toml_path).unwrap_or_default();
    if !existing.contains("[codegraph]") {
        let mut s = existing;
        if !s.ends_with('\n') && !s.is_empty() {
            s.push('\n');
        }
        s.push_str("\n[codegraph]\nsrc  = \"builtin:codegraph\"\nwhen = \"always\"\n");
        std::fs::write(&toml_path, s)?;
        ui::ok(&format!("registered 'codegraph' → {}", toml_path.display()));
    }
    Ok(())
}

/// Register CodeGraph's local MCP server with omp. The same `codegraph` binary
/// serves MCP over stdio with `codegraph serve --mcp`; project indexes still live
/// per-repo in `.codegraph/` and are initialized by `ensure_codegraph_init`.
pub(crate) fn ensure_codegraph_mcp(env: &env_detect::Env) -> Result<()> {
    if which::which("codegraph").is_err() {
        ui::warn("codegraph binary not on PATH — skipping codegraph MCP registration");
        return deregister_omp_mcp(&env.home, "codegraph");
    }
    register_omp_mcp(&env.home, "codegraph", "codegraph", &["serve", "--mcp"], &[])
}

/// If `<root>/.codegraph/` is missing and the `codegraph` binary is on PATH,
/// run `codegraph init <root>`. Best-effort: warns on failure, never bails.
pub(crate) fn ensure_codegraph_init(root: &Path) {
    let marker = root.join(".codegraph");
    if marker.exists() {
        ui::skip(&marker.display().to_string(), "codegraph already initialised");
        return;
    }
    if which::which("codegraph").is_err() {
        ui::warn("codegraph binary not on PATH — skipping `codegraph init`");
        return;
    }
    ui::step(&format!("codegraph init {}", root.display()));
    let st = Command::new("codegraph").arg("init").arg(root).status();
    match st {
        Ok(s) if s.success() => ui::ok(&format!("initialised {}", marker.display())),
        Ok(s) => ui::warn(&format!("`codegraph init` exited {} — run manually", s)),
        Err(e) => ui::warn(&format!("could not invoke codegraph: {}", e)),
    }
}

/// Ensure the `codebase-memory-mcp` binary is installed (upstream installer,
/// binary-only) and registered as an omp MCP server. Mirrors `ensure_codegraph`:
/// `8sync harness` auto-sets-up code intelligence so the agent gets the graph
/// tools (search_graph/trace_path/get_architecture/…) with zero manual config.
pub(crate) fn ensure_codebase_memory_mcp(env: &env_detect::Env) -> Result<()> {
    if which::which("codebase-memory-mcp").is_err() {
        ui::step("codebase-memory-mcp (binary missing — upstream installer, binary-only)");
        // The upstream installer is a POSIX `curl | bash` script; Windows has no
        // `sh`, so skip it there rather than shelling out to a missing tool.
        if crate::platform::os() == crate::platform::Os::Windows {
            ui::warn("codebase-memory-mcp has no Windows installer yet — skipping MCP registration");
            return deregister_omp_mcp(&env.home, "codebase-memory-mcp");
        }
        let url = "https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/main/install.sh";
        let st = Command::new("sh")
            .arg("-c")
            .arg(format!("curl -fsSL {} | bash -s -- --skip-config", url))
            .status();
        match st {
            Ok(s) if s.success() => ui::ok("codebase-memory-mcp installed"),
            Ok(s) => ui::warn(&format!("codebase-memory-mcp installer exited {} — continuing", s)),
            Err(e) => ui::warn(&format!("could not run installer: {} — continuing", e)),
        }
    } else {
        let v = env_detect::cmd_version("codebase-memory-mcp", &["--version"]).unwrap_or_default();
        ui::skip("codebase-memory-mcp", &format!("present ({})", v));
    }
    // Only register when the binary actually resolved — otherwise omp would get
    // a broken MCP entry pointing at a missing command. Purge any stale entry.
    if which::which("codebase-memory-mcp").is_err() {
        ui::warn("codebase-memory-mcp not on PATH — skipping MCP registration");
        return deregister_omp_mcp(&env.home, "codebase-memory-mcp");
    }
    // Self-index on every MCP connect — no manual reindex needed thereafter.
    let _ = Command::new("codebase-memory-mcp")
        .args(["config", "set", "auto_index", "true"])
        .status();
    register_omp_mcp(&env.home, "codebase-memory-mcp", "codebase-memory-mcp", &[], &[])
}

/// Idempotently add an MCP server `name` (stdio `command` + `args`) to omp's user
/// MCP config (`~/.omp/agent/mcp.json`), preserving any servers already there.
fn register_omp_mcp(home: &Path, name: &str, command: &str, args: &[&str], env: &[(&str, &str)]) -> Result<()> {
    let mcp_path = home.join(".omp/agent/mcp.json");
    if let Some(p) = mcp_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut root: serde_json::Value = std::fs::read_to_string(&mcp_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let obj = root.as_object_mut().unwrap();
    obj.entry("$schema").or_insert_with(|| {
        serde_json::Value::String(
            "https://raw.githubusercontent.com/can1357/oh-my-pi/main/packages/coding-agent/src/config/mcp-schema.json"
                .to_string(),
        )
    });
    let servers = obj.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }
    let smap = servers.as_object_mut().unwrap();
    let mut desired = serde_json::json!({ "type": "stdio", "command": command, "args": args });
    // Only emit an `env` key when there are vars — keeps the stored entry for the
    // env-less servers byte-identical (so the equality self-heal check holds).
    if !env.is_empty() {
        let env_obj: serde_json::Map<String, serde_json::Value> = env
            .iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
            .collect();
        desired
            .as_object_mut()
            .expect("stdio mcp server is an object")
            .insert("env".into(), serde_json::Value::Object(env_obj));
    }
    if smap.get(name) == Some(&desired) {
        ui::skip(name, "already in omp mcp.json");
        return Ok(());
    }
    // Self-heal: update in place when the command/args changed (e.g. serena's
    // executable rename) instead of skipping a stale entry.
    let updating = smap.contains_key(name);
    smap.insert(name.to_string(), desired);
    std::fs::write(&mcp_path, serde_json::to_string_pretty(&root)?)?;
    let verb = if updating { "updated" } else { "registered" };
    ui::ok(&format!("{} {} MCP → {}", verb, name, mcp_path.display()));
    Ok(())
}

/// Best-effort bootstrap of `uv` (Astral's Python tool manager) — the canonical
/// installer for both `headroom-ai[mcp]` and serena (`uvx`). User-level curl
/// install (no sudo); lands in `~/.local/bin` (already on PATH). Idempotent.
/// Returns true if `uv` is available afterwards.
fn ensure_uv() -> bool {
    if which::which("uv").is_ok() {
        return true;
    }
    ui::step("uv (missing — bootstrapping Astral uv: powers headroom + serena)");
    if crate::platform::os() == crate::platform::Os::Windows {
        // No POSIX `sh` on Windows — use uv's PowerShell installer instead.
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", "irm https://astral.sh/uv/install.ps1 | iex"])
            .status();
    } else {
        let _ = Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://astral.sh/uv/install.sh | sh")
            .status();
    }
    which::which("uv").is_ok()
}

/// Install an npm-distributed CLI on Windows, where the upstream `curl | sh`
/// installers don't run. Prefers `bun` (how omp itself self-updates), falls
/// back to `npm`, else errors with an actionable hint. Resolves the tool via
/// `which` because `std::process::Command` on Windows won't find `npm.cmd`
/// from the bare name `npm`.
pub(crate) fn install_node_pkg(bin: &str, pkg_name: &str) -> Result<()> {
    if let Ok(bun) = which::which("bun") {
        let st = Command::new(bun).args(["add", "-g", pkg_name]).status()?;
        if !st.success() {
            anyhow::bail!("`bun add -g {pkg_name}` failed");
        }
    } else if let Ok(npm) = which::which("npm") {
        let st = Command::new(npm).args(["install", "-g", pkg_name]).status()?;
        if !st.success() {
            anyhow::bail!("`npm install -g {pkg_name}` failed");
        }
    } else {
        anyhow::bail!(
            "cannot install {bin} ({pkg_name}): no `bun` or `npm` on PATH. \
             Install Bun (https://bun.sh) or Node.js, then re-run."
        );
    }
    Ok(())
}

/// Remove a stale MCP server from omp's `mcp.json` (e.g. a tool whose binary
/// failed to install) so omp never fails at startup spawning a missing
/// executable. No-op when absent or the file is unreadable.
fn deregister_omp_mcp(home: &Path, name: &str) -> Result<()> {
    let mcp_path = home.join(".omp/agent/mcp.json");
    let Ok(s) = std::fs::read_to_string(&mcp_path) else {
        return Ok(());
    };
    let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&s) else {
        return Ok(());
    };
    let removed = root
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .is_some_and(|m| m.remove(name).is_some());
    if removed {
        std::fs::write(&mcp_path, serde_json::to_string_pretty(&root)?)?;
        ui::warn(&format!(
            "{} not installed — removed its stale MCP entry (omp won't error at startup)",
            name
        ));
    }
    Ok(())
}

/// Best-effort: build/refresh the codebase-memory-mcp knowledge graph for `root`.
pub(crate) fn index_codebase_memory(root: &Path) {
    if which::which("codebase-memory-mcp").is_err() {
        return;
    }
    ui::step("codebase-memory-mcp index (knowledge graph)");
    let arg = serde_json::json!({ "repo_path": root.display().to_string() }).to_string();
    let _ = Command::new("codebase-memory-mcp")
        .args(["cli", "index_repository"])
        .arg(arg)
        .status();
}

/// Ensure `headroom` (context-compression MCP) is installed + registered as an
/// omp MCP server. Headroom compresses long tool outputs / logs / diffs before
/// they reach the model (60–95% fewer tokens) — complements codegraph/cbm.
pub(crate) fn ensure_headroom_mcp(env: &env_detect::Env) -> Result<()> {
    if which::which("headroom").is_err() {
        ui::step("headroom (missing — installing headroom-ai[mcp] via uv)");
        if ensure_uv() {
            let _ = Command::new("uv")
                .args(["tool", "install", "headroom-ai[mcp]"])
                .status();
        }
        // Fallback for boxes with pipx/pip but no uv (e.g. curl bootstrap
        // blocked). POSIX-shell only — skipped on Windows (no `sh`; uv's
        // PowerShell installer above is the path there).
        if which::which("headroom").is_err() && crate::platform::os() != crate::platform::Os::Windows {
            let cmd = "if command -v pipx >/dev/null 2>&1; then pipx install 'headroom-ai[mcp]'; \
elif command -v pip >/dev/null 2>&1; then pip install --user 'headroom-ai[mcp]' \
|| pip install --user --break-system-packages 'headroom-ai[mcp]'; fi";
            let _ = Command::new("sh").arg("-c").arg(cmd).status();
        }
    }
    // Register ONLY when the binary exists — never leave a broken MCP entry that
    // makes omp fail at startup. If still missing, purge any stale entry.
    if which::which("headroom").is_ok() {
        let v = env_detect::cmd_version("headroom", &["--version"]).unwrap_or_default();
        ui::ok(&format!("headroom present ({})", v.trim()));
        register_omp_mcp(&env.home, "headroom", "headroom", &["mcp", "serve"], &[])
    } else {
        ui::warn("headroom unavailable — skipped MCP (install `uv`: https://astral.sh/uv, then re-run `8sync harness`)");
        deregister_omp_mcp(&env.home, "headroom")
    }
}

/// Enable omp's local long-term memory (Mnemopi) in the user's omp settings
/// (`~/.omp/agent/config.yml`) so the agent recalls + retains durable project
/// memory across sessions — "deep awareness that never forgets". API-only by
/// design: `llmMode: smol` reuses the configured online model and
/// `noEmbeddings: true` uses full-text recall, so there are NO local model
/// downloads (runs on any machine). Idempotent + non-clobbering: skips if
/// Mnemopi is already configured or the user authored their own `memory:` block.
/// Ensure omp's anti-forget stack in the user's settings (`~/.omp/agent/config.yml`):
/// (1) Mnemopi long-term memory (API-only — no local model), and (2) compaction
/// tuned to fire at 50% context + when idle (snapcompact strategy stays the omp
/// default), so the agent stops forgetting skills/rules/workflow past ~50%.
/// Idempotent sentinel-block; never clobbers a user-authored `memory:` block.
pub(crate) fn ensure_omp_memory_config(home: &Path) -> Result<()> {
    let cfg = home.join(".omp/agent/config.yml");
    if let Some(p) = cfg.parent() { std::fs::create_dir_all(p)?; }
    // omp rewrites/normalizes config.yml and strips comments, so detect by KEY
    // presence (not sentinel markers) and only append top-level keys when absent.
    let mut s = std::fs::read_to_string(&cfg).unwrap_or_default();
    let mut changed = false;
    let has_mnemopi = s.contains("backend: mnemopi");
    let has_memory_key = s.lines().any(|l| l.starts_with("memory:"));
    if has_mnemopi {
        ui::skip("mnemopi memory", "backend already set");
    } else if has_memory_key {
        ui::warn("config.yml has its own `memory:` — left as-is");
    } else {
        s.push_str("\nmemory:\n  backend: mnemopi\nmnemopi:\n  scoping: per-project-tagged\n  llmMode: smol\n  noEmbeddings: true\n  polyphonicRecall: true\n");
        changed = true;
        ui::ok("mnemopi memory enabled (API-only)");
    }
    if s.lines().any(|l| l.starts_with("compaction:")) {
        ui::skip("compaction@50%", "key already present (user-configured)");
    } else {
        s.push_str("\ncompaction:\n  thresholdPercent: 50\n  idleEnabled: true\n");
        changed = true;
        ui::ok("compaction@50% + idle enabled (anti-forget)");
    }
    if changed { std::fs::write(&cfg, s)?; }
    Ok(())
}

/// Seed the team-default model-role routing into `~/.omp/agent/config.yml` so a
/// fresh machine's omp knows which provider/model to use per role (default/plan/
/// advisor/tiny/commit). Roles point ONLY at the 9router providers that
/// `gateway::seed_default` seeds into `models.yml`, so there are no dangling
/// references. Key-presence idempotent: if the file already has a `modelRoles:`
/// block (user-configured), it is left untouched.
pub(crate) fn ensure_omp_model_roles(home: &Path) -> Result<()> {
    let cfg = home.join(".omp/agent/config.yml");
    if let Some(p) = cfg.parent() { std::fs::create_dir_all(p)?; }
    let mut s = std::fs::read_to_string(&cfg).unwrap_or_default();
    if s.lines().any(|l| l.starts_with("modelRoles:")) {
        ui::skip("modelRoles", "already set (user-configured)");
        return Ok(());
    }
    if !s.is_empty() && !s.ends_with('\n') { s.push('\n'); }
    s.push_str(
        "modelRoles:\n  \
         default: 9router-cc/cc/claude-opus-4-8:medium\n  \
         plan: 9router-cc/cc/claude-opus-4-8:high\n  \
         advisor: 9router-cx/cx/gpt-5.4-mini:high\n  \
         tiny: 9router-cx/cx/gpt-5.4-mini:medium\n  \
         commit: 9router-cx/cx/gpt-5.4-mini:medium\n",
    );
    std::fs::write(&cfg, s)?;
    ui::ok("modelRoles seeded (9router: opus for default/plan, gpt-5.4-mini for advisor/tiny/commit)");
    Ok(())
}

/// Keep the STEP-0 MCP servers' tools ALWAYS VISIBLE via `mcp.discoveryDefaultServers`
/// in `~/.omp/agent/config.yml`. omp's default `tools.discoveryMode: auto` hides ALL
/// MCP tools behind a `search_tool_bm25` discovery hop once the registry exceeds 40
/// tools — measured effect: serena/headroom 0 calls across 29 sessions. Listing the
/// four harness servers keeps their full catalogs in the active tool set (verified in
/// omp 16.4.8: the setting filters discoverable MCP tools by `serverName` and merges
/// them into the session baseline). `tools.essentialOverride` does NOT work for this —
/// omp filters its entries to BUILT-IN tool names only. Key-presence idempotent:
/// never overrides a user-set `discoveryDefaultServers`; migrates away the inert
/// essentialOverride block earlier 8sync builds wrote (exact-match removal only).
pub(crate) fn ensure_mcp_tools_visible(home: &Path) -> Result<()> {
    // omp ≥17 replaced the pre-17 bm25 discovery hop (+ `mcp.discoveryDefaultServers`)
    // with `tools.xdev` (default on): MCP tools mount as `xd://` device URLs, callable
    // via read/write without shipping schemas every request. The old key is obsolete
    // (absent from omp's schema) — writing it is dead weight omp strips on rewrite,
    // which is exactly the churn that made STEP-0 look like it kept "regressing".
    if env_detect::omp_major().is_some_and(|m| m >= 17) {
        ui::ok("STEP-0 MCP tools mounted as xd:// devices (omp ≥17 tools.xdev) — codegraph/serena/cbm/headroom callable, no config key needed");
        return Ok(());
    }
    const SERVERS: &[&str] = &["codegraph", "codebase-memory-mcp", "headroom", "serena"];
    // The exact block written by the earlier essentialOverride approach. MCP names
    // in essentialOverride are filtered out by omp (builtins only) AND clobber the
    // builtin essential defaults — remove it, but ONLY this byte-exact 8sync block.
    const LEGACY_PIN: &str = "tools:\n  essentialOverride:\n    - mcp__codebase_memory_mcp_search_graph\n    - mcp__codebase_memory_mcp_trace_path\n    - mcp__codebase_memory_mcp_get_architecture\n    - mcp__codebase_memory_mcp_get_code_snippet\n    - mcp__serena_find_symbol\n    - mcp__serena_find_referencing_symbols\n    - mcp__serena_get_symbols_overview\n    - mcp__headroom_compress\n";
    const LEGACY_PIN_WITH_ZAI: &str = "tools:\n  essentialOverride:\n    - mcp__codebase_memory_mcp_search_graph\n    - mcp__codebase_memory_mcp_trace_path\n    - mcp__codebase_memory_mcp_get_architecture\n    - mcp__codebase_memory_mcp_get_code_snippet\n    - mcp__serena_find_symbol\n    - mcp__serena_find_referencing_symbols\n    - mcp__serena_get_symbols_overview\n    - mcp__headroom_compress\n    - mcp__zai_vision_extract_text_from_screenshot\n    - mcp__zai_vision_analyze_image\n";
    let cfg = home.join(".omp/agent/config.yml");
    if let Some(p) = cfg.parent() { std::fs::create_dir_all(p)?; }
    let mut s = std::fs::read_to_string(&cfg).unwrap_or_default();
    let mut changed = false;
    if s.contains(LEGACY_PIN_WITH_ZAI) {
        s = s.replace(LEGACY_PIN_WITH_ZAI, "");
        changed = true;
        ui::info("migrated: dropped inert tools.essentialOverride MCP pin (builtins-only setting)");
    } else if s.contains(LEGACY_PIN) {
        s = s.replace(LEGACY_PIN, "");
        changed = true;
        ui::info("migrated: dropped inert tools.essentialOverride MCP pin (builtins-only setting)");
    }
    if s.contains("discoveryDefaultServers") {
        ui::skip("STEP-0 MCP visibility", "mcp.discoveryDefaultServers already set (user-configured)");
        if changed { std::fs::write(&cfg, s)?; }
        return Ok(());
    }
    let list: String = SERVERS.iter().map(|t| format!("    - {t}\n")).collect();
    if s.lines().any(|l| l.starts_with("mcp:")) {
        // Insert under the existing top-level `mcp:` block (same approach as
        // compaction::set_threshold).
        s = s
            .lines()
            .map(|l| {
                if l.starts_with("mcp:") {
                    format!("{l}\n  discoveryDefaultServers:\n{}", list.trim_end())
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !s.ends_with('\n') {
            s.push('\n');
        }
    } else {
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(&format!("\nmcp:\n  discoveryDefaultServers:\n{list}"));
    }
    std::fs::write(&cfg, s)?;
    ui::ok("STEP-0 MCP servers always visible (mcp.discoveryDefaultServers) — codegraph/serena/cbm/headroom callable, no search_tool_bm25 hop");
    Ok(())
}

/// Deploy the anti-forget recall hook to `~/.omp/hooks/pre/8sync-recall.ts`.
/// The hook injects a lean ref bundle (skill index + live STATE) at every
/// `before_agent_start` and into every compaction summary, so the agent keeps
/// the skill/rule/workflow index fresh even past 50% context / compaction.
/// Idempotent: skipped if the deployed file is byte-identical to the asset.
pub(crate) fn ensure_recall_hook(home: &Path) -> Result<()> {
    let dir = home.join(".omp/hooks/pre");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(crate::brand::ns_file("recall.ts"));
    let Some(body) = assets::read("hooks/8sync-recall.ts") else { return Ok(()); };
    if std::fs::read(&target).ok().as_deref() == Some(body.as_bytes()) {
        ui::skip("recall hook", "already deployed");
        return Ok(());
    }
    std::fs::write(&target, body.as_bytes())?;
    ui::ok(&format!("recall hook → {}", target.display()));
    Ok(())
}

/// Deploy the always-apply operating directives to `~/.omp/agent/APPEND_SYSTEM.md`.
/// omp appends this verbatim to EVERY system prompt (never compacts away), so the
/// code-intel-first rule + always-on skill manifest are read on every turn — the
/// fix for "skills/rules are defined but the agent ignores them past ~50% context".
/// Idempotent (byte-identical skip); appended, so omp's base prompt is preserved.
pub(crate) fn ensure_append_system(home: &Path) -> Result<()> {
    let Some(body) = assets::read("configs/omp/APPEND_SYSTEM.md") else {
        return Ok(());
    };
    let body = crate::brand::render(&body).into_owned();
    let target = home.join(".omp/agent/APPEND_SYSTEM.md");
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }
    if std::fs::read_to_string(&target).ok().as_deref() == Some(body.as_str()) {
        ui::skip("APPEND_SYSTEM.md", "already deployed");
        return Ok(());
    }
    std::fs::write(&target, &body)?;
    ui::ok(&format!("always-on directives → {}", target.display()));
    Ok(())
}

/// Deploy the bundled MCP `server.json` standard spec to `~/.omp/specs/` so it's
/// present on the machine by default — the on-disk ground truth every omp session
/// follows when writing/reasoning about `mcp.json`. APPEND_SYSTEM points here.
/// Idempotent (byte-identical skip).
pub(crate) fn ensure_mcp_spec(home: &Path) -> Result<()> {
    let Some(body) = assets::read("specs/mcp-server.md") else {
        return Ok(());
    };
    let body = crate::brand::render(&body).into_owned();
    let target = home.join(".omp/specs/mcp-server.md");
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }
    if std::fs::read_to_string(&target).ok().as_deref() == Some(body.as_str()) {
        ui::skip("mcp-server.md", "spec already deployed");
        return Ok(());
    }
    std::fs::write(&target, &body)?;
    ui::ok(&format!("MCP standard spec → {}", target.display()));
    Ok(())
}

/// Register serena (LSP-based semantic code toolkit) as an omp MCP server, giving
/// the agent symbol-level find + precise edits — token-cheaper than blind file
/// reads/rewrites. Launched via `uvx` (always-latest, no install); bootstraps
/// `uv` if absent. Skipped (and any stale entry purged) only if uv can't install.
pub(crate) fn ensure_serena_mcp(env: &env_detect::Env) -> Result<()> {
    if which::which("uvx").is_err() && which::which("uv").is_err() {
        ensure_uv();
    }
    if which::which("uvx").is_ok() || which::which("uv").is_ok() {
        register_omp_mcp(
            &env.home,
            "serena",
            "uvx",
            &[
                "--from",
                "git+https://github.com/oraios/serena",
                "serena",
                "start-mcp-server",
                "--context",
                "claude-code",
            ],
            &[],
        )
    } else {
        ui::skip("serena MCP", "needs `uv` (https://astral.sh/uv) — install failed, skipped");
        deregister_omp_mcp(&env.home, "serena")
    }
}

/// Remove the legacy Z.AI vision MCP entry + bundled skill directory. ckit no
/// longer registers this MCP by default; image tasks should use built-in image
/// tools or specialist skills.
pub(crate) fn deregister_zai_vision_mcp(home: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(home.join(".omp/skills/zai-vision"));
    deregister_omp_mcp(home, "zai-vision")
}

/// Exact tool catalogs for the MCP servers `8sync harness` auto-registers.
/// Static (spawning each server just to list tools would slow every `harness`
/// run) but kept in sync with the pinned tool sets this harness installs —
/// this is what `ensure_omp_capabilities_snapshot` embeds verbatim so the
/// agent gets EXACT tool names instead of guessing/hallucinating them (the
/// codegraph-verb hallucination bug in KNOWLEDGE.md is exactly what this
/// prevents). Unknown/user-added servers get no catalog — the snapshot says so.
fn known_mcp_tool_catalog(server: &str) -> &'static [(&'static str, &'static str)] {
    match server {
        "codegraph" => &[
            ("codegraph_explore", "primary: relevant source + call paths + blast radius in one call"),
            ("codegraph_search", "search symbols/files in the local .codegraph index"),
            ("codegraph_context", "context bundle for a symbol/query"),
            ("codegraph_callers", "who calls this symbol"),
            ("codegraph_callees", "what this symbol calls"),
            ("codegraph_impact", "blast radius / impacted symbols"),
            ("codegraph_node", "read one indexed node/symbol"),
            ("codegraph_status", "index status and pending sync"),
            ("codegraph_files", "indexed files list"),
        ],
        "codebase-memory-mcp" => &[
            ("search_graph", "BM25 / name-pattern / semantic search over functions, classes, routes"),
            ("query_graph", "raw Cypher against the knowledge graph (complex joins, aggregations)"),
            ("trace_path", "callers/callees, data-flow with args, or cross-service (HTTP/async) trace"),
            ("get_architecture", "packages/services/deps + Leiden community clusters overview"),
            ("get_code_snippet", "read a symbol's source by qualified_name (from search_graph first)"),
            ("get_graph_schema", "node labels + edge types available to query"),
            ("search_code", "grep enriched with graph context, deduped into containing functions"),
            ("detect_changes", "diff-based impact analysis vs a base ref/branch"),
            ("index_repository", "(re)index a repo; `cross-repo-intelligence` mode links routes across repos"),
            ("index_status", "indexing progress/state for a project"),
            ("list_projects", "every project currently indexed"),
            ("delete_project", "drop a project's index"),
            ("manage_adr", "get/update/list-sections of the Architecture Decision Record"),
            ("ingest_traces", "feed runtime traces into the graph to enrich edges"),
        ],
        "headroom" => &[
            ("headroom_compress", "compress >~50-line output BEFORE it enters context (60-95% fewer tokens)"),
            ("headroom_retrieve", "fetch the original uncompressed content back by its hash"),
            ("headroom_stats", "this session's compression stats (tokens/cost saved)"),
        ],
        "serena" => &[
            ("find_symbol", "locate classes/functions/methods by name path (supports include_body)"),
            ("find_referencing_symbols", "who calls/uses a symbol — run before editing an exported symbol"),
            ("find_declaration", "declaration of a symbol via a regex-captured call-site context"),
            ("find_implementations", "implementations of an interface/abstract symbol"),
            ("get_symbols_overview", "structural summary of a file (first call when opening it)"),
            ("replace_symbol_body", "precise symbol-level rewrite (MUST have read include_body=True first)"),
            ("insert_after_symbol", "insert code right after a def/class/method"),
            ("insert_before_symbol", "insert code right before a def/class (e.g. a new import)"),
            ("rename_symbol", "project-wide rename via LSP — use instead of text search/replace"),
            ("rename_file", "move/rename a file AND rewrite every import/reference"),
            ("safe_delete_symbol", "delete only if no references remain, else lists them"),
            ("replace_content", "regex/literal replace within one file (large wildcard ranges OK)"),
            ("replace_in_files", "bulk regex/literal replace across many files (dry_run previews first)"),
            ("get_diagnostics_for_file", "LSP errors/warnings grouped by symbol"),
            ("get_current_config", "active project/tools/contexts/modes"),
            ("activate_project", "switch the active project by name or path"),
            ("list_memories", "serena's own project memory notes (topic-filterable)"),
            ("read_memory", "read one serena memory by name"),
            ("write_memory", "write/update a serena memory"),
            ("edit_memory", "regex-edit a serena memory"),
            ("rename_memory", "rename/move a serena memory"),
            ("delete_memory", "delete a serena memory (only when explicitly asked)"),
            ("onboarding", "first-run project onboarding instructions"),
        ],
        _ => &[],
    }
}

/// Capture a manifest of omp's LIVE capability surface (version + key flags +
/// registered MCP servers + installed skills) to `~/.omp/capabilities.md` so the
/// agent (and `doctor`) know what omp actually offers this session — refreshed
/// every `8sync harness` run. This is the "read omp's README on every update"
/// step: omp is a binary, so we discover its surface from `omp --help` + the
/// config dirs. Surfaces the high-value flags the harness wants maximised:
/// `--advisor`, `--thinking`, `inspect_image`, the `--smol`/`--slow`/`--plan`
/// model roles, and retain/recall (Mnemopi).
pub(crate) fn ensure_omp_capabilities_snapshot(home: &Path) -> Result<()> {
    let omp_ver = env_detect::cmd_version("omp", &["--version"]).unwrap_or_default();
    let help = Command::new("omp")
        .arg("--help")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let has = |flag: &str| help.contains(flag);
    let flags: [(&str, bool); 5] = [
        ("--advisor (passive turn reviewer)", has("--advisor")),
        ("--thinking (reasoning effort)", has("--thinking")),
        ("inspect_image (built-in vision tool)", help.contains("inspect_image")),
        ("--smol / --slow / --plan (adaptive models)", has("--smol")),
        ("--skills (force-load discovery)", has("--skills")),
    ];
    // Parse the "Available Tools" block straight out of `omp --help` — this is
    // omp's OWN base tool set (read/bash/edit/write/grep/glob/lsp/browser/…),
    // distinct from the MCP servers below. Parsed (not hardcoded) so it tracks
    // whatever this installed omp version actually ships.
    let builtin_tools: Vec<(String, String)> = {
        let mut out = Vec::new();
        let mut in_section = false;
        for line in help.lines() {
            if line.trim_start().starts_with("Available Tools") {
                in_section = true;
                continue;
            }
            if !in_section {
                continue;
            }
            if line.trim().is_empty() || !line.starts_with("  ") {
                break;
            }
            if let Some((name, desc)) = line.trim().split_once('-') {
                out.push((name.trim().to_string(), desc.trim().to_string()));
            }
        }
        out
    };
    let mem_on = std::fs::read_to_string(home.join(".omp/agent/config.yml"))
        .unwrap_or_default()
        .contains("backend: mnemopi");
    // Mnemopi's memory tools are added to the agent's tool set dynamically when
    // `memory.backend: mnemopi` is configured — they don't show up in the
    // static `omp --help` (which reflects the tool-less default), so they're
    // pinned here instead, gated on `mem_on`.
    let memory_tools: &[(&str, &str)] = &[
        ("recall", "search long-term memory for specific facts/entries (ranked, raw)"),
        ("reflect", "synthesize an answer across many memories (open-ended questions)"),
        ("retain", "store durable facts (decisions, prefs, project context) for future sessions"),
        ("memory_edit", "update/forget/invalidate a specific stored memory by id (from recall)"),
    ];
    let mcp_names: Vec<String> = std::fs::read_to_string(home.join(".omp/agent/mcp.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("mcpServers")
                .and_then(|m| m.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()))
        })
        .unwrap_or_default();
    let mut mcp_names_sorted = mcp_names.clone();
    mcp_names_sorted.sort();
    let skill_count = std::fs::read_dir(home.join(".omp/skills"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let mut out = String::new();
    out.push_str("# omp capabilities snapshot\n\n");
    out.push_str(&format!(
        "Captured by `8sync harness`. omp version: **{}**\n\n",
        omp_ver.trim()
    ));
    out.push_str(
        "Refreshed every `8sync harness` run (omp self-updates via `omp update`). \
         This file is the GROUND TRUTH for exact tool names/params — call these, \
         never guess or invent a tool name.\n\n",
    );
    out.push_str("## Maximise these features\n\n");
    for (label, on) in flags.iter() {
        out.push_str(&format!(
            "- [{}] {} — {}\n",
            if *on { 'x' } else { ' ' },
            label,
            if *on { "available" } else { "not detected" }
        ));
    }
    out.push_str(&format!(
        "- [{}] retain/recall/reflect (Mnemopi long-term memory) — {}\n",
        if mem_on { 'x' } else { ' ' },
        if mem_on { "ON" } else { "OFF" }
    ));
    out.push_str(
        "\n## Modality routing (token discipline)\n\n\
         Read STRUCTURE as an image, PRECISE things as text. Vision models (Opus-class): \
         render a codegraph / diagram / dashboard / big PDF with `8sync shot`/`pdf-img` and \
         read the image (modality-fit — structure beats its adjacency-list text). NEVER \
         image-ify source code / exact config / line-numbered data — text is cheaper AND \
         lossless (Claude bills images per 28x28 patch, pay-per-pixel; the 10x/90% figure \
         needs a dedicated OCR encoder, not a screenshot). Text-only models read images \
         through model-native vision when available or built-in image/inspect tools. Full table: \
         `~/.omp/skills/image-routing/SKILL.md`.\n",
    );
    out.push_str("\n## omp built-in tools (from `omp --help`)\n\n");
    if builtin_tools.is_empty() {
        out.push_str("_(could not parse — run `omp --help` manually)_\n");
    } else {
        for (name, desc) in &builtin_tools {
            out.push_str(&format!("- `{}` — {}\n", name, desc));
        }
    }
    if mem_on {
        out.push_str("\n## Memory tools (Mnemopi — ON)\n\n");
        out.push_str("`recall`/`reflect` BEFORE answering about past sessions/decisions/prefs; `retain` durable facts AFTER. Never re-derive what's already retained.\n\n");
        for (name, desc) in memory_tools {
            out.push_str(&format!("- `{}` — {}\n", name, desc));
        }
    }
    out.push_str("\n## Registered MCP servers — EXACT tool catalog\n\n");
    out.push_str(&format!(
        "`{}` server(s) in `~/.omp/agent/mcp.json`. Use these BEFORE raw grep/read (STEP 0). Callable names are the REGISTERED forms: `mcp__<server-with-underscores>_<tool>` (e.g. `mcp__codebase_memory_mcp_search_graph`, `mcp__serena_find_symbol`; exception: `mcp__headroom_compress` — omp collapses a duplicated server prefix). The four harness servers are kept ALWAYS VISIBLE by `8sync harness` (`mcp.discoveryDefaultServers`) — call their tools directly; only other/newly-added servers' tools need one `search_tool_bm25` call first.\n\n",
        mcp_names_sorted.len()
    ));
    for name in &mcp_names_sorted {
        let tools = known_mcp_tool_catalog(name);
        out.push_str(&format!("### {}\n\n", name));
        if tools.is_empty() {
            out.push_str("_(not a pinned harness server — no static catalog; check its own docs/`--help`)_\n\n");
        } else {
            for (tool, desc) in tools {
                out.push_str(&format!("- `{}` — {}\n", tool, desc));
            }
            out.push('\n');
        }
    }
    // Local GGUF models (mistral.rs → omp providers), if any are registered.
    let reg_raw =
        std::fs::read_to_string(home.join(".config/8sync/local-models.tsv")).unwrap_or_default();
    let locals: Vec<&str> = reg_raw.lines().filter(|l| !l.trim().is_empty()).collect();
    if !locals.is_empty() {
        out.push_str("\n## Local GGUF models (mistral.rs → omp)\n\n");
        out.push_str("On-device GGUF models served by mistral.rs (Rust, memory-safe) and registered as omp providers. Use like any model: `8sync ai --model local/<name>`. Manage: `8sync harness add-local-model list|rm`.\n\n");
        for l in &locals {
            let mut it = l.splitn(3, '\t');
            let name = it.next().unwrap_or("").trim();
            let port = it.next().unwrap_or("").trim();
            if !name.is_empty() {
                out.push_str(&format!("- `local/{}` — mistral.rs on port {}\n", name, port));
            }
        }
    }
    out.push_str(&format!(
        "## Installed skills\n\n`{}` skill dir(s) in `~/.omp/skills/`.\n",
        skill_count
    ));
    let mcp_servers = mcp_names_sorted.len();
    let target = home.join(".omp/capabilities.md");
    let changed = std::fs::read_to_string(&target).ok().as_deref() != Some(out.as_str());
    std::fs::write(&target, out)?;
    if changed {
        ui::ok(&format!(
            "omp capabilities snapshot → {} ({} · {} MCP · {} skills)",
            target.display(),
            omp_ver.trim(),
            mcp_servers,
            skill_count
        ));
    } else {
        ui::skip("omp capabilities snapshot", "unchanged");
    }
    Ok(())
}
/// Best-effort: ensure the `feynman` research CLI (companion-inc/feynman) is
/// available so the 20 feynman research skills registered in agents/skills.toml
/// (deep-research, alpha-research, literature-review, …) are functional rather
/// than inert — they shell out to `feynman`/`alpha`. A failed install is
/// non-fatal (skills still list; the user can `npx @companion-ai/feynman`
/// later). Never bails the harness run.
pub(crate) fn ensure_feynman_cli() {
    if which::which("feynman").is_ok() {
        let v = env_detect::cmd_version("feynman", &["--version"]).unwrap_or_default();
        ui::skip("feynman CLI", &format!("present ({})", v));
        return;
    }
    ui::step("feynman CLI (missing — installing @companion-ai/feynman)");
    // Global install so skills resolve `feynman` directly on PATH. `npx` remains
    // the zero-install fallback, so a non-zero exit is only a soft failure.
    let cmd = "npm install -g @companion-ai/feynman 2>/dev/null || true";
    match Command::new("sh").arg("-c").arg(cmd).status() {
        Ok(s) if s.success() && which::which("feynman").is_ok() => {
            ui::ok("feynman CLI installed (research skills functional)");
        }
        _ => ui::warn(
            "feynman global install skipped/failed — skills still list (run via `npx @companion-ai/feynman`)",
        ),
    }
}

/// Deploy the `8sync-workflow` omp extension — a gsd-pi-grade surface that
/// registers model-callable workflow tools (wf_state_get/set, persisted across
/// compaction via a custom session entry) + a `/wf` status command + a
/// session_start state-restore handler. Lives in omp's config dir
/// (`~/.omp/agent/extensions/` global + `<root>/.omp/extensions/` project) so it
/// NEVER patches omp core → omp updates stay safe. The Workflow viz page
/// (`8sync harness web`) appends exported-workflow `registerTool` blocks to the
/// project copy. Idempotent (byte-identical skip), mirrors `ensure_gs_command`.
pub(crate) fn ensure_workflow_extension(home: &Path, root: Option<&Path>) -> Result<()> {
    let Some(body) = assets::read("extensions/8sync-workflow.ts") else {
        return Ok(());
    };
    let global = home.join(".omp/agent/extensions").join(crate::brand::ns_file("workflow.ts"));
    if let Some(p) = global.parent() {
        std::fs::create_dir_all(p)?;
    }
    let changed = std::fs::read_to_string(&global).map(|s| s != body).unwrap_or(true);
    std::fs::write(&global, &body)?;
    if changed {
        ui::ok(&format!("8sync-workflow extension → {}", global.display()));
    }
    if let Some(r) = root {
        let proj = r.join(".omp/extensions").join(crate::brand::ns_file("workflow.ts"));
        if let Some(p) = proj.parent() {
            std::fs::create_dir_all(p)?;
        }
        let changed = std::fs::read_to_string(&proj).map(|s| s != body).unwrap_or(true);
        std::fs::write(&proj, &body)?;
        if changed {
            ui::ok(&format!("8sync-workflow extension → {}", proj.display()));
        }
    }
    Ok(())
}

/// Deploy an omp artifact (command/extension) to the global config dir and, when
/// inside a project, the project config dir too. Byte-identical writes are quiet.
fn deploy_omp_pair(
    home: &Path,
    root: Option<&Path>,
    asset: &str,
    global_rel: &str,
    proj_rel: &str,
    label: &str,
) -> Result<()> {
    let Some(body) = assets::read(asset) else {
        return Ok(());
    };
    let body = if asset.ends_with(".md") { crate::brand::render(&body).into_owned() } else { body };
    let global = home.join(global_rel);
    if let Some(p) = global.parent() {
        std::fs::create_dir_all(p)?;
    }
    let changed = std::fs::read_to_string(&global).map(|s| s != body).unwrap_or(true);
    std::fs::write(&global, &body)?;
    if changed {
        ui::ok(&format!("{} → {}", label, global.display()));
    }
    if let Some(r) = root {
        let proj = r.join(proj_rel);
        if let Some(p) = proj.parent() {
            std::fs::create_dir_all(p)?;
        }
        let changed = std::fs::read_to_string(&proj).map(|s| s != body).unwrap_or(true);
        std::fs::write(&proj, &body)?;
        if changed {
            ui::ok(&format!("{} → {}", label, proj.display()));
        }
    }
    Ok(())
}

/// Deploy the gsd-pi-style automation engine — the `8sync-engine` omp extension
/// (durable slice/task state machine + code-enforced verify-retry gate + git
/// worktree tools) and its `/auto` orchestration command. 100% on omp core (config
/// dirs only, never patches omp) so updates stay safe. Mirrors the workflow ext.
pub(crate) fn ensure_engine(home: &Path, root: Option<&Path>) -> Result<()> {
    let eng = crate::brand::ns_file("engine.ts");
    deploy_omp_pair(
        home,
        root,
        "extensions/8sync-engine.ts",
        &format!(".omp/agent/extensions/{eng}"),
        &format!(".omp/extensions/{eng}"),
        "8sync-engine extension",
    )?;
    deploy_omp_pair(
        home,
        root,
        "commands/auto.md",
        ".omp/agent/commands/auto.md",
        ".omp/commands/auto.md",
        "/auto command",
    )?;
    deploy_omp_pair(
        home,
        root,
        "commands/feature.md",
        ".omp/agent/commands/feature.md",
        ".omp/commands/feature.md",
        "/feature command",
    )?;
    deploy_omp_pair(
        home,
        root,
        "commands/push-now.md",
        ".omp/agent/commands/push-now.md",
        ".omp/commands/push-now.md",
        "/push-now command",
    )?;
    deploy_omp_pair(
        home,
        root,
        "commands/pull-now.md",
        ".omp/agent/commands/pull-now.md",
        ".omp/commands/pull-now.md",
        "/pull-now command",
    )
}

/// One-time rebrand migration: when the binary is rebranded (`brand::NS` differs
/// from the historical `8sync`), move the `8sync`-namespaced persistent config to
/// the new namespace and remove stale deployed artifacts left under the old
/// `8sync-` filenames (the new ones deploy under `<NS>-`, so a leftover
/// `8sync-engine.ts` would make omp load the engine tools twice). AGENTS.md
/// sentinels self-heal via `skill::inject`'s legacy-aware block finder, and the
/// `~/.cache/8sync` cache dir is renamed to `~/.cache/ckit` (see `brand.rs`). No-op on
/// the default build and idempotent once migrated. Best-effort: never bails.
pub(crate) fn migrate_namespace(home: &Path) {
    if crate::brand::NS == "8sync" {
        return;
    }
    // 1. Config namespace: ~/.config/8sync → ~/.config/<NS>, kitty conf filename.
    //    Base is `~/.config` (`brand::config_dir`), NOT `dirs::config_dir()` —
    //    on macOS the latter is `~/Library/Application Support`, where an earlier
    //    build wrongly wrote config; step 1b recovers it into the XDG dir.
    {
        let cfg = crate::brand::config_dir(home).parent().map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".config"));
        // 1a. macOS recovery: pull any config an older build left under
        // `~/Library/Application Support/{8sync,<NS>}` into `~/.config/<NS>`.
        // File-level merge (not dir-rename): the target dir may already exist
        // partially (e.g. a stray models.toml), which would make a dir-rename
        // silently skip and strand global/skills.toml.
        let dst = cfg.join(crate::brand::NS);
        if let Some(mac) = dirs::config_dir().filter(|d| d != &cfg) {
            merge_dir_if_new_absent(&mac.join("8sync"), &dst);
            merge_dir_if_new_absent(&mac.join(crate::brand::NS), &dst);
        }
        merge_dir_if_new_absent(&cfg.join("8sync"), &dst);
        rename_if_new_absent(
            &cfg.join("kitty").join("8sync.conf"),
            &cfg.join("kitty").join(format!("{}.conf", crate::brand::NS)),
        );
        // 3. Old systemd user timer (the NS-named unit installs on next `up --timer`).
        let unit_dir = cfg.join("systemd/user");
        if unit_dir.join("8sync-harness-up.timer").exists() {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "8sync-harness-up.timer"])
                .status();
            let _ = std::fs::remove_file(unit_dir.join("8sync-harness-up.service"));
            let _ = std::fs::remove_file(unit_dir.join("8sync-harness-up.timer"));
        }
    }
    // Cache namespace: ~/.cache/8sync → ~/.cache/ckit (whole-dir rename — the
    // cache holds subdirs like models/ and locate-anything/ with expensive GGUF
    // downloads, so the file-level merge helper won't move them). Guarded to run
    // only when the new dir is absent, so a fresh cache is never clobbered.
    rename_if_new_absent(&home.join(".cache/8sync"), &home.join(".cache/ckit"));
    // 2. Stale global deployed artifacts under the old `8sync-` names.
    for stale in [
        home.join(".omp/hooks/pre/8sync-recall.ts"),
        home.join(".omp/agent/extensions/8sync-engine.ts"),
        home.join(".omp/agent/extensions/8sync-workflow.ts"),
    ] {
        let _ = std::fs::remove_file(&stale);
    }
}

/// `rename(old → new)` only when the old path exists and the new one does not —
/// so a rebrand migrates once and never clobbers freshly-written state.
fn rename_if_new_absent(old: &Path, new: &Path) {
    if old.exists() && !new.exists() {
        let _ = std::fs::rename(old, new);
    }
}

/// Migrate every top-level file from `old` dir into `new`, moving only entries
/// the destination lacks — so a partially-populated `new` (e.g. a stray
/// models.toml) never blocks recovery of the rest, and never clobbers config
/// the current build already wrote. Skips `*.bak`. Best-effort; leaves `old`.
fn merge_dir_if_new_absent(old: &Path, new: &Path) {
    let Ok(entries) = std::fs::read_dir(old) else { return };
    let _ = std::fs::create_dir_all(new);
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".bak") {
            continue;
        }
        let dest = new.join(&name);
        if !dest.exists() {
            let _ = std::fs::rename(entry.path(), &dest);
        }
    }
}
