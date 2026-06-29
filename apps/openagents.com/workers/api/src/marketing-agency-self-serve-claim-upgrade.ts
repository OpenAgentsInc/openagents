import { Schema as S } from 'effect'

import {
  type MarketingAgencySelfServeDeliverability,
} from './marketing-agency-self-serve-deliverability'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const MARKETING_AGENCY_SELF_SERVE_CLAIM_UPGRADE_SCHEMA =
  'openagents.marketing_agency.self_serve_claim.v1' as const

export const CLAIM_UPGRADE_CONTRACT = 'proof.claim_upgrade_receipts.v1' as const

export const MARKETING_AGENCY_WORKSPACE_PACK_PROMISE =
  'business.marketing_agency_workspace_pack.v1' as const

export const MARKETING_AGENCY_PACK_SELF_SERVE_BLOCKER_REF =
  'blocker.product_promises.marketing_agency_pack_self_serve_missing' as const

export const SELF_SERVE_GATE_DELIVERABILITY_PROVEN =
  'gate.self_serve.deliverability_proven' as const
export const SELF_SERVE_GATE_OWNER_SIGN_OFF =
  'gate.self_serve.owner_sign_off_present' as const

export const MarketingAgencySelfServeGates = S.Struct({
  deliverabilityProven: S.Boolean,
  ownerSignOffPresent: S.Boolean,
})
export type MarketingAgencySelfServeGates = typeof MarketingAgencySelfServeGates.Type

export const MarketingAgencySelfServeClaim = S.Struct({
  schema: S.Literal(MARKETING_AGENCY_SELF_SERVE_CLAIM_UPGRADE_SCHEMA),
  deliverabilityRef: S.String,
  gates: MarketingAgencySelfServeGates,
  failingGateRefs: S.Array(S.String),
  selfServeSubstantiated: S.Boolean,
  contractRef: S.Literal(CLAIM_UPGRADE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(MARKETING_AGENCY_WORKSPACE_PACK_PROMISE)]),
  promiseState: S.Literal('yellow'),
  unclearedBlockerRefs: S.Array(S.String),
  assessedAt: S.String,
})
export type MarketingAgencySelfServeClaim = typeof MarketingAgencySelfServeClaim.Type

export type MarketingAgencySelfServeClaimInput = Readonly<{
  deliverability: MarketingAgencySelfServeDeliverability
  deliverabilityRef: string
  ownerSignOffRef?: string | undefined
}>

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

export const assessMarketingAgencySelfServeClaim = (
  input: MarketingAgencySelfServeClaimInput,
  options?: { assessedAt?: string },
): MarketingAgencySelfServeClaim => {
  const deliverabilityProven = input.deliverability.selfServeReady
  const ownerSignOffPresent = isNonEmpty(input.ownerSignOffRef)

  const gates: MarketingAgencySelfServeGates = {
    deliverabilityProven,
    ownerSignOffPresent,
  }

  const failingGateRefs: string[] = []
  if (!deliverabilityProven) {
    failingGateRefs.push(SELF_SERVE_GATE_DELIVERABILITY_PROVEN)
  }
  if (!ownerSignOffPresent) {
    failingGateRefs.push(SELF_SERVE_GATE_OWNER_SIGN_OFF)
  }

  const selfServeSubstantiated = failingGateRefs.length === 0

  return {
    schema: MARKETING_AGENCY_SELF_SERVE_CLAIM_UPGRADE_SCHEMA,
    deliverabilityRef: input.deliverabilityRef,
    gates,
    failingGateRefs,
    selfServeSubstantiated,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    promiseIds: [MARKETING_AGENCY_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow',
    unclearedBlockerRefs: selfServeSubstantiated
      ? []
      : [MARKETING_AGENCY_PACK_SELF_SERVE_BLOCKER_REF],
    assessedAt: options?.assessedAt ?? currentIsoTimestamp(),
  }
}

export const MarketingAgencySelfServeClaimStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'marketing_agency_self_serve_published',
    'product_promise_registry_updated',
  ])

export type MarketingAgencySelfServeClaimStore = {
  list: () => ReadonlyArray<MarketingAgencySelfServeClaimInput>
}

export const emptyMarketingAgencySelfServeClaimStore: MarketingAgencySelfServeClaimStore =
  {
    list: () => [],
  }

export const makeInMemoryMarketingAgencySelfServeClaimStore = (
  inputs: ReadonlyArray<MarketingAgencySelfServeClaimInput>,
): MarketingAgencySelfServeClaimStore => ({
  list: () => inputs,
})

export const projectMarketingAgencySelfServeClaims = (
  inputs: ReadonlyArray<MarketingAgencySelfServeClaimInput>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const claims = inputs.map(input =>
    assessMarketingAgencySelfServeClaim(input, { assessedAt: generatedAt }),
  )
  const substantiatedCount = claims.filter(c => c.selfServeSubstantiated).length

  return {
    schema: MARKETING_AGENCY_SELF_SERVE_CLAIM_UPGRADE_SCHEMA,
    promiseIds: [MARKETING_AGENCY_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow' as const,
    generatedAt,
    staleness: MarketingAgencySelfServeClaimStaleness,
    maxStalenessSeconds: MarketingAgencySelfServeClaimStaleness.maxStalenessSeconds,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    totals: {
      assessedCount: claims.length,
      substantiatedCount,
      withheldCount: claims.length - substantiatedCount,
    },
    selfServeClaimSubstantiated: substantiatedCount > 0,
    unclearedBlockerRefs: [MARKETING_AGENCY_PACK_SELF_SERVE_BLOCKER_REF],
    claims,
  }
}
