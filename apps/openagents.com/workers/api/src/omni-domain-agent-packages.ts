import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniDomainAgentPackageAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniDomainAgentPackageAudience =
  typeof OmniDomainAgentPackageAudience.Type

export const OmniDomainAgentPackageState = S.Literals([
  'blocked',
  'deprecated',
  'draft',
  'fixture_validated',
  'marketplace_attributed',
  'org_private_enabled',
  'public_projection_ready',
  'review_recorded',
  'runtime_promoted',
  'runtime_promotion_requested',
])
export type OmniDomainAgentPackageState =
  typeof OmniDomainAgentPackageState.Type

export const OmniDomainAgentPackageDomainKind = S.Literals([
  'crm_follow_up',
  'forum',
  'general',
  'investor_ops',
  'legal_safe_hold',
  'project_ops',
  'pylon_ops',
  'site_builder',
  'support',
])
export type OmniDomainAgentPackageDomainKind =
  typeof OmniDomainAgentPackageDomainKind.Type

export const OmniDomainAgentFixtureState = S.Literals([
  'failed',
  'passed',
  'pending',
  'waived',
])
export type OmniDomainAgentFixtureState =
  typeof OmniDomainAgentFixtureState.Type

export const OmniDomainAgentReviewState = S.Literals([
  'approved',
  'changes_requested',
  'pending',
  'rejected',
])
export type OmniDomainAgentReviewState =
  typeof OmniDomainAgentReviewState.Type

export const OmniDomainAgentEnablementScope = S.Literals([
  'org_private',
  'public_projection',
  'runtime',
])
export type OmniDomainAgentEnablementScope =
  typeof OmniDomainAgentEnablementScope.Type

export const OmniDomainAgentEnablementState = S.Literals([
  'enabled',
  'pending',
  'revoked',
])
export type OmniDomainAgentEnablementState =
  typeof OmniDomainAgentEnablementState.Type

export const OmniDomainAgentPromotionState = S.Literals([
  'approved',
  'not_requested',
  'promoted',
  'rejected',
  'requested',
  'rolled_back',
])
export type OmniDomainAgentPromotionState =
  typeof OmniDomainAgentPromotionState.Type

export const OmniDomainAgentRollbackPosture = S.Literals([
  'draft_only',
  'rollback_ready',
  'rollback_required',
  'rolled_back',
  'unavailable',
])
export type OmniDomainAgentRollbackPosture =
  typeof OmniDomainAgentRollbackPosture.Type

export const OmniDomainAgentAttributionState = S.Literals([
  'candidate',
  'disputed',
  'none',
  'recorded',
  'revoked',
])
export type OmniDomainAgentAttributionState =
  typeof OmniDomainAgentAttributionState.Type

export const OmniDomainAgentPackageAuthorityBoundary = S.Literals([
  'read_only_domain_agent_package_lifecycle',
])
export type OmniDomainAgentPackageAuthorityBoundary =
  typeof OmniDomainAgentPackageAuthorityBoundary.Type

export class OmniDomainAgentPackageAuthority extends S.Class<OmniDomainAgentPackageAuthority>(
  'OmniDomainAgentPackageAuthority',
)({
  authorityBoundary: OmniDomainAgentPackageAuthorityBoundary,
  noFixtureExecution: S.Boolean,
  noMarketplaceListingMutation: S.Boolean,
  noOrgEnablementMutation: S.Boolean,
  noPaymentMutation: S.Boolean,
  noPublicProjectionMutation: S.Boolean,
  noReviewMutation: S.Boolean,
  noRollbackMutation: S.Boolean,
  noRuntimePromotionMutation: S.Boolean,
}) {}

export class OmniDomainAgentFixtureRecord extends S.Class<OmniDomainAgentFixtureRecord>(
  'OmniDomainAgentFixtureRecord',
)({
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  expectedOutcomeRefs: S.Array(S.String),
  fixtureRef: S.String,
  scenarioRefs: S.Array(S.String),
  scoreBps: S.Number,
  state: OmniDomainAgentFixtureState,
  validationReceiptRefs: S.Array(S.String),
}) {}

export class OmniDomainAgentReviewRecord extends S.Class<OmniDomainAgentReviewRecord>(
  'OmniDomainAgentReviewRecord',
)({
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  reviewRef: S.String,
  reviewerRefs: S.Array(S.String),
  state: OmniDomainAgentReviewState,
}) {}

export class OmniDomainAgentEnablementRecord extends S.Class<OmniDomainAgentEnablementRecord>(
  'OmniDomainAgentEnablementRecord',
)({
  approvalReceiptRefs: S.Array(S.String),
  audienceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  enablementRef: S.String,
  receiptRefs: S.Array(S.String),
  scope: OmniDomainAgentEnablementScope,
  sourceRefs: S.Array(S.String),
  state: OmniDomainAgentEnablementState,
}) {}

export class OmniDomainAgentPromotionRecord extends S.Class<OmniDomainAgentPromotionRecord>(
  'OmniDomainAgentPromotionRecord',
)({
  approvalReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  promotionRef: S.String,
  rollbackPosture: OmniDomainAgentRollbackPosture,
  rollbackRefs: S.Array(S.String),
  runtimeReceiptRefs: S.Array(S.String),
  state: OmniDomainAgentPromotionState,
}) {}

export class OmniDomainAgentAttributionRecord extends S.Class<OmniDomainAgentAttributionRecord>(
  'OmniDomainAgentAttributionRecord',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  attributionRef: S.String,
  caveatRefs: S.Array(S.String),
  contributorRefs: S.Array(S.String),
  packageVersionRef: S.String,
  receiptRefs: S.Array(S.String),
  splitPolicyRefs: S.Array(S.String),
  state: OmniDomainAgentAttributionState,
}) {}

export class OmniDomainAgentPackageRecord extends S.Class<OmniDomainAgentPackageRecord>(
  'OmniDomainAgentPackageRecord',
)({
  attributionRecords: S.Array(OmniDomainAgentAttributionRecord),
  authority: OmniDomainAgentPackageAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contextTemplateRefs: S.Array(S.String),
  createdAtIso: S.String,
  displayNameRef: S.String,
  domainKind: OmniDomainAgentPackageDomainKind,
  enablementRecords: S.Array(OmniDomainAgentEnablementRecord),
  fixtureRecords: S.Array(OmniDomainAgentFixtureRecord),
  id: S.String,
  outcomeTemplateRefs: S.Array(S.String),
  packageRef: S.String,
  programSignatureRefs: S.Array(S.String),
  promotionRecords: S.Array(OmniDomainAgentPromotionRecord),
  publicProjectionRefs: S.Array(S.String),
  reviewRecords: S.Array(OmniDomainAgentReviewRecord),
  sourceRefs: S.Array(S.String),
  state: OmniDomainAgentPackageState,
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OmniDomainAgentPackageProjection extends S.Class<OmniDomainAgentPackageProjection>(
  'OmniDomainAgentPackageProjection',
)({
  attributionRecords: S.Array(OmniDomainAgentAttributionRecord),
  audience: OmniDomainAgentPackageAudience,
  authority: OmniDomainAgentPackageAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contextTemplateRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  displayNameRef: S.String,
  domainKind: OmniDomainAgentPackageDomainKind,
  enablementRecords: S.Array(OmniDomainAgentEnablementRecord),
  fixtureExecutionAllowed: S.Boolean,
  fixtureRecords: S.Array(OmniDomainAgentFixtureRecord),
  fixtureValidated: S.Boolean,
  id: S.String,
  marketplaceAttributionRecorded: S.Boolean,
  marketplaceListingMutationAllowed: S.Boolean,
  orgEnablementMutationAllowed: S.Boolean,
  orgPrivateEnabled: S.Boolean,
  outcomeTemplateRefs: S.Array(S.String),
  packageRef: S.String,
  paymentMutationAllowed: S.Boolean,
  programSignatureRefs: S.Array(S.String),
  promotionRecords: S.Array(OmniDomainAgentPromotionRecord),
  publicProjectionMutationAllowed: S.Boolean,
  publicProjectionReady: S.Boolean,
  publicProjectionRefs: S.Array(S.String),
  reviewMutationAllowed: S.Boolean,
  reviewRecorded: S.Boolean,
  reviewRecords: S.Array(OmniDomainAgentReviewRecord),
  rollbackMutationAllowed: S.Boolean,
  rollbackPosture: OmniDomainAgentRollbackPosture,
  runtimePromotionAllowed: S.Boolean,
  runtimePromotionRequested: S.Boolean,
  runtimePromoted: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: OmniDomainAgentPackageState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OmniDomainAgentPackageUnsafe extends S.TaggedErrorClass<OmniDomainAgentPackageUnsafe>()(
  'OmniDomainAgentPackageUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY:
  OmniDomainAgentPackageAuthority = {
    authorityBoundary: 'read_only_domain_agent_package_lifecycle',
    noFixtureExecution: true,
    noMarketplaceListingMutation: true,
    noOrgEnablementMutation: true,
    noPaymentMutation: true,
    noPublicProjectionMutation: true,
    noReviewMutation: true,
    noRollbackMutation: true,
    noRuntimePromotionMutation: true,
  }

const stateRank: Readonly<Record<OmniDomainAgentPackageState, number>> = {
  blocked: -1,
  deprecated: -1,
  draft: 0,
  fixture_validated: 1,
  marketplace_attributed: 7,
  org_private_enabled: 3,
  public_projection_ready: 4,
  review_recorded: 2,
  runtime_promoted: 6,
  runtime_promotion_requested: 5,
}

const stateLabelByState: Readonly<
  Record<OmniDomainAgentPackageState, string>
> = {
  blocked: 'Blocked',
  deprecated: 'Deprecated',
  draft: 'Draft',
  fixture_validated: 'Fixture validated',
  marketplace_attributed: 'Marketplace attributed',
  org_private_enabled: 'Org-private enabled',
  public_projection_ready: 'Public projection ready',
  review_recorded: 'Review recorded',
  runtime_promoted: 'Runtime promoted',
  runtime_promotion_requested: 'Runtime promotion requested',
}

const publicUnsafeRefPattern =
  /(approval\.private|attribution\.private|caveat\.private|context\.private|enablement\.private|evidence\.private|fixture\.private|outcome\.private|package\.private|projection\.private|promotion\.private|provider\.|receipt\.private|review\.private|rollback\.private|scenario\.private|signature\.private|source\.|split\.private|title\.private|version\.private)/i
const agentUnsafeRefPattern =
  /(approval\.private|attribution\.private|enablement\.private|fixture\.private|package\.private|promotion\.private|provider\.private|receipt\.private|review\.private|rollback\.private|source\.private|split\.private)/i
const customerUnsafeRefPattern =
  /(approval\.private|enablement\.private|package\.private|promotion\.private|provider\.private|receipt\.private|review\.private|rollback\.private|source\.private|split\.private)/i

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeDomainPackageRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|contact[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone|record|value)|email[_-]?(address|body|html|raw|text)|fixture[_-]?payload[_-]?raw|full[_-]?(package|prompt|source)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|package[_-]?(source[_-]?private|raw|secret)|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|package|repo|source|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(attribution|auth|connector|customer|email|fixture|invoice|package|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const lifecycleAtLeast = (
  state: OmniDomainAgentPackageState,
  threshold: OmniDomainAgentPackageState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeDomainPackageRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: `${label} contains private package source, secrets, customer data, provider credentials, payment/wallet material, raw fixtures, private repos, raw logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniDomainAgentPackageAudience,
): RegExp | null => {
  switch (audience) {
    case 'agent':
      return agentUnsafeRefPattern
    case 'customer':
      return customerUnsafeRefPattern
    case 'public':
      return publicUnsafeRefPattern
    case 'operator':
    case 'team':
      return null
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniDomainAgentPackageAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniDomainAgentPackageAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniDomainAgentPackageAuthority,
): void => {
  if (
    authority.noFixtureExecution !== true ||
    authority.noMarketplaceListingMutation !== true ||
    authority.noOrgEnablementMutation !== true ||
    authority.noPaymentMutation !== true ||
    authority.noPublicProjectionMutation !== true ||
    authority.noReviewMutation !== true ||
    authority.noRollbackMutation !== true ||
    authority.noRuntimePromotionMutation !== true
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Domain agent packages are read-only lifecycle projections and cannot execute fixtures, mutate reviews, enable org access, publish projections, promote runtime packages, list marketplace entries, spend, or roll back.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertScoreBps = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0 || value > 10_000) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: `${label} must be an integer from 0 to 10000 basis points.`,
    })
  }
}

const assertFixture = (fixture: OmniDomainAgentFixtureRecord): void => {
  assertScoreBps('Domain agent fixture scoreBps', fixture.scoreBps)
  assertSafeRefs('Domain agent fixture refs', [
    fixture.fixtureRef,
    ...fixture.caveatRefs,
    ...fixture.evidenceRefs,
    ...fixture.expectedOutcomeRefs,
    ...fixture.scenarioRefs,
    ...fixture.validationReceiptRefs,
  ])

  if (
    fixture.state === 'passed' &&
    (fixture.validationReceiptRefs.length === 0 ||
      fixture.evidenceRefs.length === 0 ||
      fixture.expectedOutcomeRefs.length === 0)
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Passed domain agent fixtures require validation receipts, evidence, and expected outcome refs.',
    })
  }
}

const assertReview = (review: OmniDomainAgentReviewRecord): void => {
  assertSafeRefs('Domain agent review refs', [
    review.reviewRef,
    ...review.caveatRefs,
    ...review.evidenceRefs,
    ...review.receiptRefs,
    ...review.reviewerRefs,
  ])

  if (
    review.state === 'approved' &&
    (review.receiptRefs.length === 0 ||
      review.reviewerRefs.length === 0 ||
      review.evidenceRefs.length === 0)
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Approved domain agent reviews require reviewer, evidence, and receipt refs.',
    })
  }
}

const assertEnablement = (
  enablement: OmniDomainAgentEnablementRecord,
): void => {
  assertSafeRefs('Domain agent enablement refs', [
    enablement.enablementRef,
    ...enablement.approvalReceiptRefs,
    ...enablement.audienceRefs,
    ...enablement.caveatRefs,
    ...enablement.receiptRefs,
    ...enablement.sourceRefs,
  ])

  if (
    enablement.state === 'enabled' &&
    (enablement.approvalReceiptRefs.length === 0 ||
      enablement.receiptRefs.length === 0 ||
      enablement.audienceRefs.length === 0)
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Enabled domain agent packages require approval receipts, receipts, and audience refs.',
    })
  }
}

const assertPromotion = (
  promotion: OmniDomainAgentPromotionRecord,
): void => {
  assertSafeRefs('Domain agent promotion refs', [
    promotion.promotionRef,
    ...promotion.approvalReceiptRefs,
    ...promotion.caveatRefs,
    ...promotion.evidenceRefs,
    ...promotion.rollbackRefs,
    ...promotion.runtimeReceiptRefs,
  ])

  if (
    ['requested', 'approved', 'promoted'].includes(promotion.state) &&
    promotion.evidenceRefs.length === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Runtime promotion requests require evidence refs before promotion can proceed.',
    })
  }

  if (
    ['approved', 'promoted'].includes(promotion.state) &&
    promotion.approvalReceiptRefs.length === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Approved or promoted domain agent packages require promotion approval receipts.',
    })
  }

  if (
    promotion.state === 'promoted' &&
    (promotion.runtimeReceiptRefs.length === 0 ||
      promotion.rollbackPosture !== 'rollback_ready')
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Promoted domain agent packages require runtime receipts and rollback-ready posture.',
    })
  }

  if (
    ['rollback_ready', 'rolled_back'].includes(promotion.rollbackPosture) &&
    promotion.rollbackRefs.length === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Rollback-ready or rolled-back domain agent packages require rollback refs.',
    })
  }
}

const assertAttribution = (
  attribution: OmniDomainAgentAttributionRecord,
): void => {
  assertSafeRefs('Domain agent attribution refs', [
    attribution.attributionRef,
    attribution.packageVersionRef,
    ...attribution.acceptedOutcomeRefs,
    ...attribution.caveatRefs,
    ...attribution.contributorRefs,
    ...attribution.receiptRefs,
    ...attribution.splitPolicyRefs,
  ])

  if (
    attribution.state === 'recorded' &&
    (attribution.acceptedOutcomeRefs.length === 0 ||
      attribution.contributorRefs.length === 0 ||
      attribution.receiptRefs.length === 0 ||
      attribution.splitPolicyRefs.length === 0)
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Recorded marketplace attribution requires accepted outcome, contributor, receipt, and split policy refs.',
    })
  }
}

const passedFixtureCount = (
  record: OmniDomainAgentPackageRecord,
): number =>
  record.fixtureRecords.filter(fixture => fixture.state === 'passed').length

const approvedReviewCount = (
  record: OmniDomainAgentPackageRecord,
): number =>
  record.reviewRecords.filter(review => review.state === 'approved').length

const enabledScopeCount = (
  record: OmniDomainAgentPackageRecord,
  scope: OmniDomainAgentEnablementScope,
): number =>
  record.enablementRecords.filter(
    enablement => enablement.scope === scope && enablement.state === 'enabled',
  ).length

const promotionStateCount = (
  record: OmniDomainAgentPackageRecord,
  state: OmniDomainAgentPromotionState,
): number =>
  record.promotionRecords.filter(promotion => promotion.state === state).length

const recordedAttributionCount = (
  record: OmniDomainAgentPackageRecord,
): number =>
  record.attributionRecords.filter(attribution => attribution.state === 'recorded')
    .length

const assertPackageRecord = (
  record: OmniDomainAgentPackageRecord,
): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('Domain agent package createdAtIso', record.createdAtIso)
  assertValidIso('Domain agent package updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Domain agent package refs', [
    record.id,
    record.displayNameRef,
    record.packageRef,
    record.versionRef,
    ...record.blockerRefs,
    ...record.caveatRefs,
    ...record.contextTemplateRefs,
    ...record.outcomeTemplateRefs,
    ...record.programSignatureRefs,
    ...record.publicProjectionRefs,
    ...record.sourceRefs,
  ])
  record.attributionRecords.forEach(assertAttribution)
  record.enablementRecords.forEach(assertEnablement)
  record.fixtureRecords.forEach(assertFixture)
  record.promotionRecords.forEach(assertPromotion)
  record.reviewRecords.forEach(assertReview)

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: 'Blocked domain agent packages require blocker refs.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'fixture_validated') &&
    passedFixtureCount(record) === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: 'Fixture-validated domain agent packages require passed fixtures.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'review_recorded') &&
    approvedReviewCount(record) === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason: 'Review-recorded domain agent packages require approved reviews.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'org_private_enabled') &&
    enabledScopeCount(record, 'org_private') === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Org-private-enabled domain agent packages require org-private enablement records.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'public_projection_ready') &&
    (record.publicProjectionRefs.length === 0 ||
      enabledScopeCount(record, 'public_projection') === 0)
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Public projection ready domain agent packages require public projection refs and public projection enablement.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'runtime_promotion_requested') &&
    promotionStateCount(record, 'requested') === 0 &&
    promotionStateCount(record, 'approved') === 0 &&
    promotionStateCount(record, 'promoted') === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Runtime promotion requested domain agent packages require promotion records.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'runtime_promoted') &&
    promotionStateCount(record, 'promoted') === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Runtime-promoted domain agent packages require promoted runtime records.',
    })
  }

  if (
    lifecycleAtLeast(record.state, 'marketplace_attributed') &&
    recordedAttributionCount(record) === 0
  ) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Marketplace-attributed domain agent packages require recorded attribution records.',
    })
  }
}

const fixtureProjection = (
  fixture: OmniDomainAgentFixtureRecord,
  audience: OmniDomainAgentPackageAudience,
): OmniDomainAgentFixtureRecord | null => {
  const fixtureRef = refsForAudience(
    'Domain agent fixture refs',
    [fixture.fixtureRef],
    audience,
  )[0]

  if (fixtureRef === undefined) {
    return null
  }

  return {
    ...fixture,
    caveatRefs: refsForAudience(
      'Domain agent fixture caveat refs',
      fixture.caveatRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Domain agent fixture evidence refs',
      fixture.evidenceRefs,
      audience,
    ),
    expectedOutcomeRefs: refsForAudience(
      'Domain agent fixture expected refs',
      fixture.expectedOutcomeRefs,
      audience,
    ),
    fixtureRef,
    scenarioRefs: refsForAudience(
      'Domain agent fixture scenario refs',
      fixture.scenarioRefs,
      audience,
    ),
    validationReceiptRefs: refsForAudience(
      'Domain agent fixture validation refs',
      fixture.validationReceiptRefs,
      audience,
    ),
  }
}

const reviewProjection = (
  review: OmniDomainAgentReviewRecord,
  audience: OmniDomainAgentPackageAudience,
): OmniDomainAgentReviewRecord | null => {
  const reviewRef = refsForAudience(
    'Domain agent review refs',
    [review.reviewRef],
    audience,
  )[0]

  if (reviewRef === undefined) {
    return null
  }

  return {
    ...review,
    caveatRefs: refsForAudience(
      'Domain agent review caveat refs',
      review.caveatRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Domain agent review evidence refs',
      review.evidenceRefs,
      audience,
    ),
    receiptRefs: refsForAudience(
      'Domain agent review receipt refs',
      review.receiptRefs,
      audience,
    ),
    reviewerRefs: refsForAudience(
      'Domain agent reviewer refs',
      review.reviewerRefs,
      audience,
    ),
    reviewRef,
  }
}

const enablementProjection = (
  enablement: OmniDomainAgentEnablementRecord,
  audience: OmniDomainAgentPackageAudience,
): OmniDomainAgentEnablementRecord | null => {
  const enablementRef = refsForAudience(
    'Domain agent enablement refs',
    [enablement.enablementRef],
    audience,
  )[0]

  if (enablementRef === undefined) {
    return null
  }

  return {
    ...enablement,
    approvalReceiptRefs: refsForAudience(
      'Domain agent enablement approval refs',
      enablement.approvalReceiptRefs,
      audience,
    ),
    audienceRefs: refsForAudience(
      'Domain agent enablement audience refs',
      enablement.audienceRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Domain agent enablement caveat refs',
      enablement.caveatRefs,
      audience,
    ),
    enablementRef,
    receiptRefs: refsForAudience(
      'Domain agent enablement receipt refs',
      enablement.receiptRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Domain agent enablement source refs',
      enablement.sourceRefs,
      audience,
    ),
  }
}

const promotionProjection = (
  promotion: OmniDomainAgentPromotionRecord,
  audience: OmniDomainAgentPackageAudience,
): OmniDomainAgentPromotionRecord | null => {
  const promotionRef = refsForAudience(
    'Domain agent promotion refs',
    [promotion.promotionRef],
    audience,
  )[0]

  if (promotionRef === undefined) {
    return null
  }

  return {
    ...promotion,
    approvalReceiptRefs: refsForAudience(
      'Domain agent promotion approval refs',
      promotion.approvalReceiptRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Domain agent promotion caveat refs',
      promotion.caveatRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Domain agent promotion evidence refs',
      promotion.evidenceRefs,
      audience,
    ),
    promotionRef,
    rollbackRefs: refsForAudience(
      'Domain agent rollback refs',
      promotion.rollbackRefs,
      audience,
    ),
    runtimeReceiptRefs: refsForAudience(
      'Domain agent runtime receipt refs',
      promotion.runtimeReceiptRefs,
      audience,
    ),
  }
}

const attributionProjection = (
  attribution: OmniDomainAgentAttributionRecord,
  audience: OmniDomainAgentPackageAudience,
): OmniDomainAgentAttributionRecord | null => {
  const attributionRef = refsForAudience(
    'Domain agent attribution refs',
    [attribution.attributionRef],
    audience,
  )[0]

  if (attributionRef === undefined) {
    return null
  }

  return {
    ...attribution,
    acceptedOutcomeRefs: refsForAudience(
      'Domain agent attribution accepted outcome refs',
      attribution.acceptedOutcomeRefs,
      audience,
    ),
    attributionRef,
    caveatRefs: refsForAudience(
      'Domain agent attribution caveat refs',
      attribution.caveatRefs,
      audience,
    ),
    contributorRefs: refsForAudience(
      'Domain agent attribution contributor refs',
      attribution.contributorRefs,
      audience,
    ),
    packageVersionRef: primaryRefForAudience(
      'Domain agent attribution package version refs',
      attribution.packageVersionRef,
      audience,
      'version_ref.redacted',
    ),
    receiptRefs: refsForAudience(
      'Domain agent attribution receipt refs',
      attribution.receiptRefs,
      audience,
    ),
    splitPolicyRefs: refsForAudience(
      'Domain agent attribution split refs',
      attribution.splitPolicyRefs,
      audience,
    ),
  }
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(item => [...stringValues(item)])
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => [...stringValues(item)])
  }

  return []
}

export const omniDomainAgentPackageProjectionHasPrivateMaterial = (
  projection: OmniDomainAgentPackageProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeDomainPackageRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const projectOmniDomainAgentPackage = (
  record: OmniDomainAgentPackageRecord,
  audience: OmniDomainAgentPackageAudience,
  nowIso: string,
): OmniDomainAgentPackageProjection => {
  assertPackageRecord(record)

  const promotionRecords = record.promotionRecords
    .map(promotion => promotionProjection(promotion, audience))
    .filter((promotion): promotion is OmniDomainAgentPromotionRecord =>
      promotion !== null,
    )
  const latestPromotion = record.promotionRecords.at(-1)

  const projection: OmniDomainAgentPackageProjection = {
    attributionRecords: record.attributionRecords
      .map(attribution => attributionProjection(attribution, audience))
      .filter((attribution): attribution is OmniDomainAgentAttributionRecord =>
        attribution !== null,
      ),
    audience,
    authority: OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY,
    blockerRefs: refsForAudience(
      'Domain agent package blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Domain agent package caveat refs',
      record.caveatRefs,
      audience,
    ),
    contextTemplateRefs: refsForAudience(
      'Domain agent context template refs',
      record.contextTemplateRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    displayNameRef: primaryRefForAudience(
      'Domain agent display refs',
      record.displayNameRef,
      audience,
      'title.redacted',
    ),
    domainKind: record.domainKind,
    enablementRecords: record.enablementRecords
      .map(enablement => enablementProjection(enablement, audience))
      .filter((enablement): enablement is OmniDomainAgentEnablementRecord =>
        enablement !== null,
      ),
    fixtureExecutionAllowed: false,
    fixtureRecords: record.fixtureRecords
      .map(fixture => fixtureProjection(fixture, audience))
      .filter((fixture): fixture is OmniDomainAgentFixtureRecord =>
        fixture !== null,
      ),
    fixtureValidated: passedFixtureCount(record) > 0,
    id: primaryRefForAudience(
      'Domain agent package id refs',
      record.id,
      audience,
      'domain_package.redacted',
    ),
    marketplaceAttributionRecorded: recordedAttributionCount(record) > 0,
    marketplaceListingMutationAllowed: false,
    orgEnablementMutationAllowed: false,
    orgPrivateEnabled: enabledScopeCount(record, 'org_private') > 0,
    outcomeTemplateRefs: refsForAudience(
      'Domain agent outcome template refs',
      record.outcomeTemplateRefs,
      audience,
    ),
    packageRef: primaryRefForAudience(
      'Domain agent package refs',
      record.packageRef,
      audience,
      'package.redacted',
    ),
    paymentMutationAllowed: false,
    programSignatureRefs: refsForAudience(
      'Domain agent program signature refs',
      record.programSignatureRefs,
      audience,
    ),
    promotionRecords,
    publicProjectionMutationAllowed: false,
    publicProjectionReady:
      record.publicProjectionRefs.length > 0 &&
      enabledScopeCount(record, 'public_projection') > 0,
    publicProjectionRefs: refsForAudience(
      'Domain agent public projection refs',
      record.publicProjectionRefs,
      audience,
    ),
    reviewMutationAllowed: false,
    reviewRecorded: approvedReviewCount(record) > 0,
    reviewRecords: record.reviewRecords
      .map(review => reviewProjection(review, audience))
      .filter((review): review is OmniDomainAgentReviewRecord =>
        review !== null,
      ),
    rollbackMutationAllowed: false,
    rollbackPosture: latestPromotion?.rollbackPosture ?? 'draft_only',
    runtimePromotionAllowed: false,
    runtimePromotionRequested:
      promotionStateCount(record, 'requested') > 0 ||
      promotionStateCount(record, 'approved') > 0 ||
      promotionStateCount(record, 'promoted') > 0,
    runtimePromoted: promotionStateCount(record, 'promoted') > 0,
    sourceRefs:
      audience === 'public' || audience === 'agent'
        ? []
        : refsForAudience(
          'Domain agent source refs',
          record.sourceRefs,
          audience,
        ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    versionRef: primaryRefForAudience(
      'Domain agent package version refs',
      record.versionRef,
      audience,
      'version_ref.redacted',
    ),
  }

  if (omniDomainAgentPackageProjectionHasPrivateMaterial(projection)) {
    throw new OmniDomainAgentPackageUnsafe({
      reason:
        'Domain agent package projection contains private package source, secrets, customer data, provider credentials, payment/wallet material, raw fixtures, private repos, raw logs, raw timestamps, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniDomainAgentPackage =
  (): OmniDomainAgentPackageRecord => ({
    attributionRecords: [
      {
        acceptedOutcomeRefs: ['outcome.public.otec_revision_accepted'],
        attributionRef: 'attribution.public.site_builder_package',
        caveatRefs: ['caveat.public.attribution_not_payment'],
        contributorRefs: ['contributor.public.openagents_core'],
        packageVersionRef: 'version.public.site_builder.v1',
        receiptRefs: ['receipt.public.attribution_recorded'],
        splitPolicyRefs: ['split.public.marketplace_memory_only'],
        state: 'recorded',
      },
    ],
    authority: OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.lifecycle_projection_only'],
    contextTemplateRefs: ['context.public.site_builder_customer_request'],
    createdAtIso: '2026-06-06T22:00:00.000Z',
    displayNameRef: 'title.public.site_builder_domain_package',
    domainKind: 'site_builder',
    enablementRecords: [
      {
        approvalReceiptRefs: ['approval.public.operator_review'],
        audienceRefs: ['audience.public.openagents_team'],
        caveatRefs: ['caveat.public.org_private_only'],
        enablementRef: 'enablement.public.org_private_site_builder',
        receiptRefs: ['receipt.public.org_private_enabled'],
        scope: 'org_private',
        sourceRefs: ['source.public.package_review'],
        state: 'enabled',
      },
      {
        approvalReceiptRefs: ['approval.public.operator_public_projection'],
        audienceRefs: ['audience.public.package_gallery'],
        caveatRefs: ['caveat.public.public_projection_not_runtime'],
        enablementRef: 'enablement.public.public_projection_site_builder',
        receiptRefs: ['receipt.public.public_projection_ready'],
        scope: 'public_projection',
        sourceRefs: ['source.public.package_review'],
        state: 'enabled',
      },
    ],
    fixtureRecords: [
      {
        caveatRefs: ['caveat.public.fixture_is_synthetic'],
        evidenceRefs: ['evidence.public.fixture_passed'],
        expectedOutcomeRefs: ['outcome.public.site_revision_ready'],
        fixtureRef: 'fixture.public.site_builder_revision',
        scenarioRefs: ['scenario.public.customer_revision_request'],
        scoreBps: 9600,
        state: 'passed',
        validationReceiptRefs: ['receipt.public.fixture_validation'],
      },
    ],
    id: 'domain_package.public.site_builder.v1',
    outcomeTemplateRefs: ['outcome_template.public.site_revision'],
    packageRef: 'package.public.site_builder',
    programSignatureRefs: ['signature.public.build_site_revision'],
    promotionRecords: [
      {
        approvalReceiptRefs: ['approval.public.runtime_promotion_review'],
        caveatRefs: ['caveat.public.promotion_request_only'],
        evidenceRefs: ['evidence.public.fixture_and_review_passed'],
        promotionRef: 'promotion.public.site_builder_runtime',
        rollbackPosture: 'rollback_ready',
        rollbackRefs: ['rollback.public.previous_package_version'],
        runtimeReceiptRefs: ['receipt.public.runtime_promotion'],
        state: 'promoted',
      },
    ],
    publicProjectionRefs: ['projection.public.site_builder_package'],
    reviewRecords: [
      {
        caveatRefs: ['caveat.public.review_not_runtime_authority'],
        evidenceRefs: ['evidence.public.operator_review'],
        receiptRefs: ['receipt.public.operator_review'],
        reviewRef: 'review.public.operator_approved',
        reviewerRefs: ['reviewer.public.openagents_operator'],
        state: 'approved',
      },
    ],
    sourceRefs: [
      'source.public.package_manifest',
      'source.private.operator_notes',
    ],
    state: 'marketplace_attributed',
    updatedAtIso: '2026-06-06T22:25:00.000Z',
    versionRef: 'version.public.site_builder.v1',
  })
