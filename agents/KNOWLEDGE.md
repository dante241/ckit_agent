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

## validated: STEP-0 MCP default + remove zai-vision (2026-07-31)
- `setup.rs` Stage A now registers all STEP-0 MCPs (`codegraph`, `codebase-memory-mcp`, `headroom`, `serena`) in one `step0-mcps` try_step (trước chỉ có `codegraph`), rồi `deregister_zai_vision_mcp` để dọn máy cũ. `harness init`/`global` đã có sẵn chuỗi này.
- `zai-vision` removal: xoá bundled skill asset `assets/skills/zai-vision/`, gỡ khỏi force-load/image-routing/locate/APPEND_SYSTEM/root docs/doctor text; image understanding route qua model-native hoặc built-in image/inspect tools. `deregister_zai_vision_mcp` giờ cũng `remove_dir_all(~/.omp/skills/zai-vision)`.
- `ensure_mcp_tools_visible`: giữ 2 hằng số legacy exact-match — `LEGACY_PIN` (không zai) và `LEGACY_PIN_WITH_ZAI` — để migrate sạch cả 2 đời config user. Đây là match duy nhất còn lại của "zai" trong scan, đúng chủ đích cleanup.
- validated: `cargo check -q` + `cargo build -q` exit 0; stale-ref scan chỉ còn cleanup literal.
