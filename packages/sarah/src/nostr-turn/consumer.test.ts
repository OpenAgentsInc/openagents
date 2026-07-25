import { describe, expect, it } from "vite-plus/test";
import { LocalKeySigner } from "nostr-effect/identity";

import {
  createSealedSarahNostrStack,
  generateSarahNostrSigner,
  generateSecretKeyBytes,
} from "../nostr-identity/index.ts";
import { verifySignedEvent } from "../nostr-identity/crypto.ts";
import { SarahRelayTurnConsumer, createMemoryRelayPublisher } from "./consumer.ts";
import { testSarahNostrCipher } from "./service.ts";

describe("SarahRelayTurnConsumer (relay-primary)", () => {
  const ownerPubkey = generateSarahNostrSigner().getPublicKey();

  it("answers an owner message with ladder + kind 14 + usage metric", async () => {
    const ownerSigner = LocalKeySigner.fromPrivateKey(generateSecretKeyBytes());
    const ownerPublicKey = await ownerSigner.getPublicKey();
    const stack = createSealedSarahNostrStack({
      secretKey: generateSecretKeyBytes(),
      ownerPubkeyHex: ownerPublicKey,
    });
    const signer = stack.signer;
    const memory = createMemoryRelayPublisher();
    const consumer = new SarahRelayTurnConsumer(
      signer,
      stack.cipher,
      {
        ownerPubkey: ownerPublicKey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "22".repeat(12),
      },
      async ({ onToolActivity }) => {
        onToolActivity({
          entry: "tool.call",
          payload: { toolName: "status" },
        });
        onToolActivity({
          entry: "tool.result",
          payload: { toolName: "status", ok: true },
        });
        return {
          ok: true,
          text: "Hello from relay-primary Sarah.",
          usage: { totalTokens: 10, inputTokens: 4, outputTokens: 6 },
        };
      },
      memory.publish,
    );

    const result = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.1",
      plaintext: "hi",
      promptEventId: "aa".repeat(32),
    });

    expect(result.status).toBe("answered");
    expect(result.answerEvent?.kind).toBe(1059);
    expect(result.answerEvent?.content).not.toContain("relay-primary");
    const giftWrap = result.answerEvent;
    if (giftWrap === undefined) throw new Error("missing gift wrap");
    expect(verifySignedEvent(giftWrap)).toBe(true);
    const sealJson = await ownerSigner.nip44Decrypt(giftWrap.pubkey, giftWrap.content);
    const seal = JSON.parse(sealJson) as {
      readonly pubkey: string;
      readonly content: string;
      readonly kind: number;
    };
    expect(seal.kind).toBe(13);
    const rumorJson = await ownerSigner.nip44Decrypt(seal.pubkey, seal.content);
    const rumor = JSON.parse(rumorJson) as { readonly kind: number; readonly content: string };
    expect(rumor.kind).toBe(14);
    expect(rumor.content).toBe("Hello from relay-primary Sarah.");
    ownerSigner.dispose();
    expect(result.usageMetric?.kind).toBe(44200);
    expect(result.durableEvents.some((e) => e.kind === 44300)).toBe(true);
    expect(result.liveEvents.some((e) => e.kind === 24200)).toBe(true);
    expect(memory.events.length).toBeGreaterThanOrEqual(5);

    // Duplicate claim skipped
    const again = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.1",
      plaintext: "again",
    });
    expect(again.status).toBe("skipped");
  });

  it("finishes interrupted on agent failure", async () => {
    const signer = generateSarahNostrSigner();
    const memory = createMemoryRelayPublisher();
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "33".repeat(12),
      },
      async () => ({ ok: false, detail: "inference_down" }),
      memory.publish,
    );

    const result = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.fail",
      plaintext: "hi",
    });
    expect(result.status).toBe("failed");
    expect(result.detail).toBe("inference_down");
    const last = result.durableEvents[result.durableEvents.length - 1];
    expect(last?.tags.some((t) => t[0] === "entry" && t[1] === "turn.interrupted")).toBe(true);
  });

  it("abandons an unconfirmed start so the same turn can retry", async () => {
    const signer = generateSarahNostrSigner();
    const memory = createMemoryRelayPublisher();
    let failNext = true;
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "44".repeat(12),
      },
      async () => ({ ok: true, text: "retried" }),
      async (event) => {
        if (failNext) {
          failNext = false;
          throw new Error("relay unavailable");
        }
        await memory.publish(event);
      },
    );

    const first = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.retry",
      plaintext: "retry me",
    });
    expect(first.status).toBe("service_unavailable");

    const retried = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.retry",
      plaintext: "retry me",
    });
    expect(retried.status).toBe("answered");
  });

  it("never reports an answer when a later relay receipt is unavailable", async () => {
    const signer = generateSarahNostrSigner();
    let publications = 0;
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "55".repeat(12),
      },
      async () => ({
        ok: true,
        text: "must not be reported",
        toolActivities: [{ entry: "tool.result", payload: { ok: true } }],
      }),
      async () => {
        publications += 1;
        if (publications === 3) throw new Error("relay unavailable");
      },
    );

    const result = await consumer.handleOwnerMessage({
      turnRef: "turn.relay.later-failure",
      plaintext: "run",
    });
    expect(result.status).toBe("service_unavailable");
    expect(result.answerEvent).toBeUndefined();
  });

  for (const failedPublication of [2, 3, 4] as const) {
    it(`reports service unavailable when required publication ${failedPublication} is unconfirmed`, async () => {
      const signer = generateSarahNostrSigner();
      let publications = 0;
      const consumer = new SarahRelayTurnConsumer(
        signer,
        testSarahNostrCipher(),
        {
          ownerPubkey,
          sarahPubkey: signer.getPublicKey(),
          conversation: "sarah." + "66".repeat(12),
        },
        async () => ({
          ok: true,
          text: "receipt gated",
          usage: { totalTokens: 3, inputTokens: 1, outputTokens: 2 },
        }),
        async () => {
          publications += 1;
          if (publications === failedPublication) throw new Error("receipt unavailable");
        },
      );

      const result = await consumer.handleOwnerMessage({
        turnRef: `turn.relay.publication-${failedPublication}`,
        plaintext: "run",
      });
      expect(result.status).toBe("service_unavailable");
    });
  }

  it("keeps interrupt pending until the runner emits a terminal outcome", async () => {
    const signer = generateSarahNostrSigner();
    const memory = createMemoryRelayPublisher();
    let started: (() => void) | undefined;
    const runnerStarted = new Promise<void>((resolve) => {
      started = resolve;
    });
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "77".repeat(12),
      },
      async ({ signal }) => {
        started?.();
        await new Promise<void>((resolve) => signal.addEventListener("abort", () => resolve()));
        return { ok: false, detail: "owner_interrupted" };
      },
      memory.publish,
    );

    const handling = consumer.handleOwnerMessage({
      turnRef: "turn.relay.interrupt",
      plaintext: "run",
    });
    await runnerStarted;
    await expect(consumer.interrupt("turn.relay.interrupt")).resolves.toBe("pending");
    await expect(handling).resolves.toMatchObject({
      status: "failed",
      detail: "owner_interrupted",
    });
    expect(memory.events.some((event) => event.kind === 24200)).toBe(true);
    expect(
      memory.events.some((event) =>
        event.tags.some((tag) => tag[0] === "entry" && tag[1] === "turn.interrupted"),
      ),
    ).toBe(true);
  });

  it("does not report agent failure until the interrupted terminal record is confirmed", async () => {
    const signer = generateSarahNostrSigner();
    let publications = 0;
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "88".repeat(12),
      },
      async () => ({ ok: false, detail: "inference_down" }),
      async () => {
        publications += 1;
        if (publications === 2) throw new Error("terminal receipt unavailable");
      },
    );

    await expect(
      consumer.handleOwnerMessage({
        turnRef: "turn.relay.interrupted-unconfirmed",
        plaintext: "run",
      }),
    ).resolves.toMatchObject({ status: "service_unavailable" });
  });
});
