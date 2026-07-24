import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vite-plus/test";

import {
  CODEX_LEASE_GRANT_PATH,
  CODEX_LEASE_ACQUIRE_PATH,
  CODEX_NATIVE_AUTH_IMPORT_PATH,
  NativeCodexAuthError,
  cleanupNativeCodexScratch,
  materializeNativeCodexAuth,
  runNativeCodexAuthCli,
  type NativeCodexAccountReader,
} from "./native-codex-auth";

const authSecret = "native-access-token-must-not-print";
const authJson = JSON.stringify({
  OPENAI_API_KEY: null,
  auth_mode: "chatgptAuthTokens",
  last_refresh: "2026-07-24T00:00:00.000Z",
  tokens: {
    access_token: authSecret,
    account_id: "account-native-test",
    id_token: "native-id-token-must-not-print",
    refresh_token: "native-refresh-token-must-not-print",
  },
});

const withFixture = async (
  run: (
    fixture: Readonly<{
      root: string;
      sourceAuthJson: string;
      scratchRoot: string;
    }>,
  ) => Promise<void>,
): Promise<void> => {
  const root = mkdtempSync(join(tmpdir(), "oa-native-codex-auth-"));
  const sourceHome = join(root, "source-codex-home");
  const sourceAuthJson = join(sourceHome, "auth.json");
  const scratchRoot = join(root, "scratch");
  try {
    mkdirSync(sourceHome, { recursive: true, mode: 0o700 });
    writeFileSync(sourceAuthJson, authJson, { encoding: "utf8", mode: 0o600, flag: "wx" });
    await run({ root, sourceAuthJson, scratchRoot });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const authenticatedReader: NativeCodexAccountReader = async (input) => {
  expect(lstatSync(input.codexHome).mode & 0o777).toBe(0o700);
  const copiedAuth = join(input.codexHome, "auth.json");
  expect(lstatSync(copiedAuth).mode & 0o777).toBe(0o600);
  expect(readFileSync(copiedAuth, "utf8")).toBe(authJson);
  return {
    account: { type: "chatgpt", email: "owner@example.test" },
    requiresOpenaiAuth: true,
  };
};

describe("native Codex auth materialization", () => {
  test("validates through account/read in an isolated mode-0700 CODEX_HOME", async () => {
    await withFixture(async (fixture) => {
      const sourceBefore = readFileSync(fixture.sourceAuthJson, "utf8");
      const result = await materializeNativeCodexAuth({
        sourceAuthJson: fixture.sourceAuthJson,
        scratchRoot: fixture.scratchRoot,
        accountReader: authenticatedReader,
      });

      expect(result.account).toEqual({
        authenticated: true,
        accountType: "chatgpt",
        requiresOpenaiAuth: true,
        expectedIdentityMatched: null,
      });
      expect(readFileSync(fixture.sourceAuthJson, "utf8")).toBe(sourceBefore);
      cleanupNativeCodexScratch({ codexHome: result.codexHome, scratchRoot: fixture.scratchRoot });
      expect(existsSync(result.codexHome)).toBe(false);
    });
  });

  test("the native app-server probe sends only the initialization handshake and account/read", async () => {
    await withFixture(async (fixture) => {
      const methodLog = join(fixture.root, "app-server-methods.jsonl");
      const fakeCodex = join(fixture.root, "fake-codex.mjs");
      writeFileSync(
        fakeCodex,
        `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
import { createInterface } from "node:readline";
const methods = ${JSON.stringify(methodLog)};
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line);
  if (typeof message.method === "string") appendFileSync(methods, message.method + "\\n");
  if (message.method === "initialize") process.stdout.write(JSON.stringify({ id: message.id, result: {} }) + "\\n");
  if (message.method === "account/read") process.stdout.write(JSON.stringify({ id: message.id, result: { account: { type: "chatgpt", email: "owner@example.test" }, requiresOpenaiAuth: true } }) + "\\n");
}
`,
        { encoding: "utf8", mode: 0o700, flag: "wx" },
      );

      const result = await materializeNativeCodexAuth({
        sourceAuthJson: fixture.sourceAuthJson,
        scratchRoot: fixture.scratchRoot,
        codexBinary: fakeCodex,
        expectedAccountEmail: "owner@example.test",
      });
      expect(readFileSync(methodLog, "utf8").trim().split("\n")).toEqual([
        "initialize",
        "initialized",
        "account/read",
      ]);
      cleanupNativeCodexScratch({
        codexHome: result.codexHome,
        scratchRoot: fixture.scratchRoot,
      });
    });
  });

  test("refuses credential files readable by group or other users", async () => {
    await withFixture(async (fixture) => {
      chmodSync(fixture.sourceAuthJson, 0o644);
      await expect(
        materializeNativeCodexAuth({
          sourceAuthJson: fixture.sourceAuthJson,
          scratchRoot: fixture.scratchRoot,
          accountReader: authenticatedReader,
        }),
      ).rejects.toMatchObject<Partial<NativeCodexAuthError>>({ reason: "source_permissions" });
    });
  });

  test("refuses to use the default Codex home as scratch custody", async () => {
    await withFixture(async (fixture) => {
      await expect(
        materializeNativeCodexAuth({
          sourceAuthJson: fixture.sourceAuthJson,
          scratchRoot: join(homedir(), ".codex", "openagents-scratch"),
          accountReader: authenticatedReader,
        }),
      ).rejects.toMatchObject<Partial<NativeCodexAuthError>>({ reason: "scratch_invalid" });
    });
  });

  test("status and dry-run clean scratch and never emit credential material", async () => {
    await withFixture(async (fixture) => {
      let fetchCalls = 0;
      const result = await runNativeCodexAuthCli(
        [
          "dry-run",
          "--source-auth-json",
          fixture.sourceAuthJson,
          "--scratch-root",
          fixture.scratchRoot,
          "--base-url",
          "https://openagents.com",
          "--lease-ref",
          "lease.codex.test",
        ],
        {},
        {
          accountReader: authenticatedReader,
          fetchImpl: async () => {
            fetchCalls += 1;
            throw new Error("dry-run must not call the network");
          },
        },
      );

      expect(result).toMatchObject({
        ok: true,
        command: "dry-run",
        authenticated: true,
        retainedScratch: false,
        liveHandoffAttempted: false,
        handoffReady: false,
        handoffMissing: ["owner_email", "runner_session_id", "workroom_id", "operator_token"],
      });
      expect(fetchCalls).toBe(0);
      expect(JSON.stringify(result)).not.toContain(authSecret);
    });
  });

  test("requires separate explicit arming before importing credentials", async () => {
    await withFixture(async (fixture) => {
      let fetchCalls = 0;
      await expect(
        runNativeCodexAuthCli(
          [
            "handoff",
            "--source-auth-json",
            fixture.sourceAuthJson,
            "--scratch-root",
            fixture.scratchRoot,
            "--base-url",
            "https://openagents.com",
            "--owner-email",
            "owner@example.test",
            "--lease-ref",
            "lease.codex.test",
            "--runner-session-id",
            "turn.codex.test",
            "--workroom-id",
            "workroom.codex.test",
          ],
          { OPENAGENTS_OPERATOR_ADMIN_TOKEN: "operator-secret" },
          {
            accountReader: authenticatedReader,
            fetchImpl: async () => {
              fetchCalls += 1;
              throw new Error("unarmed handoff must not call the network");
            },
          },
        ),
      ).rejects.toMatchObject<Partial<NativeCodexAuthError>>({
        reason: "live_credential_import_not_armed",
      });
      expect(fetchCalls).toBe(0);
    });
  });

  test("requires grant arming before credential import makes a network call", async () => {
    await withFixture(async (fixture) => {
      let fetchCalls = 0;
      await expect(
        runNativeCodexAuthCli(
          [
            "handoff",
            "--source-auth-json",
            fixture.sourceAuthJson,
            "--scratch-root",
            fixture.scratchRoot,
            "--base-url",
            "https://openagents.com",
            "--owner-email",
            "owner@example.test",
            "--lease-ref",
            "lease.codex.test",
            "--runner-session-id",
            "turn.codex.test",
            "--workroom-id",
            "workroom.codex.test",
            "--allow-live-credential-import",
          ],
          { OPENAGENTS_OPERATOR_ADMIN_TOKEN: "operator-secret" },
          {
            accountReader: authenticatedReader,
            fetchImpl: async () => {
              fetchCalls += 1;
              throw new Error("partially armed handoff must not call the network");
            },
          },
        ),
      ).rejects.toMatchObject<Partial<NativeCodexAuthError>>({ reason: "live_handoff_not_armed" });
      expect(fetchCalls).toBe(0);
    });
  });

  test("refuses handoff when the native identity differs from its explicit expected identity", async () => {
    await withFixture(async (fixture) => {
      let fetchCalls = 0;
      await expect(
        runNativeCodexAuthCli(
          [
            "handoff",
            "--source-auth-json",
            fixture.sourceAuthJson,
            "--scratch-root",
            fixture.scratchRoot,
            "--base-url",
            "https://openagents.com",
            "--owner-email",
            "different-owner@example.test",
            "--native-account-email",
            "another-native-owner@example.test",
            "--lease-ref",
            "lease.codex.test",
            "--runner-session-id",
            "turn.codex.test",
            "--workroom-id",
            "workroom.codex.test",
            "--allow-live-handoff",
            "--allow-live-credential-import",
          ],
          { OPENAGENTS_OPERATOR_ADMIN_TOKEN: "operator-secret" },
          {
            accountReader: authenticatedReader,
            fetchImpl: async () => {
              fetchCalls += 1;
              throw new Error("identity mismatch must not call the network");
            },
          },
        ),
      ).rejects.toMatchObject<Partial<NativeCodexAuthError>>({
        reason: "account_validation_failed",
      });
      expect(fetchCalls).toBe(0);
    });
  });

  test("imports selected native auth then issues only refs through the existing lease grant route", async () => {
    await withFixture(async (fixture) => {
      const observedRequests: Request[] = [];
      const result = await runNativeCodexAuthCli(
        [
          "handoff",
          "--source-auth-json",
          fixture.sourceAuthJson,
          "--scratch-root",
          fixture.scratchRoot,
          "--base-url",
          "https://openagents.com",
          "--owner-email",
          "custody-owner@example.test",
          "--native-account-email",
          "owner@example.test",
          "--lease-ref",
          "lease.codex.test",
          "--runner-session-id",
          "turn.codex.test",
          "--workroom-id",
          "workroom.codex.test",
          "--allow-live-handoff",
          "--allow-live-credential-import",
        ],
        { OPENAGENTS_OPERATOR_ADMIN_TOKEN: "operator-secret" },
        {
          accountReader: authenticatedReader,
          fetchImpl: async (input, init) => {
            const request = new Request(input, init);
            observedRequests.push(request.clone());
            if (request.url.endsWith(CODEX_NATIVE_AUTH_IMPORT_PATH)) {
              return new Response(
                JSON.stringify({
                  ok: true,
                  providerAccountRef: "provider-account.codex.test",
                  accountStatus: "connected",
                  custodyStatus: "stored",
                }),
                { status: 201, headers: { "content-type": "application/json" } },
              );
            }
            if (request.url.endsWith(CODEX_LEASE_ACQUIRE_PATH)) {
              return new Response(
                JSON.stringify({
                  leaseRef: "lease.codex.acquired",
                  providerAccountRef: "provider-account.codex.test",
                  expiresAt: "2026-07-24T08:00:00.000Z",
                  status: "active",
                }),
                { status: 201, headers: { "content-type": "application/json" } },
              );
            }
            return new Response(
              JSON.stringify({
                leaseRef: "lease.codex.acquired",
                providerAccountRef: "provider-account.codex.test",
                grant: {
                  grantRef: "grant.codex.test",
                  expiresAt: "2026-07-24T08:00:00.000Z",
                  runnerSessionId: "turn.codex.test",
                },
              }),
              { status: 201, headers: { "content-type": "application/json" } },
            );
          },
        },
      );

      expect(observedRequests.map((request) => request.url)).toEqual([
        `https://openagents.com${CODEX_NATIVE_AUTH_IMPORT_PATH}`,
        `https://openagents.com${CODEX_LEASE_ACQUIRE_PATH}`,
        `https://openagents.com${CODEX_LEASE_GRANT_PATH}`,
      ]);
      const importText = await observedRequests[0]!.text();
      expect(JSON.parse(importText)).toEqual({
        email: "custody-owner@example.test",
        auth: {
          type: "oauth",
          access: authSecret,
          refresh: "native-refresh-token-must-not-print",
          expires: 0,
          accountId: "account-native-test",
          idToken: "native-id-token-must-not-print",
        },
      });
      const leaseText = await observedRequests[1]!.text();
      expect(JSON.parse(leaseText)).toEqual({
        email: "custody-owner@example.test",
        providerAccountRef: "provider-account.codex.test",
        requiredProvider: "chatgpt_codex",
        requestedAction: "agent_computer.codex_turn",
        runId: "turn.codex.test",
      });
      expect(leaseText).not.toContain(authSecret);
      const grantText = await observedRequests[2]!.text();
      expect(JSON.parse(grantText)).toEqual({
        email: "custody-owner@example.test",
        leaseRef: "lease.codex.acquired",
        runnerSessionId: "turn.codex.test",
        workroomId: "workroom.codex.test",
        requestedAction: "agent_computer.codex_turn",
      });
      expect(grantText).not.toContain(authSecret);
      expect(JSON.stringify(result)).toContain("grant.codex.test");
      expect(result.custodyImport).toEqual({
        providerAccountRef: "provider-account.codex.test",
        accountStatus: "connected",
        custodyStatus: "stored",
      });
      expect(JSON.stringify(result)).not.toContain("operator-secret");
      expect(JSON.stringify(result)).not.toContain(authSecret);

      expect(result.retainedScratch).toBe(false);
    });
  });
});
