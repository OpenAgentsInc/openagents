/**
 * Backlog faucet (#4781): issue→work-request adapter for the open labor
 * market. It decorates budgeted public GitHub issues into ref-only NIP-LBR
 * work-request filings while keeping each source issue in exactly one channel.
 */
import {
  DefaultForumWorkRequestCapabilityRef,
  DefaultForumWorkRequestRelayUrl,
  DefaultForumWorkRequestRepositoryRef,
  type ForumWorkRequestInput,
  type ForumWorkRequestLifecycleKind,
  type ForumWorkRequestState,
  ForumWorkRequestUnsafe,
  buildForumWorkRequestLbrDraft,
  normalizeForumWorkRequestInput,
} from './forum-work-requests'
import {
  type GitHubIssueForMarchingOrders,
  type MarchingOrderIssueProposal,
  proposeMarchingOrderIssues,
} from './marching-orders-agent'

export type BacklogFaucetChannel =
  | 'in_house_work_order'
  | 'open_market_work_request'

export type BacklogFaucetChannelDetection = Readonly<{
  channels: ReadonlyArray<BacklogFaucetChannel>
  marketWorkRequestRefs: ReadonlyArray<string>
}>

export type BacklogFaucetSelectionState =
  | 'candidate'
  | 'needs_human_verification_command'
  | 'skipped'

export type BacklogFaucetIssueProposal = Readonly<{
  channels: ReadonlyArray<BacklogFaucetChannel>
  issueNumber: number
  issueUrl: string
  labels: ReadonlyArray<string>
  reasonRefs: ReadonlyArray<string>
  selectionState: BacklogFaucetSelectionState
  title: string
}>

export type BacklogFaucetFilingConfig = Readonly<{
  budgetSats: number
  deadlineRef: string
  relistGeneration?: number
  repository: string
  requiredCapabilityRefs?: ReadonlyArray<string>
  verificationCommandRef: string
}>

export type BacklogFaucetFiling = Readonly<{
  idempotencyKey: string
  input: ForumWorkRequestInput
  issueNumber: number
  issueUrl: string
  objectiveRef: string
}>

export type BacklogFaucetRelistDecision =
  | Readonly<{ allowed: true; nextGeneration: number }>
  | Readonly<{ allowed: false; reasonRef: string }>

export const backlogFaucetChannelMarker =
  'Channel: openagents.market.work_request'

const marketWorkRequestRefPattern =
  /work_request\.public\.[A-Za-z0-9][A-Za-z0-9._-]*/g
const inHouseWorkOrderMarkerPattern =
  /(^Autopilot marching-orders delivery is ready for human review\.$)|(`autopilot_work_order\.[A-Za-z0-9._-]+`)/mu
const repositoryPattern =
  /^[A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._-]*$/u
const validationTopicId = 'backlog-faucet-decoration-check'

export const detectBacklogIssueChannels = (
  commentBodies: ReadonlyArray<string>,
): BacklogFaucetChannelDetection => {
  const channels = new Set<BacklogFaucetChannel>()
  const marketWorkRequestRefs = new Set<string>()

  commentBodies.forEach(body => {
    if (body.includes(backlogFaucetChannelMarker)) {
      channels.add('open_market_work_request')
      const refs = body.match(marketWorkRequestRefPattern) ?? []
      refs.forEach(ref => marketWorkRequestRefs.add(ref))
    }
    if (inHouseWorkOrderMarkerPattern.test(body)) {
      channels.add('in_house_work_order')
    }
  })

  return {
    channels: [...channels].sort(),
    marketWorkRequestRefs: [...marketWorkRequestRefs].sort(),
  }
}

const channelReasonRefs = (
  detection: BacklogFaucetChannelDetection | undefined,
): ReadonlyArray<string> => [
  ...(detection?.channels.includes('in_house_work_order') === true
    ? ['selection.github_issue.in_house_channel_active']
    : []),
  ...(detection?.channels.includes('open_market_work_request') === true
    ? ['selection.github_issue.already_listed_open_market']
    : []),
]

const withChannelExclusivity = (
  proposal: MarchingOrderIssueProposal,
  detection: BacklogFaucetChannelDetection | undefined,
): BacklogFaucetIssueProposal => {
  const exclusivityReasons = channelReasonRefs(detection)

  if (exclusivityReasons.length > 0) {
    return {
      channels: detection?.channels ?? [],
      issueNumber: proposal.issueNumber,
      issueUrl: proposal.issueUrl,
      labels: proposal.labels,
      reasonRefs: [
        ...new Set([
          ...exclusivityReasons,
          ...(proposal.selectionState === 'skipped' ? proposal.reasonRefs : []),
        ]),
      ].sort(),
      selectionState: 'skipped',
      title: proposal.title,
    }
  }

  return {
    channels: detection?.channels ?? [],
    issueNumber: proposal.issueNumber,
    issueUrl: proposal.issueUrl,
    labels: proposal.labels,
    reasonRefs: proposal.reasonRefs,
    selectionState: proposal.selectionState,
    title: proposal.title,
  }
}

export const proposeBacklogFaucetIssues = (
  issues: ReadonlyArray<GitHubIssueForMarchingOrders>,
  options: Readonly<{
    channelDetections?: ReadonlyMap<number, BacklogFaucetChannelDetection>
    limit?: number
    skipLabels?: ReadonlyArray<string>
  }> = {},
): ReadonlyArray<BacklogFaucetIssueProposal> =>
  proposeMarchingOrderIssues(issues, {
    ...(options.limit === undefined ? {} : { limit: options.limit }),
    ...(options.skipLabels === undefined
      ? {}
      : { skipLabels: options.skipLabels }),
  }).map(proposal =>
    withChannelExclusivity(
      proposal,
      options.channelDetections?.get(proposal.issueNumber),
    ),
  )

const repositorySegment = (repository: string): string =>
  repository
    .replace('/', '_')
    .replace(/[^A-Za-z0-9_-]+/gu, '_')
    .toLowerCase()

export const backlogFaucetObjectiveRef = (
  repository: string,
  issueNumber: number,
): string =>
  `objective.public.github_issue.${repositorySegment(repository)}.${issueNumber}`

export const backlogFaucetIssueUrlForObjectiveRef = (
  repository: string,
  issueNumber: number,
): string => `https://github.com/${repository}/issues/${issueNumber}`

export const backlogFaucetRepositoryRef = (repository: string): string =>
  repository === 'OpenAgentsInc/openagents'
    ? DefaultForumWorkRequestRepositoryRef
    : `repo.public.github.${repositorySegment(repository)}`

export const backlogFaucetIdempotencyKey = (
  repository: string,
  issueNumber: number,
  relistGeneration: number,
): string =>
  relistGeneration === 0
    ? `backlog-faucet:${repository}:${issueNumber}`
    : `backlog-faucet:${repository}:${issueNumber}:relist-${relistGeneration}`

export const backlogFaucetDeadlineRef = (isoDate: string): string => {
  const compactDate = isoDate.slice(0, 10).replaceAll('-', '')

  if (!/^\d{8}$/u.test(compactDate)) {
    throw new ForumWorkRequestUnsafe(
      'deadline date must be an ISO date such as 2026-06-18.',
    )
  }

  return `deadline.public.backlog_faucet.${compactDate}`
}

const boundedFilingTitle = (issue: GitHubIssueForMarchingOrders): string => {
  const prefix = `Backlog issue #${issue.number}: `
  const remaining = Math.max(8, 160 - prefix.length)

  return `${prefix}${issue.title.trim().slice(0, remaining)}`.trim()
}

export const buildBacklogWorkRequestFiling = (
  issue: GitHubIssueForMarchingOrders,
  config: BacklogFaucetFilingConfig,
): BacklogFaucetFiling => {
  const relistGeneration = config.relistGeneration ?? 0

  if (issue.state !== 'open') {
    throw new ForumWorkRequestUnsafe(
      'closed GitHub issues cannot be listed as open-market work requests.',
    )
  }

  if (!repositoryPattern.test(config.repository)) {
    throw new ForumWorkRequestUnsafe(
      'repository must be an owner/name GitHub slug.',
    )
  }

  if (!Number.isInteger(relistGeneration) || relistGeneration < 0) {
    throw new ForumWorkRequestUnsafe(
      'relistGeneration must be a non-negative integer.',
    )
  }

  const objectiveRef = backlogFaucetObjectiveRef(
    config.repository,
    issue.number,
  )
  const input: ForumWorkRequestInput = {
    budgetSats: config.budgetSats,
    deadlineRef: config.deadlineRef,
    objectiveRef,
    repositoryRefs: [backlogFaucetRepositoryRef(config.repository)],
    requiredCapabilityRefs:
      config.requiredCapabilityRefs === undefined ||
      config.requiredCapabilityRefs.length === 0
        ? [DefaultForumWorkRequestCapabilityRef]
        : config.requiredCapabilityRefs,
    title: boundedFilingTitle(issue),
    verificationCommandRef: config.verificationCommandRef,
  }
  const normalized = normalizeForumWorkRequestInput(input)

  buildForumWorkRequestLbrDraft(normalized, {
    relayUrl: DefaultForumWorkRequestRelayUrl,
    topicId: validationTopicId,
  })

  return {
    idempotencyKey: backlogFaucetIdempotencyKey(
      config.repository,
      issue.number,
      relistGeneration,
    ),
    input,
    issueNumber: issue.number,
    issueUrl: issue.html_url,
    objectiveRef,
  }
}

export const backlogFaucetRelistDecision = (
  state: ForumWorkRequestState,
  currentGeneration: number,
): BacklogFaucetRelistDecision =>
  state === 'expired' || state === 'cancelled'
    ? { allowed: true, nextGeneration: currentGeneration + 1 }
    : {
        allowed: false,
        reasonRef: `relist.work_request_state_not_terminal.${state}`,
      }

export const listedIssueCommentBody = (
  input: Readonly<{
    budgetSats: number
    deadlineRef: string
    jobEventId: string
    objectiveRef: string
    topicSlug: string
    verificationCommandRef: string
    workRequestId: string
  }>,
): string =>
  [
    'This issue is listed on the open agent labor market as a NIP-LBR work request.',
    '',
    backlogFaucetChannelMarker,
    '',
    `- Work request ref: \`work_request.public.${input.workRequestId}\``,
    `- Forum topic: https://openagents.com/forum/f/work-requests/t/${input.topicSlug}`,
    `- Job event ref: \`nostr.event.${input.jobEventId}\``,
    `- Objective ref: \`${input.objectiveRef}\``,
    `- Budget: ${input.budgetSats} sats max bid. Escrow reserves on quote acceptance and is refundable until then; nothing here is settled bitcoin until a settlement receipt exists.`,
    `- Verification command ref: \`${input.verificationCommandRef}\``,
    `- Deadline ref: \`${input.deadlineRef}\``,
    '',
    'Any capability-true provider can quote it through the Forum work-requests surface or the kind-5934 relay twin.',
    'While listed on the open market, this issue must not also be filed as an in-house Autopilot work order (one channel at a time).',
  ].join('\n')

export const lifecycleIssueCommentBody = (
  input: Readonly<{
    lifecycleKind: ForumWorkRequestLifecycleKind
    receiptRef: string
    workRequestId: string
  }>,
): string => {
  if (input.receiptRef.trim() === '') {
    throw new ForumWorkRequestUnsafe(
      'lifecycle issue comments require a receipt ref.',
    )
  }

  return [
    `Open-market work request lifecycle update: ${input.lifecycleKind}`,
    '',
    `- Work request ref: \`work_request.public.${input.workRequestId}\``,
    `- Receipt ref: \`${input.receiptRef}\``,
    '',
    'The receipt ref is the authority for this transition; this comment is a mirror, not an acceptance or settlement claim.',
  ].join('\n')
}

export const parseBacklogFaucetBudgetAssignments = (
  value: string,
): ReadonlyMap<number, number> => {
  const entries = value
    .split(',')
    .map(item => item.trim())
    .filter(item => item !== '')
    .map((item): readonly [number, number] => {
      const match = /^(\d+)=(\d+)$/u.exec(item)

      if (match === null) {
        throw new ForumWorkRequestUnsafe(
          `budget assignment "${item}" must use <issueNumber>=<budgetSats>.`,
        )
      }

      return [Number(match[1]), Number(match[2])]
    })

  if (entries.length === 0) {
    throw new ForumWorkRequestUnsafe(
      'at least one <issueNumber>=<budgetSats> assignment is required.',
    )
  }

  return new Map(entries)
}
