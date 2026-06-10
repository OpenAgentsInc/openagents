import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { publicProductPromisesDocument } from './product-promises'

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
  latestGapAuditUrl: S.String,
  lastUpdated: S.String,
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
  schemaVersion: S.String,
  sourceRefs: S.Array(S.String),
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

    expect(decoded.version).toBe('2026-06-09.19')
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
    expect(decoded.verificationSummary.topBlockedPromises.length).toBeGreaterThan(
      0,
    )
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
          state: 'yellow',
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
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.forum_tip_browser_checkout_polish',
            'blocker.product_promises.forum_tip_strict_smooth_live_smoke_unfunded',
            'blocker.product_promises.forum_tip_webhook_live_callback_smoke_missing',
            'blocker.product_promises.forum_tip_refund_reversal_public_smoke',
            'blocker.product_promises.forum_tip_broader_wallet_coverage',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
            'route:/api/forum/paid-actions/mdk/webhooks',
            'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
            'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
            'apps/openagents.com/docs/forum-tip-payout-smoke.md',
            'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
          ]),
          promiseId: 'forum.content_tipping.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
            'blocker.product_promises.forum_tip_strict_smooth_live_smoke_unfunded',
            'blocker.product_promises.forum_tip_webhook_live_callback_smoke_missing',
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
})
