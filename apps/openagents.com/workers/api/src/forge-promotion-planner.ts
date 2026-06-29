import type {
  ForgeCoordinationIssueRow,
  ForgeCoordinationPrRow,
  ForgeCoordinationStatusRow,
  ForgeVerificationReceipt,
} from '@openagentsinc/forge-protocol'
import { Schema as S } from 'effect'
import { createHash } from 'node:crypto'

import { parseJsonWithSchema } from './json-boundary'

const StringArray = S.Array(S.String)

const gitObjectPattern = /^[0-9a-f]{40}(?:[0-9a-f]{24})?$/u

export const forgePromotionGateRefs = [
  'gate.forge.merge-deploy-gate',
  'gate.forge.issue-close-safe',
  'gate.forge.command-execution-source-verified',
  'gate.forge.operator-grounded-assertion',
  'gate.forge.deletion-poison-guard',
] as const

export type ForgePromotionReadyEntry = Readonly<{
  changeRef: string
  issueRef: string
  prRef: string
  queuePosition: number
  promotionRef: string
  baseHead: string
  candidateHead: string
  verificationRef: string
  waitsForActualHead: string | null
  gateRefs: ReadonlyArray<string>
}>

export type ForgePromotionBlockedEntry = Readonly<{
  changeRef: string
  issueRef: string
  prRef: string
  queuePosition: number
  blockedReasonRef: string
  detail: string
}>

export type ForgePromotionPlan = Readonly<{
  actualHead: string
  baseHead: string
  virtualHead: string
  branchBaseForNextAssignment: string
  nextActualPromotion: ForgePromotionReadyEntry | null
  ready: ReadonlyArray<ForgePromotionReadyEntry>
  blocked: ReadonlyArray<ForgePromotionBlockedEntry>
}>

type PlanInput = Readonly<{
  actualHead: string
  changes: ReadonlyArray<ForgeCoordinationPrRow>
  issues: ReadonlyArray<ForgeCoordinationIssueRow>
  statuses: ReadonlyArray<ForgeCoordinationStatusRow>
  verifications: ReadonlyArray<ForgeVerificationReceipt>
}>

const jsonStringArray = (value: string): ReadonlyArray<string> =>
  parseJsonWithSchema(StringArray, value)

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash('sha256').update(value).digest('hex').slice(0, 24)}`

const changeSortKey = (change: ForgeCoordinationPrRow): string =>
  `${change.created_at}\0${change.pr_ref}\0${change.change_ref}`

const latestStatusBySubject = (
  statuses: ReadonlyArray<ForgeCoordinationStatusRow>,
): ReadonlyMap<string, ForgeCoordinationStatusRow> => {
  const latest = new Map<string, ForgeCoordinationStatusRow>()
  for (const status of statuses) {
    const current = latest.get(status.subject_ref)
    if (
      current === undefined ||
      `${status.created_at}\0${status.status_ref}` >
        `${current.created_at}\0${current.status_ref}`
    ) {
      latest.set(status.subject_ref, status)
    }
  }
  return latest
}

const latestVerificationByChange = (
  verifications: ReadonlyArray<ForgeVerificationReceipt>,
): ReadonlyMap<string, ForgeVerificationReceipt> => {
  const latest = new Map<string, ForgeVerificationReceipt>()
  for (const verification of verifications) {
    const current = latest.get(verification.change_ref)
    if (
      current === undefined ||
      `${verification.completed_at}\0${verification.verification_ref}` >
        `${current.completed_at}\0${current.verification_ref}`
    ) {
      latest.set(verification.change_ref, verification)
    }
  }
  return latest
}

const sourceVerified = (verification: ForgeVerificationReceipt): boolean =>
  verification.command_ref.startsWith('command.public.pylon_khala.verify.') &&
  verification.command_args.length > 0 &&
  verification.executor_identity_ref.startsWith('agent.public.')

const hasDeletionPoison = (blockerRefs: ReadonlyArray<string>): boolean =>
  blockerRefs.some(
    ref =>
      ref.includes('deletion_poison') ||
      ref.includes('delete_poison') ||
      ref.includes('protected_path_deleted') ||
      ref.includes('mass_deletion'),
  )

export const planForgePromotionQueue = (
  input: PlanInput,
): ForgePromotionPlan => {
  const ready: Array<ForgePromotionReadyEntry> = []
  const blocked: Array<ForgePromotionBlockedEntry> = []
  const issuesByRef = new Map(
    input.issues.map(issue => [issue.issue_ref, issue]),
  )
  const latestStatus = latestStatusBySubject(input.statuses)
  const latestVerification = latestVerificationByChange(input.verifications)
  let virtualHead = input.actualHead

  const block = (
    change: ForgeCoordinationPrRow,
    queuePosition: number,
    blockedReasonRef: string,
    detail: string,
  ) => {
    blocked.push({
      blockedReasonRef,
      changeRef: change.change_ref,
      detail,
      issueRef: change.issue_ref,
      prRef: change.pr_ref,
      queuePosition,
    })
  }

  if (!gitObjectPattern.test(input.actualHead)) {
    for (const [index, change] of input.changes.entries()) {
      block(
        change,
        index,
        'forge.promotion.blocked.invalid_actual_head',
        'canonical target ref is missing or not a pinned Git object id',
      )
    }
    return {
      actualHead: input.actualHead,
      baseHead: input.actualHead,
      branchBaseForNextAssignment: input.actualHead,
      blocked,
      nextActualPromotion: null,
      ready,
      virtualHead: input.actualHead,
    }
  }

  for (const [queuePosition, change] of [...input.changes]
    .sort((a, b) => changeSortKey(a).localeCompare(changeSortKey(b)))
    .entries()) {
    const issue = issuesByRef.get(change.issue_ref)
    const status = latestStatus.get(change.change_ref)
    const blockerRefs = jsonStringArray(change.blocker_refs_json)
    const verification =
      change.verification_ref === null
        ? undefined
        : latestVerification.get(change.change_ref)

    if (change.state !== 'ready') {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.change_not_ready',
        `change ${change.change_ref} is ${change.state}`,
      )
      continue
    }
    if (issue === undefined || issue.state !== 'open') {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.issue-close-safe',
        `issue ${change.issue_ref} is not open`,
      )
      continue
    }
    if (status?.state === 'closed') {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.issue-close-safe',
        `latest change status for ${change.change_ref} is closed`,
      )
      continue
    }
    if (blockerRefs.length > 0) {
      block(
        change,
        queuePosition,
        hasDeletionPoison(blockerRefs)
          ? 'forge.promotion.blocked.deletion-poison-guard'
          : 'forge.promotion.blocked.operator-grounded-assertion',
        `change ${change.change_ref} carries blocker refs`,
      )
      continue
    }
    if (
      verification === undefined ||
      verification.verification_ref !== change.verification_ref
    ) {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.merge-deploy-gate',
        `change ${change.change_ref} has no matching verification receipt`,
      )
      continue
    }
    if (
      verification.verdict !== 'passed' ||
      verification.exit_code !== 0 ||
      verification.base_head !== change.base_head ||
      verification.head_head !== change.patch_head
    ) {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.merge-deploy-gate',
        `verification ${verification.verification_ref} does not prove this base/head`,
      )
      continue
    }
    if (!sourceVerified(verification)) {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.command-execution-source-verified',
        `verification ${verification.verification_ref} is not from the public Pylon Khala command source`,
      )
      continue
    }
    if (
      !gitObjectPattern.test(change.base_head) ||
      !gitObjectPattern.test(change.patch_head)
    ) {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.invalid_change_head',
        'change base/head must be pinned Git object ids',
      )
      continue
    }
    if (change.base_head !== virtualHead) {
      block(
        change,
        queuePosition,
        'forge.promotion.blocked.stale-base',
        `change base ${change.base_head} does not match virtual head ${virtualHead}`,
      )
      continue
    }

    const promotionRef = stableRef(
      'promotion.forge.nextActualPromotion',
      `${change.change_ref}:${change.base_head}:${change.patch_head}`,
    )
    ready.push({
      baseHead: change.base_head,
      candidateHead: change.patch_head,
      changeRef: change.change_ref,
      gateRefs: forgePromotionGateRefs,
      issueRef: change.issue_ref,
      prRef: change.pr_ref,
      promotionRef,
      queuePosition,
      verificationRef: verification.verification_ref,
      waitsForActualHead:
        change.base_head === input.actualHead ? null : change.base_head,
    })
    virtualHead = change.patch_head
  }

  return {
    actualHead: input.actualHead,
    baseHead: input.actualHead,
    branchBaseForNextAssignment: virtualHead,
    blocked,
    nextActualPromotion:
      ready.find(entry => entry.waitsForActualHead === null) ?? null,
    ready,
    virtualHead,
  }
}
