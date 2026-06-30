import { describe, expect, test } from "bun:test"

import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"

describe("Khala Code desktop RPC handlers", () => {
  test("answers native desktop status probes instead of falling through", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
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
    await expect(handlers.pylonStatus()).resolves.toMatchObject({
      available: false,
      capability: "pylon",
      ok: true,
      status: "not_configured",
    })
    await expect(handlers.codexAccountsStatus()).resolves.toMatchObject({
      available: false,
      capability: "codex_accounts",
      ok: true,
      status: "not_configured",
    })
    await expect(handlers.tokenAccountingStatus()).resolves.toMatchObject({
      available: false,
      capability: "token_accounting",
      ok: true,
      status: "not_configured",
    })
  })
})
