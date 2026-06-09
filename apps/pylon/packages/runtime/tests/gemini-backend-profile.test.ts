import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  GEMINI_API_PROFILE_ID,
  GEMINI_BACKEND_KIND,
  GEMINI_DEFAULT_BASE_URL,
  GEMINI_DEFAULT_MODEL_ID,
  makeGeminiAuthHeaders,
  resolveBackendProfile,
  resolveGeminiApiKey,
  resolveGeminiBackendProfile,
} from "../src";

describe("Gemini backend profile and auth", () => {
  test("registers Gemini as a direct API backend without changing the default Apple FM profile", async () => {
    const defaultProfile = await Effect.runPromise(resolveBackendProfile());
    const gemini = await Effect.runPromise(resolveGeminiBackendProfile());

    expect(defaultProfile.kind).toBe("apple_fm_bridge");
    expect(gemini).toMatchObject({
      id: GEMINI_API_PROFILE_ID,
      kind: GEMINI_BACKEND_KIND,
      baseUrl: GEMINI_DEFAULT_BASE_URL,
      baseUrlSource: "default",
      model: GEMINI_DEFAULT_MODEL_ID,
      attachMode: "direct_api",
      auth: "api_key",
      streamMode: "sse",
    });
  });

  test("resolves explicit Gemini base URL before env and default", async () => {
    const explicit = await Effect.runPromise(
      resolveGeminiBackendProfile({
        explicitBaseUrl: "https://gemini-proxy.example/v1beta",
        env: {
          PROBE_GEMINI_BASE_URL: "https://ignored.example/v1beta",
        },
      }),
    );
    const fromEnv = await Effect.runPromise(
      resolveGeminiBackendProfile({
        env: {
          PROBE_GEMINI_BASE_URL: "https://gemini-env.example/v1beta",
        },
      }),
    );

    expect(explicit.baseUrl).toBe("https://gemini-proxy.example/v1beta");
    expect(explicit.baseUrlSource).toBe("explicit");
    expect(fromEnv.baseUrl).toBe("https://gemini-env.example/v1beta");
    expect(fromEnv.baseUrlSource).toBe("PROBE_GEMINI_BASE_URL");
  });

  test("resolves Gemini API key with Opencode-compatible precedence", async () => {
    const explicit = await Effect.runPromise(
      resolveGeminiApiKey({
        apiKey: "explicit-secret",
        env: {
          GOOGLE_GENERATIVE_AI_API_KEY: "google-secret",
          GEMINI_API_KEY: "gemini-secret",
        },
      }),
    );
    const google = await Effect.runPromise(
      resolveGeminiApiKey({
        env: {
          GOOGLE_GENERATIVE_AI_API_KEY: "google-secret",
          GEMINI_API_KEY: "gemini-secret",
        },
      }),
    );
    const gemini = await Effect.runPromise(
      resolveGeminiApiKey({
        env: {
          GEMINI_API_KEY: "gemini-secret",
        },
      }),
    );

    expect(explicit.source).toBe("explicit");
    expect(makeGeminiAuthHeaders(explicit)).toEqual({ "x-goog-api-key": "explicit-secret" });
    expect(google.source).toBe("GOOGLE_GENERATIVE_AI_API_KEY");
    expect(gemini.source).toBe("GEMINI_API_KEY");
    expect(JSON.stringify(explicit.receipt)).not.toContain("explicit-secret");
    expect(JSON.stringify(google.receipt)).not.toContain("google-secret");
    expect(JSON.stringify(gemini.receipt)).not.toContain("gemini-secret");
  });

  test("fails with a typed error when Gemini API key is missing", async () => {
    await expect(Effect.runPromise(resolveGeminiApiKey({ env: {} }))).rejects.toMatchObject({
      _tag: "GeminiAuthError",
      missingCredential: true,
    });
  });
});
