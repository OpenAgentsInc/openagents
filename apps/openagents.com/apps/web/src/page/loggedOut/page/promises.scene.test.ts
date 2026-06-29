import { Scene } from 'foldkit'
import { describe, test } from 'vitest'

import { LoggedOut } from '../../../model'
import { ProductPromisesRoute } from '../../../route'
import { update } from '../../../update'
import {
  type PublicProductPromises,
  type PublicPromiseTransitions,
  IdlePublicProductPromises,
  IdlePublicPromiseTransitions,
  LoadedPublicProductPromises,
  LoadedPublicPromiseTransitions,
} from '../model'
import { view } from './promises'

const promisesFixture: PublicProductPromises = {
  canonicalDocsUrl: 'https://openagents.com/docs/product-promises',
  currentMonorepoStatus: {
    caveats: [],
    liveDeploymentRefs: [],
    pylonV03Refs: [],
    status: 'live',
    summary: 'fixture',
  },
  latestGapAuditUrl: '',
  lastUpdated: '2026-06-19T00:00:00.000Z',
  notes: [],
  promises: [
    {
      audience: ['public'],
      authorityBoundary: 'fixture boundary',
      blockerRefs: [],
      claim: 'A negotiated labor job can settle end to end.',
      evidenceRefs: ['docs/labor/evidence.md'],
      productArea: 'labor',
      promiseId: 'labor.forum_work_requests.v1',
      reportPath: 'forum',
      safeCopy: 'fixture safe copy',
      sourceRefs: [],
      state: 'green',
      unsafeCopy: 'fixture unsafe copy',
      verification: 'fixture verification',
    },
  ],
  publicDocsUrl: '',
  reportPath: {
    defaultForumUrl: '',
    forumSlug: 'product-promises',
    forumTopicApi: '',
    rule: '',
    strictBugForm: '',
  },
  schemaVersion: '1',
  sourceRefs: [],
  states: {},
  verificationSummary: {
    blockedPromiseCount: 0,
    evidenceRefCount: 1,
    promiseCount: 1,
    promisesWithBlockersCount: 0,
    topBlockedPromises: [],
    uniqueBlockerCount: 0,
    uniqueBlockers: [],
  },
  version: '2026-06-19.8',
}

const transitionsFixture: PublicPromiseTransitions = {
  kind: 'product_promise_transitions',
  publicSafe: true,
  rule: 'A passing receipt is mechanical evidence for a proposed state transition. It is not the transition itself.',
  receipts: [
    {
      checkedAt: '2026-06-15T00:00:00.000Z',
      checks: [
        { kind: 'promise_exists', result: 'passed' },
        { kind: 'evidence_refs_present', result: 'passed' },
      ],
      evidenceRefs: ['docs/labor/evidence.md'],
      exception: {
        approvedByRef: 'owner:operator-route#5017',
        expiresAt: '2026-12-31T00:00:00.000Z',
        reasonRef: 'docs/promises/reconciliation.md',
      },
      fromState: 'green',
      promiseId: 'labor.forum_work_requests.v1',
      receiptId: 'promise_transition_a38a3472',
      registryVersion: '2026-06-14.1',
      result: 'exception',
      toState: 'green',
    },
  ],
}

describe('claim-upgrade audit panel', () => {
  test('renders the audit panel heading and rule before any receipts load', () => {
    Scene.scene(
      {
        update,
        view: () =>
          view(IdlePublicProductPromises(), IdlePublicPromiseTransitions()),
      },
      Scene.with(LoggedOut.init(ProductPromisesRoute())),
      Scene.expect(
        Scene.role('heading', { name: 'Claim-upgrade audit panel' }),
      ).toExist(),
      Scene.expect(
        Scene.text('proof.claim_upgrade_receipts.v1'),
      ).toExist(),
    )
  })

  test('renders a transition receipt with before/after, owner signoff, and green-flip backing', () => {
    Scene.scene(
      {
        update,
        view: () =>
          view(
            LoadedPublicProductPromises({ promises: promisesFixture }),
            LoadedPublicPromiseTransitions({
              transitions: transitionsFixture,
            }),
          ),
      },
      Scene.with(LoggedOut.init(ProductPromisesRoute())),
      // The receipt id is the dereferenceable handle for the flip.
      Scene.expect(Scene.text('promise_transition_a38a3472')).toExist(),
      // Before/after state of the recorded transition.
      Scene.expect(Scene.text('Before / after')).toExist(),
      // Owner signoff is surfaced from the policy-exception approver ref.
      Scene.expect(
        Scene.text(
          'Approved by owner:operator-route#5017 (expires 2026-12-31T00:00:00.000Z).',
        ),
      ).toExist(),
      // The audit summary tallies green flips and their receipt backing.
      Scene.expect(Scene.text('Green flips receipt-backed')).toExist(),
      // The "receipt is not the flip" rule must remain on the page.
      Scene.expect(
        Scene.text(
          'A passing receipt is mechanical evidence for a proposed state transition. It is not the transition itself.',
        ),
      ).toExist(),
    )
  })
})
