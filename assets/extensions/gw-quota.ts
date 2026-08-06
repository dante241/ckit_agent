// gw-quota — omp/pi extension: show the 9router ai-gateway per-key usage quota
// on its own line under the editor via ctx.ui.setWidget(placement:"belowEditor").
//
// It reads the ACTIVE model's provider from ctx.model.provider, looks up that
// provider's { baseUrl, apiKey } in ~/.omp/agent/models.yml, and polls
// `${baseUrl}/quota` (the gateway's GET /claude/v1/quota | /codex/v1/quota).
// Auth: sends BOTH x-api-key (Anthropic path) and Authorization: Bearer (Codex
// path) so one file works for cloudgo-cc and cloudgo-cx.
//
// The endpoint returns NO dollar amounts — only a used percentage. Widget text:
// `Quota <bar> <pct>% ↻<reset-time>` where the meter bar + color both track the
// percentage (success/warning/error). Keys with no budget (budget_source
// "none") and non-gateway providers are hidden. All failures are swallowed
// (session never affected): a non-200 just clears the slot. `/gwquota` forces a
// refresh and reports detail via a notification.
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@oh-my-pi/pi-coding-agent";

const STATUS_KEY = "gw-quota";
const REFRESH_MS = 180_000; // poll the gateway at most once every 3 minutes

export interface ProviderConf {
  baseUrl: string;
  apiKey: string;
}

export interface Quota {
  budget_source: string; // "override" | "team_policy" | "none"
  used_percent: number | null; // null when the key is unlimited
  window_reset_at: string | null; // ISO rollover time; null if never used
  exceeded: boolean;
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

// A budget applies only when the server resolved one (override or team policy)
// AND reported a percentage; otherwise the key is unlimited → nothing to show.
function hasBudget(q: Quota | undefined): q is Quota {
  return !!q && q.budget_source !== "none" && q.used_percent != null;
}

async function fetchQuota(p: ProviderConf): Promise<Quota | undefined> {
  const url = p.baseUrl.replace(/\/+$/, "") + "/quota";
  const res = await fetch(url, {
    headers: { "x-api-key": p.apiKey, authorization: "Bearer " + p.apiKey },
  });
  if (!res.ok) return undefined;
  return (await res.json()) as Quota;
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

// Local wall-clock HH:MM (24h) at which the window rolls over — the reset
// time itself, not a countdown.
function resetClock(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function statusText(q: Quota): string {
  const pct = Math.round(q.used_percent ?? 0);
  const sgr = levelSgr(pct, q.exceeded);
  const clock = resetClock(q.window_reset_at);
  const body = `Quota ${bar(pct, sgr)} ${paint(sgr, pct + "%")}`;
  return clock ? `${body} ${paint("90", "↻" + clock)}` : body;
}

function render(ctx: ExtensionContext, q: Quota | undefined): void {
  try {
    if (!hasBudget(q)) {
      ctx.ui.setWidget(STATUS_KEY, undefined); // unlimited / unknown → hide
      return;
    }
    // belowEditor widget: its own line directly under the input box with NO
    // surrounding blank-line spacer. The footer hook-status slot (setStatus)
    // sits above omp's mandatory status→editor gap, so a quota there always
    // looks like it has a dangling empty line under it — this avoids that.
    ctx.ui.setWidget(STATUS_KEY, [statusText(q)], { placement: "belowEditor" });
  } catch {
    /* stale/torn-down context — ignore */
  }
}

export default function gwQuotaExtension(pi: ExtensionAPI): void {
  pi.setLabel("gw-quota");

  let providers = loadProviders();
  let timer: NodeJS.Timeout | undefined;
  let generation = 0;
  let lastFetch = 0; // ms epoch of the last actual gateway poll (throttle gate)

  async function tick(ctx: ExtensionContext, myGen: number): Promise<void> {
    if (myGen !== generation) return;
    // Throttle: fetch at most once per REFRESH_MS regardless of trigger source
    // (interval fires on the boundary; turn_end must not poll more often).
    const now = Date.now();
    if (now - lastFetch < REFRESH_MS) return;
    lastFetch = now;
    const prov = providers[ctx.model?.provider ?? ""];
    if (!isGatewayProvider(prov)) {
      render(ctx, undefined);
      return;
    }
    try {
      const q = await fetchQuota(prov);
      if (myGen !== generation) return;
      render(ctx, q);
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
    description: "Refresh + show ai-gateway usage quota for the active provider",
    handler: async (_args, ctx) => {
      providers = loadProviders();
      const provId = ctx.model?.provider;
      const prov = providers[provId ?? ""];
      if (!isGatewayProvider(prov)) {
        ctx.ui.notify(`gw-quota: no gateway provider for active model (${provId ?? "?"})`, "warning");
        return;
      }
      try {
        const q = await fetchQuota(prov);
        render(ctx, q);
        if (!hasBudget(q)) {
          ctx.ui.notify("gw-quota: unlimited (no budget on this key/team)", "info");
          return;
        }
        const pct = Math.round(q.used_percent ?? 0);
        const clock = resetClock(q.window_reset_at);
        ctx.ui.notify(
          `gw-quota [${q.budget_source}]: ${pct}% used${q.exceeded ? " — EXCEEDED" : ""}${
            clock ? `, resets ${clock}` : ""
          }`,
          q.exceeded ? "error" : "info",
        );
      } catch (err) {
        ctx.ui.notify("gw-quota: fetch failed: " + (err instanceof Error ? err.message : String(err)), "error");
      }
    },
  });
}
