// Status strip under the Board header: one count tile per lifecycle lane plus
// "Lỗi", each with a one-line hint. A tile filters the current view (click
// again to clear). "Chờ bạn" turns amber and "Lỗi" red only when non-zero.
import type { BoardTaskSummary } from "./api";
import { askingYou } from "./Asks";
import { isPaused } from "./Interject";
import { money } from "./Board";
import { laneOf, type Lane } from "./EpicBoard";

export type StripFilter = Lane | "fail";

const TILES: { id: StripFilter; name: string }[] = [
  { id: "new", name: "Chưa bắt đầu" },
  { id: "plan", name: "Lên kế hoạch" },
  { id: "you", name: "Chờ bạn" },
  { id: "dev", name: "Đang làm" },
  { id: "test", name: "Kiểm thử" },
  { id: "fail", name: "Lỗi" },
  { id: "done", name: "Xong" },
];

export function matches(f: StripFilter, t: BoardTaskSummary): boolean {
  return f === "fail" ? t.status === "failed" : laneOf(t) === f;
}

const ids = (list: BoardTaskSummary[]) => list.map((t) => t.id).join(", ");
const parts = (xs: (string | false)[]) => xs.filter(Boolean).join(", ");

/** One short line under the count: what is in the tile. */
function hint(f: StripFilter, list: BoardTaskSummary[]): string {
  if (list.length === 0) return f === "you" ? "Không có việc chờ" : "Không có";
  const n = (p: (t: BoardTaskSummary) => boolean) => list.filter(p).length;
  switch (f) {
    case "new":
      return "Chờ bấm Bắt đầu";
    case "plan": {
      const run = n((t) => t.status === "running");
      const queued = n((t) => t.status === "queued");
      const failed = n((t) => t.status === "failed");
      return parts([run > 0 && `${run} đang chạy`, queued > 0 && `${queued} chờ lượt`, failed > 0 && `${failed} lỗi`]) || `${list.length} việc`;
    }
    case "you": {
      const ask = n((t) => askingYou(t) || (t.status === "waiting" && !isPaused(t) && !["approval", "acceptance", "investigate"].includes(t.stage)));
      const approve = n((t) => t.status === "waiting" && !isPaused(t) && t.stage === "approval");
      const accept = n((t) => t.status === "waiting" && !isPaused(t) && t.stage === "acceptance");
      const read = n((t) => t.status === "waiting" && !isPaused(t) && t.stage === "investigate");
      const paused = n((t) => isPaused(t));
      return parts([approve > 0 && `${approve} duyệt`, ask > 0 && `${ask} trả lời`, accept > 0 && `${accept} nghiệm thu`, read > 0 && `${read} đọc`, paused > 0 && `${paused} tạm dừng`]);
    }
    case "dev":
    case "test":
    case "fail":
      return ids(list);
    case "done":
      return `tổng ${money(list.reduce((s, t) => s + t.cost, 0))}`;
  }
}

export function StatusStrip({ tasks, active, onPick }: { tasks: BoardTaskSummary[]; active: StripFilter | null; onPick: (f: StripFilter | null) => void }) {
  return (
    <div className="ss" role="group" aria-label="Lọc theo trạng thái">
      {TILES.map((tile) => {
        const list = tasks.filter((t) => matches(tile.id, t));
        const on = active === tile.id;
        const tone = list.length > 0 && tile.id === "you" ? " is-need" : list.length > 0 && tile.id === "fail" ? " is-err" : "";
        const sub = hint(tile.id, list);
        return (
          <button
            key={tile.id}
            type="button"
            className={`ss-tile lane-${tile.id}${tone}${on ? " is-on" : ""}`}
            aria-pressed={on}
            title={on ? "Bỏ lọc" : `Chỉ hiện: ${tile.name}`}
            onClick={() => onPick(on ? null : tile.id)}
          >
            <span className="ss-top">
              <span className="ss-n">{list.length}</span>
              <span className="ss-dot" aria-hidden="true" />
            </span>
            <span className="ss-name">{tile.name}</span>
            <span className="ss-sub" title={sub}>{sub}</span>
          </button>
        );
      })}
    </div>
  );
}
