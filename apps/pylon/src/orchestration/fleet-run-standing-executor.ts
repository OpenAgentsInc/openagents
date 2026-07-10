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
import type {
  PylonFleetRunSteeringConsumer,
  PylonFleetRunSteeringConsumerFactory,
} from "./fleet-run-steering-consumer.js"
import type {
  PylonFleetRunSteeringFollowUpDispatcher,
  PylonFleetRunSteeringFollowUpDispatcherFactory,
} from "./fleet-run-steering-follow-up-dispatcher.js"

export type PylonStandingFleetRunAdapters = {
  readonly capacity: FleetRunSupervisorCapacity
  readonly livenessProbe: FleetRunOwnerLocalLivenessProbe
  readonly planner: FleetRunSupervisorPlanner
  readonly runner: FleetRunSupervisorRunner
}

export type PylonStandingFleetRunAdapterFactoryContext = {
  readonly runtime: PylonFleetRunRuntime
  readonly store: PylonFleetRunRuntime["store"]
}

export type PylonStandingFleetRunAdapterFactory = (
  context: PylonStandingFleetRunAdapterFactoryContext,
) => PylonStandingFleetRunAdapters | Promise<PylonStandingFleetRunAdapters>

type OpenPylonStandingFleetRunExecutorCommonInput = {
  readonly bootstrap?: PylonFleetRunBootstrap | undefined
  readonly clock?: Partial<FleetRunSupervisorClock> | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly now?: (() => Date) | undefined
  readonly onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  readonly pylonRef: string
  readonly runRef: string
  readonly startImmediately?: boolean | undefined
  /** Optional accepted-claim steering delivery composition seam. */
  readonly steeringConsumerFactory?: PylonFleetRunSteeringConsumerFactory | undefined
  /** Optional restart-safe executor for locally queued steering follow-ups. */
  readonly steeringFollowUpDispatcherFactory?: PylonFleetRunSteeringFollowUpDispatcherFactory | undefined
  readonly tickIntervalMs?: number | undefined
}

type OpenPylonStandingFleetRunExecutorDirectInput = PylonStandingFleetRunAdapters & {
  readonly adapterFactory?: never
}

type OpenPylonStandingFleetRunExecutorFactoryInput = {
  readonly adapterFactory: PylonStandingFleetRunAdapterFactory
  readonly capacity?: never
  readonly livenessProbe?: never
  readonly planner?: never
  readonly runner?: never
}

export type OpenPylonStandingFleetRunExecutorInput =
  OpenPylonStandingFleetRunExecutorCommonInput &
  (OpenPylonStandingFleetRunExecutorDirectInput | OpenPylonStandingFleetRunExecutorFactoryInput)

export type PylonStandingFleetRunConstructionFailure =
  | "adapter_factory_failed"
  | "invalid_adapter_config"
  | "invalid_adapter_factory_result"
  | "steering_consumer_failed"
  | "steering_follow_up_dispatcher_failed"

export class PylonStandingFleetRunConstructionError extends Error {
  readonly failure: PylonStandingFleetRunConstructionFailure
  readonly blockerRefs: readonly string[]

  constructor(failure: PylonStandingFleetRunConstructionFailure) {
    super(`Pylon standing FleetRun construction failed: ${failure}`)
    this.name = "PylonStandingFleetRunConstructionError"
    this.failure = failure
    this.blockerRefs = [`blocker.pylon.fleet_run.standing_${failure}`]
  }
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

const directAdapterKeys = ["capacity", "livenessProbe", "planner", "runner"] as const

const assertAdapterConstructionInput = (
  input: OpenPylonStandingFleetRunExecutorInput,
): "direct" | "factory" => {
  const record = input as unknown as Record<string, unknown>
  const factoryProvided = record.adapterFactory !== undefined
  const hasFactory = typeof record.adapterFactory === "function"
  const directValues = directAdapterKeys.map(key => record[key])
  const directCount = directValues.filter(value => value !== undefined).length
  if (hasFactory && directCount === 0) return "factory"
  if (!factoryProvided && directCount === directAdapterKeys.length && adaptersAreValid(record)) return "direct"
  throw new PylonStandingFleetRunConstructionError("invalid_adapter_config")
}

const adaptersAreValid = (value: unknown): value is PylonStandingFleetRunAdapters => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false
  const adapters = value as Record<string, unknown>
  const capacity = adapters.capacity
  const planner = adapters.planner
  const runner = adapters.runner
  return (
    typeof adapters.livenessProbe === "function" &&
    capacity !== null && typeof capacity === "object" &&
    typeof (capacity as { accounts?: unknown }).accounts === "function" &&
    planner !== null && typeof planner === "object" &&
    typeof (planner as { plan?: unknown }).plan === "function" &&
    runner !== null && typeof runner === "object" &&
    typeof (runner as { dispatch?: unknown }).dispatch === "function"
  )
}

const directAdapters = (
  input: OpenPylonStandingFleetRunExecutorInput,
): PylonStandingFleetRunAdapters => {
  const direct = input as OpenPylonStandingFleetRunExecutorCommonInput &
    OpenPylonStandingFleetRunExecutorDirectInput
  return {
    capacity: direct.capacity,
    livenessProbe: direct.livenessProbe,
    planner: direct.planner,
    runner: direct.runner,
  }
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
  const adapterConstruction = assertAdapterConstructionInput(input)
  const runtimeInput: OpenPylonFleetRunRuntimeInput = {
    ...(input.bootstrap === undefined ? {} : { bootstrap: input.bootstrap }),
    ...(input.env === undefined ? {} : { env: input.env }),
    ...(input.now === undefined ? {} : { now: input.now }),
  }
  const runtime = await openPylonFleetRunRuntime(runtimeInput)
  let steeringConsumer: PylonFleetRunSteeringConsumer | null = null
  let steeringFollowUpDispatcher: PylonFleetRunSteeringFollowUpDispatcher | null = null
  try {
    let adapters: PylonStandingFleetRunAdapters
    if (adapterConstruction === "factory") {
      try {
        const factory = (input as OpenPylonStandingFleetRunExecutorCommonInput &
          OpenPylonStandingFleetRunExecutorFactoryInput).adapterFactory
        const constructed = await factory({ runtime, store: runtime.store })
        if (!adaptersAreValid(constructed)) {
          throw new PylonStandingFleetRunConstructionError("invalid_adapter_factory_result")
        }
        adapters = constructed
      } catch (error) {
        if (error instanceof PylonStandingFleetRunConstructionError) throw error
        throw new PylonStandingFleetRunConstructionError("adapter_factory_failed")
      }
    } else {
      adapters = directAdapters(input)
    }
    const now = input.now?.() ?? new Date()
    const recovery = await recoverInterruptedFleetRunAssignments({
      store: runtime.store,
      probe: adapters.livenessProbe,
      runRef: input.runRef,
      now,
    })
    const snapshot = await runtime.manager.resume({
      capacity: adapters.capacity,
      planner: adapters.planner,
      pylonRef: input.pylonRef,
      runRef: input.runRef,
      runner: adapters.runner,
      ...(input.clock === undefined ? {} : { clock: input.clock }),
      ...(input.onLifecycle === undefined ? {} : { onLifecycle: input.onLifecycle }),
      ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
      ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
    })
    if (input.steeringConsumerFactory !== undefined) {
      const binding = runtime.store.getFleetRun(input.runRef)?.authorityBinding
      if (
        binding?.phase !== "accepted" ||
        binding.pylonRef !== input.pylonRef
      ) {
        throw new PylonStandingFleetRunConstructionError("steering_consumer_failed")
      }
      try {
        const candidate = await input.steeringConsumerFactory({
          store: runtime.store,
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: binding.claimRef,
        })
        if (
          candidate === null ||
          typeof candidate !== "object" ||
          typeof candidate.tick !== "function" ||
          typeof candidate.close !== "function"
        ) {
          throw new PylonStandingFleetRunConstructionError("steering_consumer_failed")
        }
        steeringConsumer = candidate
      } catch (error) {
        if (error instanceof PylonStandingFleetRunConstructionError) throw error
        throw new PylonStandingFleetRunConstructionError("steering_consumer_failed")
      }
    }
    if (input.steeringFollowUpDispatcherFactory !== undefined) {
      const binding = runtime.store.getFleetRun(input.runRef)?.authorityBinding
      if (binding?.phase !== "accepted" || binding.pylonRef !== input.pylonRef) {
        throw new PylonStandingFleetRunConstructionError("steering_follow_up_dispatcher_failed")
      }
      try {
        const candidate = await input.steeringFollowUpDispatcherFactory({
          store: runtime.store,
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: binding.claimRef,
        })
        if (
          candidate === null || typeof candidate !== "object" ||
          typeof candidate.tick !== "function" || typeof candidate.close !== "function"
        ) throw new PylonStandingFleetRunConstructionError("steering_follow_up_dispatcher_failed")
        steeringFollowUpDispatcher = candidate
      } catch (error) {
        if (error instanceof PylonStandingFleetRunConstructionError) throw error
        throw new PylonStandingFleetRunConstructionError("steering_follow_up_dispatcher_failed")
      }
    }
    return {
      close: async () => {
        try {
          await steeringFollowUpDispatcher?.close()
        } finally {
          try {
            await steeringConsumer?.close()
          } finally {
            await runtime.close()
          }
        }
      },
      recovery,
      runtime,
      snapshot,
    }
  } catch (error) {
    try {
      await (steeringFollowUpDispatcher as PylonFleetRunSteeringFollowUpDispatcher | null)?.close()
    } finally {
      try {
        await (steeringConsumer as PylonFleetRunSteeringConsumer | null)?.close()
      } finally {
        await runtime.close()
      }
    }
    throw error
  }
}
