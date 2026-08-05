/**
 * Deterministic fixtures and fault-injection drivers for session-store
 * tests. The crash driver is how the crash-mid-write requirement is
 * exercised: it tears a chosen write (persisting a truncated string, as a
 * real power loss or quota abort would) and refuses all subsequent writes,
 * simulating the process dying at that instant.
 */
import { Effect } from "effect";

import { StorageDriverError } from "./errors.js";
import { memoryStringKv, type StringKv } from "./kv.js";
import type { StoredSwapSession } from "./model.js";

export interface CrashPlan {
  /** 1-based index of the `set` call to tear; later calls fail outright. */
  readonly tearOnSet: number;
  /** Fraction of the value persisted by the torn write (default 0.5). */
  readonly keepFraction?: number;
}

export interface CrashingKv extends StringKv {
  readonly setCalls: () => number;
  /** The underlying storage as it stood "at the crash". */
  readonly survivingKv: () => StringKv;
  readonly snapshot: () => Record<string, string>;
}

/**
 * Wrap a memory KV so the Nth `set` persists only a prefix of its value and
 * then fails, and every later write fails — the storage then reflects
 * exactly what a crash at that write would leave behind.
 */
export const crashingStringKv = (
  plan: CrashPlan,
  initial?: Readonly<Record<string, string>>,
): CrashingKv => {
  const inner = memoryStringKv(initial);
  let sets = 0;
  let crashed = false;
  const crashError = (key: string) =>
    new StorageDriverError({ operation: "set", key, detail: "simulated crash" });
  return {
    get: inner.get,
    keys: inner.keys,
    delete: (key) =>
      crashed ? Effect.fail(crashError(key)) : inner.delete(key),
    set: (key, value) =>
      Effect.gen(function* () {
        if (crashed) return yield* crashError(key);
        sets += 1;
        if (sets === plan.tearOnSet) {
          crashed = true;
          const keep = Math.max(1, Math.floor(value.length * (plan.keepFraction ?? 0.5)));
          yield* inner.set(key, value.slice(0, keep));
          return yield* crashError(key);
        }
        yield* inner.set(key, value);
      }),
    setCalls: () => sets,
    survivingKv: () => memoryStringKv(inner.snapshot()),
    snapshot: () => inner.snapshot(),
  };
};

export const TEST_REQUESTER_PUBKEY = "aa".repeat(32);
export const TEST_PROVIDER_PUBKEY = "bb".repeat(32);

/** A minimal valid in-flight session record. */
export const sampleSession = (
  sessionId: string,
  overrides: Partial<StoredSwapSession> = {},
): StoredSwapSession => ({
  sessionId,
  createdAt: 1_754_000_000,
  updatedAt: 1_754_000_000,
  relayUrl: "wss://relay.example",
  swapType: "submarine",
  requesterPubkey: TEST_REQUESTER_PUBKEY,
  providerPubkey: TEST_PROVIDER_PUBKEY,
  offeringAddress: `39601:${TEST_PROVIDER_PUBKEY}:btc-ln`,
  projection: {
    state: "ordered",
    terminal: false,
    outcome: null,
    rung: null,
    unclaimedFunds: false,
  },
  signedRecords: [
    {
      id: "11".repeat(32),
      pubkey: TEST_REQUESTER_PUBKEY,
      created_at: 1_754_000_000,
      kind: 39_602,
      tags: [["d", sessionId]],
      content: "{}",
      sig: "cc".repeat(64),
    },
  ],
  exitPackages: [
    {
      packageDigestHex: "dd".repeat(32),
      package: { role: "requester", leg: "source", broadcastMode: "presigned" },
    },
  ],
  effectLedger: [],
  engineSnapshot: null,
  secretHandles: ["handle:refund-key:v1"],
  ...overrides,
});
