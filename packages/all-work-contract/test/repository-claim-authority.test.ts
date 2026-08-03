import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  emptyRepositoryClaimAuthorityState,
  fileRepositoryClaimStateStoreLayer,
  importHistoricalRepositoryClaimComments,
  initializeFileRepositoryClaimState,
  inMemoryRepositoryClaimStateStoreLayer,
  REPOSITORY_CLAIM_WRITE_CAPABILITY,
  RepositoryClaimAuthority,
  RepositoryClaimAuthorityLive,
  RepositoryClaimStateStore,
} from "../src/index.ts";

const observedAt = "2026-08-03T08:00:00Z";
const owner = "principal:omega:local-owner";

const request = (
  expectedRevision: number,
  id: string,
  occurredAt: string,
  command: Record<string, unknown>,
  effectivePrincipalRef = owner,
) => ({
  requestRef: `claim-request:${id}`,
  idempotencyKey: `repository-claim-${id}`,
  expectedRevision,
  effectivePrincipalRef,
  capabilityRef: REPOSITORY_CLAIM_WRITE_CAPABILITY,
  occurredAt,
  command,
});

const packet = (
  expectedRevision: number,
  id: string,
  workRef: string,
  ownedPaths: ReadonlyArray<string>,
  hotContracts: ReadonlyArray<string> = [],
) =>
  request(expectedRevision, `packet-${id}`, observedAt, {
    command: "create_packet",
    packetRef: `work-packet:${id}`,
    workRef,
    repositoryRef: "repository:openagents",
    title: `Packet ${id}`,
    scope: `Bounded scope ${id}`,
    ownedPaths,
    hotFiles: [],
    hotContracts,
    verification: "Run the named focused verification.",
  });

const layer = () =>
  RepositoryClaimAuthorityLive.pipe(
    Layer.provide(
      inMemoryRepositoryClaimStateStoreLayer(emptyRepositoryClaimAuthorityState(observedAt)),
    ),
  );

describe("native Repository Work Claim authority", () => {
  it("creates, claims, heartbeats, blocks, reports status, and explicitly releases without GitHub writes", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      const created = yield* authority.execute(
        packet(0, "omega-224", "work:omega:224", ["crates/agent_ui"]),
      );
      const claimed = yield* authority.execute(
        request(1, "claim", "2026-08-03T08:01:00Z", {
          command: "claim_packet",
          packetRef: "work-packet:omega-224",
          claimRef: "repository-claim:omega-224",
        }),
      );
      const heartbeat = yield* authority.execute(
        request(2, "heartbeat", "2026-08-03T08:02:00Z", {
          command: "heartbeat",
          claimRef: "repository-claim:omega-224",
          expectedGeneration: 1,
          evidenceRefs: ["evidence:commit:one"],
        }),
      );
      const status = yield* authority.execute(
        request(3, "status", "2026-08-03T08:03:00Z", {
          command: "status",
          claimRef: "repository-claim:omega-224",
          expectedGeneration: 1,
          detail: "Canonical authority implementation is in progress.",
          evidenceRefs: ["evidence:status:one"],
        }),
      );
      const blocked = yield* authority.execute(
        request(4, "block", "2026-08-03T08:04:00Z", {
          command: "block",
          claimRef: "repository-claim:omega-224",
          expectedGeneration: 1,
          detail: "Waiting for the final build gate.",
          evidenceRefs: ["evidence:blocker:build-gate"],
        }),
      );
      return yield* authority.execute(
        request(5, "release", "2026-08-03T08:05:00Z", {
          command: "release",
          claimRef: "repository-claim:omega-224",
          expectedGeneration: 1,
          evidenceRefs: ["evidence:landed:one"],
        }),
      );
    }).pipe(Effect.provide(layer()));
    const result = await Effect.runPromise(journey);
    expect(result.ledger.claims[0]).toMatchObject({
      state: "released",
      holderRef: owner,
      generation: 1,
      releaseEvidenceRefs: ["evidence:landed:one"],
    });
    expect(result.ledger.audit.map((entry) => entry.kind)).toEqual([
      "packet_created",
      "claimed",
      "heartbeat",
      "status",
      "blocked",
      "released",
    ]);
    expect(result.receipt.githubWriteCount).toBe(0);
  });

  it("refuses colliding paths and hot contracts while non-conflicting packets proceed", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(
        packet(0, "one", "work:one", ["packages/all-work-contract"], ["all-work generated schema"]),
      );
      yield* authority.execute(
        request(1, "claim-one", observedAt, {
          command: "claim_packet",
          packetRef: "work-packet:one",
          claimRef: "repository-claim:one",
        }),
      );
      yield* authority.execute(
        packet(
          2,
          "two",
          "work:two",
          ["packages/all-work-contract/src"],
          ["all-work generated schema"],
        ),
      );
      const collision = yield* authority
        .execute(
          request(3, "claim-two", observedAt, {
            command: "claim_packet",
            packetRef: "work-packet:two",
            claimRef: "repository-claim:two",
          }),
        )
        .pipe(Effect.flip);
      yield* authority.execute(packet(3, "three", "work:three", ["apps/unrelated"]));
      const admitted = yield* authority.execute(
        request(4, "claim-three", observedAt, {
          command: "claim_packet",
          packetRef: "work-packet:three",
          claimRef: "repository-claim:three",
        }),
      );
      return { collision, admitted };
    }).pipe(Effect.provide(layer()));
    const { collision, admitted } = await Effect.runPromise(journey);
    expect(collision).toMatchObject({ reason: "claim_collision" });
    expect(collision.detail).toContain("repository-claim:one");
    expect(admitted.receipt.admitted).toBe(true);
  });

  it("requires 90 minutes plus an inactive process/worktree audit and fences late generations", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "stale", "work:stale", ["packages/stale"]));
      yield* authority.execute(
        request(1, "claim-stale", observedAt, {
          command: "claim_packet",
          packetRef: "work-packet:stale",
          claimRef: "repository-claim:stale",
        }),
      );
      const timeOnly = yield* authority.execute(
        request(
          2,
          "time-only",
          "2026-08-03T09:31:00Z",
          {
            command: "takeover",
            claimRef: "repository-claim:stale",
            expectedGeneration: 1,
            auditRef: "evidence:audit:active",
            auditFoundActiveWork: true,
            auditedAt: "2026-08-03T09:31:00Z",
          },
          "principal:omega:other",
        ),
      );
      const takeover = yield* authority.execute(
        request(
          3,
          "takeover",
          "2026-08-03T09:32:00Z",
          {
            command: "takeover",
            claimRef: "repository-claim:stale",
            expectedGeneration: 1,
            auditRef: "evidence:audit:inactive",
            auditFoundActiveWork: false,
            auditedAt: "2026-08-03T09:32:00Z",
          },
          "principal:omega:other",
        ),
      );
      const late = yield* authority
        .execute(
          request(
            4,
            "late-heartbeat",
            "2026-08-03T09:33:00Z",
            {
              command: "heartbeat",
              claimRef: "repository-claim:stale",
              expectedGeneration: 1,
              evidenceRefs: ["evidence:late"],
            },
            "principal:omega:other",
          ),
        )
        .pipe(Effect.flip);
      return { timeOnly, takeover, late };
    }).pipe(Effect.provide(layer()));
    const result = await Effect.runPromise(journey);
    expect(result.timeOnly.receipt).toMatchObject({
      admitted: false,
      refusalReason: "staleness_requires_90_minutes_and_inactive_audit",
    });
    expect(result.takeover.ledger.claims[0]).toMatchObject({
      holderRef: "principal:omega:other",
      generation: 2,
    });
    expect(result.late).toMatchObject({ reason: "stale_generation" });
  });

  it("replays idempotently, rejects changed replay, and preserves one generation under concurrency", async () => {
    const sharedLayer = layer();
    const setup = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "race", "work:race", ["packages/race"]));
      const claim = request(1, "race-claim", observedAt, {
        command: "claim_packet",
        packetRef: "work-packet:race",
        claimRef: "repository-claim:race",
      });
      const first = yield* authority.execute(claim);
      const replay = yield* authority.execute(claim);
      const changed = yield* authority
        .execute({ ...claim, command: { ...claim.command, claimRef: "repository-claim:changed" } })
        .pipe(Effect.flip);
      return { first, replay, changed };
    }).pipe(Effect.provide(sharedLayer));
    const result = await Effect.runPromise(setup);
    expect(result.replay.receipt).toEqual(result.first.receipt);
    expect(result.changed).toMatchObject({ reason: "idempotency_conflict" });
    expect(result.replay.ledger.claims).toHaveLength(1);
  });

  it("restarts from an atomically persisted claim ledger", async () => {
    const directory = mkdtempSync(resolve(tmpdir(), "repository-claim-authority-"));
    const initial = emptyRepositoryClaimAuthorityState(observedAt);
    await Effect.runPromise(initializeFileRepositoryClaimState(directory, initial));
    const persistedLayer = RepositoryClaimAuthorityLive.pipe(
      Layer.provide(fileRepositoryClaimStateStoreLayer(directory)),
    );
    await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* RepositoryClaimAuthority;
        yield* authority.execute(packet(0, "restart", "work:restart", ["packages/restart"]));
      }).pipe(Effect.provide(persistedLayer)),
    );
    const loaded = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* RepositoryClaimStateStore;
        return yield* store.load;
      }).pipe(Effect.provide(fileRepositoryClaimStateStoreLayer(directory))),
    );
    expect(loaded?.ledger.packets[0]?.packetRef).toBe("work-packet:restart");
    rmSync(directory, { recursive: true, force: true });
  });

  it("keeps GitHub claim comments as inert source-linked history with gap facts", () => {
    const initial = emptyRepositoryClaimAuthorityState(observedAt).ledger;
    const imported = importHistoricalRepositoryClaimComments(
      initial,
      [
        {
          sourceRef: "evidence:github-comment:5163207845",
          workRef: "work:github:openagentsinc-omega:224",
          repositoryRef: "repository:omega",
          authorRef: "principal:github:atlantispleb",
          observedAt,
          body: "CLAIM\nactor/session: historical",
        },
      ],
      false,
      "2026-08-03T08:10:00Z",
    );
    expect(imported.claims).toEqual([]);
    expect(imported.packets[0]).toMatchObject({ state: "canceled" });
    expect(imported.audit[0]).toMatchObject({
      kind: "historical_import",
      evidenceRefs: ["evidence:github-comment:5163207845"],
    });
    expect(imported.completeness).toMatchObject({ state: "gap" });
  });
});
