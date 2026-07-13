import { createHash } from "node:crypto"

import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import { Effect } from "effect"

import type { BootstrapSummary } from "../bootstrap.js"
import type { CloudSessionGrantBinding } from "../openagents-cloud-provider.js"
import { assertPublicProjectionSafe } from "../state.js"
import type { ControlSessionExecutor, ControlSessionLane } from "../node/control-sessions.js"
import type {
  FleetRunSupervisorDispatchInput,
  FleetRunSupervisorDispatchResult,
  FleetRunSupervisorCapacity,
  FleetRunSupervisorClock,
  FleetRunSupervisorObservedEvent,
} from "./fleet-run-supervisor.js"
import type { FleetRunOwnerLocalLivenessProbe } from "./fleet-run-recovery.js"
import {
  createPylonDurableFleetRunPlanner,
  type CreatePylonDurableFleetRunPlannerInput,
} from "./fleet-run-durable-planner.js"
import { openPylonStandingFleetRunExecutor, type PylonStandingFleetRunExecutor } from "./fleet-run-standing-executor.js"

export const PYLON_MANAGED_CLOUD_FLEET_TARGET_SCHEMA = "openagents.pylon.managed_cloud_fleet_target.v1" as const

export const PYLON_MANAGED_CLOUD_FLEET_TUPLE_SCHEMA = "openagents.pylon.managed_cloud_fleet_tuple.v1" as const

export const PYLON_MANAGED_CLOUD_FLEET_BLOCKERS = {
  cloudEvidenceInvalid: "blocker.pylon.managed_cloud_fleet.cloud_evidence_invalid",
  executorFailed: "blocker.pylon.managed_cloud_fleet.executor_failed",
  grantUnavailable: "blocker.pylon.managed_cloud_fleet.grant_unavailable",
  lifecycleProjectionFailed: "blocker.pylon.managed_cloud_fleet.lifecycle_projection_failed",
  targetRequired: "blocker.pylon.managed_cloud_fleet.target_required",
  tupleInvalid: "blocker.pylon.managed_cloud_fleet.tuple_invalid",
  workerUnsupported: "blocker.pylon.managed_cloud_fleet.worker_unsupported",
} as const

const DEFAULT_TIMEOUT_MS = 30 * 60 * 1_000
const MAX_TIMEOUT_MS = 4 * 60 * 60 * 1_000
const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/u
const GRANT_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,179}$/u
const REPOSITORY_PATTERN = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u
const BRANCH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._\/-]{0,199}$/u
const COMMIT_PATTERN = /^[a-f0-9]{40}$/u
const VERIFY_ARG_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:@%+=,\/~\/-]{0,199}$/u

export type PylonManagedCloudFleetExactTuple = Readonly<{
  schema: typeof PYLON_MANAGED_CLOUD_FLEET_TUPLE_SCHEMA
  targetPreference: "managed_cloud"
  runRef: string
  taskId: string
  claimRef: string
  workUnitRef: string
  workerAccountRef: string
  workerKind: "codex"
  repository: Readonly<{
    fullName: string
    branch: string
    commit: string
  }>
  verifierRef: string
  fingerprint: string
}>

export type PylonManagedCloudFleetGrantPort = (
  tuple: PylonManagedCloudFleetExactTuple,
) => Promise<CloudSessionGrantBinding | null>

/**
 * Integration supplies the existing cloud executor with this exact grant
 * binding captured in its `grantBindingForSession` closure. Keeping the
 * factory injected prevents this adapter from reading ambient credentials or
 * inventing a second grant authority.
 */
export type PylonManagedCloudFleetExecutorPort = (
  input: Readonly<{
    binding: Required<Pick<CloudSessionGrantBinding, "authGrantRef" | "providerAccountRef" | "ownerRef">>
    sessionRef: string
    tuple: PylonManagedCloudFleetExactTuple
  }>,
) => ControlSessionExecutor

export type PylonManagedCloudFleetTargetProjection = Readonly<{
  schema: typeof PYLON_MANAGED_CLOUD_FLEET_TARGET_SCHEMA
  targetPreference: "managed_cloud"
  capacityClass: "managed_cloud"
  executionTargetRef: string | null
  targetEvidenceRef: string | null
  fallbackRefs: readonly []
}>

export type PylonManagedCloudFleetRunDispatchResult = FleetRunSupervisorDispatchResult &
  Readonly<{
    target: PylonManagedCloudFleetTargetProjection
  }>

export type PylonManagedCloudFleetRunDispatchInput = Readonly<{
  targetPreference: "managed_cloud"
  dispatch: FleetRunSupervisorDispatchInput
}>

export type PylonManagedCloudFleetRunClaimedWorkPort = Readonly<{
  dispatch: (input: PylonManagedCloudFleetRunDispatchInput) => Promise<PylonManagedCloudFleetRunDispatchResult>
}>

export type CreatePylonManagedCloudFleetRunClaimedWorkPortInput = Readonly<{
  summary: BootstrapSummary
  resolveGrantBinding: PylonManagedCloudFleetGrantPort
  createExecutor: PylonManagedCloudFleetExecutorPort
  cloudLane?: Exclude<ControlSessionLane, "local"> | undefined
  now?: (() => Date) | undefined
  timeoutMs?: number | undefined
}>

export type CreatePylonRemoteManagedCloudFleetRunClaimedWorkPortInput = Readonly<{
  agentToken: string
  baseUrl: string
  pylonRef: string
  fetchImpl?: typeof fetch | undefined
  now?: (() => Date) | undefined
}>

export type OpenPylonManagedCloudStandingFleetRunExecutorInput = Readonly<{
  summary: BootstrapSummary
  pylonRef: string
  runRef: string
  capacity: FleetRunSupervisorCapacity
  livenessProbe: FleetRunOwnerLocalLivenessProbe
  resolveGrantBinding: PylonManagedCloudFleetGrantPort
  createExecutor: PylonManagedCloudFleetExecutorPort
  cloudLane?: Exclude<ControlSessionLane, "local"> | undefined
  planner?: Omit<CreatePylonDurableFleetRunPlannerInput, "store"> | undefined
  clock?: Partial<FleetRunSupervisorClock> | undefined
  now?: (() => Date) | undefined
  onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  startImmediately?: boolean | undefined
  tickIntervalMs?: number | undefined
  timeoutMs?: number | undefined
}>

type PreparedDispatch = Readonly<{
  assignmentRef: string
  closeoutRef: string
  executionTargetRef: string
  objective: string
  sessionRef: string
  tuple: PylonManagedCloudFleetExactTuple
  verifierRef: string
  verifyArgs: readonly string[]
  workspaceRef: string
}>

const stableDigest = (value: string, length = 24): string =>
  createHash("sha256").update(value).digest("hex").slice(0, length)

const stableRef = (prefix: string, seed: string): string => `${prefix}.${stableDigest(seed)}`

const validPublicRef = (value: unknown): value is string => typeof value === "string" && PUBLIC_REF_PATTERN.test(value)

const validExactRef = (value: unknown): value is string =>
  typeof value === "string" && value.length >= 3 && value.length <= 180 && PUBLIC_REF_PATTERN.test(value)

const boundedTimeout = (value: number | undefined): number =>
  value === undefined || !Number.isFinite(value)
    ? DEFAULT_TIMEOUT_MS
    : Math.max(1_000, Math.min(MAX_TIMEOUT_MS, Math.trunc(value)))

const verificationArgs = (command: string | undefined): readonly string[] | null => {
  const value = command?.trim()
  if (value === undefined || value.length === 0 || value.length > 1_000) {
    return null
  }
  const args = value.split(/\s+/u).filter(Boolean)
  if (
    args.length === 0 ||
    args.length > 20 ||
    args.some(
      (arg) => !VERIFY_ARG_PATTERN.test(arg) || arg.startsWith("/") || arg.includes("..") || /[;&|`$<>\\]/u.test(arg),
    )
  ) {
    return null
  }
  return args
}

const prepareDispatch = (input: PylonManagedCloudFleetRunDispatchInput): PreparedDispatch | null => {
  const { dispatch } = input
  const { claim, run, workUnit } = dispatch
  const verifyArgs = verificationArgs(workUnit.verify)
  const authorityTarget = run.authorityBinding?.targetPreference

  if (
    input.targetPreference !== "managed_cloud" ||
    dispatch.workerKind !== "codex" ||
    run.state !== "running" ||
    claim.state !== "in_progress" ||
    claim.assignmentRef !== null ||
    claim.runRef !== run.runRef ||
    claim.workUnitRef !== workUnit.workUnitRef ||
    claim.workerAccountRef !== dispatch.accountRef ||
    !validExactRef(run.runRef) ||
    !validExactRef(dispatch.taskId) ||
    !validExactRef(claim.claimRef) ||
    !validExactRef(workUnit.workUnitRef) ||
    !validExactRef(dispatch.accountRef) ||
    (authorityTarget !== undefined &&
      String(authorityTarget) !== "managed_cloud" &&
      String(authorityTarget) !== "auto") ||
    (dispatch.executionTarget ?? "managed_cloud") !== "managed_cloud" ||
    typeof workUnit.repo !== "string" ||
    !REPOSITORY_PATTERN.test(workUnit.repo) ||
    typeof workUnit.branch !== "string" ||
    !BRANCH_PATTERN.test(workUnit.branch) ||
    workUnit.branch.includes("..") ||
    typeof workUnit.baseCommit !== "string" ||
    !COMMIT_PATTERN.test(workUnit.baseCommit) ||
    verifyArgs === null
  ) {
    return null
  }

  const verifierRef = stableRef("verifier.public.pylon.managed_cloud", workUnit.verify ?? "")
  const tupleSeed = JSON.stringify({
    schema: PYLON_MANAGED_CLOUD_FLEET_TUPLE_SCHEMA,
    targetPreference: input.targetPreference,
    runRef: run.runRef,
    taskId: dispatch.taskId,
    claimRef: claim.claimRef,
    workUnitRef: workUnit.workUnitRef,
    workerAccountRef: dispatch.accountRef,
    workerKind: dispatch.workerKind,
    repository: {
      fullName: workUnit.repo,
      branch: workUnit.branch,
      commit: workUnit.baseCommit,
    },
    verifierCommand: workUnit.verify,
  })
  const fingerprint = stableDigest(tupleSeed, 64)
  const tuple: PylonManagedCloudFleetExactTuple = {
    schema: PYLON_MANAGED_CLOUD_FLEET_TUPLE_SCHEMA,
    targetPreference: "managed_cloud",
    runRef: run.runRef,
    taskId: dispatch.taskId,
    claimRef: claim.claimRef,
    workUnitRef: workUnit.workUnitRef,
    workerAccountRef: dispatch.accountRef,
    workerKind: "codex",
    repository: {
      fullName: workUnit.repo,
      branch: workUnit.branch,
      commit: workUnit.baseCommit,
    },
    verifierRef,
    fingerprint,
  }
  const assignmentRef = stableRef("assignment.pylon.managed_cloud", fingerprint)
  return {
    assignmentRef,
    closeoutRef: stableRef("closeout.public.pylon.managed_cloud", fingerprint),
    executionTargetRef: stableRef("execution_target.pylon.managed_cloud", fingerprint),
    objective: [run.objective, workUnit.title, workUnit.body]
      .filter((value) => typeof value === "string" && value.trim().length > 0)
      .join("\n\n"),
    sessionRef: stableRef("session.pylon.managed_cloud", fingerprint),
    tuple,
    verifierRef,
    verifyArgs,
    workspaceRef: stableRef("workspace.pylon.managed_cloud", fingerprint),
  }
}

const validGrantBinding = (
  value: CloudSessionGrantBinding | null,
): value is Required<Pick<CloudSessionGrantBinding, "authGrantRef" | "providerAccountRef" | "ownerRef">> =>
  value !== null &&
  typeof value.authGrantRef === "string" &&
  GRANT_REF_PATTERN.test(value.authGrantRef) &&
  typeof value.providerAccountRef === "string" &&
  GRANT_REF_PATTERN.test(value.providerAccountRef) &&
  typeof value.ownerRef === "string" &&
  value.ownerRef.length >= 3 &&
  value.ownerRef.length <= 180 &&
  !/\s/u.test(value.ownerRef)

const targetProjection = (
  prepared: PreparedDispatch | null,
  targetEvidenceRef: string | null = null,
): PylonManagedCloudFleetTargetProjection => ({
  schema: PYLON_MANAGED_CLOUD_FLEET_TARGET_SCHEMA,
  targetPreference: "managed_cloud",
  capacityClass: "managed_cloud",
  executionTargetRef: prepared?.executionTargetRef ?? null,
  targetEvidenceRef,
  // This adapter has no local executor port. An empty list is affirmative
  // evidence that no target substitution occurred.
  fallbackRefs: [],
})

const fixedLifecycle = (
  input: Readonly<{
    assignmentRef: string | null
    blockerRef: string
    now: Date
  }>,
): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  event: input.assignmentRef === null ? "assignment_run.no_assignment" : "assignment_run.runtime_failed",
  observedAt: input.now.toISOString(),
  ...(input.assignmentRef === null ? {} : { assignmentRef: input.assignmentRef }),
  status: "rejected",
  blockerRefs: [input.blockerRef],
})

const fixedResult = (
  input: Readonly<{
    blockerRef: string
    now: Date
    prepared: PreparedDispatch | null
    status: "blocked" | "failed"
    lifecycle?: readonly PylonAssignmentRunLifecycleEvent[] | undefined
  }>,
): PylonManagedCloudFleetRunDispatchResult => {
  const inheritedLifecycle = [...(input.lifecycle ?? [])]
  const lifecycle = inheritedLifecycle.some((event) => event.blockerRefs?.includes(input.blockerRef))
    ? inheritedLifecycle
    : [
        ...inheritedLifecycle,
        fixedLifecycle({
          assignmentRef: input.prepared?.assignmentRef ?? null,
          blockerRef: input.blockerRef,
          now: input.now,
        }),
      ]
  const result: PylonManagedCloudFleetRunDispatchResult = {
    assignmentRef: input.prepared?.assignmentRef ?? null,
    accountRefHash: null,
    closeoutRef: null,
    lifecycle,
    status: input.status,
    summary:
      input.status === "blocked"
        ? "Managed-cloud claimed work was blocked before execution."
        : "Managed-cloud claimed work failed safely.",
    usageEvidence: null,
    target: targetProjection(input.prepared),
  }
  assertPublicProjectionSafe(result, "pylonManagedCloudFleetRunFailure")
  return result
}

const cloudTargetEvidence = (
  input: Readonly<{
    prepared: PreparedDispatch
    cloudRunner: unknown
    resourceUsageReceiptRef: unknown
    responseDigestRef: unknown
  }>,
): Readonly<{
  artifactRefs: readonly string[]
  resourceUsageReceiptRef: string
  targetEvidenceRef: string
}> | null => {
  if (
    typeof input.cloudRunner !== "object" ||
    input.cloudRunner === null ||
    !("lane" in input.cloudRunner) ||
    !("providerLane" in input.cloudRunner)
  ) {
    return null
  }
  const lane = input.cloudRunner.lane
  const providerLane = input.cloudRunner.providerLane
  if (
    (lane !== "cloud-gcp" && lane !== "cloud-shc") ||
    (providerLane !== "gcp" && providerLane !== "shc") ||
    !validPublicRef(input.resourceUsageReceiptRef) ||
    (input.responseDigestRef !== null && !validPublicRef(input.responseDigestRef))
  ) {
    return null
  }
  try {
    assertPublicProjectionSafe(
      {
        resourceUsageReceiptRef: input.resourceUsageReceiptRef,
        responseDigestRef: input.responseDigestRef,
      },
      "pylonManagedCloudFleetRunSourceEvidence",
    )
  } catch {
    return null
  }
  const targetEvidenceRef = stableRef(
    "evidence.public.pylon.managed_cloud.target",
    JSON.stringify({
      tupleFingerprint: input.prepared.tuple.fingerprint,
      lane,
      providerLane,
      resourceUsageReceiptRef: input.resourceUsageReceiptRef,
    }),
  )
  return {
    artifactRefs: typeof input.responseDigestRef === "string" ? [input.responseDigestRef] : [],
    resourceUsageReceiptRef: input.resourceUsageReceiptRef,
    targetEvidenceRef,
  }
}

export function createPylonManagedCloudFleetRunClaimedWorkPort(
  options: CreatePylonManagedCloudFleetRunClaimedWorkPortInput,
): PylonManagedCloudFleetRunClaimedWorkPort {
  const now = options.now ?? (() => new Date())
  const timeoutMs = boundedTimeout(options.timeoutMs)
  const lane = options.cloudLane ?? "auto"

  return {
    dispatch: async (input) => {
      if (
        input.targetPreference !== "managed_cloud" ||
        (lane !== "auto" && lane !== "cloud-gcp" && lane !== "cloud-shc")
      ) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.targetRequired,
          now: now(),
          prepared: null,
          status: "blocked",
        })
      }
      if (input.dispatch.workerKind !== "codex") {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.workerUnsupported,
          now: now(),
          prepared: null,
          status: "blocked",
        })
      }
      const prepared = prepareDispatch(input)
      if (prepared === null) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.tupleInvalid,
          now: now(),
          prepared: null,
          status: "blocked",
        })
      }

      let binding: CloudSessionGrantBinding | null
      try {
        binding = await Effect.runPromise(
          Effect.tryPromise({
            try: () => options.resolveGrantBinding(prepared.tuple),
            catch: () => new Error("managed_cloud_grant_unavailable"),
          }),
        )
      } catch {
        binding = null
      }
      if (!validGrantBinding(binding)) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.grantUnavailable,
          now: now(),
          prepared,
          status: "blocked",
        })
      }

      let executor: ControlSessionExecutor
      try {
        executor = options.createExecutor({
          binding,
          sessionRef: prepared.sessionRef,
          tuple: prepared.tuple,
        })
      } catch {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed,
          now: now(),
          prepared,
          status: "failed",
        })
      }

      const lifecycle: PylonAssignmentRunLifecycleEvent[] = []
      let lifecycleTail: Promise<void> = Promise.resolve()
      const publish = (event: PylonAssignmentRunLifecycleEvent): void => {
        lifecycle.push(event)
        if (input.dispatch.onLifecycle !== undefined) {
          lifecycleTail = lifecycleTail.then(() =>
            Effect.runPromise(
              Effect.tryPromise({
                try: async () => {
                  await input.dispatch.onLifecycle?.(event)
                },
                catch: () => new Error("managed_cloud_lifecycle_projection_failed"),
              }),
            ),
          )
        }
      }

      publish({
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.runtime_started",
        observedAt: now().toISOString(),
        assignmentRef: prepared.assignmentRef,
        status: "running",
        phase: "runtime_active",
      })

      let progressIndex = 0
      let executed: Awaited<ReturnType<ControlSessionExecutor>>
      try {
        executed = await Effect.runPromise(
          Effect.tryPromise({
            try: () =>
              executor({
                adapter: "codex",
                account: null,
                lane,
                abortSignal: new AbortController().signal,
                // The cloud executor must not receive or use a host checkout.
                // Exact public repository pins live in `tuple.repository`.
                cwd: ".",
                env: {},
                emit: () => {
                  progressIndex += 1
                  publish({
                    schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
                    event: "assignment_run.runtime_progress",
                    observedAt: now().toISOString(),
                    assignmentRef: prepared.assignmentRef,
                    status: "running",
                    phase: "runtime_active",
                    progressRef: stableRef(
                      "progress.public.pylon.managed_cloud",
                      `${prepared.tuple.fingerprint}:${progressIndex}`,
                    ),
                  })
                },
                objective: prepared.objective,
                sessionRef: prepared.sessionRef,
                summary: options.summary,
                timeoutMs,
                verify: [...prepared.verifyArgs],
                workspaceRef: prepared.workspaceRef,
              }),
            catch: () => new Error("managed_cloud_executor_failed"),
          }),
        )
      } catch {
        publish(
          fixedLifecycle({
            assignmentRef: prepared.assignmentRef,
            blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed,
            now: now(),
          }),
        )
        try {
          await lifecycleTail
        } catch {
          return fixedResult({
            blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.lifecycleProjectionFailed,
            now: now(),
            prepared,
            status: "failed",
            lifecycle,
          })
        }
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed,
          now: now(),
          prepared,
          status: "failed",
          lifecycle,
        })
      }

      const evidence = cloudTargetEvidence({
        prepared,
        cloudRunner: executed.cloudRunner,
        resourceUsageReceiptRef: executed.resourceUsageReceiptRef,
        responseDigestRef: executed.responseDigestRef,
      })
      if (evidence === null) {
        publish(
          fixedLifecycle({
            assignmentRef: prepared.assignmentRef,
            blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid,
            now: now(),
          }),
        )
        try {
          await lifecycleTail
        } catch {
          return fixedResult({
            blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.lifecycleProjectionFailed,
            now: now(),
            prepared,
            status: "failed",
            lifecycle,
          })
        }
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid,
          now: now(),
          prepared,
          status: "failed",
          lifecycle,
        })
      }

      const passed = executed.devCheck.state === "passed"
      publish({
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.completed",
        observedAt: now().toISOString(),
        assignmentRef: prepared.assignmentRef,
        status: passed ? "closed" : "rejected",
        closeoutRef: prepared.closeoutRef,
        ...(passed
          ? {}
          : {
              blockerRefs: [PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed],
            }),
      })
      try {
        await lifecycleTail
      } catch {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.lifecycleProjectionFailed,
          now: now(),
          prepared,
          status: "failed",
          lifecycle,
        })
      }

      const verificationEvidenceRefs = [evidence.targetEvidenceRef, evidence.resourceUsageReceiptRef]
      const result: PylonManagedCloudFleetRunDispatchResult = {
        assignmentRef: prepared.assignmentRef,
        accountRefHash: null,
        // The cloud runtime close event is not the accepted Sarah closeout.
        // Keep the supervisor carrier empty until exact owner-attributed token
        // evidence can supply its real closeout ref atomically.
        closeoutRef: null,
        lifecycle,
        marginalCostClass: input.dispatch.claim.marginalCostClass ?? "not_measured",
        status: passed ? "completed" : "failed",
        summary: passed
          ? "Managed-cloud Codex work completed with receipt-backed target evidence."
          : "Managed-cloud Codex work returned failed verification evidence.",
        usageEvidence: null,
        verification: passed
          ? {
              truth: "passed",
              verifierRef: prepared.verifierRef,
              evidenceRefs: verificationEvidenceRefs,
            }
          : {
              truth: "failed",
              verifierRef: prepared.verifierRef,
              evidenceRefs: verificationEvidenceRefs,
            },
        artifactRefs: evidence.artifactRefs,
        proofRefs: verificationEvidenceRefs,
        authorityReceiptRefs: [evidence.resourceUsageReceiptRef],
        target: targetProjection(prepared, evidence.targetEvidenceRef),
      }
      assertPublicProjectionSafe(result, "pylonManagedCloudFleetRunResult")
      return result
    },
  }
}

/**
 * Brokered FC-4 transport. Provider grants, execution tokens, GitHub write
 * authority, and cloud-control credentials stay server-side; the Pylon sends
 * only the exact accepted tuple and receives terminal public receipt refs.
 */
export function createPylonRemoteManagedCloudFleetRunClaimedWorkPort(
  options: CreatePylonRemoteManagedCloudFleetRunClaimedWorkPortInput,
): PylonManagedCloudFleetRunClaimedWorkPort {
  const fetchImpl = options.fetchImpl ?? fetch
  const now = options.now ?? (() => new Date())
  return {
    dispatch: async (input) => {
      const prepared = prepareDispatch(input)
      if (prepared === null) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.tupleInvalid,
          now: now(),
          prepared: null,
          status: "blocked",
        })
      }
      let body: unknown
      try {
        const response = await fetchImpl(
          `${options.baseUrl.replace(/\/+$/u, "")}/api/pylons/${encodeURIComponent(options.pylonRef)}/fleet-runs/${encodeURIComponent(prepared.tuple.runRef)}/managed-units/dispatch`,
          {
            method: "POST",
            headers: {
              Authorization: `Bearer ${options.agentToken}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              schema: "openagents.pylon.managed_cloud_fleet_dispatch.request.v1",
              targetPreference: "managed_cloud",
              taskId: prepared.tuple.taskId,
              claimRef: prepared.tuple.claimRef,
              workUnitRef: prepared.tuple.workUnitRef,
              workerAccountRef: prepared.tuple.workerAccountRef,
              workerKind: "codex",
              objective: prepared.objective,
              unitObjective: input.dispatch.workUnit.body,
              repository: prepared.tuple.repository,
              verifierRef: prepared.verifierRef,
              verifierCommand: input.dispatch.workUnit.verify,
              verify: [...prepared.verifyArgs],
              fingerprint: prepared.tuple.fingerprint,
            }),
          },
        )
        if (!response.ok) throw new Error("managed_fleet_dispatch_refused")
        body = await response.json()
      } catch {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed,
          now: now(),
          prepared,
          status: "failed",
        })
      }
      if (
        typeof body !== "object" ||
        body === null ||
        (body as { schema?: unknown }).schema !== "openagents.pylon.managed_cloud_fleet_dispatch.result.v1" ||
        (body as { state?: unknown }).state !== "completed"
      ) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid,
          now: now(),
          prepared,
          status: "failed",
        })
      }
      const result = body as {
        agentComputerRef?: unknown
        placementRef?: unknown
        lifecycleReceiptRefs?: unknown
        resourceUsageReceiptRefs?: unknown
        artifactRef?: unknown
      }
      const lifecycleReceiptRefs = Array.isArray(result.lifecycleReceiptRefs)
        ? result.lifecycleReceiptRefs.filter(validPublicRef)
        : []
      const usageRefs = Array.isArray(result.resourceUsageReceiptRefs)
        ? result.resourceUsageReceiptRefs.filter(validPublicRef)
        : []
      if (
        !validPublicRef(result.agentComputerRef) ||
        !validPublicRef(result.placementRef) ||
        lifecycleReceiptRefs.length === 0 ||
        usageRefs.length === 0
      ) {
        return fixedResult({
          blockerRef: PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid,
          now: now(),
          prepared,
          status: "failed",
        })
      }
      const targetEvidenceRef = stableRef(
        "evidence.public.pylon.managed_cloud.target",
        `${prepared.tuple.fingerprint}:${result.placementRef}:${usageRefs[0]}`,
      )
      const lifecycle: PylonAssignmentRunLifecycleEvent[] = [
        {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.runtime_started",
          observedAt: now().toISOString(),
          assignmentRef: prepared.assignmentRef,
          status: "running",
          phase: "runtime_active",
        },
        {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.completed",
          observedAt: now().toISOString(),
          assignmentRef: prepared.assignmentRef,
          status: "closed",
          closeoutRef: prepared.closeoutRef,
        },
      ]
      for (const event of lifecycle) await input.dispatch.onLifecycle?.(event)
      const evidenceRefs = [targetEvidenceRef, ...lifecycleReceiptRefs, ...usageRefs]
      const completed: PylonManagedCloudFleetRunDispatchResult = {
        assignmentRef: prepared.assignmentRef,
        accountRefHash: null,
        closeoutRef: null,
        lifecycle,
        marginalCostClass: input.dispatch.claim.marginalCostClass ?? "not_measured",
        status: "completed",
        summary: "Managed-cloud Codex work completed on an Agent Computer with broker-held authority.",
        usageEvidence: null,
        verification: {
          truth: "passed",
          verifierRef: prepared.verifierRef,
          evidenceRefs,
        },
        artifactRefs: validPublicRef(result.artifactRef) ? [result.artifactRef] : [],
        proofRefs: evidenceRefs,
        authorityReceiptRefs: usageRefs,
        target: targetProjection(prepared, targetEvidenceRef),
      }
      assertPublicProjectionSafe(completed, "pylonRemoteManagedCloudFleetRunResult")
      return completed
    },
  }
}

/**
 * Compose the managed-cloud claimed-work adapter into the same durable
 * planner/supervisor used by owner-local FleetRuns. All provider-account,
 * grant, capacity, and restart-liveness authority stays injected: this layer
 * never derives it from a public capacity class and has no local runner port.
 */
export async function openPylonManagedCloudStandingFleetRunExecutor(
  input: OpenPylonManagedCloudStandingFleetRunExecutorInput,
): Promise<PylonStandingFleetRunExecutor> {
  return await openPylonStandingFleetRunExecutor({
    bootstrap: input.summary,
    pylonRef: input.pylonRef,
    runRef: input.runRef,
    ...(input.clock === undefined ? {} : { clock: input.clock }),
    ...(input.now === undefined ? {} : { now: input.now }),
    ...(input.onLifecycle === undefined ? {} : { onLifecycle: input.onLifecycle }),
    ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
    ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
    adapterFactory: ({ store }) => {
      const claimedWork = createPylonManagedCloudFleetRunClaimedWorkPort({
        summary: input.summary,
        resolveGrantBinding: input.resolveGrantBinding,
        createExecutor: input.createExecutor,
        ...(input.cloudLane === undefined ? {} : { cloudLane: input.cloudLane }),
        ...(input.now === undefined ? {} : { now: input.now }),
        ...(input.timeoutMs === undefined ? {} : { timeoutMs: input.timeoutMs }),
      })
      return {
        capacity: {
          accounts: async (capacityInput) =>
            (await input.capacity.accounts(capacityInput)).map((account) => ({
              ...account,
              executionTarget: "managed_cloud" as const,
              acceptedDataPostures: ["broker_safe"] as const,
              repositoryAccess: true,
              managedIsolation: true,
            })),
        },
        livenessProbe: input.livenessProbe,
        planner: createPylonDurableFleetRunPlanner({
          ...(input.planner ?? {}),
          store,
        }),
        runner: {
          dispatch: (dispatch) => {
            // The supervisor persists `in_progress` immediately before it
            // invokes the runner, while its immutable call snapshot still
            // carries the preceding `claimed` state. Bind cloud authority to
            // the durable post-transition claim, never to the stale snapshot.
            const durableClaim = store.getWorkClaim(dispatch.claim.claimRef)
            return claimedWork.dispatch({
              targetPreference: "managed_cloud",
              dispatch: {
                ...dispatch,
                ...(durableClaim === null ? {} : { claim: durableClaim }),
              },
            })
          },
        },
      }
    },
  })
}
