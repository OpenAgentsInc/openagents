import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { OmniProjectionAudience } from './omni-data-classification'

export const OmniOutcomePowerDataState = S.Literals([
  'measured',
  'mixed',
  'modeled',
  'unknown',
])
export type OmniOutcomePowerDataState = typeof OmniOutcomePowerDataState.Type

export const OmniOutcomePowerSettlementState = S.Literals([
  'mixed',
  'not_settled',
  'payable',
  'settled',
  'verified',
])
export type OmniOutcomePowerSettlementState =
  typeof OmniOutcomePowerSettlementState.Type

export const OmniOutcomePowerAuthorityBoundary = S.Literals([
  'read_only_power_productivity_projection',
])
export type OmniOutcomePowerAuthorityBoundary =
  typeof OmniOutcomePowerAuthorityBoundary.Type

export class OmniOutcomePowerProductivityAuthority extends S.Class<OmniOutcomePowerProductivityAuthority>(
  'OmniOutcomePowerProductivityAuthority',
)({
  authorityBoundary: OmniOutcomePowerAuthorityBoundary,
  noEnergyMeterMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPowerMarketClaimUpgrade: S.Boolean,
  noProviderSettlementMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
}) {}

export class OmniOutcomePowerProductivityRecord extends S.Class<OmniOutcomePowerProductivityRecord>(
  'OmniOutcomePowerProductivityRecord',
)({
  acceptedGrossProfitCents: S.Number,
  acceptedOutcomeCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedRevenueCents: S.Number,
  authority: OmniOutcomePowerProductivityAuthority,
  caveatRefs: S.Array(S.String),
  darkCapacityReasonRefs: S.Array(S.String),
  darkCapacityWattHours: S.Number,
  energyEvidenceRefs: S.Array(S.String),
  energyModelRefs: S.Array(S.String),
  energyWattHours: S.NullOr(S.Number),
  id: S.String,
  measuredEnergyRefs: S.Array(S.String),
  powerDataState: OmniOutcomePowerDataState,
  providerPayableCents: S.Number,
  providerSettledCents: S.Number,
  settlementRefs: S.Array(S.String),
  settlementState: OmniOutcomePowerSettlementState,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomRefs: S.Array(S.String),
}) {}

export class OmniOutcomePowerProductivityAggregate extends S.Class<OmniOutcomePowerProductivityAggregate>(
  'OmniOutcomePowerProductivityAggregate',
)({
  acceptedGrossProfitCents: S.Number,
  acceptedGrossProfitCentsPerKwh: S.NullOr(S.Number),
  acceptedOutcomeCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedOutcomesPerKwh: S.NullOr(S.Number),
  acceptedOutcomesPerMwh: S.NullOr(S.Number),
  acceptedRevenueCents: S.Number,
  acceptedRevenueCentsPerKwh: S.NullOr(S.Number),
  caveatRefs: S.Array(S.String),
  darkCapacityMwh: S.Number,
  darkCapacityReasonRefs: S.Array(S.String),
  darkCapacityWattHours: S.Number,
  energyEvidenceRefs: S.Array(S.String),
  energyKwh: S.NullOr(S.Number),
  energyModelRefs: S.Array(S.String),
  energyMwh: S.NullOr(S.Number),
  energyWattHours: S.NullOr(S.Number),
  measuredEnergyClaimAllowed: S.Boolean,
  measuredEnergyRefs: S.Array(S.String),
  modeledEnergyClaimAllowed: S.Boolean,
  powerDataState: OmniOutcomePowerDataState,
  powerDataStateLabel: S.String,
  providerPayableCents: S.Number,
  providerPayableCentsPerKwh: S.NullOr(S.Number),
  providerSettledCents: S.Number,
  settlementClaimAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  settlementState: OmniOutcomePowerSettlementState,
  settlementStateLabel: S.String,
  sourceRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export class OmniOutcomePowerProductivityWorkKindMetric extends S.Class<OmniOutcomePowerProductivityWorkKindMetric>(
  'OmniOutcomePowerProductivityWorkKindMetric',
)({
  ...OmniOutcomePowerProductivityAggregate.fields,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workKindLabel: S.String,
}) {}

export class OmniOutcomePowerProductivityProjection extends S.Class<OmniOutcomePowerProductivityProjection>(
  'OmniOutcomePowerProductivityProjection',
)({
  audience: OmniProjectionAudience,
  authority: OmniOutcomePowerProductivityAuthority,
  energyMeterMutationAllowed: S.Boolean,
  generatedFromRecordCount: S.Number,
  liveWalletSpendAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  powerMarketClaimUpgradeAllowed: S.Boolean,
  providerSettlementMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  totals: OmniOutcomePowerProductivityAggregate,
  updatedAtDisplay: S.NullOr(S.String),
  workKindMetrics: S.Array(OmniOutcomePowerProductivityWorkKindMetric),
}) {}

export class OmniOutcomePowerProductivityUnsafe extends S.TaggedErrorClass<OmniOutcomePowerProductivityUnsafe>()(
  'OmniOutcomePowerProductivityUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY:
  OmniOutcomePowerProductivityAuthority = {
    authorityBoundary: 'read_only_power_productivity_projection',
    noEnergyMeterMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPowerMarketClaimUpgrade: true,
    noProviderSettlementMutation: true,
    noPublicClaimUpgrade: true,
  }

const workKindLabelByKind: Readonly<Record<OmniAcceptedOutcomeWorkKind, string>> = {
  adjustment: 'Adjustment',
  business: 'Business',
  coding: 'Coding',
  existing_project_import: 'Existing project import',
  legal_sensitive: 'Legal-sensitive',
  site: 'Site',
}

const powerDataStateLabelByState:
  Readonly<Record<OmniOutcomePowerDataState, string>> = {
    measured: 'Measured',
    mixed: 'Mixed',
    modeled: 'Modeled',
    unknown: 'Unknown',
  }

const settlementStateLabelByState:
  Readonly<Record<OmniOutcomePowerSettlementState, string>> = {
    mixed: 'Mixed',
    not_settled: 'Not settled',
    payable: 'Payable',
    settled: 'Settled',
    verified: 'Verified',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePowerProductivityRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mac[_-]?address|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(hardware|key|wallet)|provider[_-]?(grant|payload|secret|telemetry|token)|raw[_-]?(auth|energy|host|invoice|meter|payment|payload|payout|power|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_outcome\.private|caveat\.private|energy\.private|evidence\.private|meter\.private|model\.private|provider\.private|settlement\.private|source\.private|workroom\.)/i
const teamUnsafeRefPattern =
  /(energy\.private|meter\.private|provider\.private|settlement\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafePowerProductivityRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: `${label} contains private customer data, private hardware telemetry, raw energy or meter telemetry, provider secrets, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, raw logs, private repo refs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent' || audience === 'customer') {
    return publicUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const redactRefsForAudience = (
  audience: typeof OmniProjectionAudience.Type,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const pattern = audienceUnsafePattern(audience)
  const refsToProject = uniqueRefs(refs)

  if (pattern === null) {
    return refsToProject
  }

  return refsToProject.filter(ref => !pattern.test(ref))
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertRecord = (record: OmniOutcomePowerProductivityRecord): void => {
  Object.entries({
    acceptedGrossProfitCents: record.acceptedGrossProfitCents,
    acceptedOutcomeCount: record.acceptedOutcomeCount,
    acceptedRevenueCents: record.acceptedRevenueCents,
    darkCapacityWattHours: record.darkCapacityWattHours,
    providerPayableCents: record.providerPayableCents,
    providerSettledCents: record.providerSettledCents,
  }).forEach(([label, value]) => assertNonNegativeInteger(label, value))

  if (record.energyWattHours !== null) {
    assertNonNegativeInteger('energyWattHours', record.energyWattHours)
  }

  if (
    record.powerDataState === 'measured' &&
    record.measuredEnergyRefs.length === 0
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Measured energy productivity requires measured energy refs.',
    })
  }

  if (
    record.powerDataState === 'modeled' &&
    record.energyModelRefs.length === 0
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Modeled energy productivity requires model refs.',
    })
  }

  if (
    record.powerDataState === 'unknown' &&
    record.energyWattHours !== null &&
    record.energyWattHours > 0
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Unknown energy productivity cannot carry positive energy.',
    })
  }

  if (record.powerDataState === 'unknown' && record.caveatRefs.length === 0) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Unknown energy productivity requires caveat refs.',
    })
  }

  if (
    record.darkCapacityWattHours > 0 &&
    record.darkCapacityReasonRefs.length === 0
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Dark-capacity energy requires dark-capacity reason refs.',
    })
  }

  if (
    record.providerSettledCents > 0 &&
    record.settlementState !== 'settled'
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Provider settled value requires settled settlement state.',
    })
  }

  if (
    record.settlementState === 'settled' &&
    (record.providerSettledCents <= 0 || record.settlementRefs.length === 0)
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason:
        'Settled power productivity records require provider settled value and settlement refs.',
    })
  }

  if (record.providerSettledCents > record.providerPayableCents) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason: 'Provider settled value cannot exceed provider payable value.',
    })
  }

  if (
    record.authority.noEnergyMeterMutation !== true ||
    record.authority.noLiveWalletSpend !== true ||
    record.authority.noPayoutDispatch !== true ||
    record.authority.noPowerMarketClaimUpgrade !== true ||
    record.authority.noProviderSettlementMutation !== true ||
    record.authority.noPublicClaimUpgrade !== true
  ) {
    throw new OmniOutcomePowerProductivityUnsafe({
      reason:
        'Power productivity records must remain read-only and cannot mutate meters, wallets, payouts, power-market claims, provider settlement, or public claims.',
    })
  }

  assertSafeRefs('Power productivity accepted outcome refs', record.acceptedOutcomeRefs)
  assertSafeRefs('Power productivity caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Power productivity dark capacity reason refs',
    record.darkCapacityReasonRefs,
  )
  assertSafeRefs('Power productivity energy evidence refs', record.energyEvidenceRefs)
  assertSafeRefs('Power productivity energy model refs', record.energyModelRefs)
  assertSafeRefs('Power productivity measured energy refs', record.measuredEnergyRefs)
  assertSafeRefs('Power productivity settlement refs', record.settlementRefs)
  assertSafeRefs('Power productivity source refs', record.sourceRefs)
  assertSafeRefs('Power productivity workroom refs', record.workroomRefs)
}

const combineState = <State extends string>(
  states: ReadonlyArray<State>,
  unknownState: State,
  mixedState: State,
): State => {
  const meaningful = uniqueRefs(states).filter(state => state !== unknownState)

  if (meaningful.length === 0) {
    return unknownState
  }

  if (meaningful.length === 1) {
    return meaningful[0] as State
  }

  return mixedState
}

const sum = (
  records: ReadonlyArray<OmniOutcomePowerProductivityRecord>,
  pick: (record: OmniOutcomePowerProductivityRecord) => number,
): number => records.reduce((total, record) => total + pick(record), 0)

const nullableEnergySum = (
  records: ReadonlyArray<OmniOutcomePowerProductivityRecord>,
): number | null => {
  const knownRecords = records.filter(record => record.energyWattHours !== null)

  if (knownRecords.length === 0) {
    return null
  }

  return knownRecords.reduce(
    (total, record) => total + (record.energyWattHours ?? 0),
    0,
  )
}

const refs = (
  records: ReadonlyArray<OmniOutcomePowerProductivityRecord>,
  pick: (record: OmniOutcomePowerProductivityRecord) => ReadonlyArray<string>,
): ReadonlyArray<string> => uniqueRefs(records.flatMap(record => [...pick(record)]))

const rounded = (value: number): number => Math.round(value * 1000) / 1000

const perKwh = (value: number, wattHours: number | null): number | null =>
  wattHours === null || wattHours <= 0
    ? null
    : rounded(value / (wattHours / 1000))

const aggregateRecords = (
  records: ReadonlyArray<OmniOutcomePowerProductivityRecord>,
  audience: typeof OmniProjectionAudience.Type,
): OmniOutcomePowerProductivityAggregate => {
  const energyWattHours = nullableEnergySum(records)
  const darkCapacityWattHours = sum(records, record => record.darkCapacityWattHours)
  const measuredEnergyRefs = redactRefsForAudience(
    audience,
    refs(records, record => record.measuredEnergyRefs),
  )
  const settlementRefs = redactRefsForAudience(
    audience,
    refs(records, record => record.settlementRefs),
  )
  const powerDataState = combineState(
    records.map(record => record.powerDataState),
    'unknown',
    'mixed',
  )
  const settlementState = combineState(
    records.map(record => record.settlementState),
    'not_settled',
    'mixed',
  )
  const acceptedOutcomeCount = sum(records, record => record.acceptedOutcomeCount)
  const acceptedRevenueCents = sum(records, record => record.acceptedRevenueCents)
  const acceptedGrossProfitCents = sum(
    records,
    record => record.acceptedGrossProfitCents,
  )
  const providerPayableCents = sum(records, record => record.providerPayableCents)

  return {
    acceptedGrossProfitCents,
    acceptedGrossProfitCentsPerKwh: perKwh(
      acceptedGrossProfitCents,
      energyWattHours,
    ),
    acceptedOutcomeCount,
    acceptedOutcomeRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.acceptedOutcomeRefs),
    ),
    acceptedOutcomesPerKwh: perKwh(acceptedOutcomeCount, energyWattHours),
    acceptedOutcomesPerMwh:
      energyWattHours === null || energyWattHours <= 0
        ? null
        : rounded(acceptedOutcomeCount / (energyWattHours / 1_000_000)),
    acceptedRevenueCents,
    acceptedRevenueCentsPerKwh: perKwh(acceptedRevenueCents, energyWattHours),
    caveatRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.caveatRefs),
    ),
    darkCapacityMwh: rounded(darkCapacityWattHours / 1_000_000),
    darkCapacityReasonRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.darkCapacityReasonRefs),
    ),
    darkCapacityWattHours,
    energyEvidenceRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.energyEvidenceRefs),
    ),
    energyKwh:
      energyWattHours === null ? null : rounded(energyWattHours / 1000),
    energyModelRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.energyModelRefs),
    ),
    energyMwh:
      energyWattHours === null ? null : rounded(energyWattHours / 1_000_000),
    energyWattHours,
    measuredEnergyClaimAllowed:
      measuredEnergyRefs.length > 0 && powerDataState === 'measured',
    measuredEnergyRefs,
    modeledEnergyClaimAllowed:
      powerDataState === 'modeled' &&
      records.some(record => record.energyModelRefs.length > 0),
    powerDataState,
    powerDataStateLabel: powerDataStateLabelByState[powerDataState],
    providerPayableCents,
    providerPayableCentsPerKwh: perKwh(providerPayableCents, energyWattHours),
    providerSettledCents: sum(records, record => record.providerSettledCents),
    settlementClaimAllowed:
      settlementRefs.length > 0 &&
      records.some(
        record =>
          record.settlementState === 'settled' &&
          record.providerSettledCents > 0,
      ),
    settlementRefs,
    settlementState,
    settlementStateLabel: settlementStateLabelByState[settlementState],
    sourceRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.sourceRefs),
    ),
    workroomRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.workroomRefs),
    ),
  }
}

export const projectOmniOutcomePowerProductivity = (
  records: ReadonlyArray<OmniOutcomePowerProductivityRecord>,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniOutcomePowerProductivityProjection => {
  records.forEach(assertRecord)

  const workKinds = uniqueRefs(
    records.map(record => record.workKind),
  ) as ReadonlyArray<OmniAcceptedOutcomeWorkKind>
  const updatedAtIso =
    [...records]
      .map(record => record.updatedAtIso)
      .sort()
      .at(-1) ?? null

  return {
    audience,
    authority: OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
    energyMeterMutationAllowed: false,
    generatedFromRecordCount: records.length,
    liveWalletSpendAllowed: false,
    payoutDispatchMutationAllowed: false,
    powerMarketClaimUpgradeAllowed: false,
    providerSettlementMutationAllowed: false,
    publicClaimUpgradeAllowed: false,
    totals: aggregateRecords(records, audience),
    updatedAtDisplay:
      updatedAtIso === null
        ? null
        : friendlyBlueprintMissionBriefingTime(updatedAtIso, nowIso),
    workKindMetrics: workKinds.map(workKind => ({
      ...aggregateRecords(
        records.filter(record => record.workKind === workKind),
        audience,
      ),
      workKind,
      workKindLabel: workKindLabelByKind[workKind],
    })),
  }
}
