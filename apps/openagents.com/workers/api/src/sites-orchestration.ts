import { Effect, Schema as S } from 'effect'

import {
  type RecordSiteBuilderPreviewCandidateInput,
  type SiteBuilderPreviewCandidate,
  type SiteBuilderPreviewRunnerResult,
  recordSiteBuilderPreviewCandidate,
} from './sites-builder-preview-runner'
import {
  type RecordSiteBuilderRepairAttemptInput,
  type SiteBuilderRepairAttemptRecord,
  type SiteBuilderRepairFailureKind,
  recordSiteBuilderRepairAttempt,
} from './sites-builder-repair-loop'
import {
  type OperatorSiteBuilderSessionProjection,
  type PublicSiteBuilderSessionProjection,
  type SiteBuilderPhaseKind,
  type SiteBuilderRuntime,
  type SiteBuilderSessionStatus,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  readSiteBuilderSessionProjection,
  recordSiteBuilderPhaseRun,
  systemSiteBuilderRuntime,
} from './sites-builder-sessions'

/**
 * Native Sites prompt -> build -> deploy ORCHESTRATION core (WS-D1, #4981).
 *
 * This module is a thin phase driver. It does NOT re-implement the existing
 * builder-session, preview-runner, repair-loop, saved-version, provisioning,
 * or Sites deploy services. It composes them:
 *
 *   - phase progress is tracked through `recordSiteBuilderPhaseRun`
 *   - preview readiness is delegated to `recordSiteBuilderPreviewCandidate`
 *   - build failures are funneled into `recordSiteBuilderRepairAttempt`
 *   - the durable save + deploy of an actual Site version remain owned by
 *     `saveSiteBuilderVersion` and `AutopilotSitesService.deployVersion`; the
 *     orchestrator only records the `save` / `deploy` phase outcome and the
 *     resulting public-safe refs.
 *
 * The result is always a typed session-state projection so callers (route +
 * future builder UI) get one consistent view of where the session is.
 */

/** Canonical ordered phase plan for a prompt -> build -> deploy run. */
export const SITE_ORCHESTRATION_PHASE_ORDER: ReadonlyArray<SiteBuilderPhaseKind> =
  [
    'planning',
    'foundation',
    'core',
    'styling',
    'integration',
    'optimization',
    'preview',
    'save',
    'deploy',
  ] as const

export const SiteOrchestrationOutcome = S.Literals([
  'phase_started',
  'phase_advanced',
  'preview_ready',
  'preview_pending',
  'build_failed',
  'build_repair_scheduled',
  'build_repair_exhausted',
  'saved',
  'deployed',
])
export type SiteOrchestrationOutcome = typeof SiteOrchestrationOutcome.Type

const DEFAULT_REPAIR_BUDGET = 3

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/

/** Session statuses the orchestrator drives the parent session toward. */
const statusForPhaseOutcome = (
  phaseKind: SiteBuilderPhaseKind,
  outcome: SiteOrchestrationOutcome,
): SiteBuilderSessionStatus | null => {
  if (outcome === 'build_repair_exhausted') {
    return 'failed'
  }

  if (outcome === 'preview_ready') {
    return 'preview_ready'
  }

  if (outcome === 'saved') {
    return 'saved'
  }

  if (outcome === 'deployed') {
    return 'deployed'
  }

  if (phaseKind === 'deploy') {
    return 'deploying'
  }

  if (phaseKind === 'save') {
    return 'review_ready'
  }

  if (phaseKind === 'planning') {
    return 'planning'
  }

  return 'building'
}

export type AdvanceSiteOrchestrationInput = Readonly<{
  /** The builder session being driven. */
  sessionId: string
  /** Idempotency key prefix; per-phase keys derive deterministically. */
  idempotencyKey: string
  /** The phase to advance. Must be a member of the canonical plan. */
  phaseKind: SiteBuilderPhaseKind
  /** Short public-safe title for the phase event. */
  title: string
  /** Short public-safe customer summary for the phase event. */
  summary: string
  /**
   * Build/runtime failure for this phase, if any. When present the
   * orchestrator routes through the repair loop instead of marking success.
   */
  failure?:
    | Readonly<{
        failureKind: SiteBuilderRepairFailureKind
        failureSummary: string
        retryBudget?: number | undefined
        attemptNumber?: number | undefined
      }>
    | undefined
  /**
   * Preview candidate for the `preview` phase. When present the orchestrator
   * composes the existing preview runner to select a tier and record it.
   */
  previewCandidate?: SiteBuilderPreviewCandidate | undefined
  /**
   * Public-safe ref to the durable artifact produced by `save` (the saved
   * Site version id) or `deploy` (the deployment id). The orchestrator records
   * it on the phase but does not itself create the Site version/deployment.
   */
  resultRef?: string | undefined
}>

export type SiteOrchestrationState = Readonly<{
  outcome: SiteOrchestrationOutcome
  phaseKind: SiteBuilderPhaseKind
  sessionStatus: SiteBuilderSessionStatus
  nextPhase: SiteBuilderPhaseKind | null
  preview: SiteBuilderPreviewRunnerResult | null
  repairAttempt: SiteBuilderRepairAttemptRecord | null
  operator: OperatorSiteBuilderSessionProjection
  public: PublicSiteBuilderSessionProjection
}>

export type SiteOrchestrationError =
  | SiteBuilderSessionStorageError
  | SiteBuilderSessionValidationError

const nextPhaseAfter = (
  phaseKind: SiteBuilderPhaseKind,
): SiteBuilderPhaseKind | null => {
  const index = SITE_ORCHESTRATION_PHASE_ORDER.indexOf(phaseKind)

  if (index === -1 || index === SITE_ORCHESTRATION_PHASE_ORDER.length - 1) {
    return null
  }

  return SITE_ORCHESTRATION_PHASE_ORDER[index + 1] ?? null
}

const validate = (
  input: AdvanceSiteOrchestrationInput,
): Effect.Effect<void, SiteBuilderSessionValidationError> =>
  Effect.try({
    try: () => {
      if (!SAFE_REF_PATTERN.test(input.sessionId)) {
        throw new SiteBuilderSessionValidationError({
          reason: 'sessionId must be a public-safe ref.',
        })
      }

      if (!SAFE_REF_PATTERN.test(input.idempotencyKey)) {
        throw new SiteBuilderSessionValidationError({
          reason: 'idempotencyKey must be a public-safe ref.',
        })
      }

      if (!SITE_ORCHESTRATION_PHASE_ORDER.includes(input.phaseKind)) {
        throw new SiteBuilderSessionValidationError({
          reason: `phaseKind ${input.phaseKind} is not part of the orchestration plan.`,
        })
      }

      if (
        input.resultRef !== undefined &&
        !SAFE_REF_PATTERN.test(input.resultRef)
      ) {
        throw new SiteBuilderSessionValidationError({
          reason: 'resultRef must be a public-safe ref.',
        })
      }
    },
    catch: error =>
      error instanceof SiteBuilderSessionValidationError
        ? error
        : new SiteBuilderSessionValidationError({
            reason: error instanceof Error ? error.message : String(error),
          }),
  })

/**
 * Advance a single phase of the prompt -> build -> deploy run.
 *
 * - On `failure`, the repair loop is invoked. If retry budget remains the
 *   phase is recorded `blocked` (repair scheduled); if exhausted it is
 *   recorded `failed`.
 * - On the `preview` phase with a `previewCandidate`, the preview runner picks
 *   a tier and records the preview; outcome is `preview_ready` only when the
 *   recorded preview is `ready`.
 * - Otherwise the phase is recorded `succeeded` and the next phase (if any) is
 *   reported so a caller can chain forward.
 */
export const advanceSiteBuilderOrchestration = (
  db: D1Database,
  input: AdvanceSiteOrchestrationInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<SiteOrchestrationState, SiteOrchestrationError> =>
  Effect.gen(function* () {
    yield* validate(input)

    const phaseKey = `${input.idempotencyKey}:phase:${input.phaseKind}`
    let outcome: SiteOrchestrationOutcome
    let preview: SiteBuilderPreviewRunnerResult | null = null
    let repairAttempt: SiteBuilderRepairAttemptRecord | null = null

    if (input.failure !== undefined) {
      const retryBudget = input.failure.retryBudget ?? DEFAULT_REPAIR_BUDGET
      const repairInput: RecordSiteBuilderRepairAttemptInput = {
        failureKind: input.failure.failureKind,
        failureSummary: input.failure.failureSummary,
        idempotencyKey: `${input.idempotencyKey}:repair:${input.phaseKind}`,
        phaseKind: input.phaseKind,
        retryBudget,
        sessionId: input.sessionId,
        ...(input.failure.attemptNumber === undefined
          ? {}
          : { attemptNumber: input.failure.attemptNumber }),
      }
      repairAttempt = yield* recordSiteBuilderRepairAttempt(
        db,
        {
          ...repairInput,
          status:
            repairInput.attemptNumber !== undefined &&
            repairInput.attemptNumber >= retryBudget
              ? 'failed'
              : 'blocked',
        },
        runtime,
      )

      const exhausted = repairAttempt.attemptNumber >= retryBudget
      outcome = exhausted ? 'build_repair_exhausted' : 'build_repair_scheduled'

      yield* recordSiteBuilderPhaseRun(
        db,
        {
          idempotencyKey: phaseKey,
          phaseKind: input.phaseKind,
          sessionId: input.sessionId,
          status: exhausted ? 'failed' : 'blocked',
          summary: input.summary,
          title: input.title,
          visibility: 'customer',
        },
        runtime,
      )
    } else if (
      input.phaseKind === 'preview' &&
      input.previewCandidate !== undefined
    ) {
      const candidateInput: RecordSiteBuilderPreviewCandidateInput = {
        candidate: input.previewCandidate,
        idempotencyKey: `${input.idempotencyKey}:preview`,
        sessionId: input.sessionId,
      }
      preview = yield* recordSiteBuilderPreviewCandidate(
        db,
        candidateInput,
        runtime,
      )
      const ready = preview.preview.status === 'ready'
      outcome = ready ? 'preview_ready' : 'preview_pending'

      yield* recordSiteBuilderPhaseRun(
        db,
        {
          idempotencyKey: phaseKey,
          phaseKind: 'preview',
          sessionId: input.sessionId,
          status: ready ? 'succeeded' : 'running',
          summary: input.summary,
          title: input.title,
          visibility: 'customer',
        },
        runtime,
      )
    } else {
      outcome =
        input.phaseKind === 'save'
          ? 'saved'
          : input.phaseKind === 'deploy'
            ? 'deployed'
            : input.phaseKind === 'planning'
              ? 'phase_started'
              : 'phase_advanced'

      yield* recordSiteBuilderPhaseRun(
        db,
        {
          idempotencyKey: phaseKey,
          phaseKind: input.phaseKind,
          sessionId: input.sessionId,
          status: 'succeeded',
          summary: input.summary,
          title: input.title,
          visibility: 'customer',
          ...(input.resultRef === undefined
            ? {}
            : { metadata: { resultRef: input.resultRef } }),
        },
        runtime,
      )
    }

    const sessionStatus =
      statusForPhaseOutcome(input.phaseKind, outcome) ?? 'building'
    const projection = yield* readSiteBuilderSessionProjection(
      db,
      input.sessionId,
    )

    return {
      // A failed/exhausted/scheduled-repair phase does not advance: the caller
      // either retries the same phase (scheduled) or stops (failed/exhausted).
      nextPhase:
        input.failure !== undefined ? null : nextPhaseAfter(input.phaseKind),
      operator: projection.operator,
      outcome,
      phaseKind: input.phaseKind,
      preview,
      public: projection.public,
      repairAttempt,
      sessionStatus,
    } satisfies SiteOrchestrationState
  })

/** Read the current orchestration state without advancing a phase. */
export const readSiteBuilderOrchestrationState = (
  db: D1Database,
  sessionId: string,
): Effect.Effect<
  Readonly<{
    operator: OperatorSiteBuilderSessionProjection
    public: PublicSiteBuilderSessionProjection
  }>,
  SiteOrchestrationError
> => readSiteBuilderSessionProjection(db, sessionId)
