// Wiring (spec §B): route ONE Khala turn through the Blueprint chat-program
// runtime so a Khala turn produces an evidence-only `BlueprintProgramRunRecord`
// — the first real Khala-on-Blueprint call.
//
// Today `khala-chat-program.ts` assembles a system message and calls the model
// directly; `executeBlueprintChatProgramTurn` (chat-program-runtime.ts) exists
// but was never invoked from the Khala path. This thin adapter closes that seam:
//
//   khalaTurnToProgram(req) -> BlueprintProgramRunRecord   (authorityBoundary: 'evidence_only')
//
// It is EVIDENCE-ONLY and writes nothing. Invariants (spec §B.2 / §G), carried
// not weakened:
//   - Selection is by the typed structured selector inside the runtime, NOT by
//     branching on user text (no-keyword-routing). We pass typed turn constraints
//     (family + risk ceiling + allowed surfaces + supported tool refs); the
//     runtime's `selectBlueprintChatProgramSignature` does the rest.
//   - The produced run record is asserted evidence-only by the runtime
//     (`assertProgramRunEvidenceOnly`): authorityBoundary='evidence_only',
//     directMutationDisabled, noDeploy/noEmail/noSpend/noSourceMutation all set.
//   - Any requested direct effect is DENIED by the runtime (it must be an
//     approval-gated Action Submission). This adapter never arms a write path.
//   - Public-safe: the run record exposes refs/digests/redacted handles only.
//
// The session runtime here is a DETERMINISTIC, evidence-only offer session: it
// emits the refusal-posture/offer response as refs only (no raw text, no real
// inference, no spend). The real streaming inference path is unchanged; this
// adapter is the evidence-emitting Blueprint wiring that rides ALONGSIDE it.

import { Effect } from 'effect'

import { compactRandomId, currentIsoTimestamp } from '../../runtime-primitives'
import type { BlueprintObjectiveSurface } from '../schemas/objective'
import type { BlueprintProgramFamily, BlueprintProgramRiskClass } from '../schemas/program'
import type { BlueprintProgramRunRecord } from '../schemas/program-run'
import {
  executeBlueprintChatProgramTurn,
  type BlueprintChatProgramRuntimePrimitives,
  type BlueprintChatProgramSessionResult,
  type BlueprintChatProgramSessionRuntime,
  type BlueprintChatProgramTurnError,
  type BlueprintChatProgramTurnInput,
  type BlueprintChatProgramTurnResult,
} from './chat-program-runtime'
import {
  emitGepaCandidateFeedback,
  type GepaCandidateFeedback,
  type GepaCandidateFeedbackError,
  type ProgramFailureFinding,
  type ProgramFailureVerdict,
} from './chat-program-failure-gepa'

// One model, no variants (the workspace invariant).
export const KHALA_PROGRAM_MODEL = 'openagents/khala'

/**
 * The typed Khala → program request (spec §B.1). Built from an existing Khala
 * turn; carries ONLY structured constraints, never user text the selector would
 * branch on. The refs are public-safe summaries/hashes of the turn.
 */
export type KhalaProgramTurnRequest = Readonly<{
  /** A stable ref for this Khala turn (e.g. a turn id). */
  turnRef: string
  /** Public-safe summary ref for the prompt (NEVER the raw prompt). */
  promptSummaryRef: string
  /** A hash of the input snapshot (NEVER raw inputs). */
  inputSnapshotHash: string
  /** The objective ref for this session. */
  sessionObjectiveRef: string
  /** Bounded surfaces this turn may touch. */
  allowedSurfaces: ReadonlyArray<BlueprintObjectiveSurface>
  /** Supported tool refs the chat runtime may scope to (typed, bounded). */
  supportedToolRefs: ReadonlyArray<string>
  /** Numeric-equivalent risk ceiling passed to the selector. */
  riskCeiling: BlueprintProgramRiskClass
  /** The Blueprint program family to select within (typed; NOT user text). */
  preferredFamily?: BlueprintProgramFamily
  /** Optional context-pack ref that NARROWS authority (never widens). */
  contextPackRef?: string
  /** Source authority refs the turn rides. */
  sourceAuthorityRefs: ReadonlyArray<string>
  /** Backend capability refs available to the session. */
  backendCapabilityRefs: ReadonlyArray<string>
  /** A public-safe response summary ref the offer session will surface. */
  responseSummaryRef?: string
}>

/** Deterministic refs for an offer session result (evidence-only; no inference). */
const offerSessionResult = (
  req: KhalaProgramTurnRequest,
  runtime: BlueprintChatProgramRuntimePrimitives,
): BlueprintChatProgramSessionResult => {
  const sessionRef = `session.khala_offer.${req.turnRef}`
  const observedAt = runtime.nowIso()
  return {
    confidence: 0.8,
    evidenceRefs: [`evidence.khala_offer.response.${req.turnRef}`],
    events: [
      {
        eventRef: `event.khala_offer.spawned.${req.turnRef}`,
        evidenceRefs: [`evidence.khala_offer.spawned.${req.turnRef}`],
        observedAt,
        phase: 'session.spawn',
        receiptRefs: [`receipt.khala_offer.spawned.${req.turnRef}`],
        safeProjection: true,
        state: 'completed',
        summaryRef: req.responseSummaryRef ?? `summary.khala_offer.${req.turnRef}`,
      },
    ],
    latencyMs: 0,
    receiptRefs: [`receipt.khala_offer.finished.${req.turnRef}`],
    renderedResponseRef: `rendered_response.khala_offer.${req.turnRef}`,
    // Evidence-only: the refusal-posture/offer turn requests NO direct effects.
    requestedDirectEffects: [],
    responseRef: `response.khala_offer.${req.turnRef}`,
    responseSummaryRef: req.responseSummaryRef ?? `response_summary.khala_offer.${req.turnRef}`,
    sessionRef,
    toolCallbackRefs: [],
    typedOutput: {
      responseRef: `response.khala_offer.${req.turnRef}`,
      state: 'completed',
    },
    usage: { truth: 'unknown' },
  }
}

/**
 * A deterministic, evidence-only offer session runtime. It performs NO real
 * inference and requests NO direct effects — it returns public-safe refs only,
 * so the resulting Program Run record is evidence-only by construction.
 */
export const makeKhalaOfferSessionRuntime = (
  req: KhalaProgramTurnRequest,
  runtime: BlueprintChatProgramRuntimePrimitives,
): BlueprintChatProgramSessionRuntime => ({
  spawnSession: () => Effect.succeed(offerSessionResult(req, runtime)),
})

/** Map the typed Khala request into the runtime's `BlueprintChatProgramTurnInput`. */
const toTurnInput = (
  req: KhalaProgramTurnRequest,
): BlueprintChatProgramTurnInput => ({
  actorRef: 'actor.khala.public_chat',
  allowedSurfaces: [...req.allowedSurfaces],
  backendCapabilityRefs: [...req.backendCapabilityRefs],
  backendKind: 'claude_agent',
  backendProfileId: 'backend_profile.khala.inference',
  costRef: `cost.khala_program.${req.turnRef}`,
  inputSnapshotHash: req.inputSnapshotHash,
  model: KHALA_PROGRAM_MODEL,
  preferredFamily: req.preferredFamily ?? 'continuation',
  promptSummaryRef: req.promptSummaryRef,
  riskCeiling: req.riskCeiling,
  routeRef: 'route.khala.chat_program',
  sessionAdapter: 'claude_agent',
  sessionObjectiveRef: req.sessionObjectiveRef,
  sourceAuthorityRefs: [...req.sourceAuthorityRefs],
  supportedToolRefs: [...req.supportedToolRefs],
  turnRef: req.turnRef,
  ...(req.contextPackRef === undefined ? {} : { contextPackRef: req.contextPackRef }),
})

export type KhalaTurnToProgramInput = Readonly<{
  request: KhalaProgramTurnRequest
  /** Optional deterministic primitives (for tests); production uses the default. */
  runtime?: BlueprintChatProgramRuntimePrimitives
}>

const systemPrimitives: BlueprintChatProgramRuntimePrimitives = {
  makeLookupId: () => compactRandomId('khala_signature_lookup'),
  makeMenuId: () => compactRandomId('khala_tool_menu'),
  makeProgramRunId: () => compactRandomId('khala_program_run'),
  nowIso: currentIsoTimestamp,
}

/**
 * Run ONE Khala turn through the Blueprint chat-program runtime and return the
 * full evidence-only turn result (which carries the `BlueprintProgramRunRecord`).
 * The runtime guarantees the record is evidence-only and denies any direct
 * effect; this adapter adds no write authority.
 */
export const executeKhalaProgramTurn = (
  input: KhalaTurnToProgramInput,
): Effect.Effect<BlueprintChatProgramTurnResult, BlueprintChatProgramTurnError> => {
  const runtime = input.runtime ?? systemPrimitives
  return executeBlueprintChatProgramTurn({
    runtime,
    sessionRuntime: makeKhalaOfferSessionRuntime(input.request, runtime),
    turn: toTurnInput(input.request),
  })
}

/**
 * Convenience: run the turn and project just the evidence-only
 * `BlueprintProgramRunRecord` (the spec §B output the Khala response references
 * via a `programRunRef`).
 */
export const khalaTurnToProgramRunRecord = (
  input: KhalaTurnToProgramInput,
): Effect.Effect<BlueprintProgramRunRecord, BlueprintChatProgramTurnError> =>
  executeKhalaProgramTurn(input).pipe(Effect.map(result => result.programRun))

/**
 * QA failure learning (#6195) — the Blueprint/GEPA wiring on the Khala surface.
 *
 * Run ONE Khala turn through the Blueprint program runtime and, given an honest
 * failure verdict + the contradicted findings (from the qa-runner verify stage),
 * emit the claim-level, EVIDENCE-ONLY GEPA candidate-feedback signal the
 * optimizer may consume. The program-run record is emitted by the runtime
 * (evidence-only by construction) and the feedback derives ONLY from that
 * record's public-safe refs — so the failure becomes governed optimizer signal
 * without any write authority, self-promotion, or live behavior change.
 *
 * This is reachable from the worker's Khala Blueprint surface; the report-side
 * strategies (suggest/auto_commit/open_pr) live in apps/qa-runner.
 */
export const khalaFailureTurnToGepaCandidateFeedback = (
  input: KhalaTurnToProgramInput & {
    readonly trigger: ProgramFailureVerdict
    readonly findings: ReadonlyArray<ProgramFailureFinding>
    readonly optimizerKind?: GepaCandidateFeedback['optimizerKind']
  },
): Effect.Effect<
  GepaCandidateFeedback,
  BlueprintChatProgramTurnError | GepaCandidateFeedbackError
> =>
  khalaTurnToProgramRunRecord(input).pipe(
    Effect.flatMap(programRun =>
      emitGepaCandidateFeedback({
        programRun,
        trigger: input.trigger,
        findings: input.findings,
        ...(input.optimizerKind === undefined ? {} : { optimizerKind: input.optimizerKind }),
      }),
    ),
  )
