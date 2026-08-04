<!-- 8sync:harness:begin -->
## 🧠 8sync harness

- **Always-on (đọc theo thứ tự; CORE đọc body ngay, SPECIALIST đọc khi task khớp):** codegraph → karpathy → ponytail → assp → impeccable → taste → 8sync-cli → image-routing.
- **Cách tận dụng:** codegraph = explore code (query/callers/callees, không grep) · karpathy + ponytail = YAGNI, làm ít nhất, xoá > thêm · impeccable = design CHUẨN, BẮT BUỘC khi UI/design (đọc body lúc đó) + taste chống slop.
- **Output lớn (>~300 dòng) → BẮT BUỘC `headroom_compress`** trước khi vào context.
- **Sau mỗi thay đổi:** cập nhật `CHANGELOG.md` (Unreleased) + ghi học được vào file này (prefix `validated:` nếu test/build xác nhận, `hypothesis:` nếu chưa).
<!-- 8sync:harness:end -->

# KNOWLEDGE (8sync managed — append-only)

## Learnings (append-only — ghi DƯỚI đây; KHÔNG sửa block `8sync:harness` ở trên)

Mỗi entry prefix `validated:` (test/build xác nhận) · `hypothesis:` (chưa) · `failure:` (lỗi đã gặp + cách sửa; đọc đầu phiên để khỏi lặp).

_empty_

## 2026-07-28 — ckit config dir fix (rebrand incomplete)
- **Bug:** config không vào `~/.config/ckit/` sau rebrand 8sync→ckit. Hai gốc rễ cộng dồn:
  1. `env_detect.rs` dùng `dirs::config_dir()` → macOS trả `~/Library/Application Support`, không phải `~/.config`. Đã ép `xdg_config = XDG_CONFIG_HOME | ~/.config` mọi OS.
  2. Nhiều call-site hard-code literal `"8sync/..."` (setup/theme/bg/doctor/skill/harness) thay vì `brand::NS` → ghi/đọc nhầm namespace.
- **Fix:** single source = `brand::config_dir(&home)` (`~/.config/<NS>`) cho MỌI reader+writer config runtime. `ns_file()` cho kitty artifact filenames.
- **Migration:** `migrate_namespace` nâng thành `merge_dir_if_new_absent` (move file-level, không dir-rename) + macOS recovery kéo config từ Application Support về `~/.config/ckit`. Dir-rename cũ bị skip khi đích tồn tại một phần (stray models.toml) → strand global/skills.toml.
- **Còn lại (không đụng):** bin name Cargo vẫn `8sync`; `.cache/8sync` + `8sync-*.ts` artifact cố ý literal (xem brand.rs).
- validated: `8sync doctor` xanh, config ở `~/.config/ckit/{global,skills,models}.toml`.

## 2026-07-28 — workspace dir su-code → agents (runtime)
- **Y/c:** `ckit` chạy trong project phải dùng thư mục `agents/` thay `su-code/`.
- **Chẩn:** code seed (memory.rs, here.rs, note.rs) + gitignore-managed ĐÃ dùng `agents/`. Chỗ lệch còn lại: `skill/inject.rs` — template prose ghi vào AGENTS.md của project vẫn dạy AI đọc/ghi `su-code/{STATE,KNOWLEDGE,PLAYBOOKS}.md` + skills-vendored `su-code/skills`. Đó là nguồn khiến runtime lệch.
- **Fix:** đổi `su-code/skills`→`agents/skills` + 3 dòng Quy-tắc-bất-biến (`agents/{KNOWLEDGE,STATE,PLAYBOOKS}.md`) trong inject.rs.
- **GIỮ (identity, cố ý):** `selfup.rs REPO_NAME="su-code"`, clone/install URL, `Cargo.toml repository` + package `name="su-code"` (log build), `deploy.rs` legacy-cleanup path. Đổi = phá `ckit up`/install.
- validated: seed project tạm → chỉ tạo `agents/`, AGENTS.md trỏ 7/7 `agents/*.md`, 0 workspace-path `su-code/`.
- **Data còn tồn:** folder `su-code/` cũ trên disk (KNOWLEDGE 23KB, archive 29 files, plans/workflows/skills.toml) CHƯA merge hết sang `agents/` — chưa xoá, chờ user quyết.

## validated: ckit setup seed omp models.yml + config.yml (2026-07-31)
- omp binary CHỈ đọc `~/.omp/agent/models.yml` của nó — KHÔNG fallback sang `~/.config/ckit/models.toml`. models.toml chỉ là bảng routing tên→role; catalog provider thật (baseUrl/apiKey/model) phải nằm trong omp models.yml.
- Seed pattern: `gateway::seed_default(path)` ghi template placeholder CHỈ khi file absent (khác `apply` vốn bail nếu thiếu key + backup/overwrite). `deploy::ensure_omp_model_roles` key-presence idempotent (skip nếu có `modelRoles:`).
- Wire tại `setup.rs` Stage-A else-block: step `omp-models` + `omp-config`, sau `install_configs`.
- An ninh: template PHẢI dùng `__NINE_ROUTER_KEY__`, tuyệt đối không bake key thật. Audit known key fragments must be empty before public push.
- Rust `\`-line-continuation trong string literal nuốt whitespace đầu dòng tiếp → YAML indent sạch (verified bằng rustc mini-prog).

## validated: GitHub private release + IP placeholder (2026-07-31, thay GitLab)
- Chọn GitHub repo dante241/ckit_agent (khỏi nuôi self-host runner; public sau khi sanitize internal refs).
- Private repo GitHub: tải asset KHÔNG dùng browser_download_url (redirect S3, token vô hiệu). PHẢI: API `releases/assets/{id}` + header `Authorization: Bearer <token>` + `Accept: application/octet-stream`. Resolve id từ releases/latest hoặc releases/tags/<tag>.
- Token: env CKIT_GITHUB_TOKEN | GITHUB_TOKEN (scope repo). selfup.rs + install.sh/ps1 đều cần.
- CI: GitHub Actions release.yml (đã có sẵn, GitHub tự cấp runner 3 OS) — KHÔNG hard-code repo (dùng context). Xóa .gitlab-ci.yml.
- Infra URL KHÔNG bake vào binary: gateway-models.yml baseUrl=`__NINE_ROUTER_URL__`; gateway::apply() thay từ $NINE_ROUTER_URL (bắt buộc, bail nếu thiếu) hoặc URL đã deploy (preserve). Thêm lệnh `gateway url <URL>` mirror `gateway key`. Verify: strings binary không chứa internal URL.
- failure-avoided: đừng để default URL trong binary — ngay cả private repo, binary tải máy dev vẫn strings ra IP. Placeholder bắt buộc env là an toàn nhất.

## update: repo PUBLIC → bỏ token bắt buộc (2026-07-31)
- Repo dante241/ckit_agent chuyển PUBLIC. Public GitHub release: tải qua `browser_download_url` KHÔNG cần auth (khác private phải dùng releases/assets/{id}+octet-stream). Bỏ yêu cầu token ở selfup.rs + install.sh/ps1.
- Token giờ OPTIONAL: chỉ thêm header nếu có env, để né rate-limit API anon 60 req/h. Helper api_curl_args(url, &Option<token>) thêm -H auth khi Some.
- Audit public repo assets/: gateway/9router sạch; sample VCS URLs in vtiger-php pack were placeholder-sanitized before public push; crm.domain.com/IP examples are placeholders/examples.

## validated: setup Stage A/B phải gate bằng is_cachyos_or_arch, KHÔNG phải os()==Linux (2026-07-31)
- **Bug thực tế trên Ubuntu VM:** `ckit setup` fail `sudo: pacman: command not found` vì bước `paru` (AUR helper) guard bằng `platform::os() == Os::Linux` → chạy cho MỌI distro Linux, nhưng `install_aur_helper` gọi `pkg::pacman_install_safe(["git","base-devel"])` chỉ có trên Arch.
- **Fix:** đổi guard bước `paru` sang `env.is_cachyos_or_arch()` (setup.rs ~143). Stage B (profiles) cũng sai cùng kiểu: `os() != Linux` → Ubuntu không skip; đổi sang `!env.is_cachyos_or_arch()` (setup.rs ~176). `is_cachyos_or_arch()` = os_id ∈ {cachyos, arch, manjaro, endeavouros} (env_detect.rs:35).
- `install_core_pkg` (gh) đã đúng: `pkg_manager()` trả None trên Ubuntu → chỉ warn "install gh manually", không fail.
- validated: `cargo build -q` exit 0. Ubuntu setup không còn đụng pacman; Stage A core (omp/codegraph/MCP/skills/config) chạy tiếp bình thường.

## validated: STEP-0 MCP default + remove zai-vision (2026-07-31)
- `setup.rs` Stage A now registers all STEP-0 MCPs (`codegraph`, `codebase-memory-mcp`, `headroom`, `serena`) in one `step0-mcps` try_step (trước chỉ có `codegraph`), rồi `deregister_zai_vision_mcp` để dọn máy cũ. `harness init`/`global` đã có sẵn chuỗi này.
- `zai-vision` removal: xoá bundled skill asset `assets/skills/zai-vision/`, gỡ khỏi force-load/image-routing/locate/APPEND_SYSTEM/root docs/doctor text; image understanding route qua model-native hoặc built-in image/inspect tools. `deregister_zai_vision_mcp` giờ cũng `remove_dir_all(~/.omp/skills/zai-vision)`.
- `ensure_mcp_tools_visible`: giữ 2 hằng số legacy exact-match — `LEGACY_PIN` (không zai) và `LEGACY_PIN_WITH_ZAI` — để migrate sạch cả 2 đời config user. Đây là match duy nhất còn lại của "zai" trong scan, đúng chủ đích cleanup.
- validated: `cargo check -q` + `cargo build -q` exit 0; stale-ref scan chỉ còn cleanup literal.

## validated: ckit 0.1.3 release rebrand cleanup (2026-08-04)
- Release tags are immutable identities: do NOT move `v0.1.2` after publishing. For public update after `0.1.2`, bump workspace `version` + `Cargo.lock` package stanza to `0.1.3`, document it in `CHANGELOG.md`, commit, then create new tag `v0.1.3`.
- Public rebrand scan must include shipped install paths, not only README/docs/runtime help. `scripts/alexdev-install.sh` is user-facing and must use `ckit`, `~/.config/ckit/profiles`, `dante241/ckit_agent` installer URL, `ckit setup --profile alexdev`, and `ckit doctor`.
- Keep `assets/skills/8sync-cli/` as the stable skill ID unless also updating asset dir/frontmatter, `deploy.rs` bundled mapping, and `inject.rs` rank/core matching together. For this release, only render prose/commands; do not introduce `ckit-cli`.
- validated: `cargo check -q`, `cargo build -q`, `bash -n scripts/alexdev-install.sh`, `cargo run -q -- --version` (`ckit 0.1.3`), and raw shipped-file stale scan all exit 0.

## validated: ckit self-update must verify temp assets before rename (2026-08-04)
- Failure: `ckit up` on Darwin arm64 installed `ckit-v0.1.3-linux-x86_64` because `selfup.rs` hard-coded `ASSET_SUFFIX = "-linux-x86_64"`; shell then returned `zsh: exec format error: ckit`.
- Fix pattern: choose release asset by `std::env::consts::{OS, ARCH}` using the same suffixes as `install.sh`/release workflow: Linux `x86_64`/`aarch64`, Darwin `x86_64`/`arm64`, Windows `x86_64.exe`.
- Guard pattern: after curl download, set Unix mode `0755`, run the temp file with `--version`, require exact `ckit <expected_version>`, and only then `rename` over `~/.local/bin/ckit`. This protects against wrong-OS, non-executable, and wrong-version assets.
- Recovery pattern: if `~/.local/bin/ckit` is bricked but repo build exists, run `install -m755 target/release/ckit ~/.local/bin/ckit` first, then rebuild/install the hotfix.
- validated: `cargo test -q selfup`, `cargo check -q`, `cargo build --release -q`, local install to `~/.local/bin/ckit`, `ckit --version` (`ckit 0.1.4`), `file` (`Mach-O 64-bit executable arm64`), and generated help scans for top-level/root/up/theme/bg all exit clean with zero legacy `8sync`/`sync8` hits.

## validated: docs should separate setup, daily loop, and feature workflow (2026-08-04)
- Public docs need a practical usage section before the command reference: initial machine/project setup (`install.sh`, `ckit setup`, `ckit doctor`, `ckit harness`, `ckit .`), daily commands (`ckit .`, `ckit ai`, `ckit find`, `ckit run`, `ckit note`, `ckit ship`), and long-task feature flow.
- Feature docs entrypoint for long work: start in omp with `/plan @assets/skills/feature/SKILL.md <task>`, then use `/feature new <slug>`, `/feature plan`, `/feature go`, `/feature ship`, and `/feature status`. Keep `ckit feature list|switch` as deterministic helpers outside omp.
- validated: static docs check used Python `HTMLParser` plus required-content asserts; stale user-facing scan for `sync8|Sync8|8-Sync-Dev/su-code` passed; browser DOM check on `file://.../docs/index.html` found `How to use ckit`, setup/daily sections, `/plan @assets/skills/feature/SKILL.md`, `/feature new customer-portal`, and the new `Usage` nav link.

## validated: ckit setup must not shell to POSIX `sh` on native Windows (2026-08-04)
- Failure: default `ckit setup` (strict mode) aborted on Windows because `install_omp`/`install_codegraph` ran `Command::new("sh")` (curl|sh) — `sh` is absent, `run_loud` errors, `try_step` (yolo=false) propagates and bails. `ensure_codebase_memory_mcp` also `sh`-installed then registered the MCP unconditionally, leaving omp a broken command entry.
- Fix: runtime `platform::os() == Os::Windows` branches (NOT `#[cfg(windows)]`, so the host `cargo check`/`build` type-checks them) install omp/codegraph via npm packages (`@oh-my-pi/pi-coding-agent`, `@colbymchenry/codegraph`) using `bun add -g` / `npm install -g` resolved via `which` (Command won't find `npm.cmd` from bare `npm`); `ensure_uv` uses uv's PowerShell installer; cbm/headroom register only when the binary exists.
- Decision: `bun add -g <pkg>` is correct (verified against `bun add --help`, bun 1.3.14: `-g, --global  Install globally`). Do NOT "fix" it to `bun install -g`.
- validated: `cargo build -q`, `cargo check -q`, `cargo test -q` (20 passed) all exit 0 on macOS; `ckit setup --dry-run` clean. `cargo check --target x86_64-pc-windows-msvc` is blocked here by the `zstd-sys` C cross-compile (no MSVC/Windows-SDK headers on this mac) — the real Windows build is CI's `windows-latest` runner in `release.yml`.

## validated: docs/index.html — install by platform, verify block, omp-centric daily, no dashboard (2026-08-04)
- Structure the landing page as: Quick start (two cards: macOS/Linux `install.sh` vs Windows `install.ps1`) → a "verify the install" codeblock (`ckit doctor` is the source of truth; plus `ckit --version` / `omp --version` / `codegraph --version` / `gh auth status`) → How to use (one-time setup vs "Daily: open omp and code" with `/auto`|`/plan`|`/feature`). Keep Install and Usage adjacent.
- The dashboard (`ckit harness web`) was removed per user: drop the section, the `#dashboard` nav link, the feature-grid card, AND the command-table row — grep `dashboard`/`harness web` must return 0.
- validated: Python `HTMLParser` parse-ok; grep counts `dashboard`=0, `harness web`=0; browser DOM check — section order `install,usage,commands,skills,update,docs`, `#install`.nextElementSibling is `#usage`, verify block + two side-by-side install cards render.

## validated: post-install config + verify guidance for omp (2026-08-04)
- omp config lives in `~/.omp/agent/`: `config.yml` (memory backend = Mnemopi per-project, compaction, `modelRoles`, tool approval), `models.yml` (9router model catalog + gateway `apiKey`), `mcp.json` (4 STEP-0 servers). ckit's per-role model picks are in `~/.config/ckit/models.toml`.
- The gateway **API key AND endpoint URL** live in `~/.omp/agent/models.yml`, substituted from `$NINE_ROUTER_KEY`/`$NINE_ROUTER_URL` (or preserved) by `ckit harness gateway apply`. Set them via `ckit harness gateway key <KEY>` and `ckit harness gateway url http://<host>:<port>/v1`, then `ckit harness gateway verify` (pings `cc/claude-sonnet-5`, expects HTTP 200) — do NOT hand-edit the substituted values; route models via `ckit harness model`. NEVER quote a live `sk-...` key/IP in docs/commits.
- validated (2026-08-04): the deployed gateway config passed `ckit harness gateway verify` → `gateway healthy — cc/claude-sonnet-5 → HTTP 200`, so the provided key+endpoint already run. (Keep the actual `<host>:<port>` out of git.)
- Verify MCP inside omp with `/mcp list` — expect `codebase-memory-mcp`, `codegraph`, `headroom`, `serena` all `connected [stdio]`; if not, re-run `ckit harness` then `ckit doctor`. Windows may skip `codebase-memory-mcp` (no installer yet).
- Migrate a Claude Code project into omp with `omp --from-claude` (or `--from-codex`) — imports the session; Mnemopi (per-project) recalls/retains from there. There is no folder-import CLI for `.claude/projects/<slug>/memory`.

## validated: ckit setup — codegraph best-effort + local-config-first ordering (2026-08-04)
- Bug (Windows): strict `ckit setup` aborted at the `codegraph` step (its install can fail on a bare box — cause unconfirmed, no Windows host here) BEFORE seeding `~/.omp/agent/models.yml` (a later step) → both codegraph and models.yml missing. `@colbymchenry/codegraph` exists on npm (bin `codegraph`).
- Fix: `install_codegraph` is best-effort (warn, never `?`-bail) on all platforms; Stage A now runs the local-file steps (path-bootstrap, configs, `models.yml` seed, `config.yml`) BEFORE the fallible external installs (gh/omp/codegraph), so omp config always lands. `step0-mcps` stays AFTER installs (registers only present binaries). omp stays fatal (essential); codegraph optional.
- UI/brand: `ui::{info,ok,warn,step,skip}` pass through `brand::render` which rewrites `8sync`→ active `NS`/`CMD` at DISPLAY time; files written via `fs` are NOT rendered. So any hardcoded `8sync-*` artifact filename mismatches the UI. Use `brand::ns_file("<suffix>")` for namespace artifacts (fixed the fish PATH file: was hardcoded `8sync-path.fish`, now `ns_file` = `ckit-path.fish`). Do NOT change the zsh/bash idempotency MARKER string (existing rc files match on it — changing appends a duplicate block).
- validated: `cargo build -q` + `cargo test -q` (20 pass) exit 0; `ckit setup --dry-run` shows local-first order and `ckit-path.fish` consistently.
