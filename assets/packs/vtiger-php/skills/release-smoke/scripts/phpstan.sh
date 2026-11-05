#!/usr/bin/env bash
# phpstan.sh <ref> <out_dir> — PHPStan on every PHP file origin/release..<ref> changes (release-bundle's
# phpstan-check: new errors after vs before), then split the new errors into
#   same on master (same file + message on origin/master: code that came from master as is) and
#   release-only (read each one), plus calls with too few arguments (ArgumentCountError on PHP 8).
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
REF="${1:?usage: phpstan.sh <ref> <out_dir>}"
OUT="${2:?}"
CHECK="$HERE/../../release-bundle/scripts/phpstan-check.sh"
[ -f "$CHECK" ] || { echo "SKIP: không thấy release-bundle/scripts/phpstan-check.sh"; exit 0; }
cd "$VT" || exit 2
git fetch -q origin master release 2>/dev/null
RES=$(ls -t agents/release-queue/bundle-result-*.json 2>/dev/null | head -1)
RELEASE_PHPSTAN_PROCS="${RELEASE_PHPSTAN_PROCS:-3}" nice -n 10 bash "$CHECK" origin/release "$REF" "$OUT/phpstan.json" ${RES:+"$VT/$RES"} | tail -2
[ -s "$OUT/phpstan.json" ] || exit 0

# Same files on origin/master, analysed the same way (vtiger's config without baselines)
MW=$(mktemp -d /tmp/release-smoke-master.XXXXXX); rmdir "$MW"
git worktree add -q --detach "$MW" origin/master || exit 2
trap 'git -C "$VT" worktree remove --force "$MW" >/dev/null 2>&1; git -C "$VT" worktree prune' EXIT
rm -rf "$MW/tools/phpstan"; mkdir -p "$MW/tools"; cp -R "$VT/tools/phpstan" "$MW/tools/phpstan"
[ -e "$MW/tools/vendor" ] || ln -s "$VT/tools/vendor" "$MW/tools/vendor"
sed '/^includes:/,/^[[:space:]]*$/d' "$VT/tools/phpstan/phpstan.neon.dist" \
    | awk -v tmp="$MW/.phpstan-tmp" '{print} /^parameters:/{print "    tmpDir: " tmp; print "    parallel:"; print "        maximumNumberOfProcesses: 3"}' \
    > "$MW/tools/phpstan/release-check.neon"
for f in "$VT"/include/utils/*.php[78].php "$VT"/include/EntryPoints/*.php[78].php; do
    [ -f "$f" ] && [ ! -e "$MW/${f#"$VT"/}" ] && cp "$f" "$MW/${f#"$VT"/}"
done
FILES=$(jq -r '.errors[].file' "$OUT/phpstan.json" | sort -u | while read -r f; do [ -f "$MW/$f" ] && echo "$f"; done)
[ -n "$FILES" ] && (cd "$MW" && nice -n 10 php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/release-check.neon \
    --error-format=json --no-progress $FILES > "$OUT/phpstan-master.json" 2>/dev/null)

python3 - "$OUT" "$MW" <<'EOF'
import json, re, sys
out, mw = sys.argv[1], sys.argv[2]
new = json.load(open(f'{out}/phpstan.json'))['errors']
try:
    m = json.load(open(f'{out}/phpstan-master.json'))['files']
except Exception:
    m = {}
on_master = {(f.split(mw + '/', 1)[-1], e['message']) for f, v in m.items() for e in v['messages']}
rel_only = [e for e in new if (e['file'], e['message']) not in on_master]
few = []
for e in new:
    g = re.search(r'invoked with (\d+) parameters?, (?:at least )?(\d+)', e['message'])
    if e['identifier'] == 'arguments.count' and g and int(g.group(1)) < int(g.group(2)):
        few.append(e)
# Same on master is no excuse for a call that fatals: keep missing named symbols (master's baseline hides them)
CONCRETE = {'function.notFound', 'staticMethod.notFound', 'classConstant.notFound', 'staticProperty.notFound'}    # class.notFound: Smarty/legacy lib noise
few_ids = {id(e) for e in few}
concrete = [e for e in new if (e['file'], e['message']) in on_master and id(e) not in few_ids
            and e['identifier'] in CONCRETE and not re.search(r'\bobject\b|_Base_Model|Vtiger_(Record|Module)_Model', e['message'])]
json.dump(rel_only, open(f'{out}/phpstan-release-only.json', 'w'), ensure_ascii=False, indent=1)
print(f'phpstan: {len(new)} lỗi mới so với origin/release; y hệt master {len(new) - len(rel_only)}; chỉ release có {len(rel_only)}; '
      f'gọi thiếu tham số {len(few)}; có cả trên master nhưng là symbol cụ thể không tồn tại {len(concrete)}')
for title, rows in (('Chỉ release có', rel_only), ('Gọi thiếu tham số (ArgumentCountError)', few),
                    ('Có cả trên master — symbol cụ thể, cần đọc', concrete)):
    if rows:
        print(f'-- {title}')
    for e in rows:
        print(f"  {e['file']}:{e['line']} [{e['identifier']}] {e['message'][:150]}")
EOF
