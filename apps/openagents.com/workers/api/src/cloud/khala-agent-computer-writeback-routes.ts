// MM-C5 (#8477) executor ingest — the server route the Agent Computer executor
// calls to REPORT a branch/PR writeback outcome, closing the last wiring gap
// between the isolated Firecracker microVM (#8503) and the thread-scoped
// `writeback.recorded` runtime event.
//
// THE GAP THIS CLOSES. `publishKhalaAgentComputerWriteback`
// (`khala-agent-computer-writeback.ts`) is a fully-built, tested server function
// — the authorization gate + thread-scoped event recorder for the hosted /
// Agent Computer lane — but until this route it had NO caller. The runtime
// inside an Agent Computer (the #8473 org-cloud executor / Pylon publisher)
// pushes the branch / opens the PR under the user's brokered GitHub credential
// (#8475), then must tell OpenAgents the public-safe OUTCOME so the mobile
// Khala Sync client can tail a tappable link out of the private thread scope.
// This is that ingest endpoint. It mirrors the exact discipline of the sibling
// `khala-cloud-runtime-usage-routes.ts` (the #8473 exact-usage ingest): a
// registered-agent bearer, an owner-posting-authority gate, a bounded typed
// body, and authoritative Postgres through the standard KHALA_SYNC_DB client.
//
// AUTHORITY / SAFETY INVARIANTS (see apps/openagents.com/INVARIANTS.md ->
// "Khala Mobile Agent Computers"):
// - The caller is a registered programmatic agent (the executor identity). A
//   linked user-Pylon agent may only report a writeback for ITS OWN owner user
//   id; a cross-owner post is a typed 403 and never reaches the recorder.
// - The recorder itself re-checks the user's GitHub write authorization and the
//   turn ownership; this route never fabricates a success. A `failed` outcome
//   stays `failed`, and a permission-blocked success is recorded as an honest
//   `failed` `writeback.recorded` event, not a silent drop.
// - Refs only: no diff/patch bytes, no credentials, no local paths, no raw
//   OAuth token ever crosses this boundary.

import { Effect, Match as M, Schema as S } from 'effect'

import type { MutatorRegistry, SyncSql } from '@openagentsinc/khala-sync-server'

import {
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from '../agent-registration'
import { readAgentBearerToken as bearerTokenFromRequest } from '../auth/bearer-token'
import type { GitHubWriteRepository } from '../github-write-connections'
import type { KhalaIdentityWriteAuthority } from './khala-agent-computer-writeback'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from '../http/responses'
import { decodeUnknownWithSchema, parseJsonUnknown } from '../json-boundary'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  KhalaAgentComputerWritebackOutcome,
  type KhalaAgentComputerWritebackPublishDependencies,
  type KhalaAgentComputerWritebackPublishResult,
  publishKhalaAgentComputerWriteback,
} from './khala-agent-computer-writeback'

type HttpResponse = globalThis.Response

export const KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH =
  '/api/khala/cloud/runtime-turn-writeback'

export const KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION =
  'openagents.khala_agent_computer_writeback.v1' as const

export const KHALA_AGENT_COMPUTER_WRITEBACK_RESULT_SCHEMA_VERSION =
  'openagents.khala_agent_computer_writeback_result.v1' as const

const MAX_BODY_BYTES = 64 * 1024

const NonEmptyString = S.Trim.check(S.isMinLength(1), S.isMaxLength(512))

// The typed executor -> Worker ingest body. `outcome` reuses the exact
// public-safe outcome contract the recorder validates, so shape rules
// (a `failed` outcome must carry a reasonRef, a PR outcome must carry a PR url,
// etc.) are enforced once, in the recorder.
class KhalaAgentComputerWritebackIngestBody extends S.Class<KhalaAgentComputerWritebackIngestBody>(
  'KhalaAgentComputerWritebackIngestBody',
)({
  schemaVersion: S.Literal(KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION),
  ownerUserId: NonEmptyString,
  turnId: NonEmptyString,
  outcome: KhalaAgentComputerWritebackOutcome,
}) {}

export type KhalaAgentComputerWritebackIngest =
  typeof KhalaAgentComputerWritebackIngestBody.Type

// Injectable publish seam so route tests exercise the full route -> recorder
// path against fakes (a fake SyncSql, a fake GitHub write repository, and a
// recording executePush) without a live database. Defaults to the real
// `publishKhalaAgentComputerWriteback`.
export type KhalaAgentComputerWritebackPublishFn = (
  deps: KhalaAgentComputerWritebackPublishDependencies,
  input: Readonly<{
    turnId: string
    userId: string
    outcome: KhalaAgentComputerWritebackOutcome
  }>,
) => Promise<KhalaAgentComputerWritebackPublishResult>

export type KhalaAgentComputerWritebackDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  githubWriteRepository: (env: Bindings) => GitHubWriteRepository
  /**
   * Optional brokerable github-IDENTITY authority (the push's real credential
   * source). When wired, the success gate accepts a usable identity
   * authorization as an alternative to an explicit write-connection row.
   */
  identityWriteAuthority?: (env: Bindings) => KhalaIdentityWriteAuthority
  binding: (env: Bindings) => KhalaSyncHyperdriveBinding | undefined
  registry: MutatorRegistry
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  publish?: KhalaAgentComputerWritebackPublishFn | undefined
  nowIso?: (() => string) | undefined
}>

class WritebackUnauthorized extends S.TaggedErrorClass<WritebackUnauthorized>()(
  'WritebackUnauthorized',
  {},
) {}

class WritebackForbidden extends S.TaggedErrorClass<WritebackForbidden>()(
  'WritebackForbidden',
  { reason: S.String },
) {}

class WritebackValidationError extends S.TaggedErrorClass<WritebackValidationError>()(
  'WritebackValidationError',
  { reason: S.String },
) {}

class WritebackStorageUnconfigured extends S.TaggedErrorClass<WritebackStorageUnconfigured>()(
  'WritebackStorageUnconfigured',
  {},
) {}

class WritebackStorageError extends S.TaggedErrorClass<WritebackStorageError>()(
  'WritebackStorageError',
  { reason: S.String },
) {}

type WritebackRouteError =
  | WritebackForbidden
  | WritebackStorageError
  | WritebackStorageUnconfigured
  | WritebackUnauthorized
  | WritebackValidationError

const routeErrorResponse = (error: WritebackRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      WritebackUnauthorized: () => unauthorized(),
      WritebackForbidden: err =>
        noStoreJsonResponse(
          { error: 'khala_agent_computer_writeback_forbidden', reason: err.reason },
          { status: 403 },
        ),
      WritebackValidationError: err =>
        noStoreJsonResponse(
          {
            error: 'khala_agent_computer_writeback_validation_error',
            reason: err.reason,
          },
          { status: 400 },
        ),
      WritebackStorageUnconfigured: () =>
        noStoreJsonResponse(
          {
            error: 'khala_agent_computer_writeback_storage_unconfigured',
            reason: 'KHALA_SYNC_DB is not configured for this environment.',
          },
          { status: 503 },
        ),
      WritebackStorageError: err =>
        noStoreJsonResponse(
          {
            error: 'khala_agent_computer_writeback_storage_error',
            reason: err.reason,
          },
          { status: 503 },
        ),
    }),
    M.exhaustive,
  )

const requireAgent = <Bindings>(
  dependencies: KhalaAgentComputerWritebackDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, WritebackUnauthorized> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) {
    return Effect.fail(new WritebackUnauthorized({}))
  }
  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new WritebackUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new WritebackUnauthorized({}))
        : Effect.succeed(session),
  )
}

// A linked user-Pylon agent may only report a writeback for its OWN owner user
// id (parity with the usage route's owner-posting-authority gate). An agent with
// no linked openauth owner (a first-party org executor) is not narrowed here;
// the recorder still enforces per-turn ownership.
const requireOwnerPostingAuthority = (
  session: ProgrammaticAgentSession,
  body: KhalaAgentComputerWritebackIngestBody,
): Effect.Effect<void, WritebackForbidden> => {
  const linkedOwner = session.credential.openauthUserId?.trim()
  if (
    linkedOwner !== undefined &&
    linkedOwner !== '' &&
    linkedOwner !== body.ownerUserId
  ) {
    return Effect.fail(
      new WritebackForbidden({
        reason:
          'linked user-pylon agents may only report a writeback for their own owner user id',
      }),
    )
  }
  return Effect.void
}

// HTTP status for each typed publish decision. `recorded` and
// `permission_blocked` both produced an honest thread event, so both are 200
// (the body's `decision`/`status` carry the real result); the rest map to the
// standard client/error codes.
const publishResultResponse = (
  result: KhalaAgentComputerWritebackPublishResult,
  body: KhalaAgentComputerWritebackIngestBody,
): HttpResponse => {
  const base = {
    schemaVersion: KHALA_AGENT_COMPUTER_WRITEBACK_RESULT_SCHEMA_VERSION,
    ownerUserId: body.ownerUserId,
    turnId: body.turnId,
  }
  if (result.ok) {
    return noStoreJsonResponse({
      ...base,
      ok: true,
      decision: result.decision,
      status: result.status,
      eventId: result.eventId,
      sequence: result.sequence,
      threadId: result.threadId,
    })
  }
  if (result.decision === 'permission_blocked') {
    return noStoreJsonResponse({
      ...base,
      ok: false,
      decision: result.decision,
      reason: result.reason,
      message: result.message,
      recordedEventId: result.recordedEventId,
    })
  }
  const status =
    result.decision === 'turn_not_found'
      ? 404
      : result.decision === 'owner_mismatch'
        ? 403
        : result.decision === 'outcome_invalid'
          ? 400
          : 409
  return noStoreJsonResponse(
    {
      ...base,
      ok: false,
      decision: result.decision,
      detail: result.detail,
    },
    { status },
  )
}

const routeWritebackIngest = <Bindings>(
  dependencies: KhalaAgentComputerWritebackDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, WritebackRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new WritebackValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new WritebackValidationError({
        reason: `Agent Computer writeback payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new WritebackValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Agent Computer writeback schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          KhalaAgentComputerWritebackIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* requireOwnerPostingAuthority(session, body)

    const binding = dependencies.binding(env)
    if (
      binding === undefined ||
      typeof binding.connectionString !== 'string' ||
      binding.connectionString.length === 0
    ) {
      return yield* new WritebackStorageUnconfigured({})
    }

    const publish = dependencies.publish ?? publishKhalaAgentComputerWriteback
    const makeSqlClient =
      dependencies.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
    const githubWriteRepository = dependencies.githubWriteRepository(env)
    const identityWriteAuthority = dependencies.identityWriteAuthority?.(env)

    const result = yield* Effect.tryPromise({
      catch: error =>
        new WritebackStorageError({
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: async () => {
        const client = await makeSqlClient(binding.connectionString)
        try {
          return await publish(
            {
              githubWriteRepository,
              ...(identityWriteAuthority === undefined
                ? {}
                : { identityWriteAuthority }),
              registry: dependencies.registry,
              sql: client.sql as SyncSql,
            },
            {
              outcome: body.outcome,
              turnId: body.turnId,
              userId: body.ownerUserId,
            },
          )
        } finally {
          await client.end()
        }
      },
    })

    return publishResultResponse(result, body)
  })

export const makeKhalaAgentComputerWritebackRoutes = <Bindings>(
  dependencies: KhalaAgentComputerWritebackDependencies<Bindings>,
) => ({
  handleKhalaAgentComputerWritebackIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (
      new URL(request.url).pathname !==
      KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH
    ) {
      return Effect.succeed(
        noStoreJsonResponse({ error: 'not_found' }, { status: 404 }),
      )
    }
    return routeWritebackIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})

export const nowIsoForWriteback = (): string => currentIsoTimestamp()
