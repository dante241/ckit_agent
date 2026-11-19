// Board view "Review MR": the project's open PRs/MRs and their last `ckit review`.
// A review runs in the background on the server (read-only, never the working
// tree); posting to GitLab/GitHub is a separate click, so the boss reads first.
import { useState } from "react";
import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { api, type MrComment, type MrFinding, type MrFlags, type MrItem, type MrLabel } from "./api";
import { Select } from "./Select";
import { EmptyState, ErrorState, Loading } from "./ui";
import { Markdown } from "./markdown";
import { serverMessage } from "./Steps";
import { Glyph } from "./icons";
import { ago } from "./Board";

type Tone = "idle" | "queue" | "run" | "err" | "warn" | "new" | "ok" | "ext";

/** ckit's review of the MR's current commit is on GitLab/GitHub: posted from here, or seen
 *  in the MR's comments (CI, another machine). */
const postedNow = (m: MrItem) => m.posted_head === m.head_sha || Boolean(m.review?.posted_at && m.review.head_sha === m.head_sha);

/** A review written without ckit ("## Code Review…" comment). Counts for the current commit when it
 *  cites it, or cites no commit at all; one citing an older commit means new commits came since. */
const otherFor = (m: MrItem): "now" | "old" | null => {
  const o = m.other_review;
  if (!o) return null;
  return !o.sha || m.head_sha.startsWith(o.sha) ? "now" : "old";
};

/** Where an MR stands: running state first, then the current commit against what was
 *  reviewed here (`review`), what ckit posted on the MR (`posted_head`) and reviews written
 *  without ckit (`other_review`). */
/** The project's `/review-pr` already ran on the MR's current commit. */
const commandNow = (m: MrItem) => (m.command_run && m.command_run.head_sha === m.head_sha ? m.command_run : null);

function status(m: MrItem): { tone: Tone; label: string } {
  if (m.queued) return { tone: "queue", label: "Chờ review" };
  if (m.running) return { tone: "run", label: "Đang review" };
  if (m.error) return { tone: "err", label: "Review lỗi" };
  const c = commandNow(m);
  if (c) {
    if (c.verdict === "ask") return { tone: "warn", label: "Chờ bạn duyệt" };
    if (c.verdict !== "pass") return { tone: "err", label: "Review lỗi" };
    if (c.flags?.dry_run) return { tone: "new", label: `Dry-run${c.blocking > 0 ? ` (${c.blocking} lỗi chặn)` : ""}` };
    return c.blocking > 0 ? { tone: "warn", label: `Cần fix (${c.blocking})` } : { tone: "ok", label: c.merged ? "Đã merge" : "Đã review" };
  }
  if (postedNow(m)) return { tone: "ok", label: "Đã đăng" };
  if (m.review?.head_sha === m.head_sha) return { tone: "new", label: "Chưa đăng" };
  if (otherFor(m) === "now") return { tone: "ext", label: "Đã review (ngoài ckit)" };
  if (m.review || m.posted_head || m.command_run || otherFor(m) === "old") return { tone: "warn", label: "Có commit mới" };
  return { tone: "idle", label: "Chưa review" };
}

const busyOf = (m: MrItem) => m.running || m.queued;

const isoOf = (secs: number) => new Date(secs * 1000).toISOString();
const blocking = (f: MrFinding) => f.severity.toLowerCase() === "blocking";

function Counts({ m }: { m: MrItem }) {
  const fs = m.review?.findings ?? [];
  if (!m.review) return null;
  const b = fs.filter(blocking).length;
  const s = fs.length - b;
  if (!fs.length) return <span className="mr-count is-clean">Không có lỗi</span>;
  return (
    <span className="mr-counts">
      {b > 0 && <span className="mr-count is-block">{b} cần sửa</span>}
      {s > 0 && <span className="mr-count">{s} gợi ý</span>}
    </span>
  );
}

/** The MR's labels as neutral chips; the label's own color only as a dot (any hue stays readable). */
function Labels({ list }: { list: MrLabel[] }) {
  if (!list.length) return null;
  return (
    <span className="mr-labels">
      {list.map((l) => (
        <span key={l.name} className="mr-label" title={l.name}>
          <span className="mr-label-dot" style={l.color ? { background: l.color } : undefined} aria-hidden="true" />
          {l.name}
        </span>
      ))}
    </span>
  );
}

const createdMs = (m: MrItem) => Date.parse(m.created) || 0;

/** What the list can be narrowed to; "" = any. */
type Filters = { q: string; status: string; label: string; author: string; target: string; hideDraft: boolean };
const NO_FILTERS: Filters = { q: "", status: "", label: "", author: "", target: "", hideDraft: false };
/** Per project: a label / author / branch of one repo means nothing in another. */
const filtersKey = (root: string) => `ckit.mr.filters:${root}`;
const STATUS_FILTERS: { id: string; label: string; test: (m: MrItem) => boolean }[] = [
  { id: "todo", label: "Chưa review / có commit mới", test: (m) => ["idle", "warn"].includes(status(m).tone) },
  { id: "busy", label: "Đang / chờ review", test: busyOf },
  { id: "unposted", label: "Chưa đăng", test: (m) => status(m).tone === "new" },
  { id: "blocking", label: "Có lỗi cần sửa", test: (m) => Boolean(m.review?.findings.some(blocking)) },
  { id: "posted", label: "Đã đăng", test: (m) => status(m).tone === "ok" },
  { id: "ext", label: "Đã review ngoài ckit", test: (m) => status(m).tone === "ext" },
  { id: "failed", label: "Review lỗi", test: (m) => status(m).tone === "err" },
];

function loadFilters(root: string): Filters {
  try {
    return { ...NO_FILTERS, ...JSON.parse(localStorage.getItem(filtersKey(root)) ?? "{}") };
  } catch {
    return NO_FILTERS;
  }
}

function matchesFilters(m: MrItem, f: Filters): boolean {
  const q = f.q.trim().toLowerCase().replace(/^[!#]/, "");
  if (q && ![m.title, String(m.number), m.author, m.source, m.target].some((s) => s.toLowerCase().includes(q))) return false;
  if (f.status && !STATUS_FILTERS.find((s) => s.id === f.status)?.test(m)) return false;
  if (f.label && !m.labels.some((l) => l.name === f.label)) return false;
  if (f.author && m.author !== f.author) return false;
  if (f.target && m.target !== f.target) return false;
  return !(f.hideDraft && m.draft);
}

const uniq = (xs: string[]) => [...new Set(xs.filter(Boolean))].sort((a, b) => a.localeCompare(b, "vi"));

export function MrReviews({ root }: { root: string }) {
  const qc = useQueryClient();
  const key = ["board-mrs", root];
  const q = useQuery({
    queryKey: key,
    queryFn: () => api.boardMrs(root),
    enabled: Boolean(root),
    // A review finishes on the server: poll while one runs or waits, else refresh slowly.
    refetchInterval: (query) => (query.state.data?.items.some(busyOf) ? 3000 : 60_000),
  });
  const [sel, setSel] = useState<number | null>(null);
  const [filters, setFiltersState] = useState<Filters>(() => loadFilters(root));
  // The view stays mounted when the boss switches project: read that project's filters.
  const [filtersRoot, setFiltersRoot] = useState(root);
  if (filtersRoot !== root) {
    setFiltersRoot(root);
    setFiltersState(loadFilters(root));
  }
  const setFilters = (patch: Partial<Filters>) =>
    setFiltersState((f) => {
      const next = { ...f, ...patch };
      localStorage.setItem(filtersKey(root), JSON.stringify(next));
      return next;
    });
  const [picked, setPicked] = useState<Set<number>>(new Set());
  // Newest created first (the server asks GitLab/GitHub for that order too).
  const all = [...(q.data?.items ?? [])].sort((a, b) => createdMs(b) - createdMs(a));
  const items = all.filter((m) => matchesFilters(m, filters));
  const cur = items.find((m) => m.number === sel) ?? items[0];
  const host = q.data?.host === "github" ? "GitHub" : "GitLab";
  const mark = q.data?.host === "github" ? "#" : "!";
  const refresh = () => qc.invalidateQueries({ queryKey: key });
  // Projects with their own `/review-pr` run that command (it posts, labels, merges itself).
  const command = q.data?.command ?? null;
  const [flags, setFlags] = useState<MrFlags>({ no_merge: false, dry_run: false });
  const review = useMutation({ mutationFn: (n: number) => api.boardMrReview(root, n, flags), onSettled: refresh });
  const post = useMutation({ mutationFn: (n: number) => api.boardMrPost(root, n), onSettled: refresh });
  const answer = useMutation({ mutationFn: (a: { n: number; post: boolean; note: string }) => api.boardMrAnswer(root, a.n, a.post, a.note), onSettled: refresh });
  // Several MRs at once: the server queues them (5 run at a time). Refused ones are reported.
  const batch = useMutation({
    mutationFn: async (ns: number[]) => {
      const res = await Promise.allSettled(ns.map((n) => api.boardMrReview(root, n, flags)));
      const failed = res.flatMap((r, i) => (r.status === "rejected" ? [`${mark}${ns[i]}: ${serverMessage(r.reason as Error)}`] : []));
      setPicked(new Set(ns.filter((_, i) => res[i]?.status === "rejected")));
      if (failed.length) throw new Error(`${failed.length} MR không gửi được: ${failed.join("; ")}`);
    },
    onSettled: refresh,
  });
  const pickable = items.filter((m) => !busyOf(m));
  const chosen = pickable.filter((m) => picked.has(m.number));
  const allPicked = pickable.length > 0 && chosen.length === pickable.length;
  const toggle = (n: number) =>
    setPicked((s) => {
      const next = new Set(s);
      if (!next.delete(n)) next.add(n);
      return next;
    });
  const active = (Object.keys(NO_FILTERS) as (keyof Filters)[]).filter((k) => filters[k] !== NO_FILTERS[k]).length;

  if (q.isLoading) return <Loading rows={4} />;
  if (q.error) return <ErrorState message={serverMessage(q.error as Error)} />;
  if (!all.length)
    return (
      <EmptyState
        icon={<Glyph name="check" />}
        title="Không có MR nào đang mở"
        hint={<>Khi có merge request mới trên {host}, nó hiện ở đây để bạn cho AI review. Muốn review tự động mỗi lần push: chạy <code>ckit review init</code> trong repo.</>}
      />
    );

  return (
    <div className="mr-view">
      <div className="mr-filters" role="search" aria-label="Lọc merge request">
        <input className="mr-search" type="search" placeholder={`Tìm tiêu đề, ${mark}số, người tạo, nhánh`} value={filters.q} onChange={(e) => setFilters({ q: e.target.value })} aria-label="Tìm merge request" />
        <Select compact aria-label="Trạng thái review" value={filters.status} active={filters.status !== ""} onChange={(e) => setFilters({ status: e.target.value })}>
          <option value="">Mọi trạng thái</option>
          {STATUS_FILTERS.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
        </Select>
        <Select compact aria-label="Label" value={filters.label} active={filters.label !== ""} onChange={(e) => setFilters({ label: e.target.value })}>
          <option value="">Mọi label</option>
          {uniq(all.flatMap((m) => m.labels.map((l) => l.name))).map((l) => <option key={l} value={l}>{l}</option>)}
        </Select>
        <Select compact aria-label="Người tạo" value={filters.author} active={filters.author !== ""} onChange={(e) => setFilters({ author: e.target.value })}>
          <option value="">Mọi người tạo</option>
          {uniq(all.map((m) => m.author)).map((a) => <option key={a} value={a}>{a}</option>)}
        </Select>
        <Select compact aria-label="Nhánh đích" value={filters.target} active={filters.target !== ""} onChange={(e) => setFilters({ target: e.target.value })}>
          <option value="">Mọi nhánh đích</option>
          {uniq(all.map((m) => m.target)).map((t) => <option key={t} value={t}>→ {t}</option>)}
        </Select>
        <label className="mr-check">
          <input type="checkbox" checked={filters.hideDraft} onChange={(e) => setFilters({ hideDraft: e.target.checked })} />
          Ẩn Draft
        </label>
        {active > 0 && <button className="link mr-clear" onClick={() => setFilters(NO_FILTERS)}>Bỏ lọc</button>}
        <span className="mr-shown">{items.length}/{all.length} MR</span>
      </div>

      {command && (
        <div className="mr-cmd" role="note">
          <p>
            Review chạy <code>/review-pr</code> của dự án (<code>{command.replace(`${root}/`, "")}</code>) đúng như khi bạn gõ trong omp:
            mọi bước của lệnh đó, kể cả đăng comment, gắn label, merge và cập nhật PMS nếu lệnh có làm. Khi lệnh cần hỏi bạn, Board dừng lại chờ bạn duyệt.
          </p>
          <span className="mr-cmd-flags">
            <label className="mr-check">
              <input type="checkbox" checked={flags.dry_run} onChange={(e) => setFlags((f) => ({ ...f, dry_run: e.target.checked }))} />
              Chỉ xem (<code>--dry-run</code>)
            </label>
            <label className="mr-check">
              <input type="checkbox" checked={flags.no_merge} onChange={(e) => setFlags((f) => ({ ...f, no_merge: e.target.checked }))} />
              Không merge (<code>--no-merge</code>)
            </label>
          </span>
        </div>
      )}
      <div className="mr-split">
        <div className="mr-listcol">
          <div className="mr-batch">
            <label className="mr-check">
              <input
                type="checkbox"
                checked={allPicked}
                ref={(el) => { if (el) el.indeterminate = chosen.length > 0 && !allPicked; }}
                disabled={!pickable.length}
                onChange={() => setPicked(allPicked ? new Set() : new Set(pickable.map((m) => m.number)))}
              />
              {chosen.length ? `Đã chọn ${chosen.length}` : "Chọn tất cả"}
            </label>
            <button className="primary" disabled={!chosen.length || batch.isPending} onClick={() => batch.mutate(chosen.map((m) => m.number))}>
              {batch.isPending ? "Đang gửi…" : chosen.length ? `Review ${chosen.length} MR` : "Review nhiều MR"}
            </button>
          </div>
          {batch.error && <p className="hint hint-err mr-batch-err" role="alert">{(batch.error as Error).message}</p>}
          {items.length === 0 ? (
            <p className="mr-empty">Không có MR nào khớp bộ lọc. <button className="link" onClick={() => setFilters(NO_FILTERS)}>Bỏ lọc</button></p>
          ) : (
            <div className="mr-list" role="listbox" aria-label="Merge request đang mở">
              {items.map((m) => {
                const st = status(m);
                const on = m.number === cur?.number;
                return (
                  <div key={m.number} className={`mr-item${on ? " is-on" : ""}`}>
                    <input
                      type="checkbox"
                      className="mr-pick"
                      aria-label={`Chọn ${mark}${m.number} để review`}
                      checked={picked.has(m.number) && !busyOf(m)}
                      disabled={busyOf(m)}
                      onChange={() => toggle(m.number)}
                    />
                    <button type="button" role="option" aria-selected={on} className="mr-row" onClick={() => setSel(m.number)}>
                      <span className="mr-row-top">
                        <span className="mr-row-title">{m.title}</span>
                        <span className={`mr-st is-${st.tone}`}>{st.label}</span>
                      </span>
                      <span className="mr-row-meta">
                        <span className="mr-no">{mark}{m.number}</span>
                        {m.draft && <span className="mr-draft">Draft</span>}
                        <span>{m.author}</span>
                        <span title={`Tạo ${new Date(m.created).toLocaleString("vi-VN")}`}>tạo {ago(m.created)}</span>
                        {!m.command_run && <Counts m={m} />}
                      </span>
                      <Labels list={m.labels} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      {cur && (
        <MrDetail
          key={cur.number}
          root={root}
          m={cur}
          host={host}
          mark={mark}
          reviewing={review.isPending && review.variables === cur.number}
          posting={post.isPending && post.variables === cur.number}
          actionError={
            review.error && review.variables === cur.number
              ? serverMessage(review.error as Error)
              : post.error && post.variables === cur.number
                ? serverMessage(post.error as Error)
                : answer.error && answer.variables?.n === cur.number
                  ? serverMessage(answer.error as Error)
                  : null
          }
          onReview={() => review.mutate(cur.number)}
          onPost={() => post.mutate(cur.number)}
          onAnswer={(p, note) => answer.mutate({ n: cur.number, post: p, note })}
          answering={answer.isPending && answer.variables?.n === cur.number}
          command={command}
          flags={flags}
        />
      )}
      </div>
    </div>
  );
}

function MrDetail({ root, m, host, mark, reviewing, posting, actionError, onReview, onPost, onAnswer, answering, command, flags }: {
  root: string; m: MrItem; host: string; mark: string; reviewing: boolean; posting: boolean; actionError: string | null;
  onReview: () => void; onPost: () => void; onAnswer: (post: boolean, note: string) => void; answering: boolean; command: string | null; flags: MrFlags;
}) {
  const qc = useQueryClient();
  // The MR's comments; also tells the list which commit ckit's posted summary covers.
  const comments = useQuery({
    queryKey: ["board-mr-comments", root, m.number, m.head_sha, m.review?.posted_at],
    queryFn: async () => {
      const res = await api.boardMrComments(root, m.number);
      // The list's status reads these; they are only known once the comments are loaded.
      qc.setQueryData<{ host: "gitlab" | "github"; items: MrItem[] }>(["board-mrs", root], (d) =>
        d && { ...d, items: d.items.map((x) => (x.number === m.number ? { ...x, posted_head: res.state.posted_head, posted_by: res.state.posted_by, other_review: res.state.other_review } : x)) },
      );
      return res.items;
    },
  });
  const r = m.review;
  const st = status(m);
  const current = Boolean(r && r.head_sha === m.head_sha);
  const busy = busyOf(m) || reviewing || posting;
  const block = r?.findings.filter(blocking) ?? [];
  const sugg = r?.findings.filter((f) => !blocking(f)) ?? [];
  return (
    <section className="mr-detail" aria-label={`${mark}${m.number}`}>
      <header className="mr-head">
        <div className="mr-head-text">
          <h3 className="mr-title">{m.title}</h3>
          <p className="mr-sub">
            <span className="mr-no">{mark}{m.number}</span> · {m.author} · tạo {new Date(m.created).toLocaleString("vi-VN")} · <code>{m.source}</code> → <code>{m.target}</code>
          </p>
          <Labels list={m.labels} />
        </div>
        <a className="mr-open" href={m.url} target="_blank" rel="noopener noreferrer">Mở trên {host} ↗</a>
      </header>

      <div className="mr-actions">
        {command ? (
          <button className="primary" onClick={onReview} disabled={busy}>
            {m.queued ? "Đang chờ lượt…" : m.running || reviewing ? "Đang chạy /review-pr…" : `Chạy /review-pr ${mark}${m.number}${flags.no_merge ? " --no-merge" : ""}${flags.dry_run ? " --dry-run" : ""}`}
          </button>
        ) : current && !postedNow(m) ? (
          <>
            <button className="primary" onClick={onPost} disabled={busy}>{posting ? "Đang đăng…" : `Đăng lên ${host}`}</button>
            <button onClick={onReview} disabled={busy}>Review lại</button>
          </>
        ) : current ? (
          <>
            <button onClick={onReview} disabled={busy}>Review lại</button>
            <button onClick={onPost} disabled={busy} title="Cập nhật comment tổng kết; comment theo dòng không đăng lặp">{posting ? "Đang đăng…" : "Đăng lại"}</button>
          </>
        ) : (
          <button className="primary" onClick={onReview} disabled={busy}>
            {m.queued ? "Đang chờ lượt…" : m.running || reviewing ? "Đang review…" : r ? "Review commit mới" : "Review MR"}
          </button>
        )}
        <span className={`mr-st is-${st.tone}`}>{st.label}</span>
      </div>

      {m.queued && <p className="mr-note" role="status">Đang chờ lượt: mỗi lúc chạy 5 review, MR này bắt đầu khi có chỗ trống.</p>}
      {m.running && (
        <p className="mr-note" role="status">
          {command
            ? "/review-pr đang chạy (thường 5 đến 20 phút). Bạn có thể rời trang, kết quả vẫn được lưu."
            : "AI đang đọc diff và code liên quan (thường 1 đến 5 phút). Bạn có thể rời trang, kết quả vẫn được lưu."}
        </p>
      )}
      {m.error && !busyOf(m) && <ErrorState message={m.error} />}
      {actionError && <p className="hint hint-err" role="alert">{actionError}</p>}

      {command ? (
        <CommandResult m={m} host={host} busy={busy || answering} onAnswer={onAnswer} />
      ) : r ? (
        <div className="mr-result">
          {!current && <p className="mr-note is-warn">Kết quả dưới đây của commit cũ <code>{r.head_sha.slice(0, 8)}</code>. MR đã có commit mới, review lại trước khi đăng.</p>}
          <div className="mr-summary"><Markdown source={r.summary} /></div>
          {r.findings.length === 0 && <p className="mr-clean">Không thấy vấn đề nào cần sửa.</p>}
          <Findings title="Cần sửa" tone="block" list={block} />
          <Findings title="Gợi ý" tone="sugg" list={sugg} />
          <p className="mr-foot">
            Review commit <code>{r.head_sha.slice(0, 8)}</code> {ago(isoOf(r.at))}
            {r.model ? ` · ${r.model}` : ""}
            {r.posted_at ? ` · đã đăng ${ago(isoOf(r.posted_at))}` : " · chưa đăng"}
          </p>
        </div>
      ) : (
        !busyOf(m) && !m.error && (
          <p className="mr-note">
            {postedNow(m)
              ? `ckit review của commit này đã có trên ${host}${m.posted_by ? ` (đăng bởi ${m.posted_by})` : ""}, xem bình luận bên dưới. Bấm Review MR để chạy lại ở đây.`
              : otherFor(m) === "now"
                ? `${m.other_review!.author} đã review MR này ngoài ckit${m.other_review!.sha ? ` (commit ${m.other_review!.sha})` : ""}, xem bình luận bên dưới. Bấm Review MR nếu muốn AI review thêm.`
                : `Chưa review. AI đọc diff của MR (chỉ đọc, không sửa code), kết quả hiện ở đây để bạn xem trước, rồi mới đăng lên ${host}.`}
          </p>
        )
      )}
      <Comments host={host} q={comments} />
    </section>
  );
}

/** Outcome of the last `/review-pr` run from the Board. The review itself is the comment it posted. */
function CommandResult({ m, host, busy, onAnswer }: { m: MrItem; host: string; busy: boolean; onAnswer: (post: boolean, note: string) => void }) {
  const c = m.command_run;
  const [note, setNote] = useState("");
  if (!c) return busyOf(m) || m.error ? null : <p className="mr-note">Chưa chạy /review-pr từ Board cho MR này. Review đã đăng (nếu có) nằm trong bình luận bên dưới.</p>;
  const stale = c.head_sha !== m.head_sha;
  const ask = c.verdict === "ask";
  return (
    <div className="mr-result">
      {stale && <p className="mr-note is-warn">Lần chạy dưới đây là của commit cũ <code>{c.head_sha.slice(0, 8)}</code>. MR đã có commit mới.</p>}
      <div className={`mr-cmd-res${c.verdict === "fail" ? " is-fail" : ask || c.blocking > 0 ? " is-warn" : ""}`}>
        <div className="mr-cmd-facts">
          <span>{ask ? "Cần bạn duyệt" : c.verdict === "pass" ? "Đã chạy xong" : "Không chạy hết"}</span>
          {c.verdict !== "fail" && <span>{c.blocking > 0 ? `${c.blocking} lỗi chặn merge` : "Không có lỗi chặn"}</span>}
          {c.flags?.dry_run && <span>--dry-run</span>}
          <span>{c.posted ? "Đã đăng comment" : "Chưa đăng comment"}</span>
          <span>{c.merged ? "Đã merge" : c.flags?.no_merge ? "--no-merge" : "Chưa merge"}</span>
        </div>
        {c.message && <div className="mr-summary"><Markdown source={c.message} /></div>}
        {ask && (
          <div className="mr-ask">
            {c.draft && (
              <details open>
                <summary>Comment sẽ đăng lên {host}</summary>
                <div className="mr-c-body"><Markdown source={c.draft} /></div>
              </details>
            )}
            <textarea className="mr-ask-note" rows={2} placeholder="Sửa gì trước khi đăng (không bắt buộc)" value={note} onChange={(e) => setNote(e.target.value)} />
            <div className="mr-actions">
              <button className="primary" disabled={busy} onClick={() => onAnswer(true, note)}>{note.trim() ? "Sửa rồi đăng" : `Đăng lên ${host}`}</button>
              <button disabled={busy} onClick={() => onAnswer(false, "")}>Không đăng</button>
            </div>
          </div>
        )}
      </div>
      <p className="mr-foot">
        /review-pr trên commit <code>{c.head_sha.slice(0, 8)}</code> {ago(isoOf(c.ended))}
        {` · ${Math.max(1, Math.round((c.ended - c.at) / 60))} phút`}
        {c.cost > 0 ? ` · $${c.cost.toFixed(2)}` : ""}
        {" · log "}<code>{c.log.split("/").slice(-1)[0]}</code>
      </p>
    </div>
  );
}

/** Everyone's comments on the MR, oldest first: ckit's own marked, line comments with `path:line`. */
function Comments({ host, q }: { host: string; q: UseQueryResult<MrComment[]> }) {
  const list = q.data ?? [];
  return (
    <section className="mr-comments" aria-label={`Bình luận trên ${host}`}>
      <h4>Bình luận trên {host} {q.data && <span className="mr-group-n">{list.length}</span>}</h4>
      {q.isLoading ? (
        <Loading rows={2} />
      ) : q.error ? (
        <p className="hint hint-err" role="alert">Không tải được bình luận: {serverMessage(q.error as Error)}</p>
      ) : list.length === 0 ? (
        <p className="mr-note">Chưa có bình luận nào.</p>
      ) : (
        <ol className="mr-thread">
          {list.map((c) => (
            <li key={c.id} className={`mr-comment${c.ckit ? " is-ckit" : ""}${c.resolved ? " is-resolved" : ""}`}>
              <div className="mr-c-head">
                <b>{c.author}</b>
                {c.ckit && <span className="mr-c-tag">ckit review</span>}
                {c.path && <code className="mr-f-at">{c.line ? `${c.path}:${c.line}` : c.path}</code>}
                {c.resolved && <span className="mr-c-tag is-ok">đã resolve</span>}
                <span className="mr-c-at" title={new Date(c.at).toLocaleString("vi-VN")}>{ago(c.at)}</span>
              </div>
              <div className="mr-c-body"><Markdown source={c.body} /></div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function Findings({ title, tone, list }: { title: string; tone: "block" | "sugg"; list: MrFinding[] }) {
  if (!list.length) return null;
  return (
    <section className={`mr-group is-${tone}`}>
      <h4>{title} <span className="mr-group-n">{list.length}</span></h4>
      <ol className="mr-findings">
        {list.map((f, i) => (
          <li key={i} className="mr-finding">
            <div className="mr-f-head">
              <code className="mr-f-at">{f.line > 0 ? `${f.path}:${f.line}` : f.path}</code>
              {f.inline && <span className="mr-f-inline">comment tại dòng</span>}
            </div>
            <b className="mr-f-title">{f.title}</b>
            {f.body && <div className="mr-f-body"><Markdown source={f.body} /></div>}
          </li>
        ))}
      </ol>
    </section>
  );
}
