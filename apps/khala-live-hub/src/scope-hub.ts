// ScopeHub — per-scope Khala Sync live-hub core (CFG-5, #8520).
//
// The owned Cloud Run replacement for the Worker's `KhalaSyncHubDO`
// (docs/khala-sync/SPEC.md §5). Semantics are a 1:1 port of the DO —
// the wire/HTTP contract is IDENTICAL so capture, the Worker proxy, and
// clients see no change:
//
//   append         — capture appends a batch of encoded ChangelogEntry rows.
//                    Scope-checked, versions ascending across version groups
//                    (multiple entries MAY share one version), dense with the
//                    window edge (409 khala_sync_hub_version_gap +
//                    expectedFirstVersion on a gap), idempotent on replay
//                    (entries with version <= last_version are ignored). On
//                    append the hub fans out DeltaFrames to attached sockets.
//   attachSocket   — live-tail WebSocket attach; the per-socket cursor lives
//                    in an in-process map (no hibernation: this is a
//                    long-lived Bun process). On attach the socket is caught
//                    up from its cursor out of the window, or told to
//                    MustRefetch when the cursor is behind the retained
//                    window.
//   log            — offset-resumable LogPage catch-up served from the window
//                    when the requested range is inside it. The hub NEVER
//                    falls through to Postgres for a log read — the route
//                    layer does that (KS-4.3). Error contract (same as the
//                    DO):
//                      * 410 Gone + SyncError{cursor_behind_retained_window,
//                        retryable:false} — window empty, or
//                        cursor < window_start_version - 1.
//                      * 409 Conflict + SyncError{storage_unavailable,
//                        retryable:true} — cursor > last_version (client
//                        ahead of a reset/rebuilding hub).
//   accessChanged  — broadcast MustRefetch(access_changed) to EVERY socket
//                    and close them all (KS-7.1; the hub holds no identity,
//                    so revocation is scope-wide and correctness is restored
//                    at reconnect through the route layer's resolver).
//
// STATE IS A CACHE: Postgres is authoritative, no business writes originate
// here, and a fresh hub starts empty. Unlike the DO (whose SQLite persisted
// across isolate restarts), a Cloud Run instance restart loses the window —
// that is fine because `hydrate` (src/rebuild.ts) rebuilds the newest window
// from Postgres on first touch, and an empty hub accepts any starting append
// version (the mid-stream rehydrate path the DO already had).
//
// Window bounds come from `@openagentsinc/khala-sync-server/hub`
// (HUB_WINDOW_MAX_ENTRIES / HUB_WINDOW_MAX_BYTES); eviction removes whole
// OLDEST version groups past either bound, advancing window_start_version,
// and always retains the newest version group.
//
// SHARDING EXTENSION POINT (documented, deliberately not built): the service
// owns a `Map<scope, ScopeHub>` behind `LiveHubService` (src/service.ts).
// One instance handles current scale; when one instance is not enough, the
// seam to shard on is `scopeHubFor(scope)` — route each scope to
// `hash(scope) % N` service instances (consistent hashing at the proxy or a
// scope→shard lookup). Nothing in this class assumes a single process beyond
// the in-memory socket map, which is already per-scope.

import { Database } from "bun:sqlite"
import { Schema as S } from "effect"

import {
  type ChangelogEntry,
  decodeChangelogEntry,
  DeltaFrame,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  MustRefetchFrame,
  PingFrame,
  SyncError,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  decodeLiveFrame,
  encodeChangelogEntry,
  encodeLiveFrame,
} from "@openagentsinc/khala-sync"
import {
  HUB_WINDOW_MAX_BYTES,
  HUB_WINDOW_MAX_ENTRIES,
} from "@openagentsinc/khala-sync-server/hub"

// ---------------------------------------------------------------------------
// Wire constants (identical to the DO's)
// ---------------------------------------------------------------------------

export const LIVE_HUB_LOG_DEFAULT_LIMIT = 500
export const LIVE_HUB_LOG_MAX_LIMIT = 1000

const decodeScope = S.decodeUnknownSync(SyncScope)
const encodeLogPage = S.encodeSync(LogPage)
const encodeSyncError = S.encodeSync(SyncError)

const liveFrameText = (
  frame: DeltaFrame | MustRefetchFrame | PingFrame,
): string => JSON.stringify(encodeLiveFrame(frame))

/** The exact keepalive text the server answers pings with (DO parity). */
export const LIVE_HUB_PING_TEXT = liveFrameText(new PingFrame())

const utf8ByteLength = (text: string): number =>
  new TextEncoder().encode(text).byteLength

const json = (value: unknown, init: ResponseInit = {}): Response => {
  const headers = new Headers(init.headers)
  headers.set("cache-control", "no-store")
  return Response.json(value, { ...init, headers })
}

const methodNotAllowed = (allowed: ReadonlyArray<string>): Response =>
  json(
    { error: "method_not_allowed" },
    { status: 405, headers: { allow: allowed.join(", ") } },
  )

const syncErrorResponse = (
  status: number,
  code: SyncError["code"],
  messageSafe: string,
  retryable: boolean,
): Response =>
  json(encodeSyncError(new SyncError({ code, messageSafe, retryable })), {
    status,
  })

const behindWindowResponse = (): Response =>
  syncErrorResponse(
    410,
    "cursor_behind_retained_window",
    "Cursor is behind the hub retained window; re-bootstrap or serve from Postgres.",
    false,
  )

const aheadOfWindowResponse = (): Response =>
  syncErrorResponse(
    409,
    "storage_unavailable",
    "Cursor is ahead of the hub window (hub is empty or rebuilding); serve from Postgres.",
    true,
  )

export const parseNonNegativeInt = (
  raw: string | null,
): number | undefined => {
  if (raw === null || raw.trim() === "" || !/^\d+$/.test(raw.trim())) {
    return undefined
  }
  const value = Number.parseInt(raw.trim(), 10)
  return Number.isSafeInteger(value) && value >= 0 ? value : undefined
}

// ---------------------------------------------------------------------------
// Socket seam (structural, so tests drive the real class with fakes)
// ---------------------------------------------------------------------------

export type HubSocketLike = Readonly<{
  send: (message: string) => void
  close: (code?: number, reason?: string) => void
}>

export type ScopeHubBounds = Readonly<{
  maxEntries?: number | undefined
  maxBytes?: number | undefined
}>

type EntryRow = Readonly<{ payload: string; version: number }>

type MetaRow = Readonly<{
  last_version: number
  window_start_version: number
}>

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
// The per-scope hub
// ---------------------------------------------------------------------------

export class ScopeHub {
  readonly scope: SyncScope
  private readonly db: Database
  private readonly maxEntries: number
  private readonly maxBytes: number
  private readonly sockets = new Map<HubSocketLike, { cursor: number }>()

  constructor(scope: SyncScope, bounds: ScopeHubBounds = {}) {
    this.scope = scope
    this.maxEntries =
      bounds.maxEntries !== undefined && bounds.maxEntries > 0
        ? bounds.maxEntries
        : HUB_WINDOW_MAX_ENTRIES
    this.maxBytes =
      bounds.maxBytes !== undefined && bounds.maxBytes > 0
        ? bounds.maxBytes
        : HUB_WINDOW_MAX_BYTES

    // In-memory SQLite per scope: the same window/eviction SQL the DO ran
    // over DO SQLite, so the semantics port is 1:1 auditably.
    this.db = new Database(":memory:")
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS entries (
        version INTEGER NOT NULL,
        entity_type TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        payload TEXT NOT NULL,
        payload_bytes INTEGER NOT NULL,
        PRIMARY KEY (version, entity_type, entity_id)
      )
    `)
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS meta (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        window_start_version INTEGER NOT NULL,
        last_version INTEGER NOT NULL
      )
    `)
    this.db
      .query(
        "INSERT OR IGNORE INTO meta (id, window_start_version, last_version) VALUES (1, 0, 0)",
      )
      .run()
  }

  /** Currently attached live sockets (test/observability surface). */
  socketCount(): number {
    return this.sockets.size
  }

  /** Window meta (test/observability surface). */
  window(): Readonly<{ lastVersion: number; windowStartVersion: number }> {
    const meta = this.meta()
    return {
      lastVersion: meta.last_version,
      windowStartVersion: meta.window_start_version,
    }
  }

  /** Release the in-memory database (service eviction/shutdown). */
  dispose(): void {
    for (const ws of [...this.sockets.keys()]) {
      try {
        ws.close(1001, "khala_live_hub_shutdown")
      } catch {
        // already gone
      }
    }
    this.sockets.clear()
    this.db.close()
  }

  // -------------------------------------------------------------------------
  // append
  // -------------------------------------------------------------------------

  /**
   * Append a decoded body `{ scope, entries }` (the exact
   * POST /append JSON contract of the DO, including every error body and
   * status). Also used by hydrate (a rebuild is just an append into an
   * empty window — the mid-stream rehydrate path).
   */
  append(body: unknown): Response {
    const record = (typeof body === "object" && body !== null
      ? body
      : undefined) as Record<string, unknown> | undefined
    if (record === undefined || typeof record["scope"] !== "string") {
      return json(
        { error: "khala_sync_hub_append_invalid", reason: "missing scope" },
        { status: 400 },
      )
    }
    if (!Array.isArray(record["entries"]) || record["entries"].length === 0) {
      return json(
        {
          error: "khala_sync_hub_append_invalid",
          reason: "entries must be a non-empty array",
        },
        { status: 400 },
      )
    }

    let scope: SyncScope
    let entries: Array<ChangelogEntry>
    try {
      scope = decodeScope(record["scope"])
      entries = record["entries"].map((raw) => decodeChangelogEntry(raw))
    } catch (error) {
      return json(
        {
          error: "khala_sync_hub_append_invalid",
          reason:
            error instanceof Error
              ? error.message.slice(0, 300)
              : "undecodable",
        },
        { status: 400 },
      )
    }

    if (scope !== this.scope || entries.some((entry) => entry.scope !== scope)) {
      return json({ error: "khala_sync_hub_scope_mismatch" }, { status: 409 })
    }

    // Versions non-decreasing across entries (one version group may hold
    // several entities); strictly ascending across version groups.
    for (let i = 1; i < entries.length; i++) {
      if (entries[i]!.version < entries[i - 1]!.version) {
        return json(
          {
            error: "khala_sync_hub_append_invalid",
            reason: "entry versions must be ascending",
          },
          { status: 400 },
        )
      }
    }

    const meta = this.meta()
    const previousLastVersion = meta.last_version

    // Idempotent on replay (delivery is at-least-once).
    const fresh = entries.filter(
      (entry) => entry.version > previousLastVersion,
    )
    if (fresh.length === 0) {
      return json({
        appended: 0,
        duplicates: entries.length,
        lastVersion: previousLastVersion,
        ok: true,
        windowStartVersion: meta.window_start_version,
      })
    }

    // Density with the window edge (SPEC invariant 1). An empty window
    // accepts any starting version — that IS the rehydrate path.
    const versions = [...new Set(fresh.map((entry) => entry.version))]
    const expectedFirst = previousLastVersion + 1
    if (previousLastVersion > 0 && versions[0]! !== expectedFirst) {
      return json(
        {
          error: "khala_sync_hub_version_gap",
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
            error: "khala_sync_hub_version_gap",
            expectedFirstVersion: versions[i - 1]! + 1,
            receivedFirstVersion: versions[i],
          },
          { status: 409 },
        )
      }
    }

    const insert = this.db.query(
      `INSERT OR REPLACE INTO entries
         (version, entity_type, entity_id, payload, payload_bytes)
       VALUES (?, ?, ?, ?, ?)`,
    )
    for (const entry of fresh) {
      const payload = JSON.stringify(encodeChangelogEntry(entry))
      insert.run(
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
    this.db
      .query(
        "UPDATE meta SET window_start_version = ?, last_version = ? WHERE id = 1",
      )
      .run(windowStart, newLastVersion)

    this.evict()
    const after = this.meta()

    this.fanOut(fresh, newLastVersion, after)

    return json({
      appended: fresh.length,
      duplicates: entries.length - fresh.length,
      lastVersion: after.last_version,
      ok: true,
      windowStartVersion: after.window_start_version,
    })
  }

  /**
   * Evict whole OLDEST version groups while the window exceeds either
   * bound, advancing window_start_version. Always retains the newest
   * version group (never split a group).
   */
  private evict(): void {
    for (;;) {
      const stats = this.db
        .query<
          {
            bytes: number | null
            count: number
            hi: number | null
            lo: number | null
          },
          []
        >(
          `SELECT COUNT(*) AS count, SUM(payload_bytes) AS bytes,
                  MIN(version) AS lo, MAX(version) AS hi
             FROM entries`,
        )
        .get()
      if (
        stats === null ||
        stats.count === 0 ||
        stats.lo === null ||
        stats.hi === null ||
        stats.lo === stats.hi
      ) {
        break
      }
      if (
        stats.count <= this.maxEntries &&
        (stats.bytes ?? 0) <= this.maxBytes
      ) {
        break
      }
      this.db.query("DELETE FROM entries WHERE version = ?").run(stats.lo)
    }

    const lowest = this.db
      .query<{ lo: number | null }, []>(
        "SELECT MIN(version) AS lo FROM entries",
      )
      .get()
    if (lowest !== null && lowest.lo !== null) {
      this.db
        .query("UPDATE meta SET window_start_version = ? WHERE id = 1")
        .run(lowest.lo)
    }
  }

  /**
   * Post-append fan-out (identical three-way policy to the DO):
   *   - cursor at the window edge / inside the batch → DeltaFrames from the
   *     in-memory batch (contiguity checked against the batch's FIRST
   *     version, so a rebuilding hub never sends a gapped batch);
   *   - cursor behind the batch but covered by the post-eviction window →
   *     the missing window entries as DeltaFrames in order;
   *   - cursor behind the retained window → MustRefetch + close.
   */
  private fanOut(
    batch: ReadonlyArray<ChangelogEntry>,
    newLastVersion: number,
    window: MetaRow,
  ): void {
    const batchFirstVersion = batch[0]!.version
    for (const [ws, state] of [...this.sockets.entries()]) {
      const cursor = state.cursor
      if (cursor >= newLastVersion) continue

      try {
        if (cursor >= batchFirstVersion - 1) {
          this.sendDeltaFrames(
            ws,
            batch.filter((entry) => entry.version > cursor),
          )
          state.cursor = newLastVersion
        } else if (cursor >= window.window_start_version - 1) {
          this.sendDeltaFrames(ws, this.windowEntriesAfter(cursor))
          state.cursor = newLastVersion
        } else {
          this.mustRefetch(ws, "cursor_behind_retained_window")
        }
      } catch {
        // A dead socket must never poison fan-out to the healthy ones.
        this.sockets.delete(ws)
        try {
          ws.close(1011, "khala_sync_hub_send_failed")
        } catch {
          // already gone
        }
      }
    }
  }

  private sendDeltaFrames(
    ws: HubSocketLike,
    entries: ReadonlyArray<ChangelogEntry>,
  ): void {
    for (const group of groupByVersion(entries)) {
      ws.send(
        liveFrameText(
          new DeltaFrame({
            cursor: SyncVersion.make(group.version),
            entries: group.entries,
            scope: this.scope,
          }),
        ),
      )
    }
  }

  private mustRefetch(
    ws: HubSocketLike,
    reason: MustRefetchFrame["reason"],
  ): void {
    this.sockets.delete(ws)
    try {
      ws.send(
        liveFrameText(new MustRefetchFrame({ reason, scope: this.scope })),
      )
    } catch {
      // socket already dead; close below is best-effort
    }
    try {
      ws.close(1000, "khala_sync_must_refetch")
    } catch {
      // already closed
    }
  }

  // -------------------------------------------------------------------------
  // live sockets
  // -------------------------------------------------------------------------

  /**
   * Attach one live socket and catch it up from `cursor` out of the window
   * (or MustRefetch when the cursor is behind the retained window) — the
   * DO's `attachSocket` accept/catch-up logic verbatim.
   */
  attachSocket(ws: HubSocketLike, cursor: number): void {
    this.sockets.set(ws, { cursor })

    const meta = this.meta()
    if (meta.last_version === 0) {
      // Fresh/empty hub: nothing to catch up from. Keep the socket; the
      // first append decides (its window_start covers or MustRefetches).
      return
    }
    if (cursor >= meta.last_version) {
      // At the edge (or ahead of a rebuilding hub): live tail from here.
      return
    }
    if (cursor < meta.window_start_version - 1) {
      this.mustRefetch(ws, "cursor_behind_retained_window")
      return
    }
    this.sendDeltaFrames(ws, this.windowEntriesAfter(cursor))
    this.sockets.get(ws)!.cursor = meta.last_version
  }

  /** Detach on close/error (Bun's close handler calls this). */
  detachSocket(ws: HubSocketLike): void {
    this.sockets.delete(ws)
  }

  /** TEST-ONLY: current cursor of an attached socket. */
  socketCursor(ws: HubSocketLike): number | undefined {
    return this.sockets.get(ws)?.cursor
  }

  /**
   * TEST-ONLY: rewind an attached socket's cursor (the DO tests did this
   * through `serializeAttachment`) so fan-out-time catch-up and
   * evicted-past-cursor paths are drivable deterministically.
   */
  setSocketCursor(ws: HubSocketLike, cursor: number): void {
    const state = this.sockets.get(ws)
    if (state !== undefined) state.cursor = cursor
  }

  /**
   * Server→client delta channel: the only client-initiated frame is
   * PingFrame (mutations go through HTTP push, SPEC §3). Anything else is
   * ignored — no state transitions originate from inbound frames.
   */
  onSocketMessage(ws: HubSocketLike, message: string | Buffer): void {
    if (typeof message !== "string") return
    try {
      const frame = decodeLiveFrame(JSON.parse(message) as unknown)
      if (frame._tag === "PingFrame") {
        ws.send(LIVE_HUB_PING_TEXT)
      }
    } catch {
      // Undecodable inbound data on a server→client channel: ignore.
    }
  }

  /** Keepalive tick: ping every attached socket (Cloud Run idle guard). */
  pingAll(): void {
    for (const ws of [...this.sockets.keys()]) {
      try {
        ws.send(LIVE_HUB_PING_TEXT)
      } catch {
        this.sockets.delete(ws)
        try {
          ws.close(1011, "khala_sync_hub_send_failed")
        } catch {
          // already gone
        }
      }
    }
  }

  // -------------------------------------------------------------------------
  // access-changed (KS-7.1 — SPEC §7 invariant 7 push half)
  // -------------------------------------------------------------------------

  /**
   * Scope access changed: broadcast `MustRefetch(access_changed)` to EVERY
   * attached socket and close them all. Returns the notified count (the
   * DO's response contract).
   */
  accessChanged(): number {
    let notified = 0
    for (const ws of [...this.sockets.keys()]) {
      this.mustRefetch(ws, "access_changed")
      notified += 1
    }
    return notified
  }

  // -------------------------------------------------------------------------
  // log
  // -------------------------------------------------------------------------

  /**
   * `GET /log?scope=&cursor=&limit=` served from the window — status map
   * and page trimming identical to the DO (whole version groups only; a
   * single version group larger than the limit is served whole).
   */
  log(params: URLSearchParams): Response {
    const scopeRaw = params.get("scope")
    let scope: SyncScope
    try {
      scope = decodeScope(scopeRaw)
    } catch {
      return json(
        { error: "khala_sync_hub_log_invalid", reason: "invalid scope" },
        { status: 400 },
      )
    }
    const cursor = parseNonNegativeInt(params.get("cursor") ?? "0")
    if (cursor === undefined) {
      return json(
        { error: "khala_sync_hub_log_invalid", reason: "invalid cursor" },
        { status: 400 },
      )
    }
    const limitRaw = params.get("limit")
    const limit = Math.min(
      limitRaw === null
        ? LIVE_HUB_LOG_DEFAULT_LIMIT
        : (parseNonNegativeInt(limitRaw) ?? 0),
      LIVE_HUB_LOG_MAX_LIMIT,
    )
    if (limit < 1) {
      return json(
        { error: "khala_sync_hub_log_invalid", reason: "invalid limit" },
        { status: 400 },
      )
    }
    if (scope !== this.scope) {
      return json({ error: "khala_sync_hub_scope_mismatch" }, { status: 409 })
    }

    const meta = this.meta()
    if (meta.last_version === 0) {
      return behindWindowResponse()
    }
    if (cursor < meta.window_start_version - 1) {
      return behindWindowResponse()
    }
    if (cursor > meta.last_version) {
      return aheadOfWindowResponse()
    }

    const rows = this.db
      .query<EntryRow, [number, number]>(
        `SELECT version, payload FROM entries
          WHERE version > ?
          ORDER BY version ASC, entity_type ASC, entity_id ASC
          LIMIT ?`,
      )
      .all(cursor, limit + 1)

    let page: Array<EntryRow>
    if (rows.length <= limit) {
      page = rows
    } else {
      const overflowVersion = rows[limit]!.version
      const trimmed = rows
        .slice(0, limit)
        .filter((row) => row.version !== overflowVersion)
      page =
        trimmed.length > 0
          ? trimmed
          : this.db
              .query<EntryRow, [number]>(
                `SELECT version, payload FROM entries
                  WHERE version = ?
                  ORDER BY entity_type ASC, entity_id ASC`,
              )
              .all(overflowVersion)
    }

    const entries = page.map((row) =>
      decodeChangelogEntry(JSON.parse(row.payload) as unknown),
    )
    const nextCursor =
      page.length > 0 ? page[page.length - 1]!.version : cursor

    return json(
      encodeLogPage(
        new LogPage({
          entries,
          nextCursor: SyncVersionWatermark.make(nextCursor),
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          upToDate: nextCursor >= meta.last_version,
        }),
      ),
    )
  }

  // -------------------------------------------------------------------------
  // window primitives
  // -------------------------------------------------------------------------

  private meta(): MetaRow {
    const row = this.db
      .query<MetaRow, []>(
        "SELECT window_start_version, last_version FROM meta WHERE id = 1",
      )
      .get()
    if (row === null) {
      throw new Error("khala-live-hub: meta row missing (constructor bug)")
    }
    return row
  }

  private windowEntriesAfter(cursor: number): Array<ChangelogEntry> {
    return this.db
      .query<EntryRow, [number]>(
        `SELECT version, payload FROM entries
          WHERE version > ?
          ORDER BY version ASC, entity_type ASC, entity_id ASC`,
      )
      .all(cursor)
      .map((row) => decodeChangelogEntry(JSON.parse(row.payload) as unknown))
  }
}

export { methodNotAllowed as liveHubMethodNotAllowed, json as liveHubJson }
