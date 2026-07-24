/**
 * Framed protocol server for supervised omega-effectd.
 *
 * Owns durable Full Auto registries under the injected data root.
 * Mutation API remains full-auto-run-actions. GPUI is not a caller here.
 */

import { createInterface } from "node:readline"

import {
  resolveFullAutoRegistryPath,
  resolveFullAutoRunReportsPath,
  resolveFullAutoRunsPath,
  type OmegaEffectdPaths,
} from "../paths.ts"
import { openFullAutoRegistry } from "../engine/full-auto-registry.ts"
import {
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  openFullAutoRunRegistry,
  type FullAutoRun,
} from "../engine/full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "../engine/full-auto-run-report.ts"
import {
  FULL_AUTO_CONTROL_CALLER_LABEL,
  getFullAutoRunAction,
  listFullAutoRunsAction,
  pauseFullAutoRunAction,
  resumeFullAutoRunAction,
  stopFullAutoRunAction,
  type FullAutoRunActionContext,
} from "../engine/full-auto-run-actions.ts"
import type { FullAutoControlCapabilities } from "../engine/full-auto-control-server.ts"
import { FULL_AUTO_DEFAULT_LANE } from "../engine/full-auto-lane.ts"
import type { OmegaEffectdService } from "../service.ts"
import {
  isOmegaEffectdRequest,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  OMEGA_EFFECTD_PROTOCOL_VERSION,
  OMEGA_EFFECTD_SERVICE_VERSION,
  redactDiagnosticText,
  type OmegaEffectdHealthResult,
  type OmegaEffectdInitializeResult,
  type OmegaEffectdProtocolError,
  type OmegaEffectdResponse,
  type OmegaEffectdRunSnapshot,
} from "./framed.ts"

const projectSnapshot = (run: FullAutoRun): OmegaEffectdRunSnapshot => ({
  runRef: run.runRef,
  threadRef: run.threadRef ?? null,
  state: run.state,
  title: run.title,
  updatedAt: run.updatedAt,
})

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

export const createOmegaEffectdFramedServer = (
  service: OmegaEffectdService,
  paths: OmegaEffectdPaths,
): OmegaEffectdFramedServer => {
  let generation = 0
  let initialized = false

  const runRegistry = openFullAutoRunRegistry(resolveFullAutoRunsPath(paths))
  const registry = openFullAutoRegistry(resolveFullAutoRegistryPath(paths))
  const reportStore = openFullAutoRunReportStore(resolveFullAutoRunReportsPath(paths))

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
        capabilities: ["health", "list_runs", "get_run", "pause", "resume", "stop"],
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
          updatedAt: run.updatedAt,
        }))
        return respond(request.id, true, { runs })
      }
      case "get_run": {
        const params = (request.params ?? {}) as { runRef?: string }
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(request.id, false, undefined, redactedError("invalid_request", "get_run requires runRef."))
        }
        const outcome = getFullAutoRunAction(actionContext(), params.runRef)
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", outcome.error.message),
          )
        }
        return respond(request.id, true, {
          run: {
            runRef: outcome.value.runRef,
            threadRef: outcome.value.threadRef,
            state: outcome.value.state,
            title: outcome.value.title,
            updatedAt: outcome.value.updatedAt,
          },
        })
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
        const run = runRegistry.get(params.runRef)
        return respond(request.id, true, {
          run: run ? projectSnapshot(run) : null,
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
