// Owner-auth endpoint for the Artanis operator chat channel (issue #6363).
//
//   POST /api/operator/artanis/chat   body { messages: [{role,content}] }
//                                      -> { reply, servedVia, servedModel, ... }
//
// This is the authenticated, owner-scoped private channel to the REAL Artanis
// operator agent. It admits a signed-in browser session, an owner-linked agent
// bearer, or the admin API token, then keys memory and awareness to that
// authenticated owner user id. It routes the
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

import { Effect, Match as M, Schedule, Schema as S } from 'effect'

import {
  ARTANIS_OPERATOR_APPROVAL_GATE_REF,
  ARTANIS_OPERATOR_CHANNEL_REF,
  ARTANIS_OPERATOR_MEMORY_TURN_LIMIT,
  type ArtanisOperatorKhalaClient,
  ArtanisOperatorMessage,
  type ArtanisOperatorTool,
  artanisOperatorTurn,
} from './artanis-operator'
import { makeArtanisOperatorTools } from './artanis-operator-tools'
import {
  appendArtanisMemory,
  type ArtanisMemoryEntry,
  type ArtanisOwnerMemoryStore,
  loadArtanisMemory,
  makeD1ArtanisOwnerMemoryStore,
} from './artanis-owner-memory'
import {
  type ArtanisAwarenessReaders,
  type ArtanisSituationalAwareness,
  buildArtanisSituationalAwareness,
} from './artanis-situational-awareness'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { InferenceMessage, InferenceResult } from './inference/provider-adapter'
import { logWorkerRouteWarning, workerErrorName } from './observability'
import { openAgentsDatabase } from './runtime'
import {
  compactRandomId,
  currentIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

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

type OperatorArtanisTraceInput = Readonly<{
  ownerUserId: string
  requestMessages: ReadonlyArray<InferenceMessage>
  responseId: string
  result: InferenceResult
}>

type ArtanisOperatorThreadMessage = Readonly<{
  authorId: string
  authorKind: 'owner' | 'agent' | 'operator' | 'system' | 'tool'
  body: string
  callerId: string
  createdAt: string
  messageRef: string
  threadRef: string
}>

type ArtanisOperatorThreadInput = Readonly<{
  callerId: string
  messages: ReadonlyArray<
    Readonly<{
      authorId: string
      authorKind: ArtanisOperatorThreadMessage['authorKind']
      body: string
      metadata?: Readonly<Record<string, unknown>>
    }>
  >
  nowIso: string
  threadRef: string
  title: string
}>

type ArtanisOperatorThreadStore = Readonly<{
  appendTurn: (
    input: ArtanisOperatorThreadInput,
  ) => Promise<ReadonlyArray<ArtanisOperatorThreadMessage>>
  loadThreadMessages: (
    threadRef: string,
  ) => Promise<ReadonlyArray<ArtanisOperatorThreadMessage>>
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
  // Resolves an agent bearer token to an owner session when the token is linked
  // to an authenticated OpenAuth user (or is the owner-promoted Artanis agent).
  // Lets the Khala CLI (which authenticates with an `oa_agent_` bearer linked
  // via `khala login`) reach the same per-user Artanis channel as a browser
  // session. Returns undefined for unlinked/non-owner tokens.
  resolveOwnerAgentBearer?: (
    request: Request,
    env: Bindings,
  ) => Promise<Session | undefined>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  // Test/override seams. Production defaults to the D1 store + reader-less
  // awareness build (honest-absence buckets + code-anchored goals).
  makeMemoryStore?: (env: Bindings) => ArtanisOwnerMemoryStore
  awarenessReaders?: (env: Bindings) => ArtanisAwarenessReaders
  // The owner-scoped tool table Artanis can invoke in the bounded tool-calling
  // loop (#6364): public repo-read tools (#6365) + the gated Codex dispatch tool
  // (#6366). It receives the authenticated owner SESSION so the gated dispatch
  // can wire an owner-scoped execution seam (own-capacity, no-spend, behind the
  // approval gate). Defaults to `makeArtanisOperatorTools()` (plan-only); tests
  // override it.
  makeOperatorTools?: (
    env: Bindings,
    session: Session,
  ) => ReadonlyArray<ArtanisOperatorTool>
  makeThreadStore?: (env: Bindings) => ArtanisOperatorThreadStore
  emitOwnerTrace?: (
    input: OperatorArtanisTraceInput,
    env: Bindings,
  ) => Promise<unknown>
}>

class OperatorArtanisChatUnauthorized extends S.TaggedErrorClass<OperatorArtanisChatUnauthorized>()(
  'OperatorArtanisChatUnauthorized',
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

type OperatorArtanisChatError =
  | OperatorArtanisChatBadRequest
  | OperatorArtanisChatSessionError
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
      OperatorArtanisChatSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
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

// Persistence around the Artanis operator chat (thread history, owner memory,
// situational-awareness reads, and the post-turn thread append) is GROUNDING
// and CONTINUITY, not the reply itself: the owner's answer comes from the
// Khala-powered operator turn. A transient D1 failure on any of these must
// therefore degrade grounding/continuity, NOT drop a good reply behind an
// opaque 500. Each persistence step below is fail-soft: it retries briefly,
// then on failure logs the SPECIFIC underlying error (for Cloudflare Logs/Tail
// diagnosability) and records a public-safe `degraded` marker on the response.
const TRANSIENT_STORE_RETRY = Schedule.recurs(2)

// Markers surfaced on the response so the operator (and future debuggers) can
// see which non-critical persistence step degraded this turn without leaking
// the underlying error or any owner chat text.
type ArtanisChatDegradedMarker =
  | 'thread_history_unavailable'
  | 'owner_memory_unavailable'
  | 'situational_awareness_unavailable'
  | 'thread_persist_failed'

// Log the SPECIFIC underlying storage error so a future failure is diagnosable,
// instead of swallowing it behind an opaque response. Routed through the
// redacted Effect observability helper (never raw console): only the failing
// step and the error's own name/message are emitted — and those field values
// are redacted — so owner chat text, prompts, and secrets never reach logs.
const logArtanisStorageDegrade = (step: string, error: unknown): void => {
  logWorkerRouteWarning('artanis_operator_chat_persistence_degraded', {
    errorMessage: error instanceof Error ? error.message : String(error),
    errorName: workerErrorName(error),
    step,
  })
}

// Last-resort honest-absence awareness when even the reader-less build cannot be
// produced. Mirrors the `ArtanisSituationalAwareness` shape with empty buckets;
// honest absence over fabrication.
const emptyArtanisAwareness = (
  ownerId: string,
  generatedAt: string,
): ArtanisSituationalAwareness => ({
  generatedAt,
  goals: { epics: [], roadmapRef: '', roadmapSummary: '' },
  kind: 'artanis_situational_awareness',
  ongoingOps: {
    activeAssignments: [],
    fleetReadiness: null,
    publicCounter: null,
    recentDeploys: [],
    tokenPace: null,
  },
  ownerId,
  ownerOnly: true,
  recentActions: { assignments: [], commits: [], issueChanges: [], ticks: [] },
})

const requireAuthenticatedSession = <
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

    const resolveOwnerAgentBearer = dependencies.resolveOwnerAgentBearer
    if (resolveOwnerAgentBearer !== undefined) {
      const ownerAgentSession = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisChatSessionError({ error }),
        try: () => resolveOwnerAgentBearer(request, env),
      })

      if (ownerAgentSession !== undefined) {
        return ownerAgentSession
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorArtanisChatSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorArtanisChatUnauthorized({})
    }

    return session
  })

// The owner-scoped memory id. We key memory on the authenticated admin/owner
// user id so continuity holds for the owner across sessions and never crosses
// owners.
const ownerIdForSession = (session: OperatorArtanisChatSession): string =>
  `owner:${session.user.userId}`

const ChatRequestBody = S.Struct({
  caller_id: S.optionalKey(S.String),
  messages: S.Array(ArtanisOperatorMessage),
  thread_id: S.optionalKey(S.String),
})

const normalizeOptionalText = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? undefined : trimmed
}

const requestedThreadRef = (value: string | undefined): string | undefined =>
  normalizeOptionalText(value)

const requestedCallerId = (
  value: string | undefined,
  session: OperatorArtanisChatSession,
): string => normalizeOptionalText(value) ?? session.user.userId

const callerKindForCallerId = (
  callerId: string,
): 'owner' | 'agent' | 'operator' | 'system' => {
  const normalized = callerId.toLowerCase()
  if (normalized === 'owner' || normalized.startsWith('github:')) return 'owner'
  if (normalized.includes('codex') || normalized.includes('claude')) return 'agent'
  if (normalized === 'system') return 'system'
  return 'operator'
}

const authorKindForMessage = (
  message: ArtanisOperatorMessage,
): ArtanisOperatorThreadMessage['authorKind'] =>
  message.role === 'assistant'
    ? 'agent'
    : message.role === 'system'
      ? 'system'
      : 'owner'

const messageRoleForThreadMessage = (
  message: ArtanisOperatorThreadMessage,
): ArtanisOperatorMessage['role'] =>
  message.authorKind === 'owner'
    ? 'user'
    : message.authorKind === 'system'
      ? 'system'
      : 'assistant'

const threadMessageToOperatorMessage = (
  message: ArtanisOperatorThreadMessage,
): ArtanisOperatorMessage => ({
  content: message.body,
  role: messageRoleForThreadMessage(message),
})

const makeThreadRef = (): string =>
  `artanis_thread:${compactRandomId('thread')}`

const makeMessageRef = (): string =>
  `artanis_message:${compactRandomId('msg')}`

const threadTitleFromMessages = (
  messages: ReadonlyArray<ArtanisOperatorMessage>,
): string => {
  const title = messages.find(message => message.role === 'user')?.content ?? ''
  return title.trim().slice(0, 120)
}

const makeD1ArtanisOperatorThreadStore = (
  db: D1Database,
): ArtanisOperatorThreadStore => ({
  appendTurn: async input => {
    const callerKind = callerKindForCallerId(input.callerId)
    await db
      .prepare(
        `INSERT INTO artanis_threads (
            thread_ref,
            caller_id,
            caller_kind,
            subject_agent_ref,
            subject_agent_kind,
            title,
            last_message_at,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(thread_ref) DO UPDATE SET
            caller_id = excluded.caller_id,
            caller_kind = excluded.caller_kind,
            title = CASE
              WHEN artanis_threads.title = '' THEN excluded.title
              ELSE artanis_threads.title
            END,
            last_message_at = excluded.last_message_at,
            updated_at = excluded.updated_at`,
      )
      .bind(
        input.threadRef,
        input.callerId,
        callerKind,
        'artanis',
        'artanis',
        input.title,
        input.nowIso,
        input.nowIso,
        input.nowIso,
      )
      .run()

    const inserted: Array<ArtanisOperatorThreadMessage> = []
    for (const message of input.messages) {
      const messageRef = makeMessageRef()
      await db
        .prepare(
          `INSERT INTO artanis_messages (
              message_ref,
              thread_ref,
              caller_id,
              author_id,
              author_kind,
              body,
              metadata_json,
              created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          messageRef,
          input.threadRef,
          input.callerId,
          message.authorId,
          message.authorKind,
          message.body,
          JSON.stringify(message.metadata ?? {}),
          input.nowIso,
        )
        .run()
      inserted.push({
        authorId: message.authorId,
        authorKind: message.authorKind,
        body: message.body,
        callerId: input.callerId,
        createdAt: input.nowIso,
        messageRef,
        threadRef: input.threadRef,
      })
    }
    return inserted
  },

  loadThreadMessages: async threadRef => {
    const rows = await db
      .prepare(
        `SELECT message_ref, thread_ref, caller_id, author_id, author_kind, body, created_at
           FROM artanis_messages
          WHERE thread_ref = ?
          ORDER BY created_at ASC`,
      )
      .bind(threadRef)
      .all<{
        author_id: string
        author_kind: string
        body: string
        caller_id: string
        created_at: string
        message_ref: string
        thread_ref: string
      }>()

    return rows.results
      .filter(
        row =>
          row.author_kind === 'owner' ||
          row.author_kind === 'agent' ||
          row.author_kind === 'operator' ||
          row.author_kind === 'system' ||
          row.author_kind === 'tool',
      )
      .map(row => ({
        authorId: row.author_id,
        authorKind: row.author_kind as ArtanisOperatorThreadMessage['authorKind'],
        body: row.body,
        callerId: row.caller_id,
        createdAt: row.created_at,
        messageRef: row.message_ref,
        threadRef: row.thread_ref,
      }))
  },
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

const wantsEventStream = (request: Request): boolean =>
  request.headers
    .get('accept')
    ?.split(',')
    .map(part => part.trim().toLowerCase())
    .some(
      part =>
        part === 'text/event-stream' ||
        part.startsWith('text/event-stream;'),
    ) === true

const sseData = (value: unknown): string =>
  `data: ${JSON.stringify(value)}\n\n`

const artanisSseResponse = (turn: Readonly<{
  callerId: string
  degraded: ReadonlyArray<string>
  deferredToApprovalGate: boolean
  iterations: number
  pendingApprovalGates: unknown
  persona: unknown
  reply: string
  requestedModel: string
  servedModel: string
  servedVia: string
  threadRef: string
  toolInvocations: unknown
}>) =>
  new Response(
    [
      sseData({
        choices: [
          {
            delta: { role: 'assistant' },
            index: 0,
          },
        ],
      }),
      ...(turn.reply.length === 0
        ? []
        : [
            sseData({
              choices: [
                {
                  delta: { content: turn.reply },
                  index: 0,
                },
              ],
            }),
          ]),
      sseData({
        approvalGateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
        caller_id: turn.callerId,
        channelRef: ARTANIS_OPERATOR_CHANNEL_REF,
        degraded: turn.degraded,
        deferredToApprovalGate: turn.deferredToApprovalGate,
        iterations: turn.iterations,
        pendingApprovalGates: turn.pendingApprovalGates,
        persona: turn.persona,
        requestedModel: turn.requestedModel,
        servedModel: turn.servedModel,
        servedVia: turn.servedVia,
        thread_id: turn.threadRef,
        toolInvocations: turn.toolInvocations,
      }),
      'data: [DONE]\n\n',
    ].join(''),
    {
      headers: {
        'Cache-Control': 'no-store',
        'Content-Type': 'text/event-stream; charset=utf-8',
      },
    },
  )

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
    const makeThreadStore =
      dependencies.makeThreadStore ??
      ((bindings: Bindings) =>
        makeD1ArtanisOperatorThreadStore(openAgentsDatabase(bindings)))
    const awarenessReaders = dependencies.awarenessReaders?.(env) ?? {}

    return Effect.gen(function* () {
      const session = yield* requireAuthenticatedSession(
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
      const threadRef = requestedThreadRef(body.thread_id) ?? makeThreadRef()
      const callerId = requestedCallerId(body.caller_id, session)
      const store = makeMemoryStore(env)
      const threadStore = makeThreadStore(env)
      const degraded: Array<ArtanisChatDegradedMarker> = []

      // Thread history (continuity). Fail-soft: a read failure degrades to no
      // prior thread context rather than dropping the turn.
      const historicalMessages = yield* Effect.tryPromise(() =>
        threadStore.loadThreadMessages(threadRef),
      ).pipe(
        Effect.retry(TRANSIENT_STORE_RETRY),
        Effect.catch(error =>
          Effect.sync(() => {
            logArtanisStorageDegrade('load_thread_history', error)
            degraded.push('thread_history_unavailable')
            return [] as ReadonlyArray<ArtanisOperatorThreadMessage>
          }),
        ),
      )
      const turnMessages = [
        ...historicalMessages.map(threadMessageToOperatorMessage),
        ...body.messages,
      ]

      // Load grounded state: owner memory (owner-scoped) + live situational
      // awareness (bounded, owner-scoped). Both are best-effort: a storage
      // failure degrades grounding but must not drop the conversation, so we
      // fall back to empty memory / a reader-less awareness build.
      const memory = yield* Effect.tryPromise(() =>
        loadArtanisMemory(store, ownerId, ARTANIS_OPERATOR_MEMORY_TURN_LIMIT),
      ).pipe(
        Effect.retry(TRANSIENT_STORE_RETRY),
        Effect.catch(error =>
          Effect.sync(() => {
            logArtanisStorageDegrade('load_owner_memory', error)
            degraded.push('owner_memory_unavailable')
            return [] as ReadonlyArray<ArtanisMemoryEntry>
          }),
        ),
      )

      const awareness = yield* Effect.tryPromise(() =>
        buildArtanisSituationalAwareness(ownerId, awarenessReaders),
      ).pipe(
        Effect.retry(TRANSIENT_STORE_RETRY),
        Effect.catch(error =>
          Effect.sync(() => {
            logArtanisStorageDegrade('build_situational_awareness', error)
            degraded.push('situational_awareness_unavailable')
            return emptyArtanisAwareness(ownerId, currentIsoTimestamp())
          }),
        ),
      )

      const tools =
        dependencies.makeOperatorTools?.(env, session) ??
        makeArtanisOperatorTools()

      const turn = yield* artanisOperatorTurn({
        awareness,
        khalaClient,
        memory,
        messages: turnMessages,
        ownerId,
        tools,
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

      const nowIso = currentIsoTimestamp()
      const inboundMessages = body.messages.map(message => ({
        authorId:
          message.role === 'user'
            ? callerId
            : message.role === 'assistant'
              ? 'artanis'
              : 'system',
        authorKind: authorKindForMessage(message),
        body: message.content,
        metadata: { source: 'operator_artanis_chat_request' },
      }))
      // Persist this turn to the thread ledger (continuity). Fail-soft: this
      // runs AFTER we already have a good reply, so a transient D1 write failure
      // must not turn a successful turn into a 500. Retry briefly, then degrade.
      yield* Effect.tryPromise(() =>
        threadStore.appendTurn({
          callerId,
          messages: [
            ...inboundMessages,
            {
              authorId: 'artanis',
              authorKind: 'agent',
              body: turn.reply,
              metadata: { source: 'operator_artanis_chat_reply' },
            },
          ],
          nowIso,
          threadRef,
          title: threadTitleFromMessages(turnMessages),
        }),
      ).pipe(
        Effect.retry(TRANSIENT_STORE_RETRY),
        Effect.catch(error =>
          Effect.sync(() => {
            logArtanisStorageDegrade('append_thread_turn', error)
            degraded.push('thread_persist_failed')
          }),
        ),
      )

      if (dependencies.emitOwnerTrace !== undefined) {
        yield* Effect.tryPromise(() =>
          dependencies.emitOwnerTrace!(
            {
              ownerUserId: session.user.userId,
              requestMessages: body.messages.map(message => ({
                content: message.content,
                role: message.role,
              })),
              responseId: `operator-artanis:${randomUuid()}`,
              result: {
                content: turn.reply,
                finishReason: 'stop',
                servedModel: turn.servedModel,
                usage: {
                  completionTokens: 0,
                  promptTokens: 0,
                  totalTokens: 0,
                },
              },
            },
            env,
          ),
        ).pipe(Effect.catch(() => Effect.void))
      }

      if (wantsEventStream(request)) {
        return dependencies.appendRefreshedSessionCookies(
          artanisSseResponse({ ...turn, callerId, degraded, threadRef }),
          session,
        )
      }

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          approvalGateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
          channelRef: ARTANIS_OPERATOR_CHANNEL_REF,
          caller_id: callerId,
          // Public-safe non-fatal markers for any persistence step that degraded
          // this turn (empty when fully healthy). The reply is unaffected.
          degraded,
          deferredToApprovalGate: turn.deferredToApprovalGate,
          historicalMessageCount: historicalMessages.length,
          iterations: turn.iterations,
          pendingApprovalGates: turn.pendingApprovalGates,
          persona: turn.persona,
          reply: turn.reply,
          requestedModel: turn.requestedModel,
          servedModel: turn.servedModel,
          servedVia: turn.servedVia,
          thread_id: threadRef,
          toolInvocations: turn.toolInvocations,
        }),
        session,
      )
    }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
  },
})
