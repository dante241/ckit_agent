// Shared vocabulary of the task page: run names, verdicts, times, thread kinds.
import type { BoardMsg, BoardMsgKind, BoardRun, BoardStage } from "./api";
import { STAGE_LABEL, verdictTone } from "./Board";

// What a run did, in the boss's words (no "(go)" / "(ship)").
const STEP_NAME: Partial<Record<BoardStage, string>> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận",
  solution: "Lập kế hoạch",
  review: "Phản biện kế hoạch",
  dev: "Code",
  test: "Kiểm thử + đóng phase",
  security: "Review bảo mật",
  report: "Báo cáo",
  action: "Thực hiện",
};
export const stepName = (s: BoardStage) => STEP_NAME[s] ?? STAGE_LABEL[s];
/** What the run did: "Thực hiện: Tạo merge request" for an action the boss confirmed. */
export const runStep = (r: BoardRun) => (r.stage === "action" && r.title ? `Thực hiện: ${r.title}` : stepName(r.stage));
export const runTitle = (r: BoardRun) => (r.stage === "action" ? runStep(r) : `${stepName(r.stage)} #${r.n}`);

export const runLive = (r: BoardRun) => r.ended === null;
/** Reviewer / Tester said no, or the step exited with an error. */
export function runFailed(r: BoardRun): boolean {
  if (r.verdict && verdictTone(r.verdict) === "err") return true;
  return r.ended !== null && r.exit !== null && r.exit !== 0;
}
export function verdictText(v: string): string {
  const t = verdictTone(v);
  if (t) return t === "ok" ? "Đạt" : "Không đạt";
  if (/interrupt|paused/i.test(v)) return "Bị ngắt";
  return v;
}

/** Seconds between two instants (`b` null = now). */
export function secsBetween(a: string | null | undefined, b: string | null | undefined): number {
  const x = a ? Date.parse(a) : NaN;
  const y = b ? Date.parse(b) : Date.now();
  return Number.isNaN(x) || Number.isNaN(y) ? 0 : Math.max(0, Math.round((y - x) / 1000));
}
/** "45 giây", "3 phút", "1 giờ 28 phút", "2 ngày 3 giờ". */
export function secsText(s: number): string {
  if (s < 60) return `${s} giây`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} phút`;
  const h = Math.floor(m / 60);
  if (h < 48) return m % 60 ? `${h} giờ ${m % 60} phút` : `${h} giờ`;
  return `${Math.floor(h / 24)} ngày${h % 24 ? ` ${h % 24} giờ` : ""}`;
}
export const runSecs = (r: BoardRun) => secsBetween(r.started, r.ended);

/** "23:41". */
export function hhmm(iso: string | null | undefined): string {
  const d = iso ? new Date(iso) : null;
  if (!d || Number.isNaN(d.getTime())) return "";
  return d.toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit" });
}

/** What the running step is doing, as a phrase starting with "đang" ("đang edit: …",
 *  "đang chờ subagent: …", "đang chạy"). Log/run activity carries a "Leader · Kế hoạch M0 ·
 *  3 phút · " prefix the live line already shows, so it is dropped; PM text gets "đang: ". */
export function nowActivity(now: { activity: string | null; source?: string | null } | null | undefined): string | null {
  let a = now?.activity?.trim();
  if (!a) return null;
  if (now?.source === "log" || now?.source === "run") {
    const k = a.lastIndexOf(" · đang");
    if (k >= 0) a = a.slice(k + 3);
    a = a.replace(/^đang: /, "đang ");
  }
  return /^đang\s/i.test(a) || a === "đang" ? a : `đang: ${a}`;
}

/** Thread kind; servers before `kind` existed get the same split from `from`. */
export function msgKind(m: BoardMsg): BoardMsgKind {
  if (m.kind) return m.kind;
  const f = m.from;
  if (f === "system") return "system";
  if (f === "user" || f === "boss" || f.startsWith("user → ")) return "boss";
  if (f === "coordinator" && /duyệt .*thay/i.test(m.text)) return "delegated_approval";
  if (f === "coordinator" || f.startsWith("coordinator → ")) return "coordinator";
  if (f.includes(" → ")) return "question";
  if (f === "po" || f === "ba") return "answer";
  return "leader";
}
