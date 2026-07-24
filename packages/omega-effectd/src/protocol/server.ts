/**
 * Framed protocol server for supervised omega-effectd.
 *
 * Owns durable Full Auto registries under the injected data root.
 * Mutation API remains full-auto-run-actions. GPUI is not a caller here.
 */

import { createInterface } from "node:readline"

import {
  resolveAgentComputerSessionsPath,
  resolveFullAutoNativeBindingsPath,
  resolveFullAutoRegistryPath,
  resolveFullAutoRunReportsPath,
  resolveFullAutoRunsPath,
  type OmegaEffectdPaths,
} from "../paths.ts"
import { openFullAutoRegistry } from "../engine/full-auto-registry.ts"
import {
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  openFullAutoRunRegistry,
} from "../engine/full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "../engine/full-auto-run-report.ts"
import {
  assessFullAutoNativeBoundary,
  buildFullAutoNativeBinding,
  openFullAutoNativeBindingStore,
  projectFullAutoNativeEvidence,
} from "../engine/full-auto-native-binding.ts"
import {
  openAgentComputerSessionStore,
  refreshAgentComputerSession,
  runAgentComputerTurn,
  startAgentComputerSession,
} from "../engine/agent-computer-sessions.ts"
import type { FetchLike } from "@openagentsinc/agent-harness-environment"
import {
  FULL_AUTO_CONTROL_CALLER_LABEL,
  getFullAutoRunAction,
  getFullAutoRunReceiptAction,
  getFullAutoRunReportAction,
  listFullAutoRunsAction,
  pauseFullAutoRunAction,
  resumeFullAutoRunAction,
  retryFullAutoRunNowAction,
  startFullAutoRunAction,
  stopFullAutoRunAction,
  type FullAutoRunActionContext,
} from "../engine/full-auto-run-actions.ts"
import {
  applyFullAutoRunControlIntent,
  type FullAutoRunControlAction,
} from "../engine/full-auto-run-control-intent.ts"
import type { FullAutoControlCapabilities } from "../engine/full-auto-control-server.ts"
import { decodeFullAutoControlRunStartRequest } from "../engine/full-auto-control-contract.ts"
import { FULL_AUTO_DEFAULT_LANE, FULL_AUTO_LANE_POLICIES } from "../engine/full-auto-lane.ts"
import {
  FULL_AUTO_MAX_CONCURRENT_RUNS,
  projectFullAutoCapacityLedger,
} from "../engine/full-auto-capacity.ts"
import { FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS } from "../engine/full-auto-reconcile.ts"
import {
  decideFullAutoLivenessNotification,
  type FullAutoStallCause,
} from "../engine/full-auto-liveness.ts"
import type { FullAutoRotationReason } from "../engine/full-auto-registry.ts"
import type { FullAutoRunState } from "../engine/full-auto-run-registry.ts"
import type { OmegaEffectdService } from "../service.ts"
import {
  isOmegaEffectdRequest,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  OMEGA_EFFECTD_PROTOCOL_VERSION,
  OMEGA_EFFECTD_SERVICE_VERSION,
  redactDiagnosticText,
  type OmegaEffectdAttentionResult,
  type OmegaEffectdCapacityResult,
  type OmegaEffectdHealthResult,
  type OmegaEffectdInitializeResult,
  type OmegaEffectdNativeBinding,
  type OmegaEffectdNativeEvidence,
  type OmegaEffectdProtocolError,
  type OmegaEffectdPublishProjectionResult,
  type OmegaEffectdResponse,
  type OmegaEffectdRunDetail,
  type OmegaEffectdSyncStatus,
} from "./framed.ts"

const OWNER_CONFIGURABLE_GUARDRAILS = Object.freeze([
  "maxWallClockMs",
  "maxTurns",
  "maxPerTurnFailures",
  "tokenBudgetRef",
] as const)

const redactedError = (
  code: OmegaEffectdProtocolError["code"],
  message: string,
): OmegaEffectdProtocolError => ({
  code,
  message: redactDiagnosticText(message),
})

export type OmegaEffectdFramedServer = Readonly<{
  generation: () => number
  handleLine: (line: string) => Promise<OmegaEffectdResponse | null>
  serveStdio: () => Promise<void>
}>

export type OmegaEffectdFramedServerOptions = Readonly<{
  /** Test-only fetch injection for Agent Computer Worker calls. */
  agentComputerFetch?: FetchLike
  /** Test-only sleep injection for Agent Computer turn polling. */
  agentComputerSleep?: (durationMs: number) => import("effect").Effect.Effect<void>
}>

export const createOmegaEffectdFramedServer = (
  service: OmegaEffectdService,
  paths: OmegaEffectdPaths,
  options: OmegaEffectdFramedServerOptions = {},
): OmegaEffectdFramedServer => {
  let generation = 0
  let initialized = false

  const runRegistry = openFullAutoRunRegistry(resolveFullAutoRunsPath(paths))
  const registry = openFullAutoRegistry(resolveFullAutoRegistryPath(paths))
  const reportStore = openFullAutoRunReportStore(resolveFullAutoRunReportsPath(paths))
  const nativeBindings = openFullAutoNativeBindingStore(resolveFullAutoNativeBindingsPath(paths))
  const agentComputerSessions = openAgentComputerSessionStore(
    resolveAgentComputerSessionsPath(paths),
  )

  const projectDetail = (
    ctx: FullAutoRunActionContext,
    runRef: string,
  ): OmegaEffectdRunDetail | null => {
    const outcome = getFullAutoRunAction(ctx, runRef)
    if (!outcome.ok) return null
    const run = outcome.value
    const report = getFullAutoRunReportAction(ctx, runRef)
    const turns = report.ok
      ? report.value.turns.map(turn => ({
          turnRef: turn.turnRef,
          lane: turn.lane,
          outcomeSummary: turn.outcomeSummary,
          createdAt: turn.createdAt,
        }))
      : []
    const binding = nativeBindings.get(runRef)
    const nativeEvidence: OmegaEffectdNativeEvidence | null = binding
      ? projectFullAutoNativeEvidence(binding)
      : null
    return {
      runRef: run.runRef,
      threadRef: run.threadRef ?? null,
      state: run.state,
      title: run.title,
      objective: run.objective,
      doneCondition: run.doneCondition,
      workspaceRef: run.workspaceRef ?? null,
      lane: run.lane ?? null,
      turnCap: run.turnCap,
      successfulAttempts: run.successfulAttempts,
      failedAttempts: run.failedAttempts,
      stallCause: run.stallCause ?? null,
      recoveryAction: run.recoveryAction,
      terminalReason: run.terminalReason ?? null,
      updatedAt: run.lastProgressAt ?? run.createdAt,
      nativeEvidence,
      turns,
    }
  }
  const capabilities: FullAutoControlCapabilities = {
    registry,
    runRegistry,
    reportStore,
    resolveWorkspaceRef: () => "workspace.omega.supervised",
    triggerReconciliation: async () => {},
    liveState: () => null,
    listTurns: () => [],
    appendSystemNote: () => {},
    createThread: (_title, _laneRef) => `thread.omega.${Date.now().toString(36)}`,
    isLaneEligible: laneRef => laneRef === FULL_AUTO_DEFAULT_LANE || laneRef === "claude-local",
    interruptLiveTurn: () => false,
  }

  const actionContext = (): FullAutoRunActionContext => ({
    capabilities,
    now: () => new Date(),
    actor: "control_api",
    callerLabel: FULL_AUTO_CONTROL_CALLER_LABEL,
  })

  const mobileActionContext = (): FullAutoRunActionContext => ({
    capabilities,
    now: () => new Date(),
    actor: "mobile",
    callerLabel: "mobile control intent",
  })

  const syncStatus = (): OmegaEffectdSyncStatus => ({
    available: false,
    publishBlocksDispatch: false,
    reason: "omega_khala_sync_session_unavailable",
  })

  const projectCapacity = (): OmegaEffectdCapacityResult => {
    const active = runRegistry.activeRuns()
    const coolingByLane = new Map<string, FullAutoRotationReason>()
    for (const record of registry.list()) {
      const history = record.rotationHistory ?? []
      const last = history[history.length - 1]
      if (last === undefined) continue
      if (
        last.reason === "account_exhausted" ||
        last.reason === "rate_limited" ||
        last.reason === "provider_error"
      ) {
        coolingByLane.set(last.toLane, last.reason)
      }
    }
    const lanes = projectFullAutoCapacityLedger({
      laneGate: laneRef => {
        if (!(laneRef in FULL_AUTO_LANE_POLICIES)) return null
        const eligible =
          capabilities.isLaneEligible?.(laneRef) ?? laneRef === FULL_AUTO_DEFAULT_LANE
        return eligible ? { admitted: true, fullAuto: true } : { admitted: false, fullAuto: false }
      },
      activeRunsByLane: lane =>
        active.filter(run => (run.profile?.lane ?? null) === lane).length,
      coolingReasonByLane: lane => coolingByLane.get(lane) ?? null,
    })
    return {
      activeRunLimit: FULL_AUTO_MAX_CONCURRENT_RUNS,
      activeRunCount: active.length,
      lanes,
      nonOverridableGuardrails: [...FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS],
      ownerConfigurableGuardrails: [...OWNER_CONFIGURABLE_GUARDRAILS],
      enabledThreadsNeverEvicted: true,
    }
  }

  const respond = (
    id: string,
    ok: boolean,
    result?: unknown,
    error?: OmegaEffectdProtocolError,
  ): OmegaEffectdResponse => ({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "response",
    id,
    generation,
    ok,
    ...(result === undefined ? {} : { result }),
    ...(error === undefined ? {} : { error }),
  })

  const requireGeneration = (requestGeneration: number, id: string): OmegaEffectdResponse | null => {
    if (!initialized) {
      return respond(id, false, undefined, redactedError("invalid_request", "Call initialize first."))
    }
    if (requestGeneration !== generation) {
      return respond(
        id,
        false,
        undefined,
        redactedError("stale_generation", `Expected generation ${generation}, got ${requestGeneration}.`),
      )
    }
    return null
  }

  const handle = async (request: {
    id: string
    generation: number
    method: string
    params?: unknown
  }): Promise<OmegaEffectdResponse> => {
    if (request.method === "initialize") {
      const params = (request.params ?? {}) as { generation?: number }
      if (typeof params.generation !== "number" || !Number.isInteger(params.generation) || params.generation < 1) {
        return respond(
          request.id,
          false,
          undefined,
          redactedError("invalid_request", "initialize requires integer generation >= 1."),
        )
      }
      generation = params.generation
      initialized = true
      await service.start()
      const health = service.health()
      const result: OmegaEffectdInitializeResult = {
        schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
        protocolVersion: OMEGA_EFFECTD_PROTOCOL_VERSION,
        serviceVersion: OMEGA_EFFECTD_SERVICE_VERSION,
        generation,
        capabilities: [
          "health",
          "list_runs",
          "get_run",
          "start",
          "pause",
          "resume",
          "stop",
          "retry",
          "get_capacity",
          "decide_attention",
          "get_report",
          "get_receipt",
          "apply_control_intent",
          "get_sync_status",
          "publish_projection",
          "get_native_binding",
          "assess_native_boundary",
          "start_agent_computer_session",
          "refresh_agent_computer_session",
          "run_agent_computer_turn",
          "get_agent_computer_session",
          "list_agent_computer_sessions",
        ],
        dataRoot: health.dataRoot,
        activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
      }
      return respond(request.id, true, result)
    }

    const fence = requireGeneration(request.generation, request.id)
    if (fence) return fence

    if (service.health().status !== "running") {
      return respond(request.id, false, undefined, redactedError("not_running", "omega-effectd is stopped."))
    }

    switch (request.method) {
      case "health": {
        const health = service.health()
        const result: OmegaEffectdHealthResult = {
          ok: true,
          status: health.status,
          generation,
          dataRoot: health.dataRoot,
          activeRunCount: runRegistry.activeRuns().length,
        }
        return respond(request.id, true, result)
      }
      case "list_runs": {
        const runs = listFullAutoRunsAction(actionContext()).map(run => ({
          runRef: run.runRef,
          threadRef: run.threadRef,
          state: run.state,
          title: run.title,
          updatedAt: run.lastProgressAt ?? run.createdAt,
        }))
        return respond(request.id, true, { runs })
      }
      case "get_run": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(request.id, false, undefined, redactedError("invalid_request", "get_run requires runRef."))
        }
        const detail = projectDetail(actionContext(), params.runRef)
        if (detail === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          )
        }
        return respond(request.id, true, { run: detail })
      }
      case "start": {
        const raw = (request.params ?? {}) as {
          projectRef?: string
          worktreeRef?: string
          worktreeAbsolutePath?: string
          gitHead?: string
          rebaseUnsafe?: boolean
        }
        const body = decodeFullAutoControlRunStartRequest(request.params ?? {})
        if (body === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "start requires workspaceRef, title, objective, and doneCondition.",
            ),
          )
        }
        if (raw.rebaseUnsafe === true) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "rebase_unsafe: refusing to start Full Auto on a rebase-unsafe worktree.",
            ),
          )
        }
        const outcome = startFullAutoRunAction(actionContext(), body)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          )
        }
        if (
          typeof raw.projectRef === "string" &&
          raw.projectRef.length > 0 &&
          typeof raw.worktreeRef === "string" &&
          raw.worktreeRef.length > 0
        ) {
          nativeBindings.put(
            buildFullAutoNativeBinding({
              runRef: outcome.value.runRef,
              workspaceRef: body.workspaceRef,
              projectRef: raw.projectRef,
              worktreeRef: raw.worktreeRef,
              worktreeAbsolutePath: raw.worktreeAbsolutePath,
              gitHead: raw.gitHead,
              rebaseUnsafe: false,
            }),
          )
        }
        const detail = projectDetail(actionContext(), outcome.value.runRef)
        return respond(request.id, true, { run: detail })
      }
      case "get_capacity": {
        return respond(request.id, true, projectCapacity())
      }
      case "decide_attention": {
        const params = (request.params ?? {}) as {
          runRef?: string
          permissionGranted?: boolean
          previousDedupKey?: string | null
        }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "decide_attention requires runRef."),
          )
        }
        const detail = projectDetail(actionContext(), params.runRef)
        if (detail === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          )
        }
        const decision = decideFullAutoLivenessNotification({
          runRef: detail.runRef,
          runTitle: detail.title,
          projectedState: detail.state as FullAutoRunState,
          cause: (detail.stallCause as FullAutoStallCause | null) ?? null,
          previousDedupKey:
            typeof params.previousDedupKey === "string" ? params.previousDedupKey : null,
          permissionGranted: params.permissionGranted === true,
        })
        const result: OmegaEffectdAttentionResult = decision
        return respond(request.id, true, { attention: result })
      }
      case "get_report": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_report requires runRef."),
          )
        }
        const outcome = getFullAutoRunReportAction(actionContext(), params.runRef)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", outcome.error.message),
          )
        }
        return respond(request.id, true, { report: outcome.value })
      }
      case "get_receipt": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_receipt requires runRef."),
          )
        }
        const outcome = getFullAutoRunReceiptAction(actionContext(), params.runRef)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", outcome.error.message),
          )
        }
        return respond(request.id, true, { receipt: outcome.value })
      }
      case "apply_control_intent": {
        const params = (request.params ?? {}) as {
          intentId?: string
          runRef?: string
          action?: string
        }
        if (
          typeof params.intentId !== "string" ||
          params.intentId.length === 0 ||
          typeof params.runRef !== "string" ||
          params.runRef.length === 0 ||
          (params.action !== "pause" && params.action !== "resume" && params.action !== "stop")
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "apply_control_intent requires intentId, runRef, and action pause|resume|stop.",
            ),
          )
        }
        const outcome = applyFullAutoRunControlIntent(mobileActionContext(), {
          intentId: params.intentId,
          runRef: params.runRef,
          action: params.action as FullAutoRunControlAction,
        })
        return respond(request.id, true, { outcome })
      }
      case "get_sync_status": {
        return respond(request.id, true, syncStatus())
      }
      case "publish_projection": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "publish_projection requires runRef."),
          )
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef)
        if (!run.ok) {
          const result: OmegaEffectdPublishProjectionResult = {
            ok: false,
            status: "run_not_found",
            reason: "No Full Auto run exists for that runRef.",
          }
          return respond(request.id, true, result)
        }
        // Honest stub: Sync publish never blocks local dispatch (exit criterion).
        const result: OmegaEffectdPublishProjectionResult = {
          ok: false,
          status: "sync_unavailable",
          reason: syncStatus().reason,
        }
        return respond(request.id, true, result)
      }
      case "get_native_binding": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_native_binding requires runRef."),
          )
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef)
        if (!run.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          )
        }
        const binding = nativeBindings.get(params.runRef)
        return respond(request.id, true, {
          binding: (binding as OmegaEffectdNativeBinding | null) ?? null,
        })
      }
      case "assess_native_boundary": {
        const params = (request.params ?? {}) as {
          runRef?: string
          currentWorktreePathDigest?: string
        }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "assess_native_boundary requires runRef."),
          )
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef)
        if (!run.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          )
        }
        const assessment = assessFullAutoNativeBoundary({
          binding: nativeBindings.get(params.runRef),
          expectedWorkspaceRef: capabilities.resolveWorkspaceRef(),
          currentWorktreePathDigest: params.currentWorktreePathDigest,
        })
        return respond(request.id, true, { assessment })
      }
      case "start_agent_computer_session": {
        const params = (request.params ?? {}) as {
          bearerToken?: string
          controlPlaneBaseUrl?: string
          repoRef?: string
          objective?: string
          adapter?: "codex" | "claude_agent"
          lane?: "cloud-gcp"
          verify?: ReadonlyArray<string>
        }
        if (
          typeof params.bearerToken !== "string" ||
          typeof params.controlPlaneBaseUrl !== "string" ||
          typeof params.repoRef !== "string" ||
          typeof params.objective !== "string"
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "start_agent_computer_session requires bearerToken, controlPlaneBaseUrl, repoRef, and objective.",
            ),
          )
        }
        const outcome = await startAgentComputerSession(agentComputerSessions, {
          bearerToken: params.bearerToken,
          controlPlaneBaseUrl: params.controlPlaneBaseUrl,
          repoRef: params.repoRef,
          objective: params.objective,
          ...(params.adapter === undefined ? {} : { adapter: params.adapter }),
          ...(params.lane === undefined ? {} : { lane: params.lane }),
          ...(params.verify === undefined ? {} : { verify: params.verify }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
        })
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          )
        }
        return respond(request.id, true, { session: outcome.session })
      }
      case "refresh_agent_computer_session": {
        const params = (request.params ?? {}) as {
          bearerToken?: string
          sessionRef?: string
        }
        if (typeof params.bearerToken !== "string" || typeof params.sessionRef !== "string") {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "refresh_agent_computer_session requires bearerToken and sessionRef.",
            ),
          )
        }
        const existing = agentComputerSessions.get(params.sessionRef)
        if (existing === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "No Agent Computer session exists for that sessionRef."),
          )
        }
        const outcome = await refreshAgentComputerSession(agentComputerSessions, {
          bearerToken: params.bearerToken,
          session: existing,
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
        })
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          )
        }
        return respond(request.id, true, { session: outcome.session })
      }
      case "run_agent_computer_turn": {
        const params = (request.params ?? {}) as {
          bearerToken?: string
          controlPlaneBaseUrl?: string
          repoRef?: string
          objective?: string
          adapter?: "codex" | "claude_agent"
          lane?: "cloud-gcp"
          verify?: ReadonlyArray<string>
        }
        if (
          typeof params.bearerToken !== "string" ||
          typeof params.controlPlaneBaseUrl !== "string" ||
          typeof params.repoRef !== "string" ||
          typeof params.objective !== "string"
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "run_agent_computer_turn requires bearerToken, controlPlaneBaseUrl, repoRef, and objective.",
            ),
          )
        }
        const outcome = await runAgentComputerTurn(agentComputerSessions, {
          bearerToken: params.bearerToken,
          controlPlaneBaseUrl: params.controlPlaneBaseUrl,
          repoRef: params.repoRef,
          objective: params.objective,
          ...(params.adapter === undefined ? {} : { adapter: params.adapter }),
          ...(params.lane === undefined ? {} : { lane: params.lane }),
          ...(params.verify === undefined ? {} : { verify: params.verify }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
          ...(options.agentComputerSleep === undefined
            ? {}
            : { sleep: options.agentComputerSleep }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { pollIntervalMs: 1, maxPollAttempts: 120 }),
        })
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          )
        }
        return respond(request.id, true, {
          session: outcome.session,
          finishReason: outcome.finishReason,
          eventKinds: outcome.eventKinds,
        })
      }
      case "get_agent_computer_session": {
        const params = (request.params ?? {}) as { sessionRef?: string }
        if (typeof params.sessionRef !== "string" || params.sessionRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_agent_computer_session requires sessionRef."),
          )
        }
        return respond(request.id, true, {
          session: agentComputerSessions.get(params.sessionRef),
        })
      }
      case "list_agent_computer_sessions": {
        return respond(request.id, true, {
          sessions: agentComputerSessions.list(),
        })
      }
      case "retry": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "retry requires runRef."),
          )
        }
        const outcome = retryFullAutoRunNowAction(actionContext(), params.runRef)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          )
        }
        const detail = projectDetail(actionContext(), params.runRef)
        return respond(request.id, true, { run: detail })
      }
      case "pause":
      case "resume":
      case "stop": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", `${request.method} requires runRef.`),
          )
        }
        const outcome =
          request.method === "pause"
            ? pauseFullAutoRunAction(actionContext(), params.runRef)
            : request.method === "resume"
              ? resumeFullAutoRunAction(actionContext(), params.runRef)
              : stopFullAutoRunAction(actionContext(), params.runRef)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          )
        }
        const detail = projectDetail(actionContext(), params.runRef)
        return respond(request.id, true, {
          run: detail,
        })
      }
      default:
        return respond(
          request.id,
          false,
          undefined,
          redactedError("unknown_method", `Unknown method ${request.method}.`),
        )
    }
  }

  const handleLine = async (line: string): Promise<OmegaEffectdResponse | null> => {
    const trimmed = line.trim()
    if (trimmed.length === 0) return null
    let parsed: unknown
    try {
      parsed = JSON.parse(trimmed)
    } catch {
      return respond(
        "invalid",
        false,
        undefined,
        redactedError("invalid_request", "Frame was not valid JSON."),
      )
    }
    if (!isOmegaEffectdRequest(parsed)) {
      return respond(
        typeof (parsed as { id?: unknown })?.id === "string"
          ? (parsed as { id: string }).id
          : "invalid",
        false,
        undefined,
        redactedError("invalid_request", "Frame was not an omega-effectd request."),
      )
    }
    try {
      return await handle(parsed)
    } catch (error) {
      const message = error instanceof Error ? error.message : "internal error"
      return respond(parsed.id, false, undefined, redactedError("internal", message))
    }
  }

  return {
    generation: () => generation,
    handleLine,
    serveStdio: async () => {
      const rl = createInterface({ input: process.stdin, crlfDelay: Infinity })
      for await (const line of rl) {
        const response = await handleLine(line)
        if (response !== null) {
          process.stdout.write(`${JSON.stringify(response)}\n`)
        }
      }
    },
  }
}
