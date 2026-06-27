import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_ISSUE_BODY_MAX_CHARS,
  ARTANIS_REPO_READ_MAX_BYTES,
  isSafeArtanisRepoPath,
  makeArtanisDispatchCodexTaskTool,
  makeArtanisGetNetworkStatsTool,
  makeArtanisListRepoDirTool,
  makeArtanisOperatorTools,
  makeArtanisReadGithubIssueTool,
  makeArtanisReadRepoFileTool,
  parseArtanisIssueNumber,
} from './artanis-operator-tools'

// A fetch stub that records the URL it was asked for and returns a canned
// Response. This is the only seam the read tools touch the network through.
const stubFetch = (
  handler: (url: string) => Response,
): {
  fetchImpl: typeof fetch
  urls: Array<string>
} => {
  const urls: Array<string> = []
  const fetchImpl = (async (input: RequestInfo | URL) => {
    const url = typeof input === 'string' ? input : input.toString()
    urls.push(url)
    return handler(url)
  }) as typeof fetch
  return { fetchImpl, urls }
}

describe('#6365 read_repo_file (public OpenAgentsInc/openagents only)', () => {
  test('returns the real file contents from raw.githubusercontent', async () => {
    const fileBody =
      '# Khala open issues master roadmap\nFirst priority: the #6316 serving track.'
    const { fetchImpl, urls } = stubFetch(
      () => new Response(fileBody, { status: 200 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })

    const result = await Effect.runPromise(
      tool.execute({
        path: 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
      }),
    )

    expect(result).toBe(fileBody)
    expect(urls[0]).toBe(
      'https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
    )
  })

  test('a 404 reads as honest "file not found", never invention', async () => {
    const { fetchImpl } = stubFetch(
      () => new Response('Not Found', { status: 404 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ path: 'docs/does-not-exist.md' }),
    )
    expect(result).toContain('file not found')
  })

  test('blocks a secret-bearing path and never fetches it', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    for (const path of [
      '.secrets/tailnet.env',
      '../../etc/passwd',
      '/etc/passwd',
      'config/wallet-mnemonic.txt',
    ]) {
      const result = await Effect.runPromise(tool.execute({ path }))
      expect(result).toContain('blocked')
    }
    expect(urls).toHaveLength(0)
  })

  test('truncates an oversized file at the byte cap', async () => {
    const big = 'a'.repeat(ARTANIS_REPO_READ_MAX_BYTES + 10)
    const { fetchImpl } = stubFetch(() => new Response(big, { status: 200 }))
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ path: 'docs/big.md' }),
    )
    expect(result).toContain('truncated')
    expect(result.length).toBeLessThan(big.length + 200)
  })

  test('invalid arguments return an honest message, not a throw', async () => {
    const { fetchImpl } = stubFetch(() => new Response('', { status: 200 }))
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ notPath: 1 }))
    expect(result).toContain('invalid arguments')
  })
})

describe('#6365 list_repo_dir', () => {
  test('lists directory entries from the GitHub contents API', async () => {
    const body = JSON.stringify([
      { name: 'README.md', type: 'file' },
      { name: 'khala', type: 'dir' },
    ])
    const { fetchImpl, urls } = stubFetch(
      () => new Response(body, { status: 200 }),
    )
    const tool = makeArtanisListRepoDirTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ path: 'docs' }))
    expect(result).toContain('README.md (file)')
    expect(result).toContain('khala (dir)')
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/contents/docs?ref=main',
    )
  })

  test('root listing accepts "" and "."', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response(JSON.stringify([{ name: 'docs', type: 'dir' }]), {
        status: 200,
      }),
    )
    const tool = makeArtanisListRepoDirTool({ fetchImpl })
    await Effect.runPromise(tool.execute({ path: '.' }))
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/contents/?ref=main',
    )
  })
})

describe('isSafeArtanisRepoPath', () => {
  test('accepts normal repo paths', () => {
    expect(
      isSafeArtanisRepoPath(
        'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
      ),
    ).toBe(true)
    expect(isSafeArtanisRepoPath('apps/openagents.com/workers/api/src/x.ts')).toBe(
      true,
    )
  })
  test('rejects traversal, absolute, secret, and node_modules paths', () => {
    expect(isSafeArtanisRepoPath('../secrets')).toBe(false)
    expect(isSafeArtanisRepoPath('/etc/passwd')).toBe(false)
    expect(isSafeArtanisRepoPath('.secrets/tailnet.env')).toBe(false)
    expect(isSafeArtanisRepoPath('node_modules/x/index.js')).toBe(false)
    expect(isSafeArtanisRepoPath('')).toBe(false)
  })
})

describe('#6366 dispatch_codex_task (gated; plan-only without a seam)', () => {
  const tool = makeArtanisDispatchCodexTaskTool()

  test('is a gated pylon_job_dispatch tool with a run() entry point', () => {
    expect(tool.kind).toBe('gated')
    expect(tool.riskyActionKind).toBe('pylon_job_dispatch')
    expect(typeof tool.run).toBe('function')
    // No bare execute(): execution is decided inside run() behind the gate.
    expect('execute' in tool).toBe(false)
  })

  test('with NO execution seam it DEFERS and returns the public-safe plan', async () => {
    const result = await Effect.runPromise(
      tool.run({
        branch: 'main',
        filePaths: ['apps/openagents.com/workers/api/src/foo.ts'],
        issue: 6320,
        objective: 'Improve serving throughput per the roadmap.',
        verify:
          'bun run --cwd apps/openagents.com/workers/api test -- src/foo.test.ts',
      }),
    )
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('execution_not_wired')
    expect(result.plan).toContain('pylon khala request')
    expect(result.plan).toContain('--workflow codex_agent_task')
    expect(result.plan).toContain('--repo OpenAgentsInc/openagents')
    expect(result.plan).toContain('run-no-spend')
    expect(result.plan).toContain('#6320')
    expect(result.plan).toContain('src/foo.test.ts')
  })

  test('blocks a dispatch field carrying non-public-safe material (deferred)', async () => {
    const result = await Effect.runPromise(
      tool.run({
        objective: 'use the bearer token sk-abc123 to pay the payout',
      }),
    )
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('invalid_arguments')
    expect(result.plan).toContain('blocked')
  })

  test('requires a public-safe objective (deferred invalid_arguments)', async () => {
    const result = await Effect.runPromise(tool.run({}))
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('invalid_arguments')
    expect(result.plan).toContain('invalid arguments')
  })
})

describe('#6366 dispatch_codex_task (gated; LIVE execution behind the gate)', () => {
  test('owner-approved dispatch CREATES an assignment and returns the real ref', async () => {
    const createCalls: Array<unknown> = []
    const tool = makeArtanisDispatchCodexTaskTool({
      execution: {
        createCodexAssignment: plan => {
          createCalls.push(plan)
          return Effect.succeed({
            assignmentRef: 'assignment.public.khala_coding.live123',
            durableRequestId: 'req-live123',
            kind: 'created',
            pylonRef: 'pylon.owner.alpha',
          } as const)
        },
        isOwnerApproved: () => Effect.succeed(true),
      },
    })

    const result = await Effect.runPromise(
      tool.run({ objective: 'Burn down public issue work per the roadmap.' }),
    )
    expect(result.outcome).toBe('executed')
    if (result.outcome !== 'executed') return
    expect(result.assignmentRef).toBe('assignment.public.khala_coding.live123')
    expect(result.durableRequestId).toBe('req-live123')
    expect(createCalls).toHaveLength(1)
    // No-spend invariant is asserted in the public-safe summary.
    expect(result.summary).toContain('unpaid_smoke')
    expect(result.summary).toContain('settlement: not_applicable')
    expect(result.summary).toContain('payoutClaimAllowed: false')
    expect(result.summary).toContain('pylonRef: pylon.owner.alpha')
  })

  test('NO owner approval -> DEFERS without ever calling the create seam', async () => {
    const createCalls: Array<unknown> = []
    const tool = makeArtanisDispatchCodexTaskTool({
      execution: {
        createCodexAssignment: plan => {
          createCalls.push(plan)
          return Effect.succeed({
            assignmentRef: 'assignment.public.khala_coding.should_not_happen',
            durableRequestId: null,
            kind: 'created',
            pylonRef: 'pylon.owner.alpha',
          } as const)
        },
        isOwnerApproved: () => Effect.succeed(false),
      },
    })

    const result = await Effect.runPromise(
      tool.run({ objective: 'Burn down public issue work per the roadmap.' }),
    )
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('no_effective_owner_approval')
    // The create seam must NOT run when approval is missing.
    expect(createCalls).toHaveLength(0)
    expect(result.plan).toContain('pylon khala request')
  })

  test('approved but NO eligible Pylon -> DEFERS with the typed reason', async () => {
    const tool = makeArtanisDispatchCodexTaskTool({
      execution: {
        createCodexAssignment: () =>
          Effect.succeed({
            kind: 'rejected',
            reason: 'no_eligible_linked_pylon',
          } as const),
        isOwnerApproved: () => Effect.succeed(true),
      },
    })

    const result = await Effect.runPromise(
      tool.run({ objective: 'Burn down public issue work per the roadmap.' }),
    )
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('no_eligible_linked_pylon')
  })

  test('a non-public-safe objective never reaches the seam', async () => {
    const createCalls: Array<unknown> = []
    const approvalCalls: Array<unknown> = []
    const tool = makeArtanisDispatchCodexTaskTool({
      execution: {
        createCodexAssignment: plan => {
          createCalls.push(plan)
          return Effect.succeed({
            assignmentRef: 'assignment.public.khala_coding.should_not_happen',
            durableRequestId: null,
            kind: 'created',
            pylonRef: 'pylon.owner.alpha',
          } as const)
        },
        isOwnerApproved: () => {
          approvalCalls.push(true)
          return Effect.succeed(true)
        },
      },
    })

    const result = await Effect.runPromise(
      tool.run({
        objective: 'use the bearer token sk-abc123 to pay the payout',
      }),
    )
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('invalid_arguments')
    // Neither the approval gate nor the create seam is consulted for bad input.
    expect(approvalCalls).toHaveLength(0)
    expect(createCalls).toHaveLength(0)
  })
})

describe('read_github_issue (public OpenAgentsInc/openagents only)', () => {
  const issueBody = (over: Record<string, unknown> = {}) =>
    JSON.stringify({
      body: 'Build read_github_issue so Artanis can pull exact requirements.',
      comments: 0,
      state: 'open',
      title: 'read_github_issue operator tool',
      ...over,
    })

  test('returns title/state/body for a public issue number', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response(issueBody(), { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ issue_number: 6311 }),
    )
    expect(result).toContain('Issue #6311: read_github_issue operator tool')
    expect(result).toContain('State: open')
    expect(result).toContain(
      'Build read_github_issue so Artanis can pull exact requirements.',
    )
    expect(result).toContain('Comments: (none)')
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues/6311',
    )
  })

  test('accepts a "#"-prefixed numeric string and renders bounded comments', async () => {
    const { fetchImpl, urls } = stubFetch(url =>
      url.endsWith('/comments?per_page=20')
        ? new Response(
            JSON.stringify([
              {
                body: 'Acceptance: fake-fetch test + endpoint proof.',
                created_at: '2026-06-27T00:00:00Z',
                user: { login: 'chris' },
              },
            ]),
            { status: 200 },
          )
        : new Response(issueBody({ comments: 1 }), { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ issue_number: '#6320' }),
    )
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues/6320',
    )
    expect(urls[1]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues/6320/comments?per_page=20',
    )
    expect(result).toContain('Comments (showing 1 of 1):')
    expect(result).toContain('@chris')
    expect(result).toContain('Acceptance: fake-fetch test + endpoint proof.')
  })

  test('a 404 reads as honest "(issue not found: #N)", never invention', async () => {
    const { fetchImpl } = stubFetch(
      () => new Response('Not Found', { status: 404 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ issue_number: 999999 }),
    )
    expect(result).toBe('(issue not found: #999999)')
  })

  test('blocks non-numeric / private input and never fetches it', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    for (const issue_number of [
      'open the .secrets file',
      'https://example.com/private',
      '12.5',
      '-3',
      0,
    ]) {
      const result = await Effect.runPromise(tool.execute({ issue_number }))
      expect(result).toContain('blocked')
    }
    expect(urls).toHaveLength(0)
  })

  test('a missing issue argument returns an honest invalid-arguments message', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ notIssue: 1 }))
    expect(result).toContain('invalid arguments')
    expect(urls).toHaveLength(0)
  })

  test('truncates an oversized issue body at the char cap', async () => {
    const big = 'x'.repeat(ARTANIS_ISSUE_BODY_MAX_CHARS + 50)
    const { fetchImpl } = stubFetch(
      () =>
        new Response(issueBody({ body: big, comments: 0 }), { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ issue_number: 6359 }),
    )
    expect(result).toContain('truncated')
  })

  test('comment-fetch failure still returns the issue, with an honest note', async () => {
    const { fetchImpl } = stubFetch(url =>
      url.includes('/comments')
        ? new Response('boom', { status: 500 })
        : new Response(issueBody({ comments: 2 }), { status: 200 }),
    )
    const tool = makeArtanisReadGithubIssueTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ issue_number: 6311 }),
    )
    expect(result).toContain('Issue #6311')
    expect(result).toContain('could not be fetched')
  })
})

describe('parseArtanisIssueNumber', () => {
  test('accepts positive integers and numeric strings', () => {
    expect(parseArtanisIssueNumber({ issue_number: 6311 })).toEqual({
      kind: 'number',
      value: 6311,
    })
    expect(parseArtanisIssueNumber({ issue: '6320' })).toEqual({
      kind: 'number',
      value: 6320,
    })
    expect(parseArtanisIssueNumber({ number: '#6359' })).toEqual({
      kind: 'number',
      value: 6359,
    })
  })
  test('flags absent vs invalid distinctly', () => {
    expect(parseArtanisIssueNumber({}).kind).toBe('absent')
    expect(parseArtanisIssueNumber({ issue_number: 'foo' }).kind).toBe(
      'invalid',
    )
    expect(parseArtanisIssueNumber({ issue_number: -1 }).kind).toBe('invalid')
    expect(parseArtanisIssueNumber({ issue_number: 1.5 }).kind).toBe('invalid')
    expect(parseArtanisIssueNumber(null).kind).toBe('absent')
  })
})

describe('#6359 get_network_stats (live public stats + token pace)', () => {
  const statsFetch = (): typeof fetch =>
    (async (input: RequestInfo | URL) => {
      const url = typeof input === 'string' ? input : input.toString()
      if (url.includes('/khala-tokens-served/history')) {
        return new Response(
          JSON.stringify({
            series: [
              { day: '2026-06-26', tokensServed: 328_100_000 },
              { day: '2026-06-27', tokensServed: 100_000_000 },
            ],
          }),
          { status: 200 },
        )
      }
      if (url.includes('/khala-tokens-served/model-mix')) {
        return new Response(
          JSON.stringify({
            groups: [
              { family: 'glm', label: 'GLM', pct: 100, reqs: 1, tokens: 500 },
            ],
          }),
          { status: 200 },
        )
      }
      if (url.includes('/khala-tokens-served')) {
        return new Response(JSON.stringify({ tokensServed: 5_000_000_000 }), {
          status: 200,
        })
      }
      return new Response('not found', { status: 404 })
    }) as typeof fetch

  test('returns a public-safe snapshot with the pace block and flags behind-pace', async () => {
    const tool = makeArtanisGetNetworkStatsTool({
      fetchImpl: statsFetch(),
      // 17:00 UTC = 12:00 CDT -> 50% of the Central day elapsed.
      nowIso: () => '2026-06-27T17:00:00.000Z',
    })
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_network_stats')

    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('BEHIND PACE')
    expect(result).toContain('All-time tokens served: 5,000,000,000')
    // The JSON snapshot carries the structured pace block.
    expect(result).toContain('"behindPace":true')
    expect(result).toContain('"paceProjection":200000000')
    expect(result).toContain('"yesterdayTokens":328100000')
  })

  test('uses the loadStats override (D1 path) instead of HTTP when provided', async () => {
    let httpCalled = false
    const httpFetch = (async () => {
      httpCalled = true
      return new Response('{}', { status: 200 })
    }) as typeof fetch
    const tool = makeArtanisGetNetworkStatsTool({
      fetchImpl: httpFetch,
      loadStats: async () => ({
        allTimeTokensServed: 777,
        generatedAt: '2026-06-27T17:00:00.000Z',
        history: [{ day: '2026-06-27', tokensServed: 50 }],
        modelMix: [],
        pace: {
          behindPace: true,
          day: '2026-06-27',
          fractionOfCentralDayElapsed: 0.5,
          gapToTarget4x: 999,
          paceProjection: 100,
          target10x: 1000,
          target4x: 400,
          todayTokens: 50,
          yesterdayTokens: 100,
        },
        timezone: 'America/Chicago',
        todayTokens: 50,
      }),
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(httpCalled).toBe(false)
    expect(result).toContain('All-time tokens served: 777')
    expect(result).toContain('BEHIND PACE')
  })

  test('degrades to an honest message when the stats endpoints are unreachable', async () => {
    const failing = (async () => {
      throw new Error('network down')
    }) as typeof fetch
    const tool = makeArtanisGetNetworkStatsTool({
      fetchImpl: failing,
      nowIso: () => '2026-06-27T17:00:00.000Z',
    })
    const result = await Effect.runPromise(tool.execute({}))
    // Fail-soft: zeroed all-time + an empty-ish pace, never a thrown turn.
    expect(result).toContain('All-time tokens served: 0')
  })
})

describe('makeArtanisOperatorTools default table', () => {
  test('includes the repo-read tools, the issue-read tool, and the dispatch tool', () => {
    const tools = makeArtanisOperatorTools()
    const names = tools.map(tool => tool.definition.name).sort()
    expect(names).toEqual([
      'dispatch_codex_task',
      'get_network_stats',
      'list_repo_dir',
      'read_github_issue',
      'read_repo_file',
    ])
  })

  test('a shared repoRead fetch stub also drives the issue-read tool', async () => {
    const { fetchImpl, urls } = stubFetch(
      () =>
        new Response(
          JSON.stringify({
            body: 'b',
            comments: 0,
            state: 'open',
            title: 't',
          }),
          { status: 200 },
        ),
    )
    const tools = makeArtanisOperatorTools({ repoRead: { fetchImpl } })
    const issueTool = tools.find(
      tool => tool.definition.name === 'read_github_issue',
    )
    expect(issueTool).toBeDefined()
    if (issueTool && issueTool.kind === 'read') {
      const result = await Effect.runPromise(
        issueTool.execute({ issue_number: 6311 }),
      )
      expect(result).toContain('Issue #6311: t')
      expect(urls[0]).toBe(
        'https://api.github.com/repos/OpenAgentsInc/openagents/issues/6311',
      )
    }
  })
})
