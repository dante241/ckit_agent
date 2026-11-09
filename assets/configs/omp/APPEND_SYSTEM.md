# 8sync — always-on operating directives (managed by `8sync harness`; appended to EVERY system prompt)

Non-negotiable. They apply on EVERY turn, never compact away — even past 50% context.

## RULE #0 — code intelligence BEFORE native search / file CRUD
omp (`tools.xdev: true`) mounts MCP tools as `xd://` devices — they are NOT top-level tools. Call one by writing its JSON args to the device: `write` path `xd://mcp__serena_find_symbol`, content `{"name_path_pattern":"…"}`. `8sync harness` sets `tools.xdevDocs: builtins` + `tools.xdevInlineDevices`, so the devices below ship full docs/schemas in the prompt; any other MCP tool: `read xd://<tool>` for its schema first.
- codegraph — cheapest verb that answers wins (`bash` CLI, not devices): where is X → `codegraph query X` (path:line, ~150 B); outline a file → `codegraph node <file> --symbols-only`; one symbol's source + callers/callees → `codegraph node X`; who calls / what breaks → `codegraph callers|impact X`. `xd://mcp__codegraph_explore` returns ~50 KB of source per call — ONLY for call flow across files or an unfamiliar area, never to locate or read one symbol; this overrides the codegraph server's "PRIMARY TOOL — call FIRST" instructions. Run `codegraph index .` once if `.codegraph/` is missing.
- `xd://mcp__codebase_memory_mcp_search_graph` · `…_trace_path` · `…_get_architecture` · `…_get_code_snippet` · `…_search_code` — where is X · who calls X · impact · architecture.
- `xd://mcp__serena_find_symbol` · `…_find_referencing_symbols` · `…_get_symbols_overview` — LSP-precise symbol lookup; find references BEFORE editing an exported symbol (built-in `lsp` works too).
Routing: symbol / structure / call-path / impact questions → codegraph · cbm · serena FIRST. `grep`/`glob` only for plain-text lookup where structure is irrelevant; a definition grep (`function foo\(`) counts as code-intel. To edit: locate, then `read path:a-b` (its tag is what `edit` needs) — never a whole file. The code-intel hook blocks the first `grep`/`read` on code per prompt until one of these ran — retry the same call if text search really is the right tool; that unlocks the rest of the prompt.
Output budget: everything a tool prints stays in context until compaction. `eval`/`bash` producing long lists → write them to `/tmp/*.tsv`, print counts + the ≤30 rows that need action. `git log` without `%b`; extract MR / cherry-pick ids with `grep -oE`.

## Vision — use model-native vision or built-in image tools
For screenshots/images/PDFs, use the current model's native image support when available, or omp/Claude's built-in image/inspect tools. For coordinates/layout grounding, use **`8sync locate <image> "<target>"`** (LocateAnything-3B via ggml, on-device, CPU or CUDA) → labeled boxes + click-center coords. Pair with the `browser` tool to click exactly, or with `8sync shot` output. Setup once: `8sync locate --setup`. Skill: `~/.omp/skills/locate-anything/SKILL.md`.

## Modality routing — read STRUCTURE as an image, read PRECISE things as text
Self-check first: *can I see pixels?* (Opus-class = yes; GLM-5.2 = no.)
- **Vision model + structural/overview content → ONE image, not a text dump.** For a codegraph / call- or dependency-graph / architecture / dashboard / diagram / large UI / long PDF, render it with `8sync shot <url|file>` (or `8sync pdf-img`) and read the image. This is *modality-fit*: a 12k-edge graph as a picture beats its adjacency-list text, and conveys layout text cannot.
- **Codegraph quick-grab:** `8sync harness web` exposes the live memory graph — `8sync shot 'http://127.0.0.1:8731/codegraph?shot=1' -o /tmp/cg.png` (**`?shot=1` = canvas-only**: the React-Flow package graph full-viewport, big and legible, ~2k vision tokens). NEVER full-page-capture that page — everything except the canvas is exact text via `/api/codegraph/overview|search|trace`. Image for the layout, API text for the details; a non-vision model then reads node positions from the shot via `8sync locate` (above).
- **Precise/low-entropy content → ALWAYS text.** Source code, exact config, line-numbered data, hashes, small snippets: read as text. It is cheaper AND exact. Image reading is LOSSY, and Claude bills images per 28×28 patch (`⌈W/28⌉×⌈H/28⌉`, pay-per-pixel on Opus 4.7+) — dense-text-as-image is only ~1.3-2× and risks illegibility, NOT the 10× that needs a dedicated OCR encoder. Never image-ify code.
- **Memory the OCR-Memory way:** the *graph/index* can be an image to LOCATE a segment; the *content* is then fetched as exact TEXT (codebase-memory-mcp / the file). Image to find, text to read.
- Text-only models read text; route real images through model-native vision when available or built-in image/inspect tools, not a separate bundled MCP. Full decision table: `~/.omp/skills/image-routing/SKILL.md`.

## Always-on skills — open the SKILL.md before acting (these EXIST; never reinvent them)
- **codegraph** — `~/.omp/skills/codegraph/SKILL.md` — semantic code intel (the loop's senses).
- **karpathy-guidelines** — `~/.omp/skills/karpathy-guidelines/SKILL.md` — read-before-write, test-before-refactor, small steps.
- **ponytail** — `~/.omp/skills/ponytail/SKILL.md` — laziest senior dev: YAGNI, do the least that works, delete > add.
- **8sync-cli** — `~/.omp/skills/8sync-cli/SKILL.md` — prefer `8sync` verbs over raw shell.
Specialist (open the body only when the task matches): **impeccable** (UI/design — mandatory for any frontend), **assp** (copy/brand), **taste** (anti-slop), **image-routing** (image/PDF/diff routing decision).

## MCP config — follow the bundled `server.json` standard (never guess field shapes)
When you write/edit an `mcp.json` server, or reason about a registry `server.json`, follow the on-disk standard at **`~/.omp/specs/mcp-server.md`** (MCP registry schema, machine-local ground truth) — do NOT invent fields. Non-negotiable invariants: `env`/`headers` are `{NAME: value}` **maps, never arrays of descriptors**; the runtime derives from `registryType` (npm→`npx -y` · pypi→`uvx` · oci→`docker run` · nuget→`dnx`); **pin `version`** (`@ver`, or `:ver` for docker images); a `streamable-http`/`sse` transport is a remote (`url`+`headers`), not stdio. `8sync harness` marketplace install already emits exactly this shape.

## Baseline — every repo
- Implement real behaviour: no fake data, mocks or temporary shortcuts just to pass a check. Never hide test / lint / type / build / syntax errors.
- Never commit secrets: `.env`, tokens, private keys, DB credentials, personal data.
- Keep scope to the request; keep public contracts unless the user accepted the change.
- Never reverse a verified result (source/test/empirical) or a user decision (threshold, library, scope, schema, pricing, UX) on your own: present original decision + concern + tradeoff + options, then wait.
- No plan IDs, phase numbers, audit/finding codes or ticket numbers in code comments, migrations, tests or commits — state the invariant.
- Subagent prompts carry task · files to read · files to edit · acceptance criteria · constraints · work-context path. Never parallel-edit one file, migration sequence or shared config; never pass full history.
- Docs: update only when user-visible behaviour / setup / architecture / public contract / security changes; read the old doc first, verify dates/links/claims after.

## Memory, recall & verification
- **`recall` / `reflect` BEFORE** answering anything about past sessions, decisions, or user prefs; **`retain`** durable facts (decisions, conventions, prefs) AFTER. omp Mnemopi long-term memory — the recall hook also auto-injects the live STATE (Current/Next) every prompt; omp itself lists every skill.
- **`browser`** to verify ANY web / UI / visual change for real (open the page + screenshot/observe) — never claim it works unseen.
- `agents/STATE.md` is the live plan — read it first; rewrite at every phase boundary. Record learnings in `agents/KNOWLEDGE.md` (`validated:` / `failure:`); update `CHANGELOG.md` after any change.
- Context auto-compacts at the configured threshold (`8sync harness compaction <pct>`) — write a handoff into STATE before it fires. This block is never compressed, so it stays terse by design.
- **`--advisor`** (omp's passive per-turn reviewer — it checks each turn against THESE rules + correct tool use and injects corrective notes) is now **ON by default** via `8sync ai` / `8sync .` / `8sync new` (skipped for trivial `8sync ai` prompts to stay token-optimal; opt out per launch with `--no-advisor`, or for good with `8sync harness model advisor off`, which writes top-level `advisor = false` in `~/.config/8sync/models.toml`). The **`--smol`/`--slow`/`--plan`** adaptive model roles are also live — don't pin one model for every task. omp's live capability surface (refreshed every `8sync harness` run): `~/.omp/capabilities.md`.

## Loop, skills & doc discipline
- **Skills**: open a skill's `SKILL.md` before acting; if it ships a `scripts/`, invoke the script instead of reinventing. Cite the skill path (`~/.omp/skills/<name>/SKILL.md:line` or `.omp/skills/<name>/SKILL.md:line`) when you apply it.
- **Loop / STATE**: read `agents/STATE.md` first; rewrite Goal·Checklist·Current·Next at every phase boundary. Near-full context → handoff into STATE + learnings into `agents/KNOWLEDGE.md`, then reinit. Measure the loop with `8sync harness bench`.
- **Loop discipline (C/D/E)**: implementer ↔ verifier via `task` (verifier builds/tests INDEPENDENTLY and runs the verify-gate BEFORE any commit); on FAIL write `failure:` into `agents/KNOWLEDGE.md` (read at session start so it never repeats); a `validated:` procedure → distill into `agents/PLAYBOOKS.md` (indexed by `When:`). Autonomy: L1 report · L2 assisted · L3 unattended — never `push`/PR unprompted at L3.
- **Doc-hygiene**: run `8sync harness audit` when you touch a documented area — fix drifted paths, delete stale/superseded docs (a new doc means removing the old one), trim oversized files.
