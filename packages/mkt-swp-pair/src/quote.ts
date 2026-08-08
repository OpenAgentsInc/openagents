/**
 * Quote amount/fee terms and their local reproduction (openagents#9316 §5).
 *
 * MKT-SWP §3.3: every Quote states separated fee components
 * (`provider_fee`, `miner_fee_budget`, `lightning_routing_fee_budget`),
 * who pays, the rounding rule (`floor_output_sats`), and a deterministic
 * `amount_equation` from the v1 allowlist. The output amount is the **fill
 * promise** — a provider may not reduce it after Order because its route
 * or miner fee changed. This module re-derives the promised output from
 * the quoted terms with exact bigint arithmetic (mirroring the immortal
 * client's `verify_amount_equation`); a mismatch surfaces
 * `swp_amount_equation_mismatch` and the panel never renders an
 * unreproduced number as the promise.
 */
import { satsFromWire } from "./amount.js";
import type { PinnedPriceFeed } from "./price-feed.js";

/**
 * The v1 `amount_equation` allowlist. MKT-SWP §3.3 defines both
 * identifiers as names for the single v1 integer-satoshi equation
 * (`output = input - provider_fee - miner_fee_budget -
 * lightning_routing_fee_budget`); a verifier applies that arithmetic to
 * whichever identifier is declared, so the rendered label always
 * describes the arithmetic that produced the number.
 */
export const AMOUNT_EQUATIONS = [
  "input_minus_provider_and_quoted_fees",
  "one_to_one_less_quoted_fees",
] as const;

export type AmountEquation = (typeof AMOUNT_EQUATIONS)[number];

export const ROUNDING_RULE = "floor_output_sats";

export type FeePayer = "requester" | "provider";

/**
 * Who pays each fee component (MKT-SWP §3.3: every Quote states "who pays
 * each miner and routing fee"). One payer per component — a Quote where
 * the requester funds the miner budget while the provider absorbs the
 * routing budget is representable and rendered per row, never collapsed
 * to one scalar stamped onto every component. The §4.4 finite list of
 * *selectable* fee-payer options belongs to the Order flow (SWAP-3);
 * these are the payers of the Quote the session holds.
 */
export interface FeePayerAssignment {
  readonly providerFee: FeePayer;
  readonly minerFeeBudget: FeePayer;
  readonly lightningRoutingFeeBudget: FeePayer;
}

/**
 * The amount and fee members of one Quote's `terms`, verbatim wire
 * strings. The session holds whichever Quote SWAP-3 selected; this module
 * only renders and re-derives its terms. The ordered asset-id pair the
 * Quote commits (§4.3) rides along so applying the quote can bind it to
 * the selected direction rather than trusting call ordering.
 */
export interface SwapQuoteTerms {
  /** The Quote's committed ordered pair (§4.3), exact `asset_id` strings. */
  readonly inputAssetId: string;
  readonly outputAssetId: string;
  readonly inputAmount: string;
  readonly outputAmount: string;
  readonly feeBps: string;
  readonly providerFee: string;
  readonly minerFeeBudget: string;
  readonly lightningRoutingFeeBudget: string;
  readonly maximumTotalFee: string;
  readonly feePayers: FeePayerAssignment;
  readonly rounding: string;
  readonly amountEquation: string;
  /** Exact pinned price feed (MKT-SWP §3.4), `null` when none is pinned. */
  readonly priceFeed: PinnedPriceFeed | null;
}

export interface VerifiedQuoteAmounts {
  readonly inputSats: bigint;
  readonly outputSats: bigint;
  readonly feeBps: bigint;
  readonly providerFeeSats: bigint;
  readonly minerFeeBudgetSats: bigint;
  readonly lightningRoutingFeeBudgetSats: bigint;
  readonly maximumTotalFeeSats: bigint;
  readonly amountEquation: AmountEquation;
}

export type QuoteTermsVerification =
  | { readonly ok: true; readonly amounts: VerifiedQuoteAmounts }
  | {
      readonly ok: false;
      readonly error:
        | "swp_invalid_amount"
        | "swp_invalid_fee"
        | "swp_amount_equation_mismatch";
      readonly detail: string;
    };

const isAmountEquation = (value: string): value is AmountEquation =>
  (AMOUNT_EQUATIONS as readonly string[]).includes(value);

/**
 * Reproduce the promised output from the quoted terms. Exact rules,
 * matching the engine verifier:
 * - every amount is a canonical §3.2 wire string;
 * - `fee_bps <= 10000` and `provider_fee = floor(input * fee_bps / 10000)`;
 * - `output = input - provider_fee - miner_fee_budget -
 *   lightning_routing_fee_budget`, non-negative and positive;
 * - `maximum_total_fee` is exactly the component sum;
 * - `rounding` is `floor_output_sats`; `amount_equation` is allowlisted.
 */
export const verifyQuoteTerms = (
  terms: SwapQuoteTerms,
): QuoteTermsVerification => {
  const amounts = {
    inputSats: satsFromWire(terms.inputAmount),
    outputSats: satsFromWire(terms.outputAmount),
    feeBps: satsFromWire(terms.feeBps),
    providerFeeSats: satsFromWire(terms.providerFee),
    minerFeeBudgetSats: satsFromWire(terms.minerFeeBudget),
    lightningRoutingFeeBudgetSats: satsFromWire(
      terms.lightningRoutingFeeBudget,
    ),
    maximumTotalFeeSats: satsFromWire(terms.maximumTotalFee),
  };
  for (const [name, value] of Object.entries(amounts)) {
    if (value === null) {
      return {
        ok: false,
        error: "swp_invalid_amount",
        detail: `${name} is not a canonical satoshi decimal string`,
      };
    }
  }
  const inputSats = amounts.inputSats as bigint;
  const outputSats = amounts.outputSats as bigint;
  const feeBps = amounts.feeBps as bigint;
  const providerFeeSats = amounts.providerFeeSats as bigint;
  const minerFeeBudgetSats = amounts.minerFeeBudgetSats as bigint;
  const lightningRoutingFeeBudgetSats =
    amounts.lightningRoutingFeeBudgetSats as bigint;
  const maximumTotalFeeSats = amounts.maximumTotalFeeSats as bigint;

  if (feeBps > 10_000n) {
    return {
      ok: false,
      error: "swp_invalid_fee",
      detail: "fee_bps exceeds 10000",
    };
  }
  if (!isAmountEquation(terms.amountEquation)) {
    return {
      ok: false,
      error: "swp_amount_equation_mismatch",
      detail: `amount_equation "${terms.amountEquation}" is not in the v1 allowlist`,
    };
  }
  if (terms.rounding !== ROUNDING_RULE) {
    return {
      ok: false,
      error: "swp_amount_equation_mismatch",
      detail: `rounding "${terms.rounding}" is not ${ROUNDING_RULE}`,
    };
  }
  const expectedProviderFee = (inputSats * feeBps) / 10_000n;
  if (providerFeeSats !== expectedProviderFee) {
    return {
      ok: false,
      error: "swp_amount_equation_mismatch",
      detail: `provider_fee ${providerFeeSats} does not equal floor(input * fee_bps / 10000) = ${expectedProviderFee}`,
    };
  }
  const expectedOutput =
    inputSats -
    providerFeeSats -
    minerFeeBudgetSats -
    lightningRoutingFeeBudgetSats;
  if (expectedOutput <= 0n || outputSats !== expectedOutput) {
    return {
      ok: false,
      error: "swp_amount_equation_mismatch",
      detail: `output_amount ${outputSats} does not reproduce from the quoted terms (expected ${expectedOutput})`,
    };
  }
  const expectedTotal =
    providerFeeSats + minerFeeBudgetSats + lightningRoutingFeeBudgetSats;
  if (maximumTotalFeeSats !== expectedTotal) {
    return {
      ok: false,
      error: "swp_amount_equation_mismatch",
      detail: `maximum_total_fee ${maximumTotalFeeSats} is not the component sum ${expectedTotal}`,
    };
  }
  return {
    ok: true,
    amounts: {
      inputSats,
      outputSats,
      feeBps,
      providerFeeSats,
      minerFeeBudgetSats,
      lightningRoutingFeeBudgetSats,
      maximumTotalFeeSats,
      amountEquation: terms.amountEquation,
    },
  };
};
