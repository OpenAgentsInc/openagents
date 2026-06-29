import { describe, expect, test } from "bun:test";

import {
  containsProviderSecretMaterial,
  decodeOpenAiDeviceCodeResponse,
  decodeOpenAiDeviceTokenResponse,
  decodeOpenAiOAuthTokenResponse,
  isPublicSecretReference,
  providerAccountPublicMetadataJson,
  redactProviderAccountLogValue,
  requirePublicSecretReference,
  sanitizeProviderAccountText,
} from "./index.js";

describe("provider account secret policy", () => {
  test.each([
    "sk-abcdefghijklmnopqrstuvwxyz",
    "gho_abcdefghijklmnopqrstuvwxyz",
    "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
    '{"access_token":"secret"}',
    '{"refresh_token":"secret"}',
    '{"id_token":"secret"}',
    '{"code_verifier":"secret"}',
    '{"device_code":"secret"}',
    '{"device_auth_id":"secret"}',
    '{"authorization_code":"secret"}',
    '{"openai":{"type":"oauth","refresh":"fake-refresh","access":"fake-access","expires":1900000000000}}',
    "OPENAI_API_KEY=sk-testsecret000000000",
    "ANTHROPIC_API_KEY=fake-key-for-marker-test",
    "GEMINI_API_KEY=fake-key-for-marker-test",
    "AIzaFakeKeyForMarkerTest0000000",
    "CODEX_ACCESS_TOKEN=secret",
    'OPENCODE_AUTH_CONTENT={"openai":{"type":"oauth"}}',
    "/tmp/auth.json",
    "-----BEGIN PRIVATE KEY-----",
    "eyJaaaaaaaaaaa.eyJddddddddddd.ccccccccccccc",
  ])("detects credential-shaped text: %s", (value) => {
    expect(containsProviderSecretMaterial(value)).toBe(true);
  });

  test.each([
    "codex-auth://provider-account_abc",
    "secret://openagents/provider-account/abc",
    "vault://openagents/provider-account/abc",
    "gcp-secret://projects/openagents/secrets/codex/versions/latest",
    "cloud-secret://openagents/codex/abc",
    "provider-account://provider-account_abc",
    "github-write://github-write_abc",
  ])("accepts public secret refs: %s", (value) => {
    expect(isPublicSecretReference(value)).toBe(true);
    expect(requirePublicSecretReference(value) as string).toBe(value);
  });

  test.each([
    "",
    "   ",
    "https://example.com/token",
    "codex-auth://line\nbreak",
    "codex-auth://sk-abcdefghijklmnopqrstuvwxyz",
    '{"refresh_token":"secret"}',
  ])("rejects unsafe secret refs: %s", (value) => {
    expect(isPublicSecretReference(value)).toBe(false);
    expect(() => requirePublicSecretReference(value)).toThrow(/stable refs/);
  });

  test("sanitizes public display text without allowing secret material", () => {
    expect(sanitizeProviderAccountText("  Gabriel   Main  ", 120)).toBe("Gabriel Main");
    expect(sanitizeProviderAccountText("OPENAI_API_KEY=sk-testsecret000000000")).toBeUndefined();
  });

  test("rejects public metadata containing secret material", () => {
    expect(
      providerAccountPublicMetadataJson({
        accountLabel: "Main ChatGPT",
        providerAccountRef: "provider-account_123",
        status: "connected",
      }),
    ).toBe('{"accountLabel":"Main ChatGPT","providerAccountRef":"provider-account_123","status":"connected"}');
    expect(() =>
      providerAccountPublicMetadataJson({
        accountLabel: '{"refresh_token":"secret"}',
        providerAccountRef: "provider-account_123",
        status: "connected",
      }),
    ).toThrow(/secret material/);
  });

  test("redacts OpenCode auth JSON and authorization headers in logs", () => {
    const redacted = redactProviderAccountLogValue({
      authorization: "Bearer fake-header-token-0000000000",
      content:
        '{"openai":{"type":"oauth","refresh":"fake-refresh","access":"fake-access","expires":1900000000000}}',
      githubToken: "gho_abcdefghijklmnopqrstuvwxyz",
      path: "/tmp/auth.json",
    });

    expect(redacted).toContain("Bearer [REDACTED]");
    expect(redacted).toContain('\\"refresh\\":\\"[REDACTED]\\"');
    expect(redacted).toContain('\\"access\\":\\"[REDACTED]\\"');
    expect(redacted).toContain("auth.json:[REDACTED]");
    expect(redacted).not.toContain("fake-refresh");
    expect(redacted).not.toContain("fake-access");
    expect(redacted).not.toContain("fake-header-token");
    expect(redacted).not.toContain("gho_abcdefghijklmnopqrstuvwxyz");
    expect(redacted).toContain("gho_[REDACTED]");
  });
});

describe("OpenAI provider payload decoders", () => {
  test("decodes and trims device code responses", () => {
    expect(
      decodeOpenAiDeviceCodeResponse({
        device_auth_id: " device-auth-1 ",
        expires_at: " 2026-06-04T00:00:00.000Z ",
        expires_in: "900",
        interval: 5,
        user_code: " CODE-123 ",
      }),
    ).toEqual({
      device_auth_id: "device-auth-1",
      expires_at: "2026-06-04T00:00:00.000Z",
      expires_in: "900",
      interval: 5,
      user_code: "CODE-123",
    });
  });

  test("decodes device token responses", () => {
    expect(
      decodeOpenAiDeviceTokenResponse({
        authorization_code: " auth-code ",
        code_verifier: " verifier ",
      }),
    ).toEqual({
      authorization_code: "auth-code",
      code_verifier: "verifier",
    });
  });

  test("decodes OAuth token responses without accepting empty tokens", () => {
    expect(
      decodeOpenAiOAuthTokenResponse({
        access_token: " access-token ",
        expires_in: 3600,
        id_token: " id-token ",
        refresh_token: " refresh-token ",
      }),
    ).toEqual({
      access_token: "access-token",
      expires_in: 3600,
      id_token: "id-token",
      refresh_token: "refresh-token",
    });

    expect(() =>
      decodeOpenAiOAuthTokenResponse({
        access_token: "",
        refresh_token: "refresh-token",
      }),
    ).toThrow();
  });
});
