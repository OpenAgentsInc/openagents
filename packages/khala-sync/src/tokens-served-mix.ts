import { Schema as S } from "effect"

/**
 * Public tokens-served aggregate snapshot entity contracts (KS-6.7, #8417;
 * SPEC §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * The public "Khala Tokens Served" model-mix, demand-mix, channel-mix, and
 * per-day history reads (`public-khala-tokens-served-{model,demand,channel}-mix-routes.ts`,
 * `public-khala-tokens-served-history-routes.ts`) each compute their payload
 * live at request time from the KS-8.2 (#8308) D1/Postgres daily-rollup
 * twins via `token-usage-ledger.ts`'s `readPublicTokensServedModelMix` /
 * `readPublicTokensServedDemandMix` / `readPublicTokensServedChannelMix` /
 * `readPublicTokensServedHistory`. Those reads are already rollup-backed
 * (not an unbounded raw-event scan), but every request still repeats the
 * same small aggregate query, and two of the four routes today mislabel
 * their staleness contract as `rebuilt_on_transition` while actually being
 * `live_at_read`.
 *
 * This module is the khala-sync projection SHAPE for a stored snapshot of
 * each already-shaped mix/history read, one snapshot per bounded window
 * (`today`/`7d`/`30d`/`all` — history additionally keyed by timezone). Each
 * snapshot's post-image is EXACTLY the ledger's own shaped output for that
 * window (same `groups`/`totalTokens`/`series` fields the routes already
 * serve), so the projection and the exact ledger read stay byte-for-byte
 * comparable by construction — there is no separate shaping logic to drift.
 *
 * MANY ENTITIES, ONE SCOPE (mirrors gym-run-progress's `runRef`-keyed
 * multi-entity scope): all four snapshot kinds ride
 * `scope.public.tokens-served-aggregates`, distinguished by `entityType`
 * and keyed by `entityId = window` (mix snapshots) or
 * `entityId = "<window>:<timezone>"` (history snapshot, since the read also
 * varies by timezone; bucket is currently always `"day"`).
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): bounded window/bucket/
 * demand-kind/demand-channel/model-family literals, bounded label strings,
 * non-negative integer counts, and ISO timestamps. No raw provider id,
 * model id, account ref, actor ref, prompt, or per-event row can decode
 * into this shape — same discipline the live routes already enforce before
 * this projection is ever fed a post-image.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./fleet, ./public-counter, ./settled-feed, and ./gym.
 */

// ---------------------------------------------------------------------------
// Entity type names (changelog `entityType` values) + scope channel id
// ---------------------------------------------------------------------------

/** The `<channel>` segment of the scope: `scope.public.tokens-served-aggregates`. */
export const TOKENS_SERVED_AGGREGATES_CHANNEL_ID = "tokens-served-aggregates"

export const TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE =
  "tokens_served_model_mix_snapshot"
export const TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE =
  "tokens_served_demand_mix_snapshot"
export const TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE =
  "tokens_served_channel_mix_snapshot"
export const TOKENS_SERVED_HISTORY_ENTITY_TYPE =
  "tokens_served_history_snapshot"

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

export const TokensServedWindow = S.Literals(["today", "7d", "30d", "all"])
export type TokensServedWindow = typeof TokensServedWindow.Type

/** The only bucket supported today; modeled as a literal, not a free string. */
export const TokensServedHistoryBucket = S.Literals(["day"])
export type TokensServedHistoryBucket = typeof TokensServedHistoryBucket.Type

export const TokensServedNonNegativeInt = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const TokensServedIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

/** Calendar day key, `YYYY-MM-DD`, in the snapshot's declared timezone. */
export const TokensServedDayKey = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}$/),
)

/** Bounded public-safe label/ref string (already-sanitized labels/refs only). */
export const TokensServedLabel = S.String.check(S.isMaxLength(200))

export const TokensServedDemandKind = S.Literals([
  "external",
  "internal",
  "internal_stress",
  "own_capacity",
  "unlabeled",
])
export type TokensServedDemandKind = typeof TokensServedDemandKind.Type

export const TokensServedDemandChannel = S.Literals([
  "khala_api",
  "direct_local",
])
export type TokensServedDemandChannel = typeof TokensServedDemandChannel.Type

export const TokensServedModelFamily = S.Literals([
  "glm",
  "fireworks_deepseek",
  "pylon_codex",
  "codex_direct",
  "pylon_claude",
  "gpt_oss",
  "gemini",
  "other",
])
export type TokensServedModelFamily = typeof TokensServedModelFamily.Type

// ---------------------------------------------------------------------------
// tokens_served_model_mix_snapshot entity — entityId = window
// ---------------------------------------------------------------------------

export class TokensServedModelMixGroupEntity extends S.Class<TokensServedModelMixGroupEntity>(
  "TokensServedModelMixGroupEntity",
)({
  family: TokensServedModelFamily,
  label: TokensServedLabel,
  pct: S.Number,
  reqs: TokensServedNonNegativeInt,
  tokens: TokensServedNonNegativeInt,
}) {}

export class TokensServedModelMixSnapshotEntity extends S.Class<TokensServedModelMixSnapshotEntity>(
  "TokensServedModelMixSnapshotEntity",
)({
  generatedAt: TokensServedIsoTimestamp,
  groups: S.Array(TokensServedModelMixGroupEntity),
  totalTokens: TokensServedNonNegativeInt,
  window: TokensServedWindow,
}) {}

// ---------------------------------------------------------------------------
// tokens_served_demand_mix_snapshot entity — entityId = window
// ---------------------------------------------------------------------------

export class TokensServedDemandMixGroupEntity extends S.Class<TokensServedDemandMixGroupEntity>(
  "TokensServedDemandMixGroupEntity",
)({
  client: TokensServedLabel,
  kind: TokensServedDemandKind,
  pct: S.Number,
  reqs: TokensServedNonNegativeInt,
  source: TokensServedLabel,
  tokens: TokensServedNonNegativeInt,
}) {}

export class TokensServedDemandMixSnapshotEntity extends S.Class<TokensServedDemandMixSnapshotEntity>(
  "TokensServedDemandMixSnapshotEntity",
)({
  generatedAt: TokensServedIsoTimestamp,
  groups: S.Array(TokensServedDemandMixGroupEntity),
  totalTokens: TokensServedNonNegativeInt,
  window: TokensServedWindow,
}) {}

// ---------------------------------------------------------------------------
// tokens_served_channel_mix_snapshot entity — entityId = window
// ---------------------------------------------------------------------------

export class TokensServedChannelMixGroupEntity extends S.Class<TokensServedChannelMixGroupEntity>(
  "TokensServedChannelMixGroupEntity",
)({
  channel: TokensServedDemandChannel,
  label: TokensServedLabel,
  pct: S.Number,
  reqs: TokensServedNonNegativeInt,
  tokens: TokensServedNonNegativeInt,
}) {}

export class TokensServedChannelMixSnapshotEntity extends S.Class<TokensServedChannelMixSnapshotEntity>(
  "TokensServedChannelMixSnapshotEntity",
)({
  generatedAt: TokensServedIsoTimestamp,
  groups: S.Array(TokensServedChannelMixGroupEntity),
  totalTokens: TokensServedNonNegativeInt,
  window: TokensServedWindow,
}) {}

// ---------------------------------------------------------------------------
// tokens_served_history_snapshot entity — entityId = "<window>:<timezone>"
// ---------------------------------------------------------------------------

export class TokensServedHistoryPointEntity extends S.Class<TokensServedHistoryPointEntity>(
  "TokensServedHistoryPointEntity",
)({
  day: TokensServedDayKey,
  tokensServed: TokensServedNonNegativeInt,
}) {}

export class TokensServedHistorySnapshotEntity extends S.Class<TokensServedHistorySnapshotEntity>(
  "TokensServedHistorySnapshotEntity",
)({
  bucket: TokensServedHistoryBucket,
  generatedAt: TokensServedIsoTimestamp,
  series: S.Array(TokensServedHistoryPointEntity),
  timezone: TokensServedLabel,
  window: TokensServedWindow,
}) {}

// ---------------------------------------------------------------------------
// entityId helpers
// ---------------------------------------------------------------------------

/** Mix snapshots (model/demand/channel) are keyed by window alone. */
export const tokensServedMixSnapshotEntityId = (
  window: TokensServedWindow,
): string => window

/**
 * History snapshots are additionally keyed by timezone (bucket is currently
 * always `"day"`, so it is not part of the key). `window` values never
 * contain `:`, so this is unambiguous to split back apart if ever needed.
 */
export const tokensServedHistorySnapshotEntityId = (
  window: TokensServedWindow,
  timezone: string,
): string => `${window}:${timezone}`

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeTokensServedModelMixSnapshotEntity = S.decodeUnknownSync(
  TokensServedModelMixSnapshotEntity,
)
export const encodeTokensServedModelMixSnapshotEntity = S.encodeSync(
  TokensServedModelMixSnapshotEntity,
)

export const decodeTokensServedDemandMixSnapshotEntity = S.decodeUnknownSync(
  TokensServedDemandMixSnapshotEntity,
)
export const encodeTokensServedDemandMixSnapshotEntity = S.encodeSync(
  TokensServedDemandMixSnapshotEntity,
)

export const decodeTokensServedChannelMixSnapshotEntity = S.decodeUnknownSync(
  TokensServedChannelMixSnapshotEntity,
)
export const encodeTokensServedChannelMixSnapshotEntity = S.encodeSync(
  TokensServedChannelMixSnapshotEntity,
)

export const decodeTokensServedHistorySnapshotEntity = S.decodeUnknownSync(
  TokensServedHistorySnapshotEntity,
)
export const encodeTokensServedHistorySnapshotEntity = S.encodeSync(
  TokensServedHistorySnapshotEntity,
)
