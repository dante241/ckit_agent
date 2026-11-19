// The boss's own CloudGO PMS account (global, not per project), set up like
// Telegram: header button with a status dot + a dialog to connect, re-test or
// disconnect. Contract: GET /api/board/pms/status, POST /api/board/pms
// {base_url, username, password, access_key?}, POST /api/board/pms/key
// {access_key}, POST /api/board/pms/test, POST /api/board/pms/off, GET|POST
// /api/board/pms/import-mcp. The password and Access Key are sent once and
// never come back.
import { useEffect, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type PmsMcpPreview, type PmsStatus } from "./api";
import { isMissing } from "./ui";
import { serverMessage } from "./Steps";

/** Shared with `usePmsConfigured` (PmsPicker): one cache entry for the status. */
const KEY = ["pms-status"];
const DEFAULT_BASE = "https://pms.cloudgo.vn";

function usePmsStatus() {
  return useQuery({ queryKey: KEY, queryFn: api.pmsStatus, staleTime: 60_000, retry: false });
}

/** Board header button (status dot) + the PMS dialog. */
export function PmsButton() {
  const [open, setOpen] = useState(false);
  const q = usePmsStatus();
  if (isMissing(q.error)) return null;
  const on = q.data?.configured === true;
  const title = on ? `PMS đã kết nối: ${q.data?.display_name ?? ""}` : "PMS chưa kết nối";
  return (
    <>
      <button className="co-btn" onClick={() => setOpen(true)} aria-haspopup="dialog" title={title}>
        PMS
        <span className={`tg-dot is-${on ? "on" : "off"}`} aria-label={title} />
      </button>
      {open && createPortal(<PmsDialog onClose={() => setOpen(false)} />, document.body)}
    </>
  );
}

/** "Kết nối PMS" in Tạo task when no account is set (or `label`, e.g. "Thêm Access Key"): opens the same dialog. */
export function PmsConnectLink({ label = "Kết nối PMS" }: { label?: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" className="ghost bd-pms-open" onClick={() => setOpen(true)} aria-haspopup="dialog">{label}</button>
      {open && createPortal(<PmsDialog onClose={() => setOpen(false)} />, document.body)}
    </>
  );
}

function PmsDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const q = usePmsStatus();
  const [base, setBase] = useState(DEFAULT_BASE);
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [key, setKey] = useState("");
  const [keyOpen, setKeyOpen] = useState(false);
  const [tested, setTested] = useState(false);
  const [preview, setPreview] = useState<PmsMcpPreview | null>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Opened over Tạo task: Escape closes only this dialog.
      e.stopPropagation();
      onClose();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  // A new/removed account invalidates every cached PMS search and item.
  const save = (d: PmsStatus) => {
    qc.setQueryData(KEY, d);
    qc.removeQueries({ queryKey: ["pms-search"] });
    qc.removeQueries({ queryKey: ["pms-item"] });
    qc.removeQueries({ queryKey: ["pms-report"] });
  };
  const connect = useMutation({ mutationFn: api.pmsConnect, onSuccess: (d) => { save(d); setPass(""); setKey(""); } });
  const test = useMutation({ mutationFn: api.pmsTest, onSuccess: (d) => { save(d); setTested(true); } });
  const off = useMutation({ mutationFn: api.pmsOff, onSuccess: (d) => { save(d); setTested(false); } });
  const peek = useMutation({ mutationFn: api.pmsMcpPreview, onSuccess: setPreview });
  const imp = useMutation({ mutationFn: api.pmsMcpImport, onSuccess: (d) => { save(d); setPreview(null); } });
  const saveKey = useMutation({ mutationFn: api.pmsKey, onSuccess: (d) => { save(d); setKey(""); setKeyOpen(false); } });
  const err = [connect.error, test.error, off.error, peek.error, imp.error, saveKey.error].find(Boolean) as Error | undefined;
  const busy = connect.isPending || imp.isPending;

  const d = q.data;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    // Rendered in a portal under Tạo task's form: keep the submit from bubbling to it.
    e.stopPropagation();
    if (user.trim() && pass && !busy) connect.mutate({ base_url: base.trim(), username: user.trim(), password: pass, access_key: key.trim() || undefined });
  };
  const submitKey = (e: FormEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (key.trim() && !saveKey.isPending) saveKey.mutate(key.trim());
  };
  const disconnect = () => {
    if (window.confirm("Ngắt kết nối PMS? Board sẽ xoá tài khoản đã lưu, muốn lấy việc từ PMS phải đăng nhập lại.")) off.mutate();
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal tg-modal" role="dialog" aria-modal="true" aria-label="PMS" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <strong>PMS</strong>
          <button type="button" className="icon-btn ghost" onClick={onClose} aria-label="Đóng">✕</button>
        </div>
        <div className="modal-body">
          {q.isLoading ? (
            <p className="faint tg-text">Đang tải…</p>
          ) : q.error ? (
            <p className="hint hint-err tg-text">{serverMessage(q.error as Error)}</p>
          ) : !d?.configured ? (
            <form className="tg-form" onSubmit={submit}>
              <p className="tg-text">Đăng nhập tài khoản CloudGO PMS của bạn để chọn task và ticket ngay khi tạo task. Board chỉ đọc, không sửa gì trên PMS.</p>
              <label className="field">
                <span className="field-label">Địa chỉ PMS</span>
                <input type="text" inputMode="url" autoComplete="url" spellCheck={false} value={base} onChange={(e) => setBase(e.target.value)} placeholder={DEFAULT_BASE} />
              </label>
              <label className="field">
                <span className="field-label">Tên đăng nhập</span>
                <input type="text" autoComplete="username" spellCheck={false} value={user} onChange={(e) => setUser(e.target.value)} autoFocus />
              </label>
              <label className="field">
                <span className="field-label">Mật khẩu</span>
                <input type="password" autoComplete="current-password" value={pass} onChange={(e) => setPass(e.target.value)} />
              </label>
              <label className="field">
                <span className="field-label">Access Key <span className="faint">(tuỳ chọn, để mở report)</span></span>
                <input type="password" autoComplete="off" spellCheck={false} value={key} onChange={(e) => setKey(e.target.value)} placeholder="Trên PMS: Tuỳ chọn cá nhân → Access Key" />
              </label>
              <div className="tg-actions">
                <button type="submit" className="primary" disabled={!user.trim() || !pass || busy}>
                  {connect.isPending ? "Đang đăng nhập…" : "Kết nối"}
                </button>
              </div>
              <div className="pms-mcp">
                {preview?.found ? (
                  <>
                    <p className="tg-text">
                      Đăng nhập bằng tài khoản <strong>{preview.user}</strong> tại {preview.base_url}{preview.report ? ", kèm Access Key để mở report" : ""}? Mật khẩu lấy từ cấu hình MCP pms của omp và không hiển thị ở đây.
                    </p>
                    <div className="tg-actions">
                      <button type="button" className="primary" onClick={() => imp.mutate()} disabled={busy}>
                        {imp.isPending ? "Đang đăng nhập…" : "Dùng tài khoản này"}
                      </button>
                      <button type="button" onClick={() => setPreview(null)} disabled={imp.isPending}>Huỷ</button>
                    </div>
                  </>
                ) : (
                  <>
                    {preview && !preview.found ? <p className="hint tg-text">Không tìm thấy tài khoản PMS trong cấu hình MCP pms của omp.</p> : null}
                    <button type="button" className="ghost" onClick={() => peek.mutate()} disabled={peek.isPending || busy}>
                      {peek.isPending ? "Đang đọc…" : "Dùng tài khoản từ MCP pms"}
                    </button>
                  </>
                )}
              </div>
            </form>
          ) : (
            <>
              <p className="tg-on"><span className="tg-dot is-on" aria-hidden="true" /> Đã kết nối: {d.display_name || d.user}</p>
              <p className="tg-text muted">
                Đang dùng tài khoản <strong>{d.user}</strong> tại <a href={d.base_url ?? undefined} target="_blank" rel="noreferrer">{d.base_url}</a>. Bấm Tạo task là danh sách task và ticket PMS hiện ra để bạn chọn.
              </p>
              {tested && !test.isPending ? <p className="hint hint-ok tg-text">Đăng nhập lại thành công.</p> : null}
              <div className="pms-key">
                {d.report && !keyOpen ? (
                  <p className="tg-text">
                    Report: <strong>đã có Access Key</strong>, bạn mở được report PMS trong tab Report khi tạo task.{" "}
                    <button type="button" className="link" onClick={() => setKeyOpen(true)}>Đổi</button>
                    {" · "}
                    <button type="button" className="link" onClick={() => saveKey.mutate("")} disabled={saveKey.isPending}>Xoá</button>
                  </p>
                ) : (
                  <form className="tg-form" onSubmit={submitKey}>
                    <p className="tg-text">
                      {d.report ? "Nhập Access Key mới." : "Muốn mở report PMS khi tạo task? Thêm Access Key của bạn (trên PMS: Tuỳ chọn cá nhân → Access Key)."}
                    </p>
                    <label className="field">
                      <span className="field-label">Access Key</span>
                      <input type="password" autoComplete="off" spellCheck={false} value={key} onChange={(e) => setKey(e.target.value)} autoFocus={keyOpen} />
                    </label>
                    <div className="tg-actions">
                      <button type="submit" className="primary" disabled={!key.trim() || saveKey.isPending}>
                        {saveKey.isPending ? "Đang kiểm tra…" : "Lưu Access Key"}
                      </button>
                      {keyOpen ? <button type="button" onClick={() => { setKeyOpen(false); setKey(""); }}>Huỷ</button> : null}
                    </div>
                  </form>
                )}
              </div>
              <div className="tg-actions">
                <button type="button" onClick={() => { setTested(false); test.mutate(); }} disabled={test.isPending}>
                  {test.isPending ? "Đang kiểm tra…" : "Kiểm tra"}
                </button>
                <button type="button" className="danger" onClick={disconnect} disabled={off.isPending}>Ngắt kết nối</button>
              </div>
            </>
          )}
          {err ? <p className="hint hint-err tg-text">{serverMessage(err)}</p> : null}
        </div>
      </div>
    </div>
  );
}
