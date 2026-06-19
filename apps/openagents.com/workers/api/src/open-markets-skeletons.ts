import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

/**
 * Skeleton liquidity & risk markets (EPIC #5510, issue #5514).
 *
 * Episode 239 (docs/transcripts/239.md) names liquidity and risk among the six
 * Episode 213 open markets, with the risk market illustrated by "build an
 * agentic insurance policy for a particular claim." Today neither market is
 * built. This module is the typed protocol/model scaffold for both, plus inert
 * read-only projections behind inert endpoints.
 *
 * INERT BY CONSTRUCTION. These models describe the SHAPE a liquidity quote or
 * an agentic insurance policy would take. They:
 *   - move no money and quote no fillable price,
 *   - bind no policy and underwrite no risk,
 *   - record no real participant transaction,
 *   - produce no settlement receipt.
 *
 * Every projection carries `state: 'skeleton'`, `inert: true`,
 * `moneyMovement: 'none'`, `settledTransactionCount: 0`, and the matching
 * `*_market_unbuilt` blocker. The promise `markets.open_protocol_markets.v1`
 * stays `planned`; nothing here flips it green. Acceptance for a future green
 * is an out-of-band, receipt-first upgrade per proof.claim_upgrade_receipts.v1
 * after a real participant transaction yields a dereferenceable receipt.
 */

export const LiquidityMarketSkeletonEndpoint =
  '/api/public/markets/liquidity/skeleton'
export const RiskMarketSkeletonEndpoint = '/api/public/markets/risk/skeleton'

export const LiquidityMarketSkeletonSchemaVersion =
  'openagents.markets.liquidity_skeleton.v1'
export const RiskMarketSkeletonSchemaVersion =
  'openagents.markets.risk_skeleton.v1'

const skeletonStaleness = (
  rebuildsOn: ReadonlyArray<string>,
): PublicProjectionStalenessContract => liveAtReadStaleness(rebuildsOn)

/** A skeleton protocol message: a named, typed shape with no live behavior. */
export class SkeletonProtocolMessage extends S.Class<SkeletonProtocolMessage>(
  'SkeletonProtocolMessage',
)({
  /** Stable identifier for the message kind in the would-be protocol. */
  kind: S.String,
  /** What this message would carry, in plain language. */
  description: S.String,
  /** Typed field names the message would carry (documentation only). */
  fields: S.Array(S.String),
  /** Direction across the would-be market. */
  direction: S.Literals(['request', 'offer', 'response', 'receipt']),
  /** Always true: this message kind is documented, not implemented. */
  inert: S.Literal(true),
}) {}

const skeletonGateFields = {
  /** Always 'skeleton': scaffolding, not a market. */
  state: S.Literal('skeleton'),
  /** Always true: every endpoint and message here is inert. */
  inert: S.Literal(true),
  /** Always 'none': no money moves through this scaffold. */
  moneyMovement: S.Literal('none'),
  /** Always 0: no real participant transaction has been recorded. */
  settledTransactionCount: S.Literal(0),
  /** Always false: the open-markets promise is not green. */
  promiseGreen: S.Literal(false),
} as const

export class LiquidityMarketSkeletonProjection extends S.Class<LiquidityMarketSkeletonProjection>(
  'LiquidityMarketSkeletonProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  marketId: S.Literal('liquidity'),
  promiseRef: S.Literal('promise:markets.open_protocol_markets.v1'),
  statusLabel: S.String,
  ...skeletonGateFields,
  staleness: PublicProjectionStalenessContract,
  /** The protocol shapes a real liquidity market would use. */
  protocolMessages: S.Array(SkeletonProtocolMessage),
  blockerRefs: S.Array(S.String),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export class RiskMarketSkeletonProjection extends S.Class<RiskMarketSkeletonProjection>(
  'RiskMarketSkeletonProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  marketId: S.Literal('risk'),
  promiseRef: S.Literal('promise:markets.open_protocol_markets.v1'),
  statusLabel: S.String,
  ...skeletonGateFields,
  staleness: PublicProjectionStalenessContract,
  /** The protocol shapes a real risk/insurance market would use. */
  protocolMessages: S.Array(SkeletonProtocolMessage),
  /** The agentic-insurance-policy primitive from the Episode 239 video. */
  agenticInsurancePolicyPrimitive: S.Struct({
    name: S.Literal('agentic_insurance_policy'),
    description: S.String,
    /** Fields a policy would carry (documentation only). */
    policyFields: S.Array(S.String),
    inert: S.Literal(true),
  }),
  blockerRefs: S.Array(S.String),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

const liquidityProtocolMessages = (): ReadonlyArray<SkeletonProtocolMessage> => [
  new SkeletonProtocolMessage({
    description:
      'A taker would request liquidity for a pair/amount over an open protocol. Inert: no quote is solicited.',
    direction: 'request',
    fields: ['pair', 'side', 'amount', 'expiresAt'],
    inert: true,
    kind: 'liquidity.request',
  }),
  new SkeletonProtocolMessage({
    description:
      'A provider would offer liquidity at a price. Inert: no offer is fillable and no price is real.',
    direction: 'offer',
    fields: ['pair', 'side', 'amount', 'priceSats', 'expiresAt', 'providerRef'],
    inert: true,
    kind: 'liquidity.offer',
  }),
  new SkeletonProtocolMessage({
    description:
      'A would-be fill acknowledgement. Inert: nothing matches and nothing fills.',
    direction: 'response',
    fields: ['requestRef', 'offerRef', 'filledAmount', 'status'],
    inert: true,
    kind: 'liquidity.fill',
  }),
  new SkeletonProtocolMessage({
    description:
      'A would-be settlement receipt shape. Inert: no settlement occurs and no receipt is issued.',
    direction: 'receipt',
    fields: ['fillRef', 'settledAmountSats', 'settledAt', 'receiptRef'],
    inert: true,
    kind: 'liquidity.settlement_receipt',
  }),
]

const riskProtocolMessages = (): ReadonlyArray<SkeletonProtocolMessage> => [
  new SkeletonProtocolMessage({
    description:
      'A buyer would request coverage for a defined claim/event. Inert: no coverage is solicited.',
    direction: 'request',
    fields: ['claimDescription', 'coverageAmountSats', 'coveragePeriod'],
    inert: true,
    kind: 'risk.coverage_request',
  }),
  new SkeletonProtocolMessage({
    description:
      'An underwriter would quote a premium for the risk. Inert: no premium is real and no risk is underwritten.',
    direction: 'offer',
    fields: ['requestRef', 'premiumSats', 'coverageAmountSats', 'terms'],
    inert: true,
    kind: 'risk.premium_quote',
  }),
  new SkeletonProtocolMessage({
    description:
      'A would-be policy binding. Inert: no policy is bound and no obligation is created.',
    direction: 'response',
    fields: ['quoteRef', 'policyRef', 'status'],
    inert: true,
    kind: 'risk.policy_bind',
  }),
  new SkeletonProtocolMessage({
    description:
      'A would-be premium/claim settlement receipt shape. Inert: no premium or claim is paid and no receipt is issued.',
    direction: 'receipt',
    fields: ['policyRef', 'kind', 'amountSats', 'settledAt', 'receiptRef'],
    inert: true,
    kind: 'risk.settlement_receipt',
  }),
]

export const projectLiquidityMarketSkeleton = (
  input: { generatedAt?: string | undefined } = {},
): LiquidityMarketSkeletonProjection =>
  new LiquidityMarketSkeletonProjection({
    authorityBoundary:
      'The liquidity market skeleton grants no quoting, matching, market-making, settlement, custody, or payout authority. It documents protocol shapes only and moves no money.',
    blockerRefs: ['blocker.product_promises.liquidity_market_unbuilt'],
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    inert: true,
    marketId: 'liquidity',
    moneyMovement: 'none',
    promiseGreen: false,
    promiseRef: 'promise:markets.open_protocol_markets.v1',
    protocolMessages: liquidityProtocolMessages(),
    schemaVersion: LiquidityMarketSkeletonSchemaVersion,
    settledTransactionCount: 0,
    sourceRefs: [
      'docs/transcripts/239.md',
      'docs/transcripts/213.md',
      'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
    ],
    staleness: skeletonStaleness([
      'open_market_skeleton_transaction_recorded',
      'product_promise_registry_updated',
    ]),
    state: 'skeleton',
    statusLabel:
      'Liquidity market is an inert typed protocol skeleton. No provider, no taker, no fillable quote, no match, no money movement, no settled transaction.',
    unsafeCopy:
      'Do not claim a liquidity market exists or is live, that liquidity can be provided or taken, that any quote is real or fillable, or that any liquidity transaction has settled.',
  })

export const projectRiskMarketSkeleton = (
  input: { generatedAt?: string | undefined } = {},
): RiskMarketSkeletonProjection =>
  new RiskMarketSkeletonProjection({
    agenticInsurancePolicyPrimitive: {
      description:
        'The Episode 239 illustration: an agent builds an insurance policy for a particular claim. Documented shape only — no policy can be bound, priced, or paid here.',
      inert: true,
      name: 'agentic_insurance_policy',
      policyFields: [
        'claimDescription',
        'coverageAmountSats',
        'premiumSats',
        'coveragePeriod',
        'underwriterRef',
        'beneficiaryRef',
        'terms',
      ],
    },
    authorityBoundary:
      'The risk market skeleton grants no underwriting, policy-binding, premium-collection, claims-adjudication, payout, custody, or insurance authority. It documents protocol shapes only and moves no money.',
    blockerRefs: ['blocker.product_promises.risk_market_unbuilt'],
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    inert: true,
    marketId: 'risk',
    moneyMovement: 'none',
    promiseGreen: false,
    promiseRef: 'promise:markets.open_protocol_markets.v1',
    protocolMessages: riskProtocolMessages(),
    schemaVersion: RiskMarketSkeletonSchemaVersion,
    settledTransactionCount: 0,
    sourceRefs: [
      'docs/transcripts/239.md',
      'docs/transcripts/213.md',
      'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
    ],
    staleness: skeletonStaleness([
      'open_market_skeleton_transaction_recorded',
      'product_promise_registry_updated',
    ]),
    state: 'skeleton',
    statusLabel:
      'Risk market is an inert typed protocol skeleton, including the agentic-insurance-policy primitive. No coverage, no premium, no bound policy, no claim payout, no money movement, no settled transaction.',
    unsafeCopy:
      'Do not claim a risk or insurance market exists, that an agentic insurance policy can be bound or underwritten, that a premium or claim can be paid, or that any risk transaction has settled.',
  })
