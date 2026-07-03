import { describe, expect, test } from "bun:test"

import {
  initialKhalaCodeMainShellModel,
  updateKhalaCodeMainShellModel,
} from "../src/ui/main-shell-model"
import type {
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopThreadTokenSummary,
} from "../src/shared/rpc"

const emptyThreadTokenSummary = (
  threadId: string | null,
): KhalaCodeDesktopThreadTokenSummary => ({
  auditRows: 0,
  codexStateDbPath: "",
  codexStateTokens: 0,
  leaderboardLabel: "OpenAgents Stats",
  leaderboardSyncedTokens: 0,
  localLedgerPath: "",
  localMessageAuditLedgerPath: "",
  missingUsageTurns: 0,
  ok: true,
  pendingSyncTokens: 0,
  remoteConfigured: false,
  remoteDisabled: false,
  roleEconomics: [],
  threadId,
  totalTokens: 0,
  updatedAt: null,
  usageEventRows: 0,
})

describe("Khala Code main shell model", () => {
  test("starts with no seeded transcript or active turn", () => {
    const model = initialKhalaCodeMainShellModel({
      threadTokenSummary: emptyThreadTokenSummary(null),
    })

    expect(model.messages).toEqual([])
    expect(model.pendingTurn).toBe(false)
    expect(model.thinkingTurnId).toBeNull()
    expect(model.transcriptPinnedToEnd).toBe(true)
    expect(model.selectedHarnessMode).toBe("codex_harness")
  })

  test("copies transcript updates through the TEA reducer", () => {
    const message: KhalaCodeDesktopMessage = {
      body: "hello",
      id: "user-1",
      role: "user",
    }
    const source = [message]
    const model = updateKhalaCodeMainShellModel(
      initialKhalaCodeMainShellModel({
        threadTokenSummary: emptyThreadTokenSummary(null),
      }),
      { _tag: "MessagesChanged", messages: source },
    )
    source.splice(0)

    expect(model.messages).toEqual([message])
  })

  test("tracks shell harness, token, and approval state in one model", () => {
    const summary = {
      ...emptyThreadTokenSummary("thread-1"),
      totalTokens: 42,
    }
    const model = [
      {
        _tag: "HarnessSettingChanged" as const,
        envOverride: null,
        mode: "claude_runtime" as const,
      },
      {
        _tag: "LastResponseRuntimeModeChanged" as const,
        mode: "claude_runtime" as const,
      },
      { _tag: "ThreadTokenSummaryChanged" as const, summary },
      { _tag: "ThreadTokenPopoverChanged" as const, open: true },
      { _tag: "ClaudeApprovalDialogToggled" as const, open: true },
    ].reduce(
      updateKhalaCodeMainShellModel,
      initialKhalaCodeMainShellModel({
        threadTokenSummary: emptyThreadTokenSummary(null),
      }),
    )

    expect(model.selectedHarnessMode).toBe("claude_runtime")
    expect(model.lastResponseRuntimeMode).toBe("claude_runtime")
    expect(model.threadTokenSummary.totalTokens).toBe(42)
    expect(model.threadTokenPopoverOpen).toBe(true)
    expect(model.claudeApprovalDialogOpen).toBe(true)
  })
})
