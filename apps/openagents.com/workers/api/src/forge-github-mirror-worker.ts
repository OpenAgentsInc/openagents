import {
  ForgeGitHubMirrorHealth,
  ForgeGitHubMirrorIntent,
  ForgeGitHubMirrorObservedState,
  type ForgeGitHubMirrorReceipt,
  type ForgePromotionDecisionReceipt,
} from '@openagentsinc/forge-protocol'
import { Context, Effect, Layer, Schedule, Schema } from 'effect'

import type { ForgeGitHubMirrorStore } from './forge-github-mirror-store'

const commitObjectIdPattern = /^[a-f0-9]{40}(?:[a-f0-9]{24})?$/iu

export const ForgeOwnedCanonicalMirrorDescriptor = Schema.Struct({
  tenantRef: Schema.String,
  repositoryRef: Schema.String,
  authorityMode: Schema.Literals([
    'github_authoritative',
    'openagents_git_authoritative',
  ]),
  authorityGeneration: Schema.Number,
  sourceRef: Schema.String,
  sourceObjectId: Schema.NullOr(Schema.String),
  destinationGithubRepository: Schema.String,
  destinationGithubRef: Schema.String,
  sourceRefs: Schema.Array(Schema.String),
})
export interface ForgeOwnedCanonicalMirrorDescriptor extends Schema.Schema.Type<
  typeof ForgeOwnedCanonicalMirrorDescriptor
> {}

export class ForgeOwnedCanonicalMirrorError extends Schema.TaggedErrorClass<ForgeOwnedCanonicalMirrorError>()(
  'ForgeOwnedCanonicalMirrorError',
  {
    operation: Schema.String,
    reason: Schema.String,
    retryable: Schema.Boolean,
  },
) {}

export type ForgeOwnedCanonicalMirrorService = Readonly<{
  describe: (
    input: Readonly<{
      tenantRef: string
      repositoryRef: string
      sourceRef: string
    }>,
  ) => Effect.Effect<
    ForgeOwnedCanonicalMirrorDescriptor,
    ForgeOwnedCanonicalMirrorError
  >
  observe: (
    intent: ForgeGitHubMirrorIntent,
  ) => Effect.Effect<
    ForgeGitHubMirrorObservedState,
    ForgeOwnedCanonicalMirrorError
  >
  project: (
    intent: ForgeGitHubMirrorIntent,
  ) => Effect.Effect<
    ForgeGitHubMirrorObservedState,
    ForgeOwnedCanonicalMirrorError
  >
}>

export const makeUnavailableForgeOwnedCanonicalMirrorService =
  (): ForgeOwnedCanonicalMirrorService => {
    const unavailable = (operation: string) =>
      Effect.fail(
        new ForgeOwnedCanonicalMirrorError({
          operation,
          reason: 'forge_owned_canonical_mirror_service_unavailable',
          retryable: false,
        }),
      )

    return {
      describe: () => unavailable('ForgeOwnedCanonicalMirror.describe'),
      observe: () => unavailable('ForgeOwnedCanonicalMirror.observe'),
      project: () => unavailable('ForgeOwnedCanonicalMirror.project'),
    }
  }

export class ForgeGitHubMirrorWorkerError extends Schema.TaggedErrorClass<ForgeGitHubMirrorWorkerError>()(
  'ForgeGitHubMirrorWorkerError',
  {
    operation: Schema.String,
    reason: Schema.String,
    cause: Schema.optionalKey(Schema.Defect()),
  },
) {}

export type ForgeGitHubMirrorRunResult =
  | Readonly<{
      disposition: 'not_applicable'
      authorityMode: 'github_authoritative'
      repositoryRef: string
    }>
  | Readonly<{
      disposition: 'completed'
      intent: ForgeGitHubMirrorIntent
      observedState: ForgeGitHubMirrorObservedState
      receipt: ForgeGitHubMirrorReceipt
    }>

export type ForgeGitHubMirrorWorkerShape = Readonly<{
  health: (
    input: Readonly<{
      repositoryRef: string
      sourceRef: string
      tenantRef: string
    }>,
  ) => Effect.Effect<ForgeGitHubMirrorHealth, ForgeGitHubMirrorWorkerError>
  run: (
    input: Readonly<{
      promotion: ForgePromotionDecisionReceipt
      repositoryRef: string
    }>,
  ) => Effect.Effect<ForgeGitHubMirrorRunResult, ForgeGitHubMirrorWorkerError>
}>

export class ForgeGitHubMirrorWorker extends Context.Service<
  ForgeGitHubMirrorWorker,
  ForgeGitHubMirrorWorkerShape
>()('@openagentsinc/api-worker/ForgeGitHubMirrorWorker') {}

type ForgeGitHubMirrorWorkerDependencies = Readonly<{
  canonical: ForgeOwnedCanonicalMirrorService
  nowIso: () => string
  staleAfterSeconds?: number | undefined
  store: ForgeGitHubMirrorStore
}>

const stableRef = (prefix: string, value: string) =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest(
        'SHA-256',
        new TextEncoder().encode(value),
      )
      const digest = [...new Uint8Array(bytes)]
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')
      return `${prefix}.${digest.slice(0, 32)}`
    },
    catch: cause =>
      new ForgeGitHubMirrorWorkerError({
        operation: 'ForgeGitHubMirrorWorker.stableRef',
        reason: 'forge_github_mirror_digest_failed',
        cause,
      }),
  })

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> => [
  ...new Set(
    refs.filter(
      (ref): ref is string => ref !== null && ref !== undefined && ref !== '',
    ),
  ),
]

const retrySchedule = Schedule.exponential('50 millis').pipe(
  Schedule.either(Schedule.recurs(2)),
)

const retryCanonical = <A>(
  effect: Effect.Effect<A, ForgeOwnedCanonicalMirrorError>,
): Effect.Effect<A, ForgeOwnedCanonicalMirrorError> =>
  effect.pipe(
    Effect.retry({
      schedule: retrySchedule,
      while: error => error.retryable,
    }),
  )

const storeError = (operation: string, cause: unknown) =>
  new ForgeGitHubMirrorWorkerError({
    operation,
    reason: 'forge_github_mirror_store_failed',
    cause,
  })

const readReceipt = (
  store: ForgeGitHubMirrorStore,
  input: Readonly<{
    destinationGithubRef: string
    destinationGithubRepository: string
    promotionRef: string
    tenantRef: string
  }>,
) =>
  Effect.tryPromise({
    try: () =>
      store.readReceiptForPromotion(
        input.tenantRef,
        input.promotionRef,
        input.destinationGithubRepository,
        input.destinationGithubRef,
      ),
    catch: cause => storeError('ForgeGitHubMirrorWorker.readReceipt', cause),
  })

const recordReceipt = (
  store: ForgeGitHubMirrorStore,
  input: Parameters<ForgeGitHubMirrorStore['recordReceipt']>[0],
) =>
  Effect.tryPromise({
    try: () => store.recordReceipt(input),
    catch: cause => storeError('ForgeGitHubMirrorWorker.recordReceipt', cause),
  })

const listReceipts = (store: ForgeGitHubMirrorStore, tenantRef: string) =>
  Effect.tryPromise({
    try: () => store.listReceipts(tenantRef, { limit: 100 }),
    catch: cause => storeError('ForgeGitHubMirrorWorker.listReceipts', cause),
  })

const makeIntent = (
  descriptor: ForgeOwnedCanonicalMirrorDescriptor,
  promotion: ForgePromotionDecisionReceipt,
  requestedAt: string,
) =>
  Effect.gen(function* () {
    if (
      descriptor.sourceObjectId === null ||
      !commitObjectIdPattern.test(descriptor.sourceObjectId)
    ) {
      return yield* new ForgeGitHubMirrorWorkerError({
        operation: 'ForgeGitHubMirrorWorker.makeIntent',
        reason: 'forge_github_mirror_source_object_missing',
      })
    }

    const identity = [
      descriptor.tenantRef,
      descriptor.repositoryRef,
      descriptor.authorityGeneration,
      promotion.promotion_ref,
      descriptor.sourceRef,
      descriptor.sourceObjectId.toLowerCase(),
      descriptor.destinationGithubRepository,
      descriptor.destinationGithubRef,
    ].join(':')
    const intentRef = yield* stableRef('intent.forge.github-mirror', identity)

    return ForgeGitHubMirrorIntent.make({
      schema: 'openagents.forge.github_mirror.intent.v0.1',
      authority_generation: descriptor.authorityGeneration,
      authority_mode: descriptor.authorityMode,
      destination_github_ref: descriptor.destinationGithubRef,
      destination_github_repository: descriptor.destinationGithubRepository,
      intent_ref: intentRef,
      promotion_ref: promotion.promotion_ref,
      redacted: true,
      repository_ref: descriptor.repositoryRef,
      requested_at: requestedAt,
      source_object_id: descriptor.sourceObjectId.toLowerCase(),
      source_ref: descriptor.sourceRef,
      source_refs: uniqueRefs([
        ...descriptor.sourceRefs,
        ...promotion.source_refs,
        promotion.promotion_ref,
      ]),
      tenant_ref: descriptor.tenantRef,
    })
  })

const failureObservation = (
  intent: ForgeGitHubMirrorIntent,
  error: ForgeOwnedCanonicalMirrorError,
  observedAt: string,
) =>
  Effect.gen(function* () {
    const observationRef = yield* stableRef(
      'observation.forge.github-mirror',
      `${intent.intent_ref}:${observedAt}:${error.reason}`,
    )
    return ForgeGitHubMirrorObservedState.make({
      schema: 'openagents.forge.github_mirror.observed_state.v0.1',
      authority_generation: intent.authority_generation,
      authority_mode: intent.authority_mode,
      destination_github_ref: intent.destination_github_ref,
      destination_github_repository: intent.destination_github_repository,
      destination_object_id: null,
      divergence: 'unknown',
      error_reason: error.reason,
      intent_ref: intent.intent_ref,
      observation_ref: observationRef,
      observed_at: observedAt,
      redacted: true,
      repository_ref: intent.repository_ref,
      source_object_id: intent.source_object_id,
      source_ref: intent.source_ref,
      source_refs: uniqueRefs([...intent.source_refs, intent.intent_ref]),
      tenant_ref: intent.tenant_ref,
    })
  })

const assertObservedState = (
  intent: ForgeGitHubMirrorIntent,
  observed: ForgeGitHubMirrorObservedState,
) =>
  observed.intent_ref === intent.intent_ref &&
  observed.repository_ref === intent.repository_ref &&
  observed.authority_generation === intent.authority_generation &&
  observed.source_ref === intent.source_ref &&
  observed.source_object_id?.toLowerCase() ===
    intent.source_object_id.toLowerCase() &&
  observed.destination_github_repository ===
    intent.destination_github_repository &&
  observed.destination_github_ref === intent.destination_github_ref

const finalReceipt = (
  dependencies: ForgeGitHubMirrorWorkerDependencies,
  input: Readonly<{
    intent: ForgeGitHubMirrorIntent
    observed: ForgeGitHubMirrorObservedState
    promotion: ForgePromotionDecisionReceipt
  }>,
) =>
  Effect.gen(function* () {
    const succeeded =
      input.observed.error_reason === null &&
      input.observed.divergence === 'in_sync' &&
      input.observed.destination_object_id?.toLowerCase() ===
        input.intent.source_object_id.toLowerCase()
    const mirrorRef = yield* stableRef('mirror.github', input.intent.intent_ref)

    return yield* recordReceipt(dependencies.store, {
      change_ref: input.promotion.change_ref,
      commit_id: input.intent.source_object_id,
      completed_at: succeeded ? input.observed.observed_at : null,
      destination_github_ref: input.intent.destination_github_ref,
      destination_github_repository: input.intent.destination_github_repository,
      error_reason: succeeded
        ? null
        : (input.observed.error_reason ??
          'forge_github_mirror_post_projection_mismatch'),
      first_attempted_at: input.intent.requested_at,
      last_attempted_at: input.observed.observed_at,
      mirror_ref: mirrorRef,
      promotion_ref: input.intent.promotion_ref,
      redacted: true,
      refusal_reason: null,
      repository_ref: input.intent.repository_ref,
      source_canonical_ref: input.intent.source_ref,
      source_refs: uniqueRefs([
        ...input.intent.source_refs,
        ...input.observed.source_refs,
        input.intent.intent_ref,
        input.observed.observation_ref,
      ]),
      status: succeeded ? 'mirrored' : 'failed',
      tenant_ref: input.intent.tenant_ref,
    })
  })

const describeForPromotion = (
  dependencies: ForgeGitHubMirrorWorkerDependencies,
  input: Readonly<{
    promotion: ForgePromotionDecisionReceipt
    repositoryRef: string
  }>,
) =>
  retryCanonical(
    dependencies.canonical.describe({
      repositoryRef: input.repositoryRef,
      sourceRef: input.promotion.target_ref,
      tenantRef: input.promotion.tenant_ref,
    }),
  ).pipe(
    Effect.mapError(
      error =>
        new ForgeGitHubMirrorWorkerError({
          operation: 'ForgeGitHubMirrorWorker.describe',
          reason: error.reason,
        }),
    ),
  )

export const makeForgeGitHubMirrorWorker = (
  dependencies: ForgeGitHubMirrorWorkerDependencies,
): ForgeGitHubMirrorWorkerShape => {
  const staleAfterSeconds = dependencies.staleAfterSeconds ?? 300

  const run = Effect.fn('ForgeGitHubMirrorWorker.run')(function* (
    input: Parameters<ForgeGitHubMirrorWorkerShape['run']>[0],
  ) {
    const promotion = input.promotion
    if (promotion.decision !== 'approved' || promotion.promoted_head === null) {
      return yield* new ForgeGitHubMirrorWorkerError({
        operation: 'ForgeGitHubMirrorWorker.run',
        reason: 'forge_github_mirror_requires_approved_promotion',
      })
    }

    const descriptor = yield* describeForPromotion(dependencies, input)
    if (descriptor.authorityMode === 'github_authoritative') {
      return {
        authorityMode: descriptor.authorityMode,
        disposition: 'not_applicable',
        repositoryRef: descriptor.repositoryRef,
      } satisfies ForgeGitHubMirrorRunResult
    }
    if (
      descriptor.sourceObjectId?.toLowerCase() !==
      promotion.promoted_head.toLowerCase()
    ) {
      return yield* new ForgeGitHubMirrorWorkerError({
        operation: 'ForgeGitHubMirrorWorker.run',
        reason: 'forge_github_mirror_source_not_promoted',
      })
    }

    const intent = yield* makeIntent(
      descriptor,
      promotion,
      dependencies.nowIso(),
    )
    const existing = yield* readReceipt(dependencies.store, {
      destinationGithubRef: intent.destination_github_ref,
      destinationGithubRepository: intent.destination_github_repository,
      promotionRef: intent.promotion_ref,
      tenantRef: intent.tenant_ref,
    })
    if (
      existing?.status === 'mirrored' &&
      existing.commit_id.toLowerCase() === intent.source_object_id.toLowerCase()
    ) {
      const observed = yield* retryCanonical(
        dependencies.canonical.observe(intent),
      ).pipe(
        Effect.catch(error =>
          failureObservation(intent, error, dependencies.nowIso()),
        ),
      )
      return {
        disposition: 'completed',
        intent,
        observedState: observed,
        receipt: existing,
      } satisfies ForgeGitHubMirrorRunResult
    }

    const before = yield* retryCanonical(
      dependencies.canonical.observe(intent),
    ).pipe(
      Effect.catch(error =>
        failureObservation(intent, error, dependencies.nowIso()),
      ),
    )
    const observed =
      before.divergence === 'in_sync' &&
      before.destination_object_id?.toLowerCase() ===
        intent.source_object_id.toLowerCase()
        ? before
        : yield* retryCanonical(dependencies.canonical.project(intent)).pipe(
            Effect.catch(error =>
              failureObservation(intent, error, dependencies.nowIso()),
            ),
          )
    const normalizedObserved = assertObservedState(intent, observed)
      ? observed
      : yield* failureObservation(
          intent,
          new ForgeOwnedCanonicalMirrorError({
            operation: 'ForgeGitHubMirrorWorker.run',
            reason: 'forge_github_mirror_observation_mismatch',
            retryable: false,
          }),
          dependencies.nowIso(),
        )
    const receipt = yield* finalReceipt(dependencies, {
      intent,
      observed: normalizedObserved,
      promotion,
    })

    return {
      disposition: 'completed',
      intent,
      observedState: normalizedObserved,
      receipt,
    } satisfies ForgeGitHubMirrorRunResult
  })

  const health = Effect.fn('ForgeGitHubMirrorWorker.health')(function* (
    input: Parameters<ForgeGitHubMirrorWorkerShape['health']>[0],
  ) {
    const descriptor = yield* retryCanonical(
      dependencies.canonical.describe(input),
    ).pipe(
      Effect.mapError(
        error =>
          new ForgeGitHubMirrorWorkerError({
            operation: 'ForgeGitHubMirrorWorker.health.describe',
            reason: error.reason,
          }),
      ),
    )
    const receipts = yield* listReceipts(dependencies.store, input.tenantRef)
    const latestReceipt = receipts.find(
      receipt =>
        receipt.repository_ref === descriptor.repositoryRef &&
        receipt.destination_github_repository ===
          descriptor.destinationGithubRepository &&
        receipt.destination_github_ref === descriptor.destinationGithubRef,
    )
    if (descriptor.authorityMode === 'github_authoritative') {
      return ForgeGitHubMirrorHealth.make({
        schema: 'openagents.forge.github_mirror.health.v0.1',
        authority_generation: descriptor.authorityGeneration,
        authority_mode: descriptor.authorityMode,
        coordination_authority: false,
        destination_github_ref: descriptor.destinationGithubRef,
        destination_github_repository: descriptor.destinationGithubRepository,
        destination_object_id: descriptor.sourceObjectId,
        divergence: 'not_applicable',
        error_reason: null,
        freshness: 'not_applicable',
        last_mirrored_at: null,
        last_mirrored_object_id: null,
        last_mirrored_ref: null,
        observed_at: null,
        receipt_ref: null,
        redacted: true,
        repository_ref: descriptor.repositoryRef,
        source_object_id: descriptor.sourceObjectId,
        source_ref: descriptor.sourceRef,
        source_refs: descriptor.sourceRefs,
        stale_after_seconds: staleAfterSeconds,
        tenant_ref: descriptor.tenantRef,
      })
    }
    const intent = yield* makeIntent(
      descriptor,
      {
        schema: 'openagents.forge.promotion.decision.v0.1',
        base_head: descriptor.sourceObjectId ?? '',
        blocker_refs: [],
        candidate_head: descriptor.sourceObjectId ?? '',
        change_ref: 'health-observation',
        decided_at: dependencies.nowIso(),
        decided_by_ref: 'forge.repository.projection-service',
        decision: 'approved',
        gate_refs: [],
        gate_results: [],
        promoted_head: descriptor.sourceObjectId,
        promotion_ref: 'health-observation',
        queue_position: 0,
        queue_ref: 'health-observation',
        redacted: true,
        source_refs: descriptor.sourceRefs,
        target_ref: descriptor.sourceRef,
        tenant_ref: descriptor.tenantRef,
        verification_ref: null,
      },
      dependencies.nowIso(),
    )
    const observed = yield* retryCanonical(
      dependencies.canonical.observe(intent),
    ).pipe(
      Effect.catch(error =>
        failureObservation(intent, error, dependencies.nowIso()),
      ),
    )
    const nowMs = Date.parse(dependencies.nowIso())
    const observedMs = Date.parse(observed.observed_at)
    const stale =
      Number.isFinite(nowMs) &&
      Number.isFinite(observedMs) &&
      nowMs - observedMs > staleAfterSeconds * 1_000
    const freshness =
      observed.error_reason !== null
        ? latestReceipt?.completed_at === null ||
          latestReceipt?.completed_at === undefined
          ? 'never_observed'
          : 'stale'
        : stale
          ? 'stale'
          : 'fresh'

    return ForgeGitHubMirrorHealth.make({
      schema: 'openagents.forge.github_mirror.health.v0.1',
      authority_generation: descriptor.authorityGeneration,
      authority_mode: descriptor.authorityMode,
      coordination_authority: false,
      destination_github_ref: descriptor.destinationGithubRef,
      destination_github_repository: descriptor.destinationGithubRepository,
      destination_object_id: observed.destination_object_id,
      divergence: observed.divergence,
      error_reason:
        observed.error_reason ?? latestReceipt?.error_reason ?? null,
      freshness,
      last_mirrored_at: latestReceipt?.completed_at ?? null,
      last_mirrored_object_id: latestReceipt?.commit_id ?? null,
      last_mirrored_ref: latestReceipt?.destination_github_ref ?? null,
      observed_at: observed.observed_at,
      receipt_ref: latestReceipt?.mirror_ref ?? null,
      redacted: true,
      repository_ref: descriptor.repositoryRef,
      source_object_id: descriptor.sourceObjectId,
      source_ref: descriptor.sourceRef,
      source_refs: uniqueRefs([
        ...descriptor.sourceRefs,
        ...observed.source_refs,
        latestReceipt?.mirror_ref,
      ]),
      stale_after_seconds: staleAfterSeconds,
      tenant_ref: descriptor.tenantRef,
    })
  })

  return ForgeGitHubMirrorWorker.of({ health, run })
}

export const layerForgeGitHubMirrorWorker = (
  dependencies: ForgeGitHubMirrorWorkerDependencies,
) =>
  Layer.succeed(
    ForgeGitHubMirrorWorker,
    makeForgeGitHubMirrorWorker(dependencies),
  )
