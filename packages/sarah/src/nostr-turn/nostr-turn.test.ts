import { describe, expect, it } from "vite-plus/test";

import { generateSarahNostrSigner } from "../nostr-identity/index.ts";
import { verifySignedEvent } from "../nostr-identity/crypto.ts";
import {
  SARAH_TURN_RECORD_KIND,
  SarahNostrTurnService,
  SarahTurnClaimStore,
  testSarahNostrCipher,
} from "./index.ts";

describe("SARAH-NR-05 turn ladder publish path", () => {
  it("claims once, publishes durable started/tool/finished, and dual-publishes live AO", () => {
    const signer = generateSarahNostrSigner();
    const ownerPubkey = "01".repeat(32);
    const service = new SarahNostrTurnService(
      signer,
      testSarahNostrCipher(),
      {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversation: "sarah." + "ab".repeat(12),
      },
    );

    const started = service.startTurn({
      turnRef: "turn.test.1",
      parents: [
        {
          eventId: "aa".repeat(32),
          marker: "prompt",
        },
      ],
    });
    expect(started).not.toBeNull();
    expect(started!.durable!.kind).toBe(SARAH_TURN_RECORD_KIND);
    expect(verifySignedEvent(started!.durable!)).toBe(true);
    expect(started!.durable!.content.startsWith("nip44:v2:")).toBe(true);
    expect(started!.durable!.tags.some((t) => t[0] === "entry" && t[1] === "turn.started")).toBe(
      true,
    );

    // Second claim fails
    expect(service.startTurn({ turnRef: "turn.test.1" })).toBeNull();

    const tool = service.publishToolActivity({
      turnRef: "turn.test.1",
      entry: "tool.call",
      payload: { toolRef: "tool.fixture", name: "status" },
    });
    expect(tool.live).toBeDefined();
    expect(tool.live!.kind).toBe(24200);
    expect(verifySignedEvent(tool.live!)).toBe(true);

    const finished = service.finishTurn({
      turnRef: "turn.test.1",
      entry: "turn.finished",
      payload: { reason: "completed" },
    });
    expect(finished.entry).toBe("turn.finished");
    expect(finished.seq).toBe(3);

    // After finish, further activity requires a new claim
    expect(() =>
      service.publishToolActivity({
        turnRef: "turn.test.1",
        entry: "tool.result",
        payload: {},
      }),
    ).toThrow(/not claimed/);
  });

  it("claim store enforces exactly one holder and unreclaimable terminal", () => {
    const store = new SarahTurnClaimStore();
    expect(store.tryClaim({ turnRef: "t1", conversation: "sarah." + "00".repeat(12) })).not.toBeNull();
    expect(store.tryClaim({ turnRef: "t1", conversation: "sarah." + "00".repeat(12) })).toBeNull();
    store.complete("t1");
    // Terminal turns must not be re-claimed (exactly one answer).
    expect(store.tryClaim({ turnRef: "t1", conversation: "sarah." + "00".repeat(12) })).toBeNull();
    expect(store.tryClaim({ turnRef: "t2", conversation: "sarah." + "00".repeat(12) })).not.toBeNull();
  });
});
