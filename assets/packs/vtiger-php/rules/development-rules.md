---
description: "Baseline dev rules — YAGNI/KISS/DRY, quality gates, không fake data/mock. Đọc khi code/test/script/config."
---

# Development Rules

Use this file when editing code, tests, scripts, or configuration.

## Baseline

- Follow project docs in `docs/` and existing local patterns.
- Prefer YAGNI, KISS, and DRY in that order.
- Implement real behavior. Do not add fake data, mocks, or temporary shortcuts just to satisfy a check.
- Keep changes scoped to the request and the affected contracts.
- Name new files per the repo convention (`cloudgo-development-rules.md` § Naming Conventions: `<Component>.php`, PascalCase JS/CSS/TPL).
- Split code only when it reduces real complexity or matches existing module boundaries.

## Quality Gates

- Run the narrowest useful test first, then broaden when shared behavior or public contracts changed.
- Do not hide failing tests, lint, type, build, or syntax errors.
- Preserve public contracts unless the change intentionally updates them and the user accepted that scope.
- Keep commits focused and use conventional commit format without AI references.
- Never commit secrets, dotenv files, tokens, private keys, database credentials, or personal data.

## Tooling

- The remote is GitLab (`git.onlinecrm.vn`); use the `gitlab-mr` command / GitLab API for MR operations.
- Use current docs only when the API/tooling may have changed.
- Use relevant skills by reading their descriptions first, then opening only the needed `SKILL.md`.
