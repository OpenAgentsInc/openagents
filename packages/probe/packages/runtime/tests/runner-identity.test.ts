import { mkdtemp, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  authorizeRunnerForAssignment,
  assignmentRequiresProviderGrant,
  CHATGPT_CODEX_PROVIDER,
  decodeProbeRunAssignment,
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  PROBE_GEMINI_BACKEND_CAPABILITY,
  requiredRunnerCapabilitiesForAssignment,
  makeStaticOmegaGrantResolver,
  makeStaticProbeSecretBroker,
  prepareAuthorizedProbeAuthRun,
  scrubProbeMaterializedAuth,
  type OmegaResolvedAuthGrant,
  type ProbeRunAssignment,
  type ProbeRunnerAssignmentProof,
  type ProbeRunnerIdentity,
} from "../src";

const assignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_1",
  runnerSessionId: "runner_session_1",
  goal: "Run Probe in a sandbox",
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: "provider-account_primary" as ProbeRunAssignment["providerAccountRef"],
  authGrantRef: "provider-auth-grant_1" as ProbeRunAssignment["authGrantRef"],
});

const appleFmAssignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_1",
  runnerSessionId: "runner_session_1",
  goal: "Summarize the repository locally",
  backend: {
    kind: "apple_fm_bridge",
    profile: "apple-fm-local",
  },
});

const geminiAssignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_1",
  runnerSessionId: "runner_session_1",
  goal: "Summarize through Gemini",
  backend: {
    kind: "gemini_api",
    backendProfileId: "gemini-api",
  },
});

const runner = (overrides: Partial<ProbeRunnerIdentity> = {}): ProbeRunnerIdentity => ({
  runnerId: "runner_shc_1",
  kind: "shc",
  linkedSubject: "user_1",
  linkedAt: "2026-06-07T00:00:00.000Z",
  expiresAt: "2099-01-01T00:00:00.000Z",
  capabilities: ["probe.run", "omega.grant.resolve"],
  ...overrides,
});

const proof = (overrides: Partial<ProbeRunnerAssignmentProof> = {}): ProbeRunnerAssignmentProof => ({
  runnerId: "runner_shc_1",
  assignmentId: "assignment_1",
  runnerSessionId: "runner_session_1",
  issuedAt: "2026-06-07T00:00:00.000Z",
  nonce: "nonce_1",
  proofKind: "test",
  ...overrides,
});

const grant = (): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: "provider-account_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  runnerSessionId: "runner_session_1",
  expiresAt: "2099-01-01T00:00:00.000Z",
  status: "used",
  materialization: {
    kind: "probe_chatgpt_auth",
    provider: CHATGPT_CODEX_PROVIDER,
    providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
    target: {
      kind: "file",
      relativePath: "auth/chatgpt-codex.json",
    },
    homeIsolation: "per_run",
    scrubAfterCloseout: true,
  },
});

const secret = {
  providerSecretRef: "codex-auth://provider-account_primary",
  authContent: JSON.stringify({ type: "oauth", access: "fake-access-token" }),
};

const tempRunHome = () => mkdtemp(join(tmpdir(), "probe-runner-"));

describe("Probe runner identity gate", () => {
  test("linked runner can resolve and materialize only its assigned grant", async () => {
    const runHome = await tempRunHome();
    const materialized = await Effect.runPromise(
      prepareAuthorizedProbeAuthRun({
        runner: runner(),
        proof: proof(),
        assignment: assignment(),
        grantResolver: makeStaticOmegaGrantResolver(grant()),
        secretBroker: makeStaticProbeSecretBroker(secret),
        runHome,
      }),
    );

    expect(materialized.materializedPath).toEndWith("auth/chatgpt-codex.json");

    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));
    expect(scrubbed.kind).toBe("probe_auth_scrubbed");
    await expect(stat(materialized.materializedPath as string)).rejects.toThrow();
  });

  test("unlinked runner cannot authorize grant resolution", async () => {
    await expect(
      Effect.runPromise(
        authorizeRunnerForAssignment(
          runner({ capabilities: ["probe.run"] }),
          proof(),
          assignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("mismatched runner proof is denied before grant resolution", async () => {
    await expect(
      Effect.runPromise(authorizeRunnerForAssignment(runner(), proof({ runnerId: "runner_other" }), assignment())),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("mismatched assignment proof is denied", async () => {
    await expect(
      Effect.runPromise(authorizeRunnerForAssignment(runner(), proof({ assignmentId: "assignment_other" }), assignment())),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("SHC/Pylon assignment payloads reject raw credentials", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeRunAssignment({
          ...assignment(),
          sandbox: {
            access_token: "raw-token",
          },
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbePublicProjectionUnsafe" });
  });

  test("Apple FM assignments require backend capability but no Omega grant capability", async () => {
    await Effect.runPromise(
      authorizeRunnerForAssignment(
        runner({
          capabilities: ["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY],
        }),
        proof(),
        appleFmAssignment(),
      ),
    );

    expect(assignmentRequiresProviderGrant(appleFmAssignment())).toBe(false);
    expect(requiredRunnerCapabilitiesForAssignment(appleFmAssignment())).toEqual([
      "probe.run",
      PROBE_APPLE_FM_BACKEND_CAPABILITY,
    ]);

    await expect(
      Effect.runPromise(
        authorizeRunnerForAssignment(
          runner({
            capabilities: ["probe.run"],
          }),
          proof(),
          appleFmAssignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("Gemini assignments require Gemini backend capability but no Omega grant capability", async () => {
    await Effect.runPromise(
      authorizeRunnerForAssignment(
        runner({
          capabilities: ["probe.run", PROBE_GEMINI_BACKEND_CAPABILITY],
        }),
        proof(),
        geminiAssignment(),
      ),
    );

    expect(assignmentRequiresProviderGrant(geminiAssignment())).toBe(false);
    expect(requiredRunnerCapabilitiesForAssignment(geminiAssignment())).toEqual([
      "probe.run",
      PROBE_GEMINI_BACKEND_CAPABILITY,
    ]);

    await expect(
      Effect.runPromise(
        authorizeRunnerForAssignment(
          runner({
            capabilities: ["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY],
          }),
          proof(),
          geminiAssignment(),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("runner authorization rejects Blueprint capability refs outside the selected backend", async () => {
    await expect(
      Effect.runPromise(
        authorizeRunnerForAssignment(
          runner({
            capabilities: ["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY],
          }),
          proof(),
          {
            ...appleFmAssignment(),
            blueprint: {
              backendCapabilityRefs: ["probe.backend.openai_responses"],
              registryVersionRef: "blueprint_registry.test.v1",
            },
          },
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });
});
