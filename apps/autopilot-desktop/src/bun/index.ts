import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import {
  ApplicationMenu,
  BrowserView,
  BrowserWindow,
  PATHS,
  Updater,
} from "electrobun/bun"
import { desktopApplicationMenu } from "./application-menu.js"
import { createControlTokenResolver, discoverPylonHome } from "./node-home.js"
import {
  type NodeLaunchStatus,
  type SupervisedNode,
  superviseManagedNode,
} from "./node-launcher.js"
import { createNodeStatePoller } from "./node-state-poll.js"
import {
  createSessionEventStreamer,
  mergeSessionEventRows,
} from "./session-event-stream.js"
import { persistAndMergeTranscripts } from "./transcript-store.js"
import { loadPersistedCredential } from "./agent-onboarding.js"
import {
  claimForumTipRecipientReadiness,
  isForumTipReady,
} from "./forum-tip-recipient.js"
import { hasPostedForumIntro, postForumIntroduction } from "./forum-intro.js"
import {
  loadWorkSearchReceipt,
  searchForumWork,
} from "./forum-work-search.js"
import {
  detectExistingPylonIdentity,
  detectedIdentityShortLabel,
  loadIdentityChoice,
  projectIdentityChoiceState,
  saveIdentityChoice,
} from "./identity-choice.js"
import { resolveFirstRunLaunchChoice } from "./first-run-launch-choice.js"
import { projectOnboardingStatus } from "../shared/onboarding-status.js"
import {
  autoUpdateDisabledReason,
  autoUpdateIntervalMs,
  runAutoUpdateOnce,
} from "./auto-update.js"
import { fetchPublicPylonStats } from "./pylon-network-stats.js"
import { fetchPublicActivityTimeline } from "./public-activity-timeline.js"
import {
  promiseSurfacingReadiness as buildPromiseSurfacingReadiness,
  resolvePromiseSurfacingSettings,
  surfacePromiseGapReport,
} from "./promise-surfacing.js"
import { createSessionNotifier } from "./notifier.js"
import { raiseOsNotification } from "./os-notification.js"
import {
  builtInAgentObjective,
  resolveBuiltInAgentSettings,
} from "../shared/builtin-agent.js"
import { projectInstallReadiness } from "../shared/install-readiness.js"
import { buildInferenceGatewayReadiness } from "./inference-gateway.js"
import { buildShellTurn, resolveShellAgentToken } from "./shell-turn.js"
import { buildKhalaTurn } from "./khala-turn.js"
import { buildVerseTurn } from "./verse-turn.js"
import {
  addManagedAccount,
  listManagedAccounts,
  removeManagedAccount,
  setManagedAccountPriority,
} from "./account-management.js"
import {
  cancelSession,
  deployToCloud,
  fetchAppleFmReadiness,
  fetchNodeSparkAddress,
  fetchNodeState,
  fetchOnboardingSignals,
  probeControlToken,
  readControlToken,
  resolveApproval,
  resolveManagedWorktreeRepoRef,
  setCoordinatorPaused,
  startAppleFmSession as startAppleFmControlSession,
  spawnSession,
  submitIntent,
} from "./pylon-control.js"
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
} from "./training-runs.js"
import {
  DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type AppleFmReadinessResponse,
  type AppleFmSessionStartResponse,
  type BuiltInAgentReadinessResponse,
  type BuiltInAgentStartResponse,
  type ChooseIdentityParams,
  type ChooseIdentityResponse,
  type NodeStateMessage,
  type DesktopRPCSchema,
  type IdentityChoiceStateResponse,
  type InstallReadinessResponse,
  type OnboardingStatusResponse,
  type TrainingOperatorReadinessPylonRefSource,
  type TrainingOperatorReadinessResponse,
} from "../shared/rpc.js"

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
// AF-2 (#5899): product base URL for forum tip-recipient readiness claims.
const onboardingBaseUrl =
  Bun.env.PYLON_OPENAGENTS_BASE_URL ??
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

type OnboardingSignals = Awaited<ReturnType<typeof fetchOnboardingSignals>>
type CachedOnboardingSignals = {
  readonly cachedAtMs: number
  readonly key: string
  readonly signals: OnboardingSignals
}
type PendingOnboardingSignals = {
  readonly key: string
  readonly promise: Promise<OnboardingSignals>
}

const onboardingSignalWaitMs = Number(
  Bun.env.AUTOPILOT_DESKTOP_ONBOARDING_SIGNAL_WAIT_MS ?? "650",
)
const onboardingSignalCacheMs = Number(
  Bun.env.AUTOPILOT_DESKTOP_ONBOARDING_SIGNAL_CACHE_MS ?? "2500",
)
let cachedOnboardingSignals: CachedOnboardingSignals | null = null
let pendingOnboardingSignals: PendingOnboardingSignals | null = null

const defaultOnboardingSignals = (): OnboardingSignals => ({
  walletReceiveReady: false,
  walletBalanceSats: null,
  openAssignmentCount: 0,
})

const sleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

const onboardingSignalKey = (input: {
  readonly baseUrl: string
  readonly token: string
}): string => `${input.baseUrl}\0${input.token}`

async function readOnboardingSignalsFast(input: {
  readonly baseUrl: string
  readonly token: string
}): Promise<OnboardingSignals> {
  const key = onboardingSignalKey(input)
  const nowMs = Date.now()
  const cacheMs =
    Number.isFinite(onboardingSignalCacheMs) && onboardingSignalCacheMs > 0
      ? onboardingSignalCacheMs
      : 2500
  if (
    cachedOnboardingSignals?.key === key &&
    nowMs - cachedOnboardingSignals.cachedAtMs < cacheMs
  ) {
    return cachedOnboardingSignals.signals
  }

  if (pendingOnboardingSignals === null || pendingOnboardingSignals.key !== key) {
    const promise = fetchOnboardingSignals(input)
      .then(signals => {
        if (pendingOnboardingSignals?.key === key) {
          cachedOnboardingSignals = { cachedAtMs: Date.now(), key, signals }
        }
        return signals
      })
      .finally(() => {
        if (pendingOnboardingSignals?.key === key) {
          pendingOnboardingSignals = null
        }
      })
    pendingOnboardingSignals = { key, promise }
  }

  const waitMs =
    Number.isFinite(onboardingSignalWaitMs) && onboardingSignalWaitMs > 0
      ? onboardingSignalWaitMs
      : 650
  const timeout = sleep(waitMs).then(() => null)
  const first = await Promise.race([pendingOnboardingSignals.promise, timeout])
  if (first !== null) return first
  return cachedOnboardingSignals?.key === key
    ? cachedOnboardingSignals.signals
    : defaultOnboardingSignals()
}

// CL-45: resolve the node home once per call so a node that starts after the app
// (or rotates its home) is picked up without a restart. Falls back to the env
// default. `readControlToken` returns null when the chosen home has no token,
// which the poll surfaces as an honest offline state.
function resolveHome(): string | null {
  return discoverPylonHome({ env: Bun.env.PYLON_HOME, cwd: process.cwd() })
}

// CL-45b: clear, actionable blocker copy when NO candidate-home control token is
// accepted by the live control server — surfaced instead of a bare `control
// 401` (the symptom of a stale token in an earlier candidate home shadowing the
// canonical Pylon home).
const CONTROL_TOKEN_NOT_ACCEPTED =
  "node control token not found or not accepted — is your Pylon node running? (start it with `pylon node`)"

// CL-45b: server-validated control-token resolution for control-plane CALLS.
// `discoverPylonHome`/`readControlToken` first-match on a *readable* token,
// which dead-ends auth when an earlier candidate home (e.g. a stale
// `<repo>/.pylon-tailnet/control-token`) shadows the canonical
// `~/.openagents/pylon` home the running control server actually accepts. The
// resolver probes candidate-home tokens against the live server (POST /command
// {type:"session.list"}; 401 = reject, non-401 = accept) and uses the first
// accepted one, caching the result so it doesn't probe on every command.
const controlTokenResolver = createControlTokenResolver(() => ({
  env: Bun.env.PYLON_HOME,
  cwd: process.cwd(),
  probe: (token: string) => probeControlToken({ baseUrl: controlBaseUrl, token }),
}))

// Resolve a control token the live server accepts, or null when none of the
// candidate homes hold an accepted token. Used by every control-call handler.
async function acceptedControlTokenForCommand(): Promise<string | null> {
  const accepted = await controlTokenResolver.resolve()
  return accepted?.token ?? null
}

async function acceptedControlHomeForCommand(): Promise<{
  readonly home: string
  readonly token: string
} | null> {
  return controlTokenResolver.resolve()
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

function isGitCheckout(path: string): boolean {
  return existsSync(join(path, ".git"))
}

function nearestGitAncestor(start: string): string | null {
  let current = start
  for (let i = 0; i < 16; i += 1) {
    if (isGitCheckout(current)) return current
    const parent = dirname(current)
    if (parent === current) return null
    current = parent
  }
  return null
}

function configuredCodexWorkspacePath(home = resolveHome()): string | null {
  if (home === null) return null
  try {
    const config = JSON.parse(readFileSync(join(home, "config.json"), "utf8")) as {
      codex_workspaces?: unknown
    }
    if (!Array.isArray(config.codex_workspaces)) return null
    const rows = config.codex_workspaces
      .map((row) => row as { id?: unknown; label?: unknown; root?: unknown })
      .filter((row): row is { id?: string; label?: string; root: string } =>
        typeof row.root === "string" && row.root.trim() !== "",
      )
    const preferred = rows.filter((row) => {
      const id = row.id ?? ""
      const label = row.label ?? ""
      return id === "openagents" || id === "openagents-short" ||
        label.toLowerCase().includes("openagents")
    })
    const selected = [...preferred, ...rows].find((row) => isGitCheckout(row.root))
    return selected?.root ?? null
  } catch {
    return null
  }
}

function defaultCodingWorktreePath(home = resolveHome()): string | null {
  const configured = Bun.env.OPENAGENTS_DESKTOP_CODING_WORKTREE?.trim() ?? ""
  if (configured !== "" && isGitCheckout(configured)) return configured
  return configuredCodexWorkspacePath(home) ?? nearestGitAncestor(process.cwd())
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
  const fallbackHome = onboardingHome()
  const acceptedControl = await acceptedControlHomeForCommand()
  const home = acceptedControl?.home ?? fallbackHome
  const controlToken =
    acceptedControl?.token ?? (home === null ? null : readControlToken(home))
  const localPylonReady = acceptedControl !== null
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
      const signals = await readOnboardingSignalsFast({
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

  // AF-2 (#5899): once the wallet is receive-ready and the agent is registered,
  // best-effort claim forum tip-recipient readiness so the agent's forum posts
  // can receive Spark tips. Fire-and-forget + idempotent (a persisted receipt
  // short-circuits); never blocks the projection. Receive-only — no spend.
  const forumTipReady = home !== null && isForumTipReady(home)
  if (
    home !== null &&
    controlToken !== null &&
    agentRegistered &&
    walletReceiveReady &&
    !forumTipReady
  ) {
    void maybeClaimForumTipReady(home, controlToken)
  }

  // AF-3 (#5900): once the agent is registered and the node is online, post the
  // public forum self-introduction (once). Single-flight + idempotent (persisted
  // receipt short-circuits); never blocks the projection.
  const forumIntroPosted = home !== null && hasPostedForumIntro(home)
  if (
    home !== null &&
    agentRegistered &&
    localPylonReady &&
    !forumIntroPosted
  ) {
    void maybePostForumIntro(home, forumTipReady)
  }

  // AF-4 (#5901): read-only work-search over the typed work-requests lane on a
  // slow cadence. Discovery only — never bids/quotes/accepts/spends.
  const workReceipt = home === null ? null : loadWorkSearchReceipt(home)
  const forumWorkSearched = workReceipt !== null
  const forumWorkOpenCount = workReceipt?.openCount ?? 0
  if (home !== null && agentRegistered && localPylonReady) {
    void maybeSearchForumWork(home, workReceipt?.lastSearchedAt ?? null)
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
    forumTipReady,
    forumIntroPosted,
    forumWorkSearched,
    forumWorkOpenCount,
  })
}

// AF-4 (#5901): single-flight, slow-cadence driver for read-only work-search.
// Re-searches at most once per WORK_SEARCH_MIN_INTERVAL_MS so a 2s status poll
// does not hammer the lane. Discovery only — never commits or spends.
const WORK_SEARCH_MIN_INTERVAL_MS = 60_000
let workSearchInFlight = false
async function maybeSearchForumWork(
  home: string,
  lastSearchedAt: string | null,
): Promise<void> {
  if (workSearchInFlight) return
  if (lastSearchedAt !== null) {
    const last = Date.parse(lastSearchedAt)
    if (Number.isFinite(last) && Date.now() - last < WORK_SEARCH_MIN_INTERVAL_MS) {
      return
    }
  }
  workSearchInFlight = true
  try {
    await searchForumWork({ home, baseUrl: onboardingBaseUrl })
  } catch {
    // Non-fatal: retried on a later poll.
  } finally {
    workSearchInFlight = false
  }
}

// AF-3 (#5900): single-flight driver for the forum self-introduction post.
// Idempotent (the persisted receipt makes it a no-op once posted); never logs
// the agent token. Mentions tip-readiness in the copy when tips are claimable.
let introPostInFlight = false
async function maybePostForumIntro(
  home: string,
  forumTipReady: boolean,
): Promise<void> {
  if (introPostInFlight) return
  introPostInFlight = true
  try {
    await postForumIntroduction({
      home,
      baseUrl: onboardingBaseUrl,
      authority: { tipReady: forumTipReady },
    })
  } catch {
    // Non-fatal: a failed post is retried on a later poll. Never logs secrets.
  } finally {
    introPostInFlight = false
  }
}

// AF-2 (#5899): single-flight driver for the forum tip-recipient readiness
// claim. Reads the node's Spark address from the control server (payment
// material, never logged) and runs the receive-only claim. Guarded so repeated
// status polls don't fire overlapping claims; the persisted receipt makes it a
// no-op once claimed.
let tipClaimInFlight = false
async function maybeClaimForumTipReady(
  home: string,
  controlToken: string,
): Promise<void> {
  if (tipClaimInFlight) return
  tipClaimInFlight = true
  try {
    const sparkAddress = await fetchNodeSparkAddress({
      baseUrl: controlBaseUrl,
      token: controlToken,
    })
    await claimForumTipRecipientReadiness({
      home,
      baseUrl: onboardingBaseUrl,
      walletReceiveReady: true,
      sparkAddress,
    })
  } catch {
    // Non-fatal: a failed claim is retried on a later poll. Never logs secrets.
  } finally {
    tipClaimInFlight = false
  }
}

async function restartManagedNodeForIdentityChoice(): Promise<void> {
  cachedOnboardingSignals = null
  pendingOnboardingSignals = null
  managedNode?.stop()
  managedNode = null
  await sleep(300)
  startManagedNodeSupervisor()
}

// AO-3 (#5444): record the user's first-run identity choice. The save path
// re-verifies an existing home's seed marker before adopting it and never
// overwrites a different home. Returns the refreshed public-safe state.
async function chooseIdentityHandler(params: ChooseIdentityParams): Promise<ChooseIdentityResponse> {
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
    if (result.ok) await restartManagedNodeForIdentityChoice()
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
  if (result.ok) await restartManagedNodeForIdentityChoice()
  return {
    ok: result.ok,
    state: identityChoiceProjection(),
    ...(result.ok ? {} : { error: result.reason }),
  }
}

async function startBuiltInAgentSession(): Promise<BuiltInAgentStartResponse> {
  const readiness = builtInAgentReadinessProjection()
  const home = resolveHome()
  const token = await acceptedControlTokenForCommand()
  const worktreePath = builtInAgentWorktreePath()
  if (!readiness.ok || token === null || worktreePath === null || home === null) {
    return {
      ok: false,
      sessionRef: "",
      readiness,
      error:
        token === null && readiness.ok
          ? CONTROL_TOKEN_NOT_ACCEPTED
          : readiness.blockerRefs[0] ?? "built-in agent unavailable",
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
  const token = await acceptedControlTokenForCommand()
  const worktreePath = builtInAgentWorktreePath()
  if (!readiness.ok || token === null || worktreePath === null) {
    return {
      ok: false,
      sessionRef: "",
      readiness,
      blockerRefs: readiness.blockerRefs.length > 0
        ? readiness.blockerRefs
        : ["blocker.autopilot.apple_fm.worktree_unavailable"],
      error:
        token === null && readiness.ok
          ? CONTROL_TOKEN_NOT_ACCEPTED
          : readiness.message ?? readiness.unavailableReason ?? "local Apple FM unavailable",
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
  const token = await acceptedControlTokenForCommand()
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
          : token === null && readiness.ok
            ? CONTROL_TOKEN_NOT_ACCEPTED
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
  maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {
      // Open an external URL in the system browser. The webview routes every
      // external anchor click here so it can never navigate the app away from
      // the local UI (which would strand the user off-app, e.g. on github.com).
      // Only http(s) is honored; spawned as argv (no shell) so the URL can't
      // inject. Best-effort: a failure to launch the browser is swallowed.
      async openExternal({ url }) {
        if (/^https?:\/\//i.test(url)) {
          try {
            Bun.spawn(["open", url], { stdout: "ignore", stderr: "ignore" })
          } catch {
            // best-effort; never throw back into the webview
          }
        }
        return { ok: true }
      },
      // CL-26: the webview's "Deploy to Cloud" button routes here. We read the
      // node's control token and forward the gated deploy.cloud command — the
      // node enforces the OA_DEPLOY_ENABLE=1 fail-safe, so nothing deploys by
      // default.
      async deployCloud(params) {
        const token = await acceptedControlTokenForCommand()
        if (token === null) {
          return { accepted: false, reason: CONTROL_TOKEN_NOT_ACCEPTED, errors: [] }
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
        const token = await acceptedControlTokenForCommand()
        if (token === null) return { ok: false, status: "error", error: CONTROL_TOKEN_NOT_ACCEPTED }
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
      // #5485: OpenAgents inference-gateway readiness for default-inference
      // routing. Bun owns the API key (never crosses to the webview); the
      // readiness blob carries only the server-flag state, apiKeyPresent, model,
      // and the numeric credit balance. INERT until the gateway flag is on.
      async inferenceGatewayReadiness() {
        return buildInferenceGatewayReadiness({
          env: Bun.env,
          apiKey: Bun.env.OPENAGENTS_INFERENCE_API_KEY ?? null,
        })
      },
      // HUD H5 (#5503): one zero-base shell turn. The Bun host resolves the
      // desktop's OpenAgents agent token and calls the live inference gateway;
      // only the plain Autopilot text (or an honest configure/error message)
      // crosses back to the webview. The raw token never does.
      //
      // Token source (#5503 live-gateway fix): prefer an explicit env override
      // (OPENAGENTS_SHELL_AGENT_TOKEN / OPENAGENTS_AGENT_TOKEN), then fall back
      // to the agent credential the desktop already mints + persists during
      // auto-onboarding (AO-1, `<PYLON_HOME>/agent-credential.json`, the same
      // token the node uses for openagents.com — see node-launcher.ts). This is
      // what lets the chat authenticate as the owner's real agent identity with
      // no manual env var. The token stays host-side; it is never logged and
      // never crosses to the webview.
      async shellTurn(params) {
        return buildShellTurn({
          prompt: params.prompt,
          env: Bun.env,
          agentToken: resolveShellAgentToken(Bun.env, () => {
            const home = onboardingHome()
            return home === null
              ? null
              : (loadPersistedCredential(home)?.token ?? null)
          }),
        })
      },
      async verseTurn(params) {
        return buildVerseTurn({
          prompt: params.prompt,
          env: Bun.env,
          agentToken: resolveShellAgentToken(Bun.env, () => {
            const home = onboardingHome()
            return home === null
              ? null
              : (loadPersistedCredential(home)?.token ?? null)
          }),
        })
      },
      // M1 (#6009, EPIC #6017): one Khala cockpit turn. Submits to a
      // `openagents/khala-*` model and returns the answer plus the public-safe
      // `openagents` receipt projection. Reuses the same host-side agent token
      // resolution as shellTurn; the raw token never crosses to the webview.
      async khalaTurn(params) {
        return buildKhalaTurn({
          prompt: params.prompt,
          ...(params.model === undefined ? {} : { model: params.model }),
          env: Bun.env,
          agentToken: resolveShellAgentToken(Bun.env, () => {
            const home = onboardingHome()
            return home === null
              ? null
              : (loadPersistedCredential(home)?.token ?? null)
          }),
        })
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
        const token = await acceptedControlTokenForCommand()
        if (token === null) return { applied: false, duplicate: false, decision: params.decision }
        return resolveApproval({ baseUrl: controlBaseUrl, token, approvalRef: params.approvalRef, decision: params.decision })
      },
      // CL-51: pause/resume the node's autonomous coordinator loop.
      async setCoordinatorPaused(params) {
        const token = await acceptedControlTokenForCommand()
        if (token === null) return { paused: params.paused }
        return setCoordinatorPaused({ baseUrl: controlBaseUrl, token, paused: params.paused })
      },
      // CL-52: cancel a running/queued session.
      async cancelSession(params) {
        const token = await acceptedControlTokenForCommand()
        if (token === null) return { ok: false, state: "error" }
        return cancelSession({ baseUrl: controlBaseUrl, token, sessionRef: params.sessionRef })
      },
      // CL-57: directly spawn a bounded session on the node.
      async spawnSession(params) {
        const token = await acceptedControlTokenForCommand()
        if (token === null) return { ok: false, sessionRef: "", error: CONTROL_TOKEN_NOT_ACCEPTED }
        const defaultWorktree =
          params.useDefaultWorktree === true && !params.worktreePath && !params.repoRef
            ? defaultCodingWorktreePath()
            : null
        const result = await spawnSession({
          baseUrl: controlBaseUrl,
          token,
          adapter: params.adapter,
          objective: params.objective,
          ...(params.verify ? { verify: params.verify } : {}),
          // #4998: thread the requested execution lane through to the node.
          ...(params.lane ? { lane: params.lane } : {}),
          ...(params.timeoutSeconds ? { timeoutSeconds: params.timeoutSeconds } : {}),
          // #5471: a managed worktree (repoRef) takes precedence over an
          // existing worktree path; the two are mutually exclusive on the node.
          ...(params.repoRef
            ? { repoRef: params.repoRef }
            : params.worktreePath
              ? { worktreePath: params.worktreePath }
              : defaultWorktree !== null
                ? { worktreePath: defaultWorktree }
              : {}),
          // CS-A1: per-session provider account. The node resolves it against
          // its registry and rejects an unknown ref.
          ...(params.accountRef ? { accountRef: params.accountRef } : {}),
        })
        if (result.ok) sessionEventStreamer.watch(result.sessionRef)
        return result
      },
      // #5471: resolve a managed-worktree request (repo + base ref) to a
      // concrete repoRef the webview can pass to spawnSession. Bun runs git.
      async resolveManagedWorktree(params) {
        return resolveManagedWorktreeRepoRef({
          fullName: params.fullName,
          baseRef: params.baseRef,
          branch: params.branch,
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

// macOS needs a native Edit menu for WebKit text editing accelerators. Without
// these roles, Cmd-C/Cmd-V/Cmd-A can fall through to the system error beep
// instead of copy/paste/select-all inside the focused webview field.
ApplicationMenu.setApplicationMenu(desktopApplicationMenu)

const window = new BrowserWindow({
  title: "Autopilot",
  url: "views://autopilot-desktop/index.html",
  // Open large by default (Electrobun defaults to 800×600, too small for the
  // sidebar + dashboard). A roomy 1400×900 window with a small offset.
  frame: { x: 80, y: 60, width: 1400, height: 900 },
  rpc,
})

// #verse/mmo-characters-per-account: the per-instance Verse character.
//
// OA_CHARACTER is set on THIS Bun launcher process, but the renderer cannot see
// it: it is not VITE_-prefixed (so it is absent from the build-time
// `import.meta.env` define) and there is no `process.env` in the webview. So we
// resolve it here once and inject it as a global the renderer reads first
// (chatWorldCharacterId in src/shared/chat-world-flags.ts). Default "main" keeps
// a single instance identical to before. Two instances launched with
// OA_CHARACTER=main and OA_CHARACTER=alt become two distinct, mutually-visible
// avatars.
const verseCharacter = ((): string => {
  const trimmed = Bun.env.OA_CHARACTER?.trim()
  return trimmed !== undefined && trimmed.length > 0 ? trimmed : "main"
})()
console.log(`[autopilot-desktop] verse character: ${verseCharacter}`)
const injectVerseCharacter = (): void => {
  try {
    window.webview.executeJavascript(
      `globalThis.__OA_CHARACTER = ${JSON.stringify(verseCharacter)}`,
    )
  } catch {
    // Fail-soft: a missing webview/executeJavascript just leaves the renderer on
    // its env-fallback ("main"); we re-inject on dom-ready below regardless.
  }
}
// Belt-and-suspenders: inject as early as we can AND once the DOM is ready, so
// the value is present whether the renderer reads it before or after first
// paint. The renderer also resolves the character LAZILY (at joinRegion /
// setAvatarPosition time, well after this fires), so dom-ready timing cannot
// lose the value.
injectVerseCharacter()

// CL-30: each poll, fold the session list into the notifier so a session that
// newly enters a notify-worthy state (needs_decision / failed / completed)
// updates the in-app notification center.
//
// QUIET BY DEFAULT (zero-base directive, 2026-06-19): native OS notifications
// are OFF by default — no auto / cross-session notification spam while the owner
// is looking at the dead-simple shell. The in-app notification center still
// accumulates (it is a passive projection the hidden UI can show), but nothing
// pops a desktop banner unless the owner explicitly opts in with
// `OA_DESKTOP_OS_NOTIFICATIONS=1`. Flip the env var to restore OS banners.
const osNotificationsEnabled =
  process.env.OA_DESKTOP_OS_NOTIFICATIONS === "1"
const notifier = createSessionNotifier({
  raise: osNotificationsEnabled ? raiseOsNotification : () => {},
})

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
// under the user's chosen name. Until the user has chosen, auto-onboarding stays
// off so the app does not pre-claim the pylon or leak product env into a child
// before the identity choice.
function startManagedNodeSupervisor(): void {
  const firstRunChoice = resolveFirstRunLaunchChoice()
  managedNode = superviseManagedNode({
    cwd: process.cwd(),
    env: Bun.env,
    controlBaseUrl,
    // AO-3 (#5444): the chosen existing home to boot (null => fresh create-new
    // managed home) and the chosen create-new display name (null => neutral auto).
    useExistingHome: firstRunChoice.chosenExistingHome,
    onboardingDisplayName: firstRunChoice.chosenDisplayName,
    // #5027: in a packaged `.app` the dev repo entry is unreachable; the launcher
    // falls back to the bundled Pylon node under the app's Resources. In dev this
    // dir holds no `app/pylon-node/` bundle, so the dev path is still used.
    resourcesDir: PATHS.RESOURCES_FOLDER,
    // AO-1/AO-2 (#5442/#5443, EPIC #5441): once an identity is chosen, converge
    // to a registered, presence-live, payout-target-registered node. An explicit
    // operator OPENAGENTS_AGENT_TOKEN / PYLON_OPENAGENTS_BASE_URL is still
    // respected by the child-env builder.
    autoOnboarding: firstRunChoice.choiceMade,
    onStatus(status: NodeLaunchStatus) {
      console.log(
        `[autopilot-desktop] local node status: ${status}` +
          (managedNode?.home() ? ` (home: ${managedNode.home()})` : ""),
      )
      // #5025: surface the honest launch-lifecycle status as a webview badge.
      rpc.send.nodeLaunchStatus({ status })
    },
  })
}

startManagedNodeSupervisor()

let latestNodeState: NodeStateMessage | null = null
const pushNodeState = (rawMessage: NodeStateMessage): void => {
  // CS-A3 (#5363): persist this poll/stream event tail (keyed by sessionRef)
  // under the node home and merge the durable transcript back in, so a coding
  // session's transcript survives app/node restart and live stream reconnects.
  const message = persistAndMergeTranscripts(resolveHome(), rawMessage)
  latestNodeState = message
  rpc.send.nodeState(message)
  rpc.send.notifications(notifier.ingest(message.sessions))
  sessionEventStreamer.reconcile(message)
}

const mergeStreamedEvent = (
  sessionRef: string,
  event: Parameters<typeof mergeSessionEventRows>[1],
): void => {
  const current = latestNodeState ?? {
    ok: true,
    schema: "openagents.pylon.control.v0.3",
    sessions: [],
  }
  const nextEvents = { ...(current.events ?? {}) }
  nextEvents[sessionRef] = mergeSessionEventRows(nextEvents[sessionRef] ?? [], event)
  pushNodeState({ ...current, events: nextEvents })
}

const sessionEventStreamer = createSessionEventStreamer({
  baseUrl: controlBaseUrl,
  tokenProvider: acceptedControlTokenForCommand,
  onEvent(sessionRef, event) {
    mergeStreamedEvent(sessionRef, event)
  },
})

const poller = createNodeStatePoller({
  intervalMs: Number.isFinite(pollIntervalMs) && pollIntervalMs > 0 ? pollIntervalMs : 2000,
  onState(rawMessage) {
    pushNodeState(rawMessage)
  },
  async fetchNodeState() {
    // CL-45b: use the server-validated token so a stale token in an earlier
    // candidate home doesn't dead-end the poll at `control 401`. The resolver
    // caches the accepted token (re-validated with a cheap probe), so this stays
    // a bounded, side-effect-light call on the 2s cadence.
    const token = await acceptedControlTokenForCommand()
    if (token === null) throw new Error(CONTROL_TOKEN_NOT_ACCEPTED)
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
  injectVerseCharacter()
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
  sessionEventStreamer.stop()
  if (pylonStatsTimer !== undefined) clearInterval(pylonStatsTimer)
  if (autoUpdateTimer !== undefined) clearInterval(autoUpdateTimer)
  // Stop supervising and kill only a node we launched ourselves; an adopted
  // node we did not start is left running, and a deliberate stop never triggers
  // a restart.
  managedNode?.stop()
})
