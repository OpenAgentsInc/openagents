import {
  emptyComposerState,
  type ComposerAttachmentUploadReceipt,
  type ComposerState,
} from "@openagentsinc/composer-state"

import type {
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopRPCSchema,
  KhalaCodeDesktopRuntimeMode,
  KhalaCodeDesktopArchitectPlanArtifact,
  KhalaCodeDesktopThreadTokenSummary,
} from "../shared/rpc"

type DesktopRpcRequests = KhalaCodeDesktopRPCSchema["requests"]
export type KhalaCodeMainShellSlashCommand =
  Awaited<ReturnType<DesktopRpcRequests["slashCommandList"]>>["commands"][number]

export type KhalaCodeFollowUpDraft = {
  id: string
  text: string
}

export type KhalaCodeBootRpcName =
  | "claudeApprovalPending"
  | "codexFleetStatus"
  | "events"
  | "fleetRunList"
  | "harnessSettingRead"
  | "sessionCatalog"

export type KhalaCodeBootDegradedState = {
  readonly dataLoss: false
  readonly detail: string
  readonly kind: "khala_code_boot_rpc_degraded"
  readonly method: KhalaCodeBootRpcName
  readonly observedAt: string
  readonly recoverable: true
  readonly state: "degraded"
}

export type KhalaCodeMainShellModel = {
  activeCodexThreadId: string | null
  bootDegradedStates: KhalaCodeBootDegradedState[]
  claudeApprovalDialogOpen: boolean
  composerAttachmentReceipts: ComposerAttachmentUploadReceipt[]
  composerState: ComposerState
  dragActive: boolean
  followUpDrafts: KhalaCodeFollowUpDraft[]
  harnessEnvOverride: KhalaCodeDesktopRuntimeMode | null
  lastResponseRuntimeMode: KhalaCodeDesktopRuntimeMode
  lastSubmittedDraft: string
  lastTurnFailed: boolean
  loadedSlashCommandKey: string
  messages: KhalaCodeDesktopMessage[]
  pendingTurn: boolean
  architectPlanArtifact: KhalaCodeDesktopArchitectPlanArtifact | null
  architectPlanMode: boolean
  architectPlanPending: boolean
  selectedHarnessMode: KhalaCodeDesktopRuntimeMode
  slashCommandLoadInFlight: Promise<void> | null
  slashCommands: KhalaCodeMainShellSlashCommand[]
  thinkingTurnId: string | null
  threadTokenPopoverOpen: boolean
  threadTokenRefreshInFlight: boolean
  threadTokenRefreshQueued: boolean
  threadTokenSummary: KhalaCodeDesktopThreadTokenSummary
  transcriptPinnedToEnd: boolean
}

export type KhalaCodeMainShellMessage =
  | { readonly _tag: "ActiveCodexThreadChanged"; readonly threadId: string | null }
  | {
      readonly _tag: "BootRpcDegraded"
      readonly state: KhalaCodeBootDegradedState
    }
  | {
      readonly _tag: "BootRpcRecovered"
      readonly method: KhalaCodeBootRpcName
    }
  | { readonly _tag: "ClaudeApprovalDialogToggled"; readonly open: boolean }
  | {
      readonly _tag: "ComposerAttachmentReceiptPushed"
      readonly receipt: ComposerAttachmentUploadReceipt
    }
  | { readonly _tag: "ComposerReceiptsReset" }
  | { readonly _tag: "ComposerStateChanged"; readonly state: ComposerState }
  | { readonly _tag: "DragActiveChanged"; readonly active: boolean }
  | {
      readonly _tag: "FollowUpDraftsChanged"
      readonly drafts: readonly KhalaCodeFollowUpDraft[]
    }
  | {
      readonly _tag: "HarnessSettingChanged"
      readonly envOverride: KhalaCodeDesktopRuntimeMode | null
      readonly mode: KhalaCodeDesktopRuntimeMode
    }
  | {
      readonly _tag: "LastResponseRuntimeModeChanged"
      readonly mode: KhalaCodeDesktopRuntimeMode
    }
  | { readonly _tag: "LastSubmittedDraftChanged"; readonly draft: string }
  | { readonly _tag: "LastTurnFailedChanged"; readonly failed: boolean }
  | {
      readonly _tag: "MessagesChanged"
      readonly messages: readonly KhalaCodeDesktopMessage[]
    }
  | { readonly _tag: "PendingTurnChanged"; readonly pending: boolean }
  | {
      readonly _tag: "ArchitectPlanArtifactChanged"
      readonly artifact: KhalaCodeDesktopArchitectPlanArtifact | null
    }
  | { readonly _tag: "ArchitectPlanModeChanged"; readonly enabled: boolean }
  | { readonly _tag: "ArchitectPlanPendingChanged"; readonly pending: boolean }
  | {
      readonly _tag: "SlashCommandLoadFinished"
      readonly commands: readonly KhalaCodeMainShellSlashCommand[]
      readonly key: string
    }
  | {
      readonly _tag: "SlashCommandLoadStarted"
      readonly key: string
      readonly promise: Promise<void>
    }
  | { readonly _tag: "SlashCommandLoadStopped" }
  | { readonly _tag: "ThinkingTurnChanged"; readonly turnId: string | null }
  | { readonly _tag: "ThreadTokenPopoverChanged"; readonly open: boolean }
  | { readonly _tag: "ThreadTokenRefreshInFlightChanged"; readonly inFlight: boolean }
  | { readonly _tag: "ThreadTokenRefreshQueuedChanged"; readonly queued: boolean }
  | {
      readonly _tag: "ThreadTokenSummaryChanged"
      readonly summary: KhalaCodeDesktopThreadTokenSummary
    }
  | { readonly _tag: "TranscriptPinnedToEndChanged"; readonly pinned: boolean }

/**
 * KS-6.8 (#8418) hot-poll migration: whether the desktop shell should run
 * its short thread-token-summary refresh interval right now. `false` means
 * no interval is scheduled at all — no ambient background polling while a
 * thread is merely open and idle. Historically the shell polled every 2s
 * any time a thread was open (even fully idle); this predicate narrows that
 * to "a turn is actually streaming for the active thread", the only window
 * where the underlying device-local usage-ledger files
 * (`codex-token-usage-telemetry.ts`) can still be gaining rows. Note this is
 * NOT a khala-sync scope migration: `threadTokenSummary` has no server
 * round trip and no matching sync scope (see the comment beside
 * `syncThreadTokenPolling` in `ui/main.ts`), so there is no scope to push
 * from — the fix is bounding the poll window itself.
 */
export const shouldPollThreadTokenSummary = (
  model: Pick<KhalaCodeMainShellModel, "pendingTurn" | "activeCodexThreadId">,
): boolean => model.pendingTurn && model.activeCodexThreadId !== null

export const initialKhalaCodeMainShellModel = (
  input: Readonly<{
    threadTokenSummary: KhalaCodeDesktopThreadTokenSummary
  }>,
): KhalaCodeMainShellModel => ({
  activeCodexThreadId: null,
  bootDegradedStates: [],
  claudeApprovalDialogOpen: false,
  composerAttachmentReceipts: [],
  composerState: emptyComposerState(),
  dragActive: false,
  followUpDrafts: [],
  harnessEnvOverride: null,
  lastResponseRuntimeMode: "codex_harness",
  lastSubmittedDraft: "",
  lastTurnFailed: false,
  loadedSlashCommandKey: "",
  messages: [],
  pendingTurn: false,
  architectPlanArtifact: null,
  architectPlanMode: false,
  architectPlanPending: false,
  selectedHarnessMode: "codex_harness",
  slashCommandLoadInFlight: null,
  slashCommands: [],
  thinkingTurnId: null,
  threadTokenPopoverOpen: false,
  threadTokenRefreshInFlight: false,
  threadTokenRefreshQueued: false,
  threadTokenSummary: input.threadTokenSummary,
  transcriptPinnedToEnd: true,
})

export const updateKhalaCodeMainShellModel = (
  model: KhalaCodeMainShellModel,
  message: KhalaCodeMainShellMessage,
): KhalaCodeMainShellModel => {
  switch (message._tag) {
    case "ActiveCodexThreadChanged":
      return { ...model, activeCodexThreadId: message.threadId }
    case "BootRpcDegraded": {
      const next = [
        ...model.bootDegradedStates.filter(item => item.method !== message.state.method),
        message.state,
      ].sort((left, right) => left.method.localeCompare(right.method))
      return { ...model, bootDegradedStates: next }
    }
    case "BootRpcRecovered":
      return {
        ...model,
        bootDegradedStates: model.bootDegradedStates.filter(item => item.method !== message.method),
      }
    case "ClaudeApprovalDialogToggled":
      return { ...model, claudeApprovalDialogOpen: message.open }
    case "ComposerAttachmentReceiptPushed":
      return {
        ...model,
        composerAttachmentReceipts: [
          ...model.composerAttachmentReceipts,
          message.receipt,
        ],
      }
    case "ComposerReceiptsReset":
      return { ...model, composerAttachmentReceipts: [] }
    case "ComposerStateChanged":
      return { ...model, composerState: message.state }
    case "DragActiveChanged":
      return { ...model, dragActive: message.active }
    case "FollowUpDraftsChanged":
      return { ...model, followUpDrafts: [...message.drafts] }
    case "HarnessSettingChanged":
      return {
        ...model,
        harnessEnvOverride: message.envOverride,
        selectedHarnessMode: message.mode,
      }
    case "LastResponseRuntimeModeChanged":
      return { ...model, lastResponseRuntimeMode: message.mode }
    case "LastSubmittedDraftChanged":
      return { ...model, lastSubmittedDraft: message.draft }
    case "LastTurnFailedChanged":
      return { ...model, lastTurnFailed: message.failed }
    case "MessagesChanged":
      return { ...model, messages: [...message.messages] }
    case "PendingTurnChanged":
      return { ...model, pendingTurn: message.pending }
    case "ArchitectPlanArtifactChanged":
      return { ...model, architectPlanArtifact: message.artifact }
    case "ArchitectPlanModeChanged":
      return { ...model, architectPlanMode: message.enabled }
    case "ArchitectPlanPendingChanged":
      return { ...model, architectPlanPending: message.pending }
    case "SlashCommandLoadFinished":
      return {
        ...model,
        loadedSlashCommandKey: message.key,
        slashCommandLoadInFlight: null,
        slashCommands: [...message.commands],
      }
    case "SlashCommandLoadStarted":
      return {
        ...model,
        loadedSlashCommandKey: message.key,
        slashCommandLoadInFlight: message.promise,
      }
    case "SlashCommandLoadStopped":
      return { ...model, slashCommandLoadInFlight: null }
    case "ThinkingTurnChanged":
      return { ...model, thinkingTurnId: message.turnId }
    case "ThreadTokenPopoverChanged":
      return { ...model, threadTokenPopoverOpen: message.open }
    case "ThreadTokenRefreshInFlightChanged":
      return { ...model, threadTokenRefreshInFlight: message.inFlight }
    case "ThreadTokenRefreshQueuedChanged":
      return { ...model, threadTokenRefreshQueued: message.queued }
    case "ThreadTokenSummaryChanged":
      return { ...model, threadTokenSummary: message.summary }
    case "TranscriptPinnedToEndChanged":
      return { ...model, transcriptPinnedToEnd: message.pinned }
  }
}
