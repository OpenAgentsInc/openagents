/**
 * Backlog faucet (#4781): issue→work-request adapter for the open labor
 * market. It decorates budgeted public GitHub issues into ref-only NIP-LBR
 * work-request filings while keeping each source issue in exactly one channel.
 */
import type { LbrUnsignedEventDraft } from '@openagentsinc/nip90'

import {
  DefaultForumWorkRequestCapabilityRef,
  DefaultForumWorkRequestRelayUrl,
  DefaultForumWorkRequestRepositoryRef,
  type ForumWorkRequestInput,
  type ForumWorkRequestLifecycleKind,
  type ForumWorkRequestState,
  ForumWorkRequestUnsafe,
  assertPublicSafeWorkRequestMaterial,
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

// ---------------------------------------------------------------------------
// Faucet record contract (#4781): a budgeted backlog item becomes a validated
// NIP-LBR work-request payload plus a typed faucet record whose lifecycle is
// `drafted → approved_for_publication → published → quoted → ...`. The
// `approved_for_publication` transition is the spend gate: it exists only
// through `approveBacklogFaucetForPublication`, which requires a typed
// operator ref and a spend cap covering the budget. The dry-run path stops at
// `drafted` by construction — it never publishes and never touches escrow.
// ---------------------------------------------------------------------------

export type BacklogFaucetBudgetedItem = Readonly<{
  boundedScope: string
  budgetSats: number
  deadlineDate: string
  issueRef: string
  relistGeneration?: number
  requiredCapabilityRefs?: ReadonlyArray<string>
  title: string
  verificationCommandRef: string
}>

export type BacklogFaucetRecordState =
  | 'drafted'
  | 'approved_for_publication'
  | 'published'
  | 'quoted'
  | 'quote_accepted'
  | 'running'
  | 'delivered'
  | 'accepted'
  | 'settled'
  | 'expired'
  | 'cancelled'

export type BacklogFaucetSpendGate =
  | Readonly<{ kind: 'not_approved' }>
  | Readonly<{
      approvedAtIso: string
      kind: 'operator_approved'
      operatorRef: string
      spendCapSats: number
    }>

export type BacklogFaucetPublication =
  | Readonly<{ kind: 'not_published' }>
  | Readonly<{
      jobEventId: string
      kind: 'published'
      publishedAtIso: string
      relayRef: string
      relayUrl: string
      topicId: string
      workRequestId: string
    }>

export type BacklogFaucetTransition = Readonly<{
  atIso: string
  fromState: BacklogFaucetRecordState
  refs: ReadonlyArray<string>
  toState: BacklogFaucetRecordState
}>

export type BacklogFaucetRecord = Readonly<{
  boundedScope: string
  createdAtIso: string
  filing: BacklogFaucetFiling
  history: ReadonlyArray<BacklogFaucetTransition>
  previewDraft: LbrUnsignedEventDraft
  publication: BacklogFaucetPublication
  publicProjectionJson: string
  relayUrl: string
  repository: string
  spendGate: BacklogFaucetSpendGate
  state: BacklogFaucetRecordState
  updatedAtIso: string
}>

export type BacklogFaucetOperatorApproval = Readonly<{
  approvedAtIso: string
  operatorRef: string
  spendCapSats: number
}>

export type BacklogFaucetPublishReceipt = Readonly<{
  accepted: boolean
  jobEventId: string
  publishedAtIso: string
  relayRef: string
  relayUrl: string
  topicId: string
  workRequestId: string
}>

export type BacklogFaucetMarketState =
  | 'quoted'
  | 'quote_accepted'
  | 'running'
  | 'delivered'
  | 'accepted'
  | 'settled'
  | 'expired'
  | 'cancelled'

export type BacklogFaucetMarketTransitionInput = Readonly<{
  atIso: string
  receiptRef: string
  toState: BacklogFaucetMarketState
  validatorVerdictRef?: string
}>

export type BacklogFaucetDryRunReport = Readonly<{
  escrowed: false
  previewDraft: LbrUnsignedEventDraft
  published: false
  record: BacklogFaucetRecord
}>

export class BacklogFaucetGateError extends Error {
  readonly _tag = 'BacklogFaucetGateError'
  readonly reasonRef: string

  constructor(reasonRef: string, message: string) {
    super(message)
    this.name = 'BacklogFaucetGateError'
    this.reasonRef = reasonRef
  }
}

export const DefaultBacklogFaucetMaxBudgetSats = 50_000
export const MaxBacklogFaucetHistoryEntries = 32
export const MaxBacklogFaucetBoundedScopeLength = 280
export const MinBacklogFaucetBoundedScopeLength = 8

const backlogIssueRefPattern =
  /^https:\/\/github\.com\/([A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._-]*)\/issues\/([1-9]\d*)$/u
const operatorRefPattern = /^operator\.[A-Za-z0-9][A-Za-z0-9._-]*$/u
const jobEventIdPattern = /^[0-9a-f]{64}$/u
const isoInstantPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/u

export const parseBacklogFaucetIssueRef = (
  issueRef: string,
): Readonly<{ issueNumber: number; repository: string }> => {
  const match = backlogIssueRefPattern.exec(issueRef.trim())
  const repository = match?.[1]
  const issueNumber = match?.[2]

  if (repository === undefined || issueNumber === undefined) {
    throw new BacklogFaucetGateError(
      'faucet.item.issue_ref_invalid',
      'issueRef must be a public GitHub issue URL such as https://github.com/OpenAgentsInc/openagents/issues/4781.',
    )
  }

  return { issueNumber: Number(issueNumber), repository }
}

const validatedIsoInstant = (value: string, reasonRef: string): string => {
  if (!isoInstantPattern.test(value)) {
    throw new BacklogFaucetGateError(
      reasonRef,
      'timestamps must be caller-supplied ISO instants; this module never reads a clock.',
    )
  }

  return value
}

const validatedBoundedScope = (boundedScope: string): string => {
  const scope = boundedScope.trim()

  if (
    scope.length < MinBacklogFaucetBoundedScopeLength ||
    scope.length > MaxBacklogFaucetBoundedScopeLength ||
    scope.includes('\n')
  ) {
    throw new BacklogFaucetGateError(
      'faucet.item.bounded_scope_invalid',
      `boundedScope must be a single ${MinBacklogFaucetBoundedScopeLength}-${MaxBacklogFaucetBoundedScopeLength} character public-safe line.`,
    )
  }

  assertPublicSafeWorkRequestMaterial(scope, 'boundedScope')

  return scope
}

const validatedFaucetBudgetSats = (
  budgetSats: number,
  maxBudgetSats: number,
): number => {
  if (!Number.isInteger(budgetSats) || budgetSats <= 0) {
    throw new BacklogFaucetGateError(
      'faucet.item.budget_not_positive',
      'budgetSats must be a positive integer sat amount.',
    )
  }

  if (budgetSats > maxBudgetSats) {
    throw new BacklogFaucetGateError(
      'faucet.item.budget_exceeds_max',
      `budgetSats must not exceed the faucet cap of ${maxBudgetSats} sats.`,
    )
  }

  return budgetSats
}

const backlogFaucetPublicProjectionJson = (
  record: Omit<BacklogFaucetRecord, 'publicProjectionJson'>,
  generatedAt: string,
): string => {
  const projection = {
    boundedScope: record.boundedScope,
    budgetSats: record.filing.input.budgetSats,
    deadlineRef: record.filing.input.deadlineRef,
    generatedAt,
    issueRef: record.filing.issueUrl,
    objectiveRef: record.filing.objectiveRef,
    publication:
      record.publication.kind === 'published'
        ? {
            jobEventRef: `nostr.event.${record.publication.jobEventId}`,
            kind: 'published',
            relayRef: record.publication.relayRef,
            workRequestRef: `work_request.public.${record.publication.workRequestId}`,
          }
        : { kind: 'not_published' },
    spendGate:
      record.spendGate.kind === 'operator_approved'
        ? {
            kind: 'operator_approved',
            operatorRef: record.spendGate.operatorRef,
            spendCapSats: record.spendGate.spendCapSats,
          }
        : { kind: 'not_approved' },
    stalenessContract: 'rebuilt_on_faucet_state_transition',
    state: record.state,
    title: record.filing.input.title,
    verificationCommandRef: record.filing.input.verificationCommandRef,
    workRequestKind: record.previewDraft.kind,
  }

  assertPublicSafeWorkRequestMaterial(projection, 'faucet public projection')

  return JSON.stringify(projection)
}

const withFaucetTransition = (
  record: BacklogFaucetRecord,
  transition: Readonly<{
    atIso: string
    publication?: BacklogFaucetPublication
    refs: ReadonlyArray<string>
    spendGate?: BacklogFaucetSpendGate
    toState: BacklogFaucetRecordState
  }>,
): BacklogFaucetRecord => {
  if (record.history.length >= MaxBacklogFaucetHistoryEntries) {
    throw new BacklogFaucetGateError(
      'faucet.history.bounded',
      `faucet records keep at most ${MaxBacklogFaucetHistoryEntries} transitions.`,
    )
  }

  const next: Omit<BacklogFaucetRecord, 'publicProjectionJson'> = {
    boundedScope: record.boundedScope,
    createdAtIso: record.createdAtIso,
    filing: record.filing,
    history: [
      ...record.history,
      {
        atIso: transition.atIso,
        fromState: record.state,
        refs: transition.refs,
        toState: transition.toState,
      },
    ],
    previewDraft: record.previewDraft,
    publication: transition.publication ?? record.publication,
    relayUrl: record.relayUrl,
    repository: record.repository,
    spendGate: transition.spendGate ?? record.spendGate,
    state: transition.toState,
    updatedAtIso: transition.atIso,
  }

  return {
    ...next,
    publicProjectionJson: backlogFaucetPublicProjectionJson(
      next,
      transition.atIso,
    ),
  }
}

export const draftBacklogFaucetRecord = (
  item: BacklogFaucetBudgetedItem,
  options: Readonly<{
    maxBudgetSats?: number
    nowIso: string
    relayUrl?: string
  }>,
): BacklogFaucetRecord => {
  const nowIso = validatedIsoInstant(
    options.nowIso,
    'faucet.item.now_iso_invalid',
  )
  const boundedScope = validatedBoundedScope(item.boundedScope)
  const budgetSats = validatedFaucetBudgetSats(
    item.budgetSats,
    options.maxBudgetSats ?? DefaultBacklogFaucetMaxBudgetSats,
  )

  if (item.verificationCommandRef.trim() === '') {
    throw new BacklogFaucetGateError(
      'faucet.item.verification_command_required',
      'verificationCommandRef is required; acceptance stays validator-verdict-gated.',
    )
  }

  const parsed = parseBacklogFaucetIssueRef(item.issueRef)
  const relayUrl = options.relayUrl ?? DefaultForumWorkRequestRelayUrl
  const filing = buildBacklogWorkRequestFiling(
    {
      body: null,
      html_url: backlogFaucetIssueUrlForObjectiveRef(
        parsed.repository,
        parsed.issueNumber,
      ),
      labels: [],
      number: parsed.issueNumber,
      state: 'open',
      title: item.title,
    },
    {
      budgetSats,
      deadlineRef: backlogFaucetDeadlineRef(item.deadlineDate),
      ...(item.relistGeneration === undefined
        ? {}
        : { relistGeneration: item.relistGeneration }),
      repository: parsed.repository,
      ...(item.requiredCapabilityRefs === undefined
        ? {}
        : { requiredCapabilityRefs: item.requiredCapabilityRefs }),
      verificationCommandRef: item.verificationCommandRef,
    },
  )
  const previewDraft = buildForumWorkRequestLbrDraft(
    normalizeForumWorkRequestInput(filing.input),
    {
      relayUrl,
      topicId: `backlog-faucet-preview-${parsed.issueNumber}`,
    },
  ).draft

  const record: Omit<BacklogFaucetRecord, 'publicProjectionJson'> = {
    boundedScope,
    createdAtIso: nowIso,
    filing,
    history: [],
    previewDraft,
    publication: { kind: 'not_published' },
    relayUrl,
    repository: parsed.repository,
    spendGate: { kind: 'not_approved' },
    state: 'drafted',
    updatedAtIso: nowIso,
  }

  return {
    ...record,
    publicProjectionJson: backlogFaucetPublicProjectionJson(record, nowIso),
  }
}

export const dryRunBacklogFaucetItem = (
  item: BacklogFaucetBudgetedItem,
  options: Readonly<{
    maxBudgetSats?: number
    nowIso: string
    relayUrl?: string
  }>,
): BacklogFaucetDryRunReport => {
  const record = draftBacklogFaucetRecord(item, options)

  return {
    escrowed: false,
    previewDraft: record.previewDraft,
    published: false,
    record,
  }
}

export const approveBacklogFaucetForPublication = (
  record: BacklogFaucetRecord,
  approval: BacklogFaucetOperatorApproval,
): BacklogFaucetRecord => {
  if (record.state !== 'drafted') {
    throw new BacklogFaucetGateError(
      `faucet.transition.invalid.${record.state}.approved_for_publication`,
      'only drafted faucet records can be approved for publication.',
    )
  }

  if (!operatorRefPattern.test(approval.operatorRef)) {
    throw new BacklogFaucetGateError(
      'faucet.spend_gate.operator_ref_required',
      'approval requires a typed operator ref such as operator.openagents.maintainer.',
    )
  }

  assertPublicSafeWorkRequestMaterial(approval, 'operator approval')

  if (
    !Number.isInteger(approval.spendCapSats) ||
    approval.spendCapSats < record.filing.input.budgetSats
  ) {
    throw new BacklogFaucetGateError(
      'faucet.spend_gate.spend_cap_below_budget',
      'the operator spend cap must be an integer covering the filing budget.',
    )
  }

  const approvedAtIso = validatedIsoInstant(
    approval.approvedAtIso,
    'faucet.spend_gate.approved_at_invalid',
  )

  return withFaucetTransition(record, {
    atIso: approvedAtIso,
    refs: [approval.operatorRef, record.filing.objectiveRef],
    spendGate: {
      approvedAtIso,
      kind: 'operator_approved',
      operatorRef: approval.operatorRef,
      spendCapSats: approval.spendCapSats,
    },
    toState: 'approved_for_publication',
  })
}

export const markBacklogFaucetPublished = (
  record: BacklogFaucetRecord,
  receipt: BacklogFaucetPublishReceipt,
): BacklogFaucetRecord => {
  if (record.state !== 'approved_for_publication') {
    throw new BacklogFaucetGateError(
      record.state === 'drafted'
        ? 'faucet.transition.requires_operator_approval'
        : `faucet.transition.invalid.${record.state}.published`,
      'publication requires a prior operator approval transition.',
    )
  }

  if (record.spendGate.kind !== 'operator_approved') {
    throw new BacklogFaucetGateError(
      'faucet.spend_gate.not_approved',
      'publication requires the operator spend gate to be approved.',
    )
  }

  if (!receipt.accepted) {
    throw new BacklogFaucetGateError(
      'faucet.publish.relay_rejected',
      'only relay-accepted publish receipts can mark a faucet record published.',
    )
  }

  if (!jobEventIdPattern.test(receipt.jobEventId)) {
    throw new BacklogFaucetGateError(
      'faucet.publish.job_event_id_invalid',
      'publish receipts must carry the 64-hex relay job event id.',
    )
  }

  assertPublicSafeWorkRequestMaterial(receipt, 'publish receipt')

  const publishedAtIso = validatedIsoInstant(
    receipt.publishedAtIso,
    'faucet.publish.published_at_invalid',
  )

  return withFaucetTransition(record, {
    atIso: publishedAtIso,
    publication: {
      jobEventId: receipt.jobEventId,
      kind: 'published',
      publishedAtIso,
      relayRef: receipt.relayRef,
      relayUrl: receipt.relayUrl,
      topicId: receipt.topicId,
      workRequestId: receipt.workRequestId,
    },
    refs: [
      `nostr.event.${receipt.jobEventId}`,
      `work_request.public.${receipt.workRequestId}`,
      receipt.relayRef,
    ],
    toState: 'published',
  })
}

const allowedBacklogFaucetMarketTransitions: Readonly<
  Record<BacklogFaucetRecordState, ReadonlyArray<BacklogFaucetMarketState>>
> = {
  accepted: ['settled'],
  approved_for_publication: ['cancelled'],
  cancelled: [],
  delivered: ['accepted', 'cancelled'],
  drafted: ['cancelled'],
  expired: [],
  published: ['quoted', 'expired', 'cancelled'],
  quote_accepted: ['running', 'expired', 'cancelled'],
  quoted: ['quote_accepted', 'expired', 'cancelled'],
  running: ['delivered', 'cancelled'],
  settled: [],
}

export const advanceBacklogFaucetState = (
  record: BacklogFaucetRecord,
  input: BacklogFaucetMarketTransitionInput,
): BacklogFaucetRecord => {
  const receiptRef = input.receiptRef.trim()

  if (
    !allowedBacklogFaucetMarketTransitions[record.state].includes(input.toState)
  ) {
    throw new BacklogFaucetGateError(
      `faucet.transition.invalid.${record.state}.${input.toState}`,
      `faucet records cannot move from ${record.state} to ${input.toState}.`,
    )
  }

  if (receiptRef === '') {
    throw new BacklogFaucetGateError(
      'faucet.transition.receipt_ref_required',
      'every faucet market transition requires a public-safe receipt ref.',
    )
  }

  const validatorVerdictRef = input.validatorVerdictRef?.trim() ?? ''

  if (input.toState === 'accepted' && validatorVerdictRef === '') {
    throw new BacklogFaucetGateError(
      'faucet.acceptance.validator_verdict_required',
      'acceptance is validator-verdict-gated; a validator verdict ref is required.',
    )
  }

  const refs =
    input.toState === 'accepted'
      ? [receiptRef, validatorVerdictRef]
      : [receiptRef]

  assertPublicSafeWorkRequestMaterial(refs, 'faucet transition refs')

  return withFaucetTransition(record, {
    atIso: validatedIsoInstant(
      input.atIso,
      'faucet.transition.at_iso_invalid',
    ),
    refs,
    toState: input.toState,
  })
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
