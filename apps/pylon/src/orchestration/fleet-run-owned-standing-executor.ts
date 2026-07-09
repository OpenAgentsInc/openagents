import type { BootstrapSummary } from "../bootstrap.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../bootstrap.js"
import type { PylonAccountRegistryEntry } from "../account-registry.js"
import { resolveStatePaths } from "../state.js"
import {
  createPylonAssignmentFleetRunOwnerLocalLivenessProbe,
  type CreatePylonAssignmentFleetRunOwnerLocalLivenessProbeInput,
} from "./fleet-run-assignment-liveness.js"
import {
  createPylonDurableFleetRunPlanner,
  type CreatePylonDurableFleetRunPlannerInput,
} from "./fleet-run-durable-planner.js"
import {
  createPylonOwnedFleetRunSupervisorCapacity,
  type CreatePylonOwnedFleetRunSupervisorCapacityInput,
} from "./fleet-run-owned-capacity.js"
import {
  createPylonOwnedFleetRunSupervisorRunner,
  type CreatePylonOwnedFleetRunSupervisorRunnerInput,
} from "./fleet-run-owned-runner.js"
import {
  openPylonStandingFleetRunExecutor,
  type PylonStandingFleetRunExecutor,
} from "./fleet-run-standing-executor.js"
import type {
  FleetRunSupervisorClock,
  FleetRunSupervisorObservedEvent,
} from "./fleet-run-supervisor.js"

export type PylonOwnedStandingFleetRunCapacityOptions = Omit<
  CreatePylonOwnedFleetRunSupervisorCapacityInput,
  "defaultHomes" | "env" | "loadRegistry" | "store" | "summary"
>

export type PylonOwnedStandingFleetRunRunnerOptions = Omit<
  CreatePylonOwnedFleetRunSupervisorRunnerInput,
  | "agentToken"
  | "baseUrl"
  | "defaultHomes"
  | "fetch"
  | "loadRegistry"
  | "now"
  | "pylonRef"
  | "summary"
>

export type PylonOwnedStandingFleetRunLivenessOptions = Omit<
  CreatePylonAssignmentFleetRunOwnerLocalLivenessProbeInput,
  "assignmentStatePath" | "now"
>

export type PylonOwnedStandingFleetRunAdapterOptions = {
  /** One strict registry source is shared by capacity and dispatch custody. */
  readonly loadRegistry?: (() => Promise<readonly PylonAccountRegistryEntry[]>) | undefined
  readonly defaultHomes?: {
    readonly claudeAgent: string
    readonly codex: string
  } | undefined
  readonly capacity?: PylonOwnedStandingFleetRunCapacityOptions | undefined
  readonly runner?: PylonOwnedStandingFleetRunRunnerOptions | undefined
  readonly liveness?: PylonOwnedStandingFleetRunLivenessOptions | undefined
  readonly planner?: Omit<CreatePylonDurableFleetRunPlannerInput, "store"> | undefined
}

export type OpenPylonOwnedStandingFleetRunExecutorInput = {
  readonly agentToken?: string | undefined
  readonly baseUrl: string
  readonly clock?: Partial<FleetRunSupervisorClock> | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly fetch?: typeof fetch | undefined
  readonly now?: (() => Date) | undefined
  readonly onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  readonly options?: PylonOwnedStandingFleetRunAdapterOptions | undefined
  readonly pylonRef: string
  readonly runRef: string
  /** Reuse the daemon's full summary; otherwise it is resolved once from env. */
  readonly summary?: BootstrapSummary | undefined
  readonly startImmediately?: boolean | undefined
  readonly tickIntervalMs?: number | undefined
}

/**
 * Canonical owner-local FC-2 composition.
 *
 * One call opens one Pylon-home orchestration runtime. Only after that open do
 * all store-dependent adapters get constructed against `context.store`:
 * durable planning, named-account capacity, exact assignment execution,
 * assignment-process liveness, interrupted recovery, and standing refill.
 */
export async function openPylonOwnedStandingFleetRunExecutor(
  input: OpenPylonOwnedStandingFleetRunExecutorInput,
): Promise<PylonStandingFleetRunExecutor> {
  const env = input.env ?? process.env
  const summary = input.summary ?? createBootstrapSummary(
    parseBootstrapArgs(["--json"]),
    env,
  )
  const options = input.options ?? {}
  const statePaths = resolveStatePaths(summary.paths)

  return await openPylonStandingFleetRunExecutor({
    bootstrap: summary,
    ...(input.clock === undefined ? {} : { clock: input.clock }),
    ...(input.now === undefined ? {} : { now: input.now }),
    ...(input.onLifecycle === undefined ? {} : { onLifecycle: input.onLifecycle }),
    pylonRef: input.pylonRef,
    runRef: input.runRef,
    ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
    ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
    adapterFactory: ({ store }) => ({
      capacity: createPylonOwnedFleetRunSupervisorCapacity({
        ...options.capacity,
        store,
        summary,
        env,
        ...(options.defaultHomes === undefined ? {} : { defaultHomes: options.defaultHomes }),
        ...(options.loadRegistry === undefined ? {} : { loadRegistry: options.loadRegistry }),
      }),
      livenessProbe: createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
        ...options.liveness,
        assignmentStatePath: statePaths.assignmentState,
        ...(input.now === undefined ? {} : { now: input.now }),
      }),
      planner: createPylonDurableFleetRunPlanner({
        ...options.planner,
        store,
      }),
      runner: createPylonOwnedFleetRunSupervisorRunner({
        ...options.runner,
        summary,
        pylonRef: input.pylonRef,
        baseUrl: input.baseUrl,
        ...(input.agentToken === undefined ? {} : { agentToken: input.agentToken }),
        ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
        ...(input.now === undefined ? {} : { now: input.now }),
        ...(options.defaultHomes === undefined ? {} : { defaultHomes: options.defaultHomes }),
        ...(options.loadRegistry === undefined ? {} : { loadRegistry: options.loadRegistry }),
      }),
    }),
  })
}
