// Epic pieces of the task page: the phases fetch (ROADMAP → each phase's PLAN tasks, live
// from the engine for the phase being built) and one sub-task's detail.
// Contract: GET /api/board/:id/phases?root= → { phases, flat, live }.
import { useQuery } from "@tanstack/react-query";
import { api, type BoardStage, type BoardTask, type SubTaskRow } from "./api";
import { isMissing } from "./ui";

/** One fetch per task page. Polls while the task runs. */
export function usePhases(root: string, task: BoardTask) {
  return useQuery({
    // `updated` in the key: any task change (stage, status) refetches once.
    queryKey: ["board-phases", root, task.id, task.updated],
    queryFn: () => api.boardPhases(root, task.id),
    placeholderData: (prev) => prev,
    refetchInterval: (q) => (isMissing(q.state.error) ? false : task.status === "running" ? 4000 : false),
  });
}

// Step names for a phase's "Đang: …" line.
const STEP_LABEL: Partial<Record<BoardStage, string>> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận",
  solution: "Lập kế hoạch",
  review: "Phản biện kế hoạch",
  approval: "Chờ bạn duyệt",
  dev: "Code",
  test: "Kiểm thử",
  security: "Review bảo mật",
  report: "Báo cáo",
  acceptance: "Chờ bạn nghiệm thu",
};
export const stepLabel = (s: BoardStage) => STEP_LABEL[s] ?? s;

const STATUS_LABEL: Record<SubTaskRow["status"], string> = { todo: "Chưa làm", in_progress: "Đang làm", blocked: "Bị chặn", done: "Xong" };

export const doneN = (ts: SubTaskRow[]) => ts.filter((s) => s.status === "done").length;

// ── One task ───────────────────────────────────────────────────────────────
/** Facts of one sub-task (drawer body): full text, files, dependencies (linked), AC, verify, note, commit. */
export function SubFacts({ s, list, onOpen }: { s: SubTaskRow; list: SubTaskRow[]; onOpen: (key: string) => void }) {
  const deps = s.depends.split(/[,;\s]+/).filter(Boolean);
  const verify =
    s.retries > 0 ? `Lỗi ${s.retries} lần` : s.status === "done" ? "Đạt" : s.status === "in_progress" ? "Đang chạy" : "Chưa chạy";
  const detail = s.detail && s.detail.trim() !== s.title.trim() ? s.detail : "";
  return (
    <div className="tp-sub">
      <dl className="tp-dl">
        {detail ? (
          <>
            <dt>Chi tiết</dt>
            <dd>{detail}</dd>
          </>
        ) : null}
        <dt>File</dt>
        <dd className={s.files ? "mono" : "faint"}>{s.files || "Chưa ghi"}</dd>
        <dt>Phụ thuộc</dt>
        <dd>
          {deps.length === 0 ? (
            <span className="faint">Không</span>
          ) : (
            deps.map((k, i) => {
              const hit = list.find((x) => x.key === k);
              return (
                <span key={`${k}-${i}`}>
                  {i > 0 ? ", " : ""}
                  {hit ? (
                    <button className="link" onClick={() => onOpen(k)} title={hit.title}>{k}</button>
                  ) : (
                    <span className="mono">{k}</span>
                  )}
                  {hit ? <span className="faint"> ({STATUS_LABEL[hit.status].toLowerCase()})</span> : null}
                </span>
              );
            })
          )}
        </dd>
        <dt>Tiêu chí (AC)</dt>
        <dd className={s.ac ? "" : "faint"}>{s.ac || "Chưa gắn"}</dd>
        <dt>Kiểm tra</dt>
        <dd>
          <span className={s.retries > 0 ? "tp-warn" : ""}>{verify}</span>
          {s.note ? <p className="tp-sub-note">{s.note}</p> : null}
        </dd>
        <dt>Commit</dt>
        <dd className={s.commit ? "mono" : "faint"}>{s.commit ?? "Chưa có"}</dd>
      </dl>
    </div>
  );
}
