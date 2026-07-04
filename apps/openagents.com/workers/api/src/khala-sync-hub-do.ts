// KhalaSyncHubDO — per-scope Khala Sync hub Durable Object (KS-4.2, #8295).
//
// One DO per scope (`idFromName(scope)`), per docs/khala-sync/SPEC.md §5. The
// hub is a CACHE AND FAN-OUT LAYER ONLY: Postgres is authoritative, no
// business writes originate here, and a fresh/reset DO starts empty and
// rebuilds from capture appends. It holds the recent changelog window in DO
// SQLite and serves three internal surfaces:
//
//   POST /append   — capture (or same-Worker post-commit push) appends a batch
//                    of encoded ChangelogEntry rows. Scope-checked, versions
//                    strictly ascending across version groups (multiple
//                    entries MAY share one version — one transaction touching
//                    several entities, matching the changelog primary key
//                    (scope, version, entity_type, entity_id)), dense with the
//                    window edge, and idempotent on replay (entries with
//                    version <= last_version are ignored). On append the hub
//                    fans out DeltaFrames to hibernated sockets.
//   GET  /connect  — WebSocket upgrade via the Hibernation API
//                    (`state.acceptWebSocket`); the per-socket cursor lives in
//                    `serializeAttachment` so hibernation survives isolate
//                    eviction. On connect the socket is caught up from its
//                    cursor out of the window, or told to MustRefetch when the
//                    cursor is behind the retained window.
//   GET  /log      — offset-resumable LogPage catch-up served from the window
//                    when the requested range is inside it. The hub NEVER
//                    falls through to Postgres itself — the route layer does
//                    that (KS-4.3). Error contract (documented choice):
//                      * 410 Gone + SyncError{cursor_behind_retained_window,
//                        retryable:false} — window empty, or
//                        cursor < window_start_version - 1. 410 because the
//                        requested log range is permanently gone from this
//                        cache; the caller must re-bootstrap or read Postgres.
//                      * 409 Conflict + SyncError{storage_unavailable,
//                        retryable:true} — cursor > last_version (client ahead
//                        of a reset/rebuilding hub); the range is not YET in
//                        the window, so the route layer must serve it from
//                        Postgres instead of reporting a false `upToDate`.
//
// Window bounds come from `@openagentsinc/khala-sync-server`
// (HUB_WINDOW_MAX_ENTRIES / HUB_WINDOW_MAX_BYTES); eviction removes whole
// OLDEST version groups past either bound, advancing window_start_version,
// and always retains the newest version group. Env vars
// KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES / _MAX_BYTES may override the bounds
// (test/ops knob only; production uses the package constants).
//
// The wire frames are the khala-sync codecs (DeltaFrame / MustRefetchFrame /
// PingFrame via `encodeLiveFrame`). `webSocketMessage` handles PingFrame only
// — this is a server→client delta channel; mutations go through HTTP push
// (SPEC §3). Ping/pong is ALSO configured through
// `setWebSocketAutoResponse`, so steady-state keepalive never wakes the DO
// out of hibernation.

import {
  ChangelogEntry,
  DeltaFrame,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  MustRefetchFrame,
  PingFrame,
  SyncError,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  decodeChangelogEntry,
  decodeLiveFrame,
  encodeChangelogEntry,
  encodeLiveFrame,
} from '@openagentsinc/khala-sync'
import {
  HUB_WINDOW_MAX_BYTES,
  HUB_WINDOW_MAX_ENTRIES,
} from '@openagentsinc/khala-sync-server/hub'
import { Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'

type HttpResponse = globalThis.Response

// ---------------------------------------------------------------------------
// Internal worker route paths (registered in index.ts; admin bearer only —
// the public /api/sync/* surfaces land with KS-4.3/4.4)
// ---------------------------------------------------------------------------

export const KHALA_SYNC_HUB_APPEND_PATH = '/api/internal/khala-sync/hub/append'
export const KHALA_SYNC_HUB_LOG_PATH = '/api/internal/khala-sync/hub/log'
export const KHALA_SYNC_HUB_CONNECT_PATH =
  '/api/internal/khala-sync/hub/connect'

export const KHALA_SYNC_HUB_ROUTE_REF = 'route.internal.khala_sync.hub.v0_1'

/** /log page sizing: soft entry cap per page (whole version groups only). */
export const KHALA_SYNC_HUB_LOG_DEFAULT_LIMIT = 500
export const KHALA_SYNC_HUB_LOG_MAX_LIMIT = 1000

// ---------------------------------------------------------------------------
// Structural runtime types (subset of the Workers runtime surface, so unit
// tests can drive the REAL class over node:sqlite + fake sockets — the same
// idiom as durable-inference-real-do.test.ts)
// ---------------------------------------------------------------------------

export type HubSqlCursorLike<T> = Readonly<{
  toArray: () => Array<T>
  one: () => T | undefined
}>

export type HubSqlStorageLike = Readonly<{
  exec: <T = Record<string, unknown>>(
    query: string,
    ...bindings: Array<unknown>
  ) => HubSqlCursorLike<T>
}>

export type HubWebSocketLike = Readonly<{
  send: (message: string) => void
  close: (code?: number, reason?: string) => void
  serializeAttachment: (value: unknown) => void
  deserializeAttachment: () => unknown
}>

export type KhalaSyncHubStateLike = Readonly<{
  storage: Readonly<{ sql: HubSqlStorageLike }>
  blockConcurrencyWhile: <T>(fn: () => Promise<T>) => Promise<T>
  acceptWebSocket: (ws: HubWebSocketLike) => void
  getWebSockets: () => Array<HubWebSocketLike>
  /** Present on real DurableObjectState; optional so plain fakes still work. */
  setWebSocketAutoResponse?: (pair: unknown) => void
}>

export type KhalaSyncHubEnv = Readonly<{
  /** Test/ops overrides for the window bounds; defaults are the package constants. */
  KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES?: string
  KHALA_SYNC_HUB_WINDOW_MAX_BYTES?: string
}>

// ---------------------------------------------------------------------------
// Codec helpers
// ---------------------------------------------------------------------------

const decodeScope = S.decodeUnknownSync(SyncScope)
const encodeLogPage = S.encodeSync(LogPage)
const encodeSyncError = S.encodeSync(SyncError)

const liveFrameText = (
  frame: DeltaFrame | MustRefetchFrame | PingFrame,
): string => JSON.stringify(encodeLiveFrame(frame))

/** The exact keepalive text configured as the hibernation auto-response. */
export const KHALA_SYNC_HUB_PING_TEXT = liveFrameText(new PingFrame())

const utf8ByteLength = (text: string): number =>
  new TextEncoder().encode(text).byteLength

const json = (value: unknown, init: ResponseInit = {}): HttpResponse =>
  hubJsonResponse(value, init)

const hubJsonResponse = (
  value: unknown,
  init: ResponseInit = {},
): HttpResponse => {
  const headers = new Headers(init.headers)
  headers.set('cache-control', 'no-store')

  return Response.json(value, { ...init, headers })
}

const hubMethodNotAllowed = (
  allowedMethods: ReadonlyArray<string>,
): HttpResponse => {
  const headers = new Headers({
    allow: allowedMethods.join(', '),
    'cache-control': 'no-store',
  })

  return hubJsonResponse({ error: 'method_not_allowed' }, { status: 405, headers })
}

const syncErrorResponse = (
  status: number,
  code: SyncError['code'],
  messageSafe: string,
  retryable: boolean,
): HttpResponse =>
  json(encodeSyncError(new SyncError({ code, messageSafe, retryable })), {
    status,
  })

const behindWindowResponse = (): HttpResponse =>
  syncErrorResponse(
    410,
    'cursor_behind_retained_window',
    'Cursor is behind the hub retained window; re-bootstrap or serve from Postgres.',
    false,
  )

const aheadOfWindowResponse = (): HttpResponse =>
  syncErrorResponse(
    409,
    'storage_unavailable',
    'Cursor is ahead of the hub window (hub is empty or rebuilding); serve from Postgres.',
    true,
  )

const parseNonNegativeInt = (raw: string | null): number | undefined => {
  if (raw === null || raw.trim() === '' || !/^\d+$/.test(raw.trim())) {
    return undefined
  }
  const value = Number.parseInt(raw.trim(), 10)
  return Number.isSafeInteger(value) && value >= 0 ? value : undefined
}

const boundFromEnv = (raw: string | undefined, fallback: number): number => {
  if (raw === undefined) return fallback
  const parsed = Number.parseInt(raw, 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback
}

type EntryRow = Readonly<{ payload: string; version: number }>

type MetaRow = Readonly<{
  last_version: number
  scope: string
  window_start_version: number
}>

type SocketAttachment = Readonly<{ cursor: number }>

const readAttachmentCursor = (ws: HubWebSocketLike): number | undefined => {
  const raw = ws.deserializeAttachment()
  if (typeof raw !== 'object' || raw === null) return undefined
  const cursor = (raw as Record<string, unknown>).cursor
  return typeof cursor === 'number' && Number.isSafeInteger(cursor) && cursor >= 0
    ? cursor
    : undefined
}

const groupByVersion = (
  entries: ReadonlyArray<ChangelogEntry>,
): Array<{ entries: Array<ChangelogEntry>; version: number }> => {
  const groups: Array<{ entries: Array<ChangelogEntry>; version: number }> = []
  for (const entry of entries) {
    const last = groups[groups.length - 1]
    if (last !== undefined && last.version === entry.version) {
      last.entries.push(entry)
    } else {
      groups.push({ entries: [entry], version: entry.version })
    }
  }
  return groups
}

// ---------------------------------------------------------------------------
// The Durable Object
// ---------------------------------------------------------------------------

export class KhalaSyncHubDO {
  private readonly maxEntries: number
  private readonly maxBytes: number

  constructor(
    private readonly state: KhalaSyncHubStateLike,
    env: KhalaSyncHubEnv = {},
  ) {
    this.maxEntries = boundFromEnv(
      env.KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES,
      HUB_WINDOW_MAX_ENTRIES,
    )
    this.maxBytes = boundFromEnv(
      env.KHALA_SYNC_HUB_WINDOW_MAX_BYTES,
      HUB_WINDOW_MAX_BYTES,
    )

    state.blockConcurrencyWhile(async () => {
      // Cloudflare's sql.exec accepts multi-statement DDL, but single
      // statements keep the class drivable by single-statement test adapters.
      this.state.storage.sql.exec(`
        CREATE TABLE IF NOT EXISTS entries (
          version INTEGER NOT NULL,
          entity_type TEXT NOT NULL,
          entity_id TEXT NOT NULL,
          payload TEXT NOT NULL,
          payload_bytes INTEGER NOT NULL,
          PRIMARY KEY (version, entity_type, entity_id)
        )`)
      this.state.storage.sql.exec(`
        CREATE TABLE IF NOT EXISTS meta (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          scope TEXT NOT NULL,
          window_start_version INTEGER NOT NULL,
          last_version INTEGER NOT NULL
        )`)
    })

    // Keepalive without waking the DO: the runtime answers matching ping
    // text itself while the object stays hibernated.
    const PairCtor = (
      globalThis as {
        WebSocketRequestResponsePair?: new (
          request: string,
          response: string,
        ) => unknown
      }
    ).WebSocketRequestResponsePair
    if (
      PairCtor !== undefined &&
      typeof state.setWebSocketAutoResponse === 'function'
    ) {
      state.setWebSocketAutoResponse(
        new PairCtor(KHALA_SYNC_HUB_PING_TEXT, KHALA_SYNC_HUB_PING_TEXT),
      )
    }
  }

  async fetch(request: Request): Promise<HttpResponse> {
    const url = new URL(request.url)
    if (url.pathname === '/append') {
      if (request.method !== 'POST') return hubMethodNotAllowed(['POST'])
      return this.handleAppend(request)
    }
    if (url.pathname === '/log') {
      if (request.method !== 'GET') return hubMethodNotAllowed(['GET'])
      return this.handleLog(url)
    }
    if (url.pathname === '/connect') {
      if (request.method !== 'GET') return hubMethodNotAllowed(['GET'])
      return this.handleConnect(request, url)
    }
    return json({ error: 'not_found' }, { status: 404 })
  }

  // -------------------------------------------------------------------------
  // Hibernation handlers
  // -------------------------------------------------------------------------

  /**
   * Server→client delta channel: the only client-initiated frame is
   * PingFrame (mutations go through HTTP push, SPEC §3). Ping normally hits
   * the auto-response pair without waking the DO; this handler answers pings
   * whose serialization differs from the configured pair text. Anything else
   * is ignored (bounded: no state transitions originate from inbound frames).
   */
  webSocketMessage(ws: HubWebSocketLike, message: string | ArrayBuffer): void {
    if (typeof message !== 'string') return
    try {
      const frame = decodeLiveFrame(parseJsonUnknown(message))
      if (frame._tag === 'PingFrame') {
        ws.send(KHALA_SYNC_HUB_PING_TEXT)
      }
    } catch {
      // Undecodable inbound data on a server→client channel: ignore.
    }
  }

  webSocketClose(): void {
    // Hibernation API prunes closed sockets from getWebSockets(); no state.
  }

  webSocketError(): void {
    // Same as close: nothing to clean up — the cursor lives on the socket.
  }

  // -------------------------------------------------------------------------
  // /append
  // -------------------------------------------------------------------------

  private async handleAppend(request: Request): Promise<HttpResponse> {
    const body = (await request.json().catch(() => undefined)) as
      | Record<string, unknown>
      | undefined
    if (body === undefined || typeof body.scope !== 'string') {
      return json(
        { error: 'khala_sync_hub_append_invalid', reason: 'missing scope' },
        { status: 400 },
      )
    }
    if (!Array.isArray(body.entries) || body.entries.length === 0) {
      return json(
        {
          error: 'khala_sync_hub_append_invalid',
          reason: 'entries must be a non-empty array',
        },
        { status: 400 },
      )
    }

    let scope: SyncScope
    let entries: Array<ChangelogEntry>
    try {
      scope = decodeScope(body.scope)
      entries = body.entries.map(raw => decodeChangelogEntry(raw))
    } catch (error) {
      return json(
        {
          error: 'khala_sync_hub_append_invalid',
          reason:
            error instanceof Error ? error.message.slice(0, 300) : 'undecodable',
        },
        { status: 400 },
      )
    }

    // Scope match: every entry belongs to THIS hub's scope.
    if (entries.some(entry => entry.scope !== scope)) {
      return json({ error: 'khala_sync_hub_scope_mismatch' }, { status: 409 })
    }
    const pinned = this.pinScope(scope)
    if (!pinned) {
      return json({ error: 'khala_sync_hub_scope_mismatch' }, { status: 409 })
    }

    // Versions strictly ascending across version groups (non-decreasing
    // across entries: one version group may hold several entities).
    for (let i = 1; i < entries.length; i++) {
      if (entries[i]!.version < entries[i - 1]!.version) {
        return json(
          {
            error: 'khala_sync_hub_append_invalid',
            reason: 'entry versions must be ascending',
          },
          { status: 400 },
        )
      }
    }

    const meta = this.meta()!
    const previousLastVersion = meta.last_version

    // Idempotent on replay: entries at or below the window edge were already
    // appended (delivery is at-least-once); drop them silently.
    const fresh = entries.filter(entry => entry.version > previousLastVersion)
    if (fresh.length === 0) {
      return json({
        appended: 0,
        duplicates: entries.length,
        lastVersion: previousLastVersion,
        ok: true,
        windowStartVersion: meta.window_start_version,
      })
    }

    // Density with the window edge (SPEC invariant 1: versions are dense).
    // A gapped append would make the window lie about contiguity, so reject
    // it and let the producer resync. A fresh/reset hub (empty window)
    // accepts any starting version — that IS the rehydrate path.
    const versions = [...new Set(fresh.map(entry => entry.version))]
    const expectedFirst = previousLastVersion + 1
    if (previousLastVersion > 0 && versions[0]! !== expectedFirst) {
      return json(
        {
          error: 'khala_sync_hub_version_gap',
          expectedFirstVersion: expectedFirst,
          receivedFirstVersion: versions[0],
        },
        { status: 409 },
      )
    }
    for (let i = 1; i < versions.length; i++) {
      if (versions[i]! !== versions[i - 1]! + 1) {
        return json(
          {
            error: 'khala_sync_hub_version_gap',
            expectedFirstVersion: versions[i - 1]! + 1,
            receivedFirstVersion: versions[i],
          },
          { status: 409 },
        )
      }
    }

    const sql = this.state.storage.sql
    for (const entry of fresh) {
      const payload = JSON.stringify(encodeChangelogEntry(entry))
      sql.exec(
        `INSERT OR REPLACE INTO entries
           (version, entity_type, entity_id, payload, payload_bytes)
         VALUES (?, ?, ?, ?, ?)`,
        entry.version,
        entry.entityType,
        entry.entityId,
        payload,
        utf8ByteLength(payload),
      )
    }

    const newLastVersion = versions[versions.length - 1]!
    const windowStart =
      meta.window_start_version > 0 ? meta.window_start_version : versions[0]!
    sql.exec(
      'UPDATE meta SET window_start_version = ?, last_version = ? WHERE id = 1',
      windowStart,
      newLastVersion,
    )

    this.evict()
    const after = this.meta()!

    this.fanOut(scope, fresh, newLastVersion, after)

    return json({
      appended: fresh.length,
      duplicates: entries.length - fresh.length,
      lastVersion: after.last_version,
      ok: true,
      windowStartVersion: after.window_start_version,
    })
  }

  /**
   * Evict whole OLDEST version groups while the window exceeds either bound,
   * advancing window_start_version. Always retains the newest version group
   * (never split a group: a cursor inside a version would corrupt catch-up).
   */
  private evict(): void {
    const sql = this.state.storage.sql
    for (;;) {
      const stats = sql
        .exec<{
          bytes: number | null
          count: number
          hi: number | null
          lo: number | null
        }>(
          `SELECT COUNT(*) AS count, SUM(payload_bytes) AS bytes,
                  MIN(version) AS lo, MAX(version) AS hi
             FROM entries`,
        )
        .one()
      if (
        stats === undefined ||
        stats.count === 0 ||
        stats.lo === null ||
        stats.hi === null ||
        stats.lo === stats.hi
      ) {
        break
      }
      if (stats.count <= this.maxEntries && (stats.bytes ?? 0) <= this.maxBytes) {
        break
      }
      sql.exec('DELETE FROM entries WHERE version = ?', stats.lo)
    }

    const lowest = sql
      .exec<{ lo: number | null }>('SELECT MIN(version) AS lo FROM entries')
      .one()
    if (lowest !== undefined && lowest.lo !== null) {
      sql.exec(
        'UPDATE meta SET window_start_version = ? WHERE id = 1',
        lowest.lo,
      )
    }
  }

  /**
   * Post-append fan-out over hibernated sockets:
   *   - cursor at the window edge (or inside the batch) — i.e. the batch
   *     alone covers everything past its cursor → ONE DeltaFrame per version
   *     group of the entries it is missing, straight from the in-memory
   *     batch (contiguity check is against the batch's FIRST version, not
   *     the previous edge, so a reset hub rebuilding mid-stream never sends
   *     a gapped batch to a far-behind socket);
   *   - cursor behind the batch but still covered by the (post-eviction)
   *     window → the missing entries from the window as DeltaFrames in order;
   *   - cursor behind the retained window → MustRefetch + close (invariant 6:
   *     never a partial log).
   */
  private fanOut(
    scope: SyncScope,
    batch: ReadonlyArray<ChangelogEntry>,
    newLastVersion: number,
    window: MetaRow,
  ): void {
    const batchFirstVersion = batch[0]!.version
    for (const ws of this.state.getWebSockets()) {
      const cursor = readAttachmentCursor(ws)
      if (cursor === undefined) {
        this.mustRefetch(ws, scope, 'scope_reset')
        continue
      }
      if (cursor >= newLastVersion) continue

      try {
        if (cursor >= batchFirstVersion - 1) {
          this.sendDeltaFrames(
            ws,
            scope,
            batch.filter(entry => entry.version > cursor),
          )
          ws.serializeAttachment({ cursor: newLastVersion })
        } else if (cursor >= window.window_start_version - 1) {
          this.sendDeltaFrames(ws, scope, this.windowEntriesAfter(cursor))
          ws.serializeAttachment({ cursor: newLastVersion })
        } else {
          this.mustRefetch(ws, scope, 'cursor_behind_retained_window')
        }
      } catch {
        // A dead socket must never poison fan-out to the healthy ones.
        try {
          ws.close(1011, 'khala_sync_hub_send_failed')
        } catch {
          // already gone
        }
      }
    }
  }

  private sendDeltaFrames(
    ws: HubWebSocketLike,
    scope: SyncScope,
    entries: ReadonlyArray<ChangelogEntry>,
  ): void {
    for (const group of groupByVersion(entries)) {
      ws.send(
        liveFrameText(
          new DeltaFrame({
            cursor: SyncVersion.make(group.version),
            entries: group.entries,
            scope,
          }),
        ),
      )
    }
  }

  private mustRefetch(
    ws: HubWebSocketLike,
    scope: SyncScope,
    reason: MustRefetchFrame['reason'],
  ): void {
    try {
      ws.send(liveFrameText(new MustRefetchFrame({ reason, scope })))
    } catch {
      // socket already dead; close below is best-effort
    }
    try {
      ws.close(1000, 'khala_sync_must_refetch')
    } catch {
      // already closed
    }
  }

  // -------------------------------------------------------------------------
  // /connect
  // -------------------------------------------------------------------------

  private handleConnect(request: Request, url: URL): HttpResponse {
    if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
      return json(
        { error: 'khala_sync_hub_upgrade_required' },
        { status: 426 },
      )
    }

    const scopeRaw = url.searchParams.get('scope')
    let scope: SyncScope
    try {
      scope = decodeScope(scopeRaw)
    } catch {
      return json(
        { error: 'khala_sync_hub_connect_invalid', reason: 'invalid scope' },
        { status: 400 },
      )
    }
    const cursor = parseNonNegativeInt(url.searchParams.get('cursor') ?? '0')
    if (cursor === undefined) {
      return json(
        { error: 'khala_sync_hub_connect_invalid', reason: 'invalid cursor' },
        { status: 400 },
      )
    }
    if (!this.pinScope(scope)) {
      return json({ error: 'khala_sync_hub_scope_mismatch' }, { status: 409 })
    }

    const PairCtor = (
      globalThis as {
        WebSocketPair?: new () => { 0: WebSocket; 1: WebSocket }
      }
    ).WebSocketPair
    if (PairCtor === undefined) {
      return json(
        { error: 'khala_sync_hub_websocket_unavailable' },
        { status: 500 },
      )
    }
    const pair = new PairCtor()
    const server = pair[1] as unknown as HubWebSocketLike

    this.attachSocket(server, scope, cursor)

    // `webSocket` on ResponseInit is the Workers-runtime upgrade extension.
    return new Response(null, { status: 101, webSocket: pair[0] })
  }

  /**
   * Accept one server-side socket via the Hibernation API and catch it up
   * from `cursor` out of the window. Split from handleConnect so tests can
   * drive the REAL accept/catch-up/attachment logic without the
   * Workers-runtime WebSocketPair/101-response machinery.
   */
  attachSocket(ws: HubWebSocketLike, scope: SyncScope, cursor: number): void {
    this.state.acceptWebSocket(ws)
    ws.serializeAttachment({ cursor } satisfies SocketAttachment)

    const meta = this.meta()
    if (meta === undefined || meta.last_version === 0) {
      // Fresh/reset hub: nothing to catch up from. Keep the socket; the
      // first append decides (its window_start covers or MustRefetches).
      return
    }
    if (cursor >= meta.last_version) {
      // At the edge (or ahead of a rebuilding hub): live tail from here.
      return
    }
    if (cursor < meta.window_start_version - 1) {
      this.mustRefetch(ws, scope, 'cursor_behind_retained_window')
      return
    }
    this.sendDeltaFrames(ws, scope, this.windowEntriesAfter(cursor))
    ws.serializeAttachment({ cursor: meta.last_version } satisfies SocketAttachment)
  }

  // -------------------------------------------------------------------------
  // /log
  // -------------------------------------------------------------------------

  private handleLog(url: URL): HttpResponse {
    const scopeRaw = url.searchParams.get('scope')
    let scope: SyncScope
    try {
      scope = decodeScope(scopeRaw)
    } catch {
      return json(
        { error: 'khala_sync_hub_log_invalid', reason: 'invalid scope' },
        { status: 400 },
      )
    }
    const cursor = parseNonNegativeInt(url.searchParams.get('cursor') ?? '0')
    if (cursor === undefined) {
      return json(
        { error: 'khala_sync_hub_log_invalid', reason: 'invalid cursor' },
        { status: 400 },
      )
    }
    const limitRaw = url.searchParams.get('limit')
    const limit = Math.min(
      limitRaw === null
        ? KHALA_SYNC_HUB_LOG_DEFAULT_LIMIT
        : (parseNonNegativeInt(limitRaw) ?? 0),
      KHALA_SYNC_HUB_LOG_MAX_LIMIT,
    )
    if (limit < 1) {
      return json(
        { error: 'khala_sync_hub_log_invalid', reason: 'invalid limit' },
        { status: 400 },
      )
    }
    if (!this.pinScope(scope)) {
      return json({ error: 'khala_sync_hub_scope_mismatch' }, { status: 409 })
    }

    const meta = this.meta()!
    // Empty window (fresh/reset hub): the hub cannot prove ANY range, so
    // every cursor gets the behind-window error and the route layer serves
    // from authoritative Postgres (task contract, hub-is-cache-only).
    if (meta.last_version === 0) {
      return behindWindowResponse()
    }
    if (cursor < meta.window_start_version - 1) {
      return behindWindowResponse()
    }
    if (cursor > meta.last_version) {
      return aheadOfWindowResponse()
    }

    // Fetch limit+1 rows so a full page can be trimmed to whole version
    // groups; a version group larger than the limit is served whole (a
    // client cursor must never land inside a version).
    const rows = this.state.storage.sql
      .exec<EntryRow>(
        `SELECT version, payload FROM entries
          WHERE version > ?
          ORDER BY version ASC, entity_type ASC, entity_id ASC
          LIMIT ?`,
        cursor,
        limit + 1,
      )
      .toArray()

    let page: Array<EntryRow>
    if (rows.length <= limit) {
      page = rows
    } else {
      const overflowVersion = rows[limit]!.version
      const trimmed = rows
        .slice(0, limit)
        .filter(row => row.version !== overflowVersion)
      page =
        trimmed.length > 0
          ? trimmed
          : // Single version group larger than the limit: serve it whole.
            this.state.storage.sql
              .exec<EntryRow>(
                `SELECT version, payload FROM entries
                  WHERE version = ?
                  ORDER BY entity_type ASC, entity_id ASC`,
                overflowVersion,
              )
              .toArray()
    }

    const entries = page.map(row =>
      decodeChangelogEntry(parseJsonUnknown(row.payload)),
    )
    const nextCursor =
      page.length > 0 ? page[page.length - 1]!.version : cursor

    return json(
      encodeLogPage(
        new LogPage({
          entries,
          // Watermark semantics (KS-2.2): the client's position AFTER this
          // page — highest version in `entries`, or the request cursor when
          // the page is empty.
          nextCursor: SyncVersionWatermark.make(nextCursor),
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          upToDate: nextCursor >= meta.last_version,
        }),
      ),
    )
  }

  // -------------------------------------------------------------------------
  // Window primitives
  // -------------------------------------------------------------------------

  private meta(): MetaRow | undefined {
    return this.state.storage.sql
      .exec<MetaRow>(
        'SELECT scope, window_start_version, last_version FROM meta WHERE id = 1',
      )
      .toArray()[0]
  }

  /**
   * Pin the DO's scope on first contact (the DO cannot read its own
   * idFromName name); afterwards every request must present the same scope.
   */
  private pinScope(scope: string): boolean {
    const meta = this.meta()
    if (meta === undefined) {
      this.state.storage.sql.exec(
        'INSERT INTO meta (id, scope, window_start_version, last_version) VALUES (1, ?, 0, 0)',
        scope,
      )
      return true
    }
    return meta.scope === scope
  }

  private windowEntriesAfter(cursor: number): Array<ChangelogEntry> {
    return this.state.storage.sql
      .exec<EntryRow>(
        `SELECT version, payload FROM entries
          WHERE version > ?
          ORDER BY version ASC, entity_type ASC, entity_id ASC`,
        cursor,
      )
      .toArray()
      .map(row => decodeChangelogEntry(parseJsonUnknown(row.payload)))
  }
}

// ---------------------------------------------------------------------------
// Worker-side internal route proxy (admin bearer, KS-0.2 guard style). The
// public /api/sync/* surfaces are KS-4.3/4.4; until then the hub is reachable
// only through these operator-gated internal paths.
// ---------------------------------------------------------------------------

export type KhalaSyncHubStubLike = Readonly<{
  fetch: (request: Request) => Promise<HttpResponse>
}>

export type KhalaSyncHubNamespaceLike = Readonly<{
  idFromName: (name: string) => unknown
  get: (id: unknown) => KhalaSyncHubStubLike
}>

export type KhalaSyncHubInternalRouteDependencies = Readonly<{
  /** Same admin bearer predicate as the KS-0.2 db-smoke route. */
  requireOperator: () => Promise<boolean>
  /** `env.KHALA_SYNC_HUB` — absent until the DO binding is deployed. */
  namespace: KhalaSyncHubNamespaceLike | undefined
  doPath: '/append' | '/connect' | '/log'
}>

export const handleKhalaSyncHubInternalRoute = async (
  request: Request,
  deps: KhalaSyncHubInternalRouteDependencies,
): Promise<HttpResponse> => {
  const expectedMethod = deps.doPath === '/append' ? 'POST' : 'GET'
  if (request.method !== expectedMethod) {
    return hubMethodNotAllowed([expectedMethod])
  }

  if (!(await deps.requireOperator())) {
    return json({ error: 'unauthorized' }, { status: 401 })
  }

  const url = new URL(request.url)
  const scopeRaw = url.searchParams.get('scope')
  try {
    decodeScope(scopeRaw)
  } catch {
    return json(
      {
        error: 'khala_sync_hub_invalid_scope',
        reason: 'scope query parameter must be a valid Khala Sync scope id',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 400 },
    )
  }

  if (deps.namespace === undefined) {
    return json(
      {
        error: 'khala_sync_hub_binding_missing',
        reason:
          'Durable Object binding (env.KHALA_SYNC_HUB) is absent. Add the ' +
          'durable_objects binding + migration to wrangler.jsonc and deploy.',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 503 },
    )
  }

  const stub = deps.namespace.get(
    deps.namespace.idFromName(scopeRaw as string),
  )
  const target = new URL(
    `https://khala-sync-hub.openagents.internal${deps.doPath}`,
  )
  url.searchParams.forEach((value, key) => {
    target.searchParams.set(key, value)
  })
  // `new Request(url, request)` preserves method, headers (including the
  // WebSocket Upgrade header for /connect), and body.
  return stub.fetch(new Request(target.toString(), request))
}
