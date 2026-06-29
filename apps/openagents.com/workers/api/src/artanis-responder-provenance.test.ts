import { describe, expect, test } from 'vitest'

import {
  ARTANIS_RESPONDER_EXTERNAL_CONTRIBUTOR_FLOW_BLOCKER,
  ARTANIS_RESPONDER_TEN_UNATTENDED_TICKS_BLOCKER,
  boundedResponderSupportLimit,
  classifyAskerProvenance,
  projectArtanisResponderSupport,
  type ArtanisResponderActionRow,
} from './artanis-responder-provenance'
import type { ArtanisResponderTickReadinessProjection } from './artanis-responder-ticks'

const nowIso = '2026-06-20T00:00:00.000Z'

const tickReadiness = (
  overrides: Partial<ArtanisResponderTickReadinessProjection> = {},
): ArtanisResponderTickReadinessProjection => ({
  authorityBoundary: 'test',
  externalContributorAnsweredWithinTickWindow: true,
  kind: 'artanis_pylon_support_responder_tick_readiness',
  notes: [],
  publicSafe: true,
  qualifyingUnattendedResponderTickCount: 10,
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['test'],
  },
  tickTarget: 10,
  tickWindows: [],
  unattendedResponderTicksProven: true,
  ...overrides,
})

const row = (
  overrides: Partial<ArtanisResponderActionRow>,
): ArtanisResponderActionRow => ({
  asked_at: '2026-06-20T00:00:00.000Z',
  asker_actor_ref: 'user:user_external_1',
  asker_provenance: 'external_contributor',
  id: 'action-1',
  question_class: 'device_capability',
  replied_at: '2026-06-20T00:01:00.000Z',
  reply_post_id: 'post.public.forum.artanis.status.42',
  state: 'responded',
  tip_receipt_ref: null,
  topic_id: 'topic-1',
  ...overrides,
})

describe('classifyAskerProvenance', () => {
  test('a plain user ref is an external contributor', () => {
    expect(classifyAskerProvenance('user:user_abc')).toBe(
      'external_contributor',
    )
  })

  test('a non-internal agent ref is an external contributor', () => {
    expect(classifyAskerProvenance('agent:user_some_other_agent')).toBe(
      'external_contributor',
    )
  })

  test('operator and owner refs are owner_operator', () => {
    expect(classifyAskerProvenance('operator:op_1')).toBe('owner_operator')
    expect(classifyAskerProvenance('owner:owner_1')).toBe('owner_operator')
  })

  test('pinned admin actor refs are owner_operator, never external', () => {
    expect(
      classifyAskerProvenance('user:admin_owner', {
        adminActorRefs: ['user:admin_owner'],
      }),
    ).toBe('owner_operator')
  })

  test('the Artanis registered and delivery refs are artanis_self', () => {
    expect(
      classifyAskerProvenance('agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505'),
    ).toBe('artanis_self')
    expect(classifyAskerProvenance('agent:agent_artanis')).toBe('artanis_self')
  })

  test('empty or unrecognized refs are unknown', () => {
    expect(classifyAskerProvenance('')).toBe('unknown')
    expect(classifyAskerProvenance(null)).toBe('unknown')
    expect(classifyAskerProvenance('team:team_1')).toBe('unknown')
  })
})

describe('projectArtanisResponderSupport', () => {
  test('an answered external contributor proves the flow with a deref ref', () => {
    const projection = projectArtanisResponderSupport(
      [row({})],
      nowIso,
      tickReadiness(),
    )
    expect(projection.kind).toBe('artanis_pylon_support_responder_external_flow')
    expect(projection.publicSafe).toBe(true)
    expect(projection.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(projection.staleness.composition).toBe('live_at_read')
    expect(projection.externalContributorAnsweredCount).toBe(1)
    expect(projection.externalContributorFlowProven).toBe(true)
    expect(projection.externalInteractions).toHaveLength(1)
    expect(projection.externalInteractions[0]?.replyPostRef).toBe(
      'post.public.forum.artanis.status.42',
    )
    expect(projection.externalInteractions[0]?.publicUrl).toContain(
      '#post-post.public.forum.artanis.status.42',
    )
    expect(projection.blockerRefs).toEqual([
      ARTANIS_RESPONDER_EXTERNAL_CONTRIBUTOR_FLOW_BLOCKER,
      ARTANIS_RESPONDER_TEN_UNATTENDED_TICKS_BLOCKER,
    ])
    expect(projection.remainingBlockerRefs).toEqual([])
  })

  test('a tipped external interaction counts the economic leg', () => {
    const projection = projectArtanisResponderSupport(
      [
        row({
          state: 'tipped',
          tip_receipt_ref: 'receipt.public.forum.tip.abc',
        }),
      ],
      nowIso,
    )
    expect(projection.externalContributorTippedCount).toBe(1)
    expect(projection.externalInteractions[0]?.tipped).toBe(true)
    expect(projection.externalInteractions[0]?.tipReceiptRef).toBe(
      'receipt.public.forum.tip.abc',
    )
  })

  test('operator-authored answers never prove the external flow', () => {
    const projection = projectArtanisResponderSupport(
      [
        row({
          asker_actor_ref: 'operator:op_1',
          asker_provenance: 'owner_operator',
        }),
      ],
      nowIso,
    )
    expect(projection.ownerOperatorAnsweredCount).toBe(1)
    expect(projection.externalContributorAnsweredCount).toBe(0)
    expect(projection.externalContributorFlowProven).toBe(false)
    expect(projection.remainingBlockerRefs).toEqual([
      ARTANIS_RESPONDER_EXTERNAL_CONTRIBUTOR_FLOW_BLOCKER,
      ARTANIS_RESPONDER_TEN_UNATTENDED_TICKS_BLOCKER,
    ])
  })

  test('proposed and skipped actions are not answered interactions', () => {
    const projection = projectArtanisResponderSupport(
      [
        row({ state: 'proposed' }),
        row({ id: 'action-2', topic_id: 'topic-2', state: 'skipped' }),
      ],
      nowIso,
    )
    expect(projection.externalContributorAnsweredCount).toBe(0)
    expect(projection.externalContributorFlowProven).toBe(false)
  })

  test('legacy rows with no recorded provenance re-derive from the actor ref', () => {
    const projection = projectArtanisResponderSupport(
      [row({ asker_provenance: null, asker_actor_ref: 'user:legacy_user' })],
      nowIso,
    )
    expect(projection.externalContributorAnsweredCount).toBe(1)
    expect(projection.externalContributorFlowProven).toBe(true)
  })

  test('a smuggled non-ref reply value is redacted to null', () => {
    const projection = projectArtanisResponderSupport(
      [row({ reply_post_id: 'has spaces and <script>' })],
      nowIso,
    )
    // The interaction still counts as answered, but the unsafe ref is dropped
    // so it cannot prove the flow nor leak.
    expect(projection.externalContributorAnsweredCount).toBe(1)
    expect(projection.externalInteractions[0]?.replyPostRef).toBeNull()
    expect(projection.externalContributorFlowProven).toBe(false)
  })

  test('the artanis self actor is never counted', () => {
    const projection = projectArtanisResponderSupport(
      [
        row({
          asker_actor_ref: 'agent:agent_artanis',
          asker_provenance: 'artanis_self',
        }),
      ],
      nowIso,
    )
    expect(projection.externalContributorAnsweredCount).toBe(0)
    expect(projection.ownerOperatorAnsweredCount).toBe(0)
  })

  test('keeps the ten-tick blocker explicit until tick readiness reaches target', () => {
    const projection = projectArtanisResponderSupport(
      [row({})],
      nowIso,
      tickReadiness({
        qualifyingUnattendedResponderTickCount: 9,
        unattendedResponderTicksProven: false,
      }),
    )
    expect(projection.externalContributorFlowProven).toBe(true)
    expect(projection.remainingBlockerRefs).toEqual([
      ARTANIS_RESPONDER_TEN_UNATTENDED_TICKS_BLOCKER,
    ])
  })
})

describe('boundedResponderSupportLimit', () => {
  test('clamps to a sane bounded range', () => {
    expect(boundedResponderSupportLimit(null)).toBe(100)
    expect(boundedResponderSupportLimit('not-a-number')).toBe(100)
    expect(boundedResponderSupportLimit('0')).toBe(1)
    expect(boundedResponderSupportLimit('5')).toBe(5)
    expect(boundedResponderSupportLimit('99999')).toBe(200)
  })
})
