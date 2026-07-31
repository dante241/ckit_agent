#!/usr/bin/env python3
"""classify.py — Merge MRs + ticket statuses into classified JSON + markdown report.

Usage:
    classify.py <mrs-slim.json> <status.tsv> <output-dir>

Inputs:
    mrs-slim.json: [{iid, title, author, merged_at, web_url, labels}, ...]
    status.tsv: ticket_no<TAB>status<TAB>record_id<TAB>label<TAB>categories<TAB>assignee

Env:
    PMS_BASE_URL — used to build clickable ticket links in the markdown report.

Outputs:
    <output-dir>/classified-<DATE>.json
    <output-dir>/report-<DATE>.md
"""

import csv
import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path

TICKET_RE = re.compile(r"#(\d{3,})")
READY_STATUSES = {"Closed", "Wait Close"}
PMS_BASE_URL = os.environ.get("PMS_BASE_URL", "").rstrip("/")


def extract_ticket(title: str) -> str | None:
    m = TICKET_RE.search(title)
    return m.group(1) if m else None


def load_statuses(path: Path) -> dict[str, dict]:
    out: dict[str, dict] = {}
    with path.open() as f:
        for row in csv.reader(f, delimiter="\t"):
            if len(row) < 2:
                continue
            ticket_no, status = row[0], row[1]
            record_id = row[2] if len(row) > 2 else ""
            label = row[3] if len(row) > 3 else ""
            categories = row[4] if len(row) > 4 else ""
            assignee = row[5] if len(row) > 5 else ""
            out[ticket_no] = {
                "status": status,
                "record_id": record_id,
                "label": label,
                "categories": categories,
                "assignee": assignee,
            }
    return out


def pms_link(record_id: str, ticket_id: str) -> str:
    """Return markdown link to PMS ticket detail, or plain '#id' if no URL/record."""
    text = f"#{ticket_id}" if ticket_id else "#?"
    if PMS_BASE_URL and record_id:
        return f"[{text}]({PMS_BASE_URL}/index.php?module=HelpDesk&view=Detail&record={record_id})"
    return text


def classify(mrs: list[dict], statuses: dict[str, dict]) -> dict:
    ready, testing, regressed, no_ticket, not_found = [], [], [], [], []
    for mr in mrs:
        tid = extract_ticket(mr["title"])
        labels = mr.get("labels") or []
        has_cho_release = "Chờ release" in labels
        entry = {
            "iid": mr["iid"],
            "ticket_id": tid,
            "title": mr["title"],
            "author": mr["author"],
            "merged_at": mr["merged_at"],
            "web_url": mr["web_url"],
            "labels": labels,
            "merge_commit_sha": mr.get("merge_commit_sha") or "",
            "squash_commit_sha": mr.get("squash_commit_sha") or "",
            "sha": mr.get("sha") or "",
        }
        if tid is None:
            entry["ticket_status"] = "NO_TICKET"
            no_ticket.append(entry)
            continue
        info = statuses.get(tid)
        if info is None or info["status"] in {"NOT_FOUND", "ERROR"}:
            entry["ticket_status"] = info["status"] if info else "UNQUERIED"
            not_found.append(entry)
            continue
        entry["ticket_status"] = info["status"]
        entry["ticket_record_id"] = info["record_id"]
        entry["ticket_label"] = info["label"]
        entry["ticket_categories"] = info.get("categories", "")
        entry["ticket_assignee"] = info.get("assignee", "")
        is_ready_status = info["status"] in READY_STATUSES
        if is_ready_status:
            ready.append(entry)
        elif has_cho_release:
            # Previously promoted to Chờ release but ticket no longer ready — rollback signal
            regressed.append(entry)
        else:
            testing.append(entry)
    return {
        "ready": ready,
        "regressed": regressed,
        "still_testing": testing,
        "no_ticket": no_ticket,
        "ticket_not_found": not_found,
    }


def render_markdown(data: dict, scan_date: str) -> str:
    now = datetime.now().strftime("%H:%M")
    s = data["summary"]
    lines = [
        f"# Release Check — {scan_date} {now}",
        "",
        f"**Repo:** {data['repo']} | **Label filter:** `{data['label_filter']}` | **Total:** {data['total_scanned']}",
        "",
        "| Category | Count |",
        "|----------|-------|",
        f"| ✅ Ready to release | {s['ready']} |",
        f"| 🔁 Regressed (rollback) | {s.get('regressed', 0)} |",
        f"| ⏳ Still testing | {s['still_testing']} |",
        f"| ⚠️ No ticket | {s['no_ticket']} |",
        f"| ❌ Ticket not found | {s['ticket_not_found']} |",
        "",
    ]

    def fmt_date(v):
        return (v or "")[:10]

    def esc(t):
        return t.replace("|", "\\|")

    def tkt(m):
        return pms_link(m.get("ticket_record_id", ""), m.get("ticket_id") or "")

    def cat(m):
        return m.get("ticket_categories") or "-"

    lines += ["## ✅ Ready to release", ""]
    if data["ready"]:
        lines += [
            "| MR | Ticket | Category | Status | Title | Author | Merged at | Labeled |",
            "|----|--------|----------|--------|-------|--------|-----------|---------|",
        ]
        for m in data["ready"]:
            had = "Chờ release" in m["labels"]
            action = "already" if had else "newly transitioned"
            lines.append(
                f"| [!{m['iid']}]({m['web_url']}) | {tkt(m)} | {cat(m)} | {m['ticket_status']} | {esc(m['title'])} | {m['author']} | {fmt_date(m['merged_at'])} | {action} |"
            )
    else:
        lines.append("_None_")
    lines.append("")

    lines += ["## 🔁 Regressed (rolled back)", ""]
    if data.get("regressed"):
        lines += [
            "| MR | Ticket | Category | Status | Title | Author | Merged at |",
            "|----|--------|----------|--------|-------|--------|-----------|",
        ]
        for m in data["regressed"]:
            lines.append(
                f"| [!{m['iid']}]({m['web_url']}) | {tkt(m)} | {cat(m)} | {m['ticket_status']} | {esc(m['title'])} | {m['author']} | {fmt_date(m['merged_at'])} |"
            )
    else:
        lines.append("_None_")
    lines.append("")

    lines += ["## ⏳ Still testing", ""]
    if data["still_testing"]:
        lines += [
            "| MR | Ticket | Category | Status | Title | Author | Merged at |",
            "|----|--------|----------|--------|-------|--------|-----------|",
        ]
        for m in data["still_testing"]:
            lines.append(
                f"| [!{m['iid']}]({m['web_url']}) | {tkt(m)} | {cat(m)} | {m['ticket_status']} | {esc(m['title'])} | {m['author']} | {fmt_date(m['merged_at'])} |"
            )
    else:
        lines.append("_None_")
    lines.append("")

    problems = data["no_ticket"] + data["ticket_not_found"]
    lines += ["## ⚠️ No ticket / ❌ Error", ""]
    if problems:
        lines += [
            "| MR | Title | Author | Merged at | Issue |",
            "|----|-------|--------|-----------|-------|",
        ]
        for m in data["no_ticket"]:
            lines.append(f"| [!{m['iid']}]({m['web_url']}) | {esc(m['title'])} | {m['author']} | {fmt_date(m['merged_at'])} | No ticket in title |")
        for m in data["ticket_not_found"]:
            lines.append(
                f"| [!{m['iid']}]({m['web_url']}) | {esc(m['title'])} | {m['author']} | {fmt_date(m['merged_at'])} | Ticket #{m['ticket_id']} {m['ticket_status']} |"
            )
    else:
        lines.append("_None_")
    lines.append("")

    lines += build_author_matrix(data)
    return "\n".join(lines) + "\n"


def build_author_matrix(data: dict) -> list[str]:
    """Matrix pivot: ticket category (rows) x MR author (columns), pending MRs only.

    Pending = everything except ``ready``. Rows = ticket categories (Bug/Improve/…),
    columns = GitLab MR authors, cells = count.
    """
    pending_buckets = ("regressed", "still_testing", "no_ticket", "ticket_not_found")
    all_mrs: list[dict] = []
    for b in pending_buckets:
        all_mrs.extend(data.get(b, []) or [])

    title = "## 👥 Matrix — MR còn lại cần xử lý (category × author)"
    if not all_mrs:
        return [title, "", "_None_", ""]

    # Count authors & categories
    authors: dict[str, int] = {}
    categories: dict[str, dict[str, int]] = {}
    for m in all_mrs:
        author = m.get("author") or "(unknown)"
        category = m.get("ticket_categories") or "(no category)"
        authors[author] = authors.get(author, 0) + 1
        cells = categories.setdefault(category, {})
        cells[author] = cells.get(author, 0) + 1

    # Sort: authors by total desc, categories by total desc
    author_order = sorted(authors.keys(), key=lambda a: (-authors[a], a.lower()))
    cat_totals = {c: sum(cells.values()) for c, cells in categories.items()}
    category_order = sorted(categories.keys(), key=lambda c: (-cat_totals[c], c.lower()))

    header_cells = ["Category"] + author_order + ["**Total**"]
    out = [
        title,
        "",
        "| " + " | ".join(header_cells) + " |",
        "|" + "|".join(["---"] * len(header_cells)) + "|",
    ]
    for cat in category_order:
        row = [cat]
        for a in author_order:
            n = categories[cat].get(a, 0)
            row.append(str(n) if n else "·")
        row.append(f"**{cat_totals[cat]}**")
        out.append("| " + " | ".join(row) + " |")
    grand = sum(authors.values())
    totals_row = ["**Total**"] + [f"**{authors[a]}**" for a in author_order] + [f"**{grand}**"]
    out.append("| " + " | ".join(totals_row) + " |")
    out.append("")
    return out


def main(argv):
    if len(argv) != 4:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    mrs_path = Path(argv[1])
    status_path = Path(argv[2])
    out_dir = Path(argv[3])
    out_dir.mkdir(parents=True, exist_ok=True)

    scan_date = datetime.now().strftime("%Y-%m-%d")
    mrs = json.loads(mrs_path.read_text())
    statuses = load_statuses(status_path)
    buckets = classify(mrs, statuses)

    result = {
        "scan_date": scan_date,
        "repo": "acme/vtiger",
        "label_filter": "Dev done | Chờ release",
        "total_scanned": len(mrs),
        "summary": {k: len(v) for k, v in buckets.items()},
        **buckets,
    }

    json_path = out_dir / f"classified-{scan_date}.json"
    md_path = out_dir / f"report-{scan_date}.md"
    json_path.write_text(json.dumps(result, ensure_ascii=False, indent=2))
    md_path.write_text(render_markdown(result, scan_date))

    # Terminal summary
    print(f"Scanned: {result['total_scanned']} MRs")
    for k, n in result["summary"].items():
        print(f"  {k}: {n}")
    ready_iids = " ".join(f"!{m['iid']}" for m in result["ready"])
    if ready_iids:
        print(f"\nReady MRs: {ready_iids}")
    regressed_iids = " ".join(f"!{m['iid']}" for m in result.get("regressed") or [])
    if regressed_iids:
        print(f"Regressed MRs: {regressed_iids} (will rollback label)")
    print(f"\nReport: {md_path}")
    print(f"JSON:   {json_path}")


if __name__ == "__main__":
    main(sys.argv)
