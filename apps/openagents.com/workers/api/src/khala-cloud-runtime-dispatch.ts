// Seam A (#8503, AC-1) — server-owned `cloud-gcp` runtime dispatch consumer.
//
// THE GAP THIS CLOSES. The in-monolith `hosted_khala` consumer
// (`khala-hosted-runtime-dispatch.ts`) answers a mobile chat turn with hosted
// Gemini text IN PROCESS. It does NOT run a real coding turn against a repo.
// Seam A is the SIBLING consumer for the `cloud-gcp` lane: for an ADMITTED
// work-context (owner + repo binding), it mints a fresh short-lived
// owner-linked execution token (step 1), bakes it into the turn-runner's
// inference block + base64 work-context (step 2), and POSTs a `/v1/placement`
// through the cloud-control adapter with `work_context_b64` (step 3). The cloud
// daemon then boots a Firecracker microVM, checks out the repo, runs ONE
// no-meter `/v1/chat/completions` under the minted bearer, and posts the EXACT
// owner-attributed `token_usage_events` receipt — the single billable row.
//
// FAIL-CLOSED. When the GCE microVM lane is NOT armed (`armed === false`, the
// production default), this consumer does NOTHING: it never reads the queue,
// never mints a token, never POSTs a placement. This mirrors the cloud-coding
// route's not-armed posture so PROD stays default-off until an operator arms
// staging/prod explicitly.
//
// TOKEN LIFECYCLE. The microVM turn runs ASYNCHRONOUSLY: the placement POST
// returns `provisioning` while the guest inference call happens seconds later.
// So the token is NOT revoked on a successful launch — it must outlive the
// guest call, bounded by its short TTL (step 1). It IS revoked immediately when
// the launch fails or the branch throws (the guest never runs). Revoking a
// successfully-launched turn's token after the turn completes is the completion
// sweep's job (call `revokeCloudRuntimeExecutionToken` with the returned
// `credentialId` once the lifecycle terminal is observed).
//
// EVENT STREAM. Each turn streams owner-attributed runtime events on a valid
// `KhalaRuntimeLane` (default `hosted_khala`) through the same sanctioned
// `executePush` + `runtime.recordEvent` path the hosted consumer uses:
// `turn.started` (the atomic claim) -> `text.delta`/`text.completed` (a
// public-safe placement status line, on a successful launch) -> `turn.finished`
// (`stop` on launch, `error` on refusal). The minted bearer NEVER appears in
// any event.

import { Cause, Effect, Exit, Option } from 'effect'

import {
  decodeKhalaRuntimeEvent,
  decodePushRequest,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeLane,
  type MutationResult,
} from '@openagentsinc/khala-sync'
import {
  executePush as executePushEngine,
  makeMutatorRegistry,
  runtimeMutators,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import type {
  CloudCodingRuntimeAdapter,
  CloudCodingSessionRequest,
} from './cloud/cloud-coding-session-routes'
import {
  buildCloudRuntimeInferenceConfig,
  buildCloudRuntimeWorkContext,
  buildCloudRuntimeWritebackConfig,
  encodeWorkContextB64,
} from './khala-cloud-runtime-inference-block'
import {
  mintCloudRuntimeExecutionToken,
  revokeCloudRuntimeExecutionToken,
  type MintExecutionTokenInput,
  type MintedExecutionToken,
} from './khala-cloud-runtime-execution-token'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

/** The CloudCodingLane marker this consumer owns. */
export const CLOUD_GCP_RUNTIME_LANE = 'cloud-gcp'

/** Default valid runtime-event lane stamped on the streamed events. */
export const CLOUD_GCP_RUNTIME_EVENT_LANE: KhalaRuntimeLane = 'hosted_khala'

/** Provider ref stamped on the streamed events' source. */
export const CLOUD_GCP_RUNTIME_PROVIDER_REF = 'openagents-agent-computer'

/** Base synthetic client group for this consumer's mutation ledger rows. */
export const CLOUD_GCP_RUNTIME_DISPATCH_CLIENT_GROUP_ID =
  'server.cloud-runtime-dispatch'

/**
 * Per-OWNER client group. A Khala Sync client group never migrates between
 * users, so a single constant group binds to the first owner and rejects every
 * other owner's dispatched turn. Scope it by owner.
 */
export const cloudGcpDispatchClientGroupIdForOwner = (
  ownerUserId: string,
): string => `${CLOUD_GCP_RUNTIME_DISPATCH_CLIENT_GROUP_ID}.${ownerUserId}`

/** Default per-tick turn budget. */
export const DEFAULT_CLOUD_GCP_RUNTIME_DISPATCH_LIMIT = 4

/** Default placement timeout (seconds). */
export const DEFAULT_CLOUD_GCP_RUNTIME_TIMEOUT_SECONDS = 1800

const refPart = (value: string): string => value.replace(/[^a-zA-Z0-9_.:-]/g, '_')

/** An admitted `cloud-gcp` work-context awaiting a microVM turn. */
export type CloudGcpAdmittedWorkContext = Readonly<{
  ownerUserId: string
  threadId: string
  turnId: string
  workContextRef: string
  repo: string
  commit: string
  branch?: string | undefined
  objective?: string | undefined
  repoBindingRef?: string | undefined
  /** Current turn `event_count` — the next event's `sequence`. */
  eventCount: number
  /** Valid runtime lane to stamp on the streamed events (default hosted_khala). */
  runtimeLane?: KhalaRuntimeLane | undefined
  /**
   * MM-C5 (#8477) branch/PR writeback. When present, the microVM pushes a
   * scoped branch (and optionally opens a PR) under the user's own GitHub
   * authorization after staging its change, then POSTs the outcome to the
   * Worker writeback route. Absent = no writeback (the proven money-path turn).
   * The block threaded into the work-context carries NO credential.
   */
  writeback?:
    | Readonly<{
        mode?: 'branch_only' | 'pull_request' | undefined
        branch?: string | undefined
        baseBranch?: string | undefined
      }>
    | undefined
}>

/** Placement launch result the injected launch seam resolves to. */
export type CloudGcpPlacementResult =
  | Readonly<{
      ok: true
      placementRef: string
      sessionId: string
      lifecycleReceiptRefs: ReadonlyArray<string>
      agentComputerState: string
    }>
  | Readonly<{ ok: false; reason: string }>

/** The injected placement launch seam (default wraps the cloud-control adapter). */
export type CloudGcpPlacementLaunchFn = (
  input: Readonly<{
    ownerUserId: string
    sessionId: string
    threadRef: string
    repoRef: string
    repoBindingRef?: string | undefined
    workContextRef: string
    objective: string
    workContextB64: string
    timeoutSeconds: number
  }>,
) => Promise<CloudGcpPlacementResult>

/** Config for building the microVM inference block. */
export type CloudGcpRuntimeInferenceSettings = Readonly<{
  baseUrl: string
  model: string
  lane?: string | undefined
  provider?: string | undefined
  backendProfile?: string | undefined
  pylonRef?: string | undefined
  noMeterSecret?: string | undefined
  ttlSeconds?: number | undefined
}>

export type CloudGcpMintFn = (
  sql: SyncSql,
  input: MintExecutionTokenInput,
) => Promise<MintedExecutionToken>

export type CloudGcpRevokeFn = (
  sql: SyncSql,
  input: Readonly<{ credentialId: string }>,
) => Promise<number>

export type CloudGcpRuntimeDispatchDependencies = Readonly<{
  sql: SyncSql
  /** GCE microVM lane armed? Fail-closed when false (production default). */
  armed: boolean
  /** Inference block settings baked into the microVM work-context. */
  inference: CloudGcpRuntimeInferenceSettings
  /** Placement launch seam (default provided by {@link makeCloudCodingAdapterLaunchSeam}). */
  launch: CloudGcpPlacementLaunchFn
  /** Reads admitted work-contexts (batch runner only). */
  readAdmitted?:
    | ((sql: SyncSql, limit: number) => Promise<ReadonlyArray<CloudGcpAdmittedWorkContext>>)
    | undefined
  /** Token mint seam (default the real mint). */
  mint?: CloudGcpMintFn | undefined
  /** Token revoke seam (default the real revoke). */
  revoke?: CloudGcpRevokeFn | undefined
  /** Push-engine seam (default the real engine). */
  executePush?: typeof executePushEngine | undefined
  /** Mutator registry (default the runtime mutators only). */
  registry?: MutatorRegistry | undefined
  /** Per-tick turn budget. */
  limit?: number | undefined
  now?: (() => string) | undefined
  uuid?: (() => string) | undefined
  log?: ((line: string, fields?: Record<string, unknown>) => void) | undefined
}>

type ResolvedDeps = Readonly<{
  sql: SyncSql
  armed: boolean
  inference: CloudGcpRuntimeInferenceSettings
  launch: CloudGcpPlacementLaunchFn
  mint: CloudGcpMintFn
  revoke: CloudGcpRevokeFn
  executePush: typeof executePushEngine
  registry: MutatorRegistry
  limit: number
  now: () => string
  uuid: () => string
  log: (line: string, fields?: Record<string, unknown>) => void
}>

const resolveDeps = (
  deps: CloudGcpRuntimeDispatchDependencies,
): ResolvedDeps => ({
  armed: deps.armed,
  executePush: deps.executePush ?? executePushEngine,
  inference: deps.inference,
  launch: deps.launch,
  limit:
    deps.limit !== undefined && Number.isSafeInteger(deps.limit) && deps.limit > 0
      ? deps.limit
      : DEFAULT_CLOUD_GCP_RUNTIME_DISPATCH_LIMIT,
  log: deps.log ?? (() => undefined),
  mint: deps.mint ?? mintCloudRuntimeExecutionToken,
  now: deps.now ?? currentIsoTimestamp,
  registry: deps.registry ?? makeMutatorRegistry([...runtimeMutators]),
  revoke: deps.revoke ?? revokeCloudRuntimeExecutionToken,
  sql: deps.sql,
  uuid: deps.uuid ?? randomUuid,
})

const eventSource = (deps: ResolvedDeps, lane: KhalaRuntimeLane) =>
  ({
    adapterKind: 'openagents_native' as const,
    lane,
    // The event `source.modelRef` is a display SAFE-REF (no `/`); the real
    // model id (which may contain `/`, e.g. `openagents/khala`) rides in the
    // inference block, not here.
    modelRef: refPart(deps.inference.model),
    providerRef: CLOUD_GCP_RUNTIME_PROVIDER_REF,
    surface: 'server' as const,
  })

const buildEvent = (
  deps: ResolvedDeps,
  turn: CloudGcpAdmittedWorkContext,
  sequence: number,
  extra: Record<string, unknown>,
): KhalaRuntimeEvent =>
  decodeKhalaRuntimeEvent({
    causalityRefs: [],
    eventId: deps.uuid(),
    observedAt: deps.now(),
    redactionClass: 'private_ref',
    schema: 'openagents.khala_runtime_event.v1',
    sequence,
    source: eventSource(deps, turn.runtimeLane ?? CLOUD_GCP_RUNTIME_EVENT_LANE),
    threadId: turn.threadId,
    turnId: turn.turnId,
    visibility: 'private',
    ...extra,
  })

/** Terminal outcome of dispatching one admitted `cloud-gcp` work-context. */
export type CloudGcpDispatchOutcome = Readonly<{
  outcome: 'launched' | 'failed' | 'skipped'
  /** Set when a token was minted (so a completion sweep can revoke it). */
  credentialId?: string
  /** Whether the token was already revoked in-band (failure/throw paths). */
  tokenRevoked: boolean
  placementRef?: string
  sessionId?: string
  reason?: string
}>

/**
 * Dispatch a single admitted `cloud-gcp` work-context end-to-end. NEVER throws
 * for an ordinary failure — it settles the turn (`turn.finished` error) and
 * revokes the minted token so the guest never runs with a live credential.
 */
export const dispatchCloudGcpRuntimeTurn = async (
  deps: CloudGcpRuntimeDispatchDependencies,
  turn: CloudGcpAdmittedWorkContext,
): Promise<CloudGcpDispatchOutcome> => {
  const resolved = resolveDeps(deps)
  const ownerId = turn.ownerUserId
  const clientGroupId = cloudGcpDispatchClientGroupIdForOwner(ownerId)
  const clientId = `${clientGroupId}.${turn.turnId}.${resolved.uuid()}`
  let seq = turn.eventCount

  const record = (mutationId: number, event: KhalaRuntimeEvent): Promise<MutationResult> =>
    resolved
      .executePush({
        registry: resolved.registry,
        request: decodePushRequest({
          clientGroupId,
          clientId,
          mutations: [
            {
              argsJson: JSON.stringify(event),
              mutationId,
              name: 'runtime.recordEvent',
            },
          ],
          protocolVersion: 1,
          schemaVersion: 1,
        }),
        sql: resolved.sql,
        userId: ownerId,
      })
      .then(response => {
        const result = response.results[0]
        if (result === undefined) {
          throw new Error('executePush returned no result for runtime.recordEvent')
        }
        return result
      })

  // 1. CLAIM: record turn.started (the atomic claim). Loser of a race skips.
  const startResult = await record(1, buildEvent(resolved, turn, seq, { kind: 'turn.started' }))
  if (startResult.status !== 'applied') {
    resolved.log('cloud_gcp_runtime_dispatch_claim_skipped', {
      errorCode: startResult.errorCode,
      turnId: turn.turnId,
    })
    return { outcome: 'skipped', tokenRevoked: false }
  }
  seq += 1

  // 2. Mint the short-lived owner-linked execution token. Everything from here
  // is wrapped so a throw still revokes it (the guest must never run with a
  // live credential we lost track of).
  let minted: MintedExecutionToken | undefined
  const messageId = `msg.${resolved.uuid()}`
  try {
    minted = await resolved.mint(resolved.sql, {
      ownerUserId: ownerId,
      ...(resolved.inference.ttlSeconds === undefined
        ? {}
        : { ttlSeconds: resolved.inference.ttlSeconds }),
    })

    // 3. Build the inference block + base64 work-context blob.
    const inferenceConfig = buildCloudRuntimeInferenceConfig({
      agentToken: minted.rawToken,
      baseUrl: resolved.inference.baseUrl,
      model: resolved.inference.model,
      ownerUserId: ownerId,
      ...(resolved.inference.lane === undefined ? {} : { lane: resolved.inference.lane }),
      ...(resolved.inference.provider === undefined
        ? {}
        : { provider: resolved.inference.provider }),
      ...(resolved.inference.backendProfile === undefined
        ? {}
        : { backendProfile: resolved.inference.backendProfile }),
      ...(resolved.inference.pylonRef === undefined
        ? {}
        : { pylonRef: resolved.inference.pylonRef }),
      ...(resolved.inference.noMeterSecret === undefined
        ? {}
        : { noMeterSecret: resolved.inference.noMeterSecret }),
    })
    const writebackConfig =
      turn.writeback === undefined
        ? undefined
        : buildCloudRuntimeWritebackConfig({
            repositoryFullName: turn.repo,
            turnId: turn.turnId,
            baseBranch: turn.branch ?? undefined,
            ...(turn.writeback.branch === undefined
              ? {}
              : { branch: turn.writeback.branch }),
            ...(turn.writeback.baseBranch === undefined
              ? {}
              : { baseBranch: turn.writeback.baseBranch }),
            ...(turn.writeback.mode === undefined
              ? {}
              : { mode: turn.writeback.mode }),
          })
    const workContext = buildCloudRuntimeWorkContext({
      commit: turn.commit,
      inference: inferenceConfig,
      repo: turn.repo,
      threadRef: turn.threadId,
      turnId: turn.turnId,
      workContextRef: turn.workContextRef,
      ...(turn.branch === undefined ? {} : { branch: turn.branch }),
      ...(turn.objective === undefined ? {} : { objective: turn.objective }),
      ...(writebackConfig === undefined ? {} : { writeback: writebackConfig }),
    })
    const workContextB64 = encodeWorkContextB64(workContext)

    // 4. POST the placement through the adapter (forwards work_context_b64).
    const sessionId = `ccs.${refPart(turn.turnId)}`
    const placement = await resolved.launch({
      objective: workContext.objective,
      ownerUserId: ownerId,
      repoRef: turn.repo,
      sessionId,
      threadRef: turn.threadId,
      timeoutSeconds: DEFAULT_CLOUD_GCP_RUNTIME_TIMEOUT_SECONDS,
      workContextB64,
      workContextRef: turn.workContextRef,
      ...(turn.repoBindingRef === undefined ? {} : { repoBindingRef: turn.repoBindingRef }),
    })

    if (!placement.ok) {
      // Launch refused: the guest never runs — revoke the token NOW.
      await record(
        2,
        buildEvent(resolved, turn, seq, {
          finishReason: 'error' satisfies KhalaRuntimeFinishReason,
          kind: 'turn.finished',
        }),
      )
      await resolved.revoke(resolved.sql, { credentialId: minted.credentialId })
      resolved.log('cloud_gcp_runtime_dispatch_launch_refused', {
        reason: placement.reason,
        turnId: turn.turnId,
      })
      return {
        credentialId: minted.credentialId,
        outcome: 'failed',
        reason: placement.reason,
        tokenRevoked: true,
      }
    }

    // 5. Launch accepted (provisioning). Stream a public-safe status line then
    // finish. The token is NOT revoked here — the microVM's inference call runs
    // asynchronously and needs the live bearer; its short TTL bounds it, and a
    // completion sweep revokes it once the lifecycle terminal is observed.
    const statusText =
      `Launched an OpenAgents Agent Computer microVM turn for ` +
      `${turn.repo}@${turn.commit.slice(0, 12)}.`
    await record(
      2,
      buildEvent(resolved, turn, seq, {
        chunkId: `chunk.${resolved.uuid()}`,
        kind: 'text.delta',
        messageId,
        text: statusText,
      }),
    )
    seq += 1
    await record(
      3,
      buildEvent(resolved, turn, seq, { kind: 'text.completed', messageId }),
    )
    seq += 1
    await record(
      4,
      buildEvent(resolved, turn, seq, {
        finishReason: 'stop' satisfies KhalaRuntimeFinishReason,
        kind: 'turn.finished',
      }),
    )
    resolved.log('cloud_gcp_runtime_dispatch_launched', {
      agentComputerState: placement.agentComputerState,
      placementRef: placement.placementRef,
      turnId: turn.turnId,
    })
    return {
      credentialId: minted.credentialId,
      outcome: 'launched',
      placementRef: placement.placementRef,
      sessionId: placement.sessionId,
      tokenRevoked: false,
    }
  } catch (error) {
    // Any thrown failure after mint MUST revoke the token so no live credential
    // is left dangling for a turn that will never run.
    let tokenRevoked = false
    if (minted !== undefined) {
      try {
        await resolved.revoke(resolved.sql, { credentialId: minted.credentialId })
        tokenRevoked = true
      } catch {
        tokenRevoked = false
      }
    }
    resolved.log('cloud_gcp_runtime_dispatch_threw', {
      detail: error instanceof Error ? error.message : 'unknown',
      turnId: turn.turnId,
    })
    return {
      ...(minted === undefined ? {} : { credentialId: minted.credentialId }),
      outcome: 'failed',
      reason: 'dispatch_threw',
      tokenRevoked,
    }
  }
}

export type CloudGcpRuntimeDispatchSummary = Readonly<{
  scanned: number
  launched: number
  failed: number
  skipped: number
}>

/**
 * One cron tick for the `cloud-gcp` lane. FAIL-CLOSED: when the lane is not
 * armed, does nothing (no queue read, no mint, no placement). Otherwise reads
 * up to `limit` admitted work-contexts and dispatches each, failure-isolated.
 */
export const runCloudGcpRuntimeDispatch = async (
  deps: CloudGcpRuntimeDispatchDependencies,
): Promise<CloudGcpRuntimeDispatchSummary> => {
  const resolved = resolveDeps(deps)
  if (!resolved.armed) {
    resolved.log('cloud_gcp_runtime_dispatch_not_armed', {})
    return { failed: 0, launched: 0, scanned: 0, skipped: 0 }
  }
  const read = deps.readAdmitted
  if (read === undefined) {
    return { failed: 0, launched: 0, scanned: 0, skipped: 0 }
  }
  const admitted = await read(resolved.sql, resolved.limit)
  let launched = 0
  let failed = 0
  let skipped = 0
  for (const turn of admitted) {
    try {
      const result = await dispatchCloudGcpRuntimeTurn(deps, turn)
      if (result.outcome === 'launched') launched += 1
      else if (result.outcome === 'failed') failed += 1
      else skipped += 1
    } catch (error) {
      failed += 1
      resolved.log('cloud_gcp_runtime_dispatch_batch_threw', {
        detail: error instanceof Error ? error.message : 'unknown',
        turnId: turn.turnId,
      })
    }
  }
  return { failed, launched, scanned: admitted.length, skipped }
}

// PRODUCTION LAUNCH SEAM --------------------------------------------------
// Bridge the injected {@link CloudGcpPlacementLaunchFn} to the real
// cloud-control adapter (`makeCloudControlCloudCodingAdapter`). Builds a
// `cloud-gcp` `CloudCodingSessionRequest` carrying `work_context_b64` in its
// options (which step 3's adapter forwards onto the /v1/placement POST) and
// runs the adapter's Effect, mapping success/typed-failure to the seam's
// result. Owner-attributed: the account ref defaults to `agent:<ownerUserId>`.

export type CloudCodingAdapterLaunchSeamConfig = Readonly<{
  /** Adapter (Codex vs claude_agent) for the placement request. Default 'codex'. */
  adapter?: 'codex' | 'claude_agent'
  /** Repo trust tier. Default 'private' (owner-owned repos). */
  repoTrustTier?: 'public' | 'private' | 'regulated'
  /** Map an ownerUserId to the placement account ref. Default `agent:<owner>`. */
  accountRefForOwner?: (ownerUserId: string) => string
}>

const defaultAccountRefForOwner = (ownerUserId: string): string =>
  `agent:${ownerUserId}`

/**
 * Wrap a live {@link CloudCodingRuntimeAdapter} as a
 * {@link CloudGcpPlacementLaunchFn}. Pure glue: no I/O of its own beyond
 * running the adapter's Effect; unit-testable with a fake adapter.
 */
export const makeCloudCodingAdapterLaunchSeam = (
  adapter: CloudCodingRuntimeAdapter,
  config: CloudCodingAdapterLaunchSeamConfig = {},
): CloudGcpPlacementLaunchFn => {
  const adapterKind = config.adapter ?? 'codex'
  const repoTrustTier = config.repoTrustTier ?? 'private'
  const accountRefForOwner =
    config.accountRefForOwner ?? defaultAccountRefForOwner
  return async input => {
    const request: CloudCodingSessionRequest = {
      adapter: adapterKind,
      lane: CLOUD_GCP_RUNTIME_LANE,
      objective: input.objective,
      options: { workContextB64: input.workContextB64 },
      repoRef: input.repoRef,
      repoTrustTier,
      timeoutSeconds: input.timeoutSeconds,
      verify: [],
      workContextRef: input.workContextRef,
      threadRef: input.threadRef,
      ...(input.repoBindingRef === undefined
        ? {}
        : { repoBindingRef: input.repoBindingRef }),
    }
    const exit = await Effect.runPromiseExit(
      adapter.launch({
        accountRef: accountRefForOwner(input.ownerUserId),
        lane: CLOUD_GCP_RUNTIME_LANE,
        request,
        sessionId: input.sessionId,
      }),
    )
    if (Exit.isSuccess(exit)) {
      const session = exit.value
      return {
        agentComputerState: session.agentComputerState,
        lifecycleReceiptRefs: session.lifecycleReceiptRefs,
        ok: true,
        placementRef:
          session.placementRef ?? `placement.cloud-coding.${session.sessionId}`,
        sessionId: session.sessionId,
      }
    }
    const failure = Cause.findErrorOption(exit.cause)
    const reason = Option.isSome(failure)
      ? failure.value.reason
      : 'cloud_placement_effect_failed'
    return { ok: false, reason }
  }
}
