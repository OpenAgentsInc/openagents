import { describe, expect, test } from "bun:test";

import {
  blueprintContractExportSeedIsPrivateDataSafe,
  blueprintPrivateFieldKey,
  isBlueprintProjectionPrivateDataSafe,
  sanitizeBlueprintProjection,
} from "./index.js";

describe("IsPrivateDataSafe predicate (single authority)", () => {
  test("safe projections pass (refs only)", () => {
    expect(
      isBlueprintProjectionPrivateDataSafe({
        id: "blueprint_contract_export.seed.v1",
        consumers: ["ai_agent", "pylon"],
        receiptRefs: ["receipt.program_run"],
      }),
    ).toBe(true);
  });

  test.each([
    { access_token: "x" },
    { accessToken: "x" },
    { refresh_token: "x" },
    { providerPayload: { anything: 1 } },
    { nested: { mnemonic: "abandon abandon" } },
    { payout_target: "bc1..." },
  ])("private-data-shaped fields fail: %j", (value) => {
    expect(isBlueprintProjectionPrivateDataSafe(value)).toBe(false);
  });

  test("string VALUES that name private fields fail", () => {
    expect(isBlueprintProjectionPrivateDataSafe("raw_email leaked")).toBe(false);
    expect(isBlueprintProjectionPrivateDataSafe("a normal ref string")).toBe(true);
  });

  test("the recursive walk is STRONGER than a flat snake_case regex (camelCase keys caught)", () => {
    // The deleted weak workers/api regex only matched snake_case substrings in
    // the stringified JSON; the canonical predicate catches camelCase keys too.
    expect(isBlueprintProjectionPrivateDataSafe({ refreshToken: "x" })).toBe(false);
  });

  test("blueprintPrivateFieldKey detects snake and camel forms", () => {
    expect(blueprintPrivateFieldKey("access_token")).toBe(true);
    expect(blueprintPrivateFieldKey("refreshToken")).toBe(true);
    expect(blueprintPrivateFieldKey("status")).toBe(false);
  });

  test("sanitize drops private-keyed fields", () => {
    const input: Record<string, string> = { id: "ok", access_token: "secret" };
    expect(sanitizeBlueprintProjection(input)).toEqual({ id: "ok" });
  });

  test("contract export seed predicate delegates to the recursive walk", () => {
    const safeSeed = {
      consumers: ["ai_agent"],
      eventCatalog: [],
      id: "blueprint_contract_export.seed.v1",
      jsonSchemas: [],
      openApi: [],
      receiptCatalog: [],
      versionRef: "blueprint_contract_export.seed.v1",
    };
    expect(blueprintContractExportSeedIsPrivateDataSafe(safeSeed as never)).toBe(true);
    expect(
      blueprintContractExportSeedIsPrivateDataSafe({ ...safeSeed, refresh_token: "leak" } as never),
    ).toBe(false);
  });
});
