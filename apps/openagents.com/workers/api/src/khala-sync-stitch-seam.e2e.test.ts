// KS-4.4 stitch-seam end-to-end verification (#8297; SPEC §3 bootstrap/log,
// §5 hub, §8 "Stitching" verification plan).
//
// Drives the FULL client convergence path against real components — no
// seams faked at the layers under test:
//
//   Postgres        a throwaway local server (initdb + pg_ctl) with the real
//                   khala-sync-server migrations applied; all writes go
//                   through the REAL KS-2.1 outbox writer
//                   (`withSyncTransaction`), all reads through the REAL
//                   KS-2.2 `bootstrap`/`logPage` — reached via the REAL
//                   Worker routes (`handleKhalaSyncBootstrap`,
//                   `handleKhalaSyncLog`) over postgres.js with the exact
//                   production client options (max 1, prepare: false).
//   Hub             a REAL `KhalaSyncHubDO` instance over node:sqlite (the
//                   KS-4.2 harness), fed by a capture-role append after
//                   every commit, addressed by the log route hub-first —
//                   with a deliberately TINY window so the catch-up loop
//                   exercises BOTH the hub-window hit AND the
//                   Postgres-fallthrough serving paths.
//   Client          the REAL `@openagentsinc/khala-sync-client` SQLite store
//                   (over the bun:sqlite→node:sqlite test adapter):
//                   `resetScope` at the bootstrap stitch cursor,
//                   `applyConfirmed` for catch-up pages and live
//                   DeltaFrames, with every page applied TWICE to prove
//                   at-least-once/apply-idempotence (invariant 4).
//
// The scenario stitches under fire: writes (updates of already-served AND
// not-yet-served entities, deletes, creates) are committed WHILE the
// bootstrap is paging, again WHILE the catch-up loop runs, and finally as a
// live multi-entity transaction fanned out to an attached hub socket. The
// acceptance is byte-equal convergence: the client store's post-images must
// equal a fresh authoritative bootstrap's canonical post-images exactly —
// zero dropped, zero duplicated, no MustRefetch along the way.
//
// Gated on local Postgres binaries (initdb/pg_ctl — `brew install
// postgresql@16`); machines without them skip instead of fail.

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Effect } from 'effect'
import postgres from 'postgres'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  type BootstrapResponse,
  type ChangelogEntry,
  decodeBootstrapResponse,
  decodeLiveFrame,
  decodeLogPage,
  encodeChangelogEntry,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  type LogPage,
  personalScope,
  SyncVersion,
} from '@openagentsinc/khala-sync'
import { type ConfirmedEntity } from '@openagentsinc/khala-sync-client'
import { openKhalaSyncStore } from '@openagentsinc/khala-sync-client/sqlite-store'
import { type SyncSql, withSyncTransaction } from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  type LocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'

import {
  handleKhalaSyncBootstrap,
  KHALA_SYNC_BOOTSTRAP_PATH,
} from './khala-sync-bootstrap-routes'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import { handleKhalaSyncLog, KHALA_SYNC_LOG_PATH } from './khala-sync-log-routes'
import type { MakeKhalaSyncPushSqlClient } from './khala-sync-push-routes'
import { FakeWebSocket, makeHub } from './test/khala-sync-hub-do-harness'

const USER_ID = 'seam-user'
const SCOPE = personalScope(USER_ID)
const DOC = EntityType.make('doc')

const MIGRATIONS_DIR = join(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations',
)

/** Tiny hub window so catch-up hits BOTH hub and Postgres serving paths. */
const HUB_WINDOW_MAX_ENTRIES = '4'

const BOOTSTRAP_PAGE_SIZE = 8
const CATCHUP_LIMIT = 3

describe.skipIf(!hasLocalPostgres())(
  'KS-4.4 stitch seam: bootstrap under concurrent writes → catch-up → live tail',
  () => {
    let pg: LocalPostgres
    let dbUrl: string
    let writerSql: ReturnType<typeof postgres>

    const hubHarness = makeHub({
      KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES: HUB_WINDOW_MAX_ENTRIES,
    })
    const hub = hubHarness.hub

    const hubNamespace: KhalaSyncHubNamespaceLike = {
      get: () => ({ fetch: request => hub.fetch(request) }),
      idFromName: name => name,
    }

    // The exact production postgres.js client discipline (SPEC §4): one
    // connection, unnamed statements only, torn down per request by the
    // routes' finally blocks.
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

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = postgres(pg.url, { max: 1, onnotice: () => {} })
      await admin.unsafe('CREATE DATABASE khala_sync_seam')
      await admin.end({ timeout: 5 })
      dbUrl = pg.urlFor('khala_sync_seam')

      // Apply the real migration set (the Bun migration runner is a Bun-only
      // module; this vitest suite applies the same ordered .sql files over
      // postgres.js — one transaction per file, like the runner).
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
    // Writer + capture roles
    // -----------------------------------------------------------------------

    type Change =
      | { readonly id: string; readonly op: 'upsert'; readonly postImage: unknown }
      | { readonly id: string; readonly op: 'delete' }

    /**
     * ONE committed Postgres transaction through the real outbox writer
     * (multi-entity changes share one version — SPEC §2.3), then the
     * capture role: append the committed entries to the real hub DO.
     */
    const commit = async (
      changes: ReadonlyArray<Change>,
    ): Promise<ReadonlyArray<ChangelogEntry>> => {
      const entries = await withSyncTransaction(
        writerSql as unknown as SyncSql,
        async writer => {
          const out: Array<ChangelogEntry> = []
          for (const change of changes) {
            out.push(
              await writer.appendChange(
                change.op === 'upsert'
                  ? {
                      entityId: EntityId.make(change.id),
                      entityType: DOC,
                      op: 'upsert',
                      postImage: change.postImage,
                      scope: SCOPE,
                    }
                  : {
                      entityId: EntityId.make(change.id),
                      entityType: DOC,
                      op: 'delete',
                      scope: SCOPE,
                    },
              ),
            )
          }
          return out
        },
      )
      const appendResponse = await hub.fetch(
        new Request('https://khala-sync-hub.openagents.internal/append', {
          body: JSON.stringify({
            entries: entries.map(entry => encodeChangelogEntry(entry)),
            scope: SCOPE,
          }),
          method: 'POST',
        }),
      )
      expect(appendResponse.status).toBe(200)
      return entries
    }

    const doc = (id: string, rev: number) => ({
      body: `body of ${id} at rev ${rev}`,
      id,
      rev,
    })

    // -----------------------------------------------------------------------
    // Route-level client transport (the REAL Worker handlers)
    // -----------------------------------------------------------------------

    const bootstrapPageVia = async (
      pageToken: string | undefined,
    ): Promise<BootstrapResponse> => {
      const response = await Effect.runPromise(
        handleKhalaSyncBootstrap(
          new Request(`https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`, {
            body: JSON.stringify({
              clientGroupId: 'cg-seam',
              pageSize: BOOTSTRAP_PAGE_SIZE,
              protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
              schemaVersion: 1,
              scope: SCOPE,
              ...(pageToken === undefined ? {} : { pageToken }),
            }),
            method: 'POST',
          }),
          {
            authenticate: async () => ({ userId: USER_ID }),
            binding: { connectionString: dbUrl },
            makeSqlClient: makeRealSqlClient,
            resolveScopeRead: async () => ({ kind: 'allowed' }),
          },
        ),
      )
      expect(response.status).toBe(200)
      expect(response.headers.get('cache-control')).toBe('no-store')
      return decodeBootstrapResponse(await response.json())
    }

    const logPageVia = async (cursor: number): Promise<LogPage> => {
      const url = new URL(`https://openagents.com${KHALA_SYNC_LOG_PATH}`)
      url.searchParams.set('scope', SCOPE)
      url.searchParams.set('cursor', String(cursor))
      url.searchParams.set('limit', String(CATCHUP_LIMIT))
      const response = await Effect.runPromise(
        handleKhalaSyncLog(new Request(url.toString(), { method: 'GET' }), {
          authenticate: async () => ({ userId: USER_ID }),
          binding: { connectionString: dbUrl },
          hubNamespace,
          makeSqlClient: makeRealSqlClient,
          resolveScopeRead: async () => ({ kind: 'allowed' }),
        }),
      )
      expect(response.status).toBe(200)
      return decodeLogPage(await response.json())
    }

    const entityKey = (entityType: string, entityId: string): string =>
      `${entityType} ${entityId}`

    /** Fresh authoritative snapshot: canonical post-images by entity key. */
    const drainAuthoritativeBootstrap = async (): Promise<{
      cursor: number
      images: Map<string, string>
    }> => {
      const images = new Map<string, string>()
      let pageToken: string | undefined
      for (;;) {
        const page = await bootstrapPageVia(pageToken)
        for (const entity of page.entities) {
          const key = entityKey(String(entity.entityType), String(entity.entityId))
          expect(images.has(key)).toBe(false) // no duplicates across pages
          images.set(key, entity.postImageJson)
        }
        if (page.nextPageToken === undefined) {
          return { cursor: Number(page.cursor), images }
        }
        pageToken = page.nextPageToken
        expect(images.size).toBeLessThan(10_000) // paranoia: no infinite paging
      }
    }

    // -----------------------------------------------------------------------
    // THE seam test
    // -----------------------------------------------------------------------

    test(
      'client converges byte-equal under writes committed while paging, catching up, and live-tailing',
      { timeout: 120_000 },
      async () => {
        // ---- seed: 30 entities, one committed transaction each (v1..v30) --
        const seedIds = Array.from({ length: 30 }, (_, i) =>
          `doc-${String(i).padStart(3, '0')}`,
        )
        for (const id of seedIds) {
          await commit([{ id, op: 'upsert', postImage: doc(id, 0) }])
        }

        const store = openKhalaSyncStore(':memory:')

        // ---- (b) start the bootstrap: page 1 pins the snapshot cursor -----
        const snapshotEntities: Array<ConfirmedEntity> = []
        const collect = (page: BootstrapResponse): void => {
          for (const entity of page.entities) {
            snapshotEntities.push({
              entityId: String(entity.entityId),
              entityType: String(entity.entityType),
              postImageJson: entity.postImageJson,
              // Snapshot rows are consistent AT the stitch cursor; entries
              // that changed after it arrive via the log with higher
              // versions and win the store's skip-stale apply.
              version: SyncVersion.make(30),
            })
          }
        }

        const page1 = await bootstrapPageVia(undefined)
        expect(page1.nextPageToken).toBeDefined()
        expect(page1.cursor).toBeUndefined()
        collect(page1)

        // ---- (c) wave A: concurrent writes WHILE paging --------------------
        // update an entity already served by page 1 (the classic dropped-
        // update trap), delete an already-served one, create a new one, and
        // update one the snapshot has NOT served yet.
        await commit([{ id: 'doc-002', op: 'upsert', postImage: doc('doc-002', 1) }]) // v31
        await commit([{ id: 'doc-004', op: 'delete' }]) // v32
        await commit([{ id: 'doc-zzz', op: 'upsert', postImage: doc('doc-zzz', 0) }]) // v33
        await commit([{ id: 'doc-020', op: 'upsert', postImage: doc('doc-020', 1) }]) // v34

        const page2 = await bootstrapPageVia(page1.nextPageToken)
        expect(page2.nextPageToken).toBeDefined()
        collect(page2)

        // wave B: another interleave — delete a not-yet-served entity and
        // create one that sorts BEFORE the pagination keyset (it must ride
        // the log, not the snapshot).
        await commit([{ id: 'doc-021', op: 'delete' }]) // v35
        await commit([{ id: 'doc-000a', op: 'upsert', postImage: doc('doc-000a', 0) }]) // v36

        let lastPage = page2
        let pages = 2
        while (lastPage.nextPageToken !== undefined) {
          lastPage = await bootstrapPageVia(lastPage.nextPageToken)
          pages += 1
          collect(lastPage)
          expect(pages).toBeLessThan(20)
        }
        expect(pages).toBeGreaterThan(2) // genuinely multi-page under writes

        // The snapshot is EXACTLY the seed set at v30 — none of the writes
        // committed while paging leaked in (self-contained token proof), and
        // nothing was dropped or duplicated.
        const stitchCursor = Number(lastPage.cursor)
        expect(stitchCursor).toBe(30)
        expect(snapshotEntities.map(e => e.entityId).sort()).toEqual(
          [...seedIds].sort(),
        )
        expect(
          JSON.parse(
            snapshotEntities.find(e => e.entityId === 'doc-002')!.postImageJson,
          ),
        ).toEqual(doc('doc-002', 0)) // pre-update image: the log will fix it

        // ---- (d) apply the snapshot: resetScope at the stitch cursor ------
        Effect.runSync(
          store.resetScope(SCOPE, snapshotEntities, SyncVersion.make(stitchCursor)),
        )

        // ---- catch up from EXACTLY the stitch cursor (hub-first route; the
        // tiny hub window forces early pages through the authoritative
        // Postgres fallthrough and serves later ones from the DO window).
        // Every page is applied TWICE: delivery is at-least-once and apply
        // must be idempotent (invariant 4).
        let cursor = stitchCursor
        let catchupPages = 0
        let injectedMidCatchupWrites = false
        for (;;) {
          const page = await logPageVia(cursor)
          if (page.entries.length > 0) {
            const applyCursor = SyncVersion.make(Number(page.nextCursor))
            Effect.runSync(store.applyConfirmed(SCOPE, page.entries, applyCursor))
            Effect.runSync(store.applyConfirmed(SCOPE, page.entries, applyCursor)) // redelivery
          }
          cursor = Number(page.nextCursor)
          catchupPages += 1
          expect(catchupPages).toBeLessThan(50)
          if (!injectedMidCatchupWrites) {
            // wave C: more writes land WHILE the client is catching up.
            injectedMidCatchupWrites = true
            await commit([{ id: 'doc-002', op: 'upsert', postImage: doc('doc-002', 2) }]) // v37
            await commit([{ id: 'doc-live', op: 'upsert', postImage: doc('doc-live', 0) }]) // v38
            continue
          }
          if (page.upToDate) break
        }
        expect(catchupPages).toBeGreaterThan(1)
        expect(cursor).toBe(38)

        // ---- live tail: attach a REAL hub socket at the caught-up cursor --
        const socket = new FakeWebSocket()
        hub.attachSocket(socket, SCOPE, cursor)
        expect(socket.sent).toHaveLength(0) // at the edge: no replay needed

        // wave D: ONE multi-entity transaction (one version group → one
        // DeltaFrame): update, delete, create together.
        const waveD = await commit([
          { id: 'doc-000', op: 'upsert', postImage: doc('doc-000', 1) },
          { id: 'doc-zzz', op: 'delete' },
          { id: 'doc-final', op: 'upsert', postImage: doc('doc-final', 0) },
        ])
        expect(new Set(waveD.map(e => Number(e.version))).size).toBe(1) // v39

        const frames = socket.sent.map(text =>
          decodeLiveFrame(JSON.parse(text) as unknown),
        )
        expect(frames.map(f => f._tag)).toEqual(['DeltaFrame'])
        for (const frame of frames) {
          if (frame._tag !== 'DeltaFrame') continue
          const applyCursor = SyncVersion.make(Number(frame.cursor))
          Effect.runSync(store.applyConfirmed(SCOPE, frame.entries, applyCursor))
          Effect.runSync(store.applyConfirmed(SCOPE, frame.entries, applyCursor)) // redelivery
        }
        expect(socket.closed).toBeUndefined() // never MustRefetch'd

        // ---- (e) byte-equal convergence against a fresh authoritative
        // bootstrap (canonical post-images), zero dropped / duplicated.
        const authoritative = await drainAuthoritativeBootstrap()
        expect(authoritative.cursor).toBe(39)

        const clientEntities = Effect.runSync(store.readEntities(SCOPE))
        const clientImages = new Map(
          clientEntities.map(entity => [
            entityKey(entity.entityType, entity.entityId),
            entity.postImageJson,
          ]),
        )
        expect(clientEntities.length).toBe(clientImages.size) // store PK sanity

        // Exact same entity set…
        expect([...clientImages.keys()].sort()).toEqual(
          [...authoritative.images.keys()].sort(),
        )
        // …with byte-identical canonical post-images.
        for (const [key, image] of authoritative.images) {
          expect(clientImages.get(key)).toBe(image)
        }

        // Spot-check the semantics the seam exists to protect:
        const clientImage = (id: string) =>
          clientImages.get(entityKey(String(DOC), id))
        expect(clientImage('doc-004')).toBeUndefined() // deleted while paging
        expect(clientImage('doc-021')).toBeUndefined() // deleted while paging
        expect(clientImage('doc-zzz')).toBeUndefined() // created, then deleted live
        expect(JSON.parse(clientImage('doc-002')!)).toEqual(doc('doc-002', 2)) // update chain wins
        expect(JSON.parse(clientImage('doc-020')!)).toEqual(doc('doc-020', 1)) // unserved-at-snapshot update
        expect(JSON.parse(clientImage('doc-000')!)).toEqual(doc('doc-000', 1)) // live update
        expect(clientImage('doc-000a')).toBeDefined() // created behind the keyset
        expect(clientImage('doc-live')).toBeDefined() // created mid-catch-up
        expect(clientImage('doc-final')).toBeDefined() // created live
        // 30 seed − 2 deleted + 3 created (doc-zzz created+deleted nets out):
        expect(clientImages.size).toBe(31)

        // The client's durable cursor ended at the authoritative version.
        expect(Number(Effect.runSync(store.cursor(SCOPE)))).toBe(39)

        Effect.runSync(store.close())
      },
    )
  },
)
