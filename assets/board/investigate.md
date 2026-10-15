## Your step: Leader — investigate / answer (READ-ONLY, does not use `/feature`)

The boss asked a question or wants research. Answer it — do not plan or build anything.
- Do NOT read the `/feature` skill, do not create planning files, do not edit, create, stage or commit ANY file in `{{ROOT}}` (the board checks `git status` before/after and fails the step if anything changed). Write only `{{RESULT}}`.
- This overrides the always-on memory rules for this step: do NOT update `agents/STATE.md`, `agents/KNOWLEDGE.md`, `CHANGELOG.md` or any other memory file, and do NOT run `codegraph index` (no index → use `read`/`grep`).
- The working tree may be on another task's unmerged branch: run `git branch --show-current`; if it is a `board/t-*` branch, say so in the answer's first line ("đọc trên nhánh … của task khác, chưa merge") and, where it matters, compare with the base via `git show <base>:<path>`.
- Use code intelligence first (codegraph / codebase-memory / serena), then `read`/`grep`; use web search for external facts and cite links. For broad questions you may fan out `scout` subagents.
- Follow-up: if the thread already holds your earlier answers, answer the boss's LATEST message using them as context.

Result file: `{"verdict": "pass", "message": "<the full answer in Markdown, in the request's language: conclusion first, then evidence with file:line / links; end with what you would do next if they want a change>"}`. Use `"need_user"` only when the question cannot be answered without information only the boss has, with ≤3 numbered questions.
