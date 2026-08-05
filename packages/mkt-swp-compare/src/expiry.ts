/**
 * Quote expiry: countdown data plus local enforcement (openagents#9318 §4).
 *
 * This is parity-plus over Boltz: our quotes are signed records with an
 * `expiration`, so the client can enforce expiry locally BEFORE funding.
 * An expired Quote is unusable — selection refuses it with the typed
 * identifier `swp_quote_expired` (§17: "Quote or reservation is expired") —
 * not merely styled stale. The clock is always injected; nothing here reads
 * wall time.
 *
 * Behaviour contract:
 * `openagents_web.swap_compare.quote_expiry_enforced.v1`.
 */
import type { CompareQuote } from "./model.js";

/**
 * The instant the Quote stops being usable: the earlier of the Quote's own
 * `expiration` and, when the Quote reserves capacity, the reservation
 * expiry. §17 folds both into `swp_quote_expired`; §5 adds that reservation
 * expiry releases only the accounting entry — it never cancels, refunds,
 * settles, or signs for anyone, which is why this stays a client-side
 * usability bound and not a lifecycle transition.
 */
export const effectiveExpiresAtSeconds = (quote: CompareQuote): number =>
  quote.reservation === null
    ? quote.expiresAtSeconds
    : Math.min(
        quote.expiresAtSeconds,
        quote.reservation.reservationExpiresAtSeconds,
      );

export type QuoteExpiryState =
  | {
      readonly state: "active";
      readonly secondsRemaining: number;
      readonly expiresAtSeconds: number;
      /** True when the reservation expiry is the binding bound. */
      readonly boundByReservation: boolean;
    }
  | {
      readonly state: "expired";
      readonly error: "swp_quote_expired";
      readonly expiredAtSeconds: number;
      readonly via: "quote" | "reservation";
    };

/**
 * Pure countdown state for one Quote at one observed instant. Expiry is
 * inclusive-exclusive: a Quote whose bound equals `nowSeconds` is already
 * expired (the record's validity ended at that second).
 */
export const quoteExpiryState = (
  quote: CompareQuote,
  nowSeconds: number,
): QuoteExpiryState => {
  const bound = effectiveExpiresAtSeconds(quote);
  const boundByReservation =
    quote.reservation !== null &&
    quote.reservation.reservationExpiresAtSeconds < quote.expiresAtSeconds;
  if (nowSeconds >= bound) {
    return {
      state: "expired",
      error: "swp_quote_expired",
      expiredAtSeconds: bound,
      via: boundByReservation ? "reservation" : "quote",
    };
  }
  return {
    state: "active",
    secondsRemaining: bound - nowSeconds,
    expiresAtSeconds: bound,
    boundByReservation,
  };
};
