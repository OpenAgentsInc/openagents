import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";

import {
  ForgeOmegaDogfoodEvidenceError,
  validateForgeOmegaDogfoodEvidence,
  verifyForgeOmegaSource,
} from "./forge-omega-dogfood";

const prepared = () =>
  JSON.parse(
    readFileSync("docs/forge/receipts/2026-07-26-omega-dogfood.prepared.json", "utf8"),
  ) as Record<string, unknown>;

describe("Forge Omega dogfood receipt", () => {
  test("accepts the real Omega prepared record without calling it complete", () => {
    expect(validateForgeOmegaDogfoodEvidence(prepared()).status).toBe("prepared");
  });

  test("rejects a fixture in place of the real Omega repository", () => {
    const receipt = prepared();
    (receipt.repository as Record<string, unknown>).githubRepository = "example/fixture";
    expect(() => validateForgeOmegaDogfoodEvidence(receipt)).toThrow(
      ForgeOmegaDogfoodEvidenceError,
    );
  });

  test("requires every live journey stage and receipt before completion", () => {
    const receipt = prepared();
    receipt.status = "completed";
    (receipt.safeguards as Record<string, unknown>).publicCutoverApplied = true;
    expect(() => validateForgeOmegaDogfoodEvidence(receipt)).toThrow(
      "$.stages.admission.state must be completed",
    );
  });

  test("refuses an Omega migration when the declared source head changes", () => {
    expect(() => verifyForgeOmegaSource({ expectedHeadObjectId: "0".repeat(40) })).toThrow(
      "real Omega source head changed",
    );
  });
});
