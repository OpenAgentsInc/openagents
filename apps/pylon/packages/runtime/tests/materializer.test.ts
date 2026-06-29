import { mkdtemp, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  GOOGLE_GEMINI_PROVIDER,
  materializeProbeAuthGrant,
  runNoProviderAuthSmoke,
  scrubProbeMaterializedAuth,
  withProbeAuthMaterialization,
  type OmegaResolvedAuthGrant,
} from "../src";

const authContent = JSON.stringify({
  type: "oauth",
  access: "fake-access-token",
  refresh: "fake-refresh-token",
  expires: 4_102_444_800_000,
});

const fileGrant = (relativePath = "auth/chatgpt-codex.json"): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: "provider-account_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  runnerSessionId: "runner_session_1",
  requestedAction: "coding-agent-run",
  expiresAt: "2099-01-01T00:00:00.000Z",
  status: "used",
  materialization: {
    kind: "probe_chatgpt_auth",
    provider: CHATGPT_CODEX_PROVIDER,
    providerSecretRef: "codex-auth://provider-account_primary" as OmegaResolvedAuthGrant["providerSecretRef"],
    target: {
      kind: "file",
      relativePath,
    },
    homeIsolation: "per_run",
    scrubAfterCloseout: true,
  },
});

const envGrant = (): OmegaResolvedAuthGrant => ({
  ...fileGrant(),
  materialization: {
    ...fileGrant().materialization,
    target: {
      kind: "env",
      name: "PROBE_CHATGPT_AUTH_CONTENT",
    },
  },
});

const geminiGrant = (): OmegaResolvedAuthGrant => ({
  grantRef: "provider-auth-grant_google_gemini_1" as OmegaResolvedAuthGrant["grantRef"],
  provider: GOOGLE_GEMINI_PROVIDER,
  providerAccountRef: "provider-account_google_gemini_primary" as OmegaResolvedAuthGrant["providerAccountRef"],
  providerSecretRef: "cloud-secret://openagents/google-gemini/primary" as OmegaResolvedAuthGrant["providerSecretRef"],
  runnerSessionId: "runner_session_1",
  requestedAction: "gemini-backend-run",
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

const secret = {
  providerSecretRef: "codex-auth://provider-account_primary",
  authContent,
  contentType: "application/json",
};

const geminiSecret = {
  providerSecretRef: "cloud-secret://openagents/google-gemini/primary",
  authContent: "brokered-gemini-key-content",
  contentType: "text/plain",
};

const tempRunHome = () => mkdtemp(join(tmpdir(), "probe-run-"));

describe("Probe auth materializer", () => {
  test("materializes fake ChatGPT auth into a per-run file and scrubs it", async () => {
    const runHome = await tempRunHome();
    const materialized = await Effect.runPromise(materializeProbeAuthGrant({ grant: fileGrant(), secret, runHome }));

    expect(materialized.materializedPath).toEndWith("auth/chatgpt-codex.json");
    expect(materialized.receipt).toMatchObject({
      kind: "probe_auth_materialized",
      contentRedacted: true,
    });

    await Effect.runPromise(runNoProviderAuthSmoke(materialized));

    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));

    expect(scrubbed).toMatchObject({
      kind: "probe_auth_scrubbed",
      contentRedacted: true,
    });

    await expect(stat(materialized.materializedPath as string)).rejects.toThrow();
  });

  test("materializes env auth without writing raw content to receipts", async () => {
    const runHome = await tempRunHome();
    const materialized = await Effect.runPromise(materializeProbeAuthGrant({ grant: envGrant(), secret, runHome }));

    expect(materialized.env.PROBE_CHATGPT_AUTH_CONTENT).toBe(authContent);
    expect(JSON.stringify(materialized.receipt)).not.toContain("fake-access-token");

    await Effect.runPromise(runNoProviderAuthSmoke(materialized));
    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));

    expect(JSON.stringify(scrubbed)).not.toContain("fake-refresh-token");
  });

  test("materializes Gemini managed API keys into the Google Generative AI env var", async () => {
    const runHome = await tempRunHome();
    const materialized = await Effect.runPromise(
      materializeProbeAuthGrant({ grant: geminiGrant(), secret: geminiSecret, runHome }),
    );

    expect(materialized.env.GOOGLE_GENERATIVE_AI_API_KEY).toBe(geminiSecret.authContent);
    expect(materialized.receipt).toMatchObject({
      provider: GOOGLE_GEMINI_PROVIDER,
      targetKind: "env",
      envName: "GOOGLE_GENERATIVE_AI_API_KEY",
      contentRedacted: true,
    });
    expect(JSON.stringify(materialized.receipt)).not.toContain(geminiSecret.authContent);

    const scrubbed = await Effect.runPromise(scrubProbeMaterializedAuth(materialized));
    expect(scrubbed).toMatchObject({
      provider: GOOGLE_GEMINI_PROVIDER,
      targetKind: "env",
      envName: "GOOGLE_GENERATIVE_AI_API_KEY",
      contentRedacted: true,
    });
    expect(JSON.stringify(scrubbed)).not.toContain(geminiSecret.authContent);
  });

  test("scrubs Gemini env materialization when user code fails", async () => {
    const runHome = await tempRunHome();
    let sawMaterializedEnv = false;

    await expect(
      Effect.runPromise(
        withProbeAuthMaterialization({ grant: geminiGrant(), secret: geminiSecret, runHome }, (materialized) => {
          sawMaterializedEnv = materialized.env.GOOGLE_GENERATIVE_AI_API_KEY === geminiSecret.authContent;
          return Effect.fail(new Error("simulated gemini run failure"));
        }),
      ),
    ).rejects.toThrow("simulated gemini run failure");

    expect(sawMaterializedEnv).toBe(true);
  });

  test("rejects brokered secrets that do not match the grant ref", async () => {
    const runHome = await tempRunHome();

    await expect(
      Effect.runPromise(
        materializeProbeAuthGrant({
          grant: fileGrant(),
          secret: {
            ...secret,
            providerSecretRef: "codex-auth://provider-account_other",
          },
          runHome,
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeAuthMaterializationError" });
  });

  test("rejects file materialization outside the run home", async () => {
    const runHome = await tempRunHome();

    await expect(
      Effect.runPromise(
        materializeProbeAuthGrant({
          grant: fileGrant("../outside.json"),
          secret,
          runHome,
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeAuthMaterializationError" });
  });

  test("scrubs file materialization when user code fails", async () => {
    const runHome = await tempRunHome();
    let materializedPath: string | undefined;

    await expect(
      Effect.runPromise(
        withProbeAuthMaterialization({ grant: fileGrant(), secret, runHome }, (materialized) => {
          materializedPath = materialized.materializedPath;
          return Effect.fail(new Error("simulated run failure"));
        }),
      ),
    ).rejects.toThrow("simulated run failure");

    await expect(stat(materializedPath as string)).rejects.toThrow();
  });
});
