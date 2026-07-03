import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BUSINESS_CONTEXT_LIBRARY_READ_ONLY_AUTHORITY,
  BusinessContextDeliverableGroundingDecision,
  BusinessContextDeliverableRecord,
  BusinessContextLibraryProjection,
  BusinessContextLibraryRecord,
  BusinessContextLibraryUnsafe,
  evaluateBusinessContextDeliverableGrounding,
  exampleBusinessContextLibrary,
  projectBusinessContextLibrary,
} from './business-context-library'

const nowIso = '2026-07-02T12:10:00.000Z'

const library = (
  overrides: Partial<BusinessContextLibraryRecord> = {},
): BusinessContextLibraryRecord =>
  S.decodeUnknownSync(BusinessContextLibraryRecord)({
    ...exampleBusinessContextLibrary(),
    ...overrides,
  })

const deliverable = (
  overrides: Partial<BusinessContextDeliverableRecord> = {},
): BusinessContextDeliverableRecord =>
  S.decodeUnknownSync(BusinessContextDeliverableRecord)({
    assertionRefs: [
      {
        assertionRef: 'assertion.customer.deliverable_scope',
        citationRefs: ['citation.customer.scope_from_template'],
      },
      {
        assertionRef: 'assertion.customer.vertical_fit',
        citationRefs: ['citation.customer.vertical_from_intake'],
      },
    ],
    caveatRefs: ['caveat.customer.draft_until_reviewed'],
    citations: [
      {
        citationRef: 'citation.customer.scope_from_template',
        factRefs: [],
        provenanceRefs: ['provenance.customer.drive_read_receipt'],
        sourceRefs: ['source.customer.formation_template'],
        spanRefs: ['span.customer.formation_template_scope'],
      },
      {
        citationRef: 'citation.customer.vertical_from_intake',
        factRefs: ['fact.customer.vertical.legal_ops'],
        provenanceRefs: ['provenance.customer.intake_spec_confirmed'],
        sourceRefs: [],
        spanRefs: [],
      },
    ],
    deliverableRef: 'deliverable.customer.formation_workflow_outline',
    generatedArtifactRefs: ['artifact.customer.formation_workflow_outline'],
    retrievalTraceRefs: ['retrieval_trace.customer.formation_workflow_outline'],
    reviewRefs: ['review.customer.operator_required'],
    workflowRef: 'workflow.customer.formation_document_pipeline',
    workspaceRef: 'workspace.customer.legal_ops_demo',
    ...overrides,
  })

describe('Business context library', () => {
  test('projects corpus sources plus structured intake facts as read-only workspace context', () => {
    const projection = projectBusinessContextLibrary(
      exampleBusinessContextLibrary(),
      'customer',
      nowIso,
    )

    expect(S.decodeUnknownSync(BusinessContextLibraryProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      corpusSourceCount: 1,
      corpusSpanCount: 1,
      deliverablePublishRequiresCitations: true,
      externalSendAllowed: false,
      factMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      readiness: 'ready',
      retrievalRequiredForFulfillment: true,
      spendAuthorityAllowed: false,
      structuredFactCount: 1,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      BUSINESS_CONTEXT_LIBRARY_READ_ONLY_AUTHORITY,
    )
    expect(projection.groundingSourceRefs).toEqual([
      'source.customer.formation_template',
    ])
    expect(projection.groundingSpanRefs).toEqual([
      'span.customer.formation_template_scope',
    ])
    expect(projection.groundingFactRefs).toEqual([
      'fact.customer.vertical.legal_ops',
    ])
    expect(projection.structuredFacts[0]).toMatchObject({
      factKind: 'vertical',
      factRef: 'fact.customer.vertical.legal_ops',
      sourceKind: 'structured_intake_spec',
      state: 'human_confirmed',
    })
  })

  test('omits customer corpus, facts, titles, workrooms, and workspace refs publicly', () => {
    const projection = projectBusinessContextLibrary(
      exampleBusinessContextLibrary(),
      'public',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(projection.id).toBe('context_library.redacted')
    expect(projection.titleRef).toBe('title.redacted')
    expect(projection.workspaceRef).toBe('redacted.workspace')
    expect(projection.workroomRefs).toEqual([])
    expect(projection.sourceBundleRefs).toEqual([])
    expect(projection.groundingSourceRefs).toEqual([])
    expect(projection.groundingSpanRefs).toEqual([])
    expect(projection.groundingFactRefs).toEqual([])
    expect(projection.structuredFacts).toEqual([])
    expect(serialized).not.toMatch(
      /(bundle|caveat|context|fact|intake|policy|provenance|rights|source|span|summary|title|workspace|workroom)\.customer/,
    )
  })

  test('allows a deliverable only when every assertion cites known corpus or intake grounding', () => {
    const decision = evaluateBusinessContextDeliverableGrounding({
      deliverable: deliverable(),
      library: exampleBusinessContextLibrary(),
      nowIso,
    })

    expect(
      S.decodeUnknownSync(BusinessContextDeliverableGroundingDecision)(
        decision,
      ),
    ).toEqual(decision)
    expect(decision).toMatchObject({
      groundedAssertionCount: 2,
      publishAllowed: true,
      reasonRef: 'reason.business_context_deliverable.grounded',
      ungroundedAssertionRefs: [],
      ungroundedCitationRefs: [],
    })
    expect(decision.blockerRefs).toEqual([])
    expect(decision.groundedSourceRefs).toEqual([
      'source.customer.formation_template',
    ])
    expect(decision.groundedSpanRefs).toEqual([
      'span.customer.formation_template_scope',
    ])
    expect(decision.groundedFactRefs).toEqual([
      'fact.customer.vertical.legal_ops',
    ])
  })

  test('blocks ungrounded citations, missing retrieval traces, and unknown assertion citations', () => {
    const decision = evaluateBusinessContextDeliverableGrounding({
      deliverable: deliverable({
        assertionRefs: [
          {
            assertionRef: 'assertion.customer.bad_fact',
            citationRefs: ['citation.customer.unknown_fact'],
          },
          {
            assertionRef: 'assertion.customer.missing_citation',
            citationRefs: ['citation.customer.not_present'],
          },
        ],
        citations: [
          {
            citationRef: 'citation.customer.unknown_fact',
            factRefs: ['fact.customer.not_in_library'],
            provenanceRefs: ['provenance.customer.intake_spec_confirmed'],
            sourceRefs: [],
            spanRefs: [],
          },
        ],
        retrievalTraceRefs: [],
      }),
      library: exampleBusinessContextLibrary(),
      nowIso,
    })

    expect(decision.publishAllowed).toBe(false)
    expect(decision.blockerRefs).toContain(
      'blocker.business_context_deliverable.retrieval_trace_missing',
    )
    expect(decision.blockerRefs).toContain(
      'blocker.business_context_deliverable.assertion_citation_missing',
    )
    expect(decision.blockerRefs).toContain(
      'blocker.business_context_deliverable.ungrounded_citation',
    )
    expect(decision.ungroundedAssertionRefs).toEqual([
      'assertion.customer.bad_fact',
      'assertion.customer.missing_citation',
    ])
    expect(decision.ungroundedCitationRefs).toEqual([
      'citation.customer.not_present',
      'citation.customer.unknown_fact',
    ])
  })

  test('blocks deliverables when corpus or structured intake facts are missing', () => {
    const noCorpusDecision = evaluateBusinessContextDeliverableGrounding({
      deliverable: deliverable(),
      library: library({ sourceBundles: [] }),
      nowIso,
    })
    const noFactsDecision = evaluateBusinessContextDeliverableGrounding({
      deliverable: deliverable(),
      library: library({ structuredFacts: [] }),
      nowIso,
    })

    expect(noCorpusDecision.publishAllowed).toBe(false)
    expect(noCorpusDecision.blockerRefs).toContain(
      'blocker.business_context_library.corpus_missing',
    )
    expect(noFactsDecision.publishAllowed).toBe(false)
    expect(noFactsDecision.blockerRefs).toContain(
      'blocker.business_context_library.intake_facts_missing',
    )
  })

  test('candidate or rejected intake facts do not ground published deliverables', () => {
    const base = exampleBusinessContextLibrary()
    const intakeFact = base.structuredFacts[0]

    if (intakeFact === undefined) {
      throw new Error('Example context library fixture requires one intake fact.')
    }

    const decision = evaluateBusinessContextDeliverableGrounding({
      deliverable: deliverable({
        assertionRefs: [
          {
            assertionRef: 'assertion.customer.vertical_fit',
            citationRefs: ['citation.customer.vertical_from_intake'],
          },
        ],
        citations: [
          {
            citationRef: 'citation.customer.vertical_from_intake',
            factRefs: ['fact.customer.vertical.legal_ops'],
            provenanceRefs: ['provenance.customer.intake_spec_confirmed'],
            sourceRefs: [],
            spanRefs: [],
          },
        ],
      }),
      library: library({
        structuredFacts: [
          {
            ...intakeFact,
            state: 'candidate',
          },
        ],
      }),
      nowIso,
    })

    expect(decision.publishAllowed).toBe(false)
    expect(decision.blockerRefs).toContain(
      'blocker.business_context_library.intake_facts_missing',
    )
    expect(decision.blockerRefs).toContain(
      'blocker.business_context_deliverable.ungrounded_citation',
    )
  })

  test('rejects unsafe raw refs and source-less citations before projection or publish checks', () => {
    expect(() =>
      projectBusinessContextLibrary(
        library({
          retrievalTraceRefs: ['raw_transcript.customer_dump'],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(BusinessContextLibraryUnsafe)

    expect(() =>
      evaluateBusinessContextDeliverableGrounding({
        deliverable: deliverable({
          citations: [
            {
              citationRef: 'citation.customer.empty',
              factRefs: [],
              provenanceRefs: ['provenance.customer.operator_review'],
              sourceRefs: [],
              spanRefs: [],
            },
          ],
        }),
        library: exampleBusinessContextLibrary(),
        nowIso,
      }),
    ).toThrow(BusinessContextLibraryUnsafe)
  })
})
