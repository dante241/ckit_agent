import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { existsSync } from "node:fs";
import { join } from "node:path";

// ckit code-intel hook — enforces RULE #0 mechanically. Per user prompt, every grep/read
// that targets code is blocked until a code-intel device ran (codegraph · codebase-memory
// · serena · lsp). Only an exact retry of a blocked call passes, so plain-text searches
// are never dead-locked but can't bypass the rule wholesale. Inactive without `.codegraph/`.
const CODE_EXT = /\.(php|tpl|js|mjs|cjs|ts|tsx|jsx|vue|go|rs|py|java|kt|rb|cs|c|cc|cpp|h|hpp|swift|scala)$/i;
const INTEL_TOOL = /^mcp__(codegraph|codebase_memory|serena)/;
const INTEL_CLI = /\bcodegraph\s+(explore|query|callers|callees|impact|node)\b/;

type Input = Record<string, unknown>;

// Local path without read selectors (`:10-20`, `:raw`, `:-60` …); "" for URLs/URIs.
function localPath(raw: unknown): string {
  if (typeof raw !== "string" || raw.includes("://")) return "";
  return raw.replace(/:(raw|conflicts|img|-?\d).*$/, "");
}

function usesIntel(toolName: string, input: Input): boolean {
  if (toolName === "lsp" || INTEL_TOOL.test(toolName)) return true;
  if (toolName === "write") return /^xd:\/\/mcp__(codegraph|codebase_memory|serena)/.test(String(input.path ?? ""));
  if (toolName === "bash") return INTEL_CLI.test(String(input.command ?? ""));
  return false;
}

// grep with no path, a directory, or any code file counts as a code search.
function targetsCode(toolName: string, input: Input): boolean {
  if (toolName === "read") return CODE_EXT.test(localPath(input.path));
  if (toolName !== "grep") return false;
  const raw = typeof input.path === "string" && input.path.trim() ? input.path : ".";
  return raw.split(";").some((p) => {
    const path = localPath(p.trim());
    return path !== "" && (CODE_EXT.test(path) || !/\.[a-z0-9]+$/i.test(path));
  });
}

// Identity of a call for the exact-retry bypass (intent text excluded: it may be reworded).
function callKey(toolName: string, input: Input): string {
  const { i: _intent, ...rest } = input;
  return `${toolName}:${JSON.stringify(rest)}`;
}

export default function (pi: ExtensionAPI): void {
  let intelUsed = false;
  const blocked = new Set<string>();

  pi.on("before_agent_start", async () => {
    intelUsed = false;
    blocked.clear();
    return undefined;
  });

  pi.on("tool_call", async (event, ctx) => {
    const input = (event.input ?? {}) as Input;
    if (usesIntel(event.toolName, input)) {
      intelUsed = true;
      return undefined;
    }
    if (intelUsed || !targetsCode(event.toolName, input)) return undefined;
    if (!existsSync(join(ctx.cwd, ".codegraph"))) return undefined;

    const key = callKey(event.toolName, input);
    if (blocked.has(key)) return undefined;
    blocked.add(key);
    return {
      block: true,
      reason:
        'code-intel-first: run `write` path `xd://mcp__codegraph_explore` content `{"query":"<symbols or question>"}` ' +
        "(or cbm/serena/lsp) before grep/read on code. If plain-text search is really the right tool, retry the same call — it will pass.",
    };
  });
}
