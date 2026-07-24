import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import type {
  DriftItem,
  DriftReport,
  KhalaShapedEvent,
  NostrDurableEventProjection,
} from "./types.ts";

const entryKey = (turnRef: string, seq: number): string => `${turnRef}#${seq}`;

/**
 * Compare a Khala-shaped durable ladder with a Nostr durable event projection.
 *
 * Public-safe only: compares entry kind, seq, and turnRef. Does not inspect
 * prompts, tool output, ciphertext, or credentials. Fails closed if either
 * list carries a secret-shaped field name.
 *
 * Matching rule: same (turnRef, seq) pairs with equal entry/kind.
 */
export const compareKhalaAndNostrDurableEvents = (input: {
  readonly khala: ReadonlyArray<KhalaShapedEvent>;
  readonly nostr: ReadonlyArray<NostrDurableEventProjection>;
}): DriftReport => {
  assertSarahNostrPublicSafe(input.khala);
  assertSarahNostrPublicSafe(input.nostr);

  const khalaByKey = new Map<string, KhalaShapedEvent>();
  for (const event of input.khala) {
    khalaByKey.set(entryKey(event.turnRef, event.seq), event);
  }
  const nostrByKey = new Map<string, NostrDurableEventProjection>();
  for (const event of input.nostr) {
    nostrByKey.set(entryKey(event.turnRef, event.seq), event);
  }

  const items: DriftItem[] = [];
  let matched = 0;

  const allKeys = new Set([...khalaByKey.keys(), ...nostrByKey.keys()]);
  const sortedKeys = [...allKeys].sort();

  for (const key of sortedKeys) {
    const khala = khalaByKey.get(key);
    const nostr = nostrByKey.get(key);

    if (khala !== undefined && nostr === undefined) {
      items.push({
        kind: "missing_on_nostr",
        turnRef: khala.turnRef,
        seq: khala.seq,
        khalaEntry: khala.kind,
      });
      continue;
    }
    if (nostr !== undefined && khala === undefined) {
      items.push({
        kind: "missing_on_khala",
        turnRef: nostr.turnRef,
        seq: nostr.seq,
        nostrEntry: nostr.entry,
        ...(nostr.eventId !== undefined ? { eventId: nostr.eventId } : {}),
      });
      continue;
    }
    if (khala !== undefined && nostr !== undefined) {
      if (khala.kind !== nostr.entry) {
        items.push({
          kind: "entry_mismatch",
          turnRef: khala.turnRef,
          seq: khala.seq,
          khalaEntry: khala.kind,
          nostrEntry: nostr.entry,
          ...(nostr.eventId !== undefined ? { eventId: nostr.eventId } : {}),
        });
        continue;
      }
      matched += 1;
    }
  }

  // seq_mismatch is reserved for future multi-key alignments; entry+seq key
  // already collapses that class into missing/mismatch above.

  return {
    ok: items.length === 0,
    matched,
    khalaCount: input.khala.length,
    nostrCount: input.nostr.length,
    items,
  };
};

/**
 * Project a signed kind-44300 event's public tags into a drift row.
 * Does not decrypt content. Expects tags: entry, turn (and optional id).
 */
export const projectNostrDurableEventForDrift = (event: {
  readonly id?: string;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
}): NostrDurableEventProjection | null => {
  if (event.kind !== 44300) return null;
  let entry: string | undefined;
  let turnRef: string | undefined;
  let seq: number | undefined;

  for (const tag of event.tags) {
    const name = tag[0];
    const value = tag[1];
    if (name === "entry" && value !== undefined) entry = value;
    if (name === "turn" && value !== undefined) turnRef = value;
    if (name === "seq" && value !== undefined) {
      const n = Number.parseInt(value, 10);
      if (Number.isFinite(n)) seq = n;
    }
  }

  // seq may live only in decrypted payload; when absent from tags, callers
  // should supply a projection list already flattened. Without turn/entry we
  // cannot compare.
  if (entry === undefined || turnRef === undefined || seq === undefined) {
    return null;
  }

  const allowed = new Set([
    "turn.started",
    "tool.call",
    "tool.result",
    "tool.error",
    "turn.finished",
    "turn.interrupted",
  ]);
  if (!allowed.has(entry)) return null;

  return {
    entry: entry as NostrDurableEventProjection["entry"],
    seq,
    turnRef,
    ...(event.id !== undefined ? { eventId: event.id } : {}),
  };
};
