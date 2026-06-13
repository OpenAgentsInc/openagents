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
  loadClaudeDevConfig,
  probeClaudeAgentReadiness,
  withClaudeAgentCapability,
} from "./claude-agent"
import {
  loadCodexDevConfig,
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  type PylonComposerAdapter,
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
import { Console, Deferred, Effect, PubSub, SubscriptionRef } from "effect"
import { classifyServiceLogLevel, formatLogTimestamp, type PylonLogLevel } from "./node/state"
import {
  applyRemoteEvent,
  applyRemotePanes,
  forkLogPersistence,
  forkNodeServices,
  logMessage,
  makePylonNodeRuntime,
  publishLogEntries,
  seedLogFeed,
  superviseLoop,
  type PylonNodeRuntime,
} from "./node/runtime"
import { runOpencodeStream } from "./opencode-run"
import { loadKeybindOverrides } from "./node/keybinds"
import { createFeedLogWriter, readPersistedLogTail } from "./node/log-persist"
import {
  buildBrokerRegistrationBody,
  postNodeRegistration,
  type BrokerRegistrationHosts,
} from "./node/discovery-register"
import { createIntentQueue } from "./node/intent-intake"
import {
  defaultControlPort,
  ensureControlToken,
  controlTokenPath,
  startControlServer,
  type ControlCommandActions,
} from "./node/control-server"
import { createControlSessionActions } from "./node/control-sessions"
import { runControlClient, sendControlCommand } from "./node/control-client"
import { loadComposerState, saveComposerState } from "./node/composer-store"
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
  sandboxModeForCodexComposerExecutionMode,
  type CodexComposerExecutionMode,
} from "./codex-composer"
import {
  claudeComposerLabel,
  permissionModeForClaudeComposerExecutionMode,
  rejectClaudeLocalDangerForPublicPath,
  runClaudeComposerStream,
  type ClaudeComposerExecutionMode,
} from "./claude-composer"
import { runProbeCli } from "../packages/runtime/src/index"
import {
  createBootstrapSummary,
  formatBootstrapText,
  parseBootstrapArgs,
  writeBootstrapFiles,
} from "./bootstrap"
import { ensurePylonLocalState, projectPublicStatus, writeRuntimeState } from "./state"
import {
  completePylonLink,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
} from "./presence"
import {
  admitPayoutTarget,
  classifyMdkWallet,
  preflightLegacySparkMigration,
  receiveWithMdk,
  reportWalletReadiness,
  requestPayoutTargetAdmission,
  sendWithMdk,
} from "./wallet"
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

// Bridge for legacy call sites (OpenCode helpers) that log from plain
// async code. Set once at dashboard boot, before any service starts.
let nodeRuntime: PylonNodeRuntime | null = null
// Quiet by default: verbose service chatter is hidden unless --verbose or
// PYLON_VERBOSE=1 (issue #4743).
let verboseMode = false

// The dynamically-loaded Solid view module (src/tui/app.tsx). index.ts must
// never import it statically: the Solid transform plugin has to be active
// before any .tsx (or solid-js) module loads — see ensureSolidRuntime().
type DashboardUiModule = typeof import("./tui/app")
type ComposerBackend = DashboardUiModule["startDashboard"] extends (options: infer Options) => unknown
  ? Options extends { composerBackend?: infer Backend }
    ? NonNullable<Backend>
    : never
  : never
type DevActions = DashboardUiModule["startDashboard"] extends (options: infer Options) => unknown
  ? Options extends { devActions?: infer Actions }
    ? NonNullable<Actions>
    : never
  : never
type ContextActions = DashboardUiModule["startDashboard"] extends (options: infer Options) => unknown
  ? Options extends { contextActions?: infer Actions }
    ? NonNullable<Actions>
    : never
  : never
let dashboardUi: DashboardUiModule | null = null

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

// The Solid JSX transform must be registered before src/tui/*.tsx loads.
// The dashboard path re-execs itself once with --preload pointing at the
// plugin inside our own dependency tree. This is deliberately NOT a bunfig
// preload: subcommands (bootstrap/status/wallet/...) must keep working even
// when the transform cannot load, and bunfig preload failures abort before
// any of our code runs. Tests still preload via bunfig's [test] section.
async function ensureSolidRuntime(): Promise<void> {
  const transformState = (globalThis as Record<symbol, { installed?: boolean } | undefined>)[
    Symbol.for("opentui.solid.transform")
  ]
  if (transformState?.installed) return
  if (Bun.env.PYLON_SOLID_REEXEC === "1") {
    throw new Error("Solid transform plugin failed to install after --preload re-exec")
  }
  const preloadPath = Bun.fileURLToPath(import.meta.resolve("@opentui/solid/preload"))
  const entry = Bun.argv[1] ?? Bun.fileURLToPath(import.meta.url)
  const child = Bun.spawn({
    cmd: [process.execPath, "--preload", preloadPath, entry, ...Bun.argv.slice(2)],
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
    env: { ...Bun.env, PYLON_SOLID_REEXEC: "1" },
  })
  // Forward termination to the child so it is never orphaned holding the
  // terminal (Ctrl+C reaches it via the tty; these cover kill/timeouts).
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP", "SIGALRM"] as const) {
    process.on(signal, () => {
      try {
        child.kill(signal)
      } catch {
        // child already gone
      }
    })
  }
  process.exit(await child.exited)
}

// Node-side wallet actions: the single execution path for money commands,
// used by the local dashboard, the control server (attach mode), and the
// headless node (issue #4740).
const nodeWalletActions: ControlCommandActions = {
  walletSend: (destinationRef, amountSats) => sendWithMdk(destinationRef, amountSats),
  walletReceive: (amountSats) => receiveWithMdk(amountSats),
  walletAdmitPayoutTarget: (kind, ref) =>
    Promise.resolve(admitPayoutTarget({ kind: kind as Parameters<typeof admitPayoutTarget>[0]["kind"], ref })),
}

// CL-34/CL-35 work-intent intake: the phone composes an "ask" and submits it;
// the node enqueues it (server-generates the id + timestamp) for the coordinator
// to plan and fan out. Persisted to the Pylon home so intents survive restart.
function makeIntentActions(persistPath: string) {
  const intentQueue = createIntentQueue({ persistPath })
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

function makeDevActions(summary: ReturnType<typeof createBootstrapSummary>): DevActions {
  const cwd = codexComposerWorkingDirectory()
  return {
    check: () => runPylonDevCheck({ allowDirty: true, cwd, env: Bun.env, summary }),
    apply: () => runPylonDevApply({ allowDirty: true, cwd, env: Bun.env, summary }),
    reload: () => runPylonDevReload({ cwd, env: Bun.env, summary }),
  }
}

function makeContextActions(summary: ReturnType<typeof createBootstrapSummary>): ContextActions {
  const cwd = codexComposerWorkingDirectory()
  return {
    refresh: () =>
      collectPylonContextProjection({
        cwd,
        dangerFlag: Bun.argv.includes("--codex-danger"),
        env: Bun.env,
        summary,
      }),
  }
}

function makeSessionActions(summary: ReturnType<typeof createBootstrapSummary>) {
  return createControlSessionActions({ summary })
}

function codexComposerWorkingDirectory() {
  const configured = Bun.env.PYLON_CODEX_CWD ?? Bun.env.PYLON_ACTIVE_REPO
  return configured && configured.trim().length > 0 ? configured : process.cwd()
}

function parseComposerAdapterOverride(args: ReadonlyArray<string>): PylonComposerAdapter | null {
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] !== "--adapter") continue
    const value = args[index + 1]
    if (!value || value.startsWith("--")) {
      throw new Error("--adapter requires codex or claude")
    }
    if (value === "codex") return "codex"
    if (value === "claude" || value === "claude_agent") return "claude_agent"
    throw new Error("--adapter must be codex or claude")
  }
  return null
}

async function makeCodexComposerBackend(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: {
    allowDangerousLocal?: boolean
    dangerFlag?: boolean
    readDevConfig?: boolean
  } = {},
): Promise<ComposerBackend> {
  const config = await loadCodexAgentConfig(summary)
  const devConfig = options.readDevConfig === false ? {} : await loadCodexDevConfig(summary)
  const dangerRequested =
    options.dangerFlag === true || devConfig.codexExecutionMode === "local_supervised_danger"
  const executionMode: CodexComposerExecutionMode =
    dangerRequested && options.allowDangerousLocal === true
      ? "local_supervised_danger"
      : "local_bounded"
  const sandboxMode = sandboxModeForCodexComposerExecutionMode(executionMode, config.sandboxMode)
  const timeoutMs =
    typeof config.timeoutSeconds === "number" && Number.isFinite(config.timeoutSeconds) && config.timeoutSeconds > 0
      ? Math.round(config.timeoutSeconds * 1000)
      : undefined
  const cwd = codexComposerWorkingDirectory()
  const statusLine = `mode: ${executionMode} | sandbox: ${sandboxMode}`
  return {
    label: executionMode === "local_supervised_danger" ? "Codex DANGER" : "Codex",
    statusLine,
    submit: async (prompt, callbacks) => {
      const result = await runCodexComposerStream(
        prompt,
        {
          config,
          cwd,
          executionMode,
          model: config.model,
          sandboxMode,
          approvalPolicy: "never",
          networkAccessEnabled: false,
          timeoutMs,
          usageStateSummary: summary,
        },
        {
          onText: callbacks.onText,
          onEvent: callbacks.onEvent,
          onUsage: (usage) => callbacks.onUsage?.({ label: "tokens", value: String(usage.totalTokens) }),
        },
      )
      await recordPylonDevCodexRun(
        {
          commandCount: result.commandCount,
          cwd,
          editedFileCount: result.editedFileCount,
          eventCount: result.eventCount,
          executionMode,
          sandboxMode,
          totalTokens: result.totalTokens,
        },
        {
          cwd,
          env: Bun.env,
          summary,
        },
      ).catch((error) => {
        callbacks.onEvent?.(
          `dev run record failed: ${error instanceof Error ? error.message : String(error)}`,
          result.eventCount,
        )
      })
      const footerParts = [
        `mode: ${executionMode}`,
        `cwd: ${cwd}`,
        `sandbox: ${sandboxMode}`,
        `events: ${result.eventCount}`,
        `tokens: ${result.totalTokens}`,
        `commands: ${result.commandCount}`,
        `files: ${result.editedFileCount}`,
      ]
      return {
        text: result.text || "(Codex completed without an assistant message.)",
        footer: footerParts.join(" | "),
      }
    },
  }
}

async function makeClaudeComposerBackend(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: {
    allowDangerousLocal?: boolean
    dangerFlag?: boolean
    readDevConfig?: boolean
  } = {},
): Promise<ComposerBackend> {
  const config = await loadClaudeAgentConfig(summary)
  const devConfig = options.readDevConfig === false ? {} : await loadClaudeDevConfig(summary)
  const dangerRequested =
    options.dangerFlag === true || devConfig.claudeExecutionMode === "local_supervised_danger"
  const executionMode: ClaudeComposerExecutionMode =
    dangerRequested && options.allowDangerousLocal === true
      ? "local_supervised_danger"
      : "local_bounded"
  const permissionMode = permissionModeForClaudeComposerExecutionMode(executionMode)
  const timeoutMs =
    typeof config.timeoutSeconds === "number" && Number.isFinite(config.timeoutSeconds) && config.timeoutSeconds > 0
      ? Math.round(config.timeoutSeconds * 1000)
      : undefined
  const cwd = codexComposerWorkingDirectory()
  const label = claudeComposerLabel(config.model, executionMode)
  const statusLine = `mode: ${executionMode} | permissions: ${permissionMode}${config.model ? ` | model: ${config.model}` : ""}`
  let sessionId: string | null = null
  return {
    label,
    statusLine,
    submit: async (prompt, callbacks) => {
      const result = await runClaudeComposerStream(
        prompt,
        {
          config,
          cwd,
          executionMode,
          permissionMode,
          model: config.model,
          resumeSessionId: sessionId,
          timeoutMs,
          usageStateSummary: summary,
        },
        {
          onText: callbacks.onText,
          onEvent: callbacks.onEvent,
          onUsage: (usage) => callbacks.onUsage?.({ label: "tokens", value: String(usage.totalTokens) }),
        },
      )
      sessionId = result.sessionId ?? sessionId
      const footerParts = [
        "adapter: claude_agent",
        `mode: ${executionMode}`,
        `permissions: ${permissionMode}`,
        `cwd: ${cwd}`,
        `events: ${result.eventCount}`,
        `tokens: ${result.totalTokens}`,
        `commands: ${result.commandCount}`,
        `files: ${result.editedFileCount}`,
        ...(result.sessionRef ? [`session: ${result.sessionRef}`] : []),
      ]
      return {
        text: result.text || "(Claude completed without an assistant message.)",
        footer: footerParts.join(" | "),
      }
    },
  }
}

async function makeComposerBackend(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: {
    allowDangerousLocal?: boolean
    dangerFlag?: boolean
    readDevConfig?: boolean
  } = {},
): Promise<ComposerBackend> {
  const devConfig = options.readDevConfig === false ? {} : await loadCodexDevConfig(summary)
  const adapter = parseComposerAdapterOverride(Bun.argv.slice(2)) ?? devConfig.defaultAdapter ?? "codex"
  if (adapter === "claude_agent") {
    // Danger flags are per-lane: --codex-danger never leaks into a Claude
    // session or vice versa.
    return makeClaudeComposerBackend(summary, {
      ...(options.allowDangerousLocal === undefined
        ? {}
        : { allowDangerousLocal: options.allowDangerousLocal }),
      dangerFlag: Bun.argv.includes("--claude-danger"),
      ...(options.readDevConfig === undefined ? {} : { readDevConfig: options.readDevConfig }),
    })
  }
  return makeCodexComposerBackend(summary, options)
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

type OpenCodeInferenceOptions = {
  label: string
  streamToUi?: boolean
  statusIntervalMs?: number
}

// OpenCode Programmatic Integration Helper. Streaming UI goes through the
// Solid feed store via the dynamically loaded view module (verbose mode
// only); all progress logging goes through the runtime log seam.
async function executeOpencodeInference(
  opencodePath: string,
  prompt: string,
  options: OpenCodeInferenceOptions,
) {
  const chatItem =
    options.streamToUi && verboseMode && dashboardUi
      ? dashboardUi.appendChatFeedItem(`**OpenCode ${options.label}**: starting...`, { streaming: true })
      : null

  const startTime = Date.now()
  const statusIntervalMs = options.statusIntervalMs ?? 5000
  let lastText = ""
  let lastUsage = { cost: 0, tokens: 0 }
  let eventCount = 0
  let lastEventSummary = "waiting for first event"

  const renderStreamingLine = () => {
    if (!chatItem) return
    const elapsedSeconds = Math.max(1, Math.round((Date.now() - startTime) / 1000))
    const visibleText = lastText.trim() || `_${lastEventSummary}_`
    const footer = `\n\n*[${options.label}: ${elapsedSeconds}s | events: ${eventCount} | tokens: ${lastUsage.tokens || "-"}]*`
    chatItem.update(`**OpenCode ${options.label}**: ${visibleText}${footer}`)
  }

  const statusTimer = setInterval(() => {
    const elapsedSeconds = Math.max(1, Math.round((Date.now() - startTime) / 1000))
    logToUi(
      `[OpenCode] ${options.label} still running (${elapsedSeconds}s, events=${eventCount}, last=${lastEventSummary}).`,
    )
    renderStreamingLine()
  }, statusIntervalMs)

  try {
    const result = await runOpencodeStream(opencodePath, prompt, {
      onText: (text) => {
        lastText = text
        renderStreamingLine()
      },
      onEvent: (summary, count) => {
        eventCount = count
        lastEventSummary = summary
        logToUi(`[OpenCode] ${options.label} event ${count}: ${summary}`)
      },
      onRaw: (line) => {
        lastEventSummary = `raw output: ${line.slice(0, 120)}`
        logToUi(`[OpenCode] ${options.label} raw output: ${line.slice(0, 240)}`)
        renderStreamingLine()
      },
      onStderr: (line) => {
        logToUi(`[OpenCode] ${options.label} stderr: ${line}`)
      },
      onUsage: (usage) => {
        lastUsage = usage
        renderStreamingLine()
      },
    })
    return result
  } finally {
    clearInterval(statusTimer)
    chatItem?.finish()
  }
}

// OpenCode Programmatic Integration Service (Diagnostics on boot)
const runOpencodeStartupInference = Effect.gen(function* () {
  yield* log("[OpenCode] Checking for local OpenCode CLI installation...")
  const opencodePath = Bun.which("opencode")
  if (opencodePath) {
    yield* log(`[OpenCode] Found OpenCode CLI at ${opencodePath}. Initiating bootup diagnostics...`)

    // 1. Get neutral log summary (<10 words)
    const bootLogs = nodeRuntime
      ? (yield* SubscriptionRef.get(nodeRuntime.logFeed))
          .map((entry) => `[${formatLogTimestamp(entry.at)}] ${entry.message}`)
          .join("\n")
      : ""
    const logSummaryResult = yield* Effect.tryPromise({
      try: () => {
        const prompt = `Here are the bootup sequence logs:\n\n${bootLogs}\n\nProvide a one line, <10 word, neutral, terminal-sounding summary of these bootup sequence logs.`
        return executeOpencodeInference(opencodePath, prompt, {
          label: "boot-summary",
          streamToUi: true,
        })
      },
      catch: (error) => new Error(`Failed to execute bootup summary: ${String(error)}`),
    })

    yield* log(`[OpenCode] Bootup Summary: "${logSummaryResult.text}"`)
    yield* log(`[OpenCode] Cost: $${logSummaryResult.cost.toFixed(4)} | Tokens: ${logSummaryResult.tokens}`)

    // 2. Read AGENTS.md and contribute a useful post somewhere on the site.
    yield* log("[OpenCode] Reading https://openagents.com/AGENTS.md and creating a useful site post...")
    const contributionResult = yield* Effect.tryPromise({
      try: () => {
        const prompt = [
          "Read https://openagents.com/AGENTS.md and follow its current agent instructions.",
          "If you have no registered agent identity, register one for this local Pylon/OpenCode context.",
          "Inspect the Forum board, recent posts, and search if needed before choosing where to contribute.",
          "Decide explicitly whether the contribution belongs as a new topic or as a reply to an existing topic.",
          "Reply only when the contribution is directly on-topic for an existing thread; create a new topic when the idea is distinct, would hijack the thread, or starts a reusable reference/discussion.",
          "Then add value on openagents.com with a concise public Forum post.",
          "The post should be useful to other agents or operators, not promotional, and should not expose secrets, wallet material, private repo content, or credentials.",
          "When finished, report the placement decision, topic/post URL, and a one-sentence summary of the value added.",
        ].join(" ")
        return executeOpencodeInference(opencodePath, prompt, {
          label: "site-post",
          streamToUi: true,
        })
      },
      catch: (error) => new Error(`Failed to execute site contribution: ${String(error)}`),
    })

    yield* log(`[OpenCode] Site Contribution:\n${contributionResult.text}`)
    yield* log(`[OpenCode] Cost: $${contributionResult.cost.toFixed(4)} | Tokens: ${contributionResult.tokens}`)
  } else {
    yield* log("[OpenCode] OpenCode CLI is not installed on this system.")
  }
})

// Main Pylon v0.3 Application Loop. Runs inside a Scope (Effect.scoped at
// the call site): every service fiber is forked into that Scope, and the
// renderer/terminal are restored by finalizers, so Ctrl+C is a deliberate
// interruption instead of process death (issue #4736).
const runPylonNode = Effect.gen(function* () {
  const smokeDashboard = Bun.argv.includes("--smoke-dashboard") || Bun.env.PYLON_SMOKE_DASHBOARD === "1"
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
  yield* logMessage(runtime, "verbose", "Initializing Pylon v0.3 observational earning node...")

  // Load and mount the Solid view (src/tui/app.tsx). Imported dynamically:
  // the Solid transform plugin must be active before any .tsx module loads —
  // ensureSolidRuntime() in main() guarantees that.
  const ui = yield* Effect.tryPromise({
    try: () => import("./tui/app"),
    catch: (error) => new Error(`Failed to load dashboard view: ${String(error)}`),
  })
  dashboardUi = ui

  const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)

  // Durable feed log (issue #4739): restore the tail of the previous
  // session's scrollback, then persist every live log entry. The persister
  // subscribes before services fork, so nothing is missed.
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

  const nodeAssignmentActions = makeAssignmentActions()
  const nodeSessionActions = makeSessionActions(bootstrapSummary)

  // Control server (issue #4740): a second terminal can attach to this
  // running node. Port conflicts (another node already serving) are reported
  // and non-fatal.
  const controlToken = yield* Effect.promise(() => ensureControlToken(bootstrapSummary.paths.home))
  const controlPort = Number(Bun.env.PYLON_CONTROL_PORT ?? defaultControlPort)
  const controlServer = yield* startControlServer(runtime, {
    token: controlToken,
    actions: {
      ...nodeWalletActions,
      ...(nodeAssignmentActions
        ? {
            assignmentsPoll: () => nodeAssignmentActions.poll(),
            assignmentsAccept: (leaseRef: string) => nodeAssignmentActions.accept(leaseRef),
          }
        : {}),
      sessions: nodeSessionActions,
      intents: makeIntentActions(`${bootstrapSummary.paths.home}/intents.json`),
    },
    port: controlPort,
    hostname: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
  }).pipe(
    Effect.catch((error) =>
      Effect.gen(function* () {
        yield* logMessage(runtime, "info", `[Control] Attach API unavailable: ${error.message}`, { transient: true })
        return null
      }),
    ),
  )
  if (controlServer) {
    yield* logMessage(
      runtime,
      "verbose",
      `[Control] Attach API on ${controlServer.url} (token: ${controlTokenPath(bootstrapSummary.paths.home)})`,
    )
    startDiscoveryHeartbeat({
      controlPort,
      controlToken,
      boundHost: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
    })
  }

  // User keybind overrides (apps/pylon home keybinds.json, Effect-Schema
  // validated). Invalid files are reported and ignored.
  const keybinds = yield* Effect.promise(() => loadKeybindOverrides(bootstrapSummary.paths.home))
  if (keybinds.state === "invalid") {
    yield* logMessage(runtime, "info", `[Keybinds] Ignoring invalid ${keybinds.path}: ${keybinds.error}`)
  }

  const composerState = yield* Effect.promise(() =>
    loadComposerState(bootstrapSummary.paths.home).catch(() => ({ history: [], stash: "" })),
  )
  const persistComposerState = (state: { history: string[]; stash: string }) => {
    void saveComposerState(bootstrapSummary.paths.home, state).catch(() => {})
  }
  const composerBackend = yield* Effect.promise(() =>
    makeComposerBackend(bootstrapSummary, {
      allowDangerousLocal: true,
      dangerFlag: Bun.argv.includes("--codex-danger"),
      readDevConfig: true,
    }),
  )
  const devActions = makeDevActions(bootstrapSummary)
  const contextActions = makeContextActions(bootstrapSummary)
  const initialContextProjection = yield* Effect.promise(() =>
    contextActions.refresh().catch((error) => {
      logToUi(`[Context] Initial refresh failed: ${error instanceof Error ? error.message : String(error)}`, "info")
      return null
    }),
  )

  const dashboard = yield* Effect.tryPromise({
    try: () =>
      ui.startDashboard({
        onRequestShutdown: requestShutdown,
        verbose: verboseMode,
        enable3d: !smokeDashboard && Bun.env.PYLON_DISABLE_3D !== "1",
        keybindOverrides: keybinds.overrides,
        assignmentActions: nodeAssignmentActions,
        composerState,
        composerBackend,
        contextActions,
        devActions,
        initialContextProjection,
        onComposerPersist: persistComposerState,
        onVerboseChange: (verbose) => {
          verboseMode = verbose
        },
        // Money flows stay node-side; the view invokes these only after an
        // explicit confirm dialog (issue #4738 exit criterion).
        walletActions: {
          send: (destinationRef, amountSats) => sendWithMdk(destinationRef, amountSats),
          receive: (amountSats) => receiveWithMdk(amountSats),
          admitPayoutTarget: (kind, ref) =>
            Promise.resolve(admitPayoutTarget({ kind: kind as Parameters<typeof admitPayoutTarget>[0]["kind"], ref })),
        },
      }),
    catch: (error) => new Error(`Failed to initialize OpenTUI renderer: ${String(error)}`),
  })
  yield* Effect.addFinalizer(() =>
    Effect.sync(() => {
      dashboard.destroy()
      dashboardUi = null
    }),
  )

  // Attach the view to the runtime seam through the bridge: replay current
  // state, then follow ref changes and batched log events. Services are
  // forked only after this, so the feed replay misses nothing.
  yield* ui.attachRuntimeToView(runtime, { verbose: verboseMode })
  yield* logMessage(runtime, "info", `[Codex] Composer ${composerBackend.statusLine ?? "mode: unavailable"}.`, {
    transient: true,
  })

  yield* logMessage(
    runtime,
    "info",
    verboseMode
      ? "Pylon v0.3 dashboard active (verbose logging)."
      : "Pylon v0.3 ready. Logs are quiet by default - relaunch with --verbose for service detail.",
    { transient: true },
  )

  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  yield* logMessage(runtime, "info", `[Identity] Pylon Nostr npub: ${localState.identity.npub}`, { transient: true })

  // Fork node services into this Scope - interrupted on shutdown.
  const presenceBaseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
  yield* forkNodeServices(runtime, {
    wallet: {
      classify: () => classifyMdkWallet(),
    },
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
      register: () => registerPylon(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
      heartbeat: () => sendHeartbeat(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
    },
  })

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

  if (!smokeDashboard && Bun.env.PYLON_DISABLE_OPENCODE_STARTUP !== "1") {
    yield* superviseLoop(runtime, "OpenCode", runOpencodeStartupInference)
  }

  if (smokeDashboard) {
    yield* logMessage(runtime, "info", "Pylon v0.3 dashboard smoke complete.", { transient: true })
    yield* Effect.sync(() => setTimeout(() => process.exit(0), 2000))
    return
  }

  // Stay alive until Ctrl+C / SIGINT / SIGTERM requests shutdown; closing
  // the surrounding Scope then interrupts every service fiber and runs the
  // renderer/terminal finalizers.
  yield* Deferred.await(shutdown)
})

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
      sessions: headlessSessionActions,
      intents: makeIntentActions(`${bootstrapSummary.paths.home}/intents.json`),
    },
    port: controlPort,
    hostname: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
  })
  yield* logMessage(
    runtime,
    "info",
    `Pylon node-core running headless. Attach with: pylon attach ${controlServer.url} (token: ${controlTokenPath(bootstrapSummary.paths.home)})`,
    { transient: true },
  )
  startDiscoveryHeartbeat({
    controlPort,
    controlToken,
    boundHost: Bun.env.PYLON_CONTROL_HOST ?? "127.0.0.1",
  })

  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  yield* logMessage(runtime, "info", `[Identity] Pylon Nostr npub: ${localState.identity.npub}`, { transient: true })

  const presenceBaseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
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
      register: () => registerPylon(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
      heartbeat: () => sendHeartbeat(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
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

// Attached TUI (issue #4740): full dashboard, but the local runtime mirrors
// a remote node over SSE and money commands round-trip the control API.
const runPylonAttach = (baseUrl: string, token: string) =>
  Effect.gen(function* () {
    verboseMode = Bun.argv.includes("--verbose") || Bun.env.PYLON_VERBOSE === "1"
    const runtime = yield* makePylonNodeRuntime
    nodeRuntime = runtime

    const shutdown = yield* Deferred.make<void>()
    const requestShutdown = () => {
      Effect.runFork(Deferred.succeed(shutdown, void 0))
      setTimeout(() => process.exit(0), 2000)
    }
    process.once("SIGINT", requestShutdown)
    process.once("SIGTERM", requestShutdown)

    const ui = yield* Effect.tryPromise({
      try: () => import("./tui/app"),
      catch: (error) => new Error(`Failed to load dashboard view: ${String(error)}`),
    })
    dashboardUi = ui
    const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
    const composerBackend = yield* Effect.promise(() =>
      makeComposerBackend(bootstrapSummary, {
        allowDangerousLocal: false,
        dangerFlag: false,
        readDevConfig: false,
      }),
    )

    const dashboard = yield* Effect.tryPromise({
      try: () =>
        ui.startDashboard({
          onRequestShutdown: requestShutdown,
          verbose: verboseMode,
          enable3d: Bun.env.PYLON_DISABLE_3D !== "1",
          composerBackend,
          contextActions: null,
          devActions: null,
          onVerboseChange: (verbose) => {
            verboseMode = verbose
          },
          walletActions: {
            send: (destinationRef, amountSats) =>
              sendControlCommand(baseUrl, token, { type: "wallet.send", destinationRef, amountSats }),
            receive: (amountSats) =>
              sendControlCommand(baseUrl, token, { type: "wallet.receive", amountSats }),
            admitPayoutTarget: (kind, ref) =>
              sendControlCommand(baseUrl, token, { type: "wallet.admit-payout-target", kind, ref }),
          },
          assignmentActions: {
            poll: async () =>
              (await sendControlCommand(baseUrl, token, { type: "assignments.poll" })) as Array<{
                assignmentRef: string
                leaseRef: string
                goal: string
                paymentMode: string
                expiresAt: string
              }>,
            accept: (leaseRef) => sendControlCommand(baseUrl, token, { type: "assignments.accept", leaseRef }),
          },
        }),
      catch: (error) => new Error(`Failed to initialize OpenTUI renderer: ${String(error)}`),
    })
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        dashboard.destroy()
        dashboardUi = null
      }),
    )

    yield* ui.attachRuntimeToView(runtime, { verbose: verboseMode })
    yield* logMessage(runtime, "info", `[Codex] Attach composer ${composerBackend.statusLine ?? "mode: unavailable"}.`, {
      transient: true,
    })
    yield* logMessage(runtime, "info", `Attaching to Pylon node at ${baseUrl}...`, { transient: true })

    let snapshotSeen = false
    yield* runControlClient(baseUrl, token, {
      onSnapshot: (snapshot) => {
        Effect.runFork(
          Effect.gen(function* () {
            yield* applyRemotePanes(runtime, snapshot)
            if (!snapshotSeen) {
              snapshotSeen = true
              yield* publishLogEntries(runtime, snapshot.logFeed)
              yield* logMessage(runtime, "info", `Attached. Restored ${snapshot.logFeed.length} log lines from the node.`, { transient: true })
            } else {
              yield* logMessage(runtime, "info", "Reconnected to node.", { transient: true })
            }
          }),
        )
      },
      onEvent: (event) => {
        Effect.runFork(applyRemoteEvent(runtime, event))
      },
      onStatus: (status, detail) => {
        if (status === "reconnecting") logToUi(`[Attach] ${detail ?? "reconnecting"}`, "info")
      },
    })

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

function parseKeyValueOptions(args: string[]) {
  return parsePresenceOptions(args)
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

async function main() {
  const args = Bun.argv.slice(2)

  if (args[0] === "bootstrap") {
    try {
      const options = parseBootstrapArgs(args.slice(1))
      const summary = createBootstrapSummary(options, Bun.env)
      if (!summary.platform.supported) {
        process.stderr.write(
          `Pylon v0.3.0-rc2 supports macOS and Linux only. Current platform: ${summary.platform.current}\n`,
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
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("presence commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = await createPresenceBootstrapSummary(presenceArgs, Bun.env)
      const clientOptions = {
        agentToken: Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }
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
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
        const result = await preflightLegacySparkMigration({
          destinationInvoiceReady: sparkOptions["destination-invoice-ready"] === true,
          dryRun: sparkOptions.execute !== true,
          env: Bun.env,
          identityMnemonicPath: stringPsionicOption(sparkOptions, "identity-mnemonic-path") ?? state.paths.identityMnemonic,
          mnemonicRecoveryRequested: sparkOptions["mnemonic-recovery"] === true,
          yes: sparkOptions.yes === true,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "blocked" ? 1 : 0
        return
      }
      const options = parseKeyValueOptions(walletArgs)
      if (command === "report-readiness") {
        const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
        const result = await receiveWithMdk(amount)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
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
        const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
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
    await Effect.runPromise(
      Effect.scoped(runHeadlessNode).pipe(
        Effect.catch((error) => Console.error(`Pylon node-core crashed: ${error.message}`)),
      ),
    )
    process.exit(0)
  }

  if (args[0] === "attach") {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), "pylon attach")
      rejectClaudeLocalDangerForPublicPath(args.slice(1), "pylon attach")
    } catch (error) {
      process.stderr.write(`Pylon attach failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
    const attachArgs = args.slice(1).filter((arg) => !arg.startsWith("--"))
    const options = parseKeyValueOptions(args.slice(1).filter((arg, index, list) => {
      if (arg.startsWith("--")) return true
      return index > 0 && (list[index - 1] ?? "").startsWith("--")
    }))
    const baseUrl = (attachArgs[0] ?? `http://127.0.0.1:${Bun.env.PYLON_CONTROL_PORT ?? defaultControlPort}`).replace(/\/$/, "")
    let token = options.token ?? Bun.env.PYLON_CONTROL_TOKEN
    if (!token) {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const tokenFile = Bun.file(controlTokenPath(summary.paths.home))
      token = (await tokenFile.exists()) ? (await tokenFile.text()).trim() : undefined
    }
    if (!token) {
      process.stderr.write("pylon attach requires --token, PYLON_CONTROL_TOKEN, or a local control-token file\n")
      process.exitCode = 1
      return
    }
    await ensureSolidRuntime()
    await Effect.runPromise(
      Effect.scoped(runPylonAttach(baseUrl, token)).pipe(
        Effect.catch((error) => Console.error(`Pylon attach failed: ${error.message}`)),
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

  // Dashboard path needs the Solid JSX transform before src/tui/*.tsx can
  // load; re-execs once with --preload when launched without the bunfig
  // preload (e.g. the packaged bin from an arbitrary cwd).
  await ensureSolidRuntime()

  await Effect.runPromise(
    Effect.scoped(runPylonNode).pipe(
      Effect.catch((error) =>
        Console.error(`Pylon v0.3 crashed on startup: ${error.message}`)
      )
    )
  )
  // Scope is closed: services interrupted, renderer stopped, terminal
  // restored. Exit explicitly so lingering library handles cannot hold the
  // process open after a deliberate shutdown.
  process.exit(0)
}

await main()
