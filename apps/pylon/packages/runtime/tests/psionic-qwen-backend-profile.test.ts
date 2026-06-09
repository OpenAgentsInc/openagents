import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PSIONIC_QWEN_BACKEND_KIND,
  PSIONIC_QWEN_DEFAULT_BASE_URL,
  PSIONIC_QWEN_DEFAULT_MODEL_ID,
  PSIONIC_QWEN_LOCAL_PROFILE_ID,
  resolveBackendProfile,
  resolvePsionicQwenBackendProfile,
} from "../src";

describe("Psionic Qwen backend profile", () => {
  test("registers Psionic Qwen as an attach-only local backend without changing the default", async () => {
    const defaultProfile = await Effect.runPromise(resolveBackendProfile());
    const psionic = await Effect.runPromise(resolvePsionicQwenBackendProfile());

    expect(defaultProfile.kind).toBe("apple_fm_bridge");
    expect(psionic).toMatchObject({
      id: PSIONIC_QWEN_LOCAL_PROFILE_ID,
      kind: PSIONIC_QWEN_BACKEND_KIND,
      baseUrl: PSIONIC_QWEN_DEFAULT_BASE_URL,
      baseUrlSource: "default",
      model: PSIONIC_QWEN_DEFAULT_MODEL_ID,
      attachMode: "attach_existing",
      auth: "none",
      readinessPath: "/health",
      streamMode: "sse",
    });
  });

  test("resolves explicit, Pylon env, Probe env, then default base URL", async () => {
    const explicit = await Effect.runPromise(
      resolvePsionicQwenBackendProfile({
        explicitBaseUrl: "http://127.0.0.1:9090",
        env: {
          PYLON_PSIONIC_BASE_URL: "http://ignored-pylon:8080",
          PROBE_PSIONIC_BASE_URL: "http://ignored-probe:8080",
        },
      }),
    );
    const pylonEnv = await Effect.runPromise(
      resolvePsionicQwenBackendProfile({
        env: {
          PYLON_PSIONIC_BASE_URL: "http://pylon-env:8080",
          PROBE_PSIONIC_BASE_URL: "http://ignored-probe:8080",
        },
      }),
    );
    const probeEnv = await Effect.runPromise(
      resolvePsionicQwenBackendProfile({
        env: {
          PROBE_PSIONIC_BASE_URL: "http://probe-env:8080",
        },
      }),
    );

    expect(explicit.baseUrl).toBe("http://127.0.0.1:9090");
    expect(explicit.baseUrlSource).toBe("explicit");
    expect(pylonEnv.baseUrl).toBe("http://pylon-env:8080");
    expect(pylonEnv.baseUrlSource).toBe("PYLON_PSIONIC_BASE_URL");
    expect(probeEnv.baseUrl).toBe("http://probe-env:8080");
    expect(probeEnv.baseUrlSource).toBe("PROBE_PSIONIC_BASE_URL");
  });
});
