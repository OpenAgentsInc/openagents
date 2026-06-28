import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_ISSUE_BODY_MAX_CHARS,
  ARTANIS_REPO_READ_MAX_BYTES,
  ARTANIS_SYNTHETIC_LOAD_MAX_TARGET_TOKENS,
  ARTANIS_SYNTHETIC_LOAD_RISKY_ACTION_KIND,
  ARTANIS_SYNTHETIC_LOAD_RUN_TYPES,
  type ArtanisGlmFleetStatus,
  type ArtanisKhalaFeedbackRecord,
  type ArtanisPylonAssignmentSummary,
  type ArtanisPylonJobStatus,
  type ArtanisSyntheticLoadRun,
  type ArtanisUnsupportedRequestRecord,
  buildArtanisSyntheticLoadPlan,
  isSafeArtanisAssignmentRef,
  isSafeArtanisRepoPath,
  makeArtanisDispatchCodexTaskTool,
  makeArtanisGetGlmFleetStatusTool,
  makeArtanisGetSyntheticLoadStatusTool,
  makeArtanisGetKhalaFeedbackTool,
  makeArtanisGetNetworkStatsTool,
  makeArtanisGetPylonJobStatusTool,
  makeArtanisGetTraceReviewTool,
  makeArtanisGetUnsupportedRequestsTool,
  makeArtanisListGithubIssuesTool,
  makeArtanisListPylonAssignmentsTool,
  makeArtanisListRepoDirTool,
  makeArtanisOperatorTools,
  makeArtanisPostForumUpdateTool,
  makeArtanisReadGithubIssueTool,
  makeArtanisReadRepoFileTool,
  makeArtanisTriggerSyntheticLoadTool,
  makeArtanisUpdateUnsupportedRequestTool,
  normalizeArtanisGlmFleetStatus,
  normalizeArtanisTraceReview,
  parseArtanisAssignmentRef,
  parseArtanisIssueNumber,
} from './artanis-operator-tools'
import {
  type ArtanisMemoryEntry,
  type ArtanisOperatorKhalaClient,
  type ArtanisSituationalAwareness,
  artanisOperatorTurn,
} from './artanis-operator'
import type {
  InferenceRequest,
  InferenceResult,
} from './inference/provider-adapter'
import { ARTANIS_RISKY_ACTION_KINDS } from './artanis-approval-gates'

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

describe('#6435 post_forum_update (gated Forum topic/reply writer)', () => {
  test('with NO execution seam it DEFERS and returns the public-safe topic plan', async () => {
    const tool = makeArtanisPostForumUpdateTool()

    const result = await Effect.runPromise(
      tool.run({
        action: 'create_topic',
        bodyText: 'I cleared the first verification pass and am moving to closeout.',
        forumSlug: 'artanis',
        title: 'Khala burndown status',
      }),
    )

    expect(tool.kind).toBe('gated')
    expect(tool.riskyActionKind).toBe('forum_post')
    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('execution_not_wired')
    expect(result.plan).toContain('POST /api/forum/forums/artanis/topics')
    expect(result.plan).toContain('Idempotency-Key: artanis-forum-create_topic-')
  })

  test('NO owner approval -> DEFERS without calling the Forum writer seam', async () => {
    const postCalls: Array<unknown> = []
    const tool = makeArtanisPostForumUpdateTool({
      execution: {
        isOwnerApproved: () => Effect.succeed(false),
        postForumUpdate: plan => {
          postCalls.push(plan)
          return Effect.succeed({
            idempotent: false,
            kind: 'created',
            postId: 'post.should_not_happen',
            publicUrl: '/forum/t/topic#post-post.should_not_happen',
            topicId: 'topic.should_not_happen',
          } as const)
        },
      },
    })

    const result = await Effect.runPromise(
      tool.run({
        action: 'reply',
        bodyText: 'Public-safe update: the delegated job is verified.',
        topicId: '88888888-4001-4001-8001-888888888888',
      }),
    )

    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('no_effective_owner_approval')
    expect(result.plan).toContain(
      'POST /api/forum/topics/88888888-4001-4001-8001-888888888888/posts',
    )
    expect(postCalls).toHaveLength(0)
  })

  test('owner-approved topic create returns the real Forum refs', async () => {
    const postCalls: Array<unknown> = []
    const tool = makeArtanisPostForumUpdateTool({
      execution: {
        isOwnerApproved: () => Effect.succeed(true),
        postForumUpdate: plan => {
          postCalls.push(plan)
          return Effect.succeed({
            idempotent: false,
            kind: 'created',
            postId: 'post.public.artanis.001',
            publicUrl: '/forum/t/topic.public.artanis.001#post-post.public.artanis.001',
            topicId: 'topic.public.artanis.001',
          } as const)
        },
      },
    })

    const result = await Effect.runPromise(
      tool.run({
        action: 'create_topic',
        bodyText: 'I opened a clean public status thread for the current burndown.',
        forumSlug: 'artanis',
        idempotencyKey: 'artanis-forum-status-thread-001',
        title: 'Artanis burndown status',
      }),
    )

    expect(result.outcome).toBe('executed')
    if (result.outcome !== 'executed') return
    expect(result.assignmentRef).toBe('forum.post.post.public.artanis.001')
    expect(result.summary).toContain('forumTopicId: topic.public.artanis.001')
    expect(result.summary).toContain('forumPostId: post.public.artanis.001')
    expect(postCalls).toEqual([
      {
        action: 'create_topic',
        bodyText:
          'I opened a clean public status thread for the current burndown.',
        forumSlug: 'artanis',
        idempotencyKey: 'artanis-forum-status-thread-001',
        title: 'Artanis burndown status',
        topicId: undefined,
      },
    ])
  })

  test('blocks unsafe body text before approval or execution', async () => {
    const approvalCalls: Array<unknown> = []
    const postCalls: Array<unknown> = []
    const tool = makeArtanisPostForumUpdateTool({
      execution: {
        isOwnerApproved: () => {
          approvalCalls.push(true)
          return Effect.succeed(true)
        },
        postForumUpdate: plan => {
          postCalls.push(plan)
          return Effect.succeed({
            idempotent: false,
            kind: 'created',
            postId: 'post.should_not_happen',
            publicUrl: '/forum/t/topic#post-post.should_not_happen',
            topicId: 'topic.should_not_happen',
          } as const)
        },
      },
    })

    const result = await Effect.runPromise(
      tool.run({
        action: 'reply',
        bodyText: 'Here is the bearer token sk-abc123 for the run.',
        topicId: '88888888-4001-4001-8001-888888888888',
      }),
    )

    expect(result.outcome).toBe('deferred')
    if (result.outcome !== 'deferred') return
    expect(result.reason).toBe('invalid_arguments')
    expect(result.plan).toContain('blocked')
    expect(approvalCalls).toHaveLength(0)
    expect(postCalls).toHaveLength(0)
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

describe('list_github_issues (public OpenAgentsInc/openagents only)', () => {
  // A fake GitHub issues payload mixing real issues (one Khala-labeled) with a
  // pull request (carries `pull_request`) that MUST be filtered out.
  const issuesPayload = JSON.stringify([
    {
      labels: [{ name: 'khala' }, { name: 'bug' }],
      number: 6401,
      pull_request: undefined,
      state: 'open',
      title: 'Khala CLI hangs on /feedback submit',
    },
    {
      labels: ['khala'],
      number: 6402,
      state: 'open',
      title: 'Khala token counter lags behind exact rows',
    },
    {
      // A pull request — has a `pull_request` object; must NOT appear.
      labels: [{ name: 'khala' }],
      number: 6403,
      pull_request: { url: 'https://api.github.com/.../pulls/6403' },
      state: 'open',
      title: 'PR: wire khala counter feed',
    },
  ])

  test('formats open Khala issues (numbers+titles+state), filters out PRs, bounds the count', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response(issuesPayload, { status: 200 }),
    )
    // maxLimit:2 also proves the count is bounded even before PR filtering.
    const tool = makeArtanisListGithubIssuesTool({ fetchImpl, maxLimit: 2 })
    const result = await Effect.runPromise(
      tool.execute({ labels: 'khala', state: 'open' }),
    )

    // Numbers + titles + state for the two real issues.
    expect(result).toContain('#6401')
    expect(result).toContain('Khala CLI hangs on /feedback submit')
    expect(result).toContain('[open]')
    expect(result).toContain('#6402')
    expect(result).toContain('Khala token counter lags behind exact rows')
    // The pull request is filtered out.
    expect(result).not.toContain('#6403')
    expect(result).not.toContain('PR: wire khala counter feed')
    // Labels are surfaced.
    expect(result).toContain('labels: khala, bug')

    // The request hit the public issues API with the right state/label filters
    // and a bounded per_page (clamped to maxLimit:2).
    expect(urls[0]).toContain(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues?',
    )
    expect(urls[0]).toContain('state=open')
    expect(urls[0]).toContain('labels=khala')
    expect(urls[0]).toContain('per_page=2')
  })

  test('defaults to open state and bounds the count to the limit', async () => {
    const many = JSON.stringify(
      Array.from({ length: 10 }, (_value, index) => ({
        labels: [],
        number: 7000 + index,
        state: 'open',
        title: `issue ${index}`,
      })),
    )
    const { fetchImpl, urls } = stubFetch(
      () => new Response(many, { status: 200 }),
    )
    const tool = makeArtanisListGithubIssuesTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ limit: 3 }))
    expect(urls[0]).toContain('state=open')
    const renderedNumbers = result
      .split('\n')
      .filter(line => line.startsWith('- #'))
    expect(renderedNumbers).toHaveLength(3)
  })

  test('an empty result reads as honest "(no open issues …)" not invention', async () => {
    const { fetchImpl } = stubFetch(
      () => new Response('[]', { status: 200 }),
    )
    const tool = makeArtanisListGithubIssuesTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ labels: 'nonexistent' }),
    )
    expect(result).toContain('no open issues found')
    expect(result).toContain('OpenAgentsInc/openagents')
  })

  test('a fetch failure reads as honest "(could not list issues)"', async () => {
    const fetchImpl = (async () => {
      throw new Error('network down')
    }) as typeof fetch
    const tool = makeArtanisListGithubIssuesTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toBe('(could not list issues)')
  })

  test('the default tool table exposes list_github_issues as a read tool', () => {
    const tools = makeArtanisOperatorTools()
    const listIssues = tools.find(
      tool => tool.definition.name === 'list_github_issues',
    )
    expect(listIssues).toBeDefined()
    expect(listIssues?.kind).toBe('read')
  })
})

// A known public-safe status fixture for the injected reader.
const passingStatus: ArtanisPylonJobStatus = {
  artifactRefs: ['artifact.public.pylon_assignment.codex.001'],
  assignmentRef: 'assignment.public.pylon_api.known_001',
  blockerRefs: [],
  closeoutRefs: ['closeout.public.pylon_assignment.001'],
  closeoutSubmitted: true,
  failureSummary: null,
  jobKind: 'codex_agent_task',
  leaseState: 'terminal',
  proofObserved: true,
  proofRefs: ['proof.public.pylon_assignment.001'],
  rejectionRefs: [],
  state: 'closeout_submitted',
  updatedAt: 'a few minutes ago',
  verifyResult: 'pass',
}

describe('get_pylon_job_status (owner-scoped Pylon job status read)', () => {
  test('resolves a known assignmentRef to a public-safe PASS status from the injected reader', async () => {
    const seen: Array<string> = []
    const tool = makeArtanisGetPylonJobStatusTool({
      reader: async ref => {
        seen.push(ref)
        return ref === passingStatus.assignmentRef ? passingStatus : null
      },
    })

    const result = await Effect.runPromise(
      tool.execute({ assignmentRef: passingStatus.assignmentRef }),
    )

    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(seen).toEqual([passingStatus.assignmentRef])
    expect(result).toContain(
      `Pylon job status for ${passingStatus.assignmentRef}`,
    )
    expect(result).toContain('Job kind: codex_agent_task')
    expect(result).toContain('State: closeout_submitted (lease: terminal)')
    expect(result).toContain('Closeout: submitted')
    expect(result).toContain('Proof observed: yes')
    expect(result).toContain('PASS')
    expect(result).toContain('proof.public.pylon_assignment.001')
    // A passing job has no failure summary line.
    expect(result).not.toContain('Failure summary')
  })

  test('renders a redacted failure summary for a FAIL closeout', async () => {
    const tool = makeArtanisGetPylonJobStatusTool({
      reader: async () => ({
        ...passingStatus,
        blockerRefs: ['blocker.public.pylon_assignment.verify_failed'],
        failureSummary:
          'rejected (rejection.public.pylon_assignment.verify_failed)',
        proofObserved: true,
        rejectionRefs: ['rejection.public.pylon_assignment.verify_failed'],
        verifyResult: 'fail',
      }),
    })
    const result = await Effect.runPromise(
      tool.execute({ assignmentRef: passingStatus.assignmentRef }),
    )
    expect(result).toContain('FAIL')
    expect(result).toContain('Failure summary:')
    expect(result).toContain('verify_failed')
  })

  test('an unknown/other-owner assignment reads as honest "(no assignment found …)"', async () => {
    const tool = makeArtanisGetPylonJobStatusTool({ reader: async () => null })
    const result = await Effect.runPromise(
      tool.execute({ assignmentRef: 'assignment.public.pylon_api.missing' }),
    )
    expect(result).toContain('no assignment found')
  })

  test('blocks an unsafe assignment ref and never calls the reader', async () => {
    let called = false
    const tool = makeArtanisGetPylonJobStatusTool({
      reader: async () => {
        called = true
        return passingStatus
      },
    })
    for (const ref of [
      '../../etc/passwd',
      'assignment with spaces',
      'assignment.with.bearer.token',
      'sk-secretmaterial',
    ]) {
      const result = await Effect.runPromise(tool.execute({ assignmentRef: ref }))
      expect(result).toContain('blocked')
    }
    expect(called).toBe(false)
  })

  test('absent argument returns an honest "invalid arguments" message', async () => {
    const tool = makeArtanisGetPylonJobStatusTool({ reader: async () => null })
    const result = await Effect.runPromise(tool.execute({ notARef: 1 }))
    expect(result).toContain('invalid arguments')
  })

  test('a reader rejection reads as a soft "(could not read status …)", never a throw', async () => {
    const tool = makeArtanisGetPylonJobStatusTool({
      reader: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(
      tool.execute({ assignmentRef: 'assignment.public.pylon_api.x' }),
    )
    expect(result).toContain('could not read status')
    expect(result).not.toMatch(/Error:/)
  })

  test('with no reader wired the tool is honest, not inventive', async () => {
    const tool = makeArtanisGetPylonJobStatusTool({})
    const result = await Effect.runPromise(
      tool.execute({ assignmentRef: 'assignment.public.pylon_api.x' }),
    )
    expect(result).toContain('could not read status')
  })
})

// A bounded set of public-safe assignment summaries in varied states, as the
// injected lister would return for the owner's recent burndown.
const assignmentRows: ReadonlyArray<ArtanisPylonAssignmentSummary> = [
  {
    assignmentRef: 'assignment.public.pylon_api.accepted_001',
    jobKind: 'codex_agent_task',
    leaseState: 'active',
    phase: 'accepted',
    state: 'accepted',
    updatedAt: 'a few minutes ago',
    verifyResult: 'unknown',
  },
  {
    assignmentRef: 'assignment.public.pylon_api.proofready_002',
    jobKind: 'codex_agent_task',
    leaseState: 'active',
    phase: 'proof-ready',
    state: 'proof_submitted',
    updatedAt: 'a few minutes ago',
    verifyResult: 'unknown',
  },
  {
    assignmentRef: 'assignment.public.pylon_api.closeout_003',
    jobKind: 'codex_agent_task',
    leaseState: 'terminal',
    phase: 'closeout_submitted',
    state: 'closeout_submitted',
    updatedAt: 'an hour ago',
    verifyResult: 'pass',
  },
  {
    assignmentRef: 'assignment.public.pylon_api.rejected_004',
    jobKind: 'codex_agent_task',
    leaseState: 'terminal',
    phase: 'rejected',
    state: 'rejected',
    updatedAt: 'an hour ago',
    verifyResult: 'fail',
  },
]

describe('list_pylon_assignments (owner-scoped bulk assignment list) — iteration 5', () => {
  test('returns a bounded public-safe summary line per assignment (ref + state + phase)', async () => {
    let askedLimit: number | undefined
    const tool = makeArtanisListPylonAssignmentsTool({
      lister: async limit => {
        askedLimit = limit
        return assignmentRows
      },
    })

    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('list_pylon_assignments')

    const result = await Effect.runPromise(tool.execute({}))

    // One bounded summary line per assignment, each carrying ref + state + phase.
    for (const row of assignmentRows) {
      expect(result).toContain(row.assignmentRef)
      expect(result).toContain(`state=${row.state}`)
      expect(result).toContain(`phase=${row.phase}`)
    }
    // The varied verify verdicts surface as short labels.
    expect(result).toContain('verify=PASS')
    expect(result).toContain('verify=FAIL')
    expect(result).toContain('verify=in-progress')
    // Header reports the bounded count.
    expect(result).toContain(
      `Recent Pylon/Codex assignments (${assignmentRows.length}):`,
    )
    // Default bounded limit was passed to the lister.
    expect(askedLimit).toBe(25)
    // No secrets/prompts: nothing private leaked into the bulk summary.
    expect(result).not.toMatch(/sk-/)
    expect(result).not.toContain('bearer')
    expect(result).not.toContain('prompt')
  })

  test('an optional state filter narrows to one lifecycle state', async () => {
    const tool = makeArtanisListPylonAssignmentsTool({
      lister: async () => assignmentRows,
    })
    const result = await Effect.runPromise(
      tool.execute({ state: 'rejected' }),
    )
    expect(result).toContain('assignment.public.pylon_api.rejected_004')
    expect(result).not.toContain('assignment.public.pylon_api.accepted_001')
    expect(result).toContain('state "rejected"')
  })

  test('a limit arg is clamped to the bounded max and floored at 1', async () => {
    const limits: Array<number> = []
    const tool = makeArtanisListPylonAssignmentsTool({
      lister: async limit => {
        limits.push(limit)
        return assignmentRows.slice(0, limit)
      },
    })
    await Effect.runPromise(tool.execute({ limit: 100000 }))
    await Effect.runPromise(tool.execute({ limit: 0 }))
    await Effect.runPromise(tool.execute({ limit: 2 }))
    // clamped to max (100), floored to default (25) for <1, honored otherwise.
    expect(limits).toEqual([100, 25, 2])
  })

  test('defensively drops a row carrying non-public-safe material', async () => {
    const tool = makeArtanisListPylonAssignmentsTool({
      lister: async () => [
        assignmentRows[0]!,
        {
          ...assignmentRows[0]!,
          assignmentRef: 'assignment.public.pylon_api.leak',
          // A regression upstream tries to leak a secret-bearing state string.
          state: 'state-with-bearer-token',
        },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('assignment.public.pylon_api.accepted_001')
    expect(result).not.toContain('bearer')
    expect(result).not.toContain('assignment.public.pylon_api.leak')
  })

  test('an owner with no assignments reads as honest absence, never invention', async () => {
    const tool = makeArtanisListPylonAssignmentsTool({ lister: async () => [] })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('no recent Pylon assignments found')
  })

  test('a lister rejection reads as a soft "(could not list assignments)", never a throw', async () => {
    const tool = makeArtanisListPylonAssignmentsTool({
      lister: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not list assignments')
    expect(result).not.toMatch(/Error:/)
  })

  test('with no lister wired the tool is honest, not inventive', async () => {
    const tool = makeArtanisListPylonAssignmentsTool({})
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not list assignments')
  })
})

// Real-shaped Khala CLI feedback rows (newest-first, as the store's listRecent
// returns them). Varied source + clientVersion; one long body to exercise the
// truncation path.
const feedbackRows: ReadonlyArray<ArtanisKhalaFeedbackRecord> = [
  {
    clientVersion: '0.4.2',
    createdAt: '2026-06-27T11:00:00.000Z',
    feedback: 'too wordy, prefer more conversational',
    feedbackRef: 'khala_feedback:fb_aaa111',
    source: 'khala-cli',
  },
  {
    clientVersion: null,
    createdAt: '2026-06-27T10:30:00.000Z',
    feedback: 'wish it could read my local git diff before answering',
    feedbackRef: 'khala_feedback:fb_bbb222',
    source: 'khala-cli',
  },
  {
    clientVersion: '0.4.1',
    createdAt: '2026-06-27T09:15:00.000Z',
    feedback: 'x'.repeat(2_000),
    feedbackRef: 'khala_feedback:fb_ccc333',
    source: 'khala-web',
  },
]

describe('get_khala_feedback (owner-scoped Khala CLI feedback read) — iteration 6', () => {
  test('returns a bounded public-safe entry per feedback record (ref + source + body)', async () => {
    let askedLimit: number | undefined
    const tool = makeArtanisGetKhalaFeedbackTool({
      reader: async limit => {
        askedLimit = limit
        return feedbackRows
      },
    })

    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_khala_feedback')

    const result = await Effect.runPromise(tool.execute({}))

    // The actual feedback body the owner needs to read is surfaced verbatim.
    expect(result).toContain('too wordy, prefer more conversational')
    expect(result).toContain('wish it could read my local git diff')
    // Each record carries its ref + source.
    for (const row of feedbackRows) {
      expect(result).toContain(row.feedbackRef)
    }
    expect(result).toContain('(khala-cli v0.4.2)')
    expect(result).toContain('(khala-web v0.4.1)')
    // Header reports the bounded count.
    expect(result).toContain(`Recent Khala CLI feedback (${feedbackRows.length}):`)
    // Default bounded limit (10) was passed to the reader.
    expect(askedLimit).toBe(10)
  })

  test('a long feedback body is truncated with an explicit marker', async () => {
    const tool = makeArtanisGetKhalaFeedbackTool({
      maxTextChars: 50,
      reader: async () => feedbackRows,
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('...[truncated]')
    // The raw 2,000-char body never lands whole in context.
    expect(result).not.toContain('x'.repeat(100))
  })

  test('a limit arg is clamped to the bounded max and floored at the default', async () => {
    const limits: Array<number> = []
    const tool = makeArtanisGetKhalaFeedbackTool({
      reader: async limit => {
        limits.push(limit)
        return feedbackRows.slice(0, limit)
      },
    })
    await Effect.runPromise(tool.execute({ limit: 100000 }))
    await Effect.runPromise(tool.execute({ limit: 0 }))
    await Effect.runPromise(tool.execute({ limit: 2 }))
    // clamped to max (50), floored to default (10) for <1, honored otherwise.
    expect(limits).toEqual([50, 10, 2])
  })

  test('defensively drops a row whose structured ref/source leaks unsafe material', async () => {
    const tool = makeArtanisGetKhalaFeedbackTool({
      reader: async () => [
        feedbackRows[0]!,
        {
          ...feedbackRows[0]!,
          feedback: 'fine body',
          // A regression upstream tries to leak a secret into the ref field.
          feedbackRef: 'khala_feedback:bearer_token_leak',
        },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('khala_feedback:fb_aaa111')
    expect(result).not.toContain('bearer')
  })

  test('an empty store reads as honest absence, never invention', async () => {
    const tool = makeArtanisGetKhalaFeedbackTool({ reader: async () => [] })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('no recent Khala CLI feedback found')
  })

  test('a reader rejection reads as a soft "(could not read feedback)", never a throw', async () => {
    const tool = makeArtanisGetKhalaFeedbackTool({
      reader: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not read feedback')
    expect(result).not.toMatch(/Error:/)
  })

  test('with no reader wired the tool is honest, not inventive', async () => {
    const tool = makeArtanisGetKhalaFeedbackTool({})
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not read feedback')
  })
})

// Two real-shaped unsupported-request ledger entries: capability gaps with
// status needs_issue, each with a distinct public-safe title + summary.
const unsupportedRows: ReadonlyArray<ArtanisUnsupportedRequestRecord> = [
  {
    githubIssueRef: null,
    nextAction: 'open_github_issue',
    requestRef: 'khala_unsupported:ur_aaa111',
    sourceKind: 'trace_review',
    status: 'needs_issue',
    summary:
      'Users repeatedly ask Khala to read their local git diff before answering and it cannot.',
    title: 'Khala cannot read the local git diff',
    triageKind: 'missing_capability',
    updatedAt: '2026-06-27T11:00:00.000Z',
  },
  {
    githubIssueRef: null,
    nextAction: 'open_github_issue',
    requestRef: 'khala_unsupported:ur_bbb222',
    sourceKind: 'forum',
    status: 'needs_issue',
    summary:
      'Testers want Khala to attach uploaded PDFs to a chat turn for extraction.',
    title: 'Khala cannot ingest uploaded PDF attachments',
    triageKind: 'missing_capability',
    updatedAt: '2026-06-27T10:30:00.000Z',
  },
]

describe('get_unsupported_requests (owner-scoped unsupported-request ledger read) — iteration 8', () => {
  test('returns a bounded public-safe entry per ledger record (ref + triage/status + title + summary)', async () => {
    let asked: { limit: number; status?: string | undefined } | undefined
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async input => {
        asked = input
        return unsupportedRows
      },
    })

    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_unsupported_requests')

    const result = await Effect.runPromise(tool.execute({}))

    // Both gap titles + summaries are surfaced.
    expect(result).toContain('Khala cannot read the local git diff')
    expect(result).toContain('read their local git diff')
    expect(result).toContain('Khala cannot ingest uploaded PDF attachments')
    expect(result).toContain('attach uploaded PDFs')
    // Each record carries its ref + triage/status.
    for (const row of unsupportedRows) {
      expect(result).toContain(row.requestRef)
    }
    expect(result).toContain('missing_capability/needs_issue')
    // Header reports the bounded count; default bounded limit (25) was passed.
    expect(result).toContain(`Unsupported-request ledger (${unsupportedRows.length}):`)
    expect(asked).toEqual({ limit: 25, status: undefined })
  })

  test('passes a bounded status filter through to the reader', async () => {
    let asked: { limit: number; status?: string | undefined } | undefined
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async input => {
        asked = input
        return unsupportedRows
      },
    })
    const result = await Effect.runPromise(
      tool.execute({ status: 'needs_issue' }),
    )
    expect(asked).toEqual({ limit: 25, status: 'needs_issue' })
    expect(result).toContain(
      `Unsupported-request ledger with status "needs_issue" (${unsupportedRows.length}):`,
    )
  })

  test('an unknown status filter is ignored (no filter passed)', async () => {
    let asked: { limit: number; status?: string | undefined } | undefined
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async input => {
        asked = input
        return unsupportedRows
      },
    })
    await Effect.runPromise(tool.execute({ status: 'banana' }))
    expect(asked).toEqual({ limit: 25, status: undefined })
  })

  test('a limit arg is clamped to the bounded max and floored at the default', async () => {
    const limits: Array<number> = []
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async input => {
        limits.push(input.limit)
        return unsupportedRows.slice(0, input.limit)
      },
    })
    await Effect.runPromise(tool.execute({ limit: 100000 }))
    await Effect.runPromise(tool.execute({ limit: 0 }))
    await Effect.runPromise(tool.execute({ limit: 2 }))
    // clamped to max (100), floored to default (25) for <1, honored otherwise.
    expect(limits).toEqual([100, 25, 2])
  })

  test('a long summary is truncated with an explicit marker', async () => {
    const tool = makeArtanisGetUnsupportedRequestsTool({
      maxSummaryChars: 40,
      reader: async () => [
        { ...unsupportedRows[0]!, summary: 'y'.repeat(2_000) },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('...[truncated]')
    expect(result).not.toContain('y'.repeat(100))
  })

  test('defensively drops a row whose structured ref leaks unsafe material', async () => {
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async () => [
        unsupportedRows[0]!,
        {
          ...unsupportedRows[1]!,
          // A regression upstream tries to leak a secret into the ref field.
          requestRef: 'khala_unsupported:bearer_token_leak',
        },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('khala_unsupported:ur_aaa111')
    expect(result).not.toContain('bearer')
  })

  test('an empty ledger reads as honest absence, never invention', async () => {
    const tool = makeArtanisGetUnsupportedRequestsTool({ reader: async () => [] })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('no unsupported requests found in the ledger')
  })

  test('a reader rejection reads as a soft failure, never a throw', async () => {
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not read unsupported requests')
    expect(result).not.toMatch(/Error:/)
  })

  test('with no reader wired the tool is honest, not inventive', async () => {
    const tool = makeArtanisGetUnsupportedRequestsTool({})
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not read unsupported requests')
  })

  // ACCEPTANCE: a full artanisOperatorTurn where the model requests
  // get_unsupported_requests. The tool executes (read, executed, not deferred),
  // both gap summaries round-trip into the conversation, and nothing leaks.
  test('a full operator turn: the model requests get_unsupported_requests and it executes, returning both gaps', async () => {
    const exampleMemory: ReadonlyArray<ArtanisMemoryEntry> = [
      {
        body: 'Owner prefers concise direct answers, no marketing copy.',
        createdAt: '2026-06-26T09:00:00.000Z',
        kind: 'note',
        memoryRef: 'mem-0',
        noteCategory: 'preference',
        ownerId: 'owner:github:14167547',
        role: null,
      },
    ]
    const exampleAwareness: ArtanisSituationalAwareness = {
      generatedAt: '2026-06-27T12:00:00.000Z',
      goals: {
        epics: [
          {
            mandate: 'Own the Khala improvement loop autonomously.',
            number: 6359,
            title: 'Artanis: autonomous owner of the loop',
          },
        ],
        roadmapRef: 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
        roadmapSummary: 'Master roadmap for the open Khala issue set.',
      },
      kind: 'artanis_situational_awareness',
      ongoingOps: {
        activeAssignments: [],
        fleetReadiness: { readyReplicas: 3, status: 'ready', totalReplicas: 3 },
        publicCounter: null,
        recentDeploys: [],
        tokenPace: null,
      },
      ownerId: 'owner:github:14167547',
      ownerOnly: true,
      recentActions: {
        assignments: [],
        commits: [],
        issueChanges: [],
        ticks: [],
      },
    }

    const toolCallResult = (name: string, args: string): InferenceResult => ({
      content: '',
      finishReason: 'tool_calls',
      servedModel: 'gpt-oss-120b',
      toolCalls: [
        {
          function: { arguments: args, name },
          id: `call_${name}`,
          type: 'function',
        },
      ],
      usage: { completionTokens: 4, promptTokens: 100, totalTokens: 104 },
    })
    const textResult = (content: string): InferenceResult => ({
      content,
      finishReason: 'stop',
      servedModel: 'gpt-oss-120b',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    })

    const script: ReadonlyArray<InferenceResult> = [
      toolCallResult('get_unsupported_requests', '{"status":"needs_issue"}'),
      textResult(
        'Two needs_issue gaps: local git diff reads and PDF attachment ingest.',
      ),
    ]
    const requests: Array<InferenceRequest> = []
    let index = 0
    const khalaClient: ArtanisOperatorKhalaClient = (
      request: InferenceRequest,
    ) => {
      requests.push(request)
      const result = script[index] ?? script[script.length - 1]!
      index += 1
      return Effect.succeed(result)
    }

    let askedStatus: string | undefined
    const tool = makeArtanisGetUnsupportedRequestsTool({
      reader: async input => {
        askedStatus = input.status
        return unsupportedRows
      },
    })

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient,
        memory: exampleMemory,
        messages: [
          {
            content: 'What capability gaps are blocking Khala adoption right now?',
            role: 'user',
          },
        ],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return

    // The read tool was actually executed with the model's status filter, and
    // it executed freely (not deferred to the approval gate).
    expect(askedStatus).toBe('needs_issue')
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: false,
        executed: true,
        executedRef: null,
        name: 'get_unsupported_requests',
        riskyActionKind: null,
      },
    ])

    // The tool result round-tripped back into the second Khala call, carrying
    // BOTH gap summaries.
    const secondConversation = requests[1]?.messages ?? []
    const toolMessage = secondConversation.find(
      message =>
        message.role === 'tool' &&
        message.content.includes('Unsupported-request ledger'),
    )
    expect(toolMessage).toBeDefined()
    const toolText = toolMessage?.content ?? ''
    expect(toolText).toContain('Khala cannot read the local git diff')
    expect(toolText).toContain('read their local git diff')
    expect(toolText).toContain('Khala cannot ingest uploaded PDF attachments')
    expect(toolText).toContain('attach uploaded PDFs')
    expect(toolText).toContain('khala_unsupported:ur_aaa111')
    expect(toolText).toContain('khala_unsupported:ur_bbb222')

    // No secrets / raw private content leaked into the tool output.
    expect(toolText).not.toMatch(
      /bearer|mnemonic|wallet|secret|sk-[a-z0-9]|\/Users\/|access[_-]?token/i,
    )
  })
})

describe('update_unsupported_request (owner-scoped ledger triage WRITE) — iteration 9', () => {
  // The existing ledger entry the fake writer triages. The writer echoes the
  // merged change back as the updated record, mirroring the real store.
  const baseRow: ArtanisUnsupportedRequestRecord = {
    githubIssueRef: null,
    nextAction: 'open_github_issue',
    requestRef: 'gap_987',
    sourceKind: 'trace_review',
    status: 'needs_issue',
    summary: 'Users want Khala to read their local git diff before answering.',
    title: 'Khala cannot read the local git diff',
    triageKind: 'missing_capability',
    updatedAt: '2026-06-27T11:00:00.000Z',
  }

  // A fake writer that resolves ONLY the known ref, applies the merged change,
  // and recomputes nextAction the way the real ledger does (issue_opened ->
  // 'none'). Any other ref is honest absence (null).
  const makeFakeWriter = () => {
    const calls: Array<unknown> = []
    const writer = async (
      update: import('./artanis-operator-tools').ArtanisUnsupportedRequestUpdate,
    ): Promise<ArtanisUnsupportedRequestRecord | null> => {
      calls.push(update)
      if (update.ref !== baseRow.requestRef) return null
      const status = update.status ?? baseRow.status
      const githubIssueRef = update.githubIssueRef ?? baseRow.githubIssueRef
      const triageKind = update.triageKind ?? baseRow.triageKind
      const nextAction =
        status === 'issue_opened' || status === 'closed' || status === 'wont_do'
          ? ('none' as const)
          : baseRow.nextAction
      return {
        ...baseRow,
        githubIssueRef,
        nextAction,
        status,
        triageKind,
        updatedAt: '2026-06-27T12:00:00.000Z',
      }
    }
    return { calls, writer }
  }

  test('it is a WRITE tool (executes freely; not gated/risky)', () => {
    const tool = makeArtanisUpdateUnsupportedRequestTool({})
    expect(tool.kind).toBe('write')
    expect(tool.definition.name).toBe('update_unsupported_request')
  })

  test('triages a known entry to issue_opened and links the issue, returning the updated record', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(
      tool.execute({ issue: 6310, ref: 'gap_987', status: 'issue_opened' }),
    )
    // The writer received the validated, normalized update.
    expect(calls).toEqual([
      {
        githubIssueRef: 'OpenAgentsInc/openagents#6310',
        ref: 'gap_987',
        status: 'issue_opened',
        triageKind: undefined,
      },
    ])
    expect(result).toContain('Updated unsupported request gap_987')
    expect(result).toContain('status: issue_opened')
    expect(result).toContain('linked issue: OpenAgentsInc/openagents#6310')
    expect(result).toContain('next action: none')
  })

  test('validates the ref: absent reads invalid arguments; unsafe reads blocked', async () => {
    const { writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    expect(
      await Effect.runPromise(tool.execute({ status: 'closed' })),
    ).toContain('invalid arguments')
    const blocked = await Effect.runPromise(
      tool.execute({ ref: '../../etc/passwd', status: 'closed' }),
    )
    expect(blocked).toContain('blocked')
    expect(blocked).not.toMatch(/Error:/)
  })

  test('rejects an unknown status value (blocked, never coerced)', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(
      tool.execute({ ref: 'gap_987', status: 'banana' }),
    )
    expect(result).toContain('blocked')
    expect(result).toContain('not a valid status')
    // It never reached the writer.
    expect(calls).toEqual([])
  })

  test('rejects an unknown triage kind (blocked, never coerced)', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(
      tool.execute({ ref: 'gap_987', triageKind: 'whatever' }),
    )
    expect(result).toContain('blocked')
    expect(result).toContain('not a valid triage kind')
    expect(calls).toEqual([])
  })

  test('rejects a non-public-safe linked-issue field (blocked)', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(
      tool.execute({
        githubIssueRef: 'token=bearer_secret_leak',
        ref: 'gap_987',
      }),
    )
    expect(result).toContain('blocked')
    expect(result).not.toContain('bearer')
    expect(calls).toEqual([])
  })

  test('requires at least one change field', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(tool.execute({ ref: 'gap_987' }))
    expect(result).toContain('at least one')
    expect(calls).toEqual([])
  })

  test('an unknown ref reads as honest "(not found …)", never invention', async () => {
    const { writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    const result = await Effect.runPromise(
      tool.execute({ ref: 'gap_does_not_exist', status: 'closed' }),
    )
    expect(result).toContain('not found')
    expect(result).toContain('gap_does_not_exist')
  })

  test('a writer rejection reads as a soft failure, never a throw', async () => {
    const tool = makeArtanisUpdateUnsupportedRequestTool({
      writer: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(
      tool.execute({ ref: 'gap_987', status: 'closed' }),
    )
    expect(result).toContain('could not update')
    expect(result).not.toMatch(/Error:/)
  })

  test('with no writer wired the tool is honest, not inventive', async () => {
    const tool = makeArtanisUpdateUnsupportedRequestTool({})
    const result = await Effect.runPromise(
      tool.execute({ ref: 'gap_987', status: 'closed' }),
    )
    expect(result).toContain('no ledger writer is wired')
  })

  test('accepts an explicit bare numeric githubIssueRef and normalizes it', async () => {
    const { calls, writer } = makeFakeWriter()
    const tool = makeArtanisUpdateUnsupportedRequestTool({ writer })
    await Effect.runPromise(
      tool.execute({ githubIssueRef: '6310', ref: 'gap_987' }),
    )
    expect(calls).toEqual([
      {
        githubIssueRef: 'OpenAgentsInc/openagents#6310',
        ref: 'gap_987',
        status: undefined,
        triageKind: undefined,
      },
    ])
  })
})

describe('parseArtanisAssignmentRef + isSafeArtanisAssignmentRef', () => {
  test('accepts a ref under several key aliases', () => {
    expect(parseArtanisAssignmentRef({ assignmentRef: 'a.b.c' })).toEqual({
      kind: 'ref',
      value: 'a.b.c',
    })
    expect(parseArtanisAssignmentRef({ assignment_ref: 'a.b.c' })).toEqual({
      kind: 'ref',
      value: 'a.b.c',
    })
    expect(parseArtanisAssignmentRef({ ref: 'a.b.c' })).toEqual({
      kind: 'ref',
      value: 'a.b.c',
    })
  })

  test('distinguishes absent from invalid', () => {
    expect(parseArtanisAssignmentRef({}).kind).toBe('absent')
    expect(parseArtanisAssignmentRef({ assignmentRef: '   ' }).kind).toBe(
      'absent',
    )
    expect(parseArtanisAssignmentRef({ assignmentRef: '../x' }).kind).toBe(
      'invalid',
    )
    expect(parseArtanisAssignmentRef({ assignmentRef: 42 }).kind).toBe('invalid')
  })

  test('isSafeArtanisAssignmentRef accepts real refs and rejects unsafe ones', () => {
    expect(isSafeArtanisAssignmentRef('assignment.public.pylon_api.abc_123')).toBe(
      true,
    )
    expect(isSafeArtanisAssignmentRef('artanis_dispatch:codex/main#1')).toBe(true)
    expect(isSafeArtanisAssignmentRef('')).toBe(false)
    expect(isSafeArtanisAssignmentRef('../../etc/passwd')).toBe(false)
    expect(isSafeArtanisAssignmentRef('has space')).toBe(false)
    expect(isSafeArtanisAssignmentRef('contains-secret-token')).toBe(false)
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

describe('trigger_synthetic_load (RISKY plan-only) — iteration 4', () => {
  const tool = makeArtanisTriggerSyntheticLoadTool()

  test('is a risky tool whose riskyActionKind is an enumerated kind', () => {
    expect(tool.kind).toBe('risky')
    expect(tool.definition.name).toBe('trigger_synthetic_load')
    // It carries a riskyActionKind, and that kind is part of the approval-gate
    // vocabulary (synthetic load maps to eval_launch).
    expect(tool.riskyActionKind).toBe(ARTANIS_SYNTHETIC_LOAD_RISKY_ACTION_KIND)
    expect(tool.riskyActionKind).toBe('eval_launch')
    expect(ARTANIS_RISKY_ACTION_KINDS).toContain(tool.riskyActionKind)
    // Structurally plan-only: no execute()/run() seam exists at all.
    expect(typeof tool.plan).toBe('function')
    expect('execute' in tool).toBe(false)
    expect('run' in tool).toBe(false)
  })

  test('plan() returns a bounded public-safe run description naming type and target', async () => {
    const plan = await Effect.runPromise(
      tool.plan({ targetTokens: 500_000_000, type: 'terminal-bench' }),
    )
    expect(plan).toContain('type=terminal-bench')
    expect(plan).toContain('target=500,000,000 tokens')
    // Public-safe framing: no spend authority, owner-gated.
    expect(plan).toContain('NO spend')
    expect(plan).toContain('owner-gated')
    // Bounded: a single short block, not an unbounded dump.
    expect(plan.length).toBeLessThan(1200)
  })

  test('out-of-range and negative targetTokens are rejected with a typed string', async () => {
    const tooBig = await Effect.runPromise(
      tool.plan({
        targetTokens: ARTANIS_SYNTHETIC_LOAD_MAX_TARGET_TOKENS + 1,
        type: 'glm-stress',
      }),
    )
    expect(tooBig).toContain('blocked')
    expect(tooBig).toContain('out of range')

    const negative = await Effect.runPromise(
      tool.plan({ targetTokens: -5, type: 'glm-stress' }),
    )
    expect(negative).toContain('blocked')

    const tooSmall = await Effect.runPromise(
      tool.plan({ targetTokens: 10, type: 'glm-stress' }),
    )
    expect(tooSmall).toContain('blocked')
  })

  test('a missing targetTokens reads as an honest invalid-arguments message', async () => {
    const result = await Effect.runPromise(
      tool.plan({ type: 'terminal-bench' }),
    )
    expect(result).toContain('invalid arguments')
  })

  test('an unknown run type is rejected with a typed blocked string', async () => {
    const unknown = await Effect.runPromise(
      tool.plan({ targetTokens: 500_000_000, type: 'mine-bitcoin' }),
    )
    expect(unknown).toContain('blocked')
    expect(unknown).toContain('not a known synthetic-load run type')

    const absentType = await Effect.runPromise(
      tool.plan({ targetTokens: 500_000_000 }),
    )
    expect(absentType).toContain('invalid arguments')
  })

  test('a non-public-safe note is redacted, never echoed into the plan', async () => {
    const plan = await Effect.runPromise(
      tool.plan({
        note: 'use the bearer token sk-abc123 to authorize',
        targetTokens: 500_000_000,
        type: 'terminal-bench',
      }),
    )
    expect(plan).toContain('(redacted)')
    expect(plan).not.toContain('sk-abc123')
    expect(plan).not.toContain('bearer')
  })

  test('a public-safe note is kept verbatim in the plan', async () => {
    const plan = await Effect.runPromise(
      tool.plan({
        note: 'behind the 4x floor at midday',
        targetTokens: 1_000_000_000,
        type: 'glm-stress',
      }),
    )
    expect(plan).toContain('behind the 4x floor at midday')
    expect(plan).toContain('type=glm-stress')
  })

  test('run-type set is exactly the two bounded synthetic-load types', () => {
    expect([...ARTANIS_SYNTHETIC_LOAD_RUN_TYPES]).toEqual([
      'terminal-bench',
      'glm-stress',
    ])
  })

  test('buildArtanisSyntheticLoadPlan honors injected bounds', () => {
    const result = buildArtanisSyntheticLoadPlan(
      { targetTokens: 5, type: 'terminal-bench' },
      { maxTargetTokens: 10, minTargetTokens: 1 },
    )
    expect(result).toContain('type=terminal-bench')
    expect(result).toContain('target=5 tokens')
  })
})

describe('get_glm_fleet_status (live GLM inference-fleet readiness) - iteration 7', () => {
  test('returns a concise summary naming status + ready/total replicas from the injected fetch', async () => {
    const urls: Array<string> = []
    const fetchImpl = (async (input: RequestInfo | URL) => {
      urls.push(typeof input === 'string' ? input : input.toString())
      return new Response(
        JSON.stringify({
          readyReplicas: 8,
          status: 'ready',
          totalReplicas: 8,
        }),
        { status: 200 },
      )
    }) as typeof fetch

    const tool = makeArtanisGetGlmFleetStatusTool({ fetchImpl })
    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_glm_fleet_status')

    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('status=ready')
    expect(result).toContain('8/8 replicas ready')
    // It hit the public-safe readiness route by default.
    expect(urls[0]).toBe(
      'https://openagents.com/api/v1/gateway/glm-fleet/readiness',
    )
  })

  test('normalizes the live route shape (status + counts.{ready,total,warm}ReplicaCount)', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          counts: {
            readyReplicaCount: 3,
            totalReplicaCount: 6,
            warmReplicaCount: 2,
          },
          kind: 'glm_fleet_readiness',
          status: 'degraded',
        }),
        { status: 200 },
      )) as typeof fetch
    const tool = makeArtanisGetGlmFleetStatusTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('status=degraded')
    expect(result).toContain('3/6 replicas ready')
    expect(result).toContain('(2 warm)')
  })

  test('a failing/unreachable fetch returns an honest "(could not fetch ...)" string, never fabricated numbers', async () => {
    const failing = (async () => {
      throw new Error('network down')
    }) as typeof fetch
    const tool = makeArtanisGetGlmFleetStatusTool({ fetchImpl: failing })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not fetch GLM fleet status')
    // No invented replica counts leak into the honest failure string.
    expect(result).not.toMatch(/replicas ready/)
    expect(result).not.toMatch(/Error:/)
  })

  test('a non-OK response reads as an honest soft failure, never invention', async () => {
    const fetchImpl = (async () =>
      new Response('inference_gateway_disabled', { status: 404 })) as typeof fetch
    const tool = makeArtanisGetGlmFleetStatusTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not fetch GLM fleet status')
    expect(result).toContain('status 404')
  })

  test('an unexpected payload shape reads as an honest soft failure', async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ nope: true }), { status: 200 })) as typeof fetch
    const tool = makeArtanisGetGlmFleetStatusTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('unexpected response shape')
  })

  test('uses the in-worker loadFleetStatus override instead of HTTP when provided', async () => {
    let httpCalled = false
    const httpFetch = (async () => {
      httpCalled = true
      return new Response('{}', { status: 200 })
    }) as typeof fetch
    const tool = makeArtanisGetGlmFleetStatusTool({
      fetchImpl: httpFetch,
      loadFleetStatus: async (): Promise<ArtanisGlmFleetStatus> => ({
        readyReplicas: 5,
        status: 'ready',
        totalReplicas: 5,
        warmReplicas: null,
      }),
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(httpCalled).toBe(false)
    expect(result).toContain('status=ready')
    expect(result).toContain('5/5 replicas ready')
    // No warm count reported -> no "(N warm)" suffix.
    expect(result).not.toContain('warm')
  })

  test('a loadFleetStatus rejection reads as an honest soft failure, never a throw', async () => {
    const tool = makeArtanisGetGlmFleetStatusTool({
      loadFleetStatus: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not fetch GLM fleet status')
    expect(result).not.toMatch(/Error:/)
  })

  test('normalizeArtanisGlmFleetStatus defensively redacts a non-public-safe status', () => {
    const normalized = normalizeArtanisGlmFleetStatus({
      readyReplicas: 1,
      status: 'ready bearer sk-abc123',
      totalReplicas: 1,
    })
    expect(normalized).not.toBeNull()
    expect(normalized?.status).toBe('(redacted)')
  })

  test('normalizeArtanisGlmFleetStatus returns null when required fields are missing', () => {
    expect(normalizeArtanisGlmFleetStatus(null)).toBeNull()
    expect(normalizeArtanisGlmFleetStatus({ status: 'ready' })).toBeNull()
    expect(
      normalizeArtanisGlmFleetStatus({ readyReplicas: 1, totalReplicas: 1 }),
    ).toBeNull()
  })
})

describe('get_synthetic_load_status (active synthetic-load runs) - iteration 12', () => {
  test('with a stubbed source returning one active run, returns a public-safe summary with run ref, state, and token-burn progress', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      reader: async (): Promise<ReadonlyArray<ArtanisSyntheticLoadRun>> => [
        {
          runRef: 'synthetic_load.terminal_bench.2026_06_27_01',
          runType: 'terminal-bench',
          state: 'running',
          targetTokens: 10_000_000,
          tokensBurned: 4_200_000,
        },
      ],
    })
    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_synthetic_load_status')

    const result = await Effect.runPromise(tool.execute({}))
    // run ref
    expect(result).toContain('synthetic_load.terminal_bench.2026_06_27_01')
    // run type + state
    expect(result).toContain('[terminal-bench]')
    expect(result).toContain('state=running')
    // token-burn progress (burned/target + percent)
    expect(result).toContain('4,200,000/10,000,000 tokens burned (42%)')
    expect(result).toContain('Synthetic-load runs (1 active):')
  })

  test('a run without a reported target shows burned tokens without a fabricated percent', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      reader: async () => [
        {
          runRef: 'synthetic_load.glm_stress.2026_06_27_09',
          runType: 'glm-stress',
          state: 'queued',
          targetTokens: null,
          tokensBurned: 1_500_000,
        },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('1,500,000 tokens burned')
    expect(result).not.toContain('%')
  })

  test('no reader wired reads as an honest "(no active synthetic-load runs)"', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool()
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toBe('(no active synthetic-load runs)')
  })

  test('a reader returning no runs reads as honest absence', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      reader: async () => [],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toBe('(no active synthetic-load runs)')
  })

  test('a reader rejection reads as an honest soft failure, never a throw', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      reader: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toBe('(could not read synthetic-load status)')
    expect(result).not.toMatch(/Error:/)
  })

  test('defensively redacts a non-public-safe run field, never leaking it into context', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      reader: async () => [
        {
          runRef: 'synthetic_load.bearer sk-abc123',
          runType: 'terminal-bench',
          state: 'running',
          targetTokens: 1_000_000,
          tokensBurned: 500_000,
        },
      ],
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('(redacted)')
    expect(result).not.toContain('sk-abc123')
  })

  test('bounds the number of summarized runs to the configured cap', async () => {
    const tool = makeArtanisGetSyntheticLoadStatusTool({
      maxRuns: 2,
      reader: async () =>
        Array.from({ length: 5 }, (_, index) => ({
          runRef: `synthetic_load.terminal_bench.run_${index}`,
          runType: 'terminal-bench',
          state: 'running',
          targetTokens: 1_000_000,
          tokensBurned: 100_000,
        })),
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('Synthetic-load runs (2 active):')
    expect(result).toContain('run_0')
    expect(result).toContain('run_1')
    expect(result).not.toContain('run_2')
  })
})

// A public-safe trace-review report fixture matching the live route shape
// (`buildKhalaTraceReviewReport` output): window + aggregates + modelMix +
// outcomes + failureModes. Used to drive the get_trace_review read tool.
const TRACE_REVIEW_REPORT_FIXTURE = {
  aggregates: {
    rawCodexEvents: { byteLength: 4096, eventCount: 9, rowCount: 3 },
    tokens: {
      eventCount: 42,
      inputTokens: 5000,
      outputTokens: 7000,
      totalTokens: 12000,
    },
    traces: { traceCount: 17, zeroStepCount: 1 },
  },
  failureModes: [
    {
      count: 4,
      evidenceRefs: ['table.token_usage_events.output_tokens_zero'],
      failureRef: 'failure.khala_trace_review.empty_response',
      label: 'Token rows with zero completion/output tokens',
      severity: 'warning',
    },
  ],
  generatedAt: '2026-06-27T00:00:00.000Z',
  modelMix: [
    {
      count: 30,
      model: 'khala',
      provider: 'openagents',
      totalTokens: 9000,
    },
    {
      count: 12,
      model: 'pylon-codex',
      provider: 'pylon-codex-own-capacity',
      totalTokens: 3000,
    },
  ],
  outcomes: [
    { count: 38, outcome: 'stop', totalTokens: 11000 },
    { count: 4, outcome: 'unknown', totalTokens: 1000 },
  ],
  reportRef: 'khala_trace_review.2026_06_27',
  schemaVersion: 'openagents.khala.trace_review.v1',
  window: {
    hours: 24,
    since: '2026-06-26T00:00:00.000Z',
    until: '2026-06-27T00:00:00.000Z',
  },
} as const

describe('get_trace_review (live Khala trace-review report) - iteration 11', () => {
  test('returns a bounded summary covering window, model mix, and outcome/failure buckets from the injected fetch', async () => {
    const urls: Array<string> = []
    const fetchImpl = (async (input: RequestInfo | URL) => {
      urls.push(typeof input === 'string' ? input : input.toString())
      return new Response(JSON.stringify(TRACE_REVIEW_REPORT_FIXTURE), {
        status: 200,
      })
    }) as typeof fetch

    const tool = makeArtanisGetTraceReviewTool({ fetchImpl })
    // It is a READ tool (executes freely, never gated/risky).
    expect(tool.kind).toBe('read')
    expect(tool.definition.name).toBe('get_trace_review')

    const result = await Effect.runPromise(tool.execute({}))
    // Window.
    expect(result).toContain('Khala trace review (last 24h')
    expect(result).toContain('2026-06-26T00:00:00.000Z -> 2026-06-27T00:00:00.000Z')
    // Aggregates.
    expect(result).toContain('42 token rows')
    expect(result).toContain('12,000 tokens')
    expect(result).toContain('17 traces')
    expect(result).toContain('3 raw Codex event rows')
    // Model mix.
    expect(result).toContain('Model mix (2):')
    expect(result).toContain('openagents/khala: 30 calls, 9,000 tokens')
    expect(result).toContain(
      'pylon-codex-own-capacity/pylon-codex: 12 calls, 3,000 tokens',
    )
    // Outcomes.
    expect(result).toContain('Outcomes (2):')
    expect(result).toContain('stop: 38 (11,000 tokens)')
    // Failure modes.
    expect(result).toContain('Failure modes (1):')
    expect(result).toContain('[warning] Token rows with zero completion')
    expect(result).toContain('failure.khala_trace_review.empty_response')
    // It hit the operator trace-review route by default.
    expect(urls[0]).toBe(
      'https://openagents.com/api/operator/khala/trace-review',
    )
  })

  test('empty model-mix / outcome / failure sections read "(none)", never fabricated buckets', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          aggregates: {
            rawCodexEvents: { rowCount: 0 },
            tokens: { eventCount: 0, totalTokens: 0 },
            traces: { traceCount: 0 },
          },
          failureModes: [],
          modelMix: [],
          outcomes: [],
          window: { hours: 24, since: 's', until: 'u' },
        }),
        { status: 200 },
      )) as typeof fetch
    const tool = makeArtanisGetTraceReviewTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('Model mix: (none)')
    expect(result).toContain('Outcomes: (none)')
    expect(result).toContain('Failure modes: (none)')
    expect(result).toContain('0 token rows')
  })

  test('a failing/unreachable fetch returns an honest "(could not fetch trace review ...)" string, never fabricated numbers', async () => {
    const failing = (async () => {
      throw new Error('network down')
    }) as typeof fetch
    const tool = makeArtanisGetTraceReviewTool({ fetchImpl: failing })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not fetch trace review')
    expect(result).not.toMatch(/token rows/)
    expect(result).not.toMatch(/Error:/)
  })

  test('a non-OK response reads as an honest soft failure, never invention', async () => {
    const fetchImpl = (async () =>
      new Response('unauthorized', { status: 401 })) as typeof fetch
    const tool = makeArtanisGetTraceReviewTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('could not fetch trace review')
    expect(result).toContain('status 401')
  })

  test('a non-object payload reads as an honest soft failure', async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify('nope'), { status: 200 })) as typeof fetch
    const tool = makeArtanisGetTraceReviewTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toContain('unexpected response shape')
  })

  test('uses the in-worker loadReport override instead of HTTP when provided', async () => {
    let httpCalled = false
    const httpFetch = (async () => {
      httpCalled = true
      return new Response('{}', { status: 200 })
    }) as typeof fetch
    const tool = makeArtanisGetTraceReviewTool({
      fetchImpl: httpFetch,
      loadReport: async () => TRACE_REVIEW_REPORT_FIXTURE,
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(httpCalled).toBe(false)
    expect(result).toContain('Khala trace review (last 24h')
    expect(result).toContain('openagents/khala: 30 calls, 9,000 tokens')
  })

  test('a loadReport rejection reads as an honest soft failure, never a throw', async () => {
    const tool = makeArtanisGetTraceReviewTool({
      loadReport: async () => {
        throw new Error('d1 down')
      },
    })
    const result = await Effect.runPromise(tool.execute({}))
    expect(result).toBe('(could not fetch trace review)')
    expect(result).not.toMatch(/Error:/)
  })

  test('bounds the buckets surfaced per section to maxBuckets', async () => {
    const tool = makeArtanisGetTraceReviewTool({
      maxBuckets: 1,
      loadReport: async () => TRACE_REVIEW_REPORT_FIXTURE,
    })
    const result = await Effect.runPromise(tool.execute({}))
    // Only the top model-mix / outcome bucket survives the bound.
    expect(result).toContain('Model mix (1):')
    expect(result).toContain('openagents/khala')
    expect(result).not.toContain('pylon-codex-own-capacity/pylon-codex')
    expect(result).toContain('Outcomes (1):')
    expect(result).not.toContain('unknown: 4')
  })

  test('preserves bounded provider/model identifiers with sk-shaped substrings, never "(redacted)"', () => {
    const normalized = normalizeArtanisTraceReview({
      aggregates: {},
      modelMix: [
        {
          count: 1,
          model: 'openagents/glm-5.2-reap-504b',
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          totalTokens: 1,
        },
      ],
      window: {},
    })
    expect(normalized).not.toBeNull()
    // The legitimate serving provider id (which contains `sk-vllm-...`) survives.
    expect(normalized?.modelMix[0]?.provider).toBe(
      'hydralisk-vllm-glm-5p2-reap-504b',
    )
    expect(normalized?.modelMix[0]?.model).toBe('openagents/glm-5.2-reap-504b')
  })

  test('normalizeArtanisTraceReview defensively redacts a non-public-safe outcome field', () => {
    const normalized = normalizeArtanisTraceReview({
      aggregates: {},
      modelMix: [],
      outcomes: [{ count: 1, outcome: 'bearer sk-abcdef0123456789', totalTokens: 1 }],
      window: {},
    })
    expect(normalized).not.toBeNull()
    expect(normalized?.outcomes[0]?.outcome).toBe('(redacted)')
  })

  test('normalizeArtanisTraceReview returns null only for a non-object body', () => {
    expect(normalizeArtanisTraceReview(null)).toBeNull()
    expect(normalizeArtanisTraceReview('x')).toBeNull()
    // An object missing sections degrades to empty/0, never null.
    const empty = normalizeArtanisTraceReview({})
    expect(empty).not.toBeNull()
    expect(empty?.modelMix).toEqual([])
    expect(empty?.totalTokens).toBe(0)
    expect(empty?.windowHours).toBeNull()
  })
})

describe('makeArtanisOperatorTools default table', () => {
  test('includes the repo-read tools, the issue-read tool, the network-stats tool, the job-status tool, and the dispatch tool', () => {
    const tools = makeArtanisOperatorTools()
    const names = tools.map(tool => tool.definition.name).sort()
    expect(names).toEqual([
      'dispatch_codex_task',
      'get_glm_fleet_status',
      'get_khala_feedback',
      'get_network_stats',
      'get_pylon_job_status',
      'get_synthetic_load_status',
      'get_trace_review',
      'get_unsupported_requests',
      'list_github_issues',
      'list_pylon_assignments',
      'list_repo_dir',
      'post_forum_update',
      'read_github_issue',
      'read_repo_file',
      'trigger_synthetic_load',
      'update_unsupported_request',
    ])
  })

  test('the default table includes a write-kind update_unsupported_request tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(
      t => t.definition.name === 'update_unsupported_request',
    )
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('write')
  })

  test('the default table includes a read-kind get_unsupported_requests tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(
      t => t.definition.name === 'get_unsupported_requests',
    )
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a read-kind list_pylon_assignments tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(
      t => t.definition.name === 'list_pylon_assignments',
    )
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a read-kind get_glm_fleet_status tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(t => t.definition.name === 'get_glm_fleet_status')
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a read-kind get_synthetic_load_status tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(
      t => t.definition.name === 'get_synthetic_load_status',
    )
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a read-kind get_khala_feedback tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(t => t.definition.name === 'get_khala_feedback')
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a read-kind get_trace_review tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(t => t.definition.name === 'get_trace_review')
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('read')
  })

  test('the default table includes a trigger_synthetic_load tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(
      t => t.definition.name === 'trigger_synthetic_load',
    )
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('risky')
  })

  test('the default table includes a gated post_forum_update tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(t => t.definition.name === 'post_forum_update')
    expect(tool).toBeDefined()
    expect(tool?.kind).toBe('gated')
    expect(tool?.kind === 'gated' ? tool.riskyActionKind : null).toBe(
      'forum_post',
    )
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
