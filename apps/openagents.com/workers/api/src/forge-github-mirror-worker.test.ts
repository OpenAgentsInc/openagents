/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import {
  type ForgeGitHubMirrorIntent,
  ForgeGitHubMirrorObservedState,
  type ForgeGitHubMirrorReceipt,
  type ForgePromotionDecisionReceipt,
  type ForgeRepositoryAuthorityMode,
} from '@openagentsinc/forge-protocol'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ForgeGitHubMirrorStore } from './forge-github-mirror-store'
import {
  ForgeGitHubMirrorWorker,
  type ForgeOwnedCanonicalMirrorDescriptor,
  ForgeOwnedCanonicalMirrorError,
  type ForgeOwnedCanonicalMirrorService,
  layerForgeGitHubMirrorWorker,
} from './forge-github-mirror-worker'

const headA = 'a'.repeat(40)
const headB = 'b'.repeat(40)
const headC = 'c'.repeat(40)
const tenantRef = 'tenant.openagents'
const repositoryRef = 'repo.openagents.openagents'
const destinationRepository = 'OpenAgentsInc/openagents'

const promotion = (
  sourceRef: string = 'refs/heads/main',
): ForgePromotionDecisionReceipt => ({
  schema: 'openagents.forge.promotion.decision.v0.1',
  base_head: headA,
  blocker_refs: [],
  candidate_head: headB,
  change_ref: 'change.forge.9251',
  decided_at: '2026-07-26T03:00:00.000Z',
  decided_by_ref: 'forge.promotion-gate',
  decision: 'approved',
  gate_refs: ['gate.verification'],
  gate_results: [],
  promoted_head: headB,
  promotion_ref: `promotion.forge.9251.${sourceRef}`,
  queue_position: 1,
  queue_ref: 'queue.forge.main',
  redacted: true,
  source_refs: ['issue.public.github.OpenAgentsInc.openagents.9251'],
  target_ref: sourceRef,
  tenant_ref: tenantRef,
  verification_ref: 'verification.forge.9251',
})

const makeMemoryStore = (): ForgeGitHubMirrorStore => {
  const receipts = new Map<string, ForgeGitHubMirrorReceipt>()
  const observations = new Map<string, ForgeGitHubMirrorObservedState>()
  return {
    async listObservationsForIntent(requestTenantRef, intentRef) {
      return [...observations.values()].filter(
        observation =>
          observation.tenant_ref === requestTenantRef &&
          observation.intent_ref === intentRef,
      )
    },
    async listReceipts(requestTenantRef, input) {
      return (
        [...receipts.values()]
          .filter(receipt => receipt.tenant_ref === requestTenantRef)
          .filter(
            receipt =>
              input.promotionRef === undefined ||
              receipt.promotion_ref === input.promotionRef,
          )
          .filter(
            receipt =>
              input.status === undefined || receipt.status === input.status,
          )
          // oxlint-disable-next-line unicorn/no-array-sort -- this is a new array.
          .sort((left, right) =>
            right.last_attempted_at.localeCompare(left.last_attempted_at),
          )
          .slice(0, input.limit)
      )
    },
    async readReceiptForPromotion(
      requestTenantRef,
      promotionRef,
      requestDestinationRepository,
      destinationRef,
    ) {
      return [...receipts.values()].find(
        receipt =>
          receipt.tenant_ref === requestTenantRef &&
          receipt.promotion_ref === promotionRef &&
          receipt.destination_github_repository ===
            requestDestinationRepository &&
          receipt.destination_github_ref === destinationRef,
      )
    },
    async readObservation(requestTenantRef, observationRef) {
      const observation = observations.get(observationRef)
      return observation?.tenant_ref === requestTenantRef
        ? observation
        : undefined
    },
    async recordObservation(observation) {
      observations.set(observation.observation_ref, observation)
      return observation
    },
    async recordReceipt(input) {
      const previous = receipts.get(input.mirror_ref)
      const receipt: ForgeGitHubMirrorReceipt = {
        schema: 'openagents.forge.github_mirror.receipt.v0.1',
        ...input,
        attempt_count: (previous?.attempt_count ?? 0) + 1,
        first_attempted_at:
          previous?.first_attempted_at ?? input.first_attempted_at,
        source_refs: [
          ...new Set([...(previous?.source_refs ?? []), ...input.source_refs]),
        ],
      }
      receipts.set(receipt.mirror_ref, receipt)
      return receipt
    },
  }
}

const descriptor = (
  sourceRef: string,
  authorityMode: ForgeRepositoryAuthorityMode = 'openagents_git_authoritative',
  sourceObjectId: string = headB,
): ForgeOwnedCanonicalMirrorDescriptor => ({
  authorityGeneration: 7,
  authorityMode,
  destinationGithubRef: sourceRef,
  destinationGithubRepository: destinationRepository,
  repositoryRef,
  sourceObjectId,
  sourceRef,
  sourceRefs: ['canonical-owned-service:repository'],
  tenantRef,
})

type CanonicalHarness = Readonly<{
  canonical: ForgeOwnedCanonicalMirrorService
  observedIntents: Array<ForgeGitHubMirrorIntent>
  projectedIntents: Array<ForgeGitHubMirrorIntent>
  setDestinationObjectId: (objectId: string | null) => void
}>

const makeCanonical = (
  input: Readonly<{
    authorityMode?: ForgeRepositoryAuthorityMode
    observedAt?: string
    destinationObjectId?: string | null
    sourceObjectId?: string
    sourceRef?: string
    transientProjectFailures?: number
  }> = {},
): CanonicalHarness => {
  const sourceRef = input.sourceRef ?? 'refs/heads/main'
  const sourceObjectId = input.sourceObjectId ?? headB
  let destinationObjectId: string | null =
    input.destinationObjectId === undefined ? headA : input.destinationObjectId
  let transientProjectFailures = input.transientProjectFailures ?? 0
  const observedIntents: Array<ForgeGitHubMirrorIntent> = []
  const projectedIntents: Array<ForgeGitHubMirrorIntent> = []

  const observedState = (
    intent: ForgeGitHubMirrorIntent,
    objectId: string | null,
  ) =>
    ForgeGitHubMirrorObservedState.make({
      schema: 'openagents.forge.github_mirror.observed_state.v0.1',
      authority_generation: intent.authority_generation,
      authority_mode: intent.authority_mode,
      destination_github_ref: intent.destination_github_ref,
      destination_github_repository: intent.destination_github_repository,
      destination_object_id: objectId,
      divergence:
        objectId === null
          ? 'destination_missing'
          : objectId === intent.source_object_id
            ? 'in_sync'
            : 'source_ahead',
      error_reason: null,
      intent_ref: intent.intent_ref,
      observation_ref: `observation.${observedIntents.length + projectedIntents.length}`,
      observed_at: input.observedAt ?? '2026-07-26T03:00:10.000Z',
      redacted: true,
      repository_ref: intent.repository_ref,
      source_object_id: intent.source_object_id,
      source_ref: intent.source_ref,
      source_refs: [intent.intent_ref],
      tenant_ref: intent.tenant_ref,
    })

  return {
    canonical: {
      describe: () =>
        Effect.succeed(
          descriptor(
            sourceRef,
            input.authorityMode ?? 'openagents_git_authoritative',
            sourceObjectId,
          ),
        ),
      observe: intent =>
        Effect.sync(() => {
          observedIntents.push(intent)
          return observedState(intent, destinationObjectId)
        }),
      project: intent =>
        Effect.suspend(() => {
          projectedIntents.push(intent)
          if (transientProjectFailures > 0) {
            transientProjectFailures -= 1
            return Effect.fail(
              new ForgeOwnedCanonicalMirrorError({
                operation: 'Canonical.project',
                reason: 'canonical_service_temporarily_unavailable',
                retryable: true,
              }),
            )
          }
          destinationObjectId = intent.source_object_id
          return Effect.succeed(observedState(intent, destinationObjectId))
        }),
    },
    observedIntents,
    projectedIntents,
    setDestinationObjectId: objectId => {
      destinationObjectId = objectId
    },
  }
}

const runWith = <A>(
  canonical: ForgeOwnedCanonicalMirrorService,
  store: ForgeGitHubMirrorStore,
  effect: Effect.Effect<A, unknown, ForgeGitHubMirrorWorker>,
  nowIso: () => string = () => '2026-07-26T03:00:00.000Z',
) =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(
        layerForgeGitHubMirrorWorker({
          canonical,
          nowIso,
          staleAfterSeconds: 300,
          store,
        }),
      ),
    ),
  )

describe('ForgeGitHubMirrorWorker', () => {
  test.each(['refs/heads/main', 'refs/tags/v1.0.0'])(
    'projects an internally promoted %s through the canonical-owned service',
    async sourceRef => {
      const canonical = makeCanonical({ sourceRef })
      const store = makeMemoryStore()
      const result = await runWith(
        canonical.canonical,
        store,
        Effect.gen(function* () {
          const worker = yield* ForgeGitHubMirrorWorker
          return yield* worker.run({
            promotion: promotion(sourceRef),
            repositoryRef,
          })
        }),
      )

      expect(result.disposition).toBe('completed')
      if (result.disposition !== 'completed') return
      expect(result.intent.source_ref).toBe(sourceRef)
      expect(result.intent.destination_github_ref).toBe(sourceRef)
      expect(result.observedState).toMatchObject({
        destination_object_id: headB,
        divergence: 'in_sync',
      })
      expect(result.receipt).toMatchObject({
        commit_id: headB,
        status: 'mirrored',
      })
      expect(result.receipt.source_refs).toContain(result.intent.intent_ref)
      expect(result.receipt.source_refs).toContain(
        result.observedState.observation_ref,
      )
      const observations = await store.listObservationsForIntent(
        tenantRef,
        result.intent.intent_ref,
      )
      expect(observations.map(item => item.observation_ref)).toEqual([
        'observation.1',
        'observation.2',
      ])
      await expect(
        store.readObservation(tenantRef, result.observedState.observation_ref),
      ).resolves.toEqual(result.observedState)
      expect(canonical.projectedIntents).toHaveLength(1)
    },
  )

  test('reuses stable identities and does not project a successful intent twice', async () => {
    const canonical = makeCanonical()
    const store = makeMemoryStore()
    const invoke = () =>
      runWith(
        canonical.canonical,
        store,
        Effect.gen(function* () {
          const worker = yield* ForgeGitHubMirrorWorker
          return yield* worker.run({
            promotion: promotion(),
            repositoryRef,
          })
        }),
      )

    const first = await invoke()
    const second = await invoke()
    expect(first.disposition).toBe('completed')
    expect(second.disposition).toBe('completed')
    if (
      first.disposition !== 'completed' ||
      second.disposition !== 'completed'
    ) {
      return
    }
    expect(second.intent.intent_ref).toBe(first.intent.intent_ref)
    expect(second.receipt.mirror_ref).toBe(first.receipt.mirror_ref)
    expect(second.receipt.attempt_count).toBe(1)
    expect(canonical.projectedIntents).toHaveLength(1)
  })

  test('does not create or project a mirror intent for github_authoritative repositories', async () => {
    const canonical = makeCanonical({
      authorityMode: 'github_authoritative',
    })
    const result = await runWith(
      canonical.canonical,
      makeMemoryStore(),
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: promotion(),
          repositoryRef,
        })
      }),
    )

    expect(result).toEqual({
      authorityMode: 'github_authoritative',
      disposition: 'not_applicable',
      repositoryRef,
    })
    expect(canonical.observedIntents).toHaveLength(0)
    expect(canonical.projectedIntents).toHaveLength(0)
  })

  test('retries a transient canonical projection with one stable intent', async () => {
    const canonical = makeCanonical({ transientProjectFailures: 2 })
    const result = await runWith(
      canonical.canonical,
      makeMemoryStore(),
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: promotion(),
          repositoryRef,
        })
      }),
    )

    expect(result.disposition).toBe('completed')
    if (result.disposition !== 'completed') return
    expect(result.receipt.status).toBe('mirrored')
    expect(canonical.projectedIntents).toHaveLength(3)
    expect(
      new Set(canonical.projectedIntents.map(intent => intent.intent_ref)).size,
    ).toBe(1)
  })

  test('bounds retryable projection failures and records the exhausted attempt', async () => {
    const canonical = makeCanonical({ transientProjectFailures: 99 })
    const result = await runWith(
      canonical.canonical,
      makeMemoryStore(),
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: promotion(),
          repositoryRef,
        })
      }),
    )

    expect(result.disposition).toBe('completed')
    if (result.disposition !== 'completed') return
    expect(result.receipt).toMatchObject({
      attempt_count: 1,
      error_reason: 'canonical_service_temporarily_unavailable',
      status: 'failed',
    })
    expect(canonical.projectedIntents).toHaveLength(3)
    expect(
      new Set(canonical.projectedIntents.map(intent => intent.intent_ref)).size,
    ).toBe(1)
  })

  test('re-observes and repairs GitHub drift after a successful receipt', async () => {
    const canonical = makeCanonical()
    const store = makeMemoryStore()
    const workerRun = () =>
      runWith(
        canonical.canonical,
        store,
        Effect.gen(function* () {
          const worker = yield* ForgeGitHubMirrorWorker
          return yield* worker.run({
            promotion: promotion(),
            repositoryRef,
          })
        }),
      )

    await workerRun()
    canonical.setDestinationObjectId(headA)
    const second = await workerRun()
    expect(second.disposition).toBe('completed')
    if (second.disposition !== 'completed') return
    expect(second.observedState).toMatchObject({
      destination_object_id: headB,
      divergence: 'in_sync',
    })
    expect(second.receipt.status).toBe('mirrored')
    expect(second.receipt.attempt_count).toBe(2)
    expect(canonical.projectedIntents).toHaveLength(2)
  })

  test('reports current divergence and stale observation without granting coordination authority', async () => {
    const canonical = makeCanonical({
      observedAt: '2026-07-26T03:00:00.000Z',
    })
    const store = makeMemoryStore()
    await runWith(
      canonical.canonical,
      store,
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: promotion(),
          repositoryRef,
        })
      }),
    )
    canonical.setDestinationObjectId(headA)

    const health = await runWith(
      canonical.canonical,
      store,
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.health({
          repositoryRef,
          sourceRef: 'refs/heads/main',
          tenantRef,
        })
      }),
      () => '2026-07-26T03:10:01.000Z',
    )

    expect(health).toMatchObject({
      authority_mode: 'openagents_git_authoritative',
      coordination_authority: false,
      destination_object_id: headA,
      divergence: 'source_ahead',
      freshness: 'stale',
      last_mirrored_object_id: headB,
      last_mirrored_ref: 'refs/heads/main',
      source_object_id: headB,
    })
  })

  test('keeps the last successful mirror when a newer promotion fails', async () => {
    const store = makeMemoryStore()
    const firstCanonical = makeCanonical()
    await runWith(
      firstCanonical.canonical,
      store,
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: promotion(),
          repositoryRef,
        })
      }),
      () => '2026-07-26T03:00:00.000Z',
    )

    const failingCanonical = makeCanonical({
      destinationObjectId: headB,
      observedAt: '2026-07-26T04:00:10.000Z',
      sourceObjectId: headC,
      transientProjectFailures: 99,
    })
    const failedPromotion: ForgePromotionDecisionReceipt = {
      ...promotion(),
      candidate_head: headC,
      change_ref: 'change.forge.9251.follow-up',
      decided_at: '2026-07-26T04:00:00.000Z',
      promoted_head: headC,
      promotion_ref: 'promotion.forge.9251.follow-up',
    }
    const failed = await runWith(
      failingCanonical.canonical,
      store,
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.run({
          promotion: failedPromotion,
          repositoryRef,
        })
      }),
      () => '2026-07-26T04:00:00.000Z',
    )
    expect(failed.disposition).toBe('completed')
    if (failed.disposition !== 'completed') return
    expect(failed.receipt).toMatchObject({
      commit_id: headC,
      status: 'failed',
    })

    const health = await runWith(
      failingCanonical.canonical,
      store,
      Effect.gen(function* () {
        const worker = yield* ForgeGitHubMirrorWorker
        return yield* worker.health({
          repositoryRef,
          sourceRef: 'refs/heads/main',
          tenantRef,
        })
      }),
      () => '2026-07-26T04:01:00.000Z',
    )

    expect(health).toMatchObject({
      error_reason: 'canonical_service_temporarily_unavailable',
      last_mirrored_object_id: headB,
      last_mirrored_ref: 'refs/heads/main',
      source_object_id: headC,
    })
    expect(health.receipt_ref).not.toBe(failed.receipt.mirror_ref)
  })
})
