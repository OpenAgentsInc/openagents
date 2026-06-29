import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  makeStaticOmegaAccountClient,
  runProbeCli,
  type PublicProviderAccount,
} from "../src";

const account = (ref: string, label: string): PublicProviderAccount => ({
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: ref as PublicProviderAccount["providerAccountRef"],
  authMode: "chatgpt_device_code",
  status: "connected",
  health: "healthy",
  secretRef: `codex-auth://${ref}` as PublicProviderAccount["secretRef"],
  accountLabel: label,
  planType: "plus",
});

describe("Probe CLI Omega auth commands", () => {
  test("probe omega link writes local runner identity state without credentials", async () => {
    const dir = await mkdtemp(join(tmpdir(), "probe-cli-"));
    const statePath = join(dir, "omega-link.json");
    const result = await Effect.runPromise(
      runProbeCli([
        "omega",
        "link",
        "--state",
        statePath,
        "--runner-id",
        "runner_local_1",
        "--subject",
        "user_1",
        "--kind",
        "local",
      ]),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("runner_local_1");

    const state = await readFile(statePath, "utf8");
    expect(state).toContain("omega.grant.resolve");
    expect(state).not.toContain("access_token");
    expect(state).not.toContain("refresh_token");
  });

  test("probe auth accounts lists multiple Omega accounts without secrets", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["auth", "accounts"], {
        accountClient: makeStaticOmegaAccountClient({
          accounts: {
            accounts: [account("provider-account_primary", "Primary"), account("provider-account_backup", "Backup")],
          },
        }),
      }),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("provider-account_primary");
    expect(result.stdout).toContain("provider-account_backup");
    expect(result.stdout).toContain("Primary");
    expect(result.stdout).not.toContain("codex-auth://");
  });

  test("probe auth add chatgpt delegates device login to Omega", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["auth", "add", "chatgpt"], {
        accountClient: makeStaticOmegaAccountClient({
          startedLogin: {
            attemptId: "attempt_1",
            providerAccountRef: "provider-account_new",
            verificationUrl: "https://auth.openai.com/codex/device",
            userCode: "OA-1234",
            expiresAt: "2099-01-01T00:00:00.000Z",
          },
          attempt: {
            attemptId: "attempt_1",
            status: "connected",
            providerAccountRef: "provider-account_new",
            accountLabel: "New ChatGPT",
          },
        }),
      }),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("https://auth.openai.com/codex/device");
    expect(result.stdout).toContain("OA-1234");
    expect(result.stdout).toContain("connected");
    expect(result.stdout).not.toContain("access_token");
  });
});
