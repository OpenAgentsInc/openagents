import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"

import {
  KhalaCodeDesktopRpcBridgeFailure,
  KhalaCodeDesktopRpcMethodSchemas,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  khalaCodeDesktopRpcDecodeFailure,
  khalaCodeDesktopRpcHandlerFailure,
} from "../src/shared/rpc"

describe("Khala Code desktop schema-first RPC contract", () => {
  test("decodes request parameters and response results by method", () => {
    const args = decodeKhalaCodeDesktopRpcParameters("submitChatMessage", [
      {
        messages: [
          {
            body: "hello",
            id: "message-1",
            role: "user",
          },
        ],
        sessionId: "session-1",
      },
    ])

    expect(args).toHaveLength(1)
    expect(args[0]).toMatchObject({ sessionId: "session-1" })

    const result = decodeKhalaCodeDesktopRpcResult("submitChatMessage", {
      backend: {
        kind: "mock",
        model: "test-model",
      },
      messages: [
        {
          body: "hello",
          id: "message-1",
          role: "user",
        },
      ],
      ok: true,
      toolNames: [],
      usedTools: [],
    })

    expect(result).toMatchObject({ ok: true })
  })

  test("rejects malformed request parameters before a handler runs", () => {
    expect(() =>
      decodeKhalaCodeDesktopRpcParameters("codexThreadRead", [{ includeTurns: true }]),
    ).toThrow()

    const failure = khalaCodeDesktopRpcDecodeFailure(
      "codexThreadRead",
      new Error("threadId is required"),
    )

    expect(S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)(failure)).toEqual({
      error: "threadId is required",
      method: "codexThreadRead",
      ok: false,
      tag: "rpc_decode_failed",
    })
  })

  test("models handler failures as distinct tagged bridge errors", () => {
    const failure = khalaCodeDesktopRpcHandlerFailure(
      "appInfo",
      new Error("boom"),
    )

    expect(S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)(failure)).toEqual({
      error: "boom",
      method: "appInfo",
      ok: false,
      tag: "rpc_handler_failed",
    })
  })

  test("covers every current request method with a schema entry", () => {
    expect(Object.keys(KhalaCodeDesktopRpcMethodSchemas).sort()).toEqual([
      "appInfo",
      "appleFmReadiness",
      "codexAccountsStatus",
      "codexAppServerRestart",
      "codexAppServerStart",
      "codexAppServerStatus",
      "codexAppServerStop",
      "codexApprovalRespond",
      "codexBackgroundTerminalsClean",
      "codexBackgroundTerminalsList",
      "codexBackgroundTerminalsTerminate",
      "codexConfigValueWrite",
      "codexEcosystemRead",
      "codexExternalAgentConfigDetect",
      "codexExternalAgentConfigImport",
      "codexExternalAgentConfigImportHistoriesRead",
      "codexFleetDelegateRun",
      "codexFleetPromoteThread",
      "codexFleetStatus",
      "codexFsGetMetadata",
      "codexFsReadFile",
      "codexFsWriteFile",
      "codexHarnessStatus",
      "codexMarketplaceAdd",
      "codexMarketplaceRemove",
      "codexMarketplaceUpgrade",
      "codexMcpOauthLogin",
      "codexMcpResourceRead",
      "codexMcpServerReload",
      "codexMcpToolCall",
      "codexMentionCandidates",
      "codexPluginInstall",
      "codexPluginUninstall",
      "codexSettingsRead",
      "codexSkillsConfigWrite",
      "codexSkillsExtraRootsSet",
      "codexThreadArchive",
      "codexThreadCompact",
      "codexThreadDelete",
      "codexThreadFork",
      "codexThreadList",
      "codexThreadRead",
      "codexThreadRename",
      "codexThreadResume",
      "codexThreadStart",
      "codexThreadUnarchive",
      "codexTurnInterrupt",
      "codexTurnStart",
      "codexTurnSteer",
      "codingStatus",
      "connectCodexAccount",
      "consumeCodexRateLimitResetCredit",
      "onDeviceDeciderStatus",
      "openExternalUrl",
      "pylonStatus",
      "removeCodexAccount",
      "slashCommandDispatch",
      "slashCommandList",
      "submitChatMessage",
      "threadTokenSummary",
      "tokenAccountingStatus",
      "toolCatalog",
    ])
  })
})
