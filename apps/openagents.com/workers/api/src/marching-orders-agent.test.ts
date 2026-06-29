import { describe, expect, test } from 'vitest'

import {
  buildMarchingOrderSubmission,
  deliveredIssueCommentBody,
  type GitHubIssueForMarchingOrders,
  proposeMarchingOrderIssues,
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

describe('marching-orders issue selection', () => {
  test('proposes bounded public-safe roadmap issues with explicit verification', () => {
    const proposals = proposeMarchingOrderIssues([
      issue({
        number: 4758,
        title: 'B4: work list + detail visibility in the web UI',
      }),
    ])

    expect(proposals).toEqual([
      expect.objectContaining({
        issueNumber: 4758,
        reasonRefs: ['selection.github_issue.bounded_public_safe_candidate'],
        selectionState: 'candidate',
      }),
    ])
  })

  test('requires human verification command approval when the issue lacks one', () => {
    const proposals = proposeMarchingOrderIssues([
      issue({
        body: 'Build the route and UI contract.',
        number: 4758,
        title: 'B4: work list + detail visibility in the web UI',
      }),
    ])

    expect(proposals[0]).toMatchObject({
      reasonRefs: ['selection.github_issue.needs_human_verification_command'],
      selectionState: 'needs_human_verification_command',
    })
  })

  test('skips private or secret-shaped issue text', () => {
    const proposals = proposeMarchingOrderIssues([
      issue({
        body: 'Use this access_token to test the private repo.',
        number: 4999,
        title: 'B9: repair API route',
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

describe('marching-orders work decoration', () => {
  test('builds a validated no-spend Autopilot work request for an approved issue', () => {
    const submission = buildMarchingOrderSubmission(
      issue({
        number: 4758,
        title: 'B4: work list + detail visibility in the web UI',
      }),
      {
        agentId: 'agent.openagents.marching_orders',
        baseUrl: 'https://openagents.com',
        branch: 'main',
        commitSha: '1234567890abcdef1234567890abcdef12345678',
        ownerRef: 'owner_ref.openagents_core',
        pylonId: 'pylon.test',
        repository: 'OpenAgentsInc/openagents',
        verificationCommand: {
          args: ['bun', 'test'],
          commandRef: 'command.public.autopilot_coder.bun_test',
        },
      },
    )

    expect(submission).toMatchObject({
      idempotencyKey:
        'marching-orders:OpenAgentsInc/openagents:4758:1234567890abcdef1234567890abcdef12345678',
      issueNumber: 4758,
    })
    expect(submission.request).toMatchObject({
      caller: {
        agentId: 'agent.openagents.marching_orders',
        kind: 'registered_agent',
        ownerRef: 'owner_ref.openagents_core',
        pylonId: 'pylon.test',
      },
      paymentPolicy: {
        buyerPaymentMode: 'free_slice',
        maxSpendCents: 0,
        settlementMode: 'no_worker_payout',
      },
      placementPolicy: {
        preferredRunnerKinds: ['requester_pylon'],
      },
      tasks: [
        {
          checkout: {
            commitSha: '1234567890abcdef1234567890abcdef12345678',
            kind: 'git_checkout',
            verificationCommand: {
              args: ['bun', 'test'],
            },
          },
          repository: {
            fullName: 'OpenAgentsInc/openagents',
            visibility: 'public',
          },
        },
      ],
    })
    expect(JSON.stringify(submission.request)).not.toMatch(
      /access_token|mnemonic|private_repo|raw_prompt|wallet/i,
    )
  })

  test('delivery comments point to refs without accepting the work', () => {
    expect(
      deliveredIssueCommentBody({
        closeoutRefs: ['assignment.closeout.summary.test'],
        resultRefs: ['result.public.pylon.claude_agent_task.edited_files.1'],
        testRefs: ['command.pylon.claude_agent_task.verification.test'],
        workOrderRef: 'autopilot_work_order.test',
      }),
    ).toContain('a human owner must review')
  })
})
