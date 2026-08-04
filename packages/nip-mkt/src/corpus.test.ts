import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";
import type { Event } from "nostr-effect/pure";

import {
  CLOSED_REASONS,
  GATEWAY_LIMITS,
  MKT_KIND_DEFINITIONS,
  OK_REASONS,
  OPAQUE_TRANSPORT,
  PRIVATE_MKT_KINDS,
  REASON_PREFIXES,
} from "./generated.js";
import {
  admitPrivateRecord,
  createPrivateAdmissionStore,
  deduplicateDeliveries,
  generateIdempotencyKey,
  isExpired,
} from "./state.js";

const fixture = <A>(name: string): A =>
  JSON.parse(
    readFileSync(
      fileURLToPath(new URL(`../contract/fixtures/nipmkt/${name}`, import.meta.url)),
      "utf8",
    ),
  ) as A;

const immutability = fixture<{
  readonly private_kinds: readonly number[];
  readonly outcomes: {
    readonly first: string;
    readonly identical: string;
    readonly conflicting: string;
  };
  readonly gateway_conflict_reason: string;
  readonly bounded_model: {
    readonly actions: readonly string[];
    readonly maximum_sequence_length: number;
  };
  readonly anchor_sequences: readonly {
    readonly candidates: readonly string[];
    readonly outcomes: readonly string[];
  }[];
}>("immutability.json");
const closing = fixture<{
  readonly bare_private: {
    readonly kinds: readonly number[];
    readonly expected: { readonly decision: string; readonly reason: string };
  };
  readonly classification: readonly {
    readonly first: number;
    readonly last: number;
    readonly nip01_class: string;
    readonly allocation: string;
  }[];
  readonly rates: {
    readonly discovery: readonly string[];
    readonly gift_wrap: readonly string[];
    readonly charging_order: readonly string[];
    readonly invalid_signature_charges: readonly string[];
    readonly expected_recipient_refusal: string;
  };
  readonly immutable_changed_bytes: {
    readonly coordinate: { readonly pubkey: string; readonly kind: number; readonly d: string };
    readonly stored: { readonly id: string; readonly content: string };
    readonly replay: { readonly id: string; readonly content: string };
    readonly changed: { readonly id: string; readonly content: string };
    readonly expected: { readonly replay: string; readonly changed: string };
  };
  readonly rewrapped_same_inner: {
    readonly inner_event_id: string;
    readonly wraps: readonly { readonly id: string }[];
    readonly expected: { readonly logical_records: number; readonly delivery_records: number };
  };
  readonly expiration_at_now: readonly {
    readonly now: number;
    readonly expiration: number;
    readonly expected: string;
  }[];
}>("relay-closing.json");
const gateway = fixture<{
  readonly bare_private_refusal: string;
  readonly gift_wrap_read_refusals: {
    readonly unauthenticated_connection: string;
    readonly unauthenticated_filter: string;
    readonly not_self_scoped: string;
  };
  readonly gift_wrap_recipient_rate_refusal: string;
  readonly read_surfaces: readonly string[];
  readonly legacy_private_rows_hidden: boolean;
  readonly legacy_wrap_requires_exactly_one_indexed_recipient: boolean;
  readonly invalid_signature_does_not_charge_keyed_rates: boolean;
  readonly rate_dimensions: {
    readonly discovery: readonly string[];
    readonly gift_wrap: readonly string[];
  };
  readonly recipient_rate_env: string;
}>("gateway-policy.json");

function coordinateEvent(
  coordinate: { readonly pubkey: string; readonly kind: number; readonly d: string },
  id: string,
): Event {
  return {
    id,
    pubkey: coordinate.pubkey,
    created_at: 1,
    kind: coordinate.kind,
    tags: [["d", coordinate.d]],
    content: "",
    sig: "0".repeat(128),
  };
}

describe("remaining Immortal NIP-MKT corpus", () => {
  test("replays immutable coordinate admission outcomes", () => {
    expect(immutability.private_kinds).toEqual(PRIVATE_MKT_KINDS);
    const fixtureCase = closing.immutable_changed_bytes;
    const store = createPrivateAdmissionStore();
    expect(
      admitPrivateRecord(
        store,
        coordinateEvent(fixtureCase.coordinate, fixtureCase.stored.id),
        fixtureCase.stored.content,
      ).decision,
    ).toBe("stored");
    expect(
      admitPrivateRecord(
        store,
        coordinateEvent(fixtureCase.coordinate, fixtureCase.replay.id),
        fixtureCase.replay.content,
      ).decision,
    ).toBe(fixtureCase.expected.replay);
    const conflict = admitPrivateRecord(
      store,
      coordinateEvent(fixtureCase.coordinate, fixtureCase.changed.id),
      fixtureCase.changed.content,
    );
    expect(conflict.decision).toBe(fixtureCase.expected.changed);
    expect(conflict).toMatchObject({
      code: "idempotency-conflict",
      reasonCode: "mkt_idempotency_conflict",
      reasonMessage: OK_REASONS.mkt_idempotency_conflict,
    });
  });

  test("returns the prior operation result for an idempotent replay", () => {
    const coordinate = { pubkey: "1".repeat(64), kind: 39605, d: "2".repeat(64) };
    const event = coordinateEvent(coordinate, "a".repeat(64));
    const store = createPrivateAdmissionStore<{ readonly acceptedAt: number }>();
    expect(admitPrivateRecord(store, event, "event-a", { acceptedAt: 7 })).toMatchObject({
      decision: "stored",
      result: { acceptedAt: 7 },
    });
    expect(admitPrivateRecord(store, event, "event-a")).toMatchObject({
      decision: "duplicate",
      previousResult: { acceptedAt: 7 },
    });
  });

  test("replays every bounded-model anchor sequence", () => {
    expect(immutability.outcomes).toEqual({
      first: "stored",
      identical: "duplicate",
      conflicting: "idempotency-conflict",
    });
    expect(immutability.gateway_conflict_reason).toBe(OK_REASONS.mkt_idempotency_conflict);
    expect(immutability.bounded_model).toEqual({
      actions: ["admit-a", "admit-b", "delete", "expire", "restart"],
      maximum_sequence_length: 6,
    });
    for (const sequence of immutability.anchor_sequences) {
      const store = createPrivateAdmissionStore();
      const outcomes = sequence.candidates.map((candidate) => {
        const logical = candidate === "event-a-alt-signature" ? "event-a" : candidate;
        const id = logical === "event-a" ? "a".repeat(64) : "b".repeat(64);
        const raw = candidate === "event-a-alt-signature" ? `${logical}:alt` : logical;
        return admitPrivateRecord(
          store,
          coordinateEvent({ pubkey: "1".repeat(64), kind: 39605, d: "2".repeat(64) }, id),
          raw,
        ).decision;
      });
      expect(outcomes, sequence.candidates.join(",")).toEqual(sequence.outcomes);
    }
  });

  test("deduplicates inner records while retaining outer deliveries", () => {
    const result = deduplicateDeliveries(
      closing.rewrapped_same_inner.wraps.map((wrap) => ({
        event: { id: closing.rewrapped_same_inner.inner_event_id },
        wrapId: wrap.id,
      })),
    );
    expect(result).toHaveLength(closing.rewrapped_same_inner.expected.logical_records);
    expect(result[0]?.deliveries).toHaveLength(
      closing.rewrapped_same_inner.expected.delivery_records,
    );
  });

  test("treats every expiration-at-now case as expired", () => {
    for (const fixtureCase of closing.expiration_at_now) {
      expect(isExpired(fixtureCase.expiration, fixtureCase.now)).toBe(
        fixtureCase.expected === "expired",
      );
    }
  });

  test("projects gateway reasons and outer transport from the contract", () => {
    expect(OPAQUE_TRANSPORT.outer_kind).toBe(1059);
    expect(OPAQUE_TRANSPORT.bare_private_publication).toBe("rejected");
    expect(OK_REASONS.mkt_private_requires_gift_wrap).toBe(gateway.bare_private_refusal);
    expect(gateway.gift_wrap_read_refusals.unauthenticated_filter).toBe(
      CLOSED_REASONS.gift_wrap_auth_required,
    );
    expect(gateway.gift_wrap_read_refusals.not_self_scoped).toBe(
      CLOSED_REASONS.gift_wrap_self_scope_required,
    );
    expect(gateway.gift_wrap_read_refusals.unauthenticated_connection).toMatch(
      new RegExp(`^${REASON_PREFIXES.find((prefix) => prefix === "auth-required:")}`),
    );
    expect(gateway.gift_wrap_recipient_rate_refusal).toBe(OK_REASONS.gift_wrap_recipient_rate);
    expect(gateway.read_surfaces).toEqual([
      "history",
      "ids",
      "count",
      "search_exclusion",
      "live_fanout",
    ]);
    expect(gateway.legacy_private_rows_hidden).toBe(true);
    expect(gateway.legacy_wrap_requires_exactly_one_indexed_recipient).toBe(true);
    expect(gateway.invalid_signature_does_not_charge_keyed_rates).toBe(true);
    expect(gateway.rate_dimensions).toEqual({
      discovery: closing.rates.discovery,
      gift_wrap: closing.rates.gift_wrap,
    });
    expect(gateway.recipient_rate_env).toBe(
      GATEWAY_LIMITS.find(({ name }) => name === "gift_wraps_per_minute_recipient")?.environment,
    );
    expect(closing.rates.expected_recipient_refusal).toBe(gateway.gift_wrap_recipient_rate_refusal);
    expect(closing.rates.charging_order).toEqual([
      "client_ip",
      "signature_verification",
      "event_author_or_outer_pubkey",
      "recipient_pubkey",
    ]);
    expect(closing.rates.invalid_signature_charges).toEqual(["client_ip"]);
  });

  test("pins the relay classification and bare-private policy", () => {
    expect(closing.bare_private.kinds).toEqual(PRIVATE_MKT_KINDS);
    expect(closing.bare_private.expected).toEqual({
      decision: "reject",
      reason: OK_REASONS.mkt_private_requires_gift_wrap,
    });
    expect(closing.classification).toEqual([
      { first: 39600, last: 39603, nip01_class: "addressable", allocation: "public_mkt" },
      { first: 39604, last: 39609, nip01_class: "addressable", allocation: "private_mkt" },
      {
        first: 39610,
        last: 39699,
        nip01_class: "addressable",
        allocation: "reserved_unallocated",
      },
    ]);
    expect(
      MKT_KIND_DEFINITIONS.every(({ classification }) => classification === "addressable"),
    ).toBe(true);
  });

  test("generates deterministic 32-byte idempotency keys from supplied material", () => {
    expect(generateIdempotencyKey(new Uint8Array(32).fill(0xab))).toBe("ab".repeat(32));
    expect(() => generateIdempotencyKey(new Uint8Array(31))).toThrow(
      "idempotency material must contain 32 bytes",
    );
  });
});
