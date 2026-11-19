// Per-task steps: the fixed step catalog, a picker for the create modal and
// the pipeline row under "Các bước" on the task page. Contract: GET /api/board/steps,
// POST /api/board (steps), POST /api/board/:id/steps.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardStage, type BoardStepDef, type BoardStepPreset, type BoardTask } from "./api";

// Fallback when the catalog endpoint is missing; the server copy wins.
const CATALOG: BoardStepDef[] = [
  { key: "investigate", label: "Điều tra / Hỏi đáp", who: "Leader", gate: true, requires: [] },
  { key: "intake", label: "Tiếp nhận (new)", who: "Leader", gate: false, requires: [] },
  { key: "solution", label: "Kế hoạch phase (plan)", who: "Leader + BA", gate: false, requires: [] },
  { key: "review", label: "Phản biện kế hoạch", who: "Reviewer", gate: false, requires: ["solution"] },
  { key: "approval", label: "Bạn duyệt kế hoạch", who: "Bạn", gate: true, requires: ["solution"] },
  { key: "dev", label: "Phát triển (go)", who: "Dev", gate: false, requires: [] },
  { key: "test", label: "Kiểm thử (ship)", who: "Tester", gate: false, requires: ["dev"] },
  { key: "security", label: "Review bảo mật", who: "Security reviewer", gate: false, requires: ["dev"] },
  { key: "report", label: "Báo cáo", who: "Leader", gate: false, requires: [] },
  { key: "acceptance", label: "Bạn nghiệm thu (tạo MR)", who: "Bạn", gate: true, requires: [] },
];

export const FULL_STEPS: BoardStage[] = ["intake", "solution", "review", "approval", "dev", "test", "report", "acceptance"];

const PRESETS: BoardStepPreset[] = [
  { name: "Feature đầy đủ", steps: FULL_STEPS },
  { name: "Sửa nhanh", steps: ["dev", "test", "report", "acceptance"] },
  { name: "Chỉ kế hoạch", steps: ["intake", "solution", "review", "approval"] },
  { name: "Điều tra", steps: ["investigate"] },
  { name: "Điều tra rồi sửa", steps: ["investigate", "dev", "test", "report", "acceptance"] },
];

// Short names for previews, chips and validation messages.
const SHORT: Partial<Record<BoardStage, string>> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận",
  solution: "Kế hoạch",
  review: "Phản biện",
  approval: "Bạn duyệt",
  dev: "Phát triển",
  test: "Kiểm thử",
  security: "Bảo mật",
  report: "Báo cáo",
  acceptance: "Nghiệm thu",
};
const short = (s: BoardStage) => SHORT[s] ?? s;

export const NO_ACCEPTANCE_NOTE = "Không nghiệm thu: nhánh để nguyên trên máy, không tạo MR.";

export function useStepCatalog(): { catalog: BoardStepDef[]; presets: BoardStepPreset[] } {
  const q = useQuery({ queryKey: ["board-steps"], queryFn: api.boardStepCatalog, staleTime: Infinity, retry: false });
  return {
    catalog: q.data?.catalog?.length ? q.data.catalog : CATALOG,
    presets: q.data?.presets?.length ? q.data.presets : PRESETS,
  };
}

/** Task steps in catalog order; old tasks without `steps` run the full feature flow. */
export function stepsOf(t: Pick<BoardTask, "steps">): BoardStage[] {
  return t.steps ?? FULL_STEPS;
}

/** Any enabled step after `stage`? (The investigate gate: continue vs close.) */
export function hasStepAfter(t: Pick<BoardTask, "steps">, stage: BoardStage): boolean {
  const order = CATALOG.map((c) => c.key);
  const at = order.indexOf(stage);
  return stepsOf(t).some((s) => order.indexOf(s) > at);
}

/** Vietnamese error for an invalid set, null when valid. */
export function validateSteps(steps: BoardStage[], catalog: BoardStepDef[]): string | null {
  if (steps.length === 0) return "Chọn ít nhất một bước.";
  const on = new Set(steps);
  for (const c of catalog) {
    if (!on.has(c.key)) continue;
    const missing = c.requires.filter((r) => !on.has(r));
    if (missing.length) return `${short(c.key)} cần bật ${missing.map(short).join(", ")}`;
  }
  return null;
}

const sameSet = (a: BoardStage[], b: BoardStage[]) => a.length === b.length && a.every((s) => b.includes(s));

/** "400 {\"error\":\"…\"}" → "…" */
export function serverMessage(e: Error): string {
  const body = e.message.replace(/^\d{3}\s+/, "");
  try {
    const j = JSON.parse(body) as { error?: string; message?: string };
    return j.error ?? j.message ?? body;
  } catch {
    return body;
  }
}

// Steps the backend repeats per phase: after each phase's test it loops back to
// the plan. The rest run once, before or after the loop.
const LOOP = new Set<BoardStage>(["solution", "review", "approval", "dev", "test"]);
const ROW_NOTE: Partial<Record<BoardStage, string>> = { security: "chạy 1 lần sau phase cuối" };

// ── Create modal: presets + numbered step list + preview ───────────────────
export function StepPicker({ value, onChange }: { value: BoardStage[]; onChange: (s: BoardStage[]) => void }) {
  const { catalog, presets } = useStepCatalog();
  const toggle = (k: BoardStage) => {
    const on = new Set(value);
    if (on.has(k)) on.delete(k);
    else on.add(k);
    onChange(catalog.map((c) => c.key).filter((s) => on.has(s)));
  };
  const error = validateSteps(value, catalog);
  // Run order = catalog order; numbers stay put when a step is off.
  const rows = catalog.map((c, i) => ({ c, n: i + 1 }));
  const firstLoop = rows.findIndex((r) => LOOP.has(r.c.key));
  const pre = rows.filter((r, i) => !LOOP.has(r.c.key) && (firstLoop < 0 || i < firstLoop));
  const loop = rows.filter((r) => LOOP.has(r.c.key));
  const post = rows.filter((r, i) => !LOOP.has(r.c.key) && firstLoop >= 0 && i > firstLoop);
  const row = ({ c, n }: { c: BoardStepDef; n: number }) => {
    const on = value.includes(c.key);
    const note = ROW_NOTE[c.key];
    return (
      <li key={c.key}>
        <label className={`bd-step ${on ? "" : "is-off"}`}>
          <span className="bd-step-num" aria-hidden="true">{n}</span>
          <span className="bd-step-main">
            <span className="bd-step-name">{c.label}</span>
            <span className="bd-step-who">{c.who}</span>
            {note ? <span className="bd-step-note">{note}</span> : null}
          </span>
          {c.gate ? <span className="tag warn bd-step-gate" title="Dừng chờ bạn">Bạn</span> : null}
          <input type="checkbox" checked={on} onChange={() => toggle(c.key)} />
        </label>
      </li>
    );
  };
  return (
    <div className="field bd-steps">
      <span className="field-label" id="bd-steps-label">Các bước</span>
      <div className="bd-presets" role="group" aria-label="Mẫu quy trình">
        {presets.map((p) => {
          const on = sameSet(p.steps, value);
          return (
            <button type="button" key={p.name} className={`bd-preset ${on ? "on" : ""}`} aria-pressed={on} onClick={() => onChange(p.steps)}>
              {p.name}
            </button>
          );
        })}
      </div>
      <ol className="bd-steplist" aria-labelledby="bd-steps-label">
        {pre.map(row)}
        {loop.length ? (
          <li className="bd-loop">
            <span className="bd-loop-title" id="bd-loop-title">
              <span aria-hidden="true">↻</span>
              {value.includes("solution") ? "Lặp lại cho mỗi phase" : "Không chia phase (làm thẳng từ yêu cầu)"}
            </span>
            <ol className="bd-loop-list" aria-labelledby="bd-loop-title">{loop.map(row)}</ol>
          </li>
        ) : null}
        {post.map(row)}
      </ol>
      {error ? (
        <p className="hint hint-err bd-steps-msg" role="alert">{error}</p>
      ) : (
        <p className="bd-steps-preview" aria-label="Thứ tự chạy">{value.map(short).join(" → ")}</p>
      )}
      {!error && !value.includes("acceptance") ? <p className="bd-steps-note">{NO_ACCEPTANCE_NOTE}</p> : null}
    </div>
  );
}

// ── Drawer: pipeline row; upcoming steps toggle ────────────────────────────
type ChipState = "done" | "current" | "upcoming" | "off";

export function Pipeline({ task: t, root }: { task: BoardTask; root: string }) {
  const qc = useQueryClient();
  const { catalog } = useStepCatalog();
  const steps = stepsOf(t);
  const order = catalog.map((c) => c.key);
  const terminal = t.stage === "done" || t.stage === "cancelled" || t.status === "done" || t.status === "cancelled";
  const at = terminal ? order.length : order.indexOf(t.stage);
  const save = useMutation({
    mutationFn: (next: BoardStage[]) => api.boardSteps(root, t.id, next),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
    },
  });
  const toggle = (k: BoardStage) => {
    const on = new Set(steps);
    if (on.has(k)) {
      const label = catalog.find((c) => c.key === k)?.label ?? k;
      if (!window.confirm(`Bỏ bước "${label}" khỏi việc ${t.id}?`)) return;
      on.delete(k);
    }
    else on.add(k);
    save.mutate(order.filter((s) => on.has(s)));
  };
  const state = (k: BoardStage, i: number): ChipState => {
    if (!steps.includes(k)) return "off";
    if (i < at) return "done";
    if (i === at) return "current";
    return "upcoming";
  };
  return (
    <div className="bd-pipe-wrap">
      <ol className="bd-pipe" aria-label="Các bước của việc">
        {catalog.map((c, i) => {
          const s = state(c.key, i);
          const editable = !terminal && i > at;
          const body = (
            <>
              {s === "done" ? <span aria-hidden="true">✓</span> : null}
              {short(c.key)}
            </>
          );
          return (
            <li key={c.key} className={`bd-pipe-step is-${s}`}>
              {editable ? (
                <button
                  type="button"
                  className="bd-pipe-chip"
                  aria-pressed={s === "upcoming"}
                  disabled={save.isPending}
                  onClick={() => toggle(c.key)}
                  title={`${c.label}: bấm để ${s === "off" ? "bật" : "bỏ"} bước này`}
                >
                  {body}
                </button>
              ) : (
                <span className="bd-pipe-chip" title={`${c.label}${s === "done" ? ": đã chạy" : s === "current" ? ": đang ở bước này" : s === "off" ? ": bỏ qua" : ""}`}>
                  {body}
                </span>
              )}
            </li>
          );
        })}
      </ol>
      {save.error ? <p className="hint hint-err bd-pipe-msg" role="alert">{serverMessage(save.error as Error)}</p> : null}
      {!terminal && !steps.includes("acceptance") ? <p className="bd-pipe-msg faint">{NO_ACCEPTANCE_NOTE}</p> : null}
    </div>
  );
}
