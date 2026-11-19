# Board — run step of the boss's `/{{CMD}}` workflow

The boss approved the plan on the board (task **{{ID}} — {{TITLE}}**, project `{{ROOT}}`). Continue `/{{CMD}}` (`{{CMD_PATH}}`) in this session from execution to completion. Approved plan: `{{PLAN}}`.

- If this session does not hold the planning run (fresh session), read `{{CMD_PATH}}` and the plan file first. If the thread below ends with a message from the boss after the plan, it is a change request: apply it.
- Do everything the command does after its approval gate: its engine loop (`engine_plan` → `engine_next` → `engine_verify` → `engine_advance`), verify gates, review, completion steps. If you use the engine, call `engine_status` first and continue with the next pending task; never redo committed work.
- Commit on the current branch `{{BRANCH}}` (base `{{BASE}}`). NEVER create, switch or delete branches, never push, never open PRs, never merge: at the boss's acceptance the board pushes the task's commits and opens the merge request. Skip any branch / push / PR step the command describes.
- Headless: no interactive human, `ask` is unavailable. Use `board_ask` only when a decision would change what gets built and nothing settles it, with your recommended default.
- **Finish** by writing `{{RESULT}}` (this run's result file): `{"verdict": "pass", "message": "<what changed, files, how verified>"}`, or `{"verdict": "fail", "message": "<the blocker for the boss>"}` if the work could not be completed. Write `message` in the language of the request. Without the result file the run counts as failed.

Original request:
<request>
{{REQUEST}}
</request>

Conversation between the boss and the team so far (oldest first):
<thread>
{{THREAD}}
</thread>
