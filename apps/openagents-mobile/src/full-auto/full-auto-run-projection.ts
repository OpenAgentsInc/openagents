import { Schema } from "effect"

/**
 * Typed mobile-side contract for the live `FullAutoRun` projection published
 * by Desktop (openagents#8981, "Publish live FullAutoRun projection from
 * Desktop for mobile"). As of this change #8981 has not landed, so this is a
 * public-safe stub shape transcribed from its parent issue (#8980) rather
 * than an imported generated schema:
 *
 *   schema id: full_auto_run.mobile_projection.v1
 *   fields: runRef, threadRef, objective, doneCondition, lifecycleState,
 *           workspaceLabel, startedAt, updatedAt
 *   (public-safe only — no raw prompts, tool output, local paths, or
 *   credentials, matching the Full Auto evidence/privacy boundary)
 *
 * When #8981 lands, reconcile this file's schema against its real published
 * shape (only field names/types should need adjustment) and update
 * `full-auto-run-projection-source.ts` to call the real endpoint.
 */
export const FullAutoRunMobileProjectionSchemaId = "full_auto_run.mobile_projection.v1"

const FullAutoRunRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(200),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const FullAutoRunThreadRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const FullAutoRunTimestamp = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(64),
)

/**
 * Lifecycle states named in #8982's acceptance text ("Running"/"Paused"/
 * "Stalled") plus the terminal states any real run authority needs. Only
 * `running`/`paused`/`stalled` count as "active" for thread-selection and
 * header-display purposes — see `isFullAutoRunLifecycleActive`.
 */
export const FullAutoRunLifecycleState = Schema.Literals([
  "running",
  "paused",
  "stalled",
  "completed",
  "failed",
  "cancelled",
])
export type FullAutoRunLifecycleState = typeof FullAutoRunLifecycleState.Type

export const FullAutoRunLifecycleStateLabel: Readonly<Record<FullAutoRunLifecycleState, string>> = {
  running: "Running",
  paused: "Paused",
  stalled: "Stalled",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
}

export const FullAutoRunMobileProjectionSchema = Schema.Struct({
  schema: Schema.Literal(FullAutoRunMobileProjectionSchemaId),
  runRef: FullAutoRunRef,
  threadRef: FullAutoRunThreadRef,
  objective: Schema.String.check(Schema.isMaxLength(2_000)),
  doneCondition: Schema.String.check(Schema.isMaxLength(2_000)),
  lifecycleState: FullAutoRunLifecycleState,
  workspaceLabel: Schema.String.check(Schema.isMaxLength(200)),
  startedAt: FullAutoRunTimestamp,
  updatedAt: FullAutoRunTimestamp,
})
export type FullAutoRunMobileProjection = typeof FullAutoRunMobileProjectionSchema.Type

export const decodeFullAutoRunMobileProjection: (value: unknown) => FullAutoRunMobileProjection =
  Schema.decodeUnknownSync(FullAutoRunMobileProjectionSchema)

/** A run is still "live" work, independent of the freshness of its last update. */
export const isFullAutoRunLifecycleActive = (state: FullAutoRunLifecycleState): boolean =>
  state === "running" || state === "paused" || state === "stalled"

/** Mobile treats an active-looking run as stale once its projection has not
 * been refreshed for this long, matching the freshness posture used by other
 * live projections (e.g. Pylon heartbeat freshness) rather than trusting a
 * possibly-abandoned `lifecycleState` forever. */
export const FULL_AUTO_RUN_STALE_AFTER_MS = 10 * 60 * 1000

export const isFullAutoRunProjectionFresh = (
  projection: FullAutoRunMobileProjection,
  nowMs: number,
  staleAfterMs: number = FULL_AUTO_RUN_STALE_AFTER_MS,
): boolean => {
  const updatedAtMs = Date.parse(projection.updatedAt)
  if (!Number.isFinite(updatedAtMs)) return false
  return nowMs - updatedAtMs < staleAfterMs
}

/** The single predicate thread-selection and header display both use to
 * decide whether a fetched projection still counts as "an active run". */
export const isFullAutoRunProjectionActive = (
  projection: FullAutoRunMobileProjection,
  nowMs: number = Date.now(),
  staleAfterMs?: number,
): boolean =>
  isFullAutoRunLifecycleActive(projection.lifecycleState) &&
  isFullAutoRunProjectionFresh(projection, nowMs, staleAfterMs)

export type FullAutoRunProjectionResult =
  | Readonly<{ state: "active"; projection: FullAutoRunMobileProjection }>
  | Readonly<{ state: "none" }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

/** Truncate the objective for the compact mobile header per #8982 ("the
 * objective (or a truncated version)"). */
export const truncateFullAutoRunObjective = (objective: string, maxLength = 96): string =>
  objective.length <= maxLength ? objective : `${objective.slice(0, maxLength - 1).trimEnd()}…`
