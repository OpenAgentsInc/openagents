import {
  type ChangelogEntry,
  encodeChangelogEntry,
  type SyncScope,
  type SyncVersion,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import postgres from "postgres"
import {
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncStorageError,
} from "./errors.js"
import { logPage, MAX_LOG_PAGE_LIMIT } from "./read-service.js"

/**
 * Khala Sync capture worker (KS-4.1, #8294; SPEC §4 "Capture").
 *
 * A long-lived supervised Bun process with a DIRECT Postgres connection —
 * never Hyperdrive, whose transaction-mode pooling drops LISTEN/NOTIFY —
 * that tails `khala_sync_changelog` and pushes ordered batches of
 * ChangelogEntry rows to each scope's `KhalaSyncHubDO` through the deployed
 * Worker's internal append route
 * (`POST /api/internal/khala-sync/hub/append?scope=…`, admin bearer).
 *
 * ## Delivery contract
 *
 * - **At-least-once, hub dedupes by version.** The per-scope checkpoint
 *   (`khala_sync_capture_checkpoints.pushed_through_version`) advances ONLY
 *   after the hub acknowledged a batch with 2xx (including the idempotent
 *   replay acknowledgment `appended: 0` — replayed versions at or below the
 *   hub edge are dropped by the hub, so re-pushing after a crash between
 *   2xx and the checkpoint write is safe).
 * - **Whole version groups only.** Batches are read with the KS-2.2
 *   `logPage` query, whose LIMIT bounds distinct versions and never splits
 *   one version's rows across pages — so a hub append can never deliver
 *   half of a transaction's entries.
 * - **LISTEN is a wake signal only, never the data channel.** The
 *   `khala_sync_changelog_append` NOTIFY (fired by the 0001 trigger) wakes
 *   the loop; the loop always reads authoritative rows from Postgres. A
 *   short poll fallback covers dropped notifications and listener downtime.
 *
 * ## Failure posture
 *
 * - **Hub 5xx / network failure** → bounded in-pass retry with backoff;
 *   the checkpoint does not move; the daemon retries again on the next
 *   wake/poll. A single scope's failure never crashes the daemon or blocks
 *   other scopes (per-scope isolation in {@link runCapturePass}).
 * - **Hub 409 version gap** (`khala_sync_hub_version_gap`) → the hub's gap
 *   check protects ITS window edge; the 409 body carries
 *   `expectedFirstVersion`, so capture re-reads from
 *   `expectedFirstVersion - 1` and re-pushes forward (the durable
 *   checkpoint is monotonic — it never moves backwards; GREATEST on
 *   write). If the hub's expectation is already behind the Postgres
 *   retained window the re-read fails typed
 *   ({@link KhalaSyncCursorBehindRetainedWindowError}); capture logs the
 *   scope error and retries from the checkpoint on the next pass — the hub
 *   heals through client re-bootstrap, not through capture inventing a
 *   partial log.
 * - **Postgres connection loss** → the pass fails with a typed storage
 *   error; the daemon backs off (bounded exponential) and reconnects; the
 *   dedicated postgres.js LISTEN connection re-subscribes automatically on
 *   reconnect, with the poll fallback covering the window in between.
 */

// ---------------------------------------------------------------------------
// Shared constants / contracts
// ---------------------------------------------------------------------------

/** NOTIFY channel fired by the 0001 changelog trigger (wake signal only). */
export const KHALA_SYNC_NOTIFY_CHANNEL = "khala_sync_changelog_append"

export interface KhalaSyncCaptureCheckpoint {
  readonly scope: SyncScope
  readonly pushedThroughVersion: SyncVersion
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export const DEFAULT_CAPTURE_POLL_INTERVAL_MS = 5_000
/** Distinct versions per hub append batch (clamped by MAX_LOG_PAGE_LIMIT). */
export const DEFAULT_CAPTURE_BATCH_VERSIONS = 200
export const DEFAULT_CAPTURE_MAX_PUSH_ATTEMPTS = 3
export const DEFAULT_CAPTURE_PUSH_RETRY_BACKOFF_MS = 250

/**
 * A Cloud SQL Auth Connector (unix socket) SESSION connection — the
 * production DB path after CFG-14 closed the instance's public ingress
 * (#8554). Both the Bun `SQL` pass/checkpoint connection AND the dedicated
 * postgres.js LISTEN connection dial the connector socket file directly
 * (a session connection, never a transaction pool — LISTEN/NOTIFY needs the
 * persistent session). Cloud Run mounts the socket DIRECTORY under
 * `/cloudsql/<instance>` via `--add-cloudsql-instances`.
 *
 * `socketPath` may be either the connector directory
 * (`/cloudsql/openagentsgemini:us-central1:khala-sync-pg`) or the full
 * socket file (`…/.s.PGSQL.5432`); {@link socketConnectionFromEnv} normalizes
 * a bare directory to the `.s.PGSQL.<port>` file.
 */
export interface CaptureSocketConnection {
  /** Connector socket file (or directory — normalized on build). */
  readonly socketPath: string
  readonly username: string
  readonly password: string
  readonly database: string
}

export interface CaptureConfig {
  /**
   * DIRECT Postgres connection URL (never a Hyperdrive string). Provide this
   * OR {@link socket}. When {@link socket} is set it takes precedence and the
   * connection dials the Cloud SQL Auth Connector unix socket instead.
   */
  readonly databaseUrl?: string | undefined
  /**
   * Cloud SQL Auth Connector session connection (CFG-14: public DB ingress
   * closed). When set, both the pass and LISTEN connections dial the
   * connector unix socket instead of {@link databaseUrl}. Bun's `SQL`
   * ignores `PGHOST` for a URL, so the socket path is passed explicitly.
   */
  readonly socket?: CaptureSocketConnection | undefined
  /**
   * Full URL of the Worker's internal hub append route, e.g.
   * `https://openagents.com/api/internal/khala-sync/hub/append`.
   * Capture adds the `scope` query parameter per push.
   */
  readonly hubAppendUrl: string
  /**
   * Admin bearer for the Worker's internal route
   * (OPENAGENTS_ADMIN_API_TOKEN). Optional when {@link hubToken} is provided
   * — a LiveHub-only capture deploy (CFG-5/#8554) pushes with the shared
   * LiveHub bearer and has no Worker admin token.
   */
  readonly adminToken?: string | undefined
  /**
   * Bearer for `hubAppendUrl` when it is NOT the Worker's admin-guarded
   * internal route — e.g. the LiveHub Cloud Run service's shared service
   * token (CFG-5, #8520). Defaults to `adminToken`.
   */
  readonly hubToken?: string | undefined
  /**
   * OPTIONAL fail-soft mirror hub (CFG-5 cutover aid): after every batch
   * the PRIMARY hub acknowledged, the same batch is pushed best-effort to
   * this second append URL. Mirror failures are logged and NEVER gate the
   * checkpoint or fail the pass — the mirror (LiveHub) also rebuilds its
   * window from Postgres on demand, so a missed mirror append only delays
   * its live fan-out, never its correctness.
   */
  readonly mirrorAppendUrl?: string | undefined
  readonly mirrorToken?: string | undefined
  readonly pollIntervalMs?: number | undefined
  readonly batchVersions?: number | undefined
  readonly maxPushAttempts?: number | undefined
  readonly pushRetryBackoffMs?: number | undefined
  readonly log?: ((line: string) => void) | undefined
}

interface ResolvedCaptureConfig {
  readonly databaseUrl: string | undefined
  readonly socket: CaptureSocketConnection | undefined
  readonly hubAppendUrl: string
  readonly adminToken: string
  readonly hubToken: string
  readonly mirrorAppendUrl: string | undefined
  readonly mirrorToken: string
  readonly pollIntervalMs: number
  readonly batchVersions: number
  readonly maxPushAttempts: number
  readonly pushRetryBackoffMs: number
  readonly log: (line: string) => void
}

const positiveOrDefault = (value: number | undefined, dflt: number): number =>
  value !== undefined && Number.isSafeInteger(value) && value >= 1 ? value : dflt

const resolveConfig = (config: CaptureConfig): ResolvedCaptureConfig => {
  const socket = config.socket
  const hasUrl = config.databaseUrl !== undefined && config.databaseUrl !== ""
  if (socket === undefined && !hasUrl) {
    throw new Error("capture: databaseUrl or socket connection must be provided")
  }
  if (socket !== undefined) {
    if (socket.socketPath === "") {
      throw new Error("capture: socket.socketPath must not be empty")
    }
    if (socket.username === "") {
      throw new Error("capture: socket.username must not be empty")
    }
    if (socket.database === "") {
      throw new Error("capture: socket.database must not be empty")
    }
  }
  if (config.hubAppendUrl === "") {
    throw new Error("capture: hubAppendUrl must not be empty")
  }
  const adminToken = config.adminToken ?? ""
  const hubTokenRaw = config.hubToken ?? ""
  if (adminToken === "" && hubTokenRaw === "") {
    throw new Error("capture: adminToken or hubToken must be provided")
  }
  // Either bearer covers the other: the admin token defaults the hub bearer
  // (Worker internal route), and the hub bearer defaults the admin slot
  // (LiveHub-only deploy with no Worker admin token).
  const effectiveAdmin = adminToken !== "" ? adminToken : hubTokenRaw
  return {
    databaseUrl: hasUrl ? config.databaseUrl : undefined,
    socket,
    hubAppendUrl: config.hubAppendUrl,
    adminToken: effectiveAdmin,
    hubToken: hubTokenRaw !== "" ? hubTokenRaw : effectiveAdmin,
    mirrorAppendUrl:
      config.mirrorAppendUrl !== undefined && config.mirrorAppendUrl !== ""
        ? config.mirrorAppendUrl
        : undefined,
    mirrorToken:
      config.mirrorToken !== undefined && config.mirrorToken !== ""
        ? config.mirrorToken
        : effectiveAdmin,
    pollIntervalMs: positiveOrDefault(
      config.pollIntervalMs,
      DEFAULT_CAPTURE_POLL_INTERVAL_MS,
    ),
    batchVersions: Math.min(
      positiveOrDefault(config.batchVersions, DEFAULT_CAPTURE_BATCH_VERSIONS),
      MAX_LOG_PAGE_LIMIT,
    ),
    maxPushAttempts: positiveOrDefault(
      config.maxPushAttempts,
      DEFAULT_CAPTURE_MAX_PUSH_ATTEMPTS,
    ),
    pushRetryBackoffMs: Math.max(0, config.pushRetryBackoffMs ?? DEFAULT_CAPTURE_PUSH_RETRY_BACKOFF_MS),
    log: config.log ?? (() => {}),
  }
}

const envInt = (raw: string | undefined): number | undefined => {
  if (raw === undefined || raw.trim() === "") return undefined
  const value = Number.parseInt(raw.trim(), 10)
  return Number.isSafeInteger(value) && value >= 1 ? value : undefined
}

/**
 * Resolve a Cloud SQL Auth Connector socket connection from the standard
 * libpq env vars, selected ONLY when `PGHOST` is an absolute path (the
 * connector unix-socket directory, e.g.
 * `/cloudsql/openagentsgemini:us-central1:khala-sync-pg`). A TCP `PGHOST`
 * returns `undefined` so URL mode / libpq handles the network path. Requires
 * `PGUSER` and `PGDATABASE`; `PGPASSWORD` may be empty. Normalizes a bare
 * directory to the `.s.PGSQL.<PGPORT|5432>` socket file.
 */
export const socketConnectionFromEnv = (
  env: Record<string, string | undefined> = process.env,
): CaptureSocketConnection | undefined => {
  const host = env["PGHOST"]
  if (host === undefined || host === "" || !host.startsWith("/")) return undefined
  const username = env["PGUSER"]
  const database = env["PGDATABASE"]
  if (username === undefined || username === "") return undefined
  if (database === undefined || database === "") return undefined
  const port = env["PGPORT"] !== undefined && env["PGPORT"] !== "" ? env["PGPORT"] : "5432"
  const suffix = `/.s.PGSQL.${port}`
  const socketPath = host.endsWith(suffix) ? host : `${host}${suffix}`
  return {
    socketPath,
    username,
    password: env["PGPASSWORD"] ?? "",
    database,
  }
}

/**
 * Build a {@link CaptureConfig} from the environment:
 * `KHALA_SYNC_DATABASE_URL` (or a Cloud SQL Auth Connector socket via
 * `PGHOST`/`PGUSER`/`PGPASSWORD`/`PGDATABASE` when the URL is absent —
 * CFG-14/#8554), `KHALA_SYNC_HUB_APPEND_URL`,
 * `OPENAGENTS_ADMIN_API_TOKEN` (or `KHALA_SYNC_HUB_TOKEN` alone for a
 * LiveHub-only deploy), and optional `KHALA_SYNC_HUB_TOKEN`
 * (bearer for a non-Worker hub such as LiveHub; defaults to the admin
 * token), `KHALA_SYNC_HUB_MIRROR_APPEND_URL` / `KHALA_SYNC_HUB_MIRROR_TOKEN`
 * (fail-soft second hub, CFG-5 cutover aid), and
 * `KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS` / `KHALA_SYNC_CAPTURE_BATCH_VERSIONS`.
 * Throws with the missing variable NAMES only — never echoes values.
 */
export const captureConfigFromEnv = (
  env: Record<string, string | undefined> = process.env,
): CaptureConfig => {
  const missing: Array<string> = []
  const databaseUrl = env["KHALA_SYNC_DATABASE_URL"]
  const hubAppendUrl = env["KHALA_SYNC_HUB_APPEND_URL"]
  const adminToken = env["OPENAGENTS_ADMIN_API_TOKEN"]
  const hubTokenEnv = env["KHALA_SYNC_HUB_TOKEN"]
  const hasUrl = databaseUrl !== undefined && databaseUrl !== ""
  const socket = hasUrl ? undefined : socketConnectionFromEnv(env)
  if (!hasUrl && socket === undefined) {
    missing.push("KHALA_SYNC_DATABASE_URL")
  }
  if (hubAppendUrl === undefined || hubAppendUrl === "") {
    missing.push("KHALA_SYNC_HUB_APPEND_URL")
  }
  const hasAdmin = adminToken !== undefined && adminToken !== ""
  const hasHubToken = hubTokenEnv !== undefined && hubTokenEnv !== ""
  if (!hasAdmin && !hasHubToken) {
    missing.push("OPENAGENTS_ADMIN_API_TOKEN")
  }
  if (missing.length > 0) {
    throw new Error(`capture: missing environment variable(s): ${missing.join(", ")}`)
  }
  return {
    ...(socket === undefined ? { databaseUrl: databaseUrl! } : { socket }),
    hubAppendUrl: hubAppendUrl!,
    ...(hasAdmin ? { adminToken } : {}),
    hubToken: hubTokenEnv,
    mirrorAppendUrl: env["KHALA_SYNC_HUB_MIRROR_APPEND_URL"],
    mirrorToken: env["KHALA_SYNC_HUB_MIRROR_TOKEN"],
    pollIntervalMs: envInt(env["KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS"]),
    batchVersions: envInt(env["KHALA_SYNC_CAPTURE_BATCH_VERSIONS"]),
  }
}

// ---------------------------------------------------------------------------
// Checkpoints
// ---------------------------------------------------------------------------

interface CheckpointRow {
  readonly pushed_through_version: string | number | bigint
}

const toWatermark = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `capture checkpoint watermark out of safe range: ${String(raw)}`,
    )
  }
  return value
}

/** The scope's durable checkpoint (0 = nothing pushed yet / no row). */
export const readCheckpoint = async (sql: SQL, scope: SyncScope): Promise<number> => {
  const rows: Array<CheckpointRow> = await sql`
    SELECT pushed_through_version
      FROM khala_sync_capture_checkpoints
     WHERE scope = ${scope}
  `
  const row = rows[0]
  return row === undefined ? 0 : toWatermark(row.pushed_through_version)
}

/**
 * Advance the scope's checkpoint to `version`. Monotonic by construction
 * (GREATEST): a gap-heal re-push below the checkpoint can never move the
 * durable watermark backwards.
 */
export const advanceCheckpoint = async (
  sql: SQL,
  scope: SyncScope,
  version: number,
): Promise<void> => {
  await sql`
    INSERT INTO khala_sync_capture_checkpoints (scope, pushed_through_version)
    VALUES (${scope}, ${version})
    ON CONFLICT (scope) DO UPDATE SET
      pushed_through_version = GREATEST(
        khala_sync_capture_checkpoints.pushed_through_version,
        EXCLUDED.pushed_through_version
      ),
      updated_at = now()
  `
}

export interface PendingScope {
  readonly scope: SyncScope
  readonly pushedThroughVersion: number
  readonly lastVersion: number
}

/**
 * Startup/pass discovery: every scope with committed changelog activity
 * beyond its checkpoint, in ONE query (absent checkpoint rows behave as 0).
 */
export const pendingScopes = async (sql: SQL): Promise<Array<PendingScope>> => {
  const rows: Array<{
    readonly scope: string
    readonly last_version: string | number | bigint
    readonly pushed_through_version: string | number | bigint
  }> = await sql`
    SELECT s.scope,
           s.last_version,
           COALESCE(c.pushed_through_version, 0) AS pushed_through_version
      FROM khala_sync_scopes s
      LEFT JOIN khala_sync_capture_checkpoints c USING (scope)
     WHERE s.last_version > COALESCE(c.pushed_through_version, 0)
     ORDER BY s.scope
  `
  return rows.map((row) => ({
    scope: row.scope as SyncScope,
    pushedThroughVersion: toWatermark(row.pushed_through_version),
    lastVersion: toWatermark(row.last_version),
  }))
}

// ---------------------------------------------------------------------------
// Hub append client
// ---------------------------------------------------------------------------

export type HubAppendOutcome =
  | {
      readonly kind: "ok"
      readonly appended: number
      readonly duplicates: number
      readonly lastVersion: number
    }
  | { readonly kind: "gap"; readonly expectedFirstVersion: number }
  | {
      readonly kind: "failed"
      readonly status: number
      readonly detail: string
    }

/**
 * POST one ordered batch to the hub append route. Network failures come
 * back as `{ kind: "failed", status: 0 }` — never thrown — so the retry
 * policy in {@link captureScopePass} treats them like 5xx.
 */
export const pushBatchToHub = async (
  target: Readonly<{ appendUrl: string; token: string }>,
  scope: SyncScope,
  entries: ReadonlyArray<ChangelogEntry>,
): Promise<HubAppendOutcome> => {
  const url = new URL(target.appendUrl)
  url.searchParams.set("scope", scope)
  let response: Response
  try {
    response = await fetch(url, {
      method: "POST",
      headers: {
        authorization: `Bearer ${target.token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        scope,
        entries: entries.map((entry) => encodeChangelogEntry(entry)),
      }),
    })
  } catch (error) {
    return {
      kind: "failed",
      status: 0,
      detail: `network: ${describeErrorSafe(error)}`,
    }
  }

  const raw = (await response.json().catch(() => undefined)) as unknown
  // A body of literal JSON `null` (seen from upstream 5xx paths) must be
  // handled like an unparseable body — never dereferenced.
  const body =
    raw !== null && typeof raw === "object"
      ? (raw as Record<string, unknown>)
      : undefined

  if (response.ok && body !== undefined && body["ok"] === true) {
    const appended = typeof body["appended"] === "number" ? body["appended"] : 0
    const duplicates =
      typeof body["duplicates"] === "number" ? body["duplicates"] : 0
    const lastVersion =
      typeof body["lastVersion"] === "number" ? body["lastVersion"] : 0
    return { kind: "ok", appended, duplicates, lastVersion }
  }

  if (
    response.status === 409 &&
    body !== undefined &&
    body["error"] === "khala_sync_hub_version_gap" &&
    typeof body["expectedFirstVersion"] === "number" &&
    Number.isSafeInteger(body["expectedFirstVersion"]) &&
    body["expectedFirstVersion"] >= 1
  ) {
    return { kind: "gap", expectedFirstVersion: body["expectedFirstVersion"] }
  }

  const errorTag =
    body !== undefined && typeof body["error"] === "string"
      ? body["error"]
      : "unparseable"
  return {
    kind: "failed",
    status: response.status,
    detail: `http ${response.status} (${errorTag})`,
  }
}

const sleep = (ms: number): Promise<void> =>
  ms <= 0 ? Promise.resolve() : new Promise((resolve) => setTimeout(resolve, ms))

/** Public-safe one-line error description (never tokens or row values). */
export const describeErrorSafe = (error: unknown): string =>
  (error instanceof Error ? `${error.name}: ${error.message}` : String(error)).slice(
    0,
    300,
  )

// ---------------------------------------------------------------------------
// Per-scope pass
// ---------------------------------------------------------------------------

export interface CaptureScopeResult {
  readonly scope: SyncScope
  /** Watermark the pass started reading after (durable checkpoint). */
  readonly startedAfterVersion: number
  /** Durable checkpoint after the pass. */
  readonly pushedThroughVersion: number
  readonly entriesPushed: number
  readonly batchesPushed: number
  /** True when the scope's log was fully drained at pass end. */
  readonly upToDate: boolean
  /** Public-safe failure summary; the checkpoint reflects only 2xx pushes. */
  readonly error?: string
}

/**
 * Drain one scope: read whole-version-group batches after the checkpoint
 * (KS-2.2 `logPage` — REPEATABLE READ, never splits a version), push each
 * to the hub, advance the checkpoint on 2xx only, until `upToDate`.
 *
 * Gap healing: one 409-gap per pass triggers a re-read from the hub's own
 * `expectedFirstVersion - 1`; a second gap (or a gap expectation behind the
 * retained window) fails the pass honestly with the checkpoint unmoved.
 */
export const captureScopePass = async (
  sql: SQL,
  config: ResolvedCaptureConfig,
  scope: SyncScope,
): Promise<CaptureScopeResult> => {
  const startedAfterVersion = await readCheckpoint(sql, scope)
  let after = startedAfterVersion
  let checkpoint = startedAfterVersion
  let entriesPushed = 0
  let batchesPushed = 0
  let gapHealed = false

  const finish = (
    upToDate: boolean,
    error?: string,
  ): CaptureScopeResult => ({
    scope,
    startedAfterVersion,
    pushedThroughVersion: checkpoint,
    entriesPushed,
    batchesPushed,
    upToDate,
    ...(error === undefined ? {} : { error }),
  })

  for (;;) {
    let page
    try {
      page = await logPage(sql, {
        scope,
        afterVersion: after,
        limit: config.batchVersions,
      })
    } catch (error) {
      if (error instanceof KhalaSyncCursorBehindRetainedWindowError) {
        // Either compaction ran past our checkpoint, or a hub gap
        // expectation is behind the retained window. Capture must never
        // fabricate a partial log (SPEC invariant 6): log, leave the
        // checkpoint, and let the hub/clients heal via re-bootstrap.
        return finish(
          false,
          `retained window passed cursor ${error.afterVersion} ` +
            `(retained_from_version ${error.retainedFromVersion})`,
        )
      }
      throw error
    }

    if (page.entries.length === 0) {
      return finish(page.upToDate)
    }

    let outcome: HubAppendOutcome | undefined
    for (let attempt = 1; attempt <= config.maxPushAttempts; attempt++) {
      outcome = await pushBatchToHub(
        { appendUrl: config.hubAppendUrl, token: config.hubToken },
        scope,
        page.entries,
      )
      if (outcome.kind !== "failed") break
      config.log(
        `capture ${scope}: push attempt ${attempt}/${config.maxPushAttempts} ` +
          `failed (${outcome.detail})`,
      )
      if (attempt < config.maxPushAttempts) {
        await sleep(config.pushRetryBackoffMs * 2 ** (attempt - 1))
      }
    }

    if (outcome === undefined || outcome.kind === "failed") {
      return finish(
        false,
        outcome === undefined ? "no push attempted" : outcome.detail,
      )
    }

    if (outcome.kind === "gap") {
      if (gapHealed) {
        return finish(
          false,
          `hub reported a second version gap in one pass ` +
            `(expectedFirstVersion ${outcome.expectedFirstVersion})`,
        )
      }
      gapHealed = true
      // Re-push from the hub's own edge; the durable checkpoint stays put
      // (monotonic) — the hub dedupes anything it already holds.
      after = Math.max(0, outcome.expectedFirstVersion - 1)
      config.log(
        `capture ${scope}: hub version gap — re-reading after version ${after}`,
      )
      continue
    }

    // 2xx (including idempotent replay, appended: 0): the hub holds
    // everything through the page's nextCursor — advance the checkpoint.
    const pushedThrough = Number(page.nextCursor)
    await advanceCheckpoint(sql, scope, pushedThrough)
    checkpoint = Math.max(checkpoint, pushedThrough)
    after = pushedThrough
    entriesPushed += page.entries.length
    batchesPushed += 1

    // Fail-soft mirror (CFG-5): one best-effort push of the SAME batch to
    // the second hub. Never retried, never gates the checkpoint; a mirror
    // gap/failure is logged and heals through the mirror's own
    // Postgres-window rebuild.
    if (config.mirrorAppendUrl !== undefined) {
      const mirror = await pushBatchToHub(
        { appendUrl: config.mirrorAppendUrl, token: config.mirrorToken },
        scope,
        page.entries,
      ).catch(
        (error): HubAppendOutcome => ({
          kind: "failed",
          status: 0,
          detail: `mirror threw: ${describeErrorSafe(error)}`,
        }),
      )
      if (mirror.kind !== "ok") {
        config.log(
          `capture ${scope}: mirror push ${
            mirror.kind === "gap"
              ? `gap (expectedFirstVersion ${mirror.expectedFirstVersion})`
              : `failed (${mirror.detail})`
          } — primary checkpoint unaffected`,
        )
      }
    }

    if (page.upToDate) {
      return finish(true)
    }
  }
}

// ---------------------------------------------------------------------------
// One full pass (the --once unit)
// ---------------------------------------------------------------------------

export interface CapturePassResult {
  readonly scopes: ReadonlyArray<CaptureScopeResult>
  readonly failedScopes: number
}

/**
 * One capture pass: discover every scope with activity beyond its
 * checkpoint (single query), then drain each with per-scope error
 * isolation — one scope's storage or hub failure is reported in its
 * result and never aborts the others or throws out of the pass. Only the
 * discovery query itself can throw (connection-level failures, handled by
 * the daemon's backoff loop).
 */
export const runCapturePass = async (
  sql: SQL,
  configInput: CaptureConfig,
): Promise<CapturePassResult> => {
  const config = resolveConfig(configInput)
  const pending = await pendingScopes(sql)
  const results: Array<CaptureScopeResult> = []
  for (const item of pending) {
    try {
      results.push(await captureScopePass(sql, config, item.scope))
    } catch (error) {
      config.log(
        `capture ${item.scope}: pass failed (${describeErrorSafe(error)})`,
      )
      results.push({
        scope: item.scope,
        startedAfterVersion: item.pushedThroughVersion,
        pushedThroughVersion: item.pushedThroughVersion,
        entriesPushed: 0,
        batchesPushed: 0,
        upToDate: false,
        error: describeErrorSafe(error),
      })
    }
  }
  return {
    scopes: results,
    failedScopes: results.filter((r) => r.error !== undefined).length,
  }
}

// ---------------------------------------------------------------------------
// Connection factories (socket via the Cloud SQL Auth Connector, or URL)
// ---------------------------------------------------------------------------

/**
 * The Bun `SQL` client for pass/checkpoint work. Bun's `SQL` does NOT resolve
 * `PGHOST` from a URL to a unix socket, so a connector deploy passes the
 * socket file path explicitly; a direct-URL deploy keeps the URL form.
 */
const makeCaptureSql = (config: ResolvedCaptureConfig, max: number): SQL =>
  config.socket !== undefined
    ? new SQL({
        adapter: "postgres",
        path: config.socket.socketPath,
        username: config.socket.username,
        password: config.socket.password,
        database: config.socket.database,
        max,
      })
    : new SQL({ url: config.databaseUrl!, max })

/**
 * The dedicated postgres.js LISTEN connection (session mode — never a
 * transaction pool, which drops LISTEN/NOTIFY). postgres.js derives the
 * connector socket file from a `/`-prefixed host, so both the explicit
 * `path` (connector) and the URL form work.
 */
const makeCaptureListener = (
  config: ResolvedCaptureConfig,
): ReturnType<typeof postgres> =>
  config.socket !== undefined
    ? postgres({
        path: config.socket.socketPath,
        username: config.socket.username,
        password: config.socket.password,
        database: config.socket.database,
        max: 1,
        onnotice: () => {},
      })
    : postgres(config.databaseUrl!, { max: 1, onnotice: () => {} })

/** Single-pass convenience for the CLI `--once` mode (and cron). */
export const runCaptureOnce = async (
  configInput: CaptureConfig,
): Promise<CapturePassResult> => {
  const config = resolveConfig(configInput)
  const sql = makeCaptureSql(config, 2)
  try {
    return await runCapturePass(sql, configInput)
  } finally {
    await sql.end()
  }
}

// ---------------------------------------------------------------------------
// Daemon (LISTEN wake + poll fallback)
// ---------------------------------------------------------------------------

export interface CaptureDaemon {
  /** Manually wake the loop (the NOTIFY handler calls this). */
  readonly wake: () => void
  /**
   * Resolves once `LISTEN khala_sync_changelog_append` is established
   * (also resolves on stop, so awaiting it can never hang a shutdown).
   * Until then the poll fallback is the only wake source.
   */
  readonly listenerReady: Promise<void>
  /** Stop the loop, close connections, and wait for full shutdown. */
  readonly stop: () => Promise<void>
  /** Resolves when the daemon loop has fully exited. */
  readonly done: Promise<void>
}

const LISTENER_RETRY_MAX_MS = 30_000
const PASS_FAILURE_BACKOFF_MAX_MS = 30_000

/**
 * Start the capture daemon: an immediate resume-from-checkpoints pass,
 * then a loop woken by `NOTIFY khala_sync_changelog_append` (dedicated
 * postgres.js connection — it re-subscribes automatically on reconnect)
 * with the configured poll interval as fallback. A pass-level failure
 * (e.g. Postgres connection loss) backs off exponentially (bounded) and
 * never crashes the daemon; per-scope failures are isolated inside the
 * pass.
 */
export const startCaptureDaemon = (configInput: CaptureConfig): CaptureDaemon => {
  const config = resolveConfig(configInput)
  const log = config.log

  let stopped = false
  let wakePending = false
  let wakeResolve: (() => void) | undefined
  let signalStop: () => void = () => {}
  const stopSignal = new Promise<void>((resolve) => {
    signalStop = resolve
  })

  const wake = (): void => {
    const resolve = wakeResolve
    if (resolve !== undefined) {
      wakeResolve = undefined
      resolve()
    } else {
      wakePending = true
    }
  }

  const waitForWakeOrTimeout = (ms: number): Promise<void> => {
    if (wakePending) {
      wakePending = false
      return Promise.resolve()
    }
    return new Promise<void>((resolve) => {
      const finish = (): void => {
        clearTimeout(timer)
        if (wakeResolve === finish) wakeResolve = undefined
        resolve()
      }
      const timer = setTimeout(finish, ms)
      wakeResolve = finish
    })
  }

  // Dedicated LISTEN connection: postgres.js keeps it subscribed across
  // reconnects once established; this loop covers initial-connect failures.
  let listenSql: ReturnType<typeof postgres> | undefined
  let listenerReadyResolve: () => void = () => {}
  const listenerReady = new Promise<void>((resolve) => {
    listenerReadyResolve = resolve
  })
  const listenerTask = (async () => {
    let backoffMs = 1_000
    while (!stopped) {
      try {
        const listener = makeCaptureListener(config)
        listenSql = listener
        await listener.listen(KHALA_SYNC_NOTIFY_CHANNEL, () => wake())
        log(`capture: LISTEN ${KHALA_SYNC_NOTIFY_CHANNEL} established`)
        listenerReadyResolve()
        return
      } catch (error) {
        await listenSql?.end({ timeout: 1 }).catch(() => {})
        listenSql = undefined
        log(
          `capture: listener connect failed (${describeErrorSafe(error)}); ` +
            `retrying in ${backoffMs}ms — poll fallback active`,
        )
        await Promise.race([sleep(backoffMs), stopSignal])
        backoffMs = Math.min(backoffMs * 2, LISTENER_RETRY_MAX_MS)
      }
    }
    listenerReadyResolve()
  })()

  const sql = makeCaptureSql(config, 3)

  const done = (async () => {
    let failureBackoffMs = 0
    while (!stopped) {
      try {
        const result = await runCapturePass(sql, config)
        failureBackoffMs = 0
        if (result.failedScopes > 0) {
          log(
            `capture: pass completed with ${result.failedScopes} failed ` +
              `scope(s) of ${result.scopes.length}; retrying on next wake/poll`,
          )
        }
      } catch (error) {
        failureBackoffMs =
          failureBackoffMs === 0
            ? 1_000
            : Math.min(failureBackoffMs * 2, PASS_FAILURE_BACKOFF_MAX_MS)
        log(
          `capture: pass failed (${describeErrorSafe(error)}); ` +
            `backing off ${failureBackoffMs}ms`,
        )
      }
      if (stopped) break
      await waitForWakeOrTimeout(
        failureBackoffMs > 0 ? failureBackoffMs : config.pollIntervalMs,
      )
    }
    await sql.end().catch(() => {})
    await listenerTask.catch(() => {})
    await listenSql?.end({ timeout: 5 }).catch(() => {})
  })()

  const stop = async (): Promise<void> => {
    stopped = true
    signalStop()
    wake()
    listenerReadyResolve()
    // Unblock the listener retry loop promptly by ending its connection.
    await listenSql?.end({ timeout: 1 }).catch(() => {})
    await done
  }

  return { wake, listenerReady, stop, done }
}
