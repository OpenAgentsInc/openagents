import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AdjutantEnrichmentLedgerRuntime,
  makeAdjutantEnrichmentLedger,
  publicSafeExaSourceCards,
} from './adjutant-enrichment-ledger'

type StoredRun = Readonly<{
  approved_source_count: number
  archived_at: string | null
  assignment_id: string
  cache_hit_count: number
  completed_at: string | null
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  id: string
  plan_id: string
  request_budget: number
  request_count: number
  site_id: string | null
  software_order_id: string | null
  source_count: number
  started_at: string | null
  status: string
  subject: string
  updated_at: string
}>

type StoredQuery = Readonly<{
  assignment_id: string
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  freshness_max_age_hours: number
  id: string
  latency_ms: number | null
  query_hash: string
  query_text: string
  result_count: number
  run_id: string
  search_type: string
  source_category: string
  status: string
  updated_at: string
}>

type StoredSource = Readonly<{
  approved_at: string | null
  assignment_id: string
  created_at: string
  domain: string
  exa_request_id: string | null
  highlight_text: string | null
  id: string
  public_safe: number
  published_date: string | null
  query_id: string | null
  rejected_at: string | null
  rejected_reason: string | null
  review_status: string
  run_id: string
  search_type: string | null
  selected_text_hash: string | null
  site_id: string | null
  software_order_id: string | null
  source_category: string
  title: string
  updated_at: string
  url: string
}>

type StoredLink = Readonly<{
  approved_at: string | null
  assignment_id: string
  created_at: string
  enrichment_run_id: string
  required_for_launch: number
  research_brief_id: string | null
  status: string
  updated_at: string
}>

type LedgerMemoryState = Readonly<{
  links: Array<StoredLink>
  queries: Array<StoredQuery>
  runs: Array<StoredRun>
  sources: Array<StoredSource>
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
const numberValue = (value: unknown): number => Number(value)

const refreshSourceCounts = (state: LedgerMemoryState, runId: string): void => {
  const runIndex = state.runs.findIndex(run => run.id === runId)

  if (runIndex === -1) {
    return
  }

  const sourceCount = state.sources.filter(source => source.run_id === runId)
    .length
  const approvedSourceCount = state.sources.filter(
    source =>
      source.run_id === runId &&
      source.public_safe === 1 &&
      (source.review_status === 'approved' ||
        source.review_status === 'public_safe'),
  ).length
  const current = state.runs[runIndex]

  if (current === undefined) {
    return
  }

  state.runs[runIndex] = {
    ...current,
    approved_source_count: approvedSourceCount,
    source_count: sourceCount,
  }
}

const runStatement = (
  state: LedgerMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): void => {
  if (query.includes('INSERT INTO exa_enrichment_runs')) {
    state.runs.push({
      id: textValue(values[0]),
      assignment_id: textValue(values[1]),
      software_order_id: nullableText(values[2]),
      site_id: nullableText(values[3]),
      plan_id: textValue(values[4]),
      subject: textValue(values[5]),
      status: textValue(values[6]),
      request_budget: numberValue(values[7]),
      started_at: nullableText(values[8]),
      created_at: textValue(values[9]),
      updated_at: textValue(values[10]),
      request_count: 0,
      cache_hit_count: 0,
      source_count: 0,
      approved_source_count: 0,
      cost_dollars: null,
      error_code: null,
      error_summary: null,
      completed_at: null,
      archived_at: null,
    })

    return
  }

  if (query.includes('INSERT INTO exa_enrichment_queries')) {
    state.queries.push({
      id: textValue(values[0]),
      run_id: textValue(values[1]),
      assignment_id: textValue(values[2]),
      query_hash: textValue(values[3]),
      query_text: textValue(values[4]),
      source_category: textValue(values[5]),
      search_type: textValue(values[6]),
      freshness_max_age_hours: numberValue(values[7]),
      status: textValue(values[8]),
      result_count: numberValue(values[9]),
      latency_ms: values[10] === null ? null : numberValue(values[10]),
      cost_dollars: values[11] === null ? null : numberValue(values[11]),
      error_code: nullableText(values[12]),
      error_summary: nullableText(values[13]),
      created_at: textValue(values[14]),
      updated_at: textValue(values[15]),
    })

    return
  }

  if (query.includes('request_count = request_count + 1')) {
    const runId = textValue(values[3])
    const runIndex = state.runs.findIndex(run => run.id === runId)
    const current = state.runs[runIndex]

    if (current !== undefined) {
      state.runs[runIndex] = {
        ...current,
        cache_hit_count: current.cache_hit_count + numberValue(values[0]),
        cost_dollars:
          current.cost_dollars === null
            ? numberValue(values[1])
            : current.cost_dollars + numberValue(values[1]),
        request_count: current.request_count + 1,
        updated_at: textValue(values[2]),
      }
    }

    return
  }

  if (query.includes('INSERT INTO exa_enrichment_sources')) {
    state.sources.push({
      id: textValue(values[0]),
      run_id: textValue(values[1]),
      query_id: nullableText(values[2]),
      assignment_id: textValue(values[3]),
      software_order_id: nullableText(values[4]),
      site_id: nullableText(values[5]),
      source_category: textValue(values[6]),
      review_status: textValue(values[7]),
      title: textValue(values[8]),
      url: textValue(values[9]),
      domain: textValue(values[10]),
      published_date: nullableText(values[11]),
      highlight_text: nullableText(values[12]),
      selected_text_hash: nullableText(values[13]),
      exa_request_id: nullableText(values[14]),
      search_type: nullableText(values[15]),
      public_safe: numberValue(values[16]),
      rejected_reason: nullableText(values[17]),
      approved_at: nullableText(values[18]),
      rejected_at: nullableText(values[19]),
      created_at: textValue(values[20]),
      updated_at: textValue(values[21]),
    })
    refreshSourceCounts(state, textValue(values[1]))

    return
  }

  if (query.includes('SET review_status = ?')) {
    const sourceId = textValue(values[6])
    const sourceIndex = state.sources.findIndex(source => source.id === sourceId)
    const current = state.sources[sourceIndex]

    if (current !== undefined) {
      state.sources[sourceIndex] = {
        ...current,
        review_status: textValue(values[0]),
        public_safe: numberValue(values[1]),
        rejected_reason: nullableText(values[2]),
        approved_at: nullableText(values[3]),
        rejected_at: nullableText(values[4]),
        updated_at: textValue(values[5]),
      }
      refreshSourceCounts(state, current.run_id)
    }

    return
  }

  if (query.includes('INSERT INTO adjutant_assignment_enrichments')) {
    state.links.push({
      assignment_id: textValue(values[0]),
      enrichment_run_id: textValue(values[1]),
      research_brief_id: nullableText(values[2]),
      status: textValue(values[3]),
      required_for_launch: numberValue(values[4]),
      approved_at: nullableText(values[5]),
      created_at: textValue(values[6]),
      updated_at: textValue(values[7]),
    })
  }
}

const allRows = (
  state: LedgerMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): Array<unknown> => {
  if (query.includes('FROM exa_enrichment_queries')) {
    return state.queries.filter(queryRow => queryRow.run_id === values[0])
  }

  if (query.includes('FROM exa_enrichment_sources')) {
    return state.sources.filter(source => source.assignment_id === values[0])
  }

  return []
}

const firstRow = (
  state: LedgerMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): unknown | null => {
  if (query.includes('FROM exa_enrichment_runs')) {
    return (
      state.runs.find(
        run => run.assignment_id === values[0] && run.archived_at === null,
      ) ?? null
    )
  }

  return null
}

const makeMemoryD1 = (): D1Database & LedgerMemoryState => {
  const state: LedgerMemoryState = {
    links: [],
    queries: [],
    runs: [],
    sources: [],
  }

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
      all: <T = Record<string, unknown>>() =>
        Promise.resolve(makeResult<T>(allRows(state, query, values) as Array<T>)),
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

  const db: D1Database & LedgerMemoryState = {
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

const runtime = (): AdjutantEnrichmentLedgerRuntime => {
  let queryIndex = 0
  let runIndex = 0
  let sourceIndex = 0

  return {
    makeQueryId: () => `exa_enrichment_query_${queryIndex++}`,
    makeRunId: () => `exa_enrichment_run_${runIndex++}`,
    makeSourceId: () => `exa_enrichment_source_${sourceIndex++}`,
    nowIso: () => '2026-06-05T00:00:00.000Z',
  }
}

describe('AdjutantEnrichmentLedger', () => {
  test('stores runs, queries, source cards, and assignment links in D1', async () => {
    const db = makeMemoryD1()
    const ledger = makeAdjutantEnrichmentLedger(db, runtime())

    const run = await Effect.runPromise(
      ledger.createRun({
        assignmentId: 'adjutant_assignment_otec',
        planId: 'exa_plan_otec',
        requestBudget: 6,
        siteId: 'site_project_otec',
        softwareOrderId: 'software_order_otec',
        subject: 'Ben OTEC floating datacenter site',
      }),
    )
    const query = await Effect.runPromise(
      ledger.recordQuery({
        assignmentId: run.assignmentId,
        costDollars: 0.01,
        freshnessMaxAgeHours: 24,
        latencyMs: 42,
        queryText: 'OTEC SWAC floating datacenter public evidence',
        resultCount: 2,
        runId: run.id,
        searchType: 'auto',
        sourceCategory: 'topic_web',
        status: 'succeeded',
      }),
    )
    const source = await Effect.runPromise(
      ledger.storeSourceCard({
        assignmentId: run.assignmentId,
        exaRequestId: 'req_otec',
        highlightText: 'Ocean thermal energy conversion uses thermal gradients.',
        queryId: query.id,
        runId: run.id,
        searchType: 'auto',
        selectedText:
          'Longer source text that should be hashed instead of stored raw.',
        siteId: 'site_project_otec',
        softwareOrderId: 'software_order_otec',
        sourceCategory: 'topic_web',
        title: 'OTEC overview',
        url: 'https://example.com/otec',
      }),
    )

    await Effect.runPromise(
      ledger.linkAssignmentRun({
        assignmentId: run.assignmentId,
        enrichmentRunId: run.id,
        requiredForLaunch: true,
        status: 'needs_review',
      }),
    )

    const latestRun = await Effect.runPromise(
      ledger.latestRunForAssignment(run.assignmentId),
    )
    const queries = await Effect.runPromise(ledger.queriesForRun(run.id))
    const sources = await Effect.runPromise(
      ledger.sourceCardsForAssignment(run.assignmentId),
    )

    expect(latestRun?.id).toBe(run.id)
    expect(queries[0]?.queryHash).toHaveLength(64)
    expect(sources[0]?.selectedTextHash).toHaveLength(64)
    expect(JSON.stringify(db.sources)).not.toContain(
      'Longer source text that should be hashed instead of stored raw.',
    )
    expect(db.links[0]).toMatchObject({
      assignment_id: run.assignmentId,
      enrichment_run_id: run.id,
      required_for_launch: 1,
      status: 'needs_review',
    })
    expect(source.domain).toBe('example.com')
  })

  test('rejects secret-shaped and oversized source-card material', async () => {
    const db = makeMemoryD1()
    const ledger = makeAdjutantEnrichmentLedger(db, runtime())

    await expect(
      Effect.runPromise(
        ledger.storeSourceCard({
          assignmentId: 'adjutant_assignment_otec',
          highlightText: 'Bearer sk-secret123456789 should not persist',
          runId: 'exa_enrichment_run_1',
          sourceCategory: 'topic_web',
          title: 'Unsafe source',
          url: 'https://example.com/unsafe',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantEnrichmentLedgerUnsafePayload',
    })

    await expect(
      Effect.runPromise(
        ledger.storeSourceCard({
          assignmentId: 'adjutant_assignment_otec',
          highlightText: 'x'.repeat(1201),
          runId: 'exa_enrichment_run_1',
          sourceCategory: 'topic_web',
          title: 'Large source',
          url: 'https://example.com/large',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantEnrichmentLedgerValidationError',
    })
    expect(db.sources).toHaveLength(0)
  })

  test('checks normalized source-card storage payload size', async () => {
    const db = makeMemoryD1()
    const ledger = makeAdjutantEnrichmentLedger(db, runtime())
    const longPath = 'public-context-'.repeat(120)
    const publicHighlight = 'Public source context. '.repeat(45)

    const source = await Effect.runPromise(
      ledger.storeSourceCard({
        assignmentId: 'adjutant_assignment_large_source',
        highlightText: publicHighlight,
        runId: 'exa_enrichment_run_1',
        selectedText: publicHighlight,
        sourceCategory: 'topic_web',
        title: 'Large public source',
        url: `https://example.com/${longPath}`,
      }),
    )

    expect(source.selectedTextHash).toHaveLength(64)
    expect(db.sources).toHaveLength(1)
    expect(JSON.stringify(db.sources)).not.toContain(publicHighlight)
  })

  test('filters public-safe source cards and applies review transitions', async () => {
    const db = makeMemoryD1()
    const ledger = makeAdjutantEnrichmentLedger(db, runtime())
    const run = await Effect.runPromise(
      ledger.createRun({
        assignmentId: 'adjutant_assignment_otec',
        planId: 'exa_plan_otec',
        subject: 'OTEC site',
      }),
    )
    const source = await Effect.runPromise(
      ledger.storeSourceCard({
        assignmentId: run.assignmentId,
        highlightText: 'Sourced public context.',
        runId: run.id,
        sourceCategory: 'topic_web',
        title: 'Public source',
        url: 'https://example.com/public',
      }),
    )

    expect(
      publicSafeExaSourceCards([
        {
          ...source,
          publicSafe: true,
          reviewStatus: 'internal_only',
        },
        {
          ...source,
          publicSafe: true,
          reviewStatus: 'approved',
        },
      ]),
    ).toHaveLength(1)

    await Effect.runPromise(
      ledger.reviewSourceCard({
        publicSafe: true,
        reviewStatus: 'approved',
        sourceId: source.id,
      }),
    )

    const publicSources = await Effect.runPromise(
      ledger.publicSafeSourceCardsForAssignment(run.assignmentId),
    )
    const latestRun = await Effect.runPromise(
      ledger.latestRunForAssignment(run.assignmentId),
    )

    expect(publicSources).toHaveLength(1)
    expect(publicSources[0]?.reviewStatus).toBe('approved')
    expect(latestRun?.approvedSourceCount).toBe(1)
  })
})
