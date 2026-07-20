import { describe, expect, test } from "vite-plus/test";

import { SIGNATURE_CATALOG, catalogClaimHolds, deriveSignatureCatalog } from "./catalog.js";
import { honestChatReplySignature } from "./signatures.js";

describe("generated signature catalog claim", () => {
  test("the checked-in catalog equals a fresh derivation from the registry", () => {
    expect(catalogClaimHolds()).toBe(true);
    expect(SIGNATURE_CATALOG).toEqual(deriveSignatureCatalog());
  });

  test("HonestChatReply.v1 is admitted and its contract digest is pinned", () => {
    const entry = SIGNATURE_CATALOG.entries.find(
      (candidate) => candidate.signatureId === honestChatReplySignature.signatureId,
    );
    expect(entry?.status).toBe("admitted");
    expect(entry?.contractDigest).toMatch(/^[a-f0-9]{64}$/);
  });

  test("a drifted contract fails the catalog claim", () => {
    const drifted = deriveSignatureCatalog([
      {
        signature: {
          signatureId: honestChatReplySignature.signatureId,
          contract: { ...honestChatReplySignature.contract, title: "Drifted title" },
        },
        status: "admitted",
      },
    ]);
    expect(catalogClaimHolds(drifted)).toBe(false);
  });
});
