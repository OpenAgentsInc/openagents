import type { BootstrapSummary } from "../bootstrap.js"
import {
  ControlCommandOperationalError,
  ControlCommandValidationError,
  controlCommandValidationReason,
} from "./control-command-error.js"
import {
  openPylonOwnedStandingFleetRunExecutor,
  type OpenPylonOwnedStandingFleetRunExecutorInput,
} from "../orchestration/fleet-run-owned-standing-executor.js"
import {
  openPylonFleetRunRuntime,
} from "../orchestration/fleet-run-runtime.js"
import {
  openPylonFleetRunExecutionReporter,
  type PylonFleetRunExecutionHttpPort,
  type PylonFleetRunExecutionReporter,
} from "../orchestration/fleet-run-execution-reporter.js"
import { projectFleetRunSupervisorObservation } from "../orchestration/fleet-run-execution-projection.js"

export const PYLON_FLEET_RUN_ACTIVATION_SCHEMA =
  "openagents.pylon.fleet_run_activation.v1" as const

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
export const PYLON_NODE_MAX_ACTIVE_FLEET_RUNS = 1

export type PylonFleetRunActivationReason =
  | "active_limit_reached"
  | "executor_close_failed"
  | "executor_open_failed"
  | "managed_cloud_executor_unconfigured"
  | "run_not_runnable"
  | "transport_not_configured"
  | "unsafe_stored_ref"
  | "unknown_stored_run"

export type PylonFleetRunActivationProjection = {
  schema: typeof PYLON_FLEET_RUN_ACTIVATION_SCHEMA
  pylonRef: string
  runRef: string
  armed: boolean
  active: boolean
  state: "active" | "armed_blocked" | "disarmed" | "disarmed_cleanup_blocked"
  reason: PylonFleetRunActivationReason | null
  retryable: boolean
}

export type PylonFleetRunActivationStatus = {
  schema: typeof PYLON_FLEET_RUN_ACTIVATION_SCHEMA
  pylonRef: string
  maxActiveRuns: number
  activeRuns: number
  invalidStoredRows: number
  blockerRefs: string[]
  runs: PylonFleetRunActivationProjection[]
}

export type PylonFleetRunExecutorHandle = {
  readonly close: () => Promise<void>
}

export type PylonFleetRunExecutorOpener = (
  input: OpenPylonOwnedStandingFleetRunExecutorInput,
) => Promise<PylonFleetRunExecutorHandle>

export type OpenPylonNodeFleetRunActivationServiceInput = {
  readonly agentToken?: string | undefined
  readonly baseUrl?: string | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly executionRemote?: PylonFleetRunExecutionHttpPort | undefined
  readonly maxActiveRuns?: number | undefined
  /** Explicit managed-cloud opener. Absence blocks; it never falls back local. */
  readonly openManagedCloudExecutor?: PylonFleetRunExecutorOpener | undefined
  readonly openExecutor?: PylonFleetRunExecutorOpener | undefined
  readonly openRuntime?: typeof openPylonFleetRunRuntime | undefined
  readonly pylonRef: string
  readonly summary: BootstrapSummary
}

export type PylonNodeFleetRunActivationService = {
  readonly arm: (runRef: string) => Promise<PylonFleetRunActivationProjection>
  readonly close: () => Promise<void>
  readonly disarm: (runRef: string) => Promise<PylonFleetRunActivationProjection>
  readonly status: (runRef?: string) => Promise<PylonFleetRunActivationStatus>
}

const validateRef = (value: unknown, field: "pylon_ref" | "run_ref"): string => {
  if (typeof value !== "string" || !PUBLIC_REF_PATTERN.test(value)) {
    throw new ControlCommandValidationError(
      field === "run_ref" ? "fleet_run_ref_invalid" : "fleet_run_pylon_ref_invalid",
      `fleet run ${field.replace("_", " ")} must be a bounded public-safe ref`,
    )
  }
  return value
}

const activeLimit = (value: number | undefined): number => {
  if (value === undefined || value === PYLON_NODE_MAX_ACTIVE_FLEET_RUNS) {
    return PYLON_NODE_MAX_ACTIVE_FLEET_RUNS
  }
  throw new Error(
    "node FleetRun activation supports exactly one active run; parallelism belongs inside that run",
  )
}

const isRunnableState = (state: string): boolean =>
  state === "running"

const isTerminalState = (state: string): boolean =>
  state === "completed" || state === "stopped"

const transportConfigured = (baseUrl: string | undefined): boolean => {
  try {
    const parsed = new URL(baseUrl ?? "")
    return parsed.protocol === "http:" || parsed.protocol === "https:"
  } catch {
    return false
  }
}

const offlineExecutionRemote: PylonFleetRunExecutionHttpPort = {
  append: async () => {
    throw new Error("Pylon FleetRun execution projection transport is unavailable")
  },
}

/**
 * Open the node-owned activation authority and resume only explicitly armed
 * runs. All mutation is serialized through this service. The durable row is
 * the permission; an executor handle is merely the current process-local
 * realization of that permission.
 */
export async function openPylonNodeFleetRunActivationService(
  input: OpenPylonNodeFleetRunActivationServiceInput,
): Promise<PylonNodeFleetRunActivationService> {
  const pylonRef = validateRef(input.pylonRef, "pylon_ref")
  const maxActiveRuns = activeLimit(input.maxActiveRuns)
  const authority = await (input.openRuntime ?? openPylonFleetRunRuntime)({
    bootstrap: input.summary,
  })
  const openOwnerLocalExecutor =
    input.openExecutor ?? openPylonOwnedStandingFleetRunExecutor
  const handles = new Map<string, PylonFleetRunExecutorHandle>()
  const opening = new Set<string>()
  const blocked = new Map<string, PylonFleetRunActivationReason>()
  // A terminal store row is not enough authority to retire durable restart
  // intent: reconcile-stopped/completed runs can be revived. Only a terminal
  // observation emitted by the owned supervisor enters this set.
  const terminalLifecycle = new Set<string>()
  // Keep a monotonic signal generation per run so a lifecycle observation
  // arriving while that run's cleanup is already queued cannot be lost when
  // the older cleanup finishes. A generation advances only for a new
  // supervisor observation, which also prevents a failed close from retrying
  // itself forever without a new signal.
  const terminalLifecycleGeneration = new Map<string, number>()
  const scheduledTerminalReaps = new Set<string>()
  let nextTerminalLifecycleGeneration = 0
  let closed = false
  let tail = Promise.resolve()

  const serialize = <T>(operation: () => Promise<T>): Promise<T> => {
    const next = tail.then(operation, operation)
    tail = next.then(() => undefined, () => undefined)
    return next
  }

  const commandOperation = async <T>(operation: () => Promise<T>): Promise<T> => {
    try {
      return await operation()
    } catch (error) {
      if (controlCommandValidationReason(error) !== null) throw error
      if (error instanceof ControlCommandOperationalError) throw error
      throw new ControlCommandOperationalError(
        "fleet_run_activation_authority_unavailable",
        "fleet run activation authority unavailable",
      )
    }
  }

  const projection = (runRef: string): PylonFleetRunActivationProjection => {
    const armed = authority.store.getFleetRunActivation(pylonRef, runRef)?.armed === true
    const active = handles.has(runRef) || opening.has(runRef)
    const reason = blocked.get(runRef) ?? null
    const state = armed
      ? active
        ? "active"
        : "armed_blocked"
      : active
        ? "disarmed_cleanup_blocked"
        : "disarmed"
    return {
      schema: PYLON_FLEET_RUN_ACTIVATION_SCHEMA,
      pylonRef,
      runRef,
      armed,
      active,
      state,
      reason,
      retryable:
        reason === "active_limit_reached" ||
        reason === "executor_open_failed" ||
        reason === "managed_cloud_executor_unconfigured",
    }
  }

  const reapTerminalLifecycle = async (): Promise<boolean> => {
    let freedActiveSlot = false
    for (const runRef of [...terminalLifecycle].sort()) {
      const observedGeneration = terminalLifecycleGeneration.get(runRef)
      const clearObservedLifecycle = (): void => {
        if (terminalLifecycleGeneration.get(runRef) === observedGeneration) {
          terminalLifecycle.delete(runRef)
        }
      }
      const run = authority.store.getFleetRun(runRef)
      // A newer live state supersedes an already-queued terminal observation.
      // Never discard restart intent for running, paused, or draining work.
      if (run === null || !isTerminalState(run.state)) {
        clearObservedLifecycle()
        continue
      }

      const activation = authority.store.getFleetRunActivation(pylonRef, runRef)
      if (activation?.armed === true) {
        // Remove durable restart intent before process-local cleanup. Even if
        // close fails, this terminal run cannot resurrect after a restart.
        authority.store.setFleetRunActivation({ pylonRef, runRef, armed: false })
      }

      const handle = handles.get(runRef)
      if (handle === undefined) {
        if (!opening.has(runRef)) {
          blocked.delete(runRef)
          clearObservedLifecycle()
        }
        continue
      }
      try {
        await handle.close()
        // A failed close remains a counted process-local handle. Delete only
        // after the complete executor/reporter close succeeds.
        handles.delete(runRef)
        blocked.delete(runRef)
        clearObservedLifecycle()
        freedActiveSlot = true
      } catch {
        blocked.set(runRef, "executor_close_failed")
      }
    }
    return freedActiveSlot
  }

  const tryActivate = async (
    runRef: string,
    reapBeforeAdmission = true,
  ): Promise<void> => {
    if (reapBeforeAdmission) await reapTerminalLifecycle()
    if (handles.has(runRef) || opening.has(runRef)) {
      if (blocked.get(runRef) !== "executor_close_failed") blocked.delete(runRef)
      return
    }
    const activation = authority.store.getFleetRunActivation(pylonRef, runRef)
    if (activation?.armed !== true) return
    const run = authority.store.getFleetRun(runRef)
    if (run === null) {
      blocked.set(runRef, "unknown_stored_run")
      return
    }
    if (!isRunnableState(run.state)) {
      blocked.set(runRef, "run_not_runnable")
      return
    }
    if (!transportConfigured(input.baseUrl)) {
      blocked.set(runRef, "transport_not_configured")
      return
    }
    const openExecutor =
      run.authorityBinding?.targetPreference === "managed_cloud"
        ? input.openManagedCloudExecutor
        : openOwnerLocalExecutor
    if (openExecutor === undefined) {
      // A managed-cloud authority is never evidence for owner-local dispatch.
      // Keep restart intent armed so adding the private grant/capacity
      // composition can retry this exact run without accepting another claim.
      blocked.set(runRef, "managed_cloud_executor_unconfigured")
      return
    }
    if (handles.size + opening.size >= maxActiveRuns) {
      blocked.set(runRef, "active_limit_reached")
      return
    }

    opening.add(runRef)
    blocked.delete(runRef)
    let reporter: PylonFleetRunExecutionReporter | null = null
    try {
      reporter = run.authorityBinding?.phase !== "accepted"
        ? null
        : openPylonFleetRunExecutionReporter({
            store: authority.store,
            pylonRef,
            runRef,
            remote: input.executionRemote ?? offlineExecutionRemote,
          })
      const executionReporter = reporter
      await executionReporter?.flush().catch(() => null)
      const handle = await openExecutor({
        summary: input.summary,
        pylonRef,
        runRef,
        baseUrl: input.baseUrl ?? "",
        ...(input.agentToken === undefined ? {} : { agentToken: input.agentToken }),
        ...(input.env === undefined ? {} : { env: input.env }),
        onLifecycle: async event => {
          try {
            if (executionReporter !== null) {
              for (const projected of projectFleetRunSupervisorObservation({
                event,
                store: authority.store,
              })) {
                await executionReporter.record(projected)
              }
            }
          } finally {
            if (
              event.runRef === runRef &&
              (event.kind === "completed" || event.kind === "terminal")
            ) scheduleTerminalReap(runRef)
          }
        },
      })
      handles.set(runRef, executionReporter === null
        ? handle
        : {
            close: async () => {
              await handle.close()
              await executionReporter.close()
            },
          })
    } catch {
      await reporter?.close().catch(() => null)
      // The arm is intentional durable state. Keep it armed and expose only a
      // fixed retryable blocker; opener/provider/local-path text is private.
      blocked.set(runRef, "executor_open_failed")
    } finally {
      opening.delete(runRef)
    }
  }

  const reconcileArmed = async (reapBeforeAdmission = true): Promise<void> => {
    if (reapBeforeAdmission) await reapTerminalLifecycle()
    for (const activation of authority.store.listFleetRunActivations(pylonRef)) {
      if (!activation.armed) continue
      if (!PUBLIC_REF_PATTERN.test(activation.runRef)) {
        blocked.set(activation.runRef, "unsafe_stored_ref")
        continue
      }
      await tryActivate(activation.runRef, false)
    }
  }

  const reapTerminalLifecycleThenFill = async (): Promise<void> => {
    if (await reapTerminalLifecycle()) await reconcileArmed(false)
  }

  function scheduleTerminalReap(runRef: string): void {
    terminalLifecycle.add(runRef)
    terminalLifecycleGeneration.set(runRef, ++nextTerminalLifecycleGeneration)
    queueTerminalReap(runRef)
  }

  function queueTerminalReap(runRef: string): void {
    if (scheduledTerminalReaps.has(runRef)) return
    scheduledTerminalReaps.add(runRef)
    // The lifecycle callback may run while openExecutor is itself awaited by
    // the serialized arm operation. Queue cleanup but never await it here, or
    // a terminal-during-open event would deadlock its own handle creation.
    void serialize(async () => {
      const scheduledGeneration = terminalLifecycleGeneration.get(runRef)
      try {
        if (!closed) await reapTerminalLifecycleThenFill()
      } finally {
        scheduledTerminalReaps.delete(runRef)
        if (
          !closed &&
          terminalLifecycle.has(runRef) &&
          terminalLifecycleGeneration.get(runRef) !== scheduledGeneration
        ) queueTerminalReap(runRef)
      }
    }).catch(() => undefined)
  }

  const assertOpen = (): void => {
    if (closed) throw new Error("fleet run activation service is closed")
  }

  const service: PylonNodeFleetRunActivationService = {
    arm: (unsafeRunRef) => serialize(() => commandOperation(async () => {
      assertOpen()
      const runRef = validateRef(unsafeRunRef, "run_ref")
      if (authority.store.getFleetRun(runRef) === null) {
        throw new ControlCommandValidationError(
          "fleet_run_unknown",
          "fleet run must exist in the canonical Pylon orchestration store before it can be armed",
        )
      }
      authority.store.setFleetRunActivation({ pylonRef, runRef, armed: true })
      // A repeated arm is also the explicit retry operation for a previously
      // blocked opener. It remains idempotent while a handle is active.
      await tryActivate(runRef)
      return projection(runRef)
    })),
    disarm: (unsafeRunRef) => serialize(() => commandOperation(async () => {
      assertOpen()
      const runRef = validateRef(unsafeRunRef, "run_ref")
      // Durable intent is removed before process-local cleanup. A failed close
      // can never cause this run to be resurrected after node restart.
      authority.store.setFleetRunActivation({ pylonRef, runRef, armed: false })
      blocked.delete(runRef)
      const handle = handles.get(runRef)
      if (handle !== undefined) {
        try {
          await handle.close()
          handles.delete(runRef)
        } catch {
          blocked.set(runRef, "executor_close_failed")
        }
      }
      await reconcileArmed()
      return projection(runRef)
    })),
    status: (unsafeRunRef) => serialize(() => commandOperation(async () => {
      assertOpen()
      await reapTerminalLifecycleThenFill()
      const requested = unsafeRunRef === undefined
        ? undefined
        : validateRef(unsafeRunRef, "run_ref")
      const stored = authority.store.listFleetRunActivations(pylonRef)
      const invalidStoredRows = stored.filter(
        (activation) => !PUBLIC_REF_PATTERN.test(activation.runRef),
      ).length
      const refs = new Set(
        stored
          .map((activation) => activation.runRef)
          .filter((runRef) => PUBLIC_REF_PATTERN.test(runRef)),
      )
      for (const runRef of handles.keys()) refs.add(runRef)
      const runs = [...refs]
        .filter((runRef) => requested === undefined || runRef === requested)
        .sort()
        .map(projection)
      return {
        schema: PYLON_FLEET_RUN_ACTIVATION_SCHEMA,
        pylonRef,
        maxActiveRuns,
        activeRuns: handles.size + opening.size,
        invalidStoredRows,
        blockerRefs: invalidStoredRows === 0
          ? []
          : ["blocker.pylon.fleet_run_activation.invalid_stored_ref"],
        runs,
      }
    })),
    close: () => serialize(async () => {
      if (closed) return
      closed = true
      // Node shutdown closes process handles but deliberately does not alter
      // armed rows. Those exact explicit intents are resumed on next startup.
      for (const [runRef, handle] of handles) {
        try {
          await handle.close()
          handles.delete(runRef)
        } catch {
          // Shutdown is best-effort; the process boundary ends the handle. No
          // private executor text is logged or persisted here.
        }
      }
      await authority.close()
    }),
  }

  try {
    await serialize(reconcileArmed)
    return service
  } catch (error) {
    await authority.close()
    throw error
  }
}
