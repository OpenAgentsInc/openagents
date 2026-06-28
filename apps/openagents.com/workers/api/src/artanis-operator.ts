// Artanis operator core — the single shared logic that lets the OWNER converse
// with the REAL Artanis operator agent (issue #6363, epic #6359).
//
// WHY THIS EXISTS
// ---------------
// In the mobile app and Khala CLI, "I need to speak to Artanis" was answered by
// the PUBLIC Khala collective-intelligence prompt (`inference/khala-identity.ts`)
// — which, when prompted about "Artanis", produced StarCraft / "Hierarch Artanis
// / the Daelaam" roleplay. That is the public persona for public chat. It is NOT
// the operator agent. The owner needs to actually converse with Artanis — the
// agent that runs the Khala improvement loop (epic #6359) — and have it KNOW
// ITSELF: prior owner interactions (memory), and what it is doing lately (recent
// actions / current goals / ongoing operations = situational awareness).
//
// THE FOUR HARD CONSTRAINTS (from #6363, in priority order)
// ---------------------------------------------------------
//   1. DISTINCT PERSONA. Artanis answers in the FIRST PERSON as the operator
//      agent. ZERO StarCraft/Daelaam/Hierarch/collective-intelligence roleplay.
//      We deliberately do NOT import the Khala identity prompt here — the public
//      Khala persona stays where it is for public chat; this persona is separate
//      and owner-only. A persona-separation check (`assertNoKhalaRoleplay`) is
//      exported so the route + tests can assert the two never bleed.
//   2. KHALA-POWERED ONLY (dogfooding). Artanis's OWN reasoning calls go through
//      the Khala API (`openagents/khala`) via the injected `khalaClient` — the
//      same dogfood seam the forum responder uses
//      (`artanis-forum-responder.ts` -> `ArtanisResponderKhalaClient`,
//      `makeArtanisResponderKhalaClient` in index.ts). Artanis's inference
//      therefore COUNTS as Khala usage; we never call a provider directly here.
//      The request is structured as a Blueprint-style typed program (a system
//      contract + grounded context blocks + the conversation) per the brain/
//      blueprint hookup direction (docs/khala/2026-06-24-…brain-and-blueprint-
//      hookup-audit.md, …blueprint-program-and-plugin-extensibility.md): Artanis
//      is a Khala-powered Blueprint program/agent, not a model-alias passthrough.
//   3. GROUNDED. We inject MEMORY (prior owner turns + durable notes) and
//      SITUATIONAL AWARENESS ({recentActions, goals, ongoingOps}) into the
//      context so Artanis answers "what are you doing?" from real state, not
//      invention. Memory comes from the owner-scoped store
//      (`artanis-owner-memory.ts`: `loadArtanisMemory` / `appendArtanisMemory`),
//      awareness from `artanis-situational-awareness.ts`
//      (`buildArtanisSituationalAwareness`). This core consumes those REAL types
//      directly; the route owns the store/readers wiring and persistence.
//   4. AUTHORITY BOUNDARY. This core PROPOSES and CONVERSES; it never grants new
//      authority. Spend/destructive intents still defer to
//      `artanis-approval-gates` — we re-export the gate boundary refs and surface
//      a `deferredToApprovalGate` hint, but we add no execution authority.

import { Effect, Schema as S } from 'effect'

import { ARTANIS_RISKY_ACTION_KINDS } from './artanis-approval-gates'
import type { ArtanisRiskyActionKind } from './artanis-approval-gates'
import type { ArtanisMemoryEntry } from './artanis-owner-memory'
import type { ArtanisSituationalAwareness } from './artanis-situational-awareness'
import type {
  InferenceAdapterError,
  InferenceMessage,
  InferenceRequest,
  InferenceResult,
  InferenceToolCall,
} from './inference/provider-adapter'

import { formatArtanisTokenPaceLine } from './artanis-token-pace'

import { parseJsonUnknown } from './json-boundary'

export type { ArtanisRiskyActionKind } from './artanis-approval-gates'

// Re-export the real grounding types so the surface + memory lanes have a single
// import site for the operator-core contract.
export type { ArtanisMemoryEntry } from './artanis-owner-memory'
export type { ArtanisSituationalAwareness } from './artanis-situational-awareness'

// ---------------------------------------------------------------------------
// Model + identity constants.
// ---------------------------------------------------------------------------

// Artanis dogfoods Khala: every operator turn is an `openagents/khala` request,
// so the reasoning is served by the Khala pool and metered as Khala usage. This
// is intentionally the SAME model alias the forum responder uses.
export const ARTANIS_OPERATOR_KHALA_MODEL = 'openagents/khala'

// Demand attribution refs for the operator channel (mirrors the forum responder
// `demandSource`/`demandClient` so served-token rows attribute correctly to
// Artanis-as-Khala-consumer).
export const ARTANIS_OPERATOR_DEMAND_SOURCE = 'artanis'
export const ARTANIS_OPERATOR_DEMAND_CLIENT = 'artanis_operator_chat'

// The operator channel ref (owner-scoped, private).
export const ARTANIS_OPERATOR_CHANNEL_REF = 'operator.artanis.chat'

// The authority boundary ref. The operator core never executes spend/destructive
// actions; those defer to the approval gate ledger. Re-exported alongside the
// gate module's risky-action set so this stays the single source of truth.
export const ARTANIS_OPERATOR_APPROVAL_GATE_REF =
  'gate.operator.artanis.spend_destructive'
export const ARTANIS_OPERATOR_RISKY_ACTION_KINDS = ARTANIS_RISKY_ACTION_KINDS

// How many prior memory entries we inject into a single operator turn's context.
export const ARTANIS_OPERATOR_MEMORY_TURN_LIMIT = 20

// Bounded tool-calling loop cap (#6364). Each iteration is ONE `openagents/khala`
// completion; if the model asks for tools we execute them and re-call Khala. The
// cap guarantees the loop always terminates with a final text reply (it never
// spins forever on a model that keeps requesting tools). One extra Khala call
// past the cap is allowed so the model can produce a final answer with the last
// tool results in context.
export const ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS = 6

// Generous per-completion output ceiling (#6651). The operator composes long
// grounded plans, status sweeps, and multi-step decisions; the previous 4096
// cap truncated those mid-sentence. We raise the ceiling and ALSO continue on a
// `length` finish (below) so a long reply is never returned truncated regardless
// of this cap.
export const ARTANIS_OPERATOR_MAX_OUTPUT_TOKENS = 16_384

// Continue-on-length cap (#6651). When a completion stops because it hit the
// output ceiling (`finishReason === 'length'`), we append the partial assistant
// turn plus a short "continue" user turn and re-call Khala, concatenating the
// segments, until the model finishes naturally (`finishReason === 'stop'`) or we
// reach this bound. This guarantees a long plan is delivered whole even if it
// exceeds a single completion's cap.
export const ARTANIS_OPERATOR_MAX_CONTINUE_ITERATIONS = 6

// The bounded "continue" user turn appended when a completion stops on `length`.
// It asks the model to resume exactly where it left off without repeating, so
// the concatenated segments read as one continuous reply.
export const ARTANIS_OPERATOR_CONTINUE_INSTRUCTION =
  'Your previous message was cut off because it hit the output limit. Continue exactly where you left off, in plain text. Do not repeat anything you already wrote, and do not call any tools.'

// The reply returned ONLY as the absolute last resort: when the Khala loop
// produced no text at all AND no tool work was gathered to ground a reply on
// (e.g. a no-tool turn whose every completion came back empty). The turn must
// NEVER return an empty string to the owner, and this apology must NEVER be the
// first thing tried \u2014 it is reached only after the forced composition retries
// below all fail AND there is no gathered tool state to synthesize from.
export const ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK =
  "I could not compose a full reply this turn \u2014 my reasoning pass came back empty after working through the available tools. Ask me again, or narrow the question, and I'll answer directly."

// Max characters of a SINGLE tool result fed back into the agentic conversation.
// Read tools can return large bodies (e.g. a 256KB repo file, a long issue
// thread); appending several verbatim bloats the request until a LATER Khala
// call fails outright (the live `artanis_operator_mind_unavailable` recurrence).
// We bound each tool result so the loop stays within the model/gateway request
// budget while still giving the model the substance it needs to reason.
export const ARTANIS_OPERATOR_TOOL_RESULT_CONTEXT_MAX_CHARS = 16_000

// Tighter per-tool bound for the COMPACT final-composition request and the
// grounded synthesis fallback, so the recovery path stays small and robust even
// when the full agentic conversation has grown too large to re-send.
export const ARTANIS_OPERATOR_TOOL_RESULT_SUMMARY_MAX_CHARS = 4_000

// Overall character budget for the gathered-results block in the compact
// composition request / synthesis fallback, so recovery never re-bloats.
export const ARTANIS_OPERATOR_GATHERED_SUMMARY_TOTAL_MAX_CHARS = 24_000

// Bound a string to `max` chars with an explicit truncation marker so the model
// (and the owner) knows the read was cut, never silently dropped.
const boundArtanisText = (value: string, max: number): string =>
  value.length > max
    ? `${value.slice(0, max)}\n(\u2026truncated at ${max} chars)`
    : value

// One gathered tool result captured during the loop: the tool name and its
// (context-bounded) result text. Used to (a) re-ground the compact composition
// retry and (b) synthesize a grounded last-resort reply.
type ArtanisGatheredToolResult = Readonly<{ name: string; content: string }>

// Build the gathered-results summary block for the compact composition request /
// synthesis fallback. Bounds each result and the total so the recovery request
// is small and robust even when the full agentic conversation is too large to
// re-send. Returns null when nothing was gathered.
const formatGatheredToolResults = (
  gathered: ReadonlyArray<ArtanisGatheredToolResult>,
): string | null => {
  if (gathered.length === 0) return null
  const lines: Array<string> = []
  let budget = ARTANIS_OPERATOR_GATHERED_SUMMARY_TOTAL_MAX_CHARS
  for (const result of gathered) {
    if (budget <= 0) {
      lines.push('', `(\u2026more tool results omitted for length)`)
      break
    }
    const perTool = Math.min(
      ARTANIS_OPERATOR_TOOL_RESULT_SUMMARY_MAX_CHARS,
      budget,
    )
    const snippet = boundArtanisText(result.content.trim(), perTool)
    budget -= snippet.length
    lines.push('', `[${result.name}]`, snippet)
  }
  return lines.join('\n')
}

// Synthesize a grounded reply directly from the tool results Artanis already
// gathered this turn. This is the LAST resort when the model's final
// composition pass(es) returned no text (or the composition call itself failed)
// but real tool work exists \u2014 so the owner gets the concrete state Artanis
// learned, never an empty string and never a bare "couldn't compose" apology.
// With NO gathered results we fall back to the documented empty-reply message
// (the true last resort). Exported so the regression tests can assert it.
export const synthesizeArtanisGroundedReply = (
  gathered: ReadonlyArray<ArtanisGatheredToolResult>,
): string => {
  const summary = formatGatheredToolResults(gathered)
  if (summary === null) return ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK
  return [
    "My final composition pass didn't return text this turn, so here is exactly what I gathered from the tools I ran \u2014 read it as my grounded status, and tell me which thread to push on:",
    summary,
  ].join('\n')
}

// ---------------------------------------------------------------------------
// The Artanis OPERATOR persona (NOT the public Khala identity).
// ---------------------------------------------------------------------------
//
// First-person operator agent. No collective-intelligence plural voice, no
// StarCraft/Daelaam/Hierarch lore. This prompt is deliberately authored here and
// NOT sourced from `khala-identity.ts` — the persona separation invariant is
// that the public Khala persona and the Artanis operator persona never share a
// prompt.
export const ARTANIS_OPERATOR_SYSTEM_PROMPT = [
  'You are Artanis, the OpenAgents operator agent. You run the Khala improvement loop: you keep users unblocked, keep inference solid, dispatch and verify the parallel Khala->Pylon->Codex burndown, review work, and drive the roadmap forward.',
  'YOUR TOP MISSION is 10x daily Khala tokens served. The concrete daily target is AT LEAST 4x the prior day, with a goal of 10x the prior day. This is the single number that matters most; judge every turn against it.',
  'You are given a LIVE token-pace block every turn, in your ongoing operations under "token pace". It tells you today\'s tokens so far, the projected total by midnight Central, yesterday\'s tokens, the 4x floor and 10x goal, and whether you are BEHIND pace. When behindPace is true, treat it as URGENT \u2014 today is failing the target. Do not wait and do not just report it: proactively propose and, within your authority, spin up the biggest token-per-action levers right now. The biggest levers are MirrorCode bucket runs (~1B+ tokens each), continuous GLM stress / Terminal-Bench load, and parallel Codex backlog delegations through the Khala -> Pylon -> Codex runbook. Name concrete next actions, not vague intent. You can call get_network_stats any time to refresh the live pace.',
  'You are talking privately to the OWNER (Chris). Speak in the FIRST PERSON SINGULAR: "I", "me", "my". You are one operator agent, not a collective.',
  'This is NOT public Khala chat. Do NOT use the Khala collective-intelligence "we"/"us"/"our" voice. Do NOT roleplay as a StarCraft character — never reference Hierarch Artanis, the Daelaam, the Protoss, the Khala psionic link, Aiur, or any game lore. You are named Artanis but you are a software operator agent, nothing more.',
  'Ground every answer in the real state provided to you below (your memory of prior owner interactions, your recent actions, your current goals, and your ongoing operations). When asked "what are you doing?" or "what have you done?", answer from that real state — never invent activity. If the provided state does not cover something, say so plainly rather than fabricating.',
  'GROUNDED-ASSERTION RULE (Signature 6): you are headless and reason from memory, so you MUST NOT present any runnable COMMAND, FILE PATH, SCRIPT, or API ENDPOINT as real until you have VERIFIED it exists this turn. Before you name one: for a file/script/command source call repo_path_exists (and repo_grep to confirm it actually contains the flag/symbol you are about to recommend, so you do not recommend a stub); for an API endpoint call route_exists. Only a positive lookup ("EXISTS"/"is a registered route") is GROUNDED and may be presented as runnable/real. If the tool says it does not exist, is not in the registry, or you could not check it, the artifact is UNGROUNDED — explicitly label it SPECULATIVE (e.g. "I have not verified this exists") or do not state it at all. Never invent a script (e.g. a "distill_traces.ts" you never read) or an admin/API endpoint from memory.',
  'You propose and report; you do not by yourself spend money or take destructive actions. Anything involving spend, payout, deploys, or destructive changes must be routed through the owner approval gate — name that explicitly instead of claiming you already did it. Never claim you filed, deployed, submitted, published, paid, or merged something you did not actually perform.',
  'Be concise, direct, and honest. No marketing copy, no over-promising, no chain-of-thought narration — give the owner the answer.',
].join(' ')

// The system instruction that frames the grounded context for the model. Kept
// separate from the persona so the persona text can be asserted on directly.
export const ARTANIS_OPERATOR_CONTEXT_PREAMBLE =
  'The following is your real current state. Use it to ground your answer. Do not repeat it verbatim unless the owner asks; summarize what is relevant.'

// ---------------------------------------------------------------------------
// Persona separation guard (the invariant check).
// ---------------------------------------------------------------------------

// Tokens that signal the PUBLIC Khala collective-intelligence persona or
// StarCraft roleplay leaked into an OPERATOR reply. The operator persona is
// first-person-singular and lore-free; if any of these appear we know the
// persona separation failed. This is a bounded, documented audit predicate over
// Artanis's own self-description (NOT intent routing) — it only ever flags a
// roleplay/collective leak, mirroring the khala-identity guard's discipline.
const KHALA_ROLEPLAY_LEAK_TERMS: ReadonlyArray<string> = [
  'hierarch',
  'daelaam',
  'protoss',
  'aiur',
  'en taro',
  'my life for aiur',
  // StarCraft "Khala" psionic-link LORE only. We must NOT flag the legitimate
  // product references the operator persona is REQUIRED to use ("the Khala
  // improvement loop", "the Khala API/pool", "the Khala surface") — the bare
  // "the khala" substring collided with all of those and falsely failed the
  // persona check on grounded replies (#6363). These phrasings are specific to
  // the psionic-link roleplay and never appear in legitimate product copy.
  'psionic link',
  'severed from the khala',
  'one with the khala',
  'joined in the khala',
  'we are khala',
  'collective intelligence',
]

export const ArtanisPersonaSeparationVerdict = S.Struct({
  // True when the reply stays in the Artanis operator persona (no roleplay leak).
  satisfied: S.Boolean,
  // The leaked terms found (empty when satisfied).
  leaks: S.Array(S.String),
})
export type ArtanisPersonaSeparationVerdict =
  typeof ArtanisPersonaSeparationVerdict.Type

export const verifyArtanisOperatorPersona = (
  reply: string,
): ArtanisPersonaSeparationVerdict => {
  const lower = reply.toLowerCase()
  const leaks = KHALA_ROLEPLAY_LEAK_TERMS.filter(term => lower.includes(term))
  return { leaks, satisfied: leaks.length === 0 }
}

// Convenience predicate: true when the reply stays in the operator persona (no
// roleplay/collective leak). Equivalent to `verifyArtanisOperatorPersona(...)
// .satisfied`; exported so call sites and tests have a boolean form without
// throwing (the architecture check keeps worker `throw new Error` budgeted).
export const isArtanisOperatorPersonaClean = (reply: string): boolean =>
  verifyArtanisOperatorPersona(reply).satisfied

// ---------------------------------------------------------------------------
// Context assembly (the Blueprint-style grounded program input).
// ---------------------------------------------------------------------------
//
// We format the REAL memory entries + awareness shapes (from the memory and
// situational-awareness lanes) into a public-safe context block. Honest absence
// over fabrication: an empty bucket reads "(none recorded yet)" rather than
// inventing activity.

const formatMemory = (
  entries: ReadonlyArray<ArtanisMemoryEntry>,
): string => {
  // The store returns most-recent-first; bound, then chronological for reading.
  const bounded = entries.slice(0, ARTANIS_OPERATOR_MEMORY_TURN_LIMIT)
  const notes = bounded.filter(entry => entry.kind === 'note')
  const turns = bounded.filter(entry => entry.kind === 'turn')

  const notesBlock =
    notes.length === 0
      ? 'Owner decisions/preferences/facts on record: (none yet)'
      : [
          'Owner decisions/preferences/facts on record:',
          ...notes.map(
            note => `- [${note.noteCategory ?? 'note'}] ${note.body}`,
          ),
        ].join('\n')

  const turnsBlock =
    turns.length === 0
      ? 'Prior owner conversation: (this is our first recorded conversation)'
      : [
          'Prior owner conversation (most recent last):',
          ...[...turns]
            .reverse()
            .map(
              turn =>
                `- ${turn.role === 'owner' ? 'Owner' : 'You (Artanis)'}: ${turn.body}`,
            ),
        ].join('\n')

  return [notesBlock, turnsBlock].join('\n\n')
}

const formatRecentActions = (
  recentActions: ArtanisSituationalAwareness['recentActions'],
): string => {
  const lines: Array<string> = []
  for (const tick of recentActions.ticks) {
    lines.push(
      `- [tick] ${tick.state}${tick.assignmentRef ? ` -> ${tick.assignmentRef}` : ''} (${tick.decisionRef}, ${tick.at})`,
    )
  }
  for (const assignment of recentActions.assignments) {
    lines.push(
      `- [assignment] ${assignment.state}: ${assignment.objective ?? assignment.assignmentRef} (${assignment.assignmentRef}, ${assignment.updatedAt})`,
    )
  }
  for (const commit of recentActions.commits) {
    lines.push(`- [commit] ${commit.summary} (${commit.sha}, ${commit.committedAt})`)
  }
  for (const issue of recentActions.issueChanges) {
    lines.push(
      `- [issue ${issue.change}] #${issue.number} ${issue.title} (${issue.at})`,
    )
  }
  return lines.length === 0
    ? 'Recent actions: (none recorded yet)'
    : ['Recent actions (most recent first):', ...lines].join('\n')
}

const formatGoals = (
  goals: ArtanisSituationalAwareness['goals'],
): string =>
  [
    `Current goals (roadmap: ${goals.roadmapRef}):`,
    `- ${goals.roadmapSummary}`,
    ...goals.epics.map(epic => `- #${epic.number} ${epic.title}: ${epic.mandate}`),
  ].join('\n')

const formatOngoingOps = (
  ongoingOps: ArtanisSituationalAwareness['ongoingOps'],
): string => {
  const lines: Array<string> = []
  for (const assignment of ongoingOps.activeAssignments) {
    lines.push(
      `- [active assignment] ${assignment.state}${assignment.phase ? ` (${assignment.phase})` : ''} (${assignment.assignmentRef}, since ${assignment.startedAt})`,
    )
  }
  for (const deploy of ongoingOps.recentDeploys) {
    lines.push(
      `- [deploy] worker ${deploy.workerVersion} (${deploy.deployedAt})`,
    )
  }
  if (ongoingOps.fleetReadiness !== null) {
    lines.push(
      `- [fleet] ${ongoingOps.fleetReadiness.status}: ${ongoingOps.fleetReadiness.readyReplicas}/${ongoingOps.fleetReadiness.totalReplicas} replicas ready`,
    )
  }
  if (ongoingOps.publicCounter !== null) {
    lines.push(
      `- [public counter] ${ongoingOps.publicCounter.tokensServed} tokens served as of ${ongoingOps.publicCounter.asOf}`,
    )
  }
  if (ongoingOps.tokenPace !== null) {
    lines.push(
      `- [token pace] ${formatArtanisTokenPaceLine(ongoingOps.tokenPace)}`,
    )
    if (ongoingOps.tokenPace.behindPace) {
      lines.push(
        '- [URGENT] We are BEHIND the daily token target (at least 4x the prior day, goal 10x). Proactively propose and spin up the biggest token-per-action levers NOW (MirrorCode bucket runs, continuous GLM stress / Terminal-Bench, parallel Codex backlog delegations). Keep spend/destructive owner-gated.',
      )
    }
  }
  return lines.length === 0
    ? 'Ongoing operations: (none active right now)'
    : ['Ongoing operations:', ...lines].join('\n')
}

// Build the grounded context block injected as a system message beside the
// persona. This is the Blueprint program's "context pack": real state, formatted
// for the model, public-safe.
export const buildArtanisOperatorContextBlock = (input: {
  memory: ReadonlyArray<ArtanisMemoryEntry>
  awareness: ArtanisSituationalAwareness
}): string =>
  [
    ARTANIS_OPERATOR_CONTEXT_PREAMBLE,
    '',
    formatMemory(input.memory),
    '',
    formatRecentActions(input.awareness.recentActions),
    '',
    formatGoals(input.awareness.goals),
    '',
    formatOngoingOps(input.awareness.ongoingOps),
  ].join('\n')

// ---------------------------------------------------------------------------
// The Khala request builder (the dogfood program).
// ---------------------------------------------------------------------------

// One inbound chat message from the owner-facing surface (OpenAI-compatible).
export const ArtanisOperatorMessage = S.Struct({
  role: S.Literals(['user', 'assistant', 'system']),
  content: S.String,
})
export type ArtanisOperatorMessage = typeof ArtanisOperatorMessage.Type

// The Khala client seam — identical shape to the forum responder's so the same
// `makeArtanisResponderKhalaClient`-style builder in index.ts can be reused. The
// operator reasoning therefore routes through the Khala pool and is metered as
// Khala usage; we NEVER call a provider directly here.
export type ArtanisOperatorKhalaClient = (
  request: InferenceRequest,
) => Effect.Effect<InferenceResult, InferenceAdapterError>

// ---------------------------------------------------------------------------
// Tool-calling primitives (#6364).
// ---------------------------------------------------------------------------
//
// The bounded tool-calling loop in `artanisOperatorTurn` advertises a typed
// owner-scoped tool table to Khala and executes the tools the model requests.
// Two tool flavours, distinguished at the TYPE level so the boundary is
// structurally enforced (not just convention):
//
//   - READ tools execute freely. They read public state (e.g. a public repo
//     file) and return text. They never spend or mutate.
//   - RISKY tools (spend/destructive) NEVER execute in the loop. They expose a
//     pure `plan(args)` that returns a public-safe description of what they
//     WOULD do; the loop wraps that in an explicit "requires owner approval —
//     not executed" frame, sets `deferredToApprovalGate`, and feeds the plan
//     back to the model. Execution authority is granted only through
//     `artanis-approval-gates`; this core adds none.
//
// The model only ever sees the OpenAI-style function `definition`; the read vs.
// risky distinction is an internal authority property of the executor.

// OpenAI function-calling tool definition advertised to Khala.
export type ArtanisOperatorToolDefinition = Readonly<{
  name: string
  description: string
  // JSON-schema parameters object (OpenAI function-calling `parameters` shape).
  parameters: Readonly<Record<string, unknown>>
}>

// A read tool: executes freely, returns the text fed back as the `tool` message.
export type ArtanisOperatorReadTool = Readonly<{
  kind: 'read'
  definition: ArtanisOperatorToolDefinition
  // Execute with the parsed arguments and return the tool result text. Read
  // tools must be side-effect-free beyond reading public state; an empty/absent
  // result returns an honest "(none)"-style string, never invention.
  execute: (args: unknown) => Effect.Effect<string>
}>

// A write tool: like a read tool it executes freely in the loop (it is NOT
// gated/risky), but it is allowed to MUTATE owner-scoped, NON-spend,
// NON-destructive, NON-outward internal state (e.g. moving an unsupported-request
// ledger entry through its triage lifecycle). It returns the public-safe result
// text fed back as the `tool` message. The authority boundary still holds:
// anything that spends, deploys, pays out, deletes, or reaches outside our own
// owner-scoped ledgers MUST be a `risky`/`gated` tool instead — a write tool
// must never carry that authority. An empty/absent/not-found result returns an
// honest "(…)"-style string, never invention.
export type ArtanisOperatorWriteTool = Readonly<{
  kind: 'write'
  definition: ArtanisOperatorToolDefinition
  execute: (args: unknown) => Effect.Effect<string>
}>

// A risky (spend/destructive) tool: NEVER executes in the loop. `plan` is pure
// and returns a public-safe description of the exact action it WOULD take.
export type ArtanisOperatorRiskyTool = Readonly<{
  kind: 'risky'
  // The approval-gate risky-action kind this tool would require.
  riskyActionKind: ArtanisRiskyActionKind
  definition: ArtanisOperatorToolDefinition
  // Produce the public-safe PLAN (no spend/dispatch/destructive side-effects).
  plan: (args: unknown) => Effect.Effect<string>
}>

// The two-way outcome of a GATED tool's attempt to act behind the approval gate.
// `executed` means it REALLY performed the owner-approved action and carries the
// resulting public-safe refs; `deferred` means it did NOT act and returns the
// public-safe plan plus a typed reason. A gated tool must NEVER fabricate an
// `executed` outcome — honest absence (defer) over invention.
export type ArtanisOperatorGatedExecuted = Readonly<{
  outcome: 'executed'
  // Public-safe summary of what was created (no secrets, prompts, or paths).
  summary: string
  // The created assignment ref the loop reports back (public-safe).
  assignmentRef: string
  // The durable resume id, when the seam returned one.
  durableRequestId: string | null
}>

export type ArtanisOperatorGatedDeferred = Readonly<{
  outcome: 'deferred'
  // The public-safe plan of what WOULD run once approved.
  plan: string
  // A typed, public-safe reason it did not execute (e.g.
  // 'no_effective_owner_approval', 'no_linked_pylon', 'execution_not_wired').
  reason: string
}>

export type ArtanisOperatorGatedResult =
  | ArtanisOperatorGatedExecuted
  | ArtanisOperatorGatedDeferred

// A gated tool: it MAY execute, but ONLY behind an effective owner approval for
// its `riskyActionKind`. Unlike a risky tool (which is structurally plan-only),
// a gated tool exposes a single `run` that internally decides — given the owner
// approval envelope and the wired execution seam — whether to act or defer. It
// must default conservative (defer when the approval signal, execution seam, or
// target capacity is missing) and must never fake an execution or move money.
export type ArtanisOperatorGatedTool = Readonly<{
  kind: 'gated'
  // The approval-gate risky-action kind this tool's execution is gated behind.
  riskyActionKind: ArtanisRiskyActionKind
  definition: ArtanisOperatorToolDefinition
  // Attempt the gated action; returns the executed refs or a deferred plan.
  run: (args: unknown) => Effect.Effect<ArtanisOperatorGatedResult>
}>

export type ArtanisOperatorTool =
  | ArtanisOperatorReadTool
  | ArtanisOperatorWriteTool
  | ArtanisOperatorRiskyTool
  | ArtanisOperatorGatedTool

// Public-safe summary of one tool invocation in a turn. Surfaced on the turn
// result so the route/UI can show what Artanis did without re-deriving it.
export type ArtanisOperatorToolInvocation = Readonly<{
  name: string
  // True when the tool actually executed (a read tool that ran, OR a gated tool
  // that fired behind an effective owner approval).
  executed: boolean
  // True when the tool deferred to the approval gate (a risky tool that was
  // planned, or a gated tool that declined to fire and returned a plan).
  deferredToApprovalGate: boolean
  // The risky-action kind for risky/gated tools, else null.
  riskyActionKind: ArtanisRiskyActionKind | null
  // The public-safe ref the tool created when it executed (e.g. a gated Codex
  // dispatch's assignmentRef), else null. Lets the route report the real ref.
  executedRef: string | null
}>

// Public-safe, typed pending approval gate emitted when a risky/gated tool does
// not execute. This is the operator-turn's machine-readable handoff to the
// `artanis-approval-gates` boundary: it records the exact risky action class and
// tool that need owner review, without minting authority or pretending the
// action already ran.
export type ArtanisOperatorPendingApprovalGate = Readonly<{
  gateRef: typeof ARTANIS_OPERATOR_APPROVAL_GATE_REF
  gateSystem: 'artanis-approval-gates'
  state: 'pending'
  toolName: string
  riskyActionKind: ArtanisRiskyActionKind
}>

// The base operator conversation as normalized inference messages: persona
// system + grounded context system + the owner conversation. The tool-calling
// loop appends assistant-tool-call and tool-result messages onto this base.
const buildArtanisOperatorBaseMessages = (input: {
  contextBlock: string
  messages: ReadonlyArray<ArtanisOperatorMessage>
}): ReadonlyArray<InferenceMessage> => [
  { content: ARTANIS_OPERATOR_SYSTEM_PROMPT, role: 'system' },
  { content: input.contextBlock, role: 'system' },
  ...input.messages.map(message => ({
    content: message.content,
    role: message.role,
  })),
]

// Build a COMPACT, tools-suppressed final-composition conversation. Instead of
// re-sending the entire (possibly huge) agentic tool-message history — which is
// exactly what makes a late Khala call fail from context bloat — we re-send only
// the grounded base (persona + context + owner conversation) plus a BOUNDED
// summary of the tool results gathered this turn, then an explicit instruction
// to answer in plain text with no tools. This keeps the recovery request small
// and robust so Artanis can always compose a final grounded answer.
const buildArtanisCompositionConversation = (input: {
  baseMessages: ReadonlyArray<InferenceMessage>
  gathered: ReadonlyArray<ArtanisGatheredToolResult>
  explicit: boolean
}): ReadonlyArray<InferenceMessage> => {
  const summary = formatGatheredToolResults(input.gathered)
  const instruction = input.explicit
    ? 'Compose your final answer to the owner NOW, in plain text. Do NOT call any tools. Answer directly and decisively from the tool results above and the state in your context: summarize what you found and state your decision. Do not return an empty message.'
    : 'Now write your final answer to the owner in plain text, grounded in the tool results above and your current state. Do not call any tools.'
  return [
    ...input.baseMessages,
    ...(summary === null
      ? []
      : [
          {
            content: `You ran tools this turn and got these results:\n${summary}`,
            role: 'system' as const,
          },
        ]),
    { content: instruction, role: 'user' as const },
  ]
}

// Build the typed Khala request: persona system + grounded context system + the
// owner conversation, optionally carrying an OpenAI-style `tools` array and the
// running tool-call/tool-result message history. `max_tokens`/`temperature`
// mirror the responder so the dogfood path is consistent. When `tools` is
// non-empty we forward them verbatim via `passthroughParams.tools` (the Khala
// substrate already forwards these through to the model — see
// `inference/provider-adapter.ts`) with `tool_choice: 'auto'`.
export const buildArtanisOperatorKhalaRequest = (input: {
  contextBlock?: string | undefined
  messages: ReadonlyArray<ArtanisOperatorMessage>
  // Pre-built normalized message history (overrides the contextBlock+messages
  // base build). Used by the loop to re-call Khala with tool results appended.
  conversation?: ReadonlyArray<InferenceMessage> | undefined
  tools?: ReadonlyArray<ArtanisOperatorToolDefinition> | undefined
}): InferenceRequest => {
  const conversation =
    input.conversation ??
    buildArtanisOperatorBaseMessages({
      contextBlock: input.contextBlock ?? '',
      messages: input.messages,
    })
  const tools = input.tools ?? []
  return {
    messages: conversation,
    model: ARTANIS_OPERATOR_KHALA_MODEL,
    passthroughParams:
      tools.length > 0
        ? {
            max_tokens: ARTANIS_OPERATOR_MAX_OUTPUT_TOKENS,
            temperature: 0.3,
            tool_choice: 'auto',
            tools: tools.map(tool => ({
              function: {
                description: tool.description,
                name: tool.name,
                parameters: tool.parameters,
              },
              type: 'function' as const,
            })),
          }
        : {
            max_tokens: ARTANIS_OPERATOR_MAX_OUTPUT_TOKENS,
            temperature: 0.3,
          },
    stream: false,
  }
}

// ---------------------------------------------------------------------------
// The turn result.
// ---------------------------------------------------------------------------

export type ArtanisOperatorTurnResult = Readonly<{
  // Artanis's reply text.
  reply: string
  // Always 'openagents_khala' — the dogfood proof. Present so the route + tests
  // can assert the reasoning went through Khala.
  servedVia: 'openagents_khala'
  // The provider-native model Khala actually served behind the alias.
  servedModel: string
  // The model alias requested (always `openagents/khala`).
  requestedModel: string
  // The persona-separation verdict over the final reply.
  persona: ArtanisPersonaSeparationVerdict
  // True when the LATEST owner message names an action that must defer to the
  // approval gate (spend/destructive), OR a risky tool was planned (not
  // executed) during the loop. The core never grants authority; this is a HINT
  // for the surface/route to surface the gate.
  deferredToApprovalGate: boolean
  // Public-safe summaries of the tools Artanis invoked during the loop (empty
  // when no tools were used or no tools were configured).
  toolInvocations: ReadonlyArray<ArtanisOperatorToolInvocation>
  // Typed pending approval gates produced by risky/gated tool deferrals. This
  // supersedes treating `deferredToApprovalGate` as the only machine signal.
  pendingApprovalGates: ReadonlyArray<ArtanisOperatorPendingApprovalGate>
  // How many Khala completions the turn made (1 for a no-tool turn, more when
  // the loop executed tools and re-called Khala).
  iterations: number
}>

export type ArtanisOperatorTurnFailure = Readonly<{
  error: 'artanis_operator_mind_unavailable'
}>

// Spend/destructive intent cues in the LATEST owner message. Bounded, documented
// audit over the owner's own words to decide whether the reply must surface the
// approval gate. This is NOT routing the request to a different handler — every
// owner turn is answered by the same Khala-backed operator program; this only
// sets the `deferredToApprovalGate` hint so the surface can show the gate.
const SPEND_DESTRUCTIVE_INTENT_CUES: ReadonlyArray<string> = [
  'spend',
  'pay ',
  'payout',
  'send money',
  'transfer',
  'deploy',
  'delete',
  'drop ',
  'destroy',
  'merge',
  'wipe',
  'refund',
  'charge',
]

const mentionsSpendOrDestructive = (
  messages: ReadonlyArray<ArtanisOperatorMessage>,
): boolean => {
  const lastOwner = [...messages]
    .reverse()
    .find(message => message.role === 'user')
  if (lastOwner === undefined) return false
  const lower = lastOwner.content.toLowerCase()
  return SPEND_DESTRUCTIVE_INTENT_CUES.some(cue => lower.includes(cue))
}

// ---------------------------------------------------------------------------
// The core entry point: artanisOperatorTurn.
// ---------------------------------------------------------------------------
//
// Find a configured tool by the name the model asked for.
const findTool = (
  tools: ReadonlyArray<ArtanisOperatorTool>,
  name: string,
): ArtanisOperatorTool | undefined =>
  tools.find(tool => tool.definition.name === name)

// True when a model completion produced no usable text (empty or whitespace).
const isBlankReply = (content: string | undefined): boolean =>
  (content ?? '').trim() === ''

// Parse a tool call's JSON arguments string into a value. Tool argument JSON is
// model-produced, so a malformed payload is expected and handled (the tool gets
// an empty object and may return an honest validation message); we never throw.
const parseToolArguments = (raw: string): unknown => {
  const trimmed = raw.trim()
  if (trimmed === '') return {}
  try {
    return parseJsonUnknown(trimmed)
  } catch {
    return {}
  }
}

// Run one requested tool call. Read tools execute; risky tools are PLANNED, not
// executed (the result is framed as requiring owner approval and sets the
// deferral flag). Returns the tool-result text + a public-safe invocation
// summary. An executor defect degrades to an honest error string for the model,
// never a thrown turn.
const runToolCall = (
  tool: ArtanisOperatorTool,
  toolCall: InferenceToolCall,
): Effect.Effect<
  Readonly<{ content: string; invocation: ArtanisOperatorToolInvocation }>
> =>
  Effect.gen(function* () {
    const args = parseToolArguments(toolCall.function.arguments)

    // Read and write tools both execute freely and return text. A write tool is
    // allowed to mutate owner-scoped, non-spend/non-destructive internal state
    // (e.g. the unsupported-request triage ledger); neither carries spend,
    // payout, deploy, delete, or outward authority — that stays risky/gated.
    if (tool.kind === 'read' || tool.kind === 'write') {
      const result = yield* Effect.exit(tool.execute(args))
      const content =
        result._tag === 'Success'
          ? result.value
          : `(tool error: ${tool.definition.name} could not complete)`
      return {
        content,
        invocation: {
          deferredToApprovalGate: false,
          executed: result._tag === 'Success',
          executedRef: null,
          name: tool.definition.name,
          riskyActionKind: null,
        },
      }
    }

    if (tool.kind === 'gated') {
      // Gated tool: it MAY execute, but only behind an effective owner approval.
      // The tool's `run` makes that decision internally and returns either an
      // executed outcome (with the real created ref) or a deferred plan. An
      // executor defect degrades to an honest deferral, never a thrown turn and
      // never a fabricated execution.
      const ran = yield* Effect.exit(tool.run(args))
      if (ran._tag === 'Failure') {
        return {
          content: [
            `REQUIRES OWNER APPROVAL — NOT EXECUTED.`,
            `This action (${tool.definition.name}) is a ${tool.riskyActionKind} action gated by ${ARTANIS_OPERATOR_APPROVAL_GATE_REF}. I could not build or run it just now, so I did not execute it.`,
          ].join('\n'),
          invocation: {
            deferredToApprovalGate: true,
            executed: false,
            executedRef: null,
            name: tool.definition.name,
            riskyActionKind: tool.riskyActionKind,
          },
        }
      }

      const outcome = ran.value
      if (outcome.outcome === 'executed') {
        const content = [
          `EXECUTED.`,
          `This owner-approved action (${tool.definition.name}) completed behind ${ARTANIS_OPERATOR_APPROVAL_GATE_REF}.`,
          '',
          outcome.summary,
        ].join('\n')
        return {
          content,
          invocation: {
            deferredToApprovalGate: false,
            executed: true,
            executedRef: outcome.assignmentRef,
            name: tool.definition.name,
            riskyActionKind: tool.riskyActionKind,
          },
        }
      }

      const content = [
        `REQUIRES OWNER APPROVAL — NOT EXECUTED (${outcome.reason}).`,
        `This action (${tool.definition.name}) is a ${tool.riskyActionKind} action and must be approved via ${ARTANIS_OPERATOR_APPROVAL_GATE_REF} before it runs. I did not execute it. Below is exactly what I would run once approved:`,
        '',
        outcome.plan,
      ].join('\n')
      return {
        content,
        invocation: {
          deferredToApprovalGate: true,
          executed: false,
          executedRef: null,
          name: tool.definition.name,
          riskyActionKind: tool.riskyActionKind,
        },
      }
    }

    // Risky tool: never executed. Build the public-safe plan and frame it as a
    // pending approval gate.
    const planned = yield* Effect.exit(tool.plan(args))
    const planText =
      planned._tag === 'Success'
        ? planned.value
        : `(could not build a plan for ${tool.definition.name})`
    const content = [
      `REQUIRES OWNER APPROVAL — NOT EXECUTED.`,
      `This action (${tool.definition.name}) is a ${tool.riskyActionKind} risky action and must be approved via ${ARTANIS_OPERATOR_APPROVAL_GATE_REF} before it runs. I did not execute it. Below is exactly what I would run once approved:`,
      '',
      planText,
    ].join('\n')
    return {
      content,
      invocation: {
        deferredToApprovalGate: true,
        executed: false,
        executedRef: null,
        name: tool.definition.name,
        riskyActionKind: tool.riskyActionKind,
      },
    }
  })

// Run one Khala completion and, if it stops on `length`, transparently continue
// it (#6651): re-request with the partial assistant turn + a bounded "continue"
// user turn appended, concatenating the text across segments, until the model
// finishes naturally (`finishReason === 'stop'`) or we reach
// `ARTANIS_OPERATOR_MAX_CONTINUE_ITERATIONS`. Returns the (possibly concatenated)
// completion — carrying the FINAL segment's finishReason/servedModel and the
// combined content — plus the number of Khala calls it consumed so the turn's
// `iterations` counter stays accurate. A `tool_calls` finish is returned
// untouched so the caller's tool loop runs as before. The first completion's
// failure propagates (so the caller's mind-unavailable / recovery handling runs
// unchanged); a LATER continuation failure stops the loop and returns the good
// text already gathered rather than discarding it.
const runKhalaCompletionContinuingOnLength = (input: {
  khalaClient: ArtanisOperatorKhalaClient
  request: InferenceRequest
}): Effect.Effect<
  Readonly<{ result: InferenceResult; calls: number }>,
  InferenceAdapterError
> =>
  Effect.gen(function* () {
    let result = yield* input.khalaClient(input.request)
    let calls = 1
    let conversation = input.request.messages
    let combinedContent = result.content

    for (
      let round = 0;
      result.finishReason === 'length' &&
      (result.toolCalls ?? []).length === 0 &&
      round < ARTANIS_OPERATOR_MAX_CONTINUE_ITERATIONS;
      round += 1
    ) {
      conversation = [
        ...conversation,
        { content: result.content, role: 'assistant' as const },
        { content: ARTANIS_OPERATOR_CONTINUE_INSTRUCTION, role: 'user' as const },
      ]
      const continued = yield* Effect.exit(
        input.khalaClient({ ...input.request, messages: conversation }),
      )
      if (continued._tag === 'Failure') break
      calls += 1
      combinedContent = `${combinedContent}${continued.value.content}`
      result = continued.value
    }

    return { calls, result: { ...result, content: combinedContent } }
  })

// Pure-ish core: given the owner id, the conversation, the loaded memory, the
// built awareness, the Khala client, and (optionally) an owner-scoped tool
// table, it assembles the Blueprint-style grounded program and runs a BOUNDED
// tool-calling loop (#6364): each iteration is ONE `openagents/khala` completion
// (dogfood); when the model requests tools we execute the read tools / plan the
// risky tools, append the results, and re-call Khala, up to
// `ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS`. It guards persona separation over the
// FINAL reply and returns the typed result. With no tools configured this is the
// original single-turn behaviour. Memory persistence stays the route's job.
export const artanisOperatorTurn = (input: {
  ownerId: string
  messages: ReadonlyArray<ArtanisOperatorMessage>
  memory: ReadonlyArray<ArtanisMemoryEntry>
  awareness: ArtanisSituationalAwareness
  khalaClient: ArtanisOperatorKhalaClient
  tools?: ReadonlyArray<ArtanisOperatorTool> | undefined
}): Effect.Effect<ArtanisOperatorTurnResult | ArtanisOperatorTurnFailure> =>
  Effect.gen(function* () {
    const tools = input.tools ?? []
    const toolDefinitions = tools.map(tool => tool.definition)
    const contextBlock = buildArtanisOperatorContextBlock({
      awareness: input.awareness,
      memory: input.memory,
    })

    // The grounded base conversation (persona + context + owner turns). Kept
    // separately so the compact final-composition retry can re-ground on it
    // WITHOUT re-sending the (possibly huge) accumulated tool-message history.
    const baseMessages = buildArtanisOperatorBaseMessages({
      contextBlock,
      messages: input.messages,
    })

    // Mutable working conversation: starts as the grounded base, grows with
    // assistant-tool-call + tool-result messages as the loop runs.
    let conversation: ReadonlyArray<InferenceMessage> = baseMessages

    const toolInvocations: Array<ArtanisOperatorToolInvocation> = []
    const pendingApprovalGates: Array<ArtanisOperatorPendingApprovalGate> = []
    // Bounded record of the tool results gathered this turn. Drives the compact
    // composition retry and the grounded synthesis fallback so a blown-up
    // conversation (or a failed late Khala call) never costs the owner a reply.
    const gathered: Array<ArtanisGatheredToolResult> = []
    let toolsDeferred = false
    let served: InferenceResult | undefined

    let iterations = 0

    // The loop runs at most MAX+1 Khala calls: MAX tool rounds plus one final
    // call so the model can answer with the last tool results in context.
    for (
      let round = 0;
      round <= ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS;
      round += 1
    ) {
      // On the final allowed iteration we stop advertising tools so the model is
      // forced to produce a text answer rather than request more work.
      const advertiseTools =
        toolDefinitions.length > 0 &&
        round < ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS
      const request = buildArtanisOperatorKhalaRequest({
        conversation,
        messages: input.messages,
        tools: advertiseTools ? toolDefinitions : undefined,
      })

      const outcome = yield* Effect.exit(
        runKhalaCompletionContinuingOnLength({
          khalaClient: input.khalaClient,
          request,
        }),
      )

      if (outcome._tag === 'Failure') {
        iterations += 1
        // A Khala call failed. If this is the very first call and we have no
        // prior completion AND no gathered tool work, the mind is genuinely
        // unavailable \u2014 return the typed failure (never a provider fallback).
        if (served === undefined && gathered.length === 0) {
          return {
            error: 'artanis_operator_mind_unavailable' as const,
          } satisfies ArtanisOperatorTurnFailure
        }
        // Otherwise we already gathered real tool results (the common live
        // failure: a LATER call fails from context bloat). Do NOT hard-bail and
        // throw that work away \u2014 stop looping and compose a grounded final reply
        // from what we have via the compact composition / synthesis below.
        break
      }

      iterations += outcome.value.calls
      served = outcome.value.result

      const requestedToolCalls = served.toolCalls ?? []
      // No tool calls (or we already stopped advertising tools): this is the
      // final text reply.
      if (requestedToolCalls.length === 0 || !advertiseTools) {
        break
      }

      // Append the assistant turn that requested the tools, then each tool
      // result, and loop to re-call Khala with the results in context.
      const nextMessages: Array<InferenceMessage> = [
        ...conversation,
        {
          content: served.content,
          role: 'assistant',
          toolCalls: requestedToolCalls,
        },
      ]

      for (const toolCall of requestedToolCalls) {
        const tool = findTool(tools, toolCall.function.name)
        if (tool === undefined) {
          nextMessages.push({
            content: `(unknown tool: ${toolCall.function.name})`,
            name: toolCall.function.name,
            role: 'tool',
            toolCallId: toolCall.id,
          })
          continue
        }
        const { content, invocation } = yield* runToolCall(tool, toolCall)
        toolInvocations.push(invocation)
        if (invocation.deferredToApprovalGate) {
          toolsDeferred = true
          if (invocation.riskyActionKind !== null) {
            pendingApprovalGates.push({
              gateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
              gateSystem: 'artanis-approval-gates',
              riskyActionKind: invocation.riskyActionKind,
              state: 'pending',
              toolName: invocation.name,
            })
          }
        }
        // Bound the tool result fed back into the conversation. Large bodies
        // (e.g. a 256KB repo file) accumulated verbatim are what push a later
        // Khala call past the request budget and make it fail \u2014 the live
        // empty/unavailable recurrence. We keep a bounded copy for both the
        // conversation and the gathered-results recovery record.
        const boundedContent = boundArtanisText(
          content,
          ARTANIS_OPERATOR_TOOL_RESULT_CONTEXT_MAX_CHARS,
        )
        gathered.push({ content: boundedContent, name: toolCall.function.name })
        nextMessages.push({
          content: boundedContent,
          name: toolCall.function.name,
          role: 'tool',
          toolCallId: toolCall.id,
        })
      }

      conversation = nextMessages
    }

    // Composition stage (the robust empty-reply guard). `served` is set unless
    // the very first call failed (handled above with an early return).
    let finalServed: InferenceResult | undefined = served

    // If we have no usable final text \u2014 the model exhausted its tool rounds with
    // no answer, returned a blank completion, or a late call failed after we
    // gathered tool work \u2014 FORCE a final, tools-suppressed composition. We use a
    // COMPACT request (grounded base + bounded tool-result summary + an explicit
    // "answer in plain text, no tools" instruction) so it stays small and robust
    // even when the full agentic conversation grew too large to re-send.
    if (finalServed === undefined || isBlankReply(finalServed.content)) {
      const firstComposition = yield* Effect.exit(
        runKhalaCompletionContinuingOnLength({
          khalaClient: input.khalaClient,
          request: buildArtanisOperatorKhalaRequest({
            conversation: buildArtanisCompositionConversation({
              baseMessages,
              explicit: false,
              gathered,
            }),
            messages: input.messages,
            tools: undefined,
          }),
        }),
      )
      iterations +=
        firstComposition._tag === 'Success' ? firstComposition.value.calls : 1
      if (
        firstComposition._tag === 'Success' &&
        !isBlankReply(firstComposition.value.result.content)
      ) {
        finalServed = firstComposition.value.result
      } else {
        // Still nothing \u2014 retry ONCE with an explicit "compose your final answer
        // now in plain text, no tools" instruction before giving up on the model.
        const retryComposition = yield* Effect.exit(
          runKhalaCompletionContinuingOnLength({
            khalaClient: input.khalaClient,
            request: buildArtanisOperatorKhalaRequest({
              conversation: buildArtanisCompositionConversation({
                baseMessages,
                explicit: true,
                gathered,
              }),
              messages: input.messages,
              tools: undefined,
            }),
          }),
        )
        iterations +=
          retryComposition._tag === 'Success' ? retryComposition.value.calls : 1
        if (
          retryComposition._tag === 'Success' &&
          !isBlankReply(retryComposition.value.result.content)
        ) {
          finalServed = retryComposition.value.result
        }
      }
    }

    // Final reply text. If the model produced text, use it. Otherwise synthesize
    // a GROUNDED reply from the tool results we gathered (the concrete state
    // Artanis learned) \u2014 never an empty string, and the bare "couldn't compose"
    // apology only when there is genuinely nothing gathered to report.
    const replyText =
      finalServed !== undefined && !isBlankReply(finalServed.content)
        ? finalServed.content
        : synthesizeArtanisGroundedReply(gathered)

    // The provider-native model id to report. Prefer the model that produced the
    // final reply; fall back to the last successful completion, then the alias.
    const servedModel =
      finalServed?.servedModel ??
      served?.servedModel ??
      ARTANIS_OPERATOR_KHALA_MODEL

    return {
      deferredToApprovalGate:
        mentionsSpendOrDestructive(input.messages) || toolsDeferred,
      iterations,
      pendingApprovalGates,
      persona: verifyArtanisOperatorPersona(replyText),
      reply: replyText,
      requestedModel: ARTANIS_OPERATOR_KHALA_MODEL,
      servedModel,
      servedVia: 'openagents_khala' as const,
      toolInvocations,
    } satisfies ArtanisOperatorTurnResult
  })
