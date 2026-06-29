import {
  assertOpenAgentsAutopilotWorkRequest,
  type OpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'

export type GitHubIssueLabel = Readonly<{
  name: string
}>

export type GitHubIssueForMarchingOrders = Readonly<{
  body?: string | null
  html_url: string
  labels: ReadonlyArray<string | GitHubIssueLabel>
  number: number
  pull_request?: unknown
  state: string
  title: string
}>

export type MarchingOrderSelectionState =
  | 'candidate'
  | 'needs_human_verification_command'
  | 'skipped'

export type MarchingOrderIssueProposal = Readonly<{
  issueNumber: number
  issueUrl: string
  labels: ReadonlyArray<string>
  reasonRefs: ReadonlyArray<string>
  selectionState: MarchingOrderSelectionState
  title: string
}>

export type MarchingOrderVerificationCommand = Readonly<{
  args: ReadonlyArray<string>
  commandRef: string
}>

export type MarchingOrderSubmitConfig = Readonly<{
  agentId: string
  agentWalletRef?: string
  baseUrl: string
  branch: string
  commitSha: string
  ownerRef: string
  pylonId?: string
  repository: string
  verificationCommand: MarchingOrderVerificationCommand
}>

export type MarchingOrderSubmission = Readonly<{
  idempotencyKey: string
  issueNumber: number
  request: OpenAgentsAutopilotWorkRequest
}>

const safeSegmentPattern = /[^A-Za-z0-9_.-]+/g
const unsafeIssueTextPattern =
  /(access[_-]?token|bearer\s+|credential|customer[_-]?email|email[_-]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?(home|material|mnemonic|path|private|secret|state)|webhook[_-]?secret)/iu

export const defaultMarchingOrderSkipLabels = [
  'blocked',
  'discussion',
  'master-task',
  'needs-owner',
  'private-context',
  'question',
] as const

const labelNames = (
  labels: ReadonlyArray<string | GitHubIssueLabel>,
): ReadonlyArray<string> =>
  labels
    .map(label => (typeof label === 'string' ? label : label.name))
    .map(label => label.trim())
    .filter(label => label !== '')

const safeSegment = (value: string): string =>
  value
    .trim()
    .replace(safeSegmentPattern, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80)
    .toLowerCase()

const issueLooksRoadmapped = (
  issue: GitHubIssueForMarchingOrders,
  labels: ReadonlyArray<string>,
): boolean =>
  labels.some(label => label.toLowerCase() === 'roadmap') ||
  /^(B|M|A|P|CX)\d+:/u.test(issue.title.trim())

const issueLooksCodeBounded = (
  issue: GitHubIssueForMarchingOrders,
): boolean =>
  /(test|smoke|route|api|worker|pylon|script|cli|docs|ui|command|verification|contract|schema|work order|assignment)/iu
    .test(`${issue.title}\n${issue.body ?? ''}`)

const issueContainsExplicitVerification = (
  issue: GitHubIssueForMarchingOrders,
): boolean =>
  /(verification command|bun test|vitest|npm test|pnpm test|cargo test|go test|pytest)/iu
    .test(`${issue.title}\n${issue.body ?? ''}`)

export const proposeMarchingOrderIssues = (
  issues: ReadonlyArray<GitHubIssueForMarchingOrders>,
  options: Readonly<{
    limit?: number
    skipLabels?: ReadonlyArray<string>
  }> = {},
): ReadonlyArray<MarchingOrderIssueProposal> => {
  const skipLabels = new Set(
    (options.skipLabels ?? defaultMarchingOrderSkipLabels).map(label =>
      label.toLowerCase(),
    ),
  )

  return issues
    .filter(issue => issue.state === 'open' && issue.pull_request === undefined)
    .map(issue => {
      const labels = labelNames(issue.labels)
      const lowerLabels = new Set(labels.map(label => label.toLowerCase()))
      const reasonRefs = new Set<string>()
      const text = `${issue.title}\n${issue.body ?? ''}`

      if ([...lowerLabels].some(label => skipLabels.has(label))) {
        reasonRefs.add('selection.github_issue.skip_label')
      }
      if (unsafeIssueTextPattern.test(text)) {
        reasonRefs.add('selection.github_issue.private_or_secret_context')
      }
      if (!issueLooksRoadmapped(issue, labels)) {
        reasonRefs.add('selection.github_issue.not_roadmap_scoped')
      }
      if (!issueLooksCodeBounded(issue)) {
        reasonRefs.add('selection.github_issue.not_bounded_code_work')
      }

      if (reasonRefs.size > 0) {
        return {
          issueNumber: issue.number,
          issueUrl: issue.html_url,
          labels,
          reasonRefs: [...reasonRefs].sort(),
          selectionState: 'skipped',
          title: issue.title,
        } satisfies MarchingOrderIssueProposal
      }

      if (!issueContainsExplicitVerification(issue)) {
        reasonRefs.add('selection.github_issue.needs_human_verification_command')
        return {
          issueNumber: issue.number,
          issueUrl: issue.html_url,
          labels,
          reasonRefs: [...reasonRefs],
          selectionState: 'needs_human_verification_command',
          title: issue.title,
        } satisfies MarchingOrderIssueProposal
      }

      return {
        issueNumber: issue.number,
        issueUrl: issue.html_url,
        labels,
        reasonRefs: ['selection.github_issue.bounded_public_safe_candidate'],
        selectionState: 'candidate',
        title: issue.title,
      } satisfies MarchingOrderIssueProposal
    })
    .slice(0, options.limit ?? issues.length)
}

export const buildMarchingOrderSubmission = (
  issue: GitHubIssueForMarchingOrders,
  config: MarchingOrderSubmitConfig,
): MarchingOrderSubmission => {
  const issueRefSegment = `issue_${issue.number}`
  const commitSegment = config.commitSha.slice(0, 12)
  const titleSegment = safeSegment(issue.title) || 'untitled'
  const taskRef = `task.github_issue.${issueRefSegment}.${titleSegment}`
  const idempotencyKey =
    `marching-orders:${config.repository}:${issue.number}:${config.commitSha}`
  const request = {
    caller: {
      agentId: config.agentId,
      ...(config.agentWalletRef === undefined
        ? {}
        : { agentWalletRef: config.agentWalletRef }),
      kind: 'registered_agent',
      ownerRef: config.ownerRef,
      ...(config.pylonId === undefined ? {} : { pylonId: config.pylonId }),
    },
    clientRequestRef:
      `client.openagents.marching_orders.${issueRefSegment}.${commitSegment}`,
    intent: 'delegate_to_autopilot',
    mode: 'free_slice',
    paymentPolicy: {
      buyerPaymentMode: 'free_slice',
      maxSpendCents: 0,
      quoteRef: null,
      quotedAmountCents: null,
      settlementMode: 'no_worker_payout',
    },
    placementPolicy: {
      allowedRunnerKinds: ['requester_pylon', 'openagents_shc'],
      disallowedRunnerKinds: [],
      localOnlyAllowed: false,
      preferredRunnerKinds: ['requester_pylon'],
      privacyTier: 'public_beta',
      publicTraceAllowed: true,
      requiresSecretBroker: false,
    },
    schema: 'openagents.autopilot_work_request.v1',
    tasks: [
      {
        acceptanceCriteriaRefs: [
          `acceptance.github_issue.${issueRefSegment}.verification_command_passes`,
          `acceptance.github_issue.${issueRefSegment}.public_safe_closeout`,
          'acceptance.github_issue.no_self_acceptance',
        ],
        accessRequests: [
          {
            kind: 'github_repo_read',
            reasonRef: `access.github_issue.${issueRefSegment}.public_repo_read`,
          },
        ],
        checkout: {
          commitSha: config.commitSha,
          kind: 'git_checkout',
          verificationCommand: {
            args: [...config.verificationCommand.args],
            commandRef: config.verificationCommand.commandRef,
          },
        },
        forumReporting: {
          mode: 'operator_approved_only',
        },
        kind: 'code_change',
        objective:
          `Resolve ${config.repository} issue #${issue.number}: ${issue.title}. ` +
          `Issue URL: ${issue.html_url}. ` +
          'Return public-safe refs only; do not review or accept this work order.',
        repository: {
          branch: config.branch,
          fullName: config.repository,
          provider: 'github',
          visibility: 'public',
        },
        taskRef,
      },
    ],
  } satisfies OpenAgentsAutopilotWorkRequest

  assertOpenAgentsAutopilotWorkRequest(request)

  return {
    idempotencyKey,
    issueNumber: issue.number,
    request,
  }
}

export const deliveredIssueCommentBody = (
  input: Readonly<{
    closeoutRefs: ReadonlyArray<string>
    resultRefs: ReadonlyArray<string>
    testRefs: ReadonlyArray<string>
    workOrderRef: string
  }>,
): string =>
  [
    'Autopilot marching-orders delivery is ready for human review.',
    '',
    `- Work order: \`${input.workOrderRef}\``,
    `- Closeout refs: ${input.closeoutRefs.map(ref => `\`${ref}\``).join(', ') || '_none_'}`,
    `- Result refs: ${input.resultRefs.map(ref => `\`${ref}\``).join(', ') || '_none_'}`,
    `- Test refs: ${input.testRefs.map(ref => `\`${ref}\``).join(', ') || '_none_'}`,
    '',
    'No self-acceptance: this comment is a delivery pointer only; a human owner must review the work order.',
  ].join('\n')
