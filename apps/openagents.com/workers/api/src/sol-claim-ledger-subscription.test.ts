import {
  type SolClaim,
  type SolClaimRelease,
  type SolClaimStatus,
  type SolClaimWorkItem,
  SOL_CLAIM_ISSUE_KIND,
} from "@openagentsinc/forge-protocol";
import { Effect, Stream } from "effect";
import { getPublicKey } from "nostr-effect/pure";
import { describe, expect, it } from "vitest";

import {
  SolClaimLedgerRelayFrameError,
  solClaimReleaseToRelayPublishMessage,
  solClaimStatusToRelayPublishMessage,
  solClaimToRelayPublishMessage,
  solWorkItemToRelayPublishMessage,
} from "./sol-claim-ledger-relay";
import {
  makeInMemorySolClaimLedgerEventPersistence,
  makeSolClaimLedgerEventStore,
} from "./sol-claim-ledger-store";
import {
  type PublishedSolClaimLedgerEvent,
  makeSolClaimLedgerSubscriptionRuntime,
} from "./sol-claim-ledger-subscription";

// Deterministic test key material. Never a production key.
const secretKeyHex = "ab".repeat(32);
const secondSecretKeyHex = "cd".repeat(32);

const pubkeyOf = (hex: string): string =>
  getPublicKey(
    Uint8Array.from({ length: 32 }, (_v, i) => Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)),
  );

const repositoryCoordinate = `30617:${pubkeyOf(secretKeyHex)}:openagents`;
const otherCoordinate = `30617:${pubkeyOf(secretKeyHex)}:other-repo`;

const signOptions = (createdAtEpochSeconds: number, key = secretKeyHex) => ({
  secretKeyHex: key,
  createdAtEpochSeconds,
});

const workItem: SolClaimWorkItem = {
  work_item_ref: "sol:issue:9185",
  subject: "Move the Sol claim ledger to the owned relay",
  body: "Full Auto claim coordination should not depend on GitHub issues.",
  labels: ["forge", "stage-2"],
  repository: repositoryCoordinate,
  priority_ref: "sol:priority:p0",
};

const claim: SolClaim = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  base: "origin/main@dbc66ca96d",
  worktree: "wt-9185-sub",
  scope: "sol claim ledger live subscription runtime",
  paths: ["apps/openagents.com/workers/api/src/sol-claim-ledger-subscription.ts"],
  hot_files: [],
  hot_contracts: ["apps/openagents.com/workers/api/src/sol-claim-ledger-store.ts"],
  verification: "vitest run src/sol-claim-ledger-subscription.test.ts",
  claimed_at: "2026-07-22T01:00:00.000Z",
  legitimacy: "self_selected",
  citations: ["docs/sol/CLAIM_PROTOCOL.md", "issue #9185"],
  message: "Claiming the live subscription runtime slice.",
  repository: repositoryCoordinate,
};

const claimStatus: SolClaimStatus = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  state: "open",
  evidence_kind: "test",
  evidence: "Test Files 1 passed (1)",
  observed_at: "2026-07-22T01:30:00.000Z",
  message: "Subscription round-trip green.",
  repository: repositoryCoordinate,
};

const claimRelease: SolClaimRelease = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  outcome: "landed",
  landed_sha: "c".repeat(40),
  verification: "vitest run src/sol-claim-ledger-subscription.test.ts",
  residual: "Full Auto self-claims to the relay remain.",
  message: "Live subscription runtime landed.",
  repository: repositoryCoordinate,
};

// A second work item on a DIFFERENT work_item_ref, same coordinate.
const otherWorkItem: SolClaimWorkItem = {
  work_item_ref: "sol:issue:9999",
  subject: "Unrelated work item on the same repository coordinate",
  body: "Should be filterable out by work_item_ref.",
  labels: [],
  repository: repositoryCoordinate,
};

const newRuntime = () =>
  makeSolClaimLedgerSubscriptionRuntime(
    makeSolClaimLedgerEventStore(makeInMemorySolClaimLedgerEventPersistence()),
  );

/**
 * Collect the next `count` events a subscriber will receive. The subscription
 * is registered eagerly by `subscribe`, so events appended before this collect
 * runs are already buffered; `Stream.take` completes deterministically without
 * a wall-clock sleep.
 */
const takeFrom = (
  stream: Stream.Stream<PublishedSolClaimLedgerEvent>,
  count: number,
): Effect.Effect<ReadonlyArray<PublishedSolClaimLedgerEvent>> =>
  stream.pipe(Stream.take(count), Stream.runCollect);

describe("sol claim ledger live subscription runtime", () => {
  it("delivers a newly appended matching event to a live subscriber", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const stream = yield* runtime.subscribe(repositoryCoordinate);

          // Append AFTER subscribing; the eager registration guarantees delivery.
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
          );

          const [delivered] = yield* takeFrom(stream, 1);
          expect(delivered?.repositoryCoordinate).toBe(repositoryCoordinate);
          expect(delivered?.workItemRef).toBe("sol:issue:9185");
          expect(delivered?.pubkey).toBe(pubkeyOf(secretKeyHex));
          expect(delivered?.entry.type).toBe("claim");
          if (delivered?.entry.type !== "claim") throw new Error("unreachable");
          // Full record round-trips through sign -> store -> publish -> deliver.
          expect(delivered.entry.claim).toEqual(claim);
        }),
      ),
    );
  });

  it("delivers all four entry types on the coordinate, in append order", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const stream = yield* runtime.subscribe(repositoryCoordinate);

          yield* runtime.appendFromRelayMessage(
            solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
          );
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_100)),
          );
          yield* runtime.appendFromRelayMessage(
            solClaimStatusToRelayPublishMessage(claimStatus, signOptions(1_760_000_200)),
          );
          yield* runtime.appendFromRelayMessage(
            solClaimReleaseToRelayPublishMessage(claimRelease, signOptions(1_760_000_300)),
          );

          const delivered = yield* takeFrom(stream, 4);
          // Live stream preserves append order (unlike the newest-first query).
          expect(delivered.map((e) => e.entry.type)).toEqual([
            "work_item",
            "claim",
            "claim_status",
            "claim_release",
          ]);
        }),
      ),
    );
  });

  it("filters out events on a different repository coordinate", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const stream = yield* runtime.subscribe(repositoryCoordinate);

          // Append the OFF-coordinate event first; if it leaked it would be the
          // head of the stream. The on-coordinate event follows.
          yield* runtime.appendFromRelayMessage(
            solWorkItemToRelayPublishMessage(
              { ...otherWorkItem, repository: otherCoordinate },
              signOptions(1_760_000_000),
            ),
          );
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_100)),
          );

          const [delivered] = yield* takeFrom(stream, 1);
          expect(delivered?.repositoryCoordinate).toBe(repositoryCoordinate);
          expect(delivered?.entry.type).toBe("claim");
        }),
      ),
    );
  });

  it("narrows the live stream by kind, work_item_ref, and author", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();

          // kind-only subscriber: work items only.
          const issuesOnly = yield* runtime.subscribe(repositoryCoordinate, {
            kinds: [SOL_CLAIM_ISSUE_KIND],
          });
          // work_item_ref subscriber: only sol:issue:9999.
          const otherRefOnly = yield* runtime.subscribe(repositoryCoordinate, {
            workItemRef: "sol:issue:9999",
          });
          // author subscriber: only the second signer.
          const secondSignerOnly = yield* runtime.subscribe(repositoryCoordinate, {
            authors: [pubkeyOf(secondSecretKeyHex)],
          });

          // A claim (non-issue kind, primary signer, sol:issue:9185).
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
          );
          // Work item 9185 (issue kind, primary signer).
          yield* runtime.appendFromRelayMessage(
            solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_100)),
          );
          // Work item 9999 (issue kind, primary signer).
          yield* runtime.appendFromRelayMessage(
            solWorkItemToRelayPublishMessage(otherWorkItem, signOptions(1_760_000_200)),
          );
          // A claim from the SECOND signer.
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(
              { ...claim, actor: "agent:fleet_lane_b" },
              signOptions(1_760_000_300, secondSecretKeyHex),
            ),
          );

          // Issues-only: the two work items, in order (the two claims excluded).
          const issues = yield* takeFrom(issuesOnly, 2);
          expect(issues.map((e) => e.entry.type)).toEqual(["work_item", "work_item"]);
          expect(issues.map((e) => e.workItemRef)).toEqual(["sol:issue:9185", "sol:issue:9999"]);

          // work_item_ref 9999: only the second work item.
          const [oneRef] = yield* takeFrom(otherRefOnly, 1);
          expect(oneRef?.workItemRef).toBe("sol:issue:9999");
          expect(oneRef?.entry.type).toBe("work_item");

          // second signer: only that signer's claim.
          const [bySecond] = yield* takeFrom(secondSignerOnly, 1);
          expect(bySecond?.pubkey).toBe(pubkeyOf(secondSecretKeyHex));
          expect(bySecond?.entry.type).toBe("claim");
        }),
      ),
    );
  });

  it("does not re-deliver a deduped repeat append", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const stream = yield* runtime.subscribe(repositoryCoordinate);

          const message = solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000));
          const first = yield* runtime.appendFromRelayMessage(message);
          expect(first.stored).toBe(true);
          // Same content-addressed event again: the store dedups, so no publish.
          const repeat = yield* runtime.appendFromRelayMessage(message);
          expect(repeat.stored).toBe(false);
          // A genuinely new event, to give the stream a distinct second frame.
          yield* runtime.appendFromRelayMessage(
            solClaimStatusToRelayPublishMessage(claimStatus, signOptions(1_760_000_100)),
          );

          const delivered = yield* takeFrom(stream, 2);
          // Exactly the claim then the status — the dedup produced no delivery.
          expect(delivered.map((e) => e.entry.type)).toEqual(["claim", "claim_status"]);
          expect(delivered[0]?.eventId).toBe(first.eventId);
          expect(new Set(delivered.map((e) => e.eventId)).size).toBe(2);
        }),
      ),
    );
  });

  it("fans one event out to multiple independent subscribers", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const streamA = yield* runtime.subscribe(repositoryCoordinate);
          const streamB = yield* runtime.subscribe(repositoryCoordinate);

          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
          );

          const [a] = yield* takeFrom(streamA, 1);
          const [b] = yield* takeFrom(streamB, 1);
          expect(a?.eventId).toBe(b?.eventId);
          expect(a?.entry.type).toBe("claim");
          expect(b?.entry.type).toBe("claim");
        }),
      ),
    );
  });

  it("owns the live tail while the durable query owns the backlog", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();

          // Backlog: appended BEFORE any subscription exists.
          yield* runtime.appendFromRelayMessage(
            solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
          );

          // A subscriber joins now — it does not replay the backlog.
          const stream = yield* runtime.subscribe(repositoryCoordinate);

          // Live: appended AFTER the subscription.
          yield* runtime.appendFromRelayMessage(
            solClaimToRelayPublishMessage(claim, signOptions(1_760_000_100)),
          );

          // The durable query returns the whole backlog + live (newest first).
          const durable = yield* runtime.queryRepositoryCoordinate(repositoryCoordinate);
          expect(durable.map((e) => e.type)).toEqual(["claim", "work_item"]);
          expect(yield* runtime.count()).toBe(2);

          // The live stream carries only what was published after subscription.
          const [live] = yield* takeFrom(stream, 1);
          expect(live?.entry.type).toBe("claim");
          expect(live?.eventId).toBe(durable[0]?.eventId);
        }),
      ),
    );
  });

  it("fails a malformed relay frame with a typed frame error", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* newRuntime();
          const error = yield* Effect.flip(runtime.appendFromRelayMessage("not-json"));
          expect(error).toBeInstanceOf(SolClaimLedgerRelayFrameError);
          expect(yield* runtime.count()).toBe(0);
        }),
      ),
    );
  });
});
