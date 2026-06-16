#!/usr/bin/env bun

import { readFile } from "node:fs/promises"
import {
  PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF,
  declareTassadarExecutorCapability,
  mergeTassadarCapabilityRefs,
  writeTassadarCapabilityEvidence,
} from "./tassadar-capability"
import {
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  withClaudeAgentCapability,
} from "./claude-agent"
import {
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  withCodexAgentCapability,
} from "./codex-agent"
import { withWorkspaceMaterializerCapability } from "./workspace-materializer"
import { claimTipReadiness, readBalance, setTipPreferences, sweepStatus, tipPost } from "./tips"
import {
  ARTANIS_FORUM_SLUG,
  appendMemory,
  composeAskArtanisBody,
  forumPostTopic,
  forumReadTopic,
  forumReply,
  readMemories,
  resolveModelAdapter,
} from "./agent-surface"
import { Console, Deferred, Effect, PubSub } from "effect"
import { classifyServiceLogLevel, formatLogTimestamp, type PylonLogLevel } from "./node/state"
import {
  forkLogPersistence,
  forkNodeServices,
  logMessage,
  makePylonNodeRuntime,
  seedLogFeed,
  superviseLoop,
  type PylonNodeRuntime,
} from "./node/runtime"
import { createFeedLogWriter, readPersistedLogTail } from "./node/log-persist"
import {
  buildBrokerRegistrationBody,
  postNodeRegistration,
  type BrokerRegistrationHosts,
} from "./node/discovery-register"
import { createIntentQueue } from "./node/intent-intake"
import { createApprovalQueue } from "./node/approval-queue"
import { createCoordinatorRuntime, type CoordinatorRuntime } from "./coordinator/coordinator-runtime"
import { evaluateShipSpendGate } from "./coordinator/ship-spend-gate"
import {
  scanClaudeSessions,
  toEventRows,
  toSessionListEntry,
  type ExternalSession,
} from "./node/external-sessions"
import { scanCodexSessions } from "./node/codex-sessions"
import { homedir } from "node:os"
import {
  defaultControlPort,
  ensureControlToken,
  controlTokenPath,
  startControlServer,
  type ControlCommandActions,
} from "./node/control-server"
import { collectPylonAppleFmStatus } from "./node/apple-fm-status"
import { createControlSessionActions } from "./node/control-sessions"
import { resolveCloudControlConfig } from "./cloud-control-client"
import { makeCloudControlSessionExecutor } from "./openagents-cloud-provider"
import { collectPylonContextProjection } from "./context-projection"
import { collectPylonDevDoctor } from "./dev-doctor"
import {
  collectPylonAccountsList,
  collectPylonAccountsUsage,
  parsePylonAccountsUsageArgs,
  resolvePylonAccountUsageRefreshTargets,
  type PylonAccountsUsageArgs,
} from "./account-usage"
import {
  recordPylonDevCodexRun,
  runPylonDevApply,
  runPylonDevCheck,
  runPylonDevReload,
  type PylonDevCommandSpec,
} from "./dev-loop"
import {
  rejectCodexLocalDangerForPublicPath,
  runCodexComposerStream,
} from "./codex-composer"
import {
  rejectClaudeLocalDangerForPublicPath,
  runClaudeComposerStream,
} from "./claude-composer"
import { runProbeCli } from "../packages/runtime/src/index"
import {
  ControlEndpointError,
  runControlCommand,
} from "./node/control-cli"
import {
  PYLON_COMMAND_CATALOG,
  findCommandEntry,
  projectCommandCatalog,
} from "./cli-catalog"
import {
  activateTrainingWindow,
  admitTrainingEvidence,
  claimTrainingLease,
  closeoutTrainingWindow,
  planTrainingWindow,
  reconcileTrainingWindow,
  readTrainingStatus,
} from "./training-cockpit"
import {
  assertWorkloadFamily,
  parseTassadarWorkload,
  runValidatorAuto,
  submitReplayVerdict,
  submitTraceContribution,
} from "./tassadar-trace-client"
import { loadPinnedTassadarSelfTestWorkload } from "@openagentsinc/tassadar-executor/self-test"
import { readFile as readFileForPacket } from "node:fs/promises"
import {
  createBootstrapSummary,
  formatBootstrapText,
  parseBootstrapArgs,
  writeBootstrapFiles,
} from "./bootstrap"
import { ensurePylonLocalState, projectPublicStatus, writeRuntimeState, type PylonLocalState } from "./state"
import {
  completePylonLink,
  presenceClientOptionsFromEnv,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
} from "./presence"
import {
  admitPayoutTarget,
  classifyMdkWallet,
  classifySparkBackupReceive,
  prepareSparkBackupReceive,
  preflightLegacySparkMigration,
  readCachedSparkTarget,
  receiveWithFallback,
  receiveWithMdk,
  recommendSparkSweep,
  reportWalletReadiness,
  requestPayoutTargetAdmission,
  sendWithMdk,
  sweepSparkBackupToMdk,
  writeCachedSparkTarget,
} from "./wallet"
import {
  createSparkBackupHelper,
  legacySparkHelperRunner,
  resolveLegacySparkApiKey,
  resolveSparkBackupHelper,
} from "./spark-backup-helper"
import {
  acceptAssignment,
  pollAssignments,
  runNoSpendAssignment,
  submitAssignmentCloseout,
  submitAssignmentProgress,
  type AssignmentCloseout,
  type AssignmentProgress,
  type PylonAssignmentLease,
} from "./assignment"
import { discoverHostInventory } from "./inventory"
import {
  autoUpdateDisabledReason,
  checkForUpdate,
  downloadAndApply,
  resolveSelfBinaryPath,
} from "./self-update"
import { createOperatorSnapshot, formatOperatorSnapshotText } from "./operator"
import { inspectPsionicConnector } from "./psionic-connector"
import {
  installPsionicBinary,
  installPsionicModelArtifact,
} from "./psionic-install"
import {
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  policyFromEnv,
  relaysFromEnv,
  startNip90ProviderLoop,
} from "./provider-nip90"
import { PYLON_LABOR_CAPABILITY_REF, approveLaborFirstRun } from "./labor"
import {
  acceptPylonWorkOffer,
  createPylonWorkRequest,
  listPylonWorkOffers,
  readPylonAutopilotWorkEvents,
  readPylonAutopilotWorkStatus,
  readPylonWorkStatus,
  reviewPylonAutopilotWork,
  submitPylonAutopilotWork,
  workAcceptanceMemoryEntry,
  workRequestMemoryEntry,
} from "./work-requester"
import { hostname } from "node:os"

// Routes node lifecycle log lines from plain async call sites into the
// runtime feed once the headless node has booted.
let nodeRuntime: PylonNodeRuntime | null = null
// Quiet by default: verbose service chatter is hidden unless --verbose or
// PYLON_VERBOSE=1 (issue #4743).
let verboseMode = false

// Discovery heartbeat: opt-in (OA_DISCOVERY_BROKER), the node POSTs its
// reachable control address(es) + token to the discovery broker so the mobile
// app can auto-connect with no QR/paste ("shouldn't it auto detect"). The broker
// registry is in-memory, so we re-register on an interval to survive cold
// starts. Best-effort and unref'd — never blocks or holds the process open.
function startDiscoveryHeartbeat(opts: {
  controlPort: number
  controlToken: string
  boundHost: string
}): void {
  const broker = Bun.env.OA_DISCOVERY_BROKER
  if (!broker) return
  const owner = Bun.env.OA_DISCOVERY_OWNER ?? "chris"
  const nodeRef = Bun.env.PYLON_NODE_REF ?? hostname()
  // An externally-reachable HTTPS endpoint (e.g. `tailscale serve`): advertised
  // verbatim, reachable on any network the phone's tailnet covers, and ATS-safe.
  const publicUrl = Bun.env.OA_DISCOVERY_PUBLIC_URL
  const hosts: BrokerRegistrationHosts = {}
  if (opts.boundHost.startsWith("127.")) hosts.loopback = opts.boundHost
  else if (opts.boundHost.startsWith("100.")) hosts.tailnet = opts.boundHost
  else hosts.lan = opts.boundHost
  const beat = (): void => {
    void postNodeRegistration({
      brokerUrl: broker,
      ownerRef: owner,
      body: buildBrokerRegistrationBody({
        nodeRef,
        name: nodeRef,
        hosts,
        port: opts.controlPort,
        controlToken: opts.controlToken,
        updatedAt: new Date().toISOString(),
        ...(publicUrl ? { publicUrl } : {}),
      }),
    })
  }
  beat()
  const timer = setInterval(beat, 20_000)
  timer.unref?.()
}

// Routes a log message into the node runtime (which owns the feed and the
// event stream). Pre-boot or in plain CLI paths it falls back to stdout.
function logToUi(message: string, level: PylonLogLevel = "verbose") {
  if (nodeRuntime) {
    Effect.runFork(logMessage(nodeRuntime, level, message))
    return
  }
  if (level !== "verbose" || verboseMode) {
    void Effect.runPromise(Console.log(`[BOOT] ${message}`))
  }
}

// Effect-native logging helper (defaults to verbose — hidden unless
// --verbose / PYLON_VERBOSE=1)
const log = (message: string, level: PylonLogLevel = "verbose") =>
  Effect.sync(() => logToUi(message, level))

// Node-side wallet actions: the single execution path for money commands,
// used by the control server (attach mode) and the headless node
// (issue #4740).
const nodeWalletActions: ControlCommandActions = {
  walletSend: (destinationRef, amountSats) => sendWithMdk(destinationRef, amountSats),
  walletReceive: (amountSats) => receiveWithMdk(amountSats),
  walletAdmitPayoutTarget: (kind, ref) =>
    Promise.resolve(admitPayoutTarget({ kind: kind as Parameters<typeof admitPayoutTarget>[0]["kind"], ref })),
  // CL-23 read-only balance/earnings: project the live MDK wallet status into a
  // projection-safe subset (no offers/invoices/seed — just balance + readiness).
  walletStatus: async () => {
    const w = await classifyMdkWallet()
    return {
      configured: w.configured,
      daemonOnline: w.daemonOnline,
      balanceSats: w.balanceSats,
      receiveReady: w.receiveReady,
      sendReady: w.sendReady,
      readiness: w.readiness,
      blockerRefs: w.blockerRefs,
    }
  },
}

// CL-34/CL-35 work-intent intake: the phone composes an "ask" and submits it;
// the node enqueues it (server-generates the id + timestamp) for the coordinator
// to plan and fan out. Persisted to the Pylon home so intents survive restart.
function makeIntentActions(intentQueue: ReturnType<typeof createIntentQueue>) {
  return {
    submit: async (input: { title: string; body: string; scopeHint?: string; submittedByClientRef?: string }) => {
      const title = input.title.trim()
      const body = input.body.trim()
      if (title.length === 0) throw new Error("intent.submit requires a non-empty title")
      return intentQueue.enqueue({
        intentId: `intent.${crypto.randomUUID()}`,
        title,
        body,
        ...(input.scopeHint && input.scopeHint.trim().length > 0 ? { scopeHint: input.scopeHint.trim() } : {}),
        submittedByClientRef: input.submittedByClientRef?.trim() || "mobile",
        createdAt: new Date().toISOString(),
      })
    },
    list: async (sinceCursor?: string) => intentQueue.listSince(sinceCursor),
  }
}

// CL-16: a single shared approval queue for the node. The labor-market defers a
// job's first run for operator approval (labor_first_run_approval_required);
// that enqueues here, the client lists + resolves it, and an `approve` grants
// the real first-run approval so the job can proceed.
const approvalQueue = createApprovalQueue()

function makeApprovalActions(paths: BootstrapSummary["paths"]) {
  return {
    list: async () => ({ approvals: approvalQueue.list() }),
    resolve: async (input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }) => {
      const result = approvalQueue.resolve(input.approvalRef, input.decision, input.answer ? { answer: input.answer } : undefined)
      // Granting a labor first-run approval is the one side effect of "approve".
      if (result.applied && input.decision === "approve" && result.resolved?.jobType) {
        try {
          await approveLaborFirstRun({
            paths,
            approvedByRef: "operator.control",
            jobType: result.resolved.jobType as Parameters<typeof approveLaborFirstRun>[0]["jobType"],
            ...(result.resolved.policyRef ? { policyRef: result.resolved.policyRef } : {}),
          })
        } catch {
          // grant failure shouldn't crash the control path; the resolution stands
        }
      }
      return result
    },
  }
}

// CL-17 (rescoped): pause/resume autonomous coordinator work. The coordinator
// starts after the control server is created, so the action reads a mutable
// holder set once startCoordinator returns.
type CoordinatorHolder = { rt: CoordinatorRuntime | null }
function makeCoordinatorActions(holder: CoordinatorHolder) {
  return {
    pause: () => {
      holder.rt?.pause()
      return { paused: holder.rt?.isPaused() ?? false }
    },
    resume: () => {
      holder.rt?.resume()
      return { paused: holder.rt?.isPaused() ?? false }
    },
    status: () => ({ paused: holder.rt?.isPaused() ?? false }),
  }
}

// Labor-market deferrals feed the approval queue (the real pending source).
export function enqueueLaborApproval(input: { approvalRef: string; jobType?: string; policyRef?: string; prompt?: string }) {
  approvalQueue.enqueue({
    approvalRef: input.approvalRef,
    kind: "labor_first_run",
    prompt: input.prompt ?? `Approve first run of ${input.jobType ?? "labor job"}?`,
    ...(input.jobType ? { jobType: input.jobType } : {}),
    ...(input.policyRef ? { policyRef: input.policyRef } : {}),
  })
}

// #4951 external agent sessions: poll the host Claude Code logs and expose them
// as read-only sessions, merged into session.list / session.events so the MAIN
// conversation (Pylon-managed or not) + its sub-agents show in Autopilot.
function startExternalSessionTailer(): { list: () => ExternalSession[]; find: (ref: string) => ExternalSession | undefined } {
  let sessions: ExternalSession[] = []
  const projectsRoot = `${homedir()}/.claude/projects`
  const codexRoot = `${homedir()}/.codex/sessions`
  const poll = (): void => {
    const now = Date.now()
    let claude: ExternalSession[] = []
    let codex: ExternalSession[] = []
    try {
      claude = scanClaudeSessions({ projectsRoot, nowMs: now, maxAgeMs: 900_000, maxSessions: 12 })
    } catch {
      // best-effort
    }
    try {
      codex = scanCodexSessions({ sessionsRoot: codexRoot, nowMs: now, maxAgeMs: 900_000, maxSessions: 12 })
    } catch {
      // best-effort
    }
    sessions = [...claude, ...codex]
  }
  poll()
  const timer = setInterval(poll, 5000)
  timer.unref?.()
  return { list: () => sessions, find: (ref) => sessions.find((s) => s.sessionRef === ref) }
}

// Wrap the Pylon session actions so the control API also serves external
// sessions (read-only). spawn/cancel/artifact/eventStream stay Pylon-only.
function wrapSessionsWithExternal<T extends { list: () => Promise<any[]>; events: (ref: string) => Promise<any> }>(
  raw: T,
  store: ReturnType<typeof startExternalSessionTailer>,
): T {
  return {
    ...raw,
    list: async () => {
      const nowIso = new Date().toISOString()
      const pylon = await raw.list()
      const external = store.list().map((s) => toSessionListEntry(s, nowIso))
      return [...pylon, ...external]
    },
    events: async (ref: string) => {
      if (ref.startsWith("claude:") || ref.startsWith("codex:")) {
        const s = store.find(ref)
        if (s === undefined) throw new Error("external session not found")
        return {
          sessionRef: ref,
          eventsPath: "",
          state: s.state === "running" ? "running" : "completed",
          recentEvents: toEventRows(s),
        }
      }
      return raw.events(ref)
    },
  }
}

// CL-36 coordinator: wire the intent queue to the session executor so a
// submitted ask is planned + fanned out into coding sessions automatically.
// Each fan-out part runs in a fresh detached worktree off HEAD. Enabled unless
// OA_COORDINATOR=0. Spend note: this auto-runs coding agents on owner-composed
// asks (owner-authorized: the whole point of the loop).
function startCoordinator(
  intentQueue: ReturnType<typeof createIntentQueue>,
  sessions: { spawn: (cmd: any) => Promise<{ sessionRef: string }>; list: () => Promise<Array<{ sessionRef: string; state: string }>> },
): CoordinatorRuntime | null {
  if (Bun.env.OA_COORDINATOR === "0") return null
  const repoRoot = process.cwd()
  const runtime = createCoordinatorRuntime({
    intentQueue,
    spawnSession: async (input) => {
      const result = await sessions.spawn({
        type: "session.spawn",
        adapter: input.adapter,
        objective: input.objective,
        verify: input.verify,
        worktreePath: input.worktreePath,
      })
      return { sessionRef: result.sessionRef }
    },
    sessionState: async (ref) => {
      const list = await sessions.list()
      return list.find((s) => s.sessionRef === ref)?.state ?? null
    },
    createWorktree: async (intentId, index) => {
      const safe = intentId.replace(/[^a-zA-Z0-9._-]/g, "-")
      const dir = `/tmp/oa-coord/${safe}-${index}`
      await Bun.spawn(["git", "worktree", "remove", "--force", dir], { cwd: repoRoot, stderr: "ignore", stdout: "ignore" }).exited
      const proc = Bun.spawn(["git", "worktree", "add", "--detach", "--force", dir, "HEAD"], { cwd: repoRoot, stderr: "pipe", stdout: "ignore" })
      const code = await proc.exited
      if (code !== 0) throw new Error(`git worktree add failed (${code}) for ${dir}`)
      return dir
    },
    // CL-37/CL-41: supply the ship-step context. Fingerprints + changed paths
    // come from env (publish pipeline sets them); the spend gate is fail-safe —
    // with no configured budget (default 0) it DENIES, so an autonomous ship
    // escalates to the owner rather than spending without an explicit budget.
    shipContext: async () => {
      const env = Bun.env
      const num = (v: string | undefined, d: number) => {
        const n = Number(v)
        return Number.isFinite(n) ? n : d
      }
      const gate = evaluateShipSpendGate({
        action: "autonomous_ship",
        budget: {
          spentSats: num(env.OA_SHIP_SPENT_SATS, 0),
          budgetSats: num(env.OA_SHIP_BUDGET_SATS, 0),
          dailyCapSats: num(env.OA_SHIP_DAILY_CAP_SATS, 0),
          perShipCapSats: num(env.OA_SHIP_PER_SHIP_CAP_SATS, 0),
          shipCostSats: num(env.OA_SHIP_COST_SATS, 0),
          decidedAt: new Date().toISOString(),
        },
      })
      return {
        previousRuntimeFingerprint: (env.OA_SHIP_PREV_FINGERPRINT ?? "").trim(),
        nextRuntimeFingerprint: (env.OA_SHIP_NEXT_FINGERPRINT ?? "").trim(),
        changedPaths: (env.OA_SHIP_CHANGED_PATHS ?? "")
          .split(",")
          .map((p) => p.trim())
          .filter((p) => p.length > 0),
        spendGate: { decision: gate.decision },
      }
    },
    recordShip: (intentId, decision) => {
      logToUi(
        `[ship] ${intentId} mode=${decision.shipMode} decision=${decision.decision} eligible=${decision.eligible} (${decision.reason})`,
        "info",
      )
      // CL-38/CL-39: auto-execute the ship via OUR pipeline (no Expo/EAS cloud).
      // Triple-gated: eligible (spend-gate allowed) AND decision auto AND the
      // explicit opt-in OA_SHIP_AUTO_EXECUTE=1. Dormant by default — with no
      // budget the spend gate denies, so this never fires unexpectedly.
      if (!decision.eligible || decision.decision !== "auto") return
      if (Bun.env.OA_SHIP_AUTO_EXECUTE !== "1") {
        logToUi(`[ship] ${intentId} eligible for ${decision.shipMode} — auto-execute disabled (set OA_SHIP_AUTO_EXECUTE=1)`, "info")
        return
      }
      const repoRoot = process.cwd()
      if (decision.shipMode === "ota") {
        // CL-38: auto OTA publish to our updates server when OTA-eligible.
        logToUi(`[ship] ${intentId} auto OTA publish -> publish-ota.sh`, "info")
        Bun.spawn(["bash", "apps/oa-updates/scripts/publish-ota.sh"], { cwd: repoRoot, stdout: "ignore", stderr: "ignore" })
      } else if (decision.shipMode === "rebuild") {
        // CL-39: auto local build + Apple altool submit when a rebuild is needed
        // (no EAS). Requires the extra OA_SHIP_REBUILD_AUTO=1 since builds are heavy.
        if (Bun.env.OA_SHIP_REBUILD_AUTO !== "1") {
          logToUi(`[ship] ${intentId} rebuild needed — escalating (set OA_SHIP_REBUILD_AUTO=1 to auto-build locally)`, "info")
          return
        }
        logToUi(`[ship] ${intentId} auto local rebuild -> build-and-submit.sh`, "info")
        Bun.spawn(["bash", "clients/mobile/AutopilotRemoteControl/scripts/build-and-submit.sh"], { cwd: repoRoot, stdout: "ignore", stderr: "ignore" })
      }
    },
    log: (message) => logToUi(message, "info"),
  })
  runtime.start(5000)
  return runtime
}

// Node-side assignment actions (issue #4741). Available only when an
// OpenAgents base URL is configured; leases are cached between poll and
// accept so accept can resolve a leaseRef back to the full lease payload.
function makeAssignmentActions() {
  const baseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
  if (!baseUrl) return null
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const agentToken = Bun.env.OPENAGENTS_AGENT_TOKEN
  const clientOptions = { ...(agentToken ? { agentToken } : {}), baseUrl }
  let cachedLeases: PylonAssignmentLease[] = []
  return {
    poll: async () => {
      cachedLeases = await pollAssignments(summary, clientOptions)
      return cachedLeases.map((lease) => ({
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        goal: lease.goal,
        paymentMode: lease.paymentMode,
        expiresAt: lease.expiresAt,
      }))
    },
    accept: async (leaseRef: string) => {
      const lease = cachedLeases.find((candidate) => candidate.leaseRef === leaseRef)
      if (!lease) throw new Error("lease not found - refresh assignments first")
      return acceptAssignment(summary, lease, clientOptions)
    },
  }
}

function makeSessionActions(summary: ReturnType<typeof createBootstrapSummary>) {
  return createControlSessionActions({
    summary,
    // #4997: build the OpenAgents Cloud executor from env when a cloud control
    // plane is configured (OA_CLOUD_CONTROL_URL + OA_CLOUD_CONTROL_TOKEN). When
    // it is not, this returns null and cloud lanes degrade to the local
    // executor, so a Pylon with no cloud config still works locally as before.
    cloudExecutorFactory: (env) => {
      const resolved = resolveCloudControlConfig(env)
      if (!resolved.configured) return null
      return makeCloudControlSessionExecutor({ config: resolved.config, env })
    },
  })
}

function codexComposerWorkingDirectory() {
  const configured = Bun.env.PYLON_CODEX_CWD ?? Bun.env.PYLON_ACTIVE_REPO
  return configured && configured.trim().length > 0 ? configured : process.cwd()
}

async function runAccountsUsageRefresh(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: PylonAccountsUsageArgs,
) {
  const targets = await resolvePylonAccountUsageRefreshTargets(summary, options, { env: Bun.env })
  const prompt = "Reply with exactly: ok."
  for (const target of targets) {
    try {
      if (target.provider === "codex") {
        const config = await loadCodexAgentConfig(summary)
        await runCodexComposerStream(
          prompt,
          {
            account: target.account,
            approvalPolicy: "never",
            config,
            cwd: codexComposerWorkingDirectory(),
            env: Bun.env,
            executionMode: "local_bounded",
            ...(config.model === undefined ? {} : { model: config.model }),
            networkAccessEnabled: false,
            sandboxMode: "read-only",
            timeoutMs: 60_000,
            usageStateSummary: summary,
          },
        )
      } else {
        const config = await loadClaudeAgentConfig(summary)
        await runClaudeComposerStream(
          prompt,
          {
            account: target.account,
            config,
            cwd: codexComposerWorkingDirectory(),
            env: Bun.env,
            executionMode: "local_bounded",
            maxTurns: 1,
            ...(config.model === undefined ? {} : { model: config.model }),
            permissionMode: "acceptEdits",
            timeoutMs: 60_000,
            usageStateSummary: summary,
          },
        )
      }
    } catch {
      // Readiness and missing provider snapshots are reported in the final
      // JSON truth tiers; refresh failure must not leak raw provider errors.
    }
  }
}

const assignmentWorkerIntervalMs = () => {
  const seconds = Number(Bun.env.PYLON_ASSIGNMENT_WORKER_INTERVAL_SECONDS ?? 30)
  return Number.isFinite(seconds) && seconds >= 5 ? seconds * 1000 : 30_000
}

async function runHeadlessAssignmentWorkerLoop(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: { agentToken?: string; baseUrl: string },
  log: (message: string, level?: PylonLogLevel) => void,
) {
  const intervalMs = assignmentWorkerIntervalMs()
  log(`[Assignments] Headless worker loop enabled; polling every ${Math.round(intervalMs / 1000)}s.`)
  while (true) {
    try {
      const result = await runNoSpendAssignment(summary, options)
      if (result.ok) {
        log(`[Assignments] Completed no-spend assignment ${result.closeout.assignmentRef}.`)
      } else if (result.reason !== "no no-spend assignment lease available") {
        log(`[Assignments] No-spend run skipped: ${JSON.stringify(result)}`, "verbose")
      }
    } catch (error) {
      log(`[Assignments] Worker loop error: ${error instanceof Error ? error.message : String(error)}`, "info")
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
}

// Builds the operator-pane text the telemetry service publishes. The wallet
// projection here is the pre-seam placeholder (always offline) — wiring the
// live wallet state into the snapshot is follow-up work, kept behavior-equal
// for Phase 0.
function operatorTextFromInventory(inventory: Awaited<ReturnType<typeof discoverHostInventory>>) {
  const wallet = {
    schema: "openagents.pylon.wallet_status.v0.3" as const,
    configured: false,
    daemonOnline: false,
    balanceSats: null,
    receiveReady: false,
    sendReady: false,
    readiness: "daemon-offline" as const,
    blockerRefs: ["blocker.wallet.daemon_offline"],
    payoutTargetRefs: [],
    settlementRefs: [],
  }
  return formatOperatorSnapshotText(
    createOperatorSnapshot({ inventory, wallet: wallet as Parameters<typeof createOperatorSnapshot>[0]["wallet"] }),
  )
}

// Headless node-core (issue #4740): services + event stream + control API,
// no TUI, no Solid. Logs print to stdout with the same verbosity rules.
const runHeadlessNode = Effect.gen(function* () {
  verboseMode = Bun.argv.includes("--verbose") || Bun.env.PYLON_VERBOSE === "1"
  const runtime = yield* makePylonNodeRuntime
  nodeRuntime = runtime

  const shutdown = yield* Deferred.make<void>()
  const requestShutdown = () => {
    Effect.runFork(Deferred.succeed(shutdown, void 0))
    // GPU/native teardown (3D scene, WebGPU buffers) can wedge; never hold
    // the terminal hostage on exit.
    setTimeout(() => process.exit(0), 2000)
  }
  process.once("SIGINT", requestShutdown)
  process.once("SIGTERM", requestShutdown)

  // Stdout logger: live events only (the persisted tail is for attach
  // scrollback, not for replaying onto stdout).
  const subscription = yield* PubSub.subscribe(runtime.events)
  yield* Effect.forkScoped(
    Effect.gen(function* () {
      while (true) {
        const event = yield* PubSub.take(subscription)
        if (event.type === "log" && (event.level !== "verbose" || verboseMode)) {
          process.stdout.write(`[${formatLogTimestamp(event.at)}] ${event.message}\n`)
        }
      }
    }),
  )

  const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const persistedTail = yield* Effect.promise(() =>
    readPersistedLogTail(bootstrapSummary.paths.home, 300).catch(() => []),
  )
  if (persistedTail.length > 0) {
    yield* seedLogFeed(runtime, persistedTail)
  }
  const feedWriter = createFeedLogWriter(bootstrapSummary.paths.home, {
    onError: (message) => logToUi(`[FeedLog] Persistence disabled: ${message}`, "info"),
  })
  yield* forkLogPersistence(runtime, feedWriter)

  const controlToken = yield* Effect.promise(() => ensureControlToken(bootstrapSummary.paths.home))
  const controlPort = Number(Bun.env.PYLON_CONTROL_PORT ?? defaultControlPort)
  const headlessAssignmentActions = makeAssignmentActions()
  const headlessSessionActions = makeSessionActions(bootstrapSummary)
  const headlessExternalTailer = startExternalSessionTailer()
  const headlessSessionsWithExternal = wrapSessionsWithExternal(headlessSessionActions, headlessExternalTailer)
  const headlessIntentQueue = createIntentQueue({ persistPath: `${bootstrapSummary.paths.home}/intents.json` })
  const headlessCoordinatorHolder: CoordinatorHolder = { rt: null }
  const controlServer = yield* startControlServer(runtime, {
    token: controlToken,
    actions: {
      ...nodeWalletActions,
      ...(headlessAssignmentActions
        ? {
            assignmentsPoll: () => headlessAssignmentActions.poll(),
            assignmentsAccept: (leaseRef: string) => headlessAssignmentActions.accept(leaseRef),
          }
        : {}),
      sessions: headlessSessionsWithExternal,
      intents: makeIntentActions(headlessIntentQueue),
      accountsList: () => collectPylonAccountsList(bootstrapSummary),
      appleFmStatus: () => collectPylonAppleFmStatus({ summary: bootstrapSummary, env: Bun.env }),
      approvals: makeApprovalActions(bootstrapSummary.paths),
      coordinator: makeCoordinatorActions(headlessCoordinatorHolder),
    },
    port: controlPort,
    hostname: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
  })
  yield* logMessage(
    runtime,
    "info",
    `Pylon node-core running headless. Steer via the loopback control API at ${controlServer.url} (token: ${controlTokenPath(bootstrapSummary.paths.home)})`,
    { transient: true },
  )
  startDiscoveryHeartbeat({
    controlPort,
    controlToken,
    boundHost: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
  })
  // CL-36: close the self-driving loop on the headless node (the launchd path).
  headlessCoordinatorHolder.rt = startCoordinator(headlessIntentQueue, headlessSessionActions)

  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  yield* logMessage(runtime, "info", `[Identity] Pylon Nostr npub: ${localState.identity.npub}`, { transient: true })

  const presenceBaseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
  const presenceClientOptions = presenceClientOptionsFromEnv({
    baseUrl: presenceBaseUrl ?? "",
    env: Bun.env,
  })
  yield* forkNodeServices(runtime, {
    wallet: { classify: () => classifyMdkWallet() },
    telemetry: {
      discoverInventory: () => discoverHostInventory(),
      inspectPsionic: async () => {
        const connector = await inspectPsionicConnector({ env: Bun.env })
        return { phase: connector.phase }
      },
      makeOperatorText: operatorTextFromInventory,
    },
    heartbeat: {
      baseUrl: presenceBaseUrl,
      register: () => registerPylon(bootstrapSummary, presenceClientOptions),
      heartbeat: () => sendHeartbeat(bootstrapSummary, presenceClientOptions),
    },
  })
  if (presenceBaseUrl && Bun.env.PYLON_ASSIGNMENT_WORKER === "1") {
    yield* superviseLoop(
      runtime,
      "Assignments",
      Effect.tryPromise({
        try: () =>
          runHeadlessAssignmentWorkerLoop(
            bootstrapSummary,
            {
              ...(Bun.env.OPENAGENTS_AGENT_TOKEN ? { agentToken: Bun.env.OPENAGENTS_AGENT_TOKEN } : {}),
              baseUrl: presenceBaseUrl,
            },
            (message, level) => logToUi(message, level ?? classifyServiceLogLevel(message)),
          ),
        catch: (error) => new Error(`assignment worker loop failed: ${String(error)}`),
      }),
    )
  }
  yield* superviseLoop(
    runtime,
    "NIP-90",
    Effect.tryPromise({
      try: () =>
        startNip90ProviderLoop(bootstrapSummary, {
          log: (message) => logToUi(message, classifyServiceLogLevel(message)),
        }).then(() => undefined),
      catch: (error) => new Error(`NIP-90 provider loop failed: ${String(error)}`),
    }),
  )

  yield* Deferred.await(shutdown)
})

const runtimeCommandNamespaces = new Set([
  "apple-fm",
  "auth",
  "backend",
  "chat",
  "omega",
])

function parsePresenceOptions(args: string[]) {
  const options: Record<string, string> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const value = args[index + 1]
    if (!value || value.startsWith("--")) {
      throw new Error(`${arg} requires a value`)
    }
    options[arg.slice(2)] = value
    index += 1
  }
  return options
}

// Key/value option parser for user commands (forum/tip/work/wallet/balance/
// memories/ask-artanis). Boolean flags like `--json` map to `true` instead of
// erroring "requires a value" (#5038); use `optionString` to read string values.
function parseKeyValueOptions(args: string[]) {
  return parseCliOptions(args)
}

const presenceBootstrapValueOptions = new Set([
  "capability-ref",
  "display-name",
  "pylon-ref",
  "resource-mode",
])

const presenceBootstrapFlagOptions = new Set([
  "register-openagents",
  "setup-mdk-wallet",
])

async function persistedBootstrapArgs(env: NodeJS.ProcessEnv) {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), env)
  try {
    const config = JSON.parse(await readFile(summary.paths.config, "utf8")) as Record<string, unknown>
    const args: string[] = []
    const pushString = (key: string, flag: string) => {
      const value = config[key]
      if (typeof value === "string" && value.length > 0) {
        args.push(flag, value)
      }
    }

    pushString("pylonRef", "--pylon-ref")
    pushString("displayName", "--display-name")
    pushString("resourceMode", "--resource-mode")

    const capabilityRefs = config.capabilityRefs
    if (Array.isArray(capabilityRefs)) {
      for (const ref of capabilityRefs) {
        if (typeof ref === "string" && ref.length > 0) {
          args.push("--capability-ref", ref)
        }
      }
    }

    return args
  } catch {
    return []
  }
}

function parsePresenceBootstrapArgs(args: string[]) {
  const bootstrapArgs: string[] = []
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const key = arg.slice(2)
    if (key === "base-url") {
      index += 1
      continue
    }
    if (presenceBootstrapFlagOptions.has(key)) {
      bootstrapArgs.push(arg)
      continue
    }
    if (presenceBootstrapValueOptions.has(key)) {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`)
      }
      bootstrapArgs.push(arg, value)
      index += 1
      continue
    }
    throw new Error(`Unknown presence option: ${arg}`)
  }
  return bootstrapArgs
}

async function createPresenceBootstrapSummary(args: string[], env: NodeJS.ProcessEnv) {
  const argsFromConfig = await persistedBootstrapArgs(env)
  const argsFromPresence = parsePresenceBootstrapArgs(args)
  return createBootstrapSummary(
    parseBootstrapArgs(["--json", ...argsFromConfig, ...argsFromPresence]),
    env,
  )
}

function parsePsionicOptions(args: string[]) {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const key = arg.slice(2)
    const value = args[index + 1]
    if (!value || value.startsWith("--")) {
      options[key] = true
      continue
    }
    options[key] = value
    index += 1
  }
  return options
}

function stringPsionicOption(options: Record<string, string | true>, key: string) {
  const value = options[key]
  return typeof value === "string" ? value : undefined
}

/**
 * Build slice-1 `SparkBackupReceiveOptions` for the running node. Resolves the
 * real Breez SDK Spark helper (inert/null unless opt-in enabled + credential +
 * seed are present), reads the cached raw target from local private state, and
 * passes the `--show-local-target` flag through. Reads the local mnemonic only
 * to seed the SDK closure; the raw seed is NEVER returned or logged here.
 */
/**
 * Read the node's 12-word identity mnemonic from local private state, if it
 * exists. Returns null when the file is absent or unreadable. The raw seed is
 * used ONLY to seed the local Spark SDK closure and is NEVER logged, returned to
 * a projection, or emitted to stdout by callers.
 */
async function readIdentityMnemonicOrNull(state: PylonLocalState): Promise<string | null> {
  try {
    const { existsSync } = await import("node:fs")
    if (existsSync(state.paths.identityMnemonic)) {
      return (await readFile(state.paths.identityMnemonic, "utf8")).trim()
    }
  } catch {
    return null
  }
  return null
}

async function resolveSparkBackupOptions(
  state: PylonLocalState,
  input: { showLocalTarget?: boolean } = {},
) {
  const mnemonic = await readIdentityMnemonicOrNull(state)
  const helper = resolveSparkBackupHelper({
    env: Bun.env,
    mnemonic,
    storageDir: `${state.paths.home}/wallet/spark-backup/sdk`,
  })
  const cachedAddress = await readCachedSparkTarget(state.paths)
  return {
    env: Bun.env,
    ...(helper ? { helper } : {}),
    cachedAddress,
    showLocalTarget: input.showLocalTarget === true,
    // The embedded owner-authorized default Breez key is always compiled in, so
    // the receive backup is credential-ready out-of-box once opt-in is enabled
    // (#5078) — the status path must reflect that, like the helper resolver and
    // legacy-migration path already do.
    embeddedCredentialAvailable: true,
  }
}

function parseDevLoopOptions(args: string[]) {
  const options: { allowDirty: boolean; command?: string; json: boolean } = {
    allowDirty: false,
    json: false,
  }
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--json") {
      options.json = true
      continue
    }
    if (arg === "--allow-dirty") {
      options.allowDirty = true
      continue
    }
    if (arg === "--command") {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) throw new Error("--command requires a value")
      options.command = value
      index += 1
      continue
    }
    throw new Error(`unknown dev option: ${arg}`)
  }
  return options
}

function splitDevCommand(value: string): string[] {
  const argv: string[] = []
  let current = ""
  let quote: "'" | "\"" | null = null
  let escaped = false
  for (const char of value) {
    if (escaped) {
      current += char
      escaped = false
      continue
    }
    if (char === "\\") {
      escaped = true
      continue
    }
    if (quote) {
      if (char === quote) quote = null
      else current += char
      continue
    }
    if (char === "'" || char === "\"") {
      quote = char
      continue
    }
    if (/\s/.test(char)) {
      if (current.length > 0) {
        argv.push(current)
        current = ""
      }
      continue
    }
    current += char
  }
  if (escaped) current += "\\"
  if (quote) throw new Error("unterminated quote in --command")
  if (current.length > 0) argv.push(current)
  if (argv.length === 0) throw new Error("--command cannot be empty")
  return argv
}

function devCommandSpecFromOption(command: string | undefined, cwd: string): PylonDevCommandSpec[] | undefined {
  if (!command) return undefined
  return [
    {
      argv: splitDevCommand(command),
      cwd,
      reasonRef: "check.dev.custom_command",
    },
  ]
}

// Helper: parse `--key value` and `--flag` options out of an arg list. Returns
// a record where flags map to `true` and options to their string value.
function parseCliOptions(args: string[]): Record<string, string | true> {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const key = arg.slice(2)
    const value = args[index + 1]
    if (value === undefined || value.startsWith("--")) {
      options[key] = true
      continue
    }
    options[key] = value
    index += 1
  }
  return options
}

function optionString(options: Record<string, string | true>, key: string): string | undefined {
  const value = options[key]
  return typeof value === "string" ? value : undefined
}

// Emit a clean JSON error + nonzero exit on the steering control surfaces, so
// an agent always gets parseable output even when no node is running.
function emitControlError(command: string, error: unknown): void {
  const code = error instanceof ControlEndpointError ? error.code : "error"
  const message = error instanceof Error ? error.message : String(error)
  process.stdout.write(`${JSON.stringify({ ok: false, command, code, error: message }, null, 2)}\n`)
  process.exitCode = 1
}

function describeCheck(result: Awaited<ReturnType<typeof checkForUpdate>>): string {
  switch (result.status) {
    case "up-to-date":
      return `Pylon ${result.currentVersion} is up to date.`
    case "update-available":
      return `Update available: ${result.currentVersion} -> ${result.release.version}.`
    case "unsupported":
      return `Auto-update unsupported on ${result.reason}.`
    case "disabled":
      return `Auto-update disabled: ${result.reason}.`
  }
}

// Default-on startup OTA check. Runs before the headless node boots: if a newer
// signed release is available (and the operator has not opted out, and we are a
// compiled binary), download → verify against the pinned key → atomic-replace →
// relaunch the new binary in place. Any failure is swallowed so the node always
// falls back to running the current version — an update never blocks startup.
async function maybeAutoUpdate(): Promise<void> {
  try {
    const disabled = autoUpdateDisabledReason(Bun.env)
    if (disabled !== null) return
    const target = resolveSelfBinaryPath()
    if (target === null) return // dev / interpreter run — nothing to replace
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
    const state = await ensurePylonLocalState(summary)
    const result = await checkForUpdate({ clientId: state.identity.nodeId, env: Bun.env })
    if (result.status !== "update-available") return
    process.stderr.write(
      `Pylon: auto-updating ${result.currentVersion} -> ${result.release.version}...\n`,
    )
    const applied = await downloadAndApply({ release: result.release, targetPath: target })
    process.stderr.write(`Pylon: updated to ${applied.version}; relaunching.\n`)
    // Re-exec the freshly written binary with the same args, inheriting stdio so
    // a launchd/systemd/terminal supervisor stays attached, then exit with its code.
    const child = Bun.spawn([target, ...Bun.argv.slice(2)], {
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
      env: Bun.env,
    })
    const exitCode = await child.exited
    process.exit(exitCode)
  } catch (error) {
    process.stderr.write(
      `Pylon: auto-update skipped (${error instanceof Error ? error.message : String(error)}).\n`,
    )
  }
}

async function main() {
  const args = Bun.argv.slice(2)

  // `pylon help [--json]` and `pylon <cmd> --help` machine-readable catalog.
  if (args[0] === "help") {
    process.stdout.write(`${JSON.stringify(projectCommandCatalog(), null, 2)}\n`)
    return
  }
  if (args.includes("--help") && args[0] !== undefined && !args[0].startsWith("--")) {
    const entry = findCommandEntry(args[0])
    if (entry) {
      process.stdout.write(`${JSON.stringify(entry, null, 2)}\n`)
      return
    }
    process.stdout.write(
      `${JSON.stringify({ ok: false, error: `unknown command: ${args[0]}`, knownCommands: PYLON_COMMAND_CATALOG.map((c) => c.command) }, null, 2)}\n`,
    )
    process.exitCode = 1
    return
  }

  // CL-5035 sessions: first-class headless wrappers over the control-server
  // session verbs (session.list/spawn/cancel) the Autopilot desktop drives.
  if (args[0] === "sessions") {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === "list") {
        const { result } = await runControlCommand({ type: "session.list" }, Bun.env)
        process.stdout.write(`${JSON.stringify({ ok: true, sessions: result }, null, 2)}\n`)
        return
      }
      if (command === "spawn") {
        const adapter = optionString(options, "adapter")
        const objective = optionString(options, "objective")
        if (adapter !== "codex" && adapter !== "claude_agent") {
          throw new Error("sessions spawn --adapter must be codex or claude_agent")
        }
        if (!objective || objective.trim().length === 0) {
          throw new Error("sessions spawn requires --objective")
        }
        const verify = (() => {
          const raw = options.verify
          if (typeof raw !== "string" || raw.trim().length === 0) return []
          // The control protocol's verify is an argv array (e.g. ["test","-f",
          // "x"]); a `--verify "test -f x"` string must be tokenized, else the
          // node tries to exec a single program literally named "test -f x" and
          // the verify errors (spawn failure) even when the work succeeded.
          return raw.trim().split(/\s+/)
        })()
        const worktree = optionString(options, "worktree")
        const { result } = await runControlCommand(
          {
            type: "session.spawn",
            adapter,
            objective,
            verify,
            ...(worktree ? { worktreePath: worktree } : {}),
          },
          Bun.env,
        )
        process.stdout.write(`${JSON.stringify({ ok: true, session: result }, null, 2)}\n`)
        return
      }
      if (command === "cancel") {
        const sessionRef = optionString(options, "session-ref") ?? (args[2] && !args[2].startsWith("--") ? args[2] : undefined)
        if (!sessionRef) throw new Error("sessions cancel requires --session-ref <ref>")
        const { result } = await runControlCommand({ type: "session.cancel", sessionRef }, Bun.env)
        process.stdout.write(`${JSON.stringify({ ok: true, session: result }, null, 2)}\n`)
        return
      }
      throw new Error("usage: pylon sessions list|spawn|cancel ...")
    } catch (error) {
      emitControlError("sessions", error)
      return
    }
  }

  // CL-5035 approvals: list + resolve the node's operator approval queue.
  if (args[0] === "approvals") {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === "list") {
        const { result } = await runControlCommand({ type: "approvals.list" }, Bun.env)
        process.stdout.write(`${JSON.stringify({ ok: true, ...(result as Record<string, unknown>) }, null, 2)}\n`)
        return
      }
      if (command === "approve" || command === "deny" || command === "answer") {
        const approvalRef = optionString(options, "approval-ref") ?? (args[2] && !args[2].startsWith("--") ? args[2] : undefined)
        if (!approvalRef) throw new Error(`approvals ${command} requires --approval-ref <ref>`)
        const answer = optionString(options, "answer")
        if (command === "answer" && !answer) throw new Error("approvals answer requires --answer <text>")
        const { result } = await runControlCommand(
          {
            type: "approvals.resolve",
            approvalRef,
            decision: command,
            ...(answer ? { answer } : {}),
          },
          Bun.env,
        )
        process.stdout.write(`${JSON.stringify({ ok: true, resolution: result }, null, 2)}\n`)
        return
      }
      throw new Error("usage: pylon approvals list|approve|deny [--approval-ref <ref>]")
    } catch (error) {
      emitControlError("approvals", error)
      return
    }
  }

  // CL-5035 deploy: surface the gated node deploy-cloud action as a CLI verb.
  // Execution stays gated on the node behind OA_DEPLOY_ENABLE=1 (fail-safe);
  // this verb only forwards the request to the running node.
  if (args[0] === "deploy") {
    const command = args[1] ?? "status"
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === "status") {
        const { result } = await runControlCommand({ type: "deploy.status" }, Bun.env)
        process.stdout.write(`${JSON.stringify({ ok: true, deploy: result }, null, 2)}\n`)
        return
      }
      if (command === "cloud") {
        const target = optionString(options, "target")
        const ref = optionString(options, "ref")
        if (!target || !ref) throw new Error("deploy cloud requires --target and --ref")
        const env = optionString(options, "env")
        const { result } = await runControlCommand(
          { type: "deploy.cloud", target, ref, ...(env ? { env } : {}) },
          Bun.env,
        )
        const accepted = (result as { accepted?: boolean } | null)?.accepted === true
        process.stdout.write(`${JSON.stringify({ ok: accepted, deploy: result }, null, 2)}\n`)
        if (!accepted) process.exitCode = 1
        return
      }
      throw new Error("usage: pylon deploy cloud --target T --ref R [--env E] | pylon deploy status")
    } catch (error) {
      emitControlError("deploy", error)
      return
    }
  }

  // CL-5035 training: mirror the desktop training cockpit verbs against the
  // openagents.com training HTTP API (admin verbs need an admin token).
  if (args[0] === "training") {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error("training commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      const adminToken = optionString(options, "admin-token") ?? Bun.env.OA_TRAINING_ADMIN_TOKEN
      const net = { baseUrl, ...(adminToken ? { adminToken } : {}) }
      let result: unknown
      if (command === "plan") {
        result = await planTrainingWindow(net)
      } else if (command === "activate") {
        const windowRef = optionString(options, "window-ref")
        if (!windowRef) throw new Error("training activate requires --window-ref")
        result = await activateTrainingWindow(net, windowRef)
      } else if (command === "reconcile") {
        const windowRef = optionString(options, "window-ref")
        if (!windowRef) throw new Error("training reconcile requires --window-ref")
        result = await reconcileTrainingWindow(net, windowRef)
      } else if (command === "closeout") {
        const windowRef = optionString(options, "window-ref")
        if (!windowRef) throw new Error("training closeout requires --window-ref")
        result = await closeoutTrainingWindow(net, windowRef)
      } else if (command === "claim") {
        const pylonRef =
          optionString(options, "pylon-ref") ??
          (await ensurePylonLocalState(createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env))).identity.pylonRef
        const leaseSecondsRaw = optionString(options, "lease-seconds")
        const leaseSeconds = leaseSecondsRaw === undefined ? undefined : Number(leaseSecondsRaw)
        result = await claimTrainingLease(net, {
          pylonRef,
          ...(leaseSeconds !== undefined && Number.isFinite(leaseSeconds) ? { leaseSeconds } : {}),
        })
      } else if (command === "admit") {
        const runRef = optionString(options, "run-ref")
        if (!runRef) throw new Error("training admit requires --run-ref")
        const packetPath = optionString(options, "packet")
        if (!packetPath) throw new Error("training admit requires --packet <evidence-packet.json>")
        const packet = JSON.parse(await readFileForPacket(packetPath, "utf8")) as unknown
        result = await admitTrainingEvidence(net, { trainingRunRef: runRef, packet })
      } else if (command === "status") {
        result = await readTrainingStatus(net)
      } else if (command === "submit-trace" || command === "validate") {
        // #5054 (epic #5051), design §4.5: contributor-callable worker/validator
        // verbs. These hit the agent-gated trace-submission / replay-verdict
        // routes (#5052) — agent token, NOT admin. They run the dispatched
        // workload locally (reuse executeTassadarNumericModel) and submit the
        // worker trace commitment / validator replay digest. Client-only: no
        // wallet, settlement, or payout authority, and running them does not
        // change default node behavior (participating is opt-in by invoking
        // the verb; the background assignment worker stays PYLON_ASSIGNMENT_WORKER
        // gated and OFF by default until #5061).
        const agentToken = optionString(options, "agent-token") ?? Bun.env.OPENAGENTS_AGENT_TOKEN
        const traceNet = { baseUrl, ...(agentToken ? { agentToken } : {}) }
        const resolveDeviceRef = async (): Promise<string> =>
          optionString(options, "device-ref") ??
          (await ensurePylonLocalState(createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env))).identity.nodeId

        // #5121: opt-in validator AUTO-RUN. `pylon training validate --auto`
        // discovers the next pending worker contribution from a DISTINCT device
        // (GET /api/training/contributions/next-unpaired), replays the committed
        // pinned fixture, and submits the verdict — no manual --lease-ref/--workload.
        // `--watch` loops until a pairing (or --max-iterations), sleeping
        // --interval-ms between idle polls. Single-shot otherwise.
        if (command === "validate" && options.auto !== undefined) {
          const validatorDeviceRef = await resolveDeviceRef()
          const workload = parseTassadarWorkload(loadPinnedTassadarSelfTestWorkload())
          const runRef = optionString(options, "run-ref")
          const watch = options.watch !== undefined
          const intervalRaw = Number(optionString(options, "interval-ms") ?? "15000")
          const intervalMs = Number.isFinite(intervalRaw) && intervalRaw > 0 ? intervalRaw : 15000
          const maxRaw = Number(optionString(options, "max-iterations") ?? (watch ? "0" : "1"))
          const maxIterations = Number.isFinite(maxRaw) && maxRaw > 0 ? maxRaw : watch ? 0 : 1
          let iterations = 0
          let last: Record<string, unknown> = { ok: true, paired: false, reason: "idle_no_pending" }
          for (;;) {
            iterations += 1
            last = await runValidatorAuto(traceNet, {
              validatorDeviceRef,
              workload,
              ...(runRef ? { trainingRunRef: runRef } : {}),
            })
            const paired = (last as { paired?: boolean }).paired === true
            const failed = (last as { ok?: boolean }).ok === false
            if (!watch || paired || failed) break
            if (maxIterations > 0 && iterations >= maxIterations) break
            await new Promise((resolve) => setTimeout(resolve, intervalMs))
          }
          result = { ...last, iterations, mode: "validate_auto" }
        } else {
          const leaseRef = optionString(options, "lease-ref")
          if (!leaseRef) throw new Error(`training ${command} requires --lease-ref`)
          const workloadFamily = assertWorkloadFamily(optionString(options, "workload-family"))
          const workloadPath = optionString(options, "workload")
          if (!workloadPath) {
            throw new Error(
              command === "validate"
                ? `training validate requires --workload <dispatch.json>, or use 'validate --auto' for auto-discovery (#5121)`
                : `training submit-trace requires --workload <dispatch.json>`,
            )
          }
          const workload = parseTassadarWorkload(
            JSON.parse(await readFileForPacket(workloadPath, "utf8")) as unknown,
          )
          const deviceRef = await resolveDeviceRef()
          if (command === "submit-trace") {
            result = await submitTraceContribution(traceNet, {
              leaseRef,
              pylonDeviceRef: deviceRef,
              workload,
              workloadFamily,
              ...(optionString(options, "assignment-ref") ? { assignmentRef: optionString(options, "assignment-ref")! } : {}),
            })
          } else {
            result = await submitReplayVerdict(traceNet, {
              leaseRef,
              validatorDeviceRef: deviceRef,
              workload,
              workloadFamily,
            })
          }
        }
      } else {
        throw new Error("usage: pylon training plan|activate|claim|admit|reconcile|closeout|status|submit-trace|validate [--auto [--watch]] ...")
      }
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
      const okFlag = (result as { ok?: boolean } | null)?.ok
      if (okFlag === false) process.exitCode = 1
      return
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ ok: false, command: "training", error: error instanceof Error ? error.message : String(error) }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "bootstrap") {
    try {
      const options = parseBootstrapArgs(args.slice(1))
      const summary = createBootstrapSummary(options, Bun.env)
      if (!summary.platform.supported) {
        process.stderr.write(
          `Pylon ${summary.version} supports macOS and Linux only. Current platform: ${summary.platform.current}\n`,
        )
        process.exitCode = 1
        return
      }

      await writeBootstrapFiles(summary)
      const state = await ensurePylonLocalState(summary)
      const output = options.json ? { ...summary, localState: projectPublicStatus(state).state } : formatBootstrapText(summary)
      process.stdout.write(typeof output === "string" ? output : `${JSON.stringify(output, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon bootstrap failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "status" && args.includes("--json")) {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
    const state = await ensurePylonLocalState(summary)
    const inventory = await discoverHostInventory({ env: Bun.env })
    const psionicConnector = await inspectPsionicConnector({ env: Bun.env })
    process.stdout.write(`${JSON.stringify(projectPublicStatus(state, inventory, psionicConnector), null, 2)}\n`)
    return
  }

  if (args[0] === "inventory" && args.includes("--json")) {
    const inventory = await discoverHostInventory({ env: Bun.env })
    process.stdout.write(`${JSON.stringify(inventory, null, 2)}\n`)
    return
  }

  // `pylon update` — manual trigger for the same default-on OTA self-updater.
  //   --check  report only, don't apply
  //   --json   machine-readable result
  //   --channel <rc|stable>  override the channel (default rc)
  //   --feed-base <url>      override the feed origin (testing)
  if (args[0] === "update") {
    const options = parseKeyValueOptions(args.slice(1))
    const json = options.json === true
    const checkOnly = options.check === true
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      const result = await checkForUpdate({
        clientId: state.identity.nodeId,
        env: Bun.env,
        ...(optionString(options, "channel") ? { channel: optionString(options, "channel") } : {}),
        ...(optionString(options, "feed-base") ? { feedBase: optionString(options, "feed-base") } : {}),
      })

      if (result.status !== "update-available" || checkOnly) {
        const payload = { ...result, applied: false as const }
        process.stdout.write(json ? `${JSON.stringify(payload, null, 2)}\n` : `${describeCheck(result)}\n`)
        return
      }

      const target = resolveSelfBinaryPath()
      if (target === null) {
        const payload = { status: "dev-noop" as const, currentVersion: result.currentVersion, candidate: result.release.version, applied: false as const }
        process.stdout.write(json ? `${JSON.stringify(payload, null, 2)}\n` : `Update ${result.release.version} available; running from source (no binary to replace).\n`)
        return
      }

      const applied = await downloadAndApply({ release: result.release, targetPath: target })
      const payload = { status: "updated" as const, fromVersion: result.currentVersion, toVersion: applied.version, targetPath: applied.targetPath, applied: true as const }
      process.stdout.write(json ? `${JSON.stringify(payload, null, 2)}\n` : `Updated ${result.currentVersion} -> ${applied.version}. Restart pylon to run the new version.\n`)
      return
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (json) process.stdout.write(`${JSON.stringify({ status: "error", error: message, applied: false }, null, 2)}\n`)
      else process.stderr.write(`Pylon update failed: ${message}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "accounts") {
    try {
      const command = args[1]
      if (command === "list") {
        if (!args.includes("--json")) throw new Error("usage: pylon accounts list --json")
        const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
        const projection = await collectPylonAccountsList(summary, { env: Bun.env })
        process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
        return
      }
      if (command === "usage") {
        const options = parsePylonAccountsUsageArgs(args.slice(2))
        if (!options.json) {
          throw new Error("usage: pylon accounts usage [--account <ref-or-provider>|--provider <codex|claude_agent>|--all] [--refresh] --json")
        }
        const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
        if (options.refresh) {
          // The expensive refresh contract is intentionally opt-in. The
          // current SDK stream exposes local session usage and captures
          // provider snapshots when the underlying event stream includes
          // Codex/Claude rate-limit payloads.
          await runAccountsUsageRefresh(summary, options)
        }
        const projection = await collectPylonAccountsUsage(summary, options, { env: Bun.env })
        process.stdout.write(`${JSON.stringify({
          ...projection,
          refresh: {
            ...projection.refresh,
            performed: options.refresh,
          },
        }, null, 2)}\n`)
        return
      }
      throw new Error("usage: pylon accounts list|usage ...")
    } catch (error) {
      process.stderr.write(`Pylon accounts failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "context") {
    try {
      if (!args.includes("--json")) throw new Error("usage: pylon context --json [--codex-danger]")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const projection = await collectPylonContextProjection({
        cwd: codexComposerWorkingDirectory(),
        dangerFlag: args.includes("--codex-danger"),
        env: Bun.env,
        summary,
      })
      process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon context failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "operator" && args[1] === "snapshot" && args.includes("--json")) {
    const inventory = await discoverHostInventory({ env: Bun.env })
    const wallet = await classifyMdkWallet()
    const snapshot = createOperatorSnapshot({ inventory, wallet })
    process.stdout.write(`${JSON.stringify(snapshot, null, 2)}\n`)
    return
  }

  if (args[0] === "psionic") {
    try {
      const command = args[1]
      if (command === "doctor") {
        const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "doctor", ...args.slice(2)], { env: Bun.env }))
        if (result.stdout) process.stdout.write(result.stdout)
        if (result.stderr) process.stderr.write(result.stderr)
        process.exitCode = result.exitCode
        return
      }

      if (command === "smoke") {
        const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "smoke", ...args.slice(2)], { env: Bun.env }))
        if (result.stdout) process.stdout.write(result.stdout)
        if (result.stderr) process.stderr.write(result.stderr)
        process.exitCode = result.exitCode
        return
      }

      const options = parsePsionicOptions(args.slice(command === "models" ? 4 : 2))
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      if (command === "install") {
        const result = await installPsionicBinary(summary, {
          channel: stringPsionicOption(options, "channel") ?? "rc",
          manifestUrl: stringPsionicOption(options, "manifest-url"),
          consent: options.yes === true,
          env: Bun.env,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "installed" ? 0 : 1
        return
      }

      if (command === "models" && args[2] === "install") {
        const modelKey = args[3]
        const result = await installPsionicModelArtifact(summary, {
          modelKey,
          manifestUrl: stringPsionicOption(options, "manifest-url"),
          consent: options.yes === true,
          env: Bun.env,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "installed" ? 0 : 1
        return
      }

      throw new Error(`unknown psionic command: ${args.slice(1).join(" ")}`)
    } catch (error) {
      process.stderr.write(`Pylon Psionic installer failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "presence") {
    try {
      const command = args[1]
      const presenceArgs = args.slice(2)
      const options = parsePresenceOptions(presenceArgs)
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("presence commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = await createPresenceBootstrapSummary(presenceArgs, Bun.env)
      const clientOptions = presenceClientOptionsFromEnv({ baseUrl, env: Bun.env })
      const result =
        command === "register"
          ? await registerPylon(summary, clientOptions)
          : command === "heartbeat"
            ? await sendHeartbeat(summary, clientOptions)
            : command === "link-complete"
              ? await completePylonLink(summary, clientOptions)
              : command === "link-refresh"
                ? await refreshPylonLink(summary, clientOptions)
                : null
      if (!result) throw new Error(`unknown presence command: ${command ?? ""}`)
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon presence failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "forum" || args[0] === "memories" || args[0] === "ask-artanis") {
    try {
      const surfaceArgs = args[0] === "ask-artanis" ? args.slice(2) : args.slice(args[0] === "forum" ? 2 : 1)
      const options = parseKeyValueOptions(surfaceArgs)
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl: baseUrl ?? "https://openagents.com",
      }
      if (args[0] === "memories") {
        const entries = await readMemories(summary.paths.home)
        process.stdout.write(`${JSON.stringify({ count: entries.length, memories: entries }, null, 2)}\n`)
        return
      }
      if (args[0] === "forum") {
        const sub = args[1]
        if (sub === "read") {
          const topicId = args[2]
          if (!topicId) throw new Error("usage: pylon forum read <topic-id>")
          const topic = await forumReadTopic(networkOptions, topicId)
          process.stdout.write(`${JSON.stringify(topic, null, 2)}\n`)
          return
        }
        if (sub === "post") {
          const forumSlug = stringPsionicOption(options, "forum") ?? ARTANIS_FORUM_SLUG
          const title = stringPsionicOption(options, "title")
          const body = stringPsionicOption(options, "body")
          if (!title || !body) throw new Error("usage: pylon forum post --title T --body B [--forum slug]")
          const result = await forumPostTopic(networkOptions, { bodyText: body, forumSlug, title })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: "forum_post",
            refs: { topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null },
            summary: `posted forum topic: ${title.slice(0, 80)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        if (sub === "reply") {
          const topicId = args[2]
          const body = stringPsionicOption(options, "body")
          if (!topicId || !body) throw new Error("usage: pylon forum reply <topic-id> --body B")
          const result = await forumReply(networkOptions, { bodyText: body, topicId })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: "forum_reply",
            refs: { topicId },
            summary: `replied in topic ${topicId.slice(0, 12)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        throw new Error("usage: pylon forum post|read|reply ...")
      }
      // ask-artanis
      const question = args[1]
      if (!question || question.startsWith("--")) {
        throw new Error('usage: pylon ask-artanis "your question" [--base-url URL]')
      }
      const inventory = await discoverHostInventory({ env: Bun.env })
      const memories = await readMemories(summary.paths.home, 10)
      const adapter = resolveModelAdapter(Bun.env)
      const composed = await composeAskArtanisBody(
        {
          deviceContext: {
            backends: (inventory as { backends?: unknown }).backends ?? null,
            capabilityRefs: state.runtime.capabilityRefs,
            platform: (inventory as { platform?: unknown }).platform ?? null,
            pylonRef: state.identity.pylonRef,
          },
          memories,
          pylonRef: state.identity.pylonRef,
          question,
        },
        adapter,
      )
      const title = `Pylon device question: ${question.slice(0, 80)}`
      const result = await forumPostTopic(networkOptions, {
        bodyText: composed.bodyText,
        forumSlug: stringPsionicOption(options, "forum") ?? ARTANIS_FORUM_SLUG,
        title,
      })
      await appendMemory(summary.paths.home, {
        at: new Date().toISOString(),
        kind: "ask_artanis",
        refs: { composedBy: composed.composedBy, topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null },
        summary: `asked artanis: ${question.slice(0, 80)}`,
      })
      process.stdout.write(`${JSON.stringify({ composedBy: composed.composedBy, result }, null, 2)}\n`)
      return
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "tip" || args[0] === "balance" || args[0] === "sweep-status" || args[0] === "tip-prefs" || args[0] === "claim-tip-readiness") {
    try {
      const tipArgs = args[0] === "tip" ? args.slice(3) : args.slice(1)
      const options = parseKeyValueOptions(tipArgs)
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error(`${args[0]} requires --base-url or PYLON_OPENAGENTS_BASE_URL`)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }
      if (args[0] === "tip") {
        const postId = args[1]
        const amountSat = Number(args[2])
        if (!postId || !Number.isInteger(amountSat) || amountSat <= 0) {
          throw new Error("usage: pylon tip <post-id> <sats> [--base-url URL]")
        }
        const result = await tipPost(networkOptions, { amountSat, postId })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "balance") {
        const result = await readBalance(networkOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "sweep-status") {
        const result = await sweepStatus(networkOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "tip-prefs") {
        const prefs: Record<string, number | boolean> = {}
        if (options["sweep-enabled"] !== undefined) prefs.sweepEnabled = options["sweep-enabled"] === "true"
        if (options["sweep-threshold"] !== undefined) prefs.sweepThresholdSat = Number(options["sweep-threshold"])
        if (options["send-credits-below"] !== undefined) prefs.sendCreditsBelowSat = Number(options["send-credits-below"])
        if (options["receive-credits-below"] !== undefined) prefs.receiveCreditsBelowSat = Number(options["receive-credits-below"])
        const result = await setTipPreferences(networkOptions, prefs)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      const result = await claimTipReadiness(networkOptions, { pylonRef: state.identity.pylonRef })
      process.stdout.write(`${JSON.stringify({ claimed: true, tipRecipientReadiness: (result as { tipRecipientReadiness?: unknown }).tipRecipientReadiness ?? null }, null, 2)}\n`)
      return
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "dev" && args[1] === "doctor") {
    try {
      if (!args.includes("--json")) {
        throw new Error("usage: pylon dev doctor --json [--codex-danger] [--claude-danger]")
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const projection = await collectPylonDevDoctor({
        claudeDangerFlag: args.includes("--claude-danger"),
        cwd: process.cwd(),
        dangerFlag: args.includes("--codex-danger"),
        env: Bun.env,
        summary,
      })
      process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon dev doctor failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "dev" && (args[1] === "check" || args[1] === "apply" || args[1] === "reload")) {
    try {
      const command = args[1]
      const options = parseDevLoopOptions(args.slice(2))
      if (!options.json) {
        throw new Error(`usage: pylon dev ${command} --json [--allow-dirty]${command === "check" ? " [--command <argv>]" : ""}`)
      }
      if (options.command && command !== "check") {
        throw new Error("--command is only supported for pylon dev check")
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const cwd = codexComposerWorkingDirectory()
      const result =
        command === "check"
          ? await runPylonDevCheck({
              allowDirty: options.allowDirty,
              commands: devCommandSpecFromOption(options.command, cwd),
              cwd,
              env: Bun.env,
              summary,
            })
          : command === "apply"
            ? await runPylonDevApply({
                allowDirty: options.allowDirty,
                cwd,
                env: Bun.env,
                summary,
              })
            : await runPylonDevReload({
                cwd,
                env: Bun.env,
                summary,
              })
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
      if (result.state === "blocked" || result.state === "failed") process.exitCode = 1
      return
    } catch (error) {
      process.stderr.write(`Pylon dev ${args[1]} failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "work") {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), "pylon work")
      rejectClaudeLocalDangerForPublicPath(args.slice(1), "pylon work")
      const command = args[1]
      const workArgs = args.slice(2).flatMap((arg) => (arg === "--events" ? ["--events", "true"] : [arg]))
      const options = parseKeyValueOptions(workArgs)
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error("work commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }

      if (command === "submit") {
        const objective = args[2]
        const budgetCents = Number(options["budget-cents"] ?? options.budget ?? 0)
        if (!objective || objective.startsWith("--") || !Number.isInteger(budgetCents) || budgetCents < 0) {
          throw new Error('usage: pylon work submit "<objective>" --commit <40-char-sha> [--adapter codex|claude_agent|fable] [--budget-cents <cents>] [--repo owner/repo] [--branch main] [--verify "bun test"]')
        }
        const adapter = options.adapter
        if (
          adapter !== undefined &&
          adapter !== "codex" &&
          adapter !== "claude_agent" &&
          adapter !== "fable"
        ) {
          throw new Error("work submit --adapter must be codex, claude_agent, or fable")
        }
        const result = await submitPylonAutopilotWork(networkOptions, {
          ...(adapter === undefined ? {} : { adapter }),
          branch: options.branch,
          budgetCents,
          commit: options.commit,
          objective,
          repository: options.repo,
          verificationCommand: options.verify,
        })
        await appendMemory(summary.paths.home, {
          at: new Date().toISOString(),
          kind: "autopilot_work_submit",
          refs: {
            state: (result.work as { state?: unknown } | undefined)?.state ?? null,
            workOrderRef: (result.work as { workOrderRef?: unknown } | undefined)?.workOrderRef ?? null,
          },
          summary: `submitted autopilot work ${String((result.work as { workOrderRef?: unknown } | undefined)?.workOrderRef ?? "unknown").slice(0, 48)}`,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "request") {
        const objective = args[2]
        const budgetSats = Number(options.budget)
        if (!objective || objective.startsWith("--") || !Number.isInteger(budgetSats) || budgetSats <= 0) {
          throw new Error('usage: pylon work request "<objective>" --budget <sats> [--repo URL] [--verify command] [--deadline iso]')
        }
        const result = await createPylonWorkRequest(networkOptions, {
          budgetSats,
          deadline: options.deadline,
          objective,
          repository: options.repo,
          verificationCommand: options.verify,
        })
        await appendMemory(summary.paths.home, workRequestMemoryEntry({
          at: new Date().toISOString(),
          result,
        }))
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "offers") {
        const requestRef = args[2]
        if (!requestRef) throw new Error("usage: pylon work offers <request-ref>")
        const result = await listPylonWorkOffers(networkOptions, requestRef)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "accept") {
        const requestRef = args[2]
        const quoteRef = args[3]
        if (!requestRef || !quoteRef) throw new Error("usage: pylon work accept <request-ref> <quote-ref>")
        const result = await acceptPylonWorkOffer(networkOptions, { quoteRef, requestRef })
        await appendMemory(summary.paths.home, workAcceptanceMemoryEntry({
          at: new Date().toISOString(),
          quoteRef,
          requestRef,
          result,
        }))
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "status") {
        const requestRef = args[2]
        if (!requestRef) throw new Error("usage: pylon work status <request-ref>")
        const includeEvents = options.events !== undefined && options.events !== "false" && options.events !== "0"
        if (requestRef.startsWith("autopilot_work_order.") || includeEvents) {
          const result = includeEvents
            ? await readPylonAutopilotWorkEvents(networkOptions, requestRef)
            : await readPylonAutopilotWorkStatus(networkOptions, requestRef)
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        const result = await readPylonWorkStatus(networkOptions, requestRef)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "review") {
        const workOrderRef = args[2]
        const action = options.action
        if (
          !workOrderRef ||
          (action !== "accept" && action !== "reject" && action !== "request_changes")
        ) {
          throw new Error("usage: pylon work review <work-order-ref> --action accept|reject|request_changes")
        }
        const result = await reviewPylonAutopilotWork(networkOptions, { action, workOrderRef })
        await appendMemory(summary.paths.home, {
          at: new Date().toISOString(),
          kind: "autopilot_work_review",
          refs: { action, workOrderRef },
          summary: `reviewed autopilot work ${workOrderRef.slice(0, 48)} as ${action}`,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      throw new Error("usage: pylon work submit|status|review|request|offers|accept ...")
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "wallet") {
    try {
      const command = args[1]
      const walletArgs = args.slice(2)
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      if (command === "status") {
        const status = await classifyMdkWallet()
        const legacySparkMigration = await preflightLegacySparkMigration({
          dryRun: true,
          env: Bun.env,
          identityMnemonicPath: state.paths.identityMnemonic,
        })
        process.stdout.write(`${JSON.stringify({ ...status, legacySparkMigration }, null, 2)}\n`)
        return
      }
      if (command === "migrate-spark") {
        const sparkOptions = parsePsionicOptions(walletArgs)
        // Slice 3 (#5078): `--confirm-sweep` (or its dry-run probe `--sweep`)
        // selects the RECEIVE-SIDE reconcile half: sweep the node's OWN
        // received Spark backup funds into its OWN MDK wallet under explicit
        // consent. This is NOT a payout/send to third parties. Without the
        // sweep flags, `migrate-spark` keeps the legacy-balance migration
        // preflight below.
        const sweepBackup = sparkOptions["confirm-sweep"] === true || sparkOptions.sweep === true
        if (sweepBackup) {
          const showLocalTarget = sparkOptions["show-local-target"] === true
          const sparkBackupOptions = await resolveSparkBackupOptions(state, { showLocalTarget })
          const reconcile = await sweepSparkBackupToMdk({
            ...sparkBackupOptions,
            confirmSweep: sparkOptions["confirm-sweep"] === true,
            destinationReady:
              sparkOptions["destination-ready"] === true ||
              sparkOptions["destination-invoice-ready"] === true,
          })
          if (reconcile.state === "swept-to-mdk") {
            for (const receiptRef of reconcile.publicReceiptRefs) {
              await appendLedgerEvent(state.paths, {
                kind: "spark-backup-reconcile-swept",
                ref: receiptRef,
                data: {
                  receiptRef,
                  rail: "spark_backup",
                  sweptAmountSats: reconcile.sweptAmountSats,
                  claimedDepositCount: reconcile.claimedDepositCount,
                },
              })
            }
          }
          process.stdout.write(`${JSON.stringify({ ok: reconcile.state === "swept-to-mdk", reconcile }, null, 2)}\n`)
          process.exitCode =
            reconcile.state === "swept-to-mdk" || reconcile.state === "nothing-to-sweep" ? 0 : 1
          return
        }
        // #5085: rewire legacy migration to the Bun-native Breez SDK helper
        // seeded from the user's identity mnemonic + the embedded service key.
        // No manual Breez key, no external `spark-wallet-cli` binary. Spark is
        // deterministic from the seed, so the same identity mnemonic re-derives
        // the user's old Spark wallet and its balance.
        const identityMnemonicPath =
          stringPsionicOption(sparkOptions, "identity-mnemonic-path") ?? state.paths.identityMnemonic
        // Resolve the mnemonic the helper should derive from: an explicit
        // private-recovery path overrides the node's identity mnemonic.
        const explicitMnemonicPath = stringPsionicOption(sparkOptions, "identity-mnemonic-path")
        let migrationMnemonic: string | null = null
        if (explicitMnemonicPath) {
          try {
            migrationMnemonic = (await readFile(explicitMnemonicPath, "utf8")).trim()
          } catch {
            migrationMnemonic = null
          }
        } else {
          migrationMnemonic = await readIdentityMnemonicOrNull(state)
        }
        const legacyStorageDir = `${state.paths.home}/wallet/spark-backup/legacy-migrate`
        // Falls back gracefully: with no mnemonic the runner reports the helper
        // unavailable and the preflight surfaces the mnemonic-required blocker.
        const helperRunner = legacySparkHelperRunner({
          env: Bun.env,
          mnemonic: migrationMnemonic,
          storageDir: legacyStorageDir,
        })
        const result = await preflightLegacySparkMigration({
          destinationInvoiceReady: sparkOptions["destination-invoice-ready"] === true,
          dryRun: sparkOptions.execute !== true,
          embeddedCredentialAvailable: true,
          env: Bun.env,
          helperRunner,
          identityMnemonicPath,
          mnemonicRecoveryRequested: sparkOptions["mnemonic-recovery"] === true,
          yes: sparkOptions.yes === true,
        })

        // Consented sweep: when the user runs `migrate-spark --execute --yes`
        // with a ready destination and the preflight is otherwise ready to
        // migrate, perform the actual RECEIVE-SIDE reconcile — sweep the
        // detected legacy balance into the node's OWN MDK wallet via the
        // slice-3 `sweepSparkBackupToMdk` path. NOT a third-party payout. Fund
        // movement is gated by explicit consent here, not by
        // PYLON_SPARK_BACKUP_ENABLED, so we enable the reconcile explicitly and
        // inject the same mnemonic-backed helper.
        if (result.state === "migrated") {
          const reconcile = await sweepSparkBackupToMdk({
            enabled: true,
            embeddedCredentialAvailable: true,
            env: Bun.env,
            helper: createSparkBackupHelper({
              apiKey: resolveLegacySparkApiKey(Bun.env),
              mnemonic: migrationMnemonic ?? "",
              network: Bun.env.PYLON_SPARK_BACKUP_NETWORK === "regtest" ? "regtest" : "mainnet",
              storageDir: legacyStorageDir,
            }),
            confirmSweep: true,
            destinationReady:
              result.destinationInvoiceReady ||
              sparkOptions["destination-ready"] === true ||
              sparkOptions["destination-invoice-ready"] === true,
          })
          if (reconcile.state === "swept-to-mdk") {
            for (const receiptRef of reconcile.publicReceiptRefs) {
              await appendLedgerEvent(state.paths, {
                kind: "spark-backup-reconcile-swept",
                ref: receiptRef,
                data: {
                  receiptRef,
                  rail: "spark_backup",
                  source: "legacy_spark_migrate",
                  sweptAmountSats: reconcile.sweptAmountSats,
                  claimedDepositCount: reconcile.claimedDepositCount,
                },
              })
            }
          }
          process.stdout.write(
            `${JSON.stringify({ ok: reconcile.state === "swept-to-mdk", migration: result, reconcile }, null, 2)}\n`,
          )
          process.exitCode =
            reconcile.state === "swept-to-mdk" || reconcile.state === "nothing-to-sweep" ? 0 : 1
          return
        }

        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "blocked" ? 1 : 0
        return
      }
      const options = parseKeyValueOptions(walletArgs)
      if (command === "report-readiness") {
        const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
        if (!baseUrl) throw new Error("wallet report-readiness requires --base-url or PYLON_OPENAGENTS_BASE_URL")
        const status = await classifyMdkWallet()
        const result = await reportWalletReadiness({ status }, {
          agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
          baseUrl,
          pylonRef: state.identity.pylonRef,
        })
        // Onboarding auto-claim (#4712): a wallet that reports ready also
        // claims Forum tip-recipient readiness with a fresh BOLT 12 offer,
        // best-effort - the silent-untippable trap cannot happen to a
        // Pylon user. Failures are reported, never fatal to readiness.
        let tipReadinessClaim: Record<string, unknown> | { error: string } | null = null
        if (status.receiveReady) {
          try {
            tipReadinessClaim = await claimTipReadiness(
              {
                agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
                baseUrl,
              },
              { pylonRef: state.identity.pylonRef },
            )
          } catch (error) {
            tipReadinessClaim = { error: error instanceof Error ? error.message : String(error) }
          }
        }
        process.stdout.write(`${JSON.stringify({ status, result, tipReadinessClaim: tipReadinessClaim === null ? null : "tipRecipientReadiness" in (tipReadinessClaim as Record<string, unknown>) ? "claimed" : tipReadinessClaim }, null, 2)}\n`)
        return
      }
      if (command === "receive") {
        const amount = Number(options.amount)
        if (!Number.isFinite(amount) || amount <= 0) throw new Error("wallet receive requires --amount")
        // MDK-first; Spark backup second, only on an MDK offline/unavailable
        // class AND when the backup is opt-in enabled (slice 1 fallback).
        const sparkBackupOptions = await resolveSparkBackupOptions(state)
        const result = await receiveWithFallback(amount, { sparkBackup: sparkBackupOptions })
        if (result.rail === "spark_backup" && result.ok && result.sparkBackup.localTarget) {
          // Persist the freshly issued raw target to local private state only.
          await writeCachedSparkTarget(state.paths, result.sparkBackup.localTarget)
        }
        if (result.rail === "spark_backup" && result.receiptRef) {
          await appendLedgerEvent(state.paths, {
            kind: "backup-receive-selected",
            ref: result.receiptRef,
            data: { receiptRef: result.receiptRef, rail: "spark_backup", mdkFailureRef: result.mdkFailureRef },
          })
        }
        // Never emit the raw local target from `wallet receive`; redacted refs only.
        const redacted =
          result.rail === "spark_backup"
            ? {
                ok: result.ok,
                rail: result.rail,
                receiptRef: result.receiptRef,
                mdkFailureRef: result.mdkFailureRef,
                rawTargetAvailableLocally: result.rawTargetAvailableLocally,
                state: result.sparkBackup.projection.state,
                blockerRefs: result.sparkBackup.blockerRefs,
                nextHint: "Run `pylon wallet backup-receive --kind spark-address --show-local-target` to view the local target.",
              }
            : result
        process.stdout.write(`${JSON.stringify(redacted, null, 2)}\n`)
        return
      }
      if (command === "backup-receive") {
        const sparkOptions = parsePsionicOptions(walletArgs)
        const kind = stringPsionicOption(sparkOptions, "kind") ?? "spark-address"
        if (kind !== "spark-address") {
          throw new Error("wallet backup-receive supports --kind spark-address only")
        }
        const showLocalTarget = sparkOptions["show-local-target"] === true
        const sparkBackupOptions = await resolveSparkBackupOptions(state, { showLocalTarget })
        const result = await prepareSparkBackupReceive(sparkBackupOptions)
        if (result.ok && result.localTarget) {
          await writeCachedSparkTarget(state.paths, result.localTarget)
        }
        if (result.ok && result.receiptRef) {
          await appendLedgerEvent(state.paths, {
            kind: "backup-receive-selected",
            ref: result.receiptRef,
            data: { receiptRef: result.receiptRef, rail: "spark_backup", state: result.state },
          })
        }
        // Public-safe body always; raw local target only when explicitly asked.
        const body: Record<string, unknown> = {
          ok: result.ok,
          rail: result.rail,
          state: result.state,
          receiptRef: result.receiptRef,
          rawTargetAvailableLocally: result.rawTargetAvailableLocally,
          blockerRefs: result.blockerRefs,
          projection: result.projection,
        }
        if (showLocalTarget && result.localTarget) {
          // Local terminal output ONLY. Marked local/private.
          body.localTarget = result.localTarget
          body.localTargetNote = "LOCAL/PRIVATE: do not share, log, or post this raw target."
        }
        process.stdout.write(`${JSON.stringify(body, null, 2)}\n`)
        process.exitCode = result.ok ? 0 : 1
        return
      }
      if (command === "backup-status") {
        const sparkOptions = parsePsionicOptions(walletArgs)
        const showLocalTarget = sparkOptions["show-local-target"] === true
        const sparkBackupOptions = await resolveSparkBackupOptions(state, { showLocalTarget })
        const projection = await classifySparkBackupReceive(sparkBackupOptions)
        const sweep = recommendSparkSweep({
          detectedBalanceSats: projection.detectedBalanceSats,
          unclaimedDepositCount: projection.unclaimedDepositCount,
        })
        const body: Record<string, unknown> = {
          ok: projection.state !== "credential-missing" && projection.state !== "helper-unavailable",
          projection,
          sweep,
        }
        if (showLocalTarget) {
          const cached = await readCachedSparkTarget(state.paths)
          if (cached) {
            body.localTarget = cached
            body.localTargetNote = "LOCAL/PRIVATE: do not share, log, or post this raw target."
          }
        }
        process.stdout.write(`${JSON.stringify(body, null, 2)}\n`)
        return
      }
      if (command === "send") {
        const destinationRef = options["destination-ref"]
        if (!destinationRef) throw new Error("wallet send requires --destination-ref")
        const amount = options.amount === undefined ? undefined : Number(options.amount)
        const result = await sendWithMdk(destinationRef, amount)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "admit-payout-target") {
        const kind = options.kind as any
        const ref = options.ref
        if (!kind || !ref) throw new Error("wallet admit-payout-target requires --kind and --ref")
        const result = admitPayoutTarget({ kind, ref })
        process.stdout.write(`${JSON.stringify({ ...result, ledger: state.paths.ledger }, null, 2)}\n`)
        return
      }
      if (command === "request-payout-target-admission") {
        const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
        if (!baseUrl) throw new Error("wallet request-payout-target-admission requires --base-url or PYLON_OPENAGENTS_BASE_URL")
        const kind = options.kind as any
        const ref = options.ref
        if (!kind || !ref) throw new Error("wallet request-payout-target-admission requires --kind and --ref")
        const result = await requestPayoutTargetAdmission({ kind, ref }, {
          agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
          baseUrl,
          pylonRef: state.identity.pylonRef,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown wallet command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon wallet failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "assignment") {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), "pylon assignment")
      rejectClaudeLocalDangerForPublicPath(args.slice(1), "pylon assignment")
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const baseUrl = optionString(options, "base-url") ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("assignment commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const agentToken = options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN
      const clientOptions = { ...(agentToken ? { agentToken } : {}), baseUrl }
      if (command === "poll") {
        const leases = await pollAssignments(summary, clientOptions)
        process.stdout.write(`${JSON.stringify({ leases }, null, 2)}\n`)
        return
      }
      if (command === "accept") {
        const leaseJson = options.lease
        if (!leaseJson) throw new Error("assignment accept requires --lease JSON")
        const result = await acceptAssignment(summary, JSON.parse(leaseJson) as PylonAssignmentLease, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "progress") {
        const progressJson = options.progress
        if (!progressJson) throw new Error("assignment progress requires --progress JSON")
        const result = await submitAssignmentProgress(summary, JSON.parse(progressJson) as AssignmentProgress, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "closeout") {
        const closeoutJson = options.closeout
        if (!closeoutJson) throw new Error("assignment closeout requires --closeout JSON")
        const result = await submitAssignmentCloseout(summary, JSON.parse(closeoutJson) as AssignmentCloseout, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "run-no-spend") {
        const result = await runNoSpendAssignment(summary, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown assignment command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon assignment failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "provider") {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), "pylon provider")
      rejectClaudeLocalDangerForPublicPath(args.slice(1), "pylon provider")
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      if (command === "go-online" || command === "online") {
        const claudeAgentReadiness = await probeClaudeAgentReadiness({
          config: await loadClaudeAgentConfig(summary),
        })
        const codexAgentReadiness = await probeCodexAgentReadiness({
          config: await loadCodexAgentConfig(summary),
        })
        // W4.1 (#4750): the Tassadar executor capability is declared
        // only behind a passing self-test receipt — a real digest-pinned
        // execution on this device — never by configuration assertion.
        const tassadarDeclaration = await declareTassadarExecutorCapability()
        const evidencePath = await writeTassadarCapabilityEvidence(
          state.paths.home,
          tassadarDeclaration,
        )
        const nextRuntime = {
          ...state.runtime,
          lifecycle: "online" as const,
          capabilityRefs: withWorkspaceMaterializerCapability(
            withCodexAgentCapability(
              withClaudeAgentCapability(
                [...new Set([
                  ...mergeTassadarCapabilityRefs(state.runtime.capabilityRefs, tassadarDeclaration),
                  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
                  PYLON_LABOR_CAPABILITY_REF,
                ])],
                claudeAgentReadiness,
              ),
              codexAgentReadiness,
            ),
          ),
          blockerRefs: [...new Set([
            ...state.runtime.blockerRefs.filter((ref) =>
              ref !== "blocker.assignment.lifecycle_offline" &&
              ref !== PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF,
            ),
            ...tassadarDeclaration.blockerRefs,
          ])],
        }
        await writeRuntimeState(state.paths, nextRuntime)
        process.stdout.write(`${JSON.stringify({
          ok: true,
          lifecycle: nextRuntime.lifecycle,
          capabilityRefs: nextRuntime.capabilityRefs,
          claudeAgent: {
            state: claudeAgentReadiness.state,
            credentialSourceRef: claudeAgentReadiness.credentialSourceRef,
          },
          codexAgent: {
            state: codexAgentReadiness.state,
            credentialSourceRef: codexAgentReadiness.credentialSourceRef,
          },
          tassadar: {
            declared: tassadarDeclaration.declared,
            capabilityRef: tassadarDeclaration.capabilityRef,
            selfTestReceiptRef: tassadarDeclaration.selfTestReceiptRef,
            windowVersionRef: tassadarDeclaration.windowVersionRef,
            legRefs: tassadarDeclaration.legRefs,
            replayClassId: tassadarDeclaration.replayClassId,
            matrixRow: tassadarDeclaration.matrixRow,
            blockerRefs: tassadarDeclaration.blockerRefs,
            evidencePath,
          },
          relayUrls: relaysFromEnv(Bun.env),
          policy: policyFromEnv(Bun.env),
          stateRef: "state.public.pylon.nip90_provider.online",
        }, null, 2)}\n`)
        return
      }
      if (command === "approve-labor") {
        const approvedByRef = options["approved-by-ref"]
        if (!approvedByRef) throw new Error("provider approve-labor requires --approved-by-ref")
        const jobType = options["job-type"]
        if (jobType !== undefined && jobType !== "code_task" && jobType !== "review" && jobType !== "document_work") {
          throw new Error("provider approve-labor --job-type must be code_task, review, or document_work")
        }
        const result = await approveLaborFirstRun({
          paths: state.paths,
          approvedByRef,
          ...(jobType === undefined ? {} : { jobType }),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "go-offline" || command === "offline") {
        const nextRuntime = {
          ...state.runtime,
          lifecycle: "offline" as const,
        }
        await writeRuntimeState(state.paths, nextRuntime)
        process.stdout.write(`${JSON.stringify({
          ok: true,
          lifecycle: nextRuntime.lifecycle,
          stateRef: "state.public.pylon.nip90_provider.offline",
        }, null, 2)}\n`)
        return
      }
      if (command === "once") {
        const result = await startNip90ProviderLoop(summary, {
          once: true,
          log: (message) => process.stderr.write(`${message}\n`),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown provider command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon provider failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "node") {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), "pylon node")
      rejectClaudeLocalDangerForPublicPath(args.slice(1), "pylon node")
    } catch (error) {
      process.stderr.write(`Pylon node-core failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
    await maybeAutoUpdate()
    await Effect.runPromise(
      Effect.scoped(runHeadlessNode).pipe(
        Effect.catch((error) => Console.error(`Pylon node-core crashed: ${error.message}`)),
      ),
    )
    process.exit(0)
  }

  if (args[0] === "runtime" || runtimeCommandNamespaces.has(args[0] ?? "")) {
    const runtimeArgs = args[0] === "runtime" ? args.slice(1) : args
    const result = await Effect.runPromise(runProbeCli(runtimeArgs, { env: Bun.env }))
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    process.exitCode = result.exitCode
    return
  }

  // Default (no subcommand): Pylon is a headless, CLI-only node. Booting with
  // no arguments runs the same node-core as `pylon node` — services + event
  // stream + loopback control API, logging to stdout, no interactive UI.
  try {
    rejectCodexLocalDangerForPublicPath(args, "pylon")
    rejectClaudeLocalDangerForPublicPath(args, "pylon")
  } catch (error) {
    process.stderr.write(`Pylon node-core failed: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
    return
  }
  await maybeAutoUpdate()
  await Effect.runPromise(
    Effect.scoped(runHeadlessNode).pipe(
      Effect.catch((error) =>
        Console.error(`Pylon v0.3 crashed on startup: ${error.message}`)
      )
    )
  )
  // Scope is closed: services interrupted, process can exit cleanly. Exit
  // explicitly so lingering library handles cannot hold the process open
  // after a deliberate shutdown.
  process.exit(0)
}

await main()
