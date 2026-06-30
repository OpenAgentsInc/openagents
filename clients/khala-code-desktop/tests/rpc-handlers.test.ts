import { describe, expect, test } from "bun:test"

import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"

describe("Khala Code desktop RPC handlers", () => {
  test("answers native desktop status probes instead of falling through", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexRateLimitStatus: () => ({
        provider: "codex",
        session: {
          usedPercent: 20,
          remainingPercent: 80,
          windowMinutes: 300,
          resetsAtIso: "2026-06-30T03:00:00.000Z",
          resetDescription: "10:00 PM",
        },
        weekly: {
          usedPercent: 40,
          remainingPercent: 60,
          windowMinutes: 10080,
          resetsAtIso: null,
          resetDescription: null,
        },
        rateLimitResetCredits: {
          availableCount: 1,
          nextExpiresAtIso: "2026-07-01T03:00:00.000Z",
        },
        updatedAtIso: "2026-06-29T19:00:00.000Z",
        error: null,
        status: "ok",
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codingStatus()).resolves.toMatchObject({
      available: true,
      capability: "coding",
      ok: true,
      status: "ready",
    })
    const pylonStatus = await handlers.pylonStatus()
    expect(pylonStatus).toMatchObject({
      capability: "pylon",
      ok: true,
    })
    expect(["ready", "unavailable"]).toContain(pylonStatus.status)
    expect(typeof pylonStatus.available).toBe("boolean")
    await expect(handlers.codexAccountsStatus()).resolves.toMatchObject({
      available: true,
      accounts: [
        {
          accountRef: "default",
          credentialSource: "default_home",
          provider: "codex",
          readiness: {
            state: "ready",
            blockerRefs: [],
          },
        },
      ],
      capability: "codex_accounts",
      ok: true,
      rateLimits: {
        provider: "codex",
        session: {
          usedPercent: 20,
          windowMinutes: 300,
        },
        rateLimitResetCredits: {
          availableCount: 1,
        },
      },
      status: "ready",
    })
    await expect(handlers.tokenAccountingStatus()).resolves.toMatchObject({
      available: false,
      capability: "token_accounting",
      ok: true,
      status: "not_configured",
    })
  })

  test("surfaces provider reset-credit outcomes through RPC", async () => {
    const handler = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexRateLimitStatus: () => ({
        provider: "codex",
        session: null,
        weekly: null,
        rateLimitResetCredits: {
          availableCount: 0,
          nextExpiresAtIso: null,
        },
        updatedAtIso: "2026-06-29T19:00:00.000Z",
        error: null,
        status: "ok",
      }),
      consumeCodexRateLimitResetCredit: input => {
        expect(input.idempotencyKey).toBeTruthy()
        return "noCredit"
      },
      env: { CODEX_HOME: "/tmp/codex-home" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handler.consumeCodexRateLimitResetCredit()).resolves.toMatchObject({
      ok: true,
      outcome: "noCredit",
      status: {
        available: true,
        capability: "codex_accounts",
        accounts: [
          {
            credentialSource: "CODEX_HOME",
            homeRef: "env:CODEX_HOME",
          },
        ],
      },
    })
  })
})
