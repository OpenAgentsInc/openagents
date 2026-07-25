import { describe, expect, it } from "vite-plus/test";
import {
  createSarahNostrRelayConsumer,
  handleSarahRelayOwnerMessage,
  isSarahNostrRelayPrimaryEnabled,
} from "./sarah-nostr-relay-consumer";
import {
  createMemoryRelayPublisher,
  generateSarahNostrSigner,
  testSarahNostrCipher,
} from "@openagentsinc/sarah";

describe("sarah-nostr-relay-consumer", () => {
  it("runs relay-primary path with stub agent and memory publisher", async () => {
    const signer = generateSarahNostrSigner();
    const ownerPubkey = generateSarahNostrSigner().getPublicKey();
    const memory = createMemoryRelayPublisher();
    const consumer = createSarahNostrRelayConsumer({
      signer,
      cipher: testSarahNostrCipher(),
      conversation: {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "55".repeat(12),
      },
      publish: memory.publish,
      runAgent: async ({ onToolActivity }) => {
        onToolActivity({
          entry: "tool.call",
          payload: { toolName: "ping" },
        });
        return {
          ok: true,
          text: "pong",
          usage: { totalTokens: 3, inputTokens: 1, outputTokens: 2 },
        };
      },
    });

    const result = await consumer.handleOwnerMessage({
      turnRef: "turn.api.1",
      plaintext: "ping",
    });
    expect(result.status).toBe("answered");
    expect(result.answerEvent?.kind).toBe(1059);
    expect(result.answerEvent?.content).not.toContain("pong");
    expect(memory.events.some((e) => e.kind === 14)).toBe(false);
    expect(memory.events.some((e) => e.kind === 44300)).toBe(true);
  });

  it("handleSarahRelayOwnerMessage one-shot helper", async () => {
    const signer = generateSarahNostrSigner();
    const ownerPubkey = generateSarahNostrSigner().getPublicKey();
    const result = await handleSarahRelayOwnerMessage({
      deps: {
        signer,
        cipher: testSarahNostrCipher(),
        conversation: {
          ownerPubkey,
          sarahPubkey: signer.getPublicKey(),
          conversation: "sarah." + "77".repeat(12),
        },
        publish: createMemoryRelayPublisher().publish,
        runAgent: async () => ({ ok: true, text: "ok" }),
      },
      turnRef: "turn.api.2",
      plaintext: "hi",
    });
    expect(result.status).toBe("answered");
  });

  it("rejects partial test transport injection instead of falling back", () => {
    const signer = generateSarahNostrSigner();
    expect(() =>
      createSarahNostrRelayConsumer({
        signer,
        conversation: {
          ownerPubkey: "88".repeat(32),
          sarahPubkey: signer.getPublicKey(),
          conversation: "sarah." + "99".repeat(12),
        },
        runAgent: async () => ({ ok: true, text: "no" }),
      }),
    ).toThrow(/inject signer, cipher, and publisher together/);
  });

  it("rejects missing agent configuration instead of publishing a synthetic failure", () => {
    const signer = generateSarahNostrSigner();
    expect(() =>
      createSarahNostrRelayConsumer({
        signer,
        cipher: testSarahNostrCipher(),
        publish: createMemoryRelayPublisher().publish,
        conversation: {
          ownerPubkey: generateSarahNostrSigner().getPublicKey(),
          sarahPubkey: signer.getPublicKey(),
          conversation: "sarah." + "aa".repeat(12),
        },
      }),
    ).toThrow(/missing admitted agent runner/);
  });

  it("does not mint an identity when the Secret Manager mount is missing", () => {
    const previous = process.env.SARAH_NOSTR_IDENTITY_SECRET;
    delete process.env.SARAH_NOSTR_IDENTITY_SECRET;
    try {
      expect(() =>
        createSarahNostrRelayConsumer({
          conversation: {
            ownerPubkey: generateSarahNostrSigner().getPublicKey(),
            sarahPubkey: generateSarahNostrSigner().getPublicKey(),
            conversation: "sarah." + "bb".repeat(12),
          },
          runAgent: async () => ({ ok: true, text: "must not run" }),
        }),
      ).toThrow(/missing Secret Manager mount/);
    } finally {
      if (previous !== undefined) process.env.SARAH_NOSTR_IDENTITY_SECRET = previous;
    }
  });

  it("relay primary flag defaults off", () => {
    const prev = process.env.SARAH_NOSTR_RELAY_PRIMARY;
    delete process.env.SARAH_NOSTR_RELAY_PRIMARY;
    try {
      expect(isSarahNostrRelayPrimaryEnabled()).toBe(false);
      process.env.SARAH_NOSTR_RELAY_PRIMARY = "1";
      expect(isSarahNostrRelayPrimaryEnabled()).toBe(true);
    } finally {
      if (prev === undefined) delete process.env.SARAH_NOSTR_RELAY_PRIMARY;
      else process.env.SARAH_NOSTR_RELAY_PRIMARY = prev;
    }
  });
});
