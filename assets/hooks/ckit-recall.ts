import type { HookAPI } from "@oh-my-pi/pi-coding-agent/extensibility/hooks";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

// ckit recall hook — anti-forget (the LIVE half). The static, always-apply
// directives (RULE #0 + always-on skills) live in ~/.omp/agent/APPEND_SYSTEM.md,
// which is always in the system prompt and never compacts away; omp itself lists
// every skill. This hook adds the live STATE Current/Next at every agent-start and
// into every compaction summary.
// Hard cap ~1k token. Fail-safe: any read error is swallowed (session unaffected).
export default function (pi: HookAPI): void {
  function stateHead(): string {
    try {
      const cwd = process.cwd();
      let state = join(cwd, "agents/STATE.md");
      if (!existsSync(state)) state = join(cwd, "su-code/STATE.md"); // pre-rename fallback (un-migrated project)
      if (!existsSync(state)) return "";
      const md = readFileSync(state, "utf8");
      const grab = (heading: string): string => {
        const m = md.match(new RegExp(`## ${heading}[\\s\\S]*?(?:\\n## |$)`));
        return m ? m[0].trim() : "";
      };
      return ["Current step", "Next"].map(grab).filter(Boolean).join("\n\n");
    } catch {
      return "";
    }
  }

  function bundle(): string {
    const head = stateHead();
    if (!head) return "";
    return [
      "# ckit recall — obey ~/.omp/agent/APPEND_SYSTEM.md",
      "Code-intel first (codegraph · codebase-memory-mcp · serena via `xd://`) BEFORE grep/read; recall before / retain durable facts after; browser to verify web/UI.",
      "",
      "## STATE",
      head,
    ].join("\n").slice(0, 4000);
  }

  pi.on("before_agent_start", async () => {
    const content = bundle();
    return content ? { message: { customType: "ckit-recall", content } } : undefined;
  });

  pi.on("session.compacting", async () => {
    const content = bundle();
    return content ? { context: [content] } : undefined;
  });
}
