import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  GOOGLE_GEMINI_PROVIDER,
  PROBE_GEMINI_BACKEND_CAPABILITY,
  makeStaticOmegaGrantResolver,
  makeStaticProbeSecretBroker,
  prepareAuthorizedProbeAuthRun,
  runProbeBackendAssignment,
  scrubProbeMaterializedAuth,
  type OmegaResolvedAuthGrant,
  type ProbeRunAssignment,
  type ProbeRunnerAssignmentProof,
  type ProbeRunnerIdentity,
} from "../src";

const OMEGA_PROVIDER_ACCOUNT_IMPLEMENTATION_ISSUE = "OpenAgentsInc/autopilot-omega#526";

const assignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_managed_gemini_1",
  runnerSessionId: "runner_session_managed_gemini_1",
  goal: "Reply with managed Gemini fixture ok.",
  provider: GOOGLE_GEMINI_PROVIDER,
  providerAccountRef: "provider-account_google_gemini_primary" as ProbeRunAssignment["providerAccountRef"],
  authGrantRef: "provider-auth-grant_google_gemini_1" as ProbeRunAssignment["authGrantRef"],
  backend: {
    kind: "gemini_api",
    backendProfileId: "gemini-api",
  },
});

const runner = (): ProbeRunnerIdentity => ({
  runnerId: "runner_managed_gemini_1",
  kind: "local",
  linkedSubject: "user_1",
  linkedAt: "2026-06-08T00:00:00.000Z",
  capabilities: ["probe.run", "omega.grant.resolve", PROBE_GEMINI_BACKEND_CAPABILITY],
});

const proof = (): ProbeRunnerAssignmentProof => ({
  runnerId: "runner_managed_gemini_1",
  assignmentId: "assignment_managed_gemini_1",
  runnerSessionId: "runner_session_managed_gemini_1",
  issuedAt: "2026-06-08T00:00:00.000Z",
  nonce: "nonce_managed_gemini_1",
  proofKind: "test",
});

const grant = (): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_google_gemini_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: GOOGLE_GEMINI_PROVIDER,
  providerAccountRef: "provider-account_google_gemini_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "cloud-secret://openagents/google-gemini/primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  requestedAction: "gemini-backend-run",
  runnerSessionId: "runner_session_managed_gemini_1",
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
});

const sse = (...events: ReadonlyArray<unknown>): string =>
  `${events.map((event) => `data: ${JSON.stringify(event)}\n`).join("\n")}data: [DONE]\n\n`;

const liveApiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY ?? process.env.GEMINI_API_KEY;
const liveEnabled =
  process.env.PROBE_GEMINI_MANAGED_LIVE_SMOKE === "1" && liveApiKey !== undefined && liveApiKey.trim() !== "";
const liveTest = liveEnabled ? test : test.skip;

describe("managed Gemini provider-account E2E smoke", () => {
  test("resolves, materializes, completes, and scrubs through a fake Omega grant", async () => {
    const runHome = await mkdtemp(join(tmpdir(), "probe-managed-gemini-"));
    const secret = {
      providerSecretRef: "cloud-secret://openagents/google-gemini/primary",
      authContent: "brokered-managed-gemini-key-content",
      contentType: "text/plain",
    };
    const materialized = await Effect.runPromise(
      prepareAuthorizedProbeAuthRun({
        runner: runner(),
        proof: proof(),
        assignment: assignment(),
        grantResolver: makeStaticOmegaGrantResolver(grant()),
        secretBroker: makeStaticProbeSecretBroker(secret),
        runHome,
        now: new Date("2026-06-08T00:00:00.000Z"),
      }),
    );

    expect(materialized.env.GOOGLE_GENERATIVE_AI_API_KEY).toBe(secret.authContent);
    expect(materialized.receipt.provider).toBe(GOOGLE_GEMINI_PROVIDER);
    expect(JSON.stringify(materialized.receipt)).not.toContain(secret.authContent);

    const backendResult = await Effect.runPromise(
      runProbeBackendAssignment({
        runner: runner(),
        proof: proof(),
        assignment: assignment(),
        env: materialized.env,
        fetch: async (input, init) => {
          expect(new Headers(init?.headers).get("x-goog-api-key")).toBe(secret.authContent);
          const body = JSON.parse(String(init?.body));
          expect(body.contents[0].parts[0].text).toBe("Reply with managed Gemini fixture ok.");

          return new Response(
            sse({
              candidates: [
                {
                  content: { role: "model", parts: [{ text: "managed Gemini fixture ok" }] },
                  finishReason: "STOP",
                },
              ],
              usageMetadata: { promptTokenCount: 6, candidatesTokenCount: 5, totalTokenCount: 11 },
            }),
            { status: 200 },
          );
        },
        now: new Date("2026-06-08T00:00:01.000Z"),
      }),
    );

    expect(backendResult.backendKind).toBe("gemini_api");
    expect(backendResult.completion.text).toBe("managed Gemini fixture ok");
    expect(JSON.stringify(backendResult.events)).not.toContain(secret.authContent);
    expect(OMEGA_PROVIDER_ACCOUNT_IMPLEMENTATION_ISSUE).toBe("OpenAgentsInc/autopilot-omega#526");

    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));
    expect(scrubbed).toMatchObject({
      provider: GOOGLE_GEMINI_PROVIDER,
      envName: "GOOGLE_GENERATIVE_AI_API_KEY",
      contentRedacted: true,
    });
    expect(JSON.stringify(scrubbed)).not.toContain(secret.authContent);
  });

  liveTest("runs the managed Gemini flow against a live Gemini API key", async () => {
    const runHome = await mkdtemp(join(tmpdir(), "probe-managed-gemini-live-"));
    const secret = {
      providerSecretRef: "cloud-secret://openagents/google-gemini/live-smoke",
      authContent: liveApiKey as string,
      contentType: "text/plain",
    };
    const materialized = await Effect.runPromise(
      prepareAuthorizedProbeAuthRun({
        runner: runner(),
        proof: proof(),
        assignment: assignment(),
        grantResolver: makeStaticOmegaGrantResolver({
          ...grant(),
          providerSecretRef: secret.providerSecretRef as OmegaResolvedAuthGrant["providerSecretRef"],
          materialization: {
            ...grant().materialization,
            providerSecretRef: secret.providerSecretRef as OmegaResolvedAuthGrant["providerSecretRef"],
          },
        }),
        secretBroker: makeStaticProbeSecretBroker(secret),
        runHome,
      }),
    );
    const backendResult = await Effect.runPromise(
      runProbeBackendAssignment({
        runner: runner(),
        proof: proof(),
        assignment: {
          ...assignment(),
          goal: "Reply with exactly: probe-managed-gemini-live-ok",
        },
        env: materialized.env,
      }),
    );
    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));

    expect(backendResult.completion.text.trim().length).toBeGreaterThan(0);
    expect(JSON.stringify({ events: backendResult.events, materialized: materialized.receipt, scrubbed })).not.toContain(
      liveApiKey,
    );
  });
});
