// MM-C5 (#8477): Branch/PR writeback via the user's own GitHub authorization.
//
// THE GAP THIS CLOSES. When a Khala Code coding task runs on an OpenAgents
// Agent Computer (Firecracker microVM, the mobile-only MVP lane in #8467), the
// actual branch push / PR open happens INSIDE that microVM using the scoped
// GitHub user credential brokered by `/api/pylon/github/git-credentials`
// (#8475) — never a bot account. The Pylon path (`apps/pylon/codex-pr-publisher`)
// already covers contributor-owned runners. What was missing on the SERVER side
// for the hosted / Agent Computer lane is:
//
//   1. A user-authorization GATE that says, server-side and BEFORE we surface a
//      writeback as a success, whether this user has actually authorized repo
//      write (a connected, healthy GitHub write connection carrying the
//      required `repo`/`workflow` scopes). Missing/insufficient authorization is
//      a TYPED, public-safe failure — never a silent success.
//
//   2. RECORDING the branch/PR link as a thread-scoped `writeback.recorded`
//      runtime event, so the mobile Khala Sync client tails it out of the
//      private thread scope and renders a tappable link (and MM-G2 can notify
//      on it). This reuses the exact `executePush` + `runtime.recordEvent`
//      discipline the hosted-runtime dispatch consumer uses.
//
// AUTHORITY / SAFETY INVARIANTS (see apps/openagents.com/INVARIANTS.md ->
// "Khala Mobile Agent Computers"):
// - The writeback runs under the USER's brokered GitHub authorization, scoped
//   to the least-privilege grant they gave. This module never mints, reads, or
//   surfaces a raw OAuth token/PAT; it only reads connection health/scope refs.
// - Never force-push and never write the base branch. That rule is enforced in
//   the executor (agent computer / Pylon publisher). This server surface never
//   fabricates a success: it records exactly the reported outcome, and a
//   `failed` outcome stays `failed`.
// - Branch/PR links live only in thread-scoped runtime-event metadata. Nothing
//   here writes a public scope.
//
// STORAGE: authoritative Postgres only, through the same transaction-mode-safe
// `SyncSql` client discipline the push route uses. Reads are bounded single
// statements; every mutation goes through the sanctioned `executePush` engine
// so the changelog append and mutation ledger stay consistent.

import { Schema as S } from 'effect'

import { KhalaRuntimeWritebackStatus } from '@openagentsinc/agent-runtime-schema'
import {
  decodeKhalaRuntimeEvent,
  decodePushRequest,
  type KhalaRuntimeEvent,
  type MutationResult,
} from '@openagentsinc/khala-sync'
import {
  executePush as executePushEngine,
  makeMutatorRegistry,
  runtimeMutators,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  GITHUB_WRITE_REQUIRED_SCOPES,
  hasRequiredGitHubWriteScopes,
  type GitHubWriteRepository,
} from '../github-write-connections'
import { currentIsoTimestamp, randomUuid } from '../runtime-primitives'

/** Runtime lane stamped on Agent Computer writeback events. */
export const KHALA_WRITEBACK_LANE = 'ai_sdk_harness_sandbox' as const

/** Adapter kind stamped on the event source (a hosted Firecracker container). */
export const KHALA_WRITEBACK_ADAPTER_KIND = 'hosted_container' as const

/** Provider ref stamped on the event source (public-safe). */
export const KHALA_WRITEBACK_PROVIDER_REF = 'openagents-agent-computer'

/** Base synthetic client group for this recorder's mutation-ledger rows. */
export const KHALA_WRITEBACK_DISPATCH_CLIENT_GROUP_ID =
  'server.agent-computer-writeback'

/**
 * Per-OWNER client group. A Khala Sync client group never migrates between
 * users (the mutation ledger rejects a cross-user reuse), so a single constant
 * group would bind to the FIRST owner and reject every other owner's writeback.
 * Scoping the group by owner keeps each owner's server-side recorder its own
 * stable client group.
 */
export const writebackClientGroupIdForOwner = (ownerUserId: string): string =>
  `${KHALA_WRITEBACK_DISPATCH_CLIENT_GROUP_ID}.${ownerUserId}`

// AUTHORIZATION ------------------------------------------------------------

export const KhalaWritebackAuthorizationBlockedReason = S.Literals([
  'github_write_connection_required',
  'github_write_connection_unusable',
  'github_write_permission_missing',
])
export type KhalaWritebackAuthorizationBlockedReason =
  typeof KhalaWritebackAuthorizationBlockedReason.Type

const MISSING_CONNECTION_MESSAGE =
  'Connect a GitHub account with repository write access before OpenAgents can push a branch or open a pull request for you.'

const UNUSABLE_CONNECTION_MESSAGE =
  'Your linked GitHub write account needs to be reconnected before OpenAgents can write to your repository.'

const MISSING_SCOPES_MESSAGE =
  'Re-authorize GitHub with repository and workflow permissions before OpenAgents can push a branch or open a pull request for you.'

/** Public-safe reason ref recorded on a permission-blocked writeback event. */
export const writebackPermissionReasonRef = (
  reason: KhalaWritebackAuthorizationBlockedReason,
): string => `writeback.permission.${reason}`

export type KhalaWritebackAuthorizationSource =
  | 'github_write_connection'
  | 'github_identity'

export type KhalaWritebackAuthorization =
  | Readonly<{
      authorized: true
      source: KhalaWritebackAuthorizationSource
      connectionRef: string
      scopes: ReadonlyArray<string>
    }>
  | Readonly<{
      authorized: false
      reason: KhalaWritebackAuthorizationBlockedReason
      message: string
    }>

/**
 * The user's brokerable GitHub-IDENTITY authorization (the credential the
 * in-guest push actually uses via the SCM auth broker, #8475). `usable` means a
 * `repo`-capable identity token is present for the user in the environment the
 * broker reads. This never reads or returns the raw token — only presence.
 *
 * WHY THIS EXISTS. The writeback recorder runs AFTER the microVM has already
 * pushed under the brokered identity credential; the actual repo-scope + write
 * access is enforced at push time by the broker + GitHub (fail-closed — a
 * scope-lacking identity yields a typed `failed` push, never a real branch). So
 * a reported SUCCESS outcome PROVES the user had a working brokered
 * authorization. Gating that success ONLY on the separate
 * `github_write_connections` table caused the seam bug this closes: a real
 * pushed branch was recorded as `failed` when the user authorized via the
 * identity/broker path but had no explicit write-connection row. The gate now
 * accepts EITHER authoritative source.
 */
export type KhalaIdentityWriteAuthority = Readonly<{
  hasUsableIdentityAuthorization: (userId: string) => Promise<boolean>
}>

const MISSING_AUTHORIZATION_MESSAGE =
  'Connect your GitHub account (with repository access) before OpenAgents can push a branch or open a pull request for you.'

/**
 * Resolve whether `userId` has authorized repo write under their OWN GitHub
 * identity, accepting EITHER authoritative source:
 *  1. an explicit `github_write_connections` row (connected, healthy, stored
 *     secret ref, `repo`/`workflow` scopes), OR
 *  2. a usable brokerable github-IDENTITY authorization (the credential the
 *     in-guest push uses; presence in the broker's environment).
 *
 * Anything less is a typed, public-safe block — never an implicit allow. This
 * reads only connection health/scope refs and identity-token PRESENCE; it never
 * reads or returns a raw OAuth token.
 */
export const resolveKhalaWritebackAuthorization = async (
  repository: GitHubWriteRepository,
  userId: string,
  identityAuthority?: KhalaIdentityWriteAuthority,
): Promise<KhalaWritebackAuthorization> => {
  const connection = await repository.findUsableConnectionForUser(userId)
  const connectionUsable =
    connection !== undefined &&
    connection.status === 'connected' &&
    connection.health === 'healthy' &&
    connection.secretRef !== null &&
    hasRequiredGitHubWriteScopes(connection.scopes)
  if (connectionUsable) {
    return {
      authorized: true,
      connectionRef: connection.connectionRef,
      scopes: connection.scopes,
      source: 'github_write_connection',
    }
  }

  // Fall back to the brokerable identity authorization (the push's real
  // credential). Presence is sufficient here; the broker + GitHub enforce the
  // actual repo-scope/access at push time (fail-closed).
  if (identityAuthority !== undefined) {
    const identityUsable = await identityAuthority
      .hasUsableIdentityAuthorization(userId)
      .catch(() => false)
    if (identityUsable) {
      return {
        authorized: true,
        connectionRef: `github-identity:${userId}`,
        scopes: [...GITHUB_WRITE_REQUIRED_SCOPES],
        source: 'github_identity',
      }
    }
  }

  // Neither source is usable — return the most specific typed block.
  if (connection === undefined) {
    return {
      authorized: false,
      message: identityAuthority === undefined
        ? MISSING_CONNECTION_MESSAGE
        : MISSING_AUTHORIZATION_MESSAGE,
      reason: 'github_write_connection_required',
    }
  }
  if (
    connection.status !== 'connected' ||
    connection.health !== 'healthy' ||
    connection.secretRef === null
  ) {
    return {
      authorized: false,
      message: UNUSABLE_CONNECTION_MESSAGE,
      reason: 'github_write_connection_unusable',
    }
  }
  return {
    authorized: false,
    message: MISSING_SCOPES_MESSAGE,
    reason: 'github_write_permission_missing',
  }
}

// WRITEBACK OUTCOME --------------------------------------------------------

/**
 * Public-safe writeback outcome reported by the Agent Computer executor after
 * it has (or has not) pushed a branch / opened a PR under the user's brokered
 * GitHub authorization. Refs only: no diff/patch bytes, no credentials, no
 * local paths.
 */
export const KhalaAgentComputerWritebackOutcome = S.Struct({
  repositoryFullName: S.String,
  branch: S.String,
  branchUrl: S.String,
  status: KhalaRuntimeWritebackStatus,
  changedFileCount: S.optional(S.Number),
  pullRequestUrl: S.optional(S.String),
  pullRequestNumber: S.optional(S.Number),
  reasonRef: S.optional(S.String),
})
export type KhalaAgentComputerWritebackOutcome =
  typeof KhalaAgentComputerWritebackOutcome.Type

export const decodeKhalaAgentComputerWritebackOutcome = S.decodeUnknownSync(
  KhalaAgentComputerWritebackOutcome,
)

export type WritebackOutcomeShapeError = Readonly<{
  ok: false
  reason: 'writeback_outcome_shape_invalid'
  detail: string
}>

/**
 * Validate the reported outcome is internally consistent so we never surface a
 * misleading link:
 * - `pull_request_opened` / `pull_request_reused` must carry a PR url.
 * - `branch_pushed` / `failed` must NOT carry PR fields.
 * - `failed` must carry a `reasonRef`; a non-failed outcome must not.
 */
export const validateWritebackOutcomeShape = (
  outcome: KhalaAgentComputerWritebackOutcome,
): WritebackOutcomeShapeError | undefined => {
  const invalid = (detail: string): WritebackOutcomeShapeError => ({
    detail,
    ok: false,
    reason: 'writeback_outcome_shape_invalid',
  })
  const hasPrFields =
    outcome.pullRequestUrl !== undefined ||
    outcome.pullRequestNumber !== undefined
  if (
    outcome.status === 'pull_request_opened' ||
    outcome.status === 'pull_request_reused'
  ) {
    if (
      outcome.pullRequestUrl === undefined ||
      outcome.pullRequestUrl.length === 0
    ) {
      return invalid(`${outcome.status} outcome is missing pullRequestUrl`)
    }
  }
  if (outcome.status === 'branch_pushed' && hasPrFields) {
    return invalid('branch_pushed outcome must not carry pull-request fields')
  }
  if (outcome.status === 'failed') {
    if (hasPrFields) {
      return invalid('failed outcome must not carry pull-request fields')
    }
    if (outcome.reasonRef === undefined || outcome.reasonRef.length === 0) {
      return invalid('failed outcome must carry a reasonRef')
    }
  } else if (outcome.reasonRef !== undefined) {
    return invalid(`${outcome.status} outcome must not carry a reasonRef`)
  }
  return undefined
}

// THREAD-SCOPED EVENT RECORDING --------------------------------------------

/** The target runtime turn a writeback event is appended to. */
export type WritebackTargetTurn = Readonly<{
  turnId: string
  threadId: string
  ownerUserId: string
  eventCount: number
}>

type RuntimeTurnRow = Readonly<{
  turn_id: string
  thread_id: string
  owner_user_id: string
  event_count: string | number
}>

/**
 * Read the target turn's owner, thread, and current event count (the next
 * writeback event's `sequence`). `null` when the turn does not exist.
 */
export const readWritebackTargetTurn = async (
  sql: SyncSql,
  turnId: string,
): Promise<WritebackTargetTurn | null> => {
  const rows: Array<RuntimeTurnRow> = await sql`
    SELECT turn_id, thread_id, owner_user_id, event_count
    FROM khala_sync_runtime_turns
    WHERE turn_id = ${turnId}
    LIMIT 1
  `
  const row = rows[0]
  if (row === undefined) return null
  return {
    eventCount: Number(row.event_count),
    ownerUserId: row.owner_user_id,
    threadId: row.thread_id,
    turnId: row.turn_id,
  }
}

export type KhalaWritebackRecordingDependencies = Readonly<{
  /** Root Postgres handle (transaction-mode-safe client). */
  sql: SyncSql
  /** Mutator registry (default: the runtime mutators only). */
  registry?: MutatorRegistry | undefined
  /** Push-engine seam (default: the real engine). */
  executePush?: typeof executePushEngine | undefined
  /** Clock (default: real wall clock). */
  now?: (() => string) | undefined
  /** Id generator (default: `crypto.randomUUID`). */
  uuid?: (() => string) | undefined
}>

type ResolvedRecordingDeps = Readonly<{
  sql: SyncSql
  registry: MutatorRegistry
  executePush: typeof executePushEngine
  now: () => string
  uuid: () => string
}>

const resolveRecordingDeps = (
  deps: KhalaWritebackRecordingDependencies,
): ResolvedRecordingDeps => ({
  executePush: deps.executePush ?? executePushEngine,
  now: deps.now ?? currentIsoTimestamp,
  registry: deps.registry ?? makeMutatorRegistry([...runtimeMutators]),
  sql: deps.sql,
  uuid: deps.uuid ?? randomUuid,
})

const writebackEventSource = () =>
  ({
    adapterKind: KHALA_WRITEBACK_ADAPTER_KIND,
    lane: KHALA_WRITEBACK_LANE,
    providerRef: KHALA_WRITEBACK_PROVIDER_REF,
    surface: 'server' as const,
  })

/** Build the thread-scoped `writeback.recorded` runtime event. */
export const buildWritebackRuntimeEvent = (
  deps: ResolvedRecordingDeps,
  turn: WritebackTargetTurn,
  outcome: KhalaAgentComputerWritebackOutcome,
): KhalaRuntimeEvent => {
  const token = deps.uuid()
  return decodeKhalaRuntimeEvent({
    branch: outcome.branch,
    branchUrl: outcome.branchUrl,
    causalityRefs: [],
    ...(outcome.changedFileCount === undefined
      ? {}
      : { changedFileCount: outcome.changedFileCount }),
    eventId: `event.private.agent_computer.writeback.${token}`,
    kind: 'writeback.recorded',
    observedAt: deps.now(),
    ...(outcome.pullRequestNumber === undefined
      ? {}
      : { pullRequestNumber: outcome.pullRequestNumber }),
    ...(outcome.pullRequestUrl === undefined
      ? {}
      : { pullRequestUrl: outcome.pullRequestUrl }),
    ...(outcome.reasonRef === undefined ? {} : { reasonRef: outcome.reasonRef }),
    redactionClass: 'private_ref',
    repositoryFullName: outcome.repositoryFullName,
    schema: 'openagents.khala_runtime_event.v1',
    sequence: turn.eventCount,
    source: writebackEventSource(),
    status: outcome.status,
    threadId: turn.threadId,
    turnId: turn.turnId,
    visibility: 'private',
    writebackRef: `writeback.private.agent_computer.${token}`,
  })
}

export type KhalaWritebackRecordResult =
  | Readonly<{
      ok: true
      eventId: string
      sequence: number
      status: KhalaRuntimeWritebackStatus
      threadId: string
      ownerUserId: string
    }>
  | Readonly<{
      ok: false
      reason: 'record_rejected'
      detail: string
    }>

/**
 * Record one `writeback.recorded` event into the target turn's private thread
 * scope. The event is recorded AS THE TURN OWNER: `runtime.recordEvent`
 * resolves thread-scope ownership from `ctx.userId`, which `executePush` takes
 * as `userId`. `(turn_id, sequence)` is the dedupe key, so a duplicated record
 * attempt is rejected in-band and surfaced as a typed failure.
 */
export const recordKhalaWritebackRuntimeEvent = async (
  deps: KhalaWritebackRecordingDependencies,
  turn: WritebackTargetTurn,
  outcome: KhalaAgentComputerWritebackOutcome,
): Promise<KhalaWritebackRecordResult> => {
  const resolved = resolveRecordingDeps(deps)
  const event = buildWritebackRuntimeEvent(resolved, turn, outcome)
  const clientGroupId = writebackClientGroupIdForOwner(turn.ownerUserId)
  const clientId = `${clientGroupId}.${turn.turnId}.${resolved.uuid()}`

  const response = await resolved.executePush({
    registry: resolved.registry,
    request: decodePushRequest({
      clientGroupId,
      clientId,
      mutations: [
        {
          argsJson: JSON.stringify(event),
          mutationId: 1,
          name: 'runtime.recordEvent',
        },
      ],
      protocolVersion: 1,
      schemaVersion: 1,
    }),
    sql: resolved.sql,
    userId: turn.ownerUserId,
  })
  const result: MutationResult | undefined = response.results[0]
  if (result === undefined || result.status !== 'applied') {
    return {
      detail: result?.errorCode ?? 'executePush returned no result',
      ok: false,
      reason: 'record_rejected',
    }
  }
  return {
    eventId: event.eventId,
    ok: true,
    ownerUserId: turn.ownerUserId,
    sequence: turn.eventCount,
    status: outcome.status,
    threadId: turn.threadId,
  }
}

// ORCHESTRATION ------------------------------------------------------------

export type KhalaAgentComputerWritebackPublishInput = Readonly<{
  /** The runtime turn the coding task belongs to. */
  turnId: string
  /** The user who owns the thread AND whose GitHub authorization must be used. */
  userId: string
  /** The executor-reported writeback outcome. */
  outcome: KhalaAgentComputerWritebackOutcome
}>

export type KhalaAgentComputerWritebackPublishDependencies =
  KhalaWritebackRecordingDependencies &
    Readonly<{
      /** User GitHub write-connection authority (for the authorization gate). */
      githubWriteRepository: GitHubWriteRepository
      /**
       * Brokerable github-IDENTITY authority (the credential the in-guest push
       * uses). When present, the gate accepts a usable identity authorization
       * as an alternative to an explicit write-connection row (seam alignment).
       */
      identityWriteAuthority?: KhalaIdentityWriteAuthority | undefined
    }>

export type KhalaAgentComputerWritebackPublishResult =
  | Readonly<{
      ok: true
      decision: 'recorded'
      status: KhalaRuntimeWritebackStatus
      eventId: string
      sequence: number
      threadId: string
    }>
  | Readonly<{
      ok: false
      decision: 'permission_blocked'
      reason: KhalaWritebackAuthorizationBlockedReason
      message: string
      /** The failed writeback event surfaced to the thread (if recorded). */
      recordedEventId: string | null
    }>
  | Readonly<{
      ok: false
      decision: 'turn_not_found' | 'owner_mismatch' | 'outcome_invalid' | 'record_rejected'
      detail: string
    }>

/**
 * End-to-end server-owned writeback publish for the Agent Computer lane:
 *
 *  1. Resolve the target turn; reject an unknown turn or an owner mismatch.
 *  2. Validate the reported outcome shape.
 *  3. AUTHORIZATION GATE — for a success outcome, require the user's own GitHub
 *     write authorization (`repo`/`workflow`). If it is missing/insufficient we
 *     DO NOT surface a success; instead we record a typed `failed`
 *     `writeback.recorded` event carrying a public-safe permission reason ref
 *     so the mobile thread shows an honest "authorize GitHub" state, and return
 *     `permission_blocked`. A `failed` outcome skips the gate (it already
 *     failed in the executor) and is recorded as reported.
 *  4. Record the `writeback.recorded` event into the private thread scope.
 */
export const publishKhalaAgentComputerWriteback = async (
  deps: KhalaAgentComputerWritebackPublishDependencies,
  input: KhalaAgentComputerWritebackPublishInput,
): Promise<KhalaAgentComputerWritebackPublishResult> => {
  const shapeError = validateWritebackOutcomeShape(input.outcome)
  if (shapeError !== undefined) {
    return {
      decision: 'outcome_invalid',
      detail: shapeError.detail,
      ok: false,
    }
  }

  const turn = await readWritebackTargetTurn(deps.sql, input.turnId)
  if (turn === null) {
    return {
      decision: 'turn_not_found',
      detail: `no runtime turn ${input.turnId}`,
      ok: false,
    }
  }
  if (turn.ownerUserId !== input.userId) {
    return {
      decision: 'owner_mismatch',
      detail: 'writeback user does not own the target turn',
      ok: false,
    }
  }

  if (input.outcome.status !== 'failed') {
    const authorization = await resolveKhalaWritebackAuthorization(
      deps.githubWriteRepository,
      input.userId,
      deps.identityWriteAuthority,
    )
    if (!authorization.authorized) {
      const blockedOutcome: KhalaAgentComputerWritebackOutcome = {
        branch: input.outcome.branch,
        branchUrl: input.outcome.branchUrl,
        repositoryFullName: input.outcome.repositoryFullName,
        reasonRef: writebackPermissionReasonRef(authorization.reason),
        status: 'failed',
      }
      const recorded = await recordKhalaWritebackRuntimeEvent(
        deps,
        turn,
        blockedOutcome,
      )
      return {
        decision: 'permission_blocked',
        message: authorization.message,
        ok: false,
        reason: authorization.reason,
        recordedEventId: recorded.ok ? recorded.eventId : null,
      }
    }
  }

  const recorded = await recordKhalaWritebackRuntimeEvent(
    deps,
    turn,
    input.outcome,
  )
  if (!recorded.ok) {
    return {
      decision: 'record_rejected',
      detail: recorded.detail,
      ok: false,
    }
  }
  return {
    decision: 'recorded',
    eventId: recorded.eventId,
    ok: true,
    sequence: recorded.sequence,
    status: recorded.status,
    threadId: recorded.threadId,
  }
}
