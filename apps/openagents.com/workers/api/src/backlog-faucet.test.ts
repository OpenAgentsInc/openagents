import { describe, expect, test } from 'vitest'

import {
  BacklogFaucetGateError,
  DefaultBacklogFaucetMaxBudgetSats,
  advanceBacklogFaucetState,
  approveBacklogFaucetForPublication,
  backlogFaucetDeadlineRef,
  backlogFaucetIdempotencyKey,
  backlogFaucetIssueUrlForObjectiveRef,
  backlogFaucetObjectiveRef,
  backlogFaucetRelistDecision,
  backlogFaucetRepositoryRef,
  buildBacklogWorkRequestFiling,
  detectBacklogIssueChannels,
  draftBacklogFaucetRecord,
  dryRunBacklogFaucetItem,
  lifecycleIssueCommentBody,
  listedIssueCommentBody,
  markBacklogFaucetPublished,
  parseBacklogFaucetBudgetAssignments,
  parseBacklogFaucetIssueRef,
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

  test('rejects closed GitHub issues before producing an open work request', () => {
    expect(() =>
      buildBacklogWorkRequestFiling(
        issue({
          number: 4773,
          state: 'closed',
          title: 'A1 API parity matrix slice',
        }),
        filingConfig,
      ),
    ).toThrowError(/closed GitHub issues cannot be listed/)
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

// EXAMPLE payload only: issue #4781 decorated as a budgeted backlog item.
// This fixture demonstrates the no-spend contract; it is not a live filing,
// not a published event, and not an escrow reservation.
const exampleBacklogItem4781 = {
  boundedScope:
    'Example only: validate the faucet contract for one budgeted backlog issue without publishing or escrowing anything.',
  budgetSats: 2_000,
  deadlineDate: '2026-06-19',
  issueRef: 'https://github.com/OpenAgentsInc/openagents/issues/4781',
  title:
    'P5: backlog faucet for the open market - budgeted issues become NIP-LBR work requests',
  verificationCommandRef: 'command.public.autopilot_coder.bun_test',
}

const dryRunOptions = { nowIso: '2026-06-12T23:00:00.000Z' }

describe('backlog-faucet record contract (no-spend dry run)', () => {
  test('dry run builds, validates, and records without publishing or escrowing', () => {
    const report = dryRunBacklogFaucetItem(exampleBacklogItem4781, dryRunOptions)

    expect(report.published).toBe(false)
    expect(report.escrowed).toBe(false)
    expect(report.record.state).toBe('drafted')
    expect(report.record.spendGate).toEqual({ kind: 'not_approved' })
    expect(report.record.publication).toEqual({ kind: 'not_published' })
    expect(report.record.history).toEqual([])
    expect(report.record.repository).toBe('OpenAgentsInc/openagents')
    expect(report.record.filing.objectiveRef).toBe(
      'objective.public.github_issue.openagentsinc_openagents.4781',
    )
    expect(report.previewDraft.kind).toBe(5934)
    expect(report.previewDraft.tags).toContainEqual([
      'param',
      'lbr_objective_ref',
      'objective.public.github_issue.openagentsinc_openagents.4781',
    ])
    expect(report.previewDraft.tags).toContainEqual([
      'param',
      'lbr_verification_command_ref',
      'command.public.autopilot_coder.bun_test',
    ])
    expect(report.previewDraft.tags).toContainEqual(['bid', '2000000'])
  })

  test('records carry a public-safe projection with freshness metadata', () => {
    const record = draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions)
    const projection = JSON.parse(record.publicProjectionJson)

    expect(projection).toMatchObject({
      budgetSats: 2_000,
      generatedAt: '2026-06-12T23:00:00.000Z',
      issueRef: 'https://github.com/OpenAgentsInc/openagents/issues/4781',
      publication: { kind: 'not_published' },
      spendGate: { kind: 'not_approved' },
      stalenessContract: 'rebuilt_on_faucet_state_transition',
      state: 'drafted',
      workRequestKind: 5934,
    })
    expect(record.publicProjectionJson).not.toContain('lnbc')
  })

  test('parses issue refs and rejects non-GitHub or malformed refs', () => {
    expect(
      parseBacklogFaucetIssueRef(
        'https://github.com/OpenAgentsInc/openagents/issues/4781',
      ),
    ).toEqual({ issueNumber: 4781, repository: 'OpenAgentsInc/openagents' })
    expect(() =>
      parseBacklogFaucetIssueRef('https://example.com/issues/1'),
    ).toThrowError(BacklogFaucetGateError)
    expect(() =>
      draftBacklogFaucetRecord(
        { ...exampleBacklogItem4781, issueRef: 'issue 4781' },
        dryRunOptions,
      ),
    ).toThrowError(/public GitHub issue URL/)
  })

  test('enforces budget bounds with a typed error', () => {
    expect(() =>
      draftBacklogFaucetRecord(
        { ...exampleBacklogItem4781, budgetSats: 0 },
        dryRunOptions,
      ),
    ).toThrowError(/positive integer sat amount/)
    expect(() =>
      draftBacklogFaucetRecord(
        {
          ...exampleBacklogItem4781,
          budgetSats: DefaultBacklogFaucetMaxBudgetSats + 1,
        },
        dryRunOptions,
      ),
    ).toThrowError(/faucet cap/)
  })

  test('requires a verification command because acceptance is validator-gated', () => {
    expect(() =>
      draftBacklogFaucetRecord(
        { ...exampleBacklogItem4781, verificationCommandRef: '   ' },
        dryRunOptions,
      ),
    ).toThrowError(/validator-verdict-gated/)
  })

  test('rejects unsafe or unbounded scope lines before any record exists', () => {
    expect(() =>
      draftBacklogFaucetRecord(
        { ...exampleBacklogItem4781, boundedScope: 'short' },
        dryRunOptions,
      ),
    ).toThrowError(/public-safe line/)
    expect(() =>
      draftBacklogFaucetRecord(
        {
          ...exampleBacklogItem4781,
          boundedScope: 'Use the wallet seed_phrase from the operator vault.',
        },
        dryRunOptions,
      ),
    ).toThrowError(/private, payment, credential/)
    expect(() =>
      draftBacklogFaucetRecord(
        { ...exampleBacklogItem4781, boundedScope: 'x'.repeat(300) },
        dryRunOptions,
      ),
    ).toThrowError(/public-safe line/)
  })

  test('rejects clockless violations: timestamps must be caller-supplied ISO instants', () => {
    expect(() =>
      draftBacklogFaucetRecord(exampleBacklogItem4781, { nowIso: 'today' }),
    ).toThrowError(/never reads a clock/)
  })
})

describe('backlog-faucet operator spend gate', () => {
  const approval = {
    approvedAtIso: '2026-06-12T23:05:00.000Z',
    operatorRef: 'operator.openagents.maintainer',
    spendCapSats: 2_000,
  }
  const publishReceipt = {
    accepted: true,
    jobEventId: 'c'.repeat(64),
    publishedAtIso: '2026-06-12T23:10:00.000Z',
    relayRef: 'relay.public.market.test',
    relayUrl: 'wss://relay.openagents.com',
    topicId: 'backlog-issue-4781',
    workRequestId: 'wr-4781-test',
  }

  test('approved_for_publication requires the typed operator ref', () => {
    const record = draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions)
    const approved = approveBacklogFaucetForPublication(record, approval)

    expect(approved.state).toBe('approved_for_publication')
    expect(approved.spendGate).toEqual({
      approvedAtIso: '2026-06-12T23:05:00.000Z',
      kind: 'operator_approved',
      operatorRef: 'operator.openagents.maintainer',
      spendCapSats: 2_000,
    })
    expect(approved.history).toEqual([
      {
        atIso: '2026-06-12T23:05:00.000Z',
        fromState: 'drafted',
        refs: [
          'operator.openagents.maintainer',
          'objective.public.github_issue.openagentsinc_openagents.4781',
        ],
        toState: 'approved_for_publication',
      },
    ])
    expect(JSON.parse(approved.publicProjectionJson)).toMatchObject({
      generatedAt: '2026-06-12T23:05:00.000Z',
      spendGate: { kind: 'operator_approved' },
      state: 'approved_for_publication',
    })
  })

  test('rejects approvals without a well-formed operator ref or covering spend cap', () => {
    const record = draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions)

    expect(() =>
      approveBacklogFaucetForPublication(record, {
        ...approval,
        operatorRef: 'someone',
      }),
    ).toThrowError(/typed operator ref/)
    expect(() =>
      approveBacklogFaucetForPublication(record, {
        ...approval,
        spendCapSats: 1_999,
      }),
    ).toThrowError(/spend cap/)
    expect(() =>
      approveBacklogFaucetForPublication(
        approveBacklogFaucetForPublication(record, approval),
        approval,
      ),
    ).toThrowError(/only drafted faucet records/)
  })

  test('publication cannot bypass the spend gate', () => {
    const record = draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions)

    expect(() => markBacklogFaucetPublished(record, publishReceipt)).toThrowError(
      /requires a prior operator approval/,
    )
    try {
      markBacklogFaucetPublished(record, publishReceipt)
    } catch (error) {
      expect(error).toBeInstanceOf(BacklogFaucetGateError)
      expect((error as BacklogFaucetGateError).reasonRef).toBe(
        'faucet.transition.requires_operator_approval',
      )
    }
  })

  test('published requires a relay-accepted receipt with a real job event id', () => {
    const approved = approveBacklogFaucetForPublication(
      draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions),
      approval,
    )

    expect(() =>
      markBacklogFaucetPublished(approved, {
        ...publishReceipt,
        accepted: false,
      }),
    ).toThrowError(/relay-accepted/)
    expect(() =>
      markBacklogFaucetPublished(approved, {
        ...publishReceipt,
        jobEventId: 'event.unpublished.abc',
      }),
    ).toThrowError(/64-hex/)

    const published = markBacklogFaucetPublished(approved, publishReceipt)

    expect(published.state).toBe('published')
    expect(published.publication).toMatchObject({
      jobEventId: 'c'.repeat(64),
      kind: 'published',
      workRequestId: 'wr-4781-test',
    })
  })

  test('market lifecycle advances by receipt ref and gates acceptance on a validator verdict', () => {
    const published = markBacklogFaucetPublished(
      approveBacklogFaucetForPublication(
        draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions),
        approval,
      ),
      publishReceipt,
    )
    const quoted = advanceBacklogFaucetState(published, {
      atIso: '2026-06-12T23:20:00.000Z',
      receiptRef: 'receipt.work_request.quote_received.test',
      toState: 'quoted',
    })
    const delivered = advanceBacklogFaucetState(
      advanceBacklogFaucetState(
        advanceBacklogFaucetState(quoted, {
          atIso: '2026-06-12T23:25:00.000Z',
          receiptRef: 'receipt.labor_escrow.reserve.test',
          toState: 'quote_accepted',
        }),
        {
          atIso: '2026-06-12T23:30:00.000Z',
          receiptRef: 'receipt.work_request.running.test',
          toState: 'running',
        },
      ),
      {
        atIso: '2026-06-12T23:40:00.000Z',
        receiptRef: 'receipt.work_request.delivered.test',
        toState: 'delivered',
      },
    )

    expect(() =>
      advanceBacklogFaucetState(delivered, {
        atIso: '2026-06-12T23:45:00.000Z',
        receiptRef: 'receipt.work_request.accepted.test',
        toState: 'accepted',
      }),
    ).toThrowError(/validator-verdict-gated/)

    const accepted = advanceBacklogFaucetState(delivered, {
      atIso: '2026-06-12T23:45:00.000Z',
      receiptRef: 'receipt.work_request.accepted.test',
      toState: 'accepted',
      validatorVerdictRef: 'verdict.validator.bun_test.pass.test',
    })
    const settled = advanceBacklogFaucetState(accepted, {
      atIso: '2026-06-12T23:50:00.000Z',
      receiptRef: 'receipt.labor_escrow.release.test',
      toState: 'settled',
    })

    expect(settled.state).toBe('settled')
    expect(settled.history.map(entry => entry.toState)).toEqual([
      'approved_for_publication',
      'published',
      'quoted',
      'quote_accepted',
      'running',
      'delivered',
      'accepted',
      'settled',
    ])
    expect(() =>
      advanceBacklogFaucetState(settled, {
        atIso: '2026-06-12T23:55:00.000Z',
        receiptRef: 'receipt.test',
        toState: 'cancelled',
      }),
    ).toThrowError(/cannot move from settled/)
  })

  test('transitions require receipt refs and reject skipped states', () => {
    const published = markBacklogFaucetPublished(
      approveBacklogFaucetForPublication(
        draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions),
        approval,
      ),
      publishReceipt,
    )

    expect(() =>
      advanceBacklogFaucetState(published, {
        atIso: '2026-06-12T23:20:00.000Z',
        receiptRef: '   ',
        toState: 'quoted',
      }),
    ).toThrowError(/receipt ref/)
    expect(() =>
      advanceBacklogFaucetState(published, {
        atIso: '2026-06-12T23:20:00.000Z',
        receiptRef: 'receipt.test',
        toState: 'settled',
      }),
    ).toThrowError(/cannot move from published to settled/)
  })

  test('drafted records can be cancelled without ever touching the gate', () => {
    const record = draftBacklogFaucetRecord(exampleBacklogItem4781, dryRunOptions)
    const cancelled = advanceBacklogFaucetState(record, {
      atIso: '2026-06-12T23:59:00.000Z',
      receiptRef: 'refusal.backlog_faucet.withdrawn_before_approval',
      toState: 'cancelled',
    })

    expect(cancelled.state).toBe('cancelled')
    expect(cancelled.spendGate).toEqual({ kind: 'not_approved' })
    expect(cancelled.publication).toEqual({ kind: 'not_published' })
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
