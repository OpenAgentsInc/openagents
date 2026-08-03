import { schnorr } from "@noble/curves/secp256k1";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  decodeSignedWorkroomEnqueueRequest,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  signedWorkroomNostrEventId,
  SignedWorkroomStateStore,
  type SignedWorkroomActivity,
  type SignedWorkroomState,
} from "../src/index.ts";

const secretKey = Uint8Array.from([...Array(31).fill(0), 1]);
const signerPubkey = Buffer.from(schnorr.getPublicKey(secretKey)).toString("hex");
const actorRef = `principal:nostr:${signerPubkey}`;

type UnsignedActivity = Omit<SignedWorkroomActivity, "nostrEventId" | "signature">;

const activity = (
  overrides: Partial<SignedWorkroomActivity> = {},
): SignedWorkroomActivity => {
  const {
    nostrEventId: overriddenEventId,
    signature: overriddenSignature,
    ...projectionOverrides
  } = overrides;
  const unsigned = {
    eventRef: "signed-event:workroom:1",
    signerPubkey,
    actorRef,
    workroomRef: "workroom:omega:208",
    workRef: "work:github:openagentsinc-omega:216",
    kind: "thread" as const,
    audience: "workroom" as const,
    privacyClass: "workroom" as const,
    causalParentRefs: [],
    revision: 1,
    generation: 1,
    occurredAt: "2026-08-03T10:00:00Z",
    payloadDigest: "d".repeat(64),
    evidenceRefs: [],
    supersedesEventRef: null,
    revokesEventRef: null,
    ...projectionOverrides,
  } as UnsignedActivity;
  const nostrEventId = signedWorkroomNostrEventId(unsigned);
  const signature = Buffer.from(
    schnorr.sign(Uint8Array.from(Buffer.from(nostrEventId, "hex")), secretKey),
  ).toString("hex");
  return {
    ...unsigned,
    nostrEventId: overriddenEventId ?? nostrEventId,
    signature: overriddenSignature ?? signature,
  };
};

const request = (overrides: Record<string, unknown> = {}) =>
  decodeSignedWorkroomEnqueueRequest({
    idempotencyKey: "signed-workroom-fixture-1",
    expectedRevision: 0,
    effectivePrincipalRef: actorRef,
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
  it("keeps one deterministic NIP-01 projection vector", () => {
    const signed = activity();
    expect(signed.signerPubkey).toBe(
      "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    );
    expect(signed.nostrEventId).toBe(
      "5d1a030445b5deb18d67ba8a2629780bfdbc6133b11e1841f920c8557e0548ab",
    );
  });
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
  it("rejects changed projection bytes, invalid signatures, and signer actor substitution", async () => {
    const signed = activity();
    await expect(
      harness().execute(
        request({ activity: { ...signed, payloadDigest: "e".repeat(64) } }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_event_id" });
    await expect(
      harness().execute(
        request({ activity: { ...signed, signature: "0".repeat(128) } }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_signature" });
    await expect(
      harness().execute(request({ activity: activity({ actorRef: "principal:other" }) })),
    ).rejects.toMatchObject({ reason: "signer_actor_mismatch" });
  });
});
