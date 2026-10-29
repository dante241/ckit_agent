---
name: ci-cd-and-automation
disable-model-invocation: true
user-invokable: true
description: Automates CI/CD pipeline setup for this vtiger/CloudGO repo (GitLab CI + PHP). Use when setting up or modifying GitLab pipelines, adding quality gates (php -l, PHPStan, PHPUnit) to merge requests, or changing the dev deploy job.
---

# CI/CD and Automation (GitLab CI + PHP)

## Overview

Automate quality gates so that no change reaches production without passing syntax check, static analysis and tests. CI is the enforcement mechanism for every other skill — it catches what humans and agents miss, consistently on every change.

**Shift Left:** Catch problems as early as possible. A syntax error caught by `php -l` costs seconds; the same error after `git pull` on the dev server costs a broken site for everyone.

**Faster is Safer:** Smaller MRs and more frequent releases reduce risk. A deployment with 3 changes is easier to debug than one with 30.

## Current State of This Repo

- Hosting: GitLab (`origin` = `git.onlinecrm.vn/onlinecrm/vtiger`), merge requests (`glab`), default branch `master`. No GitHub Actions, no `package.json`.
- `.gitlab-ci.yml` today is deploy-only: one `deploy_to_dev` job (stage `deploy`, `only: master`, `GIT_STRATEGY: none`) that runs `git pull origin master` in the dev.cloudpro.vn docroot, fixes ownership, then `php cli_tool.php -c=quick_repair`.
- Quality tooling that exists locally (not wired into CI yet):
  - `php -l` (syntax)
  - PHPStan gate: `tools/vendor/bin/phpstan` with `tools/phpstan/phpstan.neon.dist` (legacy code) and `tools/phpstan/phpstan-domains.neon.dist` (`domains/`) + `tools/phpstan/phpstan-baseline.php` — see `rule://phpstan`
  - PHPUnit for `domains/*/tests` via `phpunit.xml.dist` (`tools/vendor/bin/phpunit`, suites `Unit`, `Integration`, `Legacy`; Integration/Legacy need the vtiger runtime)
  - Legacy modules have no PHPUnit suite — they are verified with standalone scripts / HTTP smoke (see `testing` skill)

## The Quality Gate Pipeline

```
Merge Request opened / updated
    │
    ▼
┌──────────────────────┐
│   PHP LINT           │  php -l on changed *.php / *.service
│   ↓ pass             │
│   PHPSTAN            │  tools/vendor/bin/phpstan on changed files
│   ↓ pass             │
│   PHPUNIT (Unit)     │  tools/vendor/bin/phpunit --testsuite Unit
│   ↓ pass             │
│   SECRET SCAN        │  gitleaks (optional)
└──────────────────────┘
    │
    ▼
  Ready for review → merge to master → deploy_to_dev
```

**No gate can be skipped.** If PHPStan fails, fix the code — don't add `@phpstan-ignore` or regenerate the baseline (forbidden by `rule://phpstan`). If a test fails, fix the code — don't skip the test.

## GitLab CI Configuration

### Merge-request quality jobs (add in front of the existing deploy job)

```yaml
stages:
  - check
  - deploy

.mr_only:
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"

php_lint:
  extends: .mr_only
  stage: check
  script:
    - git fetch origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
    - git diff --name-only --diff-filter=ACMR "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME...HEAD" -- '*.php' '*.service' > changed.txt
    - xargs -r -n1 php -l < changed.txt

phpstan:
  extends: .mr_only
  stage: check
  script:
    - composer install -d tools --no-interaction --no-progress
    - git fetch origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
    - git diff --name-only --diff-filter=ACMR "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME...HEAD" -- modules api cron/modules include/EntryPoints include/utils include/Webhooks include/Webservice include/WebserviceTraits | grep -E '\.(php|inc|service)$' > legacy.txt || true
    - git diff --name-only --diff-filter=ACMR "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME...HEAD" -- domains | grep -E '\.php$' > domains.txt || true
    - if [ -s legacy.txt ]; then xargs php -d memory_limit=4G tools/vendor/bin/phpstan analyse --no-progress -c tools/phpstan/phpstan.neon.dist < legacy.txt; fi
    - if [ -s domains.txt ]; then xargs php -d memory_limit=4G tools/vendor/bin/phpstan analyse --no-progress -c tools/phpstan/phpstan-domains.neon.dist < domains.txt; fi

phpunit_unit:
  extends: .mr_only
  stage: check
  script:
    - composer install -d tools --no-interaction --no-progress
    - tools/vendor/bin/phpunit --testsuite Unit
```

Runner prerequisites (verify before enabling):
- A runner with the repo's PHP + `composer`; the existing deploy job runs on a shell runner on the dev server with `GIT_STRATEGY: none` — quality jobs need a normal checkout (default `GIT_STRATEGY`).
- PHPStan needs the untracked, machine-local `include/utils/utils.php8.php` (never committed). Without it the gate reports ~2,400 "function not found" errors — provision it on the runner (e.g. a protected file variable copied in `before_script`) or the job is useless.
- Only ONE PHPStan process per machine (3–4 GB RAM each): set `resource_group: phpstan` or a dedicated runner.
- Signature changes need callers analysed too (PHPStan only reports in files it is given) — `rule://phpstan`.

### Deploy job (existing)

Keep `deploy_to_dev` as is, but make it depend on the checks passing on `master` if you add `check` jobs for branch pipelines too. QuickRepair runs migrations and clears `cache/templates_c/v7/`, so a failed migration surfaces in this job's log.

## Feeding CI Failures Back to Agents

```
CI fails → copy the failing job log → give it to the agent:
"The GitLab pipeline failed with this error: [paste]. Fix it and run the same check locally before pushing."
→ agent fixes → pushes → pipeline re-runs
```

**Key patterns:**

```
php -l failure   → agent fixes the syntax at the reported file:line
PHPStan error    → agent fixes the code (never ignore/baseline), re-runs the gate on the same files
PHPUnit failure  → agent reproduces with tools/vendor/bin/phpunit --filter <test>, then fixes (use the `debugger` agent for hard cases)
Deploy failure   → agent reads the QuickRepair/migration output in the job log
```

## Deployment Strategies

### Feature Flags

Deploy risky features disabled and enable them per tenant via config instead of reverting code:

- **Ship code without enabling it.** Merge to `master` early, enable when ready.
- **Roll back without redeploying.** Disable the flag instead of reverting code.
- **Canary.** Enable for one domain first, then the rest.

In this repo flags are system configs (`Settings_Vtiger_Config_Model::loadConfig()` / `saveConfig()`, see `vtiger-add-settings-config-page`) or `config_override.php` entries.

**Flag lifecycle:** Create → enable on dev → one tenant → all tenants → remove the flag and dead code.

### Staged Rollouts

```
MR merged to master
    │
    ▼
  deploy_to_dev (auto: git pull + QuickRepair on dev.cloudpro.vn)
    │ Manual verification
    ▼
  Release / production patch (manual)
    │
    ▼
  Monitor error logs + latency
    │
    ├── Errors detected → Rollback
    └── Clean → Done
```

### Rollback Plan

Every deployment should be reversible: revert the merge commit on `master` (`git revert -m 1 <merge>`), let `deploy_to_dev` redeploy, and make sure every migration in the MR has a working `down()`.

## Environment Management

```
config.inc.php / config_override.php → environment-specific, contain DB credentials: never commit real values
include/utils/utils.php8.php         → machine-local, ignored (*.php8.php)
CI secrets                            → GitLab CI/CD variables (masked + protected)
Production secrets                    → on the server only
```

CI should never have production secrets. Use separate credentials for any CI database.

## Automation Beyond CI

### Dependency updates

Composer CVE checks: see the `simulate-composer-security-upgrade` skill (checks fixes without modifying vendor/lock). Don't run `composer update` inside `tools/` unless deliberately upgrading PHPStan (the baseline is pinned to its exact version).

### Build Cop Role

Designate someone responsible for keeping `master` and the dev deploy green. When it breaks, the Build Cop fixes or reverts — not whoever caused it — so breakage doesn't pile up.

### MR Checks

- **Required approval:** at least 1 before merge
- **Pipelines must succeed:** enable "Pipelines must succeed" in the project's merge-request settings
- **Protected `master`:** no force-push
- **Merge when pipeline succeeds** for approved MRs

## CI Optimization

When the pipeline gets slow, apply in order of impact:

```
Slow pipeline?
├── Cache dependencies
│   └── cache: { key: { files: [tools/composer.lock] }, paths: [tools/vendor/] }
├── Run jobs in parallel
│   └── php_lint, phpstan, phpunit_unit are already independent jobs in the same stage
├── Only run what changed
│   └── analyse changed files only (as above); rules:changes to skip docs-only MRs
└── Move slow suites off the critical path
    └── run Integration/Legacy suites on a schedule (needs the vtiger runtime + DB)
```

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "CI is too slow" | Optimize the pipeline (see CI Optimization above), don't skip it. |
| "This change is trivial, skip CI" | Trivial changes break the dev site. CI is fast for trivial changes anyway. |
| "The test is flaky, just re-run" | Flaky tests mask real bugs. Fix the flakiness. |
| "PHPStan is noisy, add an ignore" | The baseline already holds old debt; new errors are real. Fix the code. |
| "Manual testing is enough" | Manual testing doesn't scale and isn't repeatable. Automate what you can. |

## Red Flags

- Deploy job with no check stage in front of it
- CI failures ignored or silenced
- `@phpstan-ignore` / regenerated baseline in an ordinary MR
- Tests disabled to make the pipeline pass
- Migrations without a working `down()`
- Secrets in `.gitlab-ci.yml` or committed config files

## Verification

After setting up or modifying CI:

- [ ] `php -l`, PHPStan and PHPUnit Unit jobs run on every merge request
- [ ] Failures block merge ("Pipelines must succeed")
- [ ] Runner has `utils.php8.php` provisioned and only one PHPStan process at a time
- [ ] `deploy_to_dev` still only runs on `master`
- [ ] Secrets live in GitLab CI/CD variables, not in the repo
- [ ] Rollback path documented (revert + migration `down()`)
