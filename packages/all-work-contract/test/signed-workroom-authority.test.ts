import { schnorr } from "@noble/curves/secp256k1";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  decodeSignedWorkroomDeliveryRequest,
  decodeSignedWorkroomCommitRequest,
  decodeSignedWorkroomEnqueueRequest,
  decodeSignedWorkroomPrepareRequest,
  commitSignedWorkroomActivity,
  deliverSignedWorkroomActivity,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  makeSignedWorkroomActorGrantResolverLayer,
  prepareSignedWorkroomActivity,
  signedWorkroomNostrEventId,
  SignedWorkroomStateStore,
  type SignedWorkroomActivity,
  type SignedWorkroomState,
} from "../src/index.ts";

const secretKey = Uint8Array.from([...Array(31).fill(0), 1]);
const signerPubkey = Buffer.from(schnorr.getPublicKey(secretKey)).toString("hex");
const actorRef = `principal:nostr:${signerPubkey}`;

type UnsignedActivity = Omit<SignedWorkroomActivity, "nostrEventId" | "signature">;

const activity = (overrides: Partial<SignedWorkroomActivity> = {}): SignedWorkroomActivity => {
  const {
    nostrEventId: overriddenEventId,
    signature: overriddenSignature,
    ...projectionOverrides
  } = overrides;
  const unsigned = {
    projectionProfile: "openagents.signed-workroom.v2" as const,
    eventRef: "signed-event:workroom:1",
    signerPubkey,
    actorRef,
    actorGrantRef: null,
    actorGrantGeneration: null,
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

const deliveryRequest = (overrides: Record<string, unknown> = {}) =>
  decodeSignedWorkroomDeliveryRequest({
    idempotencyKey: "signed-workroom-delivery-1",
    expectedRevision: 1,
    effectivePrincipalRef: actorRef,
    capabilityRef: "capability:workroom-activity:deliver",
    eventRef: "signed-event:workroom:1",
    attempts: [
      {
        relayUrl: "wss://relay.example",
        outcome: "accepted",
        attemptedAt: "2026-08-03T10:00:02Z",
        detail: null,
      },
    ],
    ...overrides,
  });

const prepareRequest = (overrides: Record<string, unknown> = {}) =>
  decodeSignedWorkroomPrepareRequest({
    idempotencyKey: "signed-workroom-prepare-1",
    effectivePrincipalRef: actorRef,
    capabilityRef: "capability:workroom-activity:prepare",
    signerPubkey,
    workroomRef: "workroom:omega:208",
    workRef: "work:github:openagentsinc-omega:216",
    kind: "thread",
    audience: "workroom",
    privacyClass: "workroom",
    causalParentRefs: [],
    occurredAt: "2026-08-03T10:00:00Z",
    payloadDigest: "d".repeat(64),
    evidenceRefs: [],
    supersedesEventRef: null,
    revokesEventRef: null,
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
  const executeGranted = (value: ReturnType<typeof request>, grants: ReadonlyArray<unknown>) =>
    Effect.runPromise(
      enqueueSignedWorkroomActivity(value, "2026-08-03T10:00:01Z").pipe(
        Effect.provide(layer),
        Effect.provide(makeSignedWorkroomActorGrantResolverLayer(grants)),
      ),
    );
  const deliver = (value = deliveryRequest()) =>
    Effect.runPromise(deliverSignedWorkroomActivity(value).pipe(Effect.provide(layer)));
  const prepare = (value = prepareRequest(), relays = ["wss://relay.example"]) =>
    Effect.runPromise(
      prepareSignedWorkroomActivity(value, "2026-08-03T10:00:01Z", relays).pipe(
        Effect.provide(layer),
      ),
    );
  const commit = (
    value: ReturnType<typeof decodeSignedWorkroomCommitRequest>,
    persistedAt = "2026-08-03T10:00:02Z",
    relays = ["wss://relay.example"],
  ) =>
    Effect.runPromise(
      commitSignedWorkroomActivity(value, persistedAt, relays).pipe(Effect.provide(layer)),
    );
  return { commit, deliver, execute, executeGranted, prepare, state: () => state };
};

describe("signed Workroom authority", () => {
  it("keeps one deterministic NIP-01 projection vector", () => {
    const signed = activity();
    expect(signed.signerPubkey).toBe(
      "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    );
    expect(signed.nostrEventId).toBe(
      "48b3f44d3931b0a71ee53f5c52ecaf9fd25a804a0d34f3874e5e760ab1d75b9e",
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
  it("prepares canonical bytes and commits only their exact enrolled signature", async () => {
    const value = harness();
    const prepared = await value.prepare();
    const template = JSON.parse(prepared.preparation.unsignedEventJson) as Record<string, unknown>;
    const eventId = signedWorkroomNostrEventId(prepared.preparation.activity);
    const signature = Buffer.from(
      schnorr.sign(Uint8Array.from(Buffer.from(eventId, "hex")), secretKey),
    ).toString("hex");
    const commit = decodeSignedWorkroomCommitRequest({
      idempotencyKey: "signed-workroom-commit-1",
      effectivePrincipalRef: actorRef,
      capabilityRef: "capability:workroom-activity:commit",
      preparation: prepared.preparation,
      signedEventJson: JSON.stringify({ id: eventId, ...template, sig: signature }),
    });
    const result = await value.commit(commit);
    expect(result.receipt).toMatchObject({
      eventRef: prepared.preparation.activity.eventRef,
      persistedBeforePublish: true,
      relayAcceptanceIsAuthority: false,
      admittedEffect: false,
    });
    expect(result.ledger.outbox[0]?.relayUrls).toEqual(["wss://relay.example"]);
    expect(result.ledger.activities[0]?.nostrEventId).toBe(eventId);
    await expect(
      harness().commit(
        decodeSignedWorkroomCommitRequest({
          ...commit,
          signedEventJson: JSON.stringify({
            id: eventId,
            ...template,
            content: "substituted",
            sig: signature,
          }),
        }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_preparation" });
  });
  it("fences preparation expiry, direct identity, and server relay policy", async () => {
    const value = harness();
    await expect(
      value.prepare(prepareRequest({ effectivePrincipalRef: "principal:other" })),
    ).rejects.toMatchObject({ reason: "forbidden" });
    await expect(value.prepare(prepareRequest(), [])).rejects.toMatchObject({
      reason: "relay_policy_unavailable",
    });
    const prepared = await value.prepare();
    const template = JSON.parse(prepared.preparation.unsignedEventJson) as Record<string, unknown>;
    const eventId = signedWorkroomNostrEventId(prepared.preparation.activity);
    const signature = Buffer.from(
      schnorr.sign(Uint8Array.from(Buffer.from(eventId, "hex")), secretKey),
    ).toString("hex");
    const commit = decodeSignedWorkroomCommitRequest({
      idempotencyKey: "signed-workroom-commit-expired",
      effectivePrincipalRef: actorRef,
      capabilityRef: "capability:workroom-activity:commit",
      preparation: prepared.preparation,
      signedEventJson: JSON.stringify({ id: eventId, ...template, sig: signature }),
    });
    await expect(value.commit(commit, "2026-08-03T10:05:01Z")).rejects.toMatchObject({
      reason: "preparation_expired",
    });
    await expect(
      value.commit(commit, "2026-08-03T10:00:02Z", ["wss://different.example"]),
    ).rejects.toMatchObject({ reason: "invalid_preparation" });
  });
  it("records exact multi-relay failures and converges after an idempotent retry", async () => {
    const value = harness();
    await value.execute(request({ relayUrls: ["wss://relay.example", "wss://relay.two"] }));
    const failed = await value.deliver(
      deliveryRequest({
        attempts: [
          {
            relayUrl: "wss://relay.example",
            outcome: "accepted",
            attemptedAt: "2026-08-03T10:00:02Z",
            detail: null,
          },
          {
            relayUrl: "wss://relay.two",
            outcome: "unreachable",
            attemptedAt: "2026-08-03T10:00:03Z",
            detail: "relay outage",
          },
        ],
      }),
    );
    expect(failed.receipt).toMatchObject({
      outboxState: "failed",
      relayAcceptanceIsAuthority: false,
      admittedEffect: false,
    });
    expect(failed.ledger.outbox[0]).toMatchObject({
      acceptedRelayUrls: ["wss://relay.example"],
      attemptCount: 2,
      lastError: "relay outage",
    });
    const retry = deliveryRequest({
      idempotencyKey: "signed-workroom-delivery-2",
      expectedRevision: 2,
      attempts: [
        {
          relayUrl: "wss://relay.two",
          outcome: "accepted",
          attemptedAt: "2026-08-03T10:00:04Z",
          detail: null,
        },
      ],
    });
    const accepted = await value.deliver(retry);
    expect(accepted.receipt.outboxState).toBe("accepted");
    expect(accepted.ledger.outbox[0]?.acceptedRelayUrls).toEqual([
      "wss://relay.example",
      "wss://relay.two",
    ]);
    expect(accepted.ledger.outbox[0]?.deliveryAttempts).toEqual([
      expect.objectContaining({ relayUrl: "wss://relay.example", outcome: "accepted" }),
      expect.objectContaining({ relayUrl: "wss://relay.two", outcome: "unreachable" }),
      expect.objectContaining({ relayUrl: "wss://relay.two", outcome: "accepted" }),
    ]);
    expect(await value.deliver(retry)).toEqual(accepted);
  });
  it("refuses unconfigured relays, stale revisions, and a different delivery principal", async () => {
    const value = harness();
    await value.execute();
    await expect(
      value.deliver(
        deliveryRequest({
          attempts: [
            {
              relayUrl: "wss://hostile.example",
              outcome: "accepted",
              attemptedAt: "2026-08-03T10:00:02Z",
              detail: null,
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_delivery" });
    await expect(value.deliver(deliveryRequest({ expectedRevision: 0 }))).rejects.toMatchObject({
      reason: "revision_conflict",
    });
    await expect(
      value.deliver(deliveryRequest({ effectivePrincipalRef: "principal:other" })),
    ).rejects.toMatchObject({ reason: "forbidden" });
  });
  it("replays one idempotent result and rejects changed bytes", async () => {
    const value = harness();
    const input = request();
    const first = await value.execute(input);
    expect(await value.execute(input)).toEqual(first);
    await expect(
      value.execute(request({ relayUrls: ["wss://other.example"] })),
    ).rejects.toMatchObject({ reason: "idempotency_conflict" });
  });
  it("fails closed on audience, causality, and principal mismatch", async () => {
    await expect(
      harness().execute(
        request({ activity: activity({ projectionProfile: "openagents.signed-workroom.v1" }) }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_projection_profile" });
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
      harness().execute(request({ activity: { ...signed, payloadDigest: "e".repeat(64) } })),
    ).rejects.toMatchObject({ reason: "invalid_event_id" });
    await expect(
      harness().execute(request({ activity: { ...signed, signature: "0".repeat(128) } })),
    ).rejects.toMatchObject({ reason: "invalid_signature" });
    await expect(
      harness().execute(request({ activity: activity({ actorRef: "principal:other" }) })),
    ).rejects.toMatchObject({ reason: "signer_actor_mismatch" });
  });
  it("admits non-human actors only through an exact current purpose-bound grant", async () => {
    const agentActivity = activity({
      actorRef: "principal:agent:omega-coder",
      actorGrantRef: "delegation-grant:omega-216:3",
      actorGrantGeneration: 3,
      evidenceRefs: ["evidence:actor-grant:omega-216:3"],
    });
    const grant = {
      grantRef: "delegation-grant:omega-216:3",
      issuerPrincipalRef: actorRef,
      actorRef: "principal:agent:omega-coder",
      signerPubkey,
      purpose: "purpose:signed-workroom:project-activity",
      workroomRef: "workroom:omega:208",
      workRef: "work:github:openagentsinc-omega:216",
      activityKinds: ["thread"],
      audiences: ["workroom"],
      privacyClasses: ["workroom"],
      generation: 3,
      validFrom: "2026-08-03T09:00:00Z",
      expiresAt: "2026-08-03T11:00:00Z",
      state: "active",
      evidenceRefs: ["evidence:actor-grant:omega-216:3"],
    } as const;
    const input = request({ activity: agentActivity });
    await expect(harness().execute(input)).rejects.toMatchObject({
      reason: "actor_grant_required",
    });
    await expect(harness().executeGranted(input, [grant])).resolves.toMatchObject({
      receipt: { persistedBeforePublish: true, admittedEffect: false },
    });
    await expect(
      harness().executeGranted(input, [{ ...grant, state: "revoked" }]),
    ).rejects.toMatchObject({ reason: "stale_actor_grant" });
    await expect(
      harness().executeGranted(input, [{ ...grant, expiresAt: "2026-08-03T09:30:00Z" }]),
    ).rejects.toMatchObject({ reason: "stale_actor_grant" });
    await expect(
      harness().executeGranted(input, [{ ...grant, activityKinds: ["decision"] }]),
    ).rejects.toMatchObject({ reason: "invalid_actor_grant" });
  });
  it("revalidates canonical state before replay or append", async () => {
    const first = harness();
    await first.execute();
    const stored = first.state();
    if (stored === null) throw new Error("missing signed Workroom state");
    const tampered: SignedWorkroomState = {
      ...stored,
      ledger: {
        ...stored.ledger,
        activities: [
          {
            ...stored.ledger.activities[0]!,
            signature: "0".repeat(128),
          },
        ],
      },
    };
    await expect(harness(tampered).execute()).rejects.toMatchObject({
      reason: "invalid_signature",
    });
  });
});
