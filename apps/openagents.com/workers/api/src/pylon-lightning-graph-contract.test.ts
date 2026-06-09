import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_LIGHTNING_GRAPH_CONFORMANCE_FIXTURES,
  PYLON_LIGHTNING_GRAPH_READ_ONLY_AUTHORITY,
  PylonLightningGraphProjection,
  PylonLightningGraphRecord,
  PylonLightningGraphUnsafe,
  projectPylonLightningGraph,
  pylonLightningGraphCanMutate,
  pylonLightningGraphHasNoMutationAuthority,
  pylonLightningGraphProjectionHasPrivateMaterial,
} from './pylon-lightning-graph-contract'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T10:30:00.000Z'

const graphRecord = (
  overrides: Partial<PylonLightningGraphRecord> = {},
): PylonLightningGraphRecord =>
  S.decodeUnknownSync(PylonLightningGraphRecord)({
    ...PYLON_LIGHTNING_GRAPH_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon Lightning graph contract', () => {
  test('decodes and projects the contract without mutation authority', () => {
    const record = graphRecord()
    const projection = projectPylonLightningGraph(record, 'operator', nowIso)

    expect(S.decodeUnknownSync(PylonLightningGraphRecord)(record)).toEqual(
      record,
    )
    expect(S.decodeUnknownSync(PylonLightningGraphProjection)(projection))
      .toEqual(projection)
    expect(pylonLightningGraphHasNoMutationAuthority(record.authority)).toBe(
      true,
    )
    expect(pylonLightningGraphCanMutate(record)).toBe(false)
    expect(projection.graphMutationAllowed).toBe(false)
    expect(projection.channelMutationAllowed).toBe(false)
    expect(projection.peerMutationAllowed).toBe(false)
    expect(projection.liquidityMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.walletMutationAllowed).toBe(false)
    expect(projection.implementationStatus).toBe('contract_only')
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.nodeCount).toBe(8)
    expect(projection.edgeCount).toBe(7)
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonLightningGraphProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('redacts private nodes, edges, refs, and linked private edges from public projection', () => {
    const projection = projectPylonLightningGraph(
      graphRecord(),
      'public',
      nowIso,
    )

    expect(projection.nodes.map(node => node.kind).sort()).toEqual([
      'failed_route',
      'liquidity_movement',
      'payout_event',
      'provider',
      'rail',
      'settlement_receipt',
    ])
    expect(projection.edges.map(edge => edge.kind).sort()).toEqual([
      'failed_route_on_rail',
      'provider_rail',
      'settlement_evidence',
      'work_payout',
    ])
    expect(projection.nodeCount).toBe(6)
    expect(projection.edgeCount).toBe(4)
    expect(JSON.stringify(projection)).not.toContain('private')
    expect(projection.page.nextCursorRef).toBe('cursor.public.graph_page_2')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('keeps graph filter and pagination shape redacted and bounded', () => {
    const projection = projectPylonLightningGraph(
      graphRecord({
        filters: {
          edgeKinds: ['work_payout', 'provider_rail', 'work_payout'],
          freshness: ['fresh', 'fresh'],
          nodeKinds: ['payout_event', 'provider', 'provider'],
          providerRefs: [
            'provider.public.pylon_1',
            'provider.private.operator_only',
          ],
          railRefs: ['rail.public.ldk', 'rail.private.operator_only'],
          statuses: ['settled', 'active', 'active'],
        },
      }),
      'public',
      nowIso,
    )

    expect(projection.filters.edgeKinds).toEqual([
      'provider_rail',
      'work_payout',
    ])
    expect(projection.filters.freshness).toEqual(['fresh'])
    expect(projection.filters.nodeKinds).toEqual([
      'payout_event',
      'provider',
    ])
    expect(projection.filters.providerRefs).toEqual([
      'provider.public.pylon_1',
    ])
    expect(projection.filters.railRefs).toEqual(['rail.public.ldk'])
    expect(projection.filters.statuses).toEqual(['active', 'settled'])

    expect(() =>
      projectPylonLightningGraph(
        graphRecord({
          page: {
            limit: 0,
            nextCursorRef: null,
            requestedCursorRef: null,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLightningGraphUnsafe)
  })

  test('requires unique graph ids, known edge endpoints, and evidence for failed or stale states', () => {
    const base = graphRecord()

    expect(() =>
      projectPylonLightningGraph({
        ...base,
        nodes: [
          base.nodes[0]!,
          {
            ...base.nodes[1]!,
            id: base.nodes[0]!.id,
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonLightningGraphUnsafe)

    expect(() =>
      projectPylonLightningGraph({
        ...base,
        edges: [
          {
            ...base.edges[0]!,
            toNodeId: 'node.public.missing',
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonLightningGraphUnsafe)

    expect(() =>
      projectPylonLightningGraph({
        ...base,
        nodes: [
          {
            ...base.nodes[0]!,
            blockerRefs: [],
            status: 'failed',
          },
        ],
        edges: [],
      }, 'operator', nowIso),
    ).toThrow(PylonLightningGraphUnsafe)

    expect(() =>
      projectPylonLightningGraph({
        ...base,
        nodes: [
          {
            ...base.nodes[0]!,
            caveatRefs: [],
            freshness: 'stale',
          },
        ],
        edges: [],
      }, 'operator', nowIso),
    ).toThrow(PylonLightningGraphUnsafe)
  })

  test('rejects mutation authority and unsafe Lightning, peer, wallet, payment, and payout material', () => {
    const base = graphRecord()

    expect(() =>
      projectPylonLightningGraph({
        ...base,
        authority: {
          ...PYLON_LIGHTNING_GRAPH_READ_ONLY_AUTHORITY,
          noPayoutDispatch: false,
        },
      }, 'operator', nowIso),
    ).toThrow(PylonLightningGraphUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'node pubkey', value: 'node_pubkey.raw_value' },
      { label: 'peer secret', value: 'peer_secret.raw_value' },
      { label: 'raw invoice', value: 'raw_invoice.bolt11_full' },
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'preimage', value: 'payment_preimage.raw_secret' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
      { label: 'payout target', value: 'payout_target.raw_destination' },
    ]) {
      expect(() =>
        projectPylonLightningGraph({
          ...base,
          nodes: [
            {
              ...base.nodes[0]!,
              evidenceRefs: [fixture.value],
            },
          ],
          edges: [],
        }, 'operator', nowIso),
      ).toThrow(PylonLightningGraphUnsafe)
    }
  })
})
