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

type Attempt<A, E> =
  | { readonly ok: true; readonly value: A }
  | { readonly ok: false; readonly error: E };

const attempt = <A, E, R>(effect: Effect.Effect<A, E, R>): Effect.Effect<Attempt<A, E>, never, R> =>
  effect.pipe(
    Effect.map((value): Attempt<A, E> => ({ ok: true, value })),
    Effect.catch((error) => Effect.succeed<Attempt<A, E>>({ ok: false, error })),
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

  it("fences a revoked grant and a non-holder principal", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "grant", "work:grant", ["packages/grant"]));
      yield* authority.execute(
        request(1, "grant-claim", "2026-08-03T08:01:00Z", {
          command: "claim_packet",
          packetRef: "work-packet:grant",
          claimRef: "repository-claim:grant",
        }),
      );
      // A principal whose write grant was revoked no longer presents the required
      // capability, so the command is refused before it reaches the ledger.
      const revoked = yield* attempt(
        authority.execute({
          ...request(2, "grant-revoked", "2026-08-03T08:02:00Z", {
            command: "heartbeat",
            claimRef: "repository-claim:grant",
            expectedGeneration: 1,
            evidenceRefs: ["evidence:grant:revoked"],
          }),
          capabilityRef: "capability:repository-claim:revoked",
        }),
      );
      // Holding a valid grant is not holding the claim. Another principal cannot
      // heartbeat or release work it does not hold except through audited takeover.
      const foreignBeat = yield* attempt(
        authority.execute(
          request(
            2,
            "grant-foreign-beat",
            "2026-08-03T08:03:00Z",
            {
              command: "heartbeat",
              claimRef: "repository-claim:grant",
              expectedGeneration: 1,
              evidenceRefs: ["evidence:grant:foreign"],
            },
            "principal:omega:intruder",
          ),
        ),
      );
      const foreignRelease = yield* attempt(
        authority.execute(
          request(
            2,
            "grant-foreign-release",
            "2026-08-03T08:04:00Z",
            {
              command: "release",
              claimRef: "repository-claim:grant",
              expectedGeneration: 1,
              evidenceRefs: ["evidence:grant:foreign-release"],
            },
            "principal:omega:intruder",
          ),
        ),
      );
      const ledger = yield* authority.read({ repositoryRef: "repository:openagents" });
      return { revoked, foreignBeat, foreignRelease, ledger };
    }).pipe(Effect.provide(layer()));
    const { revoked, foreignBeat, foreignRelease, ledger } = await Effect.runPromise(journey);

    expect(revoked).toMatchObject({
      ok: false,
      error: { reason: "forbidden", detail: "capability" },
    });
    expect(foreignBeat).toMatchObject({ ok: false, error: { reason: "forbidden" } });
    expect(foreignRelease).toMatchObject({ ok: false, error: { reason: "forbidden" } });
    // None of the refused commands moved the ledger or the claim.
    expect(ledger.ledger.claims[0]).toMatchObject({
      state: "claimed",
      holderRef: owner,
      generation: 1,
      evidenceRefs: [],
    });
    expect(ledger.ledger.audit.map((entry) => entry.kind)).toEqual(["packet_created", "claimed"]);
  });

  it("admits exactly one generation when two clients race for the same packet", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "race2", "work:race2", ["crates/omega_work_index"]));
      const contend = (id: string, principal: string) =>
        attempt(
          authority.execute(
            request(
              1,
              id,
              "2026-08-03T08:01:00Z",
              {
                command: "claim_packet",
                packetRef: "work-packet:race2",
                claimRef: `repository-claim:${id}`,
              },
              principal,
            ),
          ),
        );
      const outcomes = yield* Effect.all(
        [
          contend("client-a", "principal:omega:client-a"),
          contend("client-b", "principal:omega:client-b"),
        ],
        { concurrency: "unbounded" },
      );
      // The loser re-reads the ledger and retries at the fresh revision. A correct
      // revision must still not manufacture a second concurrent claim generation.
      const loserIndex = outcomes.findIndex((outcome) => !outcome.ok);
      const retry = yield* attempt(
        authority.execute(
          request(
            2,
            "loser-retry",
            "2026-08-03T08:02:00Z",
            {
              command: "claim_packet",
              packetRef: "work-packet:race2",
              claimRef: "repository-claim:loser-retry",
            },
            "principal:omega:client-b",
          ),
        ),
      );
      const ledger = yield* authority.read({ repositoryRef: "repository:openagents" });
      return { outcomes, loserIndex, retry, ledger };
    }).pipe(Effect.provide(layer()));
    const { outcomes, loserIndex, retry, ledger } = await Effect.runPromise(journey);

    const admitted = outcomes.filter((outcome) => outcome.ok);
    const refused = outcomes.filter((outcome) => !outcome.ok);
    expect(admitted).toHaveLength(1);
    expect(refused).toHaveLength(1);
    expect(loserIndex).toBeGreaterThanOrEqual(0);
    // Whether the loser lost at the request revision check or at the compare-and-swap,
    // the ledger refuses it as a revision conflict rather than silently overwriting.
    expect(refused[0]).toMatchObject({ ok: false, error: { reason: "revision_conflict" } });
    expect(retry).toMatchObject({ ok: false, error: { reason: "packet_not_ready" } });

    const claims = ledger.ledger.claims;
    expect(claims).toHaveLength(1);
    expect(claims[0]?.generation).toBe(1);
    expect(claims[0]?.state).toBe("claimed");
    expect(ledger.ledger.audit.filter((entry) => entry.kind === "claimed")).toHaveLength(1);
  });

  it("lets two clients hold non-colliding packets concurrently", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "left", "work:left", ["crates/omega_work_index"]));
      yield* authority.execute(packet(1, "right", "work:right", ["packages/omega-effectd"]));
      const claim = (revision: number, id: string, principal: string) =>
        attempt(
          authority.execute(
            request(
              revision,
              `concurrent-${id}`,
              "2026-08-03T08:01:00Z",
              {
                command: "claim_packet",
                packetRef: `work-packet:${id}`,
                claimRef: `repository-claim:${id}`,
              },
              principal,
            ),
          ),
        );
      const first = yield* Effect.all(
        [claim(2, "left", "principal:omega:left"), claim(2, "right", "principal:omega:right")],
        { concurrency: "unbounded" },
      );
      // Optimistic revision serializes the writes; the deferred client retries at the
      // fresh revision and must be admitted, because the packets do not collide.
      const pending = first.findIndex((outcome) => !outcome.ok);
      const retried =
        pending === -1
          ? null
          : yield* attempt(
              authority.execute(
                request(
                  3,
                  `retry-${pending === 0 ? "left" : "right"}`,
                  "2026-08-03T08:02:00Z",
                  {
                    command: "claim_packet",
                    packetRef: `work-packet:${pending === 0 ? "left" : "right"}`,
                    claimRef: `repository-claim:${pending === 0 ? "left" : "right"}`,
                  },
                  pending === 0 ? "principal:omega:left" : "principal:omega:right",
                ),
              ),
            );
      const ledger = yield* authority.read({ repositoryRef: "repository:openagents" });
      return { first, retried, ledger };
    }).pipe(Effect.provide(layer()));
    const { retried, ledger } = await Effect.runPromise(journey);

    if (retried !== null) {
      expect(retried.ok).toBe(true);
    }
    const claims = ledger.ledger.claims;
    expect(claims).toHaveLength(2);
    expect(claims.every((claim) => claim.state === "claimed" && claim.generation === 1)).toBe(true);
    expect(new Set(claims.map((claim) => claim.holderRef)).size).toBe(2);
  });

  it("refuses a reordered command and keeps the event cursor monotonic", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* RepositoryClaimAuthority;
      yield* authority.execute(packet(0, "order", "work:order", ["packages/order"]));
      const claimed = yield* authority.execute(
        request(1, "order-claim", "2026-08-03T08:01:00Z", {
          command: "claim_packet",
          packetRef: "work-packet:order",
          claimRef: "repository-claim:order",
        }),
      );
      const beat = yield* authority.execute(
        request(2, "order-beat", "2026-08-03T08:02:00Z", {
          command: "heartbeat",
          claimRef: "repository-claim:order",
          expectedGeneration: 1,
          evidenceRefs: ["evidence:order:one"],
        }),
      );
      // A command that was built against an older revision arrives late.
      const reordered = yield* attempt(
        authority.execute(
          request(1, "order-late", "2026-08-03T08:03:00Z", {
            command: "status",
            claimRef: "repository-claim:order",
            expectedGeneration: 1,
            detail: "Late status built against a superseded revision.",
            evidenceRefs: ["evidence:order:late"],
          }),
        ),
      );
      // A cursor gap (a revision ahead of the ledger) is refused, not buffered.
      const gap = yield* attempt(
        authority.execute(
          request(9, "order-gap", "2026-08-03T08:04:00Z", {
            command: "status",
            claimRef: "repository-claim:order",
            expectedGeneration: 1,
            detail: "Status built against a revision the ledger never reached.",
            evidenceRefs: ["evidence:order:gap"],
          }),
        ),
      );
      return { claimed, beat, reordered, gap };
    }).pipe(Effect.provide(layer()));
    const { claimed, beat, reordered, gap } = await Effect.runPromise(journey);

    expect(reordered).toMatchObject({ ok: false, error: { reason: "revision_conflict" } });
    expect(gap).toMatchObject({ ok: false, error: { reason: "revision_conflict" } });
    expect(beat.receipt.revision).toBeGreaterThan(claimed.receipt.revision);
    expect(beat.receipt.previousRevision).toBe(claimed.receipt.revision);
    expect(claimed.receipt.eventCursor).toBe(`cursor:repository-claim:${claimed.receipt.revision}`);
    expect(beat.receipt.eventCursor).toBe(`cursor:repository-claim:${beat.receipt.revision}`);
  });

  it("completes claim, status, and release while every network call fails", async () => {
    const realFetch = globalThis.fetch;
    let networkAttempts = 0;
    // Any attempt to reach GitHub — or any other host — fails hard for the whole
    // journey. A native claim/status/release that still completes proves the
    // authority has no GitHub dependency on its critical path.
    globalThis.fetch = (() => {
      networkAttempts += 1;
      throw new Error("GITHUB OUTAGE: network is unavailable");
    }) as typeof globalThis.fetch;
    try {
      // Positive control: the sabotage is real, so `networkAttempts === 0` below
      // is evidence of no network dependency rather than a no-op assertion.
      expect(() => globalThis.fetch("https://api.github.com/")).toThrow("GITHUB OUTAGE");
      networkAttempts = 0;
      const journey = Effect.gen(function* () {
        const authority = yield* RepositoryClaimAuthority;
        yield* authority.execute(packet(0, "outage", "work:outage", ["packages/outage"]));
        yield* authority.execute(
          request(1, "outage-claim", "2026-08-03T08:01:00Z", {
            command: "claim_packet",
            packetRef: "work-packet:outage",
            claimRef: "repository-claim:outage",
          }),
        );
        yield* authority.execute(
          request(2, "outage-status", "2026-08-03T08:02:00Z", {
            command: "status",
            claimRef: "repository-claim:outage",
            expectedGeneration: 1,
            detail: "Working while GitHub is unreachable.",
            evidenceRefs: ["evidence:outage:status"],
          }),
        );
        const released = yield* authority.execute(
          request(3, "outage-release", "2026-08-03T08:03:00Z", {
            command: "release",
            claimRef: "repository-claim:outage",
            expectedGeneration: 1,
            evidenceRefs: ["evidence:outage:landed"],
          }),
        );
        const ledger = yield* authority.read({ repositoryRef: "repository:openagents" });
        return { released, ledger };
      }).pipe(Effect.provide(layer()));
      const { released, ledger } = await Effect.runPromise(journey);

      expect(released.receipt.admitted).toBe(true);
      expect(released.receipt.githubWriteCount).toBe(0);
      expect(ledger.ledger.claims[0]).toMatchObject({
        state: "released",
        releaseEvidenceRefs: ["evidence:outage:landed"],
      });
      expect(networkAttempts).toBe(0);
    } finally {
      globalThis.fetch = realFetch;
    }
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
