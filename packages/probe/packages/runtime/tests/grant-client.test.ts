import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  decodeProbeRunAssignment,
  GOOGLE_GEMINI_PROVIDER,
  makeOmegaGrantResolver,
  makeStaticOmegaGrantResolver,
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
