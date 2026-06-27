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
  buildArtanisSyntheticLoadPlan,
  isSafeArtanisAssignmentRef,
  isSafeArtanisRepoPath,
  makeArtanisDispatchCodexTaskTool,
  makeArtanisGetGlmFleetStatusTool,
  makeArtanisGetKhalaFeedbackTool,
  makeArtanisGetNetworkStatsTool,
  makeArtanisGetPylonJobStatusTool,
  makeArtanisListPylonAssignmentsTool,
  makeArtanisListRepoDirTool,
  makeArtanisOperatorTools,
  makeArtanisReadGithubIssueTool,
  makeArtanisReadRepoFileTool,
  makeArtanisTriggerSyntheticLoadTool,
  normalizeArtanisGlmFleetStatus,
  parseArtanisAssignmentRef,
  parseArtanisIssueNumber,
} from './artanis-operator-tools'
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
      'list_pylon_assignments',
      'list_repo_dir',
      'read_github_issue',
      'read_repo_file',
      'trigger_synthetic_load',
    ])
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

  test('the default table includes a read-kind get_khala_feedback tool', () => {
    const tools = makeArtanisOperatorTools()
    const tool = tools.find(t => t.definition.name === 'get_khala_feedback')
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
