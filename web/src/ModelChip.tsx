// Which model a run is on: `claude-opus-5-5 · medium`, provider in the tooltip.
// Data: `run_model` on task summaries, `runs[i].model` in the task detail.
import type { RunModel } from "./api";

const join = (model: string, effort: string | null) => (effort ? `${model} · ${effort}` : model);
/** Card-width name: `claude-opus-5-5` → `opus-5-5` (full name in the tooltip). */
const short = (model: string) => model.replace(/^claude-/, "");

/** "phụ: 2× claude-opus-5-5 · medium, 1× claude-haiku-4-5 · low", or "" without subagents. */
export function subagentText(m: RunModel | null | undefined): string {
  const s = m?.subagents ?? [];
  return s.length ? `phụ: ${s.map((x) => `${x.count}× ${join(x.model, x.effort)}`).join(", ")}` : "";
}

export function ModelChip({ m, className = "" }: { m: RunModel | null | undefined; className?: string }) {
  if (!m) return null;
  const tip = [`${m.provider || "?"} · ${m.model}`, m.effort && `effort: ${m.effort}`, m.credential_id != null && `tài khoản #${m.credential_id}`, subagentText(m)]
    .filter(Boolean)
    .join("\n");
  return (
    <span className={`model-chip ${className}`.trim()} title={tip}>
      {join(short(m.model), m.effort)}
    </span>
  );
}
