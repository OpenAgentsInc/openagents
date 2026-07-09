import {
  recoverInterruptedFleetRunAssignments,
  type FleetRunInterruptedRecoveryReceipt,
  type FleetRunOwnerLocalLivenessProbe,
} from "./fleet-run-recovery.js"
import {
  openPylonFleetRunRuntime,
  type OpenPylonFleetRunRuntimeInput,
  type PylonFleetRunBootstrap,
  type PylonFleetRunRuntime,
} from "./fleet-run-runtime.js"
import type { PylonFleetRunSnapshot } from "./fleet-run-manager.js"
import type {
  FleetRunSupervisorCapacity,
  FleetRunSupervisorClock,
  FleetRunSupervisorObservedEvent,
  FleetRunSupervisorPlanner,
  FleetRunSupervisorRunner,
} from "./fleet-run-supervisor.js"

export type PylonStandingFleetRunAdapters = {
  readonly capacity: FleetRunSupervisorCapacity
  readonly livenessProbe: FleetRunOwnerLocalLivenessProbe
  readonly planner: FleetRunSupervisorPlanner
  readonly runner: FleetRunSupervisorRunner
}

export type OpenPylonStandingFleetRunExecutorInput = PylonStandingFleetRunAdapters & {
  readonly bootstrap?: PylonFleetRunBootstrap | undefined
  readonly clock?: Partial<FleetRunSupervisorClock> | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly now?: (() => Date) | undefined
  readonly onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  readonly pylonRef: string
  readonly runRef: string
  readonly startImmediately?: boolean | undefined
  readonly tickIntervalMs?: number | undefined
}

/**
 * Owner-local standing executor handle. `runtime.databasePath` remains a local
 * diagnostic and must never be serialized into a Sarah/Khala projection.
 */
export type PylonStandingFleetRunExecutor = {
  readonly close: () => Promise<void>
  readonly recovery: FleetRunInterruptedRecoveryReceipt
  readonly runtime: PylonFleetRunRuntime
  readonly snapshot: PylonFleetRunSnapshot
}

/**
 * Open one durable FleetRun, stale-close interrupted local work, then resume
 * the Pylon-owned refill loop. The order is deliberate: a replacement claim
 * is impossible until recovery has released the prior durable claim.
 *
 * Hosts inject only the Pylon adapter vocabulary. No desktop RPC, Electrobun,
 * process ID, worktree path, command, raw output, or credential type crosses
 * this seam.
 */
export async function openPylonStandingFleetRunExecutor(
  input: OpenPylonStandingFleetRunExecutorInput,
): Promise<PylonStandingFleetRunExecutor> {
  const runtimeInput: OpenPylonFleetRunRuntimeInput = {
    ...(input.bootstrap === undefined ? {} : { bootstrap: input.bootstrap }),
    ...(input.env === undefined ? {} : { env: input.env }),
    ...(input.now === undefined ? {} : { now: input.now }),
  }
  const runtime = await openPylonFleetRunRuntime(runtimeInput)
  try {
    const now = input.now?.() ?? new Date()
    const recovery = await recoverInterruptedFleetRunAssignments({
      store: runtime.store,
      probe: input.livenessProbe,
      runRef: input.runRef,
      now,
    })
    const snapshot = await runtime.manager.resume({
      capacity: input.capacity,
      planner: input.planner,
      pylonRef: input.pylonRef,
      runRef: input.runRef,
      runner: input.runner,
      ...(input.clock === undefined ? {} : { clock: input.clock }),
      ...(input.onLifecycle === undefined ? {} : { onLifecycle: input.onLifecycle }),
      ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
      ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
    })
    return {
      close: runtime.close,
      recovery,
      runtime,
      snapshot,
    }
  } catch (error) {
    await runtime.close()
    throw error
  }
}
