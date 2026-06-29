import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY,
  OmniKnowledgeExtractedSpanRecord,
  OmniKnowledgeSourceBundleProjection,
  OmniKnowledgeSourceBundleRecord,
  OmniKnowledgeSourceBundleUnsafe,
  OmniKnowledgeSourceRecord,
  exampleOmniKnowledgeSourceBundle,
  projectOmniKnowledgeSourceBundle,
} from './omni-knowledge-source-bundles'

const nowIso = '2026-06-06T22:30:00.000Z'

const sourceRecord = (
  overrides: Partial<OmniKnowledgeSourceRecord> = {},
): OmniKnowledgeSourceRecord =>
  S.decodeUnknownSync(OmniKnowledgeSourceRecord)({
    caveatRefs: ['caveat.public.public_web_source'],
    dataClassification: 'public',
    digestAlgorithm: 'sha256',
    digestRef: 'digest.public.demo_source',
    locatorRef: 'locator.public.demo_source',
    provenanceRefs: ['provenance.public.demo_import'],
    redactionPolicyRefs: ['policy.public.redacted_archive_only'],
    rightsRefs: ['rights.public.demo_source'],
    rightsState: 'public',
    sourceKind: 'file',
    sourceRef: 'source.public.demo_source',
    titleRef: 'title.public.demo_source',
    trustTier: 'reviewed',
    ...overrides,
  })

const spanRecord = (
  overrides: Partial<OmniKnowledgeExtractedSpanRecord> = {},
): OmniKnowledgeExtractedSpanRecord =>
  S.decodeUnknownSync(OmniKnowledgeExtractedSpanRecord)({
    byteEnd: null,
    byteStart: null,
    caveatRefs: ['caveat.public.span_context'],
    codeSymbolRef: null,
    columnRefs: [],
    contentDigestRef: 'digest.public.span_demo',
    dataClassification: 'public',
    excerptRef: 'excerpt.public.span_demo',
    factCandidateRefs: ['fact.public.demo'],
    id: 'span.public.demo',
    lineEnd: 12,
    lineStart: 10,
    pageNumber: null,
    provenanceRefs: ['provenance.public.demo_import'],
    redactionPolicyRefs: ['policy.public.redacted_archive_only'],
    rightsRefs: ['rights.public.demo_source'],
    rowEnd: null,
    rowStart: null,
    selectorRef: 'selector.public.span_demo',
    sourceRef: 'source.public.demo_source',
    spanKind: 'file_range',
    timeEndMs: null,
    timeStartMs: null,
    trustTier: 'reviewed',
    ...overrides,
  })

const bundleRecord = (
  overrides: Partial<OmniKnowledgeSourceBundleRecord> = {},
): OmniKnowledgeSourceBundleRecord =>
  S.decodeUnknownSync(OmniKnowledgeSourceBundleRecord)({
    ...exampleOmniKnowledgeSourceBundle(),
    ...overrides,
  })

describe('Omni knowledge source bundles', () => {
  test('projects source-backed records and extracted spans without mutation authority', () => {
    const bundle = bundleRecord({
      sources: [
        ...exampleOmniKnowledgeSourceBundle().sources,
        sourceRecord({
          digestRef: 'digest.public.table_source',
          locatorRef: 'locator.public.metrics_table',
          sourceKind: 'table',
          sourceRef: 'source.public.metrics_table',
          titleRef: 'title.public.metrics_table',
        }),
        sourceRecord({
          digestRef: 'digest.public.page_source',
          locatorRef: 'locator.public.pdf_page_source',
          sourceKind: 'file',
          sourceRef: 'source.public.pdf_page_source',
          titleRef: 'title.public.pdf_page_source',
        }),
      ],
      spans: [
        ...exampleOmniKnowledgeSourceBundle().spans,
        spanRecord({
          contentDigestRef: 'digest.public.row_span',
          excerptRef: 'excerpt.public.row_span',
          id: 'span.public.row_span',
          lineEnd: null,
          lineStart: null,
          rowEnd: 5,
          rowStart: 3,
          sourceRef: 'source.public.metrics_table',
          spanKind: 'row',
        }),
        spanRecord({
          columnRefs: ['column.public.revenue', 'column.public.power'],
          contentDigestRef: 'digest.public.table_cell_span',
          excerptRef: 'excerpt.public.table_cell_span',
          id: 'span.public.table_cell_span',
          lineEnd: null,
          lineStart: null,
          rowEnd: 7,
          rowStart: 7,
          sourceRef: 'source.public.metrics_table',
          spanKind: 'table_cell',
        }),
        spanRecord({
          contentDigestRef: 'digest.public.page_span',
          excerptRef: 'excerpt.public.page_span',
          id: 'span.public.page_span',
          lineEnd: null,
          lineStart: null,
          pageNumber: 2,
          sourceRef: 'source.public.pdf_page_source',
          spanKind: 'page',
        }),
      ],
    })
    const projection = projectOmniKnowledgeSourceBundle(
      bundle,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniKnowledgeSourceBundleProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      connectorMutationAllowed: false,
      generatedSummaryMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      rawSourceArchiveCopyAllowed: false,
      rightsMutationAllowed: false,
      sourceCount: 4,
      spanCount: 5,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY,
    )
    expect(projection.sources.map(source => source.sourceKind)).toEqual([
      'transcript',
      'repo_ref',
      'table',
      'file',
    ])
    expect(projection.spans.map(span => span.locatorLabel)).toEqual([
      'time 0-120000ms',
      'lines 42-88',
      'rows 3-5',
      'rows 7-7',
      'page 2',
    ])
    expect(projection.generatedSummaryRefs).toEqual([
      'summary.public.otec_research_brief',
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('validates page, row, transcript, code, and file ranges by span kind', () => {
    const base = exampleOmniKnowledgeSourceBundle()

    for (const span of [
      spanRecord({
        lineEnd: null,
        lineStart: null,
        pageNumber: null,
        spanKind: 'page',
      }),
      spanRecord({
        lineEnd: null,
        lineStart: null,
        rowEnd: 2,
        rowStart: 3,
        spanKind: 'row',
      }),
      spanRecord({
        lineEnd: null,
        lineStart: null,
        spanKind: 'transcript',
        timeEndMs: null,
        timeStartMs: 0,
      }),
      spanRecord({
        lineEnd: 1,
        lineStart: 3,
        spanKind: 'code',
      }),
      spanRecord({
        byteEnd: null,
        byteStart: null,
        lineEnd: null,
        lineStart: null,
        spanKind: 'file_range',
      }),
      spanRecord({
        columnRefs: [],
        lineEnd: null,
        lineStart: null,
        rowEnd: 1,
        rowStart: 1,
        spanKind: 'table_cell',
      }),
    ]) {
      expect(() =>
        projectOmniKnowledgeSourceBundle(
          {
            ...base,
            sources: [sourceRecord()],
            spans: [span],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OmniKnowledgeSourceBundleUnsafe)
    }
  })

  test('requires provenance, digest, rights, active rights state, and read-only authority', () => {
    const base = exampleOmniKnowledgeSourceBundle()

    for (const record of [
      bundleRecord({ provenanceRefs: [] }),
      bundleRecord({ rightsRefs: [] }),
      bundleRecord({
        authority: {
          ...OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY,
          noConnectorMutation: false,
        },
      }),
      bundleRecord({
        sources: [sourceRecord({ provenanceRefs: [] })],
        spans: [],
      }),
      bundleRecord({
        sources: [sourceRecord({ rightsRefs: [] })],
        spans: [],
      }),
      bundleRecord({
        sources: [sourceRecord({ rightsState: 'revoked' })],
        spans: [],
      }),
      bundleRecord({
        sources: [sourceRecord({ digestRef: 'raw_source_archive.operator' })],
        spans: [],
      }),
      bundleRecord({
        sources: [sourceRecord()],
        spans: [spanRecord({ provenanceRefs: [] })],
      }),
      bundleRecord({
        sources: [sourceRecord()],
        spans: [spanRecord({ rightsRefs: [] })],
      }),
      {
        ...base,
        sources: [sourceRecord()],
        spans: [spanRecord({ sourceRef: 'source.public.missing' })],
      },
    ]) {
      expect(() =>
        projectOmniKnowledgeSourceBundle(record, 'operator', nowIso),
      ).toThrow(OmniKnowledgeSourceBundleUnsafe)
    }
  })

  test('redacts private source, span, rights, digest, locator, excerpt, summary, and workroom refs publicly', () => {
    const projection = projectOmniKnowledgeSourceBundle(
      bundleRecord({
        bundleRef: 'bundle.private.operator_sources',
        generatedSummaryRefs: [
          'summary.public.otec_research_brief',
          'summary.private.operator_notes',
        ],
        sources: [
          sourceRecord(),
          sourceRecord({
            digestRef: 'digest.private.operator_source',
            locatorRef: 'locator.private.operator_source',
            rightsRefs: ['rights.private.operator_terms'],
            sourceRef: 'source.private.operator_source',
            titleRef: 'title.private.operator_source',
          }),
        ],
        spans: [
          spanRecord(),
          spanRecord({
            contentDigestRef: 'digest.private.operator_span',
            excerptRef: 'excerpt.private.operator_span',
            id: 'span.private.operator_span',
            sourceRef: 'source.private.operator_source',
          }),
        ],
        workroomRefs: [
          'workroom.public.otec_research',
          'workroom.private.operator_research',
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.bundleRef).toBe('bundle.redacted')
    expect(projection.generatedSummaryRefs).toEqual([
      'summary.public.otec_research_brief',
    ])
    expect(projection.sources.map(source => source.sourceRef)).toEqual([
      'source.public.demo_source',
    ])
    expect(projection.spans.map(span => span.id)).toEqual([
      'span.public.demo',
    ])
    expect(projection.workroomRefs).toEqual([])
    expect(serialized).not.toMatch(
      /(bundle|digest|excerpt|locator|rights|source|span|summary|workroom)\.private/,
    )
  })

  test('keeps generated summaries separate from source records and rejects unsafe raw/source material', () => {
    const base = exampleOmniKnowledgeSourceBundle()

    for (const record of [
      {
        ...base,
        sources: [
          sourceRecord({ sourceRef: 'source.public.generated_summary_notes' }),
        ],
        spans: [],
      },
      {
        ...base,
        sources: [
          sourceRecord({ locatorRef: 'locator.public.generated_summary_notes' }),
        ],
        spans: [],
      },
      {
        ...base,
        generatedSummaryRefs: ['summary_text.raw_generated_claim'],
      },
      {
        ...base,
        sources: [sourceRecord({ sourceRef: 'raw_transcript.operator_dump' })],
        spans: [],
      },
      {
        ...base,
        sources: [sourceRecord()],
        spans: [spanRecord({ excerptRef: 'raw_text.customer_quote' })],
      },
      {
        ...base,
        sources: [sourceRecord({ locatorRef: 'github.com/team/private' })],
        spans: [],
      },
      {
        ...base,
        caveatRefs: ['caveat.public.2026-06-06T22:00:00Z'],
      },
    ]) {
      expect(() =>
        projectOmniKnowledgeSourceBundle(record, 'operator', nowIso),
      ).toThrow(OmniKnowledgeSourceBundleUnsafe)
    }
  })
})
