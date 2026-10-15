// ckit Board — coordinator tools. Loaded ONLY into the Board coordinator's omp
// session (`omp --no-extensions -e <this file>`), never into normal sessions.
// Every tool is a thin call to the dashboard's own HTTP API, so the coordinator
// has exactly the boss's dashboard powers minus merge (the server refuses it).
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  const { z } = pi.zod;
  const base = process.env.CKIT_BOARD_API;
  const root = process.env.CKIT_BOARD_ROOT ?? "";
  if (!base) return;
  pi.setLabel("ckit board coordinator tools");

  const text = (s: string) => ({ content: [{ type: "text" as const, text: s }] });
  const q = `root=${encodeURIComponent(root)}`;
  async function call(method: "GET" | "POST", path: string, body?: unknown): Promise<any> {
    const sep = path.includes("?") ? "&" : "?";
    const r = await fetch(`${base}${path}${sep}${q}`, {
      method,
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const raw = await r.text();
    if (!r.ok) throw new Error(`${r.status}: ${raw}`);
    return raw ? JSON.parse(raw) : null;
  }
  const safe = (f: () => Promise<string>) =>
    f().then(text, (e: Error) => text(`ERROR ${e.message}`));
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
          `stage=${t.stage} status=${t.status} paused=${!!t.paused} steps=${(t.steps ?? []).join("→")} branch=${t.branch ?? "-"} cost=$${t.cost.toFixed(2)}${d.phase ? ` phase=${d.phase}` : ""}${d.progress ? ` ${d.progress.done}/${d.progress.total}` : ""}`,
          `request: ${t.request}`,
          asks.length ? `open questions:\n${asks.map((a: any) => `- ${a.id} (${a.from}→${a.to}, ${a.status}): ${a.question}`).join("\n")}` : "open questions: none",
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
    execute: (_i, p) =>
      safe(async () => {
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
    execute: (_i, p) => safe(async () => (await call("POST", `/api/board/${p.id}/inbox`, { kind: p.kind, text: p.text, from: "coordinator" }), `sent ${p.kind} to ${p.id}`)),
  });

  pi.registerTool({
    name: "board_resume",
    label: "Board: resume paused step",
    description: "Continue a paused step in its own session, optionally with an instruction.",
    parameters: z.object({ id, text: z.string().default("") }),
    execute: (_i, p) => safe(async () => (await call("POST", `/api/board/${p.id}/resume`, { kind: "note", text: p.text, from: "coordinator" }), `resumed ${p.id}`)),
  });

  pi.registerTool({
    name: "board_answer",
    label: "Board: answer a team question",
    description: "Answer a teammate's open question (from board_task asks) as the PO. Decisive, concrete, ≤10 lines.",
    parameters: z.object({ id, qid: z.string(), answer: z.string().min(1) }),
    execute: (_i, p) => safe(async () => (await call("POST", `/api/board/${p.id}/asks/${p.qid}`, { answer: p.answer, by: "po" }), `answered ${p.id} ${p.qid}`)),
  });

  pi.registerTool({
    name: "board_escalate",
    label: "Board: escalate a question to the boss",
    description: "Hand an open question to the boss (scope, money, data, security, business rules). The asking agent keeps waiting.",
    parameters: z.object({ id, qid: z.string(), reason: z.string().min(3) }),
    execute: (_i, p) => safe(async () => (await call("POST", `/api/board/${p.id}/asks/${p.qid}/escalate`, { reason: p.reason }), `escalated ${p.id} ${p.qid}`)),
  });

  pi.registerTool({
    name: "board_approve",
    label: "Board: approve a plan for the boss",
    description: "Approve a task waiting at the plan-approval gate, per the delegation policy. Cannot accept/merge (the server refuses).",
    parameters: z.object({ id }),
    execute: (_i, p) => safe(async () => (await call("POST", `/api/board/${p.id}/approve?by=coordinator`), `approved plan of ${p.id}`)),
  });
}
