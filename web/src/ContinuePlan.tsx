// "Code theo kế hoạch này": a finished plan-only task (Chỉ kế hoạch) goes on to
// code its approved plan on the same branch. The server decides when it applies
// (`can_continue`: done, Solution without Dev, branch still unmerged).
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, type BoardDetail } from "./api";

export function ContinuePlan({ d, root }: { d: BoardDetail; root: string }) {
  const qc = useQueryClient();
  const t = d.task;
  const go = useMutation({
    mutationFn: () => api.boardContinue(root, t.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task"] });
      qc.invalidateQueries({ queryKey: ["board-log"] });
    },
  });
  if (!d.can_continue) return null;
  return (
    <section className="tp-act" aria-label="Code tiếp">
      <div className="tp-act-row">
        <p className="bd-gate-msg">
          Kế hoạch đã duyệt và còn nằm trên nhánh {t.branch}. Bấm để team code theo kế hoạch này: Dev, Kiểm thử, Báo cáo rồi tới bạn nghiệm thu.
        </p>
        <div className="tp-act-btns">
          <button className="primary" onClick={() => go.mutate()} disabled={go.isPending}>
            {go.isPending ? "Đang xếp lượt…" : "Code theo kế hoạch này"}
          </button>
        </div>
      </div>
      {go.error ? <p className="hint hint-err">{(go.error as Error).message}</p> : null}
    </section>
  );
}
