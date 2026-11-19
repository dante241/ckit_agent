# Changelog

Mọi thay đổi đáng kể của `8sync` ghi vào đây. Format theo [Keep a Changelog](https://keepachangelog.com),
versioning theo [SemVer](https://semver.org). **8sync rule:** mỗi PR cập nhật mục `Unreleased`.

**Release (giữ code · changelog · docs đồng bộ):** (1) bump `Cargo.toml` → `version = "X.Y.Z"`;
(1b) chạy `cargo build --release` để refresh `Cargo.lock` — **CI build `--locked`, lock cũ sẽ fail**;
(2) đổi `## [Unreleased]` → `## [X.Y.Z] - YYYY-MM-DD`, thêm `## [Unreleased]` rỗng ở trên;
(3) `git commit` + `git tag vX.Y.Z && git push && git push --tags`. Tag → `release.yml` build
binary mọi OS + publish GitHub Release; push `main` → `pages.yml` copy `CHANGELOG.md` vào `docs/`
và redeploy — mục Changelog trên trang docs tự cập nhật (không sửa HTML tay).

## [Unreleased]

### Added
- **Telegram for interactive omp sessions** (`ckit-telegram` extension, deployed to `~/.omp/agent/extensions/` by every `ckit harness` pass; no dashboard needed). Chat in `omp` as usual. A linked session sends each turn's final answer to Telegram, plus `ask` questions (single choice → buttons; otherwise numbered text) and tool-approval waits. Text sent to it becomes the next user message, or a steer while the agent runs. A reply to a question, or a button press, answers it. `/stop` aborts the turn, `/status` shows name · idle/running · model.
  - **Group with Topics = many sessions at once.** Each omp session gets its own topic: `ckit .` (or `/resume`) brings a conversation back to the topic it used before, with the name you gave it; a new conversation (`ckit new`) takes the repo name and reopens that saved topic (`ckit`, then `ckit 2`… only while that name is held by a live session). `/new` inside omp stays in the current topic; `/resume` moves the session to that conversation's topic and closes the one it leaves. A topic is closed when its session ends. Names are claimed through `claims/<name>.json` (created exclusively, taken over from a dead process), so sessions started at the same time get distinct names; `bysession/<omp session id>.json` remembers each conversation's topic. A topic is recreated only when Telegram says it no longer exists; other errors (429, missing rights) are retried. Text in a topic goes to that topic's session; text in General lists the linked sessions. **Private chat = one session at a time:** `/tg on` takes it over.
  - **Name your topic.** `/tg on` (and `/tg name` while linked) opens a picker: the omp session title, `<repo> · <branch>` (not on main/master/develop/dev), the repo name, this repo's earlier topics, or "✎ Tên khác…" for free input; Esc keeps the current name. `/tg name <tên>` sets it directly, with completion from the same list. A name that has a saved topic reopens that topic (history kept) and closes the current one; any other name renames the current topic in Telegram (`editForumTopic`) and moves its saved mapping. A name held by another live session is refused. Sessions that link at start still take the repo name until you pick one.
  - **Readable on the phone.** Turn results and questions go out as Telegram rich messages: the agent's Markdown renders with headings, tables, code blocks and `<details>`, up to 32K characters. Images (`![](…)`) are turned into links, or into the path as code for local files, because Telegram fetches them and a local path fails the whole send. Code is left untouched. If the server rejects a rich message, the text goes out plain, and as an `.md` file when longer than 3500 characters. An old Telegram app that cannot show rich messages is never reported back to the bot, so `/tg rich off` sends plain text everywhere (`rich` in `telegram-omp.json`).
  - **`ask` on Telegram:** each option is listed with its description, and the recommended one has ⭐ and a blue button. Short labels sit two per row. "✎ Khác…" opens a reply box for a free answer. Once answered, from a button, a reply or the terminal, the buttons become one disabled "✅ <answer>". Text you send to a session is acknowledged with a 👀 reaction (✍ when it steers a running turn) instead of a chat line; it falls back to the line when reactions are off. `/stop` and `/status` appear in the `/` menu of the paired chat.
  - **One poller.** Telegram allows one `getUpdates` per bot, so one linked session polls (`~/.config/ckit/telegram-omp/poller.lock`, created exclusively; taken over when its process dies). It hands each update to the target session through `inbox/<session>.jsonl`. Every session sends its own messages. Open questions live in `asks/<message id>.json`, so a button press handled by the poller still answers another session's open terminal dialog.
  - Setup in omp: `/tg setup [token]` (no token = the bot already used by the Board, `~/.config/ckit/telegram.json`), then send `/start <code>`: in a group's General topic (Topics on; the bot is an admin with Manage Topics) or in a private chat. Once paired, `/tg setup` in another repo only allows it and links the session (no re-pairing, existing topics untouched); `/tg reset` starts over. Only the Telegram user who paired is obeyed. Config: `~/.config/ckit/telegram-omp.json` (0600).
  - **`/tg help`** prints the setup guide in omp: create the bot with @BotFather, private chat vs. group with Topics (bot admin with Manage Topics), `/tg setup`, `/start <code>`, and how topics are created, reopened and closed. The same guide shows when `/tg setup` finds no token and on bare `/tg` before setup, instead of the one-line usage hint.
  - **Allowlist:** only sessions whose cwd is under an allowed root link (`/tg allow [dir]`, `/tg deny [dir]`; `setup` adds the cwd); allowed sessions link at start. Tool permissions stay those of the omp session (start it with `--approval-mode yolo` to never wait for approval).
  - **Status under the editor, on the quota line** (one `belowEditor` widget `ckit-footline` shared with `gw-quota.ts`: `Quota 4h ━━─── 40% · Day ━━─── 47%   ● TG @bot · ckit`; alone when no quota shows) — the bot and topic, colored by state: green `● TG @bot · ckit` = receiving, yellow `◐ TG @bot …` = connecting, creating the topic or waiting for `/start <code>`, red `● TG @bot … mất kết nối` / `bị tiến trình khác chiếm bot` = network down or another poller on the bot (e.g. the Board). No status = this session is not linked. The quota meter is drawn with box-drawing `━─` instead of `▰▱`: SF Mono has `━─` and Ghostty draws them as sprites, while `▰▱` come from a fallback font (Menlo) that Ghostty scales into a ladder.
  - **`ask` works from both sides.** For a single-choice question, the terminal shows a picker and Telegram gets buttons at the same time. The first answer goes back to the agent in the same turn, and the other side is retired: the buttons turn into "✅ <answer>" and a late press reports "expired". Waiting on omp's own dialog doesn't count against omp's 30 s `tool_call` budget, but waiting on Telegram alone would. So multi-question asks, and a picker closed with Esc, go to Telegram and end the turn; the answer then arrives as the next user message. `/tg setup` skips anything already queued for the bot, and text sent while no session was linked is skipped with one notice per batch.
- **`ckit review`: automated PR/MR code review**, similar to Claude Code's GitHub review. `ckit review <n>` reviews a PR (GitHub, via `gh`) or MR (GitLab, via `glab`; the host comes from the CI env or `origin`) and prints the result. `--post` comments on the PR/MR: one summary comment, edited in place on every push (marker `<!-- ckit-review head=<sha> -->`), plus inline comments, only for blocking findings on added lines and never posted twice (keyed per finding). Drafts, the `no-ai-review` label and an already-reviewed head commit are skipped (`--force` reviews them anyway); `--model` picks the omp model. The review only comments: it never approves or blocks a merge and never pushes.
  - **Read-only sandbox.** The PR head is checked out in a detached temp worktree. The diff is taken against the merge base with the target branch. omp runs in profile `ckit-review` with the user's `models.yml` and model roles, but no MCP servers, no xd:// devices, no memory and no auto-learn, and with only `--tools read,grep,glob`. Plain `--no-tools` is not enough: omp still exposes every MCP tool, plus `learn` and `manage_skill`. The PR text and diff go in as files marked as data. Comment text from the model cannot forge the markers.
  - **No leaks, no forged markers.** The omp process gets no token/password env vars (`*TOKEN*`, `*PASSWORD*`, `GH_*`, `GLAB_*`, `GITLAB_*`, `GITHUB_*`). Before anything is printed or posted, the values of credential-like env vars and known token shapes (`ghp_`, `glpat-`, `sk-ant-`…) are masked, and findings that point outside the repo are dropped. Markers count only on comments written by the posting identity (`gh api user`, else `github-actions[bot]`; `glab api user`), so a pasted marker can neither skip a review nor hide a finding.
  - **`ckit review init`** writes the CI job: `.github/workflows/ckit-review.yml` (GitHub Actions on `pull_request`, never `pull_request_target`; `contents: read` + `pull-requests: write`) or `.gitlab/ckit-review.yml` (GitLab `merge_request_event`, same-project MRs only, stage `test` with `needs: []`, because a pipeline made only of `.pre`/`.post` jobs is never created; installs glab; needs a `GITLAB_TOKEN` project access token with the Reporter role and `api` scope, since `CI_JOB_TOKEN` cannot comment). Both clone with full depth (needed for the merge base) and accept an optional `CKIT_REVIEW_MODEL`. Existing files are kept unless `--force` is passed.
  - **Board view "Review MR"** (`?page=board&view=mr`, the fourth tab after Bảng): the project's open MRs/PRs (≤50, newest created first), each with its labels (name + the label's color as a dot), creation time and a status (Chưa review · Đang review · Chưa đăng · Đã đăng · Có commit mới · Review lỗi) and a count of findings. Click an MR to see its last review: summary, Cần sửa / Gợi ý with `path:line`, which findings will become line comments, reviewed commit, model, posted time. **Review MR** runs in the background (read-only sandbox, never the working tree, so it neither waits for nor blocks Board tasks; the list polls while it runs). **Đăng lên GitLab/GitHub** is a separate click and is refused (409) when the MR has commits the review has not seen. CLI and Board share `.cache/ckit/reviews/<n>.json`. API: `GET /api/board/mrs`, `GET /api/board/mrs/:n`, `POST /api/board/mrs/:n/review`, `POST /api/board/mrs/:n/post`.
  - Review MR filters (kept in localStorage `ckit.mr.filters`): search (title, `!number`, author, branch), review status (chưa review / có commit mới, đang / chờ review, chưa đăng, có lỗi cần sửa, đã đăng, review lỗi), label, author, target branch, hide Draft; `n/m MR` count and Bỏ lọc. **Review several MRs at once**: tick MRs (or Chọn tất cả for the filtered list) → Review N MR. The server queues them and runs at most 5 reviews at a time across projects (new state `queued` → "Chờ review"; the list polls while any review runs or waits).
  - **MR comments in the detail pane** (`GET /api/board/mrs/:n/comments`): everyone's comments on the MR, oldest first (GitLab notes without system notes; GitHub conversation + review comments), line comments with `path:line`, resolved ones dimmed, ckit's own marked "ckit review" (marker line stripped). Loading them also records which commit ckit's posted summary covers (trusted authors only: this machine's gh/glab account, GitLab project-token bots `project_<id>_bot…` as used by the CI template, `github-actions[bot]`), so an MR reviewed and posted from CI or another machine shows **Đã đăng** (with "đăng bởi …"), and a review written without ckit (first line a heading/bold "Code Review", e.g. a team review skill; the commit it cites is matched as a prefix of the MR head) shows **Đã review (ngoài ckit)**. Status: Đã đăng = ckit's review of the current commit is on the MR; Chưa đăng = reviewed here at the current commit, not posted; Đã review (ngoài ckit) = a hand-written review that cites the current commit or no commit; Có commit mới = the last review of any kind is of an older commit; Chưa review = none. Batch review reports MRs the server refused (`N MR không gửi được: …`) and keeps them ticked; filters are kept per project (`ckit.mr.filters:<root>`).
  - Concurrency: the stored result (`.cache/ckit/reviews/<n>.json`) is written via a per-run temp file + rename, so two reviews of one MR finishing together (Board + CLI) never collide on a shared `.tmp` (last one wins); omp's stderr goes to a file instead of an unread pipe (a chatty run could block until the 25-min timeout); the Board posts one MR at a time (a second click/tab gets 409 instead of creating a second summary), and every publish (Board or CLI) holds `.cache/ckit/reviews/<n>.lock` (create_new; waits up to 20 s; a lock older than 10 min is stale) and re-reads the MR's comments under it, so two processes of one checkout never both create a summary or repeat line comments. CI runs in its own checkout and is not covered by the lock.
  - **Projects with their own `/review-pr` run exactly that command** (`.omp/commands/review-pr.md`, or `.omp/skills/review-pr/SKILL.md` when there is no command file, e.g. vtiger / pms2019). Flags in the view: **Chỉ xem** (`--dry-run`) and **Không merge** (`--no-merge`). Where the command asks the boss (before posting when blocking issues are found) the run stops with `verdict: "ask"` and the draft comment; the detail pane shows **Chờ bạn duyệt** with the draft, an optional edit box and **Đăng lên GitLab / Sửa rồi đăng / Không đăng** (`POST /api/board/mrs/:n/answer`), which resumes the same omp session (`--continue`, session kept in `.cache/ckit/reviews/<n>.session` until the run ends). Up to 5 reviews run at once. The Review MR view says so in a banner; the button becomes **Chạy /review-pr !n** (with a **Không merge** `--no-merge` toggle, also used by batch review). `board/mrs/cmd.rs` runs a normal headless omp session (full tools, the project's skills, MCP, `glab`; `--approval-mode yolo`, Board review model role) with `assets/board/review-pr.md`: read the command and its skill and execute it EXACTLY on `<n> [--no-merge]` (pre-checks, PMS ticket, knowledge/rules, business manifest, 5 agents, validators, confidence ≥80, comment template, labels, squash merge with `--remove-source-branch` when clean, PMS ticket → Testing). It runs in a temporary detached checkout of `origin/<target>` with the boss's `.omp` linked in (it is usually gitignored; a tracked `.omp` on the target wins), so the skill and rules come from the target branch, never the MR's own branch, and the working tree and Board tasks are untouched; the link is removed before the checkout. Timeout 60 min. The outcome (verdict, merged, posted, number of CRITICAL/HIGH/PERFORMANCE, message, cost, log `.cache/ckit/reviews/<n>.cmd.jsonl`) is stored in `<n>.cmd.json` and shown in the detail pane; list status: Cần fix (n) / Đã review / Đã merge / Review lỗi for the current commit. Projects without the command keep `ckit review` (read-only sandbox).
  - git/gh/glab calls run with `GIT_TERMINAL_PROMPT=0`: a remote that needs a password fails fast instead of hanging on a prompt nobody sees.
- **`ckit service`: the Board runs in the background.** `ckit service install [--port N]` registers `ckit harness web` as a per-user service (macOS LaunchAgent `ckit.board`, Linux systemd user unit `ckit-board.service`): starts at login, restarts if it dies, logs to `~/.cache/ckit/board-service.log`, uses the installer's `PATH` (omp, git, curl). `status` (default) · `restart` · `logs` · `uninstall`. With a hand-started dashboard on the port it writes the config but waits for `restart`. `ckit up` restarts an installed service on the new binary.
  - **Restarting no longer cuts a running step.** Board step runs start in their own process group (Linux unit: `KillMode=process`) and write `run.json` (pid, log, start time, the git snapshot their checks need) next to `run.pid`. A starting dashboard adopts a run whose omp is still alive: the task stays Đang chạy (thread: "Dashboard vừa khởi động lại…"), its result is recorded as usual, the project's queue keeps going, and Leader questions that were being answered are routed again. Runs that died with the old dashboard are parked as before.
- **Board inside omp (`ckit-board.ts`, deployed to `~/.omp/agent/extensions`).** In every interactive omp session: a line under the editor (`Board vtiger ▸ ⚠ 1 cần bạn (T-013) · ● T-010 Phát triển 12' · 2 xếp hàng`, or `Board: chưa chạy`), a notice when a task newly needs you, and two commands:
  - `/board`: tasks that need you, running and queued (this project when omp runs inside a Board project, else all). Per task: answer the team's questions (their options or your own text), view / approve the plan or ask for changes, Hỏi tiếp / đóng an Điều tra answer, Bắt đầu / Tiếp tục, view the error and Chạy lại, message or pause a running step; acceptance (MR) and everything else open the task's web page.
  - `/pms`: the web picker in the terminal (`ui.custom` + `@oh-my-pi/pi-tui`): tabs Ticket / Task / Report (Tab), a search box (`/`), Của tôi (`m`) and Tình trạng (`s`) filters, the list on the left (status, due, owner; loads more at the bottom) and the selected item's detail on the right (facts, description, checklist, sub-tasks, comments; PgUp/PgDn). Report: paste an ID or link; rows of a task/ticket report open their full detail. Space marks several items, `g` gives them all to the Board (one task each), `c` fills `/cook …` into the input, `o` opens the item in PMS, Enter asks Giao cho Board / Làm ở đây với /cook / Chép vào ô nhập. Same title, `<pms_source>` request and source as the web; `/cook` when the project has it on.
  - The model gets the board tools too, but every change (create, approve, message, resume) needs your confirm and is refused in a session without UI. Board step runs (`CKIT_BOARD_STAGE`) load none of it; the coordinator keeps its own tool set.
- **Board: Tạo task from a PMS report.** The PMS picker gets a third tab, **Report**: paste a report ID or link (`…record=1047`) and its rows load (500 per page, "Tải thêm", a local filter; the last 8 reports are remembered and the last one reopens). Rows of a ProjectTask / HelpDesk report open like a search result (full detail, checklist, comments); rows of any other module (timesheet, leave, contacts…) become the request as their columns, labelled and in report order, inside `<pms_source kind="record">`; the task's PMS chip links to that record. Chart reports say they have no rows. Backend: `GET /api/board/pms/report?id=&page=` (OpenAPI `getReportData`, read-only).
  - OpenAPI does not accept the CloudWORK login, so reports need the boss's PMS **Access Key**: an optional field in "Kết nối PMS", a "Thêm / Đổi / Xoá Access Key" block once connected (`POST /api/board/pms/key`, checked against PMS before it is saved), and "Dùng tài khoản từ MCP pms" imports `PMS_ACCESSKEY` too. Stored in `pms.json` (0600) as `access_key`; status only reports `report: true|false`; md5(key) and the token travel in curl's 0600 `-K` file, never argv.
- **Dashboard brand: "Ckit Harness" + new logo.** Sidebar/top-bar wordmark and tab title now read "Ckit Harness" (task tabs end in `· Ckit Harness`); the "ck" text tile is replaced by a geometric C+H connector mark (`web/src/BrandMark.tsx`, concepts B/C kept swappable) and the dashboard gets an SVG favicon with a 32px PNG fallback (`web/public/`).
- **Board: which model a task runs on, and how much quota is left.** Running task cards, the sidebar "Dự án" run line and the task page header show a compact `opus-5-5 · medium` chip (`claude-` dropped to fit a card; full model, provider, account and subagents in the tooltip); the header says "đang chạy". Each run on the task page carries its own model + effort and the subagent models it spawned.
  - **Quota** button in the Board header (next to Telegram/PMS): a small bar + % of the most-used window (5 giờ, 7 ngày…) of the account(s) the running tasks use (all accounts when nothing runs), amber above 70 %, red above 90 %. Click opens "Quota tài khoản AI": every provider account (redacted, e.g. `tu…@g…`) with each window's used %, "đặt lại sau …" countdown, plan, "Hết quota", which running task uses it, plus disabled accounts. Errors show softly and keep the last numbers.
  - Backend `board/usage.rs`: task summaries carry `run_model {provider, model, effort, credential_id}` and the detail adds `runs[i].model` (+ `subagents [{model, effort, count}]`). Read from the tail of the run log (last assistant `turn_end`, growing windows 256 KB → 32 MB because a single `agent_end` line can be ~1 MB), cached by (path, mtime, len); before the first turn ends it falls back to the session header (`model_change` + `thinking_level_change`). Subagents = child sessions `sessions/<ts>_<session id>/*.jsonl` started inside the run's time window, advisor sidecar skipped. `GET /api/board/usage` runs `omp usage --json` (OMP_DAEMON_* dropped, 20 s timeout) in a background thread at most once a minute and never blocks the request; credential ids map to accounts through omp's `auth_credentials.identity_key` (read-only, metadata columns only), emails are redacted before leaving the server.
- **Board task page rebuilt around the boss's questions (timeline + phase page).** One task reads in the fixed app shell without page scroll from 1280×800 up; each column scrolls on its own, 390 px is one column.
  - **Compact pinned region (~125 px on the task page, ~167 px on the phase page, from ~300 to 410 px before):** one header bar (← back, crumbs where the task title is the task crumb, status pill, model, a one-line row of numbers, the state's buttons and Tài liệu; an open "Nhắn" / "Yêu cầu sửa" composer drops to a full-width row under it), then one live line: `● Lập kế hoạch M0 · Leader + BA · 3 phút · đang: <latest action>` with "Xem log trực tiếp", or the gate to act on ("Duyệt kế hoạch M1. Đọc CONTEXT và PLAN rồi duyệt…"), the pause, the failure reason, or "Đã nghiệm thu… + N điểm bạn cần quyết". "Chi tiết" opens the last 5 actions, files touched, questions the Leader is answering, boss notes nobody took, the Leader's report and the REPORT.md decide list. Pause / resume / approve clicks are actions, never "Chưa có phản hồi"; a note the step took reads "Đã nhận" with its reply. Open questions for the boss stay pinned under the line.
  - **Timeline (task level):** phase rail on the left (progress, AC, tasks, runs, time, cost, rounds sent back, ACs re-graded; a card click opens that phase's page; back on the task page the card you opened stays marked: filled primary tint, ring, "Vừa xem", `aria-current`, and the timeline lands on that phase's group with a tinted header, flashed once), runs grouped by phase in the middle, decisions on the right (below the rail under 1400 px). Each group header is one row: a chevron toggle (the whole header folds / unfolds; finished phases start folded to one line), title, meta, and one "Xem phase →" button on phase groups. Each run: step name in plain Vietnamese, who, model · effort, subagents, commands and real vs harness errors, time, cost, verdict, up to 3 summary lines ("tóm tắt từ log" when the step left no message), why a review sent it back, files and commits, the questions it asked with the answer inline, notes the boss or the coordinator sent into it ("Đã nhận" / "Chưa có phản hồi"), and "Xem log". Gates sit between runs (Bạn duyệt, Điều phối duyệt thay bạn, merge). Filters: Tất cả / Câu hỏi / Trả lại / Tin nhắn. In-page actions (Xem log, Xem log trực tiếp, Báo cáo cuối, Chi tiết, Xem thêm, Tài liệu) are small secondary / ghost buttons (`.btn-sm`) with icons instead of bare text links.
  - **Phase page (`?phase=M1`):** a one-row phase switcher in the pinned region (← Tổng quan · M0 ✓ Xong · AC 18/18 · M1 ● Đang: Code · M2 ○ Chưa làm; the phase you are on is solid primary with white text), the title "Phase M1 · Read path core" with its state chip (the last crumb and the browser tab title name the phase too), goal, numbers, before/after results, then **Thực thi**: the phase's step pipeline (Kế hoạch → Phản biện → Bạn duyệt / Duyệt thay bạn → Phát triển → Kiểm thử; done ✓ with rounds, time and cost, sent-back counts, the current step highlighted with who + elapsed, the next one "Tiếp theo", plan-only tasks end on "Không có Phát triển"), a banner while nobody codes yet ("Chờ Dev (kế hoạch chưa duyệt): Leader + BA đang viết kế hoạch lần 4…") instead of "Chưa làm" on every row, and the sub-task table with headers `# · Task · File · Người làm · Trạng thái · Commit`, grouped Đang làm / Bị chặn / Chưa làm / Xong (Xong folded): one-line titles, the running task highlighted with elapsed time and the live action, blocked ones red with the reason. A row opens the sub-task drawer (`&sub=T4`: full text, files, dependencies, AC, verify, note, commit, who did it). Then **Các lần chạy**: runs grouped by step in pipeline order, each step with its attempt history (`Kế hoạch #1 Bị ngắt · #2 Đạt`) and total time and cost, approvals as their own step; each run is one card (name + attempt, who, verdict, start → end, duration, cost, model · effort, commands / errors, a red "Lý do" box when it was sent back, up to 3 summary lines behind "Xem thêm", chips that open its questions, files and commits inline, its subagents, and "Xem log"). Beside it AC (re-graded ACs with notes), questions, decisions and the phase's documents. Decisions written by the AI in PROJECT.md read "Leader".
  - **Drawers instead of tabs:** "Xem log" opens the run's log in a right drawer (human-first: prose, folded tool runs "14 thao tác: read 8, bash 3", red real errors, grey harness blocks; filters Lỗi thật / Lỗi harness / Sửa tệp, a subagent session picker, Thô JSON); Tài liệu opens the request + steps and every document. Esc closes the drawer first, then goes one level up; Back / Forward follow `?task=&phase=&sub=`. The old Trao đổi / Tài liệu / Nhật ký / Yêu cầu tabs, the resizable split, the phase kanban and the "PM theo dõi" feed on the task page are gone.
  - **"Trao đổi với Leader"** (bottom of every task page, done tasks too; refused for cancelled ones): a folding panel whose history scrolls inside it (follows new messages only while you are at the bottom), a composer that stays put (Enter sends, Shift+Enter new line), Bạn / Leader / system messages, "Leader đang trả lời…" while busy (or "Đang thực hiện: <việc>…" while a confirmed action runs), the Leader's cost per answer. Proposals are cards you edit before confirming: an **action** ("Đề xuất: Tạo merge request": editable title and instruction for the agent, a "Skill: gitlab-mr" chip with the Leader's pick or "Agent tự chọn skill", an amber "Ghi ra ngoài repo" list such as "PMS ticket #26809: ghi solution + đổi In Progress", and only when it pushes the git part: GitLab/GitHub + host, `board/t-00x` → target select from origin's branches with its default marked, the exact commits, commits that are not the task's in amber with author and time and "Commit này không phải của task, có sẵn trên nhánh gốc ở máy bạn", a required choice "Chỉ đẩy commit của task" / "Đẩy cả các commit đó"; "Xác nhận: push + thực hiện", "(nghiệm thu)" when the task waits at acceptance; then "Agent đang chạy · 2 phút · Xem log trực tiếp", and when it ends the URLs it reported ("Mở merge request ↗") and the agent's message) and **làm thêm một vòng** (the instruction; "Xác nhận: mở Vòng N"); Huỷ closes one. The header shows "Vòng N" and an `MR !42 ↗` link beside the actions; the timeline shows action runs as "Thực hiện: Tạo merge request" (live line too) and groups a revise round's runs as "Vòng N" with its instruction on top. On phones the panel sticks to the bottom of the screen.
  - **Nghiệm thu = the "Tạo merge request" action (no local merge, no board-written MR text):** at the acceptance gate the button reads "Nghiệm thu: tạo MR" and opens the action card in a drawer (draft from `GET /mr/draft`: instruction, skill, target, commits, foreign-commit choice; the skill writes the MR title and description); after "Xác nhận: push + tạo MR" the drawer shows the run live with its log, then Done with the MR link, or "Chưa có MR nên chưa nghiệm thu" with the agent's words while the task stays at the gate. While it runs the gate buttons read "Đang tạo MR…" and are disabled, as is confirming another action. A task that already has an MR shows "Nghiệm thu (MR !N đã có)" and accepts in one click. "Yêu cầu sửa" at acceptance opens the Leader chat (it can propose a new round). Every "merge" wording at acceptance is gone (steps list "Bạn nghiệm thu (tạo MR)", board card tooltip, gate text, Điều phối description).
  - Role labels follow the backend rework: the review step reads "Phản biện kế hoạch" (Reviewer, with its own seat in Phòng dự án and its own thread bubble), team questions read "Phát triển hỏi Leader" (legacy BA/PO asks and thread senders display as Leader), and the Điều phối panel describes it as your assistant (it no longer answers team questions).
  - **Backend (data for the task page; additive, old tasks keep working):** `GET /api/board/:id` runs gain `phase` (recorded when a run starts; older runs backfilled from the PM status ticks inside the run, a short run from its neighbour), `summary` (≤3 lines from the Leader's / Reviewer's thread message for the run, else the result message, else the run's closing words; `summary_source` thread/result/log; step prompts now require a non-empty `message` in the request's language), `digest {tools, errors, harness_errors, files, last_error, subagents, subagent_models, subagents_list}` (blocked grep / skipped wait / eval timeout counted apart from real errors), `commits` (task-branch commits made during the run, also after the merge) and `fail_reason`; `asks[i].run` / `thread[i].run` name the run, `thread[i].kind` (boss, leader, report, question, answer, coordinator, delegated_approval, system), `thread[i].ack` (the engine delivered the note) + `reply`, `thread[i].action` for the boss's button actions ([Tạm dừng], [Tiếp tục], Duyệt…); top-level `now` (latest PM tick of the running run, else the latest action read from the last 64 KB / 1 MB of its log, plus `recent` = its last 5 actions and `files` touched), `unanswered` (boss messages with no answer, e.g. a note the step never received; never a button action, a system line or a delivered note), `report.decide` ("Sếp cần quyết" items of REPORT.md). New `GET /api/board/:id/decisions`: PROJECT/CONTEXT Key Decisions rows (by the Leader; the boss only when the row says `user`) (an answered question folds into the row it was distilled into), the boss's gates, the coordinator's approvals for the boss, other answered questions and re-graded ACs. `/phases`: PLAN lines `T1 (sha): …`, `T1 — …` and table rows are tasks (T-004 M0 showed 0/0 instead of its 15 tasks), sub-tasks get a short `title` + full `detail` and the commit the PLAN line names; any verdict cell starting with PASS/FAIL/NEEDS-CONFIRM counts (`PASS (AC chấm lại)`: M1 was 20/20, now 23/23) and noted ACs are listed in `ac_notes`; per phase `runs`, `started`, `ended`, `cost`, `rounds_back`, `result` (before/after table rows). `/log?run=N&agent=<id>` reads a subagent session; log lines gain kind `harness`. Run logs are digested once, streamed (only message lines buffered: ~18 MB RSS after digesting T-004's 1.8 GB of logs), when the run ends, older runs on first view, cached in `<task>/digest/`; the PM's 5-minute status and the Leader's answer digest stream the log too instead of reading it whole. The coordinator's `board_task` prints `paused_by` ("new" = chưa bắt đầu).
- **Board: "Trao đổi với Leader" — chat on every task, done tasks included (backend).** `GET /api/board/:id/chat` (`{messages: [{id, from: boss|leader|system, text, at, cost, proposal?}], busy, mr}`), `POST /api/board/:id/chat {text}`, `POST …/chat/:msg/confirm {fields?: {title, instruction, target, push | include_foreign}}`, `POST …/chat/:msg/cancel`. The Leader answers from a read-only fork of the task's latest finished Dev / Test / Kế hoạch session (copied to a temp dir, `--tools=read,grep,glob --no-extensions`, the project's skills listed so it can name the right one, plan model; the original session is untouched; no session → a fresh read-only Leader with request, report, commits and thread). Turns are one at a time per task, logged and priced under `<task>/chat/` (counted in the task cost). Asked for something to be done it only proposes it (`assets/board/chat.md`); the boss confirms, optionally editing the fields:
  - **action (runs like normal omp; the model picks the project's skill):** `{kind: "action", title, instruction, skill (the Leader's pick, e.g. `gitlab-mr`), pushes, target, writes_external: [...]}` (older `merge_request`/`accept` proposals are read as the action "Tạo merge request"). On confirm the board starts a normal omp run (`assets/board/action.md`: full tools, extensions, skills, MCP, code model, `--continue` on a copy of the task's latest working session) that does it the project's way (vtiger: `.omp/skills/gitlab-mr` incl. its PMS step); the board no longer composes `glab`/`gh` commands or MR descriptions. It is a run on `task.runs` (`stage: "action"`, `title`, `logs/action-N.jsonl`, cost, `round`), `task.note` "Đang thực hiện: …" while it runs, the proposal `state: running → done | failed` with `result {url, urls, message, error}`, a thread line "Thực hiện: <title>: <agent message>"; an MR/PR URL among the result `urls` sets `task.mr {url, provider, target, at, source, head}`. Git rails stay in code: when it pushes, the target must exist on origin (else 409), the commits `origin/<target>..board/<id>` are listed on the card (`commits [{sha, short, subject, author, at, from_task}]`, `foreign`, `targets`, `remote_default`, `check_error`; `from_task` = not reachable from `task.base_sha` and committed in a run window or a board `docs: board <id>` commit) and commits that are not the task's (the boss's unpushed base commits: the real T-007 MR !434 carried one) are left out by default: the task's commits are cherry-picked onto `origin/<target>` in a temporary worktree as the board-owned `board/<id>-mr` (only the board force-pushes it; a conflict aborts cleanly with 409); `push: "all"` / `include_foreign: true` uses `board/<id>` as it is. The board pushes the MR source branch itself before the run (`board/<id>-mr` or `board/<id>`), so the agent never pushes: the prompt says "Board đã push nhánh <B>; không push, chỉ tạo MR/PR và các bước khác của skill" and "so sánh với origin/<target>, không dùng <target> local" (env `TARGET_REF=origin/<target>`; the local target may carry the boss's unpushed commits). The agent always works in a TEMP worktree on that branch (the boss's tree is never touched; removed on success, kept and named in the thread on failure) whose own config (`git config --worktree`, turning on the repo flag `extensions.worktreeConfig` once; the shared config gets no push URL) sets `remote.origin.pushurl` to `file:///dev/null/ckit-push-blocked`: every agent `git push` fails, `--no-verify` included, while `glab`/`gh` API calls work (a repo with its own `remote.origin.pushurl` cannot be covered this way; the thread says so and the next two layers remain). Second layer: a `pre-push` hook given through `GIT_CONFIG_*` env (`core.hooksPath`, the repo config is untouched; the project's own hooks are chained) that refuses any push outside `refs/heads/board/*`; after the run `git ls-remote origin` must show only board branches changed (a `--no-verify` push still fails the run as `blocked`). Preconditions (task running, gone branch, missing target, conflict, dirty tree, another turn/action running) refuse with 409 and keep the proposal open.
  - **revise ("Vòng N"):** same task, same branch: Dev → Test → Report → Acceptance re-added (workflow tasks: run → acceptance), `task.round` +1, runs carry `round`, thread line "Vòng N: …", Dev continues the latest session and gets the instruction; Dev/Test use the no-plan variants. Refused while another task holds the tree or when the branch is gone; a done task with an open MR gets its round (the MR is updated at the next acceptance).
  Telegram replies to a done task's messages go to this chat and the Leader's answer comes back on Telegram. External writes (PMS…) happen only through the project's skills in an action the boss confirmed, listed beforehand in `writes_external`.
- **Board: "Nghiệm thu" = the merge request; the board never merges locally.** At the acceptance gate the boss's acceptance commits the branch's `/feature` leftovers, resets `ACTIVE.md`/`config.json` to base's versions on the branch, puts the boss's tree back on base, then runs the action "Tạo merge request cho tính năng này vào <target>" exactly as above (the model picks the skill); the task becomes Done with "Nghiệm thu: đã tạo MR <url>" only when an MR/PR URL comes back, otherwise it stays at the gate with "Nghiệm thu chưa xong (chưa có MR): <agent message>". A pushing action confirmed from the chat while the task waits at acceptance is the acceptance. A task that already has an MR is linked at once (a later round's commits re-pushed by the board to the MR's board branch; an MR recorded before `head` existed, such as T-007 !434, is only linked). API: `GET /api/board/:id/mr/draft` (the action card: title, instruction, pushes, target/targets/remote_default/commits/foreign/check_error, `mr`); `POST /api/board/:id/approve` at acceptance takes `{target?, instruction?, push? | include_foreign?}` (an empty body with no MR yet → 400 whose body is the draft JSON; preconditions → 409; otherwise 200 with the running action run). Telegram: "🔀 Tạo MR (nghiệm thu)" previews target, the task's commit count and the left-out commits; the second tap starts the action with the defaults and the outcome is sent back. `merge_task_branch` and every local-merge path are gone; prompts (chat, cmd-run, intake, report, coordinator, the board extension) say "create the MR". Tasks without the acceptance step still end with the branch left for the boss.
- **Board: the boss keeps control of the goals (acceptance criteria).** Backend; the "Mục tiêu" view and the AC cards are in the task page.
  - **AC changes need the boss.** A step that wants to reinterpret, loosen, drop or replace an AC asks with `board_ask` `kind: "ac_change"`, `ac: {ac, old, new, reason, evidence}`. The question always goes to the boss, never times out, is not closed when its run ends, is not capped by the 5-questions limit, and has the options Duyệt / Không duyệt (Telegram buttons too). At phase close the board reads the VERIFICATION matrix: a verdict cell `PASS (<qualifier>)` (the structured sign of a re-grade or partial pass: `PASS (AC chấm lại)`, `PASS (… → staging)`; a plain `PASS` never flags, free text in other columns is ignored, a cell passing only in part such as `PASS (BE) / NEEDS-CONFIRM (FE)` now grades NEEDS-CONFIRM), or an AC whose text changed or vanished since the boss approved the plan (only when a snapshot exists), without an approved change → the phase does not close: the task waits ("Chờ bạn duyệt thay đổi AC: AC-07, AC-13 (M1)") with one card per AC. All approved → the Tester's result applies (next phase / report); any rejected → back to Dev with the boss's comment. Answers: `POST /api/board/:id/asks/:qid` with `{answer}` (starting with "Duyệt" = approve) or `{decision: "approve"|"reject", answer: "<comment>"}`. Decisions list them as kind `ac_change` by the boss. On T-004 this flags M1 AC-07/13/16 and M2 AC-12, which the team had re-graded itself.
  - **Coverage gate before Phản biện.** After a Kế hoạch run passes, the board checks the phase: every AC of the CONTEXT table is tagged by a PLAN task (`[AC: AC-01, AC-02]`, ranges `AC-01…AC-04`, `tất cả`) and every task tags an AC (`[AC: —]` only with `[tier: verify-only]` / `infra`). Gaps send the plan back to Kế hoạch (a review round spent, at most 3, then the boss's approval) with a system line listing them; the Reviewer is not run. Built in (the `/feature` skill's `coverage.py` checks an external spec against COVERAGE.md, a different thing).
  - **The coordinator approves only what the boss approved.** The boss's plan approval stores `approved_snapshot` (hashes of REQUIREMENTS UC/NFR texts and the phase's AC texts). The coordinator's `board_approve` is refused when REQUIREMENTS or the phase's ACs changed since, when the plan adds ACs from outside REQUIREMENTS (a Nguồn like "REQUIREMENTS + PO REVIEW"), while an AC change waits for the boss, or when the boss never approved a plan of the task. The refusal is a thread line + Telegram message; the plan stays for the boss.
  - **`GET /api/board/:id/goals`:** requirements (UC/NFR), every phase AC with its text, UC, source, status (pass / fail / needs_confirm / pending), note, `changed` (approved by the boss or not, card, decision), tagging tasks (`M1/T4`), their commits (PLAN sha + `feat(M1/T4)` commits) and evidence; orphans (ACs without a task, tasks without an AC); the held gate.
  - Prompts: `common.md`, `test.md`, `solution.md`, `answer.md` (the Leader always escalates AC changes) and `coordinator.md` state these rules.
  - **Task page: "Mục tiêu"** (button next to Tài liệu, `&view=goals`, `&phase=M1` filters; the phase page's AC panel links to it): one row per AC grouped by phase, AC → nội dung (UC, Nguồn) → task chips (open the sub-task) → commits → kết quả + bằng chứng, a badge "Đổi AC · bạn đã duyệt / bạn không duyệt / chờ bạn duyệt / bạn chưa duyệt" on every re-graded AC (row tinted until you approve), a "Cần xem" block at the top (re-graded ACs you never approved, ACs without a task, tasks without an AC), the original requirements folded, "Chỉ AC cần xem". Under 900 px each AC is a card.
  - **AC-change cards:** under the live line (on Mục tiêu, inside the page) one card per change: AC id, phase, who proposed it, AC hiện tại → Đề xuất side by side, Lý do, Bằng chứng, a comment box, "Không duyệt" (needs a comment: it goes to Dev with the AC) and "Duyệt thay đổi"; no "tự quyết sau 20 phút" note. The pill, the live line, the board card button ("Duyệt đổi AC") and the sidebar read "Chờ bạn duyệt thay đổi AC"; the reply composer is hidden at this gate. Decisions show "Duyệt đổi AC" / "Không duyệt đổi AC" with your comment.
- **Board: Giao việc from CloudGO PMS.** The Giao việc modal is now wide (1080 px) with two columns: Tiêu đề + Yêu cầu on the left, Quy trình + Các bước on the right. It drops to one column under 860 px. "Lấy từ PMS" next to Tiêu đề opens a picker across the full width; when no PMS account is connected it shows "Kết nối PMS" instead, which opens the PMS panel.
  - **PMS panel** (header button "PMS" with a status dot, next to Telegram): each user connects their own account. Form: Địa chỉ PMS (default `https://pms.cloudgo.vn`), Tên đăng nhập, Mật khẩu (masked, never sent back). "Kết nối" logs in and shows `Đã kết nối: <Họ Tên>`; "Kiểm tra" logs in again; "Ngắt kết nối" deletes the saved account. "Dùng tài khoản từ MCP pms" reads `~/.omp/agent/mcp.json` only when clicked, shows the username and URL it will use, and needs a second click to confirm.
  - **Filters:** Ticket / Task tabs, search by title or number (`PT12345`, `#67890`), "Của tôi" (default on, main owner) and Tình trạng (Đang mở / Đã xong / Mọi tình trạng). "Bộ lọc" opens a popover with:
    - **Danh sách:** the boss's own and shared PMS saved lists (vtiger custom views).
    - **Dự án:** the 100 most recently modified open projects.
    - **Tình trạng:** exact statuses, several at once, from the real picklist with its colours and Vietnamese labels.
    - **Mức ưu tiên**, **Phân loại** (tickets) or **Loại công việc** (tasks), and **Người phụ trách**.
  - Filters apply as you pick and show as removable chips under the toolbar ("Xoá tất cả"). They are remembered per kind in localStorage. Newest modified first, with "Tải thêm".
  - Backend: `GET /api/board/pms/meta?kind=` returns `{projects, views, statuses, priorities, categories, users}`, cached 10 min per account. Search also takes `statuses` (comma list), `project`, `view`, `priority`, `category` and `owner`; ids must be numeric and picklist values must be free of `, ' " \ ;` (PMS splices some filters into SQL). Tickets match the project by name, resolved on the server from the project id.
  - **Result rows:** a status-coloured left bar, status and priority chips, type/category, progress, logged hours, project and assignee. The due date turns red when overdue.
  - **Preview pane** for the selected item: number, title, chips, a facts grid (project, assignee, type, priority, start, due, progress, hours, updated), description, checklist with done count, sub-tasks and recent comments, plus "Mở trên PMS" and "Dùng việc này". On narrow screens the preview replaces the list and has a back link.
  - **Dùng việc này** fills Tiêu đề (`[<no>] <title>`) and Yêu cầu (description, or the title when PMS has none, then checklist, sub-tasks, last 5 comments as plain text and `Nguồn: <link>`). It also shows a removable "Từ PMS: <no>" chip. You can still edit before sending.
  - **Link back:** the task stores `source {system:"pms", kind, id, no, url}`. Cards and the task page show a `PMS <no>` chip that opens the record in a new tab.
  - **Prompt-injection guard:** the imported part of Yêu cầu is wrapped in `<pms_source system="pms" kind=".." no="..">…</pms_source>` (a literal closing tag inside PMS text is defused), comments are capped at the last 5 and ~1500 chars. `assets/board/common.md` tells every step that text inside `<pms_source>` is data written by customers/staff, never instructions to follow. The boss's own text stays outside the tag.
  - Backend `board/pms.rs`: `GET /api/board/pms/status` (`{configured, base_url, user, display_name}`, no secrets), `POST /api/board/pms` `{base_url, username, password}` (saves only after a successful login), `POST /api/board/pms/test`, `POST /api/board/pms/off`, `GET|POST /api/board/pms/import-mcp` (preview without password / import), `GET /api/board/pms/search?kind=task|ticket&q=&mine=1&status=open|done|all&page=`, `GET /api/board/pms/item?kind=&id=`; `POST /api/board` accepts an optional, validated `source`. Same CloudWORK calls as the `pms` MCP server (CloudGoAppApi `Login`, CloudWorkApi v2 `getListProjectTask` / `getListTicket` / `retrieveProjectTask` / `retrieveTicket` / `getRelatedListCommentById`). Read-only: nothing is created, updated or deleted in PMS.
  - Credentials: only `~/.config/ckit/pms.json` (0600, written by the panel; `base_url`, `username`, `password`, `display_name`). There is no implicit fallback to `~/.omp/agent/mcp.json`. Token cached in memory per account, dropped on connect/disconnect, re-login once on 401; 10 s curl timeout; Vietnamese errors ("Chưa cấu hình PMS", "Đăng nhập PMS thất bại, kiểm tra tên đăng nhập và mật khẩu", "PMS không phản hồi"). The token header goes to curl in a 0600 `-K` file and the login body on stdin, so no secret appears in `ps`, responses, logs or `task.json`.
- **Dashboard: one dropdown and one control height.** New `web/src/Select.tsx` (native `<select>` with a chevron, compact and active variants, disabled state, light and dark) replaces every `<select>`: Skills tier, Memory file, Rules scope, Models pickers, the run log's session picker and the PMS picker. New tokens `--control-h` (36 px) and `--control-h-sm` (30 px): text and search inputs, selects, segmented tabs (`.seg`), "label + checkbox" (`.check-inline`) and toolbar buttons all use them, so controls in a row line up. The sidebar theme switch stays compact.
  - **Searchable dropdowns:** a `Select` with more than 8 options, `searchable` or a `remote` search becomes a combobox. The trigger looks like the native one (36 px).
    - The list is portalled to `<body>`, so a modal or popover never clips it. It opens with its search box focused.
    - Matching ignores Vietnamese diacritics ("phuong" finds "Phường"), and matches are highlighted.
    - Keyboard: ↑↓, Home/End, Enter, Esc (closes only the list) and type-ahead (a letter on the closed trigger opens it already filtered).
    - The chosen row has a checkmark; "Không tìm thấy" shows when nothing matches. Under 480 px the list is a full-width bottom sheet.
  - `MultiSelect` (checkboxes + search + "Chọn tất cả" / "Bỏ chọn") replaces the status chips in the PMS "Bộ lọc".
  - PMS Dự án and Người phụ trách also search on the server once 2 characters are typed. Results not in the cached list show under "Tìm trên PMS", and a ticket filter on a project found that way gets its name from PMS `retrieveProject`.
  - New endpoints `GET /api/board/pms/projects?q=` (`getListProject` `projectname c`, closed projects too, 30 rows) and `GET /api/board/pms/users?q=` (the full user list, paged by 100). `q` is 1–60 characters without `' " \ ; , % _`.
- **Board** (`ckit harness web` → Delivery → Board): a virtual project room that delivers every task through your **`/feature` workflow**. You create a task and only talk to the Leader (who also acts as PO). Each step is a headless `omp -p` run with a role prompt (`assets/board/*.md`) that follows the matching `/feature` reference:
  - **Intake (Leader) = `/feature new`:** the board scaffolds `agents/planning/board-t-NNN/` (`ckit feature new`) and the Leader fills PROJECT/REQUIREMENTS/ROADMAP, cutting phases. It asks you if the request is ambiguous.
  - **Plan (Leader + `ba` agent) = `/feature plan`** for the active phase: M<x>-CONTEXT (scope, Goal, AC table) + PLAN, plan-review per config.
  - **PO review (Leader)** of the phase plan; a rejection sends it back to Plan, at most 3 times.
  - **You approve** (Gate 2; the first phase includes the ROADMAP as Gate 1).
  - **Dev = `/feature go`:** engine loop with atomic per-task commits on branch `board/<id>`.
  - **Test = `/feature ship`:** multi-lens review, tester, M<x>-VERIFICATION AC matrix, converge, close phase (knowledge map, ROADMAP, archive on the last phase). If ROADMAP has another phase, it loops back to Plan for that phase.
  - **Leader report**, then **you accept**: the branch's `/feature` leftovers (`agents/`, `docs/`, CHANGELOG) are committed — except untracked files you already had there before the task: the board never commits them, and every step's prompt lists them as off-limits (agents must stage by explicit path); that part is prompt-enforced — `ACTIVE.md`/`config.json` are reset to base's versions (your own active feature on base is untouched), and the task's merge request is created (see "Nghiệm thu" above; was a local `git merge --no-ff`). Other uncommitted edits block it with the file list instead of being swept in. Or you reply: a question is meant to return to the accept gate, and a change request adds a revision phase. Both reply paths are prompt-driven and not yet verified live.

  Every board request carries the project explicitly (`?root=`, validated against the board registry), so with the same task id in two repos an action never lands on the wrong one.
  - **All projects view** (Board default): one card per project with waiting/failed/running/queued counts, cost and task rows; a cross-project "Cần bạn" strip; each row opens its task bound to that project. "Giao việc" has a project picker (any git repo; new projects join the board). The nav badge counts waiting tasks across all projects.
  - The dashboard is now task-first: it opens on Board; the sidebar reads Board, Models, Dự án, and one ⚙ "Cài đặt & công cụ" item at the bottom that opens every other page grouped (Session, Configure, Runtime, Discover, Quality, Projects) and shows as active while you are on one (deep links still work).
  - "Giao việc" lists the steps as one numbered column in run order (number, step, who, "Bạn" on gates, checkbox on the right; off steps stay numbered and dimmed). Kế hoạch → Kiểm thử sit in a "Lặp lại cho mỗi phase" box (titled "Không chia phase (làm thẳng từ yêu cầu)" when Kế hoạch is off); Review bảo mật, Báo cáo and Bạn nghiệm thu run once after it.
  - **Per-task steps**: "Giao việc" picks which steps the task runs — presets (Feature đầy đủ · Sửa nhanh · Chỉ kế hoạch · Điều tra · Điều tra rồi sửa) or tick each step of the fixed catalog: Điều tra / Hỏi đáp → Tiếp nhận → Kế hoạch → PO duyệt → Bạn duyệt kế hoạch → Phát triển → Kiểm thử → Review bảo mật → Báo cáo → Bạn nghiệm thu. Disabled steps are skipped; dependencies are checked (PO duyệt / Bạn duyệt need Kế hoạch; Kiểm thử / Review bảo mật need Phát triển). The drawer shows the task's pipeline and lets you switch steps that have not run yet (`POST /api/board/:id/steps`; catalog + presets at `GET /api/board/steps`).
    - **Điều tra / Hỏi đáp**: the Leader answers in the thread (code intelligence, web search, `scout`), then waits: reply to ask more, or Tiếp tục / Đóng. It is read-only — the board compares `git status` before and after and fails the step if a file changed (`.serena/` and `.codegraph/` caches are added to `.git/info/exclude` next to `.cache/ckit/`) — and it may run while another task holds the project's tree; it then says it read that task's unmerged branch. A plan the PO rejects 3 times stops the task (Failed) when "Bạn duyệt kế hoạch" is off, instead of going to Dev unseen. Review bảo mật re-runs build + tests after its own fixes and reverts a fix that breaks them.
    - Without Kế hoạch, Phát triển and Kiểm thử work straight from the request (`engine_*` loop, acceptance checks derived from the request, `TEST.md`) and no `/feature` tree is created. **Review bảo mật** reviews the task's diff, fixes clear HIGH/CRITICAL issues and writes `SECURITY.md`.
    - Without Bạn nghiệm thu, the task ends after its last step and the branch is left unmerged for you (stated in the thread). Old tasks keep the full pipeline.
  - **Team questions mid-step (`board_ask`)**: a running step (Dev, Tester, …) can ask the BA, the PO (Leader) or you and keep working once answered, instead of guessing. The question goes into the task thread (`Phát triển → PO`); BA/PO questions get a short read-only session (no write/bash tools, no extensions) that answers or escalates to you; questions for you put the task in "Cần bạn" with an answer box (option chips) in the drawer, and you may also pre-empt a BA/PO answer. No answer in 20 minutes (`CKIT_BOARD_ASK_TIMEOUT`) → the step takes its stated default and marks it `(auto-decided)`; max 5 questions per run; the step timeout does not count time spent waiting. The tool lives in the engine extension and exists only in Board runs (`CKIT_BOARD_ASK_DIR`).
  - **Interject into a running step**: from the drawer, "Gửi" injects a message into the live agent session (omp `steer`, delivered at its next tool boundary — no restart, context kept); "Tạm dừng" makes it stop at a safe point (commit a verified sub-task, leave the rest, write `verdict: paused`), hard stop after 5 minutes; "Tiếp tục" resumes the SAME omp session (`--continue`) and the engine picks up at the next pending sub-task. A pending pause also releases a step waiting in `board_ask`.
  - **Coordinator (Điều phối viên)**: one standing agent per project (a persistent omp session run a turn at a time — no resident process), chat panel on the Board. **Opt-in per project**: off until you switch it on or write to it in the panel. The board then also wakes it on events: PO questions from `board_ask` (it answers or escalates instead of a fresh PO session), a plan waiting for approval, PM warnings (replacing the PM's own AI alert), and a 10-minute oversight look at each running Dev/Test step that moved. Delegated powers: answer as PO, approve plans per its policy (override in `~/.config/ckit/board-policy.md`), message/pause/resume running steps, create tasks. It never edits code and can never accept/merge (the server refuses). Tools are a separate extension (`assets/extensions/ckit-board.ts`) loaded only into its session. Messages waiting for it survive a dashboard restart.
  - **Nothing runs until you say so**: a new task waits at "Chưa bắt đầu" until you press Bắt đầu (optionally with an extra instruction), and opening the dashboard parks whatever was running or queued when it stopped (Ctrl+C, crash, reboot) as "Tạm dừng" until you press Tiếp tục. An interrupted step then continues in its own omp session (`--continue`, told to check git/engine state, not redo committed work, and which open questions were closed); its cost so far is recorded. An omp left running by a killed dashboard is stopped first (only if its command line carries that task's session dir). The coordinator can only resume steps it paused itself. Every change to a task's steps is written to its thread, and removing a step in the pipeline asks for confirmation.
  - Finished (done/cancelled) tasks are hidden in every Board view; "Hiện việc đã xong (N)" shows them (remembered per browser).
  - **Bảng = kanban of đầu mục by lifecycle**: one card per job in Chưa bắt đầu / Lên kế hoạch / Chờ bạn (the pill says what is needed: duyệt kế hoạch, trả lời câu hỏi, nghiệm thu, đang tạm dừng…) / Đang làm / Kiểm thử / Xong (with "Hiện việc đã xong"), each card with its phase and task progress; opening a card opens its task page. Replaces the column-per-pipeline-step board. The dashboard's `index.html` is now served `no-cache` so an updated `ckit` never shows a stale UI.
  - **Sidebar project monitor**: a "Dự án" section in the left menu lists every Board project with a status dot (amber = needs you, red = failed, pulsing blue = running, grey = idle), one count (amber when something waits on you, neutral when it is only failures; the dot says which), and one quiet line: what needs you (e.g. `T-001 Duyệt kế hoạch`), else what is running (`T-002 Dev đang làm · 12'`, from the run's start), else the queue. Click a project to open its board, the line to open that task. Summary in the header (`2 chạy · 1 chờ · 1 lỗi`); collapsible. A finished task whose phase never closed now says so on its page (`Đã nghiệm thu khi M0 chưa đóng (AC 13/14)`) instead of counting it done. Retries are written to the task thread with their source.
  - **Leaner menu, readable step picker**: the left menu is Board, Models, Dự án and one "Cài đặt & công cụ" item at the bottom that opens the tool pages (Skills, Memory, MCP, Codegraph…). The Workflow page is removed (the Board replaces it; `?page=workflow` opens the Board). In "Giao việc", steps are one numbered list in run order: the steps that repeat per phase (Kế hoạch → PO duyệt → Bạn duyệt → Phát triển → Kiểm thử) sit in a "Lặp lại cho mỗi phase" box; Review bảo mật, Báo cáo and Nghiệm thu run once after the last phase.
  - **Telegram**: connect a bot from the Board header (paste the @BotFather token, then send `/start <code>` to the bot; only that chat is ever answered). The board messages you when a task needs you: a new task to start, a paused task, plan approval, acceptance, a question from the team (options as buttons), intake questions, an investigate answer, a failure, and when a task is done. Buttons act directly (Bắt đầu / Tiếp tục, Duyệt kế hoạch, Chạy lại, Nghiệm thu with a second confirmation before merging); replying to a message answers that gate or question; any other text goes to the project's coordinator and its reply comes back to Telegram; `/status` lists what waits on you. Token in `~/.config/ckit/telegram.json` (0600); HTTP via `curl`; `CKIT_TELEGRAM_API` points it at another Bot API server.
  - Board spawns `omp` without inherited `OMP_DAEMON_*` variables: a dashboard started from inside an omp session made every step load that session's repo extensions (a stale engine without `board_ask`).

  One task owns the working tree from intake to acceptance (`ACTIVE.md` points at one feature), so other tasks in the same project queue with a note. The worker serves every board project in the background; the board shows other projects that have running or waiting tasks, with one-click switch. The task page shows the `/feature` documents (tables rendered) plus REVIEW/REPORT, a live log, cost, cancel, and retry. Board state lives in `<project>/.cache/ckit/board/<id>/`.
  - **Project room view**: seats for Sếp, Leader (PO), BA, Dev, Tester and PM, each showing who is working, on what, and for how long.
  - **PM**: every 5 minutes it posts a status line per running step. These lines are built from the run log at no cost and include the phase, elapsed time, number of commands, files edited and tool errors. The PM calls an AI only when a step runs past 30 minutes, stalls, or hits a burst of tool errors; that AI call runs with `--no-tools --no-skills --no-extensions --no-rules`, uses `@smol` then the default model, and backs off 30 minutes after failures. It also writes a board report when a task ends (done, cancelled or failed), or when you press the button.
  - **Plan-only tasks can go on to code**: at the approval gate of a "Chỉ kế hoạch" task you get "Duyệt và code luôn" (adds Phát triển, Kiểm thử, Báo cáo, Nghiệm thu and starts Dev) next to "Duyệt, chỉ lưu kế hoạch". A finished plan-only task whose branch is still unmerged shows "Code theo kế hoạch này": Dev picks the approved plan up on the same branch (waiting if another task holds the working tree). Telegram offers both as buttons ("Duyệt + code", "Code theo kế hoạch này").

  ⚠️ Steps run `omp --approval-mode yolo`, so agents execute shell commands and edits in the project unattended. That is what makes the background loop possible; only create Board tasks in repos where you accept that. `.cache/ckit/` is added to `.git/info/exclude` when a project's board is first used, so board state never lands on a task branch.
- Shared `ba` task agent (`assets/agents/ba.md`, deployed to `~/.omp/agent/agents/ba.md` by `ckit harness` / `ckit up`): a read-only Business Analyst on `model: "@plan"`, so it follows whatever `modelRoles.plan` you set (`ckit harness model claude+glm` sets it). The Board Leader now spawns it as `task {agent:"ba"}` in the solution meeting. Global only: a project's own `.omp/agents/ba.md` still wins and is never removed.
- Dashboard markdown renders GFM tables (AC matrices, ROADMAP) and no longer italicizes `snake_case` words.
- Dashboard **Models** page now leads with "Board dùng model nào": every Board step (plus coordinator, answerer and advisor) with the actual model omp will run, resolved through your omp `modelRoles` (e.g. `code=@task → omp task → claude-opus-5-5 · medium`), and one picker per task class offering omp roles (with their model) or a free model id. Roles/tasks tables moved under "Nâng cao"; the hardcoded Opus/GLM philosophy card is gone. `/api/models` gains `board`, `resolved`, `omp_roles`, `omp_config`, `omp_reviewer`.
- **Board runs your own slash commands**: "Giao việc" now has a **Quy trình** row with `/feature (mặc định)` and one chip per command you enabled for the project. "Quản lý" lists every command in `~/.omp/agent/commands/*.md` and `<project>/.omp/commands/*.md`; the project's file wins when both have the same name. Each row shows its description and path, and you switch it on per project. A command is never enabled on its own: you switch it on, or its file has `board: true` in the frontmatter. A `/cook` task always runs these steps: "Lên kế hoạch (/cook)" (the command runs only up to writing its plan; if the run changed any file outside `.omp/plans/`, `agents/` or `.cache/ckit/`, switched branch or committed, the step fails with "Bước kế hoạch đã sửa code / đã đổi nhánh sang … / đã commit N commit") → you approve, and the plan file is shown as `PLAN (/cook)` → "Thực hiện (/cook)" (continues the same omp session to completion and commits on the task branch; it fails if it leaves that branch; never pushes) → you accept, which merges. New `GET/POST /api/board/workflows`; `POST /api/board` takes `workflow`.
- Dashboard redesign, **"CloudGO Admin" frame with a status strip and card actions** (light "Sáng" by default, complete dark set, `cloudgo-ui-ux` palette: primary `#1B74E4`, ink `#273444`, red `#CB2134`). White sidebar rail with a hairline border: `ck` wordmark + "Đội AI của bạn", the current page as a solid blue item, the "Dự án" monitor, ⚙ "Cài đặt & công cụ" and **Giao diện: Sáng / Tối** at the bottom (top bar on phones; saved in `localStorage` `ckit.theme`, applied before first paint). White cards on a faint cool field, 12px panels, 10px cards, sentence-case section titles, gray-track segmented controls with a raised white pill.
  - **Status strip** under the Board header: one count tile per lane (Chưa bắt đầu, Lên kế hoạch, Chờ bạn, Đang làm, Kiểm thử, Lỗi, Xong), each with one hint line (`2 duyệt, 1 trả lời`, the running task ids, the total cost of finished work). A tile filters the current view (Tất cả dự án, Phòng dự án, Bảng); click it again to clear. "Chờ bạn" turns amber and "Lỗi" red only when non-zero. On phones the strip is one horizontal scroll row.
  - **Actions on cards**: a card (Bảng), a "Cần bạn" row (Tất cả dự án) and a row on the Sếp seat (Phòng dự án) that waits on you carries its button: **Duyệt kế hoạch** (approves; a "Chỉ kế hoạch" task opens its page to choose between "chỉ lưu kế hoạch" and "code luôn"), **Bắt đầu / Tiếp tục**, **Chạy lại**, and **Nghiệm thu / Trả lời / Xem trả lời**, which open the task page at the action (merge keeps its confirmation; the question box gets focus). Same endpoints as the task page; the button never opens the card.
  - **Bảng** follows the lifecycle lanes with a colored dot per lane; only "Chờ bạn" gets a tinted well and an amber count. Cards: id, title, one pill, phase progress (blue, green when complete, red on failure), cost and age. "Xong" collapses into a dashed rail with its count; click it to show the column. On phones the lanes become horizontal snap columns.
  - **App shell: the window never scrolls.** Sidebar | main column fill 100dvh; every page keeps its header (title, summary, actions, and on Board the status strip + view toolbar) fixed and scrolls only its body, with `scrollbar-gutter: stable` so a scrollbar appearing (data loading, filters, "Hiện việc đã xong") no longer shifts the layout sideways for mouse users. Bảng lanes fill the height, each lane header stays and its cards scroll inside; the board scrolls sideways when lanes overflow. The task page pins the crumbs, header and "Bây giờ" band; its columns scroll on their own (see the task page bullet above). Phones: the top bar stays, the page scrolls under it.
  - Status colors stay on status only: amber = cần bạn, red = lỗi, teal = đang chạy, green = xong, grey = everything else. All text ≥4.5:1 in both themes; no text overflow at 390px. Every color comes from tokens in `web/src/styles.css` (`[data-theme="dark"]` holds the dark set). UI copy no longer uses em dashes (empty values on tool pages read "not set").
- "Giao việc" no longer asks which project: the project search and list are gone, the modal shows `Dự án: <tên>` for the board you are viewing (or the active project from "Tất cả dự án"). Switch project in the sidebar first.

### Changed
- **Tạo task defaults to `/cook`.** The Quy trình row lists `/cook (mặc định)` first and preselects it; `/feature` is one click away. A project with `/cook` switched off falls back to `/feature` as before.
- **Board: "Giao việc" is now "Tạo task", opening on PMS.** The header and empty-state button, the modal title ("Tạo task mới") and the submit button read "Tạo task". With a PMS account connected the modal opens straight on the PMS picker; "Tự nhập" (replaces the ✕) closes it to type the task by hand, "Lấy từ PMS" brings it back. Friendlier copy in the modal, the picker ("Không tìm thấy … nào", "Dùng task/ticket này") and the PMS panel.
- **Board opens on "Bảng" (kanban).** A bare `?page=board` (and the Board nav item) now shows the current project's kanban; "Tất cả dự án" and "Phòng dự án" are one click away and still deep-link with `&view=all` / `&view=room`.
- **Board team roles (boss decision).** The **Leader** (also the BA) does Tiếp nhận, Kế hoạch, Báo cáo and answers every team question; there is no separate PO/BA answerer. `board_ask` targets are `leader` | `boss` (legacy `ba`/`po` in existing question files route to the Leader). The answer comes from the Leader who planned the phase: the session file of the task's latest finished Kế hoạch run (matched by the run log's session id, else the first session created inside the run's time window) is copied into a temp dir and continued with `omp --continue --session-dir <tmp> --tools=read,grep,glob --no-extensions --no-skills` (plan-class model), then the temp dir is deleted; the original plan session is never written, and concurrent questions each get their own copy. Without a plan session (quick tasks) a fresh read-only Leader answers. Both get every boss and Leader message of the thread plus the last 15 others, and a 4-line digest of the asking run (files edited, last tool error, last intent). Escalation to the boss is unchanged (scope, money, real data, permissions, undocumented business rule).
  - Kế hoạch (`assets/board/solution.md`, workflow `cmd-plan.md`) must record every decision with its reason and rejected alternatives in the CONTEXT `## Decisions` section (or the plan file), since later questions are answered from it.
  - The review step is now an **independent Reviewer**: label "Phản biện kế hoạch", who "Reviewer", thread sender `reviewer`; it did not write the plan and never answers team questions. Step key `review` and the review (strong) model class are unchanged.
  - The **coordinator** is the boss's assistant (chat/Telegram, status, create tasks, watch stuck/looping/costly runs, approve a plan only under delegation). It no longer receives team questions: `board_answer`/`board_escalate` and `POST /api/board/:id/asks/:qid/escalate` are removed, and `POST /api/board/:id/asks/:qid` answers as the boss only. It runs with `--tools=read,grep,glob,board_*` and no edit/write/bash.
  - Models page: "Leader trả lời team" (plan class) and "Phản biện kế hoạch" rows.

### Removed
- Dashboard Workflow page (react-flow editor that saved/exported workflows); a stale `?page=workflow` link opens Board. The `/api/workflows*` endpoints stay in the backend.

### Fixed
- Board runs that continue a session (/cook run step resuming its plan session, resume after restart, revise rounds, the chat Leader's fork, action runs) kept the resumed session's thinking level: T-007's Dev ran at `high` (the plan step's) instead of the code class's `medium`. Every Board omp call now passes an explicit `--thinking <level>` from the class's resolved model (`@role` → omp `modelRoles` value, or a model id, with its `:level` suffix; no level → omitted). `/api/models` rows (`resolved`, `board`) carry `thinking`, the level each step really runs with.
- Release builds could embed a stale dashboard: after a web-only change cargo did not recompile the crate, so `rust-embed` kept the previous `web/dist` and an installed `ckit` showed the old UI. `build.rs` now passes a content hash of `web/dist/index.html` as `WEB_DIST_STAMP`, which `assets.rs` reads, so a new bundle always recompiles and re-embeds.
- Board: a project reached through a symlink (`/tmp/x` vs `/private/tmp/x`, a linked `~/Projects`) showed up twice with split state; registered roots are now resolved and de-duplicated. A run interrupted by a restart now ends at its log's last write instead of the restart time (it showed as a 16-hour run).
- Dashboard task page jumped while you read it: each new Trao đổi message (every 2 s poll on a running task) called `scrollIntoView`, which scrolled the whole page to the thread; Điều phối did the same on every busy flip. The thread and the coordinator chat now pin only their own pane, only while you are already at its bottom, and start at the newest message; "Trả lời câu hỏi" and card actions reveal their target inside its pane. The header progress bar and the Board status strip / summary line are reserved while data loads, so pages no longer shift down after opening (task page CLS 0.15 → 0.0005). With classic (always-visible) scrollbars, filtering or switching Board views no longer moves the header 11 px sideways.
- Dashboard text inputs without an explicit `type` (Skills filter/import, MCP install, Rules import, Marketplace and Knowledge search) rendered as unstyled browser boxes; they now use the shared input style. The "Cài đặt & công cụ" item showed the current tool page name in grey on its blue active background (1.35:1); it is now readable.
- Editing files under `assets/` now rebuilds the binary: `rust-embed`'s `compression` feature does not register embedded files with cargo, so prompt/skill edits could ship stale; `build.rs` now declares `rerun-if-changed=../../assets`.
- `ckit harness web` started inside a project now opens that project (and remembers it) instead of jumping back to the last project saved in `~/.config/ckit/web-session.json`; started outside any project, it still reopens the last one.
- `ckit harness web` rejects requests whose `Host`/`Origin` is not loopback (403). Bodyless POSTs such as approve/retry/cancel were "simple" cross-origin requests any web page could fire at `127.0.0.1` (merge a task, re-run a yolo agent); DNS rebinding is blocked too.
- Dashboard project switcher listed only projects under `~`: omp names sessions of projects outside home `--a-b--` (vs `-a-b`), which the slug resolver rejected, so every repo in e.g. `/Library/WebServer/...` was missing and the sidebar fell back to showing another project. Both forms now resolve, the project the dashboard is on is always listed, and the sidebar never shows a different project than the active one.
- Board cost: a run that ended was billed onto whatever run was last at that moment, so when a second run of the task was claimed while the first was still going (a dashboard restarted mid-run), the first run's cost landed on the newer run and the runs left open were never billed (task T-001 showed $12.10 instead of $12.94; its plan run 3 showed $2.93 instead of $0.28). A run is now closed by its own log file, every run is priced from its own log (`message_end` `usage.cost.total`, a `--continue` run bills only its new turns), and the task cost is re-summed from its runs plus the BA/PO answer sessions instead of accumulated. On dashboard start, runs no process owns any more are closed as `interrupted` and the whole task is re-billed from its logs.
- Board: a run that finishes after another run of its task was claimed (or after the task left that step) no longer applies its verdict. It used to advance the task with a superseded attempt's result (T-001: the old plan run's `pass` moved the task on). It is now only closed and billed, and the thread gets one line `Bỏ qua kết quả của <stage>-<n> (đã có lượt chạy mới hơn)`; the current run's cancel/pause requests are left for it.
- Board: each run writes its verdict to its own file `<task>/results/<stage>-<n>.json` (`CKIT_BOARD_RESULT` and every step prompt via `{{RESULT}}`, the resume prompt included) instead of the shared `result.json`, so a superseded run writing late can no longer hand its verdict to the current run. The legacy `result.json` is read only when the run's own file is missing and it was written during that run (an agent following an older prompt in a continued session). A finishing run removes `run.pid` only while it still holds its own pid, so a newer run's pid stays for a restarted dashboard to stop.

## [0.1.32] - 2026-10-07

### Added
- `/feature` R16 knowledge map: `plan` reads `docs/knowledge/INDEX.md` and the map of the code area a phase touches before code-intel/scouts, and embeds the relevant excerpt in scout/dev/reviewer prompts so scouts only cover what the map lacks. Scouts report a "Bổ sung bản đồ" section that `plan` copies into the new `🗺 Bổ sung bản đồ` section of `M<x>-CONTEXT.md`. `ship` Step 3 now starts by updating `docs/knowledge/` (flow, "to do X → edit A → B → C", gotchas, verify; anchored by symbol names, never line numbers) from that section plus the phase diff, before the `docs: M<x> close phase` commit — every phase, not only at feature end, so later phases and later features stop re-scouting the same code.

## [0.1.31] - 2026-10-07

### Added
- `gw-quota` extension now also shows the Claude subscription quota when the active provider is `anthropic`: `Claude 5h ▰▱▱▱▱ 4% 4h41m · 7d Fable ▱▱▱▱▱ 0% 16h31m`, in the same belowEditor widget and `▰▱` meter as the gateway quota. It reads `omp usage --provider anthropic --json --no-extensions` (every 3 min, only while an `anthropic` model is active; gateway providers keep their `/quota` path), shows every window omp reports including model-scoped weekly buckets (which the built-in `usage` status-line segment drops), and adds a reset countdown because subscription windows roll over. `/gwquota` covers both sources. The widget reflects the first logged-in Claude OAuth account; with no OAuth login it stays hidden.

## [0.1.30] - 2026-10-06

### Fixed
- `/feature`: every "phase done ⇔ all AC PASS" statement (SKILL.md, R5, plan.md, M-CONTEXT and M-VERIFICATION templates) now points to the NEEDS-CONFIRM conditional close in `ship.md` Step 2.6 instead of contradicting it.
- `coverage.py`: a custom `Codes:` regex with capture groups now yields whole matches (was tuples from `findall`).

## [0.1.29] - 2026-10-06

### Added
- `/feature` traces an external spec end to end. `new` records the requirement source in `REQUIREMENTS.md`; when a BA spec file exists it builds `COVERAGE.md` (new template: every spec code → phase → status) and a per-phase "must read" list. `plan` reads the phase's spec sections verbatim, adds a **Nguồn** (source) column to every AC, checks coverage both ways (every phase code has an AC or a "moved to phase" row), embeds the source in plan-review, re-checks inherited Integration Contracts against the latest decisions, and reads the design for UI phases. `go` embeds the verbatim spec excerpts in dev prompts. `ship` grades against the source, adds a source column to the VERIFICATION matrix, and defines conditional close for NEEDS-CONFIRM (Step 2.6) and shipping several phases together.
- `skills/feature/scripts/coverage.py`: checks that every spec code and US·AC has a COVERAGE row (exit 1 on missing/GAP or when no code matches), and `--phase Mx` prints the codes a phase must read (ranges like `M4–M6`, letter phases like `MC`). Spec path and code regex come from anchored `> Spec:` / `> Codes:` lines in COVERAGE.md.
- feature-rules R12 (requirement source), R13 (inserting/rescoping a phase mid-feature), R14 (spec revised by BA), R15 (user decision vs AI proposal: `PROJECT.md` Key Decisions gain an "Ai quyết" column).

## [0.1.28] - 2026-10-06

### Added
- `engine_plan {append: true}` keeps the current plan (goal, tasks, statuses) and adds the new slices after it, so new tasks may depend on existing keys and `engine_status` shows the whole phase. Duplicate or unknown keys are still rejected and nothing is saved. With no plan on disk it creates one. `/feature ship` converge rounds now append instead of replacing the phase plan.

## [0.1.27] - 2026-10-06

### Added
- `/feature ship` converge step: every AC that fails acceptance is classified (missing code / wrong code / bad AC / blocked). Missing or wrong code becomes new tasks appended to the phase PLAN under `## Converge round <n>`, run through the `go` loop, then re-verified (plus a cheap regression re-run of every passing AC). Bad AC stops and asks the user; blocked items become NEEDS-CONFIRM. Bounded by `workflow.max_converge_rounds` (default 3); stops early when the same AC fails twice with the same evidence. `M<x>-VERIFICATION.md` gains a `Converge rounds` table.

## [0.1.26] - 2026-10-06

### Fixed
- `/push-now`, `/pull-now` read and write `agents/STATE.md`, `agents/KNOWLEDGE.md` and `agents/planning/` (they still pointed at the pre-rename `su-code/`).
- `harness` no longer wipes `~/.omp/agent/mcp.json` when it does not parse (hand edit with a comment or trailing comma): it warns and leaves the file untouched. Same guard for the web dashboard's "add MCP".
- `/feature go` parallel mode: the out-of-scope check subtracts files of tasks still in progress and files dirty before `go` started, since all dev agents share one working tree. `engine_status` and `engine_ready` now list the files of in-progress tasks.
- Claude Code leftovers in `/cook` and the `vtiger-php` pack: `AskUserQuestion` → `ask`, `subagent_type: "code-reviewer"` → `agent: code-reviewer`, `.claude/plans/` → `.omp/plans/`, `WebFetch` → `read <url>`; `primary-workflow` names a fallback when `librarian`/`designer` are not in the agent list.
- `harness global` removes the retired `~/.omp/skills/00-force-load.md` (superseded by `APPEND_SYSTEM.md`).

## [0.1.25] - 2026-10-06

### Changed
- `vtiger-php` pack ships its agents: `code-reviewer`, `debugger`, `tester`, `docs-manager`, `code-simplifier` install into `.omp/agents/` with the pack (the `fix`, `review-pr`, `commit`, `release-doc` skills call them). They now spawn `scout` (was the missing `explore`) and have `lsp`.
- `vtiger-php` `primary-workflow` rule uses omp's own agents and tools (`scout`, `librarian`, `designer`, `task`, `todo`, `ask`) instead of `planner`/`researcher`/`ui-ux-designer`/`fullstack-developer` and Claude Code tool names.
- `feature` skill: one "agent per role" table; `explore`/`plan`/`Tester` replaced by `scout`/`task`/`tester` (fallback `task`), code review uses `code-reviewer` when the project has it (fallback `reviewer`). Paths point at the global `~/.omp/skills/<name>/` and `~/.omp/agent/commands/auto.md` when the project has no copy.

### Fixed
- `harness` no longer overwrites a customized MCP entry in `~/.omp/agent/mcp.json`: an existing entry is replaced only when it is exactly what an earlier ckit wrote, and the old file is kept as `mcp.json.bak`.
- Pack `RULES.md` is tracked by the pack lock like every other pack file: a locally edited copy is kept (was renamed to `.bak` and replaced), and `--force` keeps a `.bak` (was overwritten with none).
- `harness global` removes the stale `~/.omp/skills/karpathy` duplicate left by older builds (previously only `skill update karpathy` did).

## [0.1.24] - 2026-10-06

### Removed
- `headroom` MCP: `harness`/`setup` no longer install or register it and drop its `mcp.json` entry; skills and commands ask for a summary of long tool output instead (omp already spills it to artifacts).

### Fixed
- `harness` reverted a tuned serena entry on every run. ckit now registers serena with `--project-from-cwd`, `--enable-web-dashboard false` and `SERENA_USAGE_REPORTING=false`.

## [0.1.23] - 2026-10-05

### Changed — skills, commands, extensions are global-only; packs have their own verb
- `harness` (init/up/global --sweep/auto) deploys commands (`/auto`, `/feature`, `/cook`, `/push-now`, `/pull-now`), the engine and workflow extensions and the bundled skills only to `~/.omp`, and moves the project's copies of them to `.omp/removed/` before injecting AGENTS.md (`.omp/commands/{auto,feature,cook,push-now,pull-now}.md`, `.omp/extensions/ckit-{engine,workflow}.ts`, `.omp/skills/<bundled>`). Project copies shadowed the global ones and were never refreshed. Exported workflows and other project files are untouched.
- `skill add builtin:<x>` installs globally only, under the same dir name as `harness` (`karpathy` → `karpathy-guidelines`); `skill update` uses that name too and removes the stale duplicate the old name mapping left behind.
- Domain packs install and update only through `skill add pack:<name>` / `skill update [<name>] [--force]` run inside the project; `harness init` no longer prompts for them (it lists uninstalled packs) and `harness up --pull` never touches them. Packs are registered in the project's `agents/skills.toml` only, never the global registry.
- Pack updates no longer clobber local edits: `.omp/pack-<name>.lock` records each file's hash as installed; a file is overwritten only while it still has that hash. Edited or untracked files are kept and listed; `--force` overwrites them and saves the old copy as `.bak`. The first update of an existing project therefore lists every file that differs from the pack.
- `vtiger-php` pack: dropped its stale copies of `feature`, `codegraph` and `karpathy-guidelines` (served by the bundled global skills) and took the newer skills maintained in the vtiger repo (`release-check` on omp MCP config, English commit messages, `testing`, `user-invokable` flags). The `error-patterns*.md` catalogs stay per-project (a project's edited copy is kept by the lock).

## [0.1.22] - 2026-10-05

### Changed — engine runs independent tasks in parallel
- `engine_plan`: tasks accept optional `key`, `depends` and `files`; unknown keys and dependency cycles are rejected. Old `state.json` files still load.
- New `engine_ready {limit}`: returns every task whose dependencies are done and whose `files` don't overlap a running task (path-prefix match), up to `limit` running tasks. `engine_next` skips tasks with unfinished dependencies.
- A blocked task now blocks every task that depends on it.
- `engine_advance {files}` stages and commits only those paths and refuses a whole-tree commit while other tasks are in progress. It commits first and marks the task done only on success; a task with no changes completes without a commit.
- `/feature` skill: plans declare `[depends:]` + `[file:]` per task instead of waves; `go` dispatches dev agents for all ready tasks and verifies each as soon as it finishes. `config.workflow.min_parallel_tasks` is replaced by `max_parallel_tasks` (default 4). MCP tool names in the skill now use the `xd://` device paths.
- Engine UI label, `/engine` messages and worktree branches are `ckit` (`ckit/<slug>`); merge/remove still find worktrees opened on `8sync/<slug>`.

### Fixed
- Engine verify commands and git ran through a login shell, where macOS `path_helper` put `/usr/bin/git` first (fails on an unaccepted Xcode license). Git now runs directly and verify commands use omp's own environment.

## [0.1.21] - 2026-09-26

### Fixed
- `ckit-code-intel.ts`: parallel grep/read calls were all blocked but only the last was remembered, so retrying any other got blocked again (repeated `code-intel-first` errors). All blocked calls are now tracked; retrying any one unlocks the prompt.

## [0.1.20] - 2026-09-25

### Changed — `gw-quota` drops the reset clock
- The ai-gateway per-key budget is now a continuously refilling bucket, so `window_reset_at`
  ("full again at") no longer marks a rollover and the `↻HH:MM` suffix misled. The widget is now
  just `Quota 4h ▰▰▱▱▱ 40% · Day ▰▰▱▱▱ 47%`; `/gwquota` drops `resets HH:MM` too.

## [0.1.19] - 2026-09-25

### Changed
- `ckit-code-intel.ts`: an exact retry of the blocked grep/read now unlocks grep/read for the rest of the prompt (0.1.18 unlocked only that one call, so each new call cost another block + retry turn). Other calls stay blocked until that retry or a code-intel call.

## [0.1.18] - 2026-09-25

### Fixed
- `ckit-code-intel.ts`: after one block, any later grep/read passed; now only an exact retry of a blocked call passes until code-intel runs.

## [0.1.17] - 2026-09-24

### Fixed
- `harness init` also seeds `tools.xdevDocs: builtins` (previously only `global`/`setup` did).
- `harness global` summary no longer claims MCP tools are top-level or that headroom auto-compresses output.

### Changed
- `CLAUDE.md` is no longer stub-created; it is only refreshed if a project already has one (omp reads `AGENTS.md`).

## [0.1.16] - 2026-09-24

### Fixed
- Hooks deploy to `~/.omp/agent/hooks/pre/` (the dir omp scans); the recall hook in `~/.omp/hooks/pre/` never ran and is removed on refresh.
- `harness global`/`setup` also seed `tools.xdevDocs: builtins` — omp ≥18 defaults to `catalog`, which silently ignored `tools.xdevInlineDevices`.
- `APPEND_SYSTEM.md` RULE #0 and the capabilities snapshot describe the real `xd://` calling convention instead of claiming MCP tools are top-level.

### Added
- `ckit-code-intel.ts` hook: blocks the first grep/read on code per prompt until codegraph/cbm/serena/lsp ran; a retry passes.

### Changed
- AGENTS.md force-load block drops the on-demand skill index (omp lists skills natively); other agent files keep the full block.
- Recall hook injects only the live STATE (no duplicate skill list); `doctor` checks `xdevDocs` and both hooks.

## [0.1.15] - 2026-09-24

### Changed — vtiger-php pack: PHP/JS rules always-on + new JS/CSS conventions
- Re-synced `php-conventions.md`, `javascript-rules.md`, `css-rules.md` from the
  live vtiger project.
- `php-conventions.md` and `javascript-rules.md` switch from TTSR edit-scoped
  loading (`scope`/`condition`) to `alwaysApply: true`, so the rules are fully
  embedded while writing AND reviewing code (stops the write-then-fix loop).
- JS: brace style matches PHP (K&R open, Stroustrup `else`); block-spacing
  rule; naming mirrors PHP — camelCase for client UI state/methods,
  snake_case only for keys mirroring backend payloads / DB columns.
- PHP: no blank spacer between an attribution `// End <Name>` marker and the
  closing delimiter it belongs to.
- CSS: Bootstrap filter/action bars must size columns for the longest
  supported language, or let buttons wrap/stack.

## [0.1.14] - 2026-09-24

### Changed — `gw-quota` shows the daily budget
- The ai-gateway `/quota` response now also carries a per-day budget
  (`daily_budget_source`, `daily_used_percent`, `daily_exceeded`) and
  `window_seconds`. The widget renders one segment per applicable budget:
  `Quota 4h ▰▰▰▰▱ 75% ↻16:59 · Day ▰▰▱▱▱ 40%` (window labelled by its length
  and shows its reset clock; the day always resets at midnight so shows none;
  each segment hidden when its budget is `none`). `/gwquota` reports both.
  Older gateways without the daily fields still render the window-only line.

### Changed — `ckit up` applies the release it installs
- After swapping the binary, `ckit up` now runs `<new binary> harness global`
  itself, so bundled skills / APPEND_SYSTEM / omp extensions (e.g.
  `gw-quota.ts`) / MCP from the new release land in `~/.omp` without a second
  command. It spawns the NEW binary (the running process still embeds the old
  assets). Idempotent: identical files skip, user-owned files untouched. If
  that step fails, `ckit up` prints the manual command. Remaining step for
  users: restart omp. README no longer claims `ckit up` runs `omp update`.

## [0.1.13] - 2026-08-27

### Changed — vtiger-php pack: comments must be English (all comments, not just attribution)
- Re-synced `php-conventions.md`, `javascript-rules.md`, `css-rules.md` from the
  live vtiger project: the comment-language rule now requires ALL code comments
  (attribution `<REASON>`, docblocks/`@param`/`@return`, inline `//`, TPL `{* *}`)
  to be English; legacy Vietnamese comments get rewritten in English when touched.

## [0.1.12] - 2026-08-27

### Changed — tighten vtiger-php pack attribution/comment rules
- `php-conventions.md`: no `@author`/`Author:` header means no defined owner —
  use `git blame`/`git log` to find who wrote a touched function; `<REASON>` in
  every `Added by`/`Modified by` comment MUST be English; comment blocks capped
  at 2 lines describing behaviour/invariant, not decisions.
- `javascript-rules.md`: renamed § to "Comments & Modification Tracking";
  defers to `php-conventions.md` attribution rules, applied identically to JS.
- `css-rules.md`: new "Comments & Modification Tracking" § applying the same
  ownership-based attribution to CSS (`/* */`) and TPL (`{* *}`).

## [0.1.11] - 2026-08-22

### Changed — sync vtiger rule/skill edits back into the bundled `vtiger-php` pack
- Re-synced all 16 pack rule files from the live vtiger project (frontmatter +
  body): `.claude/plans/` → `.omp/plans/`, "auto-loaded by Claude Code" → omp
  rulebook wording, and `command/skill` phrasing now that `cook` is a command.
- **New:** `install_pack` now deploys the pack-root `RULES.md` → project
  `.omp/RULES.md` (sticky always-apply rule), backup-on-diff so local edits are
  never clobbered. Ships with the 16 rules on every `skill add pack:` /
  `skill update`.
- **Clean cutover: `cook` skill → `/cook` command.** Removed the bundled
  `packs/vtiger-php/skills/cook` skill; added `assets/commands/cook.md`.
  `ensure_engine` deploys `/cook` (agent + project) at every harness path;
  `cleanup_legacy_gs` now also removes the retired `.omp/skills/cook`.

## [0.1.10] - 2026-08-07

### Changed — clearer post-update guidance for `ckit up`
- After a successful self-update, `ckit up` now prints an explicit "next steps"
  block: `omp update` → `ckit harness global` → `ckit doctor` → `ckit .`. Makes
  it obvious that skills / `00-force-load.md` / `APPEND_SYSTEM.md` / MCP under
  `~/.omp` do NOT auto-sync and must be re-deployed via `ckit harness global`.
- Added an explicit "already up to date" message on the no-update path.
- Fixed a stale doc comment (`ckit up -h` top line) that falsely claimed a full
  system update (omp + pacman/AUR + rustup + flatpak); `ckit up` swaps the ckit
  binary only.

## [0.1.9] - 2026-08-07

### Added — `ckit new` starts a fresh (optionally named) omp session
- `ckit .` always resumes the last session (`omp --continue`). New verb
  `ckit new` does the SAME full seeding (project root, AGENTS.md + agents/*
  memory, skills block) but execs `omp` WITHOUT `--continue`, so a brand-new
  session starts instead of reopening the old one.
- Named sessions: `ckit new <name>` starts the fresh session in omp's DEFAULT
  session dir (so it also shows up in omp's own `/resume` picker) and records
  which session file omp created in `~/.cache/ckit/named-sessions.json`; then
  `ckit . <name>` reopens exactly that session (`omp --resume <file>`). An
  unknown name falls back to `--continue` with a warning. omp has no custom-title
  flag, so the name is a ckit-side reopen label, not the title shown inside omp.

## [0.1.8] - 2026-08-06

### Added — `gw-quota` omp extension bundled
- New bundled omp extension `gw-quota.ts`: a belowEditor widget showing the
  9router ai-gateway per-key usage quota (`Quota ▰▰▱▱▱ 40% ↻HH:MM`) for the
  active model's provider, plus a `/gwquota` refresh command. It reads the
  provider's `{ baseUrl, apiKey }` from `~/.omp/agent/models.yml` and polls the
  gateway's `/quota` endpoint; non-gateway providers and unlimited keys are
  hidden, all failures are swallowed (never affects the session).
- Deploys GLOBAL-ONLY to `~/.omp/agent/extensions/gw-quota.ts` (reads global
  `models.yml`, so never mirrored into a project and never brand-namespaced).
  Rides `ensure_engine`, so it lands on every `ckit harness` / `harness global`
  / `harness init` / `harness up`. Idempotent (byte-identical skip).

### Fixed — release guide now refreshes Cargo.lock
- v0.1.7's first CI release failed: version bump didn't update the committed
  `Cargo.lock`, and `release.yml` builds with `--locked` → "cannot update the lock
  file". The release guide (CHANGELOG header + docs site) now includes
  `cargo build --release` + committing `Cargo.lock` before tagging.

## [0.1.7] - 2026-08-06

### Added — `codebase-memory-mcp` removal that survives `ckit harness`
- `ckit harness` re-installs + re-registers the STEP-0 MCP servers on every run,
  so deleting `codebase-memory-mcp` by hand never stuck. `ckit harness` now HONORS
  `disabledServers` in `~/.omp/agent/mcp.json`: list `codebase-memory-mcp` there and
  harness skips its install/registration and deregisters the `mcpServers` entry —
  the removal survives re-runs. Remove it from `disabledServers` to let harness
  manage it again.
- Reclaim disk after removing: `codebase-memory-mcp uninstall` +
  `rm -rf ~/.cache/codebase-memory-mcp` (the graph DBs can be multi-GB).

### Added — docs site renders the changelog live
- `docs/index.html` gains a **Changelog** section that fetches `CHANGELOG.md` and
  renders it (marked), plus a "Release & docs update" guide. `pages.yml` copies
  the root `CHANGELOG.md` into `docs/` on deploy, so the site is a live view of
  this file — one source of truth, zero per-release HTML edits.
- Configure section documents the 4 MCP servers: **when** they install (`ckit setup`
  once + every `ckit harness`/`harness global`, idempotent) and **how** (codegraph
  `curl|sh` · cbm `curl|bash` · headroom `headroom-ai[mcp]` via `uv` · serena via
  `uvx`), plus the stdio-per-session runtime note and `disabledServers`.

### Added — `ckit harness` now SEEDS the STEP-0 inline-tools config (omp ≥17)
- Until now `tools.xdev` + `tools.xdevInlineDevices` were a **manual** hand-edit
  of `~/.omp/agent/config.yml`; on omp ≥17 `ensure_mcp_tools_visible` just
  no-op'd. A fresh machine therefore lacked the inline-schema optimization, so
  the model never auto-reached codegraph/cbm/serena (the "STEP-0 defined but
  never called" gap). `ensure_mcp_tools_visible` now seeds, idempotently:
  ```yaml
  tools:
    xdev: true
    xdevInlineDevices:
      - "mcp__codegraph_explore"
      - "mcp__codebase_memory*_search_graph"
      - "mcp__codebase_memory*_trace_path"
      - "mcp__codebase_memory*_get_architecture"
      - "mcp__codebase_memory*_get_code_snippet"
      - "mcp__codebase_memory*_search_code"
      - "mcp__serena_find_symbol"
      - "mcp__serena_find_referencing_symbols"
      - "mcp__serena_get_symbols_overview"
  ```
  Key-presence idempotent: never overrides a user-authored `xdevInlineDevices`,
  inserts UNDER an existing `tools:` block so a user's `approvalMode` survives.
  Runs from `ckit harness`, `ckit harness global`, and `ckit setup`.
- **Manual, NOT seeded (by design):** `tools.approvalMode: yolo` (auto-approve
  every tool call). Auto-approving `bash`/deletes machine-wide is an
  autonomy/security choice — opt in yourself by adding `approvalMode: yolo`
  under `tools:` in `~/.omp/agent/config.yml` if you run unattended (L3) loops.

### Removed — dead pre-17 MCP-visibility path
- omp has been ≥17 for a long time, so the pre-17 branch of
  `ensure_mcp_tools_visible` was unreachable dead code: the
  `mcp.discoveryDefaultServers` seeding (gen-2) and the byte-exact
  `tools.essentialOverride` migration (gen-1 cleanup). Dropped both; the
  function is now a single `tools.xdevInlineDevices` seeder.

### Fixed — APPEND_SYSTEM overstated codegraph's MCP surface
- RULE #0 claimed `mcp__codegraph__codegraph_explore` (wrong: double-underscore;
  omp's actual id is single `mcp__codegraph_explore`) plus "the rest of
  CodeGraph's MCP catalog — callers/callees/impact/status". Grounded `tools/list`
  on the installed codegraph shows it registers **exactly one** MCP tool,
  `codegraph_explore`; callers/callees/impact/node/query/status are `codegraph`
  **CLI verbs** (run via bash), not MCP tools. The bad guidance made the agent
  invent non-existent `mcp__codegraph__callers`-style calls and give up.
- Corrected both lines: name the sole MCP tool correctly, and route precise
  single-slice lookups to the `codegraph` CLI via `bash`. Redeployed to
  `~/.omp/agent/APPEND_SYSTEM.md` via `ckit harness global`.

### Changed — de-duplicate the always-on rulebook (one source of truth)
- The code-intel-first / STEP-0 rulebook was restated in 3 always-on places
  (`APPEND_SYSTEM.md`, the injected AGENTS.md block, and `00-force-load.md`),
  burning ~8k tokens/turn of duplicated system-prompt text without changing
  agent behavior. Now `~/.omp/agent/APPEND_SYSTEM.md` is the **single** always-on
  rule source (loop C/D/E · doc-hygiene · skill discipline migrated into it); the
  AGENTS.md sentinel block is slimmed to a **pointer + the project skill index**
  (force-load prefix ~659 tok, down from the full rulebook).

### Removed — dead `00-force-load.md`
- omp 17 never loaded `~/.omp/skills/00-force-load.md` (not a skill dir with
  `SKILL.md`, and the recall hook lists only directories) — it was written by 3
  code paths and read by none. Dropped the emitters (`harness init`/`global`/
  `setup`), the `doctor` check, the bundled asset, and every help/status line that
  named it; all now point at `APPEND_SYSTEM.md`.

## [0.1.6] — 2026-08-04

### Fixed — `ckit setup` no longer loses `models.yml` when codegraph fails (Windows)
- On Windows the `codegraph` install could fail, and because it was a hard step
  in strict `ckit setup` it aborted the run **before** `models.yml` was seeded —
  leaving both `models.yml` and `codegraph` missing. `install_codegraph` is now
  **best-effort** (warns, never aborts), and Stage A seeds local config (PATH,
  configs, `models.yml`, `config.yml`) **before** the fallible tool installs, so
  the omp config always lands. Retry codegraph any time with `ckit harness`.
- The fish PATH-bootstrap file now follows the active brand namespace via
  `brand::ns_file` (`ckit-path.fish`), matching every UI message; a stale
  legacy `8sync-path.fish` from an older build is removed.

### Changed — docs restructure install by platform + daily use centred on omp
- `docs/index.html` splits Quick start into side-by-side **macOS/Linux** vs
  **Windows (PowerShell)** cards, reframes daily use around opening `omp`
  (`ckit .` → `/auto` / `/plan` / `/feature`), and demotes harness/`ckit` verbs
  to one-time setup + optional shell shortcuts.
- Removed the Dashboard (`ckit harness web`) section, its nav link, and the
  command-table row; added a "verify the install" block (`ckit doctor`,
  `ckit --version`, `omp --version`, `codegraph --version`, `gh auth status`)
  so Install sits directly next to How-to-use.
- Added a **Configure &amp; verify** section (docs + README): the omp config files
  (`config.yml`, `models.yml`, `models.toml`, `mcp.json`); setting the gateway
  **API key + endpoint URL** with `ckit harness gateway key <KEY>` /
  `ckit harness gateway url <URL>` / `verify`; verifying MCP with `/mcp list`
  (macOS/Linux: all four; Windows may skip codebase-memory-mcp); and migrating a
  Claude Code project via `omp --from-claude` into per-project Mnemopi.

## [0.1.5] — 2026-08-04

### Changed — docs explain setup, daily use, and feature workflow
- `docs/index.html` now separates first-time machine/project setup from daily commands,
  explains why each common command exists, and documents starting a long task with
  `/plan @assets/skills/feature/SKILL.md` plus the `/feature new|plan|go|ship|status`
  flow.
- `assets/skills/feature/SKILL.md` and `ckit feature -h` now use the public
  `ckit feature` command name for deterministic feature helpers.

### Fixed — `ckit setup` / `ckit harness` install the AI core on native Windows
- `install_omp` and `install_codegraph` no longer shell out to POSIX `sh` on
  Windows (which aborted `ckit setup` before it did anything). They now install
  the npm packages `@oh-my-pi/pi-coding-agent` (omp) and `@colbymchenry/codegraph`
  via `bun add -g` / `npm install -g`, resolved through `which` so `npm.cmd` is
  found. Shared helper `deploy::install_node_pkg`.
- `ensure_codegraph` (harness path) and `ensure_uv` gained matching Windows
  branches (npm for codegraph; uv's PowerShell installer for headroom/serena).
- `ensure_codebase_memory_mcp` now registers the MCP server only when the binary
  is actually present and purges any stale entry otherwise, so a skipped/failed
  install no longer leaves omp with a broken MCP command (Windows + any host).
- Docs (`README.md`, `docs/index.html`) document the Windows path: `install.ps1`
  one-liner + the `bun`/`npm`/`winget`/`uv` prerequisites for `ckit setup`.

## [0.1.4] — 2026-08-04

### Fixed — self-update chooses the correct platform asset and refuses bad downloads
- `ckit up` no longer hard-codes the Linux x86_64 release asset. It now resolves
  the same platform suffixes as `install.sh` (`linux-x86_64`, `linux-aarch64`,
  `darwin-x86_64`, `darwin-arm64`, `windows-x86_64.exe`).
- The updater now chmods the downloaded temp asset, runs temp `ckit --version`,
  and only renames it over `~/.local/bin/ckit` when the asset is executable on
  the current machine and reports the expected version. This prevents a wrong-OS
  or wrong-version release asset from bricking the installed command.

## [0.1.3] — 2026-08-04

### Added — Lark (larksuite) to the `apps-personal` profile
- `assets/profiles/apps-personal.toml` now installs Lark via AUR `larksuite-bin`
  (v7.66.11, URL larksuite.com) alongside Bitwarden. AUR bin package auto-fetches the
  official `.deb` at build time and is pacman-tracked — the 437 MB binary is NEVER
  committed to the repo/GitHub, only the download reference. Sets `requires.aur_helper`.
  China build (Feishu) is a separate package, `feishu-bin`. Opt-in; the `alexdev`
  bundle intentionally excludes `apps-personal`.

### Fixed — finish the public `8sync` → `ckit` rebrand for shipped install paths
- User-facing docs and CLI help no longer point to the legacy `8-Sync-Dev/su-code`
  repository or `sync8`/`8sync` command names for the rebranded binary. Root help,
  flow help, `README.md`, `docs/index.html`, web package metadata, and install scripts
  now use `dante241/ckit_agent` + `ckit` consistently.
- `scripts/alexdev-install.sh` now installs and invokes `ckit`, writes the profile
  override under `~/.config/ckit/profiles`, and calls `ckit setup --profile alexdev`
  / `ckit doctor`. The bundled skill ID intentionally remains `8sync-cli` because
  `deploy.rs` and `inject.rs` use it as the stable always-on skill identifier.

### Fixed — STEP-0 MCP tools were connected but NEVER called (0.09% usage) → always visible
- Root cause (measured over 29 omp sessions / 13,854 tool calls: serena 0 · headroom 0 ·
  cbm 10 · zai 3 calls; `search_tool_bm25` taken 2×): omp's default
  `tools.discoveryMode: auto` hides ALL MCP tools behind a `search_tool_bm25` discovery
  hop once the registry exceeds 40 tools (this stack registers 48), and every instruction
  surface taught BASE tool names (`search_graph`, `find_symbol`) that are not callable —
  the registered forms are `mcp__codebase_memory_mcp_search_graph`, `mcp__serena_find_symbol`, ….
- Fix: `8sync harness`/`harness global` now writes `mcp.discoveryDefaultServers:
  [codebase-memory-mcp, headroom, serena, zai-vision]` into `~/.omp/agent/config.yml`
  (`ensure_mcp_tools_visible` in `crates/cli/src/verbs/skill/deploy.rs`) — the four
  harness servers' full catalogs stay in the active tool set, zero discovery hops.
  Key-presence idempotent; never overrides a user-set value. NOTE:
  `tools.essentialOverride` does NOT work for MCP (omp filters entries to built-in
  tool names only — verified in omp 16.4.8 bundle); an inert pin block from that
  approach is auto-migrated away (byte-exact match only).
- Instruction surfaces now teach the REGISTERED `mcp__…` names + the
  `search_tool_bm25` escape hatch for other/new servers: `APPEND_SYSTEM.md` RULE #0,
  `00-force-load.md`, the AGENTS.md STEP-0 sentinel template (`inject.rs`), the
  capabilities catalog header, and the feature-skill R10 literals (assets + su-code
  mirror). Stale names dropped everywhere: `semantic_query` (never existed on cbm),
  `codegraph search/deps/defs` (real 1.1.2 verbs: `query/explore/node/callers/callees/impact`).
  Unfollowable headroom mandate ("compress before it enters context") rewritten to the
  followable form: compress what YOU re-emit (`mcp__headroom_compress`).
- `8sync doctor` now flags hidden MCP tools (`discoveryDefaultServers` missing) and a
  dead serena (mcp.json + uvx check). Live probe verified: `omp -p` calls
  `mcp__codebase_memory_mcp_search_graph` and `mcp__serena_find_symbol` directly — both OK,
  no discovery hop; toolstats records serena/cbm optimizer calls for the first time.

### Fixed — omp ≥17 dropped bm25 discovery; the recurring "MCP HIDDEN" was a phantom
- omp 17.x replaced the pre-17 tool-discovery model (the `search_tool_bm25` hop +
  `mcp.discoveryDefaultServers`) with `tools.xdev` (default on): MCP tools mount as
  `xd://mcp__…` device URLs, callable via read/write without shipping schemas every
  request. `discoveryDefaultServers` is gone from omp's settings schema — writing it is
  a no-op that omp strips on the next config rewrite. That churn is exactly why STEP-0
  looked like it "regressed" after every omp self-upgrade (upgrade resets `config.yml`
  → doctor's string-check screamed HIDDEN → re-harness → repeat), while the tools were
  callable all along (verified live via `xd://mcp__codebase_memory_mcp_*` this session).
- `ensure_mcp_tools_visible` (`deploy.rs`) now detects omp ≥17 and skips the dead key
  (nothing to configure — `tools.xdev` mounts them); pre-17 keeps the discoveryDefaultServers write.
- `8sync doctor` MCP check is omp-version-aware: omp ≥17 reports the xd:// mount (OK)
  instead of the false "HIDDEN behind search_tool_bm25" warning; the `harness global`
  summary bullet updated to match. New `env_detect::omp_major()` parses omp's major version.

### Added — omp `/push-now` command (cross-machine handoff + commit + push)
- New embedded command `assets/commands/push-now.md`, deployed by `8sync harness`
  (`ensure_engine` in `crates/cli/src/verbs/skill/deploy.rs`) to
  `~/.omp/agent/commands/push-now.md` (global) + `<repo>/.omp/commands/push-now.md`
  (project), alongside `/auto` and `/feature`. `/push-now [msg]` is the
  "I'm switching machines right now" verb: it rewrites `su-code/STATE.md` with a
  cold-resume handoff (branch/HEAD, what changed this session, done/next/blockers,
  new-machine runbook), updates `CHANGELOG.md`/`KNOWLEDGE.md` if code changed, then
  `git add -A` + commit (gitleaks-gated, no `--no-verify` past a real secret) +
  `git push` origin current branch. No PR, no branch switch, no tag bump, no
  force-push (that's `8sync ship` / a release). Since it's in `assets/`, a fresh
  machine gets it automatically after `git pull && bash scripts/bootstrap.sh && 8sync harness`.

### Added — omp `/pull-now` command (cold-resume: pull + orient + prepare)
- New embedded command `assets/commands/pull-now.md`, deployed by `8sync harness`
  the same way (`ensure_engine`) to `~/.omp/agent/commands/pull-now.md` +
  `<repo>/.omp/commands/pull-now.md`. `/pull-now [go]` is the **receiving end of
  `/push-now`**: safe `git pull` (ff-only → rebase on divergence; stop on conflict
  or dirty tree), then read `su-code/STATE.md` HANDOFF + recent `KNOWLEDGE.md`
  learnings + `CHANGELOG.md`/`git log` to understand exactly where the project is,
  prepare the workspace (rebuild + `8sync harness` if `crates/`/`assets/` changed,
  verify the per-machine gotchas the handoff listed, `8sync doctor`), and report
  current state + the single next concrete action. `go` = start that action;
  empty = orient + prepare, then STOP for the human. No push, no clobber.

## [0.52.0] — 2026-07-09

### Added — `8sync vpn`: SoftEther VPN Client + VPN Gate (study-through-another-region)
- New top-level verb `vpn [install|gui|list|on|off|status]` (`crates/cli/src/verbs/vpn.rs`).
  Connect through **VPN Gate** (University of Tsukuba academic public relays) the
  way the Windows client does. `install` pulls the native Linux engine
  `softethervpn` (the maintained RTM 4.44 build — **not** the `-git` 5.x dev
  edition) + the **Windows VPN Client Manager GUI under Wine**
  (`softethervpn-client-manager`, where the Windows-style region-switch plugin
  lives; `--no-gui` skips it) + `dhcpcd`, and enables the client service.
  `gui` opens that manager. SoftEther has **no native Linux GUI** and its Linux
  client **can't rewrite the routing table itself**, so the reliable region-switch
  is the CLI: `list [CC]` ranks relays from the VPN Gate CSV API (optional
  2-letter country); `on [CC|ip]` picks the best relay, connects via `vpncmd`
  (HUB `VPNGATE`, user/pass `vpn`), **pins the relay route to the physical uplink**,
  DHCPs the tap, full-tunnels the default route, swaps DNS to 1.1.1.1, and
  **auto-rolls-back if egress doesn't change** (egress checked via Cloudflare's
  IP-addressed trace so it survives the DNS swap); `off` restores routes/DNS.
  VPN Gate relays are volunteer-run and **logged** — a learning tunnel only.

## [0.51.0] — 2026-07-09

### Added — `8sync feynman auth-omp`: reuse omp's auth in Feynman
- New top-level verb `feynman [auth-omp|status|off]` (`crates/cli/src/verbs/feynman.rs`).
  Feynman (companion-inc/feynman) is a Pi research agent that expects its own
  `feynman model login`; omp is a fast-moving Pi fork with a fresh model catalog +
  a credential vault. Both read `<home>/agent/auth.json` in the SAME schema, so
  `auth-omp` mirrors omp's LIVE credentials into `~/.feynman/agent/auth.json`:
  OAuth providers (Claude Pro/Max) as `{type:oauth, access:<omp token>}` **without
  the refresh token** — omp stays the sole refresher (no dueling token rotation),
  re-run when it expires; API-key providers as `{type:api_key, key:"!omp token <p>
  --raw"}` so keys resolve live from omp (no secret copied). A sidecar
  (`.8sync-omp.json`) records managed providers so `off` removes only those and
  never touches Feynman's own logins. Verified live (feynman 0.3.5 + omp 16.4.6):
  after `auth-omp`, `feynman model list` shows `anthropic/claude-opus-4-8` + `zai/*`
  (31 authed models) reusing omp's Claude OAuth; `off` reverts to 0 authed.
  Note: Claude Pro/Max OAuth via a third-party harness draws subscription
  extra-usage (billed per token).

## [0.50.0] — 2026-07-09

### Added — `8sync harness browser`: omp browser control that reaches the internet
- New `harness browser [fix|status|off]` (`crates/cli/src/verbs/harness/browser.rs`).
  omp's Puppeteer browser control could render but **fail to reach the internet** on
  the bundled `chrome-headless-shell`. `fix` (default) ensures **ungoogled-chromium-bin**
  is installed (`/usr/bin/chromium`) and exports `PUPPETEER_EXECUTABLE_PATH` +
  `BUN_CHROME_PATH` (the vars omp/Bun honor, with `--no-sandbox`) in zsh/bash/fish so
  every omp launch — direct or via `8sync .`/`8sync ai` — uses it. Idempotent
  (sentinel-managed rc block); `off` reverts to the bundled chromium, `status` shows
  the wiring. Verified: `/usr/bin/chromium` fetches pages headless; interactive
  bash/zsh resolve the exported path.

### Fixed — omp `/new` no longer lands in the wrong project root
- omp's `/new` creates a child session that **inherits the launch root** (it does NOT
  re-detect cwd), so a drifting cwd made `/new` open in the wrong project. `8sync .`
  and `8sync ai` now pin omp to the detected project root via omp's `--cwd <root>`
  flag (+ `current_dir`), so the session — and every `/new` child — is correctly
  scoped. `8sync ai` previously launched omp in the ambient cwd with no root pin.

## [0.49.1] — 2026-07-09

### Fixed — `add-model --think` now exposes a model's FULL reasoning range
- `--think` defaulted to only the efforts the user typed and always wrote
  `mode: anthropic-budget-effort` — so a reasoning model like `xai/grok-4.5` got a
  truncated set in omp's `/model` thinking picker instead of the native
  `minimal · low · medium · high · xhigh`.
- **Bare `--think`** (or `full`/`all`/`max`) now emits the **complete canonical tier
  set** `[minimal, low, medium, high, xhigh]` — matching what a native grok/claude
  reasoning model exposes. A subset (`--think "min,high"`) is mapped to canonical
  tiers and reordered; `--think off` disables. `defaultLevel` = `high`.
- **`mode` now follows the API**: `effort` for `openai-completions` (the generic
  `reasoning_effort` wire param — correct for xAI/OpenAI/most), `anthropic-budget-effort`
  for `anthropic-messages`. (omp's valid modes: `effort|budget|google-level|
  anthropic-adaptive|anthropic-budget-effort`; the flat `thinking: [..]` list form is
  rejected as input — the nested `mode`/`efforts`/`defaultLevel` block is required.)

## [0.49.0] — 2026-07-09

### Added — `8sync harness add-model`: register a REMOTE model omp's catalog lacks
- **New `harness add-model <provider/model> --url <baseUrl>`** (`crates/cli/src/verbs/harness/custom_model.rs`).
  For the case where omp hasn't shipped a model yet — or lists it with **null metadata**
  (e.g. a brand-new `xai/grok-4.5` shows `context: -`, `max-out: -`): register it as a
  full custom provider in `~/.omp/agent/models.yml`, so it appears in `/model` and routes.
- **Empirically grounded** (`omp` 16.3.12): a metadata-only merge into a built-in provider is
  **rejected** (`"baseUrl" is required when defining custom models`), so `--url` is mandatory;
  the model selector omp exposes is `<providerKey>/<modelId>`.
- Flags: `--url` (required) · `--key` (else `$<PROVIDER>_API_KEY`, else a visible placeholder) ·
  `--api openai|anthropic` (default openai-completions) · `--ctx <N>` (default 256000) ·
  `--max <N>` (default 32000) · `--vision` (adds image input) · `--think "min,low,med,high"`
  (emits a valid `anthropic-budget-effort` thinking block + marks the model reasoning-capable).
  Sub-verbs `list` / `rm <provider/model>`.
- **Source of truth** = TSV registry `~/.config/8sync/custom-models.tsv`; regenerates a
  sentinel-managed block that **coexists** with the local-models block and the 9router gateway
  providers (each `sync` strips only its own block; `gateway apply` now re-attaches both).
  Models sharing a provider are grouped under one provider key (YAML forbids dup keys).
  After writing, `add` calls `omp models --json` to **verify the config still loads** (a bad
  `--think`/`--api` combo is a loud warning, not a silent broken file).
- `add-model` was previously an undocumented alias of `add-local-model`; it now means the
  remote path. GGUF stays on `add-local-model`.

## [0.48.0] — 2026-07-08

### Added — large-scope `feature` framework (GSD) + the `8sync feature` verb
- **`feature` skill + `/feature` command** (bundled, deployed by `harness`): ports a
  spec-driven GSD framework into su-code for LARGE multi-phase scopes — a planning
  tree at `su-code/planning/<slug>/` (`PROJECT`/`REQUIREMENTS`/`ROADMAP`/`STATE` +
  per-phase `M<n>-{CONTEXT,PLAN,VERIFICATION}`), an `ACTIVE` pointer to switch between
  features across sessions, and an acceptance-criteria (AC) contract. Phase execution
  (`/feature go`) delegates to the existing `engine_*` verify-gate loop — it adds no
  new execution engine. Small/single-concern work still uses `/auto`.
- **`8sync feature` verb** (deterministic, no model): `new <slug>` scaffolds the
  planning tree from bundled templates + activates it; `switch <slug>` flips `ACTIVE`;
  `status` prints the active STATE position; `list` shows features (★ active) +
  archived. `plan`/`go`/`ship` need model judgement → run via `/feature` in omp.

### Added — single-source CLI command name (`brand.rs`) — rebrand in one place
- New `crates/cli/src/brand.rs` is the single source of the CLI identity: `CMD` (the
  invoked command name) + `NS` (on-disk config namespace, AGENTS sentinels, deployed
  artifact filenames), both defaulting to `8sync` (or set `SC_CMD`/`SC_NS` at build
  time). A rename now propagates to clap `name`/`bin_name`, every help/EXAMPLES block
  (one `rebrand` pass over the clap command tree), all runtime `8sync <verb>` prose
  (routed through `brand::render` at the `ui::*` chokepoint + the cheatsheets), config
  paths (`~/.config/<NS>`), `kitty/<NS>.conf`, AGENTS.md sentinels, and deployed
  `<NS>-recall.ts` / `<NS>-engine.ts` / `<NS>-workflow.ts` / `<NS>-harness-up` names.
- The default build is **byte-identical** to before (`render` is an identity function
  when `CMD == NS == "8sync"`; `8sync {help,flow}` diff empty). A rebuilt
  `SC_CMD=sc SC_NS=sc` binary emits `sc …` everywhere with zero stray `8sync` command
  tokens and `Usage: sc`.
- Excluded from the rebrand (fixed identifiers): the `8-Sync-Dev` org + `github.com`
  self-update URLs, the bundled `8sync-cli` skill dir, and the `.cache/8sync/`
  namespace (derived/gitignored; the verbatim `8sync-engine.ts` extension couples to
  it — rendering `.ts` code is out of scope).
- A one-time migration shim (runs in `setup`/`harness`/`doctor`, only when rebranded)
  renames `~/.config/8sync` → `~/.config/<NS>` + `kitty/<NS>.conf`, removes stale
  `8sync-` deployed artifacts, and `skill::inject` recognises the legacy `8sync:`
  sentinels to self-heal existing `AGENTS.md` files in place.

### Added — dashboard Knowledge browser + Create-Project
- **Knowledge page** (`Discover → Knowledge`): auto-fetches `sindresorhus/awesome`
  (raw README via `curl`, cached `.cache/8sync/knowledge/` 6h TTL), parses it into
  categories → entries, and lets you browse/search, multi-select, and **Save to
  project** → appends curated links to `<project>/su-code/REFERENCES.md` (deduped
  by URL). Backend `crates/cli/src/verbs/harness/knowledge.rs` + `GET /api/knowledge`
  + `POST /api/knowledge/apply`.
- **New Project** button (`Projects → Workspaces`): a modal scaffolds a fresh
  8sync project — `mkdir` + `git init` + AGENTS.md/su-code memory/skills block —
  and applies the chosen extras: skills vendored into `su-code/skills/`, MCP servers
  into the project's `.omp/mcp.json`, and knowledge into `su-code/REFERENCES.md`,
  then activates it. Backend `here::scaffold_project` + `POST /api/projects/create`.

### Added — `harness model` two-model combo preset
- `8sync harness model <strong>+<cheap>` (or `model=claude+glm`) sets **every omp
  role** across two providers in one shot, the cost-optimal split: cheap model
  does the mechanical bulk (`default`/`task` high · `smol`/`tiny`/`commit` minimal ·
  `advisor`), strong model does the thinking (`vision`/`slow` high · **`plan`/
  `designer`/`reviewer` xhigh**). Writes omp `~/.omp/agent/config.yml` `modelRoles`
  + `task.agentModelOverrides.reviewer` (line-based, preserves every other key)
  and keeps `~/.config/8sync/models.toml` in sync.
- Aliases: `claude`/`opus` → `anthropic/claude-opus-4-8`, `sonnet` → `claude-sonnet-5`,
  `glm`/`zai` → `zai/glm-5.2`, `haiku` → `claude-haiku-4-5`; a bare `provider/model`
  passes through. `vision` routes to the strong model because `glm-5.2` is text-only.
- `xhigh` is valid on **direct** `anthropic/*` (the 9router gateway caps at `high`).
- General `key=value` shorthand added to `harness` dispatch (`compaction=50` etc.).
- `harness model` view now also prints omp's live `modelRoles` (not just models.toml).

## [0.47.0] — 2026-07-06

### Added — cross-platform builds: macOS + Windows (was Linux/Arch-only)
- **New `crate::platform` module** — the OS seam. `platform::os()` (compile-time
  constant per target), `os_name()`, `require_linux()` (clean no-op guard for
  Linux-only verbs), `pkg_manager()` (pacman ⁄ brew ⁄ winget), `install_core_pkg()`
  (per-manager package-id map), and a **cross-platform periodic timer** —
  `install_timer`/`remove_timer` backed by **systemd user timer (Linux) ⁄ launchd
  LaunchAgent (macOS) ⁄ Scheduled Task (Windows)**. Uses only cross-platform
  std/crate APIs (no `std::os::unix`) so one body compiles on every target.
- **Portable prebuilts:** dropped `target-cpu=native` from `.cargo/config.toml`
  (it baked the build host's ISA into the binary → SIGILL on older CPUs of the
  same arch). Release binaries now run on any CPU of the target arch.
- **`harness up --timer` + `clean --timer`** now route through `platform::*` —
  identical systemd behavior on Linux (incl. the 0.46.2 cgroup memory bounds),
  launchd `StartInterval` on macOS, `schtasks /SC MINUTE` on Windows.
- **`setup` Stage A is cross-platform:** `gh` installs via the native package
  manager (`github-cli` on pacman, `gh` on brew/winget); `paru`/AUR is skipped
  off-Linux; Arch-only Stage B profiles are skipped with a clear note on
  macOS/Windows. `omp`/`codegraph` keep their curl installers (POSIX shells).
- **`sec`, `bt`, `clean`** guard with `require_linux` — a clean "Linux-only"
  message + no-op on macOS/Windows instead of shelling out to absent tools.
- **CI (`.github/workflows/release.yml`):** on a `v*` tag, a matrix builds real
  binaries on native runners — musl-static Linux x86_64/aarch64 (aarch64 via
  `cross`), macOS x86_64 (macos-13) + arm64 (macos-14), Windows x86_64 (MSVC) —
  and publishes them to a GitHub Release. Assets keep the `8sync-<tag>-<os>-<arch>`
  scheme (`--locked`, `contents: write`).
- **Installers:** `install.sh` extended to the full os×arch matrix (linux
  x86_64/aarch64, darwin x86_64/arm64) keeping the source-build fallback; new
  **`install.ps1`** (Windows) — `irm …/install.ps1 | iex`, `-Uninstall`,
  `$env:SUSYNC_VERSION` pin, User-PATH wiring.
- Verified: Linux release build clean (0 warnings, 6.16 MB); Linux runtime
  smoke (sec/clean/help) unaffected. mac/Windows compilation + binaries are
  produced and verified by the CI native runners (a Linux host cannot build
  MSVC/Apple-SDK targets or the C deps `rusqlite`/`zstd-sys` without each
  platform's toolchain).

## [0.46.2] — 2026-07-06

### Fixed — `harness up --timer` OOM-killed the machine
- The generated `8sync-harness-up.service` had **no cgroup resource limits**, so
  the per-tick `codegraph index` (peak RSS ~5.3 GB on a large repo, e.g. `zus`)
  tripped the kernel OOM killer — reported as `Result: oom-kill, Mem peak: 5.3G`
  every 10 min — thrashing swap and taking down other apps.
- The service unit is now bounded to its own cgroup + de-prioritized:
  `MemoryHigh=2G` (reclaim throttle), `MemoryMax=4G` (hard ceiling — kills only
  this unit, never the machine), `MemorySwapMax=512M`, `OOMPolicy=stop`,
  `Nice=15` / `CPUWeight=10` / `IOWeight=10`, `TimeoutStartSec=900`.
- Verified: codegraph now held at ~2 GB by reclaim pressure instead of ballooning
  to 5.3 GB. Re-run `8sync harness up --timer <dur>` in a project to regenerate
  the bounded unit (already-installed unbounded units are overwritten).

## [0.46.1] — 2026-07-06

### Fixed — sweep now redeploys the project-level `/auto` command
- `8sync harness global --sweep` migrated a project's memory folder
  (`agents/` → `su-code/`) but left the project's `.omp/commands/auto.md`
  (and `8sync-engine.ts`) untouched — so `/auto` in a swept repo kept reading
  `agents/STATE.md` from a stale copy deployed by an older binary (project
  commands take precedence over the global one in omp).
- `stamp_project` (the per-repo sweep layer) now calls `deploy::ensure_engine`,
  refreshing both the `/auto` command and the engine extension in every swept
  project. Byte-identical writes stay quiet. Verified: all projects under
  `~/Projects` now have `su-code/`-only `/auto` commands (0 stale).

## [0.46.0] — 2026-07-06

### Changed — the agent-memory folder is now `su-code/` (was `agents/`), a distinctive project marker
- **BREAKING (auto-migrated):** every project's agent-memory dir is renamed
  `agents/` → **`su-code/`** so a su-code-managed repo is unambiguously
  identifiable (the old `agents/`/`AGENTS.md` markers are generic — every repo
  has them). The `AGENTS.md` anchor file stays (open-standard entry point); only
  the folder moves, and its links are rewritten to point at `su-code/`.
- **Detection now keys on `su-code/`.** `is_omp_project` (sweep) + the harness
  project-root detection recognise `su-code/` (with `agents/` kept only as a
  legacy migration trigger). `8sync harness global --sweep [DIR]` (the sweep
  command — **not** `harness all up`) migrates every legacy project it finds.
- **Auto-migration** (`memory::migrate_legacy_layout`, runs in
  `here`/`init`/`up`/bare-harness/sweep): renames `agents/` → `su-code/` and
  rewrites `agents/` → `su-code/` references in the anchor + live memory
  markdown. **Guarded:** only fires on a real 8sync memory dir (identified by
  `STATE.md`/`KNOWLEDGE.md`/`PROJECT.md`/`PLAYBOOKS.md`/`skills.toml`/`skills/`),
  so a source package literally named `agents/` is never touched. `.agents/`
  and `subagents/` are protected from the text rewrite. Idempotent.
- All 8sync-authored code, assets, skills, docs, and the recall hook (with an
  `agents/` fallback for un-migrated repos) updated to `su-code/`. Historical
  CHANGELOG entries left as-written (they document the `agents/` era).
  Verified E2E: legacy project migrated (folder + refs), guard skipped a
  non-memory `agents/` source dir, this repo dogfood-migrated, build clean.

## [0.45.0] — 2026-07-06

### Added — MCP `server.json` standard conformance (marketplace install) + the spec as a machine-wide default
- The marketplace MCP install now conforms to the official MCP registry
  `server.json` spec (schema `2025-12-11`, `modelcontextprotocol/registry`).
  `official_install` (`crates/cli/src/verbs/harness/marketplace.rs`) rewritten to honor:
  - **`registryType` → runtime**: `npm`→`npx -y` · `pypi`→`uvx` · `oci`→`docker run -i --rm`
    (+ `-e NAME` env forwarding) · `nuget`→`dnx` (was: everything defaulted to `npx`).
  - **`version` pinning** — `identifier@version` (or `identifier:version` for docker images).
  - **`runtimeArguments` + `packageArguments`** rendered into the command line
    (named/positional Argument shapes).
  - **`transport.type`** — a package/remote with `streamable-http`/`sse` becomes a
    remote (`{type,url,headers}`), not stdio.
  - **BUGFIX: `environmentVariables` → a `{NAME: value}` MAP**, not the array of
    descriptors it used to write (which produced an unusable `mcp.json` env for any
    server needing secrets). Required-but-empty vars are surfaced in the install note.
- `/api/mcp/add` (`McpAddBody`, `web.rs`) + the dashboard install flow (`api.ts`,
  `App.tsx`) thread `env`/`headers` maps end-to-end (were dropped before).
  Verified live against the registry: docker (`apithreshold` → `docker run … -e … img:0.1.0`),
  pypi (`armor-mcp` → `uvx armor-mcp@0.6.1` + `env:{ARMOR_API_KEY:""}`), npm — all via the UI, 0 console errors.
- **The standard is now a machine-local default that forces AI adherence.** A distilled
  spec reference ships in the binary (`assets/specs/mcp-server.md`) and `8sync harness
  global`/`init`/`up` deploy it to **`~/.omp/specs/mcp-server.md`** (`ensure_mcp_spec`).
  A short rule in the always-on `APPEND_SYSTEM.md` points every omp session at it:
  when writing/editing `mcp.json`, follow the on-disk standard — `env`/`headers` are
  maps, runtime from `registryType`, pin `version` — never invent fields.

### Added — `/auto` engine: gitleaks gate before every autonomous commit
- `engine_advance {commit:true}` (`assets/extensions/8sync-engine.ts`) now runs a
  secret gate (`gitleaks protect --staged`, matching the 8sync pre-commit hook)
  before committing — a finding aborts the commit and unstages, so an unattended
  `/auto` run can't leak a secret when the pre-commit hook is absent. No-op when
  gitleaks isn't installed (best-effort, no regression).
- `/auto` reviewed + independently functional-tested (Bun harness): all 6 `engine_*`
  tools register; verify-gate FAIL→WARN(2×)→BLOCK(3× doom-loop); `engine_advance`
  refuses an unverified task; pass→advance→done; trivial no-verify advance; commit path.

### Added — plan: Agent Terminal App (deep-researched, build later)
- `agents/plans/agent-terminal-app.md` — Tauri v2 + xterm.js/WebGL + portable-pty +
  Zellij-backend (resurrection sau reboot) + omp sidecar. Positioning: "cmux for every OS"
  (cmux: 22.3k stars/4 tháng, macOS-only → khe cross-platform đang mở). Gồm stack đã chốt,
  4 phase MVP, trend mechanics, rủi ro, metrics, next actions + sources.

### Fixed — omp startup error `providers: must be an object (was null)`
- `~/.omp/agent/models.yml` was left with a bare `providers:` key (YAML null) when the
  local-model registry became empty (e.g. after `add-local-model rm <last>`), making omp
  print a schema error + disable custom providers on every start. `insert_block` now
  finalizes the file: no real children ⇒ `providers: {}` (valid empty object); a later
  add reopens `{}` and inserts under it. Both branches proven live via the real binary
  and A/B'd against omp (`omp models list`): bare ⇒ error, `{}` ⇒ clean.

### Added — `harness help`: LOCAL GGUF MODEL real-flow example block
- Copy-paste flow: add from .gguf path / HF repo id / URL → `list` → use once via
  `8sync ai --model local/<name>` → set as `default`/`code` model → `rm`. Points at the
  TSV registry + sentinel-managed provider block.

## [0.44.0] — 2026-07-05

### Added — loop-engineering stop signals in the 8sync engine (doom-loop guard + real gate)
- Per the loop-engineering literature (Avi Chawla, "Prompt, Context, Harness & Loop
  Engineering", Jul 2026): an agent's own "done" is not a stop signal. Two code-enforced
  fixes in `assets/extensions/8sync-engine.ts`:
- **`engine_advance` now actually enforces the gate** — it REFUSES a task that has verify
  commands but no passing `engine_verify` run (new `verified` flag per task; previously the
  description claimed "code-enforced" but `advance` set `done` unconditionally). Tasks with
  zero verify commands keep the documented trivial-advance path.
- **No-progress detector (doom-loop guard)** — `engine_verify` fingerprints each failure
  output (FNV-1a); 2 consecutive identical failures WARN ("change the approach"), 3 BLOCK
  the task early even below `maxRetries` (a byte-identical failure means the retry did
  nothing but burn tokens). New per-task `failStreak`/`lastFailureHash` state; old
  `state.json` files load via zod defaults (backward compatible).
- `/auto` command updated: different-fix-per-retry rule, advance-refusal note, and
  unattended runs now require a hard token ceiling (omp budget `+Nk!`) as the third stop
  signal (turn/token cap). Verified end-to-end in Bun: refuse-unverified, warn-at-2,
  block-at-3 (retries 3/10), pass→advance, trivial-advance, old-state load.

### Added — `8sync harness global`: omp rules machine-wide, one key (Anthropic token-optimized)
- **`8sync harness global`** — applies the omp rule layer MACHINE-WIDE so every project that
  runs omp gets it without a per-project run: `~/.omp/skills` + `00-force-load.md`,
  `~/.omp/agent/APPEND_SYSTEM.md` (appended to EVERY omp system prompt), MCP servers
  (codebase-memory · headroom · serena · zai-vision), recall hook, capabilities snapshot,
  workflow extension + engine. CWD-independent — never touches the current project.
- **Anthropic token-optimizer defaults**: `compaction.thresholdPercent = 50` written only when
  unset (never overrides the user), headroom compression for >50-line outputs, and byte-stable
  `APPEND_SYSTEM.md` deploys (identical ⇒ skip) so the system prefix stays hot for Anthropic
  prompt caching. New `compaction::ensure_threshold_default` helper.
- **`--sweep [DIR]`** (default `~/Projects`) — stamps the per-project layer into every **omp
  project** under DIR (a git repo with `agents/` or `AGENTS.md`/`CLAUDE.md` — repos not using
  omp are skipped + reported): mirror skills (additive), inject force-load into
  AGENTS.md/CLAUDE.md, seed agents/ memory, install the gitleaks hook. Skips
  `node_modules`/`target`/hidden dirs, depth ≤ 4, found repos are not descended into.
  `--pull` re-pulls registered skills first.
- Dedup: bare `8sync harness`'s global block now calls the shared `global::global_pass()`
  (`crates/cli/src/verbs/harness/global.rs`) — one source of truth for the machine-wide layer.
- **Overwrite policy made explicit** (default = NEVER overwrite, only add what's missing):
  documented in `8sync harness help` (new OVERWRITE POLICY section) + AGENTS.md §8 as a
  repo-wide invariant. Audited: agents/*.md seed-if-missing, CHANGELOG created once, skills
  mirror additive (`--force` only), AGENTS.md sentinel-block only, gitleaks hook only-if-absent,
  config key-detect. Proven live: hand edits to a mirrored SKILL.md + STATE.md survive a
  sweep re-run byte-for-byte.

## [0.43.0] — 2026-07-05

### Added — codegraph canvas capture (`?shot=1`) + automatic locate for non-vision models
- **`/codegraph?shot=1`** on the dashboard renders ONLY the React-Flow package call graph,
  full-viewport (no sidebar/cards) — made for `8sync shot 'http://127.0.0.1:8731/codegraph?shot=1'`:
  one big, legible graph image (~2k vision tokens) instead of a full-page capture. Everything
  else on that page stays text via `/api/codegraph/overview|search|trace` — image for the
  layout, API text for the details.
- **Auto-rule baked into the always-read layer** (`APPEND_SYSTEM.md` + codegraph/image-routing/
  locate-anything skills): a non-vision model (GLM-5.2) that needs positions/layout/distribution
  from an image uses **`8sync locate`** (LocateAnything-3B, on-device ggml, CPU or CUDA)
  automatically — zai-vision answers *what it says*, locate answers *where it is*.

### Changed — README + GitHub Pages are English-first (100%)
- `README.md` fully rewritten in English with the dashboard demo: hero (State page) + Bench /
  Codegraph / Models / Marketplace screenshots (`docs/assets/dashboard-*.png`, freshly captured,
  leak-checked). `docs/index.html` (Pages) translated to English with a Dashboard gallery.

### Changed — bench now DRIVES optimization (breakdown + spine advisory, CLI/API/web)
- `BenchMetrics` (`/api/bench`) exposes the upfront breakdown — `core_tok`, `spine_tok`,
  `naive_tok` — plus `spine_advice`: a warning set when the memory spine (`agents/*.md`)
  eats **more than half the upfront budget** (the single lever bench exposes; prefix +
  CORE are fixed by design, the spine grows unbounded between consolidations).
- CLI `8sync harness bench` prints the same advisory as a `!` line with the concrete
  fix (trim `agents/STATE.md` / let `8sync harness` auto-archive KNOWLEDGE >200 lines).
- Dashboard **Bench page rebuilt**: auto-loads on mount (was an empty state until a
  manual "Run bench" click — the compute is 40 ms and deterministic), upfront breakdown
  meters (prefix / CORE / spine share of upfront), advisory callout, naive-baseline row.
  New `.meter-val-wide` for long meter values. Browser-verified: 0 console errors.
- Warning sweep: removed dead `assets::web_asset_iter`, `LocalModel` → `pub(crate)`
  (private-interface warning), unused `ctx` in `api_state`. `cargo build` is warning-free.

### Added — `8sync harness add-local-model`: local GGUF models for omp (Rust runtime)
- New subcommand `8sync harness add-local-model <path> [name]` loads a **GGUF** model
  through **mistral.rs** (pure-Rust, memory-safe inference — no C++ `llama.cpp`) and
  registers the served OpenAI endpoint as an omp provider `local/<name>`, so omp routes
  to on-device models exactly like a cloud one (`8sync ai --model local/<name>`).
- `<path>` auto-classifies: an existing `*.gguf` FILE, a HuggingFace repo id (`org/repo`),
  or a `*.gguf` URL (downloaded to `~/.cache/8sync/models/`). GGUF-only this version
  (validated by magic bytes); other formats auto-detect later. GGUF chosen for speed.
- Runner is auto-installed via the official mistral.rs `install.sh` (prebuilt per-GPU
  CUDA or CPU binary — no Rust/CUDA toolkit needed, just the NVIDIA driver for GPU).
- Each model runs as a systemd **user** service `8sync-llm-<name>.service`. For a local
  `.gguf` the served command is `mistralrs serve --host 127.0.0.1 --no-ui --format gguf
  -m <dir> -f <file>` (mistral.rs `--model-id` is a *directory*, not a file); an HF repo
  goes straight to `-m`. Port auto-allocated from 8770. A TSV registry
  (`~/.config/8sync/local-models.tsv`) is the source of truth; the omp provider lives in
  a managed sentinel block inside `~/.omp/agent/models.yml` that **`gateway apply`
  preserves** across re-deploys. `list` / `rm <name>` manage the set.
- The provider sends model **`id: default`** upstream (the only alias mistral.rs serves
  besides the model-dir path); the clean `local/<name>` handle lives in the `name:` field
  (omp fuzzy-matches both), so `--model local/<name>` still selects it.
- `doctor` reports the registered local-model count; `~/.omp/capabilities.md` lists them
  so the agent knows they exist.
- **E2E-validated** on this machine: mistral.rs 0.8.23 (auto-selected the prebuilt
  `cuda131-sm120` RTX-5080 binary) serves a 135M GGUF; `add-local-model` → systemd unit →
  `/v1/chat/completions` returns real text; `rm` tears down unit + block + registry clean.

### Added — `8sync locate`: visual grounding (NVIDIA LocateAnything-3B)
- New verb `8sync locate <image> "<prompt>"` returns labeled **bounding boxes +
  click-center coordinates** from an image — GUI element grounding (click points),
  OCR/text localization, open-set detection. Grounding, not captioning: complements
  zai-vision (describe) and the `browser` tool (act). Pipeline: `8sync shot` → `locate`
  → click. `--annotated out.png`, `--mode hybrid|slow|fast`.
- Runs **NVIDIA LocateAnything-3B** through `mudler/locate-anything.cpp` (MIT C++/ggml
  port, prebuilt GGUFs, no Python). `--setup` clones + cmake-builds the CLI (CUDA if
  the toolkit is present, else CPU) and downloads the q8_0 GGUF (~6.3 GB) to
  `~/.cache/8sync/locate-anything/`. Model license: NVIDIA research / non-commercial.
- New **always-on specialist** skill `locate-anything` (`inject.rs::always_on_rank`,
  rank 8 after `image-routing`): force-loaded into the AGENTS.md block every session and
  re-deployed/refreshed on every `8sync harness` run (bundled). Body read on trigger
  (token-lean); an APPEND_SYSTEM pointer surfaces it when exact coordinates are needed.

## [0.42.0] — 2026-07-04

### Added — modality routing: read STRUCTURE as an image, PRECISE things as text
- **`8sync shot` is now real** (was a no-op stub) — renders any URL / local HTML to
  PNG via system or omp's bundled Chromium (`~/.omp/puppeteer/chrome/…`), prints a
  vision-token estimate (`ceil(w/28)*ceil(h/28)`, 28×28 patch — no stale cap). Also
  fixes the `image-routing` skill which already pointed at it.
- New **Modality routing** directive forced across the harness: `APPEND_SYSTEM.md`
  (always-on), the `image-routing` skill (rewritten with honest economics), and the
  `capabilities.md` snapshot, enforced per-turn by `--advisor`. Rule: vision models
  render a codegraph / diagram / dashboard / big PDF to ONE image (modality-fit); code /
  exact config / line-numbered data stay TEXT (cheaper AND lossless).
- Grounded, not hyped: the 10×/90% token cut (DeepSeek-OCR, arXiv 2510.18234) needs a
  DEDICATED optical encoder — NOT a screenshot to Opus/GLM. Claude bills images per
  28×28 patch (pay-per-pixel on Opus 4.7+). Measured on this repo: STATE.md as image =
  0.87× (LOSES vs text); the 12k-edge codegraph as image ≈ 25× (structure win). The
  gate captures exactly that. OCR-Memory pattern (arXiv 2604.26622) documented: image to
  LOCATE, exact text to READ.
- **Dashboard deep-link** — `web/src/App.tsx` now reads `?page=<id>` (or `/<id>`) for
  the initial page (`pageFromUrl()` + `history.replaceState` on nav). Nav was in-memory
  only, so `8sync shot .../codegraph` used to render State; now
  `8sync shot http://127.0.0.1:8731/?page=codegraph` captures the real graph. `build.rs`
  already rebuilds the Vite bundle on `web/src` change, so a plain `cargo build` re-embeds.

## [0.41.0] — 2026-07-03

### Added — dashboard `Marketplace`: discover + install skills & MCP servers
- New **Marketplace** nav page (Discover group) in `8sync harness web`: browse,
  search, sort (Top by stars/uses · New by recency), and one-click install
  skills and MCP servers from public registries into the current project.
- **MCP sources (4):** the official registry (`registry.modelcontextprotocol.io`,
  REST API), Smithery (`registry.smithery.ai`), Glama (`glama.ai` JSON API), and
  **mcp.so scraped with the pure-Rust `scraper` crate** (HTML DOM via
  `a[href^="/server/"]`, fetched through `curl` — no reqwest, Rust-first). 135+
  merged/deduped entries; install writes a real `~/.omp/agent/mcp.json` stdio
  (`npx`/`uvx`) or remote (`http`/`sse`) entry.
- **Skills source:** GitHub repo search ranked by stars; install shells the
  existing collection-aware `8sync skill add <url>`.
- Catalog cached under `.cache/8sync/marketplace/*.json` (1h TTL — the MCP
  registry maintainers ask aggregators to poll infrequently + persist).
  (`crates/cli/src/verbs/harness/marketplace.rs`, `web.rs`, `web/src/*`)

### Added — import buttons across the dashboard (were plumbing-only)
- **Skills page**: `skillAdd`/`skillUpdate` were wired in the API client but had
  no UI — added an **Import** toolbar (github URL · `gh:owner/repo` ·
  `path:/abs/dir` folder · `builtin:name`) + **Update all**.
- **MCP page**: **Install-from-link** (`npx -y pkg`, `uvx pkg`, or an https
  remote URL → merged into `mcp.json`) + a per-server **Remove**.
- **Rules page**: **Import from a folder or GitHub repo** (`.md`/`.mdc`,
  recursively; prefers a `rules/`/`.cursor/rules`/… subdir), shallow-cloned to a
  RAII temp dir. Complements the existing inline text-add.
- New routes: `/api/marketplace`, `/api/mcp/{add,remove}`, `/api/rules/import`.
- New dep: `scraper 0.20` (pure-Rust html5ever + CSS selectors) for the mcp.so
  aggregator — HTTP still shells out to `curl`.

## [0.40.0] — 2026-07-03

### Changed — advisor default-ON (per-turn rule/tool-use reviewer)
- omp's `--advisor` (passive per-turn reviewer that checks each turn against the
  always-on rules — code-intel first, correct MCP tool names, open SKILL.md — and
  injects corrective notes) is now passed **by default** by `8sync ai` and the
  `8sync .` / resume session. Closes the last anti-forget gap: layers 1-4
  (APPEND_SYSTEM rules, recall-hook live context, Mnemopi memory, capabilities
  catalog) *declare + remind* but nothing *checked* whether the last turn actually
  obeyed — advisor is that live reviewer.
- Token-optimal gating: skipped for `trivial`-class prompts. Opt out per run with
  `8sync ai --no-advisor`, or globally via `advisor = false` in
  `~/.config/8sync/models.toml`. New `advisor` key in `ModelConfig` (default true).
- Docs: `models.toml`, `APPEND_SYSTEM.md`, and `8sync ai --help` document the
  toggle + tradeoff.

### Fixed — `8sync doctor` self-heals stale profile state
- `profile::mark_applied()` was append-only — a profile deleted from the repo
  (e.g. `caelestia.toml`, removed in `e761c31`) stayed in `~/.config/8sync/profile.toml`'s
  `applied` list forever, and `doctor` printed it back as a false positive. New
  `profile::prune_stale()` diffs `applied` against `load_all()`, drops entries that no
  longer resolve, rewrites state only if changed. Wired into `doctor` (warns once, then
  clean). Verified against a real stale state.

### Docs — README + GitHub Pages refreshed to current surface
- README + `docs/index.html`: TL;DR now leads with the one-liner install →
  `8sync harness` → `8sync harness web`; new **Dashboard** section (with screenshots)
  documents the CRUD control surface (models/skills/memory/rules/engines/Codegraph);
  full harness subcommand table (web/gateway/bench/audit/eval/toolstats); added the
  machine verbs (`bt`/`clean`/`theme`/`bg`) that were missing. Fixed stale numbers
  (binary ≈ 5.0 MB, **35** bundled skill). Landing page gains a Dashboard nav link +
  feature card + two live screenshots (`docs/assets/`).

## [0.39.0] — 2026-07-02

### Added — dashboard `Codegraph` page: visualize the codebase-memory-mcp knowledge graph
- The web dashboard (`8sync harness web`) had zero visibility into the codegraph/
  codebase-memory-mcp engines it lists on the Engines page — `search_graph`/
  `trace_path`/`get_architecture` were agent-only. New **Codegraph** nav item
  (Runtime group) renders the real graph: package call graph (elk auto-layout,
  box size ≈ node count, edges = call counts between packages), **Leiden
  cluster cards** (de-facto modules, cohesion %, top symbols — the actual
  architectural seams, not just folders), a BM25 **symbol search**, and a
  **caller/callee trace subgraph** for the selected symbol or a fan-in hotspot.
- Backend: 3 new routes (`/api/codegraph/{overview,search,trace}`) shell out to
  `codebase-memory-mcp cli <tool> <json>` (same binary+slug `harness up`
  already indexes against) — no MCP client embedded, stdout-only JSON parsing
  verified log-noise-free. Honest 404 ("not indexed yet — run `harness up`")
  when the project has no graph.
  (`crates/cli/src/verbs/harness/web.rs`, `web/src/App.tsx`, `web/src/api.ts`)

### Fixed — dashboard UI/UX audit (browser-verified against a real project)
- **Engines page**: the `codebase-memory-mcp 0.8.1` tile title rendered
  **one character per line** — `overflow-wrap: anywhere` collapsed the flex
  item's intrinsic width to zero next to a wide version tag. Fixed `.tile-head`
  to give the title `flex: 1 1 auto; min-width: 0` and wrap on word
  boundaries instead of mid-word.
- **Version tags** were inconsistent/redundant across tools (`on
  codebase-memory-mcp 0.8.1`, `on headroom, version 0.27.0` — duplicating the
  already-visible tool name). `api_engines` now extracts just the semver token.
- **Skills page** (67 skills, no way to find one): added a filter input +
  tier dropdown (`all/always/on-demand/off`) with a live "N of M" count.

### Added — `8sync harness gateway` — deploy/verify the omp model-gateway
- New subaction: `8sync harness gateway [apply|key <KEY>|verify|status]` — deploys
  `~/.omp/agent/models.yml` from a bundled template so the 9router gateway config
  (provider URL, models, API key, `thinking.mode = anthropic-budget-effort`) is
  reproducible by one command instead of hand-editing.
- `apply` is idempotent (backs up a differing file to `models.yml.bak`, preserves the
  existing key on refresh; key from `$NINE_ROUTER_KEY` or `gateway key <KEY>`).
- `verify` pings `cc/claude-sonnet-5` through the gateway — the exact path that 400'd
  before the thinking fix; HTTP 200 = healthy. `status` masks the key + flags a missing fix.
- Fixes recurring `400 thinking.enabled.budget_tokens: Field required` on claude-sonnet-5:
  omp's default `thinking:{type:adaptive}` is rejected by the gateway; the bundled template
  forces `enabled + budget_tokens`. (`crates/cli/src/verbs/harness/gateway.rs`, `assets/configs/omp/gateway-models.yml`)
### Added — 18 feynman research skills ported to omp-native (were unusable stubs)
- Audited `agents/skills.toml`'s 20 `companion-inc/feynman`-sourced skills
  (submodule-inspected at `reference/feynman`, then removed). Found 12 were
  12-line stubs pointing at feynman's OWN slash-commands (`/deepresearch`,
  `/lit`, `/recipe`, `/audit`, `/draft`, `/review`, `/compare`, `/watch`,
  `/replicate`, `/jobs`, `/log`, `/autoresearch`) — those commands only exist
  in feynman's own pi-coding-agent runtime (`prompts/*.md` +
  `extensions/research-tools.ts`), NOT in omp. Deployed as-is they were
  completely inert. 2 more (`session-search`, `preview`) had the same
  problem behind a documented fallback. Ported all 14 (deep-research,
  literature-review, autoresearch, ml-training-recipe, paper-code-audit,
  paper-writing, research-review, source-comparison, watch, replication,
  jobs, session-log, session-search, preview) into self-contained
  `assets/skills/<name>/SKILL.md` using omp's real tools (`task` in place of
  feynman's `subagent`, `web_search`/`read` in place of `fetch_content`,
  `ask` in place of `ask_user_question`, `job`/`retain` where feynman had no
  equivalent). Also re-bundled 4 genuinely-portable CLI skills (`eli5`,
  `docker`, `modal-compute`, `runpod-compute` — only cosmetic "Feynman"
  naming, no runtime dependency) as `builtin:` too. `alpha-research` is kept
  pointed at the real `companion-inc/feynman` source since it's a legitimate
  CLI wrapper (`feynman alpha ...`, needs `@companion-ai/feynman` installed
  via the existing `ensure_feynman_cli()`). Dropped `contributing`
  (feynman-repo-only, no value for su-code users).
- **Bug fixed in the process**: `agents/skills.toml` had `[peer-review]`
  pointing at feynman, but feynman renamed that skill upstream to
  `research-review` — the entry never resolved to anything on disk. Fixed
  to `[research-review]`.
- **Bug fixed in `update_skills` (`crates/cli/src/verbs/skill/update.rs`)**:
  registering ANY single skill from a git collection repo (e.g. just
  `alpha-research` from feynman's 20-skill repo) silently reinstalled
  EVERY sub-skill in that repo on every `8sync harness`/`skill update` run,
  regardless of registry membership — `contributing` kept reappearing after
  being deliberately dropped from the manifest, because the git-source loop
  treated `filter.is_none()` as "install everything found". Fixed: a
  sub-skill is only (re)installed when the URL/repo was explicitly targeted,
  the skill name was explicitly filtered, or (bulk run) it already has its
  own registry key — a collection repo no longer silently grows the
  registry.
- 18 new on-demand skills registered in `assets/skills/00-force-load.md`'s
  lookup table (55 on-demand total, up from 37).

### Fixed — kitty terminal zoom (`ctrl+shift+minus`) silently stolen by vsplit binding
- `8sync setup --profile terminal` mapped `ctrl+shift+minus` to
  `launch --location=vsplit` for the gsd-style 3-pane layout. That's kitty's
  DEFAULT font-zoom-out binding (`change_font_size all -2.0`) — user maps
  override defaults, so zoom-out silently stopped working with no error.
  Moved vsplit to `ctrl+shift+backslash` (unclaimed by any kitty default);
  `ctrl+shift+minus`/`+equal`/`+backspace` now behave stock. Re-ran
  `8sync setup --profile terminal` to regenerate the live
  `~/.config/kitty/8sync.conf` on this machine — user must reload/reopen
  kitty (font-zoom maps apply live via `kitty @ load-config` if remote
  control is on, no window-recreate needed unlike the earlier decoration fix).
  (`crates/cli/src/verbs/setup.rs:665-668`)

### Added — `~/.omp/capabilities.md` now embeds EXACT MCP/builtin/memory tool catalogs
- Previously the snapshot only said "`4` server(s) registered" — no tool names.
  Agents had to guess, which is exactly how the earlier "codegraph verb"
  hallucination bug happened (see KNOWLEDGE.md). Now `8sync harness` writes
  the FULL exact tool catalog for every registered MCP server:
  `codebase-memory-mcp` (14), `headroom` (3), `serena` (23), `zai-vision` (8)
  — plus omp's own built-in tools (parsed live from `omp --help`'s "Available
  Tools" block) and the Mnemopi memory tools (`recall`/`reflect`/`retain`/
  `memory_edit`, listed only when the backend is ON).
  (`crates/cli/src/verbs/skill/deploy.rs::known_mcp_tool_catalog` +
  `ensure_omp_capabilities_snapshot` rewrite.)
- `APPEND_SYSTEM.md` RULE #0 now names the 4 connected servers explicitly and
  points at `~/.omp/capabilities.md` as the ground truth for exact
  names/params — "never guess/invent an MCP tool name". The `8sync-recall.ts`
  hook (injected every `before_agent_start` + compaction) carries the same
  pointer so it survives past 50% context.

### Fixed — kitty lost its title bar/min-max-close/resize border on KDE (stacking WM)
- `8sync setup --profile terminal` unconditionally wrote
  `hide_window_decorations yes` into `~/.config/kitty/8sync.conf`. That's
  correct on a tiling Wayland compositor (Hyprland/HyDE — the project's
  primary target) which draws no chrome and expects clients to hide their own,
  but on a **stacking** desktop (KDE/kwin, GNOME/mutter, …) the compositor
  ALSO does not add server-side decorations for an undecorated client — the
  window ends up with no title bar, no traffic-light buttons, and no
  drag-to-resize border at all.
- New `env_detect::is_tiling_wm()` checks `is_hyde()` first, then
  `XDG_CURRENT_DESKTOP`/`DESKTOP_SESSION` against known tiling WMs
  (hyprland/sway/river/wayfire/qtile/i3/bspwm/awesome).
  `render_kitty_conf` now only emits `hide_window_decorations yes` when
  `is_tiling_wm()` is true; stacking desktops (verified live on KDE/Plasma/
  kwin/Wayland) keep normal window chrome. (`crates/cli/src/verbs/setup.rs`,
  `crates/cli/src/env_detect.rs`)
- Re-running `8sync setup --profile terminal` (idempotent) regenerates
  `~/.config/kitty/8sync.conf` with the fix; requires closing/reopening the
  kitty window (decorations are negotiated at window-creation time, not
  live-reloadable).

### Added — Z.AI vision MCP (`zai-vision`) bridges GLM-5.2's text-only gap + dedicated skill
- GLM-5.2 (omp's default model) is text-only. `8sync harness` now auto-installs
  `@z_ai/mcp-server` (npm, via `bun add -g`) and registers it as the `zai-vision`
  omp MCP server, exposing 8 GLM-5V tools (`ui_to_artifact`,
  `extract_text_from_screenshot`, `diagnose_error_screenshot`,
  `understand_technical_diagram`, `analyze_data_visualization`, `ui_diff_check`,
  `analyze_image`, `analyze_video`). Auth reuses the SAME Z.AI key already
  configured for `zai/glm-5.2` (pulled via `omp token zai`, no separate signup).
  (`crates/cli/src/verbs/skill/deploy.rs::ensure_zai_vision_mcp` +
  `resolve_zai_api_key`; wired into `harness auto`/`harness init`; reported by
  `doctor`.)
- **`register_omp_mcp` now supports per-server `env`** (only emitted when
  non-empty, so existing env-less entries stay self-heal-stable).
- **Verified end-to-end** (not illustrative): real browser screenshots run
  through the actual `zai-mcp-server` stdio process via JSON-RPC `tools/call`.
  Found and fixed a real gap — `8sync harness` now defaults
  `Z_AI_VISION_MODEL=glm-4.6v-flash`, the ONLY vision model that works on a
  stock Z.AI key with no vision resource package (paid models 400 with `1113
  insufficient balance`; verified against Z.AI's live pricing table).
- **New skill `zai-vision`** (`assets/skills/zai-vision/SKILL.md`, auto-deployed
  by `install_bundled_global`) documents the full combination matrix: browser
  screenshots, `8sync shot/pdf-img/diff-img`, codegraph/cbm diagrams, serena,
  headroom compression, `inspect_image` fallback, retain/recall, and advisor —
  plus the real verified tool-call output and a troubleshooting table for Z.AI
  error codes (1113/1211/1301/1305).
- **`~/.omp/capabilities.md`** — new live snapshot of omp's surface (advisor,
  thinking, inspect_image, adaptive model roles, retain/recall, registered MCP
  count, skill count), refreshed every `8sync harness` run, surfaced by
  `doctor` (`ensure_omp_capabilities_snapshot`).
- `APPEND_SYSTEM.md` and `image-routing` SKILL now point to `zai-vision` as the
  mandatory bridge step after routing to "image".

## [0.36.0] — 2026-06-30

### Added — `8sync bg search`: find wallpapers online (no API key) + pick with live preview
- New **`8sync bg search <query>`** sub-action. Searches **wallhaven.cc** via its public API
  (**no API key needed**, SFW, ≥1920×1080) — wallpaper-focused (incl. anime/dark), a good fit for
  the project's aesthetic without imposing an Unsplash/Pexels registration on the user.
- **Interactive (kitty)**: stages thumbnails, then opens `fzf` with a **live `kitten icat` preview
  pane** showing each candidate + its wallhaven **source link**. Enter downloads the full-res image,
  adds it to the collection, and sets it live; Esc cancels. Only the full image you pick is fetched.
- **Non-interactive**: prints the result list (id + resolution + source link) for scripting/agents.
- RAII temp cleanup; reuses the existing `add`+`set` path. No new Rust deps.
  (`crates/cli/src/verbs/bg.rs`)

## [0.35.0] — 2026-06-30

### Added — `8sync bg`: manage the kitty wallpaper at runtime (live swap + inline preview)
- New **`8sync bg`** verb: `show | get | set [file] | list | add <url|file>`. Brings back the
  wallpaper control that was removed in the slim-down — now without HyDE overlap (kitty's
  in-terminal `background_image` ≠ HyDE's desktop wallpaper).
- **Inline preview**: `bg show` renders the current wallpaper in the terminal via `kitten icat`
  (kitty graphics protocol — same mechanism omp uses); `bg list`/`bg set` (no arg) open an
  interactive **fzf picker with a live `kitten icat` preview pane** → scroll, see each image,
  Enter to set.
- **Live swap**: `bg set <file>` rewrites the `background_image` line in `8sync.conf` +
  SIGUSR1-reloads kitty (instant, no restart). The choice is recorded in
  `~/.config/8sync/wallpaper` and **`8sync setup` honors it** (re-setup no longer resets your
  wallpaper). Collection lives in `~/.config/8sync/wallpapers/` (`bg add <url>` populates it).
- Zero new Rust deps (shell-outs to `kitten`/`fzf`/`curl`) — binary stays lean.
  (`crates/cli/src/verbs/bg.rs`)

## [0.34.0] — 2026-06-30

### Added — `8sync theme`: switch kitty palettes live (readable on any wallpaper)
- New **`8sync theme`** verb: `list | set <name> | show [name]`. Six curated dark palettes
  (**tokyo-night** default · catppuccin-mocha · gruvbox-dark · nord · rose-pine · dracula), each
  a pure color fragment tuned for **wallpaper-overlay readability** (foreground + bright-black
  verified at WCAG-AA contrast ≥ 4.5:1 against the theme bg). Switching writes
  `~/.config/kitty/8sync-theme.conf` and **SIGUSR1-reloads kitty** — instant, no restart, no
  remote-control socket. `hydectl theme` still owns Hyprland/UI; this owns kitty (distinct surfaces).
  (`crates/cli/src/verbs/theme.rs`)

### Fixed — kitty config: readable text + restored `allow_remote_control` + structure/palette split
- **Readability root-cause**: deployed `8sync.conf` had `background_tint 0.55` (image 45% visible →
  bright wallpaper washed out the foreground). Raised to **0.86** (image subtle, text crisp).
- The glass **structure** (`background_opacity`/`blur`/font/splits/tabs) is now separated from the
  **palette** (`8sync-theme.conf`, swappable); `render_kitty_conf` no longer emits colors inline.
- **Restored `allow_remote_control yes`** in the managed config — it had been dropped in the
  slim-down, breaking `kitty @` live control. (`crates/cli/src/verbs/setup.rs`)
- `8sync setup --profile terminal` now deploys both files (structure + active palette); the active
  theme is recorded in `~/.config/8sync/kitty-theme` and survives re-runs.
## [0.33.0] — 2026-06-29

### Added — dashboard surfaces the live `/auto` engine run (real, not demo)
- New `/api/engine` reads the **real** gsd-pi state machine the engine drives at
  `<root>/.cache/8sync/engine/state.json`; the Engines page renders a live board — goal · progress
  bar · slice/task tree with ✓/▸/○/✗ status + retries · current task (4 s refresh, read-only mirror of
  the terminal board). Closes the gap where the dashboard showed the workflow *editor* + engine
  *binaries* but never the actual `/auto` run. `{active:false}` when none. Browser-verified, 0 console
  errors. (`crates/cli/src/verbs/harness/web.rs`, `web/src/{api.ts,App.tsx}`)

### Added — AFFiNE in the `alexdev` profile
- `affine-bin` — official prebuilt of the open-source Community Edition (AGPL/custom: free, self-hostable,
  no cloud lock-in). The from-source `affine` AUR pkg fails upstream (electron-packager zip step), so the
  prebuilt is used. (`assets/profiles/alexdev.toml`)

### Changed — always-on directives also prime recall/retain + browser
- `APPEND_SYSTEM.md` (every system prompt, never compacted) + the recall hook now explicitly prime
  **`recall`/`reflect` before · `retain` durable facts after** (Mnemopi) and **`browser` to verify any
  web/UI change for real** — on top of RULE #0 (code-intel MCPs) + skill ref-paths. Stays terse by design
  (the system prompt isn't headroom-compressed; headroom is for tool OUTPUTS).
  (`assets/configs/omp/APPEND_SYSTEM.md`, `assets/hooks/8sync-recall.ts`)
- **kitty tab bar moved to the bottom** (`tab_bar_edge bottom`) — easier tab switching. (`setup.rs` renderer)

### Fixed — `8sync harness up` now redeploys the recall hook
- `harness up` refreshed APPEND_SYSTEM/engine/workflow but not the recall hook (only init/bare-harness
  did), so hook changes never reached existing machines via `up`. Now it does.
  (`crates/cli/src/verbs/harness/up.rs`)

## [0.32.1] — 2026-06-29

### Fixed — `8sync harness` auto-installs the token-optimization MCPs (no startup error)

- `headroom` (and `serena`) were **registered in `~/.omp/agent/mcp.json` even when their binary
  wasn't installed** — so omp failed at startup with `Executable not found in $PATH: "headroom"`.
  Now `8sync harness` **bootstraps `uv`** (Astral, user-level curl install — no sudo), installs
  `headroom-ai[mcp]` through it, and **only registers an MCP whose executable actually exists** —
  a still-missing tool has its stale entry **purged** so omp never errors at startup. `uv` also
  ships the `uvx` serena needs, so both engines come up from one `8sync harness`, no manual steps.
  (`crates/cli/src/verbs/skill/deploy.rs`)

## [0.32.0] — 2026-06-29

### Performance — binary back under control (offsets bundled rusqlite)

- Enabled rust-embed's `compression` feature (transparent `include-flate` decompress on `.data` — both the
  `assets/` skills tree and the embedded `web/dist` FE shrink) and set the release profile to `opt-level = "z"`.
  Roughly halves the binary, offsetting the bundled `rusqlite` (toolstats) + impeccable + the Vite FE.
  (`crates/cli/Cargo.toml`, `Cargo.toml`)

### Fixed — wallpaper self-heal (no more kitty "render to RGB: EOF")

- `setup::deploy_wallpaper` trusted `exists()`, so a transient/blocked download left a **0-byte
  `wallpaper.png`** kitty can't render (blank background) — and the early `exists()` return meant it never
  re-tried. Now validates the file (size>0 + PNG/JPEG/WEBP magic via `is_valid_image`), adds a `Mozilla/5.0`
  UA + `--retry 2`, and **purges a corrupt file** so a re-run re-downloads. (`crates/cli/src/verbs/setup.rs`)

## [0.31.1] — 2026-06-29

### Changed — `toolstats` now reports the *actionable* ratio
- The headline is now **optimizer vs raw-search of code-lookup calls only** (optimizer = codegraph /
  cbm / serena; raw-search = grep / search / find / glob) — instead of "% of all calls", which was
  misleading (most calls are edit / bash / read-before-edit, not lookups). `read` is shown separately
  (often legitimate, not shamed) and `headroom` is labelled background/auto-compress (not an
  agent-called tool). Measured: su-code optimizer **34%** of lookups, agentic-cloudgo **25%** — vs the
  old "2% of all calls" framing. The DB is rebuilt from current sessions each run (re-categorizes).

## [0.31.0] — 2026-06-29

### Added — `8sync harness toolstats` (SQLite tool-call tracker)
- New verb that tracks how the agent **actually** uses tools, parsed from omp's own session
  JSONL, into a per-project SQLite DB (`.cache/8sync/toolstats.db`, gitignored). Reports the
  **optimizer** (codegraph / codebase-memory-mcp / serena / headroom) vs **fallback** (grep / read /
  search / find / glob) call ratio + per-tool failures, so you can see whether the STEP-0
  token-optimization stack is being used and catch failing calls (e.g. a dead MCP server).
- Idempotent (keyed on session+seq → re-run only adds new calls); inspectable with any SQLite tool.
- Motivation: across this machine's 68 omp sessions / 28k calls, the optimizer stack was **1.1%**
  of calls (serena/headroom **0**) vs **35%** raw fallback — `toolstats` makes that visible per project.

## [0.30.0] — 2026-06-29

### Changed — default `8sync setup` is AI-core only
- **Stage A now installs only the AI coding harness**: omp, codegraph, MCP servers + skills,
  github-cli, paru, PATH bootstrap, configs. The terminal/editor polish (kitty glass theme + helix
  + JetBrains Nerd font + wallpaper) is **no longer installed by default** — a fresh `8sync setup`
  is pure AI now.
- **New opt-in `terminal` stack**: `8sync setup --profile terminal` (also offered in the y/N menu
  and applied by `--full`). `docker` moved out of the terminal stack — it lives in `dev-stack`.
- **`doctor`** reports the terminal stack (kitty/helix/docker) as advisory/opt-in — no longer warns
  when it's absent.
- Personal/hardware profiles (vietnamese/unikey, warp, hardware-*, displaylink, …) stay opt-in as
  before. Nothing personal is installed unless you pick it.

## [0.29.3] — 2026-06-29

### Fixed — serena MCP "Transport closed"
- **serena's executable was renamed.** The registered command `uvx … serena-mcp-server` no longer
  exists (serena now ships `serena` with a `start-mcp-server` subcommand), so the MCP process exited
  instantly → omp reported `serena: Transport closed`. Now registers
  `uvx … serena start-mcp-server --context claude-code` (`ide-assistant` was also deprecated). Verified
  it launches (22 tools exposed, no error).
- **MCP registration now self-heals.** `register_omp_mcp` previously skipped any server already in
  `mcp.json`, so a stale entry never got corrected. It now updates in place when the command/args
  changed, and **`8sync harness up` also refreshes MCP servers** (was init/bare-harness only) — so
  `8sync harness up` fixes the stale serena entry on existing machines.

## [0.29.2] — 2026-06-29

### Fixed — Context page is now correct for ALL models (not just GLM)
- **Per-model context window.** `/api/context` hardcoded a 1,000,000-token window, so models with
  a smaller real window (e.g. `claude-haiku-4-5` 200k, `glm-4.x` 131–205k) showed an artificially
  low % and looked like they never hit the compaction threshold — while 1M models (glm-5.2,
  claude-opus) looked fine. Now the window is parsed per active model from `omp models` (cached via
  `LazyLock`), so the %, threshold marker, and "will compact" are accurate for every model. Falls
  back to an explicit `assumed` estimate only when the model isn't in omp's catalog.
- **Honest compaction copy.** omp's threshold compaction is **turn-triggered** (fires after a
  completed turn / safe mid-turn point once usage exceeds `thresholdPercent` of the real window) —
  not a hard cap, so a paused/ended session legitimately sits above the line until resumed. The page
  now says "compacts on next turn", flags idle/ended sessions (`stale`), surfaces the explanation,
  and only shows the "assumed window" badge when the window is truly unknown.
- **`build.rs` shipped stale FE.** It rebuilt the Vite bundle only when `web/dist` was *missing* and
  watched only `web/dist`, so edits to `web/src` were silently embedded stale. Now it rebuilds when
  any FE source is newer than dist and emits `rerun-if-changed` for `web/src` + configs.

## [0.29.1] — 2026-06-29

### Fixed — dashboard project switcher
- **Switching projects now actually switches the data.** `activate` only wrote an advisory
  `web-session.json`; every handler still read `detect_current_project_root()` (the launch cwd),
  so pages never changed. Now `apply_active_project` chdir's into the activated project (at startup
  + on activate) so all cwd-based handlers (State/Context/Skills/Memory/Rules/Submodules/Workflow)
  resolve to it. Verified in-browser: switch → State path + content + trigger label all update.
- **`/api/projects` cleanup** — dedup by resolved path; drop junk slugs (no session file / non-dir);
  widened the green-dot "active" window to 2h + added a `current` flag for the project being viewed
  (a project open but idle >30 min now shows correctly).

## [0.29.0] — 2026-06-29

### Added — `8sync harness web` dashboard: full redesign + Models/Projects
- **Models page** (`/api/models` get+post) — view/edit the adaptive model routing live: `[roles]`
  (default/plan/smol/slow) + `[tasks]` (plan/review/debug/code/trivial), inline selects write
  `~/.config/8sync/models.toml` immediately. Surfaces the routing philosophy: **thinking → Opus**
  (plan/review/debug/vision), **mechanical → GLM** (code/edit/default/trivial).
- **Project switcher** (`/api/projects`) — sidebar-top dropdown lists every omp project with a
  green (active) / gray (off) status dot; activate + refresh without `cd`.
- **Workflow templates** (`/api/workflows/templates`) — 3 starter graphs (research→plan→build,
  review, qa) loadable in the editor.
- **Markdown rendering** — new XSS-safe renderer (`web/src/markdown.tsx`); State/Memory/Context
  now render headings, lists, GFM checkboxes, code, emphasis (was raw text).

### Fixed
- **serena engine showed "off" wrongly** — detection now checks `mcpServers.serena` in
  `~/.omp/agent/mcp.json` + `uvx`/`uv` on PATH (serena is uvx-launched, no PATH binary), not
  `which serena`. Reports `{present,registered,runner}`.
- **Context window honesty** — `/api/context` now exposes `assumed:true`, `windowTok`,
  `thresholdPct`, `willCompact`; the FE labels the 1M window as an estimate (no false precision).
- **Workflow canvas** — react-flow viewport fixed (was a tiny broken box) to a usable 560px panel
  with fit/zoom.

### Changed
- Dashboard FE redesigned to a product-register design system (impeccable): solid surfaces,
  violet brand preserved, legible chips, grouped nav, dark + light. 14 pages, zero console errors.

## [0.28.0] — 2026-06-29

### Changed — ONE command: `/auto` (retired `/gs`)
- **Unified the autonomous entry to a single `/auto`** — removed `/gs` (command + skill +
  `ensure_gs_command` + all wiring + help/force-load refs). `/auto` (8sync-engine) is the only
  automation path; `deploy::cleanup_legacy_gs` removes the retired `/gs` from machines that had it.
- **`/auto` upgraded to gsd-pi-grade** (grounded in `reference/gsd-pi`): research INTEGRATED into
  planning (codegraph/cbm/serena scout + feynman/deep-research), fresh scoped context per task,
  mechanical verify-gate, hard **Closeout** (full suite + QA/UAT in a browser + independent re-review
  vs DoD + doc-hygiene), and a context-budget/handoff rule.
- **Verify UI for real**: web → `browser` at the dev URL; **Tauri v2 / WRY-WebKit desktop → run with
  its web-inspector/remote-debug port + point the same `browser` tool at it**.
- **`harness up` now deploys the full harness** (APPEND_SYSTEM + `/auto` engine + workflow), matching
  bare `8sync harness`.

### Added
- **`8sync harness model`** — view/edit `~/.config/8sync/models.toml` (single model-routing source):
  bare shows roles+tasks; `8sync harness model <key> <value>` sets one (e.g. `review opus`). omp
  resolves names fuzzily + falls back to an authenticated model.

## [0.27.0] — 2026-06-29

### Added — adaptive model routing

- **Per-prompt model selection** (no more single fixed model). `assets/configs/models.toml`
  (deployed → `~/.config/8sync/models.toml`) maps `[roles]` default/plan/smol/slow + `[tasks]`
  plan/review/debug/code/trivial → models (defaults: codex main · glm plan · opus review/debug ·
  haiku smol). New `crate::models` classifies the prompt heuristically and passes omp
  `--model/--plan/--smol/--slow` (omp resolves fuzzy: "glm","codex","opus"). Wired into
  `8sync ai` (+`--model` override) and `8sync .` (resume flags). omp owns the catalog — 8sync only steers.

### Added — gsd-pi-style automation engine (on omp core, no patch)

- **`8sync-engine` omp extension** (`~/.omp/agent/extensions/` + project) — durable slice/task
  state machine (`.cache/8sync/engine/state.json`) + model-callable tools `engine_plan/status/
  next/verify/advance/worktree`. **Code-enforced** verify-with-retry gate (counts attempts,
  BLOCKs at maxRetries — the agent can't skip it) and git worktree open/squash-merge/remove.
- **`/auto` command** orchestrates the engine to run a goal to DONE (right-sized, token-lean).
  Closes the gsd-pi gaps (verify/worktree as CODE, not prose). `/gs` stays a lighter skill.

### Added — context engineering (always-read + serena + tunable compaction)

- **`APPEND_SYSTEM.md`** deployed to `~/.omp/agent/` — RULE #0 (code-intel before grep/CRUD) +
  always-on skill manifest (name·purpose·ref-path) injected into EVERY system prompt (never
  compacts away) → fixes "skills/rules defined but ignored past 50%". Recall hook rewritten to
  the LIVE half (skill index + STATE Current/Next).
- **serena MCP** registered (`ensure_serena_mcp`, via `uvx`) — symbol-level code intel, prioritized
  over native search/file-CRUD. Surfaced on the dashboard Engines page + force-load RULE #0.
- **`8sync harness compaction [pct]`** — view/set `compaction.thresholdPercent` (auto-clean at 50%).

### Added — terminal: kitty glass + helix + docker (Stage A defaults)

- `8sync setup` now installs **kitty + helix + docker + docker-compose + JetBrains Nerd font** and
  deploys a **glass kitty theme** (`~/.config/kitty/8sync.conf`, included from kitty.conf — no clobber):
  transparency + blur + wallpaper + 3-pane split keymaps + violet accent. Wallpaper deployed to
  `~/.config/8sync/wallpaper.png` from `assets/wallpapers/default.png` (bundled) or `[ui].wallpaper_url`.
  Transparent helix config (`base16_transparent`) deployed if absent. `8sync doctor` checks hx/kitty/docker.

### Changed / Fixed

- **Web dashboard redesigned** to a dark glassmorphism / Hyprland aesthetic (translucent blurred panels,
  layered gradient, icon sidebar, refined type scale, light-mode + a11y fallbacks). 13 pages, react-flow
  workflow editor intact. Browser-verified: all pages render, zero console errors. (`web/src/{styles.css,App.tsx,icons.tsx}`)
- **`build.rs`** now builds the FE with bun → pnpm → npm (first available); on no toolchain it embeds a
  styled, instructive fallback page instead of a bare one-line stub.
- **Helix command fixed to `hx`** — dropped the dead `"helix"` fallback (Arch ships `/usr/bin/hx`, no
  `helix` binary); `note`/`find` now share one `pick_editor()` ($VISUAL→$EDITOR→hx→vi).

## [0.26.0] — 2026-06-27

### Added (dashboard FE enhancement)

- **Context tracker page** — live omp session token usage for the current project (reads the
  session JSONL's `contextSnapshot.promptTokens`, auto-refresh 4s). Gauge + 50% threshold marker +
  **compaction-observed badge** (detects the token drop = empirical proof auto-compact fired). `/api/context`.
  Verified real: tracks this very session 440k→447k live; detected last compact at 575k.
- **MCP servers page** — visualize `~/.omp/agent/mcp.json` (name/command/args/present). `/api/mcp`.
- **Rules CRUD page** — list/add/delete omp rule files (`.omp/rules/*` project + `~/.omp/agent/rules/*`
  global), add from pasted content (link/file/folder source). `/api/rules` (+add/delete).
- Dashboard now 12 pages (State · Context · Skills · Memory · Engines · Bench · Readiness · Workspaces ·
  Team · Submodules · MCP · Rules). Anti-slop per impeccable (no gradient text / glassmorphism / over-round;
  verb+object buttons). Browser-tested (Chromium): all pages render real data, Context live-tracking +
  Rules add-end-to-end verified.


## [0.25.0] — 2026-06-27

### Added (Phase A — anti-forget)

- **Anti-forget: compaction@50% + idle + recall hook.** `8sync harness` giờ ensure
  `~/.omp/agent/config.yml` có `compaction.thresholdPercent: 50` + `idleEnabled: true`
  (snapcompact vẫn là default), và deploy `~/.omp/hooks/pre/8sync-recall.ts` — hook inject
  lean ref bundle (skill index + live STATE) tại mỗi `before_agent_start` + vào mọi
  compaction summary → agent giữ index skills/rules/workflow qua 50% context & sau compact.
  `8sync doctor` báo "anti-forget ON/OFF". Key-based config detection (robust khi omp
  rewrite/strip comments config.yml — bỏ sentinel strategy). Verified: omp 16.2.1 load OK.

### Added (Phase B — `8sync harness web`)

- **`8sync harness web`** — dashboard Vite+React (embedded qua rust-embed) do axum serve tại
  `http://127.0.0.1:8731` (`--port`, `--no-open`). API: `/api/state` · `/api/skills` (list + toggle
  tier qua `agents/skills.toml`) · `/api/memory/:file` (get/set, allowlist) · `/api/engines`
  (codegraph/cbm/headroom/**serena** + mnemopi) · `/api/bench` · `/api/eval`. Refactor B5: tách
  `bench_metrics()`/`eval_project_data()` (home: &Path) cho cả CLI lẫn web reuse. Build.rs tự build
  FE qua pnpm khi thiếu + stub fallback. Deps: axum 0.7 + tokio + tower-http (override có chủ đích
  rule "tránh tokio" trong AGENTS.md §8, gated `harness web`). Verified real: 6 endpoint trả data
  sống (eval 96% 28/29, bench A1 PASS).

### Added (Phase C — full manage)

- **Workspace + team + submodule + skill install** qua dashboard: `/api/workspaces` (list omp
  profiles + project + activate ghi `web-session.json`), `/api/team` (subagent roster 8 loại +
  readiness reuse eval_project_data), `/api/submodules` (parse `.gitmodules` + add/pull/remove qua
  git shell-out), `/api/skills/add|update` (self-shell-out `8sync skill`). FE: 3 page mới (Workspaces,
  Team, Submodules) + nav. Verified real: workspaces/team/submodules trả data, skill add validate spec.

## [0.24.0] — 2026-06-25

### Added

- **`8sync harness eval --project` — agent-team readiness scorecard (% per vai).** Deterministic + offline:
  chấm capability coverage trên repo hiện tại theo dev · qa/testing · research · ba/po · fe · be · docs ·
  memory/learn · token-opt (engine on PATH + skill present + memory spine + stack signals). Honest READINESS
  (team được trang bị gì Ở ĐÂY), KHÔNG phải output-quality (đó là `harness eval` loop probe model+network).
  Run thật: su-code 89%, 8syncdev-pro-v2 79%.
- **`token-bench` skill (bundled) — chứng minh token-saving của code-intel trên repo thật.**
  `scripts/token_bench.py` (uv/PEP723, stdlib-only): mỗi symbol thật so codegraph-query+slice vs
  grep+read-whole-file, có def-kind correctness gate. Đo trên codebase lớn thật: 8syncdev-pro-v2 −96.6%,
  gsd-pi −78.6% (range 54–98%; symbol dùng rộng / file lớn → 95–98%), correctness gsd-pi 10/10. Cần
  ANSI-strip (codegraph tô màu cả khi pipe). Bundled qua `deploy.rs` (16 skills).
- **6 reference submodule** (inspect/track upstream; deinit, content gitignored): gstack · gsd-pi ·
  agent-reach · addyosmani/agent-skills · DietrichGebert/ponytail · **DeusData/codebase-memory-mcp**.
- **`outputs/agent-team-workflow-automation-plan.md`** — operating plan để vận hành su-code như một agent
  team: map sprint 23-specialist của gstack + loop slice/auto/worktree của gsd-pi lên `/gs` + skills +
  subagents, kèm **UI/UX Design Lane** riêng (impeccable + Clouds F + **Lighthouse 4-tiêu chí quality gate**).
- **`8sync` help dẫn đầu bằng AI TEAM (harness + `/gs`).** Cheatsheet (`8sync` / `8sync help` / `8sync flow`)
  trước đây mở đầu bằng install + vibe loop, **không hề** nhắc `8sync harness` (all-in-one) lẫn `/gs` (team
  lead) — giờ là section ĐẦU TIÊN. Fix dòng stale: `8sync skill sync` (đổi thành `skill update`; regen là
  `8sync harness`) và `8sync up` ("binary + omp" → chỉ 8sync; omp qua `omp update`).
- **`/gs <goal>` scope handshake (chỉ assisted).** Goal medium+/mơ hồ không dive thẳng: GS ground
  (codegraph/cbm) rồi đề xuất **2–4 phương án cụ thể** (scope · team size + roles/skills · effort · tradeoff,
  rút từ bench senior: impeccable+Lighthouse / senior-frontend / code-review-and-quality / senior-security /
  performance-optimization) kèm recommended default + 2–4 câu hỏi sắc qua `AskUserQuestion` — một vòng rồi
  chạy. `auto` vẫn unattended (no questions); trivial/small bỏ qua handshake. (`assets/commands/gs.md` §1b.)
- **`8sync harness eval` báo `%`** (`eval.rs:114`) — `score: N/M passed (X%)`. 3/3 = 100%.
- **`outputs/omp-customization-memory-platform-research.md`** — research grounded từ omp docs: cơ chế nhớ
  THẬT = **Mnemopi memory + cbm + spine**, dùng **model API (không local — máy yếu vẫn chạy)**, thay cho ngộ
  nhận GGUF/fine-tune (không khả thi); custom command/workflow trên ĐÚNG base omp (`.omp/commands` native,
  update không conflict); submodule auto-pull là ngộ nhận (skill đã auto-latest qua manifest+`harness up
  --pull`); agent-reach = capability layer (đọc internet), thêm làm skill.
- **Mnemopi memory wired vào `8sync harness`** (`deploy.rs::ensure_mnemopi_memory`) — `harness`/`init` bật
  `memory.backend: mnemopi` (+ `scoping: per-project-tagged`, `llmMode: smol` API, `noEmbeddings: true` FTS,
  `polyphonicRecall`) trong `~/.omp/agent/config.yml` (idempotent sentinel-block, KHÔNG clobber `memory:` của
  user). 0 local model → máy yếu chạy. `8sync doctor` báo memory ON/OFF (`doctor.rs`). Verified: omp 16.1.20
  load config OK, doctor "mnemopi memory ON". Tradeoff: recall inject token/phiên (user đã chốt bật).
- **5 reference repo = git submodule** (`reference/`, content gitignored, deinit mặc định): gstack · gsd-pi ·
  **agent-reach** · **addyosmani/agent-skills** · **DietrichGebert/ponytail**. Đăng ký để inspect/track upstream
  (`git submodule update --init --remote reference/<name>` để pull-latest đọc khi cần). Submodule = nguồn-tra-cứu;
  deploy auto-latest cho skill LIVE vẫn qua manifest + `harness up --pull`.

### Changed

- **Declutter skill registry — bỏ pack research `companion-inc/feynman` (20 skill on-demand).** Manifest
  (`agents/skills.toml` committed + `~/.config/8sync/skills.toml` machine-local) đăng ký 20 skill
  research/ML/academia (paper-writing, ml-training-recipe, literature-review, runpod/modal-compute,
  peer-review, jobs, eli5, …) — sai domain cho một coding harness + là prefix noise inject vào AGENTS.md
  mỗi phiên. Cắt cả 20 (collection re-pull là all-or-nothing theo URL — `update.rs:49`, giữ 1 cái là
  re-clone cả pack). Kết quả: on-demand 55 → 35, force-load prefix ~1998 → ~1717 tok, deferred −5k tok
  (`8sync harness bench`), A1 stable-prefix PASS. Giữ nguyên addyosmani coding-eng + design payload
  (impeccable/taste/assp) + bundled always-on.

## [0.23.0] — 2026-06-24

### Added

- **`8sync harness eval` — loop quality probe.** Runs a fixed task-suite through omp non-interactively
  (`omp -p --no-session --auto-approve`) and scores each task with a deterministic `verify.sh` (the
  verifier OWNS the assertion, so the agent can't game the check). Three bundled fixtures:
  `fix-failing-test` (correct a wrong impl until `cargo test` is green), `add-fn-with-test` (implement
  `slugify`; the verifier appends the assertions), `locate-symbol` (answer `path:line` for a symbol).
  Writes a JSON scorecard + a `--baseline` reference into the gitignored `.cache/8sync/eval/`; later
  runs print the pass-count delta vs baseline. Model + network, non-deterministic — a periodic quality
  SIGNAL, not a CI gate. Verified end-to-end: 3/3 on this machine.

### Changed

- **`/gs` L3 worktree isolation is now concrete.** The guardrail named "git-worktree isolation" with no
  mechanism; it now prescribes the exact flow — `git worktree add .gs/wt/<slug> -b gs/<slug>`, implement
  + verify + commit on that branch inside it, then `git worktree remove` (merge/PR only if asked); never
  edit `main`'s working tree directly. (`.gs/` is gitignored, v0.22.0.)

## [0.22.0] — 2026-06-24

### Added

- **`8sync harness audit` — code-backed doc-hygiene** (was prompt-only advice with zero code behind it).
  Scans committed docs (AGENTS.md/CLAUDE.md/README/CHANGELOG + `agents/*.md`) for **stale path references**
  (repo-relative paths in docs that no longer exist), **oversized docs** (>400 lines / >120-line force-load
  block), and **30-day churn hotspots** (history-awareness — docs near churned code are likeliest stale).
  Report-only: never auto-deletes (heuristic; illustrative paths flagged "review before editing"). Skips
  absolute / `~`-rooted / URL paths so the harness's own machine-generated refs don't false-positive.
  `8sync doctor` surfaces a one-line summary; `/gs` + the `gs` skill doc-hygiene step now run the audit
  instead of eyeballing.
- **`8sync doctor` AI-engine health check** — verifies the token-optimization stack is installed AND
  registered with omp ("luôn xài"): codegraph (local index) · codebase-memory-mcp (semantic graph) ·
  headroom (output compression). A missing or unregistered engine silently defeats STEP 0 token
  discipline, so doctor now flags it with the one-command fix (`8sync harness`).

### Fixed

- **codegraph STEP 0 verbs were wrong** in the force-load prefix, the subfolder-index block, and the
  KNOWLEDGE breadcrumb: they taught `codegraph search/deps/defs`, none of which exist. Corrected to the
  real CLI surface `codegraph query/callers/callees/impact` (verified against codegraph 0.9.6) so the
  agent's first explore call doesn't error out.
- **Duplicate always-on skill in the force-load list.** A stale/external `karpathy` dir alongside the
  canonical `karpathy-guidelines` (identical frontmatter `name`) made the skill appear twice — once in
  CORE, once in on-demand. `build_force_load` now dedups by frontmatter name, keeping the higher-ranked
  dir, so each logical skill is listed exactly once. Future-proof against any dir/name collision.
- **impeccable setup scripts couldn't run under 8sync's layout.** The bundled design skill referenced
  `.agents/skills/impeccable/scripts/*.mjs` (leading dot) but 8sync mirrors skills to `agents/skills/`
  (no dot). Fixed 28 references across SKILL.md + 4 reference docs → `agents/skills/`.

### Changed

- Managed `.gitignore` block now ignores `.gs/` (per-run worktree + `/gs stop` marker — machine-local).

## [0.21.0] — 2026-06-24

### Changed

- **`/gs` redesigned to right-size effort (fixes the post-`/gs` quality regression).** Eval +
  deep-research (`outputs/gs-eval-improve-research-brief.md`) found the drop was process
  over-engineering, not tokens (`harness bench`: ~8.5k upfront, 79% saved, KV-cache stable):
  the 93-line command forced a team + full Closeout on every task and `auto` "never asked".
  - **Right-size first** — classify trivial/small → **solo** (no team, no Closeout) · medium →
    solo + one verifier · large → full loop + roles + Closeout. A team is the exception you justify
    (Cognition/Anthropic: single-agent default; multi-agent only when it clears the bar).
  - **Solo-by-default delegation** — subagents only for parallel-independent / context-isolation /
    specialization; scoped objective + summary return (never free-form, never inline transcript).
  - **Autonomy confidence-gated** — strong `auto`, but a high-stakes hard-to-undo low-confidence call
    is now a blocker (Anthropic 2026: "agents learning when to ask"); prefer reversible, never compound.
  - **Doc-hygiene step** — detect stale paths / junk / superseded docs → fix or **delete** (no addition
    without the matching deletion); keep docs lean. Stale docs poison agent context.
  - **Codebase-history** — `git log/blame` + DECISIONS + cbm `detect_changes` before load-bearing edits.
  - **Leaner command** — `assets/commands/gs.md` 93 → 56 lines (lower constraint density → better
    instruction-following); full protocol stays in the `gs` skill (progressive disclosure).

## [0.20.1] — 2026-06-23

### Fixed

- **`/gs auto` actually runs unattended now.** Added an **Autonomy contract** to the `/gs` command +
  `gs` skill: in `auto`/L3 the agent NEVER calls `ask` or stops on ambiguity — it resolves unknowns by
  research (codegraph/cbm → `agents/*`/PLAYBOOKS → skills → `web_search`/`autoresearch`/`deep-research`),
  picks the boring/reversible option, logs it under a new `## Assumptions` section in `agents/STATE.md`,
  and proceeds. "Blocker" is tightened to ONLY missing credential / external approval / destructive-
  irreversible action; design choices, naming and scope are no longer stops. Note: a slash command
  cannot bypass omp's approval gate — keep `tools.approvalMode: yolo` (default) for true unattended runs.
- **`/gs` argument hint.** Added `argument-hint` frontmatter and front-loaded the description with
  `[auto | <goal> | status | next | stop]` so the autocomplete dropdown shows the modes when you type
  `/gs ` (omp renders per-argument hints only for built-ins; the description is what surfaces for
  file-based commands).
- **QA + test are now first-class gates in `/gs`.** Per-slice verify-gate explicitly runs tests + a QA
  pass and forbids skipping/weakening tests; added a mandatory **Closeout** step — full test suite +
  end-to-end QA + independent re-review against the Definition-of-Done + a handoff summary — that must
  pass before the loop reports "done". Never hands back unverified work.

### Added

- **Reference submodules `reference/gstack` + `reference/gsd-pi`** (git submodules, MIT) for studying
  the engineering-team + autonomous-loop patterns that informed `/gs`. Pointers are committed
  (reproducible) but the working trees are **deinitialized by default** so they never bloat the
  codegraph/cbm index (codegraph honors no exclude/ignore — populating them ballooned the index to
  ~3k files / 110 MB). Study on demand: `git submodule update --init reference/<name>`; re-shrink with
  `git submodule deinit -f reference/<name>`. `reference/` is also gitignored as a cbm-index guard.

## [0.20.0] — 2026-06-23

### Added

- **`/gs` — one-command autonomous engineering-team loop (omp slash command).** `/gs <goal>` plans +
  runs, bare `/gs` resumes, `/gs auto` runs unattended (L3), `/gs status|next|stop`. Drives the loop
  off `agents/STATE.md`: plan → delegate to specialist roles (`task` subagents / gstack roles if
  installed) → verify-gate → commit → record (KNOWLEDGE/PLAYBOOKS) → advance until Definition-of-Done
  or a blocker. Token-lean (codegraph + codebase-memory-mcp + headroom mandatory) and guardrailed
  (verify-gate before commit, worktree isolation + no push/PR at L3, hard-stop via `/gs stop`).
  Modeled on gsd-pi `/gsd auto`.
- **Deploy + team-sharing.** `8sync harness`/`init`/`up` write it to `~/.omp/agent/commands/gs.md`
  (global) and `<repo>/.omp/commands/gs.md` (committed → whole team gets `/gs`). New on-demand `gs`
  skill documents the protocol; `8sync harness up --timer` runs it 24/7.

## [0.19.0] — 2026-06-23

### Changed

- **Loop engineering v2 — Phase A (token & stable-prefix discipline).**
  - Force-load block (`inject.rs`) + master `00-force-load.md` split always-on skills into
    **CORE** (codegraph · karpathy · ponytail · 8sync-cli — đọc body upfront) và **SPECIALIST**
    (assp · impeccable · taste · image-routing — biết khả năng, đọc body khi task khớp /
    progressive disclosure). Thu nhỏ tập đọc-ngay; `impeccable` vẫn bắt buộc ngay khi có việc UI/design.
  - `headroom_compress` nâng từ khuyến nghị → **bắt buộc** cho output > ~50 dòng (STEP 0 + invariants).
  - KNOWLEDGE breadcrumb (`memory.rs`) bỏ timestamp `epoch:` volatile → byte-stable giữa các lần
    `harness` (thân thiện KV-cache, hết git churn). `now_stamp()` vẫn dùng cho tên file archive.
  - Plan + provenance: `outputs/harness-loop-engineering-v2-plan.md`.
- **Loop engineering v2 — Phase B (live memory & recitation).**
  - `agents/STATE.md` seeded as a structured **live plan** (Goal · DoD · Checklist · Current ·
    Next · Open-questions · Handoff) — recitation anchor (Manus todo.md pattern): read at session
    start, rewritten at each phase boundary to keep the plan in recent context.
  - Loop section (`00-force-load.md`) + generated block (`inject.rs`) gain **recitation**,
    **compaction** (near-limit → structured handoff to STATE + lessons to KNOWLEDGE → reinit, with
    `headroom_compress` as summarizer), and **budget-awareness** rules.
  - `harness bench` now counts the memory spine in the upfront budget (more honest accounting).
- **Loop engineering v2 — Phase C (maker/checker + Reflexion).**
  - Loop section + generated block: `task` implementer ↔ **independent verifier** (build/test in
    its own context, verify-gate before commit), explicit objective/boundaries/output per subagent,
    share-full-trace for dependent work, parallel only when subtasks are independent.
  - **Reflexion failure-capture**: a failed verify writes a `failure:` entry to KNOWLEDGE (symptom
    + cause + fix); recent failures are read at session start to avoid repeating them.
- **Loop engineering v2 — Phase D (procedural memory / playbooks).**
  - `agents/PLAYBOOKS.md` seeded (Voyager-style skill library): validated multi-step procedures
    distilled into reusable runbooks indexed by a `When:` line — retrieved + adapted, not re-derived.
  - Memory tiering: KNOWLEDGE = verbal lessons · PLAYBOOKS = verified procedures · DECISIONS = ADR.
    `harness bench` now counts PLAYBOOKS in the spine (6 files).
- **Loop engineering v2 — Phase E (phased autonomy + guardrails).**
  - L1 report · L2 assisted · L3 unattended defined, with guardrails (verify-gate before commit,
    gitleaks, commit scoped to `agents/`+docs, no auto `push`/PR at L3). `harness up --timer`
    per-tick job documented (read STATE → Next → verify → update spine → optional commit).

### Added

- **`8sync harness bench`** — deterministic loop-engineering benchmark (no model calls): upfront
  context budget (force-load prefix + CORE skill bodies) vs deferred (SPECIALIST + on-demand),
  the A2 progressive-disclosure saving, and an A1 KV-cache stable-prefix gate. Refactors a shared
  `inject::build_force_load()` (single source of truth for inject + bench). Baseline on this repo:
  upfront ~5.5k tok vs naive ~37.9k tok → **85% upfront cut**; A1 PASS.

## [0.18.1] — 2026-06-23

### Fixed

- **`8sync harness init` now pulls registered manifest skills** — `init` calls
  `skill update` against `agents/skills.toml` (git collections like `feynman`:
  deep-research, autoresearch, …) before mirroring, making it a true superset of
  bare `8sync harness`. Previously `init` only deployed the bundled skills + 2
  hardcoded external packs (ponytail, addyosmani), so manifest-only skills never
  reached `agents/skills/` via `init` — only bare `8sync harness` / `up --pull` did.

## [0.18.0] — 2026-06-21

### Added

- **Headroom context-compression wired as an omp MCP** — `8sync harness`/`init` auto-installs
  `headroom-ai[mcp]` (uv → pipx → pip fallback) and registers it in `~/.omp/agent/mcp.json`
  (`headroom mcp serve`, stdio). Tools `headroom_compress` / `headroom_retrieve` / `headroom_stats`
  compress long tool outputs / logs / diffs 60–95% before they reach the model. Force-injected into
  STEP 0 + `00-force-load.md`. Researched alongside PixelRAG + LocateAnything3D — **skipped**:
  PixelRAG (screenshot-RAG) overlaps `8sync shot`/`read`/`browser`; LocateAnything3D is a 3D-vision
  model (out of scope for a coding harness).

## [0.17.1] — 2026-06-21

### Fixed

- **Skills now propagate to other machines.** `8sync harness` / `skill update` write a
  committed project manifest `agents/skills.toml` (mirroring the machine-local registry) and
  read it back on any machine — so a fresh clone re-pulls the exact same skills. Previously only
  the machine-local `~/.config/8sync/skills.toml` recorded `skill add`-ed sources, so custom
  skills (e.g. git collections like feynman) never reached a second machine via harness — only
  the 15 binary-embedded skills + 2 hardcoded external packs did. (`agents/skills.toml` is a
  file, so it travels even when the `agents/skills/` directory is gitignored.)

## [0.17.0] — 2026-06-21

### Added

- **codebase-memory-mcp = first-class code-intelligence engine** — `8sync harness`/`init`
  auto-installs the binary (upstream installer, binary-only), sets `auto_index true`, and
  registers it as an omp MCP server in `~/.omp/agent/mcp.json` (idempotent, preserves other
  servers). `harness`/`up` index the repo. Mirrors `ensure_codegraph` — zero manual MCP config.
- **Code intelligence FIRST (STEP 0)** — the injected force-load block + `00-force-load.md`
  mandate codegraph + codebase-memory-mcp BEFORE grep/read for all code exploration
  (~99% token saving); raw `Read` only for read-before-edit.
- **Loop-engineering principles** (Addy Osmani / Boris Cherny) in `00-force-load.md`:
  STATE/KNOWLEDGE spine, maker/checker via `task` sub-agents, verify-gate, phased
  L1→L3 autonomy via `harness up --timer`.

## [0.16.0] — 2026-06-21

### Added

- **`8sync harness` (bare) = ONE command** — idempotent driver that makes a project
  agent-ready in a single pass: deploy/update skills + mirror (additive) + inject
  force-load + seed memory & gitleaks hook + consolidate learnings + re-index codegraph.
  `harness init` = explicit full bootstrap (progress UI); `harness up` = light refresh;
  `harness up --timer 30m` = background loop.
- **Additive skill mirror + `--force`** — `harness`/`harness init` never clobber an
  already-vendored (possibly edited) `agents/skills/<name>`; only missing skills are
  written. `harness init --force` re-mirrors everything. `harness up` now also seeds
  the gitleaks pre-commit hook.
- **`8sync skill update [name]`** — re-pull registered skills from their recorded
  source in `skills.toml` (git URL / `builtin:` / `path:`). Git sources are deduped
  per URL (a collection repo is cloned once, all sub-skills reinstalled); best-effort
  per source (offline / missing `git` warns + skips, exit 0). `name` updates just one.
- **`8sync harness up --pull`** — refresh AND re-pull every registered skill before
  re-injecting. Default `up` stays network-free + fast (timer/loop unaffected).
- **`8sync harness up --commit`** — close the self-learning loop: stage + `git commit`
  ONLY the refreshed agent memory (`agents/`, `AGENTS.md`, `CLAUDE.md`, `CHANGELOG.md`,
  `.gitignore`; never your code) so learnings persist to git in the same pass. No-op
  when nothing changed (no empty-commit spam on `--timer`); default off.
- **`8sync harness help`** — one-screen cheatsheet: commands, skill tiers, the
  commit-vs-ignore file taxonomy, and the new-machine runbook.
- **Portability**: `harness init`/`up` seed a managed `.gitignore` block (between
  `# >>> 8sync (managed) >>>` sentinels) — ignore derived (`.codegraph/`, `.cache/8sync/`)
  + secrets (`.env`, `.env.*`, keep `!.env.example`), keep agent memory + `agents/skills/`
  committed. `8sync doctor` now errors if any durable `agents/*.md` / `AGENTS.md` /
  `CHANGELOG.md` is gitignored (learnings wouldn't survive a move to a new machine).
- **`agents/KNOWLEDGE.md`** seeded with an append-only `## Learnings` zone below the
  managed breadcrumb block (overwritten each `harness up`) so learnings persist.

### Hardened (research-driven — see `outputs/harness-selfimprove-research-brief.md`)

- **Lean force-load context** — the injected on-demand skill list is now names+path
  only (one line each); full descriptions live in each `SKILL.md` (progressive
  disclosure). `8sync doctor` warns if the `AGENTS.md` force-load block exceeds 120
  lines. *Why:* Gloaguen et al. arXiv 2602.11988 (138 repos) — bloated/duplicative
  context files cut agent success and add >20% inference cost.
- **Skill version pinning (lockfile)** — `8sync skill add <url>@<ref>` pins a git
  commit/tag/branch; the resolved SHA is recorded as `rev` in `skills.toml` and
  `skill update` checks out exactly that rev (reproducible). Unpinned entries track
  latest. *Why:* mirrors Claude Code plugin marketplace (SHA pin = reproducible).
- **Secret-scanned auto-commit** — `harness up --commit` runs `gitleaks protect
  --staged` (if installed) and ABORTS on detection; `harness init` installs a
  gitleaks pre-commit hook (non-destructive); `8sync doctor` reports gitleaks.
  *Why:* GitGuardian 2026 — AI-assisted commits leak secrets ~2× baseline.
- **Bounded memory (anti context-rot)** — `harness up` consolidates the
  `## Learnings` zone past ~200 lines, archiving older entries to `agents/archive/`
  with a pointer. *Why:* 4-lever consolidation; "remember everything → remember nothing".
- **Verifier-gated learnings** — seeded `KNOWLEDGE.md` instructs prefixing entries
  `validated:` (test/build confirmed) vs `hypothesis:`. *Why:* Reflexion verifiability
  constraint — no reliable improvement beyond what's objectively verified.

## [0.15.1] — 2026-06-17

### Added

- **impeccable house design references** (`assets/skills/impeccable/references/house/`): bundled
  `frontend-agent-workflow.md` (senior coding-agent workflow) + `clouds-f.md` (senior front-end
  orchestration) + `clouds-f-rules/*.mdc` (design-redesign / responsive / performance / fix /
  refactor / security keyword routers). impeccable's SKILL.md auto-references them.

### Changed

- **Emphasised `impeccable` as THE house design system** across the force-load flow (AGENTS.md /
  CLAUDE.md block, `00-force-load.md`, sub-folder index, KNOWLEDGE breadcrumb): mandatory for any
  UI / design / redesign / audit, read with `references/house/*`.

## [0.15.0] — 2026-06-16

### Added

- **`8sync harness` verb** — one command to stand up the full agent harness.
  - `harness init`: deploy mọi bundled skill + codegraph binary + external skill
    packs (best-effort clone), mirror vào `agents/skills/`, `codegraph init`,
    seed `agents/*` memory + `CHANGELOG.md`, inject force-load vào AGENTS.md/CLAUDE.md
    + một index gọn vào **mọi sub-folder code** (progressive disclosure). Có progress
    UI `[i/N]` + thời gian.
  - `harness up`: refresh theo state hiện tại (re-inject + refresh `agents/KNOWLEDGE.md`
    breadcrumb + `codegraph index`). `--loop <dur>` chạy foreground; `--timer <dur>|off`
    cài/gỡ systemd **user timer** (đúng cách cho chạy nền, mirror `8sync clean --timer`).
- **6 bundled skill mới**: `ponytail` (always-on, lazy-senior YAGNI), `code-review-and-quality`,
  `senior-security`, `senior-frontend`, `full-flow`, `encore-deploy` (on-demand). Trước đó
  (0.14.x → nội bộ) đã thêm `assp-skill`, `impeccable`, `taste-skill`. Tổng **15 bundled**.
- **Always-on order** (đọc top-down, ưu tiên): codegraph → karpathy → ponytail → assp →
  impeccable → taste → 8sync-cli → image-routing. Inject block dạy rõ *cách tận dụng* từng skill.
- **Tech-gated skills**: `encore-deploy` chỉ hiện trong force-load block khi project dùng
  Encore (`encore.app` / `encore.dev`).
- **Opt-in skills**: `social-growth` (chiến dịch social/branding/lead-gen cho FB/YouTube/TikTok,
  page setup, insight, monthly plan + target) — KHÔNG auto-bật; bật bằng
  `8sync skill add builtin:social-growth`.
- **`8sync skill add` collection-aware**: clone repo rồi cài mọi `skills/<name>/SKILL.md`
  (vd `addyosmani/agent-skills` 24 skill, `ponytail` full); `builtin:<name>` deploy
  bundled skill từ embedded assets.
- **Sub-folder `AGENTS.md` index** + **`agents/KNOWLEDGE.md` breadcrumb** + **`CHANGELOG.md`**
  seeding tự động, để agent không bỏ sót rule và tự học theo state dự án.

### Changed

- **`8sync skill sync` → `8sync harness init`** (clean cutover, không giữ alias). `skill sync`
  in cảnh báo trỏ sang lệnh mới.
- `crates/cli/src/verbs/skill.rs` (~1340 dòng) tách thành module tree `verbs/skill/`
  (`mod` · `meta` · `discover` · `list` · `spec` · `add` · `gen` · `deploy` · `inject` · `index`),
  mỗi file < 500 dòng. Harness logic ở `verbs/harness/` (`mod` · `init` · `up` · `memory` · `external`).
- `8sync .` giờ cũng inject sub-folder index (nearest-AGENTS.md wins).
- Binary size target: < 4 MB (binary ~3.8 MB stripped, gồm 15 bundled skill).

## [0.14.2] — 2026-06-02

- fix(bt): Bluetooth vanishing after cold boot (USB autosuspend).

## [0.14.1] — 2026-05-31

- clean is project-safe: never touches models / Playwright / download caches.

## [0.14.0] — 2026-05-31

- `8sync clean`: disk/RAM reclaim + CPU/GPU report + periodic timer.

## [0.13.0] — 2026-05-31

- `8sync bt` bluetooth verb; Caelestia desktop install removed.

## [0.12.1] — 2026-05-30

- two-tier skill injection (always-on vs on-demand).
