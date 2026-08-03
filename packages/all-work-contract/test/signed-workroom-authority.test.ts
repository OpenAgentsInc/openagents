import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  decodeSignedWorkroomEnqueueRequest,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  SignedWorkroomStateStore,
  type SignedWorkroomState,
} from "../src/index.ts";

const activity = (overrides: Record<string, unknown> = {}) => ({
  eventRef: "signed-event:workroom:1",
  nostrEventId: "a".repeat(64),
  signerPubkey: "b".repeat(64),
  signature: "c".repeat(128),
  actorRef: "principal:omega:owner",
  workroomRef: "workroom:omega:208",
  workRef: "work:github:openagentsinc/omega:216",
  kind: "thread",
  audience: "workroom",
  privacyClass: "workroom",
  causalParentRefs: [],
  revision: 1,
  generation: 1,
  occurredAt: "2026-08-03T10:00:00Z",
  payloadDigest: "d".repeat(64),
  evidenceRefs: [],
  supersedesEventRef: null,
  revokesEventRef: null,
  ...overrides,
});

const request = (overrides: Record<string, unknown> = {}) =>
  decodeSignedWorkroomEnqueueRequest({
    idempotencyKey: "signed-workroom-fixture-1",
    expectedRevision: 0,
    effectivePrincipalRef: "principal:omega:owner",
    capabilityRef: "capability:workroom-activity:enqueue",
    activity: activity(),
    relayUrls: ["wss://relay.example"],
    ...overrides,
  });

const harness = (initial = emptySignedWorkroomState("2026-08-03T09:59:00Z")) => {
  let state: SignedWorkroomState | null = initial;
  const layer = Layer.succeed(
    SignedWorkroomStateStore,
    SignedWorkroomStateStore.of({
      load: Effect.sync(() => state),
      save: (expectedRevision, next) =>
        Effect.sync(() => {
          expect(state?.ledger.revision).toBe(expectedRevision);
          state = next;
        }),
    }),
  );
  const execute = (value = request()) =>
    Effect.runPromise(
      enqueueSignedWorkroomActivity(value, "2026-08-03T10:00:01Z").pipe(Effect.provide(layer)),
    );
  return { execute, state: () => state };
};

describe("signed Workroom authority", () => {
  it("persists exact signed bytes before publish", async () => {
    const value = harness();
    const result = await value.execute();
    expect(result.receipt.persistedBeforePublish).toBe(true);
    expect(result.receipt.relayAcceptanceIsAuthority).toBe(false);
    expect(result.receipt.admittedEffect).toBe(false);
    expect(value.state()?.ledger.outbox[0]?.state).toBe("pending");
  });
  it("replays one idempotent result and rejects changed bytes", async () => {
    const value = harness();
    const first = await value.execute();
    expect(await value.execute()).toEqual(first);
    await expect(
      value.execute(request({ relayUrls: ["wss://other.example"] })),
    ).rejects.toMatchObject({ reason: "idempotency_conflict" });
  });
  it("fails closed on audience, causality, and principal mismatch", async () => {
    await expect(
      harness().execute(request({ activity: activity({ privacyClass: "owner_only" }) })),
    ).rejects.toMatchObject({ reason: "invalid_audience" });
    await expect(
      harness().execute(
        request({ activity: activity({ causalParentRefs: ["signed-event:missing"] }) }),
      ),
    ).rejects.toMatchObject({ reason: "causal_gap" });
    await expect(
      harness().execute(request({ effectivePrincipalRef: "principal:other" })),
    ).rejects.toMatchObject({ reason: "forbidden" });
  });
});
