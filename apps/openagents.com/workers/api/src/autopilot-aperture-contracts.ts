import { Schema as S } from 'effect'

import { CodingAutopilotArtifactRecord } from './coding-autopilot-artifacts'
import { CodingAutopilotRepoPlacementRecord } from './coding-autopilot-repo-placement'
import { GitHubWritebackOperation } from './github-writeback-authority'
import type {
  GitHubWritebackAuthorityDecision,
  GitHubWritebackOperation as GitHubWritebackOperationType,
} from './github-writeback-authority'

export const AutopilotMissionFrontDoor = S.Literals([
  'agent_api',
  'autonomic_tick',
  'forum',
  'terminal',
  'web_composer',
  'workroom',
])
export type AutopilotMissionFrontDoor = typeof AutopilotMissionFrontDoor.Type

export const AutopilotMissionWorkOrderStatus = S.Literals([
  'accepted',
  'blocked',
  'delivered',
  'proposed',
  'queued',
  'running',
  'waiting_for_review',
])
export type AutopilotMissionWorkOrderStatus =
  typeof AutopilotMissionWorkOrderStatus.Type

export class AutopilotMissionWorkOrderLink extends S.Class<AutopilotMissionWorkOrderLink>(
  'AutopilotMissionWorkOrderLink',
)({
  artifactRefs: S.Array(S.String),
  briefingRefs: S.Array(S.String),
  canonicalRef: S.String,
  continuationRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataScopeRef: S.String,
  decisionActionRefs: S.Array(S.String),
  frontDoor: AutopilotMissionFrontDoor,
  id: S.String,
  missionRef: S.String,
  placementRecordRef: S.String,
  receiptRefs: S.Array(S.String),
  status: AutopilotMissionWorkOrderStatus,
  updatedAtIso: S.String,
  workOrderRef: S.String,
}) {}

export class AutopilotPlacementExplanation extends S.Class<AutopilotPlacementExplanation>(
  'AutopilotPlacementExplanation',
)({
  blockerRefs: S.Array(S.String),
  customerSafeReasonRefs: S.Array(S.String),
  decision: S.Literals([
    'blocked',
    'eligible',
    'needs_customer_grant',
    'needs_operator_approval',
    'needs_provider_grant',
  ]),
  generatedAt: S.String,
  missionRef: S.String,
  placementRecordRef: S.String,
  policyRefs: S.Array(S.String),
  repoRef: S.String,
  runnerBackendKind: S.String,
  staleness: S.Literal('fresh_until_placement_or_scope_transition'),
  trustTier: S.String,
}) {}

export class AutopilotWritebackPlan extends S.Class<AutopilotWritebackPlan>(
  'AutopilotWritebackPlan',
)({
  artifact: CodingAutopilotArtifactRecord,
  authorityDecision: S.Literals(['allowed', 'blocked']),
  authorityReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  missionRef: S.String,
  operation: GitHubWritebackOperation,
  workOrderRef: S.String,
}) {}

export class AutopilotApertureContractUnsafe extends S.TaggedErrorClass<AutopilotApertureContractUnsafe>()(
  'AutopilotApertureContractUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafeRefPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new AutopilotApertureContractUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw artifact material.`,
    })
  }
}

const assertIso = (label: string, value: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new AutopilotApertureContractUnsafe({
      reason: `${label} must be an ISO timestamp.`,
    })
  }
}

export const assertAutopilotMissionWorkOrderLinksAreOneToOne = (
  links: ReadonlyArray<AutopilotMissionWorkOrderLink>,
): void => {
  const missionRefs = uniqueRefs(links.map(link => link.missionRef))
  const workOrderRefs = uniqueRefs(links.map(link => link.workOrderRef))
  const canonicalRefs = uniqueRefs(links.map(link => link.canonicalRef))

  if (
    missionRefs.length !== links.length ||
    workOrderRefs.length !== links.length ||
    canonicalRefs.length !== links.length
  ) {
    throw new AutopilotApertureContractUnsafe({
      reason:
        'Mission/work-order links must be 1:1 by missionRef, workOrderRef, and canonicalRef.',
    })
  }
}

export const validateAutopilotMissionWorkOrderLink = (
  link: AutopilotMissionWorkOrderLink,
): AutopilotMissionWorkOrderLink => {
  assertSafeRefs('mission work-order identity refs', [
    link.id,
    link.canonicalRef,
    link.missionRef,
    link.workOrderRef,
    link.dataScopeRef,
    link.placementRecordRef,
  ])
  assertSafeRefs('mission work-order briefing refs', link.briefingRefs)
  assertSafeRefs('mission work-order decision refs', link.decisionActionRefs)
  assertSafeRefs('mission work-order artifact refs', link.artifactRefs)
  assertSafeRefs('mission work-order receipt refs', link.receiptRefs)
  assertSafeRefs('mission work-order continuation refs', link.continuationRefs)
  assertIso('createdAtIso', link.createdAtIso)
  assertIso('updatedAtIso', link.updatedAtIso)

  if (!link.canonicalRef.startsWith('mission_work_order.')) {
    throw new AutopilotApertureContractUnsafe({
      reason: 'canonicalRef must be a mission_work_order.* ref.',
    })
  }

  return {
    ...link,
    artifactRefs: uniqueRefs(link.artifactRefs),
    briefingRefs: uniqueRefs(link.briefingRefs),
    continuationRefs: uniqueRefs(link.continuationRefs),
    decisionActionRefs: uniqueRefs(link.decisionActionRefs),
    receiptRefs: uniqueRefs(link.receiptRefs),
  }
}

export const projectAutopilotPlacementExplanation = (
  record: CodingAutopilotRepoPlacementRecord,
  generatedAt: string,
): AutopilotPlacementExplanation => {
  assertSafeRefs('placement explanation refs', [
    record.id,
    record.missionRef,
    record.repoRef,
    ...record.policyRefs,
    ...record.blockerRefs,
    ...record.customerSafeBlockedReasonRefs,
  ])
  assertIso('generatedAt', generatedAt)

  return {
    blockerRefs: uniqueRefs(record.blockerRefs),
    customerSafeReasonRefs: uniqueRefs(record.customerSafeBlockedReasonRefs),
    decision: record.decision,
    generatedAt,
    missionRef: record.missionRef,
    placementRecordRef: record.id,
    policyRefs: uniqueRefs(record.policyRefs),
    repoRef: record.trustTier === 'public' ? record.repoRef : 'repo.redacted',
    runnerBackendKind: record.runnerBackendKind,
    staleness: 'fresh_until_placement_or_scope_transition',
    trustTier: record.trustTier,
  }
}

export type AutopilotWorkOrderWritebackInput = Readonly<{
  artifactRef: string
  authorityDecision: GitHubWritebackAuthorityDecision
  authorityReceiptRefs: ReadonlyArray<string>
  createdAtIso: string
  deliveryArtifactRefs: ReadonlyArray<string>
  id: string
  missionRef: string
  operation: GitHubWritebackOperationType
  summaryRef: string
  updatedAtIso: string
  workOrderRef: string
  workroomRefs: ReadonlyArray<string>
}>

export const planAutopilotWorkOrderWriteback = (
  input: AutopilotWorkOrderWritebackInput,
): AutopilotWritebackPlan => {
  assertSafeRefs('writeback identity refs', [
    input.id,
    input.artifactRef,
    input.missionRef,
    input.summaryRef,
    input.workOrderRef,
  ])
  assertSafeRefs('writeback authority refs', input.authorityReceiptRefs)
  assertSafeRefs('writeback delivery refs', input.deliveryArtifactRefs)
  assertSafeRefs('writeback workroom refs', input.workroomRefs)
  assertIso('createdAtIso', input.createdAtIso)
  assertIso('updatedAtIso', input.updatedAtIso)

  if (
    input.authorityDecision.decision === 'allowed' &&
    input.authorityReceiptRefs.length === 0
  ) {
    throw new AutopilotApertureContractUnsafe({
      reason: 'Allowed writeback requires at least one authority receipt ref.',
    })
  }

  if (
    input.authorityDecision.decision === 'allowed' &&
    input.deliveryArtifactRefs.length === 0
  ) {
    throw new AutopilotApertureContractUnsafe({
      reason: 'Allowed writeback requires at least one delivery artifact ref.',
    })
  }

  const authorityReceiptRefs = uniqueRefs(input.authorityReceiptRefs)
  const blockerRefs =
    input.authorityDecision.decision === 'blocked'
      ? [`blocker.github_writeback.${input.authorityDecision.blockedReason}`]
      : []

  return {
    artifact: {
      archivedAtIso: null,
      artifactKind: 'pr_draft',
      artifactRef: input.artifactRef,
      authorityReceiptRefs,
      caveatRefs:
        input.authorityDecision.decision === 'allowed'
          ? ['caveat.pr_draft.human_merge_required']
          : blockerRefs,
      createdAtIso: input.createdAtIso,
      evidenceRefs: uniqueRefs(input.deliveryArtifactRefs),
      id: input.id,
      missionRef: input.missionRef,
      publicSafe: input.authorityDecision.decision === 'allowed',
      retentionCaveatRefs: ['retention.pr_metadata_only'],
      sourceRefs: [input.workOrderRef],
      status:
        input.authorityDecision.decision === 'allowed' ? 'draft' : 'blocked',
      summaryRef: input.summaryRef,
      updatedAtIso: input.updatedAtIso,
      visibility:
        input.authorityDecision.decision === 'allowed' ? 'customer' : 'team',
      workroomRefs: uniqueRefs(input.workroomRefs),
    },
    authorityDecision: input.authorityDecision.decision,
    authorityReceiptRefs,
    blockerRefs,
    missionRef: input.missionRef,
    operation: input.operation,
    workOrderRef: input.workOrderRef,
  }
}
