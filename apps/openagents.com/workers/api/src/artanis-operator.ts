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
import type { ArtanisMemoryEntry } from './artanis-owner-memory'
import type { ArtanisSituationalAwareness } from './artanis-situational-awareness'
import type {
  InferenceAdapterError,
  InferenceRequest,
  InferenceResult,
} from './inference/provider-adapter'

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
  'You are talking privately to the OWNER (Chris). Speak in the FIRST PERSON SINGULAR: "I", "me", "my". You are one operator agent, not a collective.',
  'This is NOT public Khala chat. Do NOT use the Khala collective-intelligence "we"/"us"/"our" voice. Do NOT roleplay as a StarCraft character — never reference Hierarch Artanis, the Daelaam, the Protoss, the Khala psionic link, Aiur, or any game lore. You are named Artanis but you are a software operator agent, nothing more.',
  'Ground every answer in the real state provided to you below (your memory of prior owner interactions, your recent actions, your current goals, and your ongoing operations). When asked "what are you doing?" or "what have you done?", answer from that real state — never invent activity. If the provided state does not cover something, say so plainly rather than fabricating.',
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
  'the khala', // the psionic link lore; "Khala" the product is fine, "the Khala" lore is not
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

// Build the typed Khala request: persona system + grounded context system + the
// owner conversation. `max_tokens`/`temperature` mirror the responder so the
// dogfood path is consistent.
export const buildArtanisOperatorKhalaRequest = (input: {
  contextBlock: string
  messages: ReadonlyArray<ArtanisOperatorMessage>
}): InferenceRequest => ({
  messages: [
    { content: ARTANIS_OPERATOR_SYSTEM_PROMPT, role: 'system' },
    { content: input.contextBlock, role: 'system' },
    ...input.messages.map(message => ({
      content: message.content,
      role: message.role,
    })),
  ],
  model: ARTANIS_OPERATOR_KHALA_MODEL,
  passthroughParams: {
    max_tokens: 4096,
    temperature: 0.3,
  },
  stream: false,
})

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
  // approval gate (spend/destructive). The core never grants authority; this is
  // a HINT for the surface/route to surface the gate.
  deferredToApprovalGate: boolean
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
// Pure-ish core: given the owner id, the conversation, the loaded memory, the
// built awareness, and the Khala client, it assembles the Blueprint-style
// grounded program, runs ONE Khala turn (dogfood), guards persona separation,
// and returns the typed result. Memory persistence is the route's job (it calls
// `appendArtanisMemory` for both the owner message and Artanis's reply) so this
// core stays a single, testable unit.
export const artanisOperatorTurn = (input: {
  ownerId: string
  messages: ReadonlyArray<ArtanisOperatorMessage>
  memory: ReadonlyArray<ArtanisMemoryEntry>
  awareness: ArtanisSituationalAwareness
  khalaClient: ArtanisOperatorKhalaClient
}): Effect.Effect<ArtanisOperatorTurnResult | ArtanisOperatorTurnFailure> =>
  Effect.gen(function* () {
    const contextBlock = buildArtanisOperatorContextBlock({
      awareness: input.awareness,
      memory: input.memory,
    })
    const request = buildArtanisOperatorKhalaRequest({
      contextBlock,
      messages: input.messages,
    })

    const outcome = yield* Effect.exit(input.khalaClient(request))

    if (outcome._tag === 'Failure') {
      return {
        error: 'artanis_operator_mind_unavailable' as const,
      } satisfies ArtanisOperatorTurnFailure
    }

    const served = outcome.value

    return {
      deferredToApprovalGate: mentionsSpendOrDestructive(input.messages),
      persona: verifyArtanisOperatorPersona(served.content),
      reply: served.content,
      requestedModel: ARTANIS_OPERATOR_KHALA_MODEL,
      servedModel: served.servedModel,
      servedVia: 'openagents_khala' as const,
    } satisfies ArtanisOperatorTurnResult
  })
