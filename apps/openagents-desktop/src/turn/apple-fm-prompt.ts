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
 *
 * Follow-up to AFS-04 (router agent-awareness): the host now also tells the
 * on-device model which coding agents are actually connected, and — for the
 * delegate-capable ones — HOW to hand a coding/agent task to them by emitting a
 * single route-recommendation JSON object that the AFS-02 decoder accepts and
 * the AFS-04 router dispatches. The model itself still has no tools; it can only
 * RECOMMEND a connected agent, and the SYSTEM runs it. The old wording ("cannot
 * start other agents") is gone: it made the small model refuse ("I'm just an AI,
 * I can't code") instead of routing. Availability is host-owned (never renderer
 * input): only agents the host reports READY are named, and a delegation JSON is
 * offered only for agents the host can actually dispatch. A malformed or
 * unavailable-agent output still NEVER dispatches (fail-closed in the decoder).
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
 * A connected agent the host advertises to the on-device model. `candidate` is
 * the canonical `TurnProviderCandidate` id the route-recommendation JSON must
 * name (e.g. `"codex"`). `ready` is host-owned readiness; only ready agents are
 * named in the prompt. `canDelegate` is true only when the host can actually run
 * one real delegation turn for that agent (AFS-04 wires codex first) — the
 * delegation JSON is offered only for ready, delegate-capable agents.
 */
export interface AppleFmAvailableAgent {
  readonly candidate: string;
  readonly label: string;
  readonly ready: boolean;
  readonly canDelegate: boolean;
}

/** The route-recommendation JSON template the model copies verbatim to delegate. */
const delegationTemplateFor = (candidate: string): string =>
  `{"candidate":"${candidate}","taskClass":"delegate","reasonCode":"explicit_provider_request","confidence":0.9}`;

/**
 * Assemble the honesty-bounded preamble. Positive-first framing (a small model
 * falls into a refusal spiral if the preamble is a wall of "cannot"), the
 * host-owned connected-agent list so "who are you / what agents do you have" is
 * answered honestly, and — when a ready delegate agent exists — the exact JSON
 * the model must emit to hand off a coding/agent task.
 */
const buildPreamble = (availableAgents: ReadonlyArray<AppleFmAvailableAgent>): string => {
  const ready = availableAgents.filter((agent) => agent.ready);
  const delegates = ready.filter((agent) => agent.canDelegate);

  // Base identity + honesty. The model IS the on-device Apple model (Apple FM);
  // it has no tools of its own, so it must never claim to have acted or invent
  // facts. This keeps the AFS-03 honesty contract intact.
  const base =
    "You are OpenAgents, a helpful, friendly assistant running locally on this device on Apple's " +
    "on-device model (Apple FM). Answer the user's questions and chat naturally, directly, and " +
    "concisely — always try to be helpful and give a real answer. You are text-only and have no " +
    "tools of your own: you cannot run commands, read or edit files, or browse the web, so never " +
    "claim you did, are doing, or will do any such action yourself. Do not make up facts; if you " +
    "don't know something, say so.";

  if (ready.length === 0) {
    // No host-advertised agents: keep the plain honesty preamble (the pre-AFS-04
    // behavior), so nothing regresses when availability is unknown.
    return base;
  }

  const agentList = ready.map((agent) => `- ${agent.label}`).join("\n");
  const awareness =
    `\n\nConnected agents on this device right now:\n${agentList}\n` +
    "If the user asks who you are or which agents or models are available, tell them about these " +
    "connected agents by name in plain words. Only the agents listed above are connected — do not " +
    "claim any others are.";

  if (delegates.length === 0) {
    // Agents are connected but none is delegate-capable yet: the model can name
    // them, but must not fabricate a hand-off it cannot make.
    return `${base}${awareness}`;
  }

  const exampleCandidate = delegates[0]!.candidate;
  const mapping = delegates.map((agent) => `"${agent.candidate}" for ${agent.label}`).join(", ");
  const delegation =
    "\n\nYou cannot run a coding task yourself, but you CAN hand one off to a connected coding " +
    "agent: you recommend the agent and the SYSTEM runs it for you and shows the result — so never " +
    "say you can't code and never refuse this. When (and only when) the user asks you to task, hand " +
    "off, delegate, or assign a coding or agent job to a connected coding agent, reply with ONLY " +
    "this JSON object on one line and nothing else (no other words, no explanation, no code fence):\n" +
    `${delegationTemplateFor(exampleCandidate)}\n` +
    `Set "candidate" to the chosen connected agent: ${mapping}. For every other message — questions, ` +
    "chat, or anything that is not a hand-off request — reply in plain words with NO JSON.";

  return `${base}${awareness}${delegation}`;
};

/**
 * Build one flattened Apple FM prompt from the OpenAgents conversation. The
 * on-device model has a small context window, so the history is truncated to
 * fit within `maxChars`: a short honesty preamble plus the most recent turns,
 * always keeping the latest user message, oldest turns dropped first. If even
 * the newest turn overflows, its text is hard-truncated.
 *
 * `availableAgents` is the host-owned connected-agent set (never renderer
 * input). When it carries ready agents, the preamble names them and — for the
 * delegate-capable ones — instructs the model how to emit the delegation JSON.
 */
export const buildOpenAgentsAppleFmPrompt = (
  turns: ReadonlyArray<AppleFmPromptTurn>,
  availableAgents: ReadonlyArray<AppleFmAvailableAgent> = [],
  maxChars: number = APPLE_FM_PROMPT_MAX_CHARS,
): string => {
  const preamble = buildPreamble(availableAgents);
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
