// ckit Board in omp. Two modes, picked at load:
//  - Coordinator (env CKIT_BOARD_API, set by the Board for its coordinator
//    session, loaded with `--no-extensions -e`): the assistant's board tools.
//  - Boss (every interactive omp session, deployed to ~/.omp/agent/extensions):
//    footer line, "needs you" notices, /board and /pms, plus the same tools with
//    every state change gated by the boss's own confirm.
// Headless Board step runs (env CKIT_BOARD_STAGE) get nothing: a Dev agent must
// never hold board_approve. Everything is a call to the dashboard's HTTP API;
// the dashboard (Rust) owns the rules and the state.
import { realpathSync } from "node:fs";
import type { ExtensionAPI, ExtensionContext } from "@oh-my-pi/pi-coding-agent";

type Api = (method: "GET" | "POST", path: string, body?: unknown) => Promise<any>;

function client(base: string, root: () => string): Api {
  return async (method, path, body) => {
    const r0 = root();
    const q = r0 ? `${path.includes("?") ? "&" : "?"}root=${encodeURIComponent(r0)}` : "";
    const r = await fetch(`${base}${path}${q}`, {
      method,
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(method === "GET" ? 8000 : 120000),
    });
    const raw = await r.text();
    if (!r.ok) throw new Error(raw || `HTTP ${r.status}`);
    return raw ? JSON.parse(raw) : null;
  };
}

const text = (s: string) => ({ content: [{ type: "text" as const, text: s }] });

export default function (pi: ExtensionAPI) {
  if (process.env.CKIT_BOARD_STAGE) return;
  if (process.env.CKIT_BOARD_API) return coordinator(pi, process.env.CKIT_BOARD_API);
  boss(pi, (process.env.CKIT_BOARD_URL || "http://127.0.0.1:8731").replace(/\/+$/, ""));
}

// ── Shared tools ────────────────────────────────────────────────────────────

/** `gate`: runs before a state change; a refusal string aborts it. */
function registerTools(pi: ExtensionAPI, call: Api, who: string, gate: (ctx: ExtensionContext, what: string) => Promise<string | null>) {
  const { z } = pi.zod;
  const safe = (f: () => Promise<string>) => f().then(text, (e: Error) => text(`ERROR ${e.message}`));
  const guarded = (ctx: ExtensionContext, what: string, f: () => Promise<string>) =>
    safe(async () => (await gate(ctx, what)) ?? (await f()));
  const id = z.string().describe("task id, e.g. T-003");

  pi.registerTool({
    name: "board_status",
    label: "Board: status",
    description: "All tasks of this project: id, stage, status, paused, open questions, phase progress, cost.",
    parameters: z.object({}),
    execute: () =>
      safe(async () => {
        const d = await call("GET", "/api/board");
        const lines = (d.tasks ?? []).map(
          (t: any) =>
            `${t.id} [${t.stage}/${t.status}${t.asking ? ` · ${t.asking} câu hỏi chờ sếp` : ""}${t.phase ? ` · ${t.phase}` : ""}${t.progress ? ` ${t.progress.done}/${t.progress.total}` : ""}] ${t.title} — $${(t.cost ?? 0).toFixed(2)}${t.note ? ` (${t.note})` : ""}`,
        );
        return lines.length ? lines.join("\n") : "(no tasks)";
      }),
  });

  pi.registerTool({
    name: "board_task",
    label: "Board: task detail",
    description: "One task: steps, stage/status, open questions (asks), last thread messages, document names, and the tail of the current run log.",
    parameters: z.object({ id, log_lines: z.number().int().min(0).max(200).default(40) }),
    execute: (_i, p) =>
      safe(async () => {
        const d = await call("GET", `/api/board/${p.id}`);
        const t = d.task;
        const log = p.log_lines ? await call("GET", `/api/board/${p.id}/log`) : null;
        const asks = (t.asks ?? []).filter((a: any) => a.status === "answering" || a.status === "boss");
        return [
          `${t.id} ${t.title}`,
          `stage=${t.stage} status=${t.status} paused=${!!t.paused}${t.paused ? ` paused_by=${t.paused_by || "-"}${t.paused_by === "new" ? " (chưa bắt đầu: chờ sếp bấm Bắt đầu)" : ""}` : ""} steps=${(t.steps ?? []).join("→")} branch=${t.branch ?? "-"} cost=$${t.cost.toFixed(2)}${d.phase ? ` phase=${d.phase}` : ""}${d.progress ? ` ${d.progress.done}/${d.progress.total}` : ""}`,
          `request: ${t.request}`,
          asks.length ? `open questions (the Leader answers them, not you):\n${asks.map((a: any) => `- ${a.id} (${a.from}→${a.to}, ${a.status}): ${a.question}`).join("\n")}` : "open questions: none",
          `docs: ${(d.docs ?? []).map((x: any) => x.name).join(", ") || "-"} (read them from the project's planning tree / .cache/ckit/board/${t.id}/)`,
          `thread (last 12):\n${(t.thread ?? []).slice(-12).map((m: any) => `[${m.from}] ${m.text}`).join("\n\n")}`,
          log ? `run log tail${log.running ? " (running)" : ""}:\n${(log.lines ?? []).slice(-p.log_lines).map((l: any) => `${l.kind}: ${l.text}`).join("\n")}` : "",
        ].join("\n\n");
      }),
  });

  pi.registerTool({
    name: "board_create",
    label: "Board: create task",
    description:
      "Create a task in this project. `steps` (catalog order) optional: investigate, intake, solution, review, approval, dev, test, security, report, acceptance — omit for the full /feature pipeline. Split big work into independent tasks; tasks in one project run one at a time.",
    parameters: z.object({ title: z.string().min(3), request: z.string().min(10), steps: z.array(z.string()).optional() }),
    execute: (_i, p, _s, _u, ctx) =>
      guarded(ctx, `Tạo task "${p.title}" trên Board?`, async () => {
        const t = await call("POST", "/api/board", { title: p.title, request: p.request, steps: p.steps });
        return `created ${t.id} (${t.steps.join("→")})`;
      }),
  });

  pi.registerTool({
    name: "board_message",
    label: "Board: message / pause a step",
    description:
      "Interject into a task's RUNNING step: kind=note delivers a message into the live agent session (redirect, correction); kind=pause makes it stop at a safe point and wait (killed after 5 min if it does not). Use sparingly — only for real deviation, loops, wasted cost, or the boss's order.",
    parameters: z.object({ id, kind: z.enum(["note", "pause"]), text: z.string().default("") }),
    execute: (_i, p, _s, _u, ctx) =>
      guarded(ctx, `${p.kind === "pause" ? "Tạm dừng" : "Nhắn"} ${p.id}${p.text ? `: ${p.text}` : ""}?`, async () => (await call("POST", `/api/board/${p.id}/inbox`, { kind: p.kind, text: p.text, from: who }), `sent ${p.kind} to ${p.id}`)),
  });

  pi.registerTool({
    name: "board_resume",
    label: "Board: resume paused step",
    description: "Continue a paused step in its own session, optionally with an instruction.",
    parameters: z.object({ id, text: z.string().default("") }),
    execute: (_i, p, _s, _u, ctx) =>
      guarded(ctx, `Cho ${p.id} chạy tiếp?`, async () => (await call("POST", `/api/board/${p.id}/resume`, { kind: "note", text: p.text, from: who }), `resumed ${p.id}`)),
  });

  pi.registerTool({
    name: "board_approve",
    label: who === "coordinator" ? "Board: approve a plan for the boss" : "Board: approve a plan (asks the boss)",
    description:
      who === "coordinator"
        ? "Approve a task waiting at the plan-approval gate, per the delegation policy. Cannot accept (create the merge request): the server refuses."
        : "Ask the boss to approve the plan of a task waiting at the plan-approval gate (a confirm dialog; nothing happens unless the boss clicks). Never accepts / creates merge requests.",
    parameters: z.object({ id }),
    execute: (_i, p, _s, _u, ctx) =>
      guarded(ctx, `Duyệt kế hoạch ${p.id} cho Dev bắt đầu code?`, async () => (await call("POST", `/api/board/${p.id}/approve${who === "coordinator" ? "?by=coordinator" : ""}`), `approved plan of ${p.id}`)),
  });
}

// ── Coordinator ─────────────────────────────────────────────────────────────

function coordinator(pi: ExtensionAPI, base: string) {
  pi.setLabel("ckit board coordinator tools");
  const root = process.env.CKIT_BOARD_ROOT ?? "";
  registerTools(pi, client(base, () => root), "coordinator", async () => null);
}

// ── Boss mode ───────────────────────────────────────────────────────────────

type Summary = { id: string; title: string; stage: string; status: string; asking?: number; paused?: boolean; started?: boolean; note?: string | null; run_started?: string | null; workflow?: any };
type Project = { project: string; name: string; tasks: Summary[] };

const STAGE: Record<string, string> = {
  investigate: "Điều tra", intake: "Tiếp nhận", solution: "Kế hoạch", review: "Phản biện kế hoạch", approval: "Duyệt kế hoạch",
  dev: "Phát triển", test: "Kiểm thử", security: "Review bảo mật", report: "Báo cáo", acceptance: "Nghiệm thu", action: "Tạo MR", done: "Xong", cancelled: "Đã huỷ",
};
const terminal = (t: Summary) => t.status === "done" || t.status === "cancelled" || t.stage === "done" || t.stage === "cancelled";
const needsYou = (t: Summary) => !terminal(t) && ((t.asking ?? 0) > 0 || t.status === "waiting" || t.status === "failed");

/** What the boss is asked for, in one phrase. */
function ask(t: Summary): string {
  if ((t.asking ?? 0) > 0) return `team hỏi bạn ${t.asking} câu`;
  if (t.status === "failed") return `bước ${STAGE[t.stage] ?? t.stage} bị lỗi`;
  if (t.paused) return t.started ? "đang tạm dừng" : "chưa bắt đầu";
  if (t.stage === "approval") return "chờ bạn duyệt kế hoạch";
  if (t.stage === "acceptance") return "chờ bạn nghiệm thu";
  if (t.stage === "investigate") return "Leader đã trả lời";
  return "chờ bạn trả lời";
}

const paint = (sgr: string, s: string) => `\x1b[${sgr}m${s}\x1b[0m`;
const minutes = (iso?: string | null) => (iso ? Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60000)) : 0);

function boss(pi: ExtensionAPI, base: string) {
  pi.setLabel("ckit board");
  let projects: Project[] = [];
  let here: string | null = null; // the Board project this session works in
  let cwd = "";
  let up = false;
  let seen = new Map<string, string>(); // task key → last "needs you" phrase
  let first = true;
  let timer: ReturnType<typeof setInterval> | undefined;
  let gen = 0;

  const call = client(base, () => here ?? "");
  const callIn = (root: string) => client(base, () => root);

  const confirmGate = async (ctx: ExtensionContext, what: string): Promise<string | null> => {
    if (!ctx.hasUI) return "refused: no interactive boss in this session";
    return (await ctx.ui.confirm("Board", what)) ? null : "the boss declined";
  };
  registerTools(pi, call, "boss", confirmGate);

  function pickHere() {
    let real = cwd;
    try {
      real = realpathSync(cwd);
    } catch {
      /* keep cwd */
    }
    const match = projects
      .filter((p) => real === p.project || real.startsWith(p.project.endsWith("/") ? p.project : `${p.project}/`))
      .sort((a, b) => b.project.length - a.project.length)[0];
    here = match?.project ?? null;
  }

  const scope = () => (here ? projects.filter((p) => p.project === here) : projects);

  function footer(ctx: ExtensionContext) {
    if (!ctx.hasUI) return;
    try {
      if (!up) {
        ctx.ui.setWidget("ckit-board", [paint("90", "Board: chưa chạy (ckit service install)")], { placement: "belowEditor" });
        return;
      }
      const tasks = scope().flatMap((p) => p.tasks).filter((t) => !terminal(t));
      const need = tasks.filter(needsYou);
      const run = tasks.filter((t) => t.status === "running" && !(t.asking ?? 0));
      const queued = tasks.filter((t) => t.status === "queued");
      const parts: string[] = [];
      if (need.length) parts.push(paint("93", `⚠ ${need.length} cần bạn (${need.slice(0, 3).map((t) => t.id).join(", ")})`));
      for (const t of run.slice(0, 2)) parts.push(paint("96", `● ${t.id} ${STAGE[t.stage] ?? t.stage} ${minutes(t.run_started)}'`));
      if (queued.length) parts.push(paint("90", `${queued.length} xếp hàng`));
      const label = here ? `Board ${projects.find((p) => p.project === here)?.name ?? ""}` : "Board";
      ctx.ui.setWidget("ckit-board", [`${paint("90", `${label} ▸`)} ${parts.length ? parts.join(paint("90", " · ")) : paint("90", "không có việc nào chờ bạn")}  ${paint("90", "/board")}`], { placement: "belowEditor" });
    } catch {
      /* torn-down context */
    }
  }

  async function refresh(ctx: ExtensionContext, myGen: number) {
    try {
      const d = await client(base, () => "")("GET", "/api/board/all");
      if (myGen !== gen) return;
      projects = d.projects ?? [];
      up = true;
    } catch {
      up = false;
    }
    pickHere();
    footer(ctx);
    // A task that newly needs the boss: one notice per change.
    const now = new Map<string, string>();
    for (const p of scope()) for (const t of p.tasks) if (needsYou(t)) now.set(`${p.project}|${t.id}`, ask(t));
    if (!first && ctx.hasUI) {
      for (const [k, what] of now) {
        if (seen.get(k) === what) continue;
        const [root, id] = k.split("|");
        const t = projects.find((p) => p.project === root)?.tasks.find((x) => x.id === id);
        if (t) ctx.ui.notify(`🔔 ${t.id} ${t.title}: ${what} (gõ /board)`, t.status === "failed" ? "warning" : "info");
      }
    }
    if (up) {
      seen = now;
      first = false;
    }
  }

  pi.on("session_start", async (_e, ctx) => {
    gen++;
    const myGen = gen;
    cwd = ctx.cwd;
    clearInterval(timer);
    if (!ctx.hasUI) return;
    first = true;
    await refresh(ctx, myGen);
    timer = setInterval(() => void refresh(ctx, myGen), 20000);
    (timer as unknown as { unref?: () => void }).unref?.();
  });
  pi.on("turn_end", async (_e, ctx) => {
    if (ctx.hasUI) void refresh(ctx, gen);
  });
  pi.on("session_shutdown", async (_e, ctx) => {
    gen++;
    clearInterval(timer);
    try {
      ctx.ui.setWidget("ckit-board", undefined);
    } catch {
      /* ignore */
    }
  });

  // ── /board ────────────────────────────────────────────────────────────────

  async function openWeb(root: string, id?: string) {
    const url = `${base}/?page=board${id ? `&task=${encodeURIComponent(id)}&root=${encodeURIComponent(root)}` : ""}`;
    await pi.exec(process.platform === "darwin" ? "open" : "xdg-open", [url]).catch(() => undefined);
  }

  async function boardMenu(ctx: ExtensionContext) {
    await refresh(ctx, gen);
    if (!up) {
      ctx.ui.notify(`Board chưa chạy ở ${base}. Bật bằng \`ckit service install\` (hoặc \`ckit harness web\`).`, "warning");
      return;
    }
    const items: { label: string; root: string; t?: Summary }[] = [];
    const many = scope().length > 1;
    for (const p of scope()) {
      const pre = many ? `[${p.name}] ` : "";
      const live = p.tasks.filter((t) => !terminal(t));
      for (const t of live.filter(needsYou)) items.push({ label: `⚠ ${pre}${t.id}  ${t.title}  ·  ${ask(t)}`, root: p.project, t });
      for (const t of live.filter((t) => t.status === "running" && !needsYou(t))) items.push({ label: `● ${pre}${t.id}  ${t.title}  ·  ${STAGE[t.stage] ?? t.stage} ${minutes(t.run_started)}'`, root: p.project, t });
      for (const t of live.filter((t) => t.status === "queued")) items.push({ label: `○ ${pre}${t.id}  ${t.title}  ·  xếp hàng`, root: p.project, t });
    }
    items.push({ label: "＋ Tạo task từ PMS (/pms)", root: "" }, { label: "↗ Mở Board trên web", root: here ?? "" });
    const pick = await ctx.ui.select(here ? `Board · ${projects.find((p) => p.project === here)?.name}` : "Board · tất cả dự án", items.map((i) => i.label));
    const it = items.find((i) => i.label === pick);
    if (!it) return;
    if (it.label.startsWith("＋")) return pmsMenu(ctx);
    if (!it.t) return openWeb(it.root);
    return taskMenu(ctx, it.root, it.t.id);
  }

  async function taskMenu(ctx: ExtensionContext, root: string, id: string) {
    const api = callIn(root);
    const d = await api("GET", `/api/board/${id}`);
    const t = d.task;
    const asks = (t.asks ?? []).filter((a: any) => a.status === "boss");
    const leader = [...(t.thread ?? [])].reverse().find((m: any) => m.from !== "user" && m.from !== "system");
    const acts: string[] = [];
    if (asks.length) acts.push("Trả lời câu hỏi của team");
    if (t.status === "waiting" && t.stage === "approval" && !asks.length) acts.push("Xem kế hoạch", "Duyệt kế hoạch", "Yêu cầu sửa kế hoạch");
    else if (t.status === "waiting" && t.stage === "acceptance") acts.push("Xem báo cáo", "Nghiệm thu, tạo MR (mở web)", "Yêu cầu sửa");
    else if (t.status === "waiting" && t.paused) acts.push(t.runs?.length ? "Tiếp tục" : "Bắt đầu");
    else if (t.status === "waiting" && t.stage === "investigate" && !asks.length) acts.push("Hỏi tiếp", "Xong, đóng câu hỏi");
    else if (t.status === "waiting" && !asks.length) acts.push("Trả lời Leader");
    if (t.status === "failed") acts.push("Xem lỗi", "Chạy lại");
    if (t.status === "running") acts.push("Nhắn cho bước đang chạy", "Tạm dừng");
    acts.push("Mở trang task trên web");
    const head = `${t.id} ${t.title} · ${STAGE[t.stage] ?? t.stage}${t.note ? ` · ${t.note}` : ""}`;
    const act = await ctx.ui.select(head, acts);
    const done = (msg: string) => {
      ctx.ui.notify(msg, "info");
      void refresh(ctx, gen);
    };
    switch (act) {
      case "Trả lời câu hỏi của team": {
        for (const a of asks) {
          const other = "Trả lời khác…";
          const choice = a.options?.length ? await ctx.ui.select(`${a.id}: ${a.question}`, [...a.options, other]) : other;
          if (choice === undefined) return;
          const answer = choice === other ? await ctx.ui.input(`${a.id}: ${a.question}`, "Câu trả lời của bạn") : choice;
          if (!answer?.trim()) return;
          await api("POST", `/api/board/${id}/asks/${a.id}`, { answer: answer.trim() });
        }
        return done(`✓ Đã trả lời ${asks.length} câu cho ${id}`);
      }
      case "Xem kế hoạch":
      case "Xem báo cáo":
      case "Xem lỗi": {
        const doc = (d.docs ?? []).find((x: any) => x.name === d.focus) ?? (d.docs ?? [])[0];
        const body = act === "Xem lỗi" ? `${t.note ?? ""}\n\n${leader?.text ?? ""}` : doc ? `# ${doc.name}\n\n${doc.content}` : leader?.text ?? "(chưa có tài liệu)";
        await ctx.ui.editor(`${id} · ${act.replace("Xem ", "")}`, body);
        return taskMenu(ctx, root, id);
      }
      case "Xong, đóng câu hỏi":
        await api("POST", `/api/board/${id}/approve`);
        return done(`✓ ${id} đã đóng`);
      case "Duyệt kế hoạch":
        if (await ctx.ui.confirm("Board", `Duyệt kế hoạch ${id} cho Dev bắt đầu code?`)) {
          await api("POST", `/api/board/${id}/approve`);
          return done(`✓ Đã duyệt kế hoạch ${id}, Dev sẽ bắt đầu`);
        }
        return;
      case "Yêu cầu sửa kế hoạch":
      case "Yêu cầu sửa":
      case "Hỏi tiếp":
      case "Trả lời Leader": {
        const msg = await ctx.ui.input(act, act === "Yêu cầu sửa kế hoạch" || act === "Yêu cầu sửa" ? "Cần sửa gì?" : leader?.text?.replace(/\s+/g, " ").slice(0, 120));
        if (!msg?.trim()) return;
        await api("POST", `/api/board/${id}/reply`, { text: msg.trim() });
        return done(`✓ Đã gửi cho Leader của ${id}`);
      }
      case "Bắt đầu":
      case "Tiếp tục":
        await api("POST", `/api/board/${id}/resume`, { kind: "note", text: "" });
        return done(`✓ ${id} đã vào hàng chạy`);
      case "Chạy lại":
        await api("POST", `/api/board/${id}/retry`);
        return done(`✓ ${id} sẽ chạy lại bước ${STAGE[t.stage] ?? t.stage}`);
      case "Nhắn cho bước đang chạy": {
        const msg = await ctx.ui.input(`Nhắn ${id}`, "Agent nhận ngay trong phiên đang chạy");
        if (!msg?.trim()) return;
        await api("POST", `/api/board/${id}/inbox`, { kind: "note", text: msg.trim() });
        return done(`✓ Đã nhắn ${id}`);
      }
      case "Tạm dừng":
        if (await ctx.ui.confirm("Board", `Tạm dừng ${id} ở điểm an toàn gần nhất?`)) {
          await api("POST", `/api/board/${id}/inbox`, { kind: "pause", text: "" });
          return done(`✓ ${id} sẽ dừng ở điểm an toàn`);
        }
        return;
      case "Nghiệm thu, tạo MR (mở web)":
      case "Mở trang task trên web":
        return openWeb(root, id);
    }
  }

  // ── /pms ──────────────────────────────────────────────────────────────────

  const untag = (s: string) => s.replace(/<\/?\s*pms_source/gi, "‹pms_source");
  const attr = (s: string) => s.replace(/[^\w#.-]/g, "");
  const KIND: Record<string, string> = { task: "Task", ticket: "Ticket" };
  // Same words as the web picker (web/src/PmsPicker.tsx STATUS_VI).
  const STATUS_VI: Record<string, string> = {
    Open: "Mới", Assigned: "Đã giao", "In Progress": "Đang làm", Testing: "Đang test", "Wait For Verifying": "Chờ xác minh", "Wait Close": "Chờ đóng",
    "Wait For Response": "Chờ phản hồi", "Follow Up": "Theo dõi", Pending: "Tạm hoãn", "Pending Golive": "Chờ golive", Reopen: "Mở lại",
    Completed: "Hoàn thành", Closed: "Đã đóng", Canceled: "Đã huỷ", Cancel: "Đã huỷ", Reject: "Từ chối",
  };
  /** Statuses with no work left: never "Quá hạn". */
  const FINISHED = new Set(["Completed", "Closed", "Canceled", "Cancel", "Reject", "Wait Close"]);
  const vi = (s: string) => STATUS_VI[s] ?? s;
  const viDate = (s: string) => {
    const m = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}))?/.exec(s);
    return m ? (m[4] ? `${m[3]}/${m[2]} ${m[4]}:${m[5]}` : `${m[3]}/${m[2]}/${m[1]}`) : s;
  };

  /** Same title/request/source as the web picker (web/src/PmsPicker.tsx pmsToTask). */
  function itemToTask(d: any) {
    const parts: string[] = [d.description || d.title];
    if (d.checklist?.length) parts.push(["Checklist:", ...d.checklist.map((c: any) => `- [${c.done ? "x" : " "}] ${c.name}`)].join("\n"));
    if (d.subtasks?.length) parts.push(["Việc con:", ...d.subtasks.map((s: any) => `- ${s.title} (${s.status ? vi(s.status) : "Chưa có trạng thái"})`)].join("\n"));
    const comments: string[] = [];
    let used = 0;
    for (const c of (d.comments ?? []).slice(0, 5)) {
      const left = 1500 - used;
      if (left < 40) break;
      let line = `- ${c.author || "Ẩn danh"} (${viDate(c.at)}): ${String(c.text).replace(/\n+/g, " ")}`;
      if (line.length > left) line = `${line.slice(0, left - 1)}…`;
      comments.push(line);
      used += line.length;
    }
    if (comments.length) parts.push(["Bình luận gần đây:", ...comments].join("\n"));
    const label = `${KIND[d.kind]}${d.no ? ` ${d.no}` : ""}`;
    return {
      title: d.no ? `[${d.no}] ${d.title}` : d.title,
      request: `<pms_source system="pms" kind="${d.kind}" no="${attr(d.no)}">\n${untag(parts.join("\n\n"))}\n</pms_source>\n\nNguồn: PMS ${label}, ${d.url}`,
      source: { system: "pms", kind: d.kind, id: d.id, no: d.no, url: d.url },
    };
  }

  /** Same as reportRowToTask in the web picker. */
  function rowToTask(r: any, row: any) {
    const lines = row.cells.map((c: any) => (c.value.includes("\n") ? `${c.label}:\n${c.value}` : `${c.label}: ${c.value}`));
    const url = row.url || r.url;
    return {
      title: row.title,
      request: `<pms_source system="pms" kind="record" module="${attr(r.module)}" report="${attr(r.id)}">\n${untag([`Report: ${r.name || r.id}`, ...lines].join("\n"))}\n</pms_source>\n\nNguồn: PMS report ${r.id}${r.name ? ` (${r.name})` : ""}, ${url}`,
      source: { system: "pms", kind: "record", id: row.id, no: "", url },
    };
  }

  type Pick = { title: string; request: string; source: any };
  type Tab = "ticket" | "task" | "report";
  type Picked = { action: "menu" | "board" | "cook"; picks: Pick[] } | null;
  const STATUS_FILTER: Record<string, string> = { open: "Đang mở", done: "Đã xong", all: "Mọi tình trạng" };
  const TAB_LABEL: Record<Tab, string> = { ticket: "Ticket", task: "Task", report: "Report" };
  const PRIORITY_VI: Record<string, string> = { urgent: "Khẩn", high: "Ưu tiên cao", normal: "Thường", medium: "Trung bình", low: "Thấp" };

  /** The web picker in the terminal: tabs + search + filters over a list | preview split. */
  type Tui = typeof import("@oh-my-pi/pi-tui");
  function pmsBrowser(T: Tui, tui: any, theme: any, done: (r: Picked) => void, canReport: boolean) {
    const { Input, matchesKey, truncateToWidth, visibleWidth, wrapTextWithAnsi } = T;
    const api = client(base, () => "");
    let tab: Tab = "ticket";
    let mine = true;
    let status: "open" | "done" | "all" = "open";
    const input = new Input();
    let typing = false;
    let entries: any[] = []; // search items, or report rows
    let report: any = null;
    let page = 0;
    let hasMore = false;
    let loading = false;
    let busy = "";
    let error = "";
    let sel = 0;
    let top = 0;
    let pvTop = 0;
    /** Below NARROW columns one pane shows at a time: the list, or the selected item's detail. */
    let view: "list" | "detail" = "list";
    let lastWidth = process.stdout.columns || 120;
    const marked = new Map<string, any>();
    const details = new Map<string, any>();
    let seq = 0;
    let pvTimer: ReturnType<typeof setTimeout> | undefined;
    const redraw = () => tui.requestRender();
    const c = (color: string, s: string) => theme.fg(color, s);

    /** The PMS kind a row's detail comes from (null = report row of another module). */
    const kindOf = (): "task" | "ticket" | null => (tab === "report" ? report?.kind ?? null : tab);
    const keyOf = (e: any) => `${kindOf() ?? "row"}:${e.id}`;

    async function load(reset: boolean) {
      const my = ++seq;
      if (reset) {
        page = tab === "report" ? 1 : 0;
        sel = 0;
        top = 0;
        entries = [];
      }
      error = "";
      loading = true;
      redraw();
      try {
        if (tab === "report") {
          const raw = input.getValue().trim();
          const rid = (/[?&]record=(\d{1,12})\b/.exec(raw) ?? /^#?(\d{1,12})$/.exec(raw))?.[1];
          if (!rid) {
            report = null;
            error = raw ? "Không đọc được ID report. Dán số (vd 1047) hoặc link report." : "";
            return;
          }
          const r = await api("GET", `/api/board/pms/report?id=${rid}&page=${page}`);
          if (my !== seq) return;
          report = r;
          entries = reset ? r.rows : [...entries, ...r.rows];
          hasMore = r.has_more;
          if (!entries.length) error = r.chart ? "Report này là biểu đồ, không có dòng để chọn." : "Report này chưa có dòng nào.";
        } else {
          const q = new URLSearchParams({ kind: tab, q: input.getValue().trim(), mine: mine ? "1" : "0", status, page: String(page) });
          const r = await api("GET", `/api/board/pms/search?${q}`);
          if (my !== seq) return;
          entries = reset ? r.items : [...entries, ...r.items];
          hasMore = r.has_more;
          if (!entries.length) error = `Không tìm thấy ${tab} nào. ${mine || status !== "all" ? "Thử tắt Của tôi (m) hoặc đổi tình trạng (s)." : "Thử từ khoá khác."}`;
        }
      } catch (e) {
        if (my === seq) error = (e as Error).message;
      } finally {
        if (my === seq) loading = false;
        preview();
        redraw();
      }
    }

    /** Fetch the selected item's detail a moment after the cursor stops. */
    function preview() {
      pvTop = 0;
      clearTimeout(pvTimer);
      const e = entries[sel];
      const k = kindOf();
      if (!e || !k || details.has(keyOf(e))) return;
      pvTimer = setTimeout(async () => {
        try {
          details.set(keyOf(e), await api("GET", `/api/board/pms/item?kind=${k}&id=${e.id}`));
        } catch (err) {
          details.set(keyOf(e), { error: (err as Error).message });
        }
        redraw();
      }, 220);
    }

    async function pickOf(e: any): Promise<Pick> {
      const k = kindOf();
      if (!k) return rowToTask(report, e);
      let d = details.get(keyOf(e));
      if (!d || d.error) d = await api("GET", `/api/board/pms/item?kind=${k}&id=${e.id}`);
      return itemToTask(d);
    }

    async function finish(action: "menu" | "board" | "cook") {
      const cur = entries[sel];
      const list = action === "board" && marked.size ? [...marked.values()] : cur ? [cur] : [];
      if (!list.length) return;
      busy = `Đang lấy chi tiết ${list.length} mục…`;
      redraw();
      try {
        const picks = await Promise.all(list.map(pickOf));
        clearTimeout(pvTimer);
        done({ action, picks });
      } catch (e) {
        busy = "";
        error = (e as Error).message;
        redraw();
      }
    }

    function setTab(t: Tab) {
      tab = t;
      marked.clear();
      input.setValue("");
      report = null;
      typing = t === "report";
      if (t === "report") {
        ++seq;
        entries = [];
        loading = false;
        error = "";
        redraw();
      } else void load(true);
    }

    const move = (d: number) => {
      if (!entries.length) return;
      sel = Math.max(0, Math.min(entries.length - 1, sel + d));
      if (sel === entries.length - 1 && hasMore && !loading) {
        page++;
        void load(false);
      }
      preview();
      redraw();
    };

    // ── Drawing ──
    // Width breakpoints: ≥ WIDE list | detail 45/55 with an assignee column (when "Của tôi" is off);
    // ≥ NARROW the same split without it; below NARROW one pane at a time.
    const WIDE = 120;
    const NARROW = 100;
    const pad = (s: string, w: number) => truncateToWidth(s, w, undefined, true);
    const rows = () => Math.max(8, (process.stdout.rows || 40) - 12);
    // PMS text may arrive decomposed (NFD); compose it so the width math and the terminal agree.
    const nfc = (s: unknown) => String(s ?? "").normalize("NFC");
    // Selected row: soft dark-blue background, white text. Callers pass plain text (no ANSI), already
    // fitted: truncateToWidth resets SGR before its ellipsis, which would drop the background.
    const selRow = (plain: string) => `\x1b[48;5;24m\x1b[97m${plain}\x1b[39m\x1b[49m`;
    const today = () => {
      const d = new Date();
      return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    };
    const overdue = (due: string, status: string) => !!due && !FINISHED.has(status) && due.slice(0, 10) < today();
    // Form placeholders such as "Kết quả mong muốn:\n." add nothing to a preview; the task text keeps them.
    const previewText = (s: string) => s.replace(/(^|\n)[^\n]{1,60}:[ \t]*\n[ \t]*\.[ \t]*(?=\n|$)/g, "$1").replace(/\n{3,}/g, "\n\n").trim();
    const hint = (k: string, label: string) => `${c("accent", k)} ${c("muted", label)}`;

    function leftLines(w: number, h: number, showOwner: boolean): string[] {
      if (loading && !entries.length) return [c("muted", " Đang tải…")];
      if (!entries.length) return wrapTextWithAnsi(c("muted", ` ${error || (tab === "report" ? "Dán ID hoặc link report rồi Enter." : "")}`), w);
      const fit = Math.max(1, h - (hasMore ? 1 : 0));
      if (sel < top) top = sel;
      if (sel >= top + fit) top = sel - fit + 1;
      const shown = entries.slice(top, top + fit);
      const body = w - 1; // after the 1-column selection bar
      // Fixed columns: status, then the ID, each as wide as its widest loaded value (not just the
      // visible rows, so columns stay put while scrolling); the title takes the rest.
      const ST = Math.min(12, Math.max(3, ...entries.map((e) => visibleWidth(vi(nfc(e.status))))));
      const idW = Math.min(9, Math.max(0, ...entries.map((e) => visibleWidth(nfc(e.no)))));
      // The mark column exists only while something is marked, so every row keeps the same columns.
      const MW = marked.size ? 2 : 0;
      // Assignee column: given name (last two words), and only while the title keeps ≥ 24 columns.
      const short = (s: unknown) => nfc(s).trim().split(/\s+/).slice(-2).join(" ");
      let OW = showOwner ? Math.min(12, Math.max(4, ...entries.map((e) => visibleWidth(short(e.assignee))))) : 0;
      if (OW && body - MW - ST - 1 - (idW ? idW + 1 : 0) - OW - 1 < 24) OW = 0;
      const out: string[] = [];
      shown.forEach((e, j) => {
        const on = top + j === sel;
        const mark = MW ? (marked.has(keyOf(e)) ? "☑ " : "  ") : "";
        const markS = mark.trim() ? c("success", mark) : mark;
        if (tab === "report") {
          const meta = (e.cells ?? []).filter((x: any) => x.value !== e.title).slice(0, 2).map((x: any) => nfc(x.value).replace(/\s+/g, " ")).join(" · ");
          const title = nfc(e.title);
          out.push(
            on
              ? `${c("accent", "▌")}${selRow(pad(`${mark}${title}${meta ? ` · ${meta}` : ""}`, body))}`
              : ` ${pad(`${markS}${title}${meta ? c("muted", ` · ${meta}`) : ""}`, body)}`,
          );
          return;
        }
        const st = pad(vi(nfc(e.status)), ST);
        const id = idW ? pad(nfc(e.no), idW) : "";
        const owner = OW ? pad(short(e.assignee), OW) : "";
        const titleW = Math.max(4, body - MW - ST - 1 - (idW ? idW + 1 : 0) - (OW ? OW + 1 : 0));
        const title = pad(nfc(e.title), titleW);
        if (on) out.push(`${c("accent", "▌")}${selRow([`${mark}${st}`, id, title, owner].filter(Boolean).join(" "))}`);
        else {
          const stS = c(overdue(nfc(e.due), e.status) ? "error" : "muted", st);
          out.push(` ${[`${markS}${stS}`, id ? c("accent", id) : "", title, owner ? c("muted", owner) : ""].filter(Boolean).join(" ")}`);
        }
      });
      if (hasMore) out.push(c("muted", "  ↓ xuống cuối để tải thêm"));
      return out;
    }

    function rightLines(w: number): string[] {
      const e = entries[sel];
      if (!e) return [c("muted", " Chọn một mục bên trái để xem chi tiết.")];
      const out: string[] = [];
      const add = (s = "") => out.push(...(s ? wrapTextWithAnsi(s, w - 1).map((l) => ` ${l}`) : [""]));
      const k = kindOf();
      if (!k) {
        add(c("accent", `${report?.module || "Report"} #${e.id}`));
        add(theme.bold(nfc(e.title)));
        add();
        for (const x of e.cells ?? []) add(`${c("muted", `${nfc(x.label)}:`)} ${nfc(x.value)}`);
        return out;
      }
      const d = details.get(keyOf(e));
      const status = d?.status ?? e.status ?? "";
      const due = nfc(d?.due ?? e.due);
      const prio = String(d?.priority ?? e.priority ?? "");
      add(c("accent", `${nfc(d?.no || e.no || `#${e.id}`)} · ${TAB_LABEL[k]}`));
      add(theme.bold(nfc(d?.title ?? e.title)));
      add();
      // Two fixed metadata rows: state and dates, then where/who. Each wraps on its own.
      const sep = c("muted", " · ");
      add(
        [
          status ? theme.bold(vi(nfc(status))) : "",
          prio ? (PRIORITY_VI[prio.toLowerCase()] ?? nfc(prio)) : "",
          overdue(due, status) ? theme.bold(c("error", "Quá hạn")) : "",
          due ? viDate(due) : "",
        ]
          .filter(Boolean)
          .join(sep),
      );
      add([nfc(d?.project ?? e.project), nfc(d?.assignee ?? e.assignee)].filter(Boolean).join(sep));
      add();
      if (!d) add(c("muted", "Đang tải mô tả…"));
      else if (d.error) add(c("error", d.error));
      else {
        add(theme.bold("Mô tả"));
        for (const p of (previewText(nfc(d.description)) || "PMS chưa có mô tả, tiêu đề sẽ được dùng làm yêu cầu.").split("\n")) add(p);
        if (d.checklist?.length) {
          add();
          add(theme.bold(`Checklist ${d.checklist.filter((x: any) => x.done).length}/${d.checklist.length}`));
          for (const x of d.checklist) add(`${x.done ? c("success", "✓") : "○"} ${nfc(x.name)}`);
        }
        if (d.subtasks?.length) {
          add();
          add(theme.bold(`Việc con (${d.subtasks.length})`));
          for (const x of d.subtasks) add(`- ${nfc(x.title)} ${c("muted", `(${vi(x.status)})`)}`);
        }
        if (d.comments?.length) {
          add();
          add(theme.bold(`Bình luận gần đây (${d.comments.length})`));
          for (const x of d.comments) add(`${theme.bold(nfc(x.author) || "Ẩn danh")} ${c("muted", viDate(x.at))}: ${nfc(x.text).replace(/\n+/g, " ")}`);
        }
      }
      return out;
    }

    function render(width: number): string[] {
      lastWidth = width;
      const inner = Math.max(40, width - 2);
      const h = rows();
      const split = width >= NARROW;
      if (split) view = "list";
      const b = (s: string) => c("borderMuted", s);
      const row = (s: string) => `${b("│")}${pad(s, inner)}${b("│")}`;
      const title = " Lấy việc từ PMS ";
      const lines: string[] = [b(`╭─${title}${"─".repeat(Math.max(0, inner - 1 - visibleWidth(title)))}╮`)];

      // Top bar: tabs and the active filters; "/" turns the right side into the search field.
      const tabs = (["ticket", "task", "report"] as Tab[])
        .filter((t) => t !== "report" || canReport)
        .map((t) => (t === tab ? theme.bold(c("accent", `[${TAB_LABEL[t]}]`)) : c("muted", ` ${TAB_LABEL[t]} `)))
        .join(" ");
      const filters = tab === "report" ? "" : c("muted", `${mine ? "Của tôi ✓" : "Mọi người"} · ${STATUS_FILTER[status]}`);
      const left = ` ${tabs}   ${filters}`;
      const label = tab === "report" ? "Report:" : "Tìm:";
      const box = Math.max(12, Math.min(40, inner - visibleWidth(left) - label.length - 4));
      input.focused = typing;
      const q = input.getValue();
      const field = typing
        ? `${c("accent", label)} ${input.render(box)[0] ?? ""}`
        : q
          ? `${c("muted", label)} ${truncateToWidth(nfc(q), box)}`
          : c("dim", tab === "report" ? "/ dán ID / link report" : "/ tìm");
      const gap = Math.max(2, inner - visibleWidth(left) - visibleWidth(field) - 1);
      lines.push(row(`${left}${" ".repeat(gap)}${field}`));

      if (split) {
        const L = Math.floor(inner * 0.45);
        const R = inner - L - 1;
        lines.push(b(`├${"─".repeat(L)}┬${"─".repeat(R)}┤`));
        const lv = leftLines(L, h, width >= WIDE && !mine && tab !== "report");
        const right = rightLines(R);
        pvTop = Math.max(0, Math.min(pvTop, Math.max(0, right.length - h)));
        const rv = right.slice(pvTop, pvTop + h);
        for (let i = 0; i < h; i++) lines.push(`${b("│")}${pad(lv[i] ?? "", L)}${b("│")}${pad(rv[i] ?? "", R)}${b("│")}`);
        lines.push(b(`├${"─".repeat(L)}┴${"─".repeat(R)}┤`));
      } else {
        lines.push(b(`├${"─".repeat(inner)}┤`));
        let body: string[];
        if (view === "detail") {
          const right = rightLines(inner);
          pvTop = Math.max(0, Math.min(pvTop, Math.max(0, right.length - h)));
          body = right.slice(pvTop, pvTop + h);
        } else body = leftLines(inner, h, false);
        for (let i = 0; i < h; i++) lines.push(row(body[i] ?? ""));
        lines.push(b(`├${"─".repeat(inner)}┤`));
      }

      // Bottom bar: the actions first, with what Enter does in this pane spelled out, then navigation.
      const note = busy || (error && entries.length ? c("error", error) : "") || (marked.size ? c("success", `Đã chọn ${marked.size} mục: g để giao hết cho Board`) : "");
      if (note) lines.push(row(` ${note}`));
      const S = c("muted", " · ");
      let keys: string[][];
      if (typing) keys = [[hint("Enter", tab === "report" ? "Mở report" : "Tìm"), hint("Esc", "Thôi gõ")]];
      else {
        const enter = !split && view === "list" ? hint("Enter", "Xem chi tiết") : hint("Enter", "Lấy việc (Board · /cook · chép)");
        const filt = tab === "report" ? [] : [hint("m", "Của tôi"), hint("s", "Trạng thái")];
        const nav =
          !split && view === "detail"
            ? [hint("↑↓", "Đổi mục"), hint("PgUp/PgDn", "Cuộn"), hint("Esc", "Về danh sách")]
            : [hint("↑↓", "Chọn"), hint("/", "Tìm"), ...filt, hint("Tab", "Đổi loại"), hint("Space", "Chọn nhiều"), ...(split ? [hint("PgUp/PgDn", "Cuộn chi tiết")] : []), hint("Esc", "Thoát")];
        keys = [[theme.bold(enter), hint("g", "Giao cho Board"), hint("c", "Làm với /cook"), hint("o", "Mở PMS")], nav];
      }
      // Wrap between hints, never inside one ("Esc" and "Thoát" stay together).
      for (const group of keys) {
        let cur = "";
        for (const k of group) {
          const next = cur ? `${cur}${S}${k}` : k;
          if (cur && visibleWidth(next) > inner - 2) {
            lines.push(row(` ${cur}`));
            cur = k;
          } else cur = next;
        }
        if (cur) lines.push(row(` ${cur}`));
      }
      lines.push(b(`╰${"─".repeat(inner)}╯`));
      return lines;
    }

    function handleInput(data: string) {
      if (busy) return;
      if (typing) {
        if (matchesKey(data, "escape")) typing = false;
        else if (matchesKey(data, "enter")) {
          typing = false;
          void load(true);
        } else input.handleInput(data);
        redraw();
        return;
      }
      const split = lastWidth >= NARROW;
      if (!split && view === "detail" && (matchesKey(data, "escape") || matchesKey(data, "left"))) {
        view = "list";
        return redraw();
      }
      if (matchesKey(data, "escape")) {
        clearTimeout(pvTimer);
        return done(null);
      }
      if (matchesKey(data, "up")) return move(-1);
      if (matchesKey(data, "down")) return move(1);
      if (matchesKey(data, "pageDown")) {
        pvTop += rows() - 2;
        return redraw();
      }
      if (matchesKey(data, "pageUp")) {
        pvTop = Math.max(0, pvTop - rows() + 2);
        return redraw();
      }
      const order: Tab[] = canReport ? ["ticket", "task", "report"] : ["ticket", "task"];
      if (matchesKey(data, "shift+tab")) return setTab(order[(order.indexOf(tab) + order.length - 1) % order.length]);
      if (matchesKey(data, "tab")) return setTab(order[(order.indexOf(tab) + 1) % order.length]);
      if (matchesKey(data, "enter")) {
        if (!split && view === "list") {
          if (!entries[sel]) return;
          view = "detail";
          pvTop = 0;
          return redraw();
        }
        return void finish("menu");
      }
      if (matchesKey(data, "space")) {
        const e = entries[sel];
        if (e) {
          if (marked.has(keyOf(e))) marked.delete(keyOf(e));
          else marked.set(keyOf(e), e);
        }
        return move(1);
      }
      switch (data) {
        case "/":
          view = "list";
          typing = true;
          return redraw();
        case "m":
          if (tab === "report") return;
          mine = !mine;
          return void load(true);
        case "s":
          if (tab === "report") return;
          status = status === "open" ? "done" : status === "done" ? "all" : "open";
          return void load(true);
        case "g":
          return void finish("board");
        case "c":
          return void finish("cook");
        case "o": {
          const url = entries[sel]?.url || report?.url;
          if (url) void pi.exec(process.platform === "darwin" ? "open" : "xdg-open", [url]).catch(() => undefined);
          return;
        }
      }
    }

    void load(true);
    return { render, handleInput, invalidate: () => input.invalidate() };
  }

  async function createOnBoard(ctx: ExtensionContext, picks: Pick[]) {
    if (!here) return ctx.ui.notify("Thư mục này chưa là dự án trên Board. Mở omp trong thư mục dự án, hoặc tạo trên web.", "warning");
    const wf = await call("GET", "/api/board/workflows").catch(() => ({ commands: [] }));
    const cook = (wf.commands ?? []).some((c: any) => c.name === "cook" && c.enabled);
    const ids: string[] = [];
    for (const p of picks) ids.push((await call("POST", "/api/board", { title: p.title, request: p.request, workflow: cook ? "cook" : undefined, source: p.source })).id);
    ctx.ui.notify(`✓ Đã tạo ${ids.join(", ")} · ${cook ? "/cook" : "/feature"} · bấm Bắt đầu trong /board hoặc trên web`, "info");
    void refresh(ctx, gen);
  }

  async function pmsMenu(ctx: ExtensionContext) {
    await refresh(ctx, gen);
    if (!up) {
      ctx.ui.notify(`Board chưa chạy ở ${base}: /pms dùng tài khoản PMS đã kết nối trên Board.`, "warning");
      return;
    }
    const st = await client(base, () => "")("GET", "/api/board/pms/status");
    if (!st.configured) {
      ctx.ui.notify("Chưa kết nối PMS. Trên web: Board → nút PMS → Kết nối.", "warning");
      return;
    }
    // Loaded here, not at the top: if an omp release moves these exports, only the
    // two-pane view breaks, never the footer, /board or the coordinator's tools.
    let T: Tui;
    try {
      T = await import("@oh-my-pi/pi-tui");
    } catch {
      ctx.ui.notify("Bản omp này chưa mở được giao diện /pms. Dùng nút Tạo task trên web.", "warning");
      return;
    }
    const res = await ctx.ui.custom<Picked>((tui, theme, _kb, done) => pmsBrowser(T, tui, theme, done, !!st.report));
    if (!res?.picks.length) return;
    const p = res.picks[0];
    let action: string = res.action;
    if (action === "menu") {
      const how = await ctx.ui.select(p.title, ["Giao cho Board (chạy nền, duyệt sau)", "Làm ở đây với /cook (điền sẵn, bạn nhấn Enter)", "Chép vào ô nhập"]);
      if (!how) return;
      action = how.startsWith("Giao") ? "board" : how.startsWith("Làm") ? "cook" : "copy";
    }
    if (action === "board") await createOnBoard(ctx, res.picks);
    else if (action === "cook") ctx.ui.setEditorText(`/cook ${p.title}\n\n${p.request}`);
    else ctx.ui.setEditorText(`${p.title}\n\n${p.request}`);
  }

  pi.registerCommand("board", {
    description: "Board: việc đang chờ bạn, duyệt / trả lời / chạy lại ngay trong omp",
    handler: async (_a, ctx) => boardMenu(ctx).catch((e: Error) => ctx.ui.notify(`Board: ${e.message}`, "error")),
  });
  pi.registerCommand("pms", {
    description: "Lấy ticket / task / report từ PMS: giao cho Board hoặc làm ngay ở đây",
    handler: async (_a, ctx) => pmsMenu(ctx).catch((e: Error) => ctx.ui.notify(`PMS: ${e.message}`, "error")),
  });
}
