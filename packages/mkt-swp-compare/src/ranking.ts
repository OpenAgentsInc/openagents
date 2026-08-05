/**
 * Best-execution ranking for competing Quotes (openagents#9318 §1).
 *
 * THE RANKING RULE, explicitly:
 *
 * 1. Commitment tier is the MAJOR key. A cheaper indicative Quote is never
 *    presented as if it were as binding as a firm one, so no amount of
 *    price advantage lifts an indicative Quote above any firm Quote:
 *      tier 0 — firm + hard reservation (conforming)
 *      tier 1 — firm + soft reservation (conforming)
 *      tier 2 — indicative (conforming; reservation raises no tier)
 *      tier 3 — nonconforming (retained, never selectable)
 *      tier 4 — expired (retained, never selectable)
 * 2. Within a tier, larger `outputAmountSats` first (best execution).
 * 3. Then later effective expiry first (more time to act on equal terms).
 * 4. Then fresher `createdAtSeconds` first.
 * 5. Final total tiebreak: ascending `eventId` (lexicographic). The
 *    comparator returns 0 only for the same event, so ranking is a total
 *    order — deterministic and stable for equal terms regardless of input
 *    order.
 *
 * Behaviour contract:
 * `openagents_web.swap_compare.firm_indicative_distinct.v1` (ranking half).
 */
import { effectiveExpiresAtSeconds, quoteExpiryState } from "./expiry.js";
import { quoteConformance, type CompareQuote } from "./model.js";

export type CommitmentTier = 0 | 1 | 2 | 3 | 4;

export const commitmentTier = (
  quote: CompareQuote,
  nowSeconds: number,
): CommitmentTier => {
  if (quoteExpiryState(quote, nowSeconds).state === "expired") return 4;
  if (quoteConformance(quote).length > 0) return 3;
  if (quote.quoteClass === "firm") {
    return quote.reservationClass === "hard" ? 0 : 1;
  }
  return 2;
};

const compareBigintDesc = (a: bigint, b: bigint): number =>
  a === b ? 0 : a > b ? -1 : 1;

/**
 * Total-order comparator implementing the ranking rule above. Exported so
 * a test can assert the rule directly; `rankQuotes` is the table entry.
 */
export const compareQuotes =
  (nowSeconds: number) =>
  (a: CompareQuote, b: CompareQuote): number => {
    const tierDelta =
      commitmentTier(a, nowSeconds) - commitmentTier(b, nowSeconds);
    if (tierDelta !== 0) return tierDelta;
    const outputDelta = compareBigintDesc(a.outputAmountSats, b.outputAmountSats);
    if (outputDelta !== 0) return outputDelta;
    const expiryDelta =
      effectiveExpiresAtSeconds(b) - effectiveExpiresAtSeconds(a);
    if (expiryDelta !== 0) return expiryDelta;
    const freshnessDelta = b.createdAtSeconds - a.createdAtSeconds;
    if (freshnessDelta !== 0) return freshnessDelta;
    return a.eventId < b.eventId ? -1 : a.eventId > b.eventId ? 1 : 0;
  };

/** Rank a Quote set best-execution first under the documented rule. */
export const rankQuotes = (
  quotes: readonly CompareQuote[],
  nowSeconds: number,
): readonly CompareQuote[] => [...quotes].sort(compareQuotes(nowSeconds));
