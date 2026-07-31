---
name: ckit-cli
description: Use this skill in EVERY session inside a repo whose AGENTS.md mentions ckit. It teaches the AI which ckit verbs (shot/diff-img/pdf-img/find/note/ship/skill/run/harness) to use instead of raw shell equivalents when appropriate, saving tokens and keeping agents/* memory consistent.
---

# ckit-cli — ClaudeKit harness for VTiger/PHP work

**LOAD:** every project whose `AGENTS.md` mentions ckit.

You run under `omp`, wrapped by `ckit`. Prefer ckit verbs when they preserve project memory or reduce large raw output.

## Rules

1. Read `.omp/skills/karpathy-guidelines/SKILL.md` before non-trivial coding.
2. Read `.omp/skills/image-routing/SKILL.md` before image/PDF/diff work.
3. Read `agents/STATE.md` at session start; update it at phase boundaries.
4. Read project-local skills in `agents/skills/<name>/` when task matches frontmatter description.
5. Do not hand-edit generated force-load blocks. ckit regenerates them.

## CLI verbs

| Need | Use | Instead of |
|---|---|---|
| Setup/refresh harness | `ckit harness` | manual skill/MCP/doc sync |
| Start project AI session | `ckit .` | manual `omp --continue` setup |
| One-shot prompt | `ckit ai "..."` | ad-hoc omp call |
| Search file/symbol | `ckit find <kw>` | raw `rg`/`fd` dump |
| Screenshot UI/web route | `ckit shot <url\|file>` | describing UI by text |
| Render large diff | `ckit diff-img [ref]` | dumping long `git diff` |
| Render PDF pages | `ckit pdf-img <file>` | OCR/text dump |
| Save note | `ckit note "..."` | editing agents/*.md manually |
| Run project recipe | `ckit run test|fmt|lint|build` | remembering stack commands |
| Ship PR | `ckit ship "msg"` | manual add/commit/push/PR |

## VTiger/PHP default flow

- Need report → load `report`, then `database`, `testing`.
- Need action/ajax → load `action`, `error-handling`, `testing`.
- Need view/tpl/UI → load `view`, `ui`, `language`, `testing`.
- Need schema/query → load `database`, maybe `migration`.
- Need release/MR → load `release-check`, `gitlab-mr`, `review-pr`.

## Memory shape

```
<repo>/
├── AGENTS.md
└── agents/
    ├── PROJECT.md
    ├── KNOWLEDGE.md
    ├── DECISIONS.md
    ├── PREFERENCES.md
    ├── STATE.md
    ├── PLAYBOOKS.md
    └── skills/<name>/SKILL.md
```

Code refs use `path:line`.
