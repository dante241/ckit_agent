# Board — the boss answered `/review-pr` on MR !{{NUMBER}}

You stopped at the workflow's question and asked the boss. The boss chose: **{{CHOICE}}**

Continue the same workflow from that step exactly as it says for this choice (post: the comment, then the labels and the steps after it; never merge while blocking issues remain; "Chỉnh sửa": apply the boss's edit to the draft, then post it; "Không post": stop, nothing is posted). Your cwd is a fresh temporary checkout of `{{TARGET}}` with the same files as before; never edit, commit or push files in it.

**Finish** by writing `{{RESULT}}` with the same JSON as before (`verdict` `pass` | `fail`, `merged`, `blocking`, `posted`, `message` in Vietnamese).
