#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"

import { openLegacySqliteDatabase, type LegacySqliteDatabase as Database } from '@openagentsinc/sqlite-runtime'
import { readFile } from 'node:fs/promises'
import { existsSync, writeSync } from 'node:fs'
import { loadClaudeAgentConfig, probeClaudeAgentReadiness, withClaudeAgentCapability } from './claude-agent.js'
import {
  CODEX_AGENT_CAPABILITY_REF,
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  withCodexAgentCapability,
} from './codex-agent.js'
import {
  loadPylonAccountRegistry,
  pylonClaudeAccountHomeHasAuth,
  resolvePylonAccountSelection,
} from './account-registry.js'
import { withWorkspaceMaterializerCapability } from './workspace-materializer.js'
import {
  ARTANIS_FORUM_SLUG,
  appendMemory,
  composeAskArtanisBody,
  forumPostTopic,
  forumReadTopic,
  forumReply,
  readMemories,
  resolveModelAdapter,
} from './agent-surface.js'
import { Console, Deferred, Effect, PubSub } from 'effect'
import { classifyServiceLogLevel, formatLogTimestamp, type PylonLogLevel } from './node/state.js'
import {
  forkLogPersistence,
  forkNodeServices,
  logMessage,
  makePylonNodeRuntime,
  seedLogFeed,
  superviseLoop,
  type PylonNodeRuntime,
} from './node/runtime.js'
import { createFeedLogWriter, readPersistedLogTail } from './node/log-persist.js'
import {
  buildBrokerRegistrationBody,
  postNodeRegistration,
  type BrokerRegistrationHosts,
} from './node/discovery-register.js'
import { createIntentQueue } from './node/intent-intake.js'
import { createApprovalQueue } from './node/approval-queue.js'
import { openPylonNodeFleetRunActivationService } from './node/fleet-run-activation.js'
import { openPylonOwnedStandingFleetRunExecutor } from './orchestration/fleet-run-owned-standing-executor.js'
import { makePylonRemoteManagedCloudFleetRunCapacity } from './orchestration/fleet-run-managed-cloud-runner.js'
import {
  disabledPylonFleetRunIntakePollerStatus,
  openPylonFleetRunIntakePoller,
} from './node/fleet-run-intake-poller.js'
import { makePylonFleetRunHttpIntake } from './orchestration/fleet-run-http-intake.js'
import { makePylonFleetRunExecutionHttpPort } from './orchestration/fleet-run-execution-reporter.js'
import { openPylonFleetRunRemoteIntakeService } from './orchestration/fleet-run-remote-intake.js'
import { createCoordinatorRuntime, type CoordinatorRuntime } from './coordinator/coordinator-runtime.js'
import { evaluateShipSpendGate } from './coordinator/ship-spend-gate.js'
import { scanClaudeSessions, toEventRows, toSessionListEntry, type ExternalSession } from './node/external-sessions.js'
import { scanCodexSessions } from './node/codex-sessions.js'
import { homedir } from 'node:os'
import {
  defaultControlPort,
  ensureControlToken,
  controlTokenPath,
  startControlServer,
  type ControlCommandActions,
  type ControlCommand,
} from './node/control-server.js'
import { createSupervisedAppleFmStatusAction } from './node/apple-fm-supervised-status.js'
import {
  appleFmBackendCapacityRefs,
  collectPylonAppleFmStatus,
  withAppleFmBackendCapabilities,
} from './node/apple-fm-status.js'
import { createAppleFmSupervisedLaunch, type AppleFmSupervisedLaunch } from './node/apple-fm-supervised-launch.js'
import {
  createControlSessionActions,
  type ControlSessionSpawnCommand,
  type ControlSessionProjection,
} from './node/control-sessions.js'
import { PylonPortableSessionOperationLedger } from './portable-session-operation-ledger.js'
import { makePylonPortableCheckpointArtifactClient } from './portable-checkpoint-artifact-client.js'
import {
  openPylonPortablePhaseProductionWorker,
  portablePhaseWorkerInstanceRef,
  pylonPrivatePortablePhaseContexts,
} from './portable-phase-production.js'
import {
  makeDurablePylonPortablePhaseTargetResolver,
  openPylonPortablePhaseContextAdmissionStore,
  type PylonPortablePhaseContextAdmissionStore,
} from './portable-phase-context-admission.js'
import { resolveCloudControlConfig } from './cloud-control-client.js'
import { makeCloudControlSessionExecutor } from './openagents-cloud-provider.js'
import { collectPylonContextProjection } from './context-projection.js'
import { collectPylonDevDoctor } from './dev-doctor.js'
import { parsePylonAccountsConnectArgs, pylonCodexAuthCliOutcome, runPylonAccountsConnect } from './account-connect.js'
import {
  parsePylonAuthArgs,
  resolveOpenAgentsAgentToken,
  runPylonAuthClaude,
  runPylonAuthCodex,
  runPylonAuthOpenAgents,
} from './auth.js'
import {
  collectPylonAccountsList,
  collectPylonCodexAccountsLocal,
  collectPylonAccountsStatus,
  collectPylonAccountsUsage,
  parsePylonAccountsStatusArgs,
  parsePylonAccountsUsageArgs,
  resolvePylonAccountUsageRefreshTargets,
  type PylonAccountsUsageArgs,
} from './account-usage.js'
import { reportDirectLocalCodexUsage } from './codex-direct-local-usage-reporter.js'
import { recordCodexUsageRefreshFailure, recordCodexUsageRefreshSuccess } from './account-usage-refresh-health.js'
import {
  claudeProviderDisabledFailure,
  clearClaudeAccountHealth,
  recordClaudeProviderDisabled,
} from '@openagentsinc/pylon-core/custody/claude-account-health-ledger'
import { collectPylonOperatorAccountStatus } from './account-status.js'
import {
  collectHarnessMaintenanceStatus,
  normalizeMaintenanceHarness,
  persistHarnessMaintenanceReceipt,
  projectPublicHarnessMaintenanceReceipt,
  runHarnessMaintenanceUpdate,
  type HarnessInstallChannel,
} from '@openagentsinc/pylon-core/custody/harness-maintenance'
import { createCodexFleetOffloadPlan, parseCodexFleetOffloadArgs } from './codex-fleet-offload.js'
import {
  recordPylonDevCodexRun,
  runPylonDevApply,
  runPylonDevCheck,
  runPylonDevReload,
  type PylonDevCommandSpec,
} from './dev-loop.js'
import {
  planVirtualMergeQueuePrFastForward,
  type VirtualMergeQueuePrFastForwardRequest,
  type VirtualMergeQueueProjection,
} from './blueprint-gates/virtual-merge-queue.js'
import { rejectCodexLocalDangerForPublicPath, runCodexComposerStream } from './codex-composer.js'
import { reprimePylonCodexAccountAuthFromCustody } from './codex-custody-reprime.js'
import { rejectClaudeLocalDangerForPublicPath, runClaudeComposerStream } from './claude-composer.js'
import { runProbeCli } from '../packages/runtime/src/index.js'
import { PYLON_VERSION } from './version.js'
import { ControlEndpointError, runControlCommand } from './node/control-cli.js'
import { runSessionsExec, type ApprovalPolicy, type SessionsExecControl } from './node/sessions-exec.js'
import { parseSessionsBatchTasks, runSessionsBatch } from './node/sessions-batch.js'
import { createBoundedAutoApprovalPolicy } from './node/auto-approval-policy.js'
import { PYLON_COMMAND_CATALOG, findCommandEntry, projectCommandCatalog } from './cli-catalog.js'
import {
  formatPublicActivityCliText,
  runPublicActivityCliCommand,
  type PublicActivityCliCommand,
} from './public-activity-cli.js'
import { readFile as readFileForPacket } from 'node:fs/promises'
import {
  createBootstrapSummary,
  formatBootstrapText,
  parseBootstrapArgs,
  selectPylonHomeResolution,
  writeBootstrapFiles,
  type BootstrapSummary,
} from './bootstrap.js'
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  loadOrCreatePresenceState,
  projectPublicStatus,
  writePresenceState,
  writeRuntimeState,
  type PylonLocalState,
  type PylonPaths,
} from './state.js'
import {
  activeCodingRunCounts,
  activeCodingRunCountsByAccount,
  activeCodingRunCountsByAccountFromAssignmentLeases,
  activeCodingRuns,
  activeCodingRunCountsFromAssignmentLeases,
  maxActiveCodingRunAccountCounts,
  maxActiveCodingRunCounts,
  type PylonActiveCodingRunAccountCounts,
  type PylonActiveCodingRunCounts,
} from './active-assignment-runs.js'
import {
  completePylonLink,
  codexAccountCapacityRefs,
  codexBusyByAccount,
  claudeBusyByAccount,
  codingServiceCapacityFromRuntime,
  codingServiceCapacityRefs,
  DEFAULT_CODEX_PER_ACCOUNT_CONCURRENCY,
  localClaudeAccountCapacities,
  localCodexAccountCapacities,
  localCodingServiceReadyCounts,
  presenceClientOptionsFromEnv,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
} from './presence.js'
import {
  acceptAssignment,
  pollAssignments,
  runNoSpendAssignment,
  submitAssignmentCloseout,
  submitAssignmentProgress,
  type AssignmentRunLifecycleEvent,
  type AssignmentCloseout,
  type AssignmentProgress,
  type PylonAssignmentLease,
} from './assignment.js'
import { discoverHostInventory } from './inventory.js'
import { autoUpdateDisabledReason, checkForUpdate, downloadAndApply, resolveSelfBinaryPath } from './self-update.js'
import { createOperatorSnapshot, formatOperatorSnapshotText } from './operator.js'
import { approveLaborFirstRun } from './labor.js'
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
} from './work-requester.js'
import {
  buildPylonKhalaGitCheckoutWorkspace,
  issuePylonKhalaRequest,
  readPylonKhalaAssignmentTraceStatus,
  readPylonKhalaCloseout,
  readPylonKhalaProof,
  readPylonKhalaStatus,
  resumePylonKhalaRequest,
  type PylonKhalaWorkflow,
} from './khala-requester.js'
import {
  buildPylonKhalaBurndownPlan,
  localPylonTargetRef,
  parseKhalaBurndownIssueNumbers,
  readKhalaRoadmapIssueNumbers,
  runPylonKhalaBurndownPlan,
} from './khala-burndown.js'
import {
  buildPylonKhalaDispatchPlan,
  normalizeKhalaDispatchCandidateRefs,
  type KhalaDispatchAccountTarget,
} from './khala-dispatch.js'
import {
  buildPylonKhalaSpawnPlan,
  repeatedKhalaSpawnObjectives,
  runPylonKhalaSpawnPlan,
  type PylonKhalaSpawnWorkflow,
} from './khala-spawn.js'
import { createPylonOrchestrationStore } from './orchestration/store.js'
import type { PylonDispatchBreakerSnapshot } from './dispatch-failure-taxonomy.js'
import { pylonKhalaMcpConfig, runPylonKhalaMcpStdio } from './khala-mcp.js'
import { hostname } from 'node:os'

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
function startDiscoveryHeartbeat(opts: { controlPort: number; controlToken: string; boundHost: string }): void {
  const broker = Runtime.env.OA_DISCOVERY_BROKER
  if (!broker) return
  const owner = Runtime.env.OA_DISCOVERY_OWNER ?? 'chris'
  const nodeRef = Runtime.env.PYLON_NODE_REF ?? hostname()
  // An externally-reachable HTTPS endpoint (e.g. `tailscale serve`): advertised
  // verbatim, reachable on any network the phone's tailnet covers, and ATS-safe.
  const publicUrl = Runtime.env.OA_DISCOVERY_PUBLIC_URL
  const hosts: BrokerRegistrationHosts = {}
  if (opts.boundHost.startsWith('127.')) hosts.loopback = opts.boundHost
  else if (opts.boundHost.startsWith('100.')) hosts.tailnet = opts.boundHost
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
function logToUi(message: string, level: PylonLogLevel = 'verbose') {
  if (nodeRuntime) {
    Effect.runFork(logMessage(nodeRuntime, level, message))
    return
  }
  if (level !== 'verbose' || verboseMode) {
    void Effect.runPromise(Console.log(`[BOOT] ${message}`))
  }
}

// Effect-native logging helper (defaults to verbose — hidden unless
// --verbose / PYLON_VERBOSE=1)
const log = (message: string, level: PylonLogLevel = 'verbose') => Effect.sync(() => logToUi(message, level))

// CL-34/CL-35 work-intent intake: the phone composes an "ask" and submits it;
// the node enqueues it (server-generates the id + timestamp) for the coordinator
// to plan and fan out. Persisted to the Pylon home so intents survive restart.
function makeIntentActions(intentQueue: ReturnType<typeof createIntentQueue>) {
  return {
    submit: async (input: { title: string; body: string; scopeHint?: string; submittedByClientRef?: string }) => {
      const title = input.title.trim()
      const body = input.body.trim()
      if (title.length === 0) throw new Error('intent.submit requires a non-empty title')
      return intentQueue.enqueue({
        intentId: `intent.${crypto.randomUUID()}`,
        title,
        body,
        ...(input.scopeHint && input.scopeHint.trim().length > 0 ? { scopeHint: input.scopeHint.trim() } : {}),
        submittedByClientRef: input.submittedByClientRef?.trim() || 'mobile',
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

function makeApprovalActions(paths: PylonPaths) {
  return {
    list: async () => ({ approvals: approvalQueue.list() }),
    resolve: async (input: { approvalRef: string; decision: 'approve' | 'deny' | 'answer'; answer?: string }) => {
      const result = approvalQueue.resolve(
        input.approvalRef,
        input.decision,
        input.answer ? { answer: input.answer } : undefined,
      )
      // Granting a labor first-run approval is the one side effect of "approve".
      if (result.applied && input.decision === 'approve' && result.resolved?.jobType) {
        try {
          await approveLaborFirstRun({
            paths,
            approvedByRef: 'operator.control',
            jobType: result.resolved.jobType as Parameters<typeof approveLaborFirstRun>[0]['jobType'],
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
export function enqueueLaborApproval(input: {
  approvalRef: string
  jobType?: string
  policyRef?: string
  prompt?: string
}) {
  approvalQueue.enqueue({
    approvalRef: input.approvalRef,
    kind: 'labor_first_run',
    prompt: input.prompt ?? `Approve first run of ${input.jobType ?? 'labor job'}?`,
    ...(input.jobType ? { jobType: input.jobType } : {}),
    ...(input.policyRef ? { policyRef: input.policyRef } : {}),
  })
}

type ExternalSessionStore = {
  list: () => ExternalSession[]
  find: (ref: string) => ExternalSession | undefined
}

// #4951 external agent sessions: poll the host Claude Code logs and expose them
// as read-only sessions, merged into session.list / session.events so the MAIN
// conversation (Pylon-managed or not) + its sub-agents show in Autopilot.
function startExternalSessionTailer(): ExternalSessionStore {
  let sessions: ExternalSession[] = []
  const projectsRoot = `${homedir()}/.claude/projects`
  const codexRoot = `${homedir()}/.codex/sessions`
  const poll = (): void => {
    const now = Date.now()
    let claude: ExternalSession[] = []
    let codex: ExternalSession[] = []
    try {
      claude = scanClaudeSessions({
        projectsRoot,
        nowMs: now,
        maxAgeMs: 900_000,
        maxSessions: 12,
      })
    } catch {
      // best-effort
    }
    try {
      codex = scanCodexSessions({
        sessionsRoot: codexRoot,
        nowMs: now,
        maxAgeMs: 900_000,
        maxSessions: 12,
      })
    } catch {
      // best-effort
    }
    sessions = [...claude, ...codex]
  }
  poll()
  const timer = setInterval(poll, 1000)
  timer.unref?.()
  return {
    list: () => sessions,
    find: (ref) => sessions.find((s) => s.sessionRef === ref || (s.aliasSessionRefs ?? []).includes(ref)),
  }
}

function externalSessionEventStream(ref: string, store: ExternalSessionStore): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder()
  const sseFrame = (payload: unknown) => `data: ${JSON.stringify(payload)}\n\n`
  const seen = new Set<string>()
  let timer: ReturnType<typeof setInterval> | undefined
  let closed = false

  const close = (controller: ReadableStreamDefaultController<Uint8Array>) => {
    if (closed) return
    closed = true
    if (timer !== undefined) clearInterval(timer)
    try {
      controller.close()
    } catch {
      // already closed
    }
  }

  const flush = (controller: ReadableStreamDefaultController<Uint8Array>) => {
    const session = store.find(ref)
    if (session === undefined) {
      close(controller)
      return
    }
    for (const row of toEventRows(session)) {
      const key = `${row.observedAt}\0${row.phase}\0${row.messageText}\0${row.messageFull ?? ''}`
      if (seen.has(key)) continue
      seen.add(key)
      controller.enqueue(encoder.encode(sseFrame(row)))
    }
    if (session.state !== 'running') close(controller)
  }

  return new ReadableStream<Uint8Array>({
    start(controller) {
      flush(controller)
      if (closed) return
      timer = setInterval(() => flush(controller), 500)
      timer.unref?.()
    },
    cancel() {
      closed = true
      if (timer !== undefined) clearInterval(timer)
    },
  })
}

// Wrap the Pylon session actions so the control API also serves external
// sessions (read-only). spawn/cancel/artifact stay Pylon-only; eventStream
// tails the read-only external projection.
function wrapSessionsWithExternal<
  T extends {
    list: () => Promise<any[]>
    events: (ref: string) => Promise<any>
    eventStream?: (ref: string) => ReadableStream<Uint8Array>
  },
>(raw: T, store: ExternalSessionStore): T {
  return {
    ...raw,
    list: async () => {
      const nowIso = new Date().toISOString()
      const pylon = await raw.list()
      const external = store.list().map((s) => toSessionListEntry(s, nowIso))
      return [...pylon, ...external]
    },
    events: async (ref: string) => {
      const s = store.find(ref)
      if (s !== undefined) {
        return {
          sessionRef: ref,
          eventsPath: '',
          state: s.state === 'running' ? 'running' : 'completed',
          recentEvents: toEventRows(s),
        }
      }
      return raw.events(ref)
    },
    eventStream: ((ref: string) => {
      const s = store.find(ref)
      if (s !== undefined) return externalSessionEventStream(ref, store)
      if (raw.eventStream === undefined) throw new Error('session not found')
      return raw.eventStream(ref)
    }) as T['eventStream'],
  }
}

// CL-36 coordinator: wire the intent queue to the session executor so a
// submitted ask is planned + fanned out into coding sessions automatically.
// Each fan-out part runs in a fresh detached worktree off HEAD. Enabled unless
// OA_COORDINATOR=0. Spend note: this auto-runs coding agents on owner-composed
// asks (owner-authorized: the whole point of the loop).
function startCoordinator(
  intentQueue: ReturnType<typeof createIntentQueue>,
  sessions: {
    spawn: (cmd: any) => Promise<{ sessionRef: string }>
    list: () => Promise<Array<{ sessionRef: string; state: string }>>
  },
): CoordinatorRuntime | null {
  if (Runtime.env.OA_COORDINATOR === '0') return null
  const repoRoot = process.cwd()
  const runtime = createCoordinatorRuntime({
    intentQueue,
    spawnSession: async (input) => {
      const result = await sessions.spawn({
        type: 'session.spawn',
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
      const safe = intentId.replace(/[^a-zA-Z0-9._-]/g, '-')
      const dir = `/tmp/oa-coord/${safe}-${index}`
      await Runtime.spawn(['git', 'worktree', 'remove', '--force', dir], {
        cwd: repoRoot,
        stderr: 'ignore',
        stdout: 'ignore',
      }).exited
      const proc = Runtime.spawn(['git', 'worktree', 'add', '--detach', '--force', dir, 'HEAD'], {
        cwd: repoRoot,
        stderr: 'pipe',
        stdout: 'ignore',
      })
      const code = await proc.exited
      if (code !== 0) throw new Error(`git worktree add failed (${code}) for ${dir}`)
      return dir
    },
    // CL-37/CL-41: supply the ship-step context. Fingerprints + changed paths
    // come from env (publish pipeline sets them); the spend gate is fail-safe —
    // with no configured budget (default 0) it DENIES, so an autonomous ship
    // escalates to the owner rather than spending without an explicit budget.
    shipContext: async () => {
      const env = Runtime.env
      const num = (v: string | undefined, d: number) => {
        const n = Number(v)
        return Number.isFinite(n) ? n : d
      }
      const gate = evaluateShipSpendGate({
        action: 'autonomous_ship',
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
        previousRuntimeFingerprint: (env.OA_SHIP_PREV_FINGERPRINT ?? '').trim(),
        nextRuntimeFingerprint: (env.OA_SHIP_NEXT_FINGERPRINT ?? '').trim(),
        changedPaths: (env.OA_SHIP_CHANGED_PATHS ?? '')
          .split(',')
          .map((p) => p.trim())
          .filter((p) => p.length > 0),
        spendGate: { decision: gate.decision },
      }
    },
    recordShip: (intentId, decision) => {
      logToUi(
        `[ship] ${intentId} mode=${decision.shipMode} decision=${decision.decision} eligible=${decision.eligible} (${decision.reason})`,
        'info',
      )
      // CL-38/CL-39: auto-execute the ship via OUR pipeline (no Expo/EAS cloud).
      // Triple-gated: eligible (spend-gate allowed) AND decision auto AND the
      // explicit opt-in OA_SHIP_AUTO_EXECUTE=1. Dormant by default — with no
      // budget the spend gate denies, so this never fires unexpectedly.
      if (!decision.eligible || decision.decision !== 'auto') return
      if (Runtime.env.OA_SHIP_AUTO_EXECUTE !== '1') {
        logToUi(
          `[ship] ${intentId} eligible for ${decision.shipMode} — auto-execute disabled (set OA_SHIP_AUTO_EXECUTE=1)`,
          'info',
        )
        return
      }
      const repoRoot = process.cwd()
      if (decision.shipMode === 'ota') {
        // CL-38: auto OTA publish to our updates server when OTA-eligible.
        logToUi(`[ship] ${intentId} auto OTA publish -> publish-ota.sh`, 'info')
        Runtime.spawn(['bash', 'apps/oa-updates/scripts/publish-ota.sh'], {
          cwd: repoRoot,
          stdout: 'ignore',
          stderr: 'ignore',
        })
      } else if (decision.shipMode === 'rebuild') {
        // Native rebuilds require app-specific signing and release authority.
        // Pylon records and escalates the decision; it does not launch a retired
        // client script or infer a replacement release command.
        logToUi(
          `[ship] ${intentId} rebuild needed — escalating to the current app release owner`,
          'info',
        )
      }
    },
    log: (message) => logToUi(message, 'info'),
  })
  runtime.start(5000)
  return runtime
}

// Node-side assignment actions (issue #4741). Available only when an
// OpenAgents base URL is configured; leases are cached between poll and
// accept so accept can resolve a leaseRef back to the full lease payload.
function makeAssignmentActions() {
  const baseUrl = Runtime.env.PYLON_OPENAGENTS_BASE_URL
  if (!baseUrl) return null
  const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
  const agentToken = Runtime.env.OPENAGENTS_AGENT_TOKEN
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
      if (!lease) throw new Error('lease not found - refresh assignments first')
      return acceptAssignment(summary, lease, clientOptions)
    },
  }
}

function makeSessionActions(summary: ReturnType<typeof createBootstrapSummary>) {
  const portableDatabase = openLegacySqliteDatabase(`${summary.paths.home}/portable-session-operations.sqlite`)
  return createControlSessionActions({
    summary,
    portableLedger: new PylonPortableSessionOperationLedger(portableDatabase),
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
  const configured = Runtime.env.PYLON_CODEX_CWD ?? Runtime.env.PYLON_ACTIVE_REPO
  return configured && configured.trim().length > 0 ? configured : process.cwd()
}

async function runAccountsUsageRefresh(
  summary: ReturnType<typeof createBootstrapSummary>,
  options: PylonAccountsUsageArgs,
): Promise<{
  readonly attemptedCount: number
  readonly blockerRefs: readonly string[]
}> {
  const targets = await resolvePylonAccountUsageRefreshTargets(summary, options, { env: Runtime.env })
  if (targets.length > 0 && targets.every((target) => target.provider === 'grok')) {
    throw new Error('Grok account usage refresh is unavailable; Grok usage truth remains not_measured')
  }
  let attemptedCount = 0
  let skippedGrok = false
  const failureBlockerRefs: string[] = []
  const prompt = 'Reply with exactly: ok.'
  for (const target of targets) {
    if (target.provider === 'grok') {
      skippedGrok = true
      continue
    }
    attemptedCount += 1
    try {
      if (target.provider === 'codex') {
        const config = await loadCodexAgentConfig(summary)
        const custodyReprime = await reprimePylonCodexAccountAuthFromCustody({
          account: target.account,
          env: Runtime.env,
        })
        if (custodyReprime.status === 'blocked') {
          continue
        }
        await runCodexComposerStream(prompt, {
          account: target.account,
          approvalPolicy: 'never',
          config,
          cwd: codexComposerWorkingDirectory(),
          env: custodyReprime.env,
          executionMode: 'local_bounded',
          ...(config.model === undefined ? {} : { model: config.model }),
          networkAccessEnabled: false,
          sandboxMode: 'read-only',
          timeoutMs: 60_000,
          usageStateSummary: summary,
        })
        await recordCodexUsageRefreshSuccess(summary, target.accountRefHash)
      } else if (target.provider === 'claude_agent') {
        const config = await loadClaudeAgentConfig(summary)
        await runClaudeComposerStream(prompt, {
          account: target.account,
          config,
          cwd: codexComposerWorkingDirectory(),
          env: Runtime.env,
          executionMode: 'local_bounded',
          maxTurns: 1,
          ...(config.model === undefined ? {} : { model: config.model }),
          permissionMode: 'acceptEdits',
          timeoutMs: 60_000,
          usageStateSummary: summary,
        })
        await clearClaudeAccountHealth(summary, target.accountRefHash)
      }
    } catch (error) {
      if (target.provider === 'codex') {
        failureBlockerRefs.push(
          ...(await recordCodexUsageRefreshFailure(summary, {
            accountRefHash: target.accountRefHash,
            error,
          })),
        )
      }
      if (target.provider === 'claude_agent' && claudeProviderDisabledFailure(error)) {
        await recordClaudeProviderDisabled(summary, target.accountRefHash)
        failureBlockerRefs.push('blocker.pylon.claude_account.provider_disabled')
      }
      // Readiness and missing provider snapshots are reported in the final
      // JSON truth tiers; refresh failure must not leak raw provider errors.
    }
  }
  return {
    attemptedCount,
    blockerRefs: [
      ...(skippedGrok ? ['blocker.pylon.accounts_usage.grok_refresh_not_measured'] : []),
      ...failureBlockerRefs,
    ],
  }
}

const assignmentWorkerIntervalMs = () => {
  const seconds = Number(Runtime.env.PYLON_ASSIGNMENT_WORKER_INTERVAL_SECONDS ?? 30)
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
      if (result.ok && result.closeout) {
        log(`[Assignments] Completed no-spend assignment ${result.closeout.assignmentRef}.`)
      } else if (result.reason !== 'no no-spend assignment lease available') {
        log(`[Assignments] No-spend run skipped: ${JSON.stringify(result)}`, 'verbose')
      }
    } catch (error) {
      log(`[Assignments] Worker loop error: ${error instanceof Error ? error.message : String(error)}`, 'info')
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
}

// Builds the inventory-only operator-pane text the telemetry service publishes.
// Payment capability is intentionally retired and cannot be inferred from host
// inventory or substituted with paid capacity.
function operatorTextFromInventory(inventory: Awaited<ReturnType<typeof discoverHostInventory>>) {
  return formatOperatorSnapshotText(createOperatorSnapshot({ inventory }))
}

const retiredMoneyControlActions: ControlCommandActions = {
  walletSend: async () => ({
    ok: false,
    reason: 'money_capability_retired',
    mutationAllowed: false,
  }),
  walletReceive: async () => ({
    ok: false,
    reason: 'money_capability_retired',
    mutationAllowed: false,
  }),
  walletAdmitPayoutTarget: async () => ({
    ok: false,
    reason: 'money_capability_retired',
    mutationAllowed: false,
  }),
}

// Headless node-core (issue #4740): services + event stream + control API,
// no TUI, no Solid. Logs print to stdout with the same verbosity rules.
const runHeadlessNode = Effect.gen(function* () {
  verboseMode = Runtime.argv.includes('--verbose') || Runtime.env.PYLON_VERBOSE === '1'
  const runtime = yield* makePylonNodeRuntime
  nodeRuntime = runtime

  const shutdown = yield* Deferred.make<void>()
  // Owns the supervised local Apple FM bridge launcher (when one is wired below).
  // Stopped on shutdown so a backoff timer / live child does not outlive the node.
  let appleFmSupervisedLaunch: AppleFmSupervisedLaunch | null = null
  const requestShutdown = () => {
    Effect.runFork(Deferred.succeed(shutdown, void 0))
    // Stop supervising the local Apple FM bridge: cancel any pending backoff
    // restart and kill a live helper child. Best-effort; never blocks the exit.
    appleFmSupervisedLaunch?.stop()
    // GPU/native teardown (3D scene, WebGPU buffers) can wedge; never hold
    // the terminal hostage on exit.
    setTimeout(() => process.exit(0), 2000)
  }
  process.once('SIGINT', requestShutdown)
  process.once('SIGTERM', requestShutdown)

  // Stdout logger: live events only (the persisted tail is for attach
  // scrollback, not for replaying onto stdout).
  const subscription = yield* PubSub.subscribe(runtime.events)
  yield* Effect.forkScoped(
    Effect.gen(function* () {
      while (true) {
        const event = yield* PubSub.take(subscription)
        if (event.type === 'log' && (event.level !== 'verbose' || verboseMode)) {
          process.stdout.write(`[${formatLogTimestamp(event.at)}] ${event.message}\n`)
        }
      }
    }),
  )

  const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
  const persistedTail = yield* Effect.promise(() =>
    readPersistedLogTail(bootstrapSummary.paths.home, 300).catch(() => []),
  )
  if (persistedTail.length > 0) {
    yield* seedLogFeed(runtime, persistedTail)
  }
  const feedWriter = createFeedLogWriter(bootstrapSummary.paths.home, {
    onError: (message) => logToUi(`[FeedLog] Persistence disabled: ${message}`, 'info'),
  })
  yield* forkLogPersistence(runtime, feedWriter)

  const controlToken = yield* Effect.promise(() => ensureControlToken(bootstrapSummary.paths.home))
  const controlPort = Number(Runtime.env.PYLON_CONTROL_PORT ?? defaultControlPort)
  const headlessAssignmentActions = makeAssignmentActions()
  const headlessSessionActions = makeSessionActions(bootstrapSummary)
  const headlessExternalTailer = startExternalSessionTailer()
  const headlessSessionsWithExternal = wrapSessionsWithExternal(headlessSessionActions, headlessExternalTailer)
  const headlessIntentQueue = createIntentQueue({
    persistPath: `${bootstrapSummary.paths.home}/intents.json`,
  })
  const headlessCoordinatorHolder: CoordinatorHolder = { rt: null }
  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  const presenceBaseUrl = Runtime.env.PYLON_OPENAGENTS_BASE_URL
  const currentPresenceClientOptions = () =>
    presenceClientOptionsFromEnv({
      baseUrl: presenceBaseUrl ?? '',
      env: Runtime.env,
    })
  const presenceClientOptions = currentPresenceClientOptions()
  let portablePhaseContextStore: PylonPortablePhaseContextAdmissionStore | null = null
  if (Runtime.env.PYLON_PORTABLE_PHASE_WORKER === '1') {
    const targetRef = Runtime.env.PYLON_PORTABLE_PHASE_TARGET_REF
    const agentToken = presenceClientOptions.agentToken
    if (presenceBaseUrl === undefined || agentToken === undefined || targetRef === undefined) {
      return yield* Effect.fail(
        new Error('portable phase worker requires authenticated base URL and an exact target ref'),
      )
    }
    const portablePhaseAdmission = yield* Effect.tryPromise({
      try: () => openPylonPortablePhaseContextAdmissionStore({
        databasePath: `${bootstrapSummary.paths.home}/portable-phase/context-admissions.sqlite`,
        pylonRef: localState.identity.pylonRef,
        targetRef,
      }),
      catch: () => new Error('failed to open private portable phase context admission store'),
    })
    portablePhaseContextStore = portablePhaseAdmission.store
    portablePhaseAdmission.store.purge()
    yield* Effect.addFinalizer(() => Effect.sync(() => portablePhaseAdmission.close()))
    const durablePhaseResolver = makeDurablePylonPortablePhaseTargetResolver({
      store: portablePhaseAdmission.store,
      target: targetRef => pylonPrivatePortablePhaseContexts.target(targetRef),
    })
    const portablePhaseWorker = yield* Effect.tryPromise({
      try: () => {
        const artifactTransport = Runtime.env.PYLON_PORTABLE_CHECKPOINT_ARTIFACT_TRANSPORT === '1'
          ? makePylonPortableCheckpointArtifactClient({
              agentToken,
              baseUrl: presenceBaseUrl,
              pylonRef: localState.identity.pylonRef,
              targetRef,
            })
          : undefined
        return openPylonPortablePhaseProductionWorker({
          agentToken,
          baseUrl: presenceBaseUrl,
          pylonRef: localState.identity.pylonRef,
          targetRef,
          workerInstanceRef: portablePhaseWorkerInstanceRef(localState.identity.pylonRef, targetRef),
          stateDirectory: bootstrapSummary.paths.home,
          resolver: durablePhaseResolver,
          ...(artifactTransport === undefined ? {} : { artifactTransport }),
          onTerminalAcknowledged: operationRef => {
            portablePhaseAdmission.store.acknowledgeTerminal(operationRef)
            portablePhaseAdmission.store.purge()
          },
          onFault: (errorRef) => logToUi(`[PortablePhase] Worker stopped: ${errorRef}`, 'info'),
        })
      },
      catch: () => new Error('failed to configure portable phase worker'),
    })
    yield* Effect.addFinalizer(() => Effect.promise(() => portablePhaseWorker.close()))
    yield* logMessage(
      runtime,
      'info',
      `[PortablePhase] Worker active for ${localState.identity.pylonRef} and ${targetRef}.`,
      { transient: true },
    )
  }
  const fleetRunExecutionRemote = yield* Effect.try({
    try: () =>
      presenceClientOptions.agentToken === undefined || presenceBaseUrl === undefined
        ? undefined
        : makePylonFleetRunExecutionHttpPort({
            agentToken: presenceClientOptions.agentToken,
            baseUrl: presenceBaseUrl,
          }),
    catch: () => new Error('failed to configure FleetRun execution projection'),
  })
  const fleetRunActivation = yield* Effect.tryPromise({
    try: () =>
      openPylonNodeFleetRunActivationService({
        summary: bootstrapSummary,
        pylonRef: localState.identity.pylonRef,
        baseUrl: presenceBaseUrl,
        env: Runtime.env,
        ...(presenceClientOptions.agentToken === undefined ? {} : { agentToken: presenceClientOptions.agentToken }),
        ...(fleetRunExecutionRemote === undefined ? {} : { executionRemote: fleetRunExecutionRemote }),
        ...(presenceClientOptions.agentToken === undefined || presenceBaseUrl === undefined
          ? {}
          : {
              openHybridExecutor: (executorInput) =>
                openPylonOwnedStandingFleetRunExecutor({
                  ...executorInput,
                  options: {
                    managedCloud: {
                      capacity: makePylonRemoteManagedCloudFleetRunCapacity({
                        agentToken: presenceClientOptions.agentToken!,
                        baseUrl: presenceBaseUrl,
                        pylonRef: localState.identity.pylonRef,
                      }),
                      adapter: {
                        kind: 'remote',
                        agentToken: presenceClientOptions.agentToken!,
                        baseUrl: presenceBaseUrl,
                      },
                    },
                  },
                }),
            }),
      }),
    catch: () => new Error('failed to open owner-local FleetRun activation authority'),
  })
  yield* Effect.addFinalizer(() => Effect.promise(() => fleetRunActivation.close()))
  const fleetRunIntakePoller = yield* Effect.tryPromise({
    try: async () => {
      const agentToken = presenceClientOptions.agentToken
      if (agentToken === undefined || presenceBaseUrl === undefined) return null
      const configuredInterval = Number(Runtime.env.PYLON_FLEET_RUN_INTAKE_POLL_INTERVAL_MS ?? 5_000)
      const intervalMs =
        Number.isInteger(configuredInterval) && configuredInterval >= 250 && configuredInterval <= 300_000
          ? configuredInterval
          : 5_000
      const remote = makePylonFleetRunHttpIntake({
        agentToken,
        baseUrl: presenceBaseUrl,
      })
      const intake = await openPylonFleetRunRemoteIntakeService({
        activation: fleetRunActivation,
        bootstrap: bootstrapSummary,
        env: Runtime.env,
        pylonRef: localState.identity.pylonRef,
        remote,
      })
      return openPylonFleetRunIntakePoller({ intake, intervalMs })
    },
    catch: () => new Error('failed to configure standing FleetRun intake'),
  })
  if (fleetRunIntakePoller !== null) {
    // Registered after activation so scoped LIFO shutdown stops new intake,
    // drains the current poll, then closes active executors.
    yield* Effect.addFinalizer(() => Effect.promise(() => fleetRunIntakePoller.close()))
  }
  // Local Apple FM bridge supervision is opt-in for now (the signed-installer
  // recut + admitted-Mac from-install smoke are still open). When
  // PYLON_APPLE_FM_SUPERVISE=1 AND a helper is discovered on this host, construct
  // and start the supervised launcher; its public-safe `status()` becomes the
  // provider the apple_fm.status action attaches. When the flag is off OR no
  // helper exists, the launch is inert (supervisorStatus undefined) and the
  // action returns the unsupervised projection byte-for-byte unchanged.
  appleFmSupervisedLaunch =
    Runtime.env.PYLON_APPLE_FM_SUPERVISE === '1' ? createAppleFmSupervisedLaunch({ discover: { env: Runtime.env } }) : null
  const activePortablePhaseContextStore = portablePhaseContextStore
  const controlServer = yield* startControlServer(runtime, {
    token: controlToken,
    actions: {
      ...retiredMoneyControlActions,
      ...(headlessAssignmentActions
        ? {
            assignmentsPoll: () => headlessAssignmentActions.poll(),
            assignmentsAccept: (leaseRef: string) => headlessAssignmentActions.accept(leaseRef),
          }
        : {}),
      sessions: headlessSessionsWithExternal,
      intents: makeIntentActions(headlessIntentQueue),
      accountsList: () => collectPylonAccountsList(bootstrapSummary),
      accountsStatus: (input) =>
        input?.reset && input.accountRef
          ? collectPylonAccountsStatus(
              bootstrapSummary,
              {
                accountRef: input.accountRef,
                provider: null,
                all: false,
                json: true,
                reset: true,
              },
              { env: Runtime.env },
            )
          : input?.detailed
            ? collectPylonAccountsStatus(
                bootstrapSummary,
                {
                  accountRef: null,
                  provider: null,
                  all: true,
                  json: true,
                  reset: false,
                },
                { env: Runtime.env },
              )
            : collectPylonOperatorAccountStatus(bootstrapSummary),
      // The supervisor-status provider comes from the launch lifecycle owner
      // above (undefined unless PYLON_APPLE_FM_SUPERVISE=1 and a helper exists),
      // so by default this is the unsupervised projection unchanged.
      // See node/apple-fm-supervised-launch.ts + node/apple-fm-supervised-status.ts.
      appleFmStatus: createSupervisedAppleFmStatusAction(
        { summary: bootstrapSummary, env: Runtime.env },
        appleFmSupervisedLaunch?.supervisorStatus === undefined
          ? {}
          : { supervisorStatus: appleFmSupervisedLaunch.supervisorStatus },
      ),
      approvals: makeApprovalActions(localState.paths),
      coordinator: makeCoordinatorActions(headlessCoordinatorHolder),
      fleetRuns: fleetRunActivation,
      fleetRunIntakeStatus: async () => fleetRunIntakePoller?.status() ?? disabledPylonFleetRunIntakePollerStatus(),
      ...(activePortablePhaseContextStore === null
        ? {}
        : {
            portablePhaseContextAdmit: async (input: Parameters<PylonPortablePhaseContextAdmissionStore['admit']>[0]) =>
              activePortablePhaseContextStore.admit(input),
          }),
    },
    port: controlPort,
    hostname: Runtime.env.PYLON_CONTROL_HOST ?? '127.0.0.1',
  })
  yield* logMessage(
    runtime,
    'info',
    `Pylon node-core running headless. Steer via the loopback control API at ${controlServer.url} (token: ${controlTokenPath(bootstrapSummary.paths.home)})`,
    { transient: true },
  )
  startDiscoveryHeartbeat({
    controlPort,
    controlToken,
    boundHost: Runtime.env.PYLON_CONTROL_HOST ?? '127.0.0.1',
  })
  // CL-36: close the self-driving loop on the headless node (the launchd path).
  headlessCoordinatorHolder.rt = startCoordinator(headlessIntentQueue, headlessSessionActions)

  yield* logMessage(runtime, 'info', `[Identity] Pylon Nostr npub: ${localState.identity.npub}`, { transient: true })

  yield* forkNodeServices(runtime, {
    wallet: { classify: async () => null },
    telemetry: {
      discoverInventory: () => discoverHostInventory(),
      inspectPsionic: async () => ({ phase: 'archived' }),
      makeOperatorText: operatorTextFromInventory,
    },
    heartbeat: {
      baseUrl: presenceBaseUrl,
      register: () => registerPylon(bootstrapSummary, currentPresenceClientOptions()),
      heartbeat: () => sendHeartbeat(bootstrapSummary, currentPresenceClientOptions()),
    },
  })
  if (presenceBaseUrl && Runtime.env.PYLON_ASSIGNMENT_WORKER === '1') {
    yield* superviseLoop(
      runtime,
      'Assignments',
      Effect.tryPromise({
        try: () =>
          runHeadlessAssignmentWorkerLoop(
            bootstrapSummary,
            {
              ...(Runtime.env.OPENAGENTS_AGENT_TOKEN ? { agentToken: Runtime.env.OPENAGENTS_AGENT_TOKEN } : {}),
              baseUrl: presenceBaseUrl,
            },
            (message, level) => logToUi(message, level ?? classifyServiceLogLevel(message)),
          ),
        catch: (error) => new Error(`assignment worker loop failed: ${String(error)}`),
      }),
    )
  }
  yield* Deferred.await(shutdown)
})

const runtimeCommandNamespaces = new Set(['apple-fm', 'auth', 'backend', 'chat', 'omega'])

function parsePresenceOptions(args: string[]) {
  return parseCliOptions(args)
}

function writeJsonAndExit(value: unknown, exitCode = 0): never {
  // Do not enqueue
  // the final JSON on the patched stdout stream and then wait on its callback:
  // that callback is itself an event-loop dependency and has intermittently
  // kept an otherwise-complete heartbeat alive. Write the small result directly
  // to the stdout descriptor, then terminate regardless of retained handles.
  writeSync(process.stdout.fd, `${JSON.stringify(value, null, 2)}\n`)
  process.exit(exitCode)
}

// Key/value option parser for user commands. Boolean flags like `--json` map to `true` instead of
// erroring "requires a value" (#5038); use `optionString` to read string values.
function parseKeyValueOptions(args: string[]) {
  return parseCliOptions(args)
}

const presenceBootstrapValueOptions = new Set(['capability-ref', 'display-name', 'pylon-ref', 'resource-mode'])

const presenceBootstrapFlagOptions = new Set(['register-openagents'])

async function persistedBootstrapArgs(env: NodeJS.ProcessEnv) {
  const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), env)
  try {
    const config = JSON.parse(await readFile(summary.paths.config, 'utf8')) as Record<string, unknown>
    const args: string[] = []
    const pushString = (key: string, flag: string) => {
      const value = config[key]
      if (typeof value === 'string' && value.length > 0) {
        args.push(flag, value)
      }
    }

    pushString('pylonRef', '--pylon-ref')
    pushString('displayName', '--display-name')
    pushString('resourceMode', '--resource-mode')

    const capabilityRefs = config.capabilityRefs
    if (Array.isArray(capabilityRefs)) {
      for (const ref of capabilityRefs) {
        if (typeof ref === 'string' && ref.length > 0) {
          args.push('--capability-ref', ref)
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
    if (!arg.startsWith('--')) continue
    const key = arg.slice(2)
    if (key === 'agent-token' || key === 'base-url') {
      index += 1
      continue
    }
    if (key === 'json') continue
    if (presenceBootstrapFlagOptions.has(key)) {
      bootstrapArgs.push(arg)
      continue
    }
    if (presenceBootstrapValueOptions.has(key)) {
      const value = args[index + 1]
      if (!value || value.startsWith('--')) {
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
  return createBootstrapSummary(parseBootstrapArgs(['--json', ...argsFromConfig, ...argsFromPresence]), env)
}

function parsePsionicOptions(args: string[]) {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith('--')) continue
    const key = arg.slice(2)
    const value = args[index + 1]
    if (!value || value.startsWith('--')) {
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
  return typeof value === 'string' ? value : undefined
}

type RunningNodeProbe = {
  reachable: boolean
  baseUrl: string
  hasToken: boolean
  token: string | null
}

function controlBaseUrlFromEnv(env: NodeJS.ProcessEnv = Runtime.env): string {
  const explicit = env.PYLON_CONTROL_URL?.trim()
  if (explicit) return explicit.replace(/\/+$/, '')
  const host = env.PYLON_CONTROL_HOST ?? '127.0.0.1'
  const port = Number(env.PYLON_CONTROL_PORT ?? defaultControlPort)
  return `http://${host}:${Number.isFinite(port) ? port : defaultControlPort}`
}

function remoteReadRequested(args: string[], env: NodeJS.ProcessEnv = Runtime.env): boolean {
  return args.includes('--remote') || args.includes('--connect') || env.PYLON_CONNECT_REMOTE === '1'
}

async function probeRunningNode(state: PylonLocalState, env: NodeJS.ProcessEnv = Runtime.env): Promise<RunningNodeProbe> {
  const baseUrl = controlBaseUrlFromEnv(env)
  let token: string | null = null
  const envToken = env.PYLON_CONTROL_TOKEN?.trim()
  if (envToken) {
    token = envToken
  } else {
    try {
      const file = Runtime.file(controlTokenPath(state.paths.home))
      if (await file.exists()) {
        const text = (await file.text()).trim()
        if (text.length >= 16) token = text
      }
    } catch {
      token = null
    }
  }
  try {
    const health = await fetch(`${baseUrl}/health`, {
      signal: AbortSignal.timeout(750),
    })
    return { reachable: health.ok, baseUrl, hasToken: token !== null, token }
  } catch {
    return { reachable: false, baseUrl, hasToken: token !== null, token }
  }
}

async function readControlCommand(probe: RunningNodeProbe, command: ControlCommand): Promise<unknown | null> {
  if (!probe.reachable || !probe.token) return null
  try {
    const response = await fetch(`${probe.baseUrl}/command`, {
      method: 'POST',
      headers: {
        authorization: `Bearer ${probe.token}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify(command),
    })
    const json = (await response.json()) as { ok?: boolean; result?: unknown }
    if (!response.ok || json.ok !== true) return null
    return json.result ?? null
  } catch {
    return null
  }
}

function parseDevLoopOptions(args: string[]) {
  const options: { allowDirty: boolean; command?: string; json: boolean } = {
    allowDirty: false,
    json: false,
  }
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === '--json') {
      options.json = true
      continue
    }
    if (arg === '--allow-dirty') {
      options.allowDirty = true
      continue
    }
    if (arg === '--command') {
      const value = args[index + 1]
      if (!value || value.startsWith('--')) throw new Error('--command requires a value')
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
  let current = ''
  let quote: "'" | '"' | null = null
  let escaped = false
  for (const char of value) {
    if (escaped) {
      current += char
      escaped = false
      continue
    }
    if (char === '\\') {
      escaped = true
      continue
    }
    if (quote) {
      if (char === quote) quote = null
      else current += char
      continue
    }
    if (char === "'" || char === '"') {
      quote = char
      continue
    }
    if (/\s/.test(char)) {
      if (current.length > 0) {
        argv.push(current)
        current = ''
      }
      continue
    }
    current += char
  }
  if (escaped) current += '\\'
  if (quote) throw new Error('unterminated quote in --command')
  if (current.length > 0) argv.push(current)
  if (argv.length === 0) throw new Error('--command cannot be empty')
  return argv
}

function devCommandSpecFromOption(command: string | undefined, cwd: string): PylonDevCommandSpec[] | undefined {
  if (!command) return undefined
  return [
    {
      argv: splitDevCommand(command),
      cwd,
      reasonRef: 'check.dev.custom_command',
    },
  ]
}

// Helper: parse `--key value` and `--flag` options out of an arg list. Returns
// a record where flags map to `true` and options to their string value.
function parseCliOptions(args: string[]): Record<string, string | true> {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith('--')) continue
    const key = arg.slice(2)
    const value = args[index + 1]
    if (value === undefined || value.startsWith('--')) {
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
  return typeof value === 'string' ? value : undefined
}

function optionFlag(options: Record<string, string | true>, key: string): boolean {
  const value = options[key]
  return value === true || value === 'true'
}

function lifecycleNdjsonRequested(options: Record<string, string | true>): boolean {
  return optionFlag(options, 'lifecycle-ndjson') || optionFlag(options, 'json')
}

function positiveIntegerOption(options: Record<string, string | true>, key: string, label: string): number | undefined {
  const raw = optionString(options, key)
  if (raw === undefined) return undefined
  if (!/^[1-9][0-9]*$/.test(raw.trim())) throw new Error(`${label} must be a positive integer`)
  const parsed = Number.parseInt(raw, 10)
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(`${label} must be a positive integer`)
  return parsed
}

const numberFromRecord = (record: Record<string, unknown>, key: string): number => {
  const value = record[key]
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
}

const arrayFromRecord = (record: Record<string, unknown>, key: string): ReadonlyArray<unknown> => {
  const value = record[key]
  return Array.isArray(value) ? value : []
}

async function collectKhalaApmProjection(input: { agentToken: string; baseUrl: string; state: PylonLocalState }) {
  const observedAt = new Date().toISOString()
  const response = await fetch(new URL('/api/operator/fleet/state', input.baseUrl), {
    headers: {
      authorization: `Bearer ${input.agentToken}`,
      accept: 'application/json',
    },
  })
  if (!response.ok) {
    throw new Error(`khala apm query failed (${response.status})`)
  }
  const snapshot = (await response.json()) as Record<string, unknown>
  const pace =
    snapshot.pace !== null && typeof snapshot.pace === 'object' ? (snapshot.pace as Record<string, unknown>) : {}
  const fleet =
    snapshot.fleet !== null && typeof snapshot.fleet === 'object' ? (snapshot.fleet as Record<string, unknown>) : {}
  const activeSessionTokenEstimate =
    pace.activeSessionTokenEstimate !== null && typeof pace.activeSessionTokenEstimate === 'object'
      ? (pace.activeSessionTokenEstimate as Record<string, unknown>)
      : {}
  const derivedServerAssignments = arrayFromRecord(fleet, 'activeAssignments')
    .filter(
      (assignment): assignment is Record<string, unknown> => assignment !== null && typeof assignment === 'object',
    )
    .map((assignment) => {
      const tokens = numberFromRecord(assignment, 'tokensSoFar')
      const elapsedMs = numberFromRecord(assignment, 'elapsedMs')
      const elapsedMinutes = Math.max(elapsedMs / 60_000, 1 / 6)
      return {
        assignmentRef: typeof assignment.assignmentRef === 'string' ? assignment.assignmentRef : null,
        elapsedMs,
        source: tokens > 0 ? 'fleet.activeAssignments.tokensSoFar' : 'unavailable',
        tokenCountKind: typeof assignment.tokenCountKind === 'string' ? assignment.tokenCountKind : null,
        tokens,
        tokensPerMinute: Math.round(tokens / elapsedMinutes),
      }
    })
  const projectedServerAssignments = arrayFromRecord(activeSessionTokenEstimate, 'assignments')
  const serverAssignments =
    projectedServerAssignments.length > 0 ? projectedServerAssignments : derivedServerAssignments
  const localRuns = await activeCodingRuns(input.state.paths)
  const localCodexRuns = localRuns.filter((run) => run.service === 'codex')
  const completedTokensPerMinute = numberFromRecord(pace, 'liveBurnRateTokensPerMinute')
  const ownCapacityCodex =
    pace.ownCapacityCodex !== null && typeof pace.ownCapacityCodex === 'object'
      ? (pace.ownCapacityCodex as Record<string, unknown>)
      : {}
  const completedTokensWindow = numberFromRecord(ownCapacityCodex, 'tokensWindow')
  const derivedInFlightTokens = derivedServerAssignments.reduce((sum, assignment) => sum + assignment.tokens, 0)
  const derivedInFlightTokensPerMinute = derivedServerAssignments.reduce(
    (sum, assignment) => sum + assignment.tokensPerMinute,
    0,
  )
  const inFlightTokens = numberFromRecord(activeSessionTokenEstimate, 'inFlightTokens') || derivedInFlightTokens
  const inFlightTokensPerMinute =
    numberFromRecord(activeSessionTokenEstimate, 'inFlightTokensPerMinute') || derivedInFlightTokensPerMinute
  const activeAdjustedTokensPerMinute =
    numberFromRecord(pace, 'activeAdjustedTokensPerMinute') || completedTokensPerMinute + inFlightTokensPerMinute
  const todayTokens = numberFromRecord(pace, 'todayTokens')
  const timezone = typeof pace.timezone === 'string' && pace.timezone.trim() !== '' ? pace.timezone : 'America/Chicago'
  const text = [
    `Khala APM: ${activeAdjustedTokensPerMinute.toLocaleString()} active-adjusted tokens/min`,
    `completed-window: ${completedTokensPerMinute.toLocaleString()} tokens/min`,
    `in-flight: ${inFlightTokens.toLocaleString()} tokens across ${serverAssignments.length} server assignment(s), ${localCodexRuns.length} fresh local Codex run(s)`,
    `today (${timezone}): ${todayTokens.toLocaleString()} counted tokens`,
  ].join('\n')

  return {
    schema: 'openagents.pylon.khala_apm.v0.1',
    observedAt,
    baseUrl: input.baseUrl,
    pylonRef: input.state.identity.pylonRef,
    counted: {
      timezone,
      todayTokens,
      completedTokensPerMinute,
      tokensWindow: completedTokensWindow,
      sourceRefs: ['d1:token_usage_events'],
    },
    active: {
      adjustedTokensPerMinute: activeAdjustedTokensPerMinute,
      inFlightTokens,
      inFlightTokensPerMinute,
      localCodexRunCount: localCodexRuns.length,
      serverAssignmentCount: serverAssignments.length,
      serverAssignments,
      localRuns: localCodexRuns.map((run) => ({
        assignmentRef: run.assignmentRef,
        accountRefHash: run.accountRefHash ?? null,
        leaseRef: run.leaseRef,
        refreshedAt: run.refreshedAt,
        runRef: run.runRef,
        startedAt: run.startedAt,
      })),
      caveatRefs: arrayFromRecord(activeSessionTokenEstimate, 'caveatRefs'),
      method:
        typeof activeSessionTokenEstimate.method === 'string'
          ? activeSessionTokenEstimate.method
          : 'completed token window plus active assignment token projection',
      sourceRefs: arrayFromRecord(activeSessionTokenEstimate, 'sourceRefs'),
    },
    rawSnapshot: snapshot,
    text,
  }
}

type AssignmentLeaseNetworkOptions = {
  agentToken?: string
  baseUrl: string
}

async function serverActiveCodingRunCounts(
  summary: BootstrapSummary,
  options: AssignmentLeaseNetworkOptions | undefined,
): Promise<PylonActiveCodingRunCounts> {
  if (options === undefined) return {}
  try {
    return activeCodingRunCountsFromAssignmentLeases(await pollAssignments(summary, options))
  } catch {
    return {}
  }
}

async function serverActiveCodingRunAccountCounts(
  summary: BootstrapSummary,
  options: AssignmentLeaseNetworkOptions | undefined,
): Promise<PylonActiveCodingRunAccountCounts> {
  if (options === undefined) return {}
  try {
    return activeCodingRunCountsByAccountFromAssignmentLeases(await pollAssignments(summary, options))
  } catch {
    return {}
  }
}

async function activeDispatchBreakersForPlanning(summary: BootstrapSummary): Promise<PylonDispatchBreakerSnapshot[]> {
  const dbPath = `${summary.paths.home}/orchestration.sqlite`
  if (!existsSync(dbPath)) return []
  let db: Database | null = null
  try {
    db = openLegacySqliteDatabase(dbPath)
    db.exec('PRAGMA busy_timeout = 250')
    const store = createPylonOrchestrationStore(db)
    return store.listActiveDispatchBreakers(new Date())
  } catch {
    return []
  } finally {
    db?.close()
  }
}

async function codingCapacityForDispatch(
  summary: BootstrapSummary,
  state: PylonLocalState,
  options?: AssignmentLeaseNetworkOptions,
  env: NodeJS.ProcessEnv = Runtime.env,
) {
  const activeCounts = maxActiveCodingRunCounts(
    await activeCodingRunCounts(state.paths),
    await serverActiveCodingRunCounts(summary, options),
  )
  const codingCapacity = codingServiceCapacityFromRuntime(
    state,
    env,
    await localCodingServiceReadyCounts(summary, env),
    activeCounts,
  )
  return codingCapacity
}

async function codingAccountBusyCountsForDispatch(
  summary: BootstrapSummary,
  state: PylonLocalState,
  options?: AssignmentLeaseNetworkOptions,
): Promise<PylonActiveCodingRunAccountCounts> {
  return maxActiveCodingRunAccountCounts(
    await activeCodingRunCountsByAccount(state.paths),
    await serverActiveCodingRunAccountCounts(summary, options),
  )
}

async function availableCodexAssignments(
  summary: BootstrapSummary,
  state: PylonLocalState,
  options?: AssignmentLeaseNetworkOptions,
  env: NodeJS.ProcessEnv = Runtime.env,
): Promise<number> {
  const codexAccounts = await localCodexDispatchAccounts(summary, state, env, options)
  if (codexAccounts.length > 0) {
    return codexAccounts.reduce((sum, account) => sum + account.available, 0)
  }
  const codingCapacity = await codingCapacityForDispatch(summary, state, options, env)
  return codingCapacity.find((item) => item.service === 'codex')?.available ?? 0
}

async function availableClaudeAssignments(
  summary: BootstrapSummary,
  state: PylonLocalState,
  options?: AssignmentLeaseNetworkOptions,
  env: NodeJS.ProcessEnv = Runtime.env,
): Promise<number> {
  const claudeAccounts = await localClaudeDispatchAccounts(summary, state, env, options)
  if (claudeAccounts.length > 0) {
    return claudeAccounts.reduce((sum, account) => sum + account.available, 0)
  }
  const codingCapacity = await codingCapacityForDispatch(summary, state, options, env)
  return codingCapacity.find((item) => item.service === 'claude')?.available ?? 0
}

async function localCodexDispatchAccounts(
  summary: BootstrapSummary,
  state: PylonLocalState,
  env: NodeJS.ProcessEnv = Runtime.env,
  options?: AssignmentLeaseNetworkOptions,
) {
  return localCodexAccountCapacities(
    state,
    summary,
    env,
    codexBusyByAccount(await codingAccountBusyCountsForDispatch(summary, state, options)),
  )
}

async function localClaudeDispatchAccounts(
  summary: BootstrapSummary,
  state: PylonLocalState,
  env: NodeJS.ProcessEnv = Runtime.env,
  options?: AssignmentLeaseNetworkOptions,
) {
  return localClaudeAccountCapacities(
    state,
    summary,
    env,
    claudeBusyByAccount(await codingAccountBusyCountsForDispatch(summary, state, options)),
  )
}

function khalaCodexCapacityAdvertisementEnv(env: NodeJS.ProcessEnv, requestedSlots: number): NodeJS.ProcessEnv {
  const requestedFloor = Math.max(
    DEFAULT_CODEX_PER_ACCOUNT_CONCURRENCY,
    Number.isSafeInteger(requestedSlots) && requestedSlots > 0 ? Math.floor(requestedSlots) : 1,
  )
  const inheritedPooledConcurrency =
    positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_CONCURRENCY) ?? DEFAULT_CODEX_PER_ACCOUNT_CONCURRENCY
  const explicitPerAccountConcurrency = positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY)
  const perAccountTarget = explicitPerAccountConcurrency ?? Math.max(inheritedPooledConcurrency, requestedFloor)
  const pooledTarget = Math.max(inheritedPooledConcurrency, requestedFloor)
  return {
    ...env,
    OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY: String(perAccountTarget),
    OPENAGENTS_PYLON_CODEX_BUSY: '0',
    OPENAGENTS_PYLON_CODEX_CONCURRENCY: String(pooledTarget),
    OPENAGENTS_PYLON_CODEX_QUEUED: '0',
  }
}

function khalaClaudeCapacityAdvertisementEnv(env: NodeJS.ProcessEnv, requestedSlots: number): NodeJS.ProcessEnv {
  const requestedFloor = Math.max(
    1,
    Number.isSafeInteger(requestedSlots) && requestedSlots > 0 ? Math.floor(requestedSlots) : 1,
  )
  const inheritedPooledConcurrency = positiveIntegerEnv(env.OPENAGENTS_PYLON_CLAUDE_CONCURRENCY) ?? 1
  const explicitPerAccountConcurrency = positiveIntegerEnv(env.OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY)
  const perAccountTarget = explicitPerAccountConcurrency ?? Math.max(inheritedPooledConcurrency, requestedFloor)
  const pooledTarget = Math.max(inheritedPooledConcurrency, requestedFloor)
  return {
    ...env,
    OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY: String(perAccountTarget),
    OPENAGENTS_PYLON_CLAUDE_BUSY: '0',
    OPENAGENTS_PYLON_CLAUDE_CONCURRENCY: String(pooledTarget),
    OPENAGENTS_PYLON_CLAUDE_QUEUED: '0',
  }
}

function positiveIntegerEnv(value: string | undefined): number | null {
  const parsed = Number.parseInt(value?.trim() ?? '', 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null
}

async function localGitText(args: string[], cwd = process.cwd()): Promise<string> {
  const proc = Runtime.spawn(['git', ...args], {
    cwd,
    stderr: 'pipe',
    stdout: 'pipe',
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new Error(`git ${args[0] ?? 'command'} failed: ${stderr.trim()}`)
  return stdout.trim()
}

const gitHubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

function gitHubFullNameFromRemote(remote: string): string | null {
  const trimmed = remote.trim()
  const normalize = (path: string): string | null => {
    const fullName = path.replace(/^\/+/, '').replace(/\.git$/, '')
    return gitHubFullNamePattern.test(fullName) ? fullName : null
  }
  try {
    const url = new URL(trimmed)
    if (url.hostname !== 'github.com') return null
    return normalize(url.pathname)
  } catch {
    const ssh = /^git@github\.com:([^#?]+)$/.exec(trimmed)
    if (ssh !== null) return normalize(ssh[1] ?? '')
    return null
  }
}

async function managedWorktreeRepoRef(
  options: Record<string, string | true>,
): Promise<NonNullable<ControlSessionSpawnCommand['repoRef']>> {
  const explicitRepo = optionString(options, 'repo') ?? optionString(options, 'repo-ref')
  const fullName = explicitRepo ?? gitHubFullNameFromRemote(await localGitText(['remote', 'get-url', 'origin']))
  if (typeof fullName !== 'string' || !gitHubFullNamePattern.test(fullName)) {
    throw new Error('managed worktree requires a GitHub origin or --repo owner/name')
  }
  const baseRef = optionString(options, 'base-ref') ?? 'origin/main'
  if (!/^[A-Za-z0-9_./-]+$/.test(baseRef) || baseRef.includes('..') || baseRef.startsWith('-')) {
    throw new Error('managed worktree --base-ref is invalid')
  }
  const commitSha = await localGitText(['rev-parse', `${baseRef}^{commit}`])
  if (!/^[a-f0-9]{40}$/i.test(commitSha)) {
    throw new Error('managed worktree base ref did not resolve to a commit')
  }
  return {
    provider: 'github',
    visibility: 'public',
    fullName,
    branch: baseRef.replace(/^origin\//, ''),
    commitSha,
  }
}

function sessionLaneOption(
  options: Record<string, string | true>,
  label: string,
): ControlSessionSpawnCommand['lane'] | undefined {
  const lane = optionString(options, 'lane')
  if (lane === undefined) return undefined
  if (lane === 'auto' || lane === 'local' || lane === 'cloud-gcp') {
    return lane
  }
  throw new Error(`${label} --lane must be auto, local, or cloud-gcp`)
}

const isTerminalSessionState = (state: unknown): boolean =>
  state === 'completed' || state === 'failed' || state === 'cancelled'

async function waitForControlSessionTerminal(sessionRef: string, timeoutSeconds: number | undefined) {
  const startedAt = Date.now()
  const deadlineMs = (timeoutSeconds ?? 600) * 1000 + 30_000
  let polls = 0
  let session: ControlSessionProjection | null = null
  for (;;) {
    polls += 1
    const { result } = await runControlCommand({ type: 'session.list' }, Runtime.env)
    const sessions = Array.isArray(result) ? (result as ControlSessionProjection[]) : []
    session = sessions.find((entry) => entry.sessionRef === sessionRef) ?? null
    if (session !== null && isTerminalSessionState(session.state)) break
    if (Date.now() - startedAt >= deadlineMs) {
      return {
        session,
        events: null,
        artifact: null,
        driver: { elapsedMs: Date.now() - startedAt, polls, timedOut: true },
      }
    }
    await Runtime.sleep(250)
  }

  let events: unknown = null
  try {
    events = (await runControlCommand({ type: 'session.events', sessionRef }, Runtime.env)).result
  } catch {
    events = null
  }
  let artifact: unknown = null
  try {
    artifact = (await runControlCommand({ type: 'session.artifact', sessionRef }, Runtime.env)).result
  } catch {
    artifact = null
  }
  return {
    session,
    events,
    artifact,
    driver: { elapsedMs: Date.now() - startedAt, polls, timedOut: false },
  }
}

// Emit a clean JSON error + nonzero exit on the steering control surfaces, so
// an agent always gets parseable output even when no node is running.
function emitControlError(command: string, error: unknown): void {
  const code = error instanceof ControlEndpointError ? error.code : 'error'
  const message = error instanceof Error ? error.message : String(error)
  process.stdout.write(`${JSON.stringify({ ok: false, command, code, error: message }, null, 2)}\n`)
  process.exitCode = 1
}

function describeCheck(result: Awaited<ReturnType<typeof checkForUpdate>>): string {
  switch (result.status) {
    case 'up-to-date':
      return `Pylon ${result.currentVersion} is up to date.`
    case 'update-available':
      return `Update available: ${result.currentVersion} -> ${result.release.version}.`
    case 'unsupported':
      return `Auto-update unsupported on ${result.reason}.`
    case 'disabled':
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
    const disabled = autoUpdateDisabledReason(Runtime.env)
    if (disabled !== null) return
    const target = resolveSelfBinaryPath()
    if (target === null) return // dev / interpreter run — nothing to replace
    const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
    const state = await ensurePylonLocalState(summary)
    const result = await checkForUpdate({
      clientId: state.identity.nodeId,
      env: Runtime.env,
    })
    if (result.status !== 'update-available') return
    process.stderr.write(`Pylon: auto-updating ${result.currentVersion} -> ${result.release.version}...\n`)
    const applied = await downloadAndApply({
      release: result.release,
      targetPath: target,
    })
    process.stderr.write(`Pylon: updated to ${applied.version}; relaunching.\n`)
    // Re-exec the freshly written binary with the same args, inheriting stdio so
    // a launchd/systemd/terminal supervisor stays attached, then exit with its code.
    const child = Runtime.spawn([target, ...Runtime.argv.slice(2)], {
      stdin: 'inherit',
      stdout: 'inherit',
      stderr: 'inherit',
      env: Runtime.env,
    })
    const exitCode = await child.exited
    process.exit(exitCode)
  } catch (error) {
    process.stderr.write(`Pylon: auto-update skipped (${error instanceof Error ? error.message : String(error)}).\n`)
  }
}

// Format a node-startup failure into a clear, actionable message. The most
// common operational failure is the control port already being held by a
// running Pylon daemon (Runtime.serve throws an EADDRINUSE-class error whose
// message mentions the port). Surface that as guidance instead of a raw crash
// dump, and keep the real release version in the banner.
function formatNodeStartupError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error)
  const port = Number(Runtime.env.PYLON_CONTROL_PORT ?? defaultControlPort)
  const looksLikePortInUse =
    /EADDRINUSE/i.test(message) ||
    /port .* in use/i.test(message) ||
    /address (already )?in use/i.test(message) ||
    /failed to start server/i.test(message)
  if (looksLikePortInUse) {
    return [
      `Pylon ${PYLON_VERSION} could not start: control port ${port} is already in use.`,
      `Another Pylon node (or process) is already bound to 127.0.0.1:${port}.`,
      `Run a second instance on a different port and home, e.g.:`,
      `  PYLON_CONTROL_PORT=4726 PYLON_HOME=/tmp/pylon-alt pylon`,
      `Or stop the existing node before starting a new one.`,
    ].join('\n')
  }
  return `Pylon ${PYLON_VERSION} crashed on startup: ${message}`
}

async function main() {
  const args = Runtime.argv.slice(2)

  // `pylon --version` / `pylon -V`: print the authoritative release version
  // and exit BEFORE any runtime or control-server boot. Without this guard,
  // `--version` falls through to the no-subcommand default path below and
  // boots the headless node, which crashes when the control port is already
  // held by a running daemon.
  if (args[0] === '--version' || args[0] === '-V') {
    process.stdout.write(`${PYLON_VERSION}\n`)
    return
  }

  // `pylon help [--json]`, `pylon --help`, and `pylon -h` print the
  // machine-readable command catalog. A bare `--help`/`-h` (no subcommand)
  // must short-circuit BEFORE the no-subcommand default node boot below;
  // otherwise it falls through and starts the headless node.
  if (args[0] === 'help' || args[0] === '--help' || args[0] === '-h') {
    process.stdout.write(`${JSON.stringify(projectCommandCatalog(), null, 2)}\n`)
    return
  }
  if (args.includes('--help') && args[0] !== undefined && !args[0].startsWith('--')) {
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

  if (
    args[0] === 'activity' ||
    args[0] === 'timeline' ||
    args[0] === 'replay' ||
    args[0] === 'receipts' ||
    args[0] === 'evidence-pack'
  ) {
    const command = args[0] as PublicActivityCliCommand
    try {
      const result = await runPublicActivityCliCommand(command, args.slice(1), {
        env: Runtime.env,
      })
      process.stdout.write(result.json ? `${JSON.stringify(result, null, 2)}\n` : formatPublicActivityCliText(result))
      if (!result.ok) process.exitCode = 1
      return
    } catch (error) {
      process.stdout.write(
        `${JSON.stringify({ ok: false, command, error: error instanceof Error ? error.message : String(error) }, null, 2)}\n`,
      )
      process.exitCode = 1
      return
    }
  }

  // CL-5035 sessions: first-class headless wrappers over the control-server
  // session verbs (session.list/spawn/reply/batch/cancel) the Autopilot desktop drives.
  if (args[0] === 'sessions') {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === 'list') {
        const { result } = await runControlCommand({ type: 'session.list' }, Runtime.env)
        process.stdout.write(`${JSON.stringify({ ok: true, sessions: result }, null, 2)}\n`)
        return
      }
      if (command === 'spawn') {
        const adapter = optionString(options, 'adapter')
        const objective = optionString(options, 'objective')
        if (adapter !== 'codex' && adapter !== 'claude_agent') {
          throw new Error('sessions spawn --adapter must be codex or claude_agent')
        }
        if (!objective || objective.trim().length === 0) {
          throw new Error('sessions spawn requires --objective')
        }
        const verify = (() => {
          const raw = options.verify
          if (typeof raw !== 'string' || raw.trim().length === 0) return []
          // The control protocol's verify is an argv array. #5389: a
          // `--verify "test -f x"` string must be run SHELL-PARSED in the
          // session's worktree CWD. Naive whitespace-splitting mangled quoted
          // args and, in the single-element edge case, made the node try to
          // exec a program literally named "test -f x" — reporting a false
          // `verification_failed` even when the work succeeded. `sh -c` gives
          // correct shell semantics and the dev-check runs it in the worktree.
          return ['sh', '-c', raw.trim()]
        })()
        const worktree = optionString(options, 'worktree')
        const managedWorktree = optionFlag(options, 'managed-worktree')
        if (worktree && managedWorktree) {
          throw new Error('sessions spawn uses either --worktree or --managed-worktree, not both')
        }
        const repoRef = managedWorktree ? await managedWorktreeRepoRef(options) : undefined
        const lane = sessionLaneOption(options, 'sessions spawn')
        const { result } = await runControlCommand(
          {
            type: 'session.spawn',
            adapter,
            ...(lane === undefined ? {} : { lane }),
            objective,
            verify,
            ...(repoRef ? { repoRef } : worktree ? { worktreePath: worktree } : {}),
          },
          Runtime.env,
        )
        process.stdout.write(`${JSON.stringify({ ok: true, session: result }, null, 2)}\n`)
        return
      }
      if (command === 'reply') {
        const sessionRef = optionString(options, 'session-ref')
        const objective = optionString(options, 'objective')
        if (!sessionRef || sessionRef.trim().length === 0) {
          throw new Error('sessions reply requires --session-ref <ref>')
        }
        if (!objective || objective.trim().length === 0) {
          throw new Error('sessions reply requires --objective')
        }
        const timeoutSeconds = positiveIntegerOption(options, 'timeout-seconds', 'sessions reply --timeout-seconds')
        const { result } = await runControlCommand(
          {
            type: 'session.reply',
            sessionRef,
            objective,
            ...(timeoutSeconds === undefined ? {} : { timeoutSeconds }),
          },
          Runtime.env,
        )
        const reply = result as {
          sessionRef: string
          parentSessionRef: string
          state: string
        }
        if (!optionFlag(options, 'wait')) {
          process.stdout.write(`${JSON.stringify({ ok: true, reply }, null, 2)}\n`)
          return
        }
        const waited = await waitForControlSessionTerminal(reply.sessionRef, timeoutSeconds)
        const ok = !waited.driver.timedOut && waited.session?.state === 'completed'
        process.stdout.write(`${JSON.stringify({ ok, reply, ...waited }, null, 2)}\n`)
        if (!ok) process.exitCode = 1
        return
      }
      if (command === 'cancel') {
        const sessionRef =
          optionString(options, 'session-ref') ?? (args[2] && !args[2].startsWith('--') ? args[2] : undefined)
        if (!sessionRef) throw new Error('sessions cancel requires --session-ref <ref>')
        const { result } = await runControlCommand({ type: 'session.cancel', sessionRef }, Runtime.env)
        process.stdout.write(`${JSON.stringify({ ok: true, session: result }, null, 2)}\n`)
        return
      }
      if (command === 'batch') {
        const adapter = optionString(options, 'adapter')
        if (adapter !== 'codex' && adapter !== 'claude_agent') {
          throw new Error('sessions batch --adapter must be codex or claude_agent')
        }
        const tasksPath = optionString(options, 'tasks')
        if (!tasksPath || tasksPath.trim().length === 0) {
          throw new Error('sessions batch requires --tasks <json-file>')
        }
        const tasks = parseSessionsBatchTasks(JSON.parse(await readFile(tasksPath, 'utf8')))
        const verifyArgs = args.slice(2)
        const verifies: string[] = []
        for (let i = 0; i < verifyArgs.length; i += 1) {
          if (verifyArgs[i] === '--verify') {
            const value = verifyArgs[i + 1]
            if (typeof value === 'string' && !value.startsWith('--') && value.trim().length > 0) {
              verifies.push(value.trim())
              i += 1
            }
          }
        }
        const verify = verifies.length === 0 ? ['node', '--version'] : ['sh', '-c', verifies.join(' && ')]
        const concurrency = positiveIntegerOption(options, 'concurrency', 'sessions batch --concurrency') ?? 2
        const timeoutSeconds = positiveIntegerOption(options, 'timeout-seconds', 'sessions batch --timeout-seconds')
        const worktree = optionString(options, 'worktree')
        const managedWorktree = optionFlag(options, 'managed-worktree')
        if (worktree && managedWorktree) {
          throw new Error('sessions batch uses either --worktree or --managed-worktree, not both')
        }
        const repoRef = managedWorktree ? await managedWorktreeRepoRef(options) : undefined
        const lane = sessionLaneOption(options, 'sessions batch')
        const control: SessionsExecControl = {
          spawn: async (cmd) => {
            const { result } = await runControlCommand(cmd, Runtime.env)
            return result as { sessionRef: string; state: any }
          },
          list: async () => {
            const { result } = await runControlCommand({ type: 'session.list' }, Runtime.env)
            return result as any
          },
          events: async (sessionRef) => {
            const { result } = await runControlCommand({ type: 'session.events', sessionRef }, Runtime.env)
            return result as any
          },
          artifact: async (sessionRef) => {
            const { result } = await runControlCommand({ type: 'session.artifact', sessionRef }, Runtime.env)
            return result as any
          },
          approvalsList: async () => {
            const { result } = await runControlCommand({ type: 'approvals.list' }, Runtime.env)
            return result as {
              approvals: Array<{ approvalRef: string; kind: string }>
            }
          },
        }
        const result = await runSessionsBatch(control, {
          adapter,
          ...(lane === undefined ? {} : { lane }),
          tasks,
          verify,
          concurrency,
          ...(repoRef ? { repoRef } : worktree ? { worktreePath: worktree } : {}),
          ...(timeoutSeconds === undefined ? {} : { timeoutSeconds }),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        if (!result.ok) process.exitCode = 1
        return
      }
      // W-1 (#5377): blocking run-to-completion one-shot. Spawn a coding session
      // and drive its turn loop to a terminal state over the SAME control verbs,
      // returning a structured JSON result. Exit 0 on success-terminal, nonzero
      // on failure/timeout/approval-required. Thin wrapper — no new wire verb.
      if (command === 'exec') {
        const adapter = optionString(options, 'adapter')
        const objective = optionString(options, 'objective')
        if (adapter !== 'codex' && adapter !== 'claude_agent') {
          throw new Error('sessions exec --adapter must be codex or claude_agent')
        }
        if (!objective || objective.trim().length === 0) {
          throw new Error('sessions exec requires --objective')
        }
        // `--verify` is REPEATABLE (each is a whole command). parseCliOptions
        // collapses repeats, so collect every occurrence (as the RAW string)
        // from the argv. #5389: a `--verify "test -f x"` string must be run
        // SHELL-PARSED in the session's worktree CWD. The old behavior
        // whitespace-split the string into an argv (`["test","-f","x"]`); that
        // mangles any quoted/multi-word arg and, in the single-element edge
        // case, made the node try to exec a program literally named
        // "test -f x" — so a TRUE condition reported `verification_failed`
        // (a false negative) even when the work succeeded. Wrapping in `sh -c`
        // gives correct shell semantics (quoting, globs, operators) and the
        // dev-check already runs it in `input.cwd` (the worktree).
        const verifyArgs = args.slice(2)
        const verifies: string[] = []
        for (let i = 0; i < verifyArgs.length; i += 1) {
          if (verifyArgs[i] === '--verify') {
            const value = verifyArgs[i + 1]
            if (typeof value === 'string' && !value.startsWith('--') && value.trim().length > 0) {
              verifies.push(value.trim())
              i += 1
            }
          }
        }
        // The control session takes ONE verify argv. Run the verify command(s)
        // shell-parsed via `sh -c`. With multiple `--verify` commands, chain
        // them with `&&` so all must pass. Default to `node --version` when none
        // given so the session still produces a verify outcome.
        const verify = verifies.length === 0 ? ['node', '--version'] : ['sh', '-c', verifies.join(' && ')]
        const worktree = optionString(options, 'worktree')
        // W-3 (#5379): `--on-approval` gains `auto`, the BOUNDED auto-approve
        // policy. `--approval-policy <name>` is an explicit alias for the same
        // selection. Default stays `manual` (pause + report) — unchanged.
        const onApprovalRaw = optionString(options, 'on-approval') ?? optionString(options, 'approval-policy')
        if (
          onApprovalRaw !== undefined &&
          onApprovalRaw !== 'manual' &&
          onApprovalRaw !== 'deny' &&
          onApprovalRaw !== 'auto'
        ) {
          throw new Error('sessions exec --on-approval must be manual, deny, or auto')
        }
        const onApproval: ApprovalPolicy =
          onApprovalRaw === 'deny' ? 'deny' : onApprovalRaw === 'auto' ? 'auto' : 'manual'
        // W-3 caps for the bounded auto policy. These are only consulted when
        // onApproval === "auto"; they bound how many approvals the policy may
        // auto-approve and for how long, after which it escalates.
        const maxAutoApprovalsRaw = optionString(options, 'max-auto-approvals')
        const maxAutoApprovals =
          maxAutoApprovalsRaw === undefined ? undefined : Number.parseInt(maxAutoApprovalsRaw, 10)
        if (maxAutoApprovals !== undefined && (!Number.isFinite(maxAutoApprovals) || maxAutoApprovals <= 0)) {
          throw new Error('sessions exec --max-auto-approvals must be a positive integer')
        }
        const autoWindowSecondsRaw = optionString(options, 'auto-window-seconds')
        const autoWindowSeconds =
          autoWindowSecondsRaw === undefined ? undefined : Number.parseInt(autoWindowSecondsRaw, 10)
        if (autoWindowSeconds !== undefined && (!Number.isFinite(autoWindowSeconds) || autoWindowSeconds <= 0)) {
          throw new Error('sessions exec --auto-window-seconds must be a positive integer')
        }
        const outOfBoundsRaw = optionString(options, 'auto-out-of-bounds')
        if (outOfBoundsRaw !== undefined && outOfBoundsRaw !== 'escalate' && outOfBoundsRaw !== 'deny') {
          throw new Error('sessions exec --auto-out-of-bounds must be escalate or deny')
        }
        const outOfBounds: 'escalate' | 'deny' | undefined = outOfBoundsRaw as 'escalate' | 'deny' | undefined
        const timeoutSecondsRaw = optionString(options, 'timeout-seconds')
        const timeoutSeconds = timeoutSecondsRaw === undefined ? undefined : Number.parseInt(timeoutSecondsRaw, 10)
        if (timeoutSeconds !== undefined && (!Number.isFinite(timeoutSeconds) || timeoutSeconds <= 0)) {
          throw new Error('sessions exec --timeout-seconds must be a positive integer')
        }
        const managedWorktree = optionFlag(options, 'managed-worktree')
        if (worktree && managedWorktree) {
          throw new Error('sessions exec uses either --worktree or --managed-worktree, not both')
        }
        const repoRef = managedWorktree ? await managedWorktreeRepoRef(options) : undefined
        const lane = sessionLaneOption(options, 'sessions exec')
        // Thin control adapter: every verb forwards to the running node. No new
        // authority — this only spawns + observes the existing session surface.
        const control: SessionsExecControl = {
          spawn: async (cmd) => {
            const { result } = await runControlCommand(cmd, Runtime.env)
            return result as { sessionRef: string; state: any }
          },
          list: async () => {
            const { result } = await runControlCommand({ type: 'session.list' }, Runtime.env)
            return result as any
          },
          events: async (sessionRef) => {
            const { result } = await runControlCommand({ type: 'session.events', sessionRef }, Runtime.env)
            return result as any
          },
          artifact: async (sessionRef) => {
            const { result } = await runControlCommand({ type: 'session.artifact', sessionRef }, Runtime.env)
            return result as any
          },
          approvalsList: async () => {
            const { result } = await runControlCommand({ type: 'approvals.list' }, Runtime.env)
            return result as {
              approvals: Array<{ approvalRef: string; kind: string }>
            }
          },
          approvalsResolve: async (approvalRef, decision) => {
            const { result } = await runControlCommand({ type: 'approvals.resolve', approvalRef, decision }, Runtime.env)
            return result
          },
        }
        // W-3: when `auto` is selected, build the BOUNDED auto-approve policy and
        // pass its callback + audit accessor. The policy is scoped to the
        // declared worktree, so out-of-scope paths escalate/deny. Default
        // manual/deny keep the W-1 mapping (no callback, empty autoApprovals[]).
        const auto =
          onApproval === 'auto'
            ? createBoundedAutoApprovalPolicy({
                ...(worktree ? { scopeRoot: worktree } : {}),
                config: {
                  ...(maxAutoApprovals === undefined ? {} : { maxAutoApprovals }),
                  ...(autoWindowSeconds === undefined ? {} : { windowMs: autoWindowSeconds * 1000 }),
                  ...(outOfBounds === undefined ? {} : { outOfBounds }),
                },
              })
            : undefined
        const result = await runSessionsExec(control, {
          adapter,
          ...(lane === undefined ? {} : { lane }),
          objective,
          verify,
          ...(repoRef ? { repoRef } : worktree ? { worktreePath: worktree } : {}),
          ...(timeoutSeconds === undefined ? {} : { timeoutSeconds }),
          onApproval,
          ...(auto ? { approvalPolicy: auto.policy, approvalAudit: auto.audit } : {}),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        if (!result.ok) process.exitCode = 1
        return
      }
      throw new Error('usage: pylon sessions list|spawn|reply|batch|exec|cancel ...')
    } catch (error) {
      emitControlError('sessions', error)
      return
    }
  }

  // CL-5035 approvals: list + resolve the node's operator approval queue.
  if (args[0] === 'approvals') {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === 'list') {
        const { result } = await runControlCommand({ type: 'approvals.list' }, Runtime.env)
        process.stdout.write(`${JSON.stringify({ ok: true, ...(result as Record<string, unknown>) }, null, 2)}\n`)
        return
      }
      if (command === 'approve' || command === 'deny' || command === 'answer') {
        const approvalRef =
          optionString(options, 'approval-ref') ?? (args[2] && !args[2].startsWith('--') ? args[2] : undefined)
        if (!approvalRef) throw new Error(`approvals ${command} requires --approval-ref <ref>`)
        const answer = optionString(options, 'answer')
        if (command === 'answer' && !answer) throw new Error('approvals answer requires --answer <text>')
        const { result } = await runControlCommand(
          {
            type: 'approvals.resolve',
            approvalRef,
            decision: command,
            ...(answer ? { answer } : {}),
          },
          Runtime.env,
        )
        process.stdout.write(`${JSON.stringify({ ok: true, resolution: result }, null, 2)}\n`)
        return
      }
      throw new Error('usage: pylon approvals list|approve|deny [--approval-ref <ref>]')
    } catch (error) {
      emitControlError('approvals', error)
      return
    }
  }

  // #6695 VMQ supervisor API: expose the node-local PR fast-forward planner so
  // supervisors can verify the next actual promotion before any git push exists.
  // The planner is pure, so the CLI can run it directly without requiring a
  // live node control server.
  if (args[0] === 'vmq') {
    const command = args[1]
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === 'pr-fast-forward-plan') {
        const projectionPath = optionString(options, 'projection')
        const requestPath = optionString(options, 'request')
        if (!projectionPath || !requestPath) {
          throw new Error('vmq pr-fast-forward-plan requires --projection <json> and --request <json>')
        }
        const projection = JSON.parse(await readFile(projectionPath, 'utf8')) as VirtualMergeQueueProjection
        const request = JSON.parse(await readFile(requestPath, 'utf8')) as VirtualMergeQueuePrFastForwardRequest
        const result = planVirtualMergeQueuePrFastForward({
          projection,
          request,
        })
        process.stdout.write(`${JSON.stringify({ ok: true, result }, null, 2)}\n`)
        return
      }
      throw new Error('usage: pylon vmq pr-fast-forward-plan --projection projection.json --request request.json')
    } catch (error) {
      emitControlError('vmq', error)
      return
    }
  }

  // CL-5035 deploy: surface the gated node deploy-cloud action as a CLI verb.
  // Execution stays gated on the node behind OA_DEPLOY_ENABLE=1 (fail-safe);
  // this verb only forwards the request to the running node.
  if (args[0] === 'deploy') {
    const command = args[1] ?? 'status'
    const options = parseCliOptions(args.slice(2))
    try {
      if (command === 'status') {
        const { result } = await runControlCommand({ type: 'deploy.status' }, Runtime.env)
        process.stdout.write(`${JSON.stringify({ ok: true, deploy: result }, null, 2)}\n`)
        return
      }
      if (command === 'cloud') {
        const target = optionString(options, 'target')
        const ref = optionString(options, 'ref')
        if (!target || !ref) throw new Error('deploy cloud requires --target and --ref')
        const env = optionString(options, 'env')
        const { result } = await runControlCommand(
          { type: 'deploy.cloud', target, ref, ...(env ? { env } : {}) },
          Runtime.env,
        )
        const accepted = (result as { accepted?: boolean } | null)?.accepted === true
        process.stdout.write(`${JSON.stringify({ ok: accepted, deploy: result }, null, 2)}\n`)
        if (!accepted) process.exitCode = 1
        return
      }
      throw new Error('usage: pylon deploy cloud --target T --ref R [--env E] | pylon deploy status')
    } catch (error) {
      emitControlError('deploy', error)
      return
    }
  }

  if (args[0] === 'bootstrap') {
    try {
      const options = parseBootstrapArgs(args.slice(1))
      const summary = createBootstrapSummary(options, Runtime.env)
      if (!summary.platform.inScope) {
        // WSL reports `platform === "linux"`, so it would pass the raw `supported`
        // check; gate on `inScope` and guide a WSL contributor to a native host
        // per the documented macOS/Linux-only scope-out.
        const detail = summary.platform.wsl
          ? `Pylon ${summary.version} supports native macOS and Linux only; WSL is out of scope for the v1.0 self-serve install. Use a native macOS or Linux host.`
          : `Pylon ${summary.version} supports macOS and Linux only. Current platform: ${summary.platform.current}`
        process.stderr.write(`${detail}\n`)
        process.exitCode = 1
        return
      }

      await writeBootstrapFiles(summary)
      const state = await ensurePylonLocalState(summary)
      const output = options.json
        ? { ...summary, localState: projectPublicStatus(state).state }
        : formatBootstrapText(summary)
      process.stdout.write(typeof output === 'string' ? output : `${JSON.stringify(output, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon bootstrap failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  // `pylon status` projects this node's public status. It is READ-ONLY and must
  // NEVER bind the control port: a bare `pylon status` (no `--json`) previously
  // fell through to the default node boot, which binds 4716 and crashes with
  // EADDRINUSE when the Autopilot/GUI node is already up (the Orwell report).
  //
  // Now `status` (with or without `--json`) detects a running node and reports
  // whether the node is reachable, then prints the file-only public-status
  // projection. `--remote`/`--connect` force the remote
  // read (error if no node is reachable instead of falling back).
  if (args[0] === 'status') {
    const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
    const state = await ensurePylonLocalState(summary)
    const probe = await probeRunningNode(state, Runtime.env)
    const forceRemote = remoteReadRequested(args, Runtime.env)
    if (forceRemote && !probe.reachable) {
      const payload = {
        ok: false,
        error: `no Pylon node reachable at ${probe.baseUrl} (start one with \`pylon node\`)`,
      }
      process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
      process.exitCode = 1
      return
    }
    const inventory = await discoverHostInventory({ env: Runtime.env })
    const projection = projectPublicStatus(state, inventory)
    const output = {
      ...projection,
      node: {
        running: probe.reachable,
        controlUrl: probe.baseUrl,
      },
    }
    process.stdout.write(`${JSON.stringify(output, null, 2)}\n`)
    return
  }

  if (args[0] === 'inventory' && args.includes('--json')) {
    const inventory = await discoverHostInventory({ env: Runtime.env })
    process.stdout.write(`${JSON.stringify(inventory, null, 2)}\n`)
    return
  }

  // `pylon doctor` — read-only health diagnostic. Like `status`, it must NEVER
  // bind the control port. Previously there was no top-level `doctor`, so
  // `pylon doctor` fell through to the default node boot and crashed with
  // EADDRINUSE when a node already held 4716 (the Orwell report).
  //
  // It reports: which node home was selected + WHY (public-safe label, never
  // the seed), whether a seed/identity is present, whether a node is running,
  // and whether a node is running. `--remote`/`--connect` force the remote read.
  if (args[0] === 'doctor') {
    const homeResolution = selectPylonHomeResolution(Runtime.env)
    const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
    const state = await ensurePylonLocalState(summary)
    const seedPresent = existsSync(state.paths.identityMnemonic)
    const probe = await probeRunningNode(state, Runtime.env)
    const forceRemote = remoteReadRequested(args, Runtime.env)
    if (forceRemote && !probe.reachable) {
      const payload = {
        ok: false,
        error: `no Pylon node reachable at ${probe.baseUrl} (start one with \`pylon node\`)`,
      }
      process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
      process.exitCode = 1
      return
    }
    const report = {
      ok: true,
      schema: 'openagents.pylon.doctor.v0.1',
      version: PYLON_VERSION,
      home: {
        // PUBLIC-SAFE: a path label + the selection reason, NEVER the seed.
        path: homeResolution.home,
        source: homeResolution.source,
        seedPresent,
      },
      node: {
        running: probe.reachable,
        controlUrl: probe.baseUrl,
        controlTokenPresent: probe.hasToken,
      },
    }
    // Defense-in-depth: the projection-safety guard rejects private material.
    assertPublicProjectionSafe(report)
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
    return
  }

  // `pylon update` — manual trigger for the same default-on OTA self-updater.
  //   --check  report only, don't apply
  //   --json   machine-readable result
  //   --channel <rc|stable>  override the channel (default rc)
  //   --feed-base <url>      override the feed origin (testing)
  if (args[0] === 'update') {
    const options = parseKeyValueOptions(args.slice(1))
    const json = options.json === true
    const checkOnly = options.check === true
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const state = await ensurePylonLocalState(summary)
      const result = await checkForUpdate({
        clientId: state.identity.nodeId,
        env: Runtime.env,
        ...(optionString(options, 'channel') ? { channel: optionString(options, 'channel') } : {}),
        ...(optionString(options, 'feed-base') ? { feedBase: optionString(options, 'feed-base') } : {}),
      })

      if (result.status !== 'update-available' || checkOnly) {
        const payload = { ...result, applied: false as const }
        process.stdout.write(json ? `${JSON.stringify(payload, null, 2)}\n` : `${describeCheck(result)}\n`)
        return
      }

      const target = resolveSelfBinaryPath()
      if (target === null) {
        const payload = {
          status: 'dev-noop' as const,
          currentVersion: result.currentVersion,
          candidate: result.release.version,
          applied: false as const,
        }
        process.stdout.write(
          json
            ? `${JSON.stringify(payload, null, 2)}\n`
            : `Update ${result.release.version} available; running from source (no binary to replace).\n`,
        )
        return
      }

      const applied = await downloadAndApply({
        release: result.release,
        targetPath: target,
      })
      const payload = {
        status: 'updated' as const,
        fromVersion: result.currentVersion,
        toVersion: applied.version,
        targetPath: applied.targetPath,
        applied: true as const,
      }
      process.stdout.write(
        json
          ? `${JSON.stringify(payload, null, 2)}\n`
          : `Updated ${result.currentVersion} -> ${applied.version}. Restart pylon to run the new version.\n`,
      )
      return
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (json)
        process.stdout.write(`${JSON.stringify({ status: 'error', error: message, applied: false }, null, 2)}\n`)
      else process.stderr.write(`Pylon update failed: ${message}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'auth') {
    try {
      const options = parsePylonAuthArgs(args.slice(1))
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const onDevicePrompt = options.json
        ? undefined
        : (prompt: { userCode: string; verificationUrl: string }) => {
            process.stdout.write(`${prompt.verificationUrl}\n${prompt.userCode}\n`)
          }

      if (options.target === 'openagents') {
        const result = await runPylonAuthOpenAgents(summary, options, {
          env: Runtime.env,
          onDevicePrompt,
        })
        if (options.json) {
          process.stdout.write(`${JSON.stringify(result.projection, null, 2)}\n`)
        }
        return
      }

      if (options.target === 'claude') {
        const projection = await runPylonAuthClaude(summary, options, {
          env: Runtime.env,
        })
        if (options.json) {
          process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
        } else {
          const verb = projection.localClaude.setupTokenStatus === 'skipped_existing_auth' ? 'Reused' : 'Connected'
          process.stdout.write(`✓ ${verb} Claude account (${projection.accountRef})\n`)
        }
        return
      }

      const projection = await runPylonAuthCodex(summary, options, {
        env: Runtime.env,
        onDevicePrompt,
      })
      if (options.json) {
        process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
      } else {
        const outcome = pylonCodexAuthCliOutcome(projection.localCodex.deviceLoginStatus, projection.localCodex.reason)
        if (!outcome.ok) {
          // A present but revoked/expired credential that could not be recovered
          // must NEVER be reported as a bare success.
          process.stderr.write(
            `⚠ Codex account ${projection.accountRef} has invalid credentials${
              outcome.reason ? ` (${outcome.reason})` : ''
            }; automatic re-login did not complete.\n` +
              `Run: pylon auth codex --account ${projection.accountRef} --force-device-login\n`,
          )
          process.exitCode = 1
          return
        }
        // EP250 regression: post-success decoration (the local email lookup)
        // must never convert a completed local connect into a
        // `Pylon auth failed` exit — guard it and degrade to a ref-only line.
        let email: string | null = null
        try {
          const codexAccounts = await collectPylonCodexAccountsLocal(summary, {
            env: Runtime.env,
          })
          email = codexAccounts.find((account) => account.accountRef === projection.accountRef)?.email ?? null
        } catch {
          email = null
        }
        const verb = outcome.kind === 'reauthed' ? 'Re-authenticated' : 'Linked'
        const usageNote = outcome.reason === 'usage_limited' ? ' (note: account is usage-limited right now)' : ''
        process.stdout.write(
          `✓ ${verb} Codex account${email ? `: ${email}` : ''} (${projection.accountRef})${usageNote}\n`,
        )
        if (projection.status === 'connected_local_only') {
          // Success-with-warning (never a bare failure): local credentials
          // are written and the account is registered; only the OpenAgents
          // provider-account import is pending (e.g. server unreachable).
          process.stderr.write(
            `⚠ OpenAgents provider-account import did not complete for ${projection.accountRef}; ` +
              `local credentials are ready for local fleet work. ` +
              `Re-run this connect when openagents.com is reachable to finish the server link.\n`,
          )
        }
      }
      return
    } catch (error) {
      process.stderr.write(`Pylon auth failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  const codexAccountsAlias = args[0] === 'codex' && args[1] === 'accounts'
  const accountCommandArgs = args[0] === 'accounts' ? args.slice(1) : codexAccountsAlias ? args.slice(2) : null

  if (args[0] === 'codex' && args[1] === 'fleet') {
    try {
      const command = args[2]
      if (command !== 'offload-plan') {
        throw new Error(
          'usage: pylon codex fleet offload-plan --accounts <refs> --target <host:capacity> [--target <host:capacity> ...] --json',
        )
      }
      const options = parseCodexFleetOffloadArgs(args.slice(3))
      if (!options.json) {
        throw new Error(
          'usage: pylon codex fleet offload-plan --accounts <refs> --target <host:capacity> [--target <host:capacity> ...] --json',
        )
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const plan = await createCodexFleetOffloadPlan(summary, options)
      process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon Codex fleet failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (accountCommandArgs !== null) {
    try {
      const command = accountCommandArgs[0]
      if (command === 'list') {
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        if (accountCommandArgs.includes('--json')) {
          const projection = await collectPylonAccountsList(summary, {
            env: Runtime.env,
          })
          process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
          return
        }
        if (!codexAccountsAlias) {
          const projection = await collectPylonAccountsList(summary, {
            env: Runtime.env,
          })
          const present = projection.accounts.filter((account) => account.homeState === 'present')
          if (present.length === 0) {
            process.stdout.write('No connected Pylon accounts.\n')
            return
          }
          process.stdout.write(`Connected Pylon accounts (${present.length}):\n`)
          for (const account of present) {
            const ref = account.accountRef ?? '(default)'
            process.stdout.write(`  ${account.provider.padEnd(14)} ${ref.padEnd(18)} ${account.readiness.state}\n`)
          }
          return
        }
        // The Codex namespace alias retains its local-only email/linked-at view.
        const codex = await collectPylonCodexAccountsLocal(summary, {
          env: Runtime.env,
        })
        const presentCodex = codex.filter((account) => account.homeState === 'present')
        if (presentCodex.length === 0) {
          process.stdout.write('No connected Codex accounts.\n')
          return
        }
        process.stdout.write(`Connected Codex accounts (${presentCodex.length}):\n`)
        for (const account of presentCodex) {
          const ref = account.accountRef ?? '(default)'
          const email = account.email ?? '(email unavailable)'
          const when = account.lastLinkedAt ? new Date(account.lastLinkedAt).toLocaleString() : 'unknown'
          process.stdout.write(`  ${ref.padEnd(14)} ${email.padEnd(34)} linked ${when}\n`)
        }
        return
      }
      if (command === 'usage') {
        const options = parsePylonAccountsUsageArgs(accountCommandArgs.slice(1))
        if (!options.json) {
          throw new Error(
            'usage: pylon accounts usage [--account <ref-or-provider>|--provider <codex|claude_agent>|--all] [--refresh] [--report-local-codex-usage] --json',
          )
        }
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        const refreshResult = options.refresh
          ? await runAccountsUsageRefresh(summary, options)
          : { attemptedCount: 0, blockerRefs: [] }
        const directLocalCodexReport = await reportDirectLocalCodexUsage(summary, options, { env: Runtime.env })
        const projection = await collectPylonAccountsUsage(summary, options, {
          env: Runtime.env,
        })
        process.stdout.write(
          `${JSON.stringify(
            {
              ...projection,
              refresh: {
                ...projection.refresh,
                performed: options.refresh && refreshResult.attemptedCount > 0,
                directLocalCodexReport,
                blockerRefs: [...projection.refresh.blockerRefs, ...refreshResult.blockerRefs],
              },
              blockerRefs: [...projection.blockerRefs, ...refreshResult.blockerRefs],
            },
            null,
            2,
          )}\n`,
        )
        return
      }
      if (command === 'status') {
        const options = parsePylonAccountsStatusArgs(accountCommandArgs.slice(1))
        if (!options.json) {
          throw new Error(
            'usage: pylon accounts status [--account <ref-or-provider>|--provider <codex|claude_agent>|--all] [--reset] --json',
          )
        }
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        const projection = await collectPylonAccountsStatus(summary, options, {
          env: Runtime.env,
        })
        process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
        return
      }
      if (command === 'connect') {
        const options = parsePylonAccountsConnectArgs(accountCommandArgs.slice(1))
        if (!options.json) {
          throw new Error(
            'usage: pylon accounts connect codex|claude|grok --account <ref> [provider-specific options] --json',
          )
        }
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        const projection = await runPylonAccountsConnect(summary, options, {
          env: Runtime.env,
        })
        process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
        return
      }
      if (command === 'maintenance') {
        // Typed per-harness maintenance (MAINT-1, #8785): status projection or
        // detect → pin → update → RE-PROBE → provenance receipt. Updates
        // BINARIES only; never runs a login flow and never touches ~/.codex.
        const maintenanceArgs = accountCommandArgs.slice(1)
        if (!maintenanceArgs.includes('--json')) {
          throw new Error(
            'usage: pylon accounts maintenance [--update --harness <codex|claude|opencode> [--channel <c>] [--allow-channel-jump]] --json',
          )
        }
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        const deps = { env: Runtime.env as Record<string, string | undefined> }
        if (!maintenanceArgs.includes('--update')) {
          const projection = await collectHarnessMaintenanceStatus(deps)
          process.stdout.write(`${JSON.stringify(projection, null, 2)}\n`)
          return
        }
        const harnessIndex = maintenanceArgs.indexOf('--harness')
        const harnessRaw = harnessIndex >= 0 ? maintenanceArgs[harnessIndex + 1] : undefined
        const harness = harnessRaw === undefined ? null : normalizeMaintenanceHarness(harnessRaw)
        if (harness === null) {
          throw new Error(
            'usage: pylon accounts maintenance --update --harness <codex|claude|opencode> [--channel <c>] [--allow-channel-jump] --json',
          )
        }
        const channelIndex = maintenanceArgs.indexOf('--channel')
        const channelRaw = channelIndex >= 0 ? maintenanceArgs[channelIndex + 1] : undefined
        const knownChannels: readonly HarnessInstallChannel[] = [
          'npm-global',
          'bun-global',
          'pnpm-global',
          'homebrew',
          'native',
        ]
        if (channelRaw !== undefined && !knownChannels.includes(channelRaw as HarnessInstallChannel)) {
          throw new Error(`unknown --channel ${channelRaw}; expected one of ${knownChannels.join('|')}`)
        }
        const receipt = await runHarnessMaintenanceUpdate({
          harness,
          ...(channelRaw === undefined ? {} : { channel: channelRaw as HarnessInstallChannel }),
          allowChannelJump: maintenanceArgs.includes('--allow-channel-jump'),
          deps,
        })
        await persistHarnessMaintenanceReceipt(summary, receipt)
        process.stdout.write(`${JSON.stringify(projectPublicHarnessMaintenanceReceipt(receipt), null, 2)}\n`)
        if (receipt.outcome === 'failed' || receipt.outcome === 'channel_jump_refused') {
          process.exitCode = 1
        }
        return
      }
      throw new Error('usage: pylon accounts list|usage|status|connect|maintenance ...')
    } catch (error) {
      process.stderr.write(`Pylon accounts failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'context') {
    try {
      if (!args.includes('--json')) throw new Error('usage: pylon context --json [--codex-danger]')
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const projection = await collectPylonContextProjection({
        cwd: codexComposerWorkingDirectory(),
        dangerFlag: args.includes('--codex-danger'),
        env: Runtime.env,
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

  if (args[0] === 'operator' && args[1] === 'snapshot' && args.includes('--json')) {
    const inventory = await discoverHostInventory({ env: Runtime.env })
    const snapshot = createOperatorSnapshot({ inventory })
    process.stdout.write(`${JSON.stringify(snapshot, null, 2)}\n`)
    return
  }

  if (args[0] === 'presence') {
    try {
      const command = args[1]
      const presenceArgs = args.slice(2)
      const options = parsePresenceOptions(presenceArgs)
      // Presence one-shots are JSON-native. `--json` is accepted as an
      // idempotent no-op for supervisor/runbook parity with other Pylon
      // machine-readable commands.
      void optionFlag(options, 'json')
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error('presence commands require --base-url or PYLON_OPENAGENTS_BASE_URL')
      }
      const summary = await createPresenceBootstrapSummary(presenceArgs, Runtime.env)
      const resolvedAgentToken = await resolveOpenAgentsAgentToken({
        env: Runtime.env,
        explicitAgentToken: optionString(options, 'agent-token') ?? null,
        summary,
      })
      const clientOptions = {
        ...presenceClientOptionsFromEnv({ baseUrl, env: Runtime.env }),
        ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
        ...(command === 'heartbeat'
          ? {
              activeRunCounts: await serverActiveCodingRunCounts(summary, {
                ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
                baseUrl,
              }),
              activeRunCountsByAccount: await serverActiveCodingRunAccountCounts(summary, {
                ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
                baseUrl,
              }),
            }
          : {}),
      }
      const result =
        command === 'register'
          ? await registerPylon(summary, clientOptions)
          : command === 'heartbeat'
            ? await sendHeartbeat(summary, clientOptions)
            : command === 'link-complete'
              ? await completePylonLink(summary, clientOptions)
              : command === 'link-refresh'
                ? await refreshPylonLink(summary, clientOptions)
                : null
      if (!result) throw new Error(`unknown presence command: ${command ?? ''}`)
      if (Runtime.env.PYLON_PRESENCE_ONESHOT_TEST_HOLD_HANDLE === '1') {
        setInterval(() => undefined, 60_000)
      }
      writeJsonAndExit(result)
    } catch (error) {
      process.stderr.write(`Pylon presence failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'forum' || args[0] === 'memories' || args[0] === 'ask-artanis') {
    try {
      const surfaceArgs = args[0] === 'ask-artanis' ? args.slice(2) : args.slice(args[0] === 'forum' ? 2 : 1)
      const options = parseKeyValueOptions(surfaceArgs)
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const state = await ensurePylonLocalState(summary)
      const networkOptions = {
        agentToken: optionString(options, 'agent-token') ?? Runtime.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl: baseUrl ?? 'https://openagents.com',
      }
      if (args[0] === 'memories') {
        const entries = await readMemories(summary.paths.home)
        process.stdout.write(`${JSON.stringify({ count: entries.length, memories: entries }, null, 2)}\n`)
        return
      }
      if (args[0] === 'forum') {
        const sub = args[1]
        if (sub === 'read') {
          const topicId = args[2]
          if (!topicId) throw new Error('usage: pylon forum read <topic-id>')
          const topic = await forumReadTopic(networkOptions, topicId)
          process.stdout.write(`${JSON.stringify(topic, null, 2)}\n`)
          return
        }
        if (sub === 'post') {
          const forumSlug = stringPsionicOption(options, 'forum') ?? ARTANIS_FORUM_SLUG
          const title = stringPsionicOption(options, 'title')
          const body = stringPsionicOption(options, 'body')
          if (!title || !body) throw new Error('usage: pylon forum post --title T --body B [--forum slug]')
          const result = await forumPostTopic(networkOptions, {
            bodyText: body,
            forumSlug,
            title,
          })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: 'forum_post',
            refs: {
              topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null,
            },
            summary: `posted forum topic: ${title.slice(0, 80)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        if (sub === 'reply') {
          const topicId = args[2]
          const body = stringPsionicOption(options, 'body')
          if (!topicId || !body) throw new Error('usage: pylon forum reply <topic-id> --body B')
          const result = await forumReply(networkOptions, {
            bodyText: body,
            topicId,
          })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: 'forum_reply',
            refs: { topicId },
            summary: `replied in topic ${topicId.slice(0, 12)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        throw new Error('usage: pylon forum post|read|reply ...')
      }
      // ask-artanis
      const question = args[1]
      if (!question || question.startsWith('--')) {
        throw new Error('usage: pylon ask-artanis "your question" [--base-url URL]')
      }
      const inventory = await discoverHostInventory({ env: Runtime.env })
      const memories = await readMemories(summary.paths.home, 10)
      const adapter = resolveModelAdapter(Runtime.env)
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
        forumSlug: stringPsionicOption(options, 'forum') ?? ARTANIS_FORUM_SLUG,
        title,
      })
      await appendMemory(summary.paths.home, {
        at: new Date().toISOString(),
        kind: 'ask_artanis',
        refs: {
          composedBy: composed.composedBy,
          topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null,
        },
        summary: `asked artanis: ${question.slice(0, 80)}`,
      })
      process.stdout.write(`${JSON.stringify({ composedBy: composed.composedBy, result }, null, 2)}\n`)
      return
    } catch (error) {
      process.stdout.write(
        `${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`,
      )
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'dev' && args[1] === 'doctor') {
    try {
      if (!args.includes('--json')) {
        throw new Error('usage: pylon dev doctor --json [--codex-danger] [--claude-danger]')
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const projection = await collectPylonDevDoctor({
        claudeDangerFlag: args.includes('--claude-danger'),
        cwd: process.cwd(),
        dangerFlag: args.includes('--codex-danger'),
        env: Runtime.env,
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

  if (args[0] === 'dev' && (args[1] === 'check' || args[1] === 'apply' || args[1] === 'reload')) {
    try {
      const command = args[1]
      const options = parseDevLoopOptions(args.slice(2))
      if (!options.json) {
        throw new Error(
          `usage: pylon dev ${command} --json [--allow-dirty]${command === 'check' ? ' [--command <argv>]' : ''}`,
        )
      }
      if (options.command && command !== 'check') {
        throw new Error('--command is only supported for pylon dev check')
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const cwd = codexComposerWorkingDirectory()
      const result =
        command === 'check'
          ? await runPylonDevCheck({
              allowDirty: options.allowDirty,
              commands: devCommandSpecFromOption(options.command, cwd),
              cwd,
              env: Runtime.env,
              summary,
            })
          : command === 'apply'
            ? await runPylonDevApply({
                allowDirty: options.allowDirty,
                cwd,
                env: Runtime.env,
                summary,
              })
            : await runPylonDevReload({
                cwd,
                env: Runtime.env,
                summary,
              })
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
      if (result.state === 'blocked' || result.state === 'failed') process.exitCode = 1
      return
    } catch (error) {
      process.stderr.write(`Pylon dev ${args[1]} failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'work') {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), 'pylon work')
      rejectClaudeLocalDangerForPublicPath(args.slice(1), 'pylon work')
      const command = args[1]
      const workArgs = args.slice(2).flatMap((arg) => (arg === '--events' ? ['--events', 'true'] : [arg]))
      const options = parseKeyValueOptions(workArgs)
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error('work commands require --base-url or PYLON_OPENAGENTS_BASE_URL')
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const networkOptions = {
        agentToken: optionString(options, 'agent-token') ?? Runtime.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }

      if (command === 'submit') {
        const objective = args[2]
        const budgetCents = Number(options['budget-cents'] ?? options.budget ?? 0)
        if (!objective || objective.startsWith('--') || !Number.isInteger(budgetCents) || budgetCents < 0) {
          throw new Error(
            'usage: pylon work submit "<objective>" --commit <40-char-sha> [--adapter codex|claude_agent|fable] [--budget-cents <cents>] [--repo owner/repo] [--branch main] [--verify "pnpm exec vp test"]',
          )
        }
        const adapter = options.adapter
        if (adapter !== undefined && adapter !== 'codex' && adapter !== 'claude_agent' && adapter !== 'fable') {
          throw new Error('work submit --adapter must be codex, claude_agent, or fable')
        }
        const result = await submitPylonAutopilotWork(networkOptions, {
          ...(adapter === undefined ? {} : { adapter }),
          branch: optionString(options, 'branch'),
          budgetCents,
          commit: optionString(options, 'commit'),
          objective,
          repository: optionString(options, 'repo'),
          verificationCommand: optionString(options, 'verify'),
        })
        await appendMemory(summary.paths.home, {
          at: new Date().toISOString(),
          kind: 'autopilot_work_submit',
          refs: {
            state: (result.work as { state?: unknown } | undefined)?.state ?? null,
            workOrderRef: (result.work as { workOrderRef?: unknown } | undefined)?.workOrderRef ?? null,
          },
          summary: `submitted autopilot work ${String((result.work as { workOrderRef?: unknown } | undefined)?.workOrderRef ?? 'unknown').slice(0, 48)}`,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === 'request') {
        const objective = args[2]
        const budgetSats = Number(options.budget)
        if (!objective || objective.startsWith('--') || !Number.isInteger(budgetSats) || budgetSats <= 0) {
          throw new Error(
            'usage: pylon work request "<objective>" --budget <sats> [--repo URL] [--verify command] [--deadline iso]',
          )
        }
        const result = await createPylonWorkRequest(networkOptions, {
          budgetSats,
          deadline: optionString(options, 'deadline'),
          objective,
          repository: optionString(options, 'repo'),
          verificationCommand: optionString(options, 'verify'),
        })
        await appendMemory(
          summary.paths.home,
          workRequestMemoryEntry({
            at: new Date().toISOString(),
            result,
          }),
        )
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === 'offers') {
        const requestRef = args[2]
        if (!requestRef) throw new Error('usage: pylon work offers <request-ref>')
        const result = await listPylonWorkOffers(networkOptions, requestRef)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === 'accept') {
        const requestRef = args[2]
        const quoteRef = args[3]
        if (!requestRef || !quoteRef) throw new Error('usage: pylon work accept <request-ref> <quote-ref>')
        const result = await acceptPylonWorkOffer(networkOptions, {
          quoteRef,
          requestRef,
        })
        await appendMemory(
          summary.paths.home,
          workAcceptanceMemoryEntry({
            at: new Date().toISOString(),
            quoteRef,
            requestRef,
            result,
          }),
        )
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === 'status') {
        const requestRef = args[2]
        if (!requestRef) throw new Error('usage: pylon work status <request-ref>')
        const includeEvents = options.events !== undefined && options.events !== 'false' && options.events !== '0'
        if (requestRef.startsWith('autopilot_work_order.') || includeEvents) {
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

      if (command === 'review') {
        const workOrderRef = args[2]
        const action = options.action
        if (!workOrderRef || (action !== 'accept' && action !== 'reject' && action !== 'request_changes')) {
          throw new Error('usage: pylon work review <work-order-ref> --action accept|reject|request_changes')
        }
        const result = await reviewPylonAutopilotWork(networkOptions, {
          action,
          workOrderRef,
        })
        await appendMemory(summary.paths.home, {
          at: new Date().toISOString(),
          kind: 'autopilot_work_review',
          refs: { action, workOrderRef },
          summary: `reviewed autopilot work ${workOrderRef.slice(0, 48)} as ${action}`,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      throw new Error('usage: pylon work submit|status|review|request|offers|accept ...')
    } catch (error) {
      process.stdout.write(
        `${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`,
      )
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'mcp') {
    try {
      const command = args[1]
      const optionArgs = command === 'config' ? args.slice(2) : args.slice(1)
      const options = parseKeyValueOptions(optionArgs)
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL ?? 'https://openagents.com'
      if (command === 'config') {
        process.stdout.write(
          `${JSON.stringify(
            pylonKhalaMcpConfig({
              baseUrl,
              command: optionString(options, 'command') ?? 'pylon',
            }),
            null,
            2,
          )}\n`,
        )
        return
      }

      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const resolvedAgentToken = await resolveOpenAgentsAgentToken({
        env: Runtime.env,
        explicitAgentToken: optionString(options, 'agent-token') ?? null,
        summary,
      })
      await runPylonKhalaMcpStdio({
        network: {
          agentToken: resolvedAgentToken?.token ?? '',
          baseUrl,
        },
      })
      return
    } catch (error) {
      process.stdout.write(
        `${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`,
      )
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'khala') {
    try {
      const command = args[1]
      const khalaArgs = args.slice(2)
      const options = parseKeyValueOptions(khalaArgs)
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error('khala commands require --base-url or PYLON_OPENAGENTS_BASE_URL')
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const resolvedAgentToken = await resolveOpenAgentsAgentToken({
        env: Runtime.env,
        explicitAgentToken: optionString(options, 'agent-token') ?? null,
        summary,
      })
      const networkOptions = {
        agentToken: resolvedAgentToken?.token ?? '',
        baseUrl,
      }
      const emit = (payload: Record<string, unknown>) => {
        if (optionFlag(options, 'json')) {
          process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
          return
        }
        const text =
          typeof payload.text === 'string' && payload.text.trim() !== ''
            ? payload.text
            : JSON.stringify(payload, null, 2)
        process.stdout.write(`${text}\n`)
      }

      if (command === 'apm') {
        const state = await ensurePylonLocalState(summary)
        const result = await collectKhalaApmProjection({
          agentToken: resolvedAgentToken?.token ?? '',
          baseUrl,
          state,
        })
        emit(result)
        return
      }

      if (command === 'spawn') {
        const objective =
          optionString(options, 'objective') ??
          optionString(options, 'prompt') ??
          (args[2] !== undefined && !args[2].startsWith('--') ? args[2] : undefined)
        if (!objective) {
          throw new Error(
            'usage: pylon khala spawn --count <n> --objective <text> [--workflow claude_agent_task|codex_agent_task] [--fixture | --commit <sha> --repo owner/repo --verify <argv>] [--pylon-ref <pylonRef>] [--account <ref>] [--max-parallel n] [--execute] [--lifecycle-ndjson] [--json]',
          )
        }
        const workflow = optionString(options, 'workflow') ?? 'codex_agent_task'
        if (workflow !== 'claude_agent_task' && workflow !== 'codex_agent_task') {
          throw new Error('khala spawn --workflow must be claude_agent_task or codex_agent_task')
        }
        const spawnWorkflow = workflow as PylonKhalaSpawnWorkflow
        const accountProvider = spawnWorkflow === 'claude_agent_task' ? 'claude_agent' : 'codex'
        const workerKind = spawnWorkflow === 'claude_agent_task' ? 'Claude' : 'Codex'
        const count = positiveIntegerOption(options, 'count', 'khala spawn --count') ?? 1
        const maxParallel = positiveIntegerOption(options, 'max-parallel', 'khala spawn --max-parallel')
        const commit = optionString(options, 'commit')
        const repository = optionString(options, 'repo') ?? 'OpenAgentsInc/openagents'
        const verificationCommand = optionString(options, 'verify')
        const explicitFixture =
          optionFlag(options, 'fixture') || optionFlag(options, 'fixture-smoke') || optionFlag(options, 'codex-fixture')
        const hasWorkspacePin =
          commit !== undefined || verificationCommand !== undefined || optionString(options, 'repo') !== undefined
        if (explicitFixture && hasWorkspacePin) {
          throw new Error('khala spawn --fixture cannot be combined with --commit, --repo, or --verify')
        }
        if (!explicitFixture) {
          const missingPins = [
            commit === undefined ? '--commit' : null,
            verificationCommand === undefined ? '--verify' : null,
          ].filter((pin): pin is string => pin !== null)
          if (missingPins.length > 0) {
            throw new Error(
              `khala spawn requires explicit fixture intent (--fixture) or complete workspace pins (--commit, --repo, --verify); missing ${missingPins.join(', ')}`,
            )
          }
        }
        const state = await ensurePylonLocalState(summary)
        const targetPylonRef =
          optionString(options, 'pylon-ref') ?? optionString(options, 'target-pylon-ref') ?? localPylonTargetRef(state)
        const capacityEnv =
          spawnWorkflow === 'claude_agent_task'
            ? khalaClaudeCapacityAdvertisementEnv(Runtime.env, Math.max(count, maxParallel ?? 0))
            : khalaCodexCapacityAdvertisementEnv(Runtime.env, Math.max(count, maxParallel ?? 0))
        if (optionFlag(options, 'execute')) {
          try {
            await sendHeartbeat(summary, {
              ...presenceClientOptionsFromEnv({ baseUrl, env: capacityEnv }),
              ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
              activeRunCounts: await serverActiveCodingRunCounts(summary, networkOptions),
              activeRunCountsByAccount: await serverActiveCodingRunAccountCounts(summary, networkOptions),
            })
          } catch {
            // Presence freshness is rechecked by run-no-spend; this best-effort
            // push keeps advertised capacity current for the spawn planner.
          }
        }
        const accountSelector = optionString(options, 'account') ?? optionString(options, 'account-ref')
        const allAccounts = await collectPylonAccountsList(summary, {
          env: capacityEnv,
        })
        const accounts =
          accountSelector === undefined
            ? allAccounts
            : {
                ...allAccounts,
                accounts: allAccounts.accounts.filter(
                  (account) =>
                    account.provider === accountProvider &&
                    (account.accountRef === accountSelector ||
                      account.accountRefHash === accountSelector ||
                      (account.accountRef === null && /^(?:default|\(default\))$/iu.test(accountSelector))),
                ),
              }
        if (accountSelector !== undefined && accounts.accounts.length === 0) {
          throw new Error(`khala spawn account ${accountSelector} is not a connected ${workerKind} account`)
        }
        const advertisedCodexAccounts =
          spawnWorkflow === 'claude_agent_task'
            ? await localClaudeDispatchAccounts(summary, state, capacityEnv, networkOptions)
            : await localCodexDispatchAccounts(summary, state, capacityEnv, networkOptions)
        const advertisedCodexAvailability =
          advertisedCodexAccounts.length > 0
            ? advertisedCodexAccounts.reduce((sum, account) => sum + account.available, 0)
            : spawnWorkflow === 'claude_agent_task'
              ? await availableClaudeAssignments(summary, state, networkOptions, capacityEnv)
              : await availableCodexAssignments(summary, state, networkOptions, capacityEnv)
        const dispatchBreakers = await activeDispatchBreakersForPlanning(summary)
        const workspace = explicitFixture
          ? undefined
          : buildPylonKhalaGitCheckoutWorkspace({
              branch: optionString(options, 'branch'),
              commit,
              repository,
              verificationCommand,
            })
        const plan = buildPylonKhalaSpawnPlan({
          accounts,
          advertisedCodexAccounts,
          advertisedCodexAvailability,
          baseUrl,
          ...(optionString(options, 'branch') === undefined ? {} : { branch: optionString(options, 'branch') }),
          ...(commit === undefined ? {} : { commit }),
          dispatchBreakers,
          fixture: explicitFixture,
          ...(maxParallel === undefined ? {} : { maxParallel }),
          objectives: repeatedKhalaSpawnObjectives({ count, objective }),
          repository,
          targetPylonRef,
          ...(verificationCommand === undefined ? {} : { verificationCommand }),
          workflow: spawnWorkflow,
          ...(workspace === undefined ? {} : { workspace }),
        })
        if (!optionFlag(options, 'execute')) {
          emit(plan)
          return
        }
        const result = await runPylonKhalaSpawnPlan({
          ...(lifecycleNdjsonRequested(options)
            ? {
                deps: {
                  onWorkerLifecycle: (event) => {
                    process.stderr.write(`${JSON.stringify(event)}\n`)
                  },
                },
              }
            : {}),
          network: networkOptions,
          plan,
          summary,
        })
        emit(result)
        return
      }

      if (command === 'burndown') {
        const commit = optionString(options, 'commit')
        const repository = optionString(options, 'repo') ?? 'OpenAgentsInc/openagents'
        const verificationCommand = optionString(options, 'verify')
        const missingPins = [
          commit === undefined ? '--commit' : null,
          verificationCommand === undefined ? '--verify' : null,
        ].filter((pin): pin is string => pin !== null)
        if (commit === undefined || verificationCommand === undefined) {
          throw new Error(
            `usage: pylon khala burndown [--issues <#,...> | --roadmap <path>] --commit <sha> --verify <argv> [--repo owner/repo] [--max-parallel n] [--iterations n] [--execute] [--json]; missing ${missingPins.join(', ')}`,
          )
        }
        const state = await ensurePylonLocalState(summary)
        const targetPylonRef =
          optionString(options, 'pylon-ref') ?? optionString(options, 'target-pylon-ref') ?? localPylonTargetRef(state)
        const advertisedCodexAvailability = await availableCodexAssignments(summary, state, networkOptions)
        if (optionFlag(options, 'execute')) {
          try {
            await sendHeartbeat(summary, {
              ...presenceClientOptionsFromEnv({ baseUrl, env: Runtime.env }),
              ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
              activeRunCounts: await serverActiveCodingRunCounts(summary, networkOptions),
              activeRunCountsByAccount: await serverActiveCodingRunAccountCounts(summary, networkOptions),
            })
          } catch {
            // #6355: execution still proceeds; run-no-spend reports the precise
            // presence blocker if the server-side view remains stale.
          }
        }
        const issuesOption = optionString(options, 'issues')
        const issueNumbers =
          issuesOption === undefined
            ? await readKhalaRoadmapIssueNumbers(
                optionString(options, 'roadmap') ?? 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
              )
            : parseKhalaBurndownIssueNumbers(issuesOption)
        const accounts = await collectPylonAccountsList(summary, {
          env: Runtime.env,
        })
        const advertisedCodexAccounts = await localCodexDispatchAccounts(summary, state, Runtime.env, networkOptions)
        const dispatchBreakers = await activeDispatchBreakersForPlanning(summary)
        const maxParallel = positiveIntegerOption(options, 'max-parallel', 'khala burndown --max-parallel')
        const iterations = positiveIntegerOption(options, 'iterations', 'khala burndown --iterations')
        const branch = optionString(options, 'branch')
        const plan = buildPylonKhalaBurndownPlan({
          accounts,
          baseUrl,
          ...(branch === undefined ? {} : { branch }),
          commit,
          advertisedCodexAccounts,
          advertisedCodexAvailability,
          dispatchBreakers,
          issueNumbers,
          ...(iterations === undefined ? {} : { iterations }),
          ...(maxParallel === undefined ? {} : { maxParallel }),
          repository,
          targetPylonRef,
          verificationCommand,
        })
        if (!optionFlag(options, 'execute')) {
          emit(plan)
          return
        }
        const result = await runPylonKhalaBurndownPlan({
          network: networkOptions,
          plan,
          summary,
        })
        emit(result)
        return
      }

      if (command === 'dispatch') {
        const commit = optionString(options, 'commit')
        const repository = optionString(options, 'repo') ?? 'OpenAgentsInc/openagents'
        const verificationCommand = optionString(options, 'verify')
        const candidatesOption = optionString(options, 'candidates') ?? optionString(options, 'candidate-refs')
        const accountsOption = optionString(options, 'accounts') ?? optionString(options, 'account-targets')
        const missingPins = [
          commit === undefined ? '--commit' : null,
          verificationCommand === undefined ? '--verify' : null,
          candidatesOption === undefined ? '--candidates' : null,
          accountsOption === undefined ? '--accounts' : null,
        ].filter((pin): pin is string => pin !== null)
        if (missingPins.length > 0) {
          throw new Error(
            `usage: pylon khala dispatch --candidates <pr:1,issue:2> --accounts <refs> --commit <sha> --verify <argv> [--repo owner/repo] [--concurrency n] [--priority-lane name] [--json]; missing ${missingPins.join(', ')}`,
          )
        }
        const candidateRefsArg = candidatesOption as string
        const accountTargetsArg = accountsOption as string
        const pinnedCommit = commit as string
        const pinnedVerificationCommand = verificationCommand as string
        const state = await ensurePylonLocalState(summary)
        const targetPylonRef =
          optionString(options, 'pylon-ref') ?? optionString(options, 'target-pylon-ref') ?? localPylonTargetRef(state)
        const accounts = await collectPylonAccountsList(summary, {
          env: Runtime.env,
        })
        const wantedAccounts = new Set(
          accountTargetsArg
            .split(',')
            .map((value) => value.trim())
            .filter((value) => value !== ''),
        )
        const accountTargets: KhalaDispatchAccountTarget[] = accounts.accounts
          .filter(
            (account) =>
              account.provider === 'codex' &&
              ((account.accountRef !== null && wantedAccounts.has(account.accountRef)) ||
                wantedAccounts.has(account.accountRefHash)),
          )
          .map((account) => ({
            accountRef: account.accountRef,
            accountRefHash: account.accountRefHash,
            provider: 'codex',
          }))
        const branch = optionString(options, 'branch')
        const plan = buildPylonKhalaDispatchPlan({
          accountTargets,
          candidateRefs: normalizeKhalaDispatchCandidateRefs(candidateRefsArg.split(',')),
          concurrency: positiveIntegerOption(options, 'concurrency', 'khala dispatch --concurrency') ?? 1,
          dispatchBreakers: await activeDispatchBreakersForPlanning(summary),
          priorityLane: optionString(options, 'priority-lane') ?? 'default',
          targetPylonRef,
          verifier: {
            ...(branch === undefined ? {} : { branch }),
            commit: pinnedCommit,
            command: pinnedVerificationCommand,
            repository,
          },
        })
        emit(plan)
        return
      }

      if (command === 'request') {
        const prompt =
          optionString(options, 'prompt') ??
          optionString(options, 'objective') ??
          (args[2] !== undefined && !args[2].startsWith('--') ? args[2] : undefined)
        const workflow = optionString(options, 'workflow')
        const explicitTargetPylonRef = optionString(options, 'pylon-ref') ?? optionString(options, 'target-pylon-ref')
        const commit = optionString(options, 'commit')
        const repository = optionString(options, 'repo')
        const verificationCommand = optionString(options, 'verify')
        const explicitFixture =
          optionFlag(options, 'fixture') || optionFlag(options, 'fixture-smoke') || optionFlag(options, 'codex-fixture')
        if (!prompt) {
          throw new Error(
            'usage: pylon khala request --prompt <text> [--workflow claude_agent_task|cloud_coding_session|codex_agent_task] [--pylon-ref <pylonRef>] [--fixture | --commit <sha> --repo <owner/repo> --verify <argv>] [--no-run] [--lifecycle-ndjson] [--json]; public issue/repo codex_agent_task and claude_agent_task requests require complete workspace pins',
          )
        }
        if (
          workflow !== undefined &&
          workflow !== 'claude_agent_task' &&
          workflow !== 'cloud_coding_session' &&
          workflow !== 'codex_agent_task'
        ) {
          throw new Error(
            'khala request --workflow must be claude_agent_task, cloud_coding_session, or codex_agent_task',
          )
        }
        const hasWorkspacePin = commit !== undefined || repository !== undefined || verificationCommand !== undefined
        if (explicitFixture && hasWorkspacePin) {
          throw new Error('khala request --fixture cannot be combined with --commit, --repo, or --verify')
        }
        if ((workflow === 'codex_agent_task' || workflow === 'claude_agent_task') && !explicitFixture) {
          const missingPins = [
            commit === undefined ? '--commit' : null,
            repository === undefined ? '--repo' : null,
            verificationCommand === undefined ? '--verify' : null,
          ].filter((pin): pin is string => pin !== null)
          if (missingPins.length > 0) {
            throw new Error(
              `khala request --workflow ${workflow} requires explicit fixture intent (--fixture) or complete workspace pins (--commit, --repo, --verify); missing ${missingPins.join(', ')}`,
            )
          }
        }
        // #6354/#6421: resolve the requested account to its public-safe
        // account-ref hash BEFORE the request so the server gate admits against
        // that account's per-account capacity. The same selector then runs the
        // local no-spend assignment on that account's isolated home. The
        // provider is derived from the workflow so a claude_agent_task resolves a
        // `claude_agent` account (otherwise the selector would reject a Claude
        // ref with "not registered for this provider").
        const accountProvider = workflow === 'claude_agent_task' ? 'claude_agent' : 'codex'
        const targetsLocalCodingWorkflow = workflow === 'codex_agent_task' || workflow === 'claude_agent_task'
        const requestState = targetsLocalCodingWorkflow ? await ensurePylonLocalState(summary) : null
        const targetPylonRef =
          explicitTargetPylonRef ?? (requestState === null ? undefined : localPylonTargetRef(requestState))
        if (requestState !== null && targetPylonRef === localPylonTargetRef(requestState)) {
          const requestPresenceEnv =
            workflow === 'codex_agent_task' ? khalaCodexCapacityAdvertisementEnv(Runtime.env, 5) : Runtime.env
          await sendHeartbeat(summary, {
            ...presenceClientOptionsFromEnv({
              baseUrl,
              env: requestPresenceEnv,
            }),
            ...(resolvedAgentToken === null ? {} : { agentToken: resolvedAgentToken.token }),
            activeRunCounts: await serverActiveCodingRunCounts(summary, networkOptions),
            activeRunCountsByAccount: await serverActiveCodingRunAccountCounts(summary, networkOptions),
          })
        }
        const accountRef = optionString(options, 'account') ?? optionString(options, 'account-ref')
        const accountHome = optionString(options, 'account-home')
        const accountSelection =
          accountRef === undefined && accountHome === undefined
            ? null
            : await resolvePylonAccountSelection(summary, {
                provider: accountProvider,
                ...(accountRef === undefined ? {} : { accountRef }),
                ...(accountHome === undefined ? {} : { accountHome }),
              })
        const targetAccountRefHash = accountSelection?.accountRefHash
        const result = await issuePylonKhalaRequest(networkOptions, {
          prompt,
          ...(targetPylonRef === undefined ? {} : { targetPylonRef }),
          ...(targetAccountRefHash === undefined ? {} : { targetAccountRefHash }),
          ...(workflow === undefined ? {} : { workflow: workflow as PylonKhalaWorkflow }),
          ...(hasWorkspacePin
            ? {
                objectiveSummary: prompt,
                workspace: buildPylonKhalaGitCheckoutWorkspace({
                  branch: optionString(options, 'branch'),
                  commit,
                  repository,
                  verificationCommand,
                }),
              }
            : {}),
        })
        const assignmentRef = result.assignmentRef
        const shouldRunAssignment = assignmentRef !== null && !optionFlag(options, 'no-run')
        if (!shouldRunAssignment) {
          emit({
            ...result,
            assignmentRun: null,
            assignmentLifecycleEvents: [],
            autoRun: {
              attempted: false,
              reason: assignmentRef === null ? 'no_assignment_ref' : 'disabled_by_no_run',
              schema: 'openagents.pylon.khala_request_auto_run.v0.1',
            },
          })
          return
        }
        const assignmentLifecycleEvents: AssignmentRunLifecycleEvent[] = []
        const assignmentRun = await runNoSpendAssignment(summary, {
          ...networkOptions,
          assignmentRef,
          ...(accountRef === undefined ? {} : { accountRef }),
          ...(accountHome === undefined ? {} : { accountHome }),
          ...(lifecycleNdjsonRequested(options)
            ? {
                onLifecycleEvent: (event) => {
                  assignmentLifecycleEvents.push(event)
                  process.stderr.write(`${JSON.stringify(event)}\n`)
                },
              }
            : {}),
        })
        emit({
          ...result,
          assignmentRun,
          assignmentLifecycleEvents,
          autoRun: {
            assignmentRef,
            attempted: true,
            ok: assignmentRun.ok,
            schema: 'openagents.pylon.khala_request_auto_run.v0.1',
          },
        })
        await new Promise<void>((resolve, reject) => {
          process.stdout.write('', (error) => {
            if (error) reject(error)
            else resolve()
          })
        })
        process.exit(0)
        return
      }

      if (command === 'resume') {
        const durableRequestId =
          args[2] !== undefined && !args[2].startsWith('--') ? args[2] : optionString(options, 'resume')
        if (!durableRequestId) {
          throw new Error('usage: pylon khala resume <durable-request-id> [--offset <n>] [--json]')
        }
        const result = await resumePylonKhalaRequest(networkOptions, {
          durableRequestId,
          offset: optionString(options, 'offset'),
        })
        emit(result)
        return
      }

      if (command === 'status') {
        const assignmentRef = optionString(options, 'assignment-ref')
        if (assignmentRef !== undefined) {
          const result = await readPylonKhalaAssignmentTraceStatus(networkOptions, assignmentRef)
          emit(result)
          return
        }
        const durableRequestId =
          args[2] !== undefined && !args[2].startsWith('--') ? args[2] : optionString(options, 'resume')
        if (!durableRequestId) {
          throw new Error(
            'usage: pylon khala status <durable-request-id> [--json] | pylon khala status --assignment-ref <assignmentRef> [--json]',
          )
        }
        const result = await readPylonKhalaStatus(networkOptions, durableRequestId)
        emit(result)
        return
      }

      if (command === 'proof') {
        const assignmentRef =
          args[2] !== undefined && !args[2].startsWith('--') ? args[2] : optionString(options, 'assignment-ref')
        if (!assignmentRef) {
          throw new Error('usage: pylon khala proof <assignmentRef> [--json]')
        }
        const result = await readPylonKhalaProof(networkOptions, assignmentRef)
        emit(result)
        return
      }

      if (command === 'closeout') {
        const assignmentRef =
          args[2] !== undefined && !args[2].startsWith('--') ? args[2] : optionString(options, 'assignment-ref')
        if (!assignmentRef) {
          throw new Error('usage: pylon khala closeout <assignmentRef> [--json]')
        }
        const result = await readPylonKhalaCloseout(networkOptions, assignmentRef)
        emit(result)
        return
      }

      throw new Error('usage: pylon khala request|resume|status|proof|closeout|spawn|burndown ...')
    } catch (error) {
      process.stdout.write(
        `${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`,
      )
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'assignment') {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), 'pylon assignment')
      rejectClaudeLocalDangerForPublicPath(args.slice(1), 'pylon assignment')
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const baseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error('assignment commands require --base-url or PYLON_OPENAGENTS_BASE_URL')
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const resolvedAgentToken = await resolveOpenAgentsAgentToken({
        env: Runtime.env,
        explicitAgentToken: optionString(options, 'agent-token') ?? null,
        summary,
      })
      const agentToken = resolvedAgentToken?.token
      const clientOptions = { ...(agentToken ? { agentToken } : {}), baseUrl }
      if (command === 'poll') {
        const leases = await pollAssignments(summary, clientOptions)
        process.stdout.write(`${JSON.stringify({ leases }, null, 2)}\n`)
        return
      }
      if (command === 'accept') {
        const leaseJson = optionString(options, 'lease')
        if (!leaseJson) throw new Error('assignment accept requires --lease JSON')
        const result = await acceptAssignment(summary, JSON.parse(leaseJson) as PylonAssignmentLease, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === 'progress') {
        const progressJson = optionString(options, 'progress')
        if (!progressJson) throw new Error('assignment progress requires --progress JSON')
        const result = await submitAssignmentProgress(
          summary,
          JSON.parse(progressJson) as AssignmentProgress,
          clientOptions,
        )
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === 'closeout') {
        const closeoutJson = optionString(options, 'closeout')
        if (!closeoutJson) throw new Error('assignment closeout requires --closeout JSON')
        const result = await submitAssignmentCloseout(
          summary,
          JSON.parse(closeoutJson) as AssignmentCloseout,
          clientOptions,
        )
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === 'run-no-spend') {
        const assignmentRef = optionString(options, 'assignment-ref') ?? optionString(options, 'lease-ref')
        const accountRef = optionString(options, 'account') ?? optionString(options, 'account-ref')
        const accountHome = optionString(options, 'account-home')
        const emitJsonLifecycle = lifecycleNdjsonRequested(options)
        const result = await runNoSpendAssignment(summary, {
          ...clientOptions,
          ...(accountRef === undefined ? {} : { accountRef }),
          ...(accountHome === undefined ? {} : { accountHome }),
          ...(assignmentRef === undefined ? {} : { assignmentRef }),
          ...(emitJsonLifecycle
            ? {
                onLifecycleEvent: (event) => {
                  process.stderr.write(`${JSON.stringify(event)}\n`)
                },
              }
            : {}),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown assignment command: ${command ?? ''}`)
    } catch (error) {
      process.stderr.write(`Pylon assignment failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'provider') {
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), 'pylon provider')
      rejectClaudeLocalDangerForPublicPath(args.slice(1), 'pylon provider')
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
      const state = await ensurePylonLocalState(summary)
      const providerBaseUrl = optionString(options, 'base-url') ?? Runtime.env.PYLON_OPENAGENTS_BASE_URL
      const providerAgentToken =
        providerBaseUrl === undefined
          ? null
          : await resolveOpenAgentsAgentToken({
              env: Runtime.env,
              explicitAgentToken: optionString(options, 'agent-token') ?? null,
              summary,
            })
      const providerNetworkOptions =
        providerBaseUrl === undefined || providerAgentToken === null
          ? undefined
          : { agentToken: providerAgentToken.token, baseUrl: providerBaseUrl }
      if (command === 'go-online' || command === 'online') {
        const connectedAccounts = await loadPylonAccountRegistry(summary)
        const connectedClaudeHomes = connectedAccounts
          .filter((entry) => entry.provider === 'claude_agent' && entry.paused !== true)
          .map((entry) => entry.home)
        const claudeAgentReadiness = await probeClaudeAgentReadiness({
          config: await loadClaudeAgentConfig(summary),
          ...(connectedClaudeHomes.length === 0
            ? {}
            : {
                localSessionProbe: async () => {
                  for (const home of connectedClaudeHomes) {
                    if (await pylonClaudeAccountHomeHasAuth(home)) return true
                  }
                  return false
                },
              }),
        })
        // #6331: a Codex account connected via `accounts connect codex
        // --openagents-link` writes its auth.json into an isolated per-account
        // home, never `~/.codex`. Surface those homes so go-online declares the
        // codex capability (and the heartbeat advertises codex capacity) even
        // when the default `~/.codex` is empty.
        const connectedCodexHomes = connectedAccounts
          .filter((entry) => entry.provider === 'codex')
          .map((entry) => entry.home)
        const codexAgentReadiness = await probeCodexAgentReadiness({
          config: await loadCodexAgentConfig(summary),
          codexAccountHomes: connectedCodexHomes,
        })
        const appleFmStatus = await collectPylonAppleFmStatus({
          env: Runtime.env,
          summary,
        })
        const nextRuntime = {
          ...state.runtime,
          lifecycle: 'online' as const,
          capabilityRefs: withWorkspaceMaterializerCapability(
            withCodexAgentCapability(
              withClaudeAgentCapability(
                withAppleFmBackendCapabilities(
                  [...new Set([...state.runtime.capabilityRefs.filter((ref) => !/tassadar|psionic/i.test(ref))])],
                  appleFmStatus,
                ),
                claudeAgentReadiness,
              ),
              codexAgentReadiness,
            ),
          ),
          blockerRefs: [
            ...new Set([
              ...state.runtime.blockerRefs.filter(
                (ref) => ref !== 'blocker.assignment.lifecycle_offline' && !/tassadar|psionic/i.test(ref),
              ),
              ...appleFmStatus.blockerRefs,
            ]),
          ],
        }
        await writeRuntimeState(state.paths, nextRuntime)
        const codingCapacity = await codingCapacityForDispatch(
          summary,
          { ...state, runtime: nextRuntime },
          providerNetworkOptions,
        )
        // #6354: per-Codex-account capacity so `provider go-online --json`
        // exposes each linked account's own concurrent slots and busy load.
        const codexAccounts = await localCodexAccountCapacities(
          { ...state, runtime: nextRuntime },
          summary,
          Runtime.env,
          codexBusyByAccount(
            await codingAccountBusyCountsForDispatch(
              summary,
              { ...state, runtime: nextRuntime },
              providerNetworkOptions,
            ),
          ),
        )
        const codexAccountTotals = codexAccounts.reduce(
          (totals, account) => ({
            available: totals.available + account.available,
            busy: totals.busy + account.busy,
            queued: totals.queued + account.queued,
            ready: totals.ready + account.ready,
          }),
          { available: 0, busy: 0, queued: 0, ready: 0 },
        )
        const codexAccountRefs = codexAccountCapacityRefs(codexAccounts)
        const appleFmRefs = appleFmBackendCapacityRefs(appleFmStatus)
        const codexCapacity = codingCapacity.find((item) => item.service === 'codex') ?? {
          available: 0,
          busy: 0,
          queued: 0,
          ready: 0,
          service: 'codex' as const,
        }
        const codexDispatchCapacity =
          codexAccounts.length > 0
            ? {
                ...codexCapacity,
                available: codexAccountTotals.available,
                busy: codexAccountTotals.busy,
                queued: codexAccountTotals.queued,
                ready: codexAccountTotals.ready,
              }
            : codexCapacity
        const codingCapacityProjection = codingCapacity.some((item) => item.service === 'codex')
          ? codingCapacity.map((item) => (item.service === 'codex' ? codexDispatchCapacity : item))
          : [codexDispatchCapacity, ...codingCapacity]
        const codingRefs = codingServiceCapacityRefs(codingCapacityProjection)
        const result = {
          ok: true,
          pylonRef: state.identity.pylonRef,
          lifecycle: nextRuntime.lifecycle,
          capabilityRefs: nextRuntime.capabilityRefs,
          codingCapacity: codingCapacityProjection,
          claudeAgent: {
            state: claudeAgentReadiness.state,
            credentialSourceRef: claudeAgentReadiness.credentialSourceRef,
          },
          codexAgent: {
            state: codexAgentReadiness.state,
            credentialSourceRef: codexAgentReadiness.credentialSourceRef,
          },
          appleFmBackend: {
            backendKind: appleFmStatus.backendKind,
            profileId: appleFmStatus.profileId,
            model: appleFmStatus.model,
            status: appleFmStatus.status,
            available: appleFmStatus.available,
            capabilityRef: appleFmStatus.capability,
            advertisedCapabilities: appleFmStatus.advertisedCapabilities,
            capacityRefs: appleFmRefs.capacityRefs,
            blockerRefs: appleFmStatus.blockerRefs,
          },
          ownCapacityDispatch: {
            schema: 'openagents.pylon.own_capacity_dispatch.v1',
            codex: codexDispatchCapacity,
            assignmentGateRef: 'gate.public.pylon.assignment_dispatch.controlled.v1',
            capacityRefs: [
              ...codingRefs.capacityRefs.filter((ref) => ref.startsWith('capacity.coding.codex.')),
              ...codexAccountRefs.capacityRefs,
            ],
            loadRefs: [
              ...codingRefs.loadRefs.filter((ref) => ref.startsWith('load.coding.codex.')),
              ...codexAccountRefs.loadRefs,
            ],
            policyRefs: ['policy.public.khala_coding.own_capacity_only'],
            requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
            maxCodexAssignments: codexDispatchCapacity.ready,
            availableCodexAssignments: codexDispatchCapacity.available,
            // #6354: per-account breakdown. When present, the dispatch totals
            // above are derived from these account buckets so Desktop status
            // and spawn planning describe the same capacity.
            codexAccounts: codexAccounts.map((account) => ({
              accountKey: account.accountKey,
              available: account.available,
              busy: account.busy,
              queued: account.queued,
              ready: account.ready,
            })),
            totalAvailableCodexAssignments: codexAccountTotals.available,
            totalMaxCodexAssignments: codexAccountTotals.ready,
          },
          stateRef: 'state.public.pylon.online',
        }
        assertPublicProjectionSafe(result)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === 'go-offline' || command === 'offline') {
        const nextRuntime = {
          ...state.runtime,
          lifecycle: 'offline' as const,
        }
        await writeRuntimeState(state.paths, nextRuntime)
        process.stdout.write(
          `${JSON.stringify(
            {
              ok: true,
              lifecycle: nextRuntime.lifecycle,
              stateRef: 'state.public.pylon.offline',
            },
            null,
            2,
          )}\n`,
        )
        return
      }
      throw new Error(`unknown provider command: ${command ?? ''}`)
    } catch (error) {
      process.stderr.write(`Pylon provider failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === 'node') {
    if (args[1] === 'fleet-run-intake-status') {
      try {
        const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Runtime.env)
        const state = await ensurePylonLocalState(summary)
        const probe = await probeRunningNode(state, Runtime.env)
        const status = await readControlCommand(probe, {
          type: 'fleet_run.intake_status',
        })
        if (status === null) {
          throw new Error('running Pylon node intake status is unavailable')
        }
        process.stdout.write(`${JSON.stringify(status, null, 2)}\n`)
        return
      } catch (error) {
        process.stderr.write(
          `Pylon FleetRun intake status failed: ${error instanceof Error ? error.message : String(error)}\n`,
        )
        process.exitCode = 1
        return
      }
    }
    try {
      rejectCodexLocalDangerForPublicPath(args.slice(1), 'pylon node')
      rejectClaudeLocalDangerForPublicPath(args.slice(1), 'pylon node')
    } catch (error) {
      process.stderr.write(`Pylon node-core failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
    await maybeAutoUpdate()
    const nodeExit = await Effect.runPromise(
      Effect.scoped(runHeadlessNode).pipe(
        Effect.catch((error) => {
          process.stderr.write(`${formatNodeStartupError(error)}\n`)
          return Effect.succeed(1 as const)
        }),
      ),
    )
    process.exit(typeof nodeExit === 'number' ? nodeExit : 0)
  }

  if (args[0] === 'runtime' || runtimeCommandNamespaces.has(args[0] ?? '')) {
    const runtimeArgs = args[0] === 'runtime' ? args.slice(1) : args
    const result = await Effect.runPromise(runProbeCli(runtimeArgs, { env: Runtime.env }))
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    process.exitCode = result.exitCode
    return
  }

  // Default (no subcommand): Pylon is a headless, CLI-only node. Booting with
  // no arguments runs the same node-core as `pylon node` — services + event
  // stream + loopback control API, logging to stdout, no interactive UI.
  try {
    rejectCodexLocalDangerForPublicPath(args, 'pylon')
    rejectClaudeLocalDangerForPublicPath(args, 'pylon')
  } catch (error) {
    process.stderr.write(`Pylon node-core failed: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
    return
  }
  await maybeAutoUpdate()
  const defaultExit = await Effect.runPromise(
    Effect.scoped(runHeadlessNode).pipe(
      Effect.catch((error) => {
        process.stderr.write(`${formatNodeStartupError(error)}\n`)
        return Effect.succeed(1 as const)
      }),
    ),
  )
  // Scope is closed: services interrupted, process can exit cleanly. Exit
  // explicitly so lingering library handles cannot hold the process open
  // after a deliberate shutdown. A startup failure exits non-zero.
  process.exit(typeof defaultExit === 'number' ? defaultExit : 0)
}

await main()
