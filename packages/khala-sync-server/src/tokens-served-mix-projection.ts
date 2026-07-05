import {
  decodeTokensServedChannelMixSnapshotEntity,
  decodeTokensServedDemandMixSnapshotEntity,
  decodeTokensServedHistorySnapshotEntity,
  decodeTokensServedModelMixSnapshotEntity,
  encodeTokensServedChannelMixSnapshotEntity,
  encodeTokensServedDemandMixSnapshotEntity,
  encodeTokensServedHistorySnapshotEntity,
  encodeTokensServedModelMixSnapshotEntity,
  EntityId,
  EntityType,
  publicScope,
  TOKENS_SERVED_AGGREGATES_CHANNEL_ID,
  TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE,
  TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE,
  TOKENS_SERVED_HISTORY_ENTITY_TYPE,
  TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE,
  tokensServedHistorySnapshotEntityId,
  tokensServedMixSnapshotEntityId,
  type TokensServedChannelMixSnapshotEntity,
  type TokensServedDemandMixSnapshotEntity,
  type TokensServedHistorySnapshotEntity,
  type TokensServedModelMixSnapshotEntity,
  type TokensServedWindow,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Public tokens-served aggregate snapshot projection (KS-6.7, #8417;
 * SPEC §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * Stores a whole-breakdown JSON post-image of each already-shaped public
 * tokens-served mix/history read (see ./tokens-served-mix entity module doc
 * for why: the ledger's own `readPublicTokensServedModelMix` /
 * `readPublicTokensServedDemandMix` / `readPublicTokensServedChannelMix` /
 * `readPublicTokensServedHistory` already return the exact final shape —
 * this projection is a STORED SNAPSHOT of that output, not a re-derivation).
 *
 * UNLIKE the tokens-served public counter (./public-counter-projection.ts),
 * this projection never invents or accumulates a value itself — same
 * discipline as the settled-feed / gym-run-progress projections: each
 * snapshot is an UPSERT keyed by its own stable entityId (window, or
 * window+timezone for history), so a repeated refresh with the SAME
 * computed values is naturally idempotent, and a replayed/duplicate refresh
 * simply appends another (structurally identical) changelog version.
 *
 * WRITE: `projectTokensServedAggregateSnapshotBestEffort` upserts ONE
 * snapshot per call into `scope.public.tokens-served-aggregates` via the
 * KHALA_SYNC_DB Hyperdrive binding. FAIL-SOFT (same discipline as every
 * other KS-6.x/KS-8.x projection): a projection failure never fails or
 * slows the caller's real ledger read/ingest path.
 *
 * READ: `readTokensServedAggregateSnapshot` returns the latest post-image
 * for one (entityType, entityId) pair straight off `khala_sync_changelog`
 * (no bespoke storage table — the generic changelog IS the store here,
 * same as settled-feed/gym-run-progress).
 */

// ---------------------------------------------------------------------------
// Named system writer (SPEC §7 invariant 3) + scope
// ---------------------------------------------------------------------------

export const TOKENS_SERVED_MIX_PROJECTION_SYSTEM_REF =
  "system:tokens_served_mix_projection.token_usage_ledger.v1"

/** The tokens-served-aggregates scope: `scope.public.tokens-served-aggregates`. */
export const tokensServedAggregatesPublicScope = () =>
  publicScope(TOKENS_SERVED_AGGREGATES_CHANNEL_ID)

// ---------------------------------------------------------------------------
// Diagnostics + fail-soft outcome shape (shared across the 4 snapshot kinds)
// ---------------------------------------------------------------------------

export interface TokensServedMixProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason: "invalid_input" | "storage_failed" | "projection_failed"
  readonly messageSafe: string
}

export type TokensServedMixProjectionOutcome =
  | { readonly ok: true }
  | {
      readonly ok: false
      readonly diagnostic: TokensServedMixProjectionDiagnostic
    }

const diagnosticFromUnknown = (
  error: unknown,
): TokensServedMixProjectionDiagnostic => {
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, decode failures) can embed raw values or
  // connection strings — never echo them.
  return {
    messageSafe: "tokens-served aggregate snapshot projection failed",
    reason: "projection_failed",
  }
}

// ---------------------------------------------------------------------------
// Write (one upsert per snapshot kind)
// ---------------------------------------------------------------------------

const upsertSnapshot = async (
  sql: SyncSql,
  input: Readonly<{ entityType: string; entityId: string; postImage: unknown }>,
): Promise<void> => {
  await withSyncTransaction(sql, async writer => {
    await writer.appendChange({
      entityId: EntityId.make(input.entityId),
      entityType: EntityType.make(input.entityType),
      mutationRef: TOKENS_SERVED_MIX_PROJECTION_SYSTEM_REF,
      op: "upsert",
      postImage: input.postImage,
      scope: tokensServedAggregatesPublicScope(),
    })
  })
}

export const projectTokensServedModelMixSnapshot = async (
  sql: SyncSql,
  snapshot: TokensServedModelMixSnapshotEntity,
): Promise<void> => {
  const validated = decodeTokensServedModelMixSnapshotEntity(snapshot)
  await upsertSnapshot(sql, {
    entityId: tokensServedMixSnapshotEntityId(validated.window),
    entityType: TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE,
    postImage: encodeTokensServedModelMixSnapshotEntity(validated),
  })
}

export const projectTokensServedDemandMixSnapshot = async (
  sql: SyncSql,
  snapshot: TokensServedDemandMixSnapshotEntity,
): Promise<void> => {
  const validated = decodeTokensServedDemandMixSnapshotEntity(snapshot)
  await upsertSnapshot(sql, {
    entityId: tokensServedMixSnapshotEntityId(validated.window),
    entityType: TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE,
    postImage: encodeTokensServedDemandMixSnapshotEntity(validated),
  })
}

export const projectTokensServedChannelMixSnapshot = async (
  sql: SyncSql,
  snapshot: TokensServedChannelMixSnapshotEntity,
): Promise<void> => {
  const validated = decodeTokensServedChannelMixSnapshotEntity(snapshot)
  await upsertSnapshot(sql, {
    entityId: tokensServedMixSnapshotEntityId(validated.window),
    entityType: TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE,
    postImage: encodeTokensServedChannelMixSnapshotEntity(validated),
  })
}

export const projectTokensServedHistorySnapshot = async (
  sql: SyncSql,
  snapshot: TokensServedHistorySnapshotEntity,
): Promise<void> => {
  const validated = decodeTokensServedHistorySnapshotEntity(snapshot)
  await upsertSnapshot(sql, {
    entityId: tokensServedHistorySnapshotEntityId(
      validated.window,
      validated.timezone,
    ),
    entityType: TOKENS_SERVED_HISTORY_ENTITY_TYPE,
    postImage: encodeTokensServedHistorySnapshotEntity(validated),
  })
}

// ---------------------------------------------------------------------------
// Fail-soft producer wrappers (same discipline as fleet/public-counter/
// settled-feed)
// ---------------------------------------------------------------------------

/**
 * Shared fail-soft shape: decode `rawSnapshot` with `decode`, then `write`
 * it. A decode failure classifies as `invalid_input` (never touches
 * storage); anything else `write` throws classifies via
 * `diagnosticFromUnknown`. NEVER throws.
 */
const bestEffortProject = async <A>(
  decode: (raw: unknown) => A,
  invalidInputMessageSafe: string,
  write: (validated: A) => Promise<void>,
  rawSnapshot: unknown,
): Promise<TokensServedMixProjectionOutcome> => {
  let validated: A
  try {
    validated = decode(rawSnapshot)
  } catch {
    return {
      diagnostic: {
        messageSafe: invalidInputMessageSafe,
        reason: "invalid_input",
      },
      ok: false,
    }
  }
  try {
    await write(validated)
    return { ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

export const projectTokensServedModelMixSnapshotBestEffort = (
  sql: SyncSql,
  rawSnapshot: unknown,
): Promise<TokensServedMixProjectionOutcome> =>
  bestEffortProject(
    decodeTokensServedModelMixSnapshotEntity,
    "model-mix snapshot post-image failed contract validation",
    validated => projectTokensServedModelMixSnapshot(sql, validated),
    rawSnapshot,
  )

export const projectTokensServedDemandMixSnapshotBestEffort = (
  sql: SyncSql,
  rawSnapshot: unknown,
): Promise<TokensServedMixProjectionOutcome> =>
  bestEffortProject(
    decodeTokensServedDemandMixSnapshotEntity,
    "demand-mix snapshot post-image failed contract validation",
    validated => projectTokensServedDemandMixSnapshot(sql, validated),
    rawSnapshot,
  )

export const projectTokensServedChannelMixSnapshotBestEffort = (
  sql: SyncSql,
  rawSnapshot: unknown,
): Promise<TokensServedMixProjectionOutcome> =>
  bestEffortProject(
    decodeTokensServedChannelMixSnapshotEntity,
    "channel-mix snapshot post-image failed contract validation",
    validated => projectTokensServedChannelMixSnapshot(sql, validated),
    rawSnapshot,
  )

export const projectTokensServedHistorySnapshotBestEffort = (
  sql: SyncSql,
  rawSnapshot: unknown,
): Promise<TokensServedMixProjectionOutcome> =>
  bestEffortProject(
    decodeTokensServedHistorySnapshotEntity,
    "history snapshot post-image failed contract validation",
    validated => projectTokensServedHistorySnapshot(sql, validated),
    rawSnapshot,
  )

// ---------------------------------------------------------------------------
// Read (latest post-image for one entityType/entityId, straight off the
// changelog — no bespoke storage table)
// ---------------------------------------------------------------------------

interface ChangelogPostImageRow {
  readonly post_image_json: string | object | null
}

const parseJson = (raw: string | object | null): unknown =>
  raw === null ? null : typeof raw === "string" ? JSON.parse(raw) : raw

const readLatestSnapshotJson = async (
  sql: SqlTag,
  entityType: string,
  entityId: string,
): Promise<unknown | null> => {
  const rows: Array<ChangelogPostImageRow> = await sql`
    SELECT post_image_json
      FROM khala_sync_changelog
     WHERE scope = ${tokensServedAggregatesPublicScope()}
       AND entity_type = ${entityType}
       AND entity_id = ${entityId}
       AND op = 'upsert'
     ORDER BY version DESC
     LIMIT 1
  `
  return parseJson(rows[0]?.post_image_json ?? null)
}

export const readTokensServedModelMixSnapshot = async (
  sql: SqlTag,
  window: TokensServedWindow,
): Promise<TokensServedModelMixSnapshotEntity | null> => {
  const json = await readLatestSnapshotJson(
    sql,
    TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE,
    tokensServedMixSnapshotEntityId(window),
  )
  return json === null ? null : decodeTokensServedModelMixSnapshotEntity(json)
}

export const readTokensServedDemandMixSnapshot = async (
  sql: SqlTag,
  window: TokensServedWindow,
): Promise<TokensServedDemandMixSnapshotEntity | null> => {
  const json = await readLatestSnapshotJson(
    sql,
    TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE,
    tokensServedMixSnapshotEntityId(window),
  )
  return json === null ? null : decodeTokensServedDemandMixSnapshotEntity(json)
}

export const readTokensServedChannelMixSnapshot = async (
  sql: SqlTag,
  window: TokensServedWindow,
): Promise<TokensServedChannelMixSnapshotEntity | null> => {
  const json = await readLatestSnapshotJson(
    sql,
    TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE,
    tokensServedMixSnapshotEntityId(window),
  )
  return json === null
    ? null
    : decodeTokensServedChannelMixSnapshotEntity(json)
}

export const readTokensServedHistorySnapshot = async (
  sql: SqlTag,
  window: TokensServedWindow,
  timezone: string,
): Promise<TokensServedHistorySnapshotEntity | null> => {
  const json = await readLatestSnapshotJson(
    sql,
    TOKENS_SERVED_HISTORY_ENTITY_TYPE,
    tokensServedHistorySnapshotEntityId(window, timezone),
  )
  return json === null ? null : decodeTokensServedHistorySnapshotEntity(json)
}
