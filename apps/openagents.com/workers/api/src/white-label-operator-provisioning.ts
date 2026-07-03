import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const WHITE_LABEL_OPERATOR_PROVISIONING_SCHEMA_VERSION =
  'openagents.white_label_operator_provisioning.v1'

export type WhiteLabelOperatorProvisioningStatus =
  | 'blocked'
  | 'provisioned'

export type WhiteLabelThemeTokenAssignment = Readonly<{
  token: string
  valueRef: string
}>

export type WhiteLabelTenantClientWorkroomReceipt = Readonly<{
  surface: 'customer'
  teamId: string
  workroomId: string
}>

export type WhiteLabelPayoutLedgerEvidence = Readonly<{
  policyRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  settlementState: 'none' | 'pending' | 'settled'
}>

export type BuildWhiteLabelOperatorProvisioningInput = Readonly<{
  clientWorkrooms: ReadonlyArray<WhiteLabelTenantClientWorkroomReceipt>
  generatedAt: string
  hostname: Readonly<{
    hostname: string
    status: 'pending' | 'verified' | 'active' | 'disabled'
    teamId: string
  }>
  operatorTenantRef: string
  payoutLedger: WhiteLabelPayoutLedgerEvidence
  theme: Readonly<{
    tokenSetRef: string
    tokens: ReadonlyArray<WhiteLabelThemeTokenAssignment>
  }>
}>

export type PublicWhiteLabelOperatorProvisioningProjection = Readonly<{
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  clientWorkroomRefs: ReadonlyArray<string>
  generatedAt: string
  hostname: string
  operatorTenantRef: string
  payoutPolicyRefs: ReadonlyArray<string>
  payoutReceiptRefs: ReadonlyArray<string>
  schemaVersion: typeof WHITE_LABEL_OPERATOR_PROVISIONING_SCHEMA_VERSION
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
  status: WhiteLabelOperatorProvisioningStatus
  teamId: string
  themeTokenRefs: ReadonlyArray<string>
}>

const safeRefPattern = /^[-A-Za-z0-9][-A-Za-z0-9_.:/=]{0,300}$/
const safeHostnamePattern =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/u
const designTokenPattern =
  /^--oa-(?:color|font|radius|shadow|space|size)-[a-z0-9-]+$/u
const prohibitedPublicProjectionPattern =
  /(?:^|[._:/-])(?:access[_-]?token|auth\.json|bearer|client[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone)|invoice[_-]?raw|lnbc|lntb|lnbcrt|lno1|mdk[_-]?access[_-]?token|mnemonic|payment[_-]?(hash|preimage|secret)|payout[_-]?(destination|private|raw)|private[_-]?(customer|key|wallet)|provider[_-]?(payload|secret|token)|raw[_-]?(customer|invoice|log|payment|payout|prompt|provider|webhook)|refresh[_-]?token|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)(?:$|[._:/-])/iu

const publicSafeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(
    ref =>
      safeRefPattern.test(ref) && !prohibitedPublicProjectionPattern.test(ref),
  )

const publicSafeHostname = (hostname: string): string | null => {
  const normalized = hostname.trim().toLowerCase().replace(/\.$/u, '')

  return safeHostnamePattern.test(normalized) &&
    !prohibitedPublicProjectionPattern.test(normalized)
    ? normalized
    : null
}

const themeTokenRefs = (
  theme: BuildWhiteLabelOperatorProvisioningInput['theme'],
): ReadonlyArray<string> =>
  publicSafeRefs([
    theme.tokenSetRef,
    ...theme.tokens
      .filter(assignment => designTokenPattern.test(assignment.token))
      .map(assignment => `${assignment.token}=${assignment.valueRef}`),
  ])

export const buildWhiteLabelOperatorProvisioningProjection = (
  input: BuildWhiteLabelOperatorProvisioningInput,
): PublicWhiteLabelOperatorProvisioningProjection => {
  const hostname = publicSafeHostname(input.hostname.hostname)
  const themeRefs = themeTokenRefs(input.theme)
  const clientWorkroomRefs = publicSafeRefs(
    input.clientWorkrooms
      .filter(workroom => workroom.teamId === input.hostname.teamId)
      .map(workroom => `workroom.${workroom.surface}.${workroom.workroomId}`),
  )
  const payoutPolicyRefs = publicSafeRefs(input.payoutLedger.policyRefs)
  const payoutReceiptRefs = publicSafeRefs(input.payoutLedger.receiptRefs)
  const operatorTenantRefs = publicSafeRefs([input.operatorTenantRef])

  const blockerRefs = [
    ...(hostname === null
      ? ['blocker.white_label.hostname_not_public_safe']
      : []),
    ...(input.hostname.status === 'active'
      ? []
      : ['blocker.white_label.hostname_not_active']),
    ...(operatorTenantRefs.length === 1
      ? []
      : ['blocker.white_label.operator_tenant_ref_not_public_safe']),
    ...(themeRefs.length > 1
      ? []
      : ['blocker.white_label.theme_token_evidence_missing']),
    ...(clientWorkroomRefs.length > 0
      ? []
      : ['blocker.white_label.client_workroom_missing']),
    ...(payoutPolicyRefs.length > 0
      ? []
      : ['blocker.white_label.payout_policy_missing']),
    ...(payoutReceiptRefs.length > 0 &&
    input.payoutLedger.settlementState === 'settled'
      ? []
      : ['blocker.white_label.settled_payout_receipt_missing']),
  ]

  const status: WhiteLabelOperatorProvisioningStatus =
    blockerRefs.length === 0 ? 'provisioned' : 'blocked'

  return {
    authorityBoundary:
      'Read-only public-safe BF-8.3 provisioning projection. It proves only that an opaque operator tenant has active hostname, token-theme, tenant-client workroom, and settled payout evidence; it grants no hostname, workroom, payout, settlement, theming, customer identity, or public-copy authority.',
    blockerRefs,
    clientWorkroomRefs,
    generatedAt: input.generatedAt,
    hostname: hostname ?? 'redacted.hostname',
    operatorTenantRef: operatorTenantRefs[0] ?? 'redacted.operator_tenant',
    payoutPolicyRefs,
    payoutReceiptRefs,
    schemaVersion: WHITE_LABEL_OPERATOR_PROVISIONING_SCHEMA_VERSION,
    sourceRefs: [
      'docs/fable/ROADMAP_BIZ.md#BF-8.3',
      'promise.autopilot_sites.custom_tenant_hostnames.v1',
      'promise.workrooms.omni_client_delivery_workrooms.v1',
      'promise.autopilot_sites.partner_payout_ledger.v1',
    ],
    staleness: liveAtReadStaleness([
      'tenant_custom_hostname_state_changed',
      'tenant_client_workroom_projection_changed',
      'partner_payout_ledger_state_changed',
      'white_label_theme_token_set_changed',
    ]),
    status,
    teamId: input.hostname.teamId,
    themeTokenRefs: themeRefs,
  }
}
