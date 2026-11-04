#!/usr/bin/env bash
# report.sh <result.json> [phpstan.json] [lint.txt] — render the markdown report of a pick run (real or preview) to stdout.
# Sections: tickets picked, tickets in conflict (with hand-off instructions), tickets waiting on a hand-off MR,
# and the test list for the release environment.

set -uo pipefail
RESULT="$1"
PHPSTAN="${2:-}"
LINT="${3:-}"

jq -r --slurpfile ps <( [ -n "$PHPSTAN" ] && [ -f "$PHPSTAN" ] && cat "$PHPSTAN" || echo '{"errors":[]}' ) \
    --rawfile lint <( [ -n "$LINT" ] && [ -f "$LINT" ] && cat "$LINT" || printf '' ) '
def mr(i): "!\(i)";
def tickets: .picked | group_by(.ticket);
($ps[0].errors // []) as $errs
| ($lint | split("\n") | map(select(length > 0))) as $lintl
| ([ .picked[] | select(.mismatch > 0) | .ticket ] | unique) as $mm
| (if .preview then "# Xem trước pick release — \(.today)" else "# Release bundle — \(.today)" end),
"",
"Branch: `\(.release_branch)` · classified: `\(.classified)`" + (if .preview then " · **preview: branch không đổi**" else "" end),
"",
"## ✅ Ticket pick được (\(tickets | length) ticket, \(.picked | length) MR)",
"",
(if (.picked | length) == 0 then "_Không có_" else
 "| Ticket | MR | Ghi chú |", "|---|---|---|",
 (tickets[] |
   "| #\(.[0].ticket) | \(map(mr(.iid) + (if .status == "dropped" then " (đã có sẵn)" else "" end)) | join(" ")) | " +
   ([ (if any(.[]; .mismatch > 0) then "⚠ lệch so với vtiger: " + (map(select(.mismatch > 0) | "\(mr(.iid)) \(.mismatch) dòng") | join(", ")) else empty end),
      (if any(.[]; .noise_resolved) then "conflict file không phát hành → giữ bản release" else empty end)
    ] | join("; ")) + " |")
 end),
"",
"## ❌ Ticket conflict — chưa lên release (\(.conflicts_manual | length))",
"",
(if (.conflicts_manual | length) == 0 then "_Không có_" else
 (.conflicts_manual[] |
   "### #\(.ticket) — \(.iids | map(mr(.)) | join(" ")) (@\(.author // "?"))",
   "",
   (if (.handoff_mr // "") != "" then "MR bàn giao: \(.handoff_mr)" else empty end),
   "",
   (.instructions // "Conflict ở MR \(mr(.iid)): \(.files | join(", "))"),
   "",
   "Người release resolve theo hunk: `resolve.sh \(.ticket)` (report `resolve-\(.ticket)/report.md`).",
   "")
 end),
"",
(if (.pending_handoff | length) > 0 then
  "## ⏳ Ticket đang chờ dev resolve (không pick lại)", "",
  (.pending_handoff[] | "- #\(.ticket) \(.iids | map(mr(.)) | join(" ")) — \(.handoff_mr)"), ""
 else empty end),
"## 🧪 Danh sách test trên môi trường release",
"",
"**Bắt buộc test lại đầy đủ** (code trên release khác code đã test trên master):",
"",
(if ($mm | length) == 0 then "- _Không có_" else ($mm[] | "- #\(.) — xem dòng lệch trong `compare-*/<iid>.diff`") end),
"",
"**Sửa trước khi test** (PHPStan: thiếu function/method/class, sai số tham số, lỗi cú pháp so với release):",
"",
(if ($errs | length) == 0 then "- _Không có_" + (if $ps[0].skipped then " (PHPStan chưa chạy: \($ps[0].skipped))" else "" end)
 else ($errs[] | "- `\(.file):\(.line)` \(.message)" + (if .mr then " — do MR !\(.mr)" else "" end)) end),
"",
"**Sửa trước khi test** (lỗi cú pháp trên PHP của server release — typed property, `fn() =>`, `??=`… của master PHP 8.3):",
"",
(if ($lintl | length) == 0 then "- _Không có_"
 elif ($lintl[0] | startswith("SKIPPED:")) then "- _Chưa chạy_: \($lintl[0][9:])"
 else ($lintl[] | "- `\(.)`") end),
"",
"**Smoke test**: các ticket còn lại trong bảng ✅.",
"",
(if (.skipped_by_user | length) > 0 then "_User skip:_ " + (.skipped_by_user | map("#\(.ticket)") | join(" ")) else empty end)
' "$RESULT"
