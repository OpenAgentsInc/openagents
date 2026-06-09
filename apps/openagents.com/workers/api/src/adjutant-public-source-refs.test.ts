import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AdjutantPublicSourceRefRuntime,
  makeAdjutantPublicSourceRefService,
  plannerSourceRefs,
} from './adjutant-public-source-refs'

type StoredSourceRef = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  created_at: string
  id: string
  kind: string
  label: string | null
  normalized_domain: string
  proposed_by_user_id: string | null
  public_safe: number
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  site_id: string | null
  software_order_id: string | null
  status: string
  updated_at: string
  url: string
}>

type SourceRefMemoryState = Readonly<{
  sourceRefs: Array<StoredSourceRef>
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

const runStatement = (
  state: SourceRefMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): void => {
  if (query.includes('INSERT INTO adjutant_public_source_refs')) {
    state.sourceRefs.push({
      id: textValue(values[0]),
      assignment_id: textValue(values[1]),
      software_order_id: nullableText(values[2]),
      site_id: nullableText(values[3]),
      kind: textValue(values[4]),
      status: textValue(values[5]),
      url: textValue(values[6]),
      normalized_domain: textValue(values[7]),
      label: nullableText(values[8]),
      public_safe: numberValue(values[9]),
      proposed_by_user_id: nullableText(values[10]),
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

  if (query.includes('UPDATE adjutant_public_source_refs')) {
    const sourceRefId = textValue(values[7])
    const index = state.sourceRefs.findIndex(
      sourceRef => sourceRef.id === sourceRefId,
    )
    const current = state.sourceRefs[index]

    if (current !== undefined) {
      state.sourceRefs[index] = {
        ...current,
        status: textValue(values[0]),
        public_safe: numberValue(values[1]),
        reviewed_by_user_id: nullableText(values[2]),
        review_reason: nullableText(values[3]),
        approved_at: nullableText(values[4]),
        rejected_at: nullableText(values[5]),
        updated_at: textValue(values[6]),
      }
    }
  }
}

const allRows = (
  state: SourceRefMemoryState,
  query: string,
  values: ReadonlyArray<unknown>,
): Array<unknown> =>
  query.includes('FROM adjutant_public_source_refs')
    ? state.sourceRefs.filter(
        sourceRef =>
          sourceRef.assignment_id === values[0] &&
          sourceRef.archived_at === null,
      )
    : []

const makeMemoryD1 = (): D1Database & SourceRefMemoryState => {
  const state: SourceRefMemoryState = { sourceRefs: [] }

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
      first: () => Promise.resolve(null),
      raw,
      run: <T = Record<string, unknown>>() => {
        runStatement(state, query, values)

        return Promise.resolve(makeResult<T>())
      },
    }

    return statement
  }

  const db: D1Database & SourceRefMemoryState = {
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

const runtime = (): AdjutantPublicSourceRefRuntime => {
  let index = 0

  return {
    makeSourceRefId: () => `adjutant_public_source_ref_${index++}`,
    nowIso: () => '2026-06-05T00:00:00.000Z',
  }
}

describe('AdjutantPublicSourceRefService', () => {
  test('attaches and reviews explicit public source refs for planner use', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantPublicSourceRefService(db, runtime())
    const proposed = await Effect.runPromise(
      service.createSourceRef({
        assignmentId: 'adjutant_assignment_otec',
        kind: 'github_repository',
        label: 'Ben OTEC repository',
        proposedByUserId: 'github:14167547',
        siteId: 'site_project_otec',
        softwareOrderId: 'software_order_otec',
        url: 'https://github.com/bensilone/openagents#readme',
      }),
    )
    const profile = await Effect.runPromise(
      service.createSourceRef({
        assignmentId: 'adjutant_assignment_otec',
        kind: 'github_profile',
        status: 'public_safe',
        url: 'https://github.com/bensilone',
      }),
    )

    await Effect.runPromise(
      service.reviewSourceRef({
        publicSafe: true,
        reviewReason: 'Operator verified public repo.',
        reviewedByUserId: 'github:operator',
        sourceRefId: proposed.id,
        status: 'approved',
      }),
    )

    const sourceRefs = await Effect.runPromise(
      service.listForAssignment('adjutant_assignment_otec'),
    )
    const plannerRefs = await Effect.runPromise(
      service.plannerSourceRefsForAssignment('adjutant_assignment_otec'),
    )

    expect(sourceRefs).toHaveLength(2)
    expect(sourceRefs.find(sourceRef => sourceRef.id === proposed.id)).toMatchObject({
      publicSafe: true,
      status: 'approved',
      url: 'https://github.com/bensilone/openagents',
    })
    expect(plannerRefs).toEqual([
      {
        id: proposed.id,
        kind: 'github_repository',
        label: 'Ben OTEC repository',
        status: 'approved',
        url: 'https://github.com/bensilone/openagents',
      },
      {
        id: profile.id,
        kind: 'github_profile',
        status: 'public_safe',
        url: 'https://github.com/bensilone',
      },
    ])
  })

  test('planner projection excludes proposed, rejected, and internal-only refs', () => {
    expect(
      plannerSourceRefs([
        {
          id: 'source_ref_proposed',
          assignmentId: 'assignment',
          softwareOrderId: null,
          siteId: null,
          kind: 'generic_url',
          status: 'proposed',
          url: 'https://example.com/proposed',
          normalizedDomain: 'example.com',
          label: null,
          publicSafe: false,
          proposedByUserId: null,
          reviewedByUserId: null,
          reviewReason: null,
          approvedAt: null,
          rejectedAt: null,
          createdAt: '2026-06-05T00:00:00.000Z',
          updatedAt: '2026-06-05T00:00:00.000Z',
          archivedAt: null,
        },
        {
          id: 'source_ref_approved',
          assignmentId: 'assignment',
          softwareOrderId: null,
          siteId: null,
          kind: 'personal_site',
          status: 'approved',
          url: 'https://example.com/public',
          normalizedDomain: 'example.com',
          label: 'Public site',
          publicSafe: true,
          proposedByUserId: null,
          reviewedByUserId: 'operator',
          reviewReason: null,
          approvedAt: '2026-06-05T00:00:00.000Z',
          rejectedAt: null,
          createdAt: '2026-06-05T00:00:00.000Z',
          updatedAt: '2026-06-05T00:00:00.000Z',
          archivedAt: null,
        },
      ]),
    ).toEqual([
      {
        id: 'source_ref_approved',
        kind: 'personal_site',
        label: 'Public site',
        status: 'approved',
        url: 'https://example.com/public',
      },
    ])
  })

  test('rejects private, non-public, and mismatched source refs', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantPublicSourceRefService(db, runtime())

    await expect(
      Effect.runPromise(
        service.createSourceRef({
          assignmentId: 'assignment',
          kind: 'github_repository',
          repositoryPrivate: true,
          url: 'https://github.com/bensilone/private-repo',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantPublicSourceRefValidationError',
    })

    await expect(
      Effect.runPromise(
        service.createSourceRef({
          assignmentId: 'assignment',
          kind: 'generic_url',
          url: 'http://localhost/private',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantPublicSourceRefValidationError',
    })

    await expect(
      Effect.runPromise(
        service.createSourceRef({
          assignmentId: 'assignment',
          kind: 'linkedin_profile',
          url: 'https://linkedin.com/company/openagents',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantPublicSourceRefValidationError',
    })
    expect(db.sourceRefs).toHaveLength(0)
  })

  test('rejects secret-shaped labels and URLs', async () => {
    const db = makeMemoryD1()
    const service = makeAdjutantPublicSourceRefService(db, runtime())

    await expect(
      Effect.runPromise(
        service.createSourceRef({
          assignmentId: 'assignment',
          kind: 'generic_url',
          label: 'Bearer sk-secret123456789',
          url: 'https://example.com/public',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantPublicSourceRefUnsafePayload',
    })
  })
})
