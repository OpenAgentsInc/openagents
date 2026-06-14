import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  publicProductPromisesAnnouncementReadiness,
  publicProductPromisesDocument,
} from './product-promises'

const ProductPromiseState = S.Literals([
  'degraded',
  'green',
  'planned',
  'red',
  'withdrawn',
  'yellow',
])

const ProductPromise = S.Struct({
  audience: S.Array(S.String),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  claim: S.String,
  evidenceRefs: S.Array(S.String),
  productArea: S.String,
  promiseId: S.String,
  reportPath: S.String,
  safeCopy: S.String,
  sourceRefs: S.Array(S.String),
  state: ProductPromiseState,
  unsafeCopy: S.String,
  verification: S.String,
})

const ProductPromiseBlockedSummary = S.Struct({
  blockerRefs: S.Array(S.String),
  promiseId: S.String,
  state: S.String,
})

const ProductPromisesDocument = S.Struct({
  canonicalDocsUrl: S.String,
  currentMonorepoStatus: S.Struct({
    caveats: S.Array(S.String),
    liveDeploymentRefs: S.Array(S.String),
    pylonV03Refs: S.Array(S.String),
    status: S.String,
    summary: S.String,
  }),
  generatedAt: S.String,
  latestGapAuditUrl: S.String,
  lastUpdated: S.String,
  maxStalenessSeconds: S.Number,
  notes: S.Array(S.String),
  promises: S.Array(ProductPromise),
  publicDocsUrl: S.String,
  reportPath: S.Struct({
    defaultForumUrl: S.String,
    forumSlug: S.String,
    forumTopicApi: S.String,
    rule: S.String,
    strictBugForm: S.String,
  }),
  registryVersion: S.String,
  schemaVersion: S.String,
  sourceRefs: S.Array(S.String),
  staleness: S.Struct({
    composition: S.Literal('live_at_read'),
    contractVersion: S.Literal('projection_staleness.v1'),
    maxStalenessSeconds: S.Number,
    rebuildsOn: S.Array(S.String),
  }),
  states: S.Record(S.String, S.String),
  verificationSummary: S.Struct({
    blockedPromiseCount: S.Int,
    evidenceRefCount: S.Int,
    promiseCount: S.Int,
    promisesWithBlockersCount: S.Int,
    topBlockedPromises: S.Array(ProductPromiseBlockedSummary),
    uniqueBlockerCount: S.Int,
    uniqueBlockers: S.Array(S.String),
  }),
  version: S.String,
})

describe('public product promises document', () => {
  test('matches the browser-facing schema', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )

    expect(decoded.version).toBe('2026-06-14.1')
    expect(decoded.registryVersion).toBe(decoded.version)
    expect(Date.parse(decoded.generatedAt)).not.toBeNaN()
    expect(decoded.maxStalenessSeconds).toBe(0)
    expect(decoded.staleness.maxStalenessSeconds).toBe(0)
    expect(decoded.staleness.rebuildsOn).toEqual(
      expect.arrayContaining([
        'product_promise_registry_changed',
        'product_promise_transition_receipt_recorded',
        'product_promise_announcement_preflight',
      ]),
    )
    expect(decoded.sourceRefs.length).toBeGreaterThan(0)
    expect(decoded.sourceRefs).toContain(
      'https://github.com/OpenAgentsInc/openagents',
    )
    expect(decoded.promises.length).toBeGreaterThan(0)
    expect(decoded.verificationSummary.promiseCount).toBe(
      decoded.promises.length,
    )
    expect(decoded.verificationSummary.evidenceRefCount).toBeGreaterThan(0)
    expect(decoded.verificationSummary.uniqueBlockerCount).toBeGreaterThan(0)
    expect(
      decoded.verificationSummary.topBlockedPromises.length,
    ).toBeGreaterThan(0)
    expect(
      decoded.promises.every(promise => promise.sourceRefs.length > 0),
    ).toBe(true)
    expect(decoded.promises).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          promiseId: 'autopilot.mission_briefing.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          promiseId: 'autopilot.decision_queue.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'workrooms.source_authorized_business_objects.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'mobile.voice_approval_companion.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'pylon.no_dark_capacity_accounting.v1',
          state: 'green',
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe',
            'route:/api/public/pylon-capacity-funnel/history',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'payments.accepted_outcome_economics.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'energy.flexible_load_proof.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'training.full_pipeline_program.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'training.model_ladder.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.r1_full_rehearsal_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'training.public_distributed_training_run.v1',
          state: 'red',
          evidenceRefs: expect.arrayContaining(['docs/transcripts/236.md']),
        }),
        expect.objectContaining({
          promiseId: 'training.monday_decentralized_training_launch.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.monday_training_launch_receipts_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'pylon.largest_decentralized_training_claim.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'models.tasadar_percepta_executor.v1',
          state: 'withdrawn',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.misspelled_promise_id_withdrawn',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'models.tassadar_percepta_executor.v1',
          state: 'red',
          evidenceRefs: expect.arrayContaining([
            'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'pylon.v0_3_multi_earning_node.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'training.verification_classes.v1',
          state: 'yellow',
          evidenceRefs: expect.arrayContaining([
            'receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500',
            'promise_transition_0bfce0c5-e4dd-4d19-9221-4bc9504f2055',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'proof.demand_provenance.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'proof.claim_upgrade_receipts.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          promiseId: 'repo.open_source_code_map.v1',
          state: 'green',
          evidenceRefs: expect.arrayContaining([
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/workers/api',
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/apps/web',
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon',
            'https://github.com/OpenAgentsInc/openagents/tree/main/packages/probe',
          ]),
          authorityBoundary: expect.stringContaining(
            'does not grant write, deploy, spend',
          ),
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
          ]),
          promiseId: 'agents.cursor_forum_wallet.v1',
          state: 'green',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
            'route:/api/forum/paid-actions/mdk/webhooks',
            'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
            'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
            'apps/openagents.com/docs/forum-tip-payout-smoke.md',
            'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
            'apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md',
            'transition:promise_transition_c106102b-e51b-4d2f-84ed-a588f1a26316',
            'transition:promise_transition_feab90da-aead-49e1-9097-bd0b8bb5c11a',
            'transition:promise_transition_e632649a-acfa-4e69-ad4b-269e92c963b3',
            'transition:promise_transition_0cfba5d7-40ff-48bd-81a3-4b0758b0acd8',
          ]),
          promiseId: 'forum.content_tipping.v1',
          state: 'green',
        }),
        expect.objectContaining({
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
            'route:/api/forum/paid-actions/mdk/webhooks',
            'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
            'apps/openagents.com/docs/forum-tip-payout-smoke.md',
            'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
          ]),
          promiseId: 'payments.money_dev_kit.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/docs/2026-06-10-compliant-usage-labor-policy.md',
          ]),
          promiseId: 'provider.compliant_usage_labor.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'apps/pylon/docs/live-worker-loop-smoke.md',
          ]),
          promiseId: 'pylon.cli_tui_probe_background.v1',
          state: 'green',
        }),
      ]),
    )
  })

  test('blocks announcement copy until the live endpoint serves the announced version', () => {
    const document = publicProductPromisesDocument()

    expect(
      publicProductPromisesAnnouncementReadiness('2026-06-14.1', document),
    ).toMatchObject({
      blockerRefs: [],
      expectedVersion: '2026-06-14.1',
      maxStalenessSeconds: 0,
      servedVersion: '2026-06-14.1',
      status: 'ready',
    })
    expect(
      publicProductPromisesAnnouncementReadiness('2026-06-12.1', document),
    ).toMatchObject({
      blockerRefs: [
        'product-promises-announcement-blocker:expected-version-not-served:2026-06-12.1',
      ],
      expectedVersion: '2026-06-12.1',
      servedVersion: '2026-06-14.1',
      status: 'blocked',
    })
  })
})
