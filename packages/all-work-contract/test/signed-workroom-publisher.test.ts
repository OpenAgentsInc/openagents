import { schnorr } from "@noble/curves/secp256k1";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  decodeSignedWorkroomDeliveryRequest,
  decodeSignedWorkroomEnqueueRequest,
  deliverSignedWorkroomActivity,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  makeSignedWorkroomRelayPublisherLayer,
  publishSignedWorkroomOutbox,
  signedWorkroomNostrEventId,
  SignedWorkroomRelayPublisher,
  SignedWorkroomStateStore,
  type SignedWorkroomActivity,
  type SignedWorkroomRelaySocket,
  type SignedWorkroomState,
} from "../src/index.ts";

const secretKey = Uint8Array.from([...Array(31).fill(0), 1]);
const signerPubkey = Buffer.from(schnorr.getPublicKey(secretKey)).toString("hex");
const actorRef = `principal:nostr:${signerPubkey}`;
type UnsignedActivity = Omit<SignedWorkroomActivity, "nostrEventId" | "signature">;

const signedActivity = (): SignedWorkroomActivity => {
  const unsigned: UnsignedActivity = {
    projectionProfile: "openagents.signed-workroom.v2",
    eventRef: "signed-event:workroom:publisher-1",
    signerPubkey,
    actorRef,
    actorGrantRef: null,
    actorGrantGeneration: null,
    workroomRef: "workroom:omega:208",
    workRef: "work:github:openagentsinc-omega:216",
    kind: "agent_activity",
    audience: "workroom",
    privacyClass: "workroom",
    causalParentRefs: [],
    revision: 1,
    generation: 1,
    occurredAt: "2026-08-03T12:00:00Z",
    payloadDigest: "e".repeat(64),
    evidenceRefs: [],
    supersedesEventRef: null,
    revokesEventRef: null,
  };
  const nostrEventId = signedWorkroomNostrEventId(unsigned);
  return {
    ...unsigned,
    nostrEventId,
    signature: Buffer.from(
      schnorr.sign(Uint8Array.from(Buffer.from(nostrEventId, "hex")), secretKey),
    ).toString("hex"),
  };
};

type Listener = (event: Readonly<{ data?: unknown }>) => void;

class FakeRelaySocket implements SignedWorkroomRelaySocket {
  static readonly instances: Array<FakeRelaySocket> = [];
  readonly listeners = new Map<string, Array<Listener>>();
  readonly sent: Array<string> = [];

  constructor(readonly relayUrl: string) {
    FakeRelaySocket.instances.push(this);
    queueMicrotask(() => this.emit("open"));
  }

  addEventListener(type: string, listener: Listener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  send(data: string): void {
    this.sent.push(data);
    const frame = JSON.parse(data) as [string, { id: string }];
    const accepted = !this.relayUrl.includes("reject");
    queueMicrotask(() =>
      this.emit("message", {
        data: JSON.stringify(["OK", frame[1].id, accepted, "untrusted relay detail"]),
      }),
    );
  }

  close(): void {}

  private emit(type: string, event: Readonly<{ data?: unknown }> = {}): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

const publisherLayer = () =>
  makeSignedWorkroomRelayPublisherLayer({
    WebSocketImpl: FakeRelaySocket,
    now: () => new Date("2026-08-03T12:00:02Z"),
    receiptTimeoutMs: 50,
  });

const harness = () => {
  let state: SignedWorkroomState | null = emptySignedWorkroomState("2026-08-03T11:59:00Z");
  const storeLayer = Layer.succeed(
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
  const run = <A, E>(effect: Effect.Effect<A, E, SignedWorkroomStateStore>) =>
    Effect.runPromise(effect.pipe(Effect.provide(storeLayer)));
  return { run, state: () => state, storeLayer };
};

describe("signed Workroom relay publisher", () => {
  it("publishes the exact signed NIP-01 event and redacts relay rejection text", async () => {
    FakeRelaySocket.instances.length = 0;
    const value = signedActivity();
    const publisher = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* SignedWorkroomRelayPublisher;
        return yield* service.publish(value, "wss://reject.example/");
      }).pipe(Effect.provide(publisherLayer())),
    );
    expect(publisher).toMatchObject({
      outcome: "rejected",
      detail: "relay rejected signed Workroom event",
    });
    const sent = JSON.parse(FakeRelaySocket.instances[0]!.sent[0]!) as [string, object];
    expect(sent[0]).toBe("EVENT");
    expect(sent[1]).toMatchObject({ id: value.nostrEventId, sig: value.signature });
    expect(JSON.stringify(publisher)).not.toContain("untrusted relay detail");
  });

  it("checks actor capability before opening a socket", async () => {
    FakeRelaySocket.instances.length = 0;
    const value = harness();
    await value.run(
      enqueueSignedWorkroomActivity(
        decodeSignedWorkroomEnqueueRequest({
          idempotencyKey: "enqueue-publisher-1",
          expectedRevision: 0,
          effectivePrincipalRef: actorRef,
          capabilityRef: "capability:workroom-activity:enqueue",
          activity: signedActivity(),
          relayUrls: ["wss://relay.example/"],
        }),
        "2026-08-03T12:00:01Z",
      ),
    );
    await expect(
      Effect.runPromise(
        publishSignedWorkroomOutbox({
          idempotencyKey: "publish-publisher-1",
          eventRef: signedActivity().eventRef,
          effectivePrincipalRef: "principal:other",
          capabilityRef: "capability:workroom-activity:deliver",
        }).pipe(Effect.provide(value.storeLayer), Effect.provide(publisherLayer())),
      ),
    ).rejects.toMatchObject({ reason: "forbidden" });
    expect(FakeRelaySocket.instances).toHaveLength(0);
  });

  it("retries only unresolved persisted targets and reduces matching OK receipts", async () => {
    FakeRelaySocket.instances.length = 0;
    const value = harness();
    const activity = signedActivity();
    await value.run(
      enqueueSignedWorkroomActivity(
        decodeSignedWorkroomEnqueueRequest({
          idempotencyKey: "enqueue-publisher-2",
          expectedRevision: 0,
          effectivePrincipalRef: actorRef,
          capabilityRef: "capability:workroom-activity:enqueue",
          activity,
          relayUrls: ["wss://accepted.example/", "wss://pending.example/"],
        }),
        "2026-08-03T12:00:01Z",
      ),
    );
    await value.run(
      deliverSignedWorkroomActivity(
        decodeSignedWorkroomDeliveryRequest({
          idempotencyKey: "delivery-publisher-prior",
          expectedRevision: 1,
          effectivePrincipalRef: actorRef,
          capabilityRef: "capability:workroom-activity:deliver",
          eventRef: activity.eventRef,
          attempts: [
            {
              relayUrl: "wss://accepted.example/",
              outcome: "accepted",
              attemptedAt: "2026-08-03T12:00:01Z",
              detail: null,
            },
          ],
        }),
      ),
    );
    const result = await Effect.runPromise(
      publishSignedWorkroomOutbox({
        idempotencyKey: "delivery-publisher-retry",
        eventRef: activity.eventRef,
        effectivePrincipalRef: actorRef,
        capabilityRef: "capability:workroom-activity:deliver",
      }).pipe(Effect.provide(value.storeLayer), Effect.provide(publisherLayer())),
    );
    expect(FakeRelaySocket.instances.map((socket) => socket.relayUrl)).toEqual([
      "wss://pending.example/",
    ]);
    expect(result.receipt.outboxState).toBe("accepted");
    expect(result.receipt.relayAcceptanceIsAuthority).toBe(false);
    expect(result.receipt.admittedEffect).toBe(false);
  });
});
