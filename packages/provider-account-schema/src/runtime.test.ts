import { Effect } from "effect";
import { describe, expect, test } from "bun:test";

import { ProviderSecretRef as RootProviderSecretRef } from "./index.js";
import {
  ProviderSecretRef as RuntimeProviderSecretRef,
  canIssueProviderAccountGrant,
  containsSecretMaterial,
  isPublicSecretRef,
  requirePublicSecretRef,
  sanitizeProbePublicProjection,
  validateProbePublicProjection,
} from "./runtime.js";

describe("provider-account runtime secret policy (Probe/Pylon surface)", () => {
  test("the runtime ProviderSecretRef brand is the same authority as the root", () => {
    // Single brand authority: runtime re-uses the brand defined in ./index.
    expect(RuntimeProviderSecretRef).toBe(RootProviderSecretRef);
  });

  test.each([
    "secret://openagents/provider-account/abc",
    "vault://openagents/provider-account/abc",
    "gcp-secret://projects/openagents/secrets/codex/versions/latest",
    "cloud-secret://openagents/codex/abc",
    "provider-account://provider-account_abc",
    "codex-auth://provider-account_abc",
  ])("accepts public secret refs: %s", (value) => {
    expect(isPublicSecretRef(value)).toBe(true);
  });

  test.each([
    "sk-abcdefghijklmnopqrstuvwxyz0000",
    "AIzaabcdefghijklmnopqrstuvwxyz0000",
    '{"refresh_token":"secret"}',
    '{"access_token":"secret"}',
    'OPENCODE_AUTH_CONTENT={"openai":{"type":"oauth"}}',
    "/tmp/auth.json",
  ])("detects raw credential material: %s", (value) => {
    expect(containsSecretMaterial(value)).toBe(true);
  });

  test("requirePublicSecretRef fails closed on raw material", () => {
    const result = Effect.runSyncExit(requirePublicSecretRef("sk-aaaaaaaaaaaaaaaaaaaaaaaa"));
    expect(result._tag).toBe("Failure");
  });

  test("rejects unsafe public projections (secret-shaped key)", () => {
    const exit = Effect.runSyncExit(
      validateProbePublicProjection({ refresh_token: "anything" }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("accepts a safe public projection", () => {
    const exit = Effect.runSyncExit(
      validateProbePublicProjection({ providerAccountRef: "provider-account_1", status: "connected" }),
    );
    expect(exit._tag).toBe("Success");
  });

  test("sanitize redacts secret-keyed fields", () => {
    expect(sanitizeProbePublicProjection({ access_token: "x", label: "ok" })).toEqual({
      access_token: "[redacted]",
      label: "ok",
    });
  });

  test("grant gating requires connected+healthy+public-ref", () => {
    expect(
      canIssueProviderAccountGrant({
        provider: "chatgpt_codex",
        providerAccountRef: "provider-account_1" as never,
        authMode: "chatgpt_device_code",
        status: "connected",
        health: "healthy",
        secretRef: "codex-auth://provider-account_1" as never,
      }),
    ).toBe(true);
    expect(
      canIssueProviderAccountGrant({
        provider: "chatgpt_codex",
        providerAccountRef: "provider-account_1" as never,
        authMode: "chatgpt_device_code",
        status: "expired",
        health: "healthy",
        secretRef: "codex-auth://provider-account_1" as never,
      }),
    ).toBe(false);
  });
});
