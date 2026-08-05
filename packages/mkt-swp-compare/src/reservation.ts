/**
 * Reservation fork detection (openagents#9318 §3, MKT-SWP §5).
 *
 * Two active reservations that claim the same allocation sequence in one
 * provider bucket, or that make the capacity inequality false, are an
 * attributable `swp_reservation_fork`. The profile is explicit: forks are
 * RETAINED, never resolved by arrival time. Detection here is therefore a
 * pure function of the observed reservation SET — ingestion order cannot
 * change the result, and every fork record names all member Quotes sorted
 * by event id (a canonical order, not an arrival order).
 *
 * Behaviour contract:
 * `openagents_web.swap_compare.reservation_proof_class_distinct.v1`
 * (fork-retention half; the proof-class rendering half lives in `view.ts`).
 */
import type { CompareQuote, ReservationTerms } from "./model.js";

/** One observed reserving Quote, flattened for fork accounting. */
export interface ReservationObservation {
  readonly quoteEventId: string;
  readonly providerPubkey: string;
  readonly terms: ReservationTerms;
}

export interface ReservationFork {
  readonly error: "swp_reservation_fork";
  readonly kind: "duplicate_allocation_sequence" | "capacity_exceeded";
  /** Attribution: the provider that equivocated. */
  readonly providerPubkey: string;
  readonly capacityBucketId: string;
  /** Member Quote event ids, sorted lexicographically — never by arrival. */
  readonly memberQuoteEventIds: readonly string[];
}

/** Separator no pubkey, bucket id, or asset id can contain. */
const KEY_SEPARATOR = "\n";

const bucketKey = (observation: ReservationObservation): string =>
  [
    observation.providerPubkey,
    observation.terms.capacityBucketId,
    observation.terms.reservedAssetId,
  ].join(KEY_SEPARATOR);

export const observeReservations = (
  quotes: readonly CompareQuote[],
): readonly ReservationObservation[] =>
  quotes.flatMap(quote =>
    quote.reservation === null
      ? []
      : [
          {
            quoteEventId: quote.eventId,
            providerPubkey: quote.providerPubkey,
            terms: quote.reservation,
          },
        ],
  );

/**
 * Detect forks across one observed reservation set.
 *
 * - `duplicate_allocation_sequence`: two active reservations in one
 *   provider/bucket/asset claim the same `allocation_sequence`.
 * - `capacity_exceeded`: when the bucket's committed capacity is known to
 *   the client, the sum of active reserved amounts exceeds it. The capacity
 *   map is optional because §5 only commits a digest publicly; absence of
 *   the map means this check is simply not performed — it never counts as
 *   a pass of anything.
 *
 * Output order is canonical (bucket key, then fork kind), so two ingestion
 * orders of the same observations produce identical results.
 */
export const detectReservationForks = (
  observations: readonly ReservationObservation[],
  committedCapacitySats: ReadonlyMap<string, bigint> | null = null,
): readonly ReservationFork[] => {
  const buckets = new Map<string, ReservationObservation[]>();
  for (const observation of observations) {
    const key = bucketKey(observation);
    const bucket = buckets.get(key);
    if (bucket === undefined) buckets.set(key, [observation]);
    else bucket.push(observation);
  }

  const forks: ReservationFork[] = [];
  const sortedKeys = [...buckets.keys()].sort();
  for (const key of sortedKeys) {
    const bucket = buckets.get(key) ?? [];
    const [providerPubkey = "", capacityBucketId = ""] = key.split(KEY_SEPARATOR);

    const bySequence = new Map<string, string[]>();
    for (const observation of bucket) {
      const sequence = observation.terms.allocationSequence.toString();
      const members = bySequence.get(sequence);
      if (members === undefined) {
        bySequence.set(sequence, [observation.quoteEventId]);
      } else {
        members.push(observation.quoteEventId);
      }
    }
    const duplicateSequences = [...bySequence.entries()]
      .filter(([, members]) => new Set(members).size > 1)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    for (const [, members] of duplicateSequences) {
      forks.push({
        error: "swp_reservation_fork",
        kind: "duplicate_allocation_sequence",
        providerPubkey,
        capacityBucketId,
        memberQuoteEventIds: [...new Set(members)].sort(),
      });
    }

    const committed = committedCapacitySats?.get(key);
    if (committed !== undefined) {
      const reserved = bucket.reduce(
        (sum, observation) => sum + observation.terms.reservedAmountSats,
        0n,
      );
      if (reserved > committed) {
        forks.push({
          error: "swp_reservation_fork",
          kind: "capacity_exceeded",
          providerPubkey,
          capacityBucketId,
          memberQuoteEventIds: bucket
            .map(observation => observation.quoteEventId)
            .sort(),
        });
      }
    }
  }
  return forks;
};

/** Canonical key for the committed-capacity map, mirroring fork buckets. */
export const capacityBucketMapKey = (
  providerPubkey: string,
  capacityBucketId: string,
  reservedAssetId: string,
): string => [providerPubkey, capacityBucketId, reservedAssetId].join(KEY_SEPARATOR);

/** Event ids of every Quote implicated in any fork of the set. */
export const forkedQuoteEventIds = (
  forks: readonly ReservationFork[],
): ReadonlySet<string> =>
  new Set(forks.flatMap(fork => fork.memberQuoteEventIds));
