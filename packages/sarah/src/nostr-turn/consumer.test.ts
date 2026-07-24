import { describe, expect, it } from "vite-plus/test";

import { generateSarahNostrSigner } from "../nostr-identity/index.ts";
import { verifySignedEvent } from "../nostr-identity/crypto.ts";
import {
  SarahRelayTurnConsumer,
  createMemoryRelayPublisher,
} from "./consumer.ts";
import { testSarahNostrCipher } from "./service.ts";

describe("SarahRelayTurnConsumer (relay-primary)", () => {
  const ownerPubkey = "11".repeat(32);

  it("answers an owner message with ladder + kind 14 + usage metric", async () => {
    const signer = generateSarahNostrSigner();
    const memory = createMemoryRelayPublisher();
    const consumer = new SarahRelayTurnConsumer(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
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
    expect(result.answerEvent?.kind).toBe(14);
    expect(result.answerEvent?.content).toContain("relay-primary");
    expect(verifySignedEvent(result.answerEvent!)).toBe(true);
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
    expect(last?.tags.some((t) => t[0] === "entry" && t[1] === "turn.interrupted")).toBe(
      true,
    );
  });
});
