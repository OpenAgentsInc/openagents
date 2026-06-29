import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY,
  OmniRetrievalGraphEdge,
  OmniRetrievalGraphNode,
  OmniRetrievalSourceHit,
  OmniRetrievalTraceProjection,
  OmniRetrievalTraceRecord,
  OmniRetrievalTraceUnsafe,
  exampleOmniRetrievalTrace,
  projectOmniRetrievalTrace,
} from './omni-retrieval-trace-context'

const nowIso = '2026-06-06T22:30:00.000Z'

const hit = (
  overrides: Partial<OmniRetrievalSourceHit> = {},
): OmniRetrievalSourceHit =>
  S.decodeUnknownSync(OmniRetrievalSourceHit)({
    caveatRefs: ['caveat.public.retrieval_context'],
    exclusionReasonKind: null,
    freshness: 'fresh',
    provenanceRefs: ['provenance.public.semantic_selector'],
    rank: 1,
    reasonRef: null,
    rightsRef: 'rights.public.openagents_repo',
    scoreBps: 9000,
    selectionState: 'selected',
    sourceBundleRef: 'bundle.public.otec_research_sources',
    sourceRef: 'source.public.openagents_transcript_230',
    spanRefs: ['span.public.transcript_230_intro'],
    ...overrides,
  })

const node = (
  overrides: Partial<OmniRetrievalGraphNode> = {},
): OmniRetrievalGraphNode =>
  S.decodeUnknownSync(OmniRetrievalGraphNode)({
    caveatRefs: ['caveat.public.graph_context'],
    humanConfirmationRefs: [],
    kind: 'source',
    nodeRef: 'node.public.source.transcript_230',
    sourceRefs: ['source.public.openagents_transcript_230'],
    spanRefs: [],
    ...overrides,
  })

const edge = (
  overrides: Partial<OmniRetrievalGraphEdge> = {},
): OmniRetrievalGraphEdge =>
  S.decodeUnknownSync(OmniRetrievalGraphEdge)({
    caveatRefs: ['caveat.public.graph_edge'],
    edgeRef: 'edge.public.claim_supports_source',
    fromNodeRef: 'node.public.claim.otec_power_compute',
    humanConfirmationRefs: [],
    kind: 'supports',
    sourceRefs: ['source.public.openagents_transcript_230'],
    spanRefs: ['span.public.transcript_230_intro'],
    toNodeRef: 'node.public.source.transcript_230',
    ...overrides,
  })

const trace = (
  overrides: Partial<OmniRetrievalTraceRecord> = {},
): OmniRetrievalTraceRecord =>
  S.decodeUnknownSync(OmniRetrievalTraceRecord)({
    ...exampleOmniRetrievalTrace(),
    ...overrides,
  })

describe('Omni retrieval trace context', () => {
  test('projects selected/excluded sources, graph context, and human-confirmed facts without mutation authority', () => {
    const projection = projectOmniRetrievalTrace(
      exampleOmniRetrievalTrace(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniRetrievalTraceProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      autonomousSourceFetchAllowed: false,
      excludedCount: 1,
      factPromotionMutationAllowed: false,
      generatedSummaryMutationAllowed: false,
      graphMutationAllowed: false,
      humanConfirmedFactCount: 1,
      missingContextCount: 1,
      publicClaimUpgradeAllowed: false,
      retrievedAtDisplay: '5 minutes ago',
      selectedCount: 2,
      selectorKind: 'hybrid_semantic_graph',
      staleSelectedCount: 0,
      workroomRef: 'redacted.workroom',
    })
    expect(projection.authority).toEqual(OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY)
    expect(projection.selectedSources.map(source => source.rank)).toEqual([1, 2])
    expect(projection.selectedSources[0]).toMatchObject({
      exclusionReasonLabel: null,
      freshnessLabel: 'Fresh',
      scoreBps: 9300,
      selectionState: 'selected',
    })
    expect(projection.excludedSources[0]).toMatchObject({
      exclusionReasonKind: 'stale',
      exclusionReasonLabel: 'Stale',
      freshnessLabel: 'Stale',
      selectionState: 'excluded',
    })
    expect(projection.confirmedFacts[0]).toMatchObject({
      factRef: 'fact.public.otec_power_compute_context',
      state: 'human_confirmed',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('keeps stale selected sources and missing context visible', () => {
    const projection = projectOmniRetrievalTrace(
      trace({
        missingContext: [
          ...exampleOmniRetrievalTrace().missingContext,
          {
            kind: 'stale_memory',
            labelRef: 'label.public.refresh_old_brief',
            reasonRef: 'reason.public.old_brief_is_stale',
            requiredForRef: 'claim.public.current_site_copy',
          },
        ],
        selectedSources: [
          ...exampleOmniRetrievalTrace().selectedSources,
          hit({
            freshness: 'stale',
            rank: 3,
            scoreBps: 7800,
            sourceRef: 'source.public.old_otec_brief',
            spanRefs: ['span.public.old_otec_brief'],
          }),
        ],
      }),
      'team',
      nowIso,
    )

    expect(projection.selectedCount).toBe(3)
    expect(projection.staleSelectedCount).toBe(1)
    expect(projection.missingContextCount).toBe(2)
    expect(projection.selectedSources[2]).toMatchObject({
      freshness: 'stale',
      freshnessLabel: 'Stale',
      rank: 3,
    })
    expect(projection.missingContext.map(item => item.kind)).toEqual([
      'needed_source',
      'stale_memory',
    ])
  })

  test('requires graph refs and human confirmation refs for confirmed facts', () => {
    const base = exampleOmniRetrievalTrace()

    for (const record of [
      trace({
        graphNodes: [
          node({
            humanConfirmationRefs: [],
            sourceRefs: [],
            spanRefs: [],
          }),
        ],
        graphEdges: [],
      }),
      trace({
        graphEdges: [
          edge({ toNodeRef: 'node.public.missing' }),
        ],
      }),
      trace({
        graphEdges: [
          edge({
            humanConfirmationRefs: [],
            sourceRefs: [],
            spanRefs: [],
          }),
        ],
      }),
      trace({
        confirmedFacts: [
          {
            caveatRefs: ['caveat.public.operator_confirmation'],
            factRef: 'fact.public.no_confirmation',
            humanConfirmationRefs: [],
            sourceRefs: ['source.public.openagents_transcript_230'],
            spanRefs: [],
            state: 'human_confirmed',
          },
        ],
      }),
      {
        ...base,
        graphNodes: [
          node({
            kind: 'fact',
            nodeRef: 'node.public.fact.unconnected',
          }),
        ],
        graphEdges: [],
      },
    ]) {
      expect(() =>
        projectOmniRetrievalTrace(record, 'operator', nowIso),
      ).toThrow(OmniRetrievalTraceUnsafe)
    }
  })

  test('redacts private source hits, graph refs, confirmations, summaries, and workroom refs publicly', () => {
    const projection = projectOmniRetrievalTrace(
      trace({
        confirmedFacts: [
          ...exampleOmniRetrievalTrace().confirmedFacts,
          {
            caveatRefs: ['caveat.private.operator_confirmation'],
            factRef: 'fact.private.operator_fact',
            humanConfirmationRefs: ['confirmation.private.operator_review'],
            sourceRefs: ['source.private.operator_source'],
            spanRefs: ['span.private.operator_span'],
            state: 'human_confirmed',
          },
        ],
        generatedSummaryRefs: [
          'summary.public.otec_retrieval_context',
          'summary.private.operator_notes',
        ],
        graphEdges: [
          ...exampleOmniRetrievalTrace().graphEdges,
          edge({
            edgeRef: 'edge.private.operator_edge',
            fromNodeRef: 'node.private.operator_node',
            sourceRefs: ['source.private.operator_source'],
            spanRefs: ['span.private.operator_span'],
            toNodeRef: 'node.public.source.transcript_230',
          }),
        ],
        graphNodes: [
          ...exampleOmniRetrievalTrace().graphNodes,
          node({
            nodeRef: 'node.private.operator_node',
            sourceRefs: ['source.private.operator_source'],
            spanRefs: ['span.private.operator_span'],
          }),
        ],
        queryIntentRef: 'intent.private.operator_query',
        selectedSources: [
          ...exampleOmniRetrievalTrace().selectedSources,
          hit({
            rightsRef: 'rights.private.operator_terms',
            sourceBundleRef: 'bundle.private.operator_bundle',
            sourceRef: 'source.private.operator_source',
            spanRefs: ['span.private.operator_span'],
          }),
        ],
        workroomRef: 'workroom.private.operator',
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.queryIntentRef).toBe('query_intent.redacted')
    expect(projection.workroomRef).toBe('redacted.workroom')
    expect(projection.generatedSummaryRefs).toEqual([
      'summary.public.otec_retrieval_context',
    ])
    expect(projection.selectedCount).toBe(2)
    expect(projection.graphNodes.map(item => item.nodeRef)).not.toContain(
      'node.private.operator_node',
    )
    expect(projection.confirmedFacts.map(item => item.factRef)).not.toContain(
      'fact.private.operator_fact',
    )
    expect(serialized).not.toMatch(
      /(bundle|caveat|confirmation|edge|fact|intent|node|rights|source|span|summary|workroom)\.private/,
    )
  })

  test('rejects invalid hit state, rank, score, unsafe refs, ad hoc keyword selectors, and false authority', () => {
    const base = exampleOmniRetrievalTrace()

    for (const record of [
      trace({
        selectedSources: [
          hit({
            exclusionReasonKind: 'privacy',
            reasonRef: 'reason.public.privacy',
          }),
        ],
      }),
      trace({
        excludedSources: [
          hit({
            exclusionReasonKind: null,
            reasonRef: null,
            selectionState: 'excluded',
          }),
        ],
      }),
      trace({
        selectedSources: [hit({ rank: 0 })],
      }),
      trace({
        selectedSources: [hit({ scoreBps: 10001 })],
      }),
      trace({
        selectedSources: [
          hit({ sourceRef: 'raw_transcript.operator_dump' }),
        ],
      }),
      trace({
        selectorModelRef: 'keyword_only_selector.private',
      }),
      trace({
        caveatRefs: ['caveat.public.2026-06-06T22:00:00Z'],
      }),
      {
        ...base,
        authority: {
          ...OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY,
          noGraphMutation: false,
        },
      },
    ]) {
      expect(() =>
        projectOmniRetrievalTrace(record, 'operator', nowIso),
      ).toThrow(OmniRetrievalTraceUnsafe)
    }
  })
})
