import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  artanisNexusPylonForumEventFromReleaseGate,
  buildArtanisNexusPylonForumPublicationQueue,
  exampleArtanisNexusPylonForumBridgePolicy,
  exampleArtanisNexusPylonForumEvents,
  projectArtanisNexusPylonForumBridge,
  saveArtanisNexusPylonForumPublicationIntents,
} from './artanis-nexus-pylon-forum-bridge'
import {
  ArtanisForumPublicationUnsafe,
  artanisForumPublicationProjectionHasPrivateMaterial,
  projectArtanisForumPublicationQueue,
  selectReadyArtanisForumPublicationIntents,
} from './artanis-forum-publication'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'
import {
  currentPylonV02OmegaReleaseGateRecord,
  projectPylonV02OmegaReleaseGate,
  readyPylonV02OmegaReleaseGateRecord,
} from './pylon-v02-omega-release-gate'

const nowIso = '2026-06-07T06:30:00.000Z'

describe('Artanis Nexus/Pylon Forum bridge', () => {
  test('maps Nexus/Pylon event kinds to public-safe Artanis Forum intents', () => {
    const events = exampleArtanisNexusPylonForumEvents()
    const policy = exampleArtanisNexusPylonForumBridgePolicy()
    const queue = buildArtanisNexusPylonForumPublicationQueue({
      events,
      policy,
    })
    const projection = projectArtanisForumPublicationQueue(queue, nowIso)
    const bridge = projectArtanisNexusPylonForumBridge({
      events,
      nowIso,
      policy,
    })

    expect(projection.intentCount).toBe(8)
    expect(bridge).toMatchObject({
      blockedIntentRefs: [],
      deliveryPaused: false,
      duplicateIntentRefs: [],
      intentCount: 8,
      state: 'enabled',
    })
    expect(bridge.readyIntentRefs).toHaveLength(8)
    expect(projection.intents.map(intent => intent.targetTopicRef)).toEqual([
      'topic.public.forum.artanis.work_routing',
      'topic.public.forum.artanis.pylon_campaign',
      'topic.public.forum.artanis.pylon_release_work_log',
      'topic.public.forum.artanis.operator_questions',
      'topic.public.forum.artanis.bitcoin_accounting',
      'topic.public.forum.artanis.bitcoin_accounting',
      'topic.public.forum.artanis.pylon_release_work_log',
      'topic.public.forum.artanis.pylon_release_work_log',
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(artanisForumPublicationProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('collapses exact duplicate events to one deliverable intent', () => {
    const event = exampleArtanisNexusPylonForumEvents()[0]!
    const queue = buildArtanisNexusPylonForumPublicationQueue({
      events: [event, event],
      policy: exampleArtanisNexusPylonForumBridgePolicy(),
    })
    const projection = projectArtanisForumPublicationQueue(queue, nowIso)

    expect(projection.intentCount).toBe(1)
    expect(projection.duplicateIntentRefs).toEqual([
      'forum.public.artanis.nexus_pylon.assignment_created.event_public_artanis_nexus_pylon_assignment_created_gepa_autopilot_001',
    ])
    expect(selectReadyArtanisForumPublicationIntents(queue)).toHaveLength(1)
  })

  test('blocks publication while the bridge is paused or disabled', () => {
    const events = [exampleArtanisNexusPylonForumEvents()[0]!]
    const paused = projectArtanisNexusPylonForumBridge({
      events,
      nowIso,
      policy: {
        ...exampleArtanisNexusPylonForumBridgePolicy(),
        blockerRefs: [],
        state: 'paused',
      },
    })
    const disabled = projectArtanisNexusPylonForumBridge({
      events,
      nowIso,
      policy: {
        ...exampleArtanisNexusPylonForumBridgePolicy(),
        blockerRefs: [],
        state: 'disabled',
      },
    })

    expect(paused.deliveryPaused).toBe(true)
    expect(paused.readyIntentRefs).toEqual([])
    expect(paused.blockedIntentRefs).toHaveLength(1)
    expect(disabled.deliveryPaused).toBe(true)
    expect(disabled.readyIntentRefs).toEqual([])
    expect(disabled.blockedIntentRefs).toHaveLength(1)
  })

  test('persists ready intents idempotently for delivery by the existing bridge', async () => {
    const events = [exampleArtanisNexusPylonForumEvents()[0]!]
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)
    const input = {
      events,
      nowIso,
      policy: exampleArtanisNexusPylonForumBridgePolicy(),
    }
    const first = await Effect.runPromise(
      saveArtanisNexusPylonForumPublicationIntents(db, input),
    )
    const second = await Effect.runPromise(
      saveArtanisNexusPylonForumPublicationIntents(db, input),
    )

    expect(first).toHaveLength(1)
    expect(first[0]!.state).toBe('inserted')
    expect(second).toHaveLength(1)
    expect(second[0]!.state).toBe('retried')
    expect(store.rows('artanis_forum_publication_intents')).toHaveLength(1)
    expect(store.rows('artanis_forum_publication_intents')[0]!.state)
      .toBe('ready')
  })

  test('rejects private wallet, invoice, and customer material before posting', () => {
    const unsafeEvent = {
      ...exampleArtanisNexusPylonForumEvents()[0]!,
      publicContextRefs: ['context.public.mdk_mnemonic_leak'],
    }

    expect(() =>
      projectArtanisNexusPylonForumBridge({
        events: [unsafeEvent],
        nowIso,
        policy: exampleArtanisNexusPylonForumBridgePolicy(),
      }),
    ).toThrow(ArtanisForumPublicationUnsafe)
  })

  test('builds a public-safe passed release-gate Forum update from the current gate', () => {
    const releaseGate = projectPylonV02OmegaReleaseGate(
      currentPylonV02OmegaReleaseGateRecord(),
      'public',
      nowIso,
    )
    const event = artanisNexusPylonForumEventFromReleaseGate({
      createdAtIso: '2026-06-07T07:00:00.000Z',
      projection: releaseGate,
      updatedAtIso: '2026-06-07T07:00:00.000Z',
    })
    const queue = buildArtanisNexusPylonForumPublicationQueue({
      events: [event],
      policy: exampleArtanisNexusPylonForumBridgePolicy(),
    })
    const projection = projectArtanisForumPublicationQueue(queue, nowIso)
    const intent = projection.intents[0]!

    expect(intent.targetTopicRef).toBe(
      'topic.public.forum.artanis.pylon_release_work_log',
    )
    expect(intent.bodyText).toContain('Release gate status: passed.')
    expect(intent.bodyText).toContain('Pylon v0.2 OpenAgents Nexus proof is complete')
    expect(intent.bodyText).toContain('Public links: https://openagents.com/artanis')
    expect(intent.bodyText).toContain(
      'receipt.public.nexus.issue_438.settlement.issue_438_artanis_1780822221',
    )
    expect(intent.bodyText).toContain(
      'receipt.public.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
    )
    expect(intent.blockerRefs).toEqual([])
    expect(intent.caveatRefs).not.toContain(
      'caveat.public.pylon_v0_2_omega_release_gate_blocked',
    )
    expect(intent.pageUrls).toContain(
      'https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
    )
    expect(intent.pageUrls).toContain(
      'https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
    )
    expect(intent.receiptRefs).toContain(
      'receipt.public.nexus.issue_438.settlement.issue_438_artanis_1780822221',
    )
    expect(intent.receiptRefs).toContain(
      'receipt.public.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(intent.bodyText).not.toContain('Pylon v0.2 is shipped')
    expect(intent.bodyText).not.toContain('Pylon v0.2 is released')
    expect(intent.bodyText).not.toContain('Omega')
    expect(artanisForumPublicationProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('includes receipt links when release-gate proof is complete', () => {
    const releaseGate = projectPylonV02OmegaReleaseGate(
      readyPylonV02OmegaReleaseGateRecord(),
      'public',
      nowIso,
    )
    const event = artanisNexusPylonForumEventFromReleaseGate({
      createdAtIso: '2026-06-07T07:05:00.000Z',
      projection: releaseGate,
      updatedAtIso: '2026-06-07T07:05:00.000Z',
    })
    const queue = buildArtanisNexusPylonForumPublicationQueue({
      events: [event],
      policy: exampleArtanisNexusPylonForumBridgePolicy(),
    })
    const projection = projectArtanisForumPublicationQueue(queue, nowIso)
    const intent = projection.intents[0]!

    expect(intent.bodyText).toContain('Release gate status: passed.')
    expect(intent.bodyText).toContain(
      'https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
    )
    expect(intent.bodyText).toContain(
      'https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
    )
    expect(intent.receiptRefs).toEqual(
      expect.arrayContaining([
        'receipt.public.nexus.issue_438.settlement.issue_438_artanis_1780822221',
        'receipt.public.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
      ]),
    )
    expect(intent.blockerRefs).toEqual([])
    expect(intent.caveatRefs).not.toContain(
      'caveat.public.pylon_v0_2_omega_release_gate_blocked',
    )
    expect(artanisForumPublicationProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })
})
