import type { CoderTool } from "./coder-tools.js";
import { CODER_SURFACE_DIGESTS, SYSTEM_PROMPT_SURFACE } from "./coder-surfaces.generated.js";

/**
 * What a coder session tells the model about itself, on every lane.
 *
 * Derived from the tools actually declared rather than written out, so it
 * cannot claim a tool the session does not pass or miss one it does.
 *
 * A model with no system prompt has nothing anchoring what it is. The local
 * lane learned this first: asked what tools it had, it answered from what a
 * coding agent usually has — files, shell, search, web — and none of that was
 * declared. The thread lane had the same gap for longer and it read worse,
 * because the model there is a hosted one that knows a name for itself: asked
 * who it was, it said "I'm ChatGPT". The invented answer then sat in the
 * transcript and the next turn read it back as instruction.
 *
 * The `lane` sentence is the one true difference between the two. Everything
 * else is shared on purpose: an anchor that drifts between lanes is one that is
 * wrong on at least one of them.
 */
export const systemPrompt = (
  tools: ReadonlyArray<CoderTool>,
  lane: string,
  standing?: string,
): string => {
  const lines = [
    SYSTEM_PROMPT_SURFACE["coder.opening"].replace("{lane}", lane),
    "",
    SYSTEM_PROMPT_SURFACE["coder.concision"],
    "",
  ];

  if (tools.length === 0) {
    lines.push(SYSTEM_PROMPT_SURFACE["coder.no_tools"]);
  } else {
    lines.push(
      SYSTEM_PROMPT_SURFACE["coder.tool_list_header.node"]
        .replace("{count}", String(tools.length))
        .replace("{plural}", tools.length === 1 ? "" : "s"),
      ...tools.map((tool) => `- \`${tool.name}\``),
      "",
      // Stated as a closed list rather than by naming the capabilities that are
      // absent. The absent ones change as tools are added — this once said
      // there was no shell, and then there was one — and a system message that
      // has to be edited when the tool list changes is one that will be wrong
      // in between.
      SYSTEM_PROMPT_SURFACE["coder.tool_list_closing"],
    );
  }

  if (standing !== undefined && standing.length > 0) lines.push("", standing);

  return lines.join("\n");
};

/** The lane sentence for a session answering from a model on this machine. */
export const LOCAL_LANE = SYSTEM_PROMPT_SURFACE["coder.lane.local.node"];

/** The lane sentence for a session answering through the account's thread. */
export const THREAD_LANE = SYSTEM_PROMPT_SURFACE["coder.lane.thread"];

/**
 * The machine-readable staged-text announcement.
 *
 * A bench row records which text produced it (OpenAgentsInc/openagents#122).
 * The digests cannot be read off the repository at scoring time — a run scored
 * a week later would be pinned to whatever the tree says then — so the session
 * names them itself, on stderr, in the same shape and the same place as the
 * thread announcement (`[oa:thread <uuid>]`, #38): one line, parsed by
 * `packages/coder-effectiveness/src/harbor-job.ts` with
 * `\[oa:surfaces ([^\]]+)\]`.
 *
 * Absent from an older CLI, which is not an error: the row then records no
 * surface pin rather than a wrong one.
 */
export const surfaceAnnouncement = (
  digests: Readonly<Record<string, string>> = CODER_SURFACE_DIGESTS,
): string =>
  `[oa:surfaces ${Object.entries(digests)
    .map(([id, digest]) => `${id}=${digest}`)
    .join(",")}]`;
