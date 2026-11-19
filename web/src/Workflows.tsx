// The boss's own omp slash commands (/cook, …) as a task's workflow instead of
// the /feature pipeline. Contract: GET/POST /api/board/workflows?root=,
// POST /api/board (workflow). A workflow task always runs: its plan → the boss
// approves → its run (same omp session) → the boss accepts.
import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardCommand, type BoardStage, type BoardTaskSummary } from "./api";
import { serverMessage } from "./Steps";

/** `cook` for a workflow task (summary or full task), null for /feature. */
export function workflowName(t: Pick<BoardTaskSummary, "workflow">): string | null {
  const w = t.workflow;
  if (!w) return null;
  return typeof w === "string" ? w : w.name;
}

const STEPS: BoardStage[] = ["solution", "approval", "dev", "acceptance"];

/** Tạo task starts on this command when the project has it enabled (else /feature). */
export const DEFAULT_WORKFLOW = "cook";

/** The step names a workflow task shows instead of the /feature labels. */
export function workflowStepLabel(name: string, stage: BoardStage): string | null {
  switch (stage) {
    case "solution": return `Lên kế hoạch (/${name})`;
    case "approval": return "Bạn duyệt";
    case "dev": return `Thực hiện (/${name})`;
    case "acceptance": return "Bạn nghiệm thu";
    default: return null;
  }
}

/** A workflow task's name for its current stage; null → use the /feature label. */
export function taskStageLabel(t: Pick<BoardTaskSummary, "workflow" | "stage">): string | null {
  const name = workflowName(t);
  return name ? workflowStepLabel(name, t.stage) : null;
}

export function WorkflowChip({ t }: { t: Pick<BoardTaskSummary, "workflow"> }) {
  const name = workflowName(t);
  return name ? <span className="tag bd-wf-chip" title={`Chạy theo lệnh /${name}`}>/{name}</span> : null;
}

function useCommands(root: string) {
  return useQuery({ queryKey: ["board-workflows", root], queryFn: () => api.boardWorkflows(root), enabled: root !== "" });
}

// ── Create modal: "Quy trình" row + per-project manager ───────────────────
export function WorkflowPicker({ root, value, onChange }: { root: string; value: string | null; onChange: (w: string | null) => void }) {
  const qc = useQueryClient();
  const q = useCommands(root);
  const [manage, setManage] = useState(false);
  const commands = q.data?.commands ?? [];
  // The default command first, the rest in name order.
  const enabled = commands.filter((c) => c.enabled).sort((a, b) => Number(b.name === DEFAULT_WORKFLOW) - Number(a.name === DEFAULT_WORKFLOW));
  const toggle = useMutation({
    mutationFn: (c: BoardCommand) => api.boardWorkflowToggle(root, c.name, !c.enabled),
    onSuccess: (d, c) => {
      qc.setQueryData(["board-workflows", root], d);
      if (c.enabled && value === c.name) onChange(null);
    },
  });
  // A choice the project no longer offers (switched project, turned off) falls back to /feature.
  const offered = !value || !q.data || enabled.some((c) => c.name === value);
  useEffect(() => {
    if (!offered) onChange(null);
  }, [offered, onChange]);
  return (
    <div className="field bd-wf">
      <span className="bd-wf-head">
        <span className="field-label" id="bd-wf-label">Quy trình</span>
        <button type="button" className="link bd-wf-manage" aria-expanded={manage} onClick={() => setManage(!manage)} disabled={!root}>
          {manage ? "Xong" : "Quản lý"}
        </button>
      </span>
      <div className="bd-presets" role="group" aria-labelledby="bd-wf-label">
        {enabled.map((c) => (
          <button
            type="button"
            key={c.name}
            className={`bd-preset ${value === c.name ? "on" : ""}`}
            aria-pressed={value === c.name}
            title={c.description || c.path}
            onClick={() => onChange(c.name)}
          >
            /{c.name}{c.name === DEFAULT_WORKFLOW ? " (mặc định)" : ""}
          </button>
        ))}
        <button type="button" className={`bd-preset ${value === null ? "on" : ""}`} aria-pressed={value === null} onClick={() => onChange(null)}>
          /feature
        </button>
      </div>
      {manage ? (
        <div className="bd-wf-panel">
          <p className="bd-steps-preview">Lệnh omp của bạn (trong ~/.omp/agent/commands và .omp/commands của dự án). Bật lệnh nào thì lệnh đó hiện khi tạo task cho dự án này.</p>
          {q.error ? <p className="hint hint-err">{serverMessage(q.error as Error)}</p> : null}
          {q.data && !commands.length ? <p className="bd-steps-preview">Chưa có lệnh nào. Tạo file .md trong ~/.omp/agent/commands để có quy trình riêng.</p> : null}
          {commands.length ? (
            <ul className="bd-steplist" aria-label="Lệnh có thể dùng làm quy trình">
              {commands.map((c) => (
                <li key={c.name}>
                  <label className={`bd-step ${c.enabled ? "" : "is-off"}`}>
                    <span className="bd-step-main">
                      <span className="bd-step-name mono">/{c.name}</span>
                      {c.board ? <span className="bd-step-who">board: true trong file, luôn bật</span> : null}
                      {c.description ? <span className="bd-step-note bd-wf-desc">{c.description}</span> : null}
                      <span className="bd-step-note bd-wf-path mono" title={c.path}>{c.path}</span>
                    </span>
                    <input
                      type="checkbox"
                      checked={c.enabled}
                      disabled={c.board || toggle.isPending}
                      onChange={() => toggle.mutate(c)}
                      aria-label={`${c.enabled ? "Tắt" : "Bật"} /${c.name} cho dự án này`}
                    />
                  </label>
                </li>
              ))}
            </ul>
          ) : null}
          {toggle.error ? <p className="hint hint-err">{serverMessage(toggle.error as Error)}</p> : null}
        </div>
      ) : null}
    </div>
  );
}

/** The fixed run order of a workflow task (replaces the step picker). */
export function WorkflowSteps({ name }: { name: string }) {
  return (
    <div className="field bd-steps">
      <span className="field-label">Các bước</span>
      <p className="bd-steps-preview" aria-label="Thứ tự chạy">{STEPS.map((s) => workflowStepLabel(name, s)).join(" → ")}</p>
      <p className="bd-steps-preview faint">Bước kế hoạch chỉ được viết kế hoạch, không sửa code. Bước thực hiện chạy tiếp đúng phiên đó sau khi bạn duyệt.</p>
    </div>
  );
}

// ── Task page: the fixed pipeline row ─────────────────────────────────────
export function WorkflowPipeline({ name, stage, status }: { name: string; stage: BoardStage; status: string }) {
  const terminal = stage === "done" || stage === "cancelled" || status === "done" || status === "cancelled";
  const at = terminal ? STEPS.length : STEPS.indexOf(stage);
  return (
    <div className="bd-pipe-wrap">
      <ol className="bd-pipe" aria-label="Các bước của việc">
        {STEPS.map((s, i) => {
          const state = i < at ? "done" : i === at ? "current" : "upcoming";
          return (
            <li key={s} className={`bd-pipe-step is-${state}`}>
              <span className="bd-pipe-chip" title={state === "done" ? "đã chạy" : state === "current" ? "đang ở bước này" : undefined}>
                {state === "done" ? <span aria-hidden="true">✓</span> : null}
                {workflowStepLabel(name, s)}
              </span>
            </li>
          );
        })}
      </ol>
      <p className="bd-pipe-msg faint">Các bước của quy trình /{name} cố định.</p>
    </div>
  );
}
