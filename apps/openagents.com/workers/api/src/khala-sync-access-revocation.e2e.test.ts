// KS-7.1 revocation end-to-end verification (#8305; SPEC §3 auth, §7
// invariant 7: "Scope access is checked at bootstrap/connect and re-checked
// on membership change; revocation retracts synced state via re-bootstrap").
//
// Drives the FULL revocation path against real components:
//
//   Postgres     a throwaway local server (initdb + pg_ctl) with the real
//                khala-sync-server migrations; team-scope writes go through
//                the REAL outbox writer, reads through the REAL Worker
//                bootstrap/log route handlers over postgres.js with the
//                production client options (max 1, prepare: false).
//   Scope auth   the REAL Worker resolver (`makeKhalaSyncScopeReadResolver`)
//                over a fake D1 answering the exact production
//                `team_memberships` query from a mutable membership table —
//                removing the row IS the revocation.
//   Hub          a REAL `KhalaSyncHubDO` (node:sqlite harness) with a live
//                fake hibernation socket; the revocation trigger is the REAL
//                admin internal route handler
//                (`handleKhalaSyncHubAccessChangedRoute`).
//   Client       the REAL `@openagentsinc/khala-sync-client` store, overlay,
//                and session over a transport that mirrors the flow: HTTP
//                reads hit the REAL route handlers (so post-revocation
//                bootstraps really 403), and the live channel replays the
//                hub's MustRefetch(access_changed) broadcast.
//
// Scenario: user A reads scope.team.<id> while a member (bootstrap 200,
// log 200, connect reaches the hub; session goes live with the team rows in
// its durable store) → the membership row is removed + the access-changed
// trigger fires → the hub broadcasts MustRefetch(access_changed) and closes
// the socket → log/bootstrap/connect all 403 (connect BEFORE any hub
// contact) → the client session's denied re-bootstrap CLEARS its scope-local
// durable state and parks in the terminal `denied` phase without retrying.
//
// Gated on local Postgres binaries (initdb/pg_ctl); machines without them
// skip instead of fail.

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Effect } from 'effect'
import postgres from 'postgres'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  type BootstrapRequest,
  decodeBootstrapResponse,
  decodeLogPage,
  decodeSyncError,
  encodeBootstrapRequest,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  MustRefetchFrame,
  SyncScope,
  teamScope,
} from '@openagentsinc/khala-sync'
import {
  createKhalaSyncSession,
  createOverlay,
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
  openKhalaSyncStore,
} from '@openagentsinc/khala-sync-client'
import {
  type SyncSql,
  withSyncTransaction,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  type LocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'

import {
  handleKhalaSyncBootstrap,
  KHALA_SYNC_BOOTSTRAP_PATH,
} from './khala-sync-bootstrap-routes'
import { handleKhalaSyncConnect, KHALA_SYNC_CONNECT_PATH } from './khala-sync-connect-routes'
import {
  handleKhalaSyncHubAccessChangedRoute,
  type KhalaSyncHubNamespaceLike,
} from './khala-sync-hub-do'
import { handleKhalaSyncLog, KHALA_SYNC_LOG_PATH } from './khala-sync-log-routes'
import type { MakeKhalaSyncPushSqlClient } from './khala-sync-push-routes'
import { makeKhalaSyncScopeReadResolver } from './khala-sync-scope-auth'
import { FakeWebSocket, makeHub } from './test/khala-sync-hub-do-harness'

const TEAM_ID = 'team-revoke'
const SCOPE = teamScope(TEAM_ID)
const USER_A = 'user-a'
const DOC = EntityType.make('doc')

const MIGRATIONS_DIR = join(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations',
)

describe.skipIf(!hasLocalPostgres())(
  'KS-7.1 revocation e2e: member reads → removal + access-changed → MustRefetch, 403s, client clears and parks denied',
  () => {
    let pg: LocalPostgres
    let dbUrl: string
    let writerSql: ReturnType<typeof postgres>

    // -----------------------------------------------------------------------
    // Live membership "D1": the fake answers the EXACT production
    // team_memberships query from this mutable table. Deleting the row is
    // the revocation write (this Worker has no member-removal route today —
    // memberships are operator-managed — so the operator flow IS the
    // production flow).
    // -----------------------------------------------------------------------

    const memberships = new Map<string, string>([
      [`${TEAM_ID}:${USER_A}`, 'member'],
    ])

    const membershipD1 = {
      prepare: (sql: string) => ({
        bind: (...bindings: Array<unknown>) => ({
          first: async <T>(): Promise<T | null> => {
            if (sql.includes('FROM team_memberships')) {
              const [teamId, userId] = bindings as [string, string]
              const role = memberships.get(`${teamId}:${userId}`)
              return (role === undefined ? null : { role }) as T | null
            }
            if (sql.includes('FROM agent_runs')) return null
            if (sql.includes('FROM team_chat_messages')) return null
            throw new Error(`fake D1 has no route for: ${sql.slice(0, 60)}`)
          },
        }),
      }),
    } as unknown as D1Database

    // The exact production postgres.js client discipline (SPEC §4).
    const makeRealSqlClient: MakeKhalaSyncPushSqlClient = async connectionString => {
      const sql = postgres(connectionString, {
        connect_timeout: 10,
        max: 1,
        onnotice: () => {},
        prepare: false,
      })
      return {
        end: () => sql.end({ timeout: 5 }),
        sql: sql as unknown as SyncSql,
      }
    }

    const resolveScopeRead = () =>
      makeKhalaSyncScopeReadResolver({
        binding: { connectionString: dbUrl },
        db: membershipD1,
        makeSqlClient: makeRealSqlClient,
      })

    // -----------------------------------------------------------------------
    // Hub (real DO over node:sqlite)
    // -----------------------------------------------------------------------

    const hubHarness = makeHub()
    const hub = hubHarness.hub
    const hubNamespace: KhalaSyncHubNamespaceLike = {
      get: () => ({ fetch: request => hub.fetch(request) }),
      idFromName: name => name,
    }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = postgres(pg.url, { max: 1, onnotice: () => {} })
      await admin.unsafe('CREATE DATABASE khala_sync_revocation')
      await admin.end({ timeout: 5 })
      dbUrl = pg.urlFor('khala_sync_revocation')

      const migrator = postgres(dbUrl, { max: 1, onnotice: () => {} })
      const files = readdirSync(MIGRATIONS_DIR)
        .filter(name => name.endsWith('.sql'))
        .sort()
      expect(files.length).toBeGreaterThan(0)
      for (const file of files) {
        const content = readFileSync(join(MIGRATIONS_DIR, file), 'utf8')
        await migrator.begin(async tx => {
          await tx.unsafe(content)
        })
      }
      await migrator.end({ timeout: 5 })

      writerSql = postgres(dbUrl, { max: 1, onnotice: () => {}, prepare: false })
    }, 120_000)

    afterAll(async () => {
      await writerSql?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    // -----------------------------------------------------------------------
    // Real route handlers as the request surface
    // -----------------------------------------------------------------------

    const bootstrapVia = (body: unknown): Promise<Response> =>
      Effect.runPromise(
        handleKhalaSyncBootstrap(
          new Request(`https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`, {
            body: JSON.stringify(body),
            method: 'POST',
          }),
          {
            authenticate: async () => ({ userId: USER_A }),
            binding: { connectionString: dbUrl },
            makeSqlClient: makeRealSqlClient,
            resolveScopeRead: resolveScopeRead(),
          },
        ),
      )

    const logVia = (cursor: number): Promise<Response> => {
      const url = new URL(`https://openagents.com${KHALA_SYNC_LOG_PATH}`)
      url.searchParams.set('scope', SCOPE)
      url.searchParams.set('cursor', String(cursor))
      return Effect.runPromise(
        handleKhalaSyncLog(new Request(url.toString(), { method: 'GET' }), {
          authenticate: async () => ({ userId: USER_A }),
          binding: { connectionString: dbUrl },
          hubNamespace,
          makeSqlClient: makeRealSqlClient,
          resolveScopeRead: resolveScopeRead(),
        }),
      )
    }

    /** Connect route with a RECORDING hub stub: proves gate-before-hub. */
    const connectVia = async (): Promise<{
      response: Response
      hubHits: number
    }> => {
      let hubHits = 0
      const url = new URL(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`)
      url.searchParams.set('scope', SCOPE)
      url.searchParams.set('cursor', '0')
      const response = await Effect.runPromise(
        handleKhalaSyncConnect(
          new Request(url.toString(), {
            headers: { upgrade: 'websocket' },
            method: 'GET',
          }),
          {
            authenticate: async () => ({ userId: USER_A }),
            hubNamespace: {
              get: () => ({
                fetch: async () => {
                  hubHits += 1
                  return new Response(null, { status: 200 })
                },
              }),
              idFromName: name => name,
            },
            resolveScopeRead: resolveScopeRead(),
          },
        ),
      )
      return { hubHits, response }
    }

    /** One committed team-scope transaction + capture-role hub append. */
    const commit = async (
      changes: ReadonlyArray<{ id: string; postImage: unknown }>,
    ): Promise<void> => {
      const entries = await withSyncTransaction(
        writerSql as unknown as SyncSql,
        async writer => {
          const out = []
          for (const change of changes) {
            out.push(
              await writer.appendChange({
                entityId: EntityId.make(change.id),
                entityType: DOC,
                op: 'upsert',
                postImage: change.postImage,
                scope: SCOPE,
              }),
            )
          }
          return out
        },
      )
      const appended = await hub.fetch(
        new Request('https://khala-sync-hub.openagents.internal/append', {
          body: JSON.stringify({
            entries: entries.map(entry => ({
              committedAt: entry.committedAt,
              entityId: entry.entityId,
              entityType: entry.entityType,
              op: entry.op,
              postImageJson: entry.postImageJson,
              scope: entry.scope,
              version: entry.version,
            })),
            scope: SCOPE,
          }),
          method: 'POST',
        }),
      )
      expect(appended.status).toBe(200)
    }

    // -----------------------------------------------------------------------
    // Client transport mirroring the flow: HTTP reads through the REAL
    // route handlers; live channel controllable by the test (it replays the
    // hub broadcast, and re-checks membership on reconnect like the route).
    // -----------------------------------------------------------------------

    const liveSockets: Array<{ handlers: LiveSocketHandlers; open: boolean }> =
      []

    const throwSyncError = async (response: Response): Promise<never> => {
      const syncError = decodeSyncError(await response.json())
      throw new KhalaSyncTransportError(
        'sync_error',
        syncError.retryable,
        `khala-sync server error ${syncError.code}: ${syncError.messageSafe}`,
        { status: response.status, syncError },
      )
    }

    const transport: KhalaSyncTransport = {
      bootstrap: (request: BootstrapRequest) =>
        Effect.tryPromise({
          catch: error =>
            error instanceof KhalaSyncTransportError
              ? error
              : new KhalaSyncTransportError('network', true, String(error)),
          try: async () => {
            const response = await bootstrapVia(encodeBootstrapRequest(request))
            if (response.status !== 200) return throwSyncError(response)
            return decodeBootstrapResponse(await response.json())
          },
        }),
      connectLive: (scope, _cursor, handlers) =>
        Effect.tryPromise({
          catch: error =>
            error instanceof KhalaSyncTransportError
              ? error
              : new KhalaSyncTransportError('network', true, String(error)),
          try: async () => {
            // Mirror GET /api/sync/connect: the KS-7.1 gate runs BEFORE the
            // upgrade on every (re)connect.
            const decision = await resolveScopeRead()(USER_A, scope)
            if (decision.kind !== 'allowed') {
              throw new KhalaSyncTransportError(
                'sync_error',
                false,
                'khala-sync server error unauthorized_scope',
                {
                  status: 403,
                  syncError: decodeSyncError({
                    _tag: 'SyncError',
                    code: 'unauthorized_scope',
                    messageSafe: 'This user cannot read the requested scope.',
                    retryable: false,
                  }),
                },
              )
            }
            const record = { handlers, open: true }
            liveSockets.push(record)
            return {
              close: () => {
                record.open = false
              },
            }
          },
        }),
      logPage: (scope, cursor) =>
        Effect.tryPromise({
          catch: error =>
            error instanceof KhalaSyncTransportError
              ? error
              : new KhalaSyncTransportError('network', true, String(error)),
          try: async () => {
            expect(scope).toBe(SCOPE)
            const response = await logVia(Number(cursor))
            if (response.status !== 200) return throwSyncError(response)
            return decodeLogPage(await response.json())
          },
        }),
      push: () =>
        Effect.fail(
          new KhalaSyncTransportError('network', true, 'push unused here'),
        ),
    }

    const tick = (): Promise<void> =>
      new Promise(resolve => setTimeout(resolve, 0))
    const waitFor = async (
      condition: () => boolean,
      label: string,
    ): Promise<void> => {
      for (let i = 0; i < 3000; i++) {
        if (condition()) return
        await tick()
      }
      throw new Error(`timed out waiting for: ${label}`)
    }

    // -----------------------------------------------------------------------
    // THE revocation test
    // -----------------------------------------------------------------------

    test(
      'membership removal + access-changed retracts the team scope end to end',
      { timeout: 120_000 },
      async () => {
        await commit([
          { id: 'doc-1', postImage: { id: 'doc-1', body: 'team doc one' } },
          { id: 'doc-2', postImage: { id: 'doc-2', body: 'team doc two' } },
        ])

        // ------------------------------------------------------- while member
        const memberBootstrap = await bootstrapVia({
          clientGroupId: 'cg-check',
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion: 1,
          scope: SCOPE,
        })
        expect(memberBootstrap.status).toBe(200)
        const memberLog = await logVia(0)
        expect(memberLog.status).toBe(200)
        const memberConnect = await connectVia()
        expect(memberConnect.response.status).toBe(200)
        expect(memberConnect.hubHits).toBe(1)

        // A live hub socket for user A (the real DO accept/catch-up path).
        // Both docs committed in ONE transaction = ONE version group (v1).
        const hubSocket = new FakeWebSocket()
        hub.attachSocket(hubSocket, SyncScope.make(SCOPE), 1)
        hubSocket.sent.length = 0

        // A real client session goes live and lands the team rows durably.
        const store = openKhalaSyncStore(':memory:')
        const overlay = Effect.runSync(createOverlay(store, []))
        const session = createKhalaSyncSession(
          {
            authToken: () => 'token-a',
            baseUrl: 'https://openagents.com',
            clientGroupId: 'cg-revoke' as never,
            clientId: 'client-a' as never,
            schemaVersion: 1 as never,
          },
          store,
          overlay,
          transport,
          {
            backoffBaseMs: 1,
            backoffMaxMs: 2,
            maxBootstrapAttempts: 3,
            random: () => 0,
            sleep: () => tick(),
          },
        )
        await Effect.runPromise(session.subscribe(SCOPE))
        await waitFor(
          () => session.state(SCOPE).phase === 'live',
          'session live while member',
        )
        expect(Effect.runSync(store.readEntities(SCOPE)).length).toBe(2)
        expect(Number(Effect.runSync(store.cursor(SCOPE)))).toBe(1)
        const bootstrapsWhileMember = liveSockets.length

        // ---------------------------------------------------------- revoke
        memberships.delete(`${TEAM_ID}:${USER_A}`)

        // Fire the REAL admin trigger: POST /api/internal/khala-sync/hub/
        // access-changed { scope } → hub broadcast + socket close.
        const triggered = await handleKhalaSyncHubAccessChangedRoute(
          new Request(
            'https://openagents.com/api/internal/khala-sync/hub/access-changed',
            { body: JSON.stringify({ scope: SCOPE }), method: 'POST' },
          ),
          {
            namespace: hubNamespace,
            requireOperator: async () => true,
          },
        )
        expect(triggered.status).toBe(200)
        expect(
          (await triggered.json()) as Record<string, unknown>,
        ).toMatchObject({ notified: 1, ok: true })

        // The hub socket received MustRefetch(access_changed) and closed.
        expect(hubSocket.frames().map(f => f._tag)).toEqual([
          'MustRefetchFrame',
        ])
        expect(hubSocket.frames()[0]).toMatchObject({
          reason: 'access_changed',
          scope: SCOPE,
        })
        expect(hubSocket.closed).toBeDefined()

        // -------------------------------------------- post-revocation reads
        const deniedLog = await logVia(0)
        expect(deniedLog.status).toBe(403)
        expect(
          ((await deniedLog.json()) as { code: string }).code,
        ).toBe('unauthorized_scope')

        const deniedBootstrap = await bootstrapVia({
          clientGroupId: 'cg-check',
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion: 1,
          scope: SCOPE,
        })
        expect(deniedBootstrap.status).toBe(403)

        const deniedConnect = await connectVia()
        expect(deniedConnect.response.status).toBe(403)
        expect(deniedConnect.hubHits).toBe(0) // gate BEFORE the hub

        // ----------------------------------------------------- client leg
        // Replay the hub broadcast on the client's live channel (its socket
        // is the transport mirror of the hub socket we just proved).
        const clientSocket = liveSockets[liveSockets.length - 1]!
        expect(clientSocket.open).toBe(true)
        clientSocket.handlers.onFrame(
          new MustRefetchFrame({ reason: 'access_changed', scope: SCOPE }),
        )

        await waitFor(
          () => session.state(SCOPE).phase === 'denied',
          'session parked denied after the denied re-bootstrap',
        )

        // Invariant 7: revocation RETRACTED the synced state — the durable
        // store no longer holds the team rows or a cursor for the scope.
        expect(Effect.runSync(store.readEntities(SCOPE))).toEqual([])
        expect(Effect.runSync(store.cursor(SCOPE))).toBeNull()

        // Terminal: no reconnect after the denial.
        const socketsAtPark = liveSockets.length
        for (let i = 0; i < 25; i++) await tick()
        expect(liveSockets.length).toBe(socketsAtPark)
        expect(liveSockets.length).toBe(bootstrapsWhileMember)

        await Effect.runPromise(session.close())
        Effect.runSync(Effect.ignore(store.close()))
      },
    )
  },
)
