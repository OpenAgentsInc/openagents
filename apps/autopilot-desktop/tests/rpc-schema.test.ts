import { describe, expect, test } from "bun:test"

import {
  DesktopRpcRequestSchemas,
  decodeDesktopRpcRequestParams,
  decodeDesktopRpcResponse,
  decodeDesktopRpcWebviewMessage,
  withDesktopRpcClientDecoding,
  withDesktopRpcRequestDecoding,
  withDesktopRpcWebviewMessageDecoding,
} from "../src/shared/rpc"

describe("desktop RPC schema contract", () => {
  test("declares schemas for the complete Bun request surface", () => {
    expect(Object.keys(DesktopRpcRequestSchemas).sort()).toEqual([
      "activateTrainingWindow",
      "addManagedAccount",
      "admitTrainingRealGradientEvidence",
      "appleFmReadiness",
      "buildTrainingEvidencePacket",
      "builtinAgentReadiness",
      "cancelSession",
      "chooseIdentity",
      "claimTrainingWindowLease",
      "deployCloud",
      "getAccountStatus",
      "identityChoiceState",
      "inferenceGatewayReadiness",
      "installReadiness",
      "khalaTurn",
      "listManagedAccounts",
      "listPublicActivityTimeline",
      "listTrainingDashboard",
      "listTrainingEvidencePacketSummary",
      "listTrainingOperatorReadiness",
      "listTrainingPromiseGates",
      "listTrainingRuns",
      "onboardingStatus",
      "openExternal",
      "planTrainingRunWindow",
      "promiseSurfacingReadiness",
      "reconcileTrainingWindow",
      "removeManagedAccount",
      "requestTrainingBootstrapGrant",
      "resetAccountStatus",
      "resolveApproval",
      "resolveManagedWorktree",
      "setCoordinatorPaused",
      "setManagedAccountPriority",
      "shellTurn",
      "spawnAppleFmSession",
      "spawnSession",
      "startAppleFmSession",
      "startBuiltInAgent",
      "submitIntent",
      "surfacePromiseGap",
      "verseTurn",
    ])
  })

  test("decodes request params and rejects invalid enum values", () => {
    expect(
      decodeDesktopRpcRequestParams("spawnSession", {
        adapter: "codex",
        objective: "Run the focused check",
        lane: "local",
      }),
    ).toMatchObject({
      adapter: "codex",
      objective: "Run the focused check",
      lane: "local",
    })

    expect(() =>
      decodeDesktopRpcRequestParams("spawnSession", {
        adapter: "python",
        objective: "Run the focused check",
      }),
    ).toThrow()
  })

  test("decodes tagged handler failures as response unions", () => {
    expect(
      decodeDesktopRpcResponse("openExternal", {
        _tag: "DesktopRpcHandlerFailure",
        ok: false,
        method: "openExternal",
        error: "boom",
      }),
    ).toMatchObject({
      _tag: "DesktopRpcHandlerFailure",
      ok: false,
      method: "openExternal",
    })
  })

  test("wraps request handlers with param decode and tagged failure response", async () => {
    const handlers = withDesktopRpcRequestDecoding({
      async openExternal(params) {
        if (params.url === "https://throw.example") {
          throw new Error("browser failed")
        }
        return { ok: true }
      },
    })

    await expect(handlers.openExternal?.({ url: 42 } as never)).rejects.toThrow()
    await expect(
      handlers.openExternal?.({ url: "https://openagents.com" }),
    ).resolves.toEqual({ ok: true })
    await expect(
      handlers.openExternal?.({ url: "https://throw.example" }),
    ).resolves.toMatchObject({
      _tag: "DesktopRpcHandlerFailure",
      ok: false,
      method: "openExternal",
    })
  })

  test("wraps the webview RPC client with request and response decoding", async () => {
    let sentUrl = ""
    const client = withDesktopRpcClientDecoding({
      async openExternal(params) {
        sentUrl = params.url
        return { ok: true }
      },
    })

    await expect(client.openExternal?.({ url: 1 } as never)).rejects.toThrow()
    await expect(
      client.openExternal?.({ url: "https://openagents.com" }),
    ).resolves.toEqual({ ok: true })
    expect(sentUrl).toBe("https://openagents.com")
  })

  test("decodes webview messages before dispatch", () => {
    expect(
      decodeDesktopRpcWebviewMessage("shellControl", {
        action: "set-input",
        value: "hello",
      }),
    ).toEqual({ action: "set-input", value: "hello" })

    let received = ""
    const handlers = withDesktopRpcWebviewMessageDecoding({
      khalaToken(payload) {
        received = `${payload.turnId}:${payload.delta}`
      },
    })

    handlers.khalaToken?.({ turnId: "turn_1", delta: "hi" })
    expect(received).toBe("turn_1:hi")
    expect(() =>
      handlers.khalaToken?.({ turnId: "turn_1", delta: 1 } as never),
    ).toThrow()
  })
})
