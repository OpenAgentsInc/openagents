import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";

const script = readFileSync(
  new URL("../scripts/publish-ota.sh", import.meta.url),
  "utf8",
);

describe("mobile OTA publication contract", () => {
  test("canonicalizes the repository path to match Xcode fingerprinting", () => {
    expect(script).toContain('pwd -P)"');
  });

  test("fails closed when the native archive runtime does not match", () => {
    expect(script).toContain("OA_MOBILE_EXPECTED_RUNTIME");
    expect(script).toContain('"$RUNTIME" != "$EXPECTED_RUNTIME"');
    expect(script).toContain("does not match expected native runtime");
  });
});
