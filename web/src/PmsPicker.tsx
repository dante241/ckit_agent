// "Lấy từ PMS" in Tạo task (open by default): search the boss's CloudGO PMS tasks / HelpDesk
// tickets (or a PMS report's rows), preview one, and fill the new Board task from it.
// Read-only against PMS. Contract: /api/board/pms/{status,search,item,report}.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useInfiniteQuery, useQuery, type UseQueryResult } from "@tanstack/react-query";
import { api, type BoardSource, type PmsDetail, type PmsFilters, type PmsItem, type PmsKind, type PmsMeta, type PmsOpt, type PmsReport, type PmsReportRow, type PmsStatusFilter } from "./api";
import { PmsConnectLink } from "./PmsConnect";
import { serverMessage } from "./Steps";
import { MultiSelect, Select } from "./Select";

/** `null` while unknown (loading, or an older dashboard without the endpoint). */
export function usePmsConfigured(): boolean | null {
  const q = useQuery({ queryKey: ["pms-status"], queryFn: api.pmsStatus, staleTime: 60_000, retry: false });
  return q.data ? q.data.configured : null;
}

// ── Vocabulary ─────────────────────────────────────────────────────────────
const STATUS_VI: Record<string, string> = {
  Open: "Mới",
  Assigned: "Đã giao",
  "In Progress": "Đang làm",
  Testing: "Đang test",
  "Wait For Verifying": "Chờ xác minh",
  "Wait Close": "Chờ đóng",
  "Wait For Response": "Chờ phản hồi",
  "Follow Up": "Theo dõi",
  Pending: "Tạm hoãn",
  "Pending Golive": "Chờ golive",
  Reopen: "Mở lại",
  Completed: "Hoàn thành",
  Closed: "Đã đóng",
  Canceled: "Đã huỷ",
  Cancel: "Đã huỷ",
  Reject: "Từ chối",
};

export function pmsStatusLabel(s: string): string {
  return s ? (STATUS_VI[s] ?? s) : "Chưa có trạng thái";
}

const DONE = new Set(["Completed", "Closed", "Canceled", "Cancel", "Reject"]);

/** Tone class shared by the status chip and the row's left bar. */
function statusTone(s: string): "new" | "run" | "ok" | "idle" | "wait" {
  if (s === "Completed" || s === "Closed") return "ok";
  if (s === "Canceled" || s === "Cancel" || s === "Reject" || s === "") return "idle";
  if (s === "In Progress" || s === "Testing") return "run";
  if (s === "Open" || s === "Assigned" || s === "Reopen") return "new";
  return "wait";
}

const PRIORITY_VI: Record<string, string> = { urgent: "Khẩn", high: "Ưu tiên cao", normal: "Thường", medium: "Trung bình", low: "Thấp" };

function priority(p: string): { label: string; tone: string } | null {
  const k = p.toLowerCase();
  if (!k) return null;
  return { label: PRIORITY_VI[k] ?? p, tone: k === "urgent" || k === "high" ? "hot" : k === "low" ? "low" : "" };
}

const KIND_LABEL: Record<PmsKind, string> = { task: "Task", ticket: "Ticket" };
/** Picker tabs: the two searchable kinds plus a PMS report. */
type Tab = PmsKind | "report";
const TAB_LABEL: Record<Tab, string> = { ticket: "Ticket", task: "Task", report: "Report" };

/** `2026-10-12 13:42:33` → `12/10/2026` (`withTime` → `12/10 13:42`). */
function viDate(s: string, withTime = false): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}))?/.exec(s);
  if (!m) return s;
  return withTime && m[4] ? `${m[3]}/${m[2]} ${m[4]}:${m[5]}` : `${m[3]}/${m[2]}/${m[1]}`;
}

function overdue(it: PmsItem): boolean {
  return !!it.due && !DONE.has(it.status) && it.due.slice(0, 10) < new Date().toISOString().slice(0, 10);
}

const COMMENTS_MAX = 5;
const COMMENTS_CHARS = 1500;

/** Imported text may not close (or fake-open) the data tag it is wrapped in. */
const untag = (s: string) => s.replace(/<\/?\s*pms_source/gi, "‹pms_source");

/** Last comments (newest first, as PMS returns them), at most ~COMMENTS_CHARS in total. */
function commentLines(d: PmsDetail): string[] {
  const out: string[] = [];
  let used = 0;
  for (const c of d.comments.slice(0, COMMENTS_MAX)) {
    const left = COMMENTS_CHARS - used;
    if (left < 40) break;
    let line = `- ${c.author || "Ẩn danh"} (${viDate(c.at, true)}): ${c.text.replace(/\n+/g, " ")}`;
    if (line.length > left) line = `${line.slice(0, left - 1)}…`;
    out.push(line);
    used += line.length;
  }
  return out;
}

/** What the picker hands to Tạo task. */
export type PmsPick = { title: string; request: string; source: BoardSource };

/** Title and request text for the Board task, built from the PMS item. Everything
 * imported from PMS (written by customers/staff) sits inside `<pms_source>`, which
 * the agents treat as data, never as instructions (assets/board/common.md). */
export function pmsToTask(d: PmsDetail): PmsPick {
  // An empty PMS description still leaves the Leader a sentence to work from.
  const parts: string[] = [d.description || d.title];
  if (d.checklist.length) parts.push(["Checklist:", ...d.checklist.map((c) => `- [${c.done ? "x" : " "}] ${c.name}`)].join("\n"));
  if (d.subtasks.length) parts.push(["Việc con:", ...d.subtasks.map((s) => `- ${s.title} (${pmsStatusLabel(s.status)})`)].join("\n"));
  const comments = commentLines(d);
  if (comments.length) parts.push(["Bình luận gần đây:", ...comments].join("\n"));
  const label = `${KIND_LABEL[d.kind]}${d.no ? ` ${d.no}` : ""}`;
  const attr = (s: string) => s.replace(/[^\w#.-]/g, "");
  const body = untag(parts.join("\n\n"));
  return {
    title: d.no ? `[${d.no}] ${d.title}` : d.title,
    request: `<pms_source system="pms" kind="${d.kind}" no="${attr(d.no)}">\n${body}\n</pms_source>\n\nNguồn: PMS ${label}, ${d.url}`,
    source: { system: "pms", kind: d.kind, id: d.id, no: d.no, url: d.url },
  };
}

/** A report row of a module other than task/ticket: its columns become the request. */
export function reportRowToTask(r: PmsReport, row: PmsReportRow): PmsPick {
  const attr = (s: string) => s.replace(/[^\w#.-]/g, "");
  const lines = row.cells.map((c) => (c.value.includes("\n") ? `${c.label}:\n${c.value}` : `${c.label}: ${c.value}`));
  const body = untag([`Report: ${r.name || r.id}`, ...lines].join("\n"));
  const url = row.url || r.url;
  return {
    title: row.title,
    request: `<pms_source system="pms" kind="record" module="${attr(r.module)}" report="${attr(r.id)}">\n${body}\n</pms_source>\n\nNguồn: PMS report ${r.id}${r.name ? ` (${r.name})` : ""}, ${url}`,
    source: { system: "pms", kind: "record", id: row.id, no: "", url },
  };
}

/** Link chip to the PMS record a task came from (cards, task page). */
export function PmsSourceChip({ source }: { source?: BoardSource | null }) {
  if (!source || source.system !== "pms" || !source.url) return null;
  return (
    <a className="tag bd-pms-chip" href={source.url} target="_blank" rel="noopener noreferrer" title="Mở trên PMS (tab mới)">
      PMS {source.no || `#${source.id}`}
    </a>
  );
}

function useDebounced<T>(v: T, ms: number): T {
  const [d, setD] = useState(v);
  useEffect(() => {
    const h = setTimeout(() => setD(v), ms);
    return () => clearTimeout(h);
  }, [v, ms]);
  return d;
}

function StatusChip({ s }: { s: string }) {
  return s ? <span className={`pms-st is-${statusTone(s)}`}>{pmsStatusLabel(s)}</span> : null;
}

function PriorityChip({ p }: { p: string }) {
  const x = priority(p);
  return x && x.tone !== "" ? <span className={`pms-pri is-${x.tone}`}>{x.label}</span> : null;
}

// ── Filters (remembered per kind in localStorage) ──────────────────────────
const STORE = "ckit.pms.filters";
const NO_FILTERS: PmsFilters = { statuses: [], project: "", view: "", priority: "", category: "", owner: "" };
type Saved = { kind: Tab; mine: boolean; status: PmsStatusFilter; by: Record<PmsKind, PmsFilters> };

function loadSaved(): Saved {
  const base: Saved = { kind: "ticket", mine: true, status: "open", by: { ticket: NO_FILTERS, task: NO_FILTERS } };
  try {
    const s = JSON.parse(localStorage.getItem(STORE) ?? "null") as Partial<Saved> | null;
    if (!s) return base;
    const one = (f?: Partial<PmsFilters>): PmsFilters => ({ ...NO_FILTERS, ...f, statuses: Array.isArray(f?.statuses) ? f.statuses.filter((x) => typeof x === "string") : [] });
    return {
      kind: s.kind === "task" || s.kind === "report" ? s.kind : "ticket",
      mine: s.mine !== false,
      status: s.status === "done" || s.status === "all" ? s.status : "open",
      by: { ticket: one(s.by?.ticket), task: one(s.by?.task) },
    };
  } catch {
    return base;
  }
}

/** Number of extra filters set (the statuses list counts once). */
const filterCount = (f: PmsFilters) => [f.statuses.length > 0, f.project, f.view, f.priority, f.category, f.owner].filter(Boolean).length;

const CATEGORY_LABEL: Record<PmsKind, string> = { ticket: "Phân loại", task: "Loại công việc" };
/** Patch the current kind's filters; the function form reads the latest state (fast multi-clicks). */
type SetFilters = (p: Partial<PmsFilters> | ((f: PmsFilters) => Partial<PmsFilters>)) => void;

// ── Picker ─────────────────────────────────────────────────────────────────
export function PmsPicker({ onPick, onClose }: { onPick: (p: PmsPick) => void; onClose: () => void }) {
  const [saved, setSaved] = useState<Saved>(loadSaved);
  const { kind: tab, mine, status } = saved;
  // Search state below belongs to Ticket/Task; the Report tab keeps the last of them idle.
  const kind: PmsKind = tab === "report" ? "ticket" : tab;
  const filters = saved.by[kind];
  const [text, setText] = useState("");
  const q = useDebounced(text.trim(), 350);
  const [sel, setSel] = useState<PmsItem | null>(null);
  const [popOpen, setPopOpen] = useState(false);
  useEffect(() => {
    try {
      localStorage.setItem(STORE, JSON.stringify(saved));
    } catch {
      /* private mode: filters just won't be remembered */
    }
  }, [saved]);
  const patch = (p: Partial<Saved>) => setSaved((s) => ({ ...s, ...p }));
  const setFilters: SetFilters = (p) => {
    setSaved((s) => {
      if (s.kind === "report") return s;
      const cur = s.by[s.kind];
      const d = typeof p === "function" ? p(cur) : p;
      // Picking a person replaces "Của tôi".
      return { ...s, mine: d.owner ? false : s.mine, by: { ...s.by, [s.kind]: { ...cur, ...d } } };
    });
    setSel(null);
  };
  const meta = useQuery({ queryKey: ["pms-meta", kind], queryFn: () => api.pmsMeta(kind), staleTime: 600_000, retry: false, enabled: tab !== "report" });

  const search = useInfiniteQuery({
    queryKey: ["pms-search", kind, q, mine, status, filters],
    queryFn: ({ pageParam }) => api.pmsSearch({ kind, q, mine, status, page: pageParam, ...filters }),
    initialPageParam: 0,
    getNextPageParam: (last, all) => (last.has_more ? all.length : undefined),
    staleTime: 30_000,
    retry: false,
    enabled: tab !== "report",
  });
  const items = search.data?.pages.flatMap((p) => p.items) ?? [];
  const switchKind = (k: Tab) => {
    patch({ kind: k });
    setSel(null);
  };
  const n = filterCount(filters);
  const custom = filters.statuses.length > 0;
  const tabs = (
    <div className="seg" role="tablist" aria-label="Loại">
      {(["ticket", "task", "report"] as const).map((k) => (
        <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? "seg-on" : ""} onClick={() => switchKind(k)}>
          {TAB_LABEL[k]}
        </button>
      ))}
    </div>
  );
  if (tab === "report") return <ReportPicker tabs={tabs} onPick={onPick} onClose={onClose} />;

  return (
    <section className={`pms-panel${sel ? " has-sel" : ""}`} aria-label="Lấy việc từ PMS">
      <header className="pms-bar">
        {tabs}
        <input
          type="search"
          className="pms-search"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={kind === "task" ? "Tìm tiêu đề hoặc số PT12345" : "Tìm tiêu đề hoặc số #67890"}
          aria-label="Tìm trên PMS"
          autoFocus
        />
        <label className="check-inline">
          <input type="checkbox" checked={mine} onChange={(e) => { patch({ mine: e.target.checked }); if (e.target.checked && filters.owner) setFilters({ owner: "" }); }} /> Của tôi
        </label>
        <Select
          className="pms-status"
          aria-label="Tình trạng"
          active={custom || status !== "open"}
          value={custom ? "custom" : status}
          onChange={(e) => {
            if (e.target.value === "custom") return;
            patch({ status: e.target.value as PmsStatusFilter });
            if (custom) setFilters({ statuses: [] });
          }}
        >
          <option value="open">Đang mở</option>
          <option value="done">Đã xong</option>
          <option value="all">Mọi tình trạng</option>
          {custom ? <option value="custom">{filters.statuses.length} tình trạng</option> : null}
        </Select>
        <button
          type="button"
          className={`ghost pms-filter-btn${n ? " is-on" : ""}`}
          aria-expanded={popOpen}
          aria-haspopup="dialog"
          onClick={() => setPopOpen((o) => !o)}
        >
          <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4h11M4.5 8h7M6.5 12h3" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
          Bộ lọc
          {n ? <span className="pms-filter-n">{n}</span> : null}
        </button>
        <button type="button" className="ghost pms-close" onClick={onClose} title="Không lấy từ PMS, tự nhập tiêu đề và yêu cầu">Tự nhập</button>
        {popOpen ? <FilterPop kind={kind} f={filters} meta={meta} onChange={setFilters} onClose={() => setPopOpen(false)} /> : null}
      </header>
      {n ? <FilterChips kind={kind} f={filters} meta={meta.data} onChange={setFilters} /> : null}
      <div className="pms-body">
        <div className="pms-listcol">
          <div className="pms-list" role="listbox" aria-label="Kết quả" aria-busy={search.isFetching}>
            {search.isPending ? (
              [0, 1, 2, 3].map((i) => (
                <div key={i} className="pms-row pms-skel" aria-hidden="true">
                  <span className="skel" style={{ width: `${78 - i * 10}%` }} />
                  <span className="skel" style={{ width: "46%" }} />
                </div>
              ))
            ) : search.isError ? (
              <p className="hint hint-err pms-msg">{serverMessage(search.error as Error)}</p>
            ) : items.length === 0 ? (
              <p className="pms-msg">
                Không tìm thấy {kind} nào. {n ? "Thử bỏ bớt bộ lọc nhé." : mine || status !== "all" ? "Thử bỏ “Của tôi” hoặc chọn “Mọi tình trạng”." : "Thử từ khoá khác nhé."}
              </p>
            ) : (
              items.map((it) => (
                <Row key={`${it.kind}-${it.id}`} it={it} on={sel?.id === it.id && sel.kind === it.kind} onSelect={() => setSel(it)} />
              ))
            )}
            {search.hasNextPage ? (
              <button type="button" className="ghost pms-more" onClick={() => search.fetchNextPage()} disabled={search.isFetchingNextPage}>
                {search.isFetchingNextPage ? "Đang tải…" : "Tải thêm"}
              </button>
            ) : null}
          </div>
        </div>
        <div className="pms-previewcol">
          {sel ? <Preview it={sel} onBack={() => setSel(null)} onPick={(d) => onPick(pmsToTask(d))} /> : (
            <div className="pms-empty">
              <strong>Chọn một {kind} bên trái để xem chi tiết</strong>
              <span>Mô tả, checklist và bình luận sẽ được chép vào ô Yêu cầu. Bạn vẫn sửa lại được trước khi tạo task.</span>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function optLabel(list: PmsOpt[] | undefined, v: string): string {
  return list?.find((o) => o.value === v)?.label ?? v;
}

/** Active extra filters as removable chips under the toolbar. */
function FilterChips({ kind, f, meta, onChange }: { kind: PmsKind; f: PmsFilters; meta?: PmsMeta; onChange: SetFilters }) {
  const chips: { k: string; label: string; value: string; clear: Partial<PmsFilters> }[] = [];
  if (f.view) chips.push({ k: "view", label: "Danh sách", value: optLabel(meta?.views, f.view), clear: { view: "" } });
  if (f.project) chips.push({ k: "project", label: "Dự án", value: meta?.projects.find((p) => p.value === f.project)?.label ?? f.projectLabel ?? f.project, clear: { project: "", projectLabel: "" } });
  if (f.statuses.length) chips.push({ k: "st", label: "Tình trạng", value: f.statuses.map((s) => optLabel(meta?.statuses, s)).join(", "), clear: { statuses: [] } });
  if (f.priority) chips.push({ k: "pri", label: "Ưu tiên", value: optLabel(meta?.priorities, f.priority), clear: { priority: "" } });
  if (f.category) chips.push({ k: "cat", label: CATEGORY_LABEL[kind], value: optLabel(meta?.categories, f.category), clear: { category: "" } });
  if (f.owner) chips.push({ k: "owner", label: "Phụ trách", value: meta?.users.find((u) => u.value === f.owner)?.label ?? f.ownerLabel ?? f.owner, clear: { owner: "", ownerLabel: "" } });
  return (
    <div className="pms-chips" aria-label="Bộ lọc đang dùng">
      {chips.map((c) => (
        <span key={c.k} className="pms-chip" title={`${c.label}: ${c.value}`}>
          <span><b>{c.label}:</b> {c.value}</span>
          <button type="button" onClick={() => onChange(c.clear)} aria-label={`Bỏ lọc ${c.label}`}>✕</button>
        </span>
      ))}
      {chips.length > 1 ? <button type="button" className="link pms-chips-clear" onClick={() => onChange(NO_FILTERS)}>Xoá tất cả</button> : null}
    </div>
  );
}

/** The "Bộ lọc" popover: saved lists, project, exact statuses, priority, category, owner. Applies as you pick. */
function FilterPop({ kind, f, meta, onChange, onClose }: {
  kind: PmsKind; f: PmsFilters; meta: UseQueryResult<PmsMeta>; onChange: SetFilters; onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  // Focus lands inside once the options are there; a click outside closes.
  useEffect(() => {
    if (!meta.isPending) ref.current?.querySelector<HTMLElement>("select, button")?.focus();
  }, [meta.isPending]);
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      // Dropdown lists are portalled to <body>: a click there is still "inside".
      if (!t.closest(".pms-pop, .pms-filter-btn, .cbx-pop, .cbx-scrim")) closeRef.current();
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, []);
  const m = meta.data;
  const groups = [...new Set((m?.views ?? []).map((v) => v.group ?? ""))];
  return (
    <div
      ref={ref}
      className="pms-pop"
      role="dialog"
      aria-label="Bộ lọc PMS"
      onKeyDown={(e) => {
        // Escape closes the popover only, not the Tạo task modal.
        if (e.key === "Escape") {
          e.nativeEvent.stopPropagation();
          onClose();
        }
      }}
    >
      {meta.isPending ? (
        <div className="pms-pop-grid" aria-hidden="true">
          {[0, 1, 2, 3].map((i) => <span key={i} className="skel" style={{ height: 36 }} />)}
        </div>
      ) : meta.isError ? (
        <p className="hint hint-err">{serverMessage(meta.error as Error)}</p>
      ) : (
        <>
          <div className="pms-pop-grid">
            <label className="field pms-pop-wide">
              <span className="field-label">Danh sách theo bộ lọc của PMS</span>
              <Select aria-label="Danh sách theo bộ lọc" value={f.view} active={f.view !== ""} onChange={(e) => onChange({ view: e.target.value })}>
                <option value="">Không dùng</option>
                {groups.map((g) => (
                  <optgroup key={g} label={g || "Danh sách"}>
                    {(m?.views ?? []).filter((v) => (v.group ?? "") === g).map((v) => <option key={v.value} value={v.value}>{v.label}</option>)}
                  </optgroup>
                ))}
              </Select>
            </label>
            <label className="field pms-pop-wide">
              <span className="field-label">Dự án</span>
              <Select
                aria-label="Dự án"
                value={f.project}
                active={f.project !== ""}
                remote={api.pmsProjects}
                valueLabel={f.projectLabel}
                onChange={(e) => onChange({ project: e.target.value, projectLabel: "" })}
                onOption={(o) => onChange({ project: o.value, projectLabel: o.label })}
              >
                <option value="">Mọi dự án</option>
                {(m?.projects ?? []).map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
              </Select>
            </label>
            <label className="field">
              <span className="field-label">Mức ưu tiên</span>
              <Select aria-label="Mức ưu tiên" value={f.priority} active={f.priority !== ""} onChange={(e) => onChange({ priority: e.target.value })}>
                <option value="">Mọi mức</option>
                {(m?.priorities ?? []).map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
              </Select>
            </label>
            <label className="field">
              <span className="field-label">{CATEGORY_LABEL[kind]}</span>
              <Select aria-label={CATEGORY_LABEL[kind]} value={f.category} active={f.category !== ""} onChange={(e) => onChange({ category: e.target.value })}>
                <option value="">Tất cả</option>
                {(m?.categories ?? []).map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
              </Select>
            </label>
            <label className="field pms-pop-wide">
              <span className="field-label">Người phụ trách</span>
              <Select
                aria-label="Người phụ trách"
                value={f.owner}
                active={f.owner !== ""}
                remote={api.pmsUsers}
                valueLabel={f.ownerLabel}
                onChange={(e) => onChange({ owner: e.target.value, ownerLabel: "" })}
                onOption={(o) => onChange({ owner: o.value, ownerLabel: o.label })}
              >
                <option value="">Theo “Của tôi”</option>
                {(m?.users ?? []).map((u) => <option key={u.value} value={u.value}>{u.label}</option>)}
              </Select>
            </label>
          </div>
          <div className="field">
            <span className="field-label">Tình trạng (chọn nhiều)</span>
            <MultiSelect
              label="Tình trạng"
              empty="Theo “Đang mở / Đã xong”"
              options={m?.statuses ?? []}
              value={f.statuses}
              onChange={(statuses) => onChange({ statuses })}
            />
            <p className="pms-pop-note">Chọn tình trạng cụ thể sẽ thay cho “Đang mở / Đã xong”.</p>
          </div>
        </>
      )}
      <div className="pms-pop-foot">
        <button type="button" className="ghost" onClick={() => onChange(NO_FILTERS)} disabled={filterCount(f) === 0}>Xoá bộ lọc</button>
        <button type="button" className="primary" onClick={onClose}>Xong</button>
      </div>
    </div>
  );
}

function Row({ it, on, onSelect }: { it: PmsItem; on: boolean; onSelect: () => void }) {
  const late = overdue(it);
  const facts = [it.category, it.progress && it.progress !== "0%" ? it.progress : "", it.hours ? `${it.hours} giờ` : ""].filter(Boolean);
  const who = [it.project, it.assignee].filter(Boolean).join(" · ");
  return (
    <button type="button" role="option" aria-selected={on} className={`pms-row is-${statusTone(it.status)}${on ? " is-on" : ""}`} onClick={onSelect}>
      <span className="pms-row-head">
        <span className="pms-row-title">
          {it.no ? <span className="mono pms-no">{it.no}</span> : null}
          {it.title}
        </span>
        {it.due ? <span className={`pms-due${late ? " is-late" : ""}`}>{late ? "Quá hạn " : "Hạn "}{viDate(it.due).slice(0, 5)}</span> : null}
      </span>
      <span className="pms-row-meta">
        <StatusChip s={it.status} />
        <PriorityChip p={it.priority} />
        {facts.length ? <span>{facts.join(" · ")}</span> : null}
        {!it.due && (it.updated || it.created) ? <span>{it.updated ? "Cập nhật" : "Tạo"} {viDate(it.updated || it.created)}</span> : null}
      </span>
      {who ? <span className="pms-row-who">{who}</span> : null}
    </button>
  );
}

function Preview({ it, onBack, onPick }: { it: PmsItem; onBack: () => void; onPick: (d: PmsDetail) => void }) {
  const q = useQuery({ queryKey: ["pms-item", it.kind, it.id], queryFn: () => api.pmsItem(it.kind, it.id), staleTime: 30_000, retry: false });
  const d = q.data;
  const show = d ?? it;
  const pri = priority(show.priority);
  const facts: [string, string][] = [
    ["Dự án", show.project],
    ["Phụ trách", show.assignee],
    ["Loại", show.category],
    ["Ưu tiên", pri?.label ?? ""],
    ["Bắt đầu", show.start ? viDate(show.start) : ""],
    ["Hạn", show.due ? viDate(show.due) : ""],
    ["Tiến độ", show.progress],
    ["Giờ đã log", show.hours],
    ["Cập nhật", show.updated ? viDate(show.updated, true) : ""],
  ].filter((f): f is [string, string] => f[1] !== "");
  const done = d ? d.checklist.filter((c) => c.done).length : 0;
  return (
    <article className="pms-preview" aria-label="Chi tiết">
      <button type="button" className="link pms-back" onClick={onBack}>← Danh sách</button>
      <div className="pms-pv-head">
        <span className="mono pms-pv-no">{KIND_LABEL[it.kind]} {show.no || `#${it.id}`}</span>
        <h3>{show.title}</h3>
        <div className="pms-pv-chips">
          <StatusChip s={show.status} />
          <PriorityChip p={show.priority} />
          {overdue(show) ? <span className="pms-pri is-hot">Quá hạn</span> : null}
        </div>
      </div>
      {facts.length ? (
        <dl className="pms-facts">
          {facts.map(([k, v]) => (
            <div key={k} className={k === "Dự án" ? "is-wide" : undefined}><dt>{k}</dt><dd>{v}</dd></div>
          ))}
        </dl>
      ) : null}
      {q.isPending ? (
        <div className="pms-pv-sec" aria-hidden="true">
          <span className="skel" style={{ width: "92%" }} />
          <span className="skel" style={{ width: "80%" }} />
          <span className="skel" style={{ width: "64%" }} />
        </div>
      ) : q.isError ? (
        <p className="hint hint-err">{serverMessage(q.error as Error)}</p>
      ) : d ? (
        <>
          <section className="pms-pv-sec">
            <h4>Mô tả</h4>
            {d.description ? <p className="pms-desc">{d.description}</p> : <p className="pms-none">Mục này chưa có mô tả trên PMS, tiêu đề sẽ được dùng làm yêu cầu.</p>}
          </section>
          {d.checklist.length ? (
            <section className="pms-pv-sec">
              <h4>Checklist <span className="pms-count">{done}/{d.checklist.length}</span></h4>
              <ul className="pms-check">
                {d.checklist.map((c, i) => (
                  <li key={i} className={c.done ? "is-done" : ""}><span aria-hidden="true">{c.done ? "✓" : ""}</span>{c.name}</li>
                ))}
              </ul>
            </section>
          ) : null}
          {d.subtasks.length ? (
            <section className="pms-pv-sec">
              <h4>Việc con <span className="pms-count">{d.subtasks.length}</span></h4>
              <ul className="pms-subs">
                {d.subtasks.map((s, i) => (
                  <li key={i}><span>{s.title}</span><StatusChip s={s.status} /></li>
                ))}
              </ul>
            </section>
          ) : null}
          {d.comments.length ? (
            <section className="pms-pv-sec">
              <h4>Bình luận gần đây <span className="pms-count">{d.comments.length}</span></h4>
              <ul className="pms-comments">
                {d.comments.map((c, i) => (
                  <li key={i}>
                    <span className="pms-c-who">{c.author || "Ẩn danh"} <span>{viDate(c.at, true)}</span></span>
                    <span className="pms-c-text">{c.text}</span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </>
      ) : null}
      <footer className="pms-pv-foot">
        <a className="pms-open" href={it.url} target="_blank" rel="noopener noreferrer">Mở trên PMS ↗</a>
        <button type="button" className="primary" disabled={!d} onClick={() => d && onPick(d)}>
          Dùng {KIND_LABEL[it.kind].toLowerCase()} này
        </button>
      </footer>
    </article>
  );
}

// ── Report tab ─────────────────────────────────────────────────────────────
// Paste a report's ID or link → its rows. Rows of a task/ticket report open like a
// search result (full detail); rows of any other module become the request as-is.
const RECENT = "ckit.pms.reports";
const RECENT_MAX = 8;
type Recent = { id: string; name: string };

function loadRecent(): Recent[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT) ?? "[]") as unknown;
    return Array.isArray(v) ? v.filter((r): r is Recent => typeof r?.id === "string" && /^\d{1,12}$/.test(r.id) && typeof r.name === "string").slice(0, RECENT_MAX) : [];
  } catch {
    return [];
  }
}

/** `1047`, `#1047` or a PMS link with `record=1047` → `1047`. */
function reportIdOf(s: string): string | null {
  const t = s.trim();
  return (/[?&]record=(\d{1,12})\b/.exec(t) ?? /^#?(\d{1,12})$/.exec(t))?.[1] ?? null;
}

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").replace(/đ/g, "d").replace(/Đ/g, "D").toLowerCase();

/** A task/ticket report row as a search result, for the shared detail preview. */
function rowItem(kind: PmsKind, row: PmsReportRow): PmsItem {
  const no = row.cells.find((c) => c.label === "Mã task" || c.label === "Mã ticket")?.value ?? "";
  return { kind, id: row.id, no, title: row.title, status: "", priority: "", project: "", assignee: "", due: "", updated: "", created: "", url: row.url, category: "", progress: "", start: "", hours: "" };
}

function ReportPicker({ tabs, onPick, onClose }: { tabs: ReactNode; onPick: (p: PmsPick) => void; onClose: () => void }) {
  const status = useQuery({ queryKey: ["pms-status"], queryFn: api.pmsStatus, staleTime: 60_000, retry: false });
  const hasKey = status.data?.report === true;
  const [recent, setRecent] = useState<Recent[]>(loadRecent);
  // Reopens the last report; "Đổi report" goes back to the ID box.
  const [id, setId] = useState<string | null>(() => recent[0]?.id ?? null);
  const [input, setInput] = useState("");
  const [bad, setBad] = useState(false);
  const [filter, setFilter] = useState("");
  const [sel, setSel] = useState<PmsReportRow | null>(null);
  const q = useInfiniteQuery({
    queryKey: ["pms-report", id],
    queryFn: ({ pageParam }) => api.pmsReport(id ?? "", pageParam),
    initialPageParam: 1,
    getNextPageParam: (last, all) => (last.has_more ? all.length + 1 : undefined),
    enabled: id !== null && hasKey,
    staleTime: 60_000,
    retry: false,
  });
  const rep = q.data?.pages[0];
  useEffect(() => {
    if (!rep) return;
    setRecent((old) => {
      const next = [{ id: rep.id, name: rep.name }, ...old.filter((r) => r.id !== rep.id)].slice(0, RECENT_MAX);
      try {
        localStorage.setItem(RECENT, JSON.stringify(next));
      } catch {
        /* private mode: no history */
      }
      return next;
    });
  }, [rep]);
  const open = (raw: string) => {
    const v = reportIdOf(raw);
    setBad(v === null);
    if (v === null) return;
    setId(v);
    setInput("");
    setFilter("");
    setSel(null);
  };
  const rows = q.data?.pages.flatMap((p) => p.rows) ?? [];
  const f = fold(filter.trim());
  const shown = f ? rows.filter((r) => fold([r.title, ...r.cells.map((c) => c.value)].join(" ")).includes(f)) : rows;
  const name = rep?.name || recent.find((r) => r.id === id)?.name || "";

  return (
    <section className={`pms-panel${sel ? " has-sel" : ""}`} aria-label="Lấy việc từ report PMS">
      <header className="pms-bar">
        {tabs}
        {id !== null && hasKey ? (
          <>
            <span className="pms-rep-chip" title={name}>
              <span className="mono">Report {id}</span>
              {name ? <span className="pms-rep-name">{name}</span> : null}
            </span>
            <input type="search" className="pms-search" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Lọc các dòng trong report" aria-label="Lọc dòng" autoFocus />
            <button type="button" className="ghost pms-rep-change" onClick={() => { setId(null); setSel(null); }}>Đổi report</button>
          </>
        ) : hasKey ? (
          <>
            <input
              type="search"
              className="pms-search"
              value={input}
              onChange={(e) => { setInput(e.target.value); setBad(false); }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  open(input);
                }
              }}
              placeholder="Dán ID hoặc link report PMS, vd 1047"
              aria-label="ID hoặc link report"
              autoFocus
            />
            <button type="button" onClick={() => open(input)} disabled={!input.trim()}>Mở report</button>
          </>
        ) : null}
        <button type="button" className="ghost pms-close" onClick={onClose} title="Không lấy từ PMS, tự nhập tiêu đề và yêu cầu">Tự nhập</button>
      </header>
      {status.isPending ? null : !hasKey ? (
        <div className="pms-empty pms-rep-key">
          <strong>Cần Access Key PMS để xem report</strong>
          <span>Trên PMS, mở Tuỳ chọn cá nhân (My Preferences) và chép mục Access Key, rồi thêm vào kết nối PMS. Board chỉ đọc report, không sửa gì trên PMS.</span>
          <PmsConnectLink label="Thêm Access Key" />
        </div>
      ) : id === null ? (
        <div className="pms-rep-start">
          {bad ? <p className="hint hint-err">Không đọc được ID report. Dán số (vd 1047) hoặc link report trên PMS.</p> : null}
          {recent.length ? (
            <>
              <p className="faint">Report đã mở gần đây</p>
              <div className="pms-rep-recent">
                {recent.map((r) => (
                  <button key={r.id} type="button" className="pms-row" onClick={() => open(r.id)}>
                    <span className="pms-row-title"><span className="mono pms-no">{r.id}</span>{r.name || "Report"}</span>
                  </button>
                ))}
              </div>
            </>
          ) : (
            <p className="faint">Mở report trên PMS, chép ID trên thanh địa chỉ (số sau record=) hoặc cả link, rồi dán vào ô bên trên.</p>
          )}
        </div>
      ) : (
        <div className="pms-body">
          <div className="pms-listcol">
            <div className="pms-list" role="listbox" aria-label="Các dòng của report" aria-busy={q.isFetching}>
              {q.isPending ? (
                [0, 1, 2, 3].map((i) => (
                  <div key={i} className="pms-row pms-skel" aria-hidden="true">
                    <span className="skel" style={{ width: `${78 - i * 10}%` }} />
                    <span className="skel" style={{ width: "46%" }} />
                  </div>
                ))
              ) : q.isError ? (
                <p className="hint hint-err pms-msg">{serverMessage(q.error as Error)}</p>
              ) : rep?.chart && rows.length === 0 ? (
                <p className="pms-msg">Report này là biểu đồ nên không có dòng để chọn. Thử một report dạng bảng nhé.</p>
              ) : shown.length === 0 ? (
                <p className="pms-msg">{rows.length ? "Không có dòng nào khớp. Thử từ khoá khác nhé." : "Report này chưa có dòng nào."}</p>
              ) : (
                shown.map((r) => (
                  <button key={r.id} type="button" role="option" aria-selected={sel?.id === r.id} className={`pms-row is-idle${sel?.id === r.id ? " is-on" : ""}`} onClick={() => setSel(r)}>
                    <span className="pms-row-head"><span className="pms-row-title">{r.title}</span></span>
                    {r.cells.length ? (
                      <span className="pms-row-meta">
                        <span>{r.cells.filter((c) => c.value !== r.title).slice(0, 3).map((c) => c.value.replace(/\s+/g, " ")).join(" · ")}</span>
                      </span>
                    ) : null}
                  </button>
                ))
              )}
              {q.hasNextPage ? (
                <button type="button" className="ghost pms-more" onClick={() => q.fetchNextPage()} disabled={q.isFetchingNextPage}>
                  {q.isFetchingNextPage ? "Đang tải…" : "Tải thêm"}
                </button>
              ) : null}
            </div>
          </div>
          <div className="pms-previewcol">
            {sel && rep?.kind ? (
              <Preview key={sel.id} it={rowItem(rep.kind, sel)} onBack={() => setSel(null)} onPick={(d) => onPick(pmsToTask(d))} />
            ) : sel && rep ? (
              <RowPreview rep={rep} row={sel} onBack={() => setSel(null)} onPick={() => onPick(reportRowToTask(rep, sel))} />
            ) : (
              <div className="pms-empty">
                <strong>Chọn một dòng bên trái để xem chi tiết</strong>
                <span>{rep?.kind ? `Mỗi dòng là một ${KIND_LABEL[rep.kind].toLowerCase()}: mô tả, checklist và bình luận sẽ được chép vào ô Yêu cầu.` : "Các cột của dòng sẽ được chép vào ô Yêu cầu."} Bạn vẫn sửa lại được trước khi tạo task.</span>
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function RowPreview({ rep, row, onBack, onPick }: { rep: PmsReport; row: PmsReportRow; onBack: () => void; onPick: () => void }) {
  return (
    <article className="pms-preview" aria-label="Chi tiết dòng">
      <button type="button" className="link pms-back" onClick={onBack}>← Danh sách</button>
      <div className="pms-pv-head">
        <span className="mono pms-pv-no">{rep.module || "Report"} #{row.id}</span>
        <h3>{row.title}</h3>
      </div>
      <dl className="pms-facts">
        {row.cells.map((c, i) => (
          <div key={i} className={c.value.length > 40 ? "is-wide" : undefined}><dt>{c.label}</dt><dd className="pms-cell">{c.value}</dd></div>
        ))}
      </dl>
      <footer className="pms-pv-foot">
        <a className="pms-open" href={row.url || rep.url} target="_blank" rel="noopener noreferrer">Mở trên PMS ↗</a>
        <button type="button" className="primary" onClick={onPick}>Dùng dòng này</button>
      </footer>
    </article>
  );
}
