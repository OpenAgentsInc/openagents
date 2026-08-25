/**
 * Per-model-family tool declaration overrides.
 *
 * The first graded Gym runs measured a model-family habit, not a capability
 * gap: on the same task, gemini-3.7-flash issued fifteen single-command tool
 * rounds where gpt-5.6-luna issued six composite ones, and spent three times
 * the input tokens replaying the growing transcript
 * (`openagents.com` docs/terminalbench/2026-08-24-fix-git-run-analysis.md).
 * Saying "batch" once in the system prompt was not enough for every family.
 *
 * Gemini CLI's own harness answers the same problem inside the declarations:
 * a base declaration per tool plus per-model-family description overrides,
 * resolved at request time (`gemini-cli`
 * packages/core/src/tools/definitions/resolver.ts). This is that pattern,
 * kept as data: a family is a name derived from the model or lane, and an
 * override is an extra sentence appended to a tool's base description. The
 * base descriptions stay the single source of what a tool is; a family
 * override only adds the emphasis that family has measurably needed.
 */

import type { CoderTool } from "./coder-tools.js";

/** The families with distinct declared emphasis. `default` adds nothing. */
export type ToolFamily = "default" | "gemini" | "local";

/**
 * The family for a model name, by prefix.
 *
 * The local lane passes `"local"` explicitly rather than relying on model
 * names, because what distinguishes it is the lane's economics — free
 * tokens, slow generation — not which weights answer.
 */
export const toolFamilyOf = (model: string | undefined): ToolFamily => {
  if (model === undefined) return "default";
  const normalized = model.toLowerCase();
  if (normalized.startsWith("gemini")) return "gemini";
  if (normalized.startsWith("ollama:")) return "local";
  return "default";
};

/**
 * Extra emphasis per family and tool, appended to the base description.
 *
 * Measured, not speculative: an override earns its place with a Gym delta on
 * the same suite, and the analysis document above records why each exists.
 */
const emphasis: Partial<Record<ToolFamily, Partial<Record<string, string>>>> = {
  gemini: {
    shell:
      " IMPORTANT: batch independent commands into ONE call joined with && — " +
      "each separate call replays the whole conversation to the model, so ten " +
      "one-line calls cost several times what one composite call costs. Never " +
      "run one small inspection per call. Read only the region you need; prefer " +
      "offset/limit ranged reads or summaries over whole-file dumps, which are " +
      "token-inefficient.",
  },
  local: {
    shell:
      " This session's model generates slowly on this machine: prefer a few " +
      "composite calls over many small ones, and keep verification to one " +
      "final pass.",
  },
};

/** A tool's description as declared to this family's model. */
export const declaredDescription = (tool: CoderTool, family: ToolFamily): string => {
  const extra = emphasis[family]?.[tool.name];
  return extra === undefined ? tool.description : tool.description + extra;
};
