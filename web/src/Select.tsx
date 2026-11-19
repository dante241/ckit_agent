// The one dropdown of the dashboard. Short lists stay a native <select>
// (keyboard, mobile pickers, screen readers for free). Long ones (more than
// 8 options, `searchable`, or a `remote` search) become an ARIA combobox: same
// 36px trigger, a popover portalled to <body> (never clipped by a modal or a
// popover) with a search box, diacritic-insensitive Vietnamese matching
// ("phuong" finds "Phường"), highlighted matches, ↑↓ Enter Esc, type-ahead,
// a checkmark on the chosen row, and a bottom sheet under 480px.
// `MultiSelect` is the checkbox variant (search + Chọn tất cả / Bỏ chọn).
import {
  Children, isValidElement, useEffect, useId, useLayoutEffect, useMemo, useRef, useState,
  type ChangeEvent, type CSSProperties, type KeyboardEvent, type ReactElement, type ReactNode, type RefObject, type SelectHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";

export type SelectOpt = { value: string; label: string; group?: string; disabled?: boolean };

type Props = Omit<SelectHTMLAttributes<HTMLSelectElement>, "size" | "className" | "value"> & {
  value?: string;
  /** 30px instead of 36px (filter rows, chips). */
  compact?: boolean;
  /** Highlight as an active filter. */
  active?: boolean;
  /** Classes for the wrapper (width / flex rules). */
  className?: string;
  /** Force the searchable combobox (default: more than 8 options or `remote`). */
  searchable?: boolean;
  /** Server-side search for lists the client only holds a part of. */
  remote?: (q: string) => Promise<SelectOpt[]>;
  /** Label of the current value when it is not among the options (found by `remote`). */
  valueLabel?: string;
  /** The picked option (with its label), for callers that keep remote picks. */
  onOption?: (o: SelectOpt) => void;
  children: ReactNode;
};

const AUTO_SEARCH = 8;

// ── Vietnamese folding + highlight ─────────────────────────────────────────
const foldChar = (c: string) => c.normalize("NFD").replace(/\p{M}/gu, "").replace(/[đĐ]/g, "d").toLowerCase();

/** Lowercase, diacritics and `đ` folded away: "Phường Đức" → "phuong duc". */
export function fold(s: string): string {
  return Array.from(s, foldChar).join("");
}

/** `label` with the first diacritic-insensitive match of `q` wrapped in <mark>. */
export function highlight(label: string, q: string): ReactNode {
  const fq = fold(q.trim());
  if (!fq) return label;
  const chars = Array.from(label);
  let folded = "";
  const at: number[] = []; // folded index → original char index
  chars.forEach((c, i) => {
    const f = foldChar(c);
    for (let k = 0; k < f.length; k++) at.push(i);
    folded += f;
  });
  const hit = folded.indexOf(fq);
  if (hit < 0) return label;
  const from = at[hit] ?? 0;
  const to = (at[hit + fq.length - 1] ?? chars.length - 1) + 1;
  return (
    <>
      {chars.slice(0, from).join("")}
      <mark>{chars.slice(from, to).join("")}</mark>
      {chars.slice(to).join("")}
    </>
  );
}

const matches = (o: SelectOpt, q: string) => !q.trim() || fold(o.label).includes(fold(q.trim()));

// ── <option>/<optgroup> children → options ─────────────────────────────────
function textOf(n: ReactNode): string {
  if (n == null || typeof n === "boolean") return "";
  if (typeof n === "string" || typeof n === "number") return String(n);
  if (Array.isArray(n)) return n.map(textOf).join("");
  if (isValidElement(n)) return textOf((n.props as { children?: ReactNode }).children);
  return "";
}

function optionsOf(children: ReactNode, group?: string): SelectOpt[] {
  const out: SelectOpt[] = [];
  Children.forEach(children, (c) => {
    if (!isValidElement(c)) return;
    const el = c as ReactElement<{ value?: string | number; label?: string; disabled?: boolean; children?: ReactNode }>;
    if (el.type === "option") {
      out.push({ value: String(el.props.value ?? textOf(el.props.children)), label: textOf(el.props.children), group, disabled: el.props.disabled });
    } else if (el.type === "optgroup") {
      out.push(...optionsOf(el.props.children, el.props.label));
    } else if (el.props.children) {
      out.push(...optionsOf(el.props.children, group)); // fragments
    }
  });
  return out;
}

const fakeChange = (value: string) => ({ target: { value }, currentTarget: { value } }) as unknown as ChangeEvent<HTMLSelectElement>;

// ── Popover placement (fixed, follows the trigger) ─────────────────────────
function usePlacement(open: boolean, trigger: RefObject<HTMLElement | null>) {
  const [pos, setPos] = useState<{ style: CSSProperties; sheet: boolean } | null>(null);
  useLayoutEffect(() => {
    if (!open) return setPos(null);
    const place = () => {
      const t = trigger.current;
      if (!t) return;
      if (window.innerWidth <= 480) return setPos({ sheet: true, style: {} });
      const r = t.getBoundingClientRect();
      const width = Math.min(Math.max(r.width, 260), 440, window.innerWidth - 16);
      const left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8));
      const below = window.innerHeight - r.bottom - 12;
      const above = r.top - 12;
      const up = below < 260 && above > below;
      const maxHeight = Math.min(380, up ? above : below);
      setPos({
        sheet: false,
        style: up ? { left, width, bottom: window.innerHeight - r.top + 4, maxHeight } : { left, width, top: r.bottom + 4, maxHeight },
      });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, trigger]);
  return pos;
}

/** Close on a pointer press outside the trigger and the popover. */
function useOutside(open: boolean, refs: RefObject<HTMLElement | null>[], close: () => void) {
  const cb = useRef(close);
  cb.current = close;
  useEffect(() => {
    if (!open) return;
    const on = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!refs.some((r) => r.current?.contains(t))) cb.current();
    };
    document.addEventListener("mousedown", on);
    return () => document.removeEventListener("mousedown", on);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
}

function Chevron() {
  return (
    <svg className="select-chev" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

const PRINTABLE = /^\S$/u;

// ── Select ─────────────────────────────────────────────────────────────────
export function Select({ compact, active, className, children, searchable, remote, valueLabel, onOption, ...rest }: Props) {
  const opts = useMemo(() => optionsOf(children), [children]);
  const cls = ["select", compact ? "select-sm" : "", active ? "is-on" : "", className ?? ""].filter(Boolean).join(" ");
  if (!(searchable ?? (opts.length > AUTO_SEARCH || !!remote))) {
    return (
      <span className={cls}>
        <select {...rest}>{children}</select>
        <Chevron />
      </span>
    );
  }
  return <Combo cls={cls} opts={opts} remote={remote} valueLabel={valueLabel} onOption={onOption} {...rest} />;
}

type ComboProps = Omit<Props, "children" | "compact" | "active" | "className" | "searchable"> & { cls: string; opts: SelectOpt[] };

function Combo({ cls, opts, remote, valueLabel, onOption, value = "", onChange, disabled, id, ...rest }: ComboProps) {
  const ariaLabel = rest["aria-label"];
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [act, setAct] = useState(0);
  const [found, setFound] = useState<SelectOpt[]>([]);
  const [busy, setBusy] = useState(false);
  const trigRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const pos = usePlacement(open, trigRef);
  const close = (focus = true) => {
    setOpen(false);
    setQ("");
    setFound([]);
    if (focus) trigRef.current?.focus();
  };
  useOutside(open, [trigRef, popRef], () => close(false));

  // Remote search: debounced, only for 2+ characters.
  useEffect(() => {
    if (!open || !remote || q.trim().length < 2) return setFound([]);
    let live = true;
    setBusy(true);
    const h = setTimeout(() => {
      remote(q.trim())
        .then((r) => live && setFound(r))
        .catch(() => live && setFound([]))
        .finally(() => live && setBusy(false));
    }, 300);
    return () => {
      live = false;
      clearTimeout(h);
      setBusy(false);
    };
  }, [q, open, remote]);

  const local = opts.filter((o) => matches(o, q));
  const extra = found.filter((f) => !opts.some((o) => o.value === f.value)).map((f) => ({ ...f, group: "Tìm trên PMS" }));
  const rows = [...local, ...extra];
  const pickable = rows.filter((o) => !o.disabled);
  const current = opts.find((o) => o.value === value) ?? (value && valueLabel ? { value, label: valueLabel } : undefined);

  useEffect(() => {
    if (!open) return;
    inputRef.current?.focus();
    const i = pickable.findIndex((o) => o.value === value);
    setAct(i < 0 ? 0 : i);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
  useEffect(() => setAct(0), [q]);
  useEffect(() => {
    popRef.current?.querySelector(`[data-i="${act}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [act, open]);

  const pick = (o: SelectOpt) => {
    onChange?.(fakeChange(o.value));
    onOption?.(o);
    close();
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const n = pickable.length;
    if (e.key === "ArrowDown") setAct((a) => (n ? (a + 1) % n : 0));
    else if (e.key === "ArrowUp") setAct((a) => (n ? (a - 1 + n) % n : 0));
    else if (e.key === "Home") setAct(0);
    else if (e.key === "End") setAct(Math.max(0, n - 1));
    else if (e.key === "Enter") {
      if (pickable[act]) pick(pickable[act]);
    } else if (e.key === "Escape") {
      // Only this popover closes, not the dialog it sits in.
      e.stopPropagation();
      e.nativeEvent.stopPropagation();
      close();
    } else if (e.key === "Tab") return close(false);
    else return;
    e.preventDefault();
  };
  const onTrigKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setOpen(true);
    } else if (PRINTABLE.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
      // Type-ahead: the first letter opens the list already filtered by it.
      e.preventDefault();
      setQ(e.key);
      setOpen(true);
    }
  };

  let lastGroup: string | undefined;
  return (
    <span className={cls}>
      <button
        ref={trigRef}
        id={id}
        type="button"
        className="cbx-trigger"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => (open ? close() : setOpen(true))}
        onKeyDown={onTrigKey}
      >
        <span className="cbx-value">{current?.label ?? ""}</span>
      </button>
      <Chevron />
      {open && pos
        ? createPortal(
            <>
              {pos.sheet ? <div className="cbx-scrim" onMouseDown={() => close(false)} /> : null}
              <div ref={popRef} className={`cbx-pop${pos.sheet ? " is-sheet" : ""}`} style={pos.style}>
                {pos.sheet && ariaLabel ? <div className="cbx-sheet-title">{ariaLabel}</div> : null}
                <input
                  ref={inputRef}
                  className="cbx-search"
                  type="search"
                  value={q}
                  placeholder={remote ? "Tìm (gõ 2 ký tự để tìm trên PMS)" : "Tìm…"}
                  aria-label={`Tìm ${ariaLabel ?? ""}`.trim()}
                  aria-controls={listId}
                  aria-autocomplete="list"
                  aria-activedescendant={pickable[act] ? `${listId}-${act}` : undefined}
                  onChange={(e) => setQ(e.target.value)}
                  onKeyDown={onKey}
                />
                <ul id={listId} className="cbx-list" role="listbox" aria-label={ariaLabel}>
                  {rows.map((o) => {
                    const i = pickable.indexOf(o);
                    const head = o.group !== lastGroup && o.group ? <li className="cbx-group" role="presentation">{o.group}</li> : null;
                    lastGroup = o.group;
                    return (
                      <FragmentRow key={`${o.group ?? ""}|${o.value}`} head={head}>
                        <li
                          id={i >= 0 ? `${listId}-${i}` : undefined}
                          data-i={i}
                          role="option"
                          aria-selected={o.value === value}
                          aria-disabled={o.disabled || undefined}
                          className={`cbx-opt${i === act ? " is-act" : ""}${o.value === value ? " is-sel" : ""}`}
                          onMouseMove={() => i >= 0 && i !== act && setAct(i)}
                          onMouseDown={(e) => e.preventDefault()}
                          onClick={() => !o.disabled && pick(o)}
                        >
                          <span className="cbx-check" aria-hidden="true">{o.value === value ? "✓" : ""}</span>
                          <span className="cbx-label">{highlight(o.label, q)}</span>
                        </li>
                      </FragmentRow>
                    );
                  })}
                  {busy ? <li className="cbx-note" role="presentation">Đang tìm trên PMS…</li> : null}
                  {!busy && rows.length === 0 ? <li className="cbx-note" role="presentation">Không tìm thấy</li> : null}
                </ul>
              </div>
            </>,
            document.body,
          )
        : null}
    </span>
  );
}

function FragmentRow({ head, children }: { head: ReactNode; children: ReactNode }) {
  return (
    <>
      {head}
      {children}
    </>
  );
}

// ── MultiSelect ────────────────────────────────────────────────────────────
export function MultiSelect({ options, value, onChange, label, empty, compact, className }: {
  options: SelectOpt[];
  value: string[];
  onChange: (v: string[]) => void;
  /** Accessible name; also the sheet title on phones. */
  label: string;
  /** Trigger text when nothing is chosen. */
  empty: string;
  compact?: boolean;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [act, setAct] = useState(0);
  const trigRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const pos = usePlacement(open, trigRef);
  const close = (focus = true) => {
    setOpen(false);
    setQ("");
    if (focus) trigRef.current?.focus();
  };
  useOutside(open, [trigRef, popRef], () => close(false));
  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);
  useEffect(() => setAct(0), [q]);
  useEffect(() => {
    popRef.current?.querySelector(`[data-i="${act}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [act, open]);

  const rows = options.filter((o) => matches(o, q));
  const toggle = (v: string) => onChange(value.includes(v) ? value.filter((x) => x !== v) : [...value, v]);
  const shown = value.map((v) => options.find((o) => o.value === v)?.label ?? v);
  const text = shown.length === 0 ? empty : shown.length <= 2 ? shown.join(", ") : `${shown.length} đã chọn`;
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const n = rows.length;
    if (e.key === "ArrowDown") setAct((a) => (n ? (a + 1) % n : 0));
    else if (e.key === "ArrowUp") setAct((a) => (n ? (a - 1 + n) % n : 0));
    else if (e.key === "Enter") {
      if (rows[act]) toggle(rows[act].value);
    } else if (e.key === "Escape") {
      e.stopPropagation();
      e.nativeEvent.stopPropagation();
      close();
    } else if (e.key === "Tab") return close(false);
    else return;
    e.preventDefault();
  };
  const cls = ["select", compact ? "select-sm" : "", value.length ? "is-on" : "", className ?? ""].filter(Boolean).join(" ");
  return (
    <span className={cls}>
      <button
        ref={trigRef}
        type="button"
        className="cbx-trigger"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={label}
        onClick={() => (open ? close() : setOpen(true))}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        <span className="cbx-value">{text}</span>
      </button>
      <Chevron />
      {open && pos
        ? createPortal(
            <>
              {pos.sheet ? <div className="cbx-scrim" onMouseDown={() => close(false)} /> : null}
              <div ref={popRef} className={`cbx-pop${pos.sheet ? " is-sheet" : ""}`} style={pos.style}>
                {pos.sheet ? <div className="cbx-sheet-title">{label}</div> : null}
                <input
                  ref={inputRef}
                  className="cbx-search"
                  type="search"
                  value={q}
                  placeholder="Tìm…"
                  aria-label={`Tìm ${label}`}
                  aria-controls={listId}
                  aria-autocomplete="list"
                  aria-activedescendant={rows[act] ? `${listId}-${act}` : undefined}
                  onChange={(e) => setQ(e.target.value)}
                  onKeyDown={onKey}
                />
                <div className="cbx-bulk">
                  <button type="button" className="link" onClick={() => onChange([...new Set([...value, ...rows.map((o) => o.value)])])}>
                    Chọn tất cả{q.trim() ? " (đang lọc)" : ""}
                  </button>
                  <button type="button" className="link" onClick={() => onChange(q.trim() ? value.filter((v) => !rows.some((o) => o.value === v)) : [])} disabled={!value.length}>
                    Bỏ chọn
                  </button>
                </div>
                <ul id={listId} className="cbx-list" role="listbox" aria-label={label} aria-multiselectable="true">
                  {rows.map((o, i) => {
                    const on = value.includes(o.value);
                    return (
                      <li
                        key={o.value}
                        id={`${listId}-${i}`}
                        data-i={i}
                        role="option"
                        aria-selected={on}
                        className={`cbx-opt cbx-multi${i === act ? " is-act" : ""}${on ? " is-sel" : ""}`}
                        onMouseMove={() => i !== act && setAct(i)}
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => toggle(o.value)}
                      >
                        <span className="cbx-box" aria-hidden="true">{on ? "✓" : ""}</span>
                        <span className="cbx-label">{highlight(o.label, q)}</span>
                      </li>
                    );
                  })}
                  {rows.length === 0 ? <li className="cbx-note" role="presentation">Không tìm thấy</li> : null}
                </ul>
                {pos.sheet ? (
                  <button type="button" className="primary cbx-done" onClick={() => close()}>Xong</button>
                ) : null}
              </div>
            </>,
            document.body,
          )
        : null}
    </span>
  );
}
