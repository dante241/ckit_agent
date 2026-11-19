// Log drawer of one run, read human-first: agent text is prose; each tool call is
// "• tool intent" with its key argument muted under it; long runs of calls fold into one
// "N thao tác: read 8, bash 3" line (errors stay visible); real errors are red, harness
// blocks are grey. Filters: lỗi thật / lỗi harness / sửa tệp; a subagent picker; raw lines.
import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, type BoardLogLine, type BoardTask } from "./api";
import { Drawer, EndpointMissing, ErrorState, isMissing } from "./ui";
import { Markdown } from "./markdown";
import { ModelChip } from "./ModelChip";
import { money, useRoot, verdictTone } from "./Board";
import { hhmm, runSecs, runTitle, secsText, verdictText } from "./runs";
import { Select } from "./Select";

// ── Log lines → readable items ─────────────────────────────────────────────
/** One tool call, digested from the backend's `name {json args}` line (args may be clipped). */
type Call = { tool: string; intent: string; arg: string; argFull: string; raw: string };

// Arguments worth showing under the intent, most telling first.
const ARG_KEYS = ["path", "file", "relative_path", "command", "pattern", "query", "url", "name_path_pattern", "qualified_name", "function_name", "name"];
// Tools that change files ("Sửa tệp").
const EDIT_TOOLS = new Set(["edit", "write", "ast_edit", "apply_patch", "notebook_edit", "multi_edit"]);
// A run of this many calls folds into one summary line.
const FOLD_AT = 4;

/** A string field from possibly-clipped JSON: complete values parse, a clipped tail is kept as-is. */
function field(raw: string, key: string): string | undefined {
  const m = new RegExp(`"${key}"\\s*:\\s*"((?:[^"\\\\]|\\\\.)*)("|$)`).exec(raw);
  if (!m) return undefined;
  const body = (m[1] ?? "").replace(/\\$/, "");
  try {
    return JSON.parse(`"${body}"`) as string;
  } catch {
    return body.replace(/\\n/g, "\n").replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }
}

function parseCall(text: string): Call {
  const sp = text.indexOf(" ");
  const tool = sp < 0 ? text : text.slice(0, sp);
  const rest = sp < 0 ? "" : text.slice(sp + 1).trim();
  let obj: Record<string, unknown> | null = null;
  try {
    const v = JSON.parse(rest);
    if (v && typeof v === "object" && !Array.isArray(v)) obj = v as Record<string, unknown>;
  } catch {
    /* clipped: fall back to field() */
  }
  const get = (k: string) => (obj ? (typeof obj[k] === "string" ? (obj[k] as string) : undefined) : field(rest, k));
  const intent = (get("i") ?? get("description") ?? "").trim();
  let full = "";
  for (const k of ARG_KEYS) {
    const v = get(k);
    if (v) {
      full = v;
      break;
    }
  }
  // edit: the target is the first `[path#TAG]` header of the patch.
  if (!full) {
    const input = get("input");
    const target = input ? /\[([^\]#]+)#/.exec(input)?.[1] : undefined;
    if (target) full = target;
    else if (input) full = input;
  }
  if (!full && !intent) full = rest;
  const first = full.split("\n").find((l) => l.trim()) ?? "";
  return { tool, intent, arg: first.trim(), argFull: full, raw: rest };
}

const toolOf = (l: BoardLogLine) => (l.kind === "tool" ? parseCall(l.text).tool : "");
const isEdit = (l: BoardLogLine) => EDIT_TOOLS.has(toolOf(l));

/** Lets long paths break after `/` instead of mid-name. */
function slashBreak(s: string): ReactNode {
  if (!s.includes("/")) return s;
  const parts = s.split("/");
  return parts.map((p, i) => (
    <Fragment key={i}>
      {p}
      {i < parts.length - 1 ? <>/<wbr /></> : null}
    </Fragment>
  ));
}

/** Pretty JSON when the args are complete, the raw text when clipped. */
function prettyRaw(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

type Item = { kind: "text"; text: string } | { kind: "calls"; lines: BoardLogLine[] };

/** Consecutive tool calls (and their errors) form one group; text stands alone. */
function groupLines(lines: BoardLogLine[]): Item[] {
  const out: Item[] = [];
  for (const l of lines) {
    if (l.kind === "text") {
      out.push({ kind: "text", text: l.text });
      continue;
    }
    const last = out[out.length - 1];
    if (last && last.kind === "calls") last.lines.push(l);
    else out.push({ kind: "calls", lines: [l] });
  }
  return out;
}

function CallRow({ l }: { l: BoardLogLine }) {
  const [open, setOpen] = useState(false);
  if (l.kind === "error" || l.kind === "harness") {
    const c = l.text.indexOf(": ");
    const tool = c > 0 && c < 40 ? l.text.slice(0, c) : "";
    const msg = tool ? l.text.slice(c + 2) : l.text;
    const harness = l.kind === "harness";
    return (
      <li className={`rl-call ${harness ? "is-harness" : "is-err"}`}>
        <span className="rl-dot" aria-hidden="true">{harness ? "·" : "!"}</span>
        <div className="rl-call-main">
          <span className="rl-call-line">{tool ? <strong className="rl-tool">{tool}</strong> : null} {harness ? "bị harness chặn" : "lỗi"}</span>
          <span className="rl-err-text">{slashBreak(msg)}</span>
        </div>
      </li>
    );
  }
  const c = parseCall(l.text);
  return (
    <li className={`rl-call ${open ? "is-open" : ""}`}>
      <button className="rl-call-btn" aria-expanded={open} onClick={() => setOpen(!open)} title={open ? "Ẩn chi tiết" : "Xem tham số đầy đủ"}>
        <span className="rl-dot" aria-hidden="true">•</span>
        <span className="rl-call-main">
          <span className="rl-call-line">
            <strong className="rl-tool">{c.tool}</strong> {c.intent ? <span className="rl-intent">{c.intent}</span> : null}
          </span>
          {c.arg ? <span className="rl-arg" title={c.argFull}>{c.arg}</span> : null}
        </span>
      </button>
      {open ? <pre className="rl-raw">{slashBreak(c.argFull || prettyRaw(c.raw))}</pre> : null}
    </li>
  );
}

/** "14 thao tác: read 8, bash 3, eval 3". */
function callSummary(lines: BoardLogLine[]): string {
  const by = new Map<string, number>();
  for (const l of lines) if (l.kind === "tool") by.set(toolOf(l), (by.get(toolOf(l)) ?? 0) + 1);
  const n = [...by.values()].reduce((a, b) => a + b, 0);
  const top = [...by.entries()].sort((a, b) => b[1] - a[1]).slice(0, 4).map(([k, v]) => `${k} ${v}`);
  return `${n} thao tác: ${top.join(", ")}${by.size > 4 ? ", …" : ""}`;
}

function Calls({ lines }: { lines: BoardLogLine[] }) {
  const n = lines.filter((l) => l.kind === "tool").length;
  const [open, setOpen] = useState(n < FOLD_AT);
  const problems = lines.filter((l) => l.kind !== "tool");
  return (
    <div className="rl-calls">
      {n >= FOLD_AT ? (
        <button className="rl-fold" aria-expanded={open} onClick={() => setOpen(!open)}>
          <span>{callSummary(lines)}</span>
          <span className="rl-fold-act">{open ? "Thu gọn" : "Mở"}</span>
        </button>
      ) : null}
      {open || problems.length ? (
        <ol className="rl-call-list">
          {(open ? lines : problems).map((l, i) => <CallRow key={i} l={l} />)}
        </ol>
      ) : null}
    </div>
  );
}

// ── Drawer ─────────────────────────────────────────────────────────────────
type View = "all" | "error" | "harness" | "edit" | "raw";

export function LogDrawer({ task, run, phaseName, onClose }: { task: BoardTask; run: number; phaseName?: string; onClose: () => void }) {
  const root = useRoot();
  const r = task.runs[run];
  const [agent, setAgent] = useState("");
  const [view, setView] = useState<View>("all");
  const log = useQuery({
    queryKey: ["board-log", root, task.id, run, agent],
    queryFn: () => api.boardLog(root, task.id, run, agent || undefined),
    refetchInterval: (q) => (q.state.data?.running ? 2000 : false),
  });

  // Follow the end while the boss sits at the bottom of a live log.
  const boxRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const lines = log.data?.lines ?? [];
  useEffect(() => {
    const el = boxRef.current?.closest<HTMLElement>(".bd-dbody");
    if (el && stick.current && log.data?.running) el.scrollTop = el.scrollHeight;
  }, [lines.length, log.data?.running]);
  useEffect(() => {
    const el = boxRef.current?.closest<HTMLElement>(".bd-dbody");
    if (!el) return;
    const on = () => (stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40);
    el.addEventListener("scroll", on);
    return () => el.removeEventListener("scroll", on);
  }, []);

  if (!r) return null;
  const d = r.digest;
  const subs = d?.subagents_list ?? [];
  const nErr = lines.filter((l) => l.kind === "error").length;
  const nHarness = lines.filter((l) => l.kind === "harness").length;
  const nEdit = lines.filter(isEdit).length;
  const shown = view === "error" ? lines.filter((l) => l.kind === "error") : view === "harness" ? lines.filter((l) => l.kind === "harness") : view === "edit" ? lines.filter(isEdit) : lines;
  const chip = (v: View, label: string, n?: number) => (
    <button className={`chip ${view === v ? "is-on" : ""}`} aria-pressed={view === v} onClick={() => setView(view === v && v !== "all" ? "all" : v)}>
      {label}{n !== undefined ? ` ${n}` : ""}
    </button>
  );

  const head = (
    <>
      <h3 className="dr-title">{runTitle(r)}{r.phase ? ` · ${r.phase}${phaseName ? ` ${phaseName}` : ""}` : ""}</h3>
      <div className="dr-meta">
        {r.ended === null ? <span className="tag info"><span className="bd-spin" aria-hidden="true" />Đang chạy</span> : r.verdict ? <span className={`tag ${verdictTone(r.verdict)}`}>{verdictText(r.verdict)}</span> : null}
        <span>{hhmm(r.started)}{r.ended ? ` → ${hhmm(r.ended)}` : ""} · {secsText(runSecs(r))}</span>
        <span>{money(r.cost)}</span>
        <ModelChip m={r.model} />
        {d ? <span>{d.tools} lệnh</span> : null}
        {d?.subagents ? <span>{d.subagents} subagent</span> : null}
      </div>
    </>
  );
  const toolbar = (
    <div className="dr-filters" role="group" aria-label="Lọc log">
      {chip("all", "Diễn biến")}
      {chip("error", "Lỗi thật", nErr)}
      {chip("harness", "Lỗi harness", nHarness)}
      {chip("edit", "Sửa tệp", nEdit)}
      {subs.length ? (
        <Select compact active={agent !== ""} className="dr-session" value={agent} onChange={(e) => setAgent(e.target.value)} aria-label="Xem phiên">
          <option value="">Phiên chính · {subs.length} subagent</option>
          {subs.map((s) => (
            <option key={s.id} value={s.id}>{s.name || s.id}{s.model ? ` · ${s.model.replace(/^claude-/, "")}` : ""}</option>
          ))}
        </Select>
      ) : null}
      {chip("raw", "Thô (JSON)")}
    </div>
  );

  return (
    <Drawer label={`Log ${runTitle(r)}`} head={head} toolbar={toolbar} onClose={onClose} className="dr-log">
      <div ref={boxRef} className={`rl-log ${view === "raw" ? "is-raw" : ""}`} aria-live={log.data?.running ? "polite" : undefined} aria-busy={log.isLoading}>
        {isMissing(log.error) ? (
          <EndpointMissing endpoint={`/api/board/${task.id}/log`} />
        ) : log.error ? (
          <ErrorState message={(log.error as Error).message} />
        ) : log.isLoading ? (
          <span className="faint">Đang tải log…</span>
        ) : shown.length === 0 ? (
          <span className="faint">{lines.length === 0 ? "Log trống." : "Không có dòng nào khớp bộ lọc."}</span>
        ) : view === "raw" ? (
          shown.map((l, i) => <div key={i} className={`rl-rawline k-${l.kind}`}>{slashBreak(l.text)}</div>)
        ) : view !== "all" ? (
          <ol className="rl-call-list">{shown.map((l, i) => <CallRow key={i} l={l} />)}</ol>
        ) : (
          groupLines(shown).map((it, i) =>
            it.kind === "text" ? (
              <div key={i} className="rl-text"><Markdown source={it.text} /></div>
            ) : (
              <Calls key={i} lines={it.lines} />
            ),
          )
        )}
        {log.data?.running ? <div className="bd-log-live"><span className="bd-spin" aria-hidden="true" /> đang chạy</div> : null}
      </div>
    </Drawer>
  );
}
