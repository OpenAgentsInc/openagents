/**
 * AFS-03 host-owned Apple FM prompt assembly.
 *
 * The renderer no longer builds the authoritative Apple FM prompt. This module
 * lives on the main side (behind the shared turn kernel's Apple FM provider
 * adapter) and assembles the one flattened, honesty-bounded prompt from the
 * canonical thread history the host already holds. The renderer only submits a
 * typed `Ask` intent; the host owns the preamble, the history window, and the
 * fail-closed honesty framing.
 *
 * Keeping this in `src/turn/` (not the renderer) is the "remove the renderer
 * authority fork" invariant: no renderer code builds an authoritative prompt.
 */

/**
 * The Apple FM prompt hard cap mirrors `AppleFmStartTurnRequestSchema` (4000
 * chars, the frozen `MAX_TURN_INPUT_CHARS`). Leave a small margin so the
 * flattened prompt never trips the schema bound.
 */
export const APPLE_FM_PROMPT_MAX_CHARS = 3900;

/** A minimal conversation turn shape (role + text) both the thread store and tests satisfy. */
export interface AppleFmPromptTurn {
  readonly role: "user" | "assistant" | "system" | string;
  readonly text: string;
}

/**
 * Build one flattened Apple FM prompt from the OpenAgents conversation. The
 * on-device model has a small context window, so the history is truncated to
 * fit within `maxChars`: a short honesty preamble plus the most recent turns,
 * always keeping the latest user message, oldest turns dropped first. If even
 * the newest turn overflows, its text is hard-truncated.
 */
export const buildOpenAgentsAppleFmPrompt = (
  turns: ReadonlyArray<AppleFmPromptTurn>,
  maxChars: number = APPLE_FM_PROMPT_MAX_CHARS,
): string => {
  // Owner directive 2026-07-20: the on-device model must not lie about actions
  // it cannot take — but the earlier all-prohibition preamble backfired, pushing
  // this small model into a refusal spiral (it rejected even "what can you do"
  // with canned "as an LLM I cannot comply"). Reframe positive-first: it IS a
  // helpful assistant that answers normally; the honesty limit is a short,
  // specific note (no tools, so never claim to have acted, and don't make things
  // up), NOT a wall of "cannot" that trips the refusal reflex.
  const preamble =
    "You are OpenAgents, a helpful, friendly assistant running locally on this device. " +
    "Answer the user's questions and chat naturally, directly, and concisely — always try to be " +
    "helpful and give a real answer. You are text-only and have no tools: you cannot run commands, " +
    "read or edit files, browse the web, set reminders, or start other agents. So never claim you " +
    "did, are doing, or will do any such action; if the user asks for one, briefly say you can't do " +
    "that here, then help by answering in words. Do not make up facts; if you don't know something, " +
    "say so.";
  const lines = turns
    .map((turn) => {
      const text = turn.text.trim();
      return text === "" ? null : `${turn.role === "assistant" ? "Assistant" : "User"}: ${text}`;
    })
    .filter((line): line is string => line !== null);
  const assemble = (rows: ReadonlyArray<string>): string => `${preamble}\n\n${rows.join("\n")}\nAssistant:`;
  // Keep the newest turns that fit, dropping the oldest first, but never drop
  // the final line (the message being answered).
  let kept = lines;
  while (kept.length > 1 && assemble(kept).length > maxChars) kept = kept.slice(1);
  const prompt = assemble(kept);
  return prompt.length > maxChars ? prompt.slice(0, maxChars) : prompt;
};
