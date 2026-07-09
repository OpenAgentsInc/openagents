import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import { Effect, Schema as S } from "effect"

import type { BootstrapSummary } from "../bootstrap.js"
import { assertPublicProjectionSafe } from "../state.js"
import { assertPublicSafe } from "../work-requester.js"
import type {
  PylonFleetRunActivationProjection,
  PylonFleetRunActivationStatus,
} from "../node/fleet-run-activation.js"
import {
  decodeFleetRunWorkSourceDescriptor,
  FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
  type FleetRunWorkSourceDescriptor,
} from "./fleet-run-work-source.js"
import {
  FLEET_RUN_AUTHORITY_BINDING_SCHEMA,
  type FleetRun,
  type FleetRunAuthorityBinding,
} from "./store.js"
import {
  openPylonFleetRunRuntime,
  type OpenPylonFleetRunRuntimeInput,
  type PylonFleetRunRuntime,
} from "./fleet-run-runtime.js"

export const PYLON_FLEET_RUN_REMOTE_INTAKE_SCHEMA =
  "openagents.pylon.fleet_run_remote_intake.v1" as const

const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
const PYLON_REF = /^[a-z0-9][a-z0-9._:-]{2,119}$/u
const RUN_REF = /^fleet_run\.sarah\.[0-9a-f]{20}$/u
const CLAIM_REF = /^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u
const FINGERPRINT = /^[0-9a-f]{64}$/u
const COMMIT = /^[0-9a-f]{40}$/u
const ISSUE_REF = /^#[1-9][0-9]*$/u
const REPOSITORY_SLUG = /^[A-Za-z0-9_.-]{1,120}$/u
const BRANCH = /^(?!-)(?!refs\/)(?!.*(?:^|\/)\.)(?!.*(?:^|\/)$)(?!.*\.\.)(?!.*@\{)(?!.*\/\/)(?!.*\.lock(?:\/|$))(?!.*\.$)[A-Za-z0-9][A-Za-z0-9._/-]{0,119}$/u
const IDEMPOTENCY_KEY = /^[A-Za-z0-9][A-Za-z0-9._:-]{7,119}$/u
const CONTROL_CHARACTER = /[\u0000-\u001f\u007f]/u

const PublicRef = S.String.check(S.isPattern(PUBLIC_REF))
const PylonRef = S.String.check(S.isPattern(PYLON_REF))
const RunRef = S.String.check(S.isPattern(RUN_REF))
const ClaimRef = S.String.check(S.isPattern(CLAIM_REF))
const Fingerprint = S.String.check(S.isPattern(FINGERPRINT))
const IsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u),
)

const RemoteRepositoryPin = S.Struct({
  owner: S.String,
  name: S.String,
  branch: S.String,
  commit: S.String.check(S.isPattern(COMMIT)),
})

const RemoteVerifier = S.Struct({
  kind: S.Literal("command"),
  command: S.String,
})

const RemoteIssueListSource = S.Struct({
  kind: S.Literal("issue_list"),
  issueRefs: S.Array(S.String.check(S.isPattern(ISSUE_REF))),
})

const RemotePlanUnit = S.Struct({
  unitRef: PublicRef,
  title: S.String,
  dependsOn: S.optionalKey(S.Array(PublicRef)),
})

const RemotePlanDagSource = S.Struct({
  kind: S.Literal("plan_dag"),
  planRef: PublicRef,
  units: S.Array(RemotePlanUnit),
})

const RemoteAuthorityRequest = S.Struct({
  schema: S.Literal("sarah.coding_fleet_start.request.v1"),
  objective: S.String,
  repository: RemoteRepositoryPin,
  verifier: RemoteVerifier,
  workSource: S.Union([RemoteIssueListSource, RemotePlanDagSource]),
  workerPolicy: S.Struct({
    workerKind: S.Literals(["codex", "claude", "grok", "auto"]),
    targetPreference: S.Literals(["owner_local", "managed_cloud", "auto"]),
  }),
  targetConcurrency: S.Number,
  idempotencyKey: S.String,
})

const RemoteAuthorityRecord = S.Struct({
  schema: S.Literal("openagents.sarah.fleet_run_authority.v1"),
  runRef: RunRef,
  scope: S.String,
  ownerUserId: S.String,
  requestFingerprint: Fingerprint,
  status: S.Literal("pending_executor"),
  request: RemoteAuthorityRequest,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
})

const RemoteIntakeClaim = S.Struct({
  schema: S.Literal("openagents.sarah.fleet_run_intake_claim.v1"),
  claimRef: ClaimRef,
  runRef: RunRef,
  ownerUserId: S.String,
  pylonRef: PylonRef,
  claimIdempotencyKey: S.String,
  state: S.Literal("claimed"),
  leaseExpiresAt: IsoTimestamp,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
})

const RemoteClaimResult = S.Struct({
  duplicate: S.Boolean,
  claim: RemoteIntakeClaim,
  run: RemoteAuthorityRecord,
})

const RemoteAcceptResult = S.Struct({
  duplicate: S.Boolean,
  claim: S.Struct({
    schema: S.Literal("openagents.sarah.fleet_run_intake_claim.v1"),
    claimRef: ClaimRef,
    runRef: RunRef,
    ownerUserId: S.String,
    pylonRef: PylonRef,
    claimIdempotencyKey: S.String,
    state: S.Literal("accepted"),
    leaseExpiresAt: IsoTimestamp,
    createdAt: IsoTimestamp,
    updatedAt: IsoTimestamp,
  }),
  run: S.Struct({
    schema: S.Literal("openagents.sarah.fleet_run_authority.v1"),
    runRef: RunRef,
    scope: S.String,
    ownerUserId: S.String,
    requestFingerprint: Fingerprint,
    status: S.Literal("claimed_by_pylon"),
    request: RemoteAuthorityRequest,
    createdAt: IsoTimestamp,
    updatedAt: IsoTimestamp,
  }),
})

type DecodedRemoteClaimResult = typeof RemoteClaimResult.Type

/**
 * Authenticated server transport boundary.
 *
 * The production adapter owns the bearer token, owner resolution, request
 * signing, and server claim-idempotency key. None of those values are accepted
 * by or projected from the orchestration service. Responses are `unknown` so
 * the Effect Schema decoder remains the actual trust boundary.
 */
export type PylonFleetRunRemoteIntakePort = {
  readonly claimNext: (input: {
    readonly pylonRef: string
    /** Exact retry target after an imported server lease expires. */
    readonly runRef?: string | undefined
  }) => Promise<unknown | null>
  readonly acceptClaim: (input: {
    readonly claimRef: string
    readonly pylonRef: string
    readonly runRef: string
  }) => Promise<unknown>
}

/**
 * Fixed adapter-to-orchestrator failures. The HTTP adapter must discard server
 * text and map only these kinds; `claim_expired` is the sole kind that permits
 * an explicit exact-run lease replacement before server acceptance.
 */
export class PylonFleetRunRemotePortError extends S.TaggedErrorClass<PylonFleetRunRemotePortError>()(
  "PylonFleetRunRemotePortError",
  {
    kind: S.Literals([
      "claim_conflict",
      "claim_expired",
      "not_authorized",
      "unavailable",
    ]),
  },
) {}

/** The existing loopback/bearer-gated node activation authority. */
export type PylonFleetRunActivationPort = {
  readonly arm: (runRef: string) => Promise<PylonFleetRunActivationProjection>
  readonly status: (runRef?: string) => Promise<PylonFleetRunActivationStatus>
}

export type PylonFleetRunRemoteIntakeState =
  | "idle"
  | "imported_accept_blocked"
  | "accepted_activation_blocked"
  | "active"

export type PylonFleetRunRemoteIntakeProjection = {
  readonly schema: typeof PYLON_FLEET_RUN_REMOTE_INTAKE_SCHEMA
  readonly pylonRef: string
  readonly runRef: string | null
  readonly state: PylonFleetRunRemoteIntakeState
  readonly retryable: boolean
  readonly blockerRefs: readonly string[]
}

export type OpenPylonFleetRunRemoteIntakeServiceInput = {
  readonly activation: PylonFleetRunActivationPort
  readonly bootstrap?: Pick<BootstrapSummary, "paths"> | undefined
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly now?: (() => Date) | undefined
  readonly openRuntime?: typeof openPylonFleetRunRuntime | undefined
  readonly pylonRef: string
  readonly remote: PylonFleetRunRemoteIntakePort
}

export type PylonFleetRunRemoteIntakeService = {
  /** Reconcile durable local imports first; claim at most one new remote run. */
  readonly runOnce: () => Promise<PylonFleetRunRemoteIntakeProjection>
  /** Retry one accepted/imported local run without asking the server for more. */
  readonly reconcile: (runRef?: string) => Promise<PylonFleetRunRemoteIntakeProjection>
  readonly close: () => Promise<void>
}

export class PylonFleetRunRemoteIntakeError extends S.TaggedErrorClass<PylonFleetRunRemoteIntakeError>()(
  "PylonFleetRunRemoteIntakeError",
  {
    kind: S.Literals([
      "authority_conflict",
      "authority_invalid",
      "local_store_unavailable",
      "pylon_ref_invalid",
      "run_not_found",
    ]),
    blockerRefs: S.Array(S.String),
    runRef: S.optionalKey(S.String),
  },
) {}

const fixedError = (
  kind: PylonFleetRunRemoteIntakeError["kind"],
  runRef?: string,
): PylonFleetRunRemoteIntakeError =>
  new PylonFleetRunRemoteIntakeError({
    kind,
    blockerRefs: [`blocker.pylon.fleet_run_intake.${kind}`],
    ...(runRef === undefined || !RUN_REF.test(runRef) ? {} : { runRef }),
  })

const projection = (
  pylonRef: string,
  state: PylonFleetRunRemoteIntakeState,
  input: {
    readonly runRef?: string | null
    readonly blocker?: string | undefined
    readonly retryable?: boolean | undefined
  } = {},
): PylonFleetRunRemoteIntakeProjection => ({
  schema: PYLON_FLEET_RUN_REMOTE_INTAKE_SCHEMA,
  pylonRef,
  runRef: input.runRef ?? null,
  state,
  retryable: input.retryable ?? false,
  blockerRefs: input.blocker === undefined ? [] : [input.blocker],
})

const remoteBlockedProjection = (
  pylonRef: string,
  operation: "accept" | "claim",
  state: Extract<
    PylonFleetRunRemoteIntakeState,
    "idle" | "imported_accept_blocked"
  >,
  error: unknown,
  runRef?: string,
): PylonFleetRunRemoteIntakeProjection => {
  const kind = error instanceof PylonFleetRunRemotePortError
    ? error.kind
    : "unavailable"
  return projection(pylonRef, state, {
    ...(runRef === undefined ? {} : { runRef }),
    retryable: kind === "unavailable" || kind === "claim_expired",
    blocker: `blocker.pylon.fleet_run_intake.remote_${operation}_${kind}`,
  })
}

const sha256 = (value: unknown): string =>
  createHash("sha256").update(canonicalJson(value)).digest("hex")

const assertAuthorityPublicSafe = (
  request: typeof RemoteAuthorityRequest.Type,
): void => {
  const titles = request.workSource.kind === "plan_dag"
    ? request.workSource.units.map(unit => unit.title)
    : []
  if (
    request.objective !== request.objective.trim() ||
    CONTROL_CHARACTER.test(request.objective) ||
    !REPOSITORY_SLUG.test(request.repository.owner) ||
    !REPOSITORY_SLUG.test(request.repository.name) ||
    !BRANCH.test(request.repository.branch) ||
    !IDEMPOTENCY_KEY.test(request.idempotencyKey) ||
    request.verifier.command !== request.verifier.command.trim() ||
    request.verifier.command.length < 3 ||
    request.verifier.command.length > 240 ||
    CONTROL_CHARACTER.test(request.verifier.command) ||
    titles.some(title =>
      title !== title.trim() ||
      title.length < 1 ||
      title.length > 160 ||
      CONTROL_CHARACTER.test(title),
    )
  ) {
    throw fixedError("authority_invalid")
  }
  const projected = {
    objective: request.objective,
    repository: request.repository,
    verifier: request.verifier,
    workSource: request.workSource,
  }
  assertPublicSafe(projected, "Sarah FleetRun authority")
  assertPublicProjectionSafe(projected, "sarahFleetRunAuthority")
}

const decodeClaimResult = (value: unknown): DecodedRemoteClaimResult => {
  try {
    const decoded = S.decodeUnknownSync(RemoteClaimResult)(value, {
      onExcessProperty: "error",
    })
    assertAuthorityPublicSafe(decoded.run.request)
    const expectedRunRef = `fleet_run.sarah.${sha256({
      schema: "openagents.sarah.fleet_run_ref.v1",
      ownerUserId: decoded.run.ownerUserId,
      idempotencyKey: decoded.run.request.idempotencyKey,
    }).slice(0, 20)}`
    const expectedClaimRef = `claim.sarah_fleet_run.${sha256({
      schema: "openagents.sarah.fleet_run_claim_ref.v1",
      runRef: decoded.run.runRef,
      pylonRef: decoded.claim.pylonRef,
      claimIdempotencyKey: decoded.claim.claimIdempotencyKey,
    }).slice(0, 24)}`
    if (
      decoded.run.runRef !== decoded.claim.runRef ||
      decoded.run.ownerUserId !== decoded.claim.ownerUserId ||
      decoded.run.runRef !== expectedRunRef ||
      decoded.claim.claimRef !== expectedClaimRef ||
      decoded.run.scope !== `scope.fleet_run.${decoded.run.runRef}` ||
      sha256(decoded.run.request) !== decoded.run.requestFingerprint ||
      Date.parse(decoded.claim.leaseExpiresAt) <= Date.parse(decoded.claim.createdAt) ||
      Date.parse(decoded.claim.updatedAt) < Date.parse(decoded.claim.createdAt) ||
      decoded.run.request.workerPolicy.targetPreference === "managed_cloud" ||
      decoded.run.request.targetConcurrency < 1 ||
      decoded.run.request.targetConcurrency > 8 ||
      !Number.isInteger(decoded.run.request.targetConcurrency) ||
      decoded.run.request.objective.trim().length < 8 ||
      decoded.run.request.objective.length > 1_000
    ) {
      throw fixedError("authority_invalid", decoded.run.runRef)
    }
    return decoded
  } catch (error) {
    if (error instanceof PylonFleetRunRemoteIntakeError) throw error
    throw fixedError("authority_invalid")
  }
}

const issueNumber = (issueRef: string): number => {
  const number = Number(issueRef.slice(1))
  if (!Number.isSafeInteger(number) || number < 1) {
    throw fixedError("authority_invalid")
  }
  return number
}

const descriptorFrom = (
  claim: DecodedRemoteClaimResult,
): FleetRunWorkSourceDescriptor => {
  const request = claim.run.request
  const repository = `${request.repository.owner}/${request.repository.name}`
  const pins = {
    repo: repository,
    branch: request.repository.branch,
    baseCommit: request.repository.commit,
    verify: request.verifier.command,
  }
  try {
    return request.workSource.kind === "issue_list"
      ? decodeFleetRunWorkSourceDescriptor({
          schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
          kind: "issue_list",
          ...pins,
          issues: request.workSource.issueRefs.map(issueNumber),
        })
      : decodeFleetRunWorkSourceDescriptor({
          schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
          kind: "plan_dag",
          planRef: request.workSource.planRef,
          ...pins,
          nodes: request.workSource.units.map(unit => ({
            ref: unit.unitRef,
            title: unit.title,
            objective: request.objective,
            dependsOn: unit.dependsOn ?? [],
            ...pins,
          })),
        })
  } catch {
    throw fixedError("authority_invalid", claim.run.runRef)
  }
}

const bindingFrom = (
  claim: DecodedRemoteClaimResult,
  phase: FleetRunAuthorityBinding["phase"],
): FleetRunAuthorityBinding => ({
  schema: FLEET_RUN_AUTHORITY_BINDING_SCHEMA,
  source: "sarah_authority",
  authorityFingerprint: claim.run.requestFingerprint,
  claimRef: claim.claim.claimRef,
  pylonRef: claim.claim.pylonRef,
  targetPreference: claim.run.request.workerPolicy.targetPreference as "owner_local" | "auto",
  phase,
})

const expectedAuthorityPins = (
  claim: DecodedRemoteClaimResult,
  descriptor: FleetRunWorkSourceDescriptor,
) => ({
  runRef: claim.run.runRef,
  objective: claim.run.request.objective,
  workSource: descriptor.kind,
  workSourceDescriptor: descriptor,
  targetConcurrency: claim.run.request.targetConcurrency,
  workerKind: claim.run.request.workerPolicy.workerKind,
  dispatchKind: "supervised_dispatch" as const,
  authorityFingerprint: claim.run.requestFingerprint,
  pylonRef: claim.claim.pylonRef,
  targetPreference: claim.run.request.workerPolicy.targetPreference,
})

const storedAuthorityPins = (run: FleetRun) => ({
  runRef: run.runRef,
  objective: run.objective,
  workSource: run.workSource,
  workSourceDescriptor: run.workSourceDescriptor,
  targetConcurrency: run.targetConcurrency,
  workerKind: run.workerKind,
  dispatchKind: run.dispatchKind,
  authorityFingerprint: run.authorityBinding?.authorityFingerprint,
  pylonRef: run.authorityBinding?.pylonRef,
  targetPreference: run.authorityBinding?.targetPreference,
})

const pinsMatch = (
  run: FleetRun,
  claim: DecodedRemoteClaimResult,
  descriptor: FleetRunWorkSourceDescriptor,
): boolean =>
  canonicalJson(storedAuthorityPins(run)) ===
  canonicalJson(expectedAuthorityPins(claim, descriptor))

const withBinding = (
  run: FleetRun,
  binding: FleetRunAuthorityBinding,
): FleetRun => ({ ...run, authorityBinding: binding })

const importClaim = (
  runtime: PylonFleetRunRuntime,
  claim: DecodedRemoteClaimResult,
  pylonRef: string,
  now: Date,
): FleetRun => {
  if (claim.claim.pylonRef !== pylonRef) {
    throw fixedError("authority_invalid", claim.run.runRef)
  }
  const descriptor = descriptorFrom(claim)
  const existing = runtime.store.getFleetRun(claim.run.runRef)
  if (existing !== null) {
    if (!pinsMatch(existing, claim, descriptor)) {
      throw fixedError("authority_conflict", claim.run.runRef)
    }
    if (
      existing.authorityBinding?.phase === "accepted" &&
      existing.authorityBinding.claimRef !== claim.claim.claimRef
    ) {
      throw fixedError("authority_conflict", claim.run.runRef)
    }
    if (existing.authorityBinding?.phase === "accepted") return existing
    return runtime.store.upsertFleetRun(
      withBinding(existing, bindingFrom(claim, "imported")),
      now,
    )
  }
  return runtime.store.createFleetRun({
    runRef: claim.run.runRef,
    objective: claim.run.request.objective,
    workSource: descriptor.kind,
    workSourceDescriptor: descriptor,
    authorityBinding: bindingFrom(claim, "imported"),
    targetConcurrency: claim.run.request.targetConcurrency,
    workerKind: claim.run.request.workerPolicy.workerKind,
    state: "running",
    dispatchKind: "supervised_dispatch",
    now,
  })
}

const activationBlocker = (
  activation: PylonFleetRunActivationProjection | undefined,
): string => {
  const reason = activation?.reason
  return reason === null || reason === undefined
    ? "blocker.pylon.fleet_run_intake.activation_unavailable"
    : `blocker.pylon.fleet_run_intake.activation_${reason}`
}

/**
 * Open the restart-safe bridge from the server intake lease to the one local
 * Pylon orchestration registry and the existing node activation authority.
 */
export async function openPylonFleetRunRemoteIntakeService(
  input: OpenPylonFleetRunRemoteIntakeServiceInput,
): Promise<PylonFleetRunRemoteIntakeService> {
  if (!PYLON_REF.test(input.pylonRef)) throw fixedError("pylon_ref_invalid")
  const runtimeInput: OpenPylonFleetRunRuntimeInput = {
    ...(input.bootstrap === undefined ? {} : { bootstrap: input.bootstrap }),
    ...(input.env === undefined ? {} : { env: input.env }),
    ...(input.now === undefined ? {} : { now: input.now }),
  }
  const runtime = await (input.openRuntime ?? openPylonFleetRunRuntime)(runtimeInput)
  const now = input.now ?? (() => new Date())
  let closed = false
  let tail = Promise.resolve()

  const serialize = <A>(operation: () => Promise<A>): Promise<A> => {
    const next = tail.then(operation, operation)
    tail = next.then(() => undefined, () => undefined)
    return next
  }

  const assertOpen = (): void => {
    if (closed) throw fixedError("local_store_unavailable")
  }

  const accept = async (
    run: FleetRun,
    allowExpiredLeaseReplacement = true,
  ): Promise<PylonFleetRunRemoteIntakeProjection> => {
    const binding = run.authorityBinding
    if (binding === undefined || binding.source !== "sarah_authority") {
      throw fixedError("authority_conflict", run.runRef)
    }
    let raw: unknown
    try {
      raw = await Effect.runPromise(Effect.tryPromise({
        try: () => input.remote.acceptClaim({
          claimRef: binding.claimRef,
          pylonRef: input.pylonRef,
          runRef: run.runRef,
        }),
        catch: error =>
          error instanceof PylonFleetRunRemotePortError
            ? error
            : new PylonFleetRunRemotePortError({ kind: "unavailable" }),
      }))
    } catch (error) {
      if (
        allowExpiredLeaseReplacement &&
        error instanceof PylonFleetRunRemotePortError &&
        error.kind === "claim_expired"
      ) {
        let replacementRaw: unknown | null
        try {
          replacementRaw = await input.remote.claimNext({
            pylonRef: input.pylonRef,
            runRef: run.runRef,
          })
        } catch (replacementError) {
          return remoteBlockedProjection(
            input.pylonRef,
            "claim",
            "imported_accept_blocked",
            replacementError,
            run.runRef,
          )
        }
        if (replacementRaw !== null) {
          const replacement = decodeClaimResult(replacementRaw)
          if (replacement.run.runRef !== run.runRef) {
            throw fixedError("authority_conflict", run.runRef)
          }
          const reimported = importClaim(
            runtime,
            replacement,
            input.pylonRef,
            now(),
          )
          return await accept(reimported, false)
        }
        return remoteBlockedProjection(
          input.pylonRef,
          "accept",
          "imported_accept_blocked",
          error,
          run.runRef,
        )
      }
      return remoteBlockedProjection(
        input.pylonRef,
        "accept",
        "imported_accept_blocked",
        error,
        run.runRef,
      )
    }

    let accepted: typeof RemoteAcceptResult.Type
    try {
      accepted = S.decodeUnknownSync(RemoteAcceptResult)(raw, {
        onExcessProperty: "error",
      })
    } catch {
      throw fixedError("authority_invalid", run.runRef)
    }
    if (
      accepted.claim.claimRef !== binding.claimRef ||
      accepted.claim.runRef !== run.runRef ||
      accepted.claim.ownerUserId !== accepted.run.ownerUserId ||
      accepted.claim.pylonRef !== input.pylonRef ||
      accepted.run.runRef !== run.runRef ||
      accepted.run.requestFingerprint !== binding.authorityFingerprint ||
      sha256(accepted.run.request) !== binding.authorityFingerprint
    ) {
      throw fixedError("authority_invalid", run.runRef)
    }
    const acceptedAsClaim = decodeClaimResult({
      duplicate: accepted.duplicate,
      claim: { ...accepted.claim, state: "claimed" },
      run: { ...accepted.run, status: "pending_executor" },
    })
    const acceptedDescriptor = descriptorFrom(acceptedAsClaim)
    if (!pinsMatch(run, acceptedAsClaim, acceptedDescriptor)) {
      throw fixedError("authority_conflict", run.runRef)
    }

    let durableAccepted: FleetRun
    try {
      durableAccepted = runtime.store.upsertFleetRun(
        withBinding(run, { ...binding, phase: "accepted" }),
        now(),
      )
    } catch {
      // The server accept is idempotent. A restart sees the imported binding,
      // replays this same claim ref, then durably records accepted before arm.
      return projection(input.pylonRef, "imported_accept_blocked", {
        runRef: run.runRef,
        retryable: true,
        blocker: "blocker.pylon.fleet_run_intake.local_accept_record_unavailable",
      })
    }
    return await activate(durableAccepted)
  }

  const activate = async (run: FleetRun): Promise<PylonFleetRunRemoteIntakeProjection> => {
    try {
      await input.activation.arm(run.runRef)
      const status = await input.activation.status(run.runRef)
      const activation = status.runs.find(entry => entry.runRef === run.runRef)
      if (activation?.armed === true && activation.active === true) {
        return projection(input.pylonRef, "active", { runRef: run.runRef })
      }
      return projection(input.pylonRef, "accepted_activation_blocked", {
        runRef: run.runRef,
        retryable: activation?.retryable ?? true,
        blocker: activationBlocker(activation),
      })
    } catch {
      return projection(input.pylonRef, "accepted_activation_blocked", {
        runRef: run.runRef,
        retryable: true,
        blocker: "blocker.pylon.fleet_run_intake.activation_unavailable",
      })
    }
  }

  const localBindings = (requestedRunRef?: string): FleetRun[] =>
    runtime.store
      .listFleetRuns()
      .filter(run => run.authorityBinding?.source === "sarah_authority")
      .filter(run => run.authorityBinding?.pylonRef === input.pylonRef)
      .filter(run => requestedRunRef === undefined || run.runRef === requestedRunRef)
      .sort((left, right) => left.createdAt.localeCompare(right.createdAt) || left.runRef.localeCompare(right.runRef))

  const reconcileLocal = async (
    requestedRunRef?: string,
  ): Promise<PylonFleetRunRemoteIntakeProjection | null> => {
    const runs = localBindings(requestedRunRef)
    if (requestedRunRef !== undefined && runs.length === 0) {
      throw fixedError("run_not_found", requestedRunRef)
    }
    const run = runs.find(candidate =>
      candidate.state !== "completed" && candidate.state !== "stopped",
    )
    if (run === undefined) return null
    return run.authorityBinding?.phase === "imported"
      ? await accept(run)
      : await activate(run)
  }

  const reconcile = (runRef?: string): Promise<PylonFleetRunRemoteIntakeProjection> =>
    serialize(async () => {
      assertOpen()
      if (runRef !== undefined && !RUN_REF.test(runRef)) {
        throw fixedError("run_not_found")
      }
      try {
        return (await reconcileLocal(runRef)) ?? projection(input.pylonRef, "idle")
      } catch (error) {
        if (error instanceof PylonFleetRunRemoteIntakeError) throw error
        throw fixedError("local_store_unavailable", runRef)
      }
    })

  const runOnce = (): Promise<PylonFleetRunRemoteIntakeProjection> =>
    serialize(async () => {
      assertOpen()
      try {
        const pending = await reconcileLocal()
        if (pending !== null) return pending
      } catch (error) {
        if (error instanceof PylonFleetRunRemoteIntakeError) throw error
        throw fixedError("local_store_unavailable")
      }

      let rawClaim: unknown | null
      try {
        rawClaim = await Effect.runPromise(Effect.tryPromise({
          try: () => input.remote.claimNext({ pylonRef: input.pylonRef }),
          catch: error =>
            error instanceof PylonFleetRunRemotePortError
              ? error
              : new PylonFleetRunRemotePortError({ kind: "unavailable" }),
        }))
      } catch (error) {
        return remoteBlockedProjection(
          input.pylonRef,
          "claim",
          "idle",
          error,
        )
      }
      if (rawClaim === null) return projection(input.pylonRef, "idle")

      const claim = decodeClaimResult(rawClaim)
      let imported: FleetRun
      try {
        imported = importClaim(runtime, claim, input.pylonRef, now())
      } catch (error) {
        if (error instanceof PylonFleetRunRemoteIntakeError) throw error
        throw fixedError("local_store_unavailable", claim.run.runRef)
      }
      return imported.authorityBinding?.phase === "accepted"
        ? await activate(imported)
        : await accept(imported)
    })

  return {
    runOnce,
    reconcile,
    close: () => serialize(async () => {
      if (closed) return
      closed = true
      await runtime.close()
    }),
  }
}
