import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_MODEL_LAB_GRAPH_READ_ONLY_AUTHORITY,
  OmniModelLabEvidenceGraphProjection,
  OmniModelLabEvidenceGraphRecord,
  OmniModelLabEvidenceGraphUnsafe,
  exampleOmniModelLabEvidenceGraph,
  omniModelLabEvidenceGraphProjectionHasPrivateMaterial,
  projectOmniModelLabEvidenceGraph,
} from './omni-model-lab-evidence-graph'

const nowIso = '2026-06-06T23:30:00.000Z'

const graphRecord = (
  overrides: Partial<OmniModelLabEvidenceGraphRecord> = {},
): OmniModelLabEvidenceGraphRecord =>
  S.decodeUnknownSync(OmniModelLabEvidenceGraphRecord)({
    ...exampleOmniModelLabEvidenceGraph(),
    ...overrides,
  })

describe('Omni Model Lab evidence graph', () => {
  test('projects a connected retained-failure evidence graph without eval, training, provider, adapter, spend, runtime, routing, payout, settlement, or public-claim authority', () => {
    const projection = projectOmniModelLabEvidenceGraph(
      exampleOmniModelLabEvidenceGraph(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniModelLabEvidenceGraphProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      connected: true,
      createdAtDisplay: '20 minutes ago',
      edgeCount: 6,
      evalExecutionAllowed: false,
      modelTrainingLaunchAllowed: false,
      nodeCount: 7,
      paymentSpendAllowed: false,
      payoutMutationAllowed: false,
      providerCallAllowed: false,
      publicClaimUpgradeAllowed: false,
      rollbackReady: true,
      routingMutationAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      staleEvidenceCount: 0,
      updatedAtDisplay: '4 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_MODEL_LAB_GRAPH_READ_ONLY_AUTHORITY,
    )
    expect(projection.nodeKindCounts).toEqual([
      { count: 1, kind: 'adapter_validation' },
      { count: 1, kind: 'candidate' },
      { count: 1, kind: 'eval_rerun' },
      { count: 1, kind: 'model_artifact' },
      { count: 1, kind: 'promotion_gate' },
      { count: 1, kind: 'retained_failure' },
      { count: 1, kind: 'training_run' },
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniModelLabEvidenceGraphProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('requires same-loop nodes, edge membership, no duplicate refs, no cycles, connectivity, stale caveats, and rollback posture', () => {
    const base = exampleOmniModelLabEvidenceGraph()

    for (const badRecord of [
      graphRecord({ nodes: [] }),
      graphRecord({ edges: [] }),
      graphRecord({
        nodes: [
          { ...base.nodes[0]!, nodeRef: base.nodes[1]!.nodeRef },
          ...base.nodes.slice(1),
        ],
      }),
      graphRecord({
        edges: [
          { ...base.edges[0]!, edgeRef: base.edges[1]!.edgeRef },
          ...base.edges.slice(1),
        ],
      }),
      graphRecord({
        nodes: [
          { ...base.nodes[0]!, loopRefs: ['loop.public.other'] },
          ...base.nodes.slice(1),
        ],
      }),
      graphRecord({
        edges: [
          {
            ...base.edges[0]!,
            toNodeRef: 'candidate.public.missing',
          },
          ...base.edges.slice(1),
        ],
      }),
      graphRecord({
        edges: [
          ...base.edges,
          {
            caveatRefs: [],
            edgeRef: 'edge.public.gate_back_to_failure',
            evidenceRefs: ['evidence.public.bad_cycle'],
            fromNodeRef: 'promotion_gate.public.otect_adapter_review',
            kind: 'derived_from',
            toNodeRef: 'retained_failure.public.otect_revision_images',
          },
        ],
      }),
      graphRecord({
        edges: base.edges.slice(0, 4),
      }),
      graphRecord({
        staleEvidenceRefs: ['stale.public.eval_needs_rerun'],
        caveatRefs: [],
      }),
      graphRecord({
        nodes: [
          { ...base.nodes[0]!, staleEvidenceRefs: [], state: 'stale' },
          ...base.nodes.slice(1),
        ],
      }),
      graphRecord({
        rollback: {
          ...base.rollback,
          rollbackPosture: 'candidate',
        },
      }),
      graphRecord({
        rollback: {
          priorNodeRefs: ['artifact.public.missing'],
          rollbackPosture: 'ready',
          rollbackRefs: ['rollback.public.otect_adapter_restore'],
        },
      }),
    ]) {
      expect(() =>
        projectOmniModelLabEvidenceGraph(badRecord, 'operator', nowIso),
      ).toThrow(OmniModelLabEvidenceGraphUnsafe)
    }
  })

  test('keeps stale evidence caveats visible when valid', () => {
    const projection = projectOmniModelLabEvidenceGraph(
      graphRecord({
        caveatRefs: [
          'caveat.public.model_lab_graph_evidence_only',
          'caveat.public.eval_stale_until_rerun',
        ],
        nodes: [
          {
            ...exampleOmniModelLabEvidenceGraph().nodes[3]!,
            staleEvidenceRefs: ['stale.public.artifact_eval_outdated'],
            state: 'stale',
          },
          ...exampleOmniModelLabEvidenceGraph().nodes.slice(0, 3),
          ...exampleOmniModelLabEvidenceGraph().nodes.slice(4),
        ],
        staleEvidenceRefs: ['stale.public.graph_eval_refresh_needed'],
      }),
      'operator',
      nowIso,
    )

    expect(projection.staleEvidenceCount).toBe(2)
    expect(projection.caveatRefs).toContain(
      'caveat.public.eval_stale_until_rerun',
    )
  })

  test('redacts private node, edge, graph, loop, rollback, source, stale, and evidence refs publicly', () => {
    const base = exampleOmniModelLabEvidenceGraph()
    const projection = projectOmniModelLabEvidenceGraph(
      graphRecord({
        caveatRefs: [
          'caveat.public.model_lab_graph_evidence_only',
          'caveat.private.operator_note',
        ],
        edges: [
          {
            ...base.edges[0]!,
            edgeRef: 'edge.private.operator_edge',
            evidenceRefs: [
              'evidence.public.failure_candidate_trace',
              'evidence.private.operator_edge_trace',
            ],
            fromNodeRef: 'retained_failure.private.operator_failure',
          },
          ...base.edges.slice(1),
        ],
        graphRef: 'graph.private.operator_graph',
        loopRef: 'loop.private.operator_loop',
        nodes: [
          {
            ...base.nodes[0]!,
            evidenceRefs: [
              'evidence.public.retained_failure_summary',
              'source.private.operator_trace',
            ],
            loopRefs: ['loop.private.operator_loop'],
            nodeRef: 'retained_failure.private.operator_failure',
          },
          ...base.nodes.slice(1).map(node => ({
            ...node,
            loopRefs: ['loop.private.operator_loop'],
          })),
        ],
        rollback: {
          priorNodeRefs: [
            'artifact.public.otect_layout_adapter_v1',
          ],
          rollbackPosture: 'ready',
          rollbackRefs: [
            'rollback.public.otect_adapter_restore',
            'rollback.private.operator_restore',
          ],
        },
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.graphRef).toBe('graph.redacted.model_lab')
    expect(projection.loopRef).toBe('loop.redacted.model_lab')
    expect(projection.nodes[0]!.nodeRef).toBe('node.redacted.model_lab_graph')
    expect(projection.edges[0]!.edgeRef).toBe('edge.redacted.model_lab_graph')
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator')
    expect(omniModelLabEvidenceGraphProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects private prompts, source archives, provider payloads, datasets, model weights, secrets, payment material, raw timestamps, and mutable authority', () => {
    const base = exampleOmniModelLabEvidenceGraph()

    for (const badRecord of [
      graphRecord({ caveatRefs: ['raw_prompt.customer'] }),
      graphRecord({ staleEvidenceRefs: ['source_archive.raw'] }),
      graphRecord({ blockerRefs: ['provider_payload.raw'] }),
      graphRecord({ blockerRefs: ['dataset.private.customer'] }),
      graphRecord({ blockerRefs: ['weights.safetensors'] }),
      graphRecord({ caveatRefs: ['secret.model_lab_token'] }),
      graphRecord({ caveatRefs: ['payment_preimage.raw'] }),
      graphRecord({ caveatRefs: ['caveat.public.2026-06-06T23:00:00'] }),
      graphRecord({
        nodes: [
          {
            ...base.nodes[0]!,
            evidenceRefs: [],
          },
          ...base.nodes.slice(1),
        ],
      }),
      graphRecord({
        edges: [
          {
            ...base.edges[0]!,
            evidenceRefs: [],
          },
          ...base.edges.slice(1),
        ],
      }),
      graphRecord({
        authority: {
          ...OMNI_MODEL_LAB_GRAPH_READ_ONLY_AUTHORITY,
          noRuntimePromotion: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniModelLabEvidenceGraph(badRecord, 'operator', nowIso),
      ).toThrow(OmniModelLabEvidenceGraphUnsafe)
    }
  })
})
