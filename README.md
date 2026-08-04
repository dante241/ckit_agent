# ckit

> Terminal-first AI coding harness — **Linux (CachyOS/Arch), macOS, Windows** · Kitty + Helix + [omp](https://omp.sh).
> Keep your normal CLI workflow; AI agents observe project context, load `agents/*` memory, and execute tasks on demand.

![ckit harness web — the agent-team dashboard showing this repo's live plan](docs/assets/dashboard-state.png)

---

## Links

- **Website / docs**: <https://dante241.github.io/ckit_agent> (auto-deployed from `docs/` via [`.github/workflows/pages.yml`](.github/workflows/pages.yml))
- **Repo**: <https://github.com/dante241/ckit_agent>
- **Discussions**: <https://github.com/dante241/ckit_agent/discussions>
- **AI engine**: [omp](https://omp.sh) (oh-my-pi) — `ckit` wraps `omp --continue` to keep one session per project.

> Note: `ckit` is a **coding harness**; it does not install a desktop environment. Install Hyprland/Caelestia/HyDE separately, following their upstreams.

---

## TL;DR

```bash
# 1. Install — one-liner, prebuilt binary (NO git/rust/cargo required)
curl -fsSL https://raw.githubusercontent.com/dante241/ckit_agent/main/install.sh | sh
ckit setup                       # install the AI core (omp + codegraph + MCP/skills + gh) — y/N per profile
ckit doctor                      # verify (auto-cleans stale state if any)

# 2. Enter a project → stand up the harness (1 command, idempotent)
cd <project>
ckit harness                     # skills + codegraph index + AGENTS.md + memory — safe to re-run anytime
ckit .                           # open a session (kitty 3-pane + omp)

# 3. Dashboard — monitor + CRUD the whole agent-team from the browser
ckit harness web                 # http://127.0.0.1:8731 — models, skills, memory, rules,
                                  # engines, Codegraph graph, bench, team… edit live, writes immediately

# Daily
ckit ai "explain this codebase"  # one-shot prompt; leave empty to resume the session
ckit ship "feat: ..."            # add + commit + push + gh pr create
```

---

## Installation

### 1. One-liner (recommended) — download the prebuilt binary

No git, rustup, or cargo needed. `install.sh` resolves the latest release and downloads the matching `ckit-<tag>-<os>-<arch>` prebuilt (Linux `x86_64`/`aarch64`, macOS `x86_64`/`arm64`) from GitHub Releases into `~/.local/bin/ckit` (atomically replacing any previous version). On **Windows**, use `install.ps1` instead (below).

```bash
curl -fsSL https://raw.githubusercontent.com/dante241/ckit_agent/main/install.sh | sh
```

On **Windows** (PowerShell) the binary comes from `install.ps1`, which installs
`ckit.exe` into `%LOCALAPPDATA%\Programs\ckit` and adds it to your user PATH:

```powershell
irm https://raw.githubusercontent.com/dante241/ckit_agent/main/install.ps1 | iex
```

Then run `ckit setup` to install the AI core. On Windows it needs a couple of
tools on PATH first (all cross-platform, no Arch/pacman involved):

- **`bun`** (recommended, https://bun.sh) **or Node.js `npm`** — required for
  `omp` (`@oh-my-pi/pi-coding-agent`) and `codegraph` (`@colbymchenry/codegraph`);
  `ckit setup` installs both via `bun add -g` / `npm install -g`.
- **`winget`** (ships with Windows 10/11) — used for GitHub CLI (`gh`), needed by
  `ckit ship`.
- **`uv`** (optional, https://astral.sh/uv) — powers the `headroom` + `serena`
  MCP servers; `ckit setup` bootstraps it via uv's PowerShell installer if
  missing. Skipped cleanly if unavailable (those MCP servers are just left out).

```powershell
# with bun or Node installed:
ckit setup      # installs omp + codegraph + MCP servers; Arch-only profiles are skipped
ckit doctor     # verify
```

- **Upgrade**: run the exact same command again, or `ckit up`.
- **Pin a version**: `curl -fsSL .../install.sh | CKIT_VERSION=v0.48.0 sh` (or `$env:CKIT_VERSION` for `install.ps1`)
- **Change the install dir**: `... | CKIT_BIN_DIR=~/bin sh`
- **Uninstall**: `curl -fsSL .../install.sh | sh -s -- --uninstall`

PATH entries for `~/.local/bin`, `~/.cargo/bin`, `~/.bun/bin`, `~/.encore/bin` are auto-patched into zsh / bash / fish the first time you run `ckit setup` (see Stage A). If `~/.local/bin` is not on PATH at install time, the script prints a hint.

> **Platform support**: the AI-harness core (`omp`, `harness`, skills, the dashboard, `feature`, `ai`, `find`, `ship`) runs on **Linux, macOS, and Windows**. The Arch-only profiles (`dev-stack` pacman, `warp`, `bluetooth`, `nvidia`) and desktop bits (kitty session, HyDE) are **Linux-only** — skipped with a clear note off-Linux.

### 2. Build from source (contributors) — `scripts/bootstrap.sh`

Use this when you want to build from code (no prebuilt for your arch yet, or for development). It installs rustup (if missing) → `cargo build --release --locked` → copies the binary to `~/.local/bin/ckit`.

```bash
git clone https://github.com/dante241/ckit_agent.git
cd ckit_agent
bash scripts/bootstrap.sh
```

### 3. `ckit setup` — install the rest

Stage A (harness, always idempotent):

- `pacman -S --needed helix lazygit abduco github-cli`
- omp CLI via `curl -fsSL https://omp.sh/install | sh` (skipped if already present)
- writes configs: `~/.config/helix/`, `~/.config/kitty/ckit.session`, `~/.config/ckit/{global,skills}.toml`
- writes skills (37 bundled) to `~/.omp/skills/<name>/SKILL.md` + `00-force-load.md`. Always-on: codegraph, karpathy-guidelines, ponytail, assp-skill, impeccable, taste-skill, 8sync-cli, image-routing. On-demand: feature (large-scope GSD), code-review-and-quality, senior-security, senior-frontend, full-flow, last30days + 18 research skills (`social-growth` opt-in)

Stage B (community profiles, opt-in y/N per profile):

| Profile | Description |
|---|---|
| `dev-stack` | Docker + Node/npm/bun/pnpm + Encore + TS LSP + build chain |
| `nvidia` | Auto-detects GPU family → open-dkms / dkms (skipped if CachyOS chwd already installed it) |
| `warp` | Cloudflare WARP VPN + DoH + MASQUE (toggle via `ckit sec`) |
| `bluetooth` | bluez + bluez-utils + service enable (control via `ckit bt`) |

Common flags:

| Flag | Effect |
|---|---|
| `ckit setup --dry-run` | Print the plan, change nothing |
| `ckit setup --no-profile` | Stage A only |
| `ckit setup --community` | Stage A + dev-stack + bluetooth (does NOT include warp) |
| `ckit setup --profile <name>` | Stage A + apply one specific profile |
| `ckit setup profile list \| show <n> \| apply <n>` | Manage profiles after setup |

### 4. Update

```bash
ckit up                         # self-update the binary (GitHub release) + omp update
```

Or rebuild manually from source:

```bash
cd ckit_agent && git pull
cargo build --release
install -m755 target/release/ckit ~/.local/bin/ckit
```

System packages (`pacman -Syu`) are **not** run automatically — you decide when to update CachyOS rolling.

---

## Main verbs

### Vibe loop (daily)

| Command | Description |
|---|---|
| `ckit .` | Open/attach the current project's session. Kitty with `allow_remote_control yes` → 3-pane; otherwise → soft 1-pane + `omp --continue` inside abduco |
| `ckit ai [prompt]` | Empty/`continue` → `omp --continue`; with a prompt → `omp -p "..."` |
| `ckit find <kw>` | rg/fd + fzf preview → open the editor at `file:line` |
| `ckit note "msg" [-t tag]` | Append to `agents/NOTES.md` |
| `ckit run [dev\|build\|test\|fmt\|lint]` | Project runner via per-stack recipe |
| `ckit ship "msg"` | `git add -A && commit && push && gh pr create` |
| `ckit feature [new\|switch\|status\|list] <slug>` | Large multi-phase scopes (GSD): scaffold a planning tree `agents/planning/<slug>/` with per-phase acceptance-criteria gates + a cross-feature `ACTIVE` switch; then run `/feature plan\|go\|ship` in an omp session (`go` delegates to the `engine_*` verify-gate loop) |
| `ckit feynman [auth-omp\|status\|off]` | Bridge **Feynman** (companion-inc/feynman, a Pi research agent) to omp's already-authed providers: mirror omp's live creds into `~/.feynman/agent/auth.json` so `feynman model list` shows the **same models** reusing omp's **Claude Pro/Max OAuth** + API keys — no second login, and omp's faster-moving catalog. OAuth copied access-only (no refresh) so omp stays the sole refresher; API keys resolved live via `!omp token`. `off` removes only omp-managed entries |

### Session management (sub-commands of `.`)

`ckit . ls` / `to <n>` / `new <n> [cmd]` / `rm <n>` / `wipe` / `kick`

### Harness (agent-team bootstrap + dashboard)

| Command | Description |
|---|---|
| `ckit harness` | **One command (idempotent):** deploy/update bundled skills + codegraph binary + external packs (ponytail/addyosmani, best-effort) → `~/.omp/skills/`, mirror into `.omp/skills/`, `codegraph init`, seed `agents/*` + `CHANGELOG.md`, inject the force-load block into `AGENTS.md`/`CLAUDE.md`. Always safe to re-run |
| `ckit harness init` | First-time full bootstrap (progress UI) + managed `.gitignore` + gitleaks pre-commit hook. `--force` re-mirrors everything, overwriting |
| `ckit harness up` | Refresh state: re-inject + refresh `KNOWLEDGE.md` + re-index codegraph. `--pull` re-pulls skills · `--commit` git-commits memory (gitleaks scan first) · `--loop 10m` (foreground) · `--timer 30m\|off` (systemd user timer, for background runs) |
| **`ckit harness web`** | **Local dashboard** (axum + Vite, `http://127.0.0.1:8731`) — view & **CRUD** the whole agent-team from the browser (see the Dashboard section) |
| `ckit harness gateway [apply\|key <K>\|verify\|status]` | Deploy/verify the omp model-gateway (`~/.omp/agent/models.yml`): 9router + `thinking.mode` fix for claude-sonnet-5. `verify` pings; HTTP 200 = healthy |
| `ckit harness add-local-model <path.gguf\|org/repo\|url> [name]` | Load a local **GGUF** through **mistral.rs** (Rust, memory-safe) → serve an OpenAI endpoint + register it as omp provider `local/<name>`. `list`/`rm <name>` to manage. Then `ckit ai --model local/<name>` |
| `ckit harness add-model <provider/model> --url <baseUrl> [--key\|--api\|--ctx\|--max\|--vision\|--think]` | Register a **remote** model omp's fetched catalog lacks — or lists with null metadata (e.g. a brand-new `xai/grok-4.5`) — as a full custom provider in `~/.omp/agent/models.yml`, so it shows in `/model` and routes. `--url` required (omp rejects custom models without it); selector = `<provider>/<model>`. `list`/`rm <provider/model>` to manage. Coexists with the gateway + local-model blocks |
| `ckit harness browser [fix\|status\|off]` | Point omp's Puppeteer browser control at a real system **Chromium** (`ungoogled-chromium-bin`, installs `/usr/bin/chromium`) instead of the bundled `chrome-headless-shell` — fixes browser control that renders but **can't reach the internet**. Exports `PUPPETEER_EXECUTABLE_PATH` + `BUN_CHROME_PATH` in zsh/bash/fish (idempotent). `off` reverts, `status` shows the wiring |
| `ckit harness bench` | Measure the loop's context budget (upfront vs deferred tokens + KV-cache gate). Prints the upfront breakdown — prefix / CORE / memory-spine — and a spine advisory when the memory spine eats more than 50% of the upfront budget |
| `ckit harness audit` | Scan docs: stale paths / oversized / junk + churn (doc-hygiene) |
| `ckit harness eval [--baseline]` | Run the quality task-suite through omp; `--baseline` saves the reference |
| `ckit harness toolstats` | SQLite tracker: optimizer rate (codegraph/cbm/serena) vs fallback (grep/read) + failures per tool |
| `ckit harness compaction [pct]` | View/set the omp auto-compaction threshold (anti-forget; default 50%) |
| `ckit harness model [k v]` | View/edit `~/.config/ckit/models.toml` (routing for `/auto` + `ckit ai`) |

### Skill system

| Command | Description |
|---|---|
| `ckit skill` | List global (`~/.omp/skills/`) + local (`.omp/skills/`) skills |
| `ckit skill add <github-url>` | Clone into **both** global + project; **collection-aware** (repo with `skills/<name>/` → installs every sub-skill, e.g. `addyosmani/agent-skills`). Rewrites the `<!-- ckit:skills:* -->` block in `AGENTS.md` |
| `ckit skill add gh:owner/repo` · `<url>@<ref>` · `builtin:<name>` | Short form · pin a commit/tag (writes `rev` into `skills.toml` = lockfile) · enable an opt-in bundled skill (e.g. `builtin:social-growth`) |
| `ckit skill update [name]` | Re-pull from `src` (git dedup by URL, honors `rev` pins) |
| `ckit skill gen <id> <id>` | Fuse N local skills into 1 combined SKILL.md |

**37 skills bundled** in the binary. Always-on (read in order): codegraph → karpathy → ponytail → assp → impeccable → taste → 8sync-cli → image-routing. On-demand: feature (large-scope GSD) · code-review-and-quality · senior-security · senior-frontend · full-flow · last30days + 18 research skills (deep-research, literature-review, autoresearch, paper-writing…). `encore-deploy` is tech-gated; `social-growth` is opt-in. Idempotent: re-running `add` with the same URL → `git pull --ff-only`.


### Lifecycle

| Command | Description |
|---|---|
| `ckit setup` | Install harness + profiles (see Installation) |
| `ckit up` | Self-update the binary + `omp update` |
| `ckit doctor` | Health check (kitty remote, omp, helix, gh, configs, profiles, WARP/ufw) |
| `ckit flow` | Workflow help, ordered by usage step |
| `ckit help` | Cheatsheet (alias of `ckit` with no args) |

### AI tooling

| Command | Description |
|---|---|
| `ckit shot <url\|file>` | Render a web page/file → PNG (for the image-routing skill) |
| `ckit diff-img [ref]` | Git diff → PNG |
| `ckit pdf-img <file>` | PDF pages → PNG |
| `ckit locate <img> "<prompt>"` | Visual grounding (NVIDIA LocateAnything-3B via ggml, CPU/GPU): image + open-vocabulary prompt → labeled boxes + click-center coordinates. One-time `--setup` first; `--annotated out.png` draws the boxes. Model is research / non-commercial use only |

### Security

`ckit sec [on\|off\|toggle\|status]` — enable/disable Cloudflare WARP VPN + ufw firewall together. Subs: `sec warp …`, `sec ufw …`.

`ckit vpn [install\|gui\|list [CC]\|on [CC\|ip]\|off\|status]` — SoftEther VPN Client + **VPN Gate** (University of Tsukuba academic public relays) for "study/learn through another region". `install` pulls the native Linux engine (`softethervpn` — the maintained RTM 4.44 build, not the `-git` 5.x dev edition) + the **Windows VPN Client Manager GUI under Wine** (`softethervpn-client-manager`, `--no-gui` to skip) + `dhcpcd`, and enables the client service. `gui` opens that Windows-style manager (where the region-switch plugin lives). SoftEther has **no native Linux GUI** and its Linux client **can't rewrite the routing table itself** — so the reliable region-switch is the CLI: `on [CC]` picks the best relay (optionally by 2-letter country), connects, pins the relay route to the physical uplink, DHCPs the tap, full-tunnels the default route, swaps DNS to 1.1.1.1, and **auto-rolls-back if egress doesn't change**; `off` restores routes/DNS. VPN Gate relays are volunteer-run and **logged** — a learning tunnel, never for anything sensitive.

### Machine (desktop / housekeeping)

| Command | Description |
|---|---|
| `ckit bt [on\|off\|fix\|restart]` | Bluetooth (bluez): status / on-off / troubleshoot a dead adapter / restart |
| `ckit clean [--deep\|--ram\|--gpu\|--timer 1h]` | Reclaim disk (paccache/journal/thumbnails) + CPU/GPU/RAM report. `--deep` removes orphans; never touches models/package download caches |
| `ckit theme [list\|set <name>\|show]` | Switch the kitty color palette live (colors only, structure untouched) |
| `ckit bg [set\|list\|add <url>\|search <q>]` | Kitty wallpaper live swap + inline preview; `bg search` = wallhaven.cc (no API key) |

Every verb supports `-h` / `--help` with a detailed `EXAMPLES` block.

---

## Dashboard — `ckit harness web`

A local web app (axum backend + Vite/React FE, embedded in the binary) to **view and control the whole agent-team** instead of hand-editing config files:

```bash
ckit harness web                 # http://127.0.0.1:8731 (auto-opens the browser)
ckit harness web --port 9000     # change the port
ckit harness web --no-open       # no auto-open (background / headless)
```

The sidebar is grouped — every page reads **real data** (no mocks), and most pages support **CRUD written straight** to config/memory:

| Group | Page | What you can do |
|---|---|---|
| Session | **State · Context** | Live plan (`agents/STATE.md`), real session token/compaction stats |
| Configure | **Models · Skills · Memory · Rules** | Change the model per role/task (writes `models.toml` immediately) · filter + cycle tiers across the 37 skills · edit the 6 memory files (STATE/KNOWLEDGE…) · add/remove rules |
| Runtime | **Engines · Codegraph · MCP · Submodules** | Engine status (codegraph/cbm/headroom/serena/mnemopi) · **codebase graph**: package call graph (elk) + 12 Leiden clusters + symbol search + caller/callee tracing · MCP servers · git submodules |
| Quality | **Bench · Readiness · Team** | Run `harness bench` live — the page auto-loads with upfront breakdown meters (prefix / CORE / memory-spine) + a spine advisory · readiness gate · team roster |
| Discover | **Marketplace** | Browse + one-click install MCP servers & skills from the official registry, Smithery, Glama, and mcp.so |
| Projects/Build | **Workspaces · Workflow** | Project switcher · pipeline builder for skills/subagents/tools (exports as an omp extension) |

![Codegraph page — package call graph + Leiden clusters](docs/assets/dashboard-codegraph.png)

![Models page — adaptive model routing per role, written to config immediately](docs/assets/dashboard-models.png)

![Bench page — token-budget bench with upfront breakdown meters (prefix / CORE / memory-spine) + spine advisory](docs/assets/dashboard-bench.png)

![Marketplace page — browse + one-click install MCP servers & skills from official registry, Smithery, Glama, mcp.so](docs/assets/dashboard-marketplace.png)

---

## Project memory

The first time you run `ckit .` inside a project, these files/folders are seeded:

```
<repo>/
├── AGENTS.md                    ← anchor for every AI tool; holds the force-load skills block
└── agents/                       ← shared memory (omp/claude-code/cursor/opencode/aider)
    ├── PROJECT.md               fixed facts (stack, entrypoints)
    ├── KNOWLEDGE.md             append-only: what the AI has learned
    ├── DECISIONS.md             append-only: architecture decisions
    ├── PREFERENCES.md           append-only: user style
    ├── STATE.md                 work in progress
    ├── NOTES.md                 quick notes via `ckit note`
    └── skills/                  project-local skills (cloned via `ckit skill add <url>`)
```

`omp` manages session memory itself (`retain` / `recall` / auto-compact) — you do **not** hand-edit `agents/*.md`. `ckit note` is the only exception (appends to `NOTES.md`).

---

## Documentation site

A static page in `docs/index.html`, deployed automatically via GitHub Pages:

- **Source**: [`docs/index.html`](docs/index.html)
- **Workflow**: [`.github/workflows/pages.yml`](.github/workflows/pages.yml) (triggers: push to `main` or workflow_dispatch)
- **URL**: <https://dante241.github.io/ckit_agent>

Edit `docs/index.html` → push to `main` → Pages rebuilds in ~1 minute.

---

## Stack & contribute

Rust workspace, 1 binary (`ckit` ≈ 6.1 MB stripped — bundles the web dashboard FE + 37 skills, heaviest is `impeccable`). Toolchain pinned in `rust-toolchain.toml`. The web dashboard is built from `web/` (Vite/React) via `build.rs` and embedded with rust-embed. The CLI command name + on-disk namespace are single-sourced in `crates/cli/src/brand.rs` — set `SC_CMD`/`SC_NS` at build time to rebrand the whole binary in one place (the default build stays `ckit`, byte-identical).

Source layout:

```
crates/cli/src/
├── main.rs                       clap router
├── ui.rs · env_detect.rs · pkg.rs · assets.rs · brand.rs (single-source CLI name)
└── verbs/                        1 file / 1 verb
    ├── root.rs flow.rs setup.rs doctor.rs up.rs selfup.rs
    ├── here.rs feature.rs ai.rs ship.rs run.rs find.rs note.rs
    ├── skill.rs shot.rs diff_img.rs pdf_img.rs
    ├── profile.rs sec.rs
assets/                           embedded into the binary via rust-embed
├── configs/                      kitty.session, helix-config, fish-config, ckit/*.toml
├── presets/                      kitty preset themes
├── skills/                       37 bundled (codegraph, karpathy, ponytail, assp, impeccable, taste, 8sync-cli, image-routing, feature, code-review, senior-security/frontend, full-flow, encore-deploy, last30days, 18 research skills, …)
└── wallpapers/
```

To add a new verb: create `verbs/<new>.rs` with `pub fn run(a: Args) -> Result<()>`, add `pub mod <new>;` to `verbs/mod.rs`, and a `<New>` variant + match arm in `main.rs`.

Smoke test:

```bash
cargo build --release
./target/release/ckit --version
./target/release/ckit help
./target/release/ckit flow
./target/release/ckit doctor
./target/release/ckit skill
./target/release/ckit harness web --no-open   # dashboard → http://127.0.0.1:8731
```

See [`AGENTS.md`](AGENTS.md) for the detailed guide for AI agents / contributors.

---

## License

MIT. See [`LICENSE`](LICENSE).

`#ckit #AIAgent #VibeCoding #omp #CodingHarness #TerminalWorkflow #DeveloperTools #RustLang #KittyTerminal #HelixEditor #ArchLinux #CachyOS #OpenSource`
