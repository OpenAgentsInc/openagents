import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import type { OmniEvidenceBundleRecord } from './omni-evidence-bundles'
import { customerOmniEvidenceBundleProjection } from './omni-evidence-bundles'
import type { OmniWorkroomRecord } from './omni-workrooms'
import type { OmniWorkroomLifecycleDecisionRecord } from './omni-workroom-lifecycle'
import { customerOmniWorkroomLifecycleProjection } from './omni-workroom-lifecycle'

export const OmniMissionBriefingSectionKind = S.Literals([
  'changed',
  'built',
  'blocked',
  'review',
  'email',
  'next_action',
])
export type OmniMissionBriefingSectionKind =
  typeof OmniMissionBriefingSectionKind.Type

export const OmniMissionBriefingItemStatus = S.Literals([
  'ready',
  'blocked',
  'needs_review',
  'sent',
  'pending',
  'done',
])
export type OmniMissionBriefingItemStatus =
  typeof OmniMissionBriefingItemStatus.Type

export const OmniMissionBriefingItem = S.Struct({
  displayTime: S.NullOr(S.String),
  kind: OmniMissionBriefingSectionKind,
  ref: S.String,
  status: OmniMissionBriefingItemStatus,
  summaryRef: S.String,
})
export type OmniMissionBriefingItem = typeof OmniMissionBriefingItem.Type

export const OmniMissionBriefingProjection = S.Struct({
  empty: S.Boolean,
  generatedAtDisplay: S.String,
  sections: S.Struct({
    blocked: S.Array(OmniMissionBriefingItem),
    built: S.Array(OmniMissionBriefingItem),
    changed: S.Array(OmniMissionBriefingItem),
    email: S.Array(OmniMissionBriefingItem),
    nextAction: S.Array(OmniMissionBriefingItem),
    review: S.Array(OmniMissionBriefingItem),
  }),
  status: S.String,
  workKind: S.String,
  workroomId: S.String,
})
export type OmniMissionBriefingProjection =
  typeof OmniMissionBriefingProjection.Type

export type BuildOmniMissionBriefingInput = Readonly<{
  evidenceBundles?: ReadonlyArray<OmniEvidenceBundleRecord> | undefined
  lifecycleDecisions?:
    | ReadonlyArray<OmniWorkroomLifecycleDecisionRecord>
    | undefined
  nowIso: string
  workroom: OmniWorkroomRecord
}>

const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'customer_email',
  'email_body',
  'provider_payload',
  'raw_email',
  'raw_run_log',
]
const ISO_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_TEXT_PATTERN.test(value) &&
  !PROHIBITED_FRAGMENTS.some(fragment =>
    value.toLowerCase().includes(fragment),
  ) &&
  !ISO_TIMESTAMP_PATTERN.test(value)

const safeRef = (value: string | null | undefined): string | null => {
  if (value === null || value === undefined || value.trim() === '') {
    return null
  }

  return textIsSafe(value) ? value : null
}

const item = (
  kind: OmniMissionBriefingSectionKind,
  ref: string | null | undefined,
  summaryRef: string | null | undefined,
  status: OmniMissionBriefingItemStatus,
  displayTime: string | null,
): OmniMissionBriefingItem | null => {
  const safeItemRef = safeRef(ref)
  const safeSummaryRef = safeRef(summaryRef)

  if (safeItemRef === null || safeSummaryRef === null) {
    return null
  }

  return {
    displayTime,
    kind,
    ref: safeItemRef,
    status,
    summaryRef: safeSummaryRef,
  }
}

const compactItems = (
  values: ReadonlyArray<OmniMissionBriefingItem | null>,
): ReadonlyArray<OmniMissionBriefingItem> =>
  values.filter((value): value is OmniMissionBriefingItem => value !== null)

export const friendlyMissionBriefingTime = (
  iso: string,
  nowIso: string,
): string => {
  const time = Date.parse(iso)
  const now = Date.parse(nowIso)

  if (!Number.isFinite(time) || !Number.isFinite(now)) {
    return 'Recently'
  }

  const elapsedMs = Math.max(0, now - time)
  const minuteMs = 60_000
  const hourMs = 60 * minuteMs
  const dayMs = 24 * hourMs

  if (elapsedMs < minuteMs) {
    return 'Just now'
  }

  if (elapsedMs < hourMs) {
    const minutes = Math.floor(elapsedMs / minuteMs)

    return minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`
  }

  if (elapsedMs < dayMs) {
    const hours = Math.floor(elapsedMs / hourMs)

    return hours === 1 ? '1 hour ago' : `${hours} hours ago`
  }

  if (elapsedMs < 2 * dayMs) {
    return 'Yesterday'
  }

  const days = Math.floor(elapsedMs / dayMs)

  return `${days} days ago`
}

const evidenceItems = (
  kind: OmniMissionBriefingSectionKind,
  bundle: OmniEvidenceBundleRecord,
  nowIso: string,
): ReadonlyArray<OmniMissionBriefingItem> => {
  const projection = customerOmniEvidenceBundleProjection(bundle)
  const displayTime = friendlyMissionBriefingTime(bundle.updatedAt, nowIso)

  return compactItems(
    projection.entries.map(entry => {
      if (
        kind === 'changed' &&
        ![
          'diff',
          'generated_source',
          'research_brief',
          'source_commit',
        ].includes(entry.entryKind)
      ) {
        return null
      }

      if (
        kind === 'built' &&
        ![
          'deployment_url',
          'email_receipt',
          'receipt',
          'screenshot',
          'test_report',
        ].includes(entry.entryKind)
      ) {
        return null
      }

      if (kind !== 'changed' && kind !== 'built') {
        return null
      }

      return item(kind, entry.ref, entry.summaryRef, 'ready', displayTime)
    }),
  )
}

const lifecycleReviewItems = (
  decisions: ReadonlyArray<OmniWorkroomLifecycleDecisionRecord>,
  nowIso: string,
): ReadonlyArray<OmniMissionBriefingItem> =>
  compactItems(
    decisions.map(decision => {
      const projection = customerOmniWorkroomLifecycleProjection(decision)
      const status =
        projection.resultingState === 'accepted' ? 'done' : 'needs_review'

      return item(
        'review',
        projection.receiptRef,
        projection.customerSafeExplanationRef,
        status,
        friendlyMissionBriefingTime(decision.createdAt, nowIso),
      )
    }),
  )

const emailItems = (
  workroom: OmniWorkroomRecord,
): ReadonlyArray<OmniMissionBriefingItem> =>
  compactItems(
    workroom.emailRefs.map(ref =>
      item('email', ref, `${ref}:summary`, 'sent', null),
    ),
  )

const blockerItems = (
  workroom: OmniWorkroomRecord,
): ReadonlyArray<OmniMissionBriefingItem> =>
  compactItems(
    workroom.blockerRefs.map(ref =>
      item('blocked', ref, `${ref}:summary`, 'blocked', null),
    ),
  )

const artifactBuiltItems = (
  workroom: OmniWorkroomRecord,
  nowIso: string,
): ReadonlyArray<OmniMissionBriefingItem> =>
  compactItems(
    workroom.artifactRefs.map(ref =>
      item(
        'built',
        ref,
        `${ref}:summary`,
        'ready',
        friendlyMissionBriefingTime(workroom.updatedAt, nowIso),
      ),
    ),
  )

const nextActionRef = (
  workroom: OmniWorkroomRecord,
  decisions: ReadonlyArray<OmniWorkroomLifecycleDecisionRecord>,
): string => {
  const latestDecision = [...decisions].sort((left, right) =>
    left.createdAt < right.createdAt ? 1 : -1,
  )[0]

  if (latestDecision?.resultingState === 'accepted') {
    return 'next_action_acceptance_recorded'
  }

  if (latestDecision?.resultingState === 'revision_requested') {
    return 'next_action_revision_queue'
  }

  if (latestDecision?.resultingState === 'rejected') {
    return 'next_action_review_rejection'
  }

  if (workroom.blockerRefs.length > 0 || workroom.status === 'blocked') {
    return 'next_action_clear_blocker'
  }

  if (workroom.status === 'waiting_review') {
    return 'next_action_review_latest'
  }

  if (workroom.status === 'completed') {
    return 'next_action_complete'
  }

  return 'next_action_work_in_progress'
}

export const buildOmniMissionBriefing = (
  input: BuildOmniMissionBriefingInput,
): OmniMissionBriefingProjection => {
  const evidenceBundles = input.evidenceBundles ?? []
  const lifecycleDecisions = input.lifecycleDecisions ?? []
  const changed = evidenceBundles.flatMap(bundle =>
    evidenceItems('changed', bundle, input.nowIso),
  )
  const built = [
    ...artifactBuiltItems(input.workroom, input.nowIso),
    ...evidenceBundles.flatMap(bundle =>
      evidenceItems('built', bundle, input.nowIso),
    ),
  ]
  const blocked = blockerItems(input.workroom)
  const review = lifecycleReviewItems(lifecycleDecisions, input.nowIso)
  const email = emailItems(input.workroom)
  const nextAction = compactItems([
    item(
      'next_action',
      nextActionRef(input.workroom, lifecycleDecisions),
      `${nextActionRef(input.workroom, lifecycleDecisions)}:summary`,
      input.workroom.status === 'blocked' ? 'blocked' : 'pending',
      null,
    ),
  ])
  const sections = {
    blocked,
    built,
    changed,
    email,
    nextAction,
    review,
  }

  return {
    empty:
      changed.length === 0 &&
      built.length === 0 &&
      blocked.length === 0 &&
      review.length === 0 &&
      email.length === 0,
    generatedAtDisplay: friendlyMissionBriefingTime(
      input.workroom.updatedAt,
      input.nowIso,
    ),
    sections,
    status: input.workroom.status,
    workKind: input.workroom.workKind,
    workroomId: input.workroom.id,
  }
}
