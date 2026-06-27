import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY,
  runArtanisScheduledTick,
  runArtanisScheduledTickForWorker,
} from './artanis-scheduled-runner'
import type { ArtanisKhalaFeedbackRecord } from './artanis-operator-tools'
import type { KhalaTraceReviewReport } from './khala-trace-review-routes'
import type {
  KhalaUnsupportedRequestCreateInput,
  KhalaUnsupportedRequestListInput,
  KhalaUnsupportedRequestRecord,
  KhalaUnsupportedRequestStore,
} from './khala-unsupported-request-routes'
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

class KhalaUnsupportedRequestTestStore implements KhalaUnsupportedRequestStore {
  readonly records = new Map<string, KhalaUnsupportedRequestRecord>()

  async upsert(
    input: KhalaUnsupportedRequestCreateInput,
  ): Promise<KhalaUnsupportedRequestRecord> {
    const key = `${input.sourceKind}:${input.sourceRef}`
    const existing = this.records.get(key)
    const base = {
      createdAt: existing?.createdAt ?? input.createdAt,
      evidenceRefs: input.evidenceRefs,
      forumTopicRef: input.forumTopicRef,
      githubIssueRef: input.githubIssueRef,
      requestRef: existing?.requestRef ?? input.requestRef,
      sourceKind: input.sourceKind,
      sourceRef: input.sourceRef,
      status: input.status,
      suggestedIssueTitle: input.suggestedIssueTitle,
      summary: input.summary,
      title: input.title,
      triageKind: input.triageKind,
      updatedAt: input.updatedAt,
    }
    const issueRequired =
      (base.triageKind === 'bug' || base.triageKind === 'missing_capability') &&
      base.githubIssueRef === null
    const record: KhalaUnsupportedRequestRecord = {
      ...base,
      issueRequired,
      nextAction: issueRequired ? 'open_github_issue' : 'triage',
    }
    this.records.set(key, record)
    return record
  }

  async listRecent(
    input: KhalaUnsupportedRequestListInput,
  ): Promise<ReadonlyArray<KhalaUnsupportedRequestRecord>> {
    return [...this.records.values()]
      .filter(record =>
        input.sourceKind === undefined
          ? true
          : record.sourceKind === input.sourceKind
      )
      .filter(record =>
        input.status === undefined ? true : record.status === input.status
      )
      .filter(record =>
        input.triageKind === undefined
          ? true
          : record.triageKind === input.triageKind
      )
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, input.limit)
  }
}

const traceReviewReport = (): KhalaTraceReviewReport => ({
  aggregates: {
    rawCodexEvents: {
      assignmentCount: 0,
      byteLength: 0,
      eventCount: 0,
      rowCount: 0,
    },
    tokens: {
      estimatedUsageCount: 0,
      eventCount: 2,
      inputTokens: 20,
      outputTokens: 0,
      reasoningTokens: 0,
      totalTokens: 20,
      zeroOutputCount: 2,
    },
    traces: {
      ownerOnlyCount: 2,
      publicCount: 0,
      traceCount: 2,
      trainingConsentCount: 0,
      unlistedCount: 0,
      zeroStepCount: 0,
    },
  },
  backlogFeed: {
    itemRefs: [
      'triage.khala_trace_review.empty_response',
      'triage.khala_trace_review.estimated_usage',
      'triage.intent.khala_trace_review.deep_research',
    ],
    producedItemCount: 3,
  },
  demandSources: [],
  failureModes: [],
  generatedAt: nowIso,
  modelMix: [],
  notableTraces: [],
  outcomes: [],
  reportRef: 'khala_trace_review.2026_06_07T05_20_00_000Z',
  schemaVersion: 'openagents.khala.trace_review.v1',
  sourceTables: [
    'agent_traces',
    'token_usage_events',
    'pylon_codex_raw_events',
  ],
  triageItems: [
    {
      evidenceRefs: ['table.token_usage_events.output_tokens_zero'],
      kind: 'bug',
      priority: 'medium',
      suggestedIssueTitle: '[Khala trace review] Token rows with zero output',
      title: 'Token rows with zero output',
      triageRef: 'triage.khala_trace_review.empty_response',
    },
    {
      evidenceRefs: ['table.token_usage_events.usage_truth_not_exact'],
      kind: 'investigation',
      priority: 'medium',
      suggestedIssueTitle: '[Khala trace review] Usage truth investigation',
      title: 'Usage truth investigation',
      triageRef: 'triage.khala_trace_review.estimated_usage',
    },
    {
      evidenceRefs: ['demand_source.deep_research'],
      kind: 'missing_capability',
      priority: 'low',
      suggestedIssueTitle: '[Khala intent] Review deep_research traces',
      title: 'Review recurring user intent: deep_research',
      triageRef: 'triage.intent.khala_trace_review.deep_research',
    },
  ],
  userIntents: [],
  window: {
    hours: 24,
    since: '2026-06-06T05:20:00.000Z',
    until: nowIso,
  },
})

const feedbackRows: ReadonlyArray<ArtanisKhalaFeedbackRecord> = [
  {
    clientVersion: '1.0.0',
    createdAt: nowIso,
    feedback: 'Please support opening a GitHub issue from a Khala result.',
    feedbackRef: 'khala_feedback:fb_001',
    source: 'khala-cli',
  },
  {
    clientVersion: '1.0.0',
    createdAt: nowIso,
    feedback: 'Please support opening a GitHub issue from a Khala result.',
    feedbackRef: 'khala_feedback:fb_002',
    source: 'khala-cli',
  },
  {
    clientVersion: '1.0.0',
    createdAt: nowIso,
    feedback: 'thanks',
    feedbackRef: 'khala_feedback:fb_noise',
    source: 'khala-cli',
  },
]

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
        'receipt.public.artanis.tassadar_executor_dispatch.cron_public_artanis_20260607T0520',
        'receipt.public.artanis.tassadar_executor_replay_verified.cron_public_artanis_20260607T0520',
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
    expect(store.rows('artanis_work_routing_proposals')).toHaveLength(1)
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
    expect(store.rows('artanis_work_routing_proposals')).toHaveLength(1)
    expect(store.rows('artanis_forum_publication_intents')).toHaveLength(1)
  })

  test('hourly triage upserts trace-review and recurring feedback gaps as needs_issue', async () => {
    const store = new ArtanisPersistenceTestStore()
    const unsupportedStore = new KhalaUnsupportedRequestTestStore()
    const db = artanisPersistenceTestDb(store)
    const input = {
      db,
      enabled: true,
      khalaFeedbackReader: async () => feedbackRows,
      khalaTraceReviewLoader: async () => traceReviewReport(),
      khalaUnsupportedRequestStore: unsupportedStore,
      nowIso,
      scheduleRef: 'cron.public.artanis.triage',
    }

    const result = await Effect.runPromise(runArtanisScheduledTick(input))
    const retry = await Effect.runPromise(runArtanisScheduledTick(input))
    const rows = await unsupportedStore.listRecent({ limit: 10 })

    expect(result.unsupportedRequestRefs).toEqual([
      'khala_unsupported:khala_feedback:khala_feedback_recurring_please_support_opening_a_github_issue_from_a_khala_resu',
      'khala_unsupported:trace_review:triage_intent_khala_trace_review_deep_research',
      'khala_unsupported:trace_review:triage_khala_trace_review_empty_response',
    ])
    expect(retry.unsupportedRequestRefs).toEqual(result.unsupportedRequestRefs)
    expect(rows).toHaveLength(3)
    expect(rows).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nextAction: 'open_github_issue',
          sourceKind: 'trace_review',
          sourceRef: 'triage.khala_trace_review.empty_response',
          status: 'needs_issue',
          triageKind: 'bug',
        }),
        expect.objectContaining({
          nextAction: 'open_github_issue',
          sourceKind: 'trace_review',
          sourceRef: 'triage.intent.khala_trace_review.deep_research',
          status: 'needs_issue',
          triageKind: 'missing_capability',
        }),
        expect.objectContaining({
          evidenceRefs: ['khala_feedback:fb_001', 'khala_feedback:fb_002'],
          nextAction: 'open_github_issue',
          sourceKind: 'khala_feedback',
          status: 'needs_issue',
          triageKind: 'missing_capability',
        }),
      ]),
    )
    expect(rows.some(row => row.title === 'Usage truth investigation')).toBe(false)
    expect(rows.some(row => row.evidenceRefs.includes('khala_feedback:fb_noise'))).toBe(false)
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
