// gw-quota — omp/pi extension: show the active provider's usage quota on its
// own line under the editor via ctx.ui.setWidget(placement:"belowEditor").
//
// Two sources, picked from ctx.model.provider:
//
// 1. 9router ai-gateway providers (cloudgo-cc / cloudgo-cx): look up the
//    provider's { baseUrl, apiKey } in ~/.omp/agent/models.yml and poll
//    `${baseUrl}/quota`. Auth sends BOTH x-api-key (Anthropic path) and
//    Authorization: Bearer (Codex path). The endpoint returns only used
//    percentages for the refilling per-key bucket (`used_percent` over
//    `window_seconds`) and the calendar day (`daily_used_percent`) — the
//    bucket refills continuously, so no reset clock is shown:
//    `Quota 4h ▰▰▱▱▱ 40% · Day ▰▰▱▱▱ 47%`.
// 2. `anthropic` with a Claude subscription (OAuth login): run omp's own
//    `omp usage --provider anthropic --json` (the extension ctx exposes no
//    usage API) and show every window it reports, incl. model-scoped weekly
//    buckets the built-in `usage` status-line segment drops:
//    `Claude 5h ▰▱▱▱▱ 2% 4h52m · 7d Fable ▱▱▱▱▱ 0% 16h`. These windows roll
//    over, so each carries its reset countdown. An API-key `anthropic` login
//    yields no report → hidden.
//
// Meter + color track the percentage (green/yellow/red). Unlimited keys and
// other providers are hidden. All failures are swallowed (session never
// affected). `/gwquota` forces a refresh and reports detail via a notification.
import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@oh-my-pi/pi-coding-agent";

const STATUS_KEY = "gw-quota";
const REFRESH_MS = 180_000; // poll at most once every 3 minutes

export interface ProviderConf {
  baseUrl: string;
  apiKey: string;
}

export interface Quota {
  budget_source: string; // "override" | "team_policy" | "none"
  used_percent: number | null; // null when the key is unlimited
  window_seconds?: number; // bucket refill period (e.g. 14400 = 4h)
  exceeded: boolean;
  daily_budget_source?: string; // same enum; absent on older gateways
  daily_used_percent?: number | null;
  daily_exceeded?: boolean;
}

// One budget normalized for display.
export interface Budget {
  label: string; // "4h" | "Day" | "5h" | "7d Fable"
  source: string;
  pct: number;
  exceeded: boolean;
  resetsAt?: number; // ms epoch; only for windows that roll over (subscription)
}

// Subset of `omp usage --json` output this extension reads.
interface UsageLimit {
  id?: string;
  scope?: { windowId?: string; tier?: string };
  window?: { id?: string; resetsAt?: number };
  amount?: { usedFraction?: number };
}
export interface UsageJson {
  reports?: Array<{ provider?: string; metadata?: { email?: string }; limits?: UsageLimit[] }>;
}

// Indent-aware scan of the `providers:` block in ~/.omp/agent/models.yml,
// capturing each provider's baseUrl + apiKey. Intentionally tiny (no YAML dep).
function loadProviders(): Record<string, ProviderConf> {
  const out: Record<string, ProviderConf> = {};
  let text: string;
  try {
    text = readFileSync(join(homedir(), ".omp/agent/models.yml"), "utf8");
  } catch {
    return out;
  }
  let inProviders = false;
  let cur: string | undefined;
  for (const line of text.split(/\r?\n/)) {
    if (/^providers:\s*$/.test(line)) {
      inProviders = true;
      continue;
    }
    if (!inProviders) continue;
    if (/^\S/.test(line)) break; // dedent to a new top-level key ends providers:
    const header = line.match(/^ {2}([A-Za-z0-9_.-]+):\s*$/);
    if (header) {
      cur = header[1];
      out[cur] = { baseUrl: "", apiKey: "" };
      continue;
    }
    if (!cur) continue;
    const base = line.match(/^ {4}baseUrl:\s*(\S+)\s*$/);
    if (base) {
      out[cur].baseUrl = base[1];
      continue;
    }
    const key = line.match(/^ {4}apiKey:\s*(\S+)\s*$/);
    if (key) out[cur].apiKey = key[1];
  }
  return out;
}

// Type guard: only our ai-gateway providers expose a /quota endpoint; a bare
// `/llm/` path or the gateway host both qualify.
function isGatewayProvider(p: ProviderConf | undefined): p is ProviderConf {
  return !!p && !!p.baseUrl && !!p.apiKey && /ai-gateway|\/llm\//.test(p.baseUrl);
}

// "4h" / "30m" / "1d" from the window length; "" when unknown.
function windowLabel(sec: number | undefined): string {
  if (!sec || sec <= 0) return "";
  if (sec % 86400 === 0) return `${sec / 86400}d`;
  if (sec % 3600 === 0) return `${sec / 3600}h`;
  return `${Math.round(sec / 60)}m`;
}

// A budget applies only when the server resolved one (override or team policy)
// AND reported a percentage; otherwise that budget is unlimited → skipped.
// Empty result = key is fully unlimited → nothing to show.
function budgets(q: Quota | undefined): Budget[] {
  if (!q) return [];
  const out: Budget[] = [];
  if (q.budget_source !== "none" && q.used_percent != null) {
    out.push({
      label: windowLabel(q.window_seconds),
      source: q.budget_source,
      pct: Math.round(q.used_percent),
      exceeded: q.exceeded,
    });
  }
  if (q.daily_budget_source && q.daily_budget_source !== "none" && q.daily_used_percent != null) {
    out.push({
      label: "Day",
      source: q.daily_budget_source,
      pct: Math.round(q.daily_used_percent),
      exceeded: !!q.daily_exceeded,
    });
  }
  return out;
}

async function fetchQuota(p: ProviderConf): Promise<Quota | undefined> {
  const url = p.baseUrl.replace(/\/+$/, "") + "/quota";
  const res = await fetch(url, {
    headers: { "x-api-key": p.apiKey, authorization: "Bearer " + p.apiKey },
  });
  if (!res.ok) return undefined;
  return (await res.json()) as Quota;
}

// Every window `omp usage` reports for the first Anthropic subscription
// account, in API order (5h first). Tier-scoped weekly buckets are labeled by
// tier ("7d Fable") because they only count usage of that model family.
export function claudeBudgets(j: UsageJson | undefined): Budget[] {
  const rep = j?.reports?.find((r) => r.provider === "anthropic");
  const out: Budget[] = [];
  for (const l of rep?.limits ?? []) {
    const f = l.amount?.usedFraction;
    if (typeof f !== "number") continue;
    const win = l.scope?.windowId ?? l.window?.id ?? "";
    const tier = l.scope?.tier ? l.scope.tier[0].toUpperCase() + l.scope.tier.slice(1) : "";
    out.push({
      label: [win, tier].filter(Boolean).join(" "),
      source: rep?.metadata?.email ?? "anthropic",
      pct: Math.round(f * 100),
      exceeded: f >= 1,
      resetsAt: l.window?.resetsAt,
    });
  }
  return out;
}

// omp's compiled binary is process.execPath; under `bun cli.js` fall back to PATH.
function ompBin(): string {
  return /(^|[\\/])omp(\.exe)?$/.test(process.execPath) ? process.execPath : "omp";
}

function fetchClaudeUsage(): Promise<UsageJson | undefined> {
  return new Promise((resolve) => {
    execFile(
      ompBin(),
      // --no-extensions: `omp usage` would otherwise load every extension (this one included) per poll.
      ["usage", "--provider", "anthropic", "--json", "--no-extensions"],
      { timeout: 10_000, maxBuffer: 4 << 20 },
      (err, stdout) => {
        if (err) return resolve(undefined);
        try {
          resolve(JSON.parse(stdout) as UsageJson);
        } catch {
          resolve(undefined);
        }
      },
    );
  });
}

// "4h52m" / "16h" / "2d3h" until the window resets.
export function fmtReset(resetsAt: number, now = Date.now()): string {
  const mins = Math.max(0, Math.round((resetsAt - now) / 60_000));
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  if (h < 24) return mins % 60 ? `${h}h${mins % 60}m` : `${h}h`;
  return h % 24 ? `${Math.floor(h / 24)}d${h % 24}h` : `${Math.floor(h / 24)}d`;
}

// A quota source for the active provider: widget title + fetcher.
interface Source {
  title: string;
  fetch(): Promise<Budget[] | undefined>; // undefined = fetch failed → keep last shown
}

function sourceFor(provider: string | undefined, providers: Record<string, ProviderConf>): Source | undefined {
  const prov = providers[provider ?? ""];
  if (isGatewayProvider(prov)) {
    return { title: "Quota", fetch: async () => budgets(await fetchQuota(prov)) };
  }
  if (provider === "anthropic") {
    return {
      title: "Claude",
      fetch: async () => {
        const j = await fetchClaudeUsage();
        return j ? claudeBudgets(j) : undefined;
      },
    };
  }
  return undefined;
}

// Raw ANSI SGR so color is emitted regardless of which theme object the
// extension receives (ctx.ui.theme can be a no-color stub in some render
// contexts; i0 text components render inline ANSI directly — omp itself does
// `new i0(theme.fg(...), 1, 0)`). \x1b[0m resets.
function paint(sgr: string, s: string): string {
  return `\x1b[${sgr}m${s}\x1b[0m`;
}

// Bright green/yellow/red foreground by usage level (exceeded → red).
function levelSgr(pct: number, exceeded: boolean): string {
  if (exceeded || pct >= 90) return "91";
  if (pct >= 70) return "93";
  return "92";
}

// 5-cell meter (▰ filled in the level color, ▱ empty dim-grey) — reads
// unmistakably as a colored "quota gauge"; clamped so >100% shows a full bar.
function bar(pct: number, sgr: string): string {
  const filled = Math.max(0, Math.min(5, Math.round(pct / 20)));
  return paint(sgr, "▰".repeat(filled)) + paint("90", "▱".repeat(5 - filled));
}

function segment(b: Budget): string {
  const sgr = levelSgr(b.pct, b.exceeded);
  const head = b.label ? `${paint("90", b.label)} ` : "";
  const reset = b.resetsAt ? ` ${paint("90", fmtReset(b.resetsAt))}` : "";
  return `${head}${bar(b.pct, sgr)} ${paint(sgr, b.pct + "%")}${reset}`;
}

export function statusText(title: string, bs: Budget[]): string {
  return `${title} ` + bs.map(segment).join(paint("90", " · "));
}

function render(ctx: ExtensionContext, title: string, bs: Budget[]): void {
  try {
    if (bs.length === 0) {
      ctx.ui.setWidget(STATUS_KEY, undefined); // unlimited / unknown → hide
      return;
    }
    // belowEditor widget: its own line directly under the input box with NO
    // surrounding blank-line spacer. The footer hook-status slot (setStatus)
    // sits above omp's mandatory status→editor gap, so a quota there always
    // looks like it has a dangling empty line under it — this avoids that.
    ctx.ui.setWidget(STATUS_KEY, [statusText(title, bs)], { placement: "belowEditor" });
  } catch {
    /* stale/torn-down context — ignore */
  }
}

export default function gwQuotaExtension(pi: ExtensionAPI): void {
  pi.setLabel("gw-quota");

  let providers = loadProviders();
  let timer: NodeJS.Timeout | undefined;
  let generation = 0;
  let lastFetch = 0; // ms epoch of the last actual poll (throttle gate)

  async function tick(ctx: ExtensionContext, myGen: number): Promise<void> {
    if (myGen !== generation) return;
    // Throttle: fetch at most once per REFRESH_MS regardless of trigger source
    // (interval fires on the boundary; turn_end must not poll more often).
    const now = Date.now();
    if (now - lastFetch < REFRESH_MS) return;
    lastFetch = now;
    const src = sourceFor(ctx.model?.provider, providers);
    if (!src) {
      render(ctx, "", []);
      return;
    }
    try {
      const bs = await src.fetch();
      if (myGen !== generation || !bs) return; // failed fetch → keep last shown value
      render(ctx, src.title, bs);
    } catch {
      /* network error → keep last shown value */
    }
  }

  pi.on("session_start", async (_event, ctx) => {
    generation++;
    const myGen = generation;
    clearInterval(timer);
    providers = loadProviders();
    lastFetch = 0; // force an immediate poll for the new session
    await tick(ctx, myGen);
    timer = setInterval(() => {
      void tick(ctx, myGen);
    }, REFRESH_MS);
  });

  pi.on("turn_end", async (_event, ctx) => {
    void tick(ctx, generation);
  });

  pi.on("session_shutdown", async (_event, ctx) => {
    generation++;
    clearInterval(timer);
    timer = undefined;
    try {
      ctx.ui.setWidget(STATUS_KEY, undefined);
    } catch {
      /* ignore */
    }
  });

  pi.registerCommand("gwquota", {
    description: "Refresh + show usage quota (ai-gateway or Claude subscription) for the active provider",
    handler: async (_args, ctx) => {
      providers = loadProviders();
      const provId = ctx.model?.provider;
      const src = sourceFor(provId, providers);
      if (!src) {
        ctx.ui.notify(`gw-quota: no quota source for active model (${provId ?? "?"})`, "warning");
        return;
      }
      try {
        const bs = await src.fetch();
        if (!bs) {
          ctx.ui.notify("gw-quota: fetch failed", "error");
          return;
        }
        render(ctx, src.title, bs);
        if (bs.length === 0) {
          ctx.ui.notify("gw-quota: no usage limits reported (unlimited or API-key login)", "info");
          return;
        }
        const detail = bs
          .map(
            (b) =>
              `${b.label || "window"} [${b.source}]: ${b.pct}% used` +
              (b.resetsAt ? `, resets in ${fmtReset(b.resetsAt)}` : "") +
              (b.exceeded ? " — EXCEEDED" : ""),
          )
          .join("; ");
        ctx.ui.notify(`gw-quota ${detail}`, bs.some((b) => b.exceeded) ? "error" : "info");
      } catch (err) {
        ctx.ui.notify("gw-quota: fetch failed: " + (err instanceof Error ? err.message : String(err)), "error");
      }
    },
  });
}
