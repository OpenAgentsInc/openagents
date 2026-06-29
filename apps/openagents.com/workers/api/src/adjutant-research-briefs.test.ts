import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ExaEnrichmentSourceCard } from './adjutant-enrichment-ledger'
import {
  type AdjutantResearchBriefRuntime,
  makeAdjutantResearchBriefService,
} from './adjutant-research-briefs'

type StoredBrief = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  claims_needing_review_json: string
  created_at: string
  created_by_user_id: string | null
  enrichment_run_id: string | null
  grounded_facts_json: string
  id: string
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  source_cards_json: string
  status: string
  suggested_sections_json: string
  summary: string
  unknowns_json: string
  updated_at: string
}>

type BriefMemoryState = Readonly<{
  briefs: Array<StoredBrief>
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const textValue = (value: unknown): string => String(value)
const nullableText = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)

const runStatement = (
  state: BriefMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): void => {
  if (query.includes('INSERT INTO adjutant_research_briefs')) {
    state.briefs.push({
      id: textValue(values[0]),
      assignment_id: textValue(values[1]),
      enrichment_run_id: nullableText(values[2]),
      status: textValue(values[3]),
      summary: textValue(values[4]),
      grounded_facts_json: textValue(values[5]),
      suggested_sections_json: textValue(values[6]),
      unknowns_json: textValue(values[7]),
      claims_needing_review_json: textValue(values[8]),
      source_cards_json: textValue(values[9]),
      created_by_user_id: nullableText(values[10]),
      approved_at: nullableText(values[11]),
      rejected_at: nullableText(values[12]),
      created_at: textValue(values[13]),
      updated_at: textValue(values[14]),
      reviewed_by_user_id: null,
      review_reason: null,
      archived_at: null,
    })

    return
  }

  if (query.includes('UPDATE adjutant_research_briefs')) {
    const briefId = textValue(values[6])
    const index = state.briefs.findIndex(brief => brief.id === briefId)
    const current = state.briefs[index]

    if (current !== undefined) {
      state.briefs[index] = {
        ...current,
        status: textValue(values[0]),
        reviewed_by_user_id: nullableText(values[1]),
        review_reason: nullableText(values[2]),
        approved_at: nullableText(values[3]),
        rejected_at: nullableText(values[4]),
        updated_at: textValue(values[5]),
      }
    }
  }
}

const firstRow = (
  state: BriefMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): unknown | null => {
  if (!query.includes('FROM adjutant_research_briefs')) {
    return null
  }

  const requestedStatus = nullableText(values[1])

  return (
    state.briefs.find(
      brief =>
        brief.assignment_id === values[0] &&
        brief.archived_at === null &&
        (requestedStatus === null || brief.status === requestedStatus),
    ) ?? null
  )
}

const makeMemoryD1 = (): D1Database & BriefMemoryState => {
  const state: BriefMemoryState = { briefs: [] }

  const prepare = (query: string): D1PreparedStatement => {
    let values: ReadonlyArray<unknown> = []

    function raw<T = unknown[]>(options: {
      columnNames: true
    }): Promise<[Array<string>, ...Array<T>]>
    function raw<T = unknown[]>(options?: {
      columnNames?: false
    }): Promise<Array<T>>
    function raw<T = unknown[]>(options?: {
      columnNames?: boolean
    }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
      return options?.columnNames === true
        ? Promise.resolve([[]])
        : Promise.resolve([])
    }

    const statement: D1PreparedStatement = {
      all: <T = Record<string, unknown>>() => Promise.resolve(makeResult<T>()),
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues

        return statement
      },
      first: <T = Record<string, unknown>>() => {
        const row = firstRow(state, query, values)

        return Promise.resolve(row === null ? null : (row as T))
      },
      raw,
      run: <T = Record<string, unknown>>() => {
        runStatement(state, query, values)

        return Promise.resolve(makeResult<T>())
      },
    }

    return statement
  }

  const db: D1Database & BriefMemoryState = {
    ...state,
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare,
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare,
    }),
  }

  return db
}

const runtime = (): AdjutantResearchBriefRuntime => {
  let index = 0

  return {
    makeBriefId: () => `adjutant_research_brief_${index++}`,
    nowIso: () => '2026-06-05T00:00:00.000Z',
  }
}

const approvedSourceCard = (): ExaEnrichmentSourceCard => ({
  id: 'exa_enrichment_source_otec',
  runId: 'exa_enrichment_run_otec',
  queryId: 'exa_enrichment_query_otec',
  assignmentId: 'adjutant_assignment_otec',
  softwareOrderId: 'software_order_otec',
  siteId: 'site_project_otec',
  sourceCategory: 'topic_web',
  reviewStatus: 'approved',
  title: 'OTEC overview',
  url: 'https://example.com/otec',
  domain: 'example.com',
  publishedDate: null,
  highlightText: 'Ocean thermal energy conversion uses ocean gradients.',
  selectedTextHash: 'a'.repeat(64),
  exaRequestId: 'req_otec',
  searchType: 'auto',
  publicSafe: true,
  rejectedReason: null,
  approvedAt: '2026-06-05T00:00:00.000Z',
  rejectedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  updatedAt: '2026-06-05T00:00:00.000Z',
})

describe('AdjutantResearchBriefService', () => {
  test('creates, reviews, and reads latest approved research briefs', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantResearchBriefService(db, runtime())
    const brief = await Effect.runPromise(
      service.createBrief({
        assignmentId: 'adjutant_assignment_otec',
        createdByUserId: 'github:operator',
        customerRequest:
          'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
        enrichmentRunId: 'exa_enrichment_run_otec',
        sourceCards: [approvedSourceCard()],
      }),
    )

    expect(brief.status).toBe('needs_review')
    expect(brief.groundedFacts).toEqual([
      'Ocean thermal energy conversion uses ocean gradients.',
    ])
    expect(brief.sourceCards[0]).toMatchObject({
      id: 'exa_enrichment_source_otec',
      url: 'https://example.com/otec',
    })

    await Effect.runPromise(
      service.reviewBrief({
        briefId: brief.id,
        reviewReason: 'Approved for OTEC smoke.',
        reviewedByUserId: 'github:operator',
        status: 'approved',
      }),
    )

    const latest = await Effect.runPromise(
      service.latestApprovedBriefForAssignment('adjutant_assignment_otec'),
    )

    expect(latest?.id).toBe(brief.id)
    expect(latest?.status).toBe('approved')
    expect(latest?.approvedAt).toBe('2026-06-05T00:00:00.000Z')
  })

  test('bounds long approved-source highlights when deriving grounded facts', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantResearchBriefService(db, runtime())
    const sourceCard = {
      ...approvedSourceCard(),
      highlightText: 'ChefGroep public source context. '.repeat(50),
    }
    const brief = await Effect.runPromise(
      service.createBrief({
        assignmentId: 'adjutant_assignment_chefgroep',
        customerRequest: 'Build the ChefGroep public Site.',
        enrichmentRunId: 'exa_enrichment_run_chefgroep',
        sourceCards: [sourceCard],
      }),
    )

    expect(brief.groundedFacts).toHaveLength(1)
    expect(brief.groundedFacts[0]).toHaveLength(280)
    expect(brief.sourceCards[0]).toMatchObject({
      id: sourceCard.id,
      url: sourceCard.url,
    })
  })

  test('rejects secret-shaped brief material', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantResearchBriefService(db, runtime())

    await expect(
      Effect.runPromise(
        service.createBrief({
          assignmentId: 'adjutant_assignment_otec',
          customerRequest: 'Bearer sk-secret123456789',
          sourceCards: [],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantResearchBriefUnsafePayload',
    })
  })
})
