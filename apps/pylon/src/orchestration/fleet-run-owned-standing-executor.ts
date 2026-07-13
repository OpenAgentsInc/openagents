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
  type PylonOwnedGrokClaimedWorkPort,
  type CreatePylonOwnedFleetRunSupervisorRunnerInput,
} from "./fleet-run-owned-runner.js"
import {
  createPylonOwnedGrokClaimedWorkPort,
  type CreatePylonOwnedGrokClaimedWorkPortInput,
} from "./fleet-run-owned-grok-runner.js"
import {
  openPylonStandingFleetRunExecutor,
  type PylonStandingFleetRunExecutor,
} from "./fleet-run-standing-executor.js"
import {
  makePylonFleetRunSteeringHttpTransport,
  openPylonFleetRunSteeringConsumer,
} from "./fleet-run-steering-consumer.js"
import {
  openPylonFleetRunSteeringFollowUpDispatcher,
  type PylonFleetRunAttemptControl,
  type PylonFleetRunSteeringFollowUpCompletionSink,
} from "./fleet-run-steering-follow-up-dispatcher.js"
import type {
  FleetRunSupervisorClock,
  FleetRunSupervisorObservedEvent,
} from "./fleet-run-supervisor.js"
import {
  createPylonManagedCloudFleetRunClaimedWorkPort,
  type CreatePylonManagedCloudFleetRunClaimedWorkPortInput,
} from "./fleet-run-managed-cloud-runner.js"
import type { FleetRunSupervisorCapacity } from "./fleet-run-supervisor.js"

export type PylonOwnedStandingFleetRunCapacityOptions = Omit<
  CreatePylonOwnedFleetRunSupervisorCapacityInput,
  | "defaultHomes"
  | "env"
  | "grokExecutionAvailable"
  | "loadRegistry"
  | "store"
  | "summary"
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
  | "grok"
>

export type PylonOwnedStandingFleetRunGrokOptions = Omit<
  CreatePylonOwnedGrokClaimedWorkPortInput,
  "env" | "loadRegistry" | "now" | "store" | "summary"
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
    readonly grok?: string | undefined
  } | undefined
  readonly capacity?: PylonOwnedStandingFleetRunCapacityOptions | undefined
  readonly runner?: PylonOwnedStandingFleetRunRunnerOptions | undefined
  /** `false` is a test/diagnostic fail-closed mode; production composes the exact adapter. */
  readonly grok?: PylonOwnedStandingFleetRunGrokOptions | PylonOwnedGrokClaimedWorkPort | false | undefined
  readonly liveness?: PylonOwnedStandingFleetRunLivenessOptions | undefined
  readonly planner?: Omit<CreatePylonDurableFleetRunPlannerInput, "store"> | undefined
  /**
   * Optional broker-authorized Agent Computer lane. When present, the same
   * supervisor and claim registry expose both target classes and dispatch each
   * unit only through its selected target. Absence leaves managed units denied.
   */
  readonly managedCloud?: Readonly<{
    capacity: FleetRunSupervisorCapacity
    adapter: Omit<CreatePylonManagedCloudFleetRunClaimedWorkPortInput, "summary">
  }> | undefined
}

export type OpenPylonOwnedStandingFleetRunExecutorInput = {
  readonly agentToken?: string | undefined
  readonly baseUrl: string
  readonly clock?: Partial<FleetRunSupervisorClock> | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly fetch?: typeof fetch | undefined
  readonly now?: (() => Date) | undefined
  readonly onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  /** Body-free terminal follow-up delivery; owner-private steer bodies are never passed. */
  readonly onSteeringFollowUpCompletion?: PylonFleetRunSteeringFollowUpCompletionSink | undefined
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
  let steeringControl: PylonFleetRunAttemptControl | null = null
  const steeringTransport = input.agentToken === undefined
    ? null
    : makePylonFleetRunSteeringHttpTransport({
        agentToken: input.agentToken,
        baseUrl: input.baseUrl,
        ...(input.fetch === undefined ? {} : { fetchImpl: input.fetch }),
      })

  return await openPylonStandingFleetRunExecutor({
    bootstrap: summary,
    ...(input.clock === undefined ? {} : { clock: input.clock }),
    ...(input.now === undefined ? {} : { now: input.now }),
    ...(input.onLifecycle === undefined ? {} : { onLifecycle: input.onLifecycle }),
    pylonRef: input.pylonRef,
    runRef: input.runRef,
    ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
    ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
    ...(steeringTransport === null
      ? {}
      : {
          steeringConsumerFactory: ({ store, pylonRef, runRef, claimRef }) =>
            openPylonFleetRunSteeringConsumer({
              store,
              pylonRef,
              runRef,
              claimRef,
              transport: steeringTransport,
              ...(input.now === undefined ? {} : { now: input.now }),
            }),
          steeringFollowUpDispatcherFactory: ({ store, pylonRef, runRef, claimRef }) => {
            if (steeringControl === null) {
              throw new Error("Pylon FleetRun steering control is unavailable")
            }
            return openPylonFleetRunSteeringFollowUpDispatcher({
              store,
              control: steeringControl,
              pylonRef,
              runRef,
              claimRef,
              onCompletion:
                input.onSteeringFollowUpCompletion ??
                (async (completion) => {
                  await steeringTransport.postCompletions({
                    pylonRef,
                    runRef,
                    claimRef,
                    completions: [
                      {
                        seq: completion.seq,
                        intentId: completion.intentId,
                        state: completion.state,
                        completionRef: completion.completionRef,
                        completedAt: completion.completedAt,
                      },
                    ],
                  })
                }),
              ...(input.now === undefined ? {} : { now: input.now }),
            })
          },
        }),
    adapterFactory: ({ store }) => {
      const grok = options.grok === false
        ? undefined
        : options.grok !== undefined && "dispatch" in options.grok && "reconcile" in options.grok
          ? options.grok
          : createPylonOwnedGrokClaimedWorkPort({
              ...(options.grok ?? {}),
              summary,
              env,
              store,
              ...(input.now === undefined ? {} : { now: input.now }),
              ...(options.loadRegistry === undefined ? {} : { loadRegistry: options.loadRegistry }),
            })
      const ownerCapacity = createPylonOwnedFleetRunSupervisorCapacity({
          ...options.capacity,
          store,
          summary,
          env,
          grokExecutionAvailable: grok !== undefined,
          ...(options.defaultHomes === undefined ? {} : { defaultHomes: options.defaultHomes }),
          ...(options.loadRegistry === undefined ? {} : { loadRegistry: options.loadRegistry }),
        })
      const managedClaimedWork = options.managedCloud === undefined
        ? null
        : createPylonManagedCloudFleetRunClaimedWorkPort({
            ...options.managedCloud.adapter,
            summary,
          })
      const ownerRunner = createPylonOwnedFleetRunSupervisorRunner({
        ...options.runner,
        summary,
        pylonRef: input.pylonRef,
        baseUrl: input.baseUrl,
        ...(input.agentToken === undefined ? {} : { agentToken: input.agentToken }),
        ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
        ...(input.now === undefined ? {} : { now: input.now }),
        ...(options.defaultHomes === undefined ? {} : { defaultHomes: options.defaultHomes }),
        ...(options.loadRegistry === undefined ? {} : { loadRegistry: options.loadRegistry }),
        ...(grok === undefined ? {} : { grok }),
      })
      steeringControl = ownerRunner.steeringControl
      return {
        capacity: options.managedCloud === undefined
          ? ownerCapacity
          : {
              accounts: async capacityInput => [
                ...(await ownerCapacity.accounts(capacityInput)).map(account => ({
                  ...account,
                  executionTarget: "owner_local" as const,
                  quotaAvailable: true,
                  acceptedDataPostures: ["owner_private", "broker_safe"] as const,
                  repositoryAccess: true,
                })),
                ...(await options.managedCloud!.capacity.accounts(capacityInput)).map(account => ({
                  ...account,
                  executionTarget: "managed_cloud" as const,
                  acceptedDataPostures: ["broker_safe"] as const,
                  repositoryAccess: true,
                  managedIsolation: true,
                })),
              ],
            },
        livenessProbe: (() => {
          const assignmentProbe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
            ...options.liveness,
            assignmentStatePath: statePaths.assignmentState,
            ...(input.now === undefined ? {} : { now: input.now }),
          })
          return async (evidence) =>
            evidence.runnerKind === "grok_cli" && grok !== undefined && evidence.assignmentRef !== null
              ? await grok.probeLiveness(evidence.assignmentRef)
              : await assignmentProbe(evidence)
        })(),
        planner: createPylonDurableFleetRunPlanner({
          ...options.planner,
          store,
        }),
        runner: {
          dispatch: dispatch => {
            if (dispatch.executionTarget === "owner_local") {
              return ownerRunner.dispatch(dispatch)
            }
            if (managedClaimedWork === null) {
              throw new Error("managed-cloud per-unit executor is not configured")
            }
            const durableClaim = store.getWorkClaim(dispatch.claim.claimRef)
            return managedClaimedWork.dispatch({
              targetPreference: "managed_cloud",
              dispatch: {
                ...dispatch,
                ...(durableClaim === null ? {} : { claim: durableClaim }),
              },
            })
          },
          ...(ownerRunner.reconcile === undefined ? {} : { reconcile: ownerRunner.reconcile }),
        },
      }
    },
  })
}
