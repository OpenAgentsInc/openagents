// Three-way inference revenue split (EPIC #5474 / sub-EPIC #5475, child #5489).
//
// PURE. No Effect runtime, no IO, no clock, no env, no D1. Given a priced
// inference request (the pure `PriceResult` from `pricing.ts`), this module
// computes how the request's GROSS PROFIT (margin) fans out three ways, per the
// inference-gateway business doc (`docs/inference/2026-06-19-inference-gateway-
// business.md` §1: "every dollar of inference can split three ways — OpenAgents
// margin, the serving node, and the referrer") and the revshare-everywhere
// capstone (`docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md` §3).
//
// What splits: the MARGIN (chargeUsd − costUsd), not the whole charge. Our
// marginal cost (`costUsd`) is paid out to the upstream provider/quota and is
// never revshare; only the spread we keep is divisible. The three shares are:
//
//   - OpenAgents — the platform's retained margin.
//   - serving node — the contributor whose compute served the request (a Pylon
//     node / task worker). Zero when we served it from first-party quota
//     (Vertex) or pure passthrough with no contributor.
//   - referrer — the account that referred the paying customer, ongoing/
//     indefinite on ALL of their inference spend (sub-EPIC #5475).
//
// BOUNDARY: this module only DECIDES the numbers. It moves no money, writes no
// ledger row, and never reads who the referrer/serving node actually is. The
// accrual module (`inference-referral-accrual.ts`) feeds the REFERRER share into
// the existing RL-1 referral payout ledger; the serving-node share is fed by the
// sibling serving-node payout work (out of scope here — this module exposes the
// computed serving-node share additively so that work can consume it without a
// second split definition).

import { type FundingKind, type PriceResult } from './pricing'
import { DEFAULT_BTC_USD } from './usd-msat-conversion'

// Default referrer share of MARGIN for inference, in basis points. The standing
// RL-1 referral policy (`SITE_REFERRAL_PAYOUT_PERCENT_BPS`) is 5% of the
// qualifying amount; the inference accrual feeds the request's MARGIN as that
// qualifying amount, so the referrer's actual cut is 5% of margin. This default
// is ALIGNED to that 500 bps so the split's `referrer` share equals what the
// ledger accrues — the dashboard projection and the ledger agree by
// construction. Tunable here; if you change the ledger policy, change this to
// match (or the dashboard projection will diverge from the accrued amount).
export const DEFAULT_INFERENCE_REFERRER_MARGIN_BPS = 500 as const

// Default serving-node share of MARGIN, in basis points, WHEN a contributor
// node served the request. 30% of margin is the launch default for network
// supply (first-party Vertex quota / passthrough served requests have no
// serving node and this share is zero). Tunable; the serving-node payout work
// owns the live wiring, this is the shared definition of its size.
export const DEFAULT_INFERENCE_SERVING_NODE_MARGIN_BPS = 3000 as const

export type InferenceSplitWeights = Readonly<{
  // Referrer share of margin, in basis points (0..10000).
  referrerMarginBps: number
  // Serving-node share of margin, in basis points (0..10000). Applied ONLY when
  // `servedByContributor` is true; otherwise the serving-node share is zero and
  // its margin stays with OpenAgents.
  servingNodeMarginBps: number
}>

export const DEFAULT_INFERENCE_SPLIT_WEIGHTS: InferenceSplitWeights = {
  referrerMarginBps: DEFAULT_INFERENCE_REFERRER_MARGIN_BPS,
  servingNodeMarginBps: DEFAULT_INFERENCE_SERVING_NODE_MARGIN_BPS,
}

export type InferenceSplitInput = Readonly<{
  // The priced request (pure pricing output). `chargeUsd` is the customer charge
  // after any funding discount; `costUsd` is our marginal cost. Margin is the
  // difference.
  priced: PriceResult
  // True when a contributor (Pylon node / task worker) served the request, so
  // the serving-node share is non-zero. False for first-party quota / pure
  // passthrough (no contributor to pay).
  servedByContributor: boolean
  // Override split weights (tests / future policy). Defaults to the launch
  // weights above.
  weights?: InferenceSplitWeights
}>

// One share of the split. USD is the source-of-truth amount (pricing is USD-
// pure); sats is the floor-rounded sat amount used to FEED the sat-denominated
// referral payout ledger. Sats can be 0 for a tiny per-request margin even when
// USD is a small positive — that is expected for fine-grained inference accrual
// (the ledger refuses a zero-sat qualifying amount, so a sub-1-sat request
// simply does not accrue that request; spend accrues as it crosses 1 sat).
export type InferenceSplitShare = Readonly<{
  usd: number
  sats: number
}>

export type InferenceSplit = Readonly<{
  // Funding kind the underlying charge reflected (carried through for callers).
  fundingKind: FundingKind
  // Total customer charge (USD) for the request, after funding discount.
  chargeUsd: number
  // Our marginal cost (USD) — paid upstream, never revshare.
  costUsd: number
  // The divisible margin (USD) = max(0, chargeUsd − costUsd).
  marginUsd: number
  // The divisible margin in FLOOR-rounded sats. This is the qualifying amount
  // the accrual feeds the RL-1 ledger (the ledger then applies its 5% referral
  // policy to it). Exposed so the feed and the dashboard share one conversion.
  marginSats: number
  // The three shares (of margin).
  openagents: InferenceSplitShare
  servingNode: InferenceSplitShare
  referrer: InferenceSplitShare
}>

// BTC/USD reference rate for the USD→sat conversion used to feed the sat-
// denominated referral ledger. Pinned to the SHARED single-source rate
// (`usd-msat-conversion.ts`, #5497) so the split, the metering charge, and the
// USD->msat credit bridge all use one rate. !! BILLING TODO: replace with a live
// oracle read when one is wired (the shared module is the single knob).
export const SPLIT_DEFAULT_BTC_USD: number = DEFAULT_BTC_USD

const SATS_PER_BTC = 100_000_000 as const

// Convert a USD amount to a FLOOR-rounded sat amount. Floor (not ceil) because
// the referral ledger caps and accrues; never round a share UP past what was
// actually earned. A non-finite/≤0 USD maps to 0 sats.
export const usdToSatsFloor = (
  usd: number,
  btcUsd: number = SPLIT_DEFAULT_BTC_USD,
): number => {
  if (!Number.isFinite(usd) || usd <= 0) return 0
  if (!Number.isFinite(btcUsd) || btcUsd <= 0) return 0
  return Math.floor((usd / btcUsd) * SATS_PER_BTC)
}

const clampBps = (bps: number): number => {
  if (!Number.isFinite(bps) || bps <= 0) return 0
  return Math.min(10000, Math.floor(bps))
}

const share = (usd: number, btcUsd: number): InferenceSplitShare => ({
  sats: usdToSatsFloor(usd, btcUsd),
  usd,
})

/**
 * Compute the three-way inference margin split. PURE.
 *
 * The margin (gross profit) is split: the referrer takes `referrerMarginBps`,
 * the serving node takes `servingNodeMarginBps` WHEN a contributor served it,
 * and OpenAgents keeps the rest. Weights that would sum past 100% of margin are
 * clamped so OpenAgents' share never goes negative (the platform never pays out
 * more than it earned). A zero/negative margin yields all-zero shares.
 */
export const computeInferenceSplit = (
  input: InferenceSplitInput,
  btcUsd: number = SPLIT_DEFAULT_BTC_USD,
): InferenceSplit => {
  const weights = input.weights ?? DEFAULT_INFERENCE_SPLIT_WEIGHTS
  const chargeUsd = Math.max(0, input.priced.chargeUsd)
  const costUsd = Math.max(0, input.priced.costUsd)
  const marginUsd = Math.max(0, chargeUsd - costUsd)

  const referrerBps = clampBps(weights.referrerMarginBps)
  const servingBps = input.servedByContributor
    ? clampBps(weights.servingNodeMarginBps)
    : 0

  // Clamp the combined fan-out so OpenAgents never goes negative. If the two
  // outbound shares would exceed the whole margin, scale them down
  // proportionally; OpenAgents keeps zero in that degenerate case.
  const outboundBps = referrerBps + servingBps
  const scale = outboundBps > 10000 ? 10000 / outboundBps : 1

  const referrerUsd = (marginUsd * referrerBps * scale) / 10000
  const servingNodeUsd = (marginUsd * servingBps * scale) / 10000
  const openagentsUsd = Math.max(0, marginUsd - referrerUsd - servingNodeUsd)

  return {
    chargeUsd,
    costUsd,
    fundingKind: input.priced.fundingKind,
    marginSats: usdToSatsFloor(marginUsd, btcUsd),
    marginUsd,
    openagents: share(openagentsUsd, btcUsd),
    referrer: share(referrerUsd, btcUsd),
    servingNode: share(servingNodeUsd, btcUsd),
  }
}
