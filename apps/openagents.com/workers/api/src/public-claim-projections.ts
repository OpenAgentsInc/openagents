import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  PublicClaimCopyRule,
  PublicClaimKind,
  PublicClaimState,
  PublicClaimStateProjection,
  publicClaimCopyRuleForState,
  publicClaimStateProjection,
} from './public-claim-state'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const PublicClaimProjectionAudience = S.Literals([
  'customer',
  'operator',
  'public',
  'team',
])
export type PublicClaimProjectionAudience =
  typeof PublicClaimProjectionAudience.Type

export const PublicClaimProjectionSurface = S.Literals([
  'autopilot',
  'forum',
  'launch',
  'order',
  'provider',
  'public_agent',
  'pylon',
  'site',
  'workroom',
])
export type PublicClaimProjectionSurface =
  typeof PublicClaimProjectionSurface.Type

export const PublicClaimProjectionRecord = S.Struct({
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  customerRefs: S.Array(S.String),
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  surface: PublicClaimProjectionSurface,
  teamRefs: S.Array(S.String),
  titleRef: S.String,
  updatedAt: S.String,
})
export type PublicClaimProjectionRecord =
  typeof PublicClaimProjectionRecord.Type

export const PublicClaimProjection = S.Struct({
  audience: PublicClaimProjectionAudience,
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  copyRule: PublicClaimCopyRule,
  customerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PublicClaimStateProjection,
  subjectRef: S.String,
  surface: PublicClaimProjectionSurface,
  teamRefs: S.Array(S.String),
  titleRef: S.String,
  updatedAt: S.String,
})
export type PublicClaimProjection = typeof PublicClaimProjection.Type

export class PublicClaimProjectionUnsafe extends S.TaggedErrorClass<PublicClaimProjectionUnsafe>()(
  'PublicClaimProjectionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|token|wallet)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeRefPattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.values(value).some(valueHasPrivateMaterial)
  }

  return false
}

const safeRef = (ref: string): string | undefined => {
  const trimmed = ref.trim()

  return trimmed !== '' &&
    safeRefPattern.test(trimmed) &&
    !valueHasPrivateMaterial(trimmed)
    ? trimmed
    : undefined
}

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const projected = [...new Set(refs)].map(safeRef)

  if (projected.some(ref => ref === undefined)) {
    throw new PublicClaimProjectionUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, or customer material.`,
    })
  }

  return projected.filter((ref): ref is string => ref !== undefined).sort()
}

const assertRecordSafe = (record: PublicClaimProjectionRecord): void => {
  safeRefs('claim refs', [
    record.claimId,
    record.claimRef,
    record.subjectRef,
    record.titleRef,
  ])
  safeRefs('caveat refs', record.caveatRefs)
  safeRefs('evidence refs', record.evidenceRefs)
  safeRefs('source refs', record.sourceRefs)
  safeRefs('customer refs', record.customerRefs)
  safeRefs('team refs', record.teamRefs)
  safeRefs('operator refs', record.operatorRefs)

  if (valueHasPrivateMaterial(record.updatedAt)) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'updatedAt contains private material.',
    })
  }
}

const customerRefsForAudience = (
  record: PublicClaimProjectionRecord,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'customer' || audience === 'team' || audience === 'operator'
    ? safeRefs('customer refs', record.customerRefs)
    : []

const teamRefsForAudience = (
  record: PublicClaimProjectionRecord,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'team' || audience === 'operator'
    ? safeRefs('team refs', record.teamRefs)
    : []

const operatorRefsForAudience = (
  record: PublicClaimProjectionRecord,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'operator'
    ? safeRefs('operator refs', record.operatorRefs)
    : []

export const publicClaimProjectionHasPrivateMaterial =
  valueHasPrivateMaterial

export const projectPublicClaimRecord = (
  record: PublicClaimProjectionRecord,
  audience: PublicClaimProjectionAudience,
): PublicClaimProjection => {
  assertRecordSafe(record)

  const evidenceRefs = safeRefs('evidence refs', record.evidenceRefs)
  const state = publicClaimStateProjection({
    desiredState: record.desiredState,
    evidenceRefs,
    kind: record.claimKind,
  })
  const projection: PublicClaimProjection = {
    audience,
    caveatRefs: safeRefs('caveat refs', record.caveatRefs),
    claimId: safeRef(record.claimId) ?? record.claimId,
    claimKind: record.claimKind,
    claimRef: safeRef(record.claimRef) ?? record.claimRef,
    copyRule: publicClaimCopyRuleForState(state.state),
    customerRefs: customerRefsForAudience(record, audience),
    evidenceRefs,
    operatorRefs: operatorRefsForAudience(record, audience),
    sourceRefs: safeRefs('source refs', record.sourceRefs),
    state,
    subjectRef: safeRef(record.subjectRef) ?? record.subjectRef,
    surface: record.surface,
    teamRefs: teamRefsForAudience(record, audience),
    titleRef: safeRef(record.titleRef) ?? record.titleRef,
    updatedAt: record.updatedAt,
  }

  if (valueHasPrivateMaterial(projection)) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'Public claim projection contains private material.',
    })
  }

  return projection
}
