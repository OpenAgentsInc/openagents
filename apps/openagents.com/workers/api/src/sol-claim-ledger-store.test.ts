import {
  type SolClaim,
  type SolClaimRelease,
  type SolClaimStatus,
  type SolClaimWorkItem,
  SOL_CLAIM_ISSUE_KIND,
  SOL_CLAIM_STATUS_OPEN_KIND,
} from "@openagentsinc/forge-protocol";
import { Effect } from "effect";
import { getPublicKey } from "nostr-effect/pure";
import { describe, expect, it } from "vitest";

import {
  SolClaimLedgerNotAnEntryError,
  SolClaimLedgerSignatureError,
  parseRelayEventMessage,
  signSolClaimLedgerEvent,
  solClaimReleaseToRelayPublishMessage,
  solClaimStatusToRelayPublishMessage,
  solClaimToRelayPublishMessage,
  solWorkItemToRelayPublishMessage,
} from "./sol-claim-ledger-relay";
import {
  type SolClaimLedgerEventPersistence,
  type StoredSolClaimLedgerEvent,
  SolClaimLedgerMissingCoordinateError,
  SolClaimLedgerStorageError,
  makeInMemorySolClaimLedgerEventPersistence,
  makeSolClaimLedgerEventStore,
} from "./sol-claim-ledger-store";

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
  github_mirror_ref: "gh:OpenAgentsInc/openagents#9185",
};

const claim: SolClaim = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  base: "origin/main@96b6ca4710",
  worktree: "wt-9185",
  scope: "sol claim ledger durable EventStore",
  paths: ["apps/openagents.com/workers/api/src/sol-claim-ledger-store.ts"],
  hot_files: [],
  hot_contracts: ["packages/forge-protocol/src/sol-claim-ledger.ts"],
  verification: "vitest run src/sol-claim-ledger-store.test.ts",
  claimed_at: "2026-07-22T00:00:00.000Z",
  legitimacy: "self_selected",
  citations: ["docs/sol/CLAIM_PROTOCOL.md", "issue #9185"],
  message: "Claiming the durable EventStore slice.",
  repository: repositoryCoordinate,
  github_mirror_ref: "gh:OpenAgentsInc/openagents#9185",
};

const claimStatus: SolClaimStatus = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  state: "open",
  evidence_kind: "test",
  evidence: "Test Files 1 passed (1)",
  observed_at: "2026-07-22T00:30:00.000Z",
  message: "Round-trip test green.",
  repository: repositoryCoordinate,
};

const claimRelease: SolClaimRelease = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  outcome: "landed",
  landed_sha: "c".repeat(40),
  verification: "vitest run src/sol-claim-ledger-store.test.ts",
  residual: "Live subscription runtime remains.",
  message: "Durable EventStore landed.",
  repository: repositoryCoordinate,
};

// A second work item, on the SAME coordinate but a different work_item_ref.
const otherWorkItem: SolClaimWorkItem = {
  work_item_ref: "sol:issue:9999",
  subject: "Unrelated work item on the same repository coordinate",
  body: "Should be filterable out by work_item_ref.",
  labels: [],
  repository: repositoryCoordinate,
};

const run = <A, E>(effect: Effect.Effect<A, E>): A => Effect.runSync(effect);

/** Run an effect expected to fail and return its typed error. */
const runError = <A, E>(effect: Effect.Effect<A, E>): E => Effect.runSync(Effect.flip(effect));

const newStore = () => makeSolClaimLedgerEventStore(makeInMemorySolClaimLedgerEventPersistence());

describe("sol claim ledger durable EventStore", () => {
  it("appends a signed claim event and queries it back by repository coordinate", () => {
    const store = newStore();
    const signed = parseRelayEventMessage(
      solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
    );

    const appendResult = run(store.append(signed));
    expect(appendResult.stored).toBe(true);
    expect(appendResult.eventId).toBe(signed.id);
    expect(appendResult.repositoryCoordinate).toBe(repositoryCoordinate);
    expect(appendResult.entry.type).toBe("claim");

    const entries = run(store.queryRepositoryCoordinate(repositoryCoordinate));
    expect(entries).toHaveLength(1);
    const [entry] = entries;
    expect(entry?.type).toBe("claim");
    if (entry?.type !== "claim") throw new Error("unreachable");
    // Full record round-trips through sign -> store -> query -> verify.
    expect(entry.claim).toEqual(claim);
    expect(entry.eventId).toBe(signed.id);
    expect(entry.signer).toBe(pubkeyOf(secretKeyHex));
  });

  it("round-trips all four entry types on one coordinate, newest first", () => {
    const store = newStore();
    // Append out of chronological order to prove the store sorts on read.
    run(
      store.appendFromRelayMessage(
        solClaimStatusToRelayPublishMessage(claimStatus, signOptions(1_760_000_200)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solClaimReleaseToRelayPublishMessage(claimRelease, signOptions(1_760_000_300)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solClaimToRelayPublishMessage(claim, signOptions(1_760_000_100)),
      ),
    );

    const entries = run(store.queryRepositoryCoordinate(repositoryCoordinate));
    expect(entries.map((e) => e.type)).toEqual([
      "claim_release",
      "claim_status",
      "claim",
      "work_item",
    ]);
    expect(entries.map((e) => e.createdAtEpochSeconds)).toEqual([
      1_760_000_300, 1_760_000_200, 1_760_000_100, 1_760_000_000,
    ]);
    expect(run(store.count())).toBe(4);
  });

  it("dedups a repeat append by event id", () => {
    const store = newStore();
    const message = solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000));

    const first = run(store.appendFromRelayMessage(message));
    expect(first.stored).toBe(true);
    const second = run(store.appendFromRelayMessage(message));
    expect(second.stored).toBe(false);
    // Still exactly one stored frame; the query returns a single entry.
    expect(run(store.count())).toBe(1);
    expect(run(store.queryRepositoryCoordinate(repositoryCoordinate))).toHaveLength(1);
  });

  it("isolates events by repository coordinate", () => {
    const store = newStore();
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(
          { ...otherWorkItem, repository: otherCoordinate },
          signOptions(1_760_000_050),
        ),
      ),
    );

    const here = run(store.queryRepositoryCoordinate(repositoryCoordinate));
    expect(here).toHaveLength(1);
    expect(here[0]?.type === "work_item" && here[0].workItem.work_item_ref).toBe("sol:issue:9185");

    const there = run(store.queryRepositoryCoordinate(otherCoordinate));
    expect(there).toHaveLength(1);
    expect(there[0]?.type === "work_item" && there[0].workItem.work_item_ref).toBe(
      "sol:issue:9999",
    );
  });

  it("narrows a coordinate query by kind, work_item_ref, author, since, and limit", () => {
    const store = newStore();
    // Two work items on the same coordinate; a claim from a second signer.
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(otherWorkItem, signOptions(1_760_000_100)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solClaimStatusToRelayPublishMessage(claimStatus, signOptions(1_760_000_200)),
      ),
    );
    run(
      store.appendFromRelayMessage(
        solClaimToRelayPublishMessage(
          { ...claim, actor: "agent:fleet_lane_b", repository: repositoryCoordinate },
          signOptions(1_760_000_300, secondSecretKeyHex),
        ),
      ),
    );

    // kinds
    const issuesOnly = run(
      store.queryRepositoryCoordinate(repositoryCoordinate, { kinds: [SOL_CLAIM_ISSUE_KIND] }),
    );
    expect(issuesOnly.map((e) => e.type)).toEqual(["work_item", "work_item"]);

    // work_item_ref
    const oneWorkItem = run(
      store.queryRepositoryCoordinate(repositoryCoordinate, { workItemRef: "sol:issue:9999" }),
    );
    expect(oneWorkItem).toHaveLength(1);
    expect(oneWorkItem[0]?.type === "work_item" && oneWorkItem[0].workItem.work_item_ref).toBe(
      "sol:issue:9999",
    );

    // author (second signer's claim only)
    const bySecondSigner = run(
      store.queryRepositoryCoordinate(repositoryCoordinate, {
        authors: [pubkeyOf(secondSecretKeyHex)],
      }),
    );
    expect(bySecondSigner).toHaveLength(1);
    expect(bySecondSigner[0]?.signer).toBe(pubkeyOf(secondSecretKeyHex));

    // since (drop the two oldest)
    const recent = run(
      store.queryRepositoryCoordinate(repositoryCoordinate, { sinceEpochSeconds: 1_760_000_200 }),
    );
    expect(recent.map((e) => e.createdAtEpochSeconds)).toEqual([1_760_000_300, 1_760_000_200]);

    // limit (newest two)
    const capped = run(store.queryRepositoryCoordinate(repositoryCoordinate, { limit: 2 }));
    expect(capped.map((e) => e.createdAtEpochSeconds)).toEqual([1_760_000_300, 1_760_000_200]);
  });

  it("returns an empty list for an unknown coordinate", () => {
    const store = newStore();
    run(
      store.appendFromRelayMessage(
        solWorkItemToRelayPublishMessage(workItem, signOptions(1_760_000_000)),
      ),
    );
    expect(run(store.queryRepositoryCoordinate("30617:deadbeef:missing"))).toEqual([]);
  });

  it("fails to append a well-formed entry that carries no repository coordinate", () => {
    const store = newStore();
    // A real, validly signed work-item entry, but built WITHOUT a repository so
    // the projection emits no `a` coordinate tag. Verification passes; the
    // coordinate requirement is what rejects it.
    const { repository: _drop, ...noRepoWorkItem } = workItem;
    const signed = parseRelayEventMessage(
      solWorkItemToRelayPublishMessage(noRepoWorkItem, signOptions(1_760_000_000)),
    );
    expect(signed.tags.some((tag) => tag[0] === "a")).toBe(false);
    expect(runError(store.append(signed))).toBeInstanceOf(SolClaimLedgerMissingCoordinateError);
    expect(run(store.count())).toBe(0);
  });

  it("fails to append a tampered event with a signature error and stores nothing", () => {
    const store = newStore();
    const signed = parseRelayEventMessage(
      solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
    );
    signed.content = "tampered coordination message";
    expect(runError(store.append(signed))).toBeInstanceOf(SolClaimLedgerSignatureError);
    expect(run(store.count())).toBe(0);
  });

  it("fails to append a validly signed event that is not a ledger entry", () => {
    const store = newStore();
    const signed = signSolClaimLedgerEvent(
      {
        kind: SOL_CLAIM_STATUS_OPEN_KIND,
        tags: [
          ["a", repositoryCoordinate],
          ["t", "unrelated"],
        ],
        content: "hi",
      },
      signOptions(1_760_000_000),
    );
    expect(runError(store.append(signed))).toBeInstanceOf(SolClaimLedgerNotAnEntryError);
    expect(run(store.count())).toBe(0);
  });

  it("surfaces a persistence failure as a storage error", () => {
    const failing: SolClaimLedgerEventPersistence = {
      insert: () =>
        Effect.fail(new SolClaimLedgerStorageError({ operation: "insert", messageSafe: "boom" })),
      queryByCoordinate: () => Effect.succeed([] as ReadonlyArray<StoredSolClaimLedgerEvent>),
      count: () => Effect.succeed(0),
    };
    const store = makeSolClaimLedgerEventStore(failing);
    const signed = parseRelayEventMessage(
      solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
    );
    expect(runError(store.append(signed))).toBeInstanceOf(SolClaimLedgerStorageError);
  });

  it("never persists the signer secret key in the stored frame", () => {
    const captured: Array<StoredSolClaimLedgerEvent> = [];
    const capturing: SolClaimLedgerEventPersistence = {
      insert: (row) =>
        Effect.sync(() => {
          captured.push(row);
          return true;
        }),
      queryByCoordinate: () => Effect.succeed([] as ReadonlyArray<StoredSolClaimLedgerEvent>),
      count: () => Effect.succeed(captured.length),
    };
    const store = makeSolClaimLedgerEventStore(capturing);
    run(
      store.appendFromRelayMessage(
        solClaimToRelayPublishMessage(claim, signOptions(1_760_000_000)),
      ),
    );
    expect(captured).toHaveLength(1);
    expect(captured[0]?.frame).not.toContain(secretKeyHex);
    expect(captured[0]?.frame).toContain(pubkeyOf(secretKeyHex));
  });
});
