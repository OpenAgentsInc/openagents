/**
 * The per-signer lane fold. Two lanes, not one (issue #9321 scope 1):
 * requester and provider Status sequences are separate authored streams.
 *
 * Laws:
 * - A missing sequence number renders as a visible gap (`swp_status_gap`),
 *   unknown-with-explanation, never optimistic progress. A later Status
 *   cannot close it; only the exact missing record can.
 * - Two records from one author at the same `(session, order, seq)` are a
 *   fork (`swp_status_fork`): both retained, never resolved by arrival
 *   time, never hidden behind one chosen event.
 * - The fold is a function of the SET of records: out-of-order arrival
 *   converges deterministically and duplicates are idempotent.
 * - Claims after the first gap, fork, or previous-chain break are retained
 *   but cannot advance the last valid rung.
 */
import type { ParticipantRole, StatusClaim } from "./model.js";

export interface LaneSlot {
  readonly seq: number;
  /** All distinct records at this sequence, ordered by event id (never by arrival). */
  readonly records: readonly StatusClaim[];
  readonly fork: boolean;
  /** True when the record's `previous` does not reference the accepted predecessor. */
  readonly previousMismatch: boolean;
}

export interface LaneGap {
  readonly seq: number;
  readonly error: "swp_status_gap";
}

export interface LaneFork {
  readonly seq: number;
  readonly ids: readonly string[];
  readonly error: "swp_status_fork";
}

export interface LaneProjection {
  readonly role: ParticipantRole;
  readonly author: string | null;
  /** Present sequences ascending; gaps are in `gaps`, not silently skipped. */
  readonly slots: readonly LaneSlot[];
  readonly gaps: readonly LaneGap[];
  readonly forks: readonly LaneFork[];
  readonly maxSeq: number;
  /**
   * The contiguous, fork-free, chain-valid prefix: the only claims that may
   * advance the session. Everything else is retained for display only.
   */
  readonly validClaims: readonly StatusClaim[];
  /** Where advancement stopped, if it stopped before maxSeq. */
  readonly integrity:
    | { readonly kind: "intact" }
    | { readonly kind: "gap"; readonly atSeq: number }
    | { readonly kind: "fork"; readonly atSeq: number }
    | { readonly kind: "previous_mismatch"; readonly atSeq: number };
}

const byId = (left: StatusClaim, right: StatusClaim): number =>
  left.id < right.id ? -1 : left.id > right.id ? 1 : 0;

export function foldLane(role: ParticipantRole, claims: Iterable<StatusClaim>): LaneProjection {
  // Idempotence: identical event ids collapse to one record.
  const distinct = new Map<string, StatusClaim>();
  let author: string | null = null;
  for (const claim of claims) {
    if (claim.role !== role) continue;
    author = author ?? claim.author;
    distinct.set(claim.id, claim);
  }

  const bySeq = new Map<number, StatusClaim[]>();
  for (const claim of distinct.values()) {
    const records = bySeq.get(claim.seq) ?? [];
    records.push(claim);
    bySeq.set(claim.seq, records);
  }

  const seqs = [...bySeq.keys()].sort((a, b) => a - b);
  const maxSeq = seqs.length === 0 ? -1 : seqs[seqs.length - 1]!;

  const slots: LaneSlot[] = [];
  const gaps: LaneGap[] = [];
  const forks: LaneFork[] = [];
  let integrity: LaneProjection["integrity"] = { kind: "intact" };
  const validClaims: StatusClaim[] = [];
  let acceptedPreviousId: string | undefined;
  let broken = false;

  for (let seq = 0; seq <= maxSeq; seq += 1) {
    const records = (bySeq.get(seq) ?? []).slice().sort(byId);
    if (records.length === 0) {
      gaps.push({ seq, error: "swp_status_gap" });
      if (!broken) {
        broken = true;
        integrity = { kind: "gap", atSeq: seq };
      }
      continue;
    }
    const fork = records.length > 1;
    if (fork) {
      forks.push({ seq, ids: records.map(({ id }) => id), error: "swp_status_fork" });
    }
    const record = records[0]!;
    const previousMismatch =
      seq === 0
        ? record.previous !== undefined
        : !fork && !broken && record.previous !== acceptedPreviousId;
    slots.push({ seq, records, fork, previousMismatch });
    if (broken) continue;
    if (fork) {
      broken = true;
      integrity = { kind: "fork", atSeq: seq };
      continue;
    }
    if (previousMismatch) {
      broken = true;
      integrity = { kind: "previous_mismatch", atSeq: seq };
      continue;
    }
    validClaims.push(record);
    acceptedPreviousId = record.id;
  }

  return {
    role,
    author,
    slots,
    gaps,
    forks,
    maxSeq,
    validClaims,
    integrity,
  };
}
