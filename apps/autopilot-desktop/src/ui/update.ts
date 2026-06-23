// CL-53: the Foldkit update function — the single pure reducer over the Model.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/update.ts): a
// Match.tags-style exhaustive switch returning [Model, Command[]]. RPC effects are
// expressed as Commands (commands.ts); validation (validateIntentDraft /
// validateSpawnRequest) runs here before a command is dispatched, so the form
// state and feedback stay pure functions of the Model.

import { validateIntentDraft, validateSpawnRequest } from "@openagentsinc/autopilot-control-protocol"
import { Command } from "foldkit"

import {
  ActivateTrainingWindow,
  AddManagedAccount,
  AdmitTrainingRealGradientEvidence,
  BuildTrainingEvidencePacket,
  CancelSession,
  ClaimTrainingWindowLease,
  ChooseIdentity,
  DeployCloud,
  LoadAppleFmReadiness,
  LoadBuiltInAgentReadiness,
  LoadIdentityChoiceState,
  LoadInferenceGatewayReadiness,
  LoadInstallReadiness,
  LoadManagedAccounts,
  LoadOnboardingStatus,
  LoadPromiseSurfacingReadiness,
  LoadPublicActivityTimeline,
  LoadProofReplayBundle,
  LoadTrainingDashboard,
  LoadTrainingEvidencePacketSummary,
  LoadTrainingOperatorReadiness,
  LoadTrainingPromiseGates,
  LoadTrainingRuns,
  PlanTrainingRunWindow,
  PublishVerseLocalPose,
  QueueTrainingCloseout,
  QueueTrainingLaunch,
  PersistInputProfile,
  PersistPreferences,
  ReconcileTrainingWindow,
  RemoveManagedAccount,
  RequestTrainingBootstrapGrant,
  RespondToVerseInput,
  RespondToShellInput,
  RunVerseKhalaTurn,
  ResolveApproval,
  ResolveManagedWorktree,
  SetCoordinatorPaused,
  SetManagedAccountPriority,
  SpawnAppleFmComposerTurn,
  SpawnBatchSession,
  SpawnChatTurn,
  SpawnComposerTurn,
  SpawnShellCodingTurn,
  StartAppleFmSession,
  StartBuiltInAgent,
  SurfacePromiseGap,
  SpawnSession,
  SubmitIntent,
} from "./commands.js"
import {
  buildComposerContinuationObjective,
  parseVerifyLines,
} from "./helpers.js"
import {
  advanceSwarmBatch,
  clampSwarmBatchConcurrency,
  parseSwarmBatchObjectives,
  startSwarmBatch,
  type SwarmBatchState,
} from "./swarm-batch.js"
import {
  managedWorktreeLabel,
  parseManagedWorktreeRequest,
  type ManagedWorktreeRequest,
} from "./composer-workspace.js"
import {
  ClickedBlueprintChatSubmit,
  ClickedChatSubmit,
  ClickedComposerReply,
  ClickedComposerSpawn,
  ClickedCoordinatorToggle,
  ClickedSubmitIntent,
  ChangedVerseMode,
  ClosedAllManagedPanes,
  ClosedCommandPalette,
  MovedCommandPaletteSelection,
  NavigatedTo,
  OpenedCommandPalette,
  OpenedManagedPane,
  RanPaletteCommand,
  type Message,
} from "./message.js"
import { interpretKey } from "./keyboard.js"
import {
  codeModePaletteCommands,
  filterPaletteCommands,
  groupById,
  paletteCommands,
  type PaletteCommand,
} from "./nav.js"
import { chatWorldBuildFlags, chatWorldHudFlag } from "../shared/chat-world-flags.js"
import {
  latestVerseLocalPose,
  recordLatestVerseLocalPose,
} from "./verse-local-pose.js"
import { recordVerseSceneDiagnostic } from "./verse-scene-diagnostics.js"
import {
  decodeInputBindingOrNull,
  inputProfileWithBinding,
  inputProfileWithResetAction,
  inputProfileWithResetAll,
  inputProfileWithResetCategory,
} from "./input-profile-preferences.js"
import { safeInputProfileValue } from "./verse-input-bindings.js"
// HUD H3 (#5501): the pure PaneManager reducer + the layer accessor. update.ts
// maps each managed-pane Message to one `PaneLayerAction` and stores the result
// back on the Model. The viewport is read here (real window when present, a fixed
// fallback under test) so cascade/clamp use the live size.
import { reducePaneLayer, type PaneLayerAction, type Viewport } from "./pane-manager.js"
import {
  BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF,
  BLUEPRINT_CHAT_REPLAY_TOOL_REF,
  Model,
  modelAppleFmReadiness,
  modelBuiltInAgentReadiness,
  modelInferenceGatewayReadiness,
  modelManagedAccounts,
  modelNode,
  modelCodeModeSync,
  modelPaneLayer,
  type ChatMessage,
  type PaneId,
  type ProofReplayCommandRequest,
  type ShellCodingTarget,
  type ShellTarget,
  type VerseKhalaReceipt,
} from "./model.js"
// #5466 (EPIC #5461): live Blueprint chat — SEMANTIC signature routing + runtime
// step derivation from real session events (replaces the seeded path).
import { selectSignatureForMessage } from "./blueprint-chat-routing.js"
import { liveChatScopedSteps } from "./blueprint-chat-runtime.js"
import {
  DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
  type DesktopProofReplayProjection,
} from "../shared/proof-replays.js"
import {
  projectCodeModeSyncSnapshot,
  type CodeModeSyncAccountRow,
  type CodeModeSyncSource,
} from "./code-mode-sync.js"
import {
  nextCodeModeAccountOverride,
  projectCodeModeAccountRoute,
  type CodeModeAccountRoute,
  type CodeModeSpawnAdapter,
} from "./code-mode-account-routing.js"
import { validatePromiseSurfacingInput } from "../shared/promise-surfacing.js"
import {
  paymentParticleTsMs,
  prunePaymentParticlesByRecency,
  type ChatWorldPylonScene,
  type PaymentParticle,
} from "../shared/chat-world-scene.js"
import {
  CHAT_WORLD_GATEWAY_NODE_PREFIX,
  CHAT_WORLD_INFERENCE_NODE_PREFIX,
} from "../shared/chat-world-visualization.js"
import { VERSE_TRAINING_NODE_PREFIX } from "../shared/verse-training-visualization.js"
import { verseSpawnableSceneById } from "../shared/verse-spawned-scene.js"
import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
  ChooseIdentityResponse,
  IdentityChoiceStateResponse,
  InstallReadinessResponse,
  OnboardingStatusResponse,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
  PublicActivityTimelineResponse,
  TrainingBootstrapGrantResponse,
  TrainingDashboardSummaryResponse,
  TrainingEvidenceAdmissionResponse,
  TrainingEvidencePacketBuildResponse,
  TrainingEvidencePacketSummaryResponse,
  TrainingOperatorReadinessResponse,
  TrainingPlanResponse,
  TrainingPromiseGatesResponse,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowLeaseResponse,
} from "../shared/rpc.js"

type Result = readonly [Model, ReadonlyArray<Command.Command<Message>>]

const noCommands: ReadonlyArray<Command.Command<Message>> = []

const withCodeModeSync = (
  model: Model,
  source: CodeModeSyncSource,
): Model =>
  Model.make({
    ...model,
    codeModeSync: projectCodeModeSyncSnapshot({
      source,
      node: modelNode(model),
      managedAccounts: modelManagedAccounts(model),
      inferenceGatewayReadiness: modelInferenceGatewayReadiness(model),
      builtInAgentReadiness: modelBuiltInAgentReadiness(model),
      appleFmReadiness: modelAppleFmReadiness(model),
      selectedSessionRef: model.selectedSessionRef,
      composerAdapter: model.spawnAdapter,
      composerAccountRef: model.composerAccountRef,
    }),
  })

const verseSceneActive = (model: Model): boolean => {
  const flags = chatWorldBuildFlags()
  return flags.CHAT_WORLD_SCENE && model.pane === "chat" && model.verseEnabled
}

const verseCodeControlsEnabled = (model: Model): boolean =>
  model.verseMode === "code" || chatWorldHudFlag()

const verseControlsDisabled = (model: Model): boolean =>
  verseSceneActive(model) && !verseCodeControlsEnabled(model)

// Dev affordance (#6033): toggle an isolated scene station in/out of the live
// Verse spawn list. An unknown scene id is a no-op (never fabricate a scene). A
// freshly spawned scene starts with its portal off. Pure model state.
const toggleVerseSpawnedScene = (model: Model, sceneId: string): Model => {
  if (verseSpawnableSceneById(sceneId) === null) return model
  const present = model.verseSpawnedScenes.some((s) => s.sceneId === sceneId)
  // Freeze the avatar's pose AT SPAWN so the station is world-anchored: it stays
  // where it was dropped while the avatar walks around it (not chasing the live
  // pose every frame). Read the LIVE pose cache (`latestVerseLocalPose`), which
  // tracks the avatar every frame, rather than `model.verseSceneRestorePose`
  // (only refreshed on a chat-world-scene snapshot, so it can be stale by the time
  // the user has walked somewhere and presses 2). Null before any pose landed.
  const live = latestVerseLocalPose() ?? model.verseSceneRestorePose
  const anchor =
    live === null
      ? null
      : { x: live.x, y: live.y, z: live.z, yaw: live.yaw }
  const verseSpawnedScenes = present
    ? model.verseSpawnedScenes.filter((s) => s.sceneId !== sceneId)
    : [...model.verseSpawnedScenes, { sceneId, showPortal: false, anchor }]
  return Model.make({ ...model, verseSpawnedScenes })
}

// Flip the optional gateway-portal variant for an already-spawned scene. A scene
// that is not currently spawned (or an unknown id) is a no-op.
const toggleVerseSpawnedScenePortal = (model: Model, sceneId: string): Model => {
  if (verseSpawnableSceneById(sceneId) === null) return model
  if (!model.verseSpawnedScenes.some((s) => s.sceneId === sceneId)) return model
  const verseSpawnedScenes = model.verseSpawnedScenes.map((s) =>
    s.sceneId === sceneId ? { ...s, showPortal: !s.showPortal } : s,
  )
  return Model.make({ ...model, verseSpawnedScenes })
}

const openNewCoderSession = (model: Model): Result => {
  const codeModeAdapter =
    model.spawnAdapter === "apple_fm" ? "claude_agent" : model.spawnAdapter
  return [
    withCodeModeSync(
      Model.make({
        ...model,
        pane: "chat",
        verseEnabled: true,
        verseMode: "code",
        spawnAdapter: codeModeAdapter,
        composerAccountRef: null,
        composerSessionRef: null,
        composerReply: "",
        composerTurns: [],
        composerStatus: { text: "", tone: "idle" },
        composerPending: false,
        composerPendingObjective: null,
        spawnObjective: "",
        managedAccountsPending: true,
        managedAccountsStatus: {
          text:
            codeModeAdapter === "claude_agent"
              ? "loading Claude Agent accounts..."
              : "loading Codex accounts...",
          tone: "info" as const,
        },
      }),
      "model_tick",
    ),
    [LoadManagedAccounts()],
  ]
}

// #5730: how many active payment particles the chat-world scene keeps at once.
// Bounds the live beam/burst count behind chat and the scene-options signature
// the renderer diffs on, so a busy stream cannot grow the set without limit.
const MAX_CHAT_WORLD_PARTICLES = 24

// Append a new payment particle to the bounded active set, de-duped by id, then
// recency-pruned (P2.5 #5730) so stale beams expire instead of flying forever on
// a quiet network, then capped at MAX_CHAT_WORLD_PARTICLES. Opaque in/out (the
// runtime stores PaymentParticle as S.Unknown); we read `id` for dedupe and `ts`
// (via paymentParticleTsMs) for the pure recency window, casting only at this
// boundary the way modelChatWorldParticles already does.
const appendChatWorldParticle = (
  current: ReadonlyArray<unknown>,
  particle: unknown,
): ReadonlyArray<unknown> => {
  const id = (particle as { id?: unknown } | null)?.id
  const withoutDup =
    typeof id === "string"
      ? current.filter((p) => (p as { id?: unknown } | null)?.id !== id)
      : current
  const appended = [...withoutDup, particle]
  // Expire beams older than the recency window, using the NEW particle's own ts
  // as the reference clock (pure, deterministic). A null/unknown ts skips the
  // recency prune — the count cap below still bounds the set.
  const referenceTsMs = paymentParticleTsMs(particle as PaymentParticle)
  const pruned =
    referenceTsMs === null
      ? appended
      : prunePaymentParticlesByRecency(
          appended as ReadonlyArray<PaymentParticle>,
          referenceTsMs,
        )
  return pruned.length > MAX_CHAT_WORLD_PARTICLES
    ? pruned.slice(pruned.length - MAX_CHAT_WORLD_PARTICLES)
    : pruned
}

// #5730: recover the receipt sourceRef from a payment-endpoint detail string.
// Details are `<ref> · <sats> sats` (target) or `<ref> · from` (from), so the
// ref is everything before the first " · " separator, trimmed. The visible scene
// label stays short and human-readable.
const chatWorldRefFromLabel = (label: string): string => {
  const sep = label.indexOf(" · ")
  return (sep >= 0 ? label.slice(0, sep) : label).trim()
}

const chatWorldSceneMaterialKey = (
  scene: ChatWorldPylonScene | null,
): string => {
  if (scene === null) return "null"
  return JSON.stringify({
    empty: scene.empty,
    onlineNow: scene.onlineNow,
    growth: {
      tier: scene.growth.tier,
      scale: scene.growth.scale,
      settledSats: scene.growth.settledSats,
    },
    nodes: scene.nodes.map(node => ({
      id: node.id,
      label: node.label,
      state: node.state,
      color: node.color,
      online: node.online,
      products: [...node.products].sort(),
    })),
  })
}

let lastLocalPoseDiagnosticAtMs = 0

const SHELL_TARGET_ORDER: ReadonlyArray<ShellTarget> = [
  "current",
  "claude_code",
  "codex",
]

const nextShellTarget = (target: ShellTarget): ShellTarget => {
  const index = SHELL_TARGET_ORDER.indexOf(target)
  return SHELL_TARGET_ORDER[(index + 1) % SHELL_TARGET_ORDER.length] ?? "current"
}

const shellTargetLabel = (target: ShellTarget): string =>
  target === "claude_code"
    ? "Claude Code"
    : target === "codex"
      ? "Codex"
      : "Current"

const shellCodingAdapter = (
  target: ShellCodingTarget,
): "codex" | "claude_agent" =>
  target === "claude_code" ? "claude_agent" : "codex"

const shellCodingState = (
  model: Model,
  target: ShellCodingTarget,
): { readonly sessionRef: string | null; readonly turns: ReadonlyArray<string> } =>
  target === "claude_code"
    ? { sessionRef: model.shellClaudeSessionRef, turns: model.shellClaudeTurns }
    : { sessionRef: model.shellCodexSessionRef, turns: model.shellCodexTurns }

const writeShellCodingSuccess = (
  model: Model,
  target: ShellCodingTarget,
  prompt: string,
  sessionRef: string,
): Model =>
  target === "claude_code"
    ? Model.make({
        ...model,
        shellPending: false,
        shellClaudeSessionRef: sessionRef,
        shellClaudeTurns: [...model.shellClaudeTurns, prompt],
      })
    : Model.make({
        ...model,
        shellPending: false,
        shellCodexSessionRef: sessionRef,
        shellCodexTurns: [...model.shellCodexTurns, prompt],
      })

const prefixedShellEventBlock = (prefix: string, text: string): string =>
  `${prefix}: ${text.replace(/\n/g, "\n  ")}`

const shellEventPrefixPattern = (prefix: string): RegExp =>
  new RegExp(
    `^${prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}:`,
    "i",
  )

const shellEventText = (event: {
  readonly detail: string
  readonly full?: string
  readonly phase?: string
}): string => {
  const detail = event.detail.trim()
  const text =
    typeof event.full === "string" && event.full.trim() !== ""
      ? event.full
      : detail
  const trimmed = text.trim()
  const detailPrefix = /^([^:\n]{1,72}):\s*/.exec(detail)?.[1]?.trim() ?? ""
  if (
    trimmed !== "" &&
    (/^thinking[:…]/i.test(detail) || event.phase === "reasoning") &&
    !/^thinking[:…]/i.test(trimmed) &&
    !/^thinking tokens:/i.test(trimmed)
  ) {
    return prefixedShellEventBlock("thinking", trimmed)
  }
  if (
    trimmed !== "" &&
    (/^result:/i.test(detail) || event.phase === "tool_result") &&
    !/^result:/i.test(trimmed)
  ) {
    return prefixedShellEventBlock("result", trimmed)
  }
  if (
    trimmed !== "" &&
    event.phase === "tool_use" &&
    detailPrefix !== "" &&
    !shellEventPrefixPattern(detailPrefix).test(trimmed)
  ) {
    return prefixedShellEventBlock(detailPrefix, trimmed)
  }
  return trimmed
}

const latestShellAgentText = (
  events: ReadonlyArray<{
    readonly detail: string
    readonly full?: string
    readonly phase?: string
  }>,
): string | null => {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const text = shellEventText(events[i] ?? { detail: "" })
    const match = /^agent:\s*(.+)$/is.exec(text)
    if (match?.[1] && match[1].trim() !== "") return match[1].trim()
  }
  return null
}

const shellExternalSessionRefFromEvents = (
  events: ReadonlyArray<{
    readonly detail: string
    readonly full?: string
    readonly phase?: string
  }>,
): string | null => {
  for (const event of events) {
    const text = shellEventText(event)
    const match = /\bexternal session:\s*(session\.pylon\.[A-Za-z0-9._-]+)/.exec(text)
    if (match?.[1]) return match[1]
  }
  return null
}

const shellEventDisplayLine = (event: {
  readonly detail: string
  readonly full?: string
  readonly phase?: string
}): string | null => {
  const text = shellEventText(event)
  if (text === "") return null
  if (/^you:\s*/i.test(text)) return null
  if (/^external session:\s*session\.pylon\./i.test(text)) return null
  if (
    text === "thread started" ||
    text === "turn started" ||
    text === "turn completed" ||
    text === "task started" ||
    text === "task complete" ||
    /^control session mode:/i.test(text)
  ) {
    return null
  }
  const agent = /^agent:\s*(.+)$/is.exec(text)
  return agent?.[1]?.trim() ? agent[1].trim() : text
}

const shellStreamText = (
  events: ReadonlyArray<{
    readonly detail: string
    readonly full?: string
    readonly phase?: string
  }>,
): string | null => {
  let tokenLine: string | null = null
  const reasoningLines: string[] = []
  const bodyLines: string[] = []
  const seen = new Set<string>()
  for (const event of events) {
    const line = shellEventDisplayLine(event)
    if (line === null) continue
    if (line.startsWith("thinking tokens:")) {
      tokenLine = line
      continue
    }
    if (line.startsWith("thinking:")) {
      if (seen.has(line)) continue
      seen.add(line)
      reasoningLines.push(line)
      continue
    }
    if (seen.has(line)) continue
    seen.add(line)
    bodyLines.push(line)
  }
  const lines = [
    ...(tokenLine === null ? [] : [tokenLine]),
    ...reasoningLines,
    ...bodyLines,
  ]
  return lines.length > 0 ? lines.join("\n") : null
}

const shellTerminalFailureText = (
  model: Model,
  target: ShellCodingTarget,
  sessionRef: string,
): string | null => {
  const node = modelNode(model)
  const session = node?.sessions.find((row) => row.sessionRef === sessionRef) as
    | { state?: string; errorClass?: string | null }
    | undefined
  if (session?.state !== "failed") return null
  const suffix =
    typeof session.errorClass === "string" && session.errorClass.trim() !== ""
      ? ` (${session.errorClass})`
      : ""
  return `${shellTargetLabel(target)} failed${suffix}`
}

const shellExternalSessionRef = (model: Model, sessionRef: string): string | null => {
  const node = modelNode(model)
  const artifact = node?.artifacts?.[sessionRef] as
    | { detail?: { externalSessionRef?: unknown } }
    | undefined
  const external = artifact?.detail?.externalSessionRef
  if (typeof external === "string" && external.trim() !== "") return external
  return shellExternalSessionRefFromEvents(node?.events?.[sessionRef] ?? [])
}

const reconcileShellCodingTarget = (
  model: Model,
  target: ShellCodingTarget,
): Model => {
  const sessionRef =
    target === "claude_code" ? model.shellClaudeSessionRef : model.shellCodexSessionRef
  if (sessionRef === null) return model
  const node = modelNode(model)
  const events = node?.events?.[sessionRef] ?? []
  const externalSessionRef = shellExternalSessionRef(model, sessionRef)
  const externalEvents =
    externalSessionRef === null ? [] : (node?.events?.[externalSessionRef] ?? [])
  const streamEvents = [...events, ...externalEvents]
  const text =
    shellStreamText(streamEvents) ??
    latestShellAgentText(events) ??
    latestShellAgentText(externalEvents) ??
    shellTerminalFailureText(model, target, sessionRef)
  if (text === null) return model
  let changed = false
  const turns = model.shellTurns.map((turn) => {
    if (
      turn.role === "autopilot" &&
      turn.target === target &&
      turn.sessionRef === sessionRef &&
      turn.text !== text
    ) {
      changed = true
      return { ...turn, text }
    }
    return turn
  })
  return changed ? Model.make({ ...model, shellTurns: turns }) : model
}

const reconcileShellCodingTurns = (model: Model): Model =>
  reconcileShellCodingTarget(
    reconcileShellCodingTarget(model, "claude_code"),
    "codex",
  )

// HUD H3 (#5501): the live viewport for managed-pane cascade/clamp. Reads the
// real webview window when present; falls back to a fixed desktop size under
// test / SSR so the pure reducer always has finite bounds. Defensive about a
// zero/NaN size (a not-yet-laid-out window) by flooring to the fallback.
const PANE_FALLBACK_VIEWPORT: Viewport = { width: 1440, height: 900 }
const currentViewport = (): Viewport => {
  const g = globalThis as unknown as { innerWidth?: number; innerHeight?: number }
  const width = typeof g.innerWidth === "number" && g.innerWidth > 0
    ? g.innerWidth
    : PANE_FALLBACK_VIEWPORT.width
  const height = typeof g.innerHeight === "number" && g.innerHeight > 0
    ? g.innerHeight
    : PANE_FALLBACK_VIEWPORT.height
  return { width, height }
}

// Run one PaneLayerAction through the pure PaneManager reducer and store the new
// layer back on the opaque `paneLayer` field. The managed-pane verbs are pure
// state changes (no RPC), so they never emit a command.
const applyPaneLayerAction = (model: Model, action: PaneLayerAction): Result => {
  const next = reducePaneLayer(modelPaneLayer(model), action, currentViewport())
  return [Model.make({ ...model, paneLayer: next }), noCommands]
}

// #5469: bridge the flat swarm-batch Model fields ↔ the pure SwarmBatchState the
// swarm-batch.ts queue logic operates on, so the reducer stays a thin mapping
// over the (unit-tested) pure functions.
const readSwarmBatchState = (model: Model): SwarmBatchState => ({
  queue: model.swarmBatchQueue,
  active: model.swarmBatchActive,
  concurrency: clampSwarmBatchConcurrency(Number(model.swarmBatchConcurrency)),
  launched: model.swarmBatchLaunched,
  failed: model.swarmBatchFailed,
  total: model.swarmBatchTotal,
})

const writeSwarmBatchState = (model: Model, state: SwarmBatchState): Model =>
  Model.make({
    ...model,
    swarmBatchQueue: state.queue,
    swarmBatchActive: state.active,
    swarmBatchConcurrency: String(state.concurrency),
    swarmBatchLaunched: state.launched,
    swarmBatchFailed: state.failed,
    swarmBatchTotal: state.total,
  })

// #5472/#5485: build the PersistPreferences command from the (already-updated)
// model, so the preference handlers can't drift on which fields are saved.
const persistPreferencesFor = (model: Model): Command.Command<Message> =>
  PersistPreferences({
    theme: model.themePreference,
    defaultAdapter: model.defaultAdapter,
    defaultLane: model.defaultLane,
    showNotificationPanel: model.showNotificationPanel,
    gatewayInferenceFallback: model.gatewayInferenceFallback,
  })

const persistInputProfileFor = (model: Model): Command.Command<Message> =>
  PersistInputProfile({ profile: safeInputProfileValue(model.inputProfile) })

const proofReplayCommandRequestForModel = (
  model: Model,
): ProofReplayCommandRequest =>
  model.selectedProofReplayMode === "generated"
    ? {
        actorRef: model.generatedProofReplayActorRef,
        from: model.generatedProofReplayFrom,
        kind: model.generatedProofReplayKind,
        limit: model.generatedProofReplayLimit,
        mode: "generated",
        pairRef: model.generatedProofReplayPairRef,
        runRef: model.generatedProofReplayRunRef,
        since: model.generatedProofReplaySince,
        source: model.generatedProofReplaySource,
        to: model.generatedProofReplayTo,
        windowRef: model.generatedProofReplayWindowRef,
      }
    : { mode: "catalog", slug: model.selectedProofReplaySlug }

const loadTrainingProjectionCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> => [
  LoadTrainingRuns(),
  LoadTrainingDashboard(),
  LoadTrainingPromiseGates(),
  LoadTrainingOperatorReadiness(),
  LoadTrainingEvidencePacketSummary(),
  LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(model) }),
  LoadPublicActivityTimeline(),
]

const trainingBootstrapShouldRefresh = (
  projection: TrainingBootstrapGrantResponse,
): boolean =>
  projection.reason === "granted" ||
  projection.reason === "queued" ||
  projection.reason === "refused"

const isTrainingPane = (pane: PaneId): boolean =>
  pane === "training" || pane === "training-fullscreen"

const isNetworkPane = (pane: PaneId): boolean => pane === "network"
const isBuiltInAgentPane = (pane: PaneId): boolean => pane === "builtin-agent"
const isSettingsPane = (pane: PaneId): boolean => pane === "settings"
const isDiagnosticsPane = (pane: PaneId): boolean => pane === "diagnostics"
// AO-4 (#5445): the onboarding wizard pane refreshes the identity-choice state +
// the live onboarding chain status whenever it is opened.
const isOnboardingPane = (pane: PaneId): boolean => pane === "onboarding"
// CS-A1 / VCODE-03: account-managing panes refresh the node-local managed
// account registry. `accounts` is the dedicated Verse code-mode dock; Composer,
// Spawn, and Settings keep their legacy account surfaces.
const isAccountManagingPane = (pane: PaneId): boolean =>
  pane === "accounts" || pane === "composer" || pane === "spawn" || pane === "settings"

const diagnosticsRefreshCommands = (): ReadonlyArray<Command.Command<Message>> => [
  LoadManagedAccounts(),
  LoadInferenceGatewayReadiness(),
  LoadBuiltInAgentReadiness(),
  LoadAppleFmReadiness(),
  LoadInstallReadiness(),
]

const managedAccountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

const routeHash = (value: string): string => {
  let hash = 2166136261
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(16).padStart(8, "0")
}

const routeSafeSegment = (value: string): string =>
  value
    .trim()
    .replace(/[^A-Za-z0-9._-]+/g, ".")
    .replace(/^\.+|\.+$/g, "")
    .slice(0, 96) || "unknown"

const fallbackCodeModeAccountRows = (model: Model): readonly CodeModeSyncAccountRow[] =>
  (modelNode(model)?.accounts ?? []).map((row) => ({
    key:
      row.accountRef !== null
        ? `${row.provider}:ref:${row.accountRef}`
        : `${row.provider}:hash:${row.accountRefHash}`,
    provider: row.provider,
    accountRef: row.accountRef,
    accountRefHash: row.accountRefHash,
    label:
      row.accountRef !== null
        ? `${row.provider} ${row.accountRef}`
        : `${row.provider} default`,
    selector: row.selector,
    ready: row.ready,
    managed: false,
    live: true,
    homePresent: null,
    priority: row.priority,
    blockerRefs: row.blockerRefs,
    source: row.selector === "default_home" ? "default_home" : "live_only",
  }))

const composerRouteWorkspaceRef = (model: Model): string | null => {
  const sessions = modelCodeModeSync(model)?.sessions ?? modelNode(model)?.sessions ?? []
  const sessionRef = model.composerSessionRef ?? model.selectedSessionRef
  const sessionWorkspace =
    sessionRef === null
      ? null
      : (sessions.find((row) => row.sessionRef === sessionRef)?.workspaceRef ?? null)
  if (sessionWorkspace !== null && sessionWorkspace.trim() !== "") return sessionWorkspace
  if (model.composerWorkspaceMode === "managed") {
    const parsed = parseManagedWorktreeRequest({
      repo: model.composerManagedRepo,
      baseRef: model.composerManagedBaseRef,
    })
    return parsed.ok
      ? `workspace.github.${routeSafeSegment(parsed.request.fullName)}.${routeSafeSegment(parsed.request.baseRef)}`
      : null
  }
  const path = model.composerRepoPath.trim()
  return path === "" ? null : `workspace.local.${routeHash(path)}`
}

const composerAccountRoute = (
  model: Model,
  adapter: CodeModeSpawnAdapter,
): CodeModeAccountRoute =>
  projectCodeModeAccountRoute({
    adapter,
    selectedAccountRef: model.composerAccountRef,
    accounts: modelCodeModeSync(model)?.accounts ?? fallbackCodeModeAccountRows(model),
    sessions: modelCodeModeSync(model)?.sessions ?? modelNode(model)?.sessions ?? [],
    workspaceRef: composerRouteWorkspaceRef(model),
    allowDefaultHome: true,
  })

const composerAccountBlocker = (
  model: Model,
  adapter: CodeModeSpawnAdapter,
): string | null => composerAccountRoute(model, adapter).blocker

const composerAccountRefForAdapter = (
  model: Model,
  adapter: CodeModeSpawnAdapter,
): string | null => {
  const route = composerAccountRoute(model, adapter)
  return route.blocker === null ? route.accountRef : null
}

const onboardingCompletionPane = (
  currentPane: PaneId,
  projection: OnboardingStatusResponse,
): PaneId =>
  projection.complete && currentPane === "onboarding" ? "chat" : currentPane

const eventCountLabel = (count: number): string =>
  `${count} ${count === 1 ? "event" : "events"}`

const sourceWarningCountLabel = (count: number): string =>
  `${count} source ${count === 1 ? "warning" : "warnings"}`

const plannedRunFirstObservedAt = (
  model: Model,
  projection: TrainingRunsResponse,
): string | null => {
  if (model.trainingPlanFirstObservedAt !== null) {
    return model.trainingPlanFirstObservedAt
  }
  if (!projection.ok) return null

  const plan = model.trainingPlan as TrainingPlanResponse | null
  const runRef = plan?.trainingRunRef ?? null
  if (runRef === null || runRef.trim() === "") return null

  const observed =
    projection.runs.some(run => run.trainingRunRef === runRef) ||
    projection.summaries.some(summary => summary.run.trainingRunRef === runRef)

  return observed ? projection.fetchedAt : null
}

const chatMessageId = (prefix: string): string =>
  `${prefix}.${Date.now().toString(36)}`

const chatTimestamp = (): string => new Date().toISOString()

// EPIC #6017: coerce the opaque `RespondedVerseKhala.receipt` (a public-safe
// KhalaReceiptProjection over the RPC) into the serializable VerseKhalaReceipt
// the model holds. Tolerant: a missing/malformed block (or a turn that carried no
// `openagents` block) yields null so no effect fires. We persist only the
// routing/verification fields the effect + inspector need (rubric is dropped).
const verseKhalaReceiptFromUnknown = (
  value: unknown,
): VerseKhalaReceipt | null => {
  if (typeof value !== "object" || value === null) return null
  const record = value as Record<string, unknown>
  const asStr = (v: unknown): string => (typeof v === "string" ? v : "")
  const asNullableStr = (v: unknown): string | null =>
    typeof v === "string" && v.trim().length > 0 ? v : null
  const verification =
    record.verification === "test_passed" || record.verification === "failed"
      ? record.verification
      : "none"
  return {
    requestedModel: asStr(record.requestedModel),
    servedModel: asStr(record.servedModel),
    worker: asStr(record.worker),
    lane: asStr(record.lane).length > 0 ? asStr(record.lane) : "default",
    verification,
    verified: typeof record.verified === "boolean" ? record.verified : null,
    receipt: asNullableStr(record.receipt),
    receiptUrl: asNullableStr(record.receiptUrl),
  }
}

// #5466: the objective embeds the SEMANTICALLY-selected signature ref (not a
// hardcoded one) so the bounded session runs the program the router chose. The
// exact-replay digest/verdict are NOT asserted here — they are derived from the
// real session events on reconciliation (blueprint-chat-runtime.ts).
const buildBlueprintChatObjective = (
  prompt: string,
  signatureRef: string,
): string =>
  [
    "Run one Blueprint chat-program turn for Autopilot.",
    `Selected signature: ${signatureRef}`,
    [
      "Use the scoped tool menu only.",
      "Direct writes, deploys, email, and spend are denied unless returned as evidence for operator approval.",
    ].join(" "),
    [
      "For any Tassadar module step, report only public refs, the exact-replay verdict,",
      "and digest. Do not include raw traces.",
    ].join(" "),
    [
      `For proof replay modules, use signature ${BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF}`,
      `and tool ${BLUEPRINT_CHAT_REPLAY_TOOL_REF}; return public bundle refs only.`,
    ].join(" "),
    "",
    "User turn:",
    prompt,
  ].join("\n")

// #5471: begin a managed-worktree composer turn. Stores the fully-built
// objective on the model and fires the node-side repoRef resolution; the
// ResolvedComposerManagedWorktree handler then fires the actual SpawnComposerTurn
// once a commit SHA comes back. Shared by the first-turn and reply paths so the
// resolve→spawn handoff is identical for both.
const startManagedComposerTurn = (
  model: Model,
  request: ManagedWorktreeRequest,
  objective: string,
  turns: ReadonlyArray<string>,
): Result => [
  Model.make({
    ...model,
    composerPending: true,
    composerReply: "",
    composerPendingObjective: objective,
    composerTurns: [...turns],
    composerStatus: {
      text: `resolving managed worktree (${managedWorktreeLabel(request)})…`,
      tone: "info",
    },
  }),
  [
    ResolveManagedWorktree({
      fullName: request.fullName,
      baseRef: request.baseRef,
      branch: request.branch,
    }),
  ],
]
// #5466: reconcile the live chat turn. For the assistant message linked to the
// active chat session, re-derive its Blueprint program steps from the REAL
// session events (`node.events[chatSessionRef]`). The semantic signature
// selection is recomputed deterministically from the triggering user message
// (the user turn immediately preceding the assistant message), so the rendered
// signature stays the one the router actually chose. A step is `verified` only
// when the real event evidence says so. Pure: a function of the model + node.
const reconcileChatTurn = (model: Model): Model => {
  const sessionRef = model.chatSessionRef
  if (sessionRef === null) return model
  const node = model.node as
    | { events?: Record<string, ReadonlyArray<unknown>> }
    | null
  const events = ((node?.events?.[sessionRef] ?? []) as ReadonlyArray<{
    eventIndex: number
    phase: string
    state: string
    observedAt: string
    detail: string
    full?: string
  }>)
  const messages = model.chatMessages as ReadonlyArray<ChatMessage>
  let changed = false
  const next = messages.map((message, index) => {
    if (message.role !== "assistant" || message.linkedSessionRef !== sessionRef) {
      return message
    }
    // The semantic selection is derived from the user turn that triggered this
    // assistant message (the nearest preceding user message).
    let prompt = ""
    for (let i = index - 1; i >= 0; i--) {
      if (messages[i]!.role === "user") {
        prompt = messages[i]!.body
        break
      }
    }
    const selection = selectSignatureForMessage(prompt)
    const steps = liveChatScopedSteps({
      selection,
      linkedSessionRef: sessionRef,
      events,
      proofReplaySlug: DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
    })
    changed = true
    return { ...message, steps }
  })
  if (!changed) return model
  return Model.make({ ...model, chatMessages: next })
}

// #5464: the filtered palette list for the current query (also drives which
// command Enter runs and how the selection index clamps).
const paletteCommandSetForModel = (model: Model): ReadonlyArray<PaletteCommand> =>
  model.pane === "chat" && model.verseMode === "code"
    ? codeModePaletteCommands
    : paletteCommands

const paletteMatchesForModel = (model: Model): ReadonlyArray<PaletteCommand> =>
  filterPaletteCommands(
    model.commandPaletteQuery,
    paletteCommandSetForModel(model),
  ).map((match) => match.command)

const clampPaletteIndex = (index: number, length: number): number => {
  if (length <= 0) return 0
  if (index < 0) return 0
  if (index >= length) return length - 1
  return index
}

// Map a registry PaletteCommand to the EXISTING Message it dispatches (audit
// §5.2/§5.3 — no new control verb). `navigate` → NavigatedTo; `action` → the
// message named by `messageTag` (currently NavigatedTo / ClickedSubmitIntent /
// ClickedCoordinatorToggle). Unknown tags resolve to null (defensive no-op).
const messageForPaletteCommand = (command: PaletteCommand): Message | null => {
  if (command.kind === "navigate") return NavigatedTo({ pane: command.pane })
  switch (command.messageTag) {
    case "NavigatedTo": {
      const pane = (command.args?.pane ?? null) as PaneId | null
      return pane ? NavigatedTo({ pane }) : null
    }
    // HUD H3 (#5501): "Open <X> as a pane" — open the destination as a managed
    // floating window (the pane layer) rather than swap the single-pane router.
    case "OpenedManagedPane": {
      const pane = (command.args?.pane ?? null) as PaneId | null
      return pane ? OpenedManagedPane({ pane }) : null
    }
    case "ClickedSubmitIntent":
      return ClickedSubmitIntent()
    case "ClickedBlueprintChatSubmit":
      return ClickedBlueprintChatSubmit()
    case "ClickedCoordinatorToggle":
      return ClickedCoordinatorToggle({ paused: Boolean(command.args?.paused) })
    default:
      return null
  }
}

// #5465: the submit message for the current Chat/Composer turn. Composer's first
// turn spawns (ClickedComposerSpawn); subsequent turns continue the thread
// (ClickedComposerReply). Chat uses the #5821 Verse/Tassadar path by default.
const submitTurnMessage = (model: Model, pane: PaneId): Message | null => {
  if (pane === "chat") return ClickedChatSubmit()
  if (pane === "composer") {
    return model.composerSessionRef === null
      ? ClickedComposerSpawn()
      : ClickedComposerReply()
  }
  return null
}

export const update = (model: Model, message: Message): Result => {
  switch (message._tag) {
    // ── Inbound projections ────────────────────────────────────────────────
    case "GotNodeState":
      // #5466: each poll reconciles the live chat turn's program steps from the
      // real session events (verified only on real terminal evidence).
      return [
        withCodeModeSync(
          reconcileShellCodingTurns(
            reconcileChatTurn(Model.make({ ...model, node: message.node })),
          ),
          "node_state",
        ),
        noCommands,
      ]
    case "GotPylonStats":
      return [Model.make({ ...model, pylonStats: message.stats }), noCommands]
    case "GotChatWorldScene":
      // #5730: latest projected live chat-world pylon scene. Stored opaque;
      // chatPane reads it via modelChatWorldScene and falls back to the static
      // seed on null/empty.
      if (
        chatWorldSceneMaterialKey(model.chatWorldScene as ChatWorldPylonScene | null) ===
        chatWorldSceneMaterialKey(message.scene as ChatWorldPylonScene | null)
      ) {
        recordVerseSceneDiagnostic("chat-world-scene.noop", {
          reason: "material-key-unchanged",
        })
        return [model, noCommands]
      }
      recordVerseSceneDiagnostic("chat-world-scene.accepted", {
        hadPrevious: model.chatWorldScene !== null,
        hasRestorePose: latestVerseLocalPose() !== null,
      })
      return [
        Model.make({
          ...model,
          chatWorldScene: message.scene,
          verseSceneRestorePose: latestVerseLocalPose(),
        }),
        noCommands,
      ]
    case "GotChatWorldPaymentParticle":
      // #5730: a new evidence-bound payment particle. Append to the bounded
      // active set, de-duped by id (the backfill poll and the live stream can
      // both surface the same event), keeping the most recent ones.
      return [
        Model.make({
          ...model,
          chatWorldParticles: appendChatWorldParticle(
            model.chatWorldParticles,
            message.particle,
          ),
        }),
        noCommands,
      ]
    case "GotChatWorldMultiplayer":
      // #5825: public Cloudflare world world projection for the Verse. The webview
      // stores it opaque and the view composes it read-only with training,
      // pylon, and payment layers; reducer never owns world authority.
      return [
        Model.make({ ...model, chatWorldMultiplayer: message.world }),
        noCommands,
      ]
    case "SelectedChatWorldNode":
      // #5730/#5822/#6013: surface payment and Khala Verse source refs, or a
      // Verse training stage's public-ref detail. Plain pylon nodes still clear
      // the inspector.
      return [
        Model.make({
          ...model,
          chatWorldInspectedRef: message.id.startsWith("pay:") ||
            message.id.startsWith(CHAT_WORLD_GATEWAY_NODE_PREFIX) ||
            message.id.startsWith(CHAT_WORLD_INFERENCE_NODE_PREFIX)
            ? chatWorldRefFromLabel(message.label)
            : message.id.startsWith(VERSE_TRAINING_NODE_PREFIX)
              ? message.label
            : null,
        }),
        noCommands,
      ]
    case "ChangedVersePresenceZone":
      return [
        Model.make({ ...model, versePresenceZone: message.zone }),
        noCommands,
      ]
    case "ChangedVerseWorldItemProximity":
      return [
        Model.make({ ...model, nearVerseWorldItemId: message.itemId }),
        noCommands,
      ]
    case "ChangedVerseLocalPose":
      recordLatestVerseLocalPose(message.pose)
      if (
        message.pose.capturedAtMs - lastLocalPoseDiagnosticAtMs >= 1000
      ) {
        lastLocalPoseDiagnosticAtMs = message.pose.capturedAtMs
        recordVerseSceneDiagnostic("local-pose.cached", {
          x: message.pose.x,
          y: message.pose.y,
          z: message.pose.z,
          animation: message.pose.animation,
          capturedAtMs: message.pose.capturedAtMs,
        })
      }
      return [
        model,
        [PublishVerseLocalPose({ pose: message.pose })],
      ]
    case "SettledVerseLocalPosePublish":
      return [model, noCommands]
    case "ChangedInputProfile":
      return [
        Model.make({
          ...model,
          inputProfile: safeInputProfileValue(message.profile),
          inputBindingCapture: null,
        }),
        noCommands,
      ]
    case "StartedInputBindingCapture":
      return [
        Model.make({
          ...model,
          inputBindingCapture: {
            actionId: message.actionId,
            slot: message.slot,
          },
        }),
        noCommands,
      ]
    case "CancelledInputBindingCapture":
      return [
        Model.make({ ...model, inputBindingCapture: null }),
        noCommands,
      ]
    case "CapturedInputBinding": {
      const binding = decodeInputBindingOrNull(message.binding)
      if (binding === null) {
        return [Model.make({ ...model, inputBindingCapture: null }), noCommands]
      }
      const next = Model.make({
        ...model,
        inputProfile: inputProfileWithBinding(
          model.inputProfile,
          message.actionId,
          message.slot,
          binding,
        ),
        inputBindingCapture: null,
      })
      return [next, [persistInputProfileFor(next)]]
    }
    case "ResetInputBinding": {
      const next = Model.make({
        ...model,
        inputProfile: inputProfileWithResetAction(
          model.inputProfile,
          message.actionId,
        ),
        inputBindingCapture: null,
      })
      return [next, [persistInputProfileFor(next)]]
    }
    case "ResetInputBindingCategory": {
      const next = Model.make({
        ...model,
        inputProfile: inputProfileWithResetCategory(
          model.inputProfile,
          message.category,
        ),
        inputBindingCapture: null,
      })
      return [next, [persistInputProfileFor(next)]]
    }
    case "ResetAllInputBindings": {
      const next = Model.make({
        ...model,
        inputProfile: inputProfileWithResetAll(),
        inputBindingCapture: null,
      })
      return [next, [persistInputProfileFor(next)]]
    }
    case "SettledPersistInputProfile":
      return [model, noCommands]
    case "GotNotifications":
      return [Model.make({ ...model, notifications: message.view }), noCommands]
    case "GotNodeLaunchStatus":
      return [
        Model.make({ ...model, nodeLaunchStatus: message.status }),
        // AO-4 (#5445): a node lifecycle transition also refreshes the live
        // onboarding chain when the wizard is open, so steps move in real time.
        isOnboardingPane(model.pane)
          ? [LoadInstallReadiness(), LoadOnboardingStatus()]
          : [LoadInstallReadiness()],
      ]

    // ── Navigation ─────────────────────────────────────────────────────────
    case "NavigatedTo":
      return [
        Model.make({
          ...model,
          pane: message.pane,
          expandedEvents: [],
          ...(isBuiltInAgentPane(message.pane)
            ? {
                builtInAgentStatus: {
                  text: "checking OpenAgents compute...",
                  tone: "info" as const,
                },
                appleFmPending: true,
                appleFmStatus: {
                  text: "checking local Apple FM...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isSettingsPane(message.pane)
            ? {
                installReadinessPending: true,
                installReadinessStatus: {
                  text: "checking first-run health...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isTrainingPane(message.pane)
            ? {
                publicActivityTimelinePending: true,
                publicActivityTimelineStatus: {
                  text: "loading public activity...",
                  tone: "info" as const,
                },
                trainingRunsPending: true,
                trainingRunsStatus: {
                  text: "loading Worker projection...",
                  tone: "info" as const,
                },
                trainingDashboardPending: true,
                trainingDashboardStatus: {
                  text: "loading public dashboards...",
                  tone: "info" as const,
                },
                trainingPromiseGatesPending: true,
                trainingPromiseGatesStatus: {
                  text: "loading promise gates...",
                  tone: "info" as const,
                },
                trainingOperatorReadinessPending: true,
                trainingOperatorReadinessStatus: {
                  text: "checking operator readiness...",
                  tone: "info" as const,
                },
                trainingEvidencePacketSummaryPending: true,
                trainingEvidencePacketSummaryStatus: {
                  text: "checking evidence packet...",
                  tone: "info" as const,
                },
                proofReplayPending: true,
                proofReplayStatus: {
                  text: "loading public replay bundle...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isNetworkPane(message.pane)
            ? {
                publicActivityTimelinePending: true,
                publicActivityTimelineStatus: {
                  text: "loading public activity...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isOnboardingPane(message.pane)
            ? {
                onboardingPending: true,
                identityChoicePending: true,
                onboardingStatusLine: {
                  text: "loading onboarding status...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isAccountManagingPane(message.pane)
            ? {
                managedAccountsPending: true,
                managedAccountsStatus: {
                  text: "loading accounts...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isDiagnosticsPane(message.pane)
            ? {
                managedAccountsPending: true,
                managedAccountsStatus: {
                  text: "loading diagnostics...",
                  tone: "info" as const,
                },
              }
            : {}),
        }),
        [
          ...(isTrainingPane(message.pane)
            ? loadTrainingProjectionCommands(model)
            : isNetworkPane(message.pane)
              ? [LoadPublicActivityTimeline()]
              : isBuiltInAgentPane(message.pane)
              ? [
                  LoadBuiltInAgentReadiness(),
                  LoadAppleFmReadiness(),
                  LoadPromiseSurfacingReadiness(),
                ]
              : isSettingsPane(message.pane)
                ? [LoadInstallReadiness()]
                : message.pane === "chat"
                  ? [
                      LoadIdentityChoiceState(),
                      LoadOnboardingStatus(),
                      LoadTrainingOperatorReadiness(),
                    ]
                  : isOnboardingPane(message.pane)
                    ? [LoadIdentityChoiceState(), LoadOnboardingStatus()]
                    : noCommands),
          ...(isAccountManagingPane(message.pane)
            ? // #5485: the account-managing panes (composer/spawn/settings) are
              // exactly where the own-auth-vs-gateway routing matters, so refresh
              // the gateway readiness (credits/hint) on enter alongside accounts.
              [LoadManagedAccounts(), LoadInferenceGatewayReadiness()]
            : noCommands),
          ...(isDiagnosticsPane(message.pane) ? diagnosticsRefreshCommands() : noCommands),
        ],
      ]

    // #5463: a group jump resolves to the group's default pane, then reuses the
    // full NavigatedTo handler above (so per-pane projection loads still fire).
    case "NavigatedToGroup": {
      const group = groupById(message.group)
      if (!group) return [model, noCommands]
      return update(model, NavigatedTo({ pane: group.defaultPane }))
    }

    // ── Command palette (#5464) ─────────────────────────────────────────────
    case "OpenedCommandPalette":
      if (verseControlsDisabled(model)) return [model, noCommands]
      return [
        Model.make({
          ...model,
          commandPaletteOpen: true,
          commandPaletteQuery: "",
          commandPaletteIndex: 0,
        }),
        noCommands,
      ]
    case "ClosedCommandPalette":
      return [Model.make({ ...model, commandPaletteOpen: false }), noCommands]
    case "ChangedCommandPaletteQuery":
      // A new query re-filters the list, so reset the highlight to the top.
      return [
        Model.make({
          ...model,
          commandPaletteQuery: message.value,
          commandPaletteIndex: 0,
        }),
        noCommands,
      ]
    case "MovedCommandPaletteSelection": {
      const length = paletteMatchesForModel(model).length
      return [
        Model.make({
          ...model,
          commandPaletteIndex: clampPaletteIndex(
            model.commandPaletteIndex + message.delta,
            length,
          ),
        }),
        noCommands,
      ]
    }
    case "RanPaletteCommand": {
      // Resolve the command: by explicit id (a click) or the highlighted row
      // (Enter). Close the palette, then re-dispatch the command's existing
      // Message so all real handlers/effects run unchanged.
      const command =
        message.commandId === null
          ? (paletteMatchesForModel(model)[model.commandPaletteIndex] ?? null)
          : (paletteCommandSetForModel(model).find((c) => c.id === message.commandId) ?? null)
      const closed = Model.make({ ...model, commandPaletteOpen: false })
      if (!command) return [closed, noCommands]
      const next = messageForPaletteCommand(command)
      return next ? update(closed, next) : [closed, noCommands]
    }

    // ── Keyboard layer (#5465) ──────────────────────────────────────────────
    // PressedKey is interpreted purely (keyboard.ts) then re-dispatched as an
    // existing Message, so every shortcut reuses a real handler (no no-ops).
    case "PressedKey": {
      const keyEvent = message.code === undefined
        ? {
            key: message.key,
            meta: message.meta,
            ctrl: message.ctrl,
            shift: message.shift,
            inEditable: message.inEditable,
          }
        : {
            key: message.key,
            code: message.code,
            meta: message.meta,
            ctrl: message.ctrl,
            shift: message.shift,
            inEditable: message.inEditable,
          }
      const intent = interpretKey(model, keyEvent)
      if (
        verseControlsDisabled(model) &&
        intent.kind !== "open-coder-session" &&
        intent.kind !== "close-managed-panes" &&
        intent.kind !== "hide-code-dock" &&
        // Dev affordance (#6033): the spawn keys are explicit explore-mode shortcuts
        // (interpretKey only emits them in the Verse explore context), so they are
        // allowed through even though general explore-mode controls are disabled.
        intent.kind !== "spawn-verse-scene" &&
        intent.kind !== "toggle-verse-scene-portal"
      ) {
        return [model, noCommands]
      }
      switch (intent.kind) {
        case "none":
          return [model, noCommands]
        case "open-palette":
          return update(model, OpenedCommandPalette())
        case "close-palette":
          return update(model, ClosedCommandPalette())
        case "palette-move":
          return update(model, MovedCommandPaletteSelection({ delta: intent.delta }))
        case "palette-run":
          return update(model, RanPaletteCommand({ commandId: null }))
        case "navigate-pane":
          return update(model, NavigatedTo({ pane: intent.pane }))
        case "back-to-shell":
          // Inline the shell return — do NOT call ClosedPanes() here. The message
          // constructors are type-only imports in this module, so using one as a
          // value crashes at runtime ("Can't find variable: ClosedPanes"). This
          // is byte-identical to the ClosedPanes reducer (pane → shell, palette
          // closed).
          return [
            Model.make({ ...model, pane: "shell", commandPaletteOpen: false }),
            noCommands,
          ]
        case "submit-turn": {
          const submit = submitTurnMessage(model, intent.pane)
          return submit ? update(model, submit) : [model, noCommands]
        }
        case "toggle-verse":
          // #5730 The Verse: inline the toggle (message constructors are
          // type-only imports here). Byte-identical to the ToggleVerse reducer.
          if (verseControlsDisabled(model)) return [model, noCommands]
          return [
            Model.make({ ...model, verseEnabled: !model.verseEnabled }),
            noCommands,
          ]
        case "spawn-verse-scene":
          // Dev affordance (#6033): inline the spawn toggle (message constructors
          // are type-only imports here). Byte-identical to the SpawnedVerseScene
          // reducer below.
          return [toggleVerseSpawnedScene(model, intent.sceneId), noCommands]
        case "toggle-verse-scene-portal":
          return [
            toggleVerseSpawnedScenePortal(model, intent.sceneId),
            noCommands,
          ]
        case "open-coder-session":
          return openNewCoderSession(model)
        case "close-managed-panes":
          return update(model, ClosedAllManagedPanes())
        case "hide-code-dock":
          return update(model, ChangedVerseMode({ mode: "explore" }))
      }
    }

    case "SelectedSession":
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            pane: "session-detail",
            selectedSessionRef: message.sessionRef,
            sessionDetailView: "overview",
            expandedEvents: [],
            selectedDiffFilePath: null,
          }),
          "model_tick",
        ),
        noCommands,
      ]
    case "ChangedSessionFilter":
      return [Model.make({ ...model, sessionFilter: message.filter }), noCommands]
    case "ChangedSessionAdapterFilter":
      return [
        Model.make({ ...model, sessionAdapterFilter: message.adapter }),
        noCommands,
      ]
    case "ChangedSessionAccountFilter":
      return [
        Model.make({ ...model, sessionAccountFilter: message.account }),
        noCommands,
      ]
    case "ChangedSessionWorkspaceFilter":
      return [
        Model.make({ ...model, sessionWorkspaceFilter: message.workspace }),
        noCommands,
      ]
    case "SelectedSessionDetailView":
      return [
        Model.make({ ...model, sessionDetailView: message.view }),
        noCommands,
      ]
    case "ToggledEvent": {
      const set = new Set(model.expandedEvents)
      if (set.has(message.eventIndex)) set.delete(message.eventIndex)
      else set.add(message.eventIndex)
      return [Model.make({ ...model, expandedEvents: [...set] }), noCommands]
    }
    // #5470 session-detail diff browser: per-file expand, layout flip, browser.
    case "ToggledDiffFile": {
      const set = new Set(model.expandedDiffFiles)
      if (set.has(message.path)) set.delete(message.path)
      else set.add(message.path)
      return [
        Model.make({
          ...model,
          expandedDiffFiles: [...set],
          selectedDiffFilePath: message.path,
        }),
        noCommands,
      ]
    }
    case "SelectedDiffFile":
      return [
        Model.make({ ...model, selectedDiffFilePath: message.path }),
        noCommands,
      ]
    case "ToggledDiffViewMode":
      return [
        Model.make({ ...model, diffViewMode: model.diffViewMode === "split" ? "unified" : "split" }),
        noCommands,
      ]
    case "ToggledArtifactBrowser":
      return [Model.make({ ...model, artifactBrowserOpen: !model.artifactBrowserOpen }), noCommands]

    // #5730 The Verse: flip the runtime toggle for the game-world view. Pure
    // model toggle — the view gates the scene render on this + the build flag.
    case "ToggleVerse":
      if (verseControlsDisabled(model)) return [model, noCommands]
      return [Model.make({ ...model, verseEnabled: !model.verseEnabled }), noCommands]
    // Dev affordance (#6033): spawn / unspawn an isolated scene station into the
    // live Verse, and toggle its optional gateway portal. Pure model state.
    case "SpawnedVerseScene":
      return [toggleVerseSpawnedScene(model, message.sceneId), noCommands]
    case "ToggledVerseScenePortal":
      return [toggleVerseSpawnedScenePortal(model, message.sceneId), noCommands]
    case "ClickedHotbarNewCoderSession":
      return openNewCoderSession(model)
    case "ChangedVerseMode": {
      const enteringCode = message.mode === "code" && model.verseMode !== "code"
      const codeModeAdapter =
        model.spawnAdapter === "apple_fm" ? "claude_agent" : model.spawnAdapter
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            verseMode: message.mode,
            ...(enteringCode
              ? {
                  spawnAdapter: codeModeAdapter,
                  composerAccountRef: null,
                  managedAccountsPending: true,
                  managedAccountsStatus: {
                    text:
                      codeModeAdapter === "claude_agent"
                        ? "loading Claude Agent accounts..."
                        : "loading Codex accounts...",
                    tone: "info" as const,
                  },
                }
              : {}),
          }),
          "model_tick",
        ),
        enteringCode ? [LoadManagedAccounts()] : noCommands,
      ]
    }

    // Chat: flip a single message's "program details" disclosure (scoped-step /
    // Tassadar scaffolding). Collapsed by default; pure model toggle.
    case "ToggledChatMessageDetails": {
      const set = new Set(model.expandedChatMessages)
      if (set.has(message.messageId)) set.delete(message.messageId)
      else set.add(message.messageId)
      return [Model.make({ ...model, expandedChatMessages: [...set] }), noCommands]
    }

    // ── Coordinator toggle ───────────────────────────────────────────────────
    case "ClickedCoordinatorToggle":
      return [model, [SetCoordinatorPaused({ paused: message.paused })]]
    case "SettledCoordinatorToggle":
    case "FailedCoordinatorToggle":
      // The next node-state poll carries the authoritative paused flag.
      return [model, noCommands]

    // ── Approvals ──────────────────────────────────────────────────────────────
    case "ClickedResolveApproval":
      // Optimistically hide the row; the command confirms with the node.
      return [
        Model.make({
          ...model,
          resolvedApprovals: [...model.resolvedApprovals, message.approvalRef],
        }),
        [
          ResolveApproval({
            approvalRef: message.approvalRef,
            decision: message.decision,
          }),
        ],
      ]
    case "SettledResolveApproval": {
      // If the node did not accept it (neither applied nor duplicate), un-hide
      // the row so the next poll shows it again.
      if (message.ok) return [model, noCommands]
      return [
        Model.make({
          ...model,
          resolvedApprovals: model.resolvedApprovals.filter(
            (ref) => ref !== message.approvalRef,
          ),
        }),
        noCommands,
      ]
    }

    // ── Deploy ───────────────────────────────────────────────────────────────
    case "ClickedDeploy":
      return [
        Model.make({
          ...model,
          deployFeedback: { state: "queued", text: "deploying…" },
        }),
        [DeployCloud()],
      ]
    case "SucceededDeploy":
      return [
        Model.make({
          ...model,
          deployFeedback: { state: message.state, text: message.text },
        }),
        noCommands,
      ]

    // ── Ask Autopilot ──────────────────────────────────────────────────────────
    case "ChangedAskTitle":
      return [Model.make({ ...model, askTitle: message.value }), noCommands]
    case "ChangedAskBody":
      return [Model.make({ ...model, askBody: message.value }), noCommands]
    case "ClickedSubmitIntent": {
      const validation = validateIntentDraft({
        title: model.askTitle,
        body: model.askBody,
      })
      if (!validation.ok) {
        return [
          Model.make({
            ...model,
            askStatus: {
              text: `error: ${validation.errors[0] ?? "invalid input"}`,
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          askPending: true,
          askStatus: { text: "sending…", tone: "info" },
        }),
        [SubmitIntent({ title: validation.title, body: validation.body })],
      ]
    }
    case "SettledSubmitIntent":
      return [
        Model.make({
          ...model,
          askPending: false,
          askStatus: { text: message.text, tone: message.ok ? "success" : "error" },
          ...(message.ok ? { askTitle: "", askBody: "" } : {}),
        }),
        noCommands,
      ]

    // ── Built-in no-user-key agent (#5063) ───────────────────────────────────
    case "ClickedRefreshBuiltInAgent":
      return [
        Model.make({
          ...model,
          builtInAgentStatus: {
            text: "checking OpenAgents compute...",
            tone: "info",
          },
        }),
        [LoadBuiltInAgentReadiness()],
      ]
    case "GotBuiltInAgentReadiness": {
      const projection = message.projection as BuiltInAgentReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            builtInAgentReadiness: projection,
            builtInAgentStatus: projection.ok
              ? {
                  text: `ready · ${projection.meteringLabel}`,
                  tone: "success",
                }
              : {
                  text:
                    projection.error ??
                    `${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                  tone: projection.error ? "error" : "info",
                },
          }),
          "readiness",
        ),
        noCommands,
      ]
    }
    case "SelectedAgentMode":
      return [
        Model.make({
          ...model,
          agentMode: message.mode,
        }),
        noCommands,
      ]
    case "ClickedRefreshAppleFm":
      return [
        Model.make({
          ...model,
          appleFmPending: true,
          appleFmStatus: {
            text: "checking local Apple FM...",
            tone: "info",
          },
        }),
        [LoadAppleFmReadiness()],
      ]
    case "GotAppleFmReadiness": {
      const projection = message.projection as AppleFmReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            appleFmReadiness: projection,
            appleFmPending: false,
            appleFmStatus: projection.ok
              ? {
                  text: `ready · ${projection.model}`,
                  tone: "success",
                }
              : {
                  text:
                    projection.error ??
                    projection.message ??
                    `${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                  tone: projection.error ? "error" : "info",
                },
          }),
          "readiness",
        ),
        noCommands,
      ]
    }
    case "GotInferenceGatewayReadiness":
      // #5485: store the public-safe gateway readiness. It drives the routing
      // decision (decideInference) + the composer low-balance hint. No status
      // line of its own — the coding surfaces read it directly off the model.
      return [
        withCodeModeSync(
          Model.make({ ...model, inferenceGatewayReadiness: message.projection }),
          "readiness",
        ),
        noCommands,
      ]
    case "ClickedStartAppleFm":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          appleFmPending: true,
          appleFmStatus: {
            text: "starting local Apple FM session...",
            tone: "info",
          },
        }),
        [StartAppleFmSession()],
      ]
    case "SucceededAppleFmSession":
      return [
        Model.make({
          ...model,
          appleFmPending: false,
          appleFmStatus: {
            text: `local session online · ${message.sessionRef}`,
            tone: "success",
          },
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedAppleFmSession":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          appleFmPending: false,
          appleFmStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]
    case "ClickedStartBuiltInAgent":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          builtInAgentPending: true,
          builtInAgentStatus: {
            text: "starting hosted agent...",
            tone: "info",
          },
        }),
        [StartBuiltInAgent()],
      ]
    case "SucceededBuiltInAgent":
      return [
        Model.make({
          ...model,
          builtInAgentPending: false,
          builtInAgentStatus: {
            text: `online · ${message.sessionRef}`,
            tone: "success",
          },
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedBuiltInAgent":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          builtInAgentPending: false,
          builtInAgentStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── First-run install/runtime readiness (#5064) ─────────────────────────
    case "ClickedRefreshInstallReadiness":
      return [
        Model.make({
          ...model,
          installReadinessPending: true,
          installReadinessStatus: {
            text: "checking first-run health...",
            tone: "info",
          },
        }),
        [LoadInstallReadiness()],
      ]
    case "GotInstallReadiness": {
      const projection = message.projection as InstallReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        Model.make({
          ...model,
          installReadiness: projection,
          installReadinessPending: false,
          installReadinessStatus: projection.ok
            ? {
                text: `${projection.highestRoiAction} · ready`,
                tone: "success",
              }
            : {
                text: `${projection.highestRoiAction} · ${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                tone: "info",
              },
        }),
        noCommands,
      ]
    }

    // ── AO-3/AO-4 first-run onboarding wizard (#5444 / #5445) ───────────────
    case "ClickedRefreshOnboarding":
      return [
        Model.make({
          ...model,
          onboardingPending: true,
          onboardingStatusLine: {
            text: "refreshing onboarding status...",
            tone: "info",
          },
        }),
        [LoadIdentityChoiceState(), LoadOnboardingStatus()],
      ]
    // AO-4: retry a failed step — re-loads the chain. The Bun supervisor keeps
    // converging (restart-on-crash + offline-tolerant registration), so a retry
    // is "re-read the real state now" rather than a dead/blank screen.
    case "ClickedRetryOnboarding":
      return [
        Model.make({
          ...model,
          onboardingPending: true,
          onboardingStatusLine: { text: "retrying...", tone: "info" },
        }),
        [LoadOnboardingStatus()],
      ]
    case "TickedOnboardingStatusRefresh":
      return [model, [LoadOnboardingStatus()]]
    case "GotOnboardingStatus": {
      const projection = message.projection as OnboardingStatusResponse
      return [
        Model.make({
          ...model,
          pane: onboardingCompletionPane(model.pane, projection),
          onboardingStatus: projection,
          onboardingPending: false,
          onboardingStatusLine: projection.complete
            ? { text: "earning — onboarding complete", tone: "success" }
            : projection.ok === false
              ? { text: "status refresh needs a retry", tone: "error" }
            : projection.hasRetryableFailure
              ? { text: "a step needs a retry", tone: "error" }
              : {
                  text: `current step: ${projection.currentStepId ?? "—"}`,
                  tone: "info",
                },
        }),
        noCommands,
      ]
    }
    case "GotIdentityChoiceState":
      return [
        Model.make({
          ...model,
          identityChoiceState: message.state as IdentityChoiceStateResponse,
          identityChoicePending: false,
        }),
        noCommands,
      ]
    case "ChangedNewIdentityName":
      return [
        Model.make({ ...model, newIdentityName: message.value }),
        noCommands,
      ]
    case "ClickedUseExistingIdentity":
      return [
        Model.make({ ...model, identityChoicePending: true }),
        [ChooseIdentity({ kind: "use_existing", displayName: "" })],
      ]
    case "ClickedCreateNewIdentity":
      return [
        Model.make({ ...model, identityChoicePending: true }),
        [
          ChooseIdentity({
            kind: "create_new",
            displayName: model.newIdentityName.trim(),
          }),
        ],
      ]
    case "SettledChooseIdentity": {
      const result = message.result as ChooseIdentityResponse
      return [
        Model.make({
          ...model,
          identityChoiceState: result.state,
          identityChoicePending: false,
          onboardingStatusLine: result.ok
            ? { text: "identity chosen", tone: "success" }
            : {
                text: result.error ?? "could not record choice",
                tone: "error",
              },
        }),
        // Re-load the chain so the wizard advances past the identity step.
        result.ok ? [LoadOnboardingStatus()] : noCommands,
      ]
    }

    // ── Product Promises Forum surfacing (#5065) ────────────────────────────
    case "ChangedPromiseSurfacingPromiseId":
      return [
        Model.make({ ...model, promiseSurfacingPromiseId: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingSurface":
      return [
        Model.make({ ...model, promiseSurfacingSurface: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingClaimText":
      return [
        Model.make({ ...model, promiseSurfacingClaimText: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingExpectedBehavior":
      return [
        Model.make({
          ...model,
          promiseSurfacingExpectedBehavior: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingObservedBehavior":
      return [
        Model.make({
          ...model,
          promiseSurfacingObservedBehavior: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingEvidenceOrSteps":
      return [
        Model.make({
          ...model,
          promiseSurfacingEvidenceOrSteps: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingEnvironment":
      return [
        Model.make({
          ...model,
          promiseSurfacingEnvironment: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingImpact":
      return [
        Model.make({ ...model, promiseSurfacingImpact: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingSuggestedState":
      return [
        Model.make({
          ...model,
          promiseSurfacingSuggestedState: message.value,
        }),
        noCommands,
      ]
    case "ClickedRefreshPromiseSurfacing":
      return [
        Model.make({
          ...model,
          promiseSurfacingReadinessPending: true,
          promiseSurfacingStatus: {
            text: "checking Product Promises Forum...",
            tone: "info",
          },
        }),
        [LoadPromiseSurfacingReadiness()],
      ]
    case "GotPromiseSurfacingReadiness": {
      const projection = message.projection as PromiseSurfacingReadinessResponse
      return [
        Model.make({
          ...model,
          promiseSurfacingReadiness: projection,
          promiseSurfacingReadinessPending: false,
          promiseSurfacingStatus: projection.ok
            ? { text: "Forum posting ready", tone: "success" }
            : {
                text: projection.blockerRefs[0] ?? "Forum posting blocked",
                tone: "info",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedSurfacePromiseGap": {
      const validation = validatePromiseSurfacingInput({
        promiseId: model.promiseSurfacingPromiseId,
        surface: model.promiseSurfacingSurface,
        claimText: model.promiseSurfacingClaimText,
        expectedBehavior: model.promiseSurfacingExpectedBehavior,
        observedBehavior: model.promiseSurfacingObservedBehavior,
        evidenceOrSteps: model.promiseSurfacingEvidenceOrSteps,
        environment: model.promiseSurfacingEnvironment,
        impact: model.promiseSurfacingImpact,
        suggestedState: model.promiseSurfacingSuggestedState,
      })
      if (!validation.ok) {
        return [
          Model.make({
            ...model,
            promiseSurfacingStatus: {
              text: validation.errors[0] ?? "invalid promise report",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          promiseSurfacingSubmitPending: true,
          promiseSurfacingResult: null,
          promiseSurfacingStatus: {
            text: "checking ledger and posting report...",
            tone: "info",
          },
        }),
        [SurfacePromiseGap(validation.input)],
      ]
    }
    case "GotPromiseSurfacingResult": {
      const projection = message.projection as PromiseSurfacingResponse
      const label =
        projection.mode === "posted"
          ? `posted · ${projection.topicUrl ?? projection.topicId ?? "forum topic"}`
          : projection.mode === "drafted"
            ? `drafted · ${projection.blockerRefs[0] ?? "not posted"}`
            : projection.error ?? projection.blockerRefs[0] ?? "blocked"
      return [
        Model.make({
          ...model,
          promiseSurfacingResult: projection,
          promiseSurfacingSubmitPending: false,
          promiseSurfacingStatus: {
            text: label,
            tone: projection.ok
              ? "success"
              : projection.mode === "drafted"
                ? "info"
                : "error",
          },
        }),
        noCommands,
      ]
    }

    // ── Training launch/readiness feedback ───────────────────────────────────
    case "ClickedRefreshPublicActivity":
      return [
        Model.make({
          ...model,
          publicActivityTimelinePending: true,
          publicActivityTimelineStatus: {
            text: "refreshing public activity...",
            tone: "info",
          },
        }),
        [LoadPublicActivityTimeline()],
      ]
    case "GotPublicActivityTimeline": {
      const projection = message.projection as PublicActivityTimelineResponse
      const eventCount = projection.envelope?.events.length ?? 0
      const staleCount =
        projection.envelope?.sourceLag.filter(lag => lag.status !== "current")
          .length ?? 0
      return [
        Model.make({
          ...model,
          publicActivityTimeline: projection,
          publicActivityTimelinePending: false,
          publicActivityTimelineStatus: projection.ok
            ? {
                text: `${eventCountLabel(eventCount)} · ${sourceWarningCountLabel(staleCount)} · ${projection.sourceUrl}`,
                tone: staleCount === 0 ? "success" : "info",
              }
            : {
                text: projection.error ?? "public activity unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedRefreshTrainingRuns":
      return [
        Model.make({
          ...model,
          publicActivityTimelinePending: true,
          publicActivityTimelineStatus: {
            text: "refreshing public activity...",
            tone: "info",
          },
          trainingRunsPending: true,
          trainingRunsStatus: {
            text: "refreshing Worker projection...",
            tone: "info",
          },
          trainingDashboardPending: true,
          trainingDashboardStatus: {
            text: "refreshing public dashboards...",
            tone: "info",
          },
          trainingPromiseGatesPending: true,
          trainingPromiseGatesStatus: {
            text: "refreshing promise gates...",
            tone: "info",
          },
          trainingOperatorReadinessPending: true,
          trainingOperatorReadinessStatus: {
            text: "refreshing operator readiness...",
            tone: "info",
          },
          trainingEvidencePacketSummaryPending: true,
          trainingEvidencePacketSummaryStatus: {
            text: "refreshing evidence packet...",
            tone: "info",
          },
          proofReplayPending: true,
          proofReplayStatus: {
            text: "refreshing proof replay bundle...",
            tone: "info",
          },
        }),
        loadTrainingProjectionCommands(model),
      ]
    case "TickedVerseTrainingProjectionRefresh":
      return [model, [LoadTrainingRuns()]]
    case "SelectedTrainingSceneNode":
      return [
        Model.make({
          ...model,
          selectedTrainingSceneNodeId: message.nodeId,
        }),
        noCommands,
      ]
    case "GotTrainingRuns": {
      const projection = message.projection as TrainingRunsResponse
      const runCount = projection.runs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingRuns: projection,
          trainingRunsPending: false,
          trainingPlanFirstObservedAt: plannedRunFirstObservedAt(
            model,
            projection,
          ),
          trainingRunsStatus: projection.ok
            ? {
                text: `${runCount} runs from ${projection.sourceUrl}`,
                tone: "success",
              }
            : {
                text: projection.error ?? "training projection unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingPromiseGates": {
      const projection = message.projection as TrainingPromiseGatesResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingPromiseGates: projection,
          trainingPromiseGatesPending: false,
          trainingPromiseGatesStatus: projection.ok
            ? {
                text: `${projection.promises.length} training promises · ${blockerCount} blockers · ${projection.registryVersion}`,
                tone: blockerCount === 0 ? "success" : "info",
              }
            : {
                text: projection.error ?? "training promise gates unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingOperatorReadiness": {
      const projection = message.projection as TrainingOperatorReadinessResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingOperatorReadiness: projection,
          trainingOperatorReadinessPending: false,
          trainingOperatorReadinessStatus: projection.ok
            ? {
                text: `operator ready · ${projection.trainingBaseUrl}`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  `${blockerCount} operator blockers · ${projection.trainingBaseUrl}`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingEvidencePacketSummary": {
      const projection =
        message.projection as TrainingEvidencePacketSummaryResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingEvidencePacketSummary: projection,
          trainingEvidencePacketSummaryPending: false,
          trainingEvidencePacketSummaryStatus: projection.ok
            ? {
                text: `packet ready · ${projection.receiptRefCount} receipts · ${projection.distinctPylonCount} pylons`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  `packet blocked · ${blockerCount} blockers`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingDashboard": {
      const projection = message.projection as TrainingDashboardSummaryResponse
      const lanes = projection.leaderboards?.lanes ?? []
      const rankedLaneCount = lanes.filter(lane => lane.rowCount > 0).length
      return [
        Model.make({
          ...model,
          trainingDashboard: projection,
          trainingDashboardPending: false,
          trainingDashboardStatus: projection.ok
            ? {
                text: `${rankedLaneCount}/${lanes.length} ranked lanes from ${projection.sourceUrl}`,
                tone: "success",
              }
            : {
                text: projection.error ?? "training dashboards unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "SelectedProofReplay":
      return [
        Model.make({
          ...model,
          selectedProofReplayMode: "catalog",
          selectedProofReplaySlug: message.slug,
          proofReplay: null,
          proofReplayPending: true,
          proofReplayStatus: {
            text: "loading public replay bundle...",
            tone: "info",
          },
        }),
        [LoadProofReplayBundle({ request: { mode: "catalog", slug: message.slug } })],
      ]
    case "ChangedProofReplayGeneratedFrom":
      return [
        Model.make({ ...model, generatedProofReplayFrom: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedTo":
      return [
        Model.make({ ...model, generatedProofReplayTo: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedRunRef":
      return [
        Model.make({ ...model, generatedProofReplayRunRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedWindowRef":
      return [
        Model.make({ ...model, generatedProofReplayWindowRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedActorRef":
      return [
        Model.make({ ...model, generatedProofReplayActorRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedPairRef":
      return [
        Model.make({ ...model, generatedProofReplayPairRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedKind":
      return [
        Model.make({ ...model, generatedProofReplayKind: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedSource":
      return [
        Model.make({ ...model, generatedProofReplaySource: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedSince":
      return [
        Model.make({ ...model, generatedProofReplaySince: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedLimit":
      return [
        Model.make({ ...model, generatedProofReplayLimit: message.value }),
        noCommands,
      ]
    case "ClickedLoadGeneratedProofReplay": {
      const next = Model.make({
        ...model,
        selectedProofReplayMode: "generated",
        proofReplay: null,
        proofReplayPending: true,
        proofReplayStatus: {
          text: "loading generated public replay bundle...",
          tone: "info",
        },
      })
      return [
        next,
        [LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(next) })],
      ]
    }
    case "ClickedRefreshProofReplay":
      return [
        Model.make({
          ...model,
          proofReplayPending: true,
          proofReplayStatus: {
            text: "refreshing proof replay bundle...",
            tone: "info",
          },
        }),
        [LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(model) })],
      ]
    case "GotProofReplayBundle": {
      const projection = message.projection as DesktopProofReplayProjection
      const sourceLabel =
        projection.entry?.title ??
        projection.bundle?.title ??
        projection.filterLabel ??
        projection.sourceUrl
      const amount = projection.summary?.confirmedZapSats ?? 0
      return [
        Model.make({
          ...model,
          proofReplay: projection,
          proofReplayPending: false,
          proofReplayStatus: projection.ok
            ? {
                text: `${sourceLabel} · ${projection.summary?.eventCount ?? 0} events · ${amount.toLocaleString()} sats`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  projection.blockerRefs[0] ??
                  "proof replay unavailable",
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedPlanTrainingWindow":
      return [
        Model.make({
          ...model,
          trainingPlanPending: true,
          trainingPlanFirstObservedAt: null,
          trainingPlanStatus: {
            text: "planning R1 run window...",
            tone: "info",
          },
        }),
        [PlanTrainingRunWindow()],
      ]
    case "SettledPlanTrainingWindow": {
      const projection = message.projection as TrainingPlanResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingPlan: projection,
          trainingPlanPending: false,
          trainingPlanFirstObservedAt: null,
          trainingPlanStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedActivateTrainingWindow":
      if (message.windowRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingActivationStatus: {
              text: "no planned window selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingActivationPending: true,
          trainingActivationStatus: {
            text: `activating ${message.windowRef}...`,
            tone: "info",
          },
        }),
        [ActivateTrainingWindow({ windowRef: message.windowRef })],
      ]
    case "SettledActivateTrainingWindow": {
      const projection = message.projection as TrainingWindowActionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingActivation: projection,
          trainingActivationPending: false,
          trainingActivationStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedReconcileTrainingWindow":
      if (message.windowRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingReconcileStatus: {
              text: "no sealed window selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingReconcilePending: true,
          trainingReconcileStatus: {
            text: `reconciling ${message.windowRef}...`,
            tone: "info",
          },
        }),
        [ReconcileTrainingWindow({ windowRef: message.windowRef })],
      ]
    case "SettledReconcileTrainingWindow": {
      const projection = message.projection as TrainingWindowActionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingReconcile: projection,
          trainingReconcilePending: false,
          trainingReconcileStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedClaimTrainingLease":
      return [
        Model.make({
          ...model,
          trainingLeasePending: true,
          trainingLeaseStatus: {
            text: "claiming training lease...",
            tone: "info",
          },
        }),
        [ClaimTrainingWindowLease()],
      ]
    case "SettledClaimTrainingLease": {
      const projection = message.projection as TrainingWindowLeaseResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "pylon_ref_missing"
      return [
        Model.make({
          ...model,
          trainingLease: projection,
          trainingLeasePending: false,
          trainingLeaseStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedRequestTrainingBootstrap":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingBootstrapStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingBootstrapPending: true,
          trainingBootstrapStatus: {
            text: `requesting bootstrap for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          RequestTrainingBootstrapGrant({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledRequestTrainingBootstrap": {
      const projection = message.projection as TrainingBootstrapGrantResponse
      const inactiveReason =
        projection.reason === "pylon_ref_missing" ||
        projection.reason === "invalid_pylon_ref" ||
        projection.reason === "refused" ||
        projection.reason === "queued"
      return [
        Model.make({
          ...model,
          trainingBootstrap: projection,
          trainingBootstrapPending: false,
          trainingBootstrapStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        trainingBootstrapShouldRefresh(projection)
          ? loadTrainingProjectionCommands(model)
          : noCommands,
      ]
    }
    case "ClickedBuildTrainingEvidencePacket":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingEvidencePacketBuildStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingEvidencePacketBuildPending: true,
          trainingEvidencePacketBuildStatus: {
            text: `building evidence packet for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          BuildTrainingEvidencePacket({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledBuildTrainingEvidencePacket": {
      const projection =
        message.projection as TrainingEvidencePacketBuildResponse
      const infoReason =
        projection.reason === "disabled" ||
        projection.reason === "worker_receipts_path_missing" ||
        projection.reason === "packet_path_missing" ||
        projection.reason === "packet_blocked"
      const shouldRefresh =
        projection.reason === "written" || projection.reason === "packet_blocked"
      return [
        Model.make({
          ...model,
          trainingEvidencePacketBuild: projection,
          trainingEvidencePacketBuildPending: false,
          trainingEvidencePacketBuildStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : infoReason ? "info" : "error",
          },
        }),
        shouldRefresh ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedAdmitTrainingEvidence":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingEvidenceAdmissionStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingEvidenceAdmissionPending: true,
          trainingEvidenceAdmissionStatus: {
            text: `admitting evidence for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          AdmitTrainingRealGradientEvidence({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledAdmitTrainingEvidence": {
      const projection = message.projection as TrainingEvidenceAdmissionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing" ||
        projection.reason === "packet_path_missing"
      return [
        Model.make({
          ...model,
          trainingEvidenceAdmission: projection,
          trainingEvidenceAdmissionPending: false,
          trainingEvidenceAdmissionStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedQueueTrainingLaunch":
      return [
        Model.make({
          ...model,
          trainingLaunchPending: true,
          trainingLaunchStatus: {
            text: "queueing launch check...",
            tone: "info",
          },
        }),
        [QueueTrainingLaunch()],
      ]
    case "SettledQueueTrainingLaunch":
      return [
        Model.make({
          ...model,
          trainingLaunchPending: false,
          trainingLaunchStatus: {
            text: message.text,
            tone: message.ok ? "success" : "error",
          },
        }),
        message.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    case "ClickedQueueTrainingCloseout":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingCloseoutStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingCloseoutPending: true,
          trainingCloseoutStatus: {
            text: "queueing closeout packet...",
            tone: "info",
          },
        }),
        [
          QueueTrainingCloseout({
            trainingRunRef: message.trainingRunRef,
            windowRef: message.windowRef,
            leaseRef: message.leaseRef,
            bootstrapGrantRef: message.bootstrapGrantRef,
          }),
        ],
      ]
    case "SettledQueueTrainingCloseout":
      return [
        Model.make({
          ...model,
          trainingCloseoutPending: false,
          trainingCloseoutStatus: {
            text: message.text,
            tone: message.ok ? "success" : "error",
          },
        }),
        message.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]

    // ── Spawn ──────────────────────────────────────────────────────────────────
    case "ChangedSpawnAdapter":
      return [
        Model.make({
          ...model,
          spawnAdapter: message.adapter,
          composerAccountRef: null,
        }),
        noCommands,
      ]
    case "ChangedSpawnObjective":
      return [Model.make({ ...model, spawnObjective: message.value }), noCommands]
    case "ChangedSpawnVerify":
      return [Model.make({ ...model, spawnVerify: message.value }), noCommands]
    case "ChangedSpawnLane":
      return [Model.make({ ...model, spawnLane: message.lane }), noCommands]

    // ── Settings preferences (#5472) ─────────────────────────────────────────
    // Each handler (a) writes the preference field, (b) for theme/defaults makes
    // the choice take effect immediately, and (c) persists via PersistPreferences
    // (local, refs-only — no RPC). A `persist` helper keeps the four writes from
    // drifting on which fields get saved.
    case "ChangedThemePreference": {
      const next = Model.make({ ...model, themePreference: message.theme })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ChangedDefaultAdapter": {
      // Persist the default AND seed the live spawn adapter so the new default
      // takes effect now (spawn/composer/chat read `spawnAdapter` directly).
      const next = Model.make({
        ...model,
        defaultAdapter: message.adapter,
        spawnAdapter: message.adapter,
      })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ChangedDefaultLane": {
      const next = Model.make({
        ...model,
        defaultLane: message.lane,
        spawnLane: message.lane,
      })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ToggledNotificationPanel": {
      const next = Model.make({ ...model, showNotificationPanel: message.show })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ChangedGatewayInferenceFallback": {
      // #5485: persist the routing intent. It takes effect immediately because
      // the spawn paths read the live decision (decideInference) off the model.
      const next = Model.make({
        ...model,
        gatewayInferenceFallback: message.value,
      })
      return [next, [persistPreferencesFor(next)]]
    }
    case "SettledPersistPreferences":
      // No-op: the preference values already live in the Model; this only closes
      // the persistence command (side effect done in the command, not here).
      return [model, noCommands]
    case "ClickedSpawn": {
      const validation = validateSpawnRequest({
        adapter: model.spawnAdapter,
        objective: model.spawnObjective,
      })
      if (!validation.ok || validation.adapter === null) {
        return [
          Model.make({
            ...model,
            spawnStatus: {
              text: validation.errors[0] ?? "invalid request",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      return [
        Model.make({
          ...model,
          spawnPending: true,
          spawnStatus: { text: "sending…", tone: "info" },
        }),
        [
          SpawnSession({
            adapter: validation.adapter,
            objective: validation.objective,
            verify,
            lane: model.spawnLane,
            // CS-A1: the legacy Spawn pane has no per-account picker; the
            // composer pane is the account-aware spawn surface.
            accountRef: null,
          }),
        ],
      ]
    }
    case "SucceededSpawn":
      return [
        Model.make({
          ...model,
          spawnPending: false,
          spawnStatus: { text: "", tone: "idle" },
          spawnObjective: "",
          spawnVerify: "",
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedSpawn":
      return [
        Model.make({
          ...model,
          spawnPending: false,
          spawnStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── Session detail: cancel ──────────────────────────────────────────────────
    case "ClickedCancelSession":
      return [model, [CancelSession({ sessionRef: message.sessionRef })]]
    case "SettledCancelSession":
      // The next node-state poll carries the authoritative session state.
      return [model, noCommands]

    // ── CS-A2 (#5362): swarm quick action — open an existing session in the
    //    composer. Adopts the session as the composer's active thread (its
    //    repo/worktree + runtime) so the owner can reply/continue from the
    //    day-to-day coding surface. The live node poll then drives the
    //    transcript/approvals the composer renders — no new wire verb.
    case "ClickedOpenSessionInComposer":
      return [
        Model.make({
          ...model,
          pane: "composer",
          expandedEvents: [],
          selectedSessionRef: message.sessionRef,
          composerSessionRef: message.sessionRef,
          composerRepoPath: message.workspaceRef ?? "",
          composerReply: "",
          composerPending: false,
          composerStatus: {
            text: "adopted session — streaming transcript",
            tone: "info" as const,
          },
          spawnAdapter: message.adapter,
          // Load the managed-account registry so the picker is populated, the
          // same way NavigatedTo("composer") does.
          managedAccountsPending: true,
          managedAccountsStatus: {
            text: "loading accounts...",
            tone: "info" as const,
          },
        }),
        // #5485: + warm the gateway readiness so the composer's route hint is
        // accurate the moment an adopted session opens (matches NavigatedTo).
        [LoadManagedAccounts(), LoadInferenceGatewayReadiness()],
      ]

    // ── #5469 (EPIC #5461): swarm batch launch ──────────────────────────────────
    case "ChangedSwarmBatchObjectives":
      return [
        Model.make({ ...model, swarmBatchObjectives: message.value }),
        noCommands,
      ]
    case "ChangedSwarmBatchConcurrency":
      return [
        Model.make({ ...model, swarmBatchConcurrency: message.value }),
        noCommands,
      ]
    case "ClickedSwarmBatchLaunch": {
      // A batch already in flight must drain before a new one starts (the
      // bounded-concurrency guarantee). The button is disabled in the view too.
      if (model.swarmBatchActive > 0 || model.swarmBatchQueue.length > 0) {
        return [model, noCommands]
      }
      // Apple FM has no per-account/failover semantics and a different verb, so
      // batch launch is codex/claude only (matches SpawnBatchSession's adapter).
      const adapter = model.spawnAdapter === "apple_fm" ? "claude_agent" : model.spawnAdapter
      const accountBlocker = composerAccountBlocker(model, adapter)
      if (accountBlocker !== null) {
        return [
          Model.make({
            ...model,
            composerStatus: { text: accountBlocker, tone: "error" },
          }),
          noCommands,
        ]
      }
      const objectives = parseSwarmBatchObjectives(model.swarmBatchObjectives)
      if (objectives.length === 0) {
        return [model, noCommands]
      }
      const concurrency = clampSwarmBatchConcurrency(
        Number(model.swarmBatchConcurrency),
      )
      const verify = parseVerifyLines(model.spawnVerify)
      const { state, toDispatch } = startSwarmBatch(objectives, concurrency)
      return [
        writeSwarmBatchState(model, state),
        toDispatch.map((objective) =>
          SpawnBatchSession({
            adapter,
            objective,
            verify,
            lane: model.spawnLane,
            accountRef: composerAccountRefForAdapter(model, adapter),
          }),
        ),
      ]
    }
    case "SucceededSwarmBatchSpawn":
    case "FailedSwarmBatchSpawn": {
      const outcome =
        message._tag === "SucceededSwarmBatchSpawn" ? "launched" : "failed"
      const { state, next } = advanceSwarmBatch(readSwarmBatchState(model), outcome)
      const adapter = model.spawnAdapter === "apple_fm" ? "claude_agent" : model.spawnAdapter
      const accountBlocker = composerAccountBlocker(model, adapter)
      if (accountBlocker !== null) {
        return [
          Model.make({
            ...model,
            composerStatus: { text: accountBlocker, tone: "error" },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      return [
        writeSwarmBatchState(model, state),
        next === null
          ? noCommands
          : [
              SpawnBatchSession({
                adapter,
                objective: next,
                verify,
                lane: model.spawnLane,
                accountRef: composerAccountRefForAdapter(model, adapter),
              }),
            ],
      ]
    }

    // ── #5355: coding composer ──────────────────────────────────────────────────
    case "ChangedComposerRepoPath":
      return [Model.make({ ...model, composerRepoPath: message.value }), noCommands]
    case "ChangedComposerReply":
      return [Model.make({ ...model, composerReply: message.value }), noCommands]
    case "SelectedComposerAccount":
      return [
        withCodeModeSync(
          Model.make({ ...model, composerAccountRef: message.accountRef }),
          "model_tick",
        ),
        noCommands,
      ]
    case "ClickedOverrideComposerAccountRoute": {
      if (model.spawnAdapter === "apple_fm") return [model, noCommands]
      const override = nextCodeModeAccountOverride({
        adapter: model.spawnAdapter,
        selectedAccountRef: model.composerAccountRef,
        accounts: modelCodeModeSync(model)?.accounts ?? fallbackCodeModeAccountRows(model),
        sessions: modelCodeModeSync(model)?.sessions ?? modelNode(model)?.sessions ?? [],
        workspaceRef: composerRouteWorkspaceRef(model),
        allowDefaultHome: true,
      })
      if (override === null) {
        return [
          Model.make({
            ...model,
            composerStatus: {
              text: "no alternate account route is ready",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            composerAccountRef: override.accountRef,
            composerStatus: {
              text: `next run will use ${override.label}`,
              tone: "info",
            },
          }),
          "model_tick",
        ),
        noCommands,
      ]
    }
    // #5471: repo / worktree picker inputs.
    case "ChangedComposerWorkspaceMode":
      return [
        Model.make({ ...model, composerWorkspaceMode: message.mode }),
        noCommands,
      ]
    case "ChangedComposerManagedRepo":
      return [
        Model.make({ ...model, composerManagedRepo: message.value }),
        noCommands,
      ]
    case "ChangedComposerManagedBaseRef":
      return [
        Model.make({ ...model, composerManagedBaseRef: message.value }),
        noCommands,
      ]
    // #5471: a managed-worktree request resolved (or failed) node-side. On
    // success fire the deferred composer spawn with the resolved repoRef; on
    // failure settle the loop with the error so the pending state clears.
    case "ResolvedComposerManagedWorktree": {
      const result = message.result as
        | {
            ok?: unknown
            error?: unknown
            repoRef?: {
              provider?: unknown
              visibility?: unknown
              fullName?: unknown
              branch?: unknown
              commitSha?: unknown
            }
          }
        | null
        | undefined
      const objective = model.composerPendingObjective
      const repo = result?.repoRef
      const resolvedOk =
        result?.ok === true &&
        repo !== undefined &&
        repo.provider === "github" &&
        repo.visibility === "public" &&
        typeof repo.fullName === "string" &&
        typeof repo.branch === "string" &&
        typeof repo.commitSha === "string"
      if (!resolvedOk || objective === null || model.spawnAdapter === "apple_fm") {
        return [
          Model.make({
            ...model,
            composerPending: false,
            composerPendingObjective: null,
            composerStatus: {
              text:
                typeof result?.error === "string" && result.error.trim() !== ""
                  ? result.error
                  : "could not resolve managed worktree",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const accountBlocker = composerAccountBlocker(model, model.spawnAdapter)
      if (accountBlocker !== null) {
        return [
          Model.make({
            ...model,
            composerPending: false,
            composerPendingObjective: null,
            composerStatus: { text: accountBlocker, tone: "error" },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      return [
        Model.make({
          ...model,
          composerPendingObjective: null,
          composerStatus: {
            text: `starting coding session in ${repo.fullName as string} @ ${repo.branch as string}…`,
            tone: "info",
          },
        }),
        [
          SpawnComposerTurn({
            adapter: model.spawnAdapter,
            objective,
            verify,
            lane: model.spawnLane,
            worktreePath: null,
            repoRef: {
              provider: "github",
              visibility: "public",
              fullName: repo.fullName as string,
              branch: repo.branch as string,
              commitSha: repo.commitSha as string,
            },
            accountRef: composerAccountRefForAdapter(model, model.spawnAdapter),
          }),
        ],
      ]
    }
    case "ClickedComposerSpawn": {
      const worktreePath =
        model.composerRepoPath.trim() === "" ? null : model.composerRepoPath.trim()
      // CS-A1: Apple FM is a spawn-adapter option but uses its own control verb
      // (apple_fm.session.start), so it validates the objective directly and
      // routes through the Apple FM command rather than session.spawn.
      if (model.spawnAdapter === "apple_fm") {
        const objective = model.spawnObjective.trim()
        if (objective === "") {
          return [
            Model.make({
              ...model,
              composerStatus: { text: "objective must be non-empty", tone: "error" },
            }),
            noCommands,
          ]
        }
        return [
          Model.make({
            ...model,
            composerPending: true,
            composerStatus: { text: "starting local Apple FM session…", tone: "info" },
            composerTurns: [objective],
          }),
          [SpawnAppleFmComposerTurn({ objective, worktreePath })],
        ]
      }
      // First coding turn — reuse the shared spawn validation/fields.
      const validation = validateSpawnRequest({
        adapter: model.spawnAdapter,
        objective: model.spawnObjective,
      })
      if (!validation.ok || validation.adapter === null) {
        return [
          Model.make({
            ...model,
            composerStatus: {
              text: validation.errors[0] ?? "invalid request",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const accountBlocker = composerAccountBlocker(model, validation.adapter)
      if (accountBlocker !== null) {
        return [
          Model.make({
            ...model,
            composerStatus: { text: accountBlocker, tone: "error" },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      const objective = validation.objective
      // #5471: managed-worktree mode resolves a repoRef node-side first, then
      // spawns (see ResolvedComposerManagedWorktree). The path mode spawns
      // directly with worktreePath as before.
      if (model.composerWorkspaceMode === "managed") {
        const parsed = parseManagedWorktreeRequest({
          repo: model.composerManagedRepo,
          baseRef: model.composerManagedBaseRef,
        })
        if (!parsed.ok) {
          return [
            Model.make({
              ...model,
              composerStatus: { text: parsed.error, tone: "error" },
            }),
            noCommands,
          ]
        }
        return startManagedComposerTurn(model, parsed.request, objective, [objective])
      }
      return [
        Model.make({
          ...model,
          composerPending: true,
          composerStatus: { text: "starting coding session…", tone: "info" },
          composerTurns: [objective],
        }),
        [
          SpawnComposerTurn({
            adapter: validation.adapter,
            objective,
            verify,
            lane: model.spawnLane,
            worktreePath,
            repoRef: null,
            accountRef: composerAccountRefForAdapter(model, validation.adapter),
          }),
        ],
      ]
    }
    case "ClickedComposerReply": {
      // Follow-up turn — a continuation session.spawn carrying the prior turns.
      const followUp = model.composerReply.trim()
      if (followUp === "") {
        return [
          Model.make({
            ...model,
            composerStatus: { text: "type a follow-up first", tone: "error" },
          }),
          noCommands,
        ]
      }
      const objective = buildComposerContinuationObjective(
        model.composerTurns,
        followUp,
      )
      const verify = parseVerifyLines(model.spawnVerify)
      const worktreePath =
        model.composerRepoPath.trim() === "" ? null : model.composerRepoPath.trim()
      const continuing = Model.make({
        ...model,
        composerPending: true,
        composerReply: "",
        composerStatus: { text: "continuing the thread…", tone: "info" },
        composerTurns: [...model.composerTurns, followUp],
      })
      // CS-A1: continuation turns honor the selected runtime, including Apple FM.
      if (model.spawnAdapter === "apple_fm") {
        return [continuing, [SpawnAppleFmComposerTurn({ objective, worktreePath })]]
      }
      const accountBlocker = composerAccountBlocker(model, model.spawnAdapter)
      if (accountBlocker !== null) {
        return [
          Model.make({
            ...model,
            composerStatus: { text: accountBlocker, tone: "error" },
          }),
          noCommands,
        ]
      }
      // #5471: a managed-worktree thread keeps materializing its repoRef per
      // turn (each control session is its own bounded checkout). Resolve first,
      // then spawn the continuation.
      if (model.composerWorkspaceMode === "managed") {
        const parsed = parseManagedWorktreeRequest({
          repo: model.composerManagedRepo,
          baseRef: model.composerManagedBaseRef,
        })
        if (!parsed.ok) {
          return [
            Model.make({
              ...model,
              composerStatus: { text: parsed.error, tone: "error" },
            }),
            noCommands,
          ]
        }
        return startManagedComposerTurn(continuing, parsed.request, objective, [
          ...model.composerTurns,
          followUp,
        ])
      }
      return [
        continuing,
        [
          SpawnComposerTurn({
            adapter: model.spawnAdapter,
            objective,
            verify,
            lane: model.spawnLane,
            worktreePath,
            repoRef: null,
            accountRef: composerAccountRefForAdapter(model, model.spawnAdapter),
          }),
        ],
      ]
    }
    case "ClickedComposerNewThread":
      return [
        Model.make({
          ...model,
          composerSessionRef: null,
          composerReply: "",
          composerTurns: [],
          composerStatus: { text: "", tone: "idle" },
          composerPending: false,
          composerPendingObjective: null,
          spawnObjective: "",
        }),
        // CS-A1: a fresh thread reloads the account list so a newly added
        // account is pickable without re-navigating.
        [LoadManagedAccounts()],
      ]
    case "SucceededComposerTurn":
      return [
        Model.make({
          ...model,
          composerPending: false,
          composerSessionRef: message.sessionRef,
          selectedSessionRef: message.sessionRef,
          composerStatus: { text: "running — streaming transcript", tone: "success" },
          // First turn clears the objective box so the form is reply-ready.
          spawnObjective: "",
        }),
        noCommands,
      ]
    case "FailedComposerTurn":
      return [
        Model.make({
          ...model,
          composerPending: false,
          composerPendingObjective: null,
          composerStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── #5821: Verse/Tassadar chat, plus explicit Blueprint program chat ──────
    case "ChangedChatInput":
      return [Model.make({ ...model, chatInput: message.value }), noCommands]
    case "ClickedChatSubmit": {
      const prompt = model.chatInput.trim()
      if (prompt === "") {
        return [
          Model.make({
            ...model,
            chatStatus: { text: "message must be non-empty", tone: "error" },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          chatInput: "",
          chatPending: true,
          chatStatus: {
            text: "asking Tassadar with public Pylon and training context...",
            tone: "info",
          },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.user"),
              role: "user",
              body: prompt,
              timestamp: chatTimestamp(),
              linkedSessionRef: null,
              steps: [],
            },
          ],
        }),
        [RespondToVerseInput({ prompt })],
      ]
    }
    case "ClickedBlueprintChatSubmit": {
      const prompt = model.chatInput.trim()
      if (prompt === "") {
        return [
          Model.make({
            ...model,
            chatStatus: { text: "message must be non-empty", tone: "error" },
          }),
          noCommands,
        ]
      }
      const adapter =
        model.spawnAdapter === "claude_agent" ? "claude_agent" : "codex"
      // #5466: SEMANTIC signature selection from the free-text turn (no keyword
      // matching). The selected signature ref is embedded in the objective the
      // bounded session runs. The user message renders the chosen signature; its
      // exact-replay verdict stays `pending` (spawning) — nothing is verified yet.
      const selection = selectSignatureForMessage(prompt)
      const objective = buildBlueprintChatObjective(
        prompt,
        selection.signatureRef,
      )
      return [
        Model.make({
          ...model,
          chatInput: "",
          chatPending: true,
          chatStatus: {
            text: selection.confident
              ? `routing → ${selection.family} signature…`
              : "routing → continuation signature (default)…",
            tone: "info",
          },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.user"),
              role: "user",
              body: prompt,
              timestamp: chatTimestamp(),
              linkedSessionRef: null,
              steps: liveChatScopedSteps({
                selection,
                linkedSessionRef: null,
                events: [],
                proofReplaySlug: DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
              }),
            },
          ],
        }),
        [
          SpawnChatTurn({
            adapter,
            objective,
            verify: [],
            lane: model.spawnLane,
            accountRef: composerAccountRefForAdapter(model, adapter),
          }),
        ],
      ]
    }
    case "SucceededVerseTurn":
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatStatus: {
            text: message.ok
              ? "Tassadar answered from public Verse context"
              : message.blockerRefs.length > 0
                ? `Verse blocker: ${message.blockerRefs[0]}`
                : "Tassadar could not answer yet",
            tone: message.ok ? "success" : "error",
          },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.assistant"),
              role: "assistant",
              body: message.text,
              timestamp: chatTimestamp(),
              linkedSessionRef: null,
              // #5821: no Blueprint/exact-replay step chrome on the default
              // Verse path. Program steps appear only when the explicit
              // Blueprint command is invoked.
              steps: [],
            },
          ],
        }),
        noCommands,
      ]
    case "FailedVerseTurn":
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatStatus: { text: message.error, tone: "error" },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.assistant"),
              role: "assistant",
              body: message.error,
              timestamp: chatTimestamp(),
              linkedSessionRef: null,
              steps: [],
            },
          ],
        }),
        noCommands,
      ]
    case "SucceededChatTurn": {
      // #5466: the assistant message links to the live session. Its program
      // steps start in a queued/running state and are reconciled to real
      // verdicts by the node-state poll (reconcileChatTurn). The signature is
      // re-derived from the triggering user turn so it matches the route taken.
      const lastUserPrompt = [...model.chatMessages]
        .reverse()
        .find((m) => m.role === "user")?.body ?? ""
      const assistantSelection = selectSignatureForMessage(lastUserPrompt)
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatSessionRef: message.sessionRef,
          chatStatus: {
            text: "running — node-state poll streams the live verdict",
            tone: "success",
          },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.assistant"),
              role: "assistant",
              body: "Blueprint program turn running.",
              timestamp: chatTimestamp(),
              linkedSessionRef: message.sessionRef,
              steps: liveChatScopedSteps({
                selection: assistantSelection,
                linkedSessionRef: message.sessionRef,
                events: [],
                proofReplaySlug: DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
              }),
            },
          ],
        }),
        noCommands,
      ]
    }
    case "FailedChatTurn":
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── Zero-base shell (owner directive, 2026-06-19) ───────────────────────
    // The minimal default surface. The whole loop is a pure reducer + one
    // loopback command, so a programmatic driver (Claude) can set the input,
    // submit, and read the rendered conversation, seeing exactly what the owner
    // sees (model.shellTurns → shellTranscriptText).
    case "ChangedShellInput":
      return [Model.make({ ...model, shellInput: message.value }), noCommands]
    case "CycledShellTarget":
      return [
        Model.make({ ...model, shellTarget: nextShellTarget(model.shellTarget) }),
        noCommands,
      ]
    case "SelectedShellTarget":
      return [Model.make({ ...model, shellTarget: message.target }), noCommands]
    case "SubmittedShell": {
      const prompt = model.shellInput.trim()
      // Empty submit is a no-op (no error chrome on the clean surface).
      if (prompt === "" || model.shellPending) return [model, noCommands]
      const target = model.shellTarget
      const userTurn = {
        id: chatMessageId("shell.you"),
        role: "you" as const,
        target,
        sessionRef: null,
        text: prompt,
      }
      if (target !== "current") {
        const state = shellCodingState(model, target)
        const objective =
          state.turns.length === 0
            ? prompt
            : buildComposerContinuationObjective(state.turns, prompt)
        const worktreePath =
          model.composerRepoPath.trim() === "" ? null : model.composerRepoPath.trim()
        return [
          Model.make({
            ...model,
            shellInput: "",
            shellPending: true,
            shellTurns: [...model.shellTurns, userTurn],
          }),
          [
            SpawnShellCodingTurn({
              target,
              adapter: shellCodingAdapter(target),
              prompt,
              objective,
              verify: parseVerifyLines(model.spawnVerify),
              lane: model.spawnLane,
              worktreePath,
              useDefaultWorktree: worktreePath === null,
              accountRef: composerAccountRefForAdapter(model, shellCodingAdapter(target)),
            }),
          ],
        ]
      }
      return [
        Model.make({
          ...model,
          shellInput: "",
          shellPending: true,
          shellTurns: [...model.shellTurns, userTurn],
        }),
        [RespondToShellInput({ prompt })],
      ]
    }
    case "RespondedShell":
      return [
        Model.make({
          ...model,
          shellPending: false,
          shellTurns: [
            ...model.shellTurns,
            {
              id: chatMessageId("shell.autopilot"),
              role: "autopilot",
              target: "current",
              sessionRef: null,
              text: message.text,
            },
          ],
        }),
        noCommands,
      ]
    case "SucceededShellCodingTurn": {
      const state = shellCodingState(model, message.target)
      const label = shellTargetLabel(message.target)
      const verb = state.sessionRef === null ? "started" : "continued"
      const next = writeShellCodingSuccess(
        model,
        message.target,
        message.prompt,
        message.sessionRef,
      )
      return [
        Model.make({
          ...next,
          shellTurns: [
            ...next.shellTurns,
            {
              id: chatMessageId("shell.autopilot"),
              role: "autopilot",
              target: message.target,
              sessionRef: message.sessionRef,
              text: `${label} ${verb}: ${message.sessionRef}`,
            },
          ],
        }),
        noCommands,
      ]
    }
    case "FailedShellCodingTurn":
      return [
        Model.make({
          ...model,
          shellPending: false,
          shellTurns: [
            ...model.shellTurns,
            {
              id: chatMessageId("shell.autopilot"),
              role: "autopilot",
              target: message.target,
              sessionRef: null,
              text: `${shellTargetLabel(message.target)} failed: ${message.error}`,
            },
          ],
        }),
        noCommands,
      ]
    // ── EPIC #6017: talk to Khala from an in-world Verse textbox ────────────
    case "ChangedVerseKhalaInput":
      return [
        Model.make({ ...model, verseKhalaInput: message.value }),
        noCommands,
      ]
    case "SubmittedVerseKhala": {
      const prompt = model.verseKhalaInput.trim()
      // Empty submit / a turn already in flight is a no-op (no error chrome).
      if (prompt === "" || model.verseKhalaInFlight) return [model, noCommands]
      const turnId = chatMessageId("verse.khala")
      return [
        Model.make({
          ...model,
          verseKhalaInput: "",
          verseKhalaInFlight: true,
          verseKhalaTurnId: turnId,
          // Reset the live response bubble + the prior effect; a new turn's
          // effect only fires on its OWN real receipt (evidence-bound).
          verseKhalaResponse: "",
          verseKhalaReceipt: null,
          verseKhalaStatus: { text: "asking Khala…", tone: "info" },
        }),
        [RunVerseKhalaTurn({ prompt, turnId })],
      ]
    }
    case "GotVerseKhalaToken": {
      // Append the live delta ONLY for the active, in-flight turn.
      //
      // (1) turnId must match the active turn — a stale/concurrent turn's deltas
      //     never cross-render into the current bubble.
      // (2) the turn must still be IN FLIGHT. The terminal `RespondedVerseKhala`
      //     and the streamed `khalaToken` deltas race over the bridge: when the
      //     terminal answer lands FIRST (the RPC promise resolves before the
      //     trailing SSE frames push), `RespondedVerseKhala` already set the full
      //     answer text and flipped `verseKhalaInFlight` false. Without this guard,
      //     the late deltas then APPEND the same content again, rendering the
      //     answer TWICE (the double-response bug). Dropping post-settle deltas
      //     makes the response appear exactly once regardless of arrival order.
      if (
        message.turnId !== model.verseKhalaTurnId ||
        !model.verseKhalaInFlight
      ) {
        return [model, noCommands]
      }
      return [
        Model.make({
          ...model,
          verseKhalaResponse: model.verseKhalaResponse + message.delta,
        }),
        noCommands,
      ]
    }
    case "RespondedVerseKhala": {
      // Ignore a terminal result for a turn that is no longer the active one.
      if (message.turnId !== model.verseKhalaTurnId) return [model, noCommands]
      const receipt = verseKhalaReceiptFromUnknown(message.receipt)
      return [
        Model.make({
          ...model,
          verseKhalaInFlight: false,
          // The streamed bubble already holds the live text; fall back to the
          // terminal text when no deltas streamed (non-streaming route / error).
          verseKhalaResponse:
            model.verseKhalaResponse.trim().length > 0
              ? model.verseKhalaResponse
              : message.text,
          // EVIDENCE GATE: keep the receipt only when it carries a real ref so
          // the LOCAL crackling effect fires for genuine turns only.
          verseKhalaReceipt:
            receipt !== null && receipt.receipt !== null ? receipt : null,
          verseKhalaStatus: {
            text: message.live
              ? "Khala answered — verified receipt"
              : message.ok
                ? "Khala answered (no receipt — unverified)"
                : message.text,
            tone: message.live ? "success" : message.ok ? "info" : "error",
          },
        }),
        noCommands,
      ]
    }
    case "FailedVerseKhala": {
      if (message.turnId !== model.verseKhalaTurnId) return [model, noCommands]
      return [
        Model.make({
          ...model,
          verseKhalaInFlight: false,
          verseKhalaReceipt: null,
          verseKhalaResponse:
            model.verseKhalaResponse.trim().length > 0
              ? model.verseKhalaResponse
              : message.error,
          verseKhalaStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]
    }
    // The explicit open: reveal the KEPT full multi-pane UI behind the advanced
    // Code group. The Verse chat pane is now immersive by default (#5820), so
    // this lands on Composer to make the sidebar/code tools explicit.
    case "OpenedPanes":
      return update(model, NavigatedTo({ pane: "composer" }))
    // Return to the black shell. Pure pane switch; the panes stay mounted and
    // reopenable. Closes the palette if it was open.
    case "ClosedPanes":
      return [
        Model.make({ ...model, pane: "shell", commandPaletteOpen: false }),
        noCommands,
      ]

    // ── HUD H3: the managed pane layer (#5501) ──────────────────────────────
    // Each verb maps to ONE PaneLayerAction; the pure PaneManager reducer
    // (pane-manager.ts) is the only thing that mutates the layer (open/close/
    // focus/move/resize + cascade/clamp). The result is stored back on the
    // opaque `paneLayer` field. No new control/RPC verb, no other Model state
    // touched — the managed panes float OVER the current base surface, so the
    // shell + single-pane router never regress.
    case "OpenedManagedPane":
      if (verseControlsDisabled(model)) return [model, noCommands]
      {
        const [opened, commands] = applyPaneLayerAction(model, {
          kind: "open",
          pane: message.pane,
        })
        const accountCommands = isAccountManagingPane(message.pane)
          ? [LoadManagedAccounts(), LoadInferenceGatewayReadiness()]
          : noCommands
        const diagnosticCommands = isDiagnosticsPane(message.pane)
          ? diagnosticsRefreshCommands()
          : noCommands
        return [opened, [...commands, ...accountCommands, ...diagnosticCommands]]
      }
    case "ClosedManagedPane":
      return applyPaneLayerAction(model, { kind: "close", paneId: message.paneId })
    case "FocusedManagedPane":
      return applyPaneLayerAction(model, { kind: "focus", paneId: message.paneId })
    case "ClosedAllManagedPanes":
      return applyPaneLayerAction(model, { kind: "close-all" })
    case "StartedPaneDrag":
      return applyPaneLayerAction(model, {
        kind: "drag-start",
        paneId: message.paneId,
        drag: message.drag,
        handle: message.handle,
        pointerX: message.pointerX,
        pointerY: message.pointerY,
      })
    case "MovedPaneDragPointer":
      // The window pointermove subscription fires continuously; the reducer
      // no-ops when no drag is in flight, so this is cheap when idle.
      return applyPaneLayerAction(model, {
        kind: "drag-move",
        pointerX: message.pointerX,
        pointerY: message.pointerY,
      })
    case "EndedPaneDrag":
      return applyPaneLayerAction(model, { kind: "drag-end" })

    // ── CS-A1: account management (node-local dev.accounts) ─────────────────────
    case "ClickedRefreshManagedAccounts":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "loading accounts...", tone: "info" },
        }),
        [LoadManagedAccounts()],
      ]
    case "GotManagedAccounts": {
      const projection = message.projection as {
        ok: boolean
        accounts: ReadonlyArray<unknown>
        error?: string
      }
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            managedAccounts: message.projection,
            managedAccountsPending: false,
            managedAccountsStatus: projection.ok
              ? { text: "", tone: "idle" }
              : { text: projection.error ?? "could not load accounts", tone: "error" },
          }),
          "managed_accounts",
        ),
        noCommands,
      ]
    }
    case "ChangedAddAccountRef":
      return [Model.make({ ...model, addAccountRef: message.value }), noCommands]
    case "ChangedAddAccountProvider":
      return [Model.make({ ...model, addAccountProvider: message.provider }), noCommands]
    case "ChangedAddAccountHome":
      return [Model.make({ ...model, addAccountHome: message.value }), noCommands]
    case "ChangedAddAccountPriority":
      return [Model.make({ ...model, addAccountPriority: message.value }), noCommands]
    case "ClickedAddManagedAccount": {
      const ref = model.addAccountRef.trim()
      const home = model.addAccountHome.trim()
      if (ref === "" || home === "") {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: { text: "ref and home are required", tone: "error" },
          }),
          noCommands,
        ]
      }
      if (!managedAccountRefPattern.test(ref)) {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: {
              text: "account ref is invalid (letters, digits, . _ - ; max 80)",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const managed = modelManagedAccounts(model)
      const duplicate = managed?.accounts.some(
        (row) => row.provider === model.addAccountProvider && row.ref === ref,
      )
      if (duplicate) {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: {
              text: `account ref already exists for ${model.addAccountProvider}`,
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const priorityRaw = model.addAccountPriority.trim()
      const priorityParsed = priorityRaw === "" ? null : Number(priorityRaw)
      if (priorityParsed !== null && !Number.isFinite(priorityParsed)) {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: { text: "priority must be a number", tone: "error" },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "adding account...", tone: "info" },
        }),
        [
          AddManagedAccount({
            ref,
            provider: model.addAccountProvider,
            home,
            priority: priorityParsed,
          }),
        ],
      ]
    }
    case "ClickedRemoveManagedAccount":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "removing account...", tone: "info" },
        }),
        [RemoveManagedAccount({ ref: message.ref, provider: message.provider })],
      ]
    case "ClickedBumpManagedAccountPriority":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "updating priority...", tone: "info" },
        }),
        [
          SetManagedAccountPriority({
            ref: message.ref,
            provider: message.provider,
            priority: message.priority,
          }),
        ],
      ]
    case "SettledManagedAccountMutation": {
      const projection = message.projection as {
        ok: boolean
        accounts: ReadonlyArray<unknown>
        error?: string
      }
      // A successful mutation returns the refreshed list; clear the add-form on
      // success so the surface is ready for the next entry.
      return [
        withCodeModeSync(
          Model.make({
            ...model,
            managedAccounts: message.projection,
            managedAccountsPending: false,
            managedAccountsStatus: projection.ok
              ? { text: "saved", tone: "success" }
              : { text: projection.error ?? "account update failed", tone: "error" },
            ...(projection.ok
              ? { addAccountRef: "", addAccountHome: "", addAccountPriority: "" }
              : {}),
          }),
          "account_mutation",
        ),
        noCommands,
      ]
    }
  }
}
