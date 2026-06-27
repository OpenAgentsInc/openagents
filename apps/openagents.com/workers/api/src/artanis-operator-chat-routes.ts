// Owner-auth endpoint for the Artanis operator chat channel (issue #6363).
//
//   POST /api/operator/artanis/chat   body { messages: [{role,content}] }
//                                      -> { reply, servedVia, servedModel, ... }
//
// This is the OWNER-ONLY, PRIVATE channel to the REAL Artanis operator agent. It
// is gated exactly like the other `/api/operator/*` routes (admin API token OR a
// browser session whose email is an OpenAgents admin), and it routes the
// conversation through the Artanis operator CORE (`artanis-operator.ts`), which:
//   - uses the Artanis OPERATOR persona (NOT the public Khala collective
//     identity — no roleplay),
//   - is powered ONLY by the Khala API (the injected `khalaClient` dogfoods
//     `openagents/khala`; the reasoning is metered as Khala usage),
//   - injects owner memory + live situational awareness so Artanis answers
//     grounded in real state.
//
// Grounding uses the REAL lanes:
//   - memory   : `artanis-owner-memory.ts` (`makeD1ArtanisOwnerMemoryStore`,
//                `loadArtanisMemory`, `appendArtanisMemory`) — owner-scoped D1.
//   - awareness: `artanis-situational-awareness.ts`
//                (`buildArtanisSituationalAwareness`) — bounded, owner-scoped,
//                honest-absence over fabrication.
//
// The route owns persistence: it appends the latest owner message and Artanis's
// reply to the owner-scoped memory store so continuity holds across sessions.
// Spend/destructive asks still defer to `artanis-approval-gates`; the route
// surfaces the core's `deferredToApprovalGate` hint and adds NO execution
// authority.

import { Effect, Match as M, Schema as S } from 'effect'

import {
  ARTANIS_OPERATOR_APPROVAL_GATE_REF,
  ARTANIS_OPERATOR_CHANNEL_REF,
  ARTANIS_OPERATOR_MEMORY_TURN_LIMIT,
  type ArtanisOperatorKhalaClient,
  ArtanisOperatorMessage,
  artanisOperatorTurn,
} from './artanis-operator'
import {
  appendArtanisMemory,
  type ArtanisOwnerMemoryStore,
  loadArtanisMemory,
  makeD1ArtanisOwnerMemoryStore,
} from './artanis-owner-memory'
import {
  type ArtanisAwarenessReaders,
  buildArtanisSituationalAwareness,
} from './artanis-situational-awareness'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type OperatorArtanisChatEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type OperatorArtanisChatSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

export type OperatorArtanisChatDependencies<
  Session extends OperatorArtanisChatSession,
  Bindings extends OperatorArtanisChatEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  // The Khala-backed client that dogfoods `openagents/khala` for Artanis's
  // reasoning. The same builder the forum responder uses
  // (`makeArtanisResponderKhalaClient` in index.ts) provides it; when absent the
  // route returns a typed unavailability instead of falling back to a provider.
  makeKhalaClient: (env: Bindings) => ArtanisOperatorKhalaClient | undefined
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  // Test/override seams. Production defaults to the D1 store + reader-less
  // awareness build (honest-absence buckets + code-anchored goals).
  makeMemoryStore?: (env: Bindings) => ArtanisOwnerMemoryStore
  awarenessReaders?: (env: Bindings) => ArtanisAwarenessReaders
}>

class OperatorArtanisChatUnauthorized extends S.TaggedErrorClass<OperatorArtanisChatUnauthorized>()(
  'OperatorArtanisChatUnauthorized',
  {},
) {}

class OperatorArtanisChatForbidden extends S.TaggedErrorClass<OperatorArtanisChatForbidden>()(
  'OperatorArtanisChatForbidden',
  {},
) {}

class OperatorArtanisChatBadRequest extends S.TaggedErrorClass<OperatorArtanisChatBadRequest>()(
  'OperatorArtanisChatBadRequest',
  { reason: S.String },
) {}

class OperatorArtanisChatUnavailable extends S.TaggedErrorClass<OperatorArtanisChatUnavailable>()(
  'OperatorArtanisChatUnavailable',
  {},
) {}

class OperatorArtanisChatSessionError extends S.TaggedErrorClass<OperatorArtanisChatSessionError>()(
  'OperatorArtanisChatSessionError',
  { error: S.Defect },
) {}

class OperatorArtanisChatStorageError extends S.TaggedErrorClass<OperatorArtanisChatStorageError>()(
  'OperatorArtanisChatStorageError',
  { error: S.Defect },
) {}

type OperatorArtanisChatError =
  | OperatorArtanisChatBadRequest
  | OperatorArtanisChatForbidden
  | OperatorArtanisChatSessionError
  | OperatorArtanisChatStorageError
  | OperatorArtanisChatUnauthorized
  | OperatorArtanisChatUnavailable

const routeErrorResponse = (error: OperatorArtanisChatError) =>
  M.value(error).pipe(
    M.tags({
      OperatorArtanisChatBadRequest: badRequest =>
        noStoreJsonResponse(
          { error: 'bad_request', reason: badRequest.reason },
          { status: 400 },
        ),
      OperatorArtanisChatForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorArtanisChatSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorArtanisChatStorageError: () =>
        noStoreJsonResponse(
          { error: 'artanis_operator_chat_storage_error' },
          { status: 500 },
        ),
      OperatorArtanisChatUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      OperatorArtanisChatUnavailable: () =>
        noStoreJsonResponse(
          { error: 'artanis_operator_mind_unavailable' },
          { status: 503 },
        ),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends OperatorArtanisChatSession,
  Bindings extends OperatorArtanisChatEnv,
>(
  dependencies: OperatorArtanisChatDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisChatSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorArtanisChatSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorArtanisChatUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorArtanisChatForbidden({})
    }

    return session
  })

// The owner-scoped memory id. We key memory on the authenticated admin/owner
// user id so continuity holds for the owner across sessions and never crosses
// owners.
const ownerIdForSession = (session: OperatorArtanisChatSession): string =>
  `owner:${session.user.userId}`

const ChatRequestBody = S.Struct({
  messages: S.Array(ArtanisOperatorMessage),
})

const parseBody = (request: Request) =>
  Effect.gen(function* () {
    const raw = yield* Effect.tryPromise({
      catch: () =>
        new OperatorArtanisChatBadRequest({ reason: 'invalid_json' }),
      try: () => request.json(),
    })

    const decoded = yield* S.decodeUnknownEffect(ChatRequestBody)(raw).pipe(
      Effect.mapError(
        () => new OperatorArtanisChatBadRequest({ reason: 'invalid_messages' }),
      ),
    )

    if (decoded.messages.length === 0) {
      return yield* new OperatorArtanisChatBadRequest({
        reason: 'empty_messages',
      })
    }

    const hasOwnerMessage = decoded.messages.some(
      message => message.role === 'user',
    )
    if (!hasOwnerMessage) {
      return yield* new OperatorArtanisChatBadRequest({
        reason: 'no_owner_message',
      })
    }

    return decoded
  })

export const makeOperatorArtanisChatRoutes = <
  Session extends OperatorArtanisChatSession,
  Bindings extends OperatorArtanisChatEnv,
>(
  dependencies: OperatorArtanisChatDependencies<Session, Bindings>,
) => ({
  routeOperatorArtanisChatRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<globalThis.Response> | undefined => {
    const url = new URL(request.url)

    if (url.pathname !== '/api/operator/artanis/chat') {
      return undefined
    }

    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    const makeMemoryStore =
      dependencies.makeMemoryStore ??
      ((bindings: Bindings) =>
        makeD1ArtanisOwnerMemoryStore(openAgentsDatabase(bindings)))
    const awarenessReaders = dependencies.awarenessReaders?.(env) ?? {}

    return Effect.gen(function* () {
      const session = yield* requireAdminSession(
        dependencies,
        request,
        env,
        ctx,
      )

      const khalaClient = dependencies.makeKhalaClient(env)
      if (khalaClient === undefined) {
        return yield* new OperatorArtanisChatUnavailable({})
      }

      const body = yield* parseBody(request)
      const ownerId = ownerIdForSession(session)
      const store = makeMemoryStore(env)

      // Load grounded state: owner memory (owner-scoped) + live situational
      // awareness (bounded, owner-scoped). Both are best-effort: a storage
      // failure degrades grounding but must not drop the conversation, so we
      // fall back to empty memory / a reader-less awareness build.
      const memory = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisChatStorageError({ error }),
        try: () =>
          loadArtanisMemory(store, ownerId, ARTANIS_OPERATOR_MEMORY_TURN_LIMIT),
      }).pipe(Effect.orElseSucceed(() => []))

      const awareness = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisChatStorageError({ error }),
        try: () => buildArtanisSituationalAwareness(ownerId, awarenessReaders),
      })

      const turn = yield* artanisOperatorTurn({
        awareness,
        khalaClient,
        memory,
        messages: body.messages,
        ownerId,
      })

      if ('error' in turn) {
        return yield* new OperatorArtanisChatUnavailable({})
      }

      // Persist the latest owner message + Artanis's reply to owner memory so
      // continuity holds. Fail-soft: persistence failure must not drop a good
      // reply, so we run it but ignore its error.
      const latestOwner = [...body.messages]
        .reverse()
        .find(message => message.role === 'user')
      if (latestOwner !== undefined) {
        yield* Effect.tryPromise(() =>
          appendArtanisMemory(store, ownerId, {
            body: latestOwner.content,
            kind: 'turn',
            role: 'owner',
          }),
        ).pipe(Effect.catch(() => Effect.void))
      }
      yield* Effect.tryPromise(() =>
        appendArtanisMemory(store, ownerId, {
          body: turn.reply,
          kind: 'turn',
          role: 'artanis',
        }),
      ).pipe(Effect.catch(() => Effect.void))

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          approvalGateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
          channelRef: ARTANIS_OPERATOR_CHANNEL_REF,
          deferredToApprovalGate: turn.deferredToApprovalGate,
          persona: turn.persona,
          reply: turn.reply,
          requestedModel: turn.requestedModel,
          servedModel: turn.servedModel,
          servedVia: turn.servedVia,
        }),
        session,
      )
    }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
  },
})
