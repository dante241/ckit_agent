---
name: commit
disable-model-invocation: true
description: "Stage, review, commit and push code changes. Requires ticket number. Auto-creates feature branches from slug analysis."
user-invokable: true
---

# Commit Skill

> Stage, review, commit and push code with enforced ticket numbers and branch management.

## Usage

```
/commit                     → Interactive commit (will ask for ticket #)
/commit #12345              → Commit with ticket number
/commit #12345 --push       → Commit and push
/commit #12345 --no-review  → Skip code review (not recommended)
```

## Workflow

### Step 1: Validate Ticket Number

**MANDATORY** — Every commit and push REQUIRES a ticket number.

- Extract ticket number from args (format: `#NNNNN` or `NNNNN`)
- If no ticket number provided → **REJECT** with message:
  > "Ticket number is required. Usage: `/commit #12345`"
  > Do NOT proceed without a ticket number.
- Store as `$TICKET` (e.g., `12345`)

### Step 2: Sync Master & Generate Branch

1. Run `git status` to see all changes (staged + unstaged + untracked)
2. Run `git diff` and `git diff --cached` to understand what changed
3. If no changes exist → inform user and stop
4. Determine current branch. If on `master` or `dev`:
   - **Sync before branching** — before creating the feature branch, always update local `master` first:
     - `git checkout master`
     - `git pull origin master`
     - If the working tree had uncommitted changes, they follow you onto `master` via the checkout (git keeps unstaged edits) — do NOT stash/drop them; re-verify `git status` after the pull.
   - Analyze the changes to generate a **feature slug**:
     - Look at modified files, module names, and nature of changes
     - Generate slug: lowercase, kebab-case, max 50 chars
     - Classify change type: `feature/`, `bug/`, `hotfix/`, `refactor/`
   - Propose branch name: `<type>/#<ticket>-<slug>`
   - Examples:
     - `feature/#12345-customer-sales-by-product-report`
     - `bug/#10283-schedule-report-error-save-tracking`
     - `hotfix/#19396-release-uiux-cxday-hotfix`
     - `refactor/#15600-missing-user-privileges`
   - Use `AskUserQuestion` to confirm branch name with user
   - Create and checkout the new branch **from the freshly-pulled master**: `git checkout -b <branch-name>`
5. If already on a feature/bug branch → stay on current branch (do NOT switch to master; syncing mid-feature risks merge conflicts the user didn't ask for)

### Step 3: Code Review (before commit)

Unless `--no-review` flag is passed:

1. Spawn `code-reviewer` agent on all changed files
2. Review checklist (abbreviated):
   - PHP syntax: `php -l` on all changed `.php` files
   - No inline CSS/JS in PHP/TPL
   - Security: prepared statements, type casting, XSS prevention
   - No hardcoded secrets or credentials
   - No debug code (`var_dump`, `print_r`, `console.log` for debugging)
3. If issues found:
   - Present issues to user via `AskUserQuestion`:
     - "Fix issues before commit" (recommended)
     - "Commit anyway"
     - "Cancel commit"
   - If "Fix issues" → fix and re-review
   - If "Commit anyway" → proceed with warning
   - If "Cancel" → stop

### Step 4: Stage & Commit

1. Stage relevant files:
   - Use `git add <specific-files>` — NOT `git add .` or `git add -A`
   - Skip sensitive files: `.env`, credentials, `config.env.php`
   - Confirm staged files with user if there are untracked files
2. Generate commit message following project conventions:

**Commit message format:**
```
[<Category>] #<Ticket>: <Short description>

<Optional body — what changed and why>
```

**Category rules** (derived from commit history):
| Category | When to use |
|----------|-------------|
| `CORE` | Core system changes, releases, hotfixes |
| `CloudWORK` | CloudWork module features/fixes |
| `UI/UX` | UI/UX changes, CSS, layout updates |
| `Reports` | Report-related changes |
| `<ModuleName>` | Module-specific changes (e.g., `Accounts`, `Products`) |

**Examples:**
```
[Reports] #12345: Add Customer Sales by Product report handler

Create new fixed report with date range filter, customer and product
grouping, and revenue aggregation from SalesOrder data.

[CORE] #19396: Fix permission check in record detail view

[CloudWORK] #15600: Add missing user_privileges for inactive users
```

3. Present commit message to user for confirmation
4. Execute commit:
```bash
git commit -m "$(cat <<'EOF'
[Category] #Ticket: Description

Body if needed
EOF
)"
```

### Step 5: Push (if requested)

Only push if `--push` flag is passed OR user explicitly requests it.

1. Verify remote tracking: `git push -u origin <branch-name>`
2. Confirm push succeeded
3. Report branch URL if available

## Rules

| Rule | Detail |
|------|--------|
| **Ticket required** | NEVER commit without ticket number — reject immediately |
| **No force push** | NEVER `git push --force` to master/dev |
| **Review first** | Always review code before commit (unless `--no-review`) |
| **Specific staging** | Stage files by name, never `git add .` |
| **No secrets** | Block commit if `.env` or credential files detected |
| **No AI references** | Never mention AI/Claude in commit messages |
| **Branch from context** | Auto-create branch from change analysis when on master/dev |
| **Sync before branch** | When branching off `master`/`dev`, always `checkout master` + `pull origin master` first — never branch from stale local master |
| **Conventional format** | `[Category] #Ticket: Description` format enforced |
| **User confirmation** | Always confirm branch name + commit message before executing |
