import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeArtanisUnsupportedRequestIssueOpener,
} from './artanis-operator-unsupported-requests'
import {
  type KhalaUnsupportedRequestCreateInput,
  type KhalaUnsupportedRequestRecord,
  type KhalaUnsupportedRequestStore,
  handleOperatorKhalaUnsupportedRequests,
} from './khala-unsupported-request-routes'

const run = (effect: Effect.Effect<Response>): Promise<Response> =>
  Effect.runPromise(effect)

const issueRequiredFor = (
  record: Pick<KhalaUnsupportedRequestRecord, 'githubIssueRef' | 'triageKind'>,
): boolean =>
  (record.triageKind === 'bug' || record.triageKind === 'missing_capability') &&
  record.githubIssueRef === null

const nextActionFor = (
  record: Pick<
    KhalaUnsupportedRequestRecord,
    'forumTopicRef' | 'githubIssueRef' | 'status' | 'triageKind'
  >,
): KhalaUnsupportedRequestRecord['nextAction'] => {
  if (issueRequiredFor(record)) {
    return 'open_github_issue'
  }
  if (record.triageKind === 'needs_triage') {
    return 'triage'
  }
  if (
    record.status === 'open' &&
    record.forumTopicRef === null &&
    record.triageKind !== 'wont_do'
  ) {
    return 'link_forum_report'
  }
  return 'none'
}

const recordFromInput = (
  input: KhalaUnsupportedRequestCreateInput,
): KhalaUnsupportedRequestRecord => {
  const base = {
    createdAt: input.createdAt,
    evidenceRefs: input.evidenceRefs,
    forumTopicRef: input.forumTopicRef,
    githubIssueRef: input.githubIssueRef,
    requestRef: input.requestRef,
    sourceKind: input.sourceKind,
    sourceRef: input.sourceRef,
    status: input.status,
    suggestedIssueTitle: input.suggestedIssueTitle,
    summary: input.summary,
    title: input.title,
    triageKind: input.triageKind,
    updatedAt: input.updatedAt,
  }
  return {
    ...base,
    issueRequired: issueRequiredFor(base),
    nextAction: nextActionFor(base),
  }
}

const makeStore = (): KhalaUnsupportedRequestStore & {
  readonly records: Array<KhalaUnsupportedRequestRecord>
} => {
  const records: Array<KhalaUnsupportedRequestRecord> = []
  return {
    records,
    listRecent: async input =>
      records
        .filter(record =>
          input.status === undefined ? true : record.status === input.status,
        )
        .filter(record =>
          input.triageKind === undefined
            ? true
            : record.triageKind === input.triageKind,
        )
        .filter(record =>
          input.sourceKind === undefined
            ? true
            : record.sourceKind === input.sourceKind,
        )
        .slice(0, input.limit),
    upsert: async input => {
      const existingIndex = records.findIndex(
        record =>
          record.sourceKind === input.sourceKind &&
          record.sourceRef === input.sourceRef,
      )
      const record = recordFromInput(input)
      if (existingIndex === -1) {
        records.unshift(record)
      } else {
        records[existingIndex] = {
          ...record,
          createdAt: records[existingIndex]?.createdAt ?? record.createdAt,
          requestRef: records[existingIndex]?.requestRef ?? record.requestRef,
        }
      }
      return records[existingIndex === -1 ? 0 : existingIndex] ?? record
    },
  }
}

const postRequest = (body: unknown): Request =>
  new Request(
    'https://openagents.com/api/operator/khala/unsupported-requests',
    {
      body: JSON.stringify(body),
      headers: { 'content-type': 'application/json' },
      method: 'POST',
    },
  )

describe('khala unsupported-request operator routes', () => {
  test('stores missing capability rows as issue-required by default', async () => {
    const store = makeStore()
    const response = await run(
      handleOperatorKhalaUnsupportedRequests(
        postRequest({
          evidenceRefs: ['triage.intent.khala_trace_review.khala_cli'],
          sourceKind: 'trace_review',
          sourceRef: 'triage.intent.khala_trace_review.khala_cli',
          summary:
            'Repeated tester traces asked for a Khala workflow that is not implemented yet.',
          title: 'CLI users ask Khala to keep a resumable work queue',
          triageKind: 'missing_capability',
        }),
        {
          makeRequestRef: () => 'khala_unsupported:test',
          nowIso: () => '2026-06-26T22:00:00.000Z',
          requireAdminApiToken: async () => true,
          store,
        },
      ),
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toEqual({
      schemaVersion: 'openagents.khala.unsupported_requests.upsert.v1',
      unsupportedRequest: {
        createdAt: '2026-06-26T22:00:00.000Z',
        evidenceRefs: ['triage.intent.khala_trace_review.khala_cli'],
        forumTopicRef: null,
        githubIssueRef: null,
        issueRequired: true,
        nextAction: 'open_github_issue',
        requestRef: 'khala_unsupported:test',
        sourceKind: 'trace_review',
        sourceRef: 'triage.intent.khala_trace_review.khala_cli',
        status: 'needs_issue',
        suggestedIssueTitle:
          '[Khala unsupported] CLI users ask Khala to keep a resumable work queue',
        summary:
          'Repeated tester traces asked for a Khala workflow that is not implemented yet.',
        title: 'CLI users ask Khala to keep a resumable work queue',
        triageKind: 'missing_capability',
        updatedAt: '2026-06-26T22:00:00.000Z',
      },
    })
  })

  test('lists and filters unsupported requests for admins', async () => {
    const store = makeStore()
    await store.upsert({
      createdAt: '2026-06-26T22:00:00.000Z',
      evidenceRefs: ['khala_feedback:one'],
      forumTopicRef: 'forum.product-promises.123',
      githubIssueRef: null,
      requestRef: 'khala_unsupported:one',
      sourceKind: 'khala_feedback',
      sourceRef: 'khala_feedback:one',
      status: 'needs_issue',
      suggestedIssueTitle: '[Khala unsupported] first',
      summary: '',
      title: 'first',
      triageKind: 'bug',
      updatedAt: '2026-06-26T22:00:00.000Z',
    })
    await store.upsert({
      createdAt: '2026-06-26T22:01:00.000Z',
      evidenceRefs: ['forum.product-promises.456'],
      forumTopicRef: 'forum.product-promises.456',
      githubIssueRef: null,
      requestRef: 'khala_unsupported:two',
      sourceKind: 'forum',
      sourceRef: 'forum.product-promises.456',
      status: 'wont_do',
      suggestedIssueTitle: '[Khala unsupported] second',
      summary: '',
      title: 'second',
      triageKind: 'wont_do',
      updatedAt: '2026-06-26T22:01:00.000Z',
    })

    const response = await run(
      handleOperatorKhalaUnsupportedRequests(
        new Request(
          'https://openagents.com/api/operator/khala/unsupported-requests?status=needs_issue',
        ),
        {
          nowIso: () => '2026-06-26T22:02:00.000Z',
          requireAdminApiToken: async () => true,
          store,
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      unsupportedRequests: ReadonlyArray<KhalaUnsupportedRequestRecord>
    }
    expect(body.unsupportedRequests.map(record => record.requestRef)).toEqual([
      'khala_unsupported:one',
    ])
  })

  test('requires issue refs before marking issue_opened', async () => {
    const store = makeStore()
    const response = await run(
      handleOperatorKhalaUnsupportedRequests(
        postRequest({
          sourceKind: 'operator',
          sourceRef: 'operator.manual.review',
          status: 'issue_opened',
          title: 'Manual triage row',
          triageKind: 'bug',
        }),
        {
          requireAdminApiToken: async () => true,
          store,
        },
      ),
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({
      error: 'bad_request',
      reason: 'githubIssueRef is required when status is issue_opened',
    })
  })

  test('blocks operator reads and writes without admin auth', async () => {
    const store = makeStore()
    const response = await run(
      handleOperatorKhalaUnsupportedRequests(
        new Request(
          'https://openagents.com/api/operator/khala/unsupported-requests',
        ),
        {
          requireAdminApiToken: async () => false,
          store,
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('rejects unsupported methods', async () => {
    const store = makeStore()
    const response = await run(
      handleOperatorKhalaUnsupportedRequests(
        new Request(
          'https://openagents.com/api/operator/khala/unsupported-requests',
          { method: 'DELETE' },
        ),
        {
          requireAdminApiToken: async () => true,
          store,
        },
      ),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET, POST')
  })

  test('GitHub issue opener creates an issue from a needs_issue row and links it', async () => {
    const store = makeStore()
    await store.upsert({
      createdAt: '2026-06-27T10:00:00.000Z',
      evidenceRefs: ['triage.intent.khala_trace_review.local_diff'],
      forumTopicRef: null,
      githubIssueRef: null,
      requestRef: 'khala_unsupported:local_diff',
      sourceKind: 'trace_review',
      sourceRef: 'triage.intent.khala_trace_review.local_diff',
      status: 'needs_issue',
      suggestedIssueTitle: '[Khala unsupported] local diff reads',
      summary: 'Users ask Khala to inspect a local git diff before answering.',
      title: 'Khala cannot read local git diffs',
      triageKind: 'missing_capability',
      updatedAt: '2026-06-27T10:00:00.000Z',
    })

    const calls: Array<Readonly<{ body: unknown; url: string }>> = []
    const opener = makeArtanisUnsupportedRequestIssueOpener({
      fetchImpl: (async (input: RequestInfo | URL, init?: RequestInit) => {
        calls.push({
          body: JSON.parse(String(init?.body ?? '{}')) as unknown,
          url: String(input),
        })
        return Response.json({
          html_url:
            'https://github.com/OpenAgentsInc/openagents/issues/7001',
          number: 7001,
        })
      }) as typeof fetch,
      githubToken: 'gho_test_token_never_logged',
      nowIso: () => '2026-06-27T10:05:00.000Z',
      store,
    })

    const result = await Effect.runPromise(
      opener({ ref: 'khala_unsupported:local_diff' }),
    )

    expect(result).toMatchObject({
      issueNumber: 7001,
      issueRef: 'OpenAgentsInc/openagents#7001',
      issueUrl: 'https://github.com/OpenAgentsInc/openagents/issues/7001',
      kind: 'opened',
    })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues',
    )
    expect(calls[0]?.body).toMatchObject({
      title: '[Khala unsupported] local diff reads',
    })
    expect(JSON.stringify(calls[0]?.body)).toContain(
      'triage.intent.khala_trace_review.local_diff',
    )
    expect(JSON.stringify(calls[0]?.body)).not.toContain(
      'gho_test_token_never_logged',
    )
    expect(store.records[0]).toMatchObject({
      githubIssueRef: 'OpenAgentsInc/openagents#7001',
      requestRef: 'khala_unsupported:local_diff',
      status: 'issue_opened',
      updatedAt: '2026-06-27T10:05:00.000Z',
    })
  })

  test('GitHub issue opener is inert without a configured token', async () => {
    const store = makeStore()
    const opener = makeArtanisUnsupportedRequestIssueOpener({
      githubToken: undefined,
      store,
    })
    await expect(
      Effect.runPromise(opener({ ref: 'khala_unsupported:missing' })),
    ).resolves.toEqual({
      kind: 'rejected',
      reason: 'github_issue_token_not_configured',
    })
  })
})
