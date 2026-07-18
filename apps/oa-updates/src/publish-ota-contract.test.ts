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

  test("uses Expo's native build-time fingerprint implementation", () => {
    expect(script).toContain("createUpdatesResources.js");
    expect(script).toContain("only-fingerprint");
    expect(script).not.toContain("pnpm exec expo-updates fingerprint:generate");
  });

  test("fails closed when the native archive runtime does not match", () => {
    expect(script).toContain("OA_MOBILE_EXPECTED_RUNTIME");
    expect(script).toContain('"$RUNTIME" != "$EXPECTED_RUNTIME"');
    expect(script).toContain("does not match expected native runtime");
    expect(script).toContain("OA_MOBILE_FINGERPRINT_ONLY");
  });

  test("builds a valid Cloud Run candidate tag URL", () => {
    expect(script).toContain(
      'CANDIDATE_URL="https://${CANDIDATE_TAG}---${SERVICE_URL#https://}"',
    );
    expect(script).not.toContain("SERVICE_URL/https:");
  });
});
