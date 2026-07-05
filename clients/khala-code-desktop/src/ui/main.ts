import {
  DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  applyComposerTransaction,
  composerAttachmentId,
  emptyComposerState,
  offerComposerLargeTextPaste,
  planComposerAttachmentUpload,
  projectComposerAttachmentUploadReceipt,
  readyComposerAttachmentTransaction,
  retryComposerAttachmentTransaction,
  setComposerAttachmentStatusTransaction,
  stageComposerAttachmentFiles,
  stageComposerDroppedFiles,
  stageComposerPastedFiles,
  type ComposerAttachment,
  type ComposerAttachmentSource,
  type ComposerAttachmentUploadReceipt,
  type ComposerFileLike,
  type ComposerTransaction,
} from "@openagentsinc/composer-state"
import {
  commandComposerClassName,
  type CommandComposerAttachmentProps,
  type CommandComposerStatus,
} from "@openagentsinc/ui/ai-elements/command-composer"
import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import {
  commandComposerHudLayoutFromCssRect,
  createCommandComposerHud,
  type CommandComposerAttachmentProjection,
  type CommandComposerHudHandle,
} from "@openagentsinc/three-effect/core"
import { Electroview } from "electrobun/view"
import { Schema as S } from "effect"
import * as Three from "three"

import {
  KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT,
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  KhalaCodeDesktopChatTurnEventSchema,
  KhalaCodeDesktopRpcBridgeFailure,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  type KhalaCodeDesktopRpcMethodName,
  type KhalaCodeDesktopArchitectPlanArtifact,
  type KhalaCodeDesktopChatTurnAttachment,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopFleetLifecycleEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopComposerNativeFileGrant,
  type KhalaCodeDesktopFleetRunListResult,
  type KhalaCodeDesktopFleetStatus,
  type KhalaCodeDesktopKhalaSyncChatThread,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopMessageRole,
  type KhalaCodeDesktopOpenAgentsAuthPendingAttempt,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopSessionCatalogResult,
  type KhalaCodeDesktopThreadTokenSummary,
} from "../shared/rpc"
import {
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
  khalaCodeQaMetricUnitFor,
  type KhalaCodeQaMetricName,
  type KhalaCodeQaMetricSample,
  type KhalaCodeQaMetricsSnapshot,
} from "../shared/qa-metrics"
import { iconForCodexItem, renderMessageBody } from "./transcript-render"
import { mergeKhalaSyncChatAndRuntimeMessages } from "./khala-sync-thread-messages-core"
import { resolveAttachments } from "./attachment-resolution"
import {
  allTurnSteerOutcomesOk,
  steerEachTurn,
  summarizeTurnSteerOutcomes,
} from "./turn-steer-outcomes"
import { mountFleetPanel } from "./fleet-status"
import {
  mountKhalaCodeEditorPanel,
  type KhalaCodeEditorComposerContext,
} from "./editor-panel"
import {
  mountKhalaCodeReviewPanel,
  type KhalaCodeReviewComposerContext,
  type KhalaCodeReviewPanelHandle,
} from "./review-panel"
import {
  mountKhalaCodeTerminalPanel,
  type KhalaCodeTerminalPanelHandle,
} from "./terminal-panel"
import { mountKhalaCodeForumPanel } from "./forum-panel"
import { mountKhalaCodeRecoveryOverlay } from "./recovery-overlay-react"
import { mountKhalaCodePlansPanel } from "./plans-panel"
import { mountKhalaCodeRunEvidencePanel } from "./run-evidence-panel"
import { mountCodexSettingsPanel } from "./codex-settings-panel"
import { mountClaudeSettingsSection } from "./claude-settings-panel"
import { mountKhalaCodeUpdaterSettingsSection } from "./khala-code-updater-settings-section"
import {
  mountKhalaCodeProviderCatalogSettingsSection,
  type KhalaCodeProviderCatalogSettingsSectionHandle,
} from "./provider-catalog-settings-section"
import {
  mountKhalaCodeModelMcpPermissionSettingsSection,
  type KhalaCodeModelMcpPermissionSettingsSectionHandle,
} from "./model-mcp-permission-settings-section"
import {
  mountCodexThreadSidebar,
  type CodexThreadSelectionSource,
} from "./codex-thread-sidebar"
import {
  gymPaneStateFromBridgeProof,
  gymOptimizationRunFromBridgeProof,
  gymPaneStateFromLocation,
  restoredKhalaCodeViewFromLocationAndStorage,
  khalaCodeGymDemoBridgeProof,
  type KhalaGymProofLoadRequest,
} from "./gym-proof-loader"
import { mountKhalaCodeProjectHomePanel } from "./project-home-panel"
import type { KhalaGymBridgeProofLike } from "./gym-graph-projection"
import { mountGymPane, type GymPaneState } from "./gym-pane"
import {
  mountKhalaCodeSidebar,
  projectKhalaCodeSidebarFleetCounts,
  type KhalaCodeHotbarValue,
  type KhalaCodeSidebarHandle,
} from "./sidebar"
import {
  createKhalaCodeCommandRegistry,
  khalaCodeCommandForKeyboardEvent,
  type KhalaCodeCommandId,
  type KhalaCodeCommandPaletteRecord,
  type KhalaCodeCommandRegistry,
} from "./command-registry"
import {
  mountKhalaCodeCommandPalette,
  type KhalaCodeCommandPaletteHandle,
} from "./command-palette"
import {
  createKhalaCodeCommandKeybindingsSection,
  readKhalaCodeCommandKeybindingOverrides,
  writeKhalaCodeCommandKeybindingOverrides,
  type KhalaCodeCommandKeybindingsSectionHandle,
} from "./command-keybindings-panel"
import {
  mountKhalaCodeAppPreferencesSettingsSection,
  type KhalaCodeAppPreferencesSettingsSectionHandle,
} from "./app-preferences-settings-section"
import {
  mountKhalaCodeStatusUsageSettingsSection,
  type KhalaCodeStatusUsageSettingsSectionHandle,
} from "./status-usage-settings-section"
import {
  mountKhalaCodeSessionActionsSettingsSection,
  type KhalaCodeSessionActionsSettingsSectionHandle,
} from "./session-actions-settings-section"
import {
  mountKhalaCodeSupportSettingsSection,
  type KhalaCodeSupportSettingsSectionHandle,
} from "./support-settings-section"
import {
  mountKhalaCodeLocalServerManagerSettingsSection,
  type KhalaCodeLocalServerManagerSettingsSectionHandle,
} from "./local-server-manager-settings-section"
import { mountUnifiedInboxPanel } from "./inbox"
import {
  normalizeThreadTimestampSeconds,
  type KhalaCodeDesktopCodexThreadSummary,
} from "../shared/codex-threads"
import type { KhalaCodeDesktopCodexSettingsProjection } from "../shared/codex-settings"
import { sessionCatalogEntryToThreadSummary } from "../shared/session-catalog"
import {
  recentThreadCycleDirectionForEvent,
  recentThreadHotkeyIndexForEvent,
  recentThreadsForHotkeys,
} from "./thread-hotkeys"
import { bindRecentThreadHotkeyHints } from "./recent-thread-hotkey-hints"
import {
  renderThinkingIndicator,
  renderThreadLoadingIndicator,
} from "./transcript-status-indicators"
import {
  createKhalaComposerPromptHistory,
  insertKhalaComposerTextAtSelection,
  khalaRichComposerCommandForKey,
  normalizeKhalaComposerPasteText,
  readKhalaComposerPlainText,
  setKhalaComposerCaretToEnd,
  syncKhalaComposerEmptyState,
  writeKhalaComposerPlainText,
  type KhalaRichComposerMode,
} from "./rich-composer"
import {
  KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
  KhalaCodeDiffReviewSubmitDetailSchema,
  khalaCodeDiffReviewComment,
  khalaCodeDiffReviewLineLabel,
  khalaCodeDiffReviewSteeringNote,
  type KhalaCodeDiffReviewComment,
} from "../shared/diff-review"
import {
  KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT,
  KhalaCodeSourceControlActionSubmitDetailSchema,
  khalaCodeSourceControlActionLabel,
  khalaCodeSourceControlActionPrompt,
  khalaCodeSourceControlActionPromptText,
} from "../shared/source-control-action"
import {
  KHALA_CODE_MODEL_ROLE_ORDER,
  type KhalaCodeModelRole,
} from "../shared/model-roles"
import {
  applyKhalaCodeAppPreferences,
  readKhalaCodeAppPreferences,
  resetKhalaCodeAppPreferences,
  writeKhalaCodeAppPreferences,
} from "../shared/app-preferences"
import { projectKhalaCodeStatusUsage } from "../shared/status-usage"
import {
  projectKhalaCodeLocalServerManager,
  type KhalaCodeLocalServerManagerActionId,
} from "../shared/local-server-runtime"
import {
  khalaCodeSessionActionIntentFor,
  khalaCodeSessionNavigationTarget,
  projectKhalaCodeSessionActionIntents,
  readKhalaCodeClosedSessionTabs,
  recordKhalaCodeClosedSessionTab,
  removeKhalaCodeClosedSessionTab,
  type KhalaCodeSessionActionKind,
  type KhalaCodeSessionActionProjection,
} from "../shared/session-actions"
import {
  isKhalaCodeSupportUrlAllowed,
  projectKhalaCodeSupportEntrypoints,
  type KhalaCodeSupportEntrypointId,
  type KhalaCodeSupportProjection,
} from "../shared/support-entrypoints"
import {
  khalaCodeDeepLinkFromLocation,
  viewForKhalaCodeDeepLinkTarget,
} from "../shared/deep-links"
import {
  projectKhalaCodeTerminalWorkbench,
} from "../shared/terminal-workbench"
import {
  initialKhalaCodeMainShellModel,
  shouldPollThreadTokenSummary,
  updateKhalaCodeMainShellModel,
  type KhalaCodeBootDegradedState,
  type KhalaCodeBootRpcName,
  type KhalaCodeFollowUpDraft,
  type KhalaCodeMainShellMessage,
  type KhalaCodeMainShellSlashCommand,
} from "./main-shell-model"
import "./styles.css"

type DesktopRpc = ReturnType<typeof Electroview.defineRPC<KhalaCodeDesktopRPCSchema>>
type DesktopRpcRequests = KhalaCodeDesktopRPCSchema["requests"]
type ComposerReasoningModeOption = {
  readonly label: string
  readonly value: string
}

type ComposerReasoningModeState = {
  error: string | null
  loading: boolean
  saving: boolean
  settings: KhalaCodeDesktopCodexSettingsProjection | null
}

type ComposerProviderState =
  | "error"
  | "loading"
  | "managed"
  | "ready"
  | "saving"
  | "unavailable"

type ComposerContextChip = Readonly<{
  body?: string
  displayPath: string
  id: string
  kind: "comment" | "file" | "selection"
  lineEnd?: number
  lineStart?: number
  title: string
}>

const rpcFailureDetail = (payload: unknown): string => {
  if (payload !== null && typeof payload === "object") {
    const record = payload as Record<string, unknown>
    if (typeof record.detail === "string" && record.detail.length > 0) return record.detail
    if (typeof record.error === "string" && record.error.length > 0) return record.error
    if (typeof record.message === "string" && record.message.length > 0) return record.message
  }
  if (typeof payload === "string" && payload.length > 0) return payload
  return "unknown error"
}

const postPreviewRpc = async <Result>(
  method: KhalaCodeDesktopRpcMethodName,
  ...args: readonly unknown[]
): Promise<Result> => {
  const decodedArgs = decodeKhalaCodeDesktopRpcParameters(method, args)
  const response = await fetch(`/rpc/${encodeURIComponent(method)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ args: decodedArgs }),
  })
  if (!response.ok) {
    const text = await response.text()
    let payload: unknown = text
    try {
      payload = text.length === 0 ? "" : JSON.parse(text) as unknown
    } catch {
      payload = text
    }
    const failure = S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)
    try {
      const decodedFailure = failure(payload)
      throw new Error(`${method} failed with ${response.status}: ${decodedFailure.tag}: ${decodedFailure.error}`)
    } catch (error) {
      if (error instanceof Error && error.message.startsWith(`${method} failed with ${response.status}:`)) {
        throw error
      }
      throw new Error(`${method} failed with ${response.status}: ${rpcFailureDetail(payload)}`)
    }
  }
  const payload = await response.json() as unknown
  return decodeKhalaCodeDesktopRpcResult(method, payload) as Result
}

const previewRpc = (): DesktopRpc => ({
  request: {
    appInfo: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["appInfo"]>>>(
        "appInfo",
      ),
    appleFmReadiness: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["appleFmReadiness"]>>
      >("appleFmReadiness"),
    codexAccountsStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAccountsStatus"]>>
      >("codexAccountsStatus"),
    codexAppServerRestart: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerRestart"]>>
      >("codexAppServerRestart"),
    codexAppServerStart: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStart"]>>
      >("codexAppServerStart"),
    codexAppServerStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStatus"]>>
      >("codexAppServerStatus"),
    codexAppServerStop: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStop"]>>
      >("codexAppServerStop"),
    codexFleetDelegateRun: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetDelegateRun"]>>
      >("codexFleetDelegateRun", request),
    codexFleetStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetStatus"]>>
      >("codexFleetStatus"),
    codexFleetPromoteThread: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetPromoteThread"]>>
      >("codexFleetPromoteThread", request),
    fleetRunControl: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunControl"]>>
      >("fleetRunControl", request),
    fleetRunList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunList"]>>
      >("fleetRunList", request),
    fleetRunStart: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunStart"]>>
      >("fleetRunStart", request),
    fleetRunStatus: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunStatus"]>>
      >("fleetRunStatus", request),
    architectPlanRun: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["architectPlanRun"]>>
      >("architectPlanRun", request),
    architectPlanDecision: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["architectPlanDecision"]>>
      >("architectPlanDecision", request),
    fleetWorkerControl: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetWorkerControl"]>>
      >("fleetWorkerControl", request),
    khalaSyncFleetState: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncFleetState"]>>
      >("khalaSyncFleetState", request),
    khalaSyncFleetMutate: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncFleetMutate"]>>
      >("khalaSyncFleetMutate", request),
    khalaSyncFleetReportAccountState: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncFleetReportAccountState"]>>
      >("khalaSyncFleetReportAccountState", request),
    khalaSyncChatThreads: (request?: Parameters<DesktopRpcRequests["khalaSyncChatThreads"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatThreads"]>>
      >("khalaSyncChatThreads", request),
    khalaSyncChatMessages: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatMessages"]>>
      >("khalaSyncChatMessages", request),
    khalaSyncChatCreateThread: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatCreateThread"]>>
      >("khalaSyncChatCreateThread", request),
    khalaSyncChatAppendMessage: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatAppendMessage"]>>
      >("khalaSyncChatAppendMessage", request),
    khalaSyncChatRenameThread: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatRenameThread"]>>
      >("khalaSyncChatRenameThread", request),
    editorProviderList: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["editorProviderList"]>>
      >("editorProviderList"),
    editorWorkspaceRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["editorWorkspaceRead"]>>
      >("editorWorkspaceRead"),
    editorDirectoryRead: (request?: Parameters<DesktopRpcRequests["editorDirectoryRead"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["editorDirectoryRead"]>>
      >("editorDirectoryRead", request),
    editorFileRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["editorFileRead"]>>
      >("editorFileRead", request),
    reviewDiffRead: (request?: Parameters<DesktopRpcRequests["reviewDiffRead"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["reviewDiffRead"]>>
      >("reviewDiffRead", request),
    composerNativeFilePickerOpen: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeFilePickerOpen"]>>
      >("composerNativeFilePickerOpen", request),
    composerNativeDirectoryPickerOpen: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeDirectoryPickerOpen"]>>
      >("composerNativeDirectoryPickerOpen", request),
    composerNativeSaveDialogOpen: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeSaveDialogOpen"]>>
      >("composerNativeSaveDialogOpen", request),
    composerNativeClipboardImageRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeClipboardImageRead"]>>
      >("composerNativeClipboardImageRead"),
    composerNativeFileGrantRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeFileGrantRead"]>>
      >("composerNativeFileGrantRead", request),
    composerNativeFileGrantRelease: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["composerNativeFileGrantRelease"]>>
      >("composerNativeFileGrantRelease", request),
    forumRequest: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["forumRequest"]>>
      >("forumRequest", request),
    khalaCodePlanCatalog: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanCatalog"]>>
      >("khalaCodePlanCatalog"),
    khalaCodePlanStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanStatus"]>>
      >("khalaCodePlanStatus"),
    khalaCodeOpenAgentsAuthStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeOpenAgentsAuthStatus"]>>
      >("khalaCodeOpenAgentsAuthStatus"),
    khalaCodeOpenAgentsAuthStart: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeOpenAgentsAuthStart"]>>
      >("khalaCodeOpenAgentsAuthStart"),
    khalaCodeOpenAgentsAuthPoll: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeOpenAgentsAuthPoll"]>>
      >("khalaCodeOpenAgentsAuthPoll"),
    khalaCodePlanPurchase: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanPurchase"]>>
      >("khalaCodePlanPurchase", request),
    khalaCodeTraceCaptureStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeTraceCaptureStatus"]>>
      >("khalaCodeTraceCaptureStatus"),
    khalaCodeTraceCaptureConsentWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeTraceCaptureConsentWrite"]>>
      >("khalaCodeTraceCaptureConsentWrite", request),
    khalaCodeOutsideUserRunReport: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodeOutsideUserRunReport"]>>
      >("khalaCodeOutsideUserRunReport", request),
    claudeApprovalPending: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeApprovalPending"]>>
      >("claudeApprovalPending"),
    claudeApprovalRespond: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeApprovalRespond"]>>
      >("claudeApprovalRespond", request),
    claudeSettingsRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeSettingsRead"]>>
      >("claudeSettingsRead"),
    codexHarnessStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexHarnessStatus"]>>
      >("codexHarnessStatus"),
    codexApprovalRespond: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexApprovalRespond"]>>
      >("codexApprovalRespond", request),
    codexBackgroundTerminalsClean: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsClean"]>>
      >("codexBackgroundTerminalsClean", request),
    codexBackgroundTerminalsList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsList"]>>
      >("codexBackgroundTerminalsList", request),
    codexBackgroundTerminalsTerminate: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsTerminate"]>>
      >("codexBackgroundTerminalsTerminate", request),
    codexConfigValueWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexConfigValueWrite"]>>
      >("codexConfigValueWrite", request),
    codexEcosystemRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexEcosystemRead"]>>
      >("codexEcosystemRead", request),
    codexExternalAgentConfigDetect: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigDetect"]>>
      >("codexExternalAgentConfigDetect", request),
    codexExternalAgentConfigImport: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigImport"]>>
      >("codexExternalAgentConfigImport", request),
    codexExternalAgentConfigImportHistoriesRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigImportHistoriesRead"]>>
      >("codexExternalAgentConfigImportHistoriesRead"),
    codexFsGetMetadata: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsGetMetadata"]>>
      >("codexFsGetMetadata", request),
    codexFsReadFile: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsReadFile"]>>
      >("codexFsReadFile", request),
    codexFsWriteFile: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsWriteFile"]>>
      >("codexFsWriteFile", request),
    codexMarketplaceAdd: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceAdd"]>>
      >("codexMarketplaceAdd", request),
    codexMarketplaceRemove: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceRemove"]>>
      >("codexMarketplaceRemove", request),
    codexMarketplaceUpgrade: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceUpgrade"]>>
      >("codexMarketplaceUpgrade", request),
    codexMentionCandidates: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMentionCandidates"]>>
      >("codexMentionCandidates", request),
    codexMcpOauthLogin: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpOauthLogin"]>>
      >("codexMcpOauthLogin", request),
    codexMcpResourceRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpResourceRead"]>>
      >("codexMcpResourceRead", request),
    codexMcpServerReload: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpServerReload"]>>
      >("codexMcpServerReload"),
    codexMcpToolCall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpToolCall"]>>
      >("codexMcpToolCall", request),
    codexPluginInstall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexPluginInstall"]>>
      >("codexPluginInstall", request),
    codexPluginUninstall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexPluginUninstall"]>>
      >("codexPluginUninstall", request),
    codexModelRolePresetApply: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexModelRolePresetApply"]>>
      >("codexModelRolePresetApply", request),
    codexSettingsRead: (request?: Parameters<DesktopRpcRequests["codexSettingsRead"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSettingsRead"]>>
      >("codexSettingsRead", request),
    codexSkillsConfigWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSkillsConfigWrite"]>>
      >("codexSkillsConfigWrite", request),
    codexSkillsExtraRootsSet: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSkillsExtraRootsSet"]>>
      >("codexSkillsExtraRootsSet", request),
    codexThreadArchive: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadArchive"]>>
      >("codexThreadArchive", request),
    codexThreadCompact: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadCompact"]>>
      >("codexThreadCompact", request),
    codexThreadDelete: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadDelete"]>>
      >("codexThreadDelete", request),
    codexThreadFork: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadFork"]>>
      >("codexThreadFork", request),
    codexThreadList: (request?: Parameters<DesktopRpcRequests["codexThreadList"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadList"]>>
      >("codexThreadList", request),
    codexThreadRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadRead"]>>
      >("codexThreadRead", request),
    codexThreadRename: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadRename"]>>
      >("codexThreadRename", request),
    codexThreadResume: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadResume"]>>
      >("codexThreadResume", request),
    codexThreadStart: (request?: Parameters<DesktopRpcRequests["codexThreadStart"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadStart"]>>
      >("codexThreadStart", request),
    codexThreadUnarchive: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadUnarchive"]>>
      >("codexThreadUnarchive", request),
    sessionCatalog: (request?: Parameters<DesktopRpcRequests["sessionCatalog"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>
      >("sessionCatalog", request),
    codexTurnInterrupt: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnInterrupt"]>>
      >("codexTurnInterrupt", request),
    codexTurnStart: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnStart"]>>
      >("codexTurnStart", request),
    codexTurnSteer: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnSteer"]>>
      >("codexTurnSteer", request),
    codingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codingStatus"]>>
      >("codingStatus"),
    connectCodexAccount: accountRef =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["connectCodexAccount"]>>
      >("connectCodexAccount", accountRef),
    harnessSettingRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["harnessSettingRead"]>>
      >("harnessSettingRead"),
    harnessSettingWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["harnessSettingWrite"]>>
      >("harnessSettingWrite", request),
    modelRoleRegistryRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["modelRoleRegistryRead"]>>
      >("modelRoleRegistryRead"),
    modelRoleRegistryWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["modelRoleRegistryWrite"]>>
      >("modelRoleRegistryWrite", request),
    openExternalUrl: url =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["openExternalUrl"]>>
      >("openExternalUrl", url),
    consumeCodexRateLimitResetCredit: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["consumeCodexRateLimitResetCredit"]>>
      >("consumeCodexRateLimitResetCredit", request),
    onDeviceDeciderStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["onDeviceDeciderStatus"]>>
      >("onDeviceDeciderStatus"),
    pylonStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["pylonStatus"]>>
      >("pylonStatus"),
    qaMetricSample: sample =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["qaMetricSample"]>>
      >("qaMetricSample", sample),
    qaMetrics: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["qaMetrics"]>>
      >("qaMetrics"),
    removeCodexAccount: accountRef =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["removeCodexAccount"]>>
      >("removeCodexAccount", accountRef),
    setCodexAccountPaused: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["setCodexAccountPaused"]>>
      >("setCodexAccountPaused", request),
    slashCommandDispatch: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>
      >("slashCommandDispatch", request),
    slashCommandList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["slashCommandList"]>>
      >("slashCommandList", request),
    submitChatMessage: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["submitChatMessage"]>>
      >("submitChatMessage", request),
    tokenAccountingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["tokenAccountingStatus"]>>
      >("tokenAccountingStatus"),
    threadTokenSummary: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["threadTokenSummary"]>>
      >("threadTokenSummary", request),
    toolCatalog: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["toolCatalog"]>>>(
        "toolCatalog",
      ),
    updaterCheck: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["updaterCheck"]>>>(
        "updaterCheck",
      ),
    updaterDownload: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["updaterDownload"]>>>(
        "updaterDownload",
      ),
    updaterInstall: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["updaterInstall"]>>>(
        "updaterInstall",
      ),
    updaterStatus: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["updaterStatus"]>>>(
        "updaterStatus",
      ),
    diagnosticsSnapshot: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["diagnosticsSnapshot"]>>>(
        "diagnosticsSnapshot",
      ),
    diagnosticsExport: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["diagnosticsExport"]>>>(
        "diagnosticsExport",
      ),
    diagnosticsRendererHeartbeat: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["diagnosticsRendererHeartbeat"]>>>(
        "diagnosticsRendererHeartbeat",
      ),
    diagnosticsReportRendererFatalError: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["diagnosticsReportRendererFatalError"]>>
      >("diagnosticsReportRendererFatalError", request),
    diagnosticsRelaunch: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["diagnosticsRelaunch"]>>>(
        "diagnosticsRelaunch",
      ),
    diagnosticsQuit: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["diagnosticsQuit"]>>>(
        "diagnosticsQuit",
      ),
  },
  send: {
    chatTurnEvent: () => undefined,
    fleetLifecycleEvent: () => undefined,
    claudeApprovalRequested: () => undefined,
    diagnosticsRecoveryStateChanged: () => undefined,
  },
})

const createAsyncLineQueue = (): {
  readonly iterable: () => AsyncIterable<string>
  readonly push: (line: string) => void
} => {
  const values: string[] = []
  const waiters: Array<(value: IteratorResult<string>) => void> = []
  const push = (line: string): void => {
    const waiter = waiters.shift()
    if (waiter === undefined) {
      values.push(line)
      return
    }
    waiter({ done: false, value: line })
  }
  return {
    iterable: async function* () {
      while (true) {
        if (values.length > 0) {
          yield values.shift()!
          continue
        }
        yield await new Promise<string>(resolve => {
          waiters.push(result => {
            if (!result.done) resolve(result.value)
          })
        })
      }
    },
    push,
  }
}

const fleetLifecycleLines = createAsyncLineQueue()
const applyFleetLifecycleEvent = (event: KhalaCodeDesktopFleetLifecycleEvent): void => {
  fleetLifecycleLines.push(event.line)
}

const decodeChatTurnEvent = (input: unknown): KhalaCodeDesktopChatTurnEvent =>
  S.decodeUnknownSync(KhalaCodeDesktopChatTurnEventSchema as never)(input) as KhalaCodeDesktopChatTurnEvent

const startPreviewBridgeEvents = (): void => {
  if (!isKhalaPreviewWindow) return
  let eventSource: EventSource
  try {
    eventSource = new EventSource("/rpc/events")
  } catch (error) {
    recordBootRpcDegradedState("events", error)
    return
  }
  eventSource.addEventListener("open", () => clearBootRpcDegradedState("events"))
  eventSource.addEventListener("error", () => {
    recordBootRpcDegradedState("events", new Error("preview event stream unavailable"))
  })
  eventSource.addEventListener("chatTurnEvent", event => {
    try {
      const payload = JSON.parse((event as MessageEvent<string>).data) as {
        readonly event?: unknown
      }
      applyChatTurnEvent(decodeChatTurnEvent(payload.event))
    } catch {
      // Ignore malformed preview diagnostics; native RPC still carries typed events.
    }
  })
  eventSource.addEventListener("fleetLifecycleEvent", event => {
    try {
      const payload = JSON.parse((event as MessageEvent<string>).data) as {
        readonly detail?: { readonly line?: unknown }
      }
      if (typeof payload.detail?.line === "string") {
        fleetLifecycleLines.push(payload.detail.line)
      }
    } catch {
      // Ignore malformed preview diagnostics; lifecycle decoding is lossy-safe.
    }
  })
  eventSource.addEventListener("claudeApprovalRequested", () => {
    // KS-6.9 (#8419): preview windows share the same push signal as native
    // windows; the underlying claudeApprovalPending poll below is the
    // fallback that still applies if this event is missed.
    void pollClaudeApprovals()
  })
}

// Diagnostics/recovery surface (issue #8441). Forward-declared: the RPC
// message handler below needs to call this before `rpc` (and the mounted
// recovery overlay that owns the real implementation) exist.
let handleRecoveryStateChanged: (
  state: Parameters<KhalaCodeDesktopRPCSchema["messages"]["diagnosticsRecoveryStateChanged"]>[0],
) => void = () => {}

const nativeRpc = Electroview.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {
      chatTurnEvent(event) {
        applyChatTurnEvent(event)
      },
      fleetLifecycleEvent(event) {
        applyFleetLifecycleEvent(event)
      },
      claudeApprovalRequested() {
        // KS-6.9 (#8419): react to the push immediately instead of waiting
        // for the next 1s claudeApprovalPending poll tick. The poll stays
        // registered below as a fallback safety net.
        void pollClaudeApprovals()
      },
      diagnosticsRecoveryStateChanged(state) {
        handleRecoveryStateChanged(state)
      },
    },
  },
})

const currentPort = Number(globalThis.location?.port ?? "0")
const isKhalaPreviewWindow =
  globalThis.location?.protocol === "http:" &&
  Number.isInteger(currentPort) &&
  currentPort >= KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT &&
  currentPort < KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT + 10

// Only the explicit Khala preview server handles /rpc HTTP fallbacks. Native
// Electrobun internal ports also run on localhost, but they only accept socket
// RPC and will log noisy fallthroughs for /rpc/* requests.
const rpc = isKhalaPreviewWindow ? previewRpc() : nativeRpc
startPreviewBridgeEvents()
if (!isKhalaPreviewWindow) {
  new Electroview({ rpc: nativeRpc })
}

// Diagnostics/debug-log export and native-shell recovery UX (issue #8441):
// mount the recovery overlay, wire its three actions to the real RPCs, wire
// a renderer heartbeat so the main-process unresponsive watchdog has a
// signal to key off of, and forward fatal renderer errors into the local
// diagnostics log store so they show up in an exported debug-log archive.
const recoveryOverlayRoot = document.getElementById("recovery-overlay-root")
if (recoveryOverlayRoot !== null) {
  const recoveryOverlay = mountKhalaCodeRecoveryOverlay(recoveryOverlayRoot, {
    dispatch: {
      exportDebugLogs: async () => {
        const result = await rpc.request.diagnosticsExport()
        if (!result.ok) throw new Error(result.error)
        return { path: result.path }
      },
      quit: async () => {
        await rpc.request.diagnosticsQuit()
      },
      relaunch: async () => {
        await rpc.request.diagnosticsRelaunch()
      },
    },
  })
  handleRecoveryStateChanged = state => {
    if (state.kind === "none") {
      recoveryOverlay.hide()
      return
    }
    recoveryOverlay.show(state)
  }
  // Visual-smoke/test hook only: lets the diagnostics recovery visual smoke
  // (scripts/diagnostics-recovery-visual-smoke.ts) force each recovery state
  // without a real native watchdog/load-failure signal. Calls the same
  // `.show`/`.hide` handle a real push message would use.
  ;(globalThis as unknown as {
    __khalaCodeRecoveryOverlayTestHook?: typeof recoveryOverlay
  }).__khalaCodeRecoveryOverlayTestHook = recoveryOverlay

  const RENDERER_HEARTBEAT_INTERVAL_MS = 5_000
  setInterval(() => {
    void rpc.request.diagnosticsRendererHeartbeat().catch(() => {
      // Best-effort — a failed heartbeat call just means the next interval
      // tick retries; it must never throw into a global handler.
    })
  }, RENDERER_HEARTBEAT_INTERVAL_MS)

  const reportRendererFatalError = (
    kind: "error" | "unhandledrejection",
    message: string,
    stack?: string,
  ): void => {
    void rpc.request.diagnosticsReportRendererFatalError({
      kind,
      message,
      ...(stack === undefined ? {} : { stack }),
    }).catch(() => {
      // Best-effort — must never throw from an error handler.
    })
  }
  globalThis.addEventListener("error", (event: ErrorEvent) => {
    const error = event.error as unknown
    reportRendererFatalError(
      "error",
      event.message,
      error instanceof Error ? error.stack : undefined,
    )
  })
  globalThis.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) => {
    const reason = event.reason as unknown
    const message = reason instanceof Error ? reason.message : String(reason)
    reportRendererFatalError("unhandledrejection", message, reason instanceof Error ? reason.stack : undefined)
  })
}

const composerClasses = {
  attachment: commandComposerClassName("attachment"),
  attachmentAction: commandComposerClassName("attachmentAction"),
  dropcursor: commandComposerClassName("dropcursor"),
  status: commandComposerClassName("status"),
} as const

type ComposerIconName =
  | "attach"
  | "code"
  | "file"
  | "image"
  | "menu"
  | "microphone"
  | "model"
  | "plus"
  | "preview"
  | "remove"
  | "retry"
  | "send"
  | "settings"
  | "stop"
  | "steer"
  | "text"

const composerIconCatalog = {
  attach: "Paperclip",
  code: "Code",
  file: "File",
  image: "FileImage",
  menu: "DotsHorizontalMoreMenu",
  microphone: "Mic",
  model: "Bolt",
  plus: "Plus",
  preview: "Eye",
  remove: "Trash",
  retry: "ArrowRotateCcw",
  send: "ArrowUp",
  settings: "Settings",
  stop: "Stop",
  steer: "Reply",
  text: "Text",
} satisfies Record<ComposerIconName, IconName>

const composerIconElement = (name: ComposerIconName): HTMLSpanElement => {
  const icon = iconElement(composerIconCatalog[name], {
    className: "oa-ai-command-composer-icon",
    dataIcon: name,
  })
  icon.dataset.oaCommandComposerIcon = name
  return icon
}

const attachmentIconName = (
  kind: CommandComposerAttachmentProps["kind"],
): ComposerIconName =>
  kind === "image"
    ? "image"
    : kind === "text"
      ? "text"
      : kind === "snippet"
        ? "code"
        : "file"

const setButtonIcon = (
  button: HTMLButtonElement,
  name: ComposerIconName,
): void => {
  const current = button.querySelector(".oa-ai-command-composer-icon")
  const next = composerIconElement(name)
  if (current === null) {
    button.prepend(next)
    return
  }
  current.replaceWith(next)
}

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector)
  if (element === null) throw new Error(`Missing ${selector}`)
  return element
}

const messageList = requireElement<HTMLElement>("#message-list")
const threadTokenMeter = requireElement<HTMLElement>("#thread-token-meter")
const threadTokenCounter = requireElement<HTMLButtonElement>("#thread-token-counter")
const threadTokenCounterValue = requireElement<HTMLElement>("#thread-token-counter-value")
const threadTokenPopover = requireElement<HTMLElement>("#thread-token-popover")
const composerForm = requireElement<HTMLFormElement>("#composer-form")
const composerFrame = requireElement<HTMLElement>("#composer-frame")
const composerHudMount = requireElement<HTMLElement>("#composer-hud")
const composerFollowUpQueue = requireElement<HTMLElement>("#composer-follow-up-queue")
const composerInput = requireElement<HTMLElement>("#composer-input")
const composerRail = requireElement<HTMLElement>("#composer-rail")
const slashCommandPalette = requireElement<HTMLElement>("#slash-command-palette")
const commandPaletteRoot = requireElement<HTMLElement>("#command-palette-root")
const composerPreview = requireElement<HTMLElement>("#composer-preview")
const composerControls = requireElement<HTMLElement>("#composer-controls")
const composerStatus = requireElement<HTMLElement>("#composer-status")
const composerA11y = requireElement<HTMLElement>("#composer-a11y")
const sendButton = requireElement<HTMLButtonElement>("#send-button")
const attachButton = requireElement<HTMLButtonElement>("#attach-button")
const fileInput = requireElement<HTMLInputElement>("#file-input")
const composerPromptHistory = createKhalaComposerPromptHistory()
let composerMode: KhalaRichComposerMode = "normal"
let composerIsComposing = false

const activeTurnIds = new Set<string>()
/**
 * turnId -> threadId (or null for a not-yet-created thread), captured at
 * submit time. Unlike activeTurnIds, this is never cleared by navigation —
 * only when the owning turn genuinely finishes — so a background thread's
 * streaming state survives switching away and back
 * (khala_code.chat.streaming_indicator_survives_navigation.v1,
 * khala_code.transcript.streaming_state_cross_surface_consistency.v1).
 */
const streamingThreadIds = new Map<string, string | null>()
const activeTurnStartTimes = new Map<string, number>()
const turnFirstVisibleEventRecorded = new Set<string>()
const objectUrls = new Set<string>()
const localTextAttachments = new Map<string, string>()
const localAttachmentFiles = new Map<string, File>()
const localNativeAttachmentGrantIds = new Map<string, string>()
const localNativeAttachmentGrantLabels = new Map<string, KhalaCodeDesktopComposerNativeFileGrant>()
let composerContextChips: ComposerContextChip[] = []
const sessionIdStorageKey = "khala-code-desktop.session-id.v1"
const activeThreadIdStorageKey = "khala-code-desktop.active-thread-id.v1"
const storedSessionId = localStorage.getItem(sessionIdStorageKey)
const bootDeepLink = khalaCodeDeepLinkFromLocation(globalThis.location)
const sessionId =
  storedSessionId?.startsWith("khala-code-desktop-") === true
    ? storedSessionId
    : `khala-code-desktop-${Date.now().toString(36)}`
localStorage.setItem(sessionIdStorageKey, sessionId)
/**
 * The thread that was active when the app last quit, restored on boot
 * (khala_code.app.resumes_after_restart.v1). NOT cleared here — normal
 * `setActiveCodexThreadId` calls during the session keep the stored value in
 * sync, and a failed restore clears it explicitly (see
 * `restoreActiveThreadAfterRestart`) so a stale/deleted thread id does not
 * retry forever.
 */
const bootRestoreThreadId =
  bootDeepLink?.ok === true && bootDeepLink.target.kind === "thread"
    ? bootDeepLink.target.threadId
    : localStorage.getItem(activeThreadIdStorageKey)
const commandKeybindingOverrides: Partial<Record<KhalaCodeCommandId, string>> = {
  ...readKhalaCodeCommandKeybindingOverrides(localStorage),
}
applyKhalaCodeAppPreferences(
  document.documentElement,
  readKhalaCodeAppPreferences(localStorage),
)

const persistCommandKeybindingOverrides = (): void =>
  writeKhalaCodeCommandKeybindingOverrides(localStorage, commandKeybindingOverrides)

type ThreadSwitchPerformanceSample = {
  cacheHit: boolean
  fullMessageCount?: number
  fullRenderMs?: number
  hydratedRenderMs?: number
  optimisticMessageCount: number
  optimisticRenderMs?: number
  rpcMs?: number
  selectionId: number
  source: CodexThreadSelectionSource
  startedAt: number
  threadId: string
}

const THREAD_MESSAGE_CACHE_LIMIT = 16
const THREAD_PREFETCH_LIMIT = 4
const THREAD_LIST_CACHE_TTL_MS = 2000
const THREAD_SWITCH_INITIAL_MESSAGE_LIMIT = 80
const THREAD_SWITCH_FULL_HYDRATION_TIMEOUT_MS = 80
const THREAD_SWITCH_PERFORMANCE_SAMPLE_LIMIT = 60
const QA_METRIC_SAMPLE_LIMIT = 240
const TRANSCRIPT_SCROLL_SAMPLE_FRAME_COUNT = 12
const threadMessageCache = new Map<string, readonly KhalaCodeDesktopMessage[]>()
const threadPrefetches = new Map<string, Promise<void>>()
const khalaSyncChatThreadIds = new Set<string>()
const threadListCache = new Map<string, {
  readonly cachedAt: number
  readonly result: Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>
}>()
const threadListRequests = new Map<string, Promise<Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>>>()
const threadSwitchPerformanceSamples: ThreadSwitchPerformanceSample[] = []
const pendingThreadSwitches = new Map<number, ThreadSwitchPerformanceSample>()
const qaMetricSamples: KhalaCodeQaMetricSample[] = []

const pushQaMetricSample = (
  metric: KhalaCodeQaMetricName,
  value: number,
  context?: KhalaCodeQaMetricSample["context"],
): void => {
  if (!Number.isFinite(value)) return
  const sample: KhalaCodeQaMetricSample = {
    ...(context === undefined ? {} : { context }),
    metric,
    observedAt: new Date().toISOString(),
    unit: khalaCodeQaMetricUnitFor(metric),
    value,
  }
  qaMetricSamples.push(sample)
  while (qaMetricSamples.length > QA_METRIC_SAMPLE_LIMIT) qaMetricSamples.shift()
  void rpc.request.qaMetricSample(sample).catch(() => undefined)
}

const markQaTimer = (
  metric: KhalaCodeQaMetricName,
  startedAt: number,
  context?: KhalaCodeQaMetricSample["context"],
): void => {
  requestAnimationFrame(() => pushQaMetricSample(metric, performance.now() - startedAt, context))
}

let transcriptScrollSampleInFlight = false

const sampleTranscriptScrollDroppedFrames = (
  source: "keyboard" | "wheel",
): void => {
  if (transcriptScrollSampleInFlight) return
  transcriptScrollSampleInFlight = true
  let previousFrameAt = performance.now()
  let frameCount = 0
  let droppedFrameCount = 0
  const step = (frameAt: number): void => {
    if (frameCount > 0 && frameAt - previousFrameAt > 24) droppedFrameCount += 1
    previousFrameAt = frameAt
    frameCount += 1
    if (frameCount >= TRANSCRIPT_SCROLL_SAMPLE_FRAME_COUNT) {
      const measuredFrames = Math.max(1, frameCount - 1)
      pushQaMetricSample(
        "transcript.scroll_dropped_frames_pct",
        (droppedFrameCount / measuredFrames) * 100,
        { droppedFrames: droppedFrameCount, frames: measuredFrames, source },
      )
      transcriptScrollSampleInFlight = false
      return
    }
    requestAnimationFrame(step)
  }
  requestAnimationFrame(step)
}

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

const degradedSessionCatalog = (
  state: KhalaCodeBootDegradedState,
): KhalaCodeDesktopSessionCatalogResult => ({
  diagnostics: [`qa.boot_rpc.${state.method}.degraded: ${state.detail}`],
  entries: [],
  ok: true,
  schemaVersion: "khala-code-desktop.session-catalog.v1",
  scope: "app",
})

const degradedFleetStatus = (
  state: KhalaCodeBootDegradedState,
): KhalaCodeDesktopFleetStatus => ({
  accounts: [],
  activeAssignments: [],
  availableCodexAssignments: 0,
  maxCodexAssignments: 0,
  observedAt: state.observedAt,
  ok: false,
  processes: [],
  pylon: {
    message: `qa.boot_rpc.${state.method}.degraded: ${state.detail}`,
    pylonRef: null,
    status: "unavailable",
  },
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: `qa.boot_rpc.${state.method}.degraded`,
  },
})

const degradedFleetRunList = (): KhalaCodeDesktopFleetRunListResult => ({
  ok: false,
  runs: [],
})

const cloneMessages = (
  value: readonly KhalaCodeDesktopMessage[],
): KhalaCodeDesktopMessage[] =>
  value.map(message => ({ ...message }))

const cacheThreadMessages = (
  threadId: string,
  value: readonly KhalaCodeDesktopMessage[],
): void => {
  if (value.length === 0) return
  threadMessageCache.delete(threadId)
  threadMessageCache.set(threadId, cloneMessages(value))
  while (threadMessageCache.size > THREAD_MESSAGE_CACHE_LIMIT) {
    const oldest = threadMessageCache.keys().next().value
    if (typeof oldest !== "string") break
    threadMessageCache.delete(oldest)
  }
}

const cachedThreadMessages = (
  threadId: string,
): KhalaCodeDesktopMessage[] | null => {
  const cached = threadMessageCache.get(threadId)
  if (cached === undefined) return null
  cacheThreadMessages(threadId, cached)
  return cloneMessages(cached)
}

const cacheVisibleThreadMessages = (): void => {
  const threadId = shellModel().activeCodexThreadId
  if (threadId === null) return
  cacheThreadMessages(threadId, shellModel().messages)
}

const recentMessagesForInitialThreadRender = (
  value: readonly KhalaCodeDesktopMessage[],
): KhalaCodeDesktopMessage[] => {
  if (value.length <= THREAD_SWITCH_INITIAL_MESSAGE_LIMIT) return cloneMessages(value)
  return cloneMessages(value.slice(-THREAD_SWITCH_INITIAL_MESSAGE_LIMIT))
}

const pushThreadSwitchPerformanceSample = (
  sample: ThreadSwitchPerformanceSample,
): void => {
  threadSwitchPerformanceSamples.push(sample)
  while (threadSwitchPerformanceSamples.length > THREAD_SWITCH_PERFORMANCE_SAMPLE_LIMIT) {
    threadSwitchPerformanceSamples.shift()
  }
}

const markThreadSwitchPaint = (
  selectionId: number,
  field: "fullRenderMs" | "hydratedRenderMs" | "optimisticRenderMs",
): void => {
  requestAnimationFrame(() => {
    const sample = pendingThreadSwitches.get(selectionId)
    if (sample === undefined) return
    const durationMs = performance.now() - sample.startedAt
    sample[field] = durationMs
    const metric =
      field === "fullRenderMs"
        ? "thread_switch.full_render_ms"
        : field === "hydratedRenderMs"
          ? "thread_switch.hydrated_render_ms"
          : "thread_switch.optimistic_render_ms"
    pushQaMetricSample(metric, durationMs, { threadId: sample.threadId })
    if (field !== "hydratedRenderMs") return
    pendingThreadSwitches.delete(selectionId)
  })
}

const beginThreadSwitchPerformanceSample = (
  input: {
    readonly cacheHit: boolean
    readonly optimisticMessageCount: number
    readonly selectionId: number
    readonly source: CodexThreadSelectionSource
    readonly threadId: string
  },
): void => {
  const sample: ThreadSwitchPerformanceSample = {
    cacheHit: input.cacheHit,
    optimisticMessageCount: input.optimisticMessageCount,
    selectionId: input.selectionId,
    source: input.source,
    startedAt: performance.now(),
    threadId: input.threadId,
  }
  pendingThreadSwitches.set(input.selectionId, sample)
  pushThreadSwitchPerformanceSample(sample)
  if (input.cacheHit) pushQaMetricSample("cache.hit", 1, { threadId: input.threadId })
  markThreadSwitchPaint(input.selectionId, "optimisticRenderMs")
}

const completeThreadSwitchPerformanceSample = (
  input: {
    readonly fullMessageCount: number
    readonly selectionId?: number
  },
): void => {
  if (input.selectionId === undefined) return
  const sample = pendingThreadSwitches.get(input.selectionId)
  if (sample === undefined) return
  sample.fullMessageCount = input.fullMessageCount
  sample.rpcMs = performance.now() - sample.startedAt
  pushQaMetricSample("thread_switch.rpc_ms", sample.rpcMs, { threadId: sample.threadId })
  markThreadSwitchPaint(input.selectionId, "fullRenderMs")
}

const qaMetricsSnapshot = (): KhalaCodeQaMetricsSnapshot => {
  const samples = [...qaMetricSamples]
  return {
    budgets: khalaCodeQaMetricBudgets,
    definitions: khalaCodeQaMetricDefinitions,
    evaluations: evaluateKhalaCodeQaMetricBudgets(samples),
    ok: true,
    observedAt: new Date().toISOString(),
    samples,
    schema: "openagents.khala_code.qa_metrics.v1",
  }
}

const scheduleFullThreadHydration = (
  input: {
    readonly fullMessages: readonly KhalaCodeDesktopMessage[]
    readonly selectionId?: number
    readonly threadId: string
    readonly visibleMessages: readonly KhalaCodeDesktopMessage[]
  },
): void => {
  if (input.fullMessages.length === input.visibleMessages.length) {
    if (input.selectionId !== undefined) {
      const selectionId = input.selectionId
      requestAnimationFrame(() => pendingThreadSwitches.delete(selectionId))
    }
    return
  }
  const hydrate = (): void => {
    if (shellModel().activeCodexThreadId !== input.threadId || shellModel().messages !== input.visibleMessages) return
    setShellMessages(cloneMessages(input.fullMessages))
    render()
    if (input.selectionId !== undefined) {
      markThreadSwitchPaint(input.selectionId, "hydratedRenderMs")
    }
  }
  let hydrated = false
  const hydrateOnce = (): void => {
    if (hydrated) return
    hydrated = true
    hydrate()
  }
  const deadline = window.setTimeout(hydrateOnce, THREAD_SWITCH_FULL_HYDRATION_TIMEOUT_MS)
  requestAnimationFrame(() => {
    window.clearTimeout(deadline)
    hydrateOnce()
  })
}

const mainShellStore = {
  model: initialKhalaCodeMainShellModel({
    threadTokenSummary: emptyThreadTokenSummary(null),
  }),
}

const dispatchMainShell = (message: KhalaCodeMainShellMessage): void => {
  mainShellStore.model = updateKhalaCodeMainShellModel(
    mainShellStore.model,
    message,
  )
}

const shellModel = (): typeof mainShellStore.model => mainShellStore.model

const setShellMessages = (
  messages: readonly KhalaCodeDesktopMessage[],
): void => dispatchMainShell({ _tag: "MessagesChanged", messages })

const setShellComposerState = (
  state: typeof mainShellStore.model.composerState,
): void => dispatchMainShell({ _tag: "ComposerStateChanged", state })

const setShellFollowUpDrafts = (
  drafts: readonly KhalaCodeFollowUpDraft[],
): void => dispatchMainShell({ _tag: "FollowUpDraftsChanged", drafts })

const setArchitectPlanArtifact = (
  artifact: KhalaCodeDesktopArchitectPlanArtifact | null,
): void => dispatchMainShell({ _tag: "ArchitectPlanArtifactChanged", artifact })

const setArchitectPlanMode = (enabled: boolean): void =>
  dispatchMainShell({ _tag: "ArchitectPlanModeChanged", enabled })

const setArchitectPlanPending = (pending: boolean): void =>
  dispatchMainShell({ _tag: "ArchitectPlanPendingChanged", pending })

const composerReasoningModeState: ComposerReasoningModeState = {
  error: null,
  loading: false,
  saving: false,
  settings: null,
}
let composerAgentRole: KhalaCodeModelRole = "coder"

const formatReasoningModeLabel = (value: string): string =>
  value.length === 0
    ? "Default"
    : value
      .split(/[-_]/u)
      .map(part => part.length === 0 ? part : `${part[0]?.toUpperCase() ?? ""}${part.slice(1)}`)
      .join(" ")

const formatComposerControlLabel = (value: string): string =>
  formatReasoningModeLabel(value).replace(/\bGpt\b/gu, "GPT")

const composerSettings = (): KhalaCodeDesktopCodexSettingsProjection | null =>
  composerReasoningModeState.settings

const composerProviderState = (): ComposerProviderState => {
  const settings = composerSettings()
  if (composerReasoningModeState.loading) return "loading"
  if (composerReasoningModeState.saving) return "saving"
  if (composerReasoningModeState.error !== null || (settings?.errors.length ?? 0) > 0) return "error"
  if (settings === null || settings.providers.options.length === 0) return "unavailable"
  if (settings.requirements.managed && settings.config.originKeys.includes("model")) return "managed"
  return "ready"
}

const composerSelectedProvider = ():
  KhalaCodeDesktopCodexSettingsProjection["providers"]["selected"] => {
  const settings = composerSettings()
  if (settings === null) return null
  return settings.providers.selected ??
    (settings.models.selected?.providerId === null || settings.models.selected?.providerId === undefined
      ? null
      : settings.providers.options.find(provider => provider.id === settings.models.selected?.providerId) ?? null)
}

const composerProviderValue = (): string =>
  composerSelectedProvider()?.id ?? composerSettings()?.config.modelProvider ?? ""

const composerModelOptions = ():
  readonly KhalaCodeDesktopCodexSettingsProjection["models"]["options"][number][] => {
  const settings = composerSettings()
  if (settings === null) return []
  const provider = composerProviderValue()
  if (provider.length === 0) return settings.models.options
  const matching = settings.models.options.filter(model => model.providerId === provider)
  return matching.length > 0 ? matching : settings.models.options
}

const composerSelectedModel = ():
  KhalaCodeDesktopCodexSettingsProjection["models"]["selected"] => {
  const settings = composerSettings()
  if (settings === null) return null
  const modelValue = settings.config.model ?? settings.models.selected?.model ?? ""
  return composerModelOptions().find(model => model.model === modelValue || model.id === modelValue) ??
    settings.models.selected ??
    composerModelOptions()[0] ??
    null
}

const composerModelValue = (): string =>
  composerSelectedModel()?.model ?? composerSettings()?.config.model ?? ""

const composerVariantOptions = (): readonly {
  readonly description: string | null
  readonly id: string
  readonly name: string
}[] => composerSelectedModel()?.serviceTiers ?? []

const composerVariantValue = (): string => {
  const settings = composerSettings()
  const selected = composerSelectedModel()
  return settings?.config.serviceTier ??
    selected?.defaultServiceTier ??
    composerVariantOptions()[0]?.id ??
    ""
}

const composerVariantLabel = (): string => {
  const value = composerVariantValue()
  const option = composerVariantOptions().find(item => item.id === value)
  return option?.name ?? (value.length === 0 ? "Default" : formatComposerControlLabel(value))
}

const composerUsageA11yText = (): string => {
  const settings = composerSettings()
  if (settings === null) return "Provider usage unavailable."
  if (!settings.usage.available) return "Provider usage unavailable."
  return "Provider usage available."
}

const composerTurnSelection = (): KhalaCodeDesktopChatTurnRequest["composerSelection"] => {
  const provider = composerSelectedProvider()
  const model = composerSelectedModel()
  const reasoningEffort = composerReasoningModeValue()
  const variant = composerVariantValue()
  return {
    agentRole: composerAgentRole,
    model: model?.model ?? null,
    modelProvider: provider?.id ?? model?.providerId ?? composerSettings()?.config.modelProvider ?? null,
    providerDisplayName: provider?.displayName ?? model?.providerDisplayName ?? null,
    reasoningEffort: reasoningEffort.length === 0 ? null : reasoningEffort,
    serviceTier: variant.length === 0 ? null : variant,
    variant: variant.length === 0 ? null : variant,
    runtimeAdapter: "codex_app_server",
  }
}

const composerReasoningModeValue = (): string => {
  const settings = composerReasoningModeState.settings
  if (settings === null) return ""
  return settings.config.reasoningEffort ??
    settings.models.selected?.defaultReasoningEffort ??
    settings.models.selected?.supportedReasoningEfforts[0]?.value ??
    ""
}

const composerReasoningModeOptions = (): readonly ComposerReasoningModeOption[] => {
  const settings = composerReasoningModeState.settings
  if (settings === null) {
    return [{
      label: composerReasoningModeState.loading ? "Loading" : "Unavailable",
      value: "",
    }]
  }
  const selectedValue = composerReasoningModeValue()
  const reasoningEfforts = settings.models.selected?.supportedReasoningEfforts ?? []
  const options = reasoningEfforts.reduce<Array<ComposerReasoningModeOption>>(
    (accumulator, option) =>
      option.value.length === 0 || accumulator.some(item => item.value === option.value)
        ? accumulator
        : [
            ...accumulator,
            { label: formatReasoningModeLabel(option.value), value: option.value },
          ],
    [],
  )
  return selectedValue.length > 0 && !options.some(option => option.value === selectedValue)
    ? [...options, { label: formatReasoningModeLabel(selectedValue), value: selectedValue }]
    : options
}

const composerReasoningModeDisabled = (): boolean => {
  const settings = composerReasoningModeState.settings
  if (composerReasoningModeState.loading || composerReasoningModeState.saving || settings === null) return true
  if (composerProviderState() === "managed") return true
  return composerReasoningModeOptions().length === 0
}

const composerReasoningModeTitle = (): string => {
  if (composerReasoningModeState.loading) return "Loading reasoning modes"
  if (composerReasoningModeState.saving) return "Saving reasoning mode"
  if (composerReasoningModeState.error !== null) return composerReasoningModeState.error
  if (composerReasoningModeDisabled()) return "Reasoning modes unavailable for this model"
  return "Reasoning mode"
}

const composerReasoningModeA11yText = (): string => {
  if (composerReasoningModeState.loading) return "Reasoning loading."
  if (composerReasoningModeState.error !== null) return "Reasoning unavailable."
  return `Reasoning ${formatReasoningModeLabel(composerReasoningModeValue())}.`
}

const loadComposerReasoningModes = async (): Promise<void> => {
  composerReasoningModeState.loading = true
  composerReasoningModeState.error = null
  try {
    const settings = await controls.codexSettingsRead({ includeHiddenModels: true })
    composerReasoningModeState.settings = settings
    composerReasoningModeState.error = settings.ok ? null : settings.errors.join("\n") || "Codex settings unavailable"
  } catch (error) {
    composerReasoningModeState.error = error instanceof Error ? error.message : String(error)
  } finally {
    composerReasoningModeState.loading = false
    renderComposer()
  }
}

const ensureComposerReasoningModesLoaded = (): void => {
  if (
    composerReasoningModeState.settings !== null ||
    composerReasoningModeState.loading ||
    composerReasoningModeState.error !== null
  ) {
    return
  }
  void loadComposerReasoningModes()
}

const writeComposerConfigValue = async (
  keyPath: string,
  value: null | string,
  errorText: string,
): Promise<void> => {
  composerReasoningModeState.saving = true
  composerReasoningModeState.error = null
  renderComposer()
  try {
    const result = await controls.codexConfigValueWrite({
      keyPath,
      value,
    })
    if (result.ok) {
      if (result.settings !== undefined) {
        composerReasoningModeState.settings = result.settings
      } else {
        composerReasoningModeState.settings = await controls.codexSettingsRead({ includeHiddenModels: true })
      }
      return
    }
    composerReasoningModeState.error = result.error ?? errorText
  } catch (error) {
    composerReasoningModeState.error = error instanceof Error ? error.message : String(error)
  } finally {
    composerReasoningModeState.saving = false
    renderComposer()
    requestAnimationFrame(focusComposerInput)
  }
}

const writeComposerReasoningMode = async (value: string): Promise<void> =>
  writeComposerConfigValue(
    "model_reasoning_effort",
    value.length === 0 ? null : value,
    "Failed to save reasoning mode",
  )

const writeComposerModel = async (value: string): Promise<void> =>
  writeComposerConfigValue("model", value.length === 0 ? null : value, "Failed to save model")

const writeComposerProvider = async (value: string): Promise<void> =>
  writeComposerConfigValue(
    "model_provider",
    value.length === 0 ? null : value,
    "Failed to save provider",
  )

const cycleComposerVariant = async (): Promise<void> => {
  const variants = composerVariantOptions()
  if (variants.length <= 1) return
  const current = composerVariantValue()
  const index = Math.max(0, variants.findIndex(variant => variant.id === current))
  const next = variants[(index + 1) % variants.length]
  if (next === undefined) return
  await writeComposerConfigValue("service_tier", next.id, "Failed to save model variant")
}

const setComposerAgentRole = (role: KhalaCodeModelRole): void => {
  composerAgentRole = role
  renderComposer()
  requestAnimationFrame(focusComposerInput)
}

const bootFailureDetail = (error: unknown): string =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "boot RPC failed"

const recordBootRpcDegradedState = (
  method: KhalaCodeBootRpcName,
  error: unknown,
): KhalaCodeBootDegradedState => {
  const state: KhalaCodeBootDegradedState = {
    dataLoss: false,
    detail: bootFailureDetail(error),
    kind: "khala_code_boot_rpc_degraded",
    method,
    observedAt: new Date().toISOString(),
    recoverable: true,
    state: "degraded",
  }
  dispatchMainShell({ _tag: "BootRpcDegraded", state })
  renderBootDegradedStates()
  return state
}

const clearBootRpcDegradedState = (method: KhalaCodeBootRpcName): void => {
  dispatchMainShell({ _tag: "BootRpcRecovered", method })
  renderBootDegradedStates()
}

const compactTokenFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 1,
  notation: "compact",
})
const exactTokenFormatter = new Intl.NumberFormat("en-US")
const tokenTimestampFormatter = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  month: "short",
  day: "numeric",
})

const prefersReducedMotion =
  typeof matchMedia === "function"
    ? matchMedia("(prefers-reduced-motion: reduce)")
    : null

type ComposerHudRuntime = Readonly<{
  renderer: Three.WebGLRenderer
  scene: Three.Scene
  camera: Three.OrthographicCamera
  handle: CommandComposerHudHandle
  dispose: () => void
}>

let composerHudRuntime: ComposerHudRuntime | null = null

const messageClass = (role: KhalaCodeDesktopMessageRole): string =>
  `message-bubble message-bubble--${role}`

const KHALA_OPENAGENTS_MISSING_TOKEN_MARKER =
  "does not have an OPENAGENTS_AGENT_TOKEN"

type KhalaOpenAgentsConnectView = Readonly<{
  error?: string
  pendingAttempt?: KhalaCodeDesktopOpenAgentsAuthPendingAttempt
  state: "idle" | "starting" | "pending" | "polling" | "linked" | "failed"
  tokenPrefix?: string
}>

let khalaOpenAgentsConnectView: KhalaOpenAgentsConnectView = {
  state: "idle",
}
let khalaOpenAgentsConnectPollTimer: number | null = null

const isKhalaOpenAgentsMissingTokenMessage = (
  message: KhalaCodeDesktopMessage,
): boolean =>
  message.role === "assistant" &&
  message.body.includes(KHALA_OPENAGENTS_MISSING_TOKEN_MARKER)

const setKhalaOpenAgentsConnectView = (
  view: KhalaOpenAgentsConnectView,
): void => {
  khalaOpenAgentsConnectView = view
  renderMessages()
}

const clearKhalaOpenAgentsConnectPollTimer = (): void => {
  if (khalaOpenAgentsConnectPollTimer === null) return
  window.clearTimeout(khalaOpenAgentsConnectPollTimer)
  khalaOpenAgentsConnectPollTimer = null
}

const scheduleKhalaOpenAgentsConnectPoll = (delayMs: number): void => {
  clearKhalaOpenAgentsConnectPollTimer()
  khalaOpenAgentsConnectPollTimer = window.setTimeout(() => {
    void pollKhalaOpenAgentsConnect()
  }, delayMs)
}

const startKhalaOpenAgentsConnect = async (): Promise<void> => {
  clearKhalaOpenAgentsConnectPollTimer()
  setKhalaOpenAgentsConnectView({ state: "starting" })
  try {
    const existing = await rpc.request.khalaCodeOpenAgentsAuthStatus()
    if (existing.state === "connected") {
      setKhalaOpenAgentsConnectView({
        state: "linked",
        ...(existing.tokenPrefix === null ? {} : { tokenPrefix: existing.tokenPrefix }),
      })
      return
    }

    const result = await rpc.request.khalaCodeOpenAgentsAuthStart()
    if (!result.ok) {
      setKhalaOpenAgentsConnectView({
        error: "Khala Code could not start the OpenAgents connect flow.",
        state: "failed",
      })
      return
    }

    const pendingAttempt = {
      attemptId: result.attemptId,
      expiresAt: result.expiresAt,
      intervalSeconds: result.intervalSeconds,
      userCode: result.userCode,
      verificationUrl: result.verificationUrl,
    }
    setKhalaOpenAgentsConnectView({
      pendingAttempt,
      state: "pending",
    })
    void rpc.request.openExternalUrl(pendingAttempt.verificationUrl)
    scheduleKhalaOpenAgentsConnectPoll(
      Math.max(1, pendingAttempt.intervalSeconds) * 1000,
    )
  } catch (error) {
    setKhalaOpenAgentsConnectView({
      error: error instanceof Error ? error.message : String(error),
      state: "failed",
    })
  }
}

async function pollKhalaOpenAgentsConnect(): Promise<void> {
  const current = khalaOpenAgentsConnectView
  if (current.state !== "pending" && current.state !== "polling") return
  setKhalaOpenAgentsConnectView({
    ...current,
    state: "polling",
  })
  try {
    const result = await rpc.request.khalaCodeOpenAgentsAuthPoll()
    if (!result.ok) {
      setKhalaOpenAgentsConnectView({
        error: "Khala Code could not finish the OpenAgents connect flow.",
        state: "failed",
      })
      return
    }
    if (result.status === "pending") {
      const pendingAttempt = {
        attemptId: result.attemptId,
        expiresAt: result.expiresAt,
        intervalSeconds: result.intervalSeconds,
        userCode: result.userCode,
        verificationUrl: result.verificationUrl,
      }
      setKhalaOpenAgentsConnectView({
        pendingAttempt,
        state: "pending",
      })
      scheduleKhalaOpenAgentsConnectPoll(
        Math.max(1, pendingAttempt.intervalSeconds) * 1000,
      )
      return
    }
    if (result.status === "expired") {
      setKhalaOpenAgentsConnectView({
        error: "The OpenAgents connect code expired.",
        state: "failed",
      })
      return
    }
    clearKhalaOpenAgentsConnectPollTimer()
    setKhalaOpenAgentsConnectView({
      state: "linked",
      tokenPrefix: result.tokenPrefix,
    })
    appendMessages([{
      body: "Khala Code is connected to OpenAgents. Send the message again to continue.",
      id: nextMessageId("system"),
      role: "system",
    }])
  } catch (error) {
    setKhalaOpenAgentsConnectView({
      error: error instanceof Error ? error.message : String(error),
      state: "failed",
    })
  }
}

const renderKhalaOpenAgentsConnectPanel = (): HTMLElement => {
  const panel = document.createElement("section")
  panel.className = "khala-openagents-connect"
  panel.setAttribute("aria-label", "Connect Khala Code to OpenAgents")

  const title = document.createElement("p")
  title.className = "khala-openagents-connect-title"
  title.textContent = "Connect OpenAgents"

  const hint = document.createElement("p")
  hint.className = "khala-openagents-connect-hint"

  const actionRow = document.createElement("div")
  actionRow.className = "khala-openagents-connect-actions"

  const button = document.createElement("button")
  button.type = "button"
  button.className = "khala-openagents-connect-button"

  const view = khalaOpenAgentsConnectView
  if (view.state === "linked") {
    hint.textContent = view.tokenPrefix === undefined
      ? "Connected. Send the message again to continue."
      : `Connected with token ${view.tokenPrefix}... Send the message again to continue.`
    button.textContent = "Connected"
    button.disabled = true
  } else if (view.state === "starting") {
    hint.textContent = "Opening the OpenAgents sign-in flow."
    button.textContent = "Opening..."
    button.disabled = true
  } else if (view.state === "pending" || view.state === "polling") {
    const attempt = view.pendingAttempt
    hint.textContent = view.state === "polling"
      ? "Checking whether the browser confirmation finished."
      : "Confirm the code in your browser, then Khala Code will save the token locally."
    if (attempt !== undefined) {
      const code = document.createElement("code")
      code.className = "khala-openagents-connect-code"
      code.textContent = attempt.userCode
      actionRow.append(code)

      const link = document.createElement("button")
      link.type = "button"
      link.className = "khala-openagents-connect-secondary"
      link.textContent = "Open link"
      link.addEventListener("click", () => {
        void rpc.request.openExternalUrl(attempt.verificationUrl)
      })
      actionRow.append(link)
    }
    button.textContent = view.state === "polling" ? "Checking..." : "Check now"
    button.disabled = view.state === "polling"
    button.addEventListener("click", () => {
      void pollKhalaOpenAgentsConnect()
    })
  } else {
    hint.textContent = view.state === "failed" && view.error !== undefined
      ? view.error
      : "Sign in once and Khala Code will save an OpenAgents token for future launches."
    button.textContent = view.state === "failed" ? "Try again" : "Connect"
    button.addEventListener("click", () => {
      void startKhalaOpenAgentsConnect()
    })
  }

  actionRow.prepend(button)
  panel.append(title, hint, actionRow)
  return panel
}

const renderMessage = (message: KhalaCodeDesktopMessage): HTMLElement => {
  const article = document.createElement("article")
  article.className = messageClass(message.role)
  article.dataset.messageId = message.id
  if (message.codexItem !== undefined) {
    article.dataset.codexItemId = message.codexItem.itemId
    article.dataset.codexItemType = message.codexItem.itemType
    article.dataset.codexItemStatus = message.codexItem.status
  }

  const body = document.createElement("div")
  body.className = "message-body"
  body.append(...renderMessageBody(message.body, message.role, message.codexItem))
  if (isKhalaOpenAgentsMissingTokenMessage(message)) {
    body.append(renderKhalaOpenAgentsConnectPanel())
  }

  article.append(body)
  return article
}

const isToolCallMessage = (message: KhalaCodeDesktopMessage): boolean =>
  message.codexItem !== undefined

/**
 * Consecutive tool-call messages collapse into one line showing the latest
 * call; expanding reveals the full run, each still individually expandable
 * via its own card (khala_code.transcript.consecutive_tool_calls_collapsed.v1).
 */
const groupConsecutiveToolCallMessages = (
  messages: readonly KhalaCodeDesktopMessage[],
): ReadonlyArray<readonly KhalaCodeDesktopMessage[]> => {
  const groups: Array<readonly KhalaCodeDesktopMessage[]> = []
  for (const message of messages) {
    const last = groups.at(-1)
    if (isToolCallMessage(message) && last !== undefined && last.every(isToolCallMessage)) {
      groups[groups.length - 1] = [...last, message]
      continue
    }
    groups.push([message])
  }
  return groups
}

const renderToolCallGroupSummary = (
  messages: readonly KhalaCodeDesktopMessage[],
): HTMLElement => {
  const container = document.createElement("div")
  container.className = "tool-call-group"
  container.dataset.count = String(messages.length)
  container.dataset.expanded = "false"

  const latest = messages[messages.length - 1]
  const latestTitle = latest?.codexItem?.title ?? latest?.body ?? ""

  const summary = document.createElement("button")
  summary.type = "button"
  summary.className = "tool-call-group-summary"
  summary.setAttribute("aria-expanded", "false")
  summary.setAttribute("aria-label", `${messages.length} tool calls, latest: ${latestTitle}`)

  const icon = iconElement(iconForCodexItem(latest?.codexItem?.itemType ?? ""), {
    ariaHidden: true,
    className: "tool-call-group-summary-icon",
  })

  const label = document.createElement("span")
  label.className = "tool-call-group-summary-label"
  label.textContent = latestTitle

  const chevron = document.createElement("span")
  chevron.className = "tool-call-group-summary-chevron"
  chevron.setAttribute("aria-hidden", "true")

  const count = document.createElement("span")
  count.className = "tool-call-group-summary-count"
  count.textContent = String(messages.length)

  summary.append(icon, label, count, chevron)

  const items = document.createElement("div")
  items.className = "tool-call-group-items"
  items.hidden = true
  items.append(...messages.map(renderMessage))

  summary.addEventListener("click", () => {
    const expanded = container.dataset.expanded === "true"
    container.dataset.expanded = expanded ? "false" : "true"
    summary.setAttribute("aria-expanded", expanded ? "false" : "true")
    items.hidden = expanded
  })

  container.append(summary, items)
  return container
}

const renderTranscriptMessages = (
  messages: readonly KhalaCodeDesktopMessage[],
): readonly HTMLElement[] =>
  groupConsecutiveToolCallMessages(messages).flatMap(group =>
    group.length > 1 ? [renderToolCallGroupSummary(group)] : group.map(renderMessage)
  )

const refreshHarnessSetting = async (): Promise<void> => {
  try {
    const setting = await rpc.request.harnessSettingRead()
    shellModel().selectedHarnessMode = setting.mode
    shellModel().harnessEnvOverride = setting.envOverride
    shellModel().lastResponseRuntimeMode = setting.mode
    clearBootRpcDegradedState("harnessSettingRead")
  } catch {
    recordBootRpcDegradedState("harnessSettingRead", "harness setting unavailable; using Codex harness defaults")
    shellModel().selectedHarnessMode = "codex_harness"
    shellModel().harnessEnvOverride = null
    shellModel().lastResponseRuntimeMode = "codex_harness"
  }
  renderComposer()
}

void refreshHarnessSetting()

/*
const setHarnessMode = async (mode: KhalaCodeDesktopRuntimeMode): Promise<void> => {
  shellModel().selectedHarnessMode = mode
  renderComposer()
  try {
    const setting = await rpc.request.harnessSettingWrite({ mode })
    shellModel().selectedHarnessMode = setting.mode
    shellModel().harnessEnvOverride = setting.envOverride
  } catch {
    shellModel().selectedHarnessMode = mode
  }
  renderComposer()
}

const harnessOptions: readonly {
  readonly label: string
  readonly mode: KhalaCodeDesktopRuntimeMode
}[] = [
  { label: "Codex", mode: "codex_harness" },
  { label: "Claude", mode: "claude_runtime" },
  { label: "Khala", mode: "khala_native_runtime" },
]

const harnessLabel = (mode: KhalaCodeDesktopRuntimeMode): string =>
  harnessOptions.find(option => option.mode === mode)?.label ?? "Codex"

const renderHarnessPill = (): HTMLElement => {
  const pill = document.createElement("div")
  pill.className = "khala-harness-pill"
  pill.dataset.envOverride = shellModel().harnessEnvOverride === null ? "false" : "true"
  pill.setAttribute("role", "group")
  pill.setAttribute("aria-label", "Composer mode")
  for (const option of harnessOptions) {
    const button = document.createElement("button")
    button.type = "button"
    button.className = "khala-harness-pill-button"
    button.dataset.active = shellModel().selectedHarnessMode === option.mode ? "true" : "false"
    button.title = `Use ${option.label} runtime`
    button.setAttribute("aria-label", `Use ${option.label} runtime`)
    const label = document.createElement("span")
    label.className = "khala-harness-pill-label"
    label.textContent = option.mode === "codex_harness" ? "Custom" : option.label
    const chevron = document.createElement("span")
    chevron.className = "khala-selector-chevron"
    chevron.setAttribute("aria-hidden", "true")
    button.replaceChildren(composerIconElement("settings"), label, chevron)
    button.disabled = shellModel().harnessEnvOverride !== null
    button.addEventListener("click", () => void setHarnessMode(option.mode))
    pill.append(button)
  }
  return pill
}

const renderRuntimeBadge = (): HTMLElement => {
  const badge = document.createElement("span")
  badge.className = "khala-runtime-badge"
  badge.dataset.runtimeMode = shellModel().lastResponseRuntimeMode
  badge.title = `Runtime: ${harnessLabel(shellModel().lastResponseRuntimeMode)}`
  const label = document.createElement("span")
  label.textContent = "5.5 Light"
  const chevron = document.createElement("span")
  chevron.className = "khala-selector-chevron"
  chevron.setAttribute("aria-hidden", "true")
  badge.replaceChildren(composerIconElement("model"), label, chevron)
  return badge
}

const renderMicrophoneIndicator = (): HTMLElement => {
  const indicator = document.createElement("span")
  indicator.className = "khala-microphone-indicator"
  indicator.title = "Voice input"
  indicator.setAttribute("aria-label", "Voice input")
  indicator.replaceChildren(composerIconElement("microphone"))
  return indicator
}
*/

let threadSwitchLoadingSelectionId: number | null = null

const isNearTranscriptEnd = (): boolean =>
  messageList.scrollHeight - messageList.scrollTop - messageList.clientHeight <= 48

const maxTranscriptScrollTop = (): number =>
  Math.max(0, messageList.scrollHeight - messageList.clientHeight)

const canScrollTranscript = (): boolean =>
  maxTranscriptScrollTop() > 1

const scrollToEnd = (behavior: ScrollBehavior = "auto"): void => {
  shellModel().transcriptPinnedToEnd = true
  messageList.scrollTo({
    top: messageList.scrollHeight,
    behavior,
  })
}

const setTranscriptScrollTop = (top: number): void => {
  messageList.scrollTop = Math.max(0, Math.min(top, maxTranscriptScrollTop()))
  shellModel().transcriptPinnedToEnd = isNearTranscriptEnd()
}

const wheelDeltaPixels = (event: WheelEvent): number => {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return event.deltaY * 16
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return event.deltaY * messageList.clientHeight
  return event.deltaY
}

const isComposerScrollTarget = (target: EventTarget | null): boolean =>
  target instanceof Element &&
  target.closest("#composer-form, #composer-input, input, textarea, select, button, [contenteditable]") !== null

const proxyTranscriptWheel = (event: WheelEvent): void => {
  if (event.defaultPrevented || isComposerScrollTarget(event.target) || !canScrollTranscript()) return
  const before = messageList.scrollTop
  setTranscriptScrollTop(before + wheelDeltaPixels(event))
  if (messageList.scrollTop !== before) {
    sampleTranscriptScrollDroppedFrames("wheel")
    event.preventDefault()
  }
}

const proxyTranscriptKeyScroll = (event: KeyboardEvent): void => {
  if (event.defaultPrevented || isComposerScrollTarget(event.target) || !canScrollTranscript()) return
  const page = Math.max(80, Math.floor(messageList.clientHeight * 0.82))
  const delta =
    event.key === "PageDown" || event.key === " "
      ? page
      : event.key === "PageUp"
        ? -page
        : event.key === "ArrowDown"
          ? 40
          : event.key === "ArrowUp"
            ? -40
            : null
  if (delta === null) return
  const before = messageList.scrollTop
  setTranscriptScrollTop(before + delta)
  if (messageList.scrollTop !== before) {
    sampleTranscriptScrollDroppedFrames("keyboard")
    event.preventDefault()
  }
}

const isThreadStreaming = (threadId: string | null): boolean => {
  if (threadId === null) return false
  for (const streamingThreadId of streamingThreadIds.values()) {
    if (streamingThreadId === threadId) return true
  }
  return false
}

const recomputePendingTurnForActiveThread = (): void => {
  shellModel().pendingTurn = isThreadStreaming(shellModel().activeCodexThreadId)
  syncThreadTokenPolling()
}

const statusForComposer = (): CommandComposerStatus => {
  if (shellModel().pendingTurn) return "streaming"
  if (shellModel().lastTurnFailed) return "error"
  return "ready"
}

const statusLabelFor = (status: CommandComposerStatus): string => {
  if (status === "submitted") return "Submitted"
  if (status === "streaming") return "Streaming"
  if (status === "error") return "Needs attention"
  return "Ready"
}

const formatBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB"] as const
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  const digits = value >= 10 || unit === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${units[unit]}`
}

const formatCompactTokens = (tokens: number): string =>
  tokens >= 10_000
    ? compactTokenFormatter.format(tokens)
    : exactTokenFormatter.format(Math.max(0, Math.trunc(tokens)))

const formatExactTokens = (tokens: number): string =>
  exactTokenFormatter.format(Math.max(0, Math.trunc(tokens)))

const formatThreadTokenUpdatedAt = (value: string | null): string => {
  if (value === null) return "Not recorded"
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return "Not recorded"
  return tokenTimestampFormatter.format(date)
}

const appendThreadTokenRow = (
  root: HTMLElement,
  label: string,
  value: string,
): void => {
  const row = document.createElement("div")
  row.className = "khala-thread-token-popover-row"

  const term = document.createElement("dt")
  term.textContent = label

  const definition = document.createElement("dd")
  definition.textContent = value

  row.append(term, definition)
  root.append(row)
}

const formatThreadRoleEconomics = (
  role: KhalaCodeDesktopThreadTokenSummary["roleEconomics"][number],
): string => {
  if (role.pricingState === "subscription_covered") return "subscription-covered"
  if (role.pricingState === "not_measured") return "not_measured"
  const amount = role.costAmount === null ? 0 : role.costAmount
  const currency = role.costCurrency ?? "USD"
  return `${new Intl.NumberFormat("en-US", {
    currency,
    maximumFractionDigits: 4,
    minimumFractionDigits: 2,
    style: "currency",
  }).format(amount)} measured`
}

const renderThreadTokenCounter = (): void => {
  const summary = shellModel().threadTokenSummary
  threadTokenCounterValue.textContent = formatCompactTokens(summary.totalTokens)
  threadTokenCounter.setAttribute(
    "aria-label",
    `${formatExactTokens(summary.totalTokens)} thread tokens, ${formatExactTokens(summary.leaderboardSyncedTokens)} synced to leaderboard`,
  )
  threadTokenCounter.setAttribute("aria-expanded", String(shellModel().threadTokenPopoverOpen))
  threadTokenPopover.hidden = !shellModel().threadTokenPopoverOpen
  if (!shellModel().threadTokenPopoverOpen) return

  const title = document.createElement("div")
  title.className = "khala-thread-token-popover-title"
  title.textContent = summary.threadId === null ? "No active thread" : "Thread tokens"

  const rows = document.createElement("dl")
  rows.className = "khala-thread-token-popover-grid"
  appendThreadTokenRow(rows, "Total local", formatExactTokens(summary.totalTokens))
  appendThreadTokenRow(
    rows,
    "Leaderboard synced",
    formatExactTokens(summary.leaderboardSyncedTokens),
  )
  appendThreadTokenRow(rows, "Pending sync", formatExactTokens(summary.pendingSyncTokens))
  appendThreadTokenRow(rows, "Audit turns", exactTokenFormatter.format(summary.auditRows))
  appendThreadTokenRow(rows, "Usage events", exactTokenFormatter.format(summary.usageEventRows))
  for (const role of summary.roleEconomics) {
    appendThreadTokenRow(
      rows,
      `${role.roleRef} economics`,
      `${formatThreadRoleEconomics(role)} · ${formatExactTokens(role.tokens)}`,
    )
  }
  if (summary.codexStateTokens > 0) {
    appendThreadTokenRow(
      rows,
      summary.totalTokens === 0 && summary.auditRows === 0 && summary.usageEventRows === 0
        ? "Codex state only"
        : "Codex state",
      formatExactTokens(summary.codexStateTokens),
    )
  }
  if (summary.missingUsageTurns > 0) {
    appendThreadTokenRow(
      rows,
      "Missing usage",
      exactTokenFormatter.format(summary.missingUsageTurns),
    )
  }

  const meta = document.createElement("div")
  meta.className = "khala-thread-token-popover-meta"
  const codexOnlyState =
    summary.codexStateTokens > 0 &&
    summary.totalTokens === 0 &&
    summary.auditRows === 0 &&
    summary.usageEventRows === 0
  meta.textContent = codexOnlyState
    ? "Not Khala Code usage"
    : summary.remoteDisabled
    ? "Sync disabled"
    : summary.remoteConfigured
      ? `Updated ${formatThreadTokenUpdatedAt(summary.updatedAt)}`
      : "Remote sync not configured"

  threadTokenPopover.replaceChildren(title, rows, meta)
}

const refreshThreadTokenSummary = async (): Promise<void> => {
  const threadId = shellModel().activeCodexThreadId
  if (threadId === null) {
    shellModel().threadTokenSummary = emptyThreadTokenSummary(null)
    renderThreadTokenCounter()
    return
  }
  if (shellModel().threadTokenRefreshInFlight) {
    shellModel().threadTokenRefreshQueued = true
    return
  }

  shellModel().threadTokenRefreshInFlight = true
  try {
    const summary = await rpc.request.threadTokenSummary({ threadId })
    if (shellModel().activeCodexThreadId === threadId) {
      shellModel().threadTokenSummary = summary
      renderThreadTokenCounter()
    }
  } catch {
    if (shellModel().activeCodexThreadId === threadId) {
      shellModel().threadTokenSummary = {
        ...emptyThreadTokenSummary(threadId),
        totalTokens: shellModel().threadTokenSummary.threadId === threadId
          ? shellModel().threadTokenSummary.totalTokens
          : 0,
      }
      renderThreadTokenCounter()
    }
  } finally {
    shellModel().threadTokenRefreshInFlight = false
    if (shellModel().threadTokenRefreshQueued) {
      shellModel().threadTokenRefreshQueued = false
      void refreshThreadTokenSummary()
    }
  }
}

// KS-6.8 (#8418) hot-poll migration finding: `threadTokenSummary` is NOT a
// khala-sync scope candidate. `readKhalaCodeDesktopThreadTokenSummary`
// (codex-token-usage-telemetry.ts) reads exclusively device-local JSONL
// ledgers and a local Codex-state SQLite DB — there is no server round trip
// and no matching `scope.thread.<id>`/`scope.user.<id>` entity (the KS-8.13
// thread/message contracts in `packages/khala-sync/src/khala-code.ts` carry
// no token/usage fields). This is exactly the "device-local codex telemetry"
// class the cleanup audit's own §6.3 already excludes from sync
// consolidation, which contradicts §6.2 item 6's blanket claim that this
// poll "maps cleanly" onto a sync scope — see the doc correction alongside
// this change. The honest fix is to stop polling BLINDLY: only run the
// short refresh interval while a turn is actively streaming for the active
// thread (when local ledger writes can actually still be landing), and do
// one more refresh the moment the turn stops so trailing usage writes are
// still caught without an ambient always-on timer.
let threadTokenPollTimer = 0
let threadTokenPollActive = false

const syncThreadTokenPolling = (): void => {
  const shouldPoll = shouldPollThreadTokenSummary(shellModel())
  if (shouldPoll === threadTokenPollActive) return
  threadTokenPollActive = shouldPoll
  window.clearInterval(threadTokenPollTimer)
  threadTokenPollTimer = 0
  if (shouldPoll) {
    threadTokenPollTimer = window.setInterval(() => {
      if (document.hidden) return
      void refreshThreadTokenSummary()
    }, 2_000)
    return
  }
  // Turn just stopped (completed, interrupted, or thread switched away from
  // a streaming thread): one trailing refresh catches usage-ledger writes
  // that land just after the turn's RPC promise resolves.
  void refreshThreadTokenSummary()
}

const attachmentPropsFor = (
  attachment: ComposerAttachment,
): CommandComposerAttachmentProps => ({
  id: attachment.id,
  kind: attachment.kind,
  name: attachment.name,
  mime: attachment.mime,
  sizeBytes: attachment.sizeBytes,
  sizeLabel: formatBytes(attachment.sizeBytes),
  status: attachment.status,
  ...(attachment.previewUrl === undefined
    ? {}
    : { previewUrl: attachment.previewUrl }),
  ...(attachment.dimensions === undefined
    ? {}
    : { dimensions: attachment.dimensions }),
  ...(attachment.contentRef === undefined
    ? {}
    : { contentRef: attachment.contentRef }),
  ...(attachment.source === undefined ? {} : { source: attachment.source }),
  ...(attachment.errorText === undefined
    ? {}
    : { errorText: attachment.errorText }),
})

const statusTextForAttachment = (
  status: CommandComposerAttachmentProps["status"],
): string => {
  if (status === "uploading") return "Uploading"
  if (status === "ready") return "Ready"
  if (status === "error") return "Error"
  return "Staged"
}

const composerDraftText = (): string =>
  readKhalaComposerPlainText(composerInput)

const composerDraftTrimmed = (): string =>
  composerDraftText().trim()

const setComposerDraftText = (value: string): void => {
  writeKhalaComposerPlainText(composerInput, value)
}

const syncComposerDraftState = (): void => {
  syncKhalaComposerEmptyState(composerInput)
  composerInput.dataset.oaCommandComposerMode = composerMode
  composerInput.dataset.composerMode = composerMode
  composerForm.dataset.oaCommandComposerMode = composerMode
}

const stageComposerText = (value: string): void => {
  const existing = composerDraftText().trimEnd()
  setComposerDraftText(existing.length === 0 ? value : `${existing}\n\n${value}`)
  shellModel().lastTurnFailed = false
  renderComposer()
  requestAnimationFrame(focusComposerInput)
}

const canSubmitComposer = (): boolean =>
  composerDraftTrimmed() !== "" ||
  shellModel().composerState.doc.attachments.length > 0 ||
  composerContextChips.length > 0 ||
  (shellModel().lastTurnFailed && shellModel().lastSubmittedDraft.trim() !== "")

const renderMessages = (): void => {
  cacheVisibleThreadMessages()
  const stickToEnd = shellModel().transcriptPinnedToEnd && isNearTranscriptEnd()
  const previousScrollTop = messageList.scrollTop
  const thinking = renderThinkingIndicator(shellModel().thinkingTurnId)
  const threadLoading = renderThreadLoadingIndicator(threadSwitchLoadingSelectionId)
  messageList.replaceChildren(
    ...renderTranscriptMessages(shellModel().messages),
    ...(threadLoading === null ? [] : [threadLoading]),
    ...(thinking === null ? [] : [thinking]),
  )
  requestAnimationFrame(() => {
    if (stickToEnd) {
      scrollToEnd()
    } else {
      setTranscriptScrollTop(previousScrollTop)
    }
  })
}

const buttonLabel = (status: CommandComposerStatus): string => {
  if (status === "streaming" || status === "submitted") return "Stop"
  if (status === "error" && composerDraftTrimmed() === "") return "Retry"
  return "Send"
}

const updateComposerHudProjection = (): void => {
  if (composerHudRuntime === null) return
  const attachments: ReadonlyArray<CommandComposerAttachmentProjection> =
    shellModel().composerState.doc.attachments.map((attachment) => ({
      id: attachment.id,
      kind:
        attachment.kind === "snippet"
          ? "code"
          : attachment.kind === "file"
            ? "file"
            : attachment.kind,
      status: attachment.status,
      selected:
        shellModel().composerState.selection.selectedAttachmentId === attachment.id,
    }))
  composerHudRuntime.handle.setProjection({
    focused: document.activeElement === composerInput,
    dragActive: shellModel().dragActive,
    reducedMotion: prefersReducedMotion?.matches === true,
    attachments,
    dropcursor: {
      visible: shellModel().dragActive,
      x: 0,
      intensity: shellModel().dragActive ? 0.9 : 0,
    },
  })
}

const renderAttachmentAction = (
  label: string,
  action: "preview" | "retry" | "remove",
  attachment: CommandComposerAttachmentProps,
): HTMLButtonElement => {
  const button = document.createElement("button")
  button.type = "button"
  button.className = composerClasses.attachmentAction
  button.title = label
  button.setAttribute("aria-label", `${label} attachment: ${attachment.name}`)
  button.dataset.oaCommandComposerAttachmentAction = action
  button.dataset.attachmentId = attachment.id
  button.replaceChildren(composerIconElement(action))
  return button
}

const removeFollowUpDraft = (id: string): void => {
  setShellFollowUpDrafts(shellModel().followUpDrafts.filter(draft => draft.id !== id))
  renderComposer()
}

const editFollowUpDraft = (draft: KhalaCodeFollowUpDraft): void => {
  setComposerDraftText(draft.text)
  removeFollowUpDraft(draft.id)
  requestAnimationFrame(focusComposerInput)
}

const steerFollowUpDraft = async (draft: KhalaCodeFollowUpDraft): Promise<void> => {
  if (!shellModel().pendingTurn) {
    editFollowUpDraft(draft)
    return
  }
  const turnIds = [...activeTurnIds]
  const targets = turnIds.length === 0 ? [undefined] : turnIds
  try {
    const outcomes = await steerEachTurn(targets, turnId =>
      rpc.request.codexTurnSteer({
        clientUserMessageId: draft.id,
        sessionId,
        text: draft.text,
        ...(turnId === undefined ? {} : { turnId }),
      }))
    if (!allTurnSteerOutcomesOk(outcomes)) {
      appendMessages([{
        body: summarizeTurnSteerOutcomes(outcomes, {
          success: "Follow-up steering succeeded.",
          failurePrefix: "Follow-up steering failed",
        }),
        id: nextMessageId("system"),
        role: "system",
      }])
      return
    }
    removeFollowUpDraft(draft.id)
  } catch (error) {
    appendMessages([{
      body: `Follow-up steering failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } finally {
    requestAnimationFrame(focusComposerInput)
  }
}

const renderFollowUpDraft = (draft: KhalaCodeFollowUpDraft): HTMLElement => {
  const row = document.createElement("div")
  row.className = "khala-code-composer-follow-up"
  row.dataset.followUpDraftId = draft.id
  row.setAttribute("role", "listitem")

  const icon = document.createElement("span")
  icon.className = "khala-code-composer-follow-up-icon"
  icon.setAttribute("aria-hidden", "true")
  icon.append(composerIconElement("steer"))

  const text = document.createElement("span")
  text.className = "khala-code-composer-follow-up-text"
  text.textContent = draft.text

  const steer = document.createElement("button")
  steer.type = "button"
  steer.className = "khala-code-composer-follow-up-action"
  steer.title = "Steer this follow-up into the active turn"
  steer.setAttribute("aria-label", `Steer follow-up: ${draft.text}`)
  steer.replaceChildren(composerIconElement("steer"), document.createTextNode("Steer"))
  steer.addEventListener("click", () => void steerFollowUpDraft(draft))

  const remove = document.createElement("button")
  remove.type = "button"
  remove.className = "khala-code-composer-follow-up-icon-button"
  remove.title = "Remove follow-up"
  remove.setAttribute("aria-label", `Remove follow-up: ${draft.text}`)
  remove.replaceChildren(composerIconElement("remove"))
  remove.addEventListener("click", () => removeFollowUpDraft(draft.id))

  const edit = document.createElement("button")
  edit.type = "button"
  edit.className = "khala-code-composer-follow-up-icon-button"
  edit.title = "Edit follow-up"
  edit.setAttribute("aria-label", `Edit follow-up: ${draft.text}`)
  edit.replaceChildren(composerIconElement("menu"))
  edit.addEventListener("click", () => editFollowUpDraft(draft))

  row.replaceChildren(icon, text, steer, remove, edit)
  return row
}

const renderFollowUpQueue = (): void => {
  const drafts = shellModel().followUpDrafts
  composerFollowUpQueue.hidden = drafts.length === 0
  composerFollowUpQueue.setAttribute("role", drafts.length === 0 ? "presentation" : "list")
  composerFollowUpQueue.replaceChildren(...drafts.map(renderFollowUpDraft))
}

const nextComposerContextChipId = (kind: ComposerContextChip["kind"]): string =>
  `composer-context-${kind}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const composerContextLineLabel = (
  context: Pick<ComposerContextChip, "lineEnd" | "lineStart">,
): string => {
  if (context.lineStart === undefined) return ""
  if (context.lineEnd === undefined || context.lineEnd === context.lineStart) {
    return `:${context.lineStart}`
  }
  return `:${context.lineStart}-${context.lineEnd}`
}

const addComposerContextChip = (chip: ComposerContextChip): void => {
  const key = `${chip.kind}:${chip.displayPath}:${chip.lineStart ?? ""}:${chip.lineEnd ?? ""}:${chip.body ?? ""}`
  composerContextChips = [
    ...composerContextChips.filter(existing =>
      `${existing.kind}:${existing.displayPath}:${existing.lineStart ?? ""}:${existing.lineEnd ?? ""}:${existing.body ?? ""}` !== key
    ),
    chip,
  ].slice(-8)
  shellModel().lastTurnFailed = false
  renderComposer()
  requestAnimationFrame(focusComposerInput)
}

const removeComposerContextChip = (chipId: string): void => {
  composerContextChips = composerContextChips.filter(chip => chip.id !== chipId)
  renderComposer()
  requestAnimationFrame(focusComposerInput)
}

const addEditorComposerContext = (
  context: KhalaCodeEditorComposerContext,
): void => {
  addComposerContextChip({
    displayPath: context.displayPath,
    id: nextComposerContextChipId(context.kind),
    kind: context.kind,
    ...(context.lineEnd === undefined ? {} : { lineEnd: context.lineEnd }),
    ...(context.lineStart === undefined ? {} : { lineStart: context.lineStart }),
    title: context.displayPath,
  })
}

const addReviewComposerContext = (
  context: KhalaCodeReviewComposerContext,
): void => {
  addComposerContextChip({
    displayPath: context.displayPath,
    id: nextComposerContextChipId("file"),
    kind: "file",
    title: context.displayPath,
  })
}

const addDiffReviewComposerContext = (
  comment: KhalaCodeDiffReviewComment,
): void => {
  addComposerContextChip({
    body: comment.body,
    displayPath: comment.filePath,
    id: comment.commentRef,
    kind: "comment",
    lineEnd: comment.lineNo,
    lineStart: comment.lineNo,
    title: khalaCodeDiffReviewLineLabel(comment),
  })
}

const renderComposerContextChip = (chip: ComposerContextChip): HTMLElement => {
  const row = document.createElement("div")
  row.className = "khala-code-composer-context-chip"
  row.dataset.composerContextId = chip.id
  row.dataset.kind = chip.kind
  row.setAttribute("role", "listitem")

  const icon = document.createElement("span")
  icon.className = "khala-code-composer-context-icon"
  icon.setAttribute("aria-hidden", "true")
  icon.append(composerIconElement(chip.kind === "comment" ? "steer" : "file"))

  const text = document.createElement("span")
  text.className = "khala-code-composer-context-text"
  text.textContent = `${chip.title}${composerContextLineLabel(chip)}`

  const remove = document.createElement("button")
  remove.type = "button"
  remove.className = "khala-code-composer-context-remove"
  remove.title = "Remove context"
  remove.setAttribute("aria-label", `Remove context: ${chip.title}`)
  remove.replaceChildren(composerIconElement("remove"))
  remove.addEventListener("click", () => removeComposerContextChip(chip.id))

  row.replaceChildren(icon, text, remove)
  return row
}

const renderComposerContextDock = (): void => {
  const existing = composerForm.querySelector<HTMLElement>("[data-khala-composer-context-dock]")
  if (composerContextChips.length === 0) {
    existing?.remove()
    return
  }
  const dock = existing ?? document.createElement("div")
  dock.className = "khala-code-composer-context-dock"
  dock.dataset.khalaComposerContextDock = ""
  dock.setAttribute("role", "list")
  dock.setAttribute("aria-label", "Selected composer context")
  dock.replaceChildren(...composerContextChips.map(renderComposerContextChip))
  if (existing === null) {
    composerFrame.before(dock)
  }
}

type ComposerWorkDockItem = Readonly<{
  count: number
  icon: ComposerIconName
  id: "follow-up" | "permission" | "question" | "request-tree" | "revert" | "todo"
  label: string
  state: "active" | "blocked" | "idle" | "pending"
  title: string
}>

const failedCodexItemCount = (): number =>
  shellModel().messages.filter(message => {
    const status = message.codexItem?.status?.toLowerCase()
    return status === "failed" || status === "denied" || status === "interrupted" || status === "expired"
  }).length

const pendingApprovalCount = (): number =>
  shellModel().messages.filter(message =>
    message.codexItem?.approval !== undefined && message.codexItem.status === "pending"
  ).length

const blockedApprovalCount = (): number =>
  shellModel().messages.filter(message => {
    const status = message.codexItem?.status?.toLowerCase()
    return message.codexItem?.approval !== undefined &&
      (status === "denied" || status === "failed" || status === "expired" || status === "interrupted")
  }).length

const composerWorkDockItems = (): readonly ComposerWorkDockItem[] => {
  const followUps = shellModel().followUpDrafts.length
  const pendingApprovals = pendingApprovalCount()
  const blockedApprovals = blockedApprovalCount()
  const failedItems = failedCodexItemCount()
  const plan = shellModel().architectPlanArtifact
  const planState = shellModel().architectPlanPending || plan?.status === "pending_approval"
    ? "pending"
    : plan?.status === "rejected"
      ? "blocked"
      : plan === null
        ? "idle"
        : "active"
  return [
    {
      count: followUps,
      icon: "steer",
      id: "follow-up",
      label: followUps > 0 ? `${followUps} follow-up` : "Follow-up",
      state: followUps > 0 ? "active" : "idle",
      title: followUps > 0 ? "Queued follow-ups" : "No queued follow-ups",
    },
    {
      count: pendingApprovals + blockedApprovals,
      icon: "settings",
      id: "permission",
      label: pendingApprovals > 0 ? `${pendingApprovals} permission` : "Permission",
      state: pendingApprovals > 0 ? "pending" : blockedApprovals > 0 ? "blocked" : "idle",
      title: pendingApprovals > 0
        ? "Permission approval pending"
        : blockedApprovals > 0
          ? "Permission approval blocked or denied"
          : "No permission approval pending",
    },
    {
      count: shellModel().claudeApprovalDialogOpen ? 1 : 0,
      icon: "menu",
      id: "question",
      label: shellModel().claudeApprovalDialogOpen ? "Question" : "Questions",
      state: shellModel().claudeApprovalDialogOpen ? "pending" : "idle",
      title: shellModel().claudeApprovalDialogOpen ? "Operator question pending" : "No operator question pending",
    },
    {
      count: failedItems,
      icon: "retry",
      id: "revert",
      label: failedItems > 0 || shellModel().lastTurnFailed ? "Review" : "Revert",
      state: failedItems > 0 || shellModel().lastTurnFailed ? "blocked" : "idle",
      title: failedItems > 0 || shellModel().lastTurnFailed
        ? "Failed or interrupted work needs review"
        : "No failed work to revert",
    },
    {
      count: plan === null ? 0 : plan.dag.nodes.length,
      icon: "code",
      id: "todo",
      label: plan === null ? "Todo" : `${plan.dag.nodes.length} todos`,
      state: planState,
      title: plan === null ? "No architect todo plan" : `Architect plan ${plan.status}`,
    },
    {
      count: activeTurnIds.size,
      icon: "model",
      id: "request-tree",
      label: activeTurnIds.size > 0 ? `${activeTurnIds.size} request` : "Request tree",
      state: shellModel().pendingTurn ? "active" : shellModel().activeCodexThreadId === null ? "idle" : "pending",
      title: shellModel().pendingTurn
        ? "Request tree active"
        : shellModel().activeCodexThreadId === null
          ? "No active request tree"
          : "Thread request tree ready",
    },
  ]
}

const renderComposerWorkDock = (): void => {
  const existing = composerForm.querySelector<HTMLElement>("[data-khala-composer-work-dock]")
  const dock = existing ?? document.createElement("div")
  dock.className = "khala-code-composer-work-dock"
  dock.dataset.khalaComposerWorkDock = ""
  dock.setAttribute("aria-label", "Composer work states")
  dock.replaceChildren(...composerWorkDockItems().map(item => {
    const button = document.createElement("button")
    button.type = "button"
    button.className = "khala-code-composer-work-dock-item"
    button.dataset.workDockId = item.id
    button.dataset.state = item.state
    button.dataset.count = String(item.count)
    button.title = item.title
    button.setAttribute("aria-label", item.title)
    button.addEventListener("click", () => {
      if (item.id === "permission") {
        messageList.querySelector<HTMLButtonElement>(".codex-approval-button")?.focus()
        return
      }
      if (item.id === "follow-up") {
        composerFollowUpQueue.querySelector<HTMLButtonElement>("button")?.focus()
        return
      }
      focusComposerInput()
    })
    button.replaceChildren(
      composerIconElement(item.icon),
      document.createTextNode(item.label),
    )
    return button
  }))
  if (existing === null) {
    const contextDock = composerForm.querySelector<HTMLElement>("[data-khala-composer-context-dock]")
    ;(contextDock ?? composerFrame).before(dock)
  }
}

const openAttachmentPreview = (attachment: CommandComposerAttachmentProps): void => {
  if (attachment.previewUrl !== undefined) {
    window.open(attachment.previewUrl, "_blank", "noopener,noreferrer")
    return
  }
  if (attachment.contentRef !== undefined) {
    const text = localTextAttachments.get(attachment.contentRef)
    if (text !== undefined) {
      const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }))
      objectUrls.add(url)
      window.open(url, "_blank", "noopener,noreferrer")
    }
  }
}

const applyComposerStateTransaction = (
  transaction: ComposerTransaction,
): boolean => {
  const result = applyComposerTransaction(shellModel().composerState, transaction)
  if (!result.ok) {
    shellModel().lastTurnFailed = true
    renderComposer()
    return false
  }
  setShellComposerState(result.state)
  shellModel().lastTurnFailed = false
  renderComposer()
  return true
}

const hexFromBytes = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, "0"))
    .join("")

const sha256DigestForBytes = async (bytes: ArrayBuffer): Promise<string> => {
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return `sha256:${hexFromBytes(digest)}`
}

const arrayBufferForText = (text: string): ArrayBuffer => {
  const bytes = new TextEncoder().encode(text)
  return bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer
}

const base64FromArrayBuffer = (bytes: ArrayBuffer): string => {
  const view = new Uint8Array(bytes)
  let binary = ""
  const chunkSize = 0x8000
  for (let offset = 0; offset < view.length; offset += chunkSize) {
    binary += String.fromCharCode(...view.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

const arrayBufferFromBase64 = (value: string): ArrayBuffer => {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes.buffer
}

const pushAttachmentReceipt = (
  receipt: ComposerAttachmentUploadReceipt,
): void => {
  dispatchMainShell({ _tag: "ComposerAttachmentReceiptPushed", receipt })
}

const attachmentById = (attachmentId: string): ComposerAttachment | undefined =>
  shellModel().composerState.doc.attachments.find(attachment => attachment.id === attachmentId)

const failLocalAttachmentUpload = (
  attachmentId: string,
  errorText: string,
): void => {
  const transaction = setComposerAttachmentStatusTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    { status: "error", errorText },
  )
  if (transaction !== null) applyComposerStateTransaction(transaction)
  const attachment = attachmentById(attachmentId)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment,
      surface: "desktop-local",
      event: "error",
      errorCode: "local_register_failed",
    }))
  }
}

const runDesktopLocalAttachmentUpload = async (
  attachmentId: string,
  bytes: () => Promise<ArrayBuffer>,
): Promise<void> => {
  const planned = planComposerAttachmentUpload(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  )
  if (!planned.ok) {
    pushAttachmentReceipt(planned.receipt)
    applyComposerStateTransaction(planned.transaction)
    return
  }

  pushAttachmentReceipt(planned.plan.receipt)
  if (!applyComposerStateTransaction(planned.plan.transaction)) return

  try {
    const digest = await sha256DigestForBytes(await bytes())
    const current = attachmentById(attachmentId)
    if (current === undefined) return
    const readyTransaction = readyComposerAttachmentTransaction(
      shellModel().composerState,
      composerAttachmentId(attachmentId),
      {
        surface: "desktop-local",
        digest,
        ...(current.kind === "image" ? { thumbnailDigest: digest } : {}),
        ...(current.dimensions === undefined
          ? {}
          : { dimensions: current.dimensions }),
      },
    )
    if (readyTransaction === null) return
    if (!applyComposerStateTransaction(readyTransaction)) return
    const readyAttachment = attachmentById(attachmentId)
    if (readyAttachment !== undefined) {
      pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
        attachment: readyAttachment,
        surface: "desktop-local",
        event: "ready",
      }))
    }
  } catch (error) {
    failLocalAttachmentUpload(
      attachmentId,
      error instanceof Error ? error.message : "Attachment upload failed.",
    )
  }
}

const removeAttachment = (attachmentId: string): void => {
  const attachment = shellModel().composerState.doc.attachments.find(
    item => item.id === attachmentId,
  )
  if (attachment?.previewUrl !== undefined && objectUrls.has(attachment.previewUrl)) {
    URL.revokeObjectURL(attachment.previewUrl)
    objectUrls.delete(attachment.previewUrl)
  }
  if (attachment?.contentRef !== undefined) {
    localTextAttachments.delete(attachment.contentRef)
  }
  localAttachmentFiles.delete(attachmentId)
  releaseNativeAttachmentGrant(attachmentId)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment,
      surface: "desktop-local",
      event: "removed",
    }))
  }
  applyComposerStateTransaction({
    steps: [
      {
        _tag: "RemoveAttachment",
        attachmentId: composerAttachmentId(attachmentId),
      },
    ],
    meta: { source: "manual", time: Date.now() },
  })
}

const retryAttachment = (attachmentId: string): void => {
  const attachment = attachmentById(attachmentId)
  const transaction = retryComposerAttachmentTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
  )
  if (transaction !== null) {
    applyComposerStateTransaction(transaction)
    const retried = attachmentById(attachmentId)
    if (retried !== undefined) {
      pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
        attachment: retried,
        surface: "desktop-local",
        event: "retry",
      }))
      const file = localAttachmentFiles.get(attachmentId)
      if (file !== undefined) {
        void runDesktopLocalAttachmentUpload(attachmentId, () => file.arrayBuffer())
      } else if (retried.contentRef !== undefined) {
        const text = localTextAttachments.get(retried.contentRef)
        if (text !== undefined) {
          void runDesktopLocalAttachmentUpload(
            attachmentId,
            () => Promise.resolve(arrayBufferForText(text)),
          )
        }
      }
    }
    return
  }
  const fallback = setComposerAttachmentStatusTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    { status: "staged", errorText: null },
  )
  if (fallback !== null) applyComposerStateTransaction(fallback)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment: { ...attachment, status: "staged" },
      surface: "desktop-local",
      event: "retry",
    }))
  }
}

const renderAttachment = (
  attachment: CommandComposerAttachmentProps,
): HTMLElement => {
  const chip = document.createElement("div")
  chip.className = composerClasses.attachment
  chip.dataset.oaCommandComposerAttachment = attachment.id
  chip.dataset.kind = attachment.kind
  chip.dataset.status = attachment.status
  chip.dataset.selected = "false"
  chip.role = "listitem"
  chip.tabIndex = 0
  chip.ariaSelected = "false"

  const before = document.createElement("span")
  before.className = "oa-ai-command-composer-gapcursor"
  before.tabIndex = 0
  before.dataset.oaCommandComposerGapcursor = "before"
  before.dataset.attachmentId = attachment.id
  before.setAttribute("aria-label", `Before ${attachment.name}`)

  const icon = document.createElement("span")
  icon.className = "oa-ai-command-composer-attachment-icon"
  icon.setAttribute("aria-hidden", "true")
  icon.replaceChildren(composerIconElement(attachmentIconName(attachment.kind)))

  const preview =
    attachment.kind === "image" && attachment.previewUrl !== undefined
      ? document.createElement("img")
      : null
  if (preview !== null) {
    preview.src = attachment.previewUrl ?? ""
    preview.alt = `${attachment.name} preview`
    preview.loading = "lazy"
    preview.decoding = "async"
    preview.className = "oa-ai-command-composer-attachment-thumb"
    if (attachment.dimensions !== undefined) {
      preview.width = attachment.dimensions.width
      preview.height = attachment.dimensions.height
    }
  }

  const main = document.createElement("span")
  main.className = "oa-ai-command-composer-attachment-main"
  const name = document.createElement("span")
  name.className = "oa-ai-command-composer-attachment-name"
  name.textContent = attachment.name
  const meta = document.createElement("span")
  meta.className = "oa-ai-command-composer-attachment-meta"
  const nativeGrant = localNativeAttachmentGrantLabels.get(attachment.id)
  meta.textContent = [
    attachment.mime,
    attachment.sizeLabel ?? formatBytes(attachment.sizeBytes),
    nativeGrant?.displayPath,
  ].filter((value): value is string => value !== undefined && value.length > 0).join(" - ")
  const status = document.createElement("span")
  status.className = "oa-ai-command-composer-attachment-status"
  status.textContent = statusTextForAttachment(attachment.status)
  main.append(name, meta, status)
  if (attachment.errorText !== undefined) {
    const error = document.createElement("span")
    error.className = "oa-ai-command-composer-attachment-error"
    error.textContent = attachment.errorText
    main.append(error)
  }

  const actions = document.createElement("span")
  actions.className = "oa-ai-command-composer-attachment-actions"
  if (attachment.previewUrl !== undefined || attachment.contentRef !== undefined || attachment.status === "ready") {
    const open = renderAttachmentAction("Open", "preview", attachment)
    open.addEventListener("click", () => openAttachmentPreview(attachment))
    actions.append(open)
  }
  if (attachment.status === "error") {
    const retry = renderAttachmentAction("Retry", "retry", attachment)
    retry.addEventListener("click", () => retryAttachment(attachment.id))
    actions.append(retry)
  }
  const remove = renderAttachmentAction("Remove", "remove", attachment)
  remove.addEventListener("click", () => removeAttachment(attachment.id))
  actions.append(remove)

  const after = document.createElement("span")
  after.className = "oa-ai-command-composer-gapcursor"
  after.tabIndex = 0
  after.dataset.oaCommandComposerGapcursor = "after"
  after.dataset.attachmentId = attachment.id
  after.setAttribute("aria-label", `After ${attachment.name}`)

  chip.append(
    before,
    icon,
    ...(preview === null ? [] : [preview]),
    main,
    actions,
    after,
  )
  return chip
}

const renderAttachmentRail = (): void => {
  const attachments = shellModel().composerState.doc.attachments.map(attachmentPropsFor)
  composerRail.hidden = attachments.length === 0 && !shellModel().dragActive
  composerRail.dataset.oaCommandComposerDragActive = shellModel().dragActive ? "true" : "false"
  composerRail.replaceChildren(
    ...attachments.map(renderAttachment),
    ...(shellModel().dragActive
      ? [
          Object.assign(document.createElement("div"), {
            className: composerClasses.dropcursor,
          }),
        ]
      : []),
  )
}

const renderArchitectPlanNode = (
  node: KhalaCodeDesktopArchitectPlanArtifact["dag"]["nodes"][number],
): HTMLElement => {
  const item = document.createElement("li")
  item.className = "khala-architect-plan-node"

  const title = document.createElement("span")
  title.className = "khala-architect-plan-node-title"
  title.textContent = node.title

  const objective = document.createElement("span")
  objective.className = "khala-architect-plan-node-objective"
  objective.textContent = node.objective

  const meta = document.createElement("span")
  meta.className = "khala-architect-plan-node-meta"
  meta.textContent = [
    node.nodeRef,
    node.issue === undefined ? null : `#${node.issue}`,
    node.dependsOn === undefined || node.dependsOn.length === 0
      ? null
      : `after ${node.dependsOn.join(", ")}`,
  ].filter((value): value is string => value !== null).join(" · ")

  item.replaceChildren(title, objective, meta)
  return item
}

const decideArchitectPlan = async (decision: "approve" | "reject"): Promise<void> => {
  const artifact = shellModel().architectPlanArtifact
  if (artifact === null || shellModel().architectPlanPending) return
  setArchitectPlanPending(true)
  renderComposer()
  try {
    const activeThreadId = shellModel().activeCodexThreadId
    const result = await controls.architectPlanDecision({
      decision,
      planRef: artifact.planRef,
      sessionId,
      ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
    })
    if (!result.ok) {
      appendMessages([{ body: `Architect plan decision failed: ${result.error}`, id: nextMessageId("system"), role: "system" }])
      return
    }
    setArchitectPlanArtifact(result.artifact)
    appendMessages([{ body: result.message, id: nextMessageId("system"), role: "system" }])
  } catch (error) {
    appendMessages([{
      body: `Architect plan decision failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } finally {
    setArchitectPlanPending(false)
    renderComposer()
    requestAnimationFrame(focusComposerInput)
  }
}

const renderArchitectPlanCard = (
  artifact: KhalaCodeDesktopArchitectPlanArtifact,
): HTMLElement => {
  const card = document.createElement("section")
  card.className = "khala-architect-plan-card"
  card.dataset.architectPlanRef = artifact.planRef
  card.dataset.architectPlanStatus = artifact.status
  card.dataset.architectPlanDispatchMode = artifact.dispatchMode

  const header = document.createElement("div")
  header.className = "khala-architect-plan-header"

  const title = document.createElement("div")
  title.className = "khala-architect-plan-title"
  title.textContent = "Architect plan"

  const status = document.createElement("span")
  status.className = "khala-architect-plan-status"
  status.textContent = `${artifact.status.replace(/_/gu, " ")} · ${artifact.dispatchMode.replace("_", "-")}`

  header.replaceChildren(title, status)

  const objective = document.createElement("p")
  objective.className = "khala-architect-plan-objective"
  objective.textContent = artifact.dag.objective

  const nodes = document.createElement("ol")
  nodes.className = "khala-architect-plan-nodes"
  nodes.replaceChildren(...artifact.dag.nodes.map(renderArchitectPlanNode))

  const actions = document.createElement("div")
  actions.className = "khala-architect-plan-actions"
  const approve = document.createElement("button")
  approve.type = "button"
  approve.className = "khala-architect-plan-action khala-architect-plan-action--primary"
  approve.disabled = artifact.status !== "pending_approval" || shellModel().architectPlanPending
  approve.textContent = artifact.dispatchMode === "fleet_run" ? "Start FleetRun" : "Approve"
  approve.addEventListener("click", () => void decideArchitectPlan("approve"))

  const reject = document.createElement("button")
  reject.type = "button"
  reject.className = "khala-architect-plan-action"
  reject.disabled = artifact.status !== "pending_approval" || shellModel().architectPlanPending
  reject.textContent = "Reject"
  reject.addEventListener("click", () => void decideArchitectPlan("reject"))

  actions.replaceChildren(approve, reject)
  card.replaceChildren(header, objective, nodes, actions)
  return card
}

const renderComposerPreview = (): void => {
  const artifact = shellModel().architectPlanArtifact
  composerPreview.hidden = artifact === null
  composerPreview.replaceChildren(...(artifact === null ? [] : [renderArchitectPlanCard(artifact)]))
}

const slashCommandPlatform = (): string => {
  const userAgent = navigator.userAgent.toLowerCase()
  const platform = navigator.platform.toLowerCase()
  if (userAgent.includes("android")) return "android"
  if (platform.includes("win")) return "win32"
  if (platform.includes("mac")) return "darwin"
  if (platform.includes("linux")) return "linux"
  return "unknown"
}

const slashCommandLoadKey = (): string =>
  `${slashCommandPlatform()}:${shellModel().pendingTurn ? "active" : "idle"}`

const ensureSlashCommandsLoaded = (): void => {
  const key = slashCommandLoadKey()
  if (shellModel().loadedSlashCommandKey === key && shellModel().slashCommands.length > 0) return
  if (shellModel().slashCommandLoadInFlight !== null) return
  shellModel().slashCommandLoadInFlight = rpc.request.slashCommandList({
    activeTurn: shellModel().pendingTurn,
    platform: slashCommandPlatform(),
    sideConversation: false,
  }).then(response => {
    shellModel().slashCommands = [...response.commands]
    shellModel().loadedSlashCommandKey = key
  }).catch(() => {
    shellModel().slashCommands = []
    shellModel().loadedSlashCommandKey = key
  }).finally(() => {
    shellModel().slashCommandLoadInFlight = null
    renderSlashCommandPalette()
  })
}

const slashCommandQuery = (): string | null => {
  const value = composerDraftText().trimStart()
  if (!value.startsWith("/")) return null
  return value.slice(1).split(/\s+/)[0]?.toLowerCase() ?? ""
}

const architectSlashCommand = (): KhalaCodeMainShellSlashCommand => ({
  aliases: [],
  availability: shellModel().pendingTurn
    ? {
      available: false,
      reason: "Architect plan mode starts from an idle composer.",
    }
    : { available: true },
  availableDuringTask: false,
  availableInSideConversation: false,
  command: "architect",
  debug: false,
  description: "Run Claude architect plan mode and render an approvable DAG",
  dispatch: { action: "architect_plan", kind: "client" },
  enumName: "Architect",
  group: "turn_task",
  supportsInlineArgs: true,
  visibility: { kind: "always" },
})

const matchingSlashCommands = (): readonly KhalaCodeMainShellSlashCommand[] => {
  const query = slashCommandQuery()
  if (query === null) return []
  const local = "architect".includes(query) ? [architectSlashCommand()] : []
  const remote = shellModel().slashCommands.filter(command =>
    command.command.includes(query) ||
    command.aliases.some(alias => alias.includes(query))
  ).slice(0, 8)
  return [...local, ...remote].slice(0, 8)
}

const selectSlashCommand = (command: KhalaCodeMainShellSlashCommand): void => {
  setComposerDraftText(command.supportsInlineArgs ? `/${command.command} ` : `/${command.command}`)
  composerInput.focus({ preventScroll: true })
  setKhalaComposerCaretToEnd(composerInput)
  renderComposer()
}

function renderSlashCommandPalette(): void {
  const query = slashCommandQuery()
  if (query === null) {
    slashCommandPalette.hidden = true
    slashCommandPalette.replaceChildren()
    return
  }
  ensureSlashCommandsLoaded()
  const matches = matchingSlashCommands()
  slashCommandPalette.hidden = matches.length === 0
  slashCommandPalette.replaceChildren(
    ...matches.map(command => {
      const button = document.createElement("button")
      button.type = "button"
      button.className = "khala-code-slash-command-option"
      button.disabled = !command.availability.available
      button.dataset.commandGroup = command.group
      button.setAttribute("role", "option")
      button.title = command.availability.reason ?? command.description
      button.addEventListener("mousedown", event => event.preventDefault())
      button.addEventListener("click", () => selectSlashCommand(command))

      const name = document.createElement("span")
      name.className = "khala-code-slash-command-name"
      name.textContent = `/${command.command}`

      const description = document.createElement("span")
      description.className = "khala-code-slash-command-description"
      description.textContent = command.availability.reason ?? command.description

      button.replaceChildren(name, description)
      return button
    }),
  )
}

const renderReasoningModeSelect = (): HTMLElement => {
  ensureComposerReasoningModesLoaded()
  const control = document.createElement("label")
  control.className = "khala-reasoning-mode-control"
  control.dataset.state = composerReasoningModeState.saving
    ? "saving"
    : composerReasoningModeState.loading
      ? "loading"
      : composerReasoningModeState.error === null
        ? "ready"
        : "error"
  control.title = composerReasoningModeTitle()

  const label = document.createElement("span")
  label.className = "khala-reasoning-mode-label"
  label.textContent = "Reasoning"

  const select = document.createElement("select")
  select.className = "khala-reasoning-mode-select"
  select.name = "khala-code-reasoning-mode"
  select.setAttribute("aria-label", "Reasoning mode")
  select.disabled = composerReasoningModeDisabled()
  const selectedValue = composerReasoningModeValue()
  select.append(...composerReasoningModeOptions().map(option => {
    const item = document.createElement("option")
    item.value = option.value
    item.textContent = option.label
    item.selected = option.value === selectedValue
    return item
  }))
  select.addEventListener("change", () => void writeComposerReasoningMode(select.value))

  control.replaceChildren(label, select)
  return control
}

const composerModeLabel = (mode: KhalaRichComposerMode): string =>
  mode === "shell" ? "Shell" : "Normal"

const setComposerMode = (mode: KhalaRichComposerMode): void => {
  composerMode = mode
  syncComposerDraftState()
  renderComposer()
  requestAnimationFrame(focusComposerInput)
}

const toggleComposerMode = (): void => {
  setComposerMode(composerMode === "normal" ? "shell" : "normal")
}

const renderComposerModeButton = (): HTMLButtonElement => {
  const button = document.createElement("button")
  button.type = "button"
  button.className = "khala-composer-mode-button"
  button.dataset.mode = composerMode
  button.dataset.oaCommandComposerModeToggle = composerMode
  button.setAttribute("aria-label", `Composer mode: ${composerModeLabel(composerMode)}`)
  button.setAttribute("aria-pressed", composerMode === "shell" ? "true" : "false")
  button.title = `Composer mode: ${composerModeLabel(composerMode)}`
  const label = document.createElement("span")
  label.className = "khala-composer-mode-button-label"
  label.textContent = composerModeLabel(composerMode)
  button.replaceChildren(composerIconElement(composerMode === "shell" ? "code" : "text"), label)
  button.addEventListener("click", toggleComposerMode)
  return button
}

const renderComposerProviderState = (): HTMLElement => {
  const state = composerProviderState()
  const label = document.createElement("span")
  label.className = "khala-composer-provider-state"
  label.dataset.state = state
  label.dataset.oaCommandComposerProviderState = state
  label.title = composerReasoningModeState.error ??
    composerSettings()?.errors.join("\n") ??
    composerUsageA11yText()
  label.textContent = state === "ready"
    ? "Ready"
    : state === "managed"
      ? "Managed"
      : state === "saving"
        ? "Saving"
        : state === "loading"
          ? "Loading"
          : state === "error"
            ? "Error"
            : "Unavailable"
  return label
}

const renderComposerSelectControl = (input: {
  readonly className: string
  readonly dataControl: string
  readonly disabled: boolean
  readonly label: string
  readonly name: string
  readonly onChange: (value: string) => void
  readonly options: readonly { readonly label: string; readonly value: string }[]
  readonly title: string
  readonly value: string
}): HTMLElement => {
  const control = document.createElement("label")
  control.className = `khala-composer-select-control ${input.className}`
  control.dataset.oaCommandComposerControl = input.dataControl
  control.title = input.title
  const label = document.createElement("span")
  label.className = "khala-composer-select-label"
  label.textContent = input.label

  const select = document.createElement("select")
  select.className = "khala-composer-select"
  select.name = input.name
  select.disabled = input.disabled
  select.setAttribute("aria-label", input.label)
  select.append(...input.options.map(option => {
    const item = document.createElement("option")
    item.value = option.value
    item.textContent = option.label
    item.selected = option.value === input.value
    return item
  }))
  select.addEventListener("change", () => input.onChange(select.value))

  control.replaceChildren(label, select)
  return control
}

const renderComposerAgentSelect = (): HTMLElement =>
  renderComposerSelectControl({
    className: "khala-composer-agent-control",
    dataControl: "agent",
    disabled: shellModel().pendingTurn,
    label: "Agent",
    name: "khala-code-agent-role",
    onChange: value => {
      if ((KHALA_CODE_MODEL_ROLE_ORDER as readonly string[]).includes(value)) {
        setComposerAgentRole(value as KhalaCodeModelRole)
      }
    },
    options: KHALA_CODE_MODEL_ROLE_ORDER.map(role => ({
      label: formatComposerControlLabel(role),
      value: role,
    })),
    title: shellModel().pendingTurn
      ? "Agent can change after the active turn finishes"
      : "Agent role",
    value: composerAgentRole,
  })

const renderComposerProviderSelect = (): HTMLElement => {
  ensureComposerReasoningModesLoaded()
  const settings = composerSettings()
  const options = settings === null || settings.providers.options.length === 0
    ? [{
        label: composerReasoningModeState.loading ? "Loading" : "Unavailable",
        value: "",
      }]
    : settings.providers.options.map(provider => ({
        label: provider.displayName,
        value: provider.id,
      }))
  return renderComposerSelectControl({
    className: "khala-composer-provider-control",
    dataControl: "provider",
    disabled: composerProviderState() !== "ready",
    label: "Provider",
    name: "khala-code-model-provider",
    onChange: value => void writeComposerProvider(value),
    options,
    title: composerReasoningModeState.error ?? "Provider",
    value: composerProviderValue(),
  })
}

const renderComposerModelSelect = (): HTMLElement => {
  ensureComposerReasoningModesLoaded()
  const options = composerModelOptions()
  return renderComposerSelectControl({
    className: "khala-composer-model-control",
    dataControl: "model",
    disabled: composerReasoningModeState.loading ||
      composerReasoningModeState.saving ||
      composerProviderState() === "managed" ||
      options.length === 0,
    label: "Model",
    name: "khala-code-model",
    onChange: value => void writeComposerModel(value),
    options: options.length === 0
      ? [{
          label: composerReasoningModeState.loading ? "Loading" : "Unavailable",
          value: "",
        }]
      : options.map(model => ({
          label: model.displayName,
          value: model.model,
        })),
    title: composerReasoningModeState.error ?? "Model",
    value: composerModelValue(),
  })
}

const renderComposerVariantButton = (): HTMLButtonElement => {
  const variants = composerVariantOptions()
  const button = document.createElement("button")
  button.type = "button"
  button.className = "khala-composer-variant-button"
  button.dataset.oaCommandComposerControl = "variant"
  button.dataset.variant = composerVariantValue()
  button.disabled = composerReasoningModeState.loading ||
    composerReasoningModeState.saving ||
    composerProviderState() === "managed" ||
    variants.length <= 1
  button.title = variants.length <= 1
    ? "No variants for this model"
    : "Cycle model variant"
  button.setAttribute("aria-label", `Variant: ${composerVariantLabel()}`)
  button.replaceChildren(
    composerIconElement("model"),
    document.createTextNode(composerVariantLabel()),
  )
  button.addEventListener("click", () => void cycleComposerVariant())
  return button
}

function renderComposer(): void {
  const status = statusForComposer()
  const sendLabel = buttonLabel(status)
  const attachmentCount = shellModel().composerState.doc.attachments.length
  const contextCount = composerContextChips.length
  const followUpCount = shellModel().followUpDrafts.length

  syncComposerDraftState()
  composerForm.dataset.oaCommandComposerStatus = status
  composerForm.dataset.oaCommandComposerProviderState = composerProviderState()
  composerFrame.dataset.oaCommandComposerFrame = ""
  sendButton.disabled = !shellModel().pendingTurn && !canSubmitComposer()
  sendButton.type = shellModel().pendingTurn ? "button" : "submit"
  sendButton.title = sendLabel
  sendButton.setAttribute("aria-label", `${sendLabel} message`)
  sendButton.dataset.oaCommandComposerSubmit = shellModel().pendingTurn ? "stop" : "send"
  sendButton.dataset.status = status
  setButtonIcon(sendButton, shellModel().pendingTurn ? "stop" : "send")
  setButtonIcon(attachButton, "plus")
  sendButton.querySelector(".oa-ai-command-composer-submit-label")!.textContent =
    sendLabel
  composerControls.replaceChildren(
    attachButton,
    renderComposerModeButton(),
    renderComposerAgentSelect(),
    renderComposerProviderSelect(),
    renderComposerModelSelect(),
    renderComposerVariantButton(),
    renderReasoningModeSelect(),
    renderComposerProviderState(),
    // renderHarnessPill(),
  )

  composerStatus.className = composerClasses.status
  composerStatus.dataset.oaCommandComposerStatusLabel = status
  composerStatus.replaceChildren(
    // renderRuntimeBadge(),
    // renderMicrophoneIndicator(),
  )
  composerA11y.textContent =
    `${statusLabelFor(status)}. ${attachmentCount} attachments. ` +
    `${contextCount} context chips. ` +
    `${followUpCount} queued follow-ups. ` +
    `${composerModeLabel(composerMode)} composer mode. ` +
    `Agent ${formatComposerControlLabel(composerAgentRole)}. ` +
    `Provider ${composerSelectedProvider()?.displayName ?? "unavailable"}. ` +
    `Model ${composerSelectedModel()?.displayName ?? "unavailable"}. ` +
    `Variant ${composerVariantLabel()}. ` +
    `${composerUsageA11yText()} ` +
    `${composerReasoningModeA11yText()} ` +
    `${shellModel().architectPlanPending ? "Architect plan pending. " : ""}` +
    `${composerDraftText().length} characters.`

  renderFollowUpQueue()
  renderComposerWorkDock()
  renderComposerContextDock()
  renderAttachmentRail()
  renderComposerPreview()
  renderSlashCommandPalette()
  updateComposerHudProjection()
}

const render = (): void => {
  renderBootDegradedStates()
  renderMessages()
  renderComposer()
}

function renderBootDegradedStates(): void {
  const root = document.getElementById("boot-degraded-states")
  if (root === null) return
  const degradedStates = shellModel().bootDegradedStates
  root.hidden = degradedStates.length === 0
  root.replaceChildren()
  root.dataset.state = degradedStates.length === 0 ? "ready" : "degraded"
  root.dataset.degradedMethods = degradedStates.map(state => state.method).join(" ")
  root.dataset.degradedCount = String(degradedStates.length)
  if (degradedStates.length === 0) return
  for (const state of degradedStates) {
    const chip = document.createElement("span")
    chip.className = "khala-code-boot-degraded-chip"
    chip.dataset.bootRpc = state.method
    chip.dataset.state = state.state
    chip.dataset.kind = state.kind
    chip.title = state.detail
    chip.textContent = `${state.method}: degraded`
    root.append(chip)
  }
}

const focusComposerInput = (): void => {
  composerInput.focus({ preventScroll: true })
  setKhalaComposerCaretToEnd(composerInput)
  updateComposerHudProjection()
}

const nextMessageId = (role: KhalaCodeDesktopMessageRole): string =>
  `${role}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const nextTurnId = (): string =>
  `turn-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const nextFollowUpDraftId = (): string =>
  `follow-up-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const addMessage = (
  role: KhalaCodeDesktopMessageRole,
  body: string,
): KhalaCodeDesktopMessage => {
  const message = {
    id: nextMessageId(role),
    role,
    body,
  }
  setShellMessages([...shellModel().messages, message])
  render()
  return message
}

const appendMessages = (nextMessages: readonly KhalaCodeDesktopMessage[]): void => {
  if (nextMessages.length === 0) return
  const merged = [...shellModel().messages]
  const indexById = new Map(merged.map((message, index) => [message.id, index]))
  for (const message of nextMessages) {
    const index = indexById.get(message.id)
    if (index === undefined) {
      indexById.set(message.id, merged.length)
      merged.push(message)
    } else {
      merged[index] = message
    }
  }
  setShellMessages(merged)
  render()
}

const latestUserMessagePreview = (): string =>
  [...shellModel().messages].reverse().find(message => message.role === "user")?.body.trim() ?? ""

const markVisibleChatTurnEventPaint = (
  event: KhalaCodeDesktopChatTurnEvent,
  receivedAt: number,
): void => {
  requestAnimationFrame(() => {
    const context = {
      eventType: event.type,
      threadId: shellModel().activeCodexThreadId ?? "unknown",
      turnId: event.turnId,
    }
    pushQaMetricSample("sse.event_to_ui_ms", performance.now() - receivedAt, context)
    const turnStartedAt = activeTurnStartTimes.get(event.turnId)
    if (turnStartedAt === undefined || turnFirstVisibleEventRecorded.has(event.turnId)) return
    turnFirstVisibleEventRecorded.add(event.turnId)
    pushQaMetricSample("turn_start.first_event_ms", performance.now() - turnStartedAt, context)
  })
}

function applyChatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void {
  if (!activeTurnIds.has(event.turnId)) return
  const receivedAt = performance.now()
  let visibleEventRendered = false
  if (event.type === "thread_ready") {
    setActiveCodexThreadId(event.threadId)
    threadSidebar?.upsertPendingThread({
      preview: latestUserMessagePreview(),
      threadId: event.threadId,
    })
    enqueueKhalaSyncChatThreadCreate({
      threadId: event.threadId,
      title: latestUserMessagePreview(),
    })
    void refreshThreadTokenSummary()
    void threadSidebar?.refresh()
    return
  }
  if (
    event.type === "message_start" ||
    event.type === "message_delta" ||
    event.type === "message_replace"
  ) {
    shellModel().thinkingTurnId = null
  }
  switch (event.type) {
    case "message_start":
      shellModel().pendingTurn = true
      syncThreadTokenPolling()
      appendMessages([event.message])
      visibleEventRendered = true
      break
    case "message_delta":
      setShellMessages(shellModel().messages.map(message =>
        message.id === event.messageId
          ? { ...message, body: `${message.body}${event.delta}` }
          : message
      ))
      render()
      visibleEventRendered = true
      break
    case "message_replace":
      appendMessages([event.message])
      visibleEventRendered = true
      break
    case "message_done":
      break
    case "tool_event":
      break
  }
  if (visibleEventRendered) markVisibleChatTurnEventPaint(event, receivedAt)
}

const parseDatasetJson = <T>(value: string | undefined): T | undefined => {
  if (value === undefined || value.length === 0) return undefined
  try {
    return JSON.parse(value) as T
  } catch {
    return undefined
  }
}

const respondToCodexApproval = async (button: HTMLButtonElement): Promise<void> => {
  const requestId = parseDatasetJson<Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["requestId"]>(
    button.dataset.codexApprovalRequestId,
  )
  const method = button.dataset.codexApprovalMethod as Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["method"] | undefined
  const action = button.dataset.codexApprovalAction as Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["action"] | undefined
  if (requestId === undefined || method === undefined || action === undefined) return

  button.disabled = true
  const execpolicyAmendment = parseDatasetJson<readonly string[]>(
    button.dataset.codexApprovalExecpolicyAmendment,
  )
  const networkPolicyAmendment = parseDatasetJson<
    Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["networkPolicyAmendment"]
  >(button.dataset.codexApprovalNetworkPolicyAmendment)
  const permissions = parseDatasetJson<
    Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["permissions"]
  >(button.dataset.codexApprovalPermissions)
  const result = await rpc.request.codexApprovalRespond({
    action,
    method,
    requestId,
    ...(execpolicyAmendment === undefined ? {} : { execpolicyAmendment }),
    ...(networkPolicyAmendment === undefined ? {} : { networkPolicyAmendment }),
    ...(permissions === undefined ? {} : { permissions }),
  })
  appendMessages([{
    body: result.ok
      ? `Sent Codex approval decision: ${action}.`
      : `Codex approval decision failed: ${result.error ?? "unknown error"}`,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const nextDiffReviewCommentRef = (): string =>
  `diff_review.${Date.now().toString(36)}.${Math.random().toString(36).slice(2, 8)}`

const stageDiffReviewNoteInComposer = (note: string): void => {
  stageComposerText(note)
}

const nextSourceControlActionRef = (): string =>
  `source_control_action.${Date.now().toString(36)}.${Math.random().toString(36).slice(2, 8)}`

const stageSourceControlActionPromptInComposer = (prompt: string): void => {
  stageComposerText(prompt)
}

const handleDiffReviewSubmitDetail = async (
  detail: typeof KhalaCodeDiffReviewSubmitDetailSchema.Type,
): Promise<void> => {
  const comment = khalaCodeDiffReviewComment({
    ...detail,
    commentRef: nextDiffReviewCommentRef(),
  })
  reviewPanel?.addComment(comment)
  const note = khalaCodeDiffReviewSteeringNote(comment)

  if (!shellModel().pendingTurn) {
    addDiffReviewComposerContext(comment)
    stageDiffReviewNoteInComposer(note)
    appendMessages([{
      body: `Staged diff review comment for ${khalaCodeDiffReviewLineLabel(comment)} in the composer.`,
      id: nextMessageId("system"),
      role: "system",
    }])
    return
  }

  const turnIds = [...activeTurnIds]
  const targets = turnIds.length === 0 ? [undefined] : turnIds
  try {
    const outcomes = await steerEachTurn(targets, turnId =>
      rpc.request.codexTurnSteer({
        clientUserMessageId: comment.commentRef,
        sessionId,
        text: note,
        ...(turnId === undefined ? {} : { turnId }),
      }))
    appendMessages([{
      body: summarizeTurnSteerOutcomes(outcomes, {
        success: `Sent diff review comment to the active Codex turn: ${khalaCodeDiffReviewLineLabel(comment)}.`,
        failurePrefix: "Diff review steering failed",
      }),
      id: nextMessageId("system"),
      role: "system",
    }])
  } catch (error) {
    appendMessages([{
      body: `Diff review steering failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  }
}

const handleDiffReviewSubmit = async (event: Event): Promise<void> => {
  const rawDetail = "detail" in event
    ? (event as CustomEvent<unknown>).detail
    : undefined
  let detail: typeof KhalaCodeDiffReviewSubmitDetailSchema.Type
  try {
    detail = S.decodeUnknownSync(KhalaCodeDiffReviewSubmitDetailSchema)(rawDetail)
  } catch (error) {
    appendMessages([{
      body: `Diff review comment failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
    return
  }
  await handleDiffReviewSubmitDetail(detail)
}

const handleSourceControlActionSubmit = async (event: Event): Promise<void> => {
  const rawDetail = "detail" in event
    ? (event as CustomEvent<unknown>).detail
    : undefined
  let detail: typeof KhalaCodeSourceControlActionSubmitDetailSchema.Type
  try {
    detail = S.decodeUnknownSync(KhalaCodeSourceControlActionSubmitDetailSchema)(rawDetail)
  } catch (error) {
    appendMessages([{
      body: `Source-control action failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
    return
  }

  const prompt = khalaCodeSourceControlActionPrompt({
    ...detail,
    actionRef: nextSourceControlActionRef(),
  })
  const promptText = khalaCodeSourceControlActionPromptText(prompt)
  const label = khalaCodeSourceControlActionLabel(prompt.action)

  if (!shellModel().pendingTurn) {
    stageSourceControlActionPromptInComposer(promptText)
    appendMessages([{
      body: `Staged source-control ${label} prompt in the composer.`,
      id: nextMessageId("system"),
      role: "system",
    }])
    return
  }

  const turnIds = [...activeTurnIds]
  const targets = turnIds.length === 0 ? [undefined] : turnIds
  try {
    const outcomes = await steerEachTurn(targets, turnId =>
      rpc.request.codexTurnSteer({
        clientUserMessageId: prompt.actionRef,
        sessionId,
        text: promptText,
        ...(turnId === undefined ? {} : { turnId }),
      }))
    appendMessages([{
      body: summarizeTurnSteerOutcomes(outcomes, {
        success: `Sent source-control ${label} prompt to the active Codex turn.`,
        failurePrefix: `Source-control ${label} steering failed`,
      }),
      id: nextMessageId("system"),
      role: "system",
    }])
  } catch (error) {
    appendMessages([{
      body: `Source-control ${label} steering failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  }
}

const handledClaudeApprovals = new Set<string>()
const respondToClaudeApprovalRequest = async (
  request: Awaited<ReturnType<DesktopRpcRequests["claudeApprovalPending"]>>["requests"][number],
): Promise<void> => {
  const title = request.options.title ?? request.options.displayName ?? `Claude wants to use ${request.toolName}`
  const detail = [
    title,
    request.options.description,
    request.options.decisionReason,
    request.options.blockedPath === undefined ? null : `Path: ${request.options.blockedPath}`,
    "",
    "Type always to allow and remember suggested permissions, allow to allow once, or deny.",
  ].filter((line): line is string => line !== null && line !== undefined).join("\n")
  const answer = window.prompt(detail, "allow")?.trim().toLowerCase() ?? "deny"
  const allow = answer === "allow" || answer === "always"
  const result = await controls.claudeApprovalRespond({
    requestId: request.id,
    decision: allow
      ? {
          behavior: "allow",
          decisionClassification: answer === "always" ? "always_allow" : "allow_once",
          ...(answer === "always" && request.options.suggestions !== undefined
            ? { updatedPermissions: request.options.suggestions }
            : {}),
        }
      : {
          behavior: "deny",
          decisionClassification: "deny",
          message: "Denied by the Khala Code Desktop operator.",
        },
  })
  appendMessages([{
    body: result.ok
      ? `Sent Claude approval decision: ${allow ? "allow" : "deny"}.`
      : `Claude approval decision failed: ${result.error ?? "unknown error"}`,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const pollClaudeApprovals = async (): Promise<void> => {
  if (shellModel().claudeApprovalDialogOpen) return
  const pending = await controls.claudeApprovalPending().then(result => {
    clearBootRpcDegradedState("claudeApprovalPending")
    return result
  }).catch(error => {
    recordBootRpcDegradedState("claudeApprovalPending", error)
    return { ok: false, requests: [] } satisfies Awaited<ReturnType<DesktopRpcRequests["claudeApprovalPending"]>>
  })
  const request = pending?.requests.find(item => !handledClaudeApprovals.has(item.id))
  if (request === undefined) return
  handledClaudeApprovals.add(request.id)
  shellModel().claudeApprovalDialogOpen = true
  try {
    await respondToClaudeApprovalRequest(request)
  } finally {
    shellModel().claudeApprovalDialogOpen = false
  }
}

const attachmentSummaryForSubmit = (
  attachments: readonly ComposerAttachment[],
): string => {
  if (attachments.length === 0) return ""
  return [
    "Attachments:",
    ...attachments.map(
      attachment =>
        `- ${attachment.name} (${attachment.mime}, ${formatBytes(attachment.sizeBytes)}, ${attachment.status})`,
    ),
  ].join("\n")
}

const contextSummaryForSubmit = (
  contexts: readonly ComposerContextChip[],
): string => {
  if (contexts.length === 0) return ""
  return [
    "Selected context:",
    ...contexts.map(context => {
      const location = `${context.displayPath}${composerContextLineLabel(context)}`
      if (context.kind === "comment" && context.body !== undefined) {
        return `- comment ${location}: ${context.body}`
      }
      return `- ${context.kind} ${location}`
    }),
  ].join("\n")
}

const submittedBody = (
  text: string,
  attachments: readonly ComposerAttachment[],
  contexts: readonly ComposerContextChip[] = composerContextChips,
): string => {
  const summary = [
    contextSummaryForSubmit(contexts),
    attachmentSummaryForSubmit(attachments),
  ].filter(item => item.trim().length > 0).join("\n\n")
  if (summary === "") return text
  if (text.trim() === "") return summary
  return `${text}\n\n${summary}`
}

const imageAttachmentsForSubmit = (
  attachments: readonly ComposerAttachment[],
): Promise<{
  readonly resolved: readonly KhalaCodeDesktopChatTurnAttachment[]
  readonly failures: readonly { readonly name: string; readonly error: string }[]
}> =>
  resolveAttachments(
    attachments,
    async (attachment): Promise<KhalaCodeDesktopChatTurnAttachment | null> => {
      if (attachment.kind !== "image" || attachment.status !== "ready") return null
      const file = localAttachmentFiles.get(attachment.id)
      const grant = localNativeAttachmentGrantLabels.get(attachment.id)
      if (file === undefined && grant === undefined) return null
      const mime = file?.type ?? grant?.mime ?? attachment.mime
      if (!mime.startsWith("image/")) return null
      const bytes = file === undefined
        ? await bytesForNativeAttachmentGrant(attachment.id)
        : await file.arrayBuffer()
      return {
        dataBase64: base64FromArrayBuffer(bytes),
        id: attachment.id,
        kind: "image",
        mime: attachment.mime || mime,
        name: attachment.name || file?.name || grant?.name || "image",
        sizeBytes: attachment.sizeBytes || file?.size || grant?.sizeBytes || 0,
      }
    },
    attachment => attachment.name || attachment.id,
  )

const resetComposerDraft = (): void => {
  const grantIds = [...localNativeAttachmentGrantIds.values()]
  if (grantIds.length > 0) {
    void controls.composerNativeFileGrantRelease({ grantIds }).catch(() => undefined)
  }
  for (const url of objectUrls) URL.revokeObjectURL(url)
  objectUrls.clear()
  localAttachmentFiles.clear()
  localNativeAttachmentGrantIds.clear()
  localNativeAttachmentGrantLabels.clear()
  localTextAttachments.clear()
  composerContextChips = []
  setComposerDraftText("")
  setShellComposerState(emptyComposerState())
  renderComposer()
}

const stopActiveTurn = (): void => {
  if (!shellModel().pendingTurn) return
  const stoppedTurnIds = [...activeTurnIds]
  for (const turnId of stoppedTurnIds) {
    void rpc.request.codexTurnInterrupt({ sessionId, turnId }).catch(() => undefined)
    activeTurnStartTimes.delete(turnId)
    turnFirstVisibleEventRecorded.delete(turnId)
    streamingThreadIds.delete(turnId)
  }
  activeTurnIds.clear()
  setShellFollowUpDrafts([])
  recomputePendingTurnForActiveThread()
  shellModel().thinkingTurnId = null
  threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
  appendMessages([
    {
      body: "Requested Codex interrupt for the active turn. You can keep typing.",
      id: nextMessageId("system"),
      role: "system",
    },
  ])
  requestAnimationFrame(focusComposerInput)
}

const handleSlashCommandClientAction = async (
  result: Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>,
): Promise<string> => {
  switch (result.action) {
    case "architect_plan":
      return "Type /architect followed by a public-safe objective."
    case "clear_visible_transcript":
      setShellMessages([])
      setShellFollowUpDrafts([])
      activeTurnIds.clear()
      render()
      return "Cleared the visible transcript."
    case "copy_last_assistant_message": {
      const assistant = [...shellModel().messages].reverse().find(message => message.role === "assistant")
      if (assistant === undefined) return "No assistant message is available to copy."
      await navigator.clipboard?.writeText(assistant.body)
      return "Copied the last assistant message."
    }
    case "show_desktop_status": {
      const [harness, appServer] = await Promise.all([
        rpc.request.codexHarnessStatus(),
        rpc.request.codexAppServerStatus(),
      ])
      return [
        `Codex harness: ${harness.status}`,
        `Codex app-server: ${appServer.state}`,
        `Codex auth: ${harness.auth.state}`,
      ].join("\n")
    }
    default:
      return result.message
  }
}

const appendSlashCommandResult = async (
  result: Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>,
): Promise<void> => {
  const body =
    result.status === "client_action"
      ? await handleSlashCommandClientAction(result)
      : result.message
  appendMessages([{
    body,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const submitArchitectPlan = async (objective: string): Promise<KhalaCodeDesktopMessage | null> => {
  if (objective.trim().length === 0) return null
  shellModel().lastSubmittedDraft = objective
  shellModel().lastTurnFailed = false
  setArchitectPlanPending(true)
  resetComposerDraft()
  const message = addMessage("user", `/architect ${objective}`)
  renderComposer()
  try {
    const activeThreadId = shellModel().activeCodexThreadId
    const result = await controls.architectPlanRun({
      objective,
      sessionId,
      ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
    })
    if (!result.ok) {
      appendMessages([{ body: `Architect plan failed: ${result.error}`, id: nextMessageId("system"), role: "system" }])
      shellModel().lastTurnFailed = true
      return message
    }
    setArchitectPlanArtifact(result.artifact)
    appendMessages([{
      body: `Claude architect returned plan ${result.artifact.planRef} with ${result.artifact.dag.nodes.length} nodes.`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } catch (error) {
    shellModel().lastTurnFailed = true
    appendMessages([{
      body: `Architect plan failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } finally {
    setArchitectPlanPending(false)
    setArchitectPlanMode(false)
    renderComposer()
    requestAnimationFrame(focusComposerInput)
  }
  return message
}

const submitSlashCommand = async (
  draftText: string,
  body: string,
): Promise<KhalaCodeDesktopMessage> => {
  shellModel().lastSubmittedDraft = draftText
  shellModel().lastTurnFailed = false
  resetComposerDraft()
  const message = addMessage("user", body)
  try {
    const result = await rpc.request.slashCommandDispatch({
      activeTurn: shellModel().pendingTurn,
      platform: slashCommandPlatform(),
      raw: draftText,
      sessionId,
      sideConversation: false,
    })
    await appendSlashCommandResult(result)
  } catch (error) {
    appendMessages([{
      body: `Slash command failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } finally {
    requestAnimationFrame(focusComposerInput)
  }
  return message
}

const queueFollowUpDraft = (text: string): void => {
  setShellFollowUpDrafts([
    ...shellModel().followUpDrafts,
    { id: nextFollowUpDraftId(), text },
  ])
  resetComposerDraft()
  requestAnimationFrame(focusComposerInput)
}

const submitKhalaSyncChatMessage = async (
  threadId: string,
  message: KhalaCodeDesktopMessage,
): Promise<void> => {
  const result = await controls.khalaSyncChatAppendMessage({
    body: message.body,
    messageId: message.id,
    threadId,
  })
  if (!result.ok) {
    shellModel().lastTurnFailed = true
    appendMessages([{
      body: `Khala Sync append failed: ${result.error ?? "unknown error"}`,
      id: nextMessageId("system"),
      role: "system",
    }])
    return
  }
  cacheVisibleThreadMessages()
  void threadSidebar?.refresh()
}

const submitComposer = async (): Promise<KhalaCodeDesktopMessage | null> => {
  if (shellModel().pendingTurn) {
    const draftRaw = composerDraftText()
    const draftText = draftRaw.trim()
    const pendingContexts = [...composerContextChips]
    if (
      draftText === "" &&
      shellModel().composerState.doc.attachments.length === 0 &&
      pendingContexts.length === 0
    ) return null
    if (shellModel().composerState.doc.attachments.length > 0) {
      appendMessages([{
        body: "Finish the active turn before sending attachments. Text follow-ups can be queued now.",
        id: nextMessageId("system"),
        role: "system",
      }])
      return null
    }
    composerPromptHistory.push(composerMode, draftRaw)
    if (draftText.startsWith("/")) {
      if (draftText === "/architect" || draftText.startsWith("/architect ")) {
        return submitArchitectPlan(draftText.replace(/^\/architect\b/u, "").trim())
      }
      return submitSlashCommand(draftText, draftText)
    }
    queueFollowUpDraft(submittedBody(draftText, [], pendingContexts))
    return null
  }
  const draftRaw = composerDraftText()
  const draftText =
    draftRaw.trim() === "" && shellModel().lastTurnFailed
      ? shellModel().lastSubmittedDraft
      : draftRaw.trim()
  if (
    draftText === "" &&
    shellModel().composerState.doc.attachments.length === 0 &&
    composerContextChips.length === 0
  ) return null

  const attachments = [...shellModel().composerState.doc.attachments]
  const contexts = [...composerContextChips]
  const body = submittedBody(draftText, attachments, contexts)
  composerPromptHistory.push(composerMode, draftRaw)
  if (draftText.startsWith("/")) {
    if (draftText === "/architect" || draftText.startsWith("/architect ")) {
      return submitArchitectPlan(draftText.replace(/^\/architect\b/u, "").trim())
    }
    return submitSlashCommand(draftText, body)
  }
  if (shellModel().architectPlanMode) {
    if (attachments.length > 0) {
      appendMessages([{
        body: "Architect plan mode is read-only and accepts text objectives only. Remove attachments or turn plan mode off.",
        id: nextMessageId("system"),
        role: "system",
      }])
      return null
    }
    return submitArchitectPlan(draftText)
  }
  const { resolved: imageAttachments, failures: imageAttachmentFailures } =
    await imageAttachmentsForSubmit(attachments)
  if (imageAttachmentFailures.length > 0) {
    appendMessages([{
      body: `Skipped ${imageAttachmentFailures.length} attachment${imageAttachmentFailures.length === 1 ? "" : "s"} that could not be read: ${imageAttachmentFailures.map(failure => failure.name).join(", ")}.`,
      id: nextMessageId("system"),
      role: "system",
    }])
  }
  shellModel().lastSubmittedDraft = draftText
  shellModel().lastTurnFailed = false
  resetComposerDraft()
  const submittedThreadId = shellModel().activeCodexThreadId
  const message = addMessage("user", body)
  if (submittedThreadId !== null && khalaSyncChatThreadIds.has(submittedThreadId)) {
    await submitKhalaSyncChatMessage(submittedThreadId, message)
    requestAnimationFrame(focusComposerInput)
    return message
  }
  const turnId = nextTurnId()
  activeTurnIds.add(turnId)
  streamingThreadIds.set(turnId, submittedThreadId)
  recomputePendingTurnForActiveThread()
  shellModel().thinkingTurnId = turnId
  threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
  render()
  requestAnimationFrame(focusComposerInput)
  const turnStartedAt = performance.now()
  activeTurnStartTimes.set(turnId, turnStartedAt)
  try {
    const request: KhalaCodeDesktopChatTurnRequest = {
      ...(imageAttachments.length === 0 ? {} : { attachments: imageAttachments }),
      composerSelection: composerTurnSelection(),
      messages: shellModel().messages,
      sessionId,
      ...(submittedThreadId === null ? { startNewThread: true } : { threadId: submittedThreadId }),
      turnId,
    }
    const response = await rpc.request.submitChatMessage(request)
    pushQaMetricSample("turn_start.latency_ms", performance.now() - turnStartedAt, {
      runtimeMode: response.backend.runtimeMode ?? shellModel().lastResponseRuntimeMode,
    })
    if (activeTurnIds.has(turnId)) {
      if (response.backend.runtimeMode !== undefined) {
        shellModel().lastResponseRuntimeMode = response.backend.runtimeMode
      }
      if (response.backend.threadId !== undefined) {
        setActiveCodexThreadId(response.backend.threadId)
        threadSidebar?.upsertPendingThread({
          preview: message.body,
          threadId: response.backend.threadId,
        })
        enqueueKhalaSyncChatThreadCreate({
          threadId: response.backend.threadId,
          title: message.body,
        })
        void threadSidebar?.refresh()
      }
      if (shellModel().thinkingTurnId === turnId) shellModel().thinkingTurnId = null
      appendMessages(response.messages)
    }
  } catch (error) {
    if (activeTurnIds.has(turnId)) {
      shellModel().lastTurnFailed = true
      appendMessages([
        {
          body: `Khala Code turn failed: ${error instanceof Error ? error.message : String(error)}`,
          id: nextMessageId("system"),
          role: "system",
        },
      ])
    }
  } finally {
    activeTurnIds.delete(turnId)
    streamingThreadIds.delete(turnId)
    activeTurnStartTimes.delete(turnId)
    turnFirstVisibleEventRecorded.delete(turnId)
    if (shellModel().thinkingTurnId === turnId) shellModel().thinkingTurnId = null
    recomputePendingTurnForActiveThread()
    threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
    void threadSidebar?.refresh()
    renderComposer()
    requestAnimationFrame(focusComposerInput)
  }
  return message
}

/**
 * `submitComposer()` is invoked as a bare `void submitComposer()` at some
 * call sites with no `.catch`. Its own internal turn-submission try/catch
 * reports turn failures, but anything that throws before or around that
 * (e.g. an unexpected attachment-resolution error) would otherwise become an
 * unhandled promise rejection with zero user feedback. This reporter backs
 * every `void submitComposer()` call site with a system-message fallback.
 */
const reportSubmitComposerFailure = (error: unknown): void => {
  appendMessages([{
    body: `Failed to send message: ${error instanceof Error ? error.message : String(error)}`,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const fileLikeFor = (file: File): ComposerFileLike => {
  const previewUrl = file.type.startsWith("image/")
    ? URL.createObjectURL(file)
    : undefined
  if (previewUrl !== undefined) objectUrls.add(previewUrl)
  return {
    name: file.name,
    type: file.type,
    size: file.size,
    ...(previewUrl === undefined ? {} : { previewUrl }),
  }
}

const fileLikeForNativeGrant = (
  grant: KhalaCodeDesktopComposerNativeFileGrant,
): ComposerFileLike => ({
  name: grant.name,
  size: grant.sizeBytes,
  type: grant.mime,
})

const releaseNativeAttachmentGrant = (attachmentId: string): void => {
  const grantId = localNativeAttachmentGrantIds.get(attachmentId)
  if (grantId === undefined) return
  localNativeAttachmentGrantIds.delete(attachmentId)
  localNativeAttachmentGrantLabels.delete(attachmentId)
  void controls.composerNativeFileGrantRelease({ grantIds: [grantId] }).catch(() => undefined)
}

const bytesForNativeAttachmentGrant = async (
  attachmentId: string,
): Promise<ArrayBuffer> => {
  const grantId = localNativeAttachmentGrantIds.get(attachmentId)
  if (grantId === undefined) throw new Error("Native file grant is missing.")
  const result = await controls.composerNativeFileGrantRead({ grantId })
  if (!result.ok) throw new Error(result.error)
  return arrayBufferFromBase64(result.dataBase64)
}

const stageFiles = (
  files: readonly File[],
  source: ComposerAttachmentSource,
): void => {
  if (files.length === 0) return
  const fileLikes = files.map(fileLikeFor)
  const staged =
    source === "paste"
      ? stageComposerPastedFiles(fileLikes)
      : source === "drop"
        ? stageComposerDroppedFiles(fileLikes)
        : stageComposerAttachmentFiles(fileLikes, { source: "manual" })
  if (!applyComposerStateTransaction(staged.transaction)) return
  staged.attachments.forEach((attachment, index) => {
    const file = files[index]
    if (file === undefined) return
    localAttachmentFiles.set(attachment.id, file)
    void runDesktopLocalAttachmentUpload(attachment.id, () => file.arrayBuffer())
  })
}

const stageNativeFileGrants = (
  grants: readonly KhalaCodeDesktopComposerNativeFileGrant[],
): void => {
  if (grants.length === 0) return
  const staged = stageComposerAttachmentFiles(grants.map(fileLikeForNativeGrant), {
    source: "manual",
  })
  if (!applyComposerStateTransaction(staged.transaction)) {
    void controls.composerNativeFileGrantRelease({
      grantIds: grants.map(grant => grant.grantId),
    }).catch(() => undefined)
    return
  }
  staged.attachments.forEach((attachment, index) => {
    const grant = grants[index]
    if (grant === undefined) return
    localNativeAttachmentGrantIds.set(attachment.id, grant.grantId)
    localNativeAttachmentGrantLabels.set(attachment.id, grant)
    void runDesktopLocalAttachmentUpload(attachment.id, () =>
      bytesForNativeAttachmentGrant(attachment.id))
  })
}

const stageLargeTextPaste = (text: string): boolean => {
  const offer = offerComposerLargeTextPaste(text)
  if (!offer.offered || offer.transaction === null || offer.attachment === undefined) {
    return false
  }
  if (offer.attachment.contentRef !== undefined) {
    localTextAttachments.set(offer.attachment.contentRef, text)
  }
  const applied = applyComposerStateTransaction(offer.transaction)
  if (applied) {
    void runDesktopLocalAttachmentUpload(
      offer.attachment.id,
      () => Promise.resolve(arrayBufferForText(text)),
    )
  }
  return applied
}

const setDragActive = (active: boolean): void => {
  shellModel().dragActive = active
  renderComposer()
}

const resizeComposerHud = (): void => {
  if (composerHudRuntime === null) return
  const rect = composerFrame.getBoundingClientRect()
  const width = Math.max(1, Math.ceil(rect.width))
  const height = Math.max(1, Math.ceil(rect.height))
  composerHudRuntime.renderer.setSize(width, height, false)
  const layout = commandComposerHudLayoutFromCssRect(rect, {
    pixelsPerWorldUnit: 100,
    minWorldWidth: 2,
    minWorldHeight: 1,
  })
  composerHudRuntime.camera.left = -layout.width / 2 - 0.18
  composerHudRuntime.camera.right = layout.width / 2 + 0.18
  composerHudRuntime.camera.top = layout.height / 2 + 0.18
  composerHudRuntime.camera.bottom = -layout.height / 2 - 0.18
  composerHudRuntime.camera.updateProjectionMatrix()
  composerHudRuntime.handle.setLayout(layout)
  composerHudRuntime.handle.setResolution(width, height)
  updateComposerHudProjection()
}

const mountComposerHud = (): ComposerHudRuntime | null => {
  try {
    const renderer = new Three.WebGLRenderer({
      antialias: true,
      alpha: true,
    })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setClearColor(0x000000, 0)
    composerHudMount.replaceChildren(renderer.domElement)

    const scene = new Three.Scene()
    const camera = new Three.OrthographicCamera(-4, 4, 1.2, -1.2, 0.1, 20)
    camera.position.set(0, 0, 5)
    camera.lookAt(0, 0, 0)
    const handle = createCommandComposerHud({
      focused: true,
      reducedMotion: prefersReducedMotion?.matches === true,
      layout: commandComposerHudLayoutFromCssRect(
        composerFrame.getBoundingClientRect(),
        { pixelsPerWorldUnit: 100, minWorldWidth: 2, minWorldHeight: 1 },
      ),
    })
    scene.add(handle.object3D)

    let disposed = false
    let lastTime = performance.now()
    const frame = (time: number): void => {
      if (disposed) return
      const delta = Math.max(0, (time - lastTime) / 1000)
      lastTime = time
      handle.update(delta)
      renderer.render(scene, camera)
      requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)

    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => resizeComposerHud())

    const runtime: ComposerHudRuntime = {
      renderer,
      scene,
      camera,
      handle,
      dispose: () => {
        if (disposed) return
        disposed = true
        observer?.disconnect()
        handle.dispose()
        renderer.dispose()
        renderer.domElement.remove()
      },
    }
    composerHudRuntime = runtime
    observer?.observe(composerFrame)
    resizeComposerHud()
    return runtime
  } catch {
    composerHudMount.hidden = true
    return null
  }
}

const openComposerAttachmentPicker = async (): Promise<void> => {
  try {
    const result = await controls.composerNativeFilePickerOpen({
      maxFiles: 16,
      multiple: true,
    })
    if (result.ok && result.files.length > 0) {
      stageNativeFileGrants(result.files)
      requestAnimationFrame(focusComposerInput)
      return
    }
    if (result.ok && result.cancelled) {
      requestAnimationFrame(focusComposerInput)
      return
    }
  } catch {
    // Browser preview and older native shells fall back to the hidden input.
  }
  fileInput.click()
}

const tryStageNativeClipboardImage = async (): Promise<boolean> => {
  try {
    const result = await controls.composerNativeClipboardImageRead()
    if (!result.ok) return false
    stageNativeFileGrants([result.file])
    requestAnimationFrame(focusComposerInput)
    return true
  } catch {
    return false
  }
}

composerForm.addEventListener("submit", event => {
  event.preventDefault()
  void submitComposer()
    .catch(error => reportSubmitComposerFailure(error))
    .finally(focusComposerInput)
})

sendButton.addEventListener("click", event => {
  if (!shellModel().pendingTurn) return
  event.preventDefault()
  stopActiveTurn()
})

attachButton.addEventListener("click", () => {
  void openComposerAttachmentPicker()
})

fileInput.addEventListener("change", () => {
  stageFiles(Array.from(fileInput.files ?? []), "manual")
  fileInput.value = ""
  requestAnimationFrame(focusComposerInput)
})

composerInput.addEventListener("input", () => {
  const echoStartedAt = performance.now()
  shellModel().lastTurnFailed = false
  syncComposerDraftState()
  renderComposer()
  markQaTimer("composer.keystroke_echo_ms", echoStartedAt, {
    characters: composerDraftText().length,
    surface: "composer",
  })
})

composerInput.addEventListener("focus", updateComposerHudProjection)
composerInput.addEventListener("blur", updateComposerHudProjection)
composerInput.addEventListener("compositionstart", () => {
  composerIsComposing = true
})
composerInput.addEventListener("compositionend", () => {
  composerIsComposing = false
  syncComposerDraftState()
  renderComposer()
})

composerInput.addEventListener("keydown", event => {
  const command = khalaRichComposerCommandForKey(
    {
      altKey: event.altKey,
      ctrlKey: event.ctrlKey,
      isComposing: composerIsComposing || event.isComposing,
      key: event.key,
      metaKey: event.metaKey,
      shiftKey: event.shiftKey,
    },
    composerDraftText(),
  )
  if (command === "submit" && canSubmitComposer()) {
    event.preventDefault()
    void submitComposer().catch(error => reportSubmitComposerFailure(error))
    return
  }
  if (command === "history-previous") {
    const previous = composerPromptHistory.previous(composerMode, composerDraftText())
    if (previous === null) return
    event.preventDefault()
    setComposerDraftText(previous)
    renderComposer()
    setKhalaComposerCaretToEnd(composerInput)
    return
  }
  if (command === "history-next") {
    const next = composerPromptHistory.next(composerMode)
    if (next === null) return
    event.preventDefault()
    setComposerDraftText(next)
    renderComposer()
    setKhalaComposerCaretToEnd(composerInput)
  }
})

composerInput.addEventListener("paste", event => {
  const data = event.clipboardData
  if (data === null) return
  const files = Array.from(data.files)
  if (files.length > 0) {
    event.preventDefault()
    stageFiles(files, "paste")
    requestAnimationFrame(focusComposerInput)
    return
  }
  const text = data.getData("text/plain")
  if (text === "") {
    event.preventDefault()
    void tryStageNativeClipboardImage().then(staged => {
      if (!staged) requestAnimationFrame(focusComposerInput)
    })
    return
  }
  const normalized = normalizeKhalaComposerPasteText(text)
  if (stageLargeTextPaste(normalized)) {
    event.preventDefault()
    requestAnimationFrame(focusComposerInput)
    return
  }
  event.preventDefault()
  insertKhalaComposerTextAtSelection(composerInput, normalized)
  shellModel().lastTurnFailed = false
  renderComposer()
})

for (const target of [composerForm, composerRail]) {
  target.addEventListener("dragenter", event => {
    event.preventDefault()
    setDragActive(true)
  })
  target.addEventListener("dragover", event => {
    event.preventDefault()
    setDragActive(true)
  })
  target.addEventListener("dragleave", event => {
    if (!composerForm.contains(event.relatedTarget as Node | null)) {
      setDragActive(false)
    }
  })
  target.addEventListener("drop", event => {
    event.preventDefault()
    setDragActive(false)
    stageFiles(Array.from(event.dataTransfer?.files ?? []), "drop")
    requestAnimationFrame(focusComposerInput)
  })
}

window.addEventListener("resize", resizeComposerHud)
window.setInterval(() => {
  void pollClaudeApprovals()
}, 1000)
messageList.addEventListener("click", event => {
  const target = event.target
  if (!(target instanceof Element)) return
  const button = target.closest<HTMLButtonElement>(".codex-approval-button")
  if (button === null) return
  event.preventDefault()
  event.stopPropagation()
  void respondToCodexApproval(button)
})
messageList.addEventListener(KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT, event => {
  event.preventDefault()
  event.stopPropagation()
  void handleDiffReviewSubmit(event)
})
messageList.addEventListener(KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT, event => {
  event.preventDefault()
  event.stopPropagation()
  void handleSourceControlActionSubmit(event)
})
messageList.addEventListener("scroll", () => {
  shellModel().transcriptPinnedToEnd = isNearTranscriptEnd()
}, { passive: true })
threadTokenCounter.addEventListener("click", () => {
  shellModel().threadTokenPopoverOpen = !shellModel().threadTokenPopoverOpen
  renderThreadTokenCounter()
  if (shellModel().threadTokenPopoverOpen) void refreshThreadTokenSummary()
})
document.addEventListener("click", event => {
  if (!shellModel().threadTokenPopoverOpen) return
  const target = event.target
  if (target instanceof Node && threadTokenMeter.contains(target)) return
  shellModel().threadTokenPopoverOpen = false
  renderThreadTokenCounter()
})
window.addEventListener("wheel", proxyTranscriptWheel, { passive: false })
window.addEventListener("keydown", event => {
  if (event.key === "Escape" && shellModel().threadTokenPopoverOpen) {
    shellModel().threadTokenPopoverOpen = false
    renderThreadTokenCounter()
    return
  }
  proxyTranscriptKeyScroll(event)
})
// KS-6.8 (#8418): the old unconditional "poll every 2s while a thread is
// open" ambient timer is gone. `syncThreadTokenPolling` (see its definition
// above `refreshThreadTokenSummary`) now schedules the same 2s interval only
// while `pendingTurn` is true for the active thread, and does one trailing
// refresh the moment a turn stops — see `recomputePendingTurnForActiveThread`
// and the direct `pendingTurn` assignment sites that call it.
window.addEventListener("beforeunload", () => {
  const grantIds = [...localNativeAttachmentGrantIds.values()]
  if (grantIds.length > 0) {
    void controls.composerNativeFileGrantRelease({ grantIds }).catch(() => undefined)
  }
  for (const url of objectUrls) URL.revokeObjectURL(url)
  composerHudRuntime?.dispose()
})

const controls = {
  addMessage,
  appInfo: () => rpc.request.appInfo(),
  attachments: () => shellModel().composerState.doc.attachments.map(attachment => ({ ...attachment })),
  attachmentReceipts: () => shellModel().composerAttachmentReceipts.map(receipt => ({ ...receipt })),
  bootDegradedStates: () => shellModel().bootDegradedStates.map(state => ({ ...state })),
  composerContextChips: () => composerContextChips.map(chip => ({ ...chip })),
  composerWorkDockItems: () => composerWorkDockItems().map(item => ({ ...item })),
  clearGymProof: (): GymPaneState => {
    const state = gymPaneStateFromBridgeProof(null)
    gymPanel?.setState(state)
    gymPanel?.setVisible(false)
    return state
  },
  codexAccountsStatus: () => rpc.request.codexAccountsStatus(),
  codexAppServerRestart: () => rpc.request.codexAppServerRestart(),
  codexAppServerStart: () => rpc.request.codexAppServerStart(),
  codexAppServerStatus: () => rpc.request.codexAppServerStatus(),
  codexAppServerStop: () => rpc.request.codexAppServerStop(),
  codexFleetDelegateRun: (request: Parameters<DesktopRpcRequests["codexFleetDelegateRun"]>[0]) =>
    rpc.request.codexFleetDelegateRun(request),
  codexFleetStatus: () => rpc.request.codexFleetStatus(),
  codexFleetPromoteThread: (request: Parameters<DesktopRpcRequests["codexFleetPromoteThread"]>[0]) =>
    rpc.request.codexFleetPromoteThread(request),
  fleetRunControl: (request: Parameters<DesktopRpcRequests["fleetRunControl"]>[0]) =>
    rpc.request.fleetRunControl(request),
  fleetRunList: (request?: Parameters<DesktopRpcRequests["fleetRunList"]>[0]) =>
    rpc.request.fleetRunList(request),
  fleetRunStart: (request: Parameters<DesktopRpcRequests["fleetRunStart"]>[0]) =>
    rpc.request.fleetRunStart(request),
  architectPlanRun: (request: Parameters<DesktopRpcRequests["architectPlanRun"]>[0]) =>
    rpc.request.architectPlanRun(request),
  architectPlanDecision: (request: Parameters<DesktopRpcRequests["architectPlanDecision"]>[0]) =>
    rpc.request.architectPlanDecision(request),
  fleetWorkerControl: (request: Parameters<DesktopRpcRequests["fleetWorkerControl"]>[0]) =>
    rpc.request.fleetWorkerControl(request),
  khalaSyncFleetState: (request: Parameters<DesktopRpcRequests["khalaSyncFleetState"]>[0]) =>
    rpc.request.khalaSyncFleetState(request),
  khalaSyncFleetMutate: (request: Parameters<DesktopRpcRequests["khalaSyncFleetMutate"]>[0]) =>
    rpc.request.khalaSyncFleetMutate(request),
  khalaSyncFleetReportAccountState: (
    request: Parameters<DesktopRpcRequests["khalaSyncFleetReportAccountState"]>[0],
  ) => rpc.request.khalaSyncFleetReportAccountState(request),
  khalaSyncChatThreads: (request?: Parameters<DesktopRpcRequests["khalaSyncChatThreads"]>[0]) =>
    rpc.request.khalaSyncChatThreads(request),
  khalaSyncChatMessages: (request: Parameters<DesktopRpcRequests["khalaSyncChatMessages"]>[0]) =>
    rpc.request.khalaSyncChatMessages(request),
  khalaSyncChatCreateThread: (request: Parameters<DesktopRpcRequests["khalaSyncChatCreateThread"]>[0]) =>
    rpc.request.khalaSyncChatCreateThread(request),
  khalaSyncChatAppendMessage: (request: Parameters<DesktopRpcRequests["khalaSyncChatAppendMessage"]>[0]) =>
    rpc.request.khalaSyncChatAppendMessage(request),
  khalaSyncChatRenameThread: (request: Parameters<DesktopRpcRequests["khalaSyncChatRenameThread"]>[0]) =>
    rpc.request.khalaSyncChatRenameThread(request),
  editorProviderList: () => rpc.request.editorProviderList(),
  editorWorkspaceRead: () => rpc.request.editorWorkspaceRead(),
  editorDirectoryRead: (request?: Parameters<DesktopRpcRequests["editorDirectoryRead"]>[0]) =>
    rpc.request.editorDirectoryRead(request),
  editorFileRead: (request: Parameters<DesktopRpcRequests["editorFileRead"]>[0]) =>
    rpc.request.editorFileRead(request),
  reviewDiffRead: (request?: Parameters<DesktopRpcRequests["reviewDiffRead"]>[0]) =>
    rpc.request.reviewDiffRead(request),
  composerNativeFilePickerOpen: (request?: Parameters<DesktopRpcRequests["composerNativeFilePickerOpen"]>[0]) =>
    rpc.request.composerNativeFilePickerOpen(request),
  composerNativeDirectoryPickerOpen: (request?: Parameters<DesktopRpcRequests["composerNativeDirectoryPickerOpen"]>[0]) =>
    rpc.request.composerNativeDirectoryPickerOpen(request),
  composerNativeSaveDialogOpen: (request?: Parameters<DesktopRpcRequests["composerNativeSaveDialogOpen"]>[0]) =>
    rpc.request.composerNativeSaveDialogOpen(request),
  composerNativeClipboardImageRead: () => rpc.request.composerNativeClipboardImageRead(),
  composerNativeFileGrantRead: (request: Parameters<DesktopRpcRequests["composerNativeFileGrantRead"]>[0]) =>
    rpc.request.composerNativeFileGrantRead(request),
  composerNativeFileGrantRelease: (request: Parameters<DesktopRpcRequests["composerNativeFileGrantRelease"]>[0]) =>
    rpc.request.composerNativeFileGrantRelease(request),
  forumRequest: (request: Parameters<DesktopRpcRequests["forumRequest"]>[0]) =>
    rpc.request.forumRequest(request),
  khalaCodePlanCatalog: () => rpc.request.khalaCodePlanCatalog(),
  khalaCodePlanStatus: () => rpc.request.khalaCodePlanStatus(),
  khalaCodeOpenAgentsAuthStatus: () =>
    rpc.request.khalaCodeOpenAgentsAuthStatus(),
  khalaCodeOpenAgentsAuthStart: () =>
    rpc.request.khalaCodeOpenAgentsAuthStart(),
  khalaCodeOpenAgentsAuthPoll: () =>
    rpc.request.khalaCodeOpenAgentsAuthPoll(),
  khalaCodePlanPurchase: (request?: Parameters<DesktopRpcRequests["khalaCodePlanPurchase"]>[0]) =>
    rpc.request.khalaCodePlanPurchase(request),
  khalaCodeTraceCaptureStatus: () => rpc.request.khalaCodeTraceCaptureStatus(),
  khalaCodeTraceCaptureConsentWrite: (request: Parameters<DesktopRpcRequests["khalaCodeTraceCaptureConsentWrite"]>[0]) =>
    rpc.request.khalaCodeTraceCaptureConsentWrite(request),
  khalaCodeOutsideUserRunReport: (request?: Parameters<DesktopRpcRequests["khalaCodeOutsideUserRunReport"]>[0]) =>
    rpc.request.khalaCodeOutsideUserRunReport(request),
  claudeApprovalPending: () => rpc.request.claudeApprovalPending(),
  claudeApprovalRespond: (request: Parameters<DesktopRpcRequests["claudeApprovalRespond"]>[0]) =>
    rpc.request.claudeApprovalRespond(request),
  claudeSettingsRead: () => rpc.request.claudeSettingsRead(),
  codexHarnessStatus: () => rpc.request.codexHarnessStatus(),
  codexApprovalRespond: (request: Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]) =>
    rpc.request.codexApprovalRespond(request),
  codexBackgroundTerminalsClean: (request: Parameters<DesktopRpcRequests["codexBackgroundTerminalsClean"]>[0]) =>
    rpc.request.codexBackgroundTerminalsClean(request),
  codexBackgroundTerminalsList: (request: Parameters<DesktopRpcRequests["codexBackgroundTerminalsList"]>[0]) =>
    rpc.request.codexBackgroundTerminalsList(request),
  codexBackgroundTerminalsTerminate: (request: Parameters<DesktopRpcRequests["codexBackgroundTerminalsTerminate"]>[0]) =>
    rpc.request.codexBackgroundTerminalsTerminate(request),
  codexConfigValueWrite: (request: Parameters<DesktopRpcRequests["codexConfigValueWrite"]>[0]) =>
    rpc.request.codexConfigValueWrite(request),
  codexEcosystemRead: (request?: Parameters<DesktopRpcRequests["codexEcosystemRead"]>[0]) =>
    rpc.request.codexEcosystemRead(request),
  codexExternalAgentConfigDetect: (request?: Parameters<DesktopRpcRequests["codexExternalAgentConfigDetect"]>[0]) =>
    rpc.request.codexExternalAgentConfigDetect(request),
  codexExternalAgentConfigImport: (request: Parameters<DesktopRpcRequests["codexExternalAgentConfigImport"]>[0]) =>
    rpc.request.codexExternalAgentConfigImport(request),
  codexExternalAgentConfigImportHistoriesRead: () =>
    rpc.request.codexExternalAgentConfigImportHistoriesRead(),
  codexFsGetMetadata: (request: Parameters<DesktopRpcRequests["codexFsGetMetadata"]>[0]) =>
    rpc.request.codexFsGetMetadata(request),
  codexFsReadFile: (request: Parameters<DesktopRpcRequests["codexFsReadFile"]>[0]) =>
    rpc.request.codexFsReadFile(request),
  codexFsWriteFile: (request: Parameters<DesktopRpcRequests["codexFsWriteFile"]>[0]) =>
    rpc.request.codexFsWriteFile(request),
  codexMarketplaceAdd: (request: Parameters<DesktopRpcRequests["codexMarketplaceAdd"]>[0]) =>
    rpc.request.codexMarketplaceAdd(request),
  codexMarketplaceRemove: (request: Parameters<DesktopRpcRequests["codexMarketplaceRemove"]>[0]) =>
    rpc.request.codexMarketplaceRemove(request),
  codexMarketplaceUpgrade: (request?: Parameters<DesktopRpcRequests["codexMarketplaceUpgrade"]>[0]) =>
    rpc.request.codexMarketplaceUpgrade(request),
  codexMentionCandidates: (request?: Parameters<DesktopRpcRequests["codexMentionCandidates"]>[0]) =>
    rpc.request.codexMentionCandidates(request),
  codexMcpOauthLogin: (request: Parameters<DesktopRpcRequests["codexMcpOauthLogin"]>[0]) =>
    rpc.request.codexMcpOauthLogin(request),
  codexMcpResourceRead: (request: Parameters<DesktopRpcRequests["codexMcpResourceRead"]>[0]) =>
    rpc.request.codexMcpResourceRead(request),
  codexMcpServerReload: () => rpc.request.codexMcpServerReload(),
  codexMcpToolCall: (request: Parameters<DesktopRpcRequests["codexMcpToolCall"]>[0]) =>
    rpc.request.codexMcpToolCall(request),
  codexPluginInstall: (request: Parameters<DesktopRpcRequests["codexPluginInstall"]>[0]) =>
    rpc.request.codexPluginInstall(request),
  codexPluginUninstall: (request: Parameters<DesktopRpcRequests["codexPluginUninstall"]>[0]) =>
    rpc.request.codexPluginUninstall(request),
  codexModelRolePresetApply: (request: Parameters<DesktopRpcRequests["codexModelRolePresetApply"]>[0]) =>
    rpc.request.codexModelRolePresetApply(request),
  codexSettingsRead: (request?: Parameters<DesktopRpcRequests["codexSettingsRead"]>[0]) =>
    rpc.request.codexSettingsRead(request),
  modelRoleRegistryRead: () => rpc.request.modelRoleRegistryRead(),
  modelRoleRegistryWrite: (request: Parameters<DesktopRpcRequests["modelRoleRegistryWrite"]>[0]) =>
    rpc.request.modelRoleRegistryWrite(request),
  codexSkillsConfigWrite: (request: Parameters<DesktopRpcRequests["codexSkillsConfigWrite"]>[0]) =>
    rpc.request.codexSkillsConfigWrite(request),
  codexSkillsExtraRootsSet: (request: Parameters<DesktopRpcRequests["codexSkillsExtraRootsSet"]>[0]) =>
    rpc.request.codexSkillsExtraRootsSet(request),
  codexThreadArchive: (request: Parameters<DesktopRpcRequests["codexThreadArchive"]>[0]) =>
    rpc.request.codexThreadArchive(request),
  codexThreadCompact: (request: Parameters<DesktopRpcRequests["codexThreadCompact"]>[0]) =>
    rpc.request.codexThreadCompact(request),
  codexThreadDelete: (request: Parameters<DesktopRpcRequests["codexThreadDelete"]>[0]) =>
    rpc.request.codexThreadDelete(request),
  codexThreadFork: (request: Parameters<DesktopRpcRequests["codexThreadFork"]>[0]) =>
    rpc.request.codexThreadFork(request),
  codexThreadList: (request?: Parameters<DesktopRpcRequests["codexThreadList"]>[0]) =>
    rpc.request.codexThreadList(request),
  codexThreadRead: (request: Parameters<DesktopRpcRequests["codexThreadRead"]>[0]) =>
    rpc.request.codexThreadRead(request),
  codexThreadRename: (request: Parameters<DesktopRpcRequests["codexThreadRename"]>[0]) =>
    rpc.request.codexThreadRename(request),
  codexThreadResume: (request: Parameters<DesktopRpcRequests["codexThreadResume"]>[0]) =>
    rpc.request.codexThreadResume(request),
  codexThreadStart: (request?: Parameters<DesktopRpcRequests["codexThreadStart"]>[0]) =>
    rpc.request.codexThreadStart(request),
  codexThreadUnarchive: (request: Parameters<DesktopRpcRequests["codexThreadUnarchive"]>[0]) =>
    rpc.request.codexThreadUnarchive(request),
  sessionCatalog: (request?: Parameters<DesktopRpcRequests["sessionCatalog"]>[0]) =>
    rpc.request.sessionCatalog(request),
  codexTurnInterrupt: (request: Parameters<DesktopRpcRequests["codexTurnInterrupt"]>[0]) =>
    rpc.request.codexTurnInterrupt(request),
  codexTurnStart: (request: Parameters<DesktopRpcRequests["codexTurnStart"]>[0]) =>
    rpc.request.codexTurnStart(request),
  codexTurnSteer: (request: Parameters<DesktopRpcRequests["codexTurnSteer"]>[0]) =>
    rpc.request.codexTurnSteer(request),
  codingStatus: () => rpc.request.codingStatus(),
  connectCodexAccount: (accountRef: string) =>
    rpc.request.connectCodexAccount(accountRef),
  gymState: (): GymPaneState | null => gymPanel?.snapshot() ?? null,
  openExternalUrl: (url: string) => rpc.request.openExternalUrl(url),
  composerStatus: statusForComposer,
  consumeCodexRateLimitResetCredit: (request: Parameters<DesktopRpcRequests["consumeCodexRateLimitResetCredit"]>[0]) =>
    rpc.request.consumeCodexRateLimitResetCredit(request),
  focusComposer: focusComposerInput,
  followUpDrafts: () => shellModel().followUpDrafts.map(draft => ({ ...draft })),
  isComposerFocused: () => document.activeElement === composerInput,
  isPending: () => shellModel().pendingTurn,
  loadGymDemoProof: (): GymPaneState => {
    const state = gymPaneStateFromBridgeProof({
      proof: khalaCodeGymDemoBridgeProof,
      generatedAt: "time.khala_gym_projection.fixture",
      sourceRef: "fixture.khala_code.gym.part2_demo",
    })
    gymPanel?.setState(state)
    showGymProofPane()
    return state
  },
  loadGymProof: (
    input: KhalaGymBridgeProofLike | KhalaGymProofLoadRequest | string | null,
  ): GymPaneState => {
    const parsed =
      typeof input === "string"
        ? (JSON.parse(input) as KhalaGymBridgeProofLike | KhalaGymProofLoadRequest)
        : input
    const state = gymPaneStateFromBridgeProof(parsed)
    gymPanel?.setState(state)
    showGymProofPane()
    return state
  },
  messages: () => shellModel().messages.map(message => ({ ...message })),
  pylonStatus: () => rpc.request.pylonStatus(),
  qaMetrics: qaMetricsSnapshot,
  removeCodexAccount: (accountRef: string) =>
    rpc.request.removeCodexAccount(accountRef),
  setCodexAccountPaused: (request: { accountRef: string; paused: boolean }) =>
    rpc.request.setCodexAccountPaused(request),
  slashCommandDispatch: (request: Parameters<DesktopRpcRequests["slashCommandDispatch"]>[0]) =>
    rpc.request.slashCommandDispatch(request),
  slashCommandList: (request?: Parameters<DesktopRpcRequests["slashCommandList"]>[0]) =>
    rpc.request.slashCommandList(request),
  reset: () => {
    setShellMessages([])
    activeTurnIds.clear()
    streamingThreadIds.clear()
    setShellFollowUpDrafts([])
    resetComposerDraft()
    shellModel().pendingTurn = false
    syncThreadTokenPolling()
    shellModel().thinkingTurnId = null
    shellModel().lastTurnFailed = false
    render()
  },
  setComposerDraft: (value: string) => {
    setComposerDraftText(value)
    shellModel().lastTurnFailed = false
    renderComposer()
  },
  setGymState: (state: GymPaneState) => gymPanel?.setState(state),
  simulateLargePaste: (value: string) => stageLargeTextPaste(value),
  stageAttachmentForSmoke: (input: {
    name: string
    type?: string
    size?: number
    source?: ComposerAttachmentSource
  }) => {
    const staged = stageComposerAttachmentFiles(
      [
        {
          name: input.name,
          type: input.type ?? "text/plain",
          size: input.size ?? 10,
        },
      ],
      { source: input.source ?? "manual" },
    )
    applyComposerStateTransaction(staged.transaction)
  },
  addComposerContextForSmoke: (input: {
    body?: string
    displayPath: string
    kind?: ComposerContextChip["kind"]
    lineEnd?: number
    lineStart?: number
    title?: string
  }) => {
    const kind = input.kind ?? "file"
    addComposerContextChip({
      displayPath: input.displayPath,
      id: nextComposerContextChipId(kind),
      kind,
      ...(input.body === undefined ? {} : { body: input.body }),
      ...(input.lineEnd === undefined ? {} : { lineEnd: input.lineEnd }),
      ...(input.lineStart === undefined ? {} : { lineStart: input.lineStart }),
      title: input.title ?? input.displayPath,
    })
  },
  stopTurn: stopActiveTurn,
  submitComposer,
  tokenAccountingStatus: () => rpc.request.tokenAccountingStatus(),
  threadTokenSummary: (request?: Parameters<DesktopRpcRequests["threadTokenSummary"]>[0]) =>
    rpc.request.threadTokenSummary(request),
  threadSwitchPerformance: () => ({
    cachedThreadIds: [...threadMessageCache.keys()],
    latest: threadSwitchPerformanceSamples.at(-1) ?? null,
    pendingSelectionIds: [...pendingThreadSwitches.keys()],
    samples: threadSwitchPerformanceSamples.map(sample => ({ ...sample })),
  }),
  resetThreadSwitchPerformance: () => {
    threadSwitchPerformanceSamples.splice(0)
    pendingThreadSwitches.clear()
    qaMetricSamples.splice(0)
  },
  toolCatalog: () => rpc.request.toolCatalog(),
  updaterCheck: () => rpc.request.updaterCheck(),
  updaterDownload: () => rpc.request.updaterDownload(),
  updaterInstall: () => rpc.request.updaterInstall(),
  updaterStatus: () => rpc.request.updaterStatus(),
  diagnosticsSnapshot: () => rpc.request.diagnosticsSnapshot(),
  diagnosticsExport: () => rpc.request.diagnosticsExport(),
}

Object.assign(globalThis, {
  khalaCodeDesktop: controls,
})

void controls.appInfo().catch(() => undefined)
mountComposerHud()

const sidebarNavRoot = document.getElementById("sidebar-nav-root")
const threadSidebarEl = document.getElementById("thread-sidebar")
const fleetPanelEl = document.getElementById("fleet-panel")
const forumPanelEl = document.getElementById("forum-panel")
const inboxPanelEl = document.getElementById("inbox-panel")
const projectHomePanelEl = document.getElementById("project-home-panel")
const editorPanelEl = document.getElementById("editor-panel")
const reviewPanelEl = document.getElementById("review-panel")
const terminalPanelEl = document.getElementById("terminal-panel")
const gymPanelEl = document.getElementById("gym-panel")
const settingsPanelEl = document.getElementById("settings-panel")
const threadShell = document.querySelector<HTMLElement>(".khala-code-thread-shell")
const composerDock = document.querySelector<HTMLElement>(".composer-dock")
const initialGymState = gymPaneStateFromLocation(globalThis.location)
/**
 * Route/window state survives a desktop restart
 * (khala_code.project_home route-persistence gate, #8443): an explicit
 * `?view=` query param still wins (deep links, visual smokes), otherwise the
 * view active when the app last quit is restored from local storage.
 */
const activeViewStorageKey = "khala-code-desktop.active-view.v1"
const storedActiveView = localStorage.getItem(activeViewStorageKey)
const initialView = bootDeepLink?.ok === true
  ? viewForKhalaCodeDeepLinkTarget(bootDeepLink.target)
  : restoredKhalaCodeViewFromLocationAndStorage(globalThis.location, storedActiveView)

const setActiveCodexThreadId = (threadId: string | null): void => {
  const changed = shellModel().activeCodexThreadId !== threadId
  shellModel().activeCodexThreadId = threadId
  if (threadId === null) {
    localStorage.removeItem(activeThreadIdStorageKey)
  } else {
    localStorage.setItem(activeThreadIdStorageKey, threadId)
  }
  if (changed) {
    shellModel().threadTokenSummary = emptyThreadTokenSummary(threadId)
    renderThreadTokenCounter()
    void refreshThreadTokenSummary()
  }
  threadSidebar?.setActiveThreadId(threadId)
}

const beginCodexThreadSwitch = (input: {
  readonly selectionId: number
  readonly source: CodexThreadSelectionSource
  readonly threadId: string
}): void => {
  cacheVisibleThreadMessages()
  const cached = cachedThreadMessages(input.threadId)
  const optimisticMessages = cached === null
    ? null
    : recentMessagesForInitialThreadRender(cached)
  setActiveCodexThreadId(input.threadId)
  if (optimisticMessages !== null) {
    setShellMessages(optimisticMessages)
    threadSwitchLoadingSelectionId = null
  } else {
    setShellMessages([])
    threadSwitchLoadingSelectionId = input.selectionId
  }
  activeTurnIds.clear()
  setShellFollowUpDrafts([])
  recomputePendingTurnForActiveThread()
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  beginThreadSwitchPerformanceSample({
    cacheHit: cached !== null,
    optimisticMessageCount: optimisticMessages?.length ?? 0,
    selectionId: input.selectionId,
    source: input.source,
    threadId: input.threadId,
  })
  requestAnimationFrame(focusComposerInput)
}

const activateCodexThread = (input: {
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly selectionId?: number
  readonly threadId: string
}): void => {
  setActiveCodexThreadId(input.threadId)
  threadSwitchLoadingSelectionId = null
  cacheThreadMessages(input.threadId, input.messages)
  const visibleMessages = recentMessagesForInitialThreadRender(input.messages)
  setShellMessages(visibleMessages)
  const hydratedVisibleMessages = shellModel().messages
  activeTurnIds.clear()
  setShellFollowUpDrafts([])
  recomputePendingTurnForActiveThread()
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  completeThreadSwitchPerformanceSample({
    fullMessageCount: input.messages.length,
    ...(input.selectionId === undefined ? {} : { selectionId: input.selectionId }),
  })
  scheduleFullThreadHydration({
    fullMessages: input.messages,
    ...(input.selectionId === undefined ? {} : { selectionId: input.selectionId }),
    threadId: input.threadId,
    visibleMessages: hydratedVisibleMessages,
  })
  requestAnimationFrame(focusComposerInput)
}

const beginNewCodexThread = (): void => {
  setActiveCodexThreadId(null)
  threadSwitchLoadingSelectionId = null
  setShellMessages([])
  activeTurnIds.clear()
  setShellFollowUpDrafts([])
  recomputePendingTurnForActiveThread()
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  requestAnimationFrame(focusComposerInput)
}

const loadGymDemoOptimization = () => {
  const state = gymPaneStateFromBridgeProof({
    proof: khalaCodeGymDemoBridgeProof,
    generatedAt: new Date().toISOString(),
    sourceRef: "fixture.khala_code.gym.part2_demo",
  })
  gymPanel?.setState(state)
  showGymProofPane()
  return gymOptimizationRunFromBridgeProof(khalaCodeGymDemoBridgeProof)
}

const fleetPanel =
  fleetPanelEl === null
    ? null
    : mountFleetPanel(fleetPanelEl, {
        delegateRun: request => controls.codexFleetDelegateRun(request),
        fleetRunControl: request => controls.fleetRunControl(request),
        fleetRunList: async request => {
          try {
            const list = await controls.fleetRunList(request)
            clearBootRpcDegradedState("fleetRunList")
            return list
          } catch (error) {
            recordBootRpcDegradedState("fleetRunList", error)
            return degradedFleetRunList()
          }
        },
        fleetRunStart: request => controls.fleetRunStart(request),
        fleetWorkerControl: request => controls.fleetWorkerControl(request),
        // KS-6.2 (#8303): Khala Sync fleet source, default-on; the bun
        // handler answers honestly disabled only for explicit opt-out/setup gaps.
        khalaSyncFleetState: request => controls.khalaSyncFleetState(request),
        khalaSyncFleetMutate: request => controls.khalaSyncFleetMutate(request),
        lifecycleNdjson: fleetLifecycleLines.iterable,
        loadGymDemoProof: () => loadGymDemoOptimization(),
        startDelegationOptimization: async () => loadGymDemoOptimization(),
        fetch: async () => {
          let status: KhalaCodeDesktopFleetStatus
          try {
            status = await controls.codexFleetStatus()
            clearBootRpcDegradedState("codexFleetStatus")
          } catch (error) {
            status = degradedFleetStatus(recordBootRpcDegradedState("codexFleetStatus", error))
          }
          sidebar?.setFleetCounts(projectKhalaCodeSidebarFleetCounts(status))
          return status
        },
        removeAccount: async accountRef => {
          const result = await controls.removeCodexAccount(accountRef)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        setAccountPaused: async request => {
          const result = await controls.setCodexAccountPaused(request)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        consumeResetCredit: async request => {
          const result = await controls.consumeCodexRateLimitResetCredit(request)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        connectAccount: accountRef => controls.connectCodexAccount(accountRef),
        openExternal: url => controls.openExternalUrl(url),
      })

const inboxPanel =
  inboxPanelEl === null
    ? null
    : mountUnifiedInboxPanel(inboxPanelEl, {
        fetch: async (): Promise<import("./inbox.js").UnifiedInboxSource> => ({
          codexHarness: await controls.codexHarnessStatus(),
          ecosystem: await controls.codexEcosystemRead({}),
          fleet: await controls.codexFleetStatus(),
          pylon: await controls.pylonStatus(),
          coding: await controls.codingStatus(),
          tokenAccounting: await controls.tokenAccountingStatus(),
        }),
        onOpenFleet: () => setActiveView("fleet"),
        onOpenSettings: () => setActiveView("settings"),
        onReconnectAccount: accountRef => {
          void controls.connectCodexAccount(accountRef).then(() => {
            void refreshFleetSidebarCounts()
            void inboxPanel?.refresh()
          })
        },
        onResumeRun: async runRef => {
          await controls.fleetRunControl({ runRef, verb: "resume" })
          await refreshFleetSidebarCounts()
          await inboxPanel?.refresh()
        },
      })

const forumPanel =
  forumPanelEl === null
    ? null
    : mountKhalaCodeForumPanel(forumPanelEl, {
        request: async request => {
          const result = await controls.forumRequest(request)
          if (!result.ok) {
            throw new Error(result.error ?? `Forum request failed with ${result.status}`)
          }
          return result.payload
        },
        openExternal: url => controls.openExternalUrl(url),
      })

const editorPanel =
  editorPanelEl === null
    ? null
    : mountKhalaCodeEditorPanel(editorPanelEl, {
        editorDirectoryRead: request => controls.editorDirectoryRead(request),
        editorFileRead: request => controls.editorFileRead(request),
        editorWorkspaceRead: () => controls.editorWorkspaceRead(),
        onComposerContextSelected: addEditorComposerContext,
      })

const projectHomePanel =
  projectHomePanelEl === null
    ? null
    : mountKhalaCodeProjectHomePanel(projectHomePanelEl, {
        listSessions: () => listProjectHomeSessions(),
        onNewSession: () => {
          beginNewCodexThread()
          setActiveView("chat")
        },
        onOpenSession: session => {
          void openProjectHomeSession(session, { focusChat: true })
        },
        onOpenSessionInBackground: session => {
          void openProjectHomeSession(session, { focusChat: false })
        },
      })

const reviewPanel: KhalaCodeReviewPanelHandle | null =
  reviewPanelEl === null
    ? null
    : mountKhalaCodeReviewPanel(reviewPanelEl, {
        onCommentSubmit: detail => {
          void handleDiffReviewSubmitDetail(detail)
        },
        onComposerContextSelected: addReviewComposerContext,
        reviewDiffRead: request => controls.reviewDiffRead(request),
      })

const terminalPanel: KhalaCodeTerminalPanelHandle | null =
  terminalPanelEl === null
    ? null
    : mountKhalaCodeTerminalPanel(terminalPanelEl, {
        clean: async () => {
          const threadId = shellModel().activeCodexThreadId
          if (threadId === null) throw new Error("Open a session before cleaning terminals.")
          const result = await controls.codexBackgroundTerminalsClean({ threadId })
          if (!result.ok) throw new Error(result.error ?? "Terminal clean failed.")
        },
        copy: async text => {
          if (navigator.clipboard === undefined) throw new Error("Clipboard is unavailable.")
          await navigator.clipboard.writeText(text)
        },
        fetch: async () => {
          const threadId = shellModel().activeCodexThreadId
          if (threadId === null) {
            return projectKhalaCodeTerminalWorkbench({ activeThreadId: null })
          }
          const result = await controls.codexBackgroundTerminalsList({ limit: 25, threadId })
          if (!result.ok) throw new Error(result.error ?? "Terminal list failed.")
          return projectKhalaCodeTerminalWorkbench({
            activeThreadId: threadId,
            response: result.response,
          })
        },
        terminate: async processId => {
          const threadId = shellModel().activeCodexThreadId
          if (threadId === null) throw new Error("Open a session before terminating terminals.")
          const result = await controls.codexBackgroundTerminalsTerminate({ processId, threadId })
          if (!result.ok) throw new Error(result.error ?? "Terminal terminate failed.")
        },
      })

const gymPanel =
  gymPanelEl === null ? null : mountGymPane(gymPanelEl, initialGymState)

let claudeSettingsSection: ReturnType<typeof mountClaudeSettingsSection> | null = null
let updaterSection: ReturnType<typeof mountKhalaCodeUpdaterSettingsSection> | null = null
let providerCatalogSection: KhalaCodeProviderCatalogSettingsSectionHandle | null = null
let modelMcpPermissionSection: KhalaCodeModelMcpPermissionSettingsSectionHandle | null = null
let appPreferencesSection: KhalaCodeAppPreferencesSettingsSectionHandle | null = null
let statusUsageSection: KhalaCodeStatusUsageSettingsSectionHandle | null = null
let sessionActionsSection: KhalaCodeSessionActionsSettingsSectionHandle | null = null
let supportSection: KhalaCodeSupportSettingsSectionHandle | null = null
let localServerManagerSection: KhalaCodeLocalServerManagerSettingsSectionHandle | null = null
let plansSection: ReturnType<typeof mountKhalaCodePlansPanel> | null = null
let runEvidenceSection: ReturnType<typeof mountKhalaCodeRunEvidencePanel> | null = null
let commandRegistry: KhalaCodeCommandRegistry | null = null
let commandKeybindingsSection: KhalaCodeCommandKeybindingsSectionHandle | null = null

const settingsPanel =
  settingsPanelEl === null
    ? null
    : mountCodexSettingsPanel(settingsPanelEl, {
        applyModelRolePreset: async request => {
          const result = await controls.codexModelRolePresetApply(request)
          return {
            ok: result.ok,
            ...(result.settings === undefined ? {} : { settings: result.settings }),
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        fetch: () => controls.codexSettingsRead({ includeHiddenModels: true }),
        onRender: () => {
          void claudeSettingsSection?.refresh()
          void providerCatalogSection?.refresh()
          void modelMcpPermissionSection?.refresh()
          appPreferencesSection?.refresh()
          void statusUsageSection?.refresh()
          void sessionActionsSection?.refresh()
          void supportSection?.refresh()
          void localServerManagerSection?.refresh()
          void plansSection?.refresh()
          void runEvidenceSection?.refresh()
          void updaterSection?.refresh()
        },
        renderAdditionalSections: () =>
          [
            ...(commandKeybindingsSection === null ? [] : [commandKeybindingsSection.render()]),
            ...(providerCatalogSection === null ? [] : [providerCatalogSection.render()]),
            ...(modelMcpPermissionSection === null ? [] : [modelMcpPermissionSection.render()]),
            ...(appPreferencesSection === null ? [] : [appPreferencesSection.render()]),
            ...(statusUsageSection === null ? [] : [statusUsageSection.render()]),
            ...(localServerManagerSection === null ? [] : [localServerManagerSection.render()]),
            ...(sessionActionsSection === null ? [] : [sessionActionsSection.render()]),
            ...(supportSection === null ? [] : [supportSection.render()]),
          ],
        fetchModelRoles: () => controls.modelRoleRegistryRead(),
        writeModelRole: request => controls.modelRoleRegistryWrite(request),
        write: async request => {
          const result = await controls.codexConfigValueWrite(request)
          return {
            ok: result.ok,
            ...(result.settings === undefined ? {} : { settings: result.settings }),
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
      })
providerCatalogSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeProviderCatalogSettingsSection({
      fetch: () => controls.codexSettingsRead({ includeHiddenModels: true }),
      writeModelProvider: async providerId => {
        const result = await controls.codexConfigValueWrite({
          keyPath: "model_provider",
          value: providerId,
        })
        return {
          ok: result.ok,
          ...(result.settings === undefined ? {} : { settings: result.settings }),
          ...(result.error === undefined ? {} : { error: result.error }),
        }
      },
    })
modelMcpPermissionSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeModelMcpPermissionSettingsSection({
      fetchEcosystem: () => controls.codexEcosystemRead({}),
      fetchSettings: () => controls.codexSettingsRead({ includeHiddenModels: true }),
      writePermissionProfile: async profileId => {
        const result = await controls.codexConfigValueWrite({
          keyPath: "default_permissions",
          value: profileId,
        })
        return {
          ok: result.ok,
          ...(result.settings === undefined ? {} : { settings: result.settings }),
          ...(result.error === undefined ? {} : { error: result.error }),
        }
      },
    })
appPreferencesSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeAppPreferencesSettingsSection({
      apply: preferences => applyKhalaCodeAppPreferences(document.documentElement, preferences),
      read: () => readKhalaCodeAppPreferences(localStorage),
      reset: () => resetKhalaCodeAppPreferences(localStorage),
      write: preferences => writeKhalaCodeAppPreferences(localStorage, preferences),
    })
statusUsageSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeStatusUsageSettingsSection({
      fetch: async () => {
        const statuses = await Promise.allSettled([
          controls.codexHarnessStatus(),
          controls.codingStatus(),
          controls.pylonStatus(),
          controls.tokenAccountingStatus(),
        ])
        const runtimeStatuses = statuses
          .filter((result): result is PromiseFulfilledResult<Awaited<ReturnType<typeof controls.codexHarnessStatus>>> =>
            result.status === "fulfilled"
          )
          .map(result => result.value)
        const errors = [
          ...statuses
            .filter((result): result is PromiseRejectedResult => result.status === "rejected")
            .map(result => result.reason),
          ...(shellModel().lastTurnFailed
            ? [shellModel().messages.at(-1)?.body ?? "Last turn failed"]
            : []),
        ]
        return projectKhalaCodeStatusUsage({
          bootDegradedStates: shellModel().bootDegradedStates,
          messages: shellModel().messages,
          runtimeStatuses,
          threadTokenSummary: shellModel().threadTokenSummary,
          turnErrors: errors,
        })
      },
    })
sessionActionsSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeSessionActionsSettingsSection({
      fetch: () => projectCurrentSessionActions(),
      runAction: action => runSessionAction(action),
    })
supportSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeSupportSettingsSection({
      copyIssueMetadata: async metadata => {
        if (navigator.clipboard === undefined) throw new Error("Clipboard is unavailable.")
        await navigator.clipboard.writeText(metadata)
      },
      exportDiagnostics: () => exportSupportDiagnostics(),
      fetch: () => projectCurrentSupportEntrypoints(),
      open: (_id, url) => openSupportUrl(url),
    })
const refreshLocalServerManager = async (): Promise<void> => {
  await localServerManagerSection?.refresh()
}
const runLocalServerManagerAction = async (
  action: KhalaCodeLocalServerManagerActionId,
): Promise<void> => {
  if (action === "server.open_manager") {
    setActiveView("settings")
    return
  }
  if (action === "server.refresh") {
    await refreshLocalServerManager()
  }
}
localServerManagerSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeLocalServerManagerSettingsSection({
      fetch: async () => {
        const statuses = await Promise.allSettled([
          controls.pylonStatus(),
          controls.codexHarnessStatus(),
          controls.codingStatus(),
        ])
        const runtimeStatuses = statuses
          .filter((result): result is PromiseFulfilledResult<Awaited<ReturnType<typeof controls.pylonStatus>>> =>
            result.status === "fulfilled"
          )
          .map(result => result.value)
        return projectKhalaCodeLocalServerManager({ runtimeStatuses })
      },
      runAction: runLocalServerManagerAction,
    })
claudeSettingsSection = settingsPanelEl === null
  ? null
  : mountClaudeSettingsSection(settingsPanelEl, {
      fetch: () => controls.claudeSettingsRead(),
    })
plansSection = settingsPanelEl === null
  ? null
  : mountKhalaCodePlansPanel(settingsPanelEl, {
      catalog: () => controls.khalaCodePlanCatalog(),
      openExternal: url => controls.openExternalUrl(url),
      purchase: request => controls.khalaCodePlanPurchase(request),
      status: () => controls.khalaCodePlanStatus(),
      traceCaptureConsentWrite: request => controls.khalaCodeTraceCaptureConsentWrite(request),
      traceCaptureStatus: () => controls.khalaCodeTraceCaptureStatus(),
    })
runEvidenceSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeRunEvidencePanel(settingsPanelEl, {
      report: request => controls.khalaCodeOutsideUserRunReport(request),
    })
updaterSection = settingsPanelEl === null
  ? null
  : mountKhalaCodeUpdaterSettingsSection(settingsPanelEl, {
      check: () => controls.updaterCheck(),
      download: () => controls.updaterDownload(),
      install: () => controls.updaterInstall(),
      openReleaseNotes: url => controls.openExternalUrl(url),
      status: () => controls.updaterStatus(),
    })

const threadListCacheKey = (
  request: {
    readonly limit?: number | undefined
    readonly searchTerm?: string | undefined
  },
): string => JSON.stringify({
  limit: request.limit ?? null,
  searchTerm: request.searchTerm?.trim() ?? "",
})

const clearThreadListCache = (): void => {
  threadListCache.clear()
  threadListRequests.clear()
}

const cachedSessionCatalog = async (
  request: Parameters<DesktopRpcRequests["sessionCatalog"]>[0],
): Promise<Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>> => {
  const key = threadListCacheKey(request ?? {})
  const cached = threadListCache.get(key)
  if (cached !== undefined && performance.now() - cached.cachedAt < THREAD_LIST_CACHE_TTL_MS) {
    return cached.result
  }

  const inFlight = threadListRequests.get(key)
  if (inFlight !== undefined) return inFlight

  const promise = controls.sessionCatalog(request).then(result => {
    threadListCache.set(key, { cachedAt: performance.now(), result })
    return result
  }).finally(() => {
    threadListRequests.delete(key)
  })
  threadListRequests.set(key, promise)
  return promise
}

const chatThreadToSidebarSummary = (
  thread: KhalaCodeDesktopKhalaSyncChatThread,
): KhalaCodeDesktopCodexThreadSummary => {
  const messageLabel = thread.messageCount === 1 ? "1 message" : `${thread.messageCount} messages`
  const createdAt = normalizeThreadTimestampSeconds(thread.createdAt)
  const updatedAt = normalizeThreadTimestampSeconds(thread.updatedAt)
  const recencyAt = normalizeThreadTimestampSeconds(
    thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt,
  )
  return {
    id: thread.threadId,
    sessionId: thread.threadId,
    title: thread.title.trim() || "Untitled chat",
    preview: thread.messageCount === 0 ? "No messages yet" : messageLabel,
    cwd: null,
    projectLabel: "Khala Sync",
    status: thread.status,
    statusLabel: thread.status,
    modelProvider: null,
    source: "khala_sync_chat_thread",
    forkedFromId: null,
    parentThreadId: null,
    createdAt,
    updatedAt,
    recencyAt,
    badges: ["Khala Sync"],
    resumable: true,
    unavailableReason: null,
  }
}

const khalaSyncThreadResult = async (
  threadId: string,
): Promise<Awaited<ReturnType<DesktopRpcRequests["codexThreadResume"]>>> => {
  const result = await controls.khalaSyncChatMessages({
    limit: 500,
    threadId,
  })
  if (!result.ok) {
    throw new Error(result.error ?? "Khala Sync chat messages unavailable")
  }
  return {
    ok: true as const,
    messages: mergeKhalaSyncChatAndRuntimeMessages(result.messages, result.runtimeMessages),
    thread: {
      source: "khala_sync_chat_thread",
      threadId,
    },
    threadId,
  }
}

let projectHomeSessionSelectionCounter = 0

/**
 * Opens a session picked from the Project Home dashboard (#8443). Mirrors
 * the same resume path the Codex thread sidebar uses (`resumeThread` in its
 * mount options below) so a session opened from either surface behaves
 * identically. When `focusChat` is false the session becomes the active
 * thread but the view stays on Project Home -- the dashboard's
 * "Open in Background" action.
 */
const openProjectHomeSession = async (
  session: KhalaCodeDesktopCodexThreadSummary,
  input: Readonly<{ focusChat: boolean }>,
): Promise<void> => {
  if (session.resumable === false) return
  const threadId = session.id
  if (input.focusChat) setActiveView("chat")
  const selectionId = (projectHomeSessionSelectionCounter += 1)
  beginCodexThreadSwitch({ selectionId, source: "sidebar", threadId })
  try {
    const result = khalaSyncChatThreadIds.has(threadId)
      ? await khalaSyncThreadResult(threadId)
      : await controls.codexThreadResume({ sessionId, threadId })
    if (!result.ok) throw new Error("Session could not be resumed.")
    activateCodexThread({ messages: result.messages ?? [], selectionId, threadId: result.threadId ?? threadId })
  } catch {
    if (threadSwitchLoadingSelectionId === selectionId) threadSwitchLoadingSelectionId = null
    shellModel().lastTurnFailed = true
    render()
  }
}

const listProjectHomeSessions = async (): Promise<readonly KhalaCodeDesktopCodexThreadSummary[]> => {
  try {
    const catalog = await cachedSessionCatalog({ limit: 200, scope: "app" })
    clearBootRpcDegradedState("sessionCatalog")
    return catalog.entries.map(sessionCatalogEntryToThreadSummary)
  } catch (error) {
    const degraded = degradedSessionCatalog(recordBootRpcDegradedState("sessionCatalog", error))
    return degraded.entries.map(sessionCatalogEntryToThreadSummary)
  }
}

const currentActiveView = (): string => localStorage.getItem(activeViewStorageKey) ?? "chat"

const projectCurrentSupportEntrypoints = async (): Promise<KhalaCodeSupportProjection> => {
  const [updater, diagnostics] = await Promise.all([
    controls.updaterStatus(),
    controls.diagnosticsSnapshot().catch(() => null),
  ])
  return projectKhalaCodeSupportEntrypoints({
    activeThreadPresent: shellModel().activeCodexThreadId !== null,
    activeView: currentActiveView(),
    diagnostics,
    messageCount: shellModel().messages.length,
    updater,
  })
}

const openSupportUrl = async (url: string): Promise<boolean> => {
  if (!isKhalaCodeSupportUrlAllowed(url)) return false
  return controls.openExternalUrl(url)
}

const exportSupportDiagnostics = async (): Promise<{ readonly ok: boolean; readonly message: string }> => {
  const result = await controls.diagnosticsExport()
  if (!result.ok) return { ok: false, message: result.error }
  return {
    ok: true,
    message: `Diagnostics exported (${result.archiveBytes} bytes).`,
  }
}

const runSupportAction = async (
  action: KhalaCodeSupportEntrypointId | "copy_issue_metadata" | "export_diagnostics",
): Promise<void> => {
  if (action === "export_diagnostics") {
    await exportSupportDiagnostics()
    void supportSection?.refresh()
    return
  }
  const projection = await projectCurrentSupportEntrypoints()
  if (action === "copy_issue_metadata") {
    if (navigator.clipboard === undefined) return
    await navigator.clipboard.writeText(projection.issueMetadata)
    return
  }
  const entry = projection.entries.find(entry => entry.id === action)
  if (entry !== undefined) await openSupportUrl(entry.url)
}

const projectCurrentSessionActions = async (): Promise<KhalaCodeSessionActionProjection> =>
  projectKhalaCodeSessionActionIntents({
    activeThreadId: shellModel().activeCodexThreadId,
    closedTabs: readKhalaCodeClosedSessionTabs(localStorage),
    messages: shellModel().messages,
    sessions: await listProjectHomeSessions(),
  })

const openSessionActionTarget = async (
  threadId: string,
  input: Readonly<{ focusChat: boolean }>,
): Promise<void> => {
  const sessions = await listProjectHomeSessions()
  const session = sessions.find(candidate => candidate.id === threadId)
  if (session === undefined) return
  removeKhalaCodeClosedSessionTab(localStorage, threadId)
  await openProjectHomeSession(session, input)
}

const scrollTranscriptMessage = (direction: "next" | "previous"): boolean => {
  const nodes = [...messageList.querySelectorAll<HTMLElement>("[data-message-id]")]
  if (nodes.length < 2) return false
  const listRect = messageList.getBoundingClientRect()
  const currentIndex = nodes.findIndex(node => node.getBoundingClientRect().top >= listRect.top + 8)
  const fallbackIndex = direction === "next" ? 0 : nodes.length - 1
  const baseIndex = currentIndex < 0 ? fallbackIndex : currentIndex
  const targetIndex = Math.max(0, Math.min(
    nodes.length - 1,
    baseIndex + (direction === "next" ? 1 : -1),
  ))
  nodes[targetIndex]?.scrollIntoView({ behavior: "smooth", block: "start" })
  return true
}

const runSessionAction = async (
  action: KhalaCodeSessionActionKind,
): Promise<{ readonly ok: boolean; readonly message?: string }> => {
  const projection = await projectCurrentSessionActions()
  const intent = khalaCodeSessionActionIntentFor(projection, action)
  if (intent === null) return { ok: false, message: "Unknown session action." }
  if (!intent.enabled) return { ok: false, message: intent.reason }
  const threadId = projection.activeThreadId

  switch (action) {
    case "fork": {
      if (threadId === null) return { ok: false, message: "No active session." }
      clearThreadListCache()
      const result = await controls.codexThreadFork({ sessionId, threadId })
      const newThreadId = result.newThreadId ?? result.threadId
      if (!result.ok) return { ok: false, message: result.error ?? "Session fork failed." }
      await openSessionActionTarget(newThreadId, { focusChat: true })
      void threadSidebar?.refresh()
      return { ok: true, message: "Session forked." }
    }
    case "archive":
      if (threadId === null) return { ok: false, message: "No active session." }
      clearThreadListCache()
      recordKhalaCodeClosedSessionTab(localStorage, threadId)
      {
        const result = await controls.codexThreadArchive({ threadId })
        void threadSidebar?.refresh()
        return result.ok
          ? { ok: true, message: "Session archived." }
          : { ok: false, message: result.error ?? "Session archive failed." }
      }
    case "unarchive":
      if (threadId === null) return { ok: false, message: "No active session." }
      clearThreadListCache()
      {
        const result = await controls.codexThreadUnarchive({ threadId })
        void threadSidebar?.refresh()
        return result.ok
          ? { ok: true, message: "Session unarchived." }
          : { ok: false, message: result.error ?? "Session unarchive failed." }
      }
    case "restore_closed_tab": {
      const targetThreadId = khalaCodeSessionNavigationTarget(projection, "restore_closed_tab")
      if (targetThreadId === null) return { ok: false, message: "No closed sessions recorded." }
      await openSessionActionTarget(targetThreadId, { focusChat: true })
      return { ok: true, message: "Closed session restored." }
    }
    case "previous_session":
    case "next_session": {
      const targetThreadId = khalaCodeSessionNavigationTarget(projection, action)
      if (targetThreadId === null) return { ok: false, message: intent.reason }
      await openSessionActionTarget(targetThreadId, { focusChat: true })
      return { ok: true, message: action === "previous_session" ? "Previous session opened." : "Next session opened." }
    }
    case "previous_message":
      return scrollTranscriptMessage("previous")
        ? { ok: true, message: "Previous message focused." }
        : { ok: false, message: intent.reason }
    case "next_message":
      return scrollTranscriptMessage("next")
        ? { ok: true, message: "Next message focused." }
        : { ok: false, message: intent.reason }
    case "share":
    case "unshare":
      return { ok: false, message: intent.reason }
  }
}

const khalaSyncChatCanDriveSidebar = (
  result: Awaited<ReturnType<DesktopRpcRequests["khalaSyncChatThreads"]>>,
): boolean =>
  result.ok &&
  result.enabled &&
  result.authState === "connected" &&
  result.ownerUserId !== null &&
  result.phase !== "denied"

const khalaSyncThreadCreateRequests = new Set<string>()

const enqueueKhalaSyncChatThreadCreate = (input: {
  readonly threadId: string
  readonly title: string
}): void => {
  if (khalaSyncThreadCreateRequests.has(input.threadId)) return
  khalaSyncThreadCreateRequests.add(input.threadId)
  void controls.khalaSyncChatCreateThread({
    threadId: input.threadId,
    title: input.title.trim() || "New chat",
  }).then(result => {
    if (result.ok) void threadSidebar?.refresh()
  }).catch(() => undefined)
}

const scheduleIdle = (task: () => void): void => {
  if (typeof requestIdleCallback === "function") {
    requestIdleCallback(task, { timeout: 450 })
    return
  }
  window.setTimeout(task, 80)
}

const prefetchRecentThreadMessages = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): void => {
  for (const thread of recentThreadsForHotkeys(threads).slice(0, THREAD_PREFETCH_LIMIT)) {
    if (thread.resumable === false) continue
    if (threadMessageCache.has(thread.id) || threadPrefetches.has(thread.id)) continue
    const prefetch = controls.codexThreadRead({
      includeTurns: true,
      threadId: thread.id,
    }).then(result => {
      cacheThreadMessages(result.threadId, result.messages ?? [])
    }).catch(() => undefined).finally(() => {
      threadPrefetches.delete(thread.id)
    })
    threadPrefetches.set(thread.id, prefetch)
  }
}

const schedulePrefetchRecentThreadMessages = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): void => {
  scheduleIdle(() => prefetchRecentThreadMessages(threads))
}

const threadSidebar =
  threadSidebarEl === null
    ? null
    : mountCodexThreadSidebar(threadSidebarEl, {
        activeThreadId: () => shellModel().activeCodexThreadId,
        archiveThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadArchive({ threadId })
        },
        deleteThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadDelete({ threadId })
        },
        forkThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadFork({ sessionId, threadId })
        },
        isThreadStreaming,
        listThreads: async request => {
          try {
            const chat = await controls.khalaSyncChatThreads({
              limit: 50,
              searchTerm: request.searchTerm,
            })
            if (khalaSyncChatCanDriveSidebar(chat)) {
              khalaSyncChatThreadIds.clear()
              for (const thread of chat.threads) khalaSyncChatThreadIds.add(thread.threadId)
              const threads = chat.threads.map(chatThreadToSidebarSummary)
              return {
                ok: true as const,
                data: chat.threads,
                groups: [
                  {
                    key: "khala-sync-chat",
                    label: "Khala Sync",
                    threadIds: threads.map(thread => thread.id),
                  },
                ],
                threads,
              }
            }
          } catch {
            // Fall back to the local catalog when the sync bridge is absent or
            // rejected before it can report its explicit disabled state.
          }
          khalaSyncChatThreadIds.clear()
          let catalog: Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>
          try {
            catalog = await cachedSessionCatalog({
              limit: 50,
              scope: request.includeHomeSessions ? "all_home" : "app",
              searchTerm: request.searchTerm,
            })
            clearBootRpcDegradedState("sessionCatalog")
          } catch (error) {
            catalog = degradedSessionCatalog(recordBootRpcDegradedState("sessionCatalog", error))
          }
          const threads = catalog.entries.map(sessionCatalogEntryToThreadSummary)
          const result = {
            ok: true as const,
            data: catalog.entries,
            groups: [
              {
                key: "all-harnesses",
                label: "All sessions",
                threadIds: threads.map(thread => thread.id),
              },
            ],
            threads,
          }
          schedulePrefetchRecentThreadMessages(result.threads ?? [])
          return result
        },
        renameThread: async (threadId, name) => {
          clearThreadListCache()
          const synced = await controls.khalaSyncChatRenameThread({
            threadId,
            title: name,
          }).catch(() => null)
          if (synced?.ok === true) return { action: "rename" as const, ok: true, threadId }
          return controls.codexThreadRename({ name, threadId })
        },
        resumeThread: threadId =>
          khalaSyncChatThreadIds.has(threadId)
            ? khalaSyncThreadResult(threadId)
            : controls.codexThreadResume({ sessionId, threadId }),
        sessionId,
        unarchiveThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadUnarchive({ threadId })
        },
        onNewThreadRequested: beginNewCodexThread,
        onThreadSelectionStarted: beginCodexThreadSwitch,
        onThreadSelected: activateCodexThread,
        onThreadSelectionFailed: input => {
          if (threadSwitchLoadingSelectionId !== input.selectionId) return
          threadSwitchLoadingSelectionId = null
          render()
        },
      })

if (threadSidebar !== null) bindRecentThreadHotkeyHints(window, threadSidebar)

const COMMAND_REGISTRY_COMPAT_RECENT_THREAD_HOTKEYS =
  "recent_thread_hotkeys_depend_on_live_thread_order_and_remain_a_command_registry_compatibility_exception"

window.addEventListener("keydown", event => {
  void COMMAND_REGISTRY_COMPAT_RECENT_THREAD_HOTKEYS
  const recentThreadIndex = recentThreadHotkeyIndexForEvent(event)
  const recentThreadCycleDirection = recentThreadCycleDirectionForEvent(event)
  if (recentThreadIndex === null && recentThreadCycleDirection === null) return
  if (threadSidebar === null) return

  event.preventDefault()
  const selection = recentThreadIndex !== null
    ? threadSidebar.selectRecentThread(recentThreadIndex)
    : recentThreadCycleDirection === null
      ? Promise.resolve(false)
      : threadSidebar.selectAdjacentRecentThread(recentThreadCycleDirection)
  void selection.then(selected => {
    if (selected) setActiveView("chat")
  })
})

let sidebar: KhalaCodeSidebarHandle | null = null

const setActiveView = (value: string): void => {
  const panelOpenStartedAt = performance.now()
  const activeValue =
    value === "fleet" ||
    value === "forum" ||
    value === "inbox" ||
    value === "settings" ||
    value === "editor" ||
    value === "home" ||
    value === "review" ||
    value === "terminal"
      ? value
      : "chat"
  localStorage.setItem(activeViewStorageKey, activeValue)
  sidebar?.setSelectedValue(activeValue)
  const showChat = activeValue === "chat"
  const showFleet = activeValue === "fleet"
  const showForum = activeValue === "forum"
  const showInbox = activeValue === "inbox"
  const showSettings = activeValue === "settings"
  const showEditor = activeValue === "editor"
  const showHome = activeValue === "home"
  const showReview = activeValue === "review"
  const showTerminal = activeValue === "terminal"
  if (threadSidebarEl !== null) threadSidebarEl.hidden = !showChat
  if (fleetPanelEl !== null) fleetPanelEl.hidden = !showFleet
  if (forumPanelEl !== null) forumPanelEl.hidden = !showForum
  if (inboxPanelEl !== null) inboxPanelEl.hidden = !showInbox
  if (projectHomePanelEl !== null) projectHomePanelEl.hidden = !showHome
  if (editorPanelEl !== null) editorPanelEl.hidden = !showEditor
  if (reviewPanelEl !== null) reviewPanelEl.hidden = !showReview
  if (settingsPanelEl !== null) settingsPanelEl.hidden = !showSettings
  gymPanel?.setVisible(false)
  const showsFullPanel =
    showFleet || showForum || showInbox || showSettings || showEditor || showHome || showReview || showTerminal
  if (threadShell !== null) threadShell.hidden = showsFullPanel
  if (composerDock !== null) composerDock.hidden = showsFullPanel
  fleetPanel?.setVisible(showFleet)
  forumPanel?.setVisible(showForum)
  inboxPanel?.setVisible(showInbox)
  projectHomePanel?.setVisible(showHome)
  settingsPanel?.setVisible(showSettings)
  editorPanel?.setVisible(showEditor)
  reviewPanel?.setVisible(showReview)
  terminalPanel?.setVisible(showTerminal)
  if (showSettings) {
    void claudeSettingsSection?.refresh()
    void plansSection?.refresh()
  }
  threadSidebar?.setVisible(showChat)
  if (!showChat) markQaTimer("panel.open_ms", panelOpenStartedAt, { panel: activeValue })
  if (showChat) {
    requestAnimationFrame(focusComposerInput)
  }
}

function showGymProofPane(): void {
  if (threadSidebarEl !== null) threadSidebarEl.hidden = true
  if (fleetPanelEl !== null) fleetPanelEl.hidden = true
  if (forumPanelEl !== null) forumPanelEl.hidden = true
  if (inboxPanelEl !== null) inboxPanelEl.hidden = true
  if (projectHomePanelEl !== null) projectHomePanelEl.hidden = true
  if (editorPanelEl !== null) editorPanelEl.hidden = true
  if (reviewPanelEl !== null) reviewPanelEl.hidden = true
  if (terminalPanelEl !== null) terminalPanelEl.hidden = true
  if (settingsPanelEl !== null) settingsPanelEl.hidden = true
  if (threadShell !== null) threadShell.hidden = true
  if (composerDock !== null) composerDock.hidden = true
  gymPanel?.setVisible(true)
  fleetPanel?.setVisible(false)
  forumPanel?.setVisible(false)
  inboxPanel?.setVisible(false)
  projectHomePanel?.setVisible(false)
  settingsPanel?.setVisible(false)
  editorPanel?.setVisible(false)
  reviewPanel?.setVisible(false)
  terminalPanel?.setVisible(false)
  threadSidebar?.setVisible(false)
}

const hotbarCommandIdForValue = (value: KhalaCodeHotbarValue): KhalaCodeCommandId => {
  switch (value) {
    case "chat":
      return "view.chat"
    case "editor":
      return "view.editor"
    case "fleet":
      return "view.fleet"
    case "forum":
      return "view.forum"
    case "home":
      return "view.home"
    case "inbox":
      return "view.inbox"
    case "review":
      return "view.review"
    case "settings":
      return "view.settings"
    case "terminal":
      return "view.terminal"
  }
}

let commandPalette: KhalaCodeCommandPaletteHandle | null = null

const commandSidebarShortcutLabels = (): Partial<Record<KhalaCodeHotbarValue, string>> => {
  if (commandRegistry === null) return {}
  return {
    chat: commandRegistry.keybindingLabel("view.chat"),
    editor: commandRegistry.keybindingLabel("view.editor"),
    fleet: commandRegistry.keybindingLabel("view.fleet"),
    forum: commandRegistry.keybindingLabel("view.forum"),
    home: commandRegistry.keybindingLabel("view.home"),
    inbox: commandRegistry.keybindingLabel("view.inbox"),
    review: commandRegistry.keybindingLabel("view.review"),
    settings: commandRegistry.keybindingLabel("view.settings"),
    terminal: commandRegistry.keybindingLabel("view.terminal"),
  }
}

const syncCommandKeybindingLabels = (): void => {
  sidebar?.setShortcutLabels(commandSidebarShortcutLabels())
}

const commandPaletteRecords = (): readonly KhalaCodeCommandPaletteRecord[] => {
  const records: KhalaCodeCommandPaletteRecord[] = [
    {
      group: "project",
      id: "project:khala-code-desktop",
      kind: "project",
      metadataRef: "khala_code.palette.project.current",
      scoreHints: ["workspace", "openagents", "desktop"],
      subtitle: "Current desktop workspace",
      title: "Khala Code Desktop",
    },
  ]
  const activeThreadId = shellModel().activeCodexThreadId
  if (activeThreadId !== null) {
    records.push({
      group: "session",
      id: `session:${activeThreadId}`,
      kind: "session",
      metadataRef: "khala_code.palette.session.active",
      scoreHints: ["active thread", "current chat"],
      subtitle: activeThreadId,
      title: "Current Chat Session",
    })
  }
  for (const chip of composerContextChips
    .filter(chip => chip.kind === "file" || chip.kind === "selection")
    .slice(-8)
    .reverse()) {
    records.push({
      group: "file",
      id: `file:${chip.id}`,
      kind: "file",
      metadataRef: "khala_code.palette.file.context",
      scoreHints: [chip.displayPath, chip.title],
      subtitle: chip.kind === "selection" ? "Selected composer context" : "Composer file context",
      title: chip.displayPath,
    })
  }
  const settings = composerSettings()
  for (const provider of (settings?.providers.options ?? []).slice(0, 12)) {
    const selected = provider.id === composerSelectedProvider()?.id
    records.push({
      group: "provider",
      id: `provider:${provider.id}`,
      kind: "provider",
      metadataRef: "khala_code.palette.provider.option",
      scoreHints: [provider.id, "model provider"],
      subtitle: selected ? "Selected provider" : "Provider option",
      title: provider.displayName,
    })
  }
  for (const model of composerModelOptions().slice(0, 12)) {
    const selected = model.model === composerModelValue() || model.id === composerModelValue()
    records.push({
      group: "model",
      id: `model:${model.id}`,
      kind: "model",
      metadataRef: "khala_code.palette.model.option",
      scoreHints: [model.model, model.providerDisplayName ?? "", "model"],
      subtitle: selected ? "Selected model" : model.providerDisplayName ?? "Model option",
      title: model.displayName,
    })
  }
  records.push({
    group: "server",
    id: "server:session.refresh",
    kind: "server",
    metadataRef: "khala_code.palette.server.session_refresh",
    scoreHints: ["server", "rpc", "refresh", "sessions"],
    title: "Refresh Session Catalog",
    ...(threadSidebar === null
      ? {
          disabled: true,
          disabledReason: "Thread sidebar is unavailable",
        }
      : {
          subtitle: "Reload sessions from the desktop bridge",
        }),
  })
  records.push(
    {
      group: "server",
      id: "server:local.manager",
      kind: "server",
      metadataRef: "khala_code.palette.server.local_manager",
      scoreHints: ["local server", "pylon", "ai sdk", "runtime"],
      subtitle: "Open the Khala-owned local server manager",
      title: "Local Server Runtime",
    },
    {
      group: "server",
      id: "server:local.refresh",
      kind: "server",
      metadataRef: "khala_code.palette.server.local_refresh",
      scoreHints: ["health", "runtime", "refresh"],
      subtitle: "Reload Pylon, Codex bridge, and coding runtime status",
      title: "Refresh Local Server Health",
    },
  )
  return records
}

commandRegistry = createKhalaCodeCommandRegistry([
  {
    analyticsRef: "khala_code.command.server.open_manager",
    category: "settings",
    execute: () => {
      setActiveView("settings")
    },
    id: "server.open_manager",
    keywords: ["server", "runtime", "pylon", "ai sdk"],
    title: "Open Local Server Manager",
  },
  {
    analyticsRef: "khala_code.command.server.refresh",
    available: () => localServerManagerSection !== null,
    category: "settings",
    disabledReason: () => "Local server manager is unavailable",
    execute: () => {
      void refreshLocalServerManager()
    },
    id: "server.refresh",
    keywords: ["server", "health", "runtime"],
    title: "Refresh Local Server Health",
  },
  {
    analyticsRef: "khala_code.command.server.restart_local",
    available: () => false,
    category: "settings",
    disabledReason: () => "Khala-owned local server lifecycle controller is not wired in this build.",
    execute: () => {},
    id: "server.restart_local",
    keywords: ["server", "restart", "lifecycle"],
    title: "Restart Local Server",
  },
  {
    analyticsRef: "khala_code.command.palette.open",
    category: "workbench",
    defaultKeybindings: [{ key: "k", meta: true }, { key: "k", ctrl: true }],
    execute: () => commandPalette?.open(),
    id: "palette.open",
    keywords: ["search", "commands", "actions"],
    title: "Open Command Palette",
  },
  {
    analyticsRef: "khala_code.command.help.release_notes",
    category: "help",
    execute: () => {
      void runSupportAction("release_notes")
    },
    id: "help.release_notes",
    keywords: ["updates", "changelog"],
    title: "Open Release Notes",
  },
  {
    analyticsRef: "khala_code.command.help.docs",
    category: "help",
    execute: () => {
      void runSupportAction("docs")
    },
    id: "help.docs",
    keywords: ["documentation", "guide"],
    title: "Open Docs",
  },
  {
    analyticsRef: "khala_code.command.help.support",
    category: "help",
    execute: () => {
      void runSupportAction("support")
    },
    id: "help.support",
    keywords: ["discussion", "help"],
    title: "Open Support",
  },
  {
    analyticsRef: "khala_code.command.help.feedback",
    category: "help",
    execute: () => {
      void runSupportAction("feedback")
    },
    id: "help.feedback",
    keywords: ["request", "suggestion"],
    title: "Send Feedback",
  },
  {
    analyticsRef: "khala_code.command.help.bug_report",
    category: "help",
    execute: () => {
      void runSupportAction("bug_report")
    },
    id: "help.bug_report",
    keywords: ["issue", "error"],
    title: "Report Bug",
  },
  {
    analyticsRef: "khala_code.command.help.export_diagnostics",
    category: "help",
    execute: () => {
      void runSupportAction("export_diagnostics")
    },
    id: "help.export_diagnostics",
    keywords: ["logs", "debug"],
    title: "Export Diagnostics",
  },
  {
    analyticsRef: "khala_code.command.help.copy_issue_metadata",
    category: "help",
    execute: () => {
      void runSupportAction("copy_issue_metadata")
    },
    id: "help.copy_issue_metadata",
    keywords: ["bug report", "metadata"],
    title: "Copy Issue Metadata",
  },
  {
    analyticsRef: "khala_code.command.view.chat",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "1" }],
    execute: () => setActiveView("chat"),
    id: "view.chat",
    keywords: ["thread", "conversation"],
    title: "Open Chat",
  },
  {
    analyticsRef: "khala_code.command.view.fleet",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "2" }],
    execute: () => setActiveView("fleet"),
    id: "view.fleet",
    keywords: ["workers", "capacity"],
    title: "Open Fleet",
  },
  {
    analyticsRef: "khala_code.command.view.forum",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "3" }],
    execute: () => setActiveView("forum"),
    id: "view.forum",
    keywords: ["product promises"],
    title: "Open Forum",
  },
  {
    analyticsRef: "khala_code.command.view.inbox",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "4" }],
    execute: () => setActiveView("inbox"),
    id: "view.inbox",
    keywords: ["notifications", "attention"],
    title: "Open Inbox",
  },
  {
    analyticsRef: "khala_code.command.view.settings",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "5" }],
    execute: () => setActiveView("settings"),
    id: "view.settings",
    keywords: ["preferences", "models"],
    title: "Open Settings",
  },
  {
    analyticsRef: "khala_code.command.view.editor",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "6" }],
    execute: () => setActiveView("editor"),
    id: "view.editor",
    keywords: ["files", "source"],
    title: "Open Editor",
  },
  {
    analyticsRef: "khala_code.command.view.home",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "7" }],
    execute: () => setActiveView("home"),
    id: "view.home",
    keywords: ["projects", "dashboard", "sessions"],
    title: "Open Project Home",
  },
  {
    analyticsRef: "khala_code.command.view.review",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "8" }],
    execute: () => setActiveView("review"),
    id: "view.review",
    keywords: ["diff", "comments", "revert"],
    title: "Open Review",
  },
  {
    analyticsRef: "khala_code.command.view.terminal",
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "9" }],
    execute: () => setActiveView("terminal"),
    id: "view.terminal",
    keywords: ["shell", "pty", "background terminal"],
    title: "Open Terminal",
  },
  {
    analyticsRef: "khala_code.command.terminal.refresh",
    available: () => terminalPanel !== null,
    category: "workbench",
    disabledReason: () => "Terminal panel is unavailable",
    execute: () => {
      void terminalPanel?.refresh()
    },
    id: "terminal.refresh",
    keywords: ["shell", "reload"],
    title: "Refresh Terminals",
  },
  {
    analyticsRef: "khala_code.command.terminal.clean",
    available: () => shellModel().activeCodexThreadId !== null,
    category: "workbench",
    disabledReason: () => "Open a session before cleaning terminals",
    execute: async () => {
      const threadId = shellModel().activeCodexThreadId
      if (threadId === null) return
      await controls.codexBackgroundTerminalsClean({ threadId })
      await terminalPanel?.refresh()
    },
    id: "terminal.clean",
    keywords: ["shell", "clear exited"],
    title: "Clean Exited Terminals",
  },
  {
    analyticsRef: "khala_code.command.terminal.copy",
    available: () => terminalPanelEl?.querySelector(".khala-terminal-output") !== null,
    category: "workbench",
    disabledReason: () => "No active terminal output",
    execute: async () => {
      const output = terminalPanelEl?.querySelector<HTMLElement>(".khala-terminal-output")?.textContent ?? ""
      if (navigator.clipboard !== undefined) await navigator.clipboard.writeText(output)
    },
    id: "terminal.copy",
    keywords: ["shell", "clipboard"],
    title: "Copy Terminal Output",
  },
  {
    analyticsRef: "khala_code.command.terminal.terminate_active",
    available: () => terminalPanelEl?.querySelector(".khala-terminal-body[data-process-id]") !== null,
    category: "workbench",
    disabledReason: () => "No active terminal process",
    execute: async () => {
      const threadId = shellModel().activeCodexThreadId
      const processId = terminalPanelEl?.querySelector<HTMLElement>(".khala-terminal-body")?.dataset.processId
      if (threadId === null || processId === undefined) return
      await controls.codexBackgroundTerminalsTerminate({ processId, threadId })
      await terminalPanel?.refresh()
    },
    id: "terminal.terminate_active",
    keywords: ["shell", "stop"],
    title: "Terminate Active Terminal",
  },
  {
    analyticsRef: "khala_code.command.session.new_chat",
    category: "session",
    defaultKeybindings: [{ key: "n", meta: true }],
    execute: beginNewCodexThread,
    id: "session.new_chat",
    keywords: ["new thread"],
    title: "New Chat",
  },
  {
    analyticsRef: "khala_code.command.session.refresh",
    available: () => threadSidebar !== null,
    category: "session",
    disabledReason: () => "Thread sidebar is unavailable",
    execute: () => {
      void threadSidebar?.refresh()
    },
    id: "session.refresh",
    keywords: ["reload threads"],
    title: "Refresh Sessions",
  },
  {
    analyticsRef: "khala_code.command.session.fork",
    available: () => shellModel().activeCodexThreadId !== null,
    category: "session",
    disabledReason: () => "No active session",
    execute: () => {
      void runSessionAction("fork")
    },
    id: "session.fork",
    keywords: ["duplicate", "branch", "thread"],
    title: "Fork Session",
  },
  {
    analyticsRef: "khala_code.command.session.share",
    available: () => false,
    category: "session",
    disabledReason: () => "Sharing requires an explicit safe backing path; private local transcripts and files stay local.",
    execute: () => {
      void runSessionAction("share")
    },
    id: "session.share",
    keywords: ["publish", "link"],
    title: "Share Session",
  },
  {
    analyticsRef: "khala_code.command.session.unshare",
    available: () => false,
    category: "session",
    disabledReason: () => "No Khala/Pylon share record is attached to this local session.",
    execute: () => {
      void runSessionAction("unshare")
    },
    id: "session.unshare",
    keywords: ["private", "link"],
    title: "Unshare Session",
  },
  {
    analyticsRef: "khala_code.command.session.archive",
    available: () => shellModel().activeCodexThreadId !== null,
    category: "session",
    disabledReason: () => "No active session",
    execute: () => {
      void runSessionAction("archive")
    },
    id: "session.archive",
    keywords: ["close", "hide"],
    title: "Archive Session",
  },
  {
    analyticsRef: "khala_code.command.session.unarchive",
    available: () => shellModel().activeCodexThreadId !== null,
    category: "session",
    disabledReason: () => "No active session",
    execute: () => {
      void runSessionAction("unarchive")
    },
    id: "session.unarchive",
    keywords: ["restore", "archive"],
    title: "Unarchive Session",
  },
  {
    analyticsRef: "khala_code.command.session.restore_closed",
    available: () => readKhalaCodeClosedSessionTabs(localStorage).length > 0,
    category: "session",
    disabledReason: () => "No closed sessions recorded",
    execute: () => {
      void runSessionAction("restore_closed_tab")
    },
    id: "session.restore_closed",
    keywords: ["reopen", "restore tab"],
    title: "Restore Closed Session",
  },
  {
    analyticsRef: "khala_code.command.session.previous",
    category: "session",
    defaultKeybindings: [{ alt: true, key: "arrowleft" }],
    execute: () => {
      void runSessionAction("previous_session")
    },
    id: "session.previous",
    keywords: ["back", "thread"],
    title: "Previous Session",
  },
  {
    analyticsRef: "khala_code.command.session.next",
    category: "session",
    defaultKeybindings: [{ alt: true, key: "arrowright" }],
    execute: () => {
      void runSessionAction("next_session")
    },
    id: "session.next",
    keywords: ["forward", "thread"],
    title: "Next Session",
  },
  {
    analyticsRef: "khala_code.command.message.previous",
    available: () => shellModel().messages.length > 1,
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "arrowup" }],
    disabledReason: () => "Need at least two visible messages",
    execute: () => {
      void runSessionAction("previous_message")
    },
    id: "message.previous",
    keywords: ["transcript", "scroll"],
    title: "Previous Message",
  },
  {
    analyticsRef: "khala_code.command.message.next",
    available: () => shellModel().messages.length > 1,
    category: "navigation",
    defaultKeybindings: [{ alt: true, key: "arrowdown" }],
    disabledReason: () => "Need at least two visible messages",
    execute: () => {
      void runSessionAction("next_message")
    },
    id: "message.next",
    keywords: ["transcript", "scroll"],
    title: "Next Message",
  },
  {
    analyticsRef: "khala_code.command.composer.focus",
    category: "composer",
    execute: focusComposerInput,
    id: "composer.focus",
    keywords: ["input prompt"],
    title: "Focus Composer",
  },
  {
    analyticsRef: "khala_code.command.composer.attach_file",
    category: "composer",
    execute: () => {
      void openComposerAttachmentPicker()
    },
    id: "composer.attach_file",
    keywords: ["attachment", "file picker"],
    title: "Attach File",
  },
  {
    analyticsRef: "khala_code.command.composer.stop_turn",
    available: () => shellModel().pendingTurn,
    category: "composer",
    disabledReason: () => "No active turn is running",
    execute: stopActiveTurn,
    id: "composer.stop_turn",
    keywords: ["interrupt", "cancel"],
    title: "Stop Active Turn",
  },
], {
  getKeybindingOverrides: () => commandKeybindingOverrides,
})

commandKeybindingsSection = createKhalaCodeCommandKeybindingsSection({
  getOverrides: () => commandKeybindingOverrides,
  onChanged: syncCommandKeybindingLabels,
  registry: () => commandRegistry,
  resetAll: () => {
    for (const key of Object.keys(commandKeybindingOverrides)) {
      delete commandKeybindingOverrides[key as KhalaCodeCommandId]
    }
    persistCommandKeybindingOverrides()
  },
  setOverride: (id, value) => {
    if (value === null) delete commandKeybindingOverrides[id]
    else commandKeybindingOverrides[id] = value
    persistCommandKeybindingOverrides()
  },
})

const executeKhalaCodeCommand = async (id: KhalaCodeCommandId): Promise<void> => {
  const registry = commandRegistry
  if (registry === null) return
  await registry.execute(id)
  render()
}

const activeCommandRegistry = commandRegistry
if (activeCommandRegistry === null) {
  throw new Error("Command registry failed to initialize")
}

commandPalette = mountKhalaCodeCommandPalette(commandPaletteRoot, {
  getRecords: commandPaletteRecords,
  onExecute: result => {
    if (result.kind === "command") {
      void executeKhalaCodeCommand(result.id as KhalaCodeCommandId)
      return
    }
    if (result.kind === "file" || result.kind === "project") {
      void executeKhalaCodeCommand("view.editor")
      return
    }
    if (result.kind === "model" || result.kind === "provider") {
      void executeKhalaCodeCommand("view.settings")
      return
    }
    if (result.kind === "server") {
      void executeKhalaCodeCommand(
        result.id === "server:session.refresh"
          ? "session.refresh"
          : result.id === "server:local.refresh"
            ? "server.refresh"
            : "server.open_manager",
      )
      return
    }
    if (result.kind === "session") {
      void executeKhalaCodeCommand("view.chat")
    }
  },
  registry: activeCommandRegistry,
})

window.addEventListener("keydown", event => {
  if (commandPalette?.isOpen()) return
  const registry = commandRegistry
  if (registry === null) return
  const commandId = khalaCodeCommandForKeyboardEvent(registry, event)
  if (commandId === null) return
  event.preventDefault()
  void executeKhalaCodeCommand(commandId)
})

sidebar = sidebarNavRoot === null
  ? null
  : mountKhalaCodeSidebar(sidebarNavRoot, {
      enableKeyboardShortcuts: false,
      selectedValue: initialView,
      shortcutLabels: commandSidebarShortcutLabels(),
      onActivate: value => {
        void executeKhalaCodeCommand(hotbarCommandIdForValue(value))
      },
    })

const refreshFleetSidebarCounts = async (): Promise<void> => {
  if (sidebar === null) return
  try {
    sidebar.setFleetCounts(projectKhalaCodeSidebarFleetCounts(await controls.codexFleetStatus()))
    clearBootRpcDegradedState("codexFleetStatus")
  } catch {
    recordBootRpcDegradedState("codexFleetStatus", "fleet status unavailable")
    sidebar.setFleetCounts(null)
  }
}

if (sidebarNavRoot !== null) {
  void refreshFleetSidebarCounts()
}
setActiveView(initialView)
renderThreadTokenCounter()
void refreshThreadTokenSummary()
const firstRenderStartedAt = performance.now()
render()
markQaTimer("startup.interactive_ms", 0, { view: initialView })
markQaTimer("first_render.ms", firstRenderStartedAt, { view: initialView })
requestAnimationFrame(focusComposerInput)

/**
 * Restores the thread the user was viewing when the app last quit
 * (khala_code.app.resumes_after_restart.v1). Runs after the normal blank
 * boot render so startup stays fast; a genuinely in-flight turn cannot
 * survive process death (the Codex app-server subprocess quits with the
 * app), but the thread and its message history are not lost. Fails soft:
 * a missing/corrupt thread clears the stored id instead of retrying on
 * every future launch.
 */
const restoreActiveThreadAfterRestart = async (): Promise<void> => {
  if (bootRestoreThreadId === null || bootRestoreThreadId === shellModel().activeCodexThreadId) return
  try {
    const result = await controls.codexThreadResume({ sessionId, threadId: bootRestoreThreadId })
    activateCodexThread({ messages: result.messages ?? [], threadId: result.threadId })
  } catch {
    localStorage.removeItem(activeThreadIdStorageKey)
  }
}
void restoreActiveThreadAfterRestart()
