import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY,
  runArtanisScheduledTick,
  runArtanisScheduledTickForWorker,
} from './artanis-scheduled-runner'
import { publicProductPromisesDocument } from './product-promises'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'

const nowIso = '2026-06-07T05:20:00.000Z'
const scheduledTime = Date.parse(nowIso)

const persistedKhalaReadinessSignal = (
  store: ArtanisPersistenceTestStore,
): Record<string, unknown> => {
  const projection = JSON.parse(
    store.rows('artanis_health_snapshots')[0]!.public_projection_json,
  ) as { signals: Array<Record<string, unknown>> }
  const signal = projection.signals.find(item =>
    item.kind === 'khala_readiness'
  )

  if (signal === undefined) {
    throw new Error('Missing Khala readiness signal')
  }

  return signal
}

describe('Artanis scheduled runner', () => {
  test('stays disabled by default and records no rows', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    const result = await Effect.runPromise(
      runArtanisScheduledTick({
        db,
        enabled: false,
        nowIso,
        scheduleRef: 'cron.public.artanis.disabled',
      }),
    )

    expect(result).toMatchObject({
      enabled: false,
      state: 'disabled',
      storageReceipts: [],
    })
    expect([...store.tables.values()].flat()).toHaveLength(0)
  })

  test('runs one persisted tick to closeout without risky execution authority', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    const result = await Effect.runPromise(
      runArtanisScheduledTick({
        db,
        enabled: true,
        nowIso,
        scheduleRef: 'cron.public.artanis.20260607T0520',
      }),
    )

    expect(result.enabled).toBe(true)
    expect(result.state).toBe('completed')
    expect(result.loopRef).toBe('loop.public.artanis.scope_public_artanis_global')
    expect(result.tickRef).toBe('tick.public.artanis.cron_public_artanis_20260607T0520')
    expect(result.forbiddenAuthority).toEqual({
      adapterInstallAllowed: false,
      deploymentAllowed: false,
      evalLaunchAllowed: false,
      forumPublishAllowed: false,
      l402RedemptionAllowed: false,
      paymentSpendAllowed: false,
      providerMutationAllowed: false,
      pylonJobDispatchAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      trainingLaunchAllowed: false,
      walletSpendAllowed: false,
    })
    expect(result.approvalRequirementRefs).toContain(
      'approval.public.artanis.tassadar_executor_paid_sample.cron_public_artanis_20260607T0520',
    )
    expect(result.approvalRequirementRefs).toContain(
      'gate.public.artanis.tassadar_executor_paid_sample.cron_public_artanis_20260607T0520',
    )
    expect(result.workProposalRefs).toEqual([
      'work.public.artanis.tassadar_executor_trace.cron_public_artanis_20260607T0520',
      'work.public.artanis.khala_burndown.cron_public_artanis_20260607T0520',
    ])
    expect(result.forumIntentRefs).toEqual([
      'forum.public.artanis.tassadar_executor_trace_intent.cron_public_artanis_20260607T0520',
    ])
    expect(result.healthSnapshotRef).toBe(
      'health.public.artanis.snapshot.cron_public_artanis_20260607T0520',
    )
    expect(result.loadedContextRefs).toEqual(
      expect.arrayContaining([
        'context.private.artanis.model_lab.operator_contract_refs',
        'model_lab.public.report.autopilot_benchmark_loop',
        'nexus.public.stats',
        'pylon.public.stats',
        'state.public.artanis.persistence',
        'steering.public.autopilot_artanis',
      ]),
    )
    expect(result.storageReceipts.every(receipt =>
      receipt.executableAuthority === false
    )).toBe(true)
    expect(store.rows('artanis_loop_records')).toHaveLength(1)
    expect(store.rows('artanis_loop_ticks')).toHaveLength(1)
    expect(store.rows('artanis_loop_ticks')[0]).toMatchObject({
      closed_at: nowIso,
      state: 'completed',
    })
    const tickProjection = JSON.parse(
      store.rows('artanis_loop_ticks')[0]!.public_projection_json,
    )
    expect(tickProjection.loops[0].ticks[0]).toMatchObject({
      actionProposals: expect.arrayContaining([
        expect.objectContaining({
          kind: 'pylon_triage',
          risk: 'safe',
        }),
        expect.objectContaining({
          actionRef:
            'action.public.artanis.khala_burndown_tick.cron_public_artanis_20260607T0520',
          authorityReceiptRefs: expect.arrayContaining([
            'authority.public.artanis.owner_promotion.2026-06-27',
            'authority.public.artanis.standing_pylon_job_dispatch',
          ]),
          kind: 'pylon_triage',
          risk: 'safe',
        }),
        expect.objectContaining({
          actionRef:
            'action.public.artanis.khala_feedback_and_issue_triage.cron_public_artanis_20260607T0520',
          evidenceRefs: expect.arrayContaining([
            'api.operator.khala.feedback',
            'api.operator.khala.trace_review',
            'api.operator.khala.unsupported_requests',
            'github.public.issue.6360',
          ]),
          kind: 'status_projection',
          risk: 'safe',
        }),
        expect.objectContaining({
          actionRef:
            'action.public.artanis.inference_book_next_source.cron_public_artanis_20260607T0520',
          evidenceRefs: expect.arrayContaining([
            'docs/inference/inference-engineering-book/',
            'github.public.issue.6316',
          ]),
          kind: 'status_projection',
          risk: 'safe',
        }),
        expect.objectContaining({
          kind: 'executor_trace_replay',
          risk: 'safe',
        }),
        expect.objectContaining({
          kind: 'wallet_spend',
          risk: 'approval_required',
        }),
      ]),
      approvalRequirements: [
        expect.objectContaining({
          authorityRef: 'authority.public.artanis.operator_spend_enable',
          state: 'pending',
        }),
      ],
      receiptRefs: expect.arrayContaining([
        'receipt.public.artanis.khala_burndown_tick_plan.cron_public_artanis_20260607T0520',
        'receipt.public.artanis.khala_stale_lease_recovery_plan.cron_public_artanis_20260607T0520',
        'receipt.public.artanis.tassadar_executor_dispatch.cron_public_artanis_20260607T0520',
        'receipt.public.artanis.tassadar_executor_replay_verified.cron_public_artanis_20260607T0520',
      ]),
      selectedContextRefs: expect.arrayContaining([
        'api.operator.khala.feedback',
        'api.operator.khala.trace_review',
        'api.operator.khala.unsupported_requests',
        'docs/inference/inference-engineering-book/',
        'github.public.issue.6359',
      ]),
    })
    expect(store.rows('artanis_forum_publication_intents')).toHaveLength(1)
    expect(persistedKhalaReadinessSignal(store)).toMatchObject({
      blockerRefs: ['blocker.public.artanis.khala_readiness_not_observed'],
      caveatRefs: expect.arrayContaining([
        'authority.public.khala_readiness.credentialless_read_only',
        'authority.public.khala_readiness.no_chat_call',
        'authority.public.khala_readiness.no_mutation',
        'authority.public.khala_readiness.no_paid_call',
      ]),
      kind: 'khala_readiness',
      publicRecoveryActionRefs: [
        'recovery.public.artanis.run_khala_no_spend_monitor',
      ],
      state: 'unknown',
    })
    const forumIntentProjection = JSON.parse(
      store.rows('artanis_forum_publication_intents')[0]!
        .public_projection_json,
    )
    expect(forumIntentProjection.intents[0]).toMatchObject({
      bodyText: ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY,
      caveatRefs: [
        'caveat.public.copy_limited_to_promise_safeCopy',
        'caveat.public.no_broader_executor_or_earning_claim',
      ],
      deliveryState: 'ready',
    })
    expect(store.rows('artanis_work_routing_proposals')).toHaveLength(2)
    const workProposalProjections = store
      .rows('artanis_work_routing_proposals')
      .map(row => JSON.parse(row.public_projection_json))
    const khalaBurndownProposal = workProposalProjections
      .flatMap(projection => projection.proposals)
      .find(proposal =>
        proposal.proposalRef ===
          'work.public.artanis.khala_burndown.cron_public_artanis_20260607T0520'
      )
    expect(khalaBurndownProposal).toMatchObject({
      capability: 'coding_runtime_probe',
      sourceEvidenceRefs: expect.arrayContaining([
        'api.public.khala_served_count',
        'docs/inference/inference-engineering-book/',
        'github.public.issue.6355',
        'github.public.issue.6359',
      ]),
      targetCapabilityRefs: expect.arrayContaining([
        'capability.public.artanis.dispatch_codex_task',
        'capability.public.pylon.codex_agent_task',
      ]),
      workClass: 'validation',
    })
    expect(store.rows('artanis_approval_gates')).toHaveLength(1)
    expect(
      store.rows('artanis_forum_publication_intents')[0]!.public_projection_json,
    ).not.toMatch(/context\.private|evidence\.private|receipt\.operator|wallet_secret|raw_log/i)
  })

  test('persists a green Khala no-spend readiness observation', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    const result = await Effect.runPromise(
      runArtanisScheduledTick({
        db,
        enabled: true,
        khalaReadinessObservation: {
          publicModelIds: ['openagents/khala'],
          readinessStatus: 'ready',
          servableModelCount: 1,
        },
        nowIso,
        scheduleRef: 'cron.public.artanis.khala-ready',
      }),
    )

    expect(result.state).toBe('completed')
    expect(persistedKhalaReadinessSignal(store)).toMatchObject({
      blockerRefs: [],
      count: 0,
      kind: 'khala_readiness',
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.khala_ready'],
      sourceRefs: expect.arrayContaining([
        'gateway.public.openagents.models',
        'gateway.public.openagents.readiness',
        'model.public.openagents.khala',
        'monitor.public.khala.no_spend_readiness',
      ]),
      state: 'available',
    })
  })

  test('blocks Khala readiness on leaked public model ids without projecting them', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    await Effect.runPromise(
      runArtanisScheduledTick({
        db,
        enabled: true,
        khalaReadinessObservation: {
          leakCount: 2,
          publicModelIds: [
            'openagents/khala',
            'openagents/khala-mini',
            'accounts/fireworks/models/deepseek-v4-flash',
          ],
          readinessStatus: 'ready',
          servableModelCount: 3,
        },
        nowIso,
        scheduleRef: 'cron.public.artanis.khala-leak',
      }),
    )

    const healthProjectionJson =
      store.rows('artanis_health_snapshots')[0]!.public_projection_json

    expect(persistedKhalaReadinessSignal(store)).toMatchObject({
      blockerRefs: expect.arrayContaining([
        'blocker.public.artanis.khala_public_catalog_leak',
        'blocker.public.artanis.khala_public_catalog_not_single_model',
      ]),
      count: 2,
      kind: 'khala_readiness',
      publicRecoveryActionRefs: [
        'recovery.public.artanis.inspect_khala_gateway_catalog',
        'recovery.public.artanis.run_khala_no_spend_monitor',
      ],
      publicStatusRefs: [
        'health.public.artanis.khala_public_catalog_blocked',
      ],
      state: 'blocked',
    })
    expect(healthProjectionJson).not.toContain('openagents/khala-mini')
    expect(healthProjectionJson).not.toContain('accounts/fireworks')
    expect(healthProjectionJson).not.toContain('deepseek-v4-flash')
  })

  test('keeps the executor-trace Forum intent pinned to the promise safeCopy', () => {
    const promise = publicProductPromisesDocument().promises.find(
      item => item.promiseId === 'compute.tassadar_executor_poc.v1',
    )

    expect(promise?.safeCopy).toBe(ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY)
  })

  test('collapses duplicate scheduled retries without duplicate rows', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)
    const input = {
      db,
      enabled: true,
      nowIso,
      scheduleRef: 'cron.public.artanis.20260607T0520',
    }

    await Effect.runPromise(runArtanisScheduledTick(input))
    const retry = await Effect.runPromise(runArtanisScheduledTick(input))

    expect(retry.storageReceipts.every(receipt => receipt.idempotent)).toBe(true)
    expect(store.rows('artanis_runtime_snapshots')).toHaveLength(1)
    expect(store.rows('artanis_loop_records')).toHaveLength(1)
    expect(store.rows('artanis_loop_ticks')).toHaveLength(1)
    expect(store.rows('artanis_approval_gates')).toHaveLength(1)
    expect(store.rows('artanis_health_snapshots')).toHaveLength(1)
    expect(store.rows('artanis_work_routing_proposals')).toHaveLength(2)
    expect(store.rows('artanis_forum_publication_intents')).toHaveLength(1)
  })

  test('worker adapter reads the rollout flag and remains disabled unless explicitly enabled', async () => {
    const disabledStore = new ArtanisPersistenceTestStore()
    const disabled = await Effect.runPromise(
      runArtanisScheduledTickForWorker({
        db: artanisPersistenceTestDb(disabledStore),
        scheduledRunnerEnabled: false,
        scheduledTime,
      }),
    )

    const enabledStore = new ArtanisPersistenceTestStore()
    const enabled = await Effect.runPromise(
      runArtanisScheduledTickForWorker({
        db: artanisPersistenceTestDb(enabledStore),
        scheduledRunnerEnabled: true,
        scheduledTime,
      }),
    )

    expect(disabled.state).toBe('disabled')
    expect([...disabledStore.tables.values()].flat()).toHaveLength(0)
    expect(enabled.state).toBe('completed')
    expect(enabledStore.rows('artanis_loop_ticks')).toHaveLength(1)
  })
})
