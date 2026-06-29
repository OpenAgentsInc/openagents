import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  DEFAULT_GRANT_RESOLVE_BASE_URL,
  decodeProbeRunAssignment,
  GOOGLE_GEMINI_PROVIDER,
  LEGACY_OMEGA_BASE_URL_ENV,
  makeOmegaGrantResolver,
  makeOmegaGrantResolverFromEnv,
  makeStaticOmegaGrantResolver,
  OA_CODEX_GRANT_RESOLVE_URL_ENV,
  resolveCodexGrantResolveEndpoint,
  validateResolvedAuthGrantForAssignment,
  type OmegaResolvedAuthGrant,
  type ProbeRunAssignment,
} from "../src";

const assignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_1",
  runnerSessionId: "runner_session_1",
  goal: "Fix the failing test",
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: "provider-account_primary" as ProbeRunAssignment["providerAccountRef"],
  authGrantRef: "provider-auth-grant_1" as ProbeRunAssignment["authGrantRef"],
  repo: {
    url: "https://github.com/OpenAgentsInc/probe.git",
    branch: "main",
  },
});

const fakeRawGeminiApiKey = () => ["AI", "zaSyDUMMYRAWGEMINIKEYMATERIAL123456789"].join("");

const geminiAssignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_gemini_1",
  runnerSessionId: "runner_session_1",
  goal: "Run Gemini",
  provider: GOOGLE_GEMINI_PROVIDER,
  providerAccountRef: "provider-account_google_gemini_primary" as ProbeRunAssignment["providerAccountRef"],
  authGrantRef: "provider-auth-grant_google_gemini_1" as ProbeRunAssignment["authGrantRef"],
  backend: {
    kind: "gemini_api",
    backendProfileId: "gemini-api",
  },
});

const grant = (overrides: Partial<OmegaResolvedAuthGrant> = {}): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: "provider-account_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  requestedAction: "coding-agent-run",
  runnerSessionId: "runner_session_1",
  expiresAt: "2099-01-01T00:00:00.000Z",
  status: "used",
  materialization: {
    kind: "probe_chatgpt_auth",
    provider: CHATGPT_CODEX_PROVIDER,
    providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
    target: {
      kind: "env",
      name: "PROBE_CHATGPT_AUTH_CONTENT",
    },
    homeIsolation: "per_run",
    scrubAfterCloseout: true,
  },
  ...overrides,
});

const geminiGrant = (overrides: Partial<OmegaResolvedAuthGrant> = {}): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_google_gemini_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: GOOGLE_GEMINI_PROVIDER,
  providerAccountRef: "provider-account_google_gemini_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "cloud-secret://openagents/google-gemini/primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  requestedAction: "gemini-backend-run",
  runnerSessionId: "runner_session_1",
  expiresAt: "2099-01-01T00:00:00.000Z",
  status: "used",
  materialization: {
    kind: "probe_gemini_api_key",
    provider: GOOGLE_GEMINI_PROVIDER,
    providerSecretRef: "cloud-secret://openagents/google-gemini/primary" as OmegaResolvedAuthGrant["providerSecretRef"],
    target: {
      kind: "env",
      name: "GOOGLE_GENERATIVE_AI_API_KEY",
    },
    homeIsolation: "per_run",
    scrubAfterCloseout: true,
  },
  ...overrides,
});

describe("Omega grant resolution", () => {
  test("parses Probe run assignments carrying provider refs and grants", async () => {
    const parsed = await Effect.runPromise(decodeProbeRunAssignment(assignment()));

    expect(parsed.providerAccountRef).toBe("provider-account_primary");
    expect(parsed.authGrantRef).toBe("provider-auth-grant_1");
    expect(parsed.runnerSessionId).toBe("runner_session_1");
  });

  test("resolves a fake Omega grant into a Probe materialization plan", async () => {
    const resolver = makeStaticOmegaGrantResolver(grant());
    const resolved = await Effect.runPromise(resolver.resolveGrant(assignment()));

    expect(resolved.materialization.kind).toBe("probe_chatgpt_auth");
    expect(resolved.materialization.target).toEqual({
      kind: "env",
      name: "PROBE_CHATGPT_AUTH_CONTENT",
    });
  });

  test("resolves a fake Omega Gemini grant into a managed API key materialization plan", async () => {
    const resolver = makeStaticOmegaGrantResolver(geminiGrant());
    const resolved = await Effect.runPromise(resolver.resolveGrant(geminiAssignment()));

    expect(resolved.provider).toBe(GOOGLE_GEMINI_PROVIDER);
    expect(resolved.materialization.kind).toBe("probe_gemini_api_key");
    expect(resolved.materialization.target).toEqual({
      kind: "env",
      name: "GOOGLE_GENERATIVE_AI_API_KEY",
    });
  });

  test("uses the Gemini provider grant route when resolving managed Gemini assignments", async () => {
    const seenPaths: string[] = [];
    const resolver = makeOmegaGrantResolver({
      baseUrl: "https://openagents.example",
      fetch: async (input) => {
        seenPaths.push(new URL(String(input)).pathname);
        return Response.json(geminiGrant());
      },
    });

    await Effect.runPromise(resolver.resolveGrant(geminiAssignment()));

    expect(seenPaths).toEqual(["/api/provider-accounts/google-gemini/grants/resolve"]);
  });

  test("rejects mismatched provider account refs", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          grant({
            providerAccountRef: "provider-account_backup" as OmegaResolvedAuthGrant["providerAccountRef"],
          }),
          assignment(),
        ),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeAuthGrantMismatch",
      field: "providerAccountRef",
    });
  });

  test("rejects expired grants", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          grant({
            expiresAt: "2000-01-01T00:00:00.000Z",
          }),
          assignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeAuthGrantExpired" });
  });

  test("rejects used grant records that are not resolved materialization payloads", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          {
            ...grant(),
            materialization: undefined,
          },
          assignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeAuthGrantResolveError" });
  });

  test("rejects materialization payloads with OpenCode env names", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          {
            ...grant(),
            materialization: {
              ...grant().materialization,
              target: {
                kind: "env",
                name: "OPENCODE_AUTH_CONTENT",
              },
            },
          },
          assignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbePublicProjectionUnsafe" });
  });

  test("rejects Gemini grants with the wrong env target", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          {
            ...geminiGrant(),
            materialization: {
              ...geminiGrant().materialization,
              target: {
                kind: "env",
                name: "GEMINI_API_KEY",
              },
            },
          },
          geminiAssignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeAuthGrantResolveError" });
  });

  test("rejects unsafe Gemini grant payloads with raw key material", async () => {
    await expect(
      Effect.runPromise(
        validateResolvedAuthGrantForAssignment(
          {
            ...geminiGrant(),
            metadata: {
              apiKey: fakeRawGeminiApiKey(),
            },
          },
          geminiAssignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbePublicProjectionUnsafe" });
  });

  test("reports unavailable Omega instead of leaking assignment data", async () => {
    const resolver = makeOmegaGrantResolver({
      baseUrl: "https://omega.invalid",
      fetch: async () => new Response("unavailable", { status: 503 }),
    });

    await expect(Effect.runPromise(resolver.resolveGrant(assignment()))).rejects.toMatchObject({
      _tag: "ProbeAuthGrantResolveError",
      statusCode: 503,
    });
  });
});

// #4999 — Vortex-independent Codex grant-resolution endpoint contract.
describe("neutral Codex grant-resolution endpoint (#4999)", () => {
  test("prefers the neutral OA_CODEX_GRANT_RESOLVE_URL when set", () => {
    const endpoint = resolveCodexGrantResolveEndpoint({
      [OA_CODEX_GRANT_RESOLVE_URL_ENV]: "https://grants.openagents.example",
      [LEGACY_OMEGA_BASE_URL_ENV]: "https://legacy-vortex.invalid",
    });
    expect(endpoint.baseUrl).toBe("https://grants.openagents.example");
    expect(endpoint.baseUrlSource).toBe(OA_CODEX_GRANT_RESOLVE_URL_ENV);
  });

  test("falls back to the legacy PROBE_OMEGA_BASE_URL when the neutral var is unset", () => {
    const endpoint = resolveCodexGrantResolveEndpoint({
      [LEGACY_OMEGA_BASE_URL_ENV]: "https://legacy-vortex.example",
    });
    expect(endpoint.baseUrl).toBe("https://legacy-vortex.example");
    expect(endpoint.baseUrlSource).toBe(LEGACY_OMEGA_BASE_URL_ENV);
  });

  test("falls back to the public default when neither var is set", () => {
    const endpoint = resolveCodexGrantResolveEndpoint({});
    expect(endpoint.baseUrl).toBe(DEFAULT_GRANT_RESOLVE_BASE_URL);
    expect(endpoint.baseUrlSource).toBe("default");
  });

  test("treats blank values as unset", () => {
    const endpoint = resolveCodexGrantResolveEndpoint({
      [OA_CODEX_GRANT_RESOLVE_URL_ENV]: "   ",
      [LEGACY_OMEGA_BASE_URL_ENV]: "https://legacy.example",
    });
    expect(endpoint.baseUrl).toBe("https://legacy.example");
    expect(endpoint.baseUrlSource).toBe(LEGACY_OMEGA_BASE_URL_ENV);
  });

  test("makeOmegaGrantResolverFromEnv resolves against the neutral base URL", async () => {
    const seen: string[] = [];
    const resolver = makeOmegaGrantResolverFromEnv(
      { [OA_CODEX_GRANT_RESOLVE_URL_ENV]: "https://grants.openagents.example" },
      async (input) => {
        seen.push(String(input));
        return Response.json(grant());
      },
    );

    await Effect.runPromise(resolver.resolveGrant(assignment()));
    expect(seen).toEqual([
      "https://grants.openagents.example/api/provider-accounts/chatgpt-codex/grants/resolve",
    ]);
  });
});
