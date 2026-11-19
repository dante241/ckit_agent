// Tài liệu drawer: the original request + step list, then every document of the task
// (`/feature` tree: PROJECT, ROADMAP, M1-CONTEXT…; board REVIEW / REPORT).
import { useState } from "react";
import type { BoardDetail } from "./api";
import { Drawer } from "./ui";
import { Markdown } from "./markdown";
import { Pipeline } from "./Steps";
import { WorkflowPipeline } from "./Workflows";
import { clock } from "./Board";

export const REQUEST_DOC = "Yêu cầu";

export function DocsDrawer({ d, root, pick, onClose }: { d: BoardDetail; root: string; pick?: string; onClose: () => void }) {
  const t = d.task;
  const names = [REQUEST_DOC, ...d.docs.map((x) => x.name)];
  const fallback = d.focus && names.includes(d.focus) ? d.focus : REQUEST_DOC;
  const [cur, setCur] = useState(pick && names.includes(pick) ? pick : fallback);
  const toolbar = (
    <div className="dr-filters dr-docs" role="tablist" aria-label="Tài liệu">
      {names.map((k) => (
        <button key={k} role="tab" aria-selected={k === cur} className={`chip ${k === cur ? "is-on" : ""}`} onClick={() => setCur(k)}>
          {k}
        </button>
      ))}
    </div>
  );
  return (
    <Drawer label="Tài liệu" head={<h3 className="dr-title">Tài liệu · {t.id}</h3>} toolbar={toolbar} onClose={onClose} className="dr-docs-panel">
      {cur === REQUEST_DOC ? (
        <div className="tp-req">
          <p className="tp-req-text">{t.request}</p>
          <p className="tp-meta faint">
            Tạo {clock(t.created)}
            {t.base ? <> · từ nhánh <code>{t.base}</code></> : null}
            {t.feature ? <> · feature <code>{t.feature}</code></> : null}
            {t.rounds.solution > 0 ? ` · Reviewer trả lại ${t.rounds.solution} lần` : ""}
          </p>
          <h4 className="tp-req-title">Các bước</h4>
          {t.workflow ? <WorkflowPipeline name={t.workflow.name} stage={t.stage} status={t.status} /> : <Pipeline task={t} root={root} />}
        </div>
      ) : (
        <div className="bd-doc">
          <Markdown source={d.docs.find((x) => x.name === cur)?.content ?? ""} />
        </div>
      )}
    </Drawer>
  );
}
