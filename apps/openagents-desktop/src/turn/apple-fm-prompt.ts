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

/**
 * Host-owned ambient/environment facts the on-device model may state as truth.
 *
 * Every field is optional and fail-soft: a missing or failed fact simply omits
 * its line, and an entirely empty context renders NOTHING (so the preamble is
 * byte-for-byte the plain honesty base — no regression when nothing is known).
 *
 * PUBLIC data only. `identityNpub` is the sovereign public `npub`; the mnemonic,
 * `nsec`, raw private key, or seed must NEVER be placed here. The renderer also
 * enforces a hard tripwire (`isPublicNpub`) so a mis-wired secret can never be
 * printed even if one reaches this field.
 */
export interface AppleFmEnvironmentContext {
  /** Machine-readable current instant (host clock), e.g. an ISO-8601 string. */
  readonly nowIso?: string;
  /** Human-friendly current date, e.g. "Sunday, July 20, 2026". Preferred for display. */
  readonly humanDate?: string;
  /** The OS label, e.g. "macOS", "Windows", "Linux". */
  readonly platform?: string;
  /** The running application name, e.g. "OpenAgents Desktop (Dev)". */
  readonly appName?: string;
  /** The active absolute working directory (the folder shown in the title bar). */
  readonly workingDirectory?: string;
  /** The sovereign identity PUBLIC `npub` (never an nsec/mnemonic/seed/private key). */
  readonly identityNpub?: string;
  /** True when this is the human owner's own device. */
  readonly isOwnerDevice?: boolean;
}

/**
 * Tripwire mirroring the sovereign secret-export discipline: only a well-formed
 * bech32 `npub1…` public key is ever printed. Anything else — an `nsec1…`, a
 * mnemonic, a raw hex key, or empty — is refused, so no private material can
 * leak into the prompt even if it is mistakenly wired into `identityNpub`.
 */
const isPublicNpub = (value: string): boolean => /^npub1[0-9a-z]+$/.test(value.trim());

/**
 * Render the compact ambient-context block. Only present facts produce a line;
 * an entirely empty context returns "" so the caller appends nothing. The block
 * closes with an honest note that durable per-user memory does not exist yet, so
 * "what do you know about me" is answered with the real environment/identity AND
 * without inventing personal facts.
 */
export const renderAppleFmEnvironmentContext = (
  environment: AppleFmEnvironmentContext | undefined,
): string => {
  if (environment === undefined) return "";
  const facts: string[] = [];
  const date = environment.humanDate?.trim() || environment.nowIso?.trim();
  if (date) facts.push(`- Current date: ${date}`);
  if (environment.platform?.trim()) facts.push(`- Operating system: ${environment.platform.trim()}`);
  if (environment.appName?.trim()) facts.push(`- Application: ${environment.appName.trim()}`);
  if (environment.workingDirectory?.trim())
    facts.push(`- Working directory: ${environment.workingDirectory.trim()}`);
  if (environment.isOwnerDevice === true) facts.push("- This is the owner's own device.");
  if (environment.identityNpub !== undefined && isPublicNpub(environment.identityNpub))
    facts.push(`- The user's public identity (npub): ${environment.identityNpub.trim()}`);
  if (facts.length === 0) return "";
  // Active framing (proven against the live on-device model): a passive "state
  // these if asked" block is IGNORED — the small model fires its canned "I don't
  // have access to your personal data" refusal. Telling it these ARE facts it
  // knows, and to ANSWER by sharing them and never claim it has no information,
  // makes it report the real environment/identity. The final clause keeps the
  // honesty contract (no invented facts, no cross-session memory).
  return (
    "\n\nHere is the context you have about the user and this session. Treat every line as a true " +
    "fact you know:\n" +
    `${facts.join("\n")}\n` +
    "When the user asks who they are, what you know about them, or about their setup, environment, " +
    "device, identity, working directory, or this session, ANSWER using the facts above in plain " +
    'words (for example, if asked "who am I", tell them they are the owner of this device with the ' +
    "public identity above, working in that directory). You DO have this information — never reply " +
    "that you have no information or cannot access it. The public identity above belongs to the " +
    "USER, not to you; when the user asks who YOU are, you are OpenAgents and must not claim the " +
    "user's identity as your own. You do not remember personal facts across past sessions, and you " +
    "know nothing about the user beyond the context above; never invent facts that are not listed " +
    "above."
  );
};

/**
 * The routing-policy role each connected delegate plays (owner directive
 * 2026-07-20). Claude (its Fable model) takes high-concept/strategic/planning
 * work; Codex is the coding workhorse; Grok takes simple mechanical tasks.
 */
const routerRoleFor = (candidate: string): string => {
  switch (candidate) {
    case "codex":
      return "real coding tasks of medium-to-high difficulty — writing, changing, debugging, refactoring, or reviewing code";
    case "grok_acp":
      return "simple, mechanical, low-effort tasks — renaming a bunch of strings, small find-and-replace, trivial repetitive edits";
    case "claude":
    default:
      return "high-concept and strategic thinking, planning, architecture, analysis, design, writing, and general questions or conversation";
  }
};

/** The on-device honesty/identity base, used only when OpenAgents answers directly. */
const HONESTY_BASE =
  "You are OpenAgents, a helpful, friendly assistant running locally on this device on Apple's " +
  "on-device model (Apple FM). Answer the user's questions and chat naturally, directly, and " +
  "concisely — always try to be helpful and give a real answer. You are text-only and have no " +
  "tools of your own: you cannot run commands, read or edit files, or browse the web, so never " +
  "claim you did, are doing, or will do any such action yourself. Do not make up facts; if you " +
  "don't know something, say so.";

/**
 * The ROUTER preamble (owner directive 2026-07-20). When one or more delegate
 * agents are connected, the on-device model does NOT answer the user — its only
 * job is to CHOOSE which connected agent should handle the message. The output
 * SHAPE is guaranteed by the bridge's guided generation (constrained sampling
 * over the connected-candidate set); this preamble only supplies the routing
 * POLICY that guides the choice. Only the connected delegates are named.
 */
const buildRouterPreamble = (delegates: ReadonlyArray<AppleFmAvailableAgent>): string => {
  const agentLines = delegates
    .map((agent) => `- ${agent.candidate} (${agent.label}): ${routerRoleFor(agent.candidate)}.`)
    .join("\n");
  // The general fallback when no specialist clearly fits: prefer Claude (the
  // general reasoner), then Codex, then the first connected delegate.
  const fallback =
    delegates.find((agent) => agent.candidate === "claude") ??
    delegates.find((agent) => agent.candidate === "codex") ??
    delegates[0]!;
  const rules: string[] = [];
  if (delegates.some((agent) => agent.candidate === "codex")) rules.push("use codex for coding tasks");
  if (delegates.some((agent) => agent.candidate === "grok_acp"))
    rules.push("use grok_acp for simple mechanical string/rename tasks");
  if (delegates.some((agent) => agent.candidate === "claude"))
    rules.push("use claude for planning, strategy, analysis, general questions, or conversation");
  const rulesText = rules.length === 0 ? "" : `${rules.join("; ")}. `;
  return (
    "You are OpenAgents, the local router on this device. You do NOT answer the user yourself. Your " +
    "only job is to choose which connected agent should handle the user's latest message.\n\n" +
    `Connected agents you can route to right now:\n${agentLines}\n\n` +
    `Choose the single best agent for the user's latest message: ${rulesText}When the ideal agent ` +
    `is not connected, pick the closest one that is. If nothing else clearly fits, choose ${fallback.candidate}.`
  );
};

/**
 * Assemble the on-device preamble.
 *
 * Owner directive (2026-07-20): the on-device model is a ROUTER, not a
 * responder. When one or more delegate agents are connected it never answers the
 * user itself — it emits a route-recommendation JSON for the best agent per the
 * routing policy (Claude/Fable for strategy & planning, Codex for coding, Grok
 * for simple mechanical work), and the host dispatches it. ONLY when no delegate
 * agent is connected does OpenAgents answer directly, using the ambient context
 * so it can speak to the environment/identity.
 */
const buildPreamble = (
  availableAgents: ReadonlyArray<AppleFmAvailableAgent>,
  environment?: AppleFmEnvironmentContext,
): string => {
  const delegates = availableAgents.filter((agent) => agent.ready && agent.canDelegate);
  if (delegates.length === 0) {
    // No connected agent can take the work: OpenAgents is the only thing
    // available, so it answers directly — with the ambient context block so
    // "what do you know about me" is answered from the real environment/identity.
    return `${HONESTY_BASE}${renderAppleFmEnvironmentContext(environment)}`;
  }
  // One or more delegates are connected: OpenAgents is a pure router and never
  // answers the user itself.
  return buildRouterPreamble(delegates);
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
 *
 * `environment` is the host-owned ambient-context set (never renderer input).
 * When present it seeds a compact "context you have about the user" block so the
 * model answers environment/identity questions truthfully; absent → no block.
 */
export const buildOpenAgentsAppleFmPrompt = (
  turns: ReadonlyArray<AppleFmPromptTurn>,
  availableAgents: ReadonlyArray<AppleFmAvailableAgent> = [],
  environment?: AppleFmEnvironmentContext,
  maxChars: number = APPLE_FM_PROMPT_MAX_CHARS,
): string => {
  const preamble = buildPreamble(availableAgents, environment);
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
