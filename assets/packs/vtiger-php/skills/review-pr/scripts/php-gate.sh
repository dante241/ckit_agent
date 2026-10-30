#!/usr/bin/env bash
# Mechanical PHP gate for one GitLab MR: lint (PHP 7.4 + 8.3) + PHPStan 2.2.17 on the MR's PHP files,
# analysed on the real merge result (target branch + MR head) in a throwaway worktree.
# Usage (from the vtiger repo root): bash <skill dir>/scripts/php-gate.sh <MR_IID> [TARGET_BRANCH=master]
#   <skill dir> = .omp/skills/review-pr (project pack) or ~/.omp/skills/review-pr (pack installed with --global)
# Output: human summary + last line "GATE: PASS" | "GATE: FAIL" | "GATE: SKIP (no PHP files)". Exit 0 PASS/SKIP, 1 FAIL, 2 setup error.
set -uo pipefail

MR="${1:?usage: php-gate.sh <MR_IID> [TARGET_BRANCH]}"
TARGET="${2:-master}"
REPO_ROOT="$(git rev-parse --show-toplevel)" || exit 2
PHP74="${PHP74:-/opt/homebrew/opt/php@7.4/bin/php}"
PHP83="${PHP83:-/opt/homebrew/opt/php@8.3/bin/php}"
# Real path (macOS /tmp -> /private/tmp): the vtiger load rules resolve the repo root through realpath
WT="$(cd /tmp && pwd -P)/php-gate-mr${MR}"
LOCK="/tmp/phpstan-gate.lock.d"
die() { echo "SETUP ERROR: $*" >&2; echo "GATE: FAIL (setup)"; exit 2; }

for f in "$REPO_ROOT/tools/vendor/bin/phpstan" "$REPO_ROOT/include/utils/utils.php8.php" "$PHP74" "$PHP83"; do
	[ -e "$f" ] || die "missing $f (phpstan rule E2/E3: composer install -d tools; decoded utils.php8.php)"
done

cd "$REPO_ROOT" || exit 2
MR_REF="refs/remotes/mr/${MR}"
git fetch -q origin "$TARGET" "+refs/merge-requests/${MR}/head:${MR_REF}" || die "git fetch failed"

# PHP files the MR touches (added/copied/modified/renamed; deleted files cannot be analysed)
FILES=()
while IFS= read -r f; do FILES+=("$f"); done < <(git diff --name-only --diff-filter=ACMR "origin/${TARGET}...${MR_REF}" -- '*.php' '*.inc' '*.service')
if [ "${#FILES[@]}" -eq 0 ]; then
	echo "MR !${MR}: no PHP files changed"
	echo "GATE: SKIP (no PHP files)"
	exit 0
fi

cleanup() { cd "$REPO_ROOT" && git worktree remove --force "$WT" >/dev/null 2>&1; rm -rf "$WT"; [ "${LOCKED:-0}" = 1 ] && rmdir "$LOCK" 2>/dev/null; }
trap cleanup EXIT
cleanup
git worktree add -q --detach "$WT" "origin/${TARGET}" || die "worktree add failed"
cd "$WT" || exit 2
# Analyse what master will contain after the merge, not the possibly stale MR branch
if ! git -c user.name=gate -c user.email=gate@local merge -q --no-edit --no-ff "$MR_REF" >/dev/null 2>&1; then
	echo "MR !${MR}: merge into origin/${TARGET} has conflicts — rebase required before the gate can run"
	echo "GATE: FAIL"
	exit 1
fi
# Copy, not symlink: the load rules take the repo root from tools/vendor's real path (APFS clone when available)
cp -Rc "$REPO_ROOT/tools/vendor" tools/vendor 2>/dev/null || cp -R "$REPO_ROOT/tools/vendor" tools/vendor || die "cannot copy tools/vendor"
# Decoded ionCube sources (untracked *.php8.php, e.g. include/utils/utils.php8.php, includes/main/WebUI.php8.php)
(cd "$REPO_ROOT" && git ls-files --others --ignored --exclude-standard -- '*.php8.php') | while IFS= read -r f; do
	mkdir -p "$(dirname "$f")" && cp "$REPO_ROOT/$f" "$f"
done

FAIL=0

echo "== Lint (${#FILES[@]} PHP files)"
for bin in "$PHP74" "$PHP83"; do
	for f in "${FILES[@]}"; do
		out=$("$bin" -l "$f" 2>&1) || { echo "LINT FAIL [$("$bin" -r 'echo PHP_VERSION;')] $out"; FAIL=1; }
	done
done
[ "$FAIL" -eq 0 ] && echo "lint OK on $("$PHP74" -r 'echo PHP_VERSION;') and $("$PHP83" -r 'echo PHP_VERSION;')"

# Split files by PHPStan config; everything else is outside the gate (rule phpstan.md) and must be reviewed by hand
APP=(); DOM=(); OUT=()
for f in "${FILES[@]}"; do
	case "$f" in
		include/Webservice/*/tests/*|include/Webservice/CloudWorkApi/v1/*|modules/com_vtiger_workflow/*|modules/Webforms/*|modules/WSAPP/*|*.php[78].php) OUT+=("$f") ;;
		modules/*|api/*|cron/modules/*|include/EntryPoints/*|include/utils/*|include/Webhooks/*|include/Webservice/*|include/WebserviceTraits/*) APP+=("$f") ;;
		domains/*) DOM+=("$f") ;;
		*) OUT+=("$f") ;;
	esac
done

# One PHPStan process at a time on this machine (rule phpstan.md E5); macOS has no flock(1)
until mkdir "$LOCK" 2>/dev/null; do echo "waiting for another PHPStan run ($LOCK)"; sleep 10; done
LOCKED=1

# PHPStan must run on PHP 8.3 (rule phpstan.md E4), whatever `php` is first on PATH
stan() { "$PHP83" -d memory_limit=4G tools/vendor/bin/phpstan analyse --no-progress "$@"; }

if [ "${#APP[@]}" -gt 0 ]; then
	echo "== PHPStan 2.2.17 -c tools/phpstan/phpstan.neon.dist (${#APP[@]} files)"
	stan -c tools/phpstan/phpstan.neon.dist --error-format=table "${APP[@]}" || FAIL=1
fi
if [ "${#DOM[@]}" -gt 0 ]; then
	# domains/ carries 10 known errors in domains/Social (rule phpstan.md): the MR must not add more
	count=$(stan -c tools/phpstan/phpstan-domains.neon.dist --error-format=json 2>/dev/null | "$PHP83" -r '$j=json_decode(stream_get_contents(STDIN),true); echo (int)($j["totals"]["file_errors"] ?? -1) + (int)($j["totals"]["errors"] ?? 0);')
	echo "== PHPStan 2.2.17 -c tools/phpstan/phpstan-domains.neon.dist: $count errors (allowed: 10 known)"
	if [ "$count" -lt 0 ] || [ "$count" -gt 10 ]; then
		stan -c tools/phpstan/phpstan-domains.neon.dist --error-format=table
		FAIL=1
	fi
fi
[ "${#OUT[@]}" -gt 0 ] && printf 'Outside PHPStan gate (review by hand + run it):\n  %s\n' "${OUT[@]}"

if [ "$FAIL" -eq 0 ]; then echo "GATE: PASS"; exit 0; fi
echo "GATE: FAIL"
exit 1
