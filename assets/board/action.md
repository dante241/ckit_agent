The boss asked for a follow-up action on task {{ID}} "{{TITLE}}" (the task's work itself is done): **{{ACTION}}**

<instruction>
{{INSTRUCTION}}
</instruction>

Do it exactly as you would in a normal omp session in this project:
- Use the project's own skill or command for it, as you normally would: look at the skills available to you and read the matching `SKILL.md` before acting (the Leader suggested: {{SKILL}}). Follow the skill completely, including its follow-up steps (e.g. updating the PMS ticket) when the skill says so, except any `git push` step: the board has already pushed (see below). Without a matching skill, use the project's usual tools (`glab`, `gh`, MCP) directly.
- Writes outside the repo the boss approved: {{WRITES}}. Do not make other external writes.
- You work in `{{CWD}}`, a temporary checkout of branch `{{SOURCE}}` (task branch `{{BRANCH}}`, target `{{TARGET}}`). Never switch, create or delete branches here.
- {{PUSH}}
- Commits this branch carries over origin/{{TARGET}}:
{{COMMITS}}
- Headless: no interactive human, `ask` is unavailable; decide with sensible defaults and say what you chose.
- **Finish** by writing `{{RESULT}}`: `{"verdict": "pass", "message": "<what you did, in the language of the instruction>", "urls": ["<every URL you created or updated, e.g. the merge request>"]}`, or `{"verdict": "fail", "message": "<the blocker for the boss>", "urls": []}`. Without the result file the action counts as failed.

The task's request (context):
{{REQUEST}}
