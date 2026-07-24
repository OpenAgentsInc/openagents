import { describe, expect, it } from "vite-plus/test";

import { generateSecretKeyBytes, publicKeyFromSecret } from "./crypto.ts";
import { makeNip44OwnerCipher } from "./nip44-cipher.ts";

describe("NIP-44 owner cipher", () => {
  it("round-trips plaintext under the agent-owner conversation key", () => {
    const sarahSk = generateSecretKeyBytes();
    const ownerSk = generateSecretKeyBytes();
    const ownerPk = publicKeyFromSecret(ownerSk);

    const cipher = makeNip44OwnerCipher({
      sarahSecretKey: sarahSk,
      ownerPubkeyHex: ownerPk,
    });

    const plain = JSON.stringify({
      schema: "openagents.sarah.turn_record.v1",
      entry: "turn.started",
      secretMustNotLeak: true,
    });
    const ct = cipher.encryptToOwner(plain);
    expect(ct.includes("openagents.sarah.turn_record.v1")).toBe(false);
    expect(cipher.decryptFromSarah(ct)).toBe(plain);
  });
});
