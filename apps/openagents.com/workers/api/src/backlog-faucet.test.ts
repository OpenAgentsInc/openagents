import { describe, expect, test } from 'vitest'

import {
  backlogFaucetDeadlineRef,
  backlogFaucetIdempotencyKey,
  backlogFaucetIssueUrlForObjectiveRef,
  backlogFaucetObjectiveRef,
  backlogFaucetRelistDecision,
  backlogFaucetRepositoryRef,
  buildBacklogWorkRequestFiling,
  detectBacklogIssueChannels,
  lifecycleIssueCommentBody,
  listedIssueCommentBody,
  parseBacklogFaucetBudgetAssignments,
  proposeBacklogFaucetIssues,
} from './backlog-faucet'
import {
  buildForumWorkRequestLbrDraft,
  normalizeForumWorkRequestInput,
} from './forum-work-requests'
import {
  type GitHubIssueForMarchingOrders,
  deliveredIssueCommentBody,
} from './marching-orders-agent'

const issue = (
  input: Partial<GitHubIssueForMarchingOrders> &
    Pick<GitHubIssueForMarchingOrders, 'number' | 'title'>,
): GitHubIssueForMarchingOrders => ({
  body: input.body ?? 'Verification command: bun test',
  html_url:
    input.html_url ??
    `https://github.com/OpenAgentsInc/openagents/issues/${input.number}`,
  labels: input.labels ?? [{ name: 'roadmap' }],
  number: input.number,
  state: input.state ?? 'open',
  title: input.title,
  ...(input.pull_request === undefined
    ? {}
    : { pull_request: input.pull_request }),
})

const filingConfig = {
  budgetSats: 2_000,
  deadlineRef: 'deadline.public.backlog_faucet.20260618',
  repository: 'OpenAgentsInc/openagents',
  verificationCommandRef: 'command.public.autopilot_coder.bun_test',
}

describe('backlog-faucet selection and channel exclusivity', () => {
  test('proposes bounded public-safe issues as open-market candidates', () => {
    const proposals = proposeBacklogFaucetIssues([
      issue({
        number: 4790,
        title: 'P9: settlement visibility test coverage for payout receipts',
      }),
    ])

    expect(proposals).toEqual([
      expect.objectContaining({
        channels: [],
        issueNumber: 4790,
        reasonRefs: ['selection.github_issue.bounded_public_safe_candidate'],
        selectionState: 'candidate',
      }),
    ])
  })

  test('skips issues already in the in-house marching-orders channel', () => {
    const detections = new Map([
      [
        4790,
        detectBacklogIssueChannels([
          deliveredIssueCommentBody({
            closeoutRefs: ['assignment.closeout.test'],
            resultRefs: [],
            testRefs: [],
            workOrderRef: 'autopilot_work_order.test',
          }),
        ]),
      ],
    ])
    const proposals = proposeBacklogFaucetIssues(
      [
        issue({
          number: 4790,
          title: 'P9: settlement visibility test coverage for payout receipts',
        }),
      ],
      { channelDetections: detections },
    )

    expect(proposals[0]).toMatchObject({
      channels: ['in_house_work_order'],
      reasonRefs: ['selection.github_issue.in_house_channel_active'],
      selectionState: 'skipped',
    })
  })

  test('skips issues already listed on the open market and recovers the work-request ref', () => {
    const listedComment = listedIssueCommentBody({
      budgetSats: 2_000,
      deadlineRef: 'deadline.public.backlog_faucet.20260618',
      jobEventId: 'a'.repeat(64),
      objectiveRef:
        'objective.public.github_issue.openagentsinc_openagents.4790',
      topicSlug: 'backlog-issue-4790',
      verificationCommandRef: 'command.public.autopilot_coder.bun_test',
      workRequestId: 'wr-4790-test',
    })
    const detection = detectBacklogIssueChannels([listedComment])
    const proposals = proposeBacklogFaucetIssues(
      [
        issue({
          number: 4790,
          title: 'P9: settlement visibility test coverage for payout receipts',
        }),
      ],
      { channelDetections: new Map([[4790, detection]]) },
    )

    expect(detection).toEqual({
      channels: ['open_market_work_request'],
      marketWorkRequestRefs: ['work_request.public.wr-4790-test'],
    })
    expect(proposals[0]).toMatchObject({
      reasonRefs: ['selection.github_issue.already_listed_open_market'],
      selectionState: 'skipped',
    })
  })

  test('keeps marching-orders safety skips alongside channel skips', () => {
    const proposals = proposeBacklogFaucetIssues([
      issue({
        body: 'Use this access_token to test the private repo.',
        number: 4999,
        title: 'P9: repair API route',
      }),
    ])

    expect(proposals[0]).toMatchObject({
      reasonRefs: expect.arrayContaining([
        'selection.github_issue.private_or_secret_context',
      ]),
      selectionState: 'skipped',
    })
  })
})

describe('backlog-faucet decoration contract', () => {
  test('builds a ref-only filing that passes the live work-request surface validation', () => {
    const filing = buildBacklogWorkRequestFiling(
      issue({
        number: 4790,
        title: 'P9: settlement visibility test coverage for payout receipts',
      }),
      filingConfig,
    )

    expect(filing).toMatchObject({
      idempotencyKey: 'backlog-faucet:OpenAgentsInc/openagents:4790',
      issueNumber: 4790,
      issueUrl: 'https://github.com/OpenAgentsInc/openagents/issues/4790',
      objectiveRef:
        'objective.public.github_issue.openagentsinc_openagents.4790',
    })
    expect(filing.input).toMatchObject({
      budgetSats: 2_000,
      deadlineRef: 'deadline.public.backlog_faucet.20260618',
      repositoryRefs: ['repo.public.openagents'],
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      verificationCommandRef: 'command.public.autopilot_coder.bun_test',
    })

    const normalized = normalizeForumWorkRequestInput(filing.input)
    const lbr = buildForumWorkRequestLbrDraft(normalized, {
      relayUrl: 'wss://relay.test.openagents.dev',
      topicId: 'topic-test',
    })

    expect(normalized.budgetMsats).toBe(2_000_000)
    expect(lbr.draft.kind).toBe(5934)
    expect(lbr.draft.tags).toContainEqual([
      'param',
      'lbr_objective_ref',
      'objective.public.github_issue.openagentsinc_openagents.4790',
    ])
    expect(lbr.draft.tags).toContainEqual(['bid', '2000000'])
  })

  test('never copies issue bodies into the filing because the objective stays deref-able', () => {
    const filing = buildBacklogWorkRequestFiling(
      issue({
        body: 'A very long issue body that must never travel to the market.',
        number: 4790,
        title: 'P9: settlement visibility test coverage for payout receipts',
      }),
      filingConfig,
    )

    expect(JSON.stringify(filing.input)).not.toContain('A very long issue body')
    expect(
      backlogFaucetIssueUrlForObjectiveRef('OpenAgentsInc/openagents', 4790),
    ).toBe('https://github.com/OpenAgentsInc/openagents/issues/4790')
    expect(backlogFaucetObjectiveRef('OpenAgentsInc/openagents', 4790)).toBe(
      filing.objectiveRef,
    )
  })

  test('rejects unsafe issue titles before any filing is produced', () => {
    expect(() =>
      buildBacklogWorkRequestFiling(
        issue({
          number: 5000,
          title: 'Pay the lnbc invoice from the wallet seed',
        }),
        filingConfig,
      ),
    ).toThrowError(/private, payment, credential/)
  })

  test('rejects non-positive budgets and malformed repositories', () => {
    expect(() =>
      buildBacklogWorkRequestFiling(issue({ number: 1, title: 'P9: fix' }), {
        ...filingConfig,
        budgetSats: 0,
      }),
    ).toThrowError(/positive integer sat amount/)
    expect(() =>
      buildBacklogWorkRequestFiling(issue({ number: 1, title: 'P9: fix' }), {
        ...filingConfig,
        repository: 'not a repo slug',
      }),
    ).toThrowError(/owner\/name GitHub slug/)
  })

  test('bounds long titles to the work-request title contract', () => {
    const filing = buildBacklogWorkRequestFiling(
      issue({ number: 4790, title: 'P9: settlement '.repeat(30) }),
      filingConfig,
    )

    expect(filing.input.title.length).toBeLessThanOrEqual(160)
    expect(filing.input.title.startsWith('Backlog issue #4790: ')).toBe(true)
  })

  test('maps non-OpenAgents repositories to generic public repo refs', () => {
    expect(backlogFaucetRepositoryRef('OpenAgentsInc/openagents')).toBe(
      'repo.public.openagents',
    )
    expect(backlogFaucetRepositoryRef('SomeOrg/some-repo')).toBe(
      'repo.public.github.someorg_some-repo',
    )
  })
})

describe('backlog-faucet inventory hygiene', () => {
  test('relisting is allowed only from terminal expired/cancelled states', () => {
    expect(backlogFaucetRelistDecision('expired', 0)).toEqual({
      allowed: true,
      nextGeneration: 1,
    })
    expect(backlogFaucetRelistDecision('cancelled', 2)).toEqual({
      allowed: true,
      nextGeneration: 3,
    })
    expect(backlogFaucetRelistDecision('open', 0)).toEqual({
      allowed: false,
      reasonRef: 'relist.work_request_state_not_terminal.open',
    })
    expect(backlogFaucetRelistDecision('settled', 1)).toEqual({
      allowed: false,
      reasonRef: 'relist.work_request_state_not_terminal.settled',
    })
  })

  test('relist generations change the idempotency key deliberately', () => {
    expect(
      backlogFaucetIdempotencyKey('OpenAgentsInc/openagents', 4790, 0),
    ).toBe('backlog-faucet:OpenAgentsInc/openagents:4790')
    expect(
      backlogFaucetIdempotencyKey('OpenAgentsInc/openagents', 4790, 2),
    ).toBe('backlog-faucet:OpenAgentsInc/openagents:4790:relist-2')
  })

  test('deadline refs derive from ISO dates only', () => {
    expect(backlogFaucetDeadlineRef('2026-06-18')).toBe(
      'deadline.public.backlog_faucet.20260618',
    )
    expect(backlogFaucetDeadlineRef('2026-06-18T00:00:00Z')).toBe(
      'deadline.public.backlog_faucet.20260618',
    )
    expect(() => backlogFaucetDeadlineRef('next week')).toThrowError(/ISO date/)
  })
})

describe('backlog-faucet lifecycle linkage', () => {
  test('listing comments carry the channel marker and honest budget wording', () => {
    const body = listedIssueCommentBody({
      budgetSats: 2_000,
      deadlineRef: 'deadline.public.backlog_faucet.20260618',
      jobEventId: 'b'.repeat(64),
      objectiveRef:
        'objective.public.github_issue.openagentsinc_openagents.4790',
      topicSlug: 'backlog-issue-4790',
      verificationCommandRef: 'command.public.autopilot_coder.bun_test',
      workRequestId: 'wr-test',
    })

    expect(body).toContain('Channel: openagents.market.work_request')
    expect(body).toContain('work_request.public.wr-test')
    expect(body).toContain('Escrow reserves on quote acceptance')
    expect(body).toContain(
      'nothing here is settled bitcoin until a settlement receipt exists',
    )
    expect(body).toContain('one channel at a time')
  })

  test('lifecycle mirror comments require a receipt ref and stay claims-free', () => {
    const body = lifecycleIssueCommentBody({
      lifecycleKind: 'settled',
      receiptRef: 'receipt.labor_escrow.release.test',
      workRequestId: 'wr-test',
    })

    expect(body).toContain('lifecycle update: settled')
    expect(body).toContain('receipt.labor_escrow.release.test')
    expect(body).toContain('not an acceptance or settlement claim')
    expect(() =>
      lifecycleIssueCommentBody({
        lifecycleKind: 'settled',
        receiptRef: '   ',
        workRequestId: 'wr-test',
      }),
    ).toThrowError(/require a receipt ref/)
  })
})

describe('backlog-faucet budget assignments', () => {
  test('parses maintainer budget decorations as typed issue-to-sats pairs', () => {
    expect(parseBacklogFaucetBudgetAssignments('4790=2000, 4791=1500')).toEqual(
      new Map([
        [4790, 2_000],
        [4791, 1_500],
      ]),
    )
    expect(() => parseBacklogFaucetBudgetAssignments('4790')).toThrowError(
      /<issueNumber>=<budgetSats>/,
    )
    expect(() => parseBacklogFaucetBudgetAssignments('')).toThrowError(
      /at least one/,
    )
  })
})
