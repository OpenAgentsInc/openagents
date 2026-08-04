import type { Event } from "nostr-effect/pure";
import { Effect, Schema } from "effect";
import { OK_REASONS } from "./generated.js";

export class MktClientStateError extends Schema.TaggedErrorClass<MktClientStateError>()(
  "MktClientStateError",
  { code: Schema.Literals(["expired"]), message: Schema.String },
) {}

export type AdmissionDecision<Result = unknown> =
  | {
      readonly decision: "stored";
      readonly coordinate: string;
      readonly result: Result | undefined;
    }
  | {
      readonly decision: "duplicate";
      readonly coordinate: string;
      readonly previousResult: Result | undefined;
    }
  | {
      readonly decision: "idempotency-conflict";
      readonly coordinate: string;
      readonly code: "idempotency-conflict";
      readonly reasonCode: "mkt_idempotency_conflict";
      readonly reasonMessage: typeof OK_REASONS.mkt_idempotency_conflict;
    };

export interface PrivateAdmissionStore<Result = unknown> {
  readonly records: Map<
    string,
    { readonly id: string; readonly raw: string; readonly result: Result | undefined }
  >;
}

export interface StatusRecord {
  readonly id: string;
  readonly sessionId: string;
  readonly orderId: string;
  readonly author: string;
  readonly seq: number;
  readonly previous?: string;
  readonly state?: string;
}

export type StatusSequenceDecision =
  | {
      readonly decision: "contiguous";
      readonly lastContiguousSeq: number;
      readonly retainIds: readonly string[];
    }
  | {
      readonly decision: "gap";
      readonly missingSequences: readonly number[];
      readonly lastContiguousSeq: number;
      readonly retainIds: readonly string[];
    }
  | {
      readonly decision: "fork";
      readonly forkKey: {
        readonly sessionId: string;
        readonly orderId: string;
        readonly author: string;
        readonly seq: number;
      };
      readonly retainIds: readonly string[];
      readonly advanceState: false;
    };

export function createPrivateAdmissionStore<Result = unknown>(): PrivateAdmissionStore<Result> {
  return { records: new Map() };
}

export function privateCoordinate(event: Pick<Event, "pubkey" | "kind" | "tags">): string {
  const identifiers = event.tags.filter((tag) => tag[0] === "d");
  if (identifiers.length !== 1 || identifiers[0]?.length !== 2) {
    throw new Error("private MKT event requires exactly one d tag");
  }
  return `${event.pubkey}:${event.kind}:${identifiers[0][1]}`;
}

export function admitPrivateRecord<Result = unknown>(
  store: PrivateAdmissionStore<Result>,
  event: Event,
  raw: string,
  result?: Result,
): AdmissionDecision<Result> {
  const coordinate = privateCoordinate(event);
  const stored = store.records.get(coordinate);
  if (stored === undefined) {
    store.records.set(coordinate, { id: event.id, raw, result });
    return { decision: "stored", coordinate, result };
  }
  if (stored.id === event.id && stored.raw === raw) {
    return { decision: "duplicate", coordinate, previousResult: stored.result };
  }
  return {
    decision: "idempotency-conflict",
    coordinate,
    code: "idempotency-conflict",
    reasonCode: "mkt_idempotency_conflict",
    reasonMessage: OK_REASONS.mkt_idempotency_conflict,
  };
}

export function generateIdempotencyKey(randomBytes?: Uint8Array): string {
  const bytes = randomBytes ?? globalThis.crypto.getRandomValues(new Uint8Array(32));
  if (bytes.byteLength !== 32) throw new Error("idempotency material must contain 32 bytes");
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function analyzeStatusSequence(statuses: readonly StatusRecord[]): StatusSequenceDecision {
  if (statuses.length === 0)
    return { decision: "contiguous", lastContiguousSeq: -1, retainIds: [] };
  const streams = new Set(
    statuses.map((status) => `${status.sessionId}:${status.orderId}:${status.author}`),
  );
  if (streams.size !== 1)
    throw new Error("status sequence analysis requires one session, order, and signer");
  const grouped = new Map<string, StatusRecord[]>();
  for (const status of statuses) {
    const key = `${status.orderId}:${status.author}:${status.seq}`;
    const existing = grouped.get(key) ?? [];
    existing.push(status);
    grouped.set(key, existing);
  }
  for (const records of grouped.values()) {
    const ids = [...new Set(records.map((record) => record.id))];
    if (ids.length > 1) {
      const record = records[0]!;
      return {
        decision: "fork",
        forkKey: {
          sessionId: record.sessionId,
          orderId: record.orderId,
          author: record.author,
          seq: record.seq,
        },
        retainIds: ids,
        advanceState: false,
      };
    }
  }
  const sequences = Array.from(new Set(statuses.map((status) => status.seq)));
  sequences.sort((left, right) => left - right);
  const maximum = sequences.at(-1)!;
  const missingSequences: number[] = [];
  let lastContiguousSeq = -1;
  for (let sequence = 0; sequence <= maximum; sequence += 1) {
    if (sequences.includes(sequence)) {
      if (missingSequences.length === 0) lastContiguousSeq = sequence;
    } else {
      missingSequences.push(sequence);
    }
  }
  return missingSequences.length === 0
    ? { decision: "contiguous", lastContiguousSeq, retainIds: statuses.map(({ id }) => id) }
    : {
        decision: "gap",
        missingSequences,
        lastContiguousSeq,
        retainIds: statuses.map(({ id }) => id),
      };
}

export function activeSupersedingQuote(
  records: readonly { readonly id: string; readonly previous?: string }[],
): {
  readonly decision: "supersede";
  readonly activeQuoteId: string;
  readonly retainIds: readonly string[];
} {
  if (records.length === 0) throw new Error("at least one quote is required");
  const superseded = new Set(
    records.flatMap((record) => (record.previous ? [record.previous] : [])),
  );
  const active = records.filter((record) => !superseded.has(record.id));
  if (active.length !== 1) throw new Error("quote supersession is ambiguous");
  return {
    decision: "supersede",
    activeQuoteId: active[0]!.id,
    retainIds: records.map((record) => record.id),
  };
}

export function reserveCapacity(
  capacity: number,
  quotes: readonly { readonly id: string; readonly reservation: string; readonly units: number }[],
): {
  readonly decision: "reserved" | "conflict";
  readonly code?: "double_reservation";
  readonly retainIds: readonly string[];
  readonly effectiveReservedUnits: number;
} {
  let reserved = 0;
  for (const quote of quotes) {
    if (quote.reservation !== "hard") continue;
    if (reserved + quote.units > capacity)
      return {
        decision: "conflict",
        code: "double_reservation",
        retainIds: quotes.map(({ id }) => id),
        effectiveReservedUnits: reserved,
      };
    reserved += quote.units;
  }
  return {
    decision: "reserved",
    retainIds: quotes.map(({ id }) => id),
    effectiveReservedUnits: reserved,
  };
}

export function isExpired(expiration: number, observedAt: number): boolean {
  return expiration <= observedAt;
}

export const ensureNotExpired = Effect.fn("NipMkt.ensureNotExpired")(function* (
  expiration: number,
  observedAt: number,
) {
  if (isExpired(expiration, observedAt)) {
    return yield* new MktClientStateError({ code: "expired", message: "record is expired" });
  }
});

export function expiryDecision(
  expiration: number,
  observedAt: number,
): {
  readonly decision: "accept" | "reject";
  readonly code?: "expired";
  readonly inclusive: true;
  readonly performExternalEffect: boolean;
} {
  return isExpired(expiration, observedAt)
    ? {
        decision: "reject",
        code: "expired",
        inclusive: true,
        performExternalEffect: false,
      }
    : { decision: "accept", inclusive: true, performExternalEffect: true };
}

export function authorizeParticipant(author: string, allowedAuthors: readonly string[]): boolean {
  return allowedAuthors.includes(author);
}

export function authorizationDecision(
  kind: 39607 | 39608 | 39609,
  author: string,
  allowedAuthors: readonly string[],
): {
  readonly decision: "accept" | "reject";
  readonly code?: "unauthorized_status" | "unauthorized_cancel" | "unauthorized_close";
  readonly advanceState?: false;
  readonly cancelled?: false;
  readonly terminal?: false;
} {
  if (authorizeParticipant(author, allowedAuthors)) return { decision: "accept" };
  if (kind === 39607)
    return { decision: "reject", code: "unauthorized_status", advanceState: false };
  if (kind === 39608) return { decision: "reject", code: "unauthorized_cancel", cancelled: false };
  return { decision: "reject", code: "unauthorized_close", terminal: false };
}

export function missingCausalRecords(
  localIds: readonly string[],
  causalIds: readonly string[],
): readonly string[] {
  const local = new Set(localIds);
  return causalIds.filter((id) => !local.has(id));
}

export function settlementIsFinal(evidence: {
  readonly rung: string;
  readonly final: boolean;
}): boolean {
  return evidence.final && (evidence.rung === "measured" || evidence.rung === "verified");
}

export function settlementDecision(evidence: { readonly rung: string; readonly final: boolean }): {
  readonly decision: "settled" | "overclaim";
  readonly code?: "settlement_overclaim";
  readonly displayRung: string;
  readonly settled: boolean;
} {
  const settled = settlementIsFinal(evidence);
  return settled
    ? { decision: "settled", displayRung: evidence.rung, settled: true }
    : {
        decision: "overclaim",
        code: "settlement_overclaim",
        displayRung: evidence.rung,
        settled: false,
      };
}

export function evidenceMatchesClaim(
  status: { readonly evidenceId: string; readonly subjectId: string; readonly claimedRung: string },
  evidence: { readonly id: string; readonly subjectId: string; readonly rung: string },
): boolean {
  return (
    status.evidenceId === evidence.id &&
    status.subjectId === evidence.subjectId &&
    status.claimedRung === evidence.rung
  );
}

export function evidenceDecision(
  status: { readonly evidenceId: string; readonly subjectId: string; readonly claimedRung: string },
  evidence: { readonly id: string; readonly subjectId: string; readonly rung: string },
): {
  readonly decision: "match" | "mismatch";
  readonly code?: "evidence_subject_or_rung_mismatch";
  readonly displayRung: string;
  readonly advanceVerifiedState: boolean;
} {
  const matches = evidenceMatchesClaim(status, evidence);
  return matches
    ? {
        decision: "match",
        displayRung: evidence.rung,
        advanceVerifiedState: true,
      }
    : {
        decision: "mismatch",
        code: "evidence_subject_or_rung_mismatch",
        displayRung: "claimed",
        advanceVerifiedState: false,
      };
}

export function recoveryDecision(
  localIds: readonly string[],
  causalIds: readonly string[],
): {
  readonly decision: "complete" | "loss";
  readonly code?: "missing_causal_record";
  readonly missingIds: readonly string[];
  readonly synthesizeHistory: false;
} {
  const missingIds = missingCausalRecords(localIds, causalIds);
  return missingIds.length === 0
    ? { decision: "complete", missingIds, synthesizeHistory: false }
    : {
        decision: "loss",
        code: "missing_causal_record",
        missingIds,
        synthesizeHistory: false,
      };
}

export function deduplicateDeliveries<T extends { readonly event: { readonly id: string } }>(
  deliveries: readonly T[],
): readonly { readonly event: T["event"]; readonly deliveries: readonly T[] }[] {
  const grouped = new Map<string, T[]>();
  for (const delivery of deliveries) {
    const records = grouped.get(delivery.event.id) ?? [];
    records.push(delivery);
    grouped.set(delivery.event.id, records);
  }
  return [...grouped.values()].map((records) => ({
    event: records[0]!.event,
    deliveries: records,
  }));
}

export function deliveryDeduplicationDecision<
  T extends { readonly event: { readonly id: string }; readonly wrapId: string },
>(
  deliveries: readonly T[],
): {
  readonly decision: "deduplicate";
  readonly dedupKey: string;
  readonly logicalRecords: number;
  readonly retainDeliveryProvenance: readonly string[];
  readonly repeatExternalEffect: false;
} {
  const records = deduplicateDeliveries(deliveries);
  if (records.length !== 1) throw new Error("delivery decision requires one logical record");
  return {
    decision: "deduplicate",
    dedupKey: records[0]!.event.id,
    logicalRecords: 1,
    retainDeliveryProvenance: records[0]!.deliveries.map(({ wrapId }) => wrapId),
    repeatExternalEffect: false,
  };
}
