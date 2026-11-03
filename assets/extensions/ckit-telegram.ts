// ckit Telegram bridge for interactive omp sessions (deployed to ~/.omp/agent/extensions).
// Paired to a group with Topics: every linked omp session gets its own topic. Its turn results,
// `ask` questions (buttons) and approval waits go there, and text typed in that topic becomes its
// next user message (or a steer while it runs); `/stop`, `/status` act on that topic's session.
// Paired to a private chat instead: one session at a time.
// Telegram allows one getUpdates poller per bot, so one linked session polls (atomic lock file,
// taken over when its process dies) and hands each update to the right session through files in
// ~/.config/ckit/telegram-omp/. Every session sends its own messages. Only sessions whose cwd is
// under an allowed root (`/tg allow`) link; only the paired Telegram user is obeyed.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, chmodSync, mkdirSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import type { ExtensionAPI, ExtensionContext } from "@oh-my-pi/pi-coding-agent";

const DIR = join(homedir(), ".config", "ckit");
const CONFIG = join(DIR, "telegram-omp.json");
const SHARED = join(DIR, "telegram.json"); // the Board's bot: its token is offered by `/tg setup`
const RUN = join(DIR, "telegram-omp");
const SESS = join(RUN, "sessions"); // <sid>.json: linked sessions
const INBOX = join(RUN, "inbox"); // <sid>.jsonl: updates handed to a session by the poller
const ASKS = join(RUN, "asks"); // <message id>.json: open single-choice questions
const TOPICS = join(RUN, "topics"); // <name>.json: topic thread id + the omp conversation that owns it
const CLAIMS = join(RUN, "claims"); // <name>.json: the live session holding that topic name
const BYSESSION = join(RUN, "bysession"); // <omp session id>.json: that conversation's topic name
const POLLER = join(RUN, "poller.lock");
const HEALTH = join(RUN, "health.json");
const OFFSET = join(RUN, "offset.json");
const API = process.env.CKIT_TELEGRAM_API ?? "https://api.telegram.org";
const MAX_TEXT = 3500; // plain messages (Telegram: 4096 incl. entities)
const MAX_RICH = 32000; // rich messages (Telegram: 32768 UTF-8 characters)
const LINE = "ckit-footline"; // belowEditor widget shared with gw-quota.ts
// A blocked call always reaches the model as an error result (omp emits no tool_result for it): say plainly it is not one.
const FORWARDED = "Not an error: the user is away and the question was forwarded to their Telegram. End your turn now without asking again; their answer arrives as the next user message.";

type Config = { token: string; bot: string; chat_id?: number; user_id?: number; forum?: boolean; pair_code?: string; allow?: string[]; rich?: boolean };
type Reg = { sid: string; pid: number; cwd: string; name: string; chat?: number; thread?: number };
type Item = { kind: "text" | "stop" | "status" | "answer"; text?: string; mid?: number; question?: string };
/** `mid`: the question's own message, set on the "✎ Khác…" prompt that forwards to it. */
type AskFile = { sid: string; question: string; options: string[]; mid?: number };
type Health = "ok" | "down" | "conflict";
type TopicFile = { thread: number; chat?: number; cwd?: string; name?: string; session?: string };
type Choice = { label: string; description?: string };

const readJson = <T>(p: string): T | undefined => {
  try {
    return JSON.parse(readFileSync(p, "utf8")) as T;
  } catch {
    return undefined;
  }
};

/** Atomic (tmp + rename) so another session never reads half a file; 0600 (the config holds the token). */
function writeJson(p: string, data: unknown) {
  mkdirSync(dirname(p), { recursive: true });
  const tmp = `${p}.${process.pid}.tmp`;
  writeFileSync(tmp, JSON.stringify(data, null, 2), { mode: 0o600 });
  chmodSync(tmp, 0o600);
  renameSync(tmp, p);
}

const loadConfig = () => readJson<Config>(CONFIG);
const clip = (s: string, max = MAX_TEXT) => (s.length <= max ? s : `${s.slice(0, max)}\n… (cắt bớt)`);

/** Agent markdown for a rich message: images become links or text (Telegram fetches images; a local path fails the whole send). Code is left alone. */
function richMd(md: string): string {
  return md
    .split(/(```[\s\S]*?```|`[^`\n]*`)/)
    .map((part, i) =>
      i % 2
        ? part
        : part.replace(/!\[([^\]]*)\]\(<?([^)\s>]+)>?(?:\s+"[^"]*")?\)/g, (_m, alt: string, url: string) =>
            /^https?:\/\//i.test(url) ? `[${alt || url}](${url})` : `${alt ? `${alt} ` : ""}\`${url}\``,
          ),
    )
    .join("");
}
const isDm = (c: Config) => c.chat_id !== undefined && !c.forum;
const owner = (c: Config, uid: unknown) => uid === (c.user_id ?? c.chat_id);
const askPath = (mid: number) => join(ASKS, `${mid}.json`);
/** File stem for `name`. Long (e.g. Vietnamese) names would exceed the 255-byte file name limit once encoded: hashed instead. */
function fileKey(name: string): string {
  const enc = encodeURIComponent(name);
  return enc.length <= 200 ? enc : `h-${createHash("sha256").update(name).digest("hex").slice(0, 32)}`;
}
const topicPath = (name: string) => join(TOPICS, `${fileKey(name)}.json`);
const claimPath = (name: string) => join(CLAIMS, `${fileKey(name)}.json`);
const bindPath = (session: string) => join(BYSESSION, `${encodeURIComponent(session)}.json`);
/** Telegram's answer for a topic that no longer exists (anything else, e.g. 429 or missing rights, is retried). */
const GONE = /TOPIC_ID_INVALID|TOPIC_DELETED|thread not found/i;
const MAX_NAME = 128; // Telegram's topic name limit
const normName = (s: string | undefined) => (s ?? "").trim().replace(/\s+/g, " ");
const SUBS: [string, string][] = [
  ["on", "nối phiên này (chọn tên topic)"],
  ["name", "đặt tên topic của phiên này"],
  ["off", "ngắt phiên này"],
  ["setup", "ghép đôi bot: /tg setup [token]"],
  ["help", "hướng dẫn cài bot + group Topics"],
  ["allow", "cho phép thư mục này"],
  ["deny", "bỏ cho phép thư mục này"],
  ["rich", "định dạng tin: /tg rich on|off (off cho app Telegram cũ)"],
  ["reset", "xoá cấu hình Telegram"],
];
const GUIDE =
  "Cài Telegram cho omp:\n" +
  "1. Tạo bot: nhắn @BotFather → /newbot → đặt tên + username (đuôi 'bot') → nhận token.\n" +
  "   Đã có bot ghép sẵn (~/.config/ckit/telegram.json) thì bỏ qua: /tg setup không cần token.\n" +
  "2. Chọn nơi chat:\n" +
  "   • Chat riêng với bot: mỗi lúc một phiên omp.\n" +
  "   • Group nhiều phiên (khuyên dùng): tạo group → Edit → bật Topics → thêm bot, cho làm admin có quyền Manage Topics.\n" +
  "3. Trong omp: /tg setup <token> → nhận mã 6 số.\n" +
  "4. Gửi /start <mã> vào chat riêng với bot, hoặc topic General của group.\n" +
  "Xong: mỗi phiên omp tự có một topic (tên repo; /tg name để đổi), tự mở lại khi ckit . / /resume, đóng khi /tg off.\n" +
  "Repo khác: /tg setup ở repo đó (chỉ thêm quyền, không ghép lại). Ghép chat khác: /tg reset rồi /tg setup.";

function real(p: string): string {
  try {
    return realpathSync(p);
  } catch {
    return resolve(p);
  }
}

/** `cwd` is one of the allowed roots or inside one. */
function allowed(cfg: Config | undefined, cwd: string): boolean {
  const d = real(cwd);
  return (cfg?.allow ?? []).some((r) => d === r || d.startsWith(r.endsWith(sep) ? r : r + sep));
}

async function call(token: string, method: string, body: unknown, ms = 20_000, signal?: AbortSignal): Promise<any> {
  const timeout = AbortSignal.timeout(ms);
  const r = await fetch(`${API}/bot${token}/${method}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
  });
  return r.json();
}

/** Result of a Bot API call, undefined on any failure. */
async function tg(cfg: Config, method: string, body: Record<string, unknown>): Promise<any> {
  try {
    const r = await call(cfg.token, method, body);
    return r?.ok ? r.result : undefined;
  } catch {
    return undefined;
  }
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** Live linked sessions; registrations of dead processes are removed. */
function sessions(): Reg[] {
  let names: string[];
  try {
    names = readdirSync(SESS);
  } catch {
    return [];
  }
  const out: Reg[] = [];
  for (const n of names.filter((n) => n.endsWith(".json"))) {
    const r = readJson<Reg>(join(SESS, n));
    if (!r) continue;
    if (alive(r.pid)) out.push(r);
    else rmSync(join(SESS, n), { force: true });
  }
  return out;
}

function post(sid: string, item: Item) {
  mkdirSync(INBOX, { recursive: true });
  appendFileSync(join(INBOX, `${sid}.jsonl`), `${JSON.stringify(item)}\n`, { mode: 0o600 });
}

function take(sid: string): Item[] {
  const p = join(INBOX, `${sid}.jsonl`);
  const t = `${p}.take`;
  try {
    renameSync(p, t);
  } catch {
    return [];
  }
  const raw = readFileSync(t, "utf8");
  rmSync(t, { force: true });
  return raw.split("\n").filter(Boolean).flatMap((l) => {
    try {
      return [JSON.parse(l) as Item];
    } catch {
      return [];
    }
  });
}

const holder = () => readJson<{ sid: string; pid: number }>(POLLER);

/** Become the poller: create the lock exclusively, replacing it only when its process is dead. */
function takePoller(sid: string): boolean {
  for (let i = 0; i < 2; i++) {
    try {
      mkdirSync(RUN, { recursive: true });
      writeFileSync(POLLER, JSON.stringify({ sid, pid: process.pid }), { flag: "wx", mode: 0o600 });
      return true;
    } catch {
      const h = holder();
      if (h?.sid === sid) return true;
      if (!h || alive(h.pid)) return false; // unreadable = being written right now
      rmSync(POLLER, { force: true });
    }
  }
  return false;
}

/** Current git branch (also on an unborn one); undefined outside a repo or on a detached HEAD. */
function branch(cwd: string): string | undefined {
  try {
    return execFileSync("git", ["-C", cwd, "symbolic-ref", "--short", "-q", "HEAD"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 2000 }).trim() || undefined;
  } catch {
    return undefined;
  }
}

/** Topics saved for chat `chat` (a session of that name reopens them). */
function savedTopics(chat: number | undefined): { name: string; t: TopicFile }[] {
  let names: string[];
  try {
    names = readdirSync(TOPICS);
  } catch {
    return [];
  }
  return names
    .filter((n) => n.endsWith(".json"))
    .flatMap((n) => {
      const t = readJson<TopicFile>(join(TOPICS, n));
      return t && (t.chat === undefined || t.chat === chat) ? [{ name: t.name ?? decodeURIComponent(n.slice(0, -5)), t }] : [];
    });
}

export default function (pi: ExtensionAPI) {
  if (process.env.CKIT_BOARD_STAGE || process.env.CKIT_BOARD_API) return;
  const me = `${process.pid}-${crypto.randomUUID()}`;
  let cur: ExtensionContext | undefined;
  let reg: Reg | undefined; // set while this session is linked
  let timer: ReturnType<ExtensionContext["setInterval"]> | undefined;
  let poller: AbortController | undefined;
  let health: Health | undefined; // this session's own polling result
  let since = 0; // unix seconds: older text was sent while nobody polled; skipped, never run
  let stale = 0;
  let ticks = 0;
  let shown = "";
  let topicBusy = false;
  let topicRetryAt = 0;
  const pending = new Map<number, (answer: string) => void>(); // ask message id → open terminal dialog
  let richWarned = false;
  let menuFor: number | undefined; // chat whose `/` command menu this process already set
  let chosen: string | undefined; // name picked with /tg on or /tg name; else the repo name
  let held: string | undefined; // topic name this session has claimed (claims/<name>.json)

  const saveReg = () => reg && writeJson(join(SESS, `${me}.json`), reg);
  const linked = () => {
    const c = loadConfig();
    return !!reg && c?.chat_id !== undefined && (!c.forum || reg.thread !== undefined);
  };

  async function send(text: string, extra: Record<string, unknown> = {}): Promise<any> {
    const cfg = loadConfig();
    if (!reg || cfg?.chat_id === undefined || (cfg.forum && reg.thread === undefined)) return undefined;
    const thread = cfg.forum ? { message_thread_id: reg.thread } : {};
    return tg(cfg, "sendMessage", { chat_id: cfg.chat_id, ...thread, text: clip(text), disable_web_page_preview: true, ...extra });
  }

  const where = (cfg: Config) => ({ chat_id: cfg.chat_id, ...(cfg.forum ? { message_thread_id: reg?.thread } : {}) });

  /** Markdown to this session's chat: a rich message (tables, code, <details>…), else plain text, else an .md file when too long. */
  async function sendRich(md: string, extra: Record<string, unknown> = {}): Promise<any> {
    const cfg = loadConfig();
    if (!reg || cfg?.chat_id === undefined || (cfg.forum && reg.thread === undefined)) return undefined;
    if (cfg.rich !== false && md.length <= MAX_RICH) {
      const r = await call(cfg.token, "sendRichMessage", { ...where(cfg), rich_message: { markdown: richMd(md) }, ...extra }).catch(() => undefined);
      if (r?.ok) return r.result;
      if (!r) return undefined; // network down: plain would fail too
      // Rejected by the server (syntax, block/nesting limits, media): plain text instead. An old app showing
      // "unsupported message" is never reported back; `/tg rich off` covers that.
      if (!richWarned) cur?.ui.notify(`Telegram từ chối rich message (${r.description}); gửi dạng chữ thường.`, "warning");
      richWarned = true;
    }
    if (md.length <= MAX_TEXT) return tg(cfg, "sendMessage", { ...where(cfg), text: md, disable_web_page_preview: true, ...extra });
    return upload(cfg, `${(reg.name || "omp").replace(/[^\p{L}\p{N}._-]+/gu, "-")}.md`, md, clip(md, 900), extra);
  }

  /** sendDocument: `text` as a file, `caption` under it. */
  async function upload(cfg: Config, file: string, text: string, caption: string, extra: Record<string, unknown> = {}): Promise<any> {
    const fd = new FormData();
    for (const [k, v] of Object.entries({ ...where(cfg), caption, ...extra })) if (v !== undefined) fd.set(k, typeof v === "object" ? JSON.stringify(v) : String(v));
    fd.set("document", new Blob([text], { type: "text/markdown" }), file);
    try {
      const r = await fetch(`${API}/bot${cfg.token}/sendDocument`, { method: "POST", body: fd, signal: AbortSignal.timeout(60_000) });
      const j: any = await r.json();
      return j?.ok ? j.result : undefined;
    } catch {
      return undefined;
    }
  }

  /** Question answered: its buttons become one disabled "✅ answer" (none if the client rejects that). */
  async function markAnswered(cfg: Config, mid: number, answer: string, via = "") {
    const base = { chat_id: cfg.chat_id, message_id: mid };
    const done = { text: `✅ ${via}${answer}`.slice(0, 64), disabled: {}, style: "success" };
    if (!(await tg(cfg, "editMessageReplyMarkup", { ...base, reply_markup: { inline_keyboard: [[done]] } }))) {
      await tg(cfg, "editMessageReplyMarkup", { ...base, reply_markup: { inline_keyboard: [] } });
    }
  }

  /** From the poller: a message into a topic (undefined = General / the private chat). */
  function sendTo(cfg: Config, thread: number | undefined, text: string) {
    void tg(cfg, "sendMessage", { chat_id: cfg.chat_id, ...(thread === undefined ? {} : { message_thread_id: thread }), text: clip(text) });
  }

  // ── footer ────────────────────────────────────────────────────────────────

  function remoteHealth(): Health | undefined {
    const h = readJson<{ state: Health; at: number }>(HEALTH);
    const p = holder();
    return h && p && alive(p.pid) && Date.now() - h.at < 45_000 ? h.state : undefined;
  }

  /** Green = receiving, yellow = connecting / pairing / creating the topic, red = no connection. */
  function footer(cfg = loadConfig()) {
    if (!cur) return;
    let out: string | undefined;
    if (reg && cfg) {
      const bot = ` @${cfg.bot}`;
      const where = cfg.forum && reg.thread !== undefined ? ` · ${reg.name}` : "";
      const h = poller ? health : remoteHealth();
      const fg = (c: "success" | "warning" | "error", s: string) => cur!.ui.theme.fg(c, s);
      if (cfg.chat_id === undefined) out = fg("warning", `◐ TG${bot} chờ /start ${cfg.pair_code ?? ""}`);
      else if (cfg.forum && reg.thread === undefined) out = fg("warning", `◐ TG${bot} đang tạo topic`);
      else if (h === undefined) out = fg("warning", `◐ TG${bot}${where} đang kết nối`);
      else if (h === "ok") out = fg("success", `● TG${bot}${where}`);
      else out = fg("error", `● TG${bot}${where} ${h === "conflict" ? "bị tiến trình khác chiếm bot" : "mất kết nối"}`);
    }
    if ((out ?? "") === shown) return;
    shown = out ?? "";
    footline(cur.ui, out);
  }

  /** The Telegram status goes on gw-quota's line (widget keys are global across extensions). */
  function footline(ui: ExtensionContext["ui"], text: string | undefined) {
    const parts: Record<string, string> = ((globalThis as any).__ckitFootline ??= {});
    if (text) parts.tg = text;
    else delete parts.tg;
    const line = [parts.quota, parts.tg].filter(Boolean).join("   ");
    ui.setWidget(LINE, line ? [line] : undefined, { placement: "belowEditor" });
  }

  function setHealth(h: Health) {
    health = h;
    writeJson(HEALTH, { state: h, at: Date.now() });
    footer();
  }

  // ── linking ───────────────────────────────────────────────────────────────

  /** Link this session; an error text when it cannot. `force`: take a private chat from another session. */
  async function link(ctx: ExtensionContext, force: boolean): Promise<string | undefined> {
    cur = ctx;
    const cfg = loadConfig();
    if (!cfg) return "Telegram chưa cấu hình: /tg setup";
    if (isDm(cfg)) {
      const h = holder();
      if (force && h && h.sid !== me) rmSync(POLLER, { force: true });
      if (!takePoller(me)) return `Phiên omp khác đang nối chat riêng với @${cfg.bot} (/tg on để lấy về). Muốn nhiều phiên cùng lúc: ghép đôi với group có Topics (/tg setup).`;
    }
    reg ??= { sid: me, pid: process.pid, cwd: ctx.cwd, name: chosen ?? boundName(ctx) ?? basename(ctx.cwd) };
    saveReg();
    timer ??= ctx.setInterval(tick, 700);
    maintain();
    return undefined;
  }

  function unlink(why?: string) {
    const cfg = loadConfig();
    if (cfg?.forum && reg?.thread !== undefined && reg.chat === cfg.chat_id) void tg(cfg, "closeForumTopic", { chat_id: cfg.chat_id, message_thread_id: reg.thread });
    stopPoll();
    if (holder()?.sid === me) rmSync(POLLER, { force: true });
    if (held !== undefined) release(held);
    held = undefined;
    rmSync(join(SESS, `${me}.json`), { force: true });
    rmSync(join(INBOX, `${me}.jsonl`), { force: true });
    if (timer && cur) cur.clearTimer(timer);
    timer = undefined;
    reg = undefined;
    footer();
    if (why) cur?.ui.notify(why, "info");
  }

  function tick() {
    if (!reg) return;
    for (const it of take(me)) onItem(it);
    if (++ticks % 4 === 0) maintain();
  }

  /** Poller takeover, this session's topic, footer. */
  function maintain() {
    const cfg = loadConfig();
    if (!reg || !cfg) return;
    if (poller && holder()?.sid !== me) stopPoll();
    if (!poller && takePoller(me)) startPoll();
    if (isDm(cfg) && !poller) return unlink("Telegram đã chuyển sang phiên omp khác.");
    if (cfg.chat_id !== undefined && reg.thread !== undefined && reg.chat !== cfg.chat_id) {
      reg.thread = undefined; // paired to another chat since
      saveReg();
    }
    if (cfg.forum && cfg.chat_id !== undefined && reg.thread === undefined && !topicBusy && Date.now() >= topicRetryAt) void ensureTopic(cfg);
    footer(cfg);
  }

  /** The omp conversation shown in this session (changes on /new, /resume). */
  function convo(ctx = cur): string | undefined {
    try {
      return ctx?.sessionManager.getSessionId() || undefined;
    } catch {
      return undefined;
    }
  }

  const boundName = (ctx = cur) => {
    const s = convo(ctx);
    return s ? readJson<{ name: string }>(bindPath(s))?.name : undefined;
  };

  /** Remember `name` as this conversation's topic: `ckit .` / `/resume` come back to it. */
  function bind(name: string) {
    const s = convo();
    if (s) writeJson(bindPath(s), { name });
  }

  /** Hold topic name `name` across processes: exclusive file, taken over when its holder died. */
  function claim(name: string): boolean {
    const p = claimPath(name);
    for (let i = 0; i < 2; i++) {
      try {
        mkdirSync(CLAIMS, { recursive: true });
        writeFileSync(p, JSON.stringify({ sid: me, pid: process.pid }), { flag: "wx", mode: 0o600 });
        return true;
      } catch {
        const h = readJson<{ sid: string; pid: number }>(p);
        if (h?.sid === me) return true;
        if (!h || alive(h.pid)) return false; // unreadable = being written right now
        rmSync(p, { force: true });
      }
    }
    return false;
  }

  function release(name: string) {
    if (readJson<{ sid: string }>(claimPath(name))?.sid === me) rmSync(claimPath(name), { force: true });
  }

  /** `name` (already claimed) becomes this session's; the previous one is released. */
  function hold(name: string) {
    if (held !== undefined && held !== name) release(held);
    held = name;
  }

  /** This session's topic: the one its conversation used before (`ckit .`, /resume), else `<repo>` (`<repo> 2`… only while
   *  a live session holds it). A free saved topic is reopened, whichever conversation used it last; a missing one is created. */
  async function ensureTopic(cfg: Config) {
    if (!reg || !cur) return;
    topicBusy = true;
    try {
      const ses = convo();
      const bound = boundName();
      const base = chosen ?? bound ?? basename(reg.cwd);
      let name: string | undefined;
      for (let i = 1; i <= 50 && name === undefined; i++) {
        const n = i === 1 ? base : `${base} ${i}`;
        if (claim(n)) name = n;
      }
      if (name === undefined) {
        topicRetryAt = Date.now() + 10_000;
        return;
      }
      hold(name);
      reg.name = name;
      saveReg();
      const tf = topicPath(name);
      const saved = readJson<TopicFile>(tf);
      let thread = saved && (saved.chat === undefined || saved.chat === cfg.chat_id) ? saved.thread : undefined;
      if (thread !== undefined) {
        const r = await call(cfg.token, "reopenForumTopic", { chat_id: cfg.chat_id, message_thread_id: thread }).catch(() => undefined);
        if (!r) {
          topicRetryAt = Date.now() + 10_000; // network: retry, never create a duplicate
          return;
        }
        if (!r.ok && !/NOT_MODIFIED/i.test(String(r.description))) {
          if (!GONE.test(String(r.description))) {
            topicRetryAt = Date.now() + 60_000; // rate limit, missing rights…: the topic may well exist
            return;
          }
          thread = undefined;
        }
      }
      if (thread === undefined) {
        const r = await call(cfg.token, "createForumTopic", { chat_id: cfg.chat_id, name }).catch(() => undefined);
        if (!r?.ok) {
          topicRetryAt = Date.now() + 60_000;
          cur.ui.notify(`Telegram: chưa tạo được topic "${name}" (bot phải là admin có quyền Manage Topics): ${r?.description ?? "không gọi được Telegram"}`, "error");
          return;
        }
        thread = r.result.message_thread_id as number;
      }
      if (!reg || convo() !== ses) return; // unlinked or switched conversation meanwhile: the next tick starts over
      writeJson(tf, { thread, chat: cfg.chat_id, cwd: reg.cwd, name, session: ses } satisfies TopicFile);
      bind(name);
      reg.thread = thread;
      reg.chat = cfg.chat_id;
      saveReg();
      footer(cfg);
      void send(`🟢 omp ${name} đã nối. Gõ tin trong topic này để giao việc, /stop để dừng, /status để xem trạng thái.`);
    } finally {
      topicBusy = false;
    }
  }

  // ── polling (one session) ─────────────────────────────────────────────────

  function startPoll() {
    poller = new AbortController();
    health = undefined;
    since = Math.floor(Date.now() / 1000) - 15;
    void poll(poller.signal);
  }

  function stopPoll() {
    poller?.abort();
    poller = undefined;
  }

  async function poll(signal: AbortSignal) {
    let conflictTold = false;
    let first = true;
    while (!signal.aborted) {
      if (holder()?.sid !== me) return stopPoll();
      const cfg = loadConfig();
      if (!cfg) return stopPoll();
      const offset = readJson<{ offset: number }>(OFFSET)?.offset ?? 0;
      let res: any;
      try {
        // The first call returns at once: the footer turns green without waiting a long-poll.
        res = await call(cfg.token, "getUpdates", { offset, timeout: first ? 0 : 20, allowed_updates: ["message", "callback_query"] }, 30_000, signal);
      } catch {
        if (signal.aborted) return;
        setHealth("down");
        await sleep(5000, undefined, { signal }).catch(() => {});
        continue;
      }
      if (!res?.ok) {
        setHealth(res?.error_code === 409 ? "conflict" : "down");
        if (res?.error_code === 409 && !conflictTold) {
          conflictTold = true;
          cur?.ui.notify("Bot Telegram đang bị tiến trình khác nhận tin (dashboard Board dùng chung bot?). Đang thử lại.", "warning");
        }
        await sleep(5000, undefined, { signal }).catch(() => {});
        continue;
      }
      first = false;
      conflictTold = false;
      setHealth("ok");
      if (cfg.chat_id !== undefined && menuFor !== cfg.chat_id) {
        menuFor = cfg.chat_id;
        void tg(cfg, "setMyCommands", {
          commands: [
            { command: "stop", description: "Dừng lượt đang chạy của phiên này" },
            { command: "status", description: "Phiên đang chạy hay rảnh, model nào" },
          ],
          scope: { type: "chat", chat_id: cfg.chat_id },
        });
      }
      for (const u of res.result as any[]) {
        writeJson(OFFSET, { offset: Math.max(offset, u.update_id + 1) });
        try {
          await dispatch(loadConfig() ?? cfg, u); // pairing in this batch changes it
        } catch (e) {
          cur?.ui.notify(`Telegram: ${(e as Error).message}`, "error");
        }
      }
      if (stale > 0) {
        sendTo(cfg, undefined, `Bỏ qua ${stale} tin cũ (gửi khi chưa có phiên omp nào nối).`);
        stale = 0;
      }
    }
  }

  /** Hand an item to session `sid`, or say it is gone. */
  function route(cfg: Config, sid: string, thread: number | undefined, item: Item) {
    if (sessions().some((s) => s.sid === sid)) post(sid, item);
    else sendTo(cfg, thread, "Phiên omp đó đã đóng. Mở omp trong repo đó để nối lại.");
  }

  async function dispatch(cfg: Config, u: any) {
    const cb = u.callback_query;
    if (cb) {
      if (cfg.chat_id === undefined || cb.message?.chat?.id !== cfg.chat_id || !owner(cfg, cb.from?.id)) return;
      const mid = cb.message.message_id as number;
      const thread = cb.message.is_topic_message ? (cb.message.message_thread_id as number) : undefined;
      const a = readJson<AskFile>(askPath(mid));
      const data = String(cb.data);
      await tg(cfg, "answerCallbackQuery", { callback_query_id: cb.id });
      if (!a) return sendTo(cfg, thread, "Câu hỏi này đã hết hạn, gõ câu trả lời thay vì bấm nút.");
      if (data === "o") {
        // "✎ Khác…": a reply-to prompt; its answer goes to the same question.
        const p = await tg(cfg, "sendMessage", {
          chat_id: cfg.chat_id,
          ...(thread === undefined ? {} : { message_thread_id: thread }),
          text: `✎ Trả lời khác cho: ${a.question}`.slice(0, MAX_TEXT),
          reply_markup: { force_reply: true, input_field_placeholder: "Câu trả lời của bạn" },
        });
        if (p?.message_id) writeJson(askPath(p.message_id), { ...a, mid } satisfies AskFile);
        return;
      }
      const choice = a.options[Number(data.replace(/^a:/, ""))];
      if (choice === undefined) return;
      rmSync(askPath(mid), { force: true });
      await markAnswered(cfg, mid, choice);
      return route(cfg, a.sid, thread, { kind: "answer", mid, text: choice, question: a.question });
    }
    const m = u.message;
    if (!m) return;
    const text = String(m.text ?? "").trim().replace(/^(\/\w+)@\w+/, "$1"); // `/start@bot 123` in groups
    if (cfg.chat_id === undefined) return pair(cfg, m, text);
    if (m.chat?.id !== cfg.chat_id || !owner(cfg, m.from?.id)) return;
    if ((m.date ?? 0) < since) {
      stale++;
      return;
    }
    // In a topic every message carries the topic's id; plain text also "replies" to the topic's first message.
    const thread = cfg.forum && m.is_topic_message ? (m.message_thread_id as number) : undefined;
    if (!text) {
      if (m.photo || m.document || m.voice || m.video || m.audio) sendTo(cfg, thread, "Chỉ nhận tin dạng chữ.");
      return; // service messages (topic renamed…)
    }
    const rid = m.reply_to_message?.message_id;
    const a = rid !== undefined && rid !== thread ? readJson<AskFile>(askPath(rid)) : undefined;
    if (a) {
      const origin = a.mid ?? rid;
      rmSync(askPath(rid), { force: true });
      rmSync(askPath(origin), { force: true });
      await markAnswered(cfg, origin, text);
      return route(cfg, a.sid, thread, { kind: "answer", mid: origin, text, question: a.question });
    }
    let target: string | undefined;
    if (cfg.forum) {
      if (thread === undefined) return general(cfg);
      target = sessions().find((s) => s.chat === cfg.chat_id && s.thread === thread)?.sid;
      if (!target) return sendTo(cfg, thread, "Phiên omp của topic này đã đóng. Mở omp trong repo đó để nối lại.");
    } else {
      target = holder()?.sid; // a private chat has exactly one linked session: the poller
    }
    if (!target) return;
    if (text === "/stop") return post(target, { kind: "stop" });
    if (text === "/status" || text === "/start") return post(target, { kind: "status" });
    post(target, { kind: "text", text, mid: m.message_id });
  }

  /** General topic: which sessions are linked, where to type. */
  function general(cfg: Config) {
    const list = sessions().filter((s) => s.chat === cfg.chat_id && s.thread !== undefined).map((s) => `• ${s.name}`).join("\n");
    sendTo(cfg, undefined, list ? `Phiên omp đang nối:\n${list}\nGõ trong topic của phiên muốn giao việc.` : "Chưa có phiên omp nào nối. Mở omp trong repo đã /tg allow.");
  }

  async function pair(cfg: Config, m: any, text: string) {
    if (!cfg.pair_code || text !== `/start ${cfg.pair_code}`) return;
    const chat = m.chat?.id as number;
    const forum = m.chat?.type === "supergroup" && m.chat?.is_forum === true;
    if (m.chat?.type !== "private" && !forum) {
      await tg({ ...cfg, chat_id: chat }, "sendMessage", { chat_id: chat, text: "Group này chưa bật Topics. Bật Topics (Edit → Topics), cho bot làm admin có quyền Manage Topics, rồi gửi lại lệnh." });
      return;
    }
    const { pair_code: _code, ...keep } = cfg;
    writeJson(CONFIG, { ...keep, chat_id: chat, user_id: m.from?.id, forum });
    await tg({ ...cfg, chat_id: chat }, "sendMessage", {
      chat_id: chat,
      ...(forum && m.is_topic_message ? { message_thread_id: m.message_thread_id } : {}),
      text: forum ? "✅ Đã ghép đôi. Mỗi phiên omp có một topic riêng trong group này." : "✅ Đã ghép đôi. Gõ tin để giao việc, /stop để dừng, /status để xem trạng thái.",
    });
    cur?.ui.notify(`Telegram đã ghép đôi @${cfg.bot}${forum ? " (group Topics)" : ""}`, "info");
  }

  // ── items handed to this session ──────────────────────────────────────────

  /** `mid`: the Telegram message it came from, acknowledged with a reaction (text if reactions are off). */
  function deliver(text: string, mid?: number) {
    const idle = cur?.isIdle() ?? true;
    pi.sendUserMessage(text); // idle → new turn; running → steer
    const cfg = loadConfig();
    if (mid === undefined || cfg?.chat_id === undefined) return;
    void tg(cfg, "setMessageReaction", { chat_id: cfg.chat_id, message_id: mid, reaction: [{ type: "emoji", emoji: idle ? "👀" : "✍" }] }).then((ok) => {
      if (!ok) void send(idle ? "▶️ Đã gửi vào phiên." : "↪️ Đã chen vào lượt đang chạy.");
    });
  }

  function onItem(it: Item) {
    if (it.kind === "stop") {
      if (cur && !cur.isIdle()) {
        cur.abort();
        void send("⏹ Đã dừng lượt đang chạy.");
      } else void send("Không có gì đang chạy.");
      return;
    }
    if (it.kind === "status") {
      const s = cur?.sessionManager.getSessionName() ?? "";
      void send(`📍 ${reg?.name ?? ""}${s ? ` · ${s}` : ""}\n${cur?.isIdle() === false ? "Đang chạy" : "Đang rảnh"} · ${cur?.model?.id ?? "?"}`);
      return;
    }
    if (it.kind === "answer") {
      const open = it.mid === undefined ? undefined : pending.get(it.mid);
      if (open) return open(it.text ?? "");
      return deliver(`Trả lời câu hỏi "${it.question}": ${it.text}`);
    }
    deliver(it.text ?? "", it.mid);
  }

  // ── ask ───────────────────────────────────────────────────────────────────

  /** Send the question; the message id + options of a single-choice one (buttons). */
  async function forwardAsk(qs: any[]): Promise<{ mid: number; question: string; options: string[] } | undefined> {
    const name = reg?.name ?? "";
    if (qs.length === 1 && !qs[0].multi && qs[0].options?.length > 0) {
      const q = qs[0];
      const question = String(q.question);
      const opts: { label: string; description?: string }[] = q.options.map((o: any) => ({ label: String(o.label), description: o.description ? String(o.description) : undefined }));
      const options = opts.map((o) => o.label);
      const rec = typeof q.recommended === "number" ? q.recommended : -1;
      const lines = opts.map((o, i) => `${i + 1}. **${o.label}**${i === rec ? " ⭐" : ""}${o.description ? ` — ${o.description}` : ""}`);
      const md = `❓ **[${name}]** ${question}\n\n${lines.join("\n")}\n\n_Bấm nút, hoặc reply tin này để trả lời khác._`;
      const btn = (o: { label: string }, i: number) => ({ text: `${i + 1}. ${o.label}${i === rec ? " ⭐" : ""}`.slice(0, 64), callback_data: `a:${i}`, ...(i === rec ? { style: "primary" } : {}) });
      const pairs = opts.every((o) => o.label.length <= 18);
      const kb: any[][] = [];
      opts.forEach((o, i) => (pairs && i % 2 ? kb[kb.length - 1].push(btn(o, i)) : kb.push([btn(o, i)])));
      kb.push([{ text: "✎ Khác…", callback_data: "o" }]);
      const sent = await sendRich(md, { reply_markup: { inline_keyboard: kb } });
      if (!sent?.message_id) return undefined;
      pruneAsks();
      writeJson(askPath(sent.message_id), { sid: me, question, options } satisfies AskFile);
      return { mid: sent.message_id, question, options };
    }
    const body = qs.map((q, i) => `${i + 1}. **${q.question}**${q.options?.length ? `\n   ${q.options.map((o: any) => o.label).join(" · ")}` : ""}`).join("\n");
    await sendRich(`❓ **[${name}]** Agent hỏi:\n\n${body}\n\n_Trả lời bằng một tin, ghi theo số câu._`);
    return undefined;
  }

  function pruneAsks() {
    try {
      for (const n of readdirSync(ASKS)) {
        const p = join(ASKS, n);
        if (Date.now() - statSync(p).mtimeMs > 3 * 86_400_000) rmSync(p, { force: true });
      }
    } catch {}
  }

  // ── omp events ────────────────────────────────────────────────────────────

  pi.on("session_start", async (_e, ctx) => {
    if (ctx.mode !== "tui" || !ctx.hasUI) return;
    cur = ctx;
    const cfg = loadConfig();
    if (cfg && allowed(cfg, ctx.cwd)) await link(ctx, false).catch(() => undefined);
  });

  pi.on("session_shutdown", async () => unlink());

  // /new, /resume, fork: follow the conversation to its own topic (closing the one it leaves).
  pi.on("session_switch", async (_e, ctx) => {
    cur = ctx;
    chosen = undefined;
    shown = ""; // omp dropped every widget on the switch: redraw (quota included)
    footer();
    const cfg = loadConfig();
    if (!reg || !cfg?.forum || cfg.chat_id === undefined) return;
    // A new conversation (no topic of its own yet) stays in the current topic; /resume goes to the conversation's own.
    const next = boundName(ctx) ?? reg.name;
    if (reg.thread !== undefined && next === reg.name) {
      const tf = topicPath(next);
      const saved = readJson<TopicFile>(tf);
      if (saved) writeJson(tf, { ...saved, session: convo() } satisfies TopicFile);
      bind(next);
      return;
    }
    if (reg.thread !== undefined && reg.chat === cfg.chat_id) void tg(cfg, "closeForumTopic", { chat_id: cfg.chat_id, message_thread_id: reg.thread });
    if (held !== undefined) release(held);
    held = undefined;
    reg.thread = undefined;
    reg.name = next;
    saveReg();
    topicRetryAt = 0;
    maintain();
  });

  pi.on("agent_end", async (e) => {
    if (e.willContinue || !linked()) return;
    const last = [...e.messages].reverse().find((m: any) => m.role === "assistant") as any;
    const text = Array.isArray(last?.content)
      ? last.content.filter((c: any) => c.type === "text").map((c: any) => c.text).join("\n").trim()
      : "";
    const tail = last?.stopReason === "aborted" ? "\n(⏹ đã dừng)" : last?.stopReason === "error" ? `\n(⚠️ lỗi: ${last.errorMessage ?? "?"})` : "";
    await sendRich(`✅ **[${reg?.name ?? ""}]**\n\n${text || "Xong lượt."}${tail}`);
  });

  // Single choice: the terminal dialog and the Telegram buttons race, the first answer wins.
  // Waiting on an omp dialog does not count against the 30 s tool_call budget; Telegram alone would.
  pi.on("tool_call", async (e, ctx) => {
    if (e.toolName !== "ask" || !linked()) return;
    const qs: any[] = Array.isArray((e as any).input?.questions) ? (e as any).input.questions : [];
    const q = await forwardAsk(qs);
    let got: string | undefined;
    if (q && ctx.hasUI) {
      const dialog = new AbortController();
      let fromTelegram: string | undefined;
      pending.set(q.mid, (a) => {
        fromTelegram = a;
        dialog.abort();
      });
      const local = await ctx.ui.select(`${q.question}  (Esc: trả lời sau / qua Telegram)`, q.options, { signal: dialog.signal }).catch(() => undefined);
      pending.delete(q.mid);
      got = fromTelegram ?? local;
      if (fromTelegram === undefined && local !== undefined) {
        rmSync(askPath(q.mid), { force: true }); // answered in the terminal: retire the buttons
        const cfg = loadConfig();
        if (cfg?.chat_id !== undefined) void markAnswered(cfg, q.mid, local, "terminal: ");
      }
    }
    const text = got === undefined ? FORWARDED : `Not an error: the user answered this question: ${got}`;
    return { block: true, reason: text };
  });

  pi.on("tool_approval_requested", async (e) => {
    if (linked()) await send(`⚠️ [${reg?.name ?? ""}] Tool ${e.toolName} đang chờ duyệt ở terminal${e.reason ? `: ${e.reason}` : ""}.`);
  });

  // ── topic name ────────────────────────────────────────────────────────────

  /** Name choices: omp session title, repo · branch, repo, this repo's earlier topics. Names of other live sessions are left out. */
  function suggestions(ctx: ExtensionContext): Choice[] {
    const repo = basename(ctx.cwd);
    const taken = new Set(sessions().filter((s) => s.sid !== me).map((s) => s.name));
    const old = savedTopics(loadConfig()?.chat_id).filter(({ name, t }) => (t.cwd ? t.cwd === ctx.cwd : name === repo || name.startsWith(`${repo} `)));
    const reopen = new Set(old.map((o) => o.name));
    const out = new Map<string, string>();
    const add = (raw: string | undefined, why: string) => {
      const n = normName(raw).slice(0, MAX_NAME);
      if (n && n !== reg?.name && !taken.has(n) && !out.has(n)) out.set(n, reopen.has(n) ? `${why} · mở lại topic cũ, giữ lịch sử` : why);
    };
    add(ctx.sessionManager.getSessionName(), "tiêu đề phiên omp");
    const b = branch(ctx.cwd);
    if (b && !["main", "master", "develop", "dev"].includes(b)) add(`${repo} · ${b}`, "nhánh git");
    add(repo, "tên repo");
    for (const o of old) add(o.name, "đã dùng trước đây");
    return [...out].map(([label, description]) => ({ label, description }));
  }

  // Completion runs on every keystroke: reuse the list (git + directory scans) for a few seconds.
  let completions: { at: number; list: Choice[] } | undefined;

  const OTHER = "✎ Tên khác…";

  /** Picker over `suggestions` plus free input; undefined = keep the current name. */
  async function pickName(ctx: ExtensionContext): Promise<string | undefined> {
    const now = reg?.name ?? chosen ?? basename(ctx.cwd);
    const pick = await ctx.ui.select(`Tên topic Telegram cho phiên này (Esc: giữ "${now}")`, [...suggestions(ctx), { label: OTHER, description: "tự nhập" }]);
    if (pick !== OTHER) return pick;
    return normName(await ctx.ui.input("Tên topic", now)) || undefined;
  }

  /** Name this session: an earlier topic of that name is reopened (history kept), else the current topic is renamed. */
  async function setName(raw: string): Promise<{ ok: boolean; msg: string }> {
    const name = normName(raw);
    if (!name || name.length > MAX_NAME) return { ok: false, msg: `Tên phải từ 1 đến ${MAX_NAME} ký tự.` };
    if (name === reg?.name) return { ok: true, msg: `Đang dùng tên "${name}".` };
    if (topicBusy) return { ok: false, msg: "Đang tạo topic, thử lại sau vài giây." };
    if (!claim(name)) return { ok: false, msg: `"${name}" đang là tên của phiên omp khác.` };
    const fail = (msg: string) => {
      if (name !== held) release(name);
      return { ok: false, msg };
    };
    const cfg = loadConfig();
    if (!reg || !cfg?.forum || cfg.chat_id === undefined || reg.thread === undefined) {
      // No topic yet (private chat, not paired, or still creating): the name is used when one is made.
      chosen = name;
      hold(name);
      if (reg) {
        reg.name = name;
        saveReg();
        footer(cfg);
        if (cfg?.forum) bind(name);
      }
      return { ok: true, msg: `Tên phiên: "${name}".` };
    }
    const chat = cfg.chat_id;
    const from = reg.thread;
    const saved = readJson<TopicFile>(topicPath(name));
    if (saved && saved.thread !== from && (saved.chat === undefined || saved.chat === chat)) {
      const r = await call(cfg.token, "reopenForumTopic", { chat_id: chat, message_thread_id: saved.thread }).catch(() => undefined);
      if (!r) return fail("Không gọi được Telegram, thử lại sau.");
      if (r.ok || /NOT_MODIFIED/i.test(String(r.description))) {
        if (!reg) return fail("Phiên đã ngắt Telegram.");
        void tg(cfg, "closeForumTopic", { chat_id: chat, message_thread_id: from });
        chosen = name;
        hold(name);
        writeJson(topicPath(name), { ...saved, chat, cwd: reg.cwd, name, session: convo() } satisfies TopicFile);
        bind(name);
        reg.thread = saved.thread;
        reg.name = name;
        saveReg();
        footer(cfg);
        void send(`🟢 omp ${name} đã nối lại topic này.`);
        return { ok: true, msg: `Đã chuyển sang topic cũ "${name}" (giữ lịch sử).` };
      }
      if (!GONE.test(String(r.description))) return fail(`Telegram: ${r.description}`);
      // The saved topic was deleted: rename the current one instead.
    }
    const r = await call(cfg.token, "editForumTopic", { chat_id: chat, message_thread_id: from, name }).catch(() => undefined);
    if (!r?.ok && !/NOT_MODIFIED/i.test(String(r?.description))) return fail(`Telegram: ${r?.description ?? "không gọi được"}`);
    if (!reg) return fail("Phiên đã ngắt Telegram.");
    const prev = reg.name;
    chosen = name;
    hold(name);
    writeJson(topicPath(name), { thread: from, chat, cwd: reg.cwd, name, session: convo() } satisfies TopicFile);
    if (readJson<TopicFile>(topicPath(prev))?.thread === from) rmSync(topicPath(prev), { force: true });
    bind(name);
    reg.name = name;
    saveReg();
    footer(cfg);
    return { ok: true, msg: `Đã đổi tên topic "${prev}" thành "${name}".` };
  }

  // ── /tg ───────────────────────────────────────────────────────────────────

  pi.registerCommand("tg", {
    description: "Telegram: /tg [setup [token] | on | off | name [tên] | allow [dir] | deny [dir] | reset]",
    getArgumentCompletions: (prefix) => {
      const m = /^name\s+(.*)$/s.exec(prefix);
      if (m) {
        if (!cur) return null;
        if (!completions || Date.now() - completions.at > 5000) completions = { at: Date.now(), list: suggestions(cur) };
        const q = m[1].toLowerCase();
        return completions.list
          .filter((c) => c.label.toLowerCase().includes(q))
          .map((c) => ({ value: `name ${c.label}`, label: c.label, description: c.description }));
      }
      if (/\s/.test(prefix)) return null;
      return SUBS.filter(([s]) => s.startsWith(prefix)).map(([s, d]) => ({ value: s, label: s, description: d }));
    },
    handler: async (args, ctx) => {
      cur = ctx;
      const sub = args.trim().split(/\s+/)[0] ?? "";
      const arg = args.trim().slice(sub.length).trim();
      const cfg = loadConfig();
      if (sub === "setup") {
        const token = arg || cfg?.token || readJson<Config>(SHARED)?.token || "";
        if (!token) return ctx.ui.notify(`Chưa có token bot.\n${GUIDE}`, "warning");
        // Already paired with this bot: just allow this repo and link; re-pairing would move every session.
        if (cfg?.chat_id !== undefined && token === cfg.token) {
          writeJson(CONFIG, { ...cfg, allow: [...new Set([...(cfg.allow ?? []), real(ctx.cwd)])] });
          const err = await link(ctx, true);
          return ctx.ui.notify(err ?? `Đã ghép đôi sẵn @${cfg.bot}: phiên này đã nối. Ghép đôi lại chat khác: /tg reset rồi /tg setup.`, err ? "warning" : "info");
        }
        const meRes = await call(token, "getMe", {}).catch(() => undefined);
        if (!meRes?.ok) return ctx.ui.notify("Token không hợp lệ hoặc không gọi được Telegram.", "error");
        const bot = String(meRes.result.username);
        const code = String(Math.floor(100000 + Math.random() * 900000));
        writeJson(CONFIG, { token, bot, pair_code: code, allow: [...new Set([...(cfg?.allow ?? []), real(ctx.cwd)])] });
        // Skip what was already queued for the bot: it must never run as a prompt here.
        const last = (await call(token, "getUpdates", { offset: -1, timeout: 0 }).catch(() => undefined))?.result?.at(-1)?.update_id;
        writeJson(OFFSET, { offset: typeof last === "number" ? last + 1 : 0 });
        await link(ctx, true);
        return ctx.ui.notify(
          `Ghép đôi @${bot}: gửi  /start ${code}\n` +
            "• vào topic General của group đã bật Topics (bot là admin có quyền Manage Topics): mỗi phiên omp một topic\n" +
            "• hoặc vào chat riêng với bot: mỗi lúc một phiên",
          "info",
        );
      }
      if (sub === "on") {
        if (!cfg) return ctx.ui.notify("Chưa cấu hình: /tg setup", "warning");
        if (!allowed(cfg, ctx.cwd)) return ctx.ui.notify(`Thư mục này chưa được phép: /tg allow để thêm ${real(ctx.cwd)}`, "warning");
        if (ctx.hasUI) {
          const name = await pickName(ctx);
          const r = name ? await setName(name) : undefined;
          if (r && !r.ok) ctx.ui.notify(r.msg, "warning");
        }
        const err = await link(ctx, true);
        return ctx.ui.notify(err ?? `Telegram @${cfg.bot} nối với phiên này${reg ? ` · "${reg.name}"` : ""}.`, err ? "warning" : "info");
      }
      if (sub === "name") {
        if (!reg) return ctx.ui.notify("Phiên này chưa nối Telegram: /tg on", "warning");
        const name = arg || (ctx.hasUI ? await pickName(ctx) : undefined);
        if (!name) return;
        const r = await setName(name);
        return ctx.ui.notify(r.msg, r.ok ? "info" : "warning");
      }
      if (sub === "off") return unlink("Đã ngắt Telegram khỏi phiên này.");
      if (sub === "rich") {
        if (!cfg) return ctx.ui.notify("Chưa cấu hình: /tg setup", "warning");
        if (arg && arg !== "on" && arg !== "off") return ctx.ui.notify("Dùng: /tg rich on|off", "warning");
        const rich = arg ? arg === "on" : cfg.rich === false;
        writeJson(CONFIG, { ...cfg, rich });
        richWarned = false;
        return ctx.ui.notify(rich ? "Tin gửi dạng rich message (bảng, code, tiêu đề). App Telegram cũ chỉ hiện 'tin không hỗ trợ': khi đó /tg rich off." : "Tin gửi dạng chữ thường.", "info");
      }
      if (sub === "allow" || sub === "deny") {
        if (!cfg) return ctx.ui.notify("Chưa cấu hình: /tg setup", "warning");
        const here = real(arg || ctx.cwd);
        const allow = (cfg.allow ?? []).filter((r) => r !== here);
        if (sub === "allow") allow.push(here);
        writeJson(CONFIG, { ...cfg, allow });
        if (sub === "deny" && reg && !allowed({ ...cfg, allow }, ctx.cwd)) unlink();
        return ctx.ui.notify(`Thư mục được phép:\n${allow.join("\n") || "(trống)"}`, "info");
      }
      if (sub === "reset") {
        unlink();
        rmSync(CONFIG, { force: true });
        rmSync(RUN, { recursive: true, force: true });
        return ctx.ui.notify("Đã xoá cấu hình Telegram của omp.", "info");
      }
      if (sub === "help") return ctx.ui.notify(GUIDE, "info");
      if (!cfg) return ctx.ui.notify(`Telegram chưa cấu hình.\n${GUIDE}`, "info");
      const where = cfg.chat_id === undefined ? `chờ /start ${cfg.pair_code}` : cfg.forum ? "group Topics (mỗi phiên một topic)" : "chat riêng (mỗi lúc một phiên)";
      const all = sessions();
      ctx.ui.notify(
        `@${cfg.bot} · ${where}\n` +
          `Phiên này: ${reg ? `đã nối · "${reg.name}"${reg.thread !== undefined ? " (topic)" : ""} — đổi tên: /tg name` : "chưa nối (/tg on)"} · đang nối: ${all.length} phiên\n` +
          `Thư mục được phép: ${(cfg.allow ?? []).join(", ") || "(trống)"}\n` +
          `Định dạng: ${cfg.rich === false ? "chữ thường (/tg rich on)" : "rich message (/tg rich off nếu app Telegram cũ)"}\n` +
          "Quyền tool theo phiên omp; muốn không bị hỏi duyệt thì chạy omp --approval-mode yolo.",
        "info",
      );
    },
  });
}
