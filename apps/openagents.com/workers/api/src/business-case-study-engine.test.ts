import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  BusinessCaseStudyEndpoint,
  BusinessCaseStudyInvariantError,
  assertPublishableBusinessCaseStudy,
  buildBusinessCaseStudy,
  firstBusinessCaseStudy,
  makeInMemoryBusinessCaseStudyStore,
  projectBusinessCaseStudies,
  publicBusinessCaseStudyProjection,
  type BusinessCaseStudyInput,
} from './business-case-study-engine'
import { makeBusinessCaseStudyRoutes } from './business-case-study-engine-routes'

const baseInput: BusinessCaseStudyInput = {
  caseStudyRef: 'case_study.business.software.quick_win.002',
  engagementRef: 'engagement.business.quick_win.software.002',
  status: 'published',
  verticalDescriptor: 'software',
  title: 'Software quick-win delivery with receipts',
  summary:
    'A bounded software quick win is published with opaque refs, proof receipts, cycle time, and intake attribution.',
  startedAt: '2026-07-02T10:00:00.000Z',
  completedAt: '2026-07-03T16:00:00.000Z',
  publishedAt: '2026-07-03T18:00:00.000Z',
  acceptedOutcomeRef: 'accepted_outcome.business.quick_win.software.002',
  publicProofBundleRef: 'proof_bundle.business.quick_win.software.002',
  receiptRefs: ['receipt.business.quick_win.software.002'],
  metricDefinitionsRef: 'docs/fable/ROADMAP_BIZ.md#BF-7.2',
  metrics: [
    {
      metricRef: 'metric.business.cycle_time_hours.002',
      label: 'Cycle time',
      value: 30,
      unit: 'hours',
      evidenceRef: 'receipt.business.quick_win.software.002',
    },
  ],
  attribution: {
    sourceKind: 'content',
    sourceRef: 'source.public.business.case_study.software.002',
    captureParam: 'caseStudyRef',
    intakeAttributionRef: 'attribution.business.case_study.software.002',
  },
  privacyReview: {
    reviewed: true,
    reviewedAt: '2026-07-03T17:00:00.000Z',
    reviewerRef: 'privacy.review.operator.business_case_studies',
    decisionRef: 'privacy.decision.business.case_study.software.002',
  },
  sourceRefs: ['docs/fable/ROADMAP_BIZ.md#BF-7.4'],
}

describe('business case-study engine', () => {
  it('builds a publishable public-safe case study with cycle-time evidence', () => {
    const caseStudy = buildBusinessCaseStudy(baseInput)

    expect(caseStudy.schema).toBe('openagents.business.case_study.v1')
    expect(caseStudy.cycleTimeHours).toBe(30)
    expect(caseStudy.attribution).toMatchObject({
      captureParam: 'caseStudyRef',
      intakeAttributionRef: 'attribution.business.case_study.software.002',
    })
    expect(() => assertPublishableBusinessCaseStudy(caseStudy)).not.toThrow()
  })

  it('rejects published case studies without privacy review or metrics', () => {
    expect(() =>
      buildBusinessCaseStudy({
        ...baseInput,
        privacyReview: { ...baseInput.privacyReview, reviewed: false },
      }),
    ).toThrow(/privacyReview.reviewed=true/)

    expect(() =>
      buildBusinessCaseStudy({
        ...baseInput,
        metrics: [],
      }),
    ).toThrow(/at least one metric/)
  })

  it('rejects client-identifying text and raw payment material', () => {
    expect(() =>
      buildBusinessCaseStudy({
        ...baseInput,
        title: 'Acme customer quick win',
      }),
    ).toThrow(BusinessCaseStudyInvariantError)

    expect(() =>
      buildBusinessCaseStudy({
        ...baseInput,
        receiptRefs: ['stripe.invoice.in_123'],
      }),
    ).toThrow(BusinessCaseStudyInvariantError)
  })

  it('requires completedAt after startedAt', () => {
    expect(() =>
      buildBusinessCaseStudy({
        ...baseInput,
        completedAt: '2026-07-02T09:00:00.000Z',
      }),
    ).toThrow(/completedAt must be after startedAt/)
  })

  it('projects public fields without privacy review internals', () => {
    const projection = publicBusinessCaseStudyProjection(
      buildBusinessCaseStudy(baseInput),
    )

    expect(projection).toMatchObject({
      caseStudyRef: 'case_study.business.software.quick_win.002',
      verticalDescriptor: 'software',
      cycleTimeHours: 30,
      privacyDecisionRef: 'privacy.decision.business.case_study.software.002',
    })
    expect(projection).not.toHaveProperty('privacyReview')
  })

  it('lists only published case studies with acquisition attribution contract', () => {
    const draft = buildBusinessCaseStudy({
      ...baseInput,
      caseStudyRef: 'case_study.business.software.quick_win.draft',
      status: 'draft',
    })
    const projection = projectBusinessCaseStudies(
      [firstBusinessCaseStudy, draft],
      { generatedAt: '2026-07-03T19:00:00.000Z' },
    )

    expect(projection.totals.caseStudyCount).toBe(1)
    expect(projection.caseStudies).toMatchObject([
      { caseStudyRef: 'case_study.business.legal.quick_win.001' },
    ])
    expect(projection.attributionContract).toMatchObject({
      captureParam: 'caseStudyRef',
      requiredIntakeField: 'source_ref',
    })
  })
})

describe('business case-study routes', () => {
  it('lists published case studies with public-safe rows', async () => {
    const routes = makeBusinessCaseStudyRoutes({
      makeCaseStudyStore: () =>
        makeInMemoryBusinessCaseStudyStore([firstBusinessCaseStudy]),
    })

    const responseEffect = routes.routeBusinessCaseStudyRequest(
      new Request(
        `https://openagents.com${BusinessCaseStudyEndpoint}?view=published-case-studies`,
      ),
      {},
    )
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    const json = (await response.json()) as {
      caseStudies: ReadonlyArray<Record<string, unknown>>
      attributionContract: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(json.caseStudies).toHaveLength(1)
    expect(JSON.stringify(json)).not.toContain('@')
    expect(JSON.stringify(json)).not.toContain('stripe')
    expect(json.attributionContract.captureParam).toBe('caseStudyRef')
  })

  it('dereferences one published case study', async () => {
    const routes = makeBusinessCaseStudyRoutes({
      makeCaseStudyStore: () =>
        makeInMemoryBusinessCaseStudyStore([firstBusinessCaseStudy]),
    })

    const responseEffect = routes.routeBusinessCaseStudyRequest(
      new Request(
        `https://openagents.com${BusinessCaseStudyEndpoint}/${encodeURIComponent(firstBusinessCaseStudy.caseStudyRef)}`,
      ),
      {},
    )
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    const json = (await response.json()) as {
      caseStudy: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(json.caseStudy).toMatchObject({
      caseStudyRef: firstBusinessCaseStudy.caseStudyRef,
      status: 'published',
      verticalDescriptor: 'legal',
    })
  })
})
