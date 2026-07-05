import {
  canonicalJson,
  type ChangelogEntry,
  decodeGymRunProgressEntity,
  encodeGymRunProgressEntity,
  EntityId,
  EntityType,
  GYM_RUN_PROGRESS_ENTITY_TYPE,
  GYM_RUN_PROGRESS_PUBLIC_CHANNEL,
  type GymRunProgressEntity,
  publicScope,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

/**
 * Gym / Harbor live run-progress public scope projection (KS-6.5, #8415;
 * SPEC §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * The `/gym` follow-along panel used to push per-run snapshots ONLY through
 * the legacy `sync-worker` outbox + `SyncRoomDurableObject`
 * (`publishGymRunProgressSnapshot` in the Worker's
 * `inference/gym/run-progress-sync.ts`). This module is the same shape of
 * dual-write the KS-6.1 fleet cockpit and KS-6.3 tokens-served projections
 * already proved: on every ingested snapshot, ALSO append the
 * already-public-safe `GymRunProgressPublicProjection` post-image to
 * `scope.public.gym-run-progress`, keyed by `entityId = runRef` — a
 * best-effort SECOND write that must never fail the caller's ingest.
 *
 * NO SCOPE OWNER, NO AGGREGATE STATE: unlike `scope.fleet_run.<id>` (which
 * needs `khala_sync_scope_owners` for owner-gating) or the tokens-served
 * counter (which needs a `khala_sync_public_counters` row for its running
 * total), this scope is PUBLIC (no owner check — see
 * `resolveScopeRead`'s `public` arm in ./scope-auth.ts) and every publish is
 * a full post-image (no increment/aggregate arithmetic). So this projector
 * needs no bespoke migration — it rides the generic `khala_sync_changelog` +
 * `khala_sync_scopes` tables `withSyncTransaction` already provides.
 *
 * REDACTION BOUNDARY (invariant 9): every post-image is allowlist-mapped
 * (`gymRunProgressPostImage`, never spread) and DECODED through
 * `GymRunProgressEntity`, whose ref/label patterns structurally refuse
 * emails, filesystem paths, and whitespace. As defense in depth,
 * `assertGymRunProgressPostImageRedacted` rejects any serialized post-image
 * matching a forbidden-material SHAPE. Deliberately NOT a bare `token`
 * match: this entity legitimately carries `promptTokens`/`completionTokens`/
 * `totalTokens` COUNT fields (same reserved-shape discipline as the
 * Worker's own `checkGymRunProgressPublicSafety`).
 *
 * HONEST STATUS (KS-6.5): this is a DUAL-WRITE ADDITION, not a cutover. The
 * live `/gym` panel is read by ANONYMOUS/logged-out visitors, but
 * `GET/WS /api/sync/connect` (and /log, /bootstrap) require an
 * authenticated actor (browser session or agent bearer) even for
 * `scope.public.*` scopes — there is no anonymous read path on the khala-sync
 * connect surface yet. So the legacy sync-worker producer in
 * `run-progress-sync.ts` remains the ONLY delivery path for anonymous `/gym`
 * visitors; this projection exists so the run-progress changelog exists in
 * Postgres for parity/inspection/future migration, matching exactly what
 * KS-6.3 (#8304) already did for tokens-served before its own client
 * repoint. Deleting the legacy producer or repointing
 * `apps/web/src/subscriptions.ts` before that anonymous-read gap is closed
 * would silently break the public follow-along panel.
 */

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/** The shared public scope every Gym run rides: `scope.public.gym-run-progress`. */
export const gymRunProgressPublicScope = () =>
  publicScope(GYM_RUN_PROGRESS_PUBLIC_CHANNEL)

// ---------------------------------------------------------------------------
// Named system writer (SPEC §7 invariant 3)
// ---------------------------------------------------------------------------

export const GYM_RUN_PROGRESS_PROJECTION_SYSTEM_REF =
  "system:gym_run_progress_projection.harbor_ingest.v1"

// ---------------------------------------------------------------------------
// Redaction guard (defense in depth behind the contract patterns)
// ---------------------------------------------------------------------------

/**
 * Material that must NEVER appear in a gym run-progress post-image, checked
 * against the canonical serialization as a last line of defense (the
 * contract ref/label patterns are the structural first line). Deliberately
 * NOT a bare `token` or `apiKey` camelCase match — this entity legitimately
 * carries `promptTokens`/`completionTokens`/`totalTokens` count fields;
 * these markers reserve leak SHAPES only, mirroring the Worker's own
 * `checkGymRunProgressPublicSafety` marker list.
 */
export const GYM_RUN_PROGRESS_POST_IMAGE_FORBIDDEN_PATTERN =
  /prompt_text|response_text|rawprompt|rawresponse|trajectory|api[_-]key|authorization:|bearer[:\s]|mnemonic|secret|\/users\/|https?:\/\//i

export class GymRunProgressPostImageRedactionError extends Error {
  readonly _tag = "GymRunProgressPostImageRedactionError"
  override readonly name = "GymRunProgressPostImageRedactionError"
  constructor() {
    super(
      "refusing to project gym_run_progress: serialized post-image matches " +
        "the forbidden-material pattern (SPEC §7 invariant 9)",
    )
  }
}

const assertGymRunProgressPostImageRedacted = (postImage: unknown): void => {
  if (GYM_RUN_PROGRESS_POST_IMAGE_FORBIDDEN_PATTERN.test(canonicalJson(postImage))) {
    throw new GymRunProgressPostImageRedactionError()
  }
}

// ---------------------------------------------------------------------------
// Allowlist mapping (raw projection shape → contract entity)
// ---------------------------------------------------------------------------

/**
 * The Worker's `GymRunProgressPublicProjection` shape — ALREADY public-safe
 * (built by `buildGymRunProgress` + `projectPublicGymRunProgress` and
 * checked by `checkGymRunProgressPublicSafety` before this function is ever
 * called). This function still allowlist-maps field by field (never
 * spreads) before decoding through the contract, per invariant 9.
 */
export type RawGymRunProgressProjection =
  | {
      readonly publication: "web_authorized"
      readonly runRef: string
      readonly jobRef: string
      readonly configId: string
      readonly agent: string
      readonly profile: {
        readonly profileRef: string
        readonly publicLabel: string
        readonly model: string
        readonly attribution: string
        readonly hardwareProfile: string
        readonly contextWindowTokens: number
      }
      readonly phase: string
      readonly decisionGrade: false
      readonly inProgress: boolean
      readonly counts: {
        readonly officialDenominator: number
        readonly completed: number
        readonly completedPassed: number
        readonly completedFailed: number
        readonly running: number
        readonly pending: number
        readonly error: number
        readonly cancelled: number
      }
      readonly passRateOverCompleted: number | null
      readonly completionFraction: number
      readonly tokens: {
        readonly promptTokens: number | null
        readonly completionTokens: number | null
        readonly totalTokens: number | null
      }
      readonly elapsedMs: number | null
      readonly lastUpdatedAt: string
      readonly caveatRefs: ReadonlyArray<string>
      readonly blockerRefs: ReadonlyArray<string>
    }
  | {
      readonly publication: "local_only"
      readonly runRef: string
      readonly inProgress: boolean
      readonly decisionGrade: false
      readonly blockerRefs: ReadonlyArray<string>
      readonly lastUpdatedAt: string
    }

export const gymRunProgressPostImage = (
  projection: RawGymRunProgressProjection,
): GymRunProgressEntity => {
  if (projection.publication === "local_only") {
    return decodeGymRunProgressEntity({
      blockerRefs: [...projection.blockerRefs],
      decisionGrade: false,
      inProgress: projection.inProgress,
      lastUpdatedAt: projection.lastUpdatedAt,
      publication: "local_only",
      runRef: projection.runRef,
    })
  }
  return decodeGymRunProgressEntity({
    agent: projection.agent,
    blockerRefs: [...projection.blockerRefs],
    caveatRefs: [...projection.caveatRefs],
    completionFraction: projection.completionFraction,
    configId: projection.configId,
    counts: {
      cancelled: projection.counts.cancelled,
      completed: projection.counts.completed,
      completedFailed: projection.counts.completedFailed,
      completedPassed: projection.counts.completedPassed,
      error: projection.counts.error,
      officialDenominator: projection.counts.officialDenominator,
      pending: projection.counts.pending,
      running: projection.counts.running,
    },
    decisionGrade: false,
    elapsedMs: projection.elapsedMs,
    inProgress: projection.inProgress,
    jobRef: projection.jobRef,
    lastUpdatedAt: projection.lastUpdatedAt,
    passRateOverCompleted: projection.passRateOverCompleted,
    phase: projection.phase,
    profile: {
      attribution: projection.profile.attribution,
      contextWindowTokens: projection.profile.contextWindowTokens,
      hardwareProfile: projection.profile.hardwareProfile,
      model: projection.profile.model,
      profileRef: projection.profile.profileRef,
      publicLabel: projection.profile.publicLabel,
    },
    publication: "web_authorized",
    runRef: projection.runRef,
    tokens: {
      completionTokens: projection.tokens.completionTokens,
      promptTokens: projection.tokens.promptTokens,
      totalTokens: projection.tokens.totalTokens,
    },
  })
}

// ---------------------------------------------------------------------------
// Append (single entity upsert — no owner check, no aggregate)
// ---------------------------------------------------------------------------

export interface GymRunProgressProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason: "storage_failed" | "redaction_refused" | "projection_failed"
  readonly messageSafe: string
}

export type GymRunProgressProjectionOutcome =
  | { readonly ok: true; readonly entry: ChangelogEntry }
  | { readonly ok: false; readonly diagnostic: GymRunProgressProjectionDiagnostic }

const diagnosticFromUnknown = (
  error: unknown,
): GymRunProgressProjectionDiagnostic => {
  if (error instanceof GymRunProgressPostImageRedactionError) {
    return { messageSafe: error.message, reason: "redaction_refused" }
  }
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, mapping/decode failures) can embed raw
  // row values or connection strings — never echo them.
  return {
    messageSafe: "gym run-progress projection failed",
    reason: "projection_failed",
  }
}

/**
 * Project ONE gym run-progress snapshot into `scope.public.gym-run-progress`
 * in ONE Postgres transaction (version allocation + append), FAIL-SOFT:
 * this function NEVER throws — any failure (connection, constraint,
 * redaction refusal) rolls back the projection transaction and comes back
 * as a typed diagnostic for the caller to log.
 *
 * v1 dual-write contract: the caller invokes this AFTER (or alongside) its
 * legacy sync-worker outbox append; a projection failure must never fail
 * that ingest path.
 */
export const projectGymRunProgressBestEffort = async (
  sql: SyncSql,
  projection: RawGymRunProgressProjection,
): Promise<GymRunProgressProjectionOutcome> => {
  // Mapping + redaction are PURE (no I/O) — do them BEFORE opening a
  // transaction so a redaction refusal truly never touches storage, and so
  // it fails the same way whether or not a database is reachable.
  let postImage: unknown
  try {
    const entity = gymRunProgressPostImage(projection)
    postImage = encodeGymRunProgressEntity(entity)
    assertGymRunProgressPostImageRedacted(postImage)
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }

  try {
    const entry = await withSyncTransaction(sql, async (writer) =>
      writer.appendChange({
        entityId: EntityId.make(projection.runRef),
        entityType: EntityType.make(GYM_RUN_PROGRESS_ENTITY_TYPE),
        mutationRef: GYM_RUN_PROGRESS_PROJECTION_SYSTEM_REF,
        op: "upsert",
        postImage,
        scope: gymRunProgressPublicScope(),
      }),
    )
    return { entry, ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}
