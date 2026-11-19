// The one thing the boss can do on a task card, right on the card (Bảng,
// "Cần bạn" list, room seat). Same mutations as the task page; anything that
// needs reading or a choice (answer, acceptance = create the MR, plan-only approval) opens
// the task page instead, focused where the boss acts.
import type { MouseEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, type BoardTaskSummary } from "./api";
import { acGateOf, askingYou } from "./Asks";
import { isNew, isPaused } from "./Interject";
import { isTerminal } from "./Board";
import { focusOnOpen, type OpenFocus } from "./TaskPage";
import { Glyph } from "./icons";

type Run = "approve" | "retry" | "resume";
type Action = { label: string; tone: "primary" | "retry"; run?: Run; focus?: OpenFocus; title?: string };

/** What the card offers for this task, or null when nothing waits on the boss. */
export function cardAction(t: BoardTaskSummary): Action | null {
  if (isTerminal(t)) return null;
  if (askingYou(t)) return { label: "Trả lời", tone: "primary", focus: "ask" };
  if (t.status === "failed") return { label: "Chạy lại", tone: "retry", run: "retry" };
  if (t.status !== "waiting") return null;
  if (acGateOf(t)) return { label: "Duyệt đổi AC", tone: "primary", focus: "ask" };
  if (isPaused(t)) return { label: isNew(t) ? "Bắt đầu" : "Tiếp tục", tone: "primary", run: "resume" };
  switch (t.stage) {
    case "approval":
      return { label: "Duyệt kế hoạch", tone: "primary", run: "approve" };
    case "acceptance":
      return { label: "Nghiệm thu", tone: "primary", focus: "act", title: "Mở việc để nghiệm thu: agent đẩy nhánh của task và tạo MR bằng skill của dự án" };
    case "investigate":
      return { label: "Xem trả lời", tone: "primary", focus: "act" };
    default:
      return { label: "Trả lời", tone: "primary", focus: "reply" };
  }
}

export function CardAction({ t, root, onOpen }: { t: BoardTaskSummary; root: string; onOpen: () => void }) {
  const qc = useQueryClient();
  const a = cardAction(t);
  const act = useMutation({
    mutationFn: async (run: Run) => {
      if (run === "retry") return api.boardRetry(root, t.id);
      if (run === "resume") return api.boardResume(root, t.id, "");
      // A plan-only task chooses between "chỉ lưu kế hoạch" and "code luôn" on its page.
      const d = await qc.fetchQuery({ queryKey: ["board-task", root, t.id], queryFn: () => api.boardTask(root, t.id), staleTime: 2000 });
      if (!(d.task.steps ?? []).includes("dev")) return null;
      return api.boardApprove(root, t.id);
    },
    onSuccess: (r) => {
      if (r === null) {
        focusOnOpen("act");
        onOpen();
        return;
      }
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
      qc.invalidateQueries({ queryKey: ["board-log", root, t.id] });
    },
  });
  if (!a) return null;
  const click = (e: MouseEvent) => {
    e.stopPropagation();
    if (a.run) act.mutate(a.run);
    else {
      focusOnOpen(a.focus ?? "act");
      onOpen();
    }
  };
  return (
    <div className="ca">
      <button
        type="button"
        className={`ca-btn ${a.tone === "retry" ? "is-retry" : "primary"}`}
        onClick={click}
        disabled={act.isPending}
        aria-label={`${a.label} ${t.id}`}
        title={a.title}
      >
        {a.run === "retry" ? <Glyph name="refresh" /> : null}
        {act.isPending ? (a.run === "approve" ? "Đang duyệt…" : "Đang xếp lượt…") : a.label}
        {!a.run ? <Glyph name="next" /> : null}
      </button>
      {act.error ? <p className="ca-err" role="alert">{(act.error as Error).message}</p> : null}
    </div>
  );
}
