import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { BrowserView, BrowserWindow, PATHS, Updater } from "electrobun/bun"
import { discoverPylonHome } from "./node-home"
import {
  type NodeLaunchStatus,
  type SupervisedNode,
  superviseManagedNode,
} from "./node-launcher"
import { createNodeStatePoller } from "./node-state-poll"
import { persistAndMergeTranscripts } from "./transcript-store"
import { loadPersistedCredential } from "./agent-onboarding"
import {
  detectExistingPylonIdentity,
  detectedIdentityShortLabel,
  loadIdentityChoice,
  projectIdentityChoiceState,
  saveIdentityChoice,
} from "./identity-choice"
import { projectOnboardingStatus } from "../shared/onboarding-status"
import {
  autoUpdateDisabledReason,
  autoUpdateIntervalMs,
  runAutoUpdateOnce,
} from "./auto-update"
import { fetchPublicPylonStats } from "./pylon-network-stats"
import { fetchPublicActivityTimeline } from "./public-activity-timeline"
import {
  promiseSurfacingReadiness as buildPromiseSurfacingReadiness,
  resolvePromiseSurfacingSettings,
  surfacePromiseGapReport,
} from "./promise-surfacing"
import { createSessionNotifier } from "./notifier"
import { raiseOsNotification } from "./os-notification"
import {
  builtInAgentObjective,
  resolveBuiltInAgentSettings,
} from "../shared/builtin-agent"
import { projectInstallReadiness } from "../shared/install-readiness"
import {
  addManagedAccount,
  listManagedAccounts,
  removeManagedAccount,
  setManagedAccountPriority,
} from "./account-management"
import {
  cancelSession,
  deployToCloud,
  fetchAppleFmReadiness,
  fetchNodeState,
  fetchOnboardingSignals,
  readControlToken,
  resolveApproval,
  setCoordinatorPaused,
  startAppleFmSession as startAppleFmControlSession,
  spawnSession,
  submitIntent,
} from "./pylon-control"
import {
  activateTrainingWindow,
  admitTrainingRealGradientEvidence,
  buildTrainingEvidencePacket,
  claimTrainingWindowLease,
  fetchTrainingDashboard,
  fetchTrainingPromiseGates,
  fetchTrainingRuns,
  planTrainingRunWindow,
  readTrainingEvidencePacketSummary,
  reconcileTrainingWindow,
  requestTrainingBootstrapGrant,
} from "./training-runs"
import type {
  AppleFmReadinessResponse,
  AppleFmSessionStartResponse,
  BuiltInAgentReadinessResponse,
  BuiltInAgentStartResponse,
  ChooseIdentityParams,
  ChooseIdentityResponse,
  DesktopRPCSchema,
  IdentityChoiceStateResponse,
  InstallReadinessResponse,
  OnboardingStatusResponse,
  TrainingOperatorReadinessPylonRefSource,
  TrainingOperatorReadinessResponse,
} from "../shared/rpc"

const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pollIntervalMs = Number(Bun.env.AUTOPILOT_DESKTOP_NODE_POLL_MS ?? "2000")
const trainingBaseUrl =
  Bun.env.OPENAGENTS_TRAINING_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  "https://openagents.com"
const activityBaseUrl =
  Bun.env.OPENAGENTS_ACTIVITY_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  "https://openagents.com"
const trainingAdminToken =
  Bun.env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN ??
  Bun.env.OPENAGENTS_ADMIN_API_TOKEN ??
  null
const trainingAdminEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE === "1"
const trainingLeaseEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE === "1"
const trainingEvidenceEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE === "1"
const trainingEvidenceWriteEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE === "1"
const trainingEvidencePacketPath =
  Bun.env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH ?? null
const configuredTrainingWorkerReceiptsPath =
  Bun.env.OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH ?? null
const configuredTrainingPylonRef =
  Bun.env.OPENAGENTS_TRAINING_PYLON_REF ??
  Bun.env.PYLON_REF ??
  null
const trainingWorkerReceiptsFilename = "training-worker-receipts.json"
const builtInAgentWorktreeDirname = "builtin-agent-workspace"
const builtInAgentUsageFilename = "builtin-agent-usage.json"
const localAppleFmPrompt =
  "Run entirely locally through Apple Foundation Models. Use list_files on '.', then read_file on README.md if it exists, and answer with a concise summary of the local workspace. Refuse shell, write, network, deployment, and out-of-workspace requests."

let managedNode: SupervisedNode | null = null

// CL-45: resolve the node home once per call so a node that starts after the app
// (or rotates its home) is picked up without a restart. Falls back to the env
// default. `readControlToken` returns null when the chosen home has no token,
// which the poll surfaces as an honest offline state.
function resolveHome(): string | null {
  return discoverPylonHome({ env: Bun.env.PYLON_HOME, cwd: process.cwd() })
}

function tokenForCommand(): string | null {
  const home = resolveHome()
  return home === null ? null : readControlToken(home)
}

function readIdentityPylonRef(home: string): string | null {
  const path = join(home, "identity.json")
  if (!existsSync(path)) return null
  try {
    const record = JSON.parse(readFileSync(path, "utf8")) as {
      pylonRef?: unknown
    }
    return typeof record.pylonRef === "string" && record.pylonRef.length > 0
      ? record.pylonRef
      : null
  } catch {
    return null
  }
}

function trainingPylonRefStatus(home = resolveHome()): {
  readonly pylonRef: string | null
  readonly source: TrainingOperatorReadinessPylonRefSource
} {
  const configured = configuredTrainingPylonRef?.trim() ?? ""
  if (configured.length > 0) {
    return { pylonRef: configured, source: "env" }
  }
  if (home === null) {
    return { pylonRef: null, source: "missing" }
  }
  const identityRef = readIdentityPylonRef(home)
  return identityRef === null
    ? { pylonRef: null, source: "missing" }
    : { pylonRef: identityRef, source: "identity" }
}

function trainingPylonRefForCommand(): string | null {
  return trainingPylonRefStatus().pylonRef
}

function trainingWorkerReceiptsPathForCommand(home = resolveHome()): string | null {
  const configured = configuredTrainingWorkerReceiptsPath?.trim() ?? ""
  if (configured.length > 0) return configured
  return home === null ? null : join(home, trainingWorkerReceiptsFilename)
}

function trainingOperatorReadinessProjection(): TrainingOperatorReadinessResponse {
  const home = resolveHome()
  const controlToken = home === null ? null : readControlToken(home)
  const pylon = trainingPylonRefStatus(home)
  const adminTokenPresent = (trainingAdminToken?.trim() ?? "").length > 0
  const adminReady = trainingAdminEnabled && adminTokenPresent
  const leaseReady = trainingLeaseEnabled && pylon.pylonRef !== null
  const evidencePacketPathPresent =
    (trainingEvidencePacketPath?.trim() ?? "").length > 0
  const evidenceReady =
    trainingEvidenceEnabled && adminTokenPresent && evidencePacketPathPresent
  const localPylonReady = home !== null && controlToken !== null
  const blockerRefs: string[] = []

  if (!trainingAdminEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE")
  }
  if (!adminTokenPresent) {
    blockerRefs.push("env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN")
  }
  if (!trainingLeaseEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE")
  }
  if (pylon.pylonRef === null) {
    blockerRefs.push("pylon.identity.pylonRef")
  }
  if (home === null) {
    blockerRefs.push("pylon.home")
  }
  if (controlToken === null) {
    blockerRefs.push("pylon.control_token")
  }
  if (!trainingEvidenceEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE")
  }
  if (!evidencePacketPathPresent) {
    blockerRefs.push("env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH")
  }

  return {
    ok: blockerRefs.length === 0,
    fetchedAt: new Date().toISOString(),
    sourceUrl: "desktop:training-operator-readiness",
    trainingBaseUrl,
    adminEnabled: trainingAdminEnabled,
    adminTokenPresent,
    adminReady,
    leaseEnabled: trainingLeaseEnabled,
    leaseReady,
    pylonRefPresent: pylon.pylonRef !== null,
    pylonRefSource: pylon.source,
    pylonRef: pylon.pylonRef,
    pylonHomePresent: home !== null,
    controlTokenPresent: controlToken !== null,
    localPylonReady,
    evidenceEnabled: trainingEvidenceEnabled,
    evidencePacketPathPresent,
    evidenceReady,
    blockerRefs,
  }
}

function ensureDirectory(path: string): boolean {
  try {
    mkdirSync(path, { recursive: true })
    return true
  } catch {
    return false
  }
}

function builtInAgentWorktreePath(home = resolveHome()): string | null {
  const configured = Bun.env.OPENAGENTS_BUILTIN_AGENT_WORKTREE?.trim() ?? ""
  if (configured.length > 0) {
    return ensureDirectory(configured) ? configured : null
  }
  if (home === null) return null
  const path = join(home, "workrooms", builtInAgentWorktreeDirname)
  return ensureDirectory(path) ? path : null
}

function builtInAgentUsagePath(home: string): string {
  return join(home, builtInAgentUsageFilename)
}

function todayKey(): string {
  return new Date().toISOString().slice(0, 10)
}

function builtInAgentDailySessionsUsed(home: string | null): number {
  if (home === null) return 0
  try {
    const record = JSON.parse(readFileSync(builtInAgentUsagePath(home), "utf8")) as {
      date?: unknown
      count?: unknown
    }
    return record.date === todayKey() && typeof record.count === "number"
      ? Math.max(0, Math.floor(record.count))
      : 0
  } catch {
    return 0
  }
}

function recordBuiltInAgentStart(home: string): void {
  const count = builtInAgentDailySessionsUsed(home) + 1
  writeFileSync(
    builtInAgentUsagePath(home),
    `${JSON.stringify({ date: todayKey(), count }, null, 2)}\n`,
    "utf8",
  )
}

function builtInAgentReadinessProjection(): BuiltInAgentReadinessResponse {
  const home = resolveHome()
  const controlToken = home === null ? null : readControlToken(home)
  const localPylonReady = home !== null && controlToken !== null
  const worktreePath = builtInAgentWorktreePath(home)
  const settings = resolveBuiltInAgentSettings(Bun.env)
  const dailySessionsUsed = builtInAgentDailySessionsUsed(home)
  const blockerRefs: string[] = []

  if (!settings.enabled) {
    blockerRefs.push("blocker.autopilot.builtin_agent.disabled")
  }
  if (!localPylonReady) {
    blockerRefs.push("blocker.autopilot.builtin_agent.local_pylon_offline")
  }
  if (!settings.hostedComputeConfigured) {
    blockerRefs.push("blocker.autopilot.builtin_agent.hosted_compute_unconfigured")
  }
  if (worktreePath === null) {
    blockerRefs.push("blocker.autopilot.builtin_agent.worktree_unavailable")
  }
  if (dailySessionsUsed >= settings.dailySessionCap) {
    blockerRefs.push("blocker.autopilot.builtin_agent.daily_cap_reached")
  }

  return {
    ok: blockerRefs.length === 0,
    fetchedAt: new Date().toISOString(),
    sourceUrl: "desktop:builtin-agent-readiness",
    enabled: settings.enabled,
    localPylonReady,
    hostedComputeConfigured: settings.hostedComputeConfigured,
    userApiKeyRequired: false,
    lane: settings.lane,
    modelSet: settings.modelSet,
    maxSessionSeconds: settings.maxSessionSeconds,
    dailySessionCap: settings.dailySessionCap,
    dailySessionsUsed,
    meteringLabel: settings.meteringLabel,
    worktreePathPresent: worktreePath !== null,
    blockerRefs,
  }
}

async function appleFmReadinessProjection(): Promise<AppleFmReadinessResponse> {
  const home = resolveHome()
  const controlToken = home === null ? null : readControlToken(home)
  if (home === null || controlToken === null) {
    return {
      ok: false,
      fetchedAt: new Date().toISOString(),
      sourceUrl: "desktop:apple-fm-readiness",
      localPylonReady: false,
      available: false,
      status: "unreachable",
      backendKind: "apple_fm_bridge",
      profileId: "apple-fm-local",
      model: "apple-foundation-model",
      capability: "probe.backend.apple_fm_bridge",
      advertisedCapabilities: [],
      baseUrl: "http://127.0.0.1:11435",
      platform: null,
      version: null,
      unavailableReason: "bridge_unreachable",
      message: "Waiting for local Pylon control before checking Apple FM.",
      blockerRefs: ["blocker.autopilot.apple_fm.local_pylon_offline"],
    }
  }

  return fetchAppleFmReadiness({
    baseUrl: controlBaseUrl,
    token: controlToken,
  })
}

function runtimeKind(): "source" | "packaged" {
  return PATHS.RESOURCES_FOLDER.includes(".app/Contents/Resources")
    ? "packaged"
    : "source"
}

async function installReadinessProjection(): Promise<InstallReadinessResponse> {
  const home = resolveHome()
  const controlToken = home === null ? null : readControlToken(home)
  return projectInstallReadiness({
    fetchedAt: new Date().toISOString(),
    platform: process.platform,
    arch: process.arch,
    runtime: runtimeKind(),
    nodeLaunchStatus: managedNode?.status() ?? null,
    pylonHomePresent: home !== null,
    controlTokenPresent: controlToken !== null,
    builtInAgentReadiness: builtInAgentReadinessProjection(),
    appleFmReadiness: await appleFmReadinessProjection(),
    autoUpdateDisabledReason: autoUpdateDisabledReason(Bun.env),
  })
}

// AO-3 (#5444): the home the onboarding credential lives in. Prefer the
// supervisor's launched/adopted home (the managed home or a chosen existing
// home), falling back to discovery. Used to read the persisted agent credential
// (presence) and to build the onboarding status.
function onboardingHome(): string | null {
  return managedNode?.home() ?? resolveHome()
}

// AO-3 (#5444): public-safe first-run identity-choice projection.
function identityChoiceProjection(): IdentityChoiceStateResponse {
  return projectIdentityChoiceState()
}

// AO-4 (#5445): assemble the live onboarding chain status from REAL state — the
// persisted identity choice (AO-3), the persisted agent credential (AO-1), the
// honest node launch status, and a fail-soft read-only wallet/assignment poll
// (CL-49/CL-50). The agent token never crosses into the projection (only the
// boolean "registered"). Public-safe display only.
async function onboardingStatusProjection(): Promise<OnboardingStatusResponse> {
  const home = onboardingHome()
  const controlToken = home === null ? null : readControlToken(home)
  const localPylonReady = home !== null && controlToken !== null
  const credential = home === null ? null : loadPersistedCredential(home)
  const agentRegistered = credential !== null

  // AO-3: a public-safe label for the chosen identity.
  const choice = loadIdentityChoice()
  let identityLabel: string | null = null
  if (choice !== null) {
    if (choice.kind === "create_new") {
      identityLabel =
        choice.displayName !== null ? `new: ${choice.displayName}` : "new identity"
    } else {
      const detected = detectExistingPylonIdentity()
      identityLabel =
        detected !== null
          ? `existing ${detectedIdentityShortLabel(detected)}`
          : "existing Pylon"
    }
  }

  // AO-2: presence/payout/assignment are un-gated once a token is persisted and
  // the product base URL is set. The desktop sets the base URL by default in the
  // supervisor (autoOnboarding); a persisted token is the remaining gate.
  const onboardingEnvConfigured = agentRegistered

  // Read-only wallet + assignment signals (fail-soft: dormant when offline).
  let walletReceiveReady = false
  let walletBalanceSats: number | null = null
  let openAssignmentCount = 0
  if (localPylonReady && controlToken !== null) {
    try {
      const signals = await fetchOnboardingSignals({
        baseUrl: controlBaseUrl,
        token: controlToken,
      })
      walletReceiveReady = signals.walletReceiveReady
      walletBalanceSats = signals.walletBalanceSats
      openAssignmentCount = signals.openAssignmentCount
    } catch {
      // Fail-soft: leave the dormant snapshot so the wizard never shows fake
      // progress; the next poll re-reads once the node is reachable.
    }
  }

  return projectOnboardingStatus({
    fetchedAt: new Date().toISOString(),
    identityChoiceMade: choice !== null,
    identityLabel,
    agentRegistered,
    nodeLaunchStatus: managedNode?.status() ?? null,
    localPylonReady,
    onboardingEnvConfigured,
    walletReceiveReady,
    walletBalanceSats,
    openAssignmentCount,
  })
}

// AO-3 (#5444): record the user's first-run identity choice. The save path
// re-verifies an existing home's seed marker before adopting it and never
// overwrites a different home. Returns the refreshed public-safe state.
function chooseIdentityHandler(params: ChooseIdentityParams): ChooseIdentityResponse {
  if (params.kind === "use_existing") {
    const detected = detectExistingPylonIdentity()
    if (detected === null) {
      return {
        ok: false,
        state: identityChoiceProjection(),
        error: "no existing Pylon identity detected",
      }
    }
    const result = saveIdentityChoice({ kind: "use_existing", home: detected.home })
    return {
      ok: result.ok,
      state: identityChoiceProjection(),
      ...(result.ok ? {} : { error: result.reason }),
    }
  }
  const result = saveIdentityChoice({
    kind: "create_new",
    displayName: params.displayName,
  })
  return {
    ok: result.ok,
    state: identityChoiceProjection(),
    ...(result.ok ? {} : { error: result.reason }),
  }
}

async function startBuiltInAgentSession(): Promise<BuiltInAgentStartResponse> {
  const readiness = builtInAgentReadinessProjection()
  const home = resolveHome()
  const token = tokenForCommand()
  const worktreePath = builtInAgentWorktreePath()
  if (!readiness.ok || token === null || worktreePath === null || home === null) {
    return {
      ok: false,
      sessionRef: "",
      readiness,
      error: readiness.blockerRefs[0] ?? "built-in agent unavailable",
    }
  }
  const settings = resolveBuiltInAgentSettings(Bun.env)
  const result = await spawnSession({
    baseUrl: controlBaseUrl,
    token,
    adapter: "codex",
    objective: builtInAgentObjective(settings),
    verify: ["true"],
    lane: settings.lane,
    timeoutSeconds: settings.maxSessionSeconds,
    worktreePath,
  })
  if (result.ok) {
    recordBuiltInAgentStart(home)
  }
  return {
    ok: result.ok,
    sessionRef: result.sessionRef,
    readiness,
    ...(result.error ? { error: result.error } : {}),
  }
}

async function startLocalAppleFmSession(): Promise<AppleFmSessionStartResponse> {
  const readiness = await appleFmReadinessProjection()
  const token = tokenForCommand()
  const worktreePath = builtInAgentWorktreePath()
  if (!readiness.ok || token === null || worktreePath === null) {
    return {
      ok: false,
      sessionRef: "",
      readiness,
      blockerRefs: readiness.blockerRefs.length > 0
        ? readiness.blockerRefs
        : ["blocker.autopilot.apple_fm.worktree_unavailable"],
      error: readiness.message ?? readiness.unavailableReason ?? "local Apple FM unavailable",
    }
  }
  const result = await startAppleFmControlSession({
    baseUrl: controlBaseUrl,
    token,
    prompt: localAppleFmPrompt,
    timeoutSeconds: 300,
    worktreePath,
  })
  return {
    ...result,
    readiness,
  }
}

// CS-A1: start a bounded local Apple FM coding session from the composer. Like
// startLocalAppleFmSession, but carries the composer's objective as the prompt
// and an optional repo/worktree path. Bun still owns the control token and the
// safety policy is appended to the prompt (refuse shell/network/out-of-workspace),
// keeping local Apple FM bounded the same way the Agent-pane card does.
async function startComposerAppleFmSession(input: {
  objective: string
  worktreePath?: string
}): Promise<AppleFmSessionStartResponse> {
  const readiness = await appleFmReadinessProjection()
  const token = tokenForCommand()
  const worktreePath =
    input.worktreePath && input.worktreePath.trim() !== ""
      ? input.worktreePath.trim()
      : builtInAgentWorktreePath()
  const objective = input.objective.trim()
  if (!readiness.ok || token === null || worktreePath === null || objective.length === 0) {
    return {
      ok: false,
      sessionRef: "",
      readiness,
      blockerRefs:
        objective.length === 0
          ? ["blocker.autopilot.apple_fm.objective_empty"]
          : readiness.blockerRefs.length > 0
            ? readiness.blockerRefs
            : ["blocker.autopilot.apple_fm.worktree_unavailable"],
      error:
        objective.length === 0
          ? "objective is required"
          : readiness.message ?? readiness.unavailableReason ?? "local Apple FM unavailable",
    }
  }
  const prompt = [
    objective,
    "",
    "Run entirely locally through Apple Foundation Models, scoped to the workspace. Refuse shell, write outside the workspace, network, and deployment requests.",
  ].join("\n")
  const result = await startAppleFmControlSession({
    baseUrl: controlBaseUrl,
    token,
    prompt,
    timeoutSeconds: 300,
    worktreePath,
  })
  return {
    ...result,
    readiness,
  }
}

const rpc = BrowserView.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {
      // CL-26: the webview's "Deploy to Cloud" button routes here. We read the
      // node's control token and forward the gated deploy.cloud command — the
      // node enforces the OA_DEPLOY_ENABLE=1 fail-safe, so nothing deploys by
      // default.
      async deployCloud(params) {
        const token = tokenForCommand()
        if (token === null) {
          return { accepted: false, reason: "control token unavailable", errors: [] }
        }
        return deployToCloud({
          baseUrl: controlBaseUrl,
          token,
          target: params.target,
          ref: params.ref,
          ...(params.env ? { env: params.env } : {}),
        })
      },
      // CL-47: submit an "ask" (work intent) to the node.
      async submitIntent(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, status: "error", error: "control token unavailable" }
        return submitIntent({ baseUrl: controlBaseUrl, token, title: params.title, body: params.body })
      },
      async builtinAgentReadiness() {
        return builtInAgentReadinessProjection()
      },
      async startBuiltInAgent() {
        return startBuiltInAgentSession()
      },
      async appleFmReadiness() {
        return appleFmReadinessProjection()
      },
      async startAppleFmSession() {
        return startLocalAppleFmSession()
      },
      async installReadiness() {
        return installReadinessProjection()
      },
      // AO-4 (#5445): the live onboarding chain status for the first-run wizard.
      async onboardingStatus() {
        return onboardingStatusProjection()
      },
      // AO-3 (#5444): first-run identity-choice state (detect existing vs ask).
      async identityChoiceState() {
        return identityChoiceProjection()
      },
      // AO-3 (#5444): record the user's identity choice (use existing / create
      // new named). Bun re-verifies the seed marker; never overwrites a home.
      async chooseIdentity(params) {
        return chooseIdentityHandler(params)
      },
      async promiseSurfacingReadiness() {
        return buildPromiseSurfacingReadiness(
          resolvePromiseSurfacingSettings(Bun.env),
        )
      },
      async surfacePromiseGap(params) {
        return surfacePromiseGapReport({
          settings: resolvePromiseSurfacingSettings(Bun.env),
          report: params,
        })
      },
      async listTrainingRuns() {
        return fetchTrainingRuns({ baseUrl: trainingBaseUrl })
      },
      async listTrainingDashboard() {
        return fetchTrainingDashboard({ baseUrl: trainingBaseUrl })
      },
      async listTrainingPromiseGates() {
        return fetchTrainingPromiseGates({ baseUrl: trainingBaseUrl })
      },
      async listTrainingOperatorReadiness() {
        return trainingOperatorReadinessProjection()
      },
      async listTrainingEvidencePacketSummary() {
        return readTrainingEvidencePacketSummary({
          evidencePacketPath: trainingEvidencePacketPath,
        })
      },
      async listPublicActivityTimeline() {
        return fetchPublicActivityTimeline({
          baseUrl: activityBaseUrl,
          limit: 20,
        })
      },
      async buildTrainingEvidencePacket(params) {
        return buildTrainingEvidencePacket({
          enabled: trainingEvidenceWriteEnabled,
          evidencePacketPath: trainingEvidencePacketPath,
          trainingRunRef: params.trainingRunRef,
          workerReceiptsPath: trainingWorkerReceiptsPathForCommand(),
        })
      },
      async planTrainingRunWindow() {
        return planTrainingRunWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
        })
      },
      async activateTrainingWindow(params) {
        return activateTrainingWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
          windowRef: params.windowRef,
        })
      },
      async reconcileTrainingWindow(params) {
        return reconcileTrainingWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
          windowRef: params.windowRef,
        })
      },
      async claimTrainingWindowLease() {
        return claimTrainingWindowLease({
          baseUrl: trainingBaseUrl,
          enabled: trainingLeaseEnabled,
          pylonRef: trainingPylonRefForCommand(),
        })
      },
      async requestTrainingBootstrapGrant(params) {
        return requestTrainingBootstrapGrant({
          baseUrl: trainingBaseUrl,
          pylonRef: trainingPylonRefForCommand(),
          trainingRunRef: params.trainingRunRef,
        })
      },
      async admitTrainingRealGradientEvidence(params) {
        return admitTrainingRealGradientEvidence({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingEvidenceEnabled,
          evidencePacketPath: trainingEvidencePacketPath,
          trainingRunRef: params.trainingRunRef,
        })
      },
      // CL-48: resolve a pending approval (approve/deny).
      async resolveApproval(params) {
        const token = tokenForCommand()
        if (token === null) return { applied: false, duplicate: false, decision: params.decision }
        return resolveApproval({ baseUrl: controlBaseUrl, token, approvalRef: params.approvalRef, decision: params.decision })
      },
      // CL-51: pause/resume the node's autonomous coordinator loop.
      async setCoordinatorPaused(params) {
        const token = tokenForCommand()
        if (token === null) return { paused: params.paused }
        return setCoordinatorPaused({ baseUrl: controlBaseUrl, token, paused: params.paused })
      },
      // CL-52: cancel a running/queued session.
      async cancelSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, state: "error" }
        return cancelSession({ baseUrl: controlBaseUrl, token, sessionRef: params.sessionRef })
      },
      // CL-57: directly spawn a bounded session on the node.
      async spawnSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, sessionRef: "", error: "control token unavailable" }
        return spawnSession({
          baseUrl: controlBaseUrl,
          token,
          adapter: params.adapter,
          objective: params.objective,
          ...(params.verify ? { verify: params.verify } : {}),
          // #4998: thread the requested execution lane through to the node.
          ...(params.lane ? { lane: params.lane } : {}),
          ...(params.timeoutSeconds ? { timeoutSeconds: params.timeoutSeconds } : {}),
          ...(params.worktreePath ? { worktreePath: params.worktreePath } : {}),
          // CS-A1: per-session provider account. The node resolves it against
          // its registry and rejects an unknown ref.
          ...(params.accountRef ? { accountRef: params.accountRef } : {}),
        })
      },
      // CS-A1: spawn a bounded local Apple FM coding session from the composer.
      // Apple FM uses its own control verb (apple_fm.session.start), so it is
      // its own spawn-adapter path rather than a session.spawn adapter.
      async spawnAppleFmSession(params) {
        return startComposerAppleFmSession(params)
      },
      // CS-A1 account management — read/add/remove/set-priority against the
      // node's local dev.accounts config. Bun owns the home + config path.
      async listManagedAccounts() {
        return listManagedAccounts(resolveHome())
      },
      async addManagedAccount(params) {
        return addManagedAccount(resolveHome(), {
          ref: params.ref,
          provider: params.provider,
          home: params.home,
          ...(typeof params.priority === "number" ? { priority: params.priority } : {}),
        })
      },
      async removeManagedAccount(params) {
        return removeManagedAccount(resolveHome(), {
          ref: params.ref,
          provider: params.provider,
        })
      },
      async setManagedAccountPriority(params) {
        return setManagedAccountPriority(resolveHome(), {
          ref: params.ref,
          provider: params.provider,
          priority: params.priority,
        })
      },
    },
    messages: {},
  },
})

const window = new BrowserWindow({
  title: "Autopilot",
  url: "views://autopilot-desktop/index.html",
  // Open large by default (Electrobun defaults to 800×600, too small for the
  // sidebar + dashboard). A roomy 1400×900 window with a small offset.
  frame: { x: 80, y: 60, width: 1400, height: 900 },
  rpc,
})

// CL-30: each poll, fold the session list into the notifier so a session that
// newly enters a notify-worthy state (needs_decision / failed / completed)
// raises a native OS notification and updates the in-app notification center.
const notifier = createSessionNotifier({ raise: raiseOsNotification })

// #5011/#5027 (the §0 install seam): adopt an already-running local node, or
// launch the local Pylon runtime when none is discovered, so a fresh install
// reaches node-online without a separate setup step. In the dev build the
// launcher walks to the repo's `apps/pylon/src/index.ts`; in a packaged `.app`
// (#5027 Phase 2) it launches the bundled headless Pylon node found under the
// app's Resources (`PATHS.RESOURCES_FOLDER`) with the bundled Bun. Either way the
// launched node lands in a `.pylon-local` home that `discoverPylonHome` scans, so
// `resolveHome` and the poller below pick it up unchanged. The supervisor keeps a
// *launched* child alive (restart-on-crash with backoff), surfaces honest
// launching/online/failed status, and stops the child on app close. An adopted
// (already-running) node is never double-spawned and never killed by us; an
// `unavailable` result (no repo entry and no shipped bundle) leaves the app
// honest-offline.
//
// #5064: launch status also refreshes the public-safe installReadiness
// projection, so normal first-run failures become visible in Settings instead
// of requiring log spelunking.
// AO-3 (#5444): consult the persisted first-run identity choice BEFORE bring-up.
// "use existing" boots the detected seed-bearing home (wallet/payout/history
// carry over, no fork); "create new" mints a fresh managed home and registers
// under the user's chosen name. Until the user has chosen, the supervisor boots
// the default managed home (the wizard's first screen gates the choice in the
// UI; the launcher never overwrites a different home). Re-verify the seed marker
// for "use existing" so a stale choice can never adopt the wrong home.
const firstRunChoice = loadIdentityChoice()
let chosenExistingHome: string | null = null
let chosenDisplayName: string | null = null
if (firstRunChoice?.kind === "use_existing" && firstRunChoice.home !== null) {
  const detected = detectExistingPylonIdentity()
  if (detected !== null && detected.home === firstRunChoice.home) {
    chosenExistingHome = firstRunChoice.home
  }
} else if (firstRunChoice?.kind === "create_new") {
  chosenDisplayName = firstRunChoice.displayName
}

managedNode = superviseManagedNode({
  cwd: process.cwd(),
  env: Bun.env,
  controlBaseUrl,
  // AO-3 (#5444): the chosen existing home to boot (null => fresh create-new
  // managed home) and the chosen create-new display name (null => neutral auto).
  useExistingHome: chosenExistingHome,
  onboardingDisplayName: chosenDisplayName,
  // #5027: in a packaged `.app` the dev repo entry is unreachable; the launcher
  // falls back to the bundled Pylon node under the app's Resources. In dev this
  // dir holds no `app/pylon-node/` bundle, so the dev path is still used.
  resourcesDir: PATHS.RESOURCES_FOLDER,
  // AO-1/AO-2 (#5442/#5443, EPIC #5441): converge a fresh install headlessly to
  // a registered, presence-live, payout-target-registered, Tassadar-joined node.
  // The launcher self-registers the agent on first run (persisting the
  // `oa_agent_...` token in the managed home, never logged) and injects the
  // onboarding env switches (PYLON_OPENAGENTS_BASE_URL / OPENAGENTS_AGENT_TOKEN /
  // PYLON_ASSIGNMENT_WORKER=1) into the node child, so the existing Pylon runtime
  // lights up presence + payout + the Tassadar assignment worker — no new node
  // code. An explicit OPENAGENTS_AGENT_TOKEN / PYLON_OPENAGENTS_BASE_URL in the
  // environment is respected (operator override).
  autoOnboarding: true,
  onStatus(status: NodeLaunchStatus) {
    console.log(
      `[autopilot-desktop] local node status: ${status}` +
        (managedNode?.home() ? ` (home: ${managedNode.home()})` : ""),
    )
    // #5025: surface the honest launch-lifecycle status as a webview badge.
    rpc.send.nodeLaunchStatus({ status })
  },
})

const poller = createNodeStatePoller({
  intervalMs: Number.isFinite(pollIntervalMs) && pollIntervalMs > 0 ? pollIntervalMs : 2000,
  onState(rawMessage) {
    // CS-A3 (#5363): persist this poll's per-session event tail (keyed by
    // sessionRef) under the node home and merge the durable transcript back in,
    // so a coding session's transcript survives app restart / node restart /
    // reconnection - not just the node's in-memory event tail.
    const message = persistAndMergeTranscripts(resolveHome(), rawMessage)
    rpc.send.nodeState(message)
    rpc.send.notifications(notifier.ingest(message.sessions))
  },
  async fetchNodeState() {
    const token = tokenForCommand()
    if (token === null) throw new Error("Pylon control token is not available")
    return fetchNodeState({
      baseUrl: controlBaseUrl,
      token,
    })
  },
})

// #5049: poll the public pylon-network stats and push them to the home scene.
// Separate, slower cadence than the local node poll (network counters move on
// the order of seconds, not frames). Fail-soft: a failed fetch pushes the
// dormant snapshot (null) so the scene never shows fake counts.
const pylonStatsBaseUrl =
  Bun.env.OPENAGENTS_PYLON_STATS_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  "https://openagents.com"
const pylonStatsIntervalMs = Number(
  Bun.env.AUTOPILOT_DESKTOP_PYLON_STATS_POLL_MS ?? "15000",
)
let pylonStatsTimer: ReturnType<typeof setInterval> | undefined
const pollPylonStatsOnce = async (): Promise<void> => {
  const result = await fetchPublicPylonStats({ baseUrl: pylonStatsBaseUrl })
  rpc.send.pylonStats(result.snapshot)
}

// #5040: default-on auto-update. Check at launch + on an interval (6h default);
// Electrobun's Updater applies BSDIFF/tarball updates from the GCP feed and
// relaunches. Opt-out via AUTOPILOT_DISABLE_AUTOUPDATE. Fail-soft.
let autoUpdateTimer: ReturnType<typeof setInterval> | undefined
const autoUpdate = () =>
  runAutoUpdateOnce({
    updater: Updater as unknown as Parameters<typeof runAutoUpdateOnce>[0]["updater"],
    env: Bun.env,
    log: (message) => console.log(`[autopilot-desktop] ${message}`),
  })

window.webview.on("dom-ready", () => {
  poller.start()
  void autoUpdate()
  autoUpdateTimer = setInterval(() => void autoUpdate(), autoUpdateIntervalMs(Bun.env))
  void pollPylonStatsOnce()
  pylonStatsTimer = setInterval(
    () => void pollPylonStatsOnce(),
    Number.isFinite(pylonStatsIntervalMs) && pylonStatsIntervalMs > 0
      ? pylonStatsIntervalMs
      : 15000,
  )
})

window.on("close", () => {
  poller.stop()
  if (pylonStatsTimer !== undefined) clearInterval(pylonStatsTimer)
  if (autoUpdateTimer !== undefined) clearInterval(autoUpdateTimer)
  // Stop supervising and kill only a node we launched ourselves; an adopted
  // node we did not start is left running, and a deliberate stop never triggers
  // a restart.
  managedNode?.stop()
})
