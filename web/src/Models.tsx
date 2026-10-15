import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type ModelBoardRow, type ModelConfig, type ModelRole, type ModelTask } from "./api";
import { Page, Loading, isMissing, ErrorState, EndpointMissing, EmptyState } from "./ui";

// Models page: leads with what each Board step actually runs (resolved through
// omp `modelRoles`), one control per task class; the raw roles/tasks tables sit
// under "Nâng cao".

const CLASS_LABEL: Record<ModelTask, string> = {
  code: "Phát triển (code)",
  plan: "Lên kế hoạch, điều phối (plan)",
  review: "Review, kiểm thử (review)",
  debug: "Gỡ lỗi (debug)",
  trivial: "Việc vặt (trivial)",
};
// Code first: "which model writes the code" is the question this page answers.
const CLASS_ORDER: ModelTask[] = ["code", "plan", "review", "debug", "trivial"];

const ROLE_HINTS: Record<string, string> = {
  default: "dự phòng", plan: "lên kế hoạch", smol: "việc nhỏ", slow: "suy nghĩ sâu", vision: "ảnh",
};
const TASK_HINTS: Record<string, string> = {
  plan: "Board: kế hoạch, điều phối", review: "Board: review, test", debug: "8sync ai", code: "Board: phát triển", trivial: "8sync ai",
};

/** `anthropic/claude-opus-5-5:medium` → name `claude-opus-5-5`, effort `medium`, provider `anthropic`. */
function splitModel(id: string): { name: string; effort?: string; provider?: string } {
  const slash = id.indexOf("/");
  const provider = slash > 0 ? id.slice(0, slash) : undefined;
  const rest = slash > 0 ? id.slice(slash + 1) : id;
  const colon = rest.lastIndexOf(":");
  return colon > 0 ? { name: rest.slice(0, colon), effort: rest.slice(colon + 1), provider } : { name: rest, provider };
}

function shortModel(id: string): string {
  const m = splitModel(id);
  return m.effort ? `${m.name} · ${m.effort}` : m.name;
}

function ModelName({ id, big }: { id: string | null; big?: boolean }) {
  if (!id) return <span className="tag warn">Chưa rõ</span>;
  const m = splitModel(id);
  return (
    <span className={big ? "mb-model mb-model-big" : "mb-model"} title={id}>
      <span className="mb-model-name">{m.name}</span>
      {m.effort && <span className="mb-model-effort"> · {m.effort}</span>}
      {m.provider && <span className="mb-model-provider">{m.provider}</span>}
    </span>
  );
}

export function ModelsPage() {
  const qc = useQueryClient();
  const { data, isLoading, error } = useQuery({ queryKey: ["models"], queryFn: api.models });
  const setMut = useMutation({
    mutationFn: (v: { section: "roles" | "tasks"; key: string; value: string }) =>
      api.modelSet(v.section, v.key, v.value),
    onSuccess: (updated: ModelConfig) => qc.setQueryData(["models"], updated),
  });

  const sub = "Model nào làm từng bước của Board.";
  if (isLoading) return <Page title="Models" sub={sub}><Loading rows={5} /></Page>;
  if (error) {
    return (
      <Page title="Models" sub={sub}>
        {isMissing(error) ? <EndpointMissing endpoint="/api/models" /> : <ErrorState message={(error as Error).message} />}
      </Page>
    );
  }
  if (!data) return <Page title="Models"><EmptyState title="Chưa có cấu hình model" /></Page>;

  const ompRoles = data.omp_roles ?? {};
  const board = data.board ?? [];
  const saving = (section: "roles" | "tasks", key: string) =>
    setMut.isPending && setMut.variables?.section === section && setMut.variables?.key === key;
  const set = (section: "roles" | "tasks", key: string) => (value: string) => setMut.mutate({ section, key, value });

  const groups = CLASS_ORDER
    .map((cls) => ({ cls, rows: board.filter((r) => r.class === cls) }))
    .filter((g) => g.rows.length > 0);
  const advisor = board.filter((r) => r.class === null);
  const dev = board.find((r) => r.step === "dev");

  const roleOrder: ModelRole[] = ["default", "plan", "smol", "slow", "vision"];
  const roles = roleOrder.filter((r) => data.roles[r] !== undefined || r === "default");
  const classes = data.classes ?? CLASS_ORDER;

  return (
    <Page title="Models" sub={<>{sub} Lưu ở <code>{data.path || "~/.config/ckit/models.toml"}</code>.</>}>
      <div className="card">
        <div className="mb-head">
          <h2 className="mb-title">Board dùng model nào</h2>
          {dev && (
            <div className="mb-code">
              <span className="mb-code-label">Code bằng</span>
              <ModelName id={dev.model} big />
            </div>
          )}
        </div>
        {setMut.isError && <p className="err mb-error">Không lưu được: {(setMut.error as Error).message}</p>}
        {groups.map((g) => (
          <section key={g.cls} className="mb-group">
            <div className="mb-group-head">
              <span className="mb-group-name">{CLASS_LABEL[g.cls]}</span>
              <ModelPicker
                value={data.tasks[g.cls] ?? ""}
                ompRoles={ompRoles}
                saving={saving("tasks", g.cls)}
                label={`Model cho ${CLASS_LABEL[g.cls]}`}
                onChange={set("tasks", g.cls)}
              />
            </div>
            {g.rows.map((r) => <BoardRow key={r.step} row={r} />)}
          </section>
        ))}
        {advisor.length > 0 && (
          <section className="mb-group">
            <div className="mb-group-head">
              <span className="mb-group-name">Cố vấn mỗi lượt</span>
              <span className="faint mb-group-note">Đổi bằng role advisor trong omp</span>
            </div>
            {advisor.map((r) => <BoardRow key={r.step} row={r} />)}
          </section>
        )}
      </div>

      <details className="card bd-closed mb-advanced">
        <summary>Nâng cao</summary>
        <div className="mb-advanced-body">
          <p className="muted mb-advanced-note">
            Board chỉ đọc mục <code>tasks</code> theo lớp việc; lớp để trống dùng <code>roles.default</code>, cũng trống thì omp tự dùng role <code>default</code> của nó.
            Giá trị dạng <code>@task</code> là role của omp, đặt trong <code>{data.omp_config ?? "~/.omp/agent/config.yml"}</code>.
          </p>
          <div className="models-grid">
            <div>
              <div className="card-title">Roles</div>
              <div className="model-table">
                {roles.map((r) => (
                  <ModelRow key={r} name={r} hint={ROLE_HINTS[r]}>
                    <ModelPicker value={data.roles[r] ?? ""} ompRoles={ompRoles} saving={saving("roles", r)} label={`roles.${r}`} onChange={set("roles", r)} />
                  </ModelRow>
                ))}
              </div>
            </div>
            <div>
              <div className="card-title">Tasks theo lớp việc</div>
              <div className="model-table">
                {classes.map((cls) => (
                  <ModelRow key={cls} name={cls} hint={TASK_HINTS[cls]}>
                    <ModelPicker value={data.tasks[cls] ?? ""} ompRoles={ompRoles} saving={saving("tasks", cls)} label={`tasks.${cls}`} onChange={set("tasks", cls)} />
                  </ModelRow>
                ))}
              </div>
            </div>
            <div>
              <div className="card-title">Role của omp</div>
              <div className="model-table">
                {Object.keys(ompRoles).length === 0 && <p className="faint">omp chưa đặt modelRoles, nó dùng mặc định cài sẵn.</p>}
                {Object.entries(ompRoles).map(([k, v]) => (
                  <ModelRow key={k} name={`@${k}`}>
                    <span className="mono model-omp">{v}</span>
                  </ModelRow>
                ))}
                {data.omp_reviewer && (
                  <ModelRow name="reviewer" hint="subagent">
                    <span className="mono model-omp">{data.omp_reviewer}</span>
                  </ModelRow>
                )}
              </div>
            </div>
          </div>
        </div>
      </details>
    </Page>
  );
}

function BoardRow({ row }: { row: ModelBoardRow }) {
  return (
    <div className={row.step === "dev" ? "mb-row mb-row-dev" : "mb-row"}>
      <div className="mb-step">
        <span className="mb-step-label">{row.label}</span>
        <span className="mb-step-who">{row.who}</span>
      </div>
      <div className="mb-actual"><ModelName id={row.model} /></div>
      <div className="mb-via">{row.via}</div>
    </div>
  );
}

function ModelRow({ name, hint, children }: { name: string; hint?: string; children: ReactNode }) {
  return (
    <div className="model-row">
      <div className="model-key">
        <span className="model-key-name">{name}</span>
        {hint ? <span className="model-key-hint">{hint}</span> : null}
      </div>
      <div className="model-val">{children}</div>
    </div>
  );
}

const CUSTOM = "__custom";

/** One setting: omp default, an omp role (`@x`, with what it resolves to), or a free model id. */
function ModelPicker({ value, ompRoles, saving, label, onChange }: {
  value: string; ompRoles: Record<string, string>; saving: boolean; label: string; onChange: (v: string) => void;
}) {
  const [custom, setCustom] = useState(false);
  const [draft, setDraft] = useState("");
  useEffect(() => { if (!saving) setCustom(false); }, [value, saving]);
  const roleOpts = Object.entries(ompRoles).map(([k, m]) => ({ v: `@${k}`, label: `@${k} · ${shortModel(m)}` }));
  const known = value === "" || roleOpts.some((o) => o.v === value);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const v = draft.trim();
    if (v) onChange(v);
  };
  return (
    <div className="mb-picker">
      <select
        className="model-select"
        aria-label={label}
        value={custom ? CUSTOM : value}
        disabled={saving}
        onChange={(e) => {
          const v = e.target.value;
          if (v === CUSTOM) {
            setCustom(true);
            setDraft(value.startsWith("@") ? "" : value);
          } else {
            setCustom(false);
            onChange(v);
          }
        }}
      >
        <option value="">Mặc định của omp{ompRoles.default ? ` · ${shortModel(ompRoles.default)}` : ""}</option>
        {roleOpts.length > 0 && (
          <optgroup label="Role của omp">
            {roleOpts.map((o) => <option key={o.v} value={o.v}>{o.label}</option>)}
          </optgroup>
        )}
        {!known && <option value={value}>{value}</option>}
        <option value={CUSTOM}>Nhập model id khác…</option>
      </select>
      {custom && (
        <form className="mb-custom" onSubmit={submit}>
          <input
            type="text"
            className="mono"
            aria-label={`${label}: model id`}
            placeholder="provider/model:effort"
            value={draft}
            autoFocus
            onChange={(e) => setDraft(e.target.value)}
          />
          <button type="submit" className="primary" disabled={saving || !draft.trim()}>Lưu</button>
          <button type="button" className="ghost" onClick={() => setCustom(false)}>Huỷ</button>
        </form>
      )}
      {saving && <span className="model-saving">Đang lưu…</span>}
    </div>
  );
}
