//! `8sync harness init` — one command to stand up a maximal agent harness:
//! deploy every bundled skill (+ codegraph binary + external skill packs),
//! pull registered skills (agents/skills.toml), init codegraph, seed agent
//! memory + CHANGELOG, and inject force-load rules into the root
//! AGENTS.md/CLAUDE.md plus a compact index into every significant
//! sub-folder. Progress is tracked step by step.
use std::time::Instant;

use anyhow::Result;

use super::external::install_external_skill_packs;
use super::memory::{seed_gitleaks_hook, seed_harness_memory};
use crate::verbs::skill::pack::{discover_packs, install_pack, is_pack_installed};
use crate::verbs::skill::{deploy, discover, inject_agents_md, inject_subfolder_indexes, update};
use crate::{env_detect, ui};

/// Lightweight stepped progress indicator (no TUI dep): `▸ [i/N] label`.
struct Progress {
    total: usize,
    cur: usize,
    start: Instant,
}

impl Progress {
    fn new(total: usize) -> Self {
        Progress { total, cur: 0, start: Instant::now() }
    }
    fn step(&mut self, label: &str) {
        self.cur += 1;
        ui::step(&format!("[{}/{}] {}", self.cur, self.total, label));
    }
    fn done(&self) {
        ui::ok(&format!(
            "harness ready in {:.1}s ({} steps)",
            self.start.elapsed().as_secs_f32(),
            self.total
        ));
    }
}

pub(crate) fn harness_init(env: &env_detect::Env, _force: bool) -> Result<()> {
    ui::header("8sync harness init");
    deploy::migrate_namespace(&env.home);
    let in_project = discover::detect_current_project_root().is_some();
    let total = if in_project { 8 } else { 3 };
    let mut p = Progress::new(total);

    // 2. Deploy bundled skills (embedded assets → ~/.omp/skills).
    p.step("deploy bundled skills (codegraph · karpathy · ponytail · assp · impeccable · taste · …)");
    deploy::install_bundled_global(env)?;

    // 3. codegraph binary (auto curl installer if missing).
    p.step("ensure codegraph binary");
    deploy::ensure_codegraph(env)?;
    deploy::ensure_codegraph_mcp(env)?;
    deploy::ensure_codebase_memory_mcp(env)?;
    deploy::ensure_headroom_mcp(env)?;
    let _ = deploy::ensure_omp_memory_config(&env.home);
    let _ = deploy::ensure_recall_hook(&env.home);
    let _ = deploy::ensure_append_system(&env.home);
    let _ = deploy::ensure_mcp_spec(&env.home);
    let _ = deploy::ensure_serena_mcp(env);
    let _ = deploy::deregister_zai_vision_mcp(&env.home);
    let _ = deploy::ensure_omp_capabilities_snapshot(&env.home);
    deploy::ensure_feynman_cli();
    let _ = deploy::ensure_workflow_extension(&env.home, None);
    let _ = deploy::ensure_engine(&env.home, None);
    let _ = deploy::cleanup_legacy_gs(&env.home, None);

    // 4. External skill packs (ponytail full + addyosmani) — best-effort/network.
    p.step("download external skill packs (ponytail · addyosmani)");
    let _ = install_external_skill_packs(env);

    // Normalise every global skill dir to the 3-folder layout.
    let global_dir = env.home.join(".omp/skills");
    for d in discover::list_installed_skill_dirs(&global_dir).unwrap_or_default() {
        deploy::ensure_skill_layout(&d);
    }

    // 5-9. Project-scoped scaffolding.
    if let Some(root) = discover::detect_current_project_root() {
        // Pull every skill registered in agents/skills.toml from its source
        // (git collections like feynman, builtin:, path:) — mirrors bare
        // `8sync harness` so init is a true superset, not a smaller bootstrap.
        // No global→project skill copy: skills live in ~/.omp/skills/ (global)
        // or <root>/.omp/skills/ (project-local, only if explicitly added).
        p.step("pull registered skills (agents/skills.toml: feynman, …)");
        let _ = update::update_skills(env, &crate::brand::config_dir(&env.home).join("skills.toml"), None);
        let local_dir = root.join(".omp/skills");
        for d in discover::list_installed_skill_dirs(&local_dir).unwrap_or_default() {
            deploy::ensure_skill_layout(&d);
        }

        // Domain skill+rule packs (e.g. vtiger-php): y/N per pack not yet
        // installed in THIS project — mirrors the `8sync setup` personal-profile
        // prompt, but project-scoped (a pack writes into <root>/.omp/, so the
        // question only makes sense once a project root is known).
        p.step("domain packs (y/N each, not yet installed)");
        if env_detect::has_tty() {
            for pk in discover_packs() {
                if is_pack_installed(&root, &pk.name) {
                    continue;
                }
                let desc = if pk.description.is_empty() { pk.name.as_str() } else { pk.description.as_str() };
                let q = format!("Install pack `{}` — {}", pk.name, desc);
                if ui::prompt_yes_no(&q, false) {
                    if let Err(e) = install_pack(&root, &pk.name, false) {
                        ui::err(&format!("pack {} failed: {}", pk.name, e));
                    } else {
                        let manifest = root.join("agents/skills.toml");
                        let mut reg = discover::read_registry(&manifest);
                        reg.insert(pk.name.clone(), discover::SkillEntry {
                            src: format!("pack:{}", pk.name),
                            when: Some("on-demand".to_string()),
                            rev: None,
                        });
                        let _ = discover::write_registry(&manifest, &reg);
                    }
                }
            }
        }

        p.step("codegraph init + seed memory/CHANGELOG");
        deploy::ensure_codegraph_init(&root);
        seed_harness_memory(&root)?;
        seed_gitleaks_hook(&root);

        p.step("inject force-load → AGENTS.md / CLAUDE.md");
        inject_agents_md(&env.home, &root)?;

        p.step("inject sub-folder skill indexes");
        let n = inject_subfolder_indexes(&root)?;
        if n > 0 {
            ui::ok(&format!("dropped skill-index AGENTS.md into {} sub-folder(s)", n));
        }
        let _ = deploy::ensure_workflow_extension(&env.home, Some(&root));
        let _ = deploy::ensure_engine(&env.home, Some(&root));
        let _ = deploy::cleanup_legacy_gs(&env.home, Some(&root));
        p.done();
        ui::info("start a session: `8sync .` or `omp --continue`");
        ui::info("refresh later: `8sync harness up`  (auto: `8sync harness up --timer 30m`)");
        ui::info("opt-in skill: `8sync skill add builtin:social-growth` (social/branding/leads — không auto-bật)");
    } else {
        p.done();
        ui::warn("not inside a project (no AGENTS.md/.git/Cargo.toml/package.json/... in cwd or ancestors)");
        ui::info("  → `cd` into a project root, then re-run `8sync harness init`");
        let _ = deploy::ensure_workflow_extension(&env.home, None);
        let _ = deploy::ensure_engine(&env.home, None);
    }
    Ok(())
}

