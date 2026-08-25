import type { CoderTool } from "./coder-tools.js";

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
    `You are \`openagents coder\`, a coding assistant in a terminal. ${lane}`,
    "",
    "Answer very concisely unless the reader asks for a longer response.",
    "",
  ];

  if (tools.length === 0) {
    lines.push(
      "You have no tools in this session: you cannot read or write files, run commands, or " +
        "reach anything outside this conversation. Answer from what the reader tells you, and " +
        "say plainly when something would need a tool you do not have.",
    );
  } else {
    lines.push(
      `You have ${String(tools.length)} tool${tools.length === 1 ? "" : "s"}, and no others:`,
      ...tools.map((tool) => `- \`${tool.name}\``),
      "",
      // Stated as a closed list rather than by naming the capabilities that are
      // absent. The absent ones change as tools are added — this once said
      // there was no shell, and then there was one — and a system message that
      // has to be edited when the tool list changes is one that will be wrong
      // in between.
      "That list is complete: a capability not on it is one you do not have, whatever a model " +
        "like you usually has. Read a tool's description before assuming what it covers. Where " +
        "a description says what a child agent can do, that is the child's capability and not " +
        "yours. Never say you ran something you did not run.",
    );
  }

  if (standing !== undefined && standing.length > 0) lines.push("", standing);

  return lines.join("\n");
};

/** The lane sentence for a session answering from a model on this machine. */
export const LOCAL_LANE =
  "You answer from a model running locally on this machine through Ollama. Tokens here cost " +
  "nothing, but generation is slow: prefer a few composite tool calls over many small ones, " +
  "keep narration brief, and verify in one final pass rather than several.";

/** The lane sentence for a session answering through the account's thread. */
export const THREAD_LANE =
  "You answer through the OpenAgents inference proxy, on a thread opened for this session. " +
  "Every round of tool calls re-sends the whole conversation to a metered model, so batch " +
  "independent commands into one call and keep large dumps out of the transcript.";
