import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { existsSync } from "node:fs";
import { join } from "node:path";

// ckit code-intel hook — enforces RULE #0 mechanically. Per user prompt, the first grep/read
// that targets code before any code-intel device ran (codegraph · codebase-memory · serena ·
// lsp) is blocked. An exact retry of that call means "plain text is right here" and unlocks
// the rest of the prompt; any other code grep/read stays blocked until retry or intel.
// Inactive without `.codegraph/`.
// A grep whose pattern looks up a definition (`function foo\(`, `class Foo`, `def foo`) counts as
// code-intel: locate-then-`read path:a-b` is the cheapest route, so it is never blocked. The block
// reason points at `codegraph query`/`node` first; `codegraph_explore` returns ~50 KB of source and
// is reserved for call flow / blast radius across files.
const CODE_EXT = /\.(php|tpl|js|mjs|cjs|ts|tsx|jsx|vue|go|rs|py|java|kt|rb|cs|c|cc|cpp|h|hpp|swift|scala)$/i;
const INTEL_TOOL = /^mcp__(codegraph|codebase_memory|serena)/;
const INTEL_CLI = /\bcodegraph\s+(explore|query|callers|callees|impact|node)\b/;
const DEF_LOOKUP = /\b(function|class|interface|trait|enum|struct|def|fn|func)(\s|\\s)[+*]?\s*[\\(A-Za-z_$]/;

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

function isDefinitionLookup(toolName: string, input: Input): boolean {
  return toolName === "grep" && DEF_LOOKUP.test(String(input.pattern ?? ""));
}

// Identity of a call for the exact-retry unlock (intent text excluded: it may be reworded).
function callKey(toolName: string, input: Input): string {
  const { i: _intent, ...rest } = input;
  return `${toolName}:${JSON.stringify(rest)}`;
}

export default function (pi: ExtensionAPI): void {
  let intelUsed = false; // code-intel ran, or the agent insisted via exact retry
  const blockedKeys = new Set<string>(); // parallel calls each block; a retry of any unlocks

  pi.on("before_agent_start", async () => {
    intelUsed = false;
    blockedKeys.clear();
    return undefined;
  });

  pi.on("tool_call", async (event, ctx) => {
    const input = (event.input ?? {}) as Input;
    if (usesIntel(event.toolName, input) || isDefinitionLookup(event.toolName, input)) {
      intelUsed = true;
      return undefined;
    }
    if (intelUsed || !targetsCode(event.toolName, input)) return undefined;
    if (!existsSync(join(ctx.cwd, ".codegraph"))) return undefined;
    const key = callKey(event.toolName, input);
    if (blockedKeys.has(key)) {
      intelUsed = true;
      return undefined;
    }
    blockedKeys.add(key);
    return {
      block: true,
      reason:
        "code-intel-first: locate with `bash` `codegraph query <name>` (path:line, ~150 B) or " +
        "`codegraph node <file> --symbols-only` (outline), then `read path:a-b`. `codegraph callers|impact <name>` for " +
        "who-calls / what-breaks; `xd://mcp__codegraph_explore` (~50 KB source) only for call flow across files. " +
        "If plain-text search is really the right tool, retry the same call — it passes and unlocks this prompt.",
    };
  });
}
