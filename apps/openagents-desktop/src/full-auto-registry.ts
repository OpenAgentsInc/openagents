import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

/**
 * Full Auto (#8853): durable per-thread continuation state, owned by main.
 * This is the piece that makes Full Auto survive an app restart -- the
 * renderer no longer decides whether to continue; it only tells main "this
 * thread is enabled" (or not), and main re-evaluates that durable fact both
 * after every completed turn and again at startup, exactly the way
 * local-turn-journal.ts already owns interrupted-turn recovery truth.
 *
 * Wave 2 (FA-H2 #8875, FA-H3 #8876, FA-H5 #8878, FA-H6 #8879) extends the
 * record from an enabled-only flag toward the roadmap's durable next-turn
 * record: granted-workspace identity, bound execution profile, a durable
 * dispatch lease, and typed failure/backoff state. Every new field is
 * OPTIONAL so an existing v1 registry file still decodes -- the FA-H10
 * quarantine path must never eat a user's state because of this upgrade.
 */
export const FULL_AUTO_REGISTRY_SCHEMA = "openagents.desktop.full_auto_registry.v1" as const
export const FULL_AUTO_RECORD_LIMIT = 128
export const FULL_AUTO_BLOCKED_REASON_LIMIT = 300

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Cursor = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/**
 * FA-H6 (#8879): the execution profile the loop was granted with. Bound from
 * the initiating (renderer-sent) Full Auto turn; continuations replay it so a
 * restart-resumed loop runs on the same account/model/effort the owner
 * started it with. Fields are durable plain strings (revalidated against the
 * live contract enums at dispatch time) so a future enum change can never
 * corrupt-fail the whole registry file.
 */
export const FullAutoProfileSchema = Schema.Struct({
  /** L6 #8901: durable ProviderLane.laneRef. Optional keeps every rev-7 row decodable. */
  lane: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  accountRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  model: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  reasoningEffort: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40))),
})
export type FullAutoProfile = typeof FullAutoProfileSchema.Type

export const FullAutoRecordSchema = Schema.Struct({
  threadRef: Ref,
  enabled: Schema.Boolean,
  /**
   * Consecutive auto-dispatched continuations since Full Auto was last enabled
   * for this thread (toggling off resets it). A manual send while the toggle
   * stays on does NOT reset the count -- see FA-H7 (#8880) and the pinning
   * test in tests/full-auto-registry.test.ts. Since FA-H5 (#8878) the count
   * increments only on a SUCCESSFUL dispatch: a failed dispatch never consumes
   * a cap slot (it consumes failure/backoff budget instead).
   */
  continuationCount: Cursor,
  updatedAt: Schema.String,
  /**
   * FA-H2 (#8875): the absolute workspace path granted when Full Auto was
   * enabled. Dispatch refuses (and disables the record) when the currently
   * resolved workspace differs; a record with no workspaceRef fails closed.
   */
  workspaceRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))),
  /** FA-H6 (#8879): execution profile bound by the initiating flagged turn. */
  profile: Schema.optional(FullAutoProfileSchema),
  /**
   * FA-H3 (#8876): the durable dispatch lease. While a continuation turn ref
   * is claimed here, no other reconcile pass may dispatch this thread. Cleared
   * on dispatch completion (success or failure); a stale lease whose turn ref
   * never reached the local-turn journal is cleared at startup reconciliation.
   */
  pendingTurnRef: Schema.optional(Schema.NullOr(Ref)),
  pendingStartedAt: Schema.optional(Schema.String),
  /** FA-H5 (#8878): failure/backoff state. */
  lastFailureAt: Schema.optional(Schema.String),
  consecutiveFailures: Schema.optional(Cursor),
  /** FA-H2/FA-H5: typed, owner-visible reason the loop is blocked/disabled. */
  blockedReason: Schema.optional(Schema.NullOr(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  ))),
})
export type FullAutoRecord = typeof FullAutoRecordSchema.Type

/**
 * The record bound (FULL_AUTO_RECORD_LIMIT) is enforced write-side and applies
 * only to the disabled tail (FA-H10 #8883): enabled records are never evicted,
 * so a legitimately persisted file may exceed the limit when more than
 * FULL_AUTO_RECORD_LIMIT threads are enabled at once. The decode schema
 * therefore carries no max-length check.
 */
const FullAutoRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_REGISTRY_SCHEMA),
  records: Schema.Array(FullAutoRecordSchema),
})

export type FullAutoSetOptions = Readonly<{
  /** FA-H2: bind the granted workspace at enable time. */
  workspaceRef?: string
  /** FA-H6: bind the execution profile at enable time. */
  profile?: FullAutoProfile
  /** FA-H2/FA-H5: typed reason recorded when DISABLING a record. */
  blockedReason?: string
}>

export type FullAutoRegistry = Readonly<{
  list: () => ReadonlyArray<FullAutoRecord>
  get: (threadRef: string) => boolean
  record: (threadRef: string) => FullAutoRecord | null
  enabledThreads: () => ReadonlyArray<string>
  set: (threadRef: string, enabled: boolean, options?: FullAutoSetOptions) => FullAutoRecord
  incrementContinuation: (threadRef: string) => number
  /**
   * FA-H3: claim the durable dispatch lease with the continuation's exact
   * turn ref. Returns false (no write) when the record is missing or another
   * lease is already held -- the caller must then skip the thread.
   */
  claimPending: (threadRef: string, turnRef: string) => boolean
  clearPending: (threadRef: string) => void
  /**
   * FA-H5: record a failed dispatch -- increments consecutiveFailures, stamps
   * lastFailureAt, sets blockedReason, and releases the lease. Returns the new
   * consecutive-failure count (0 when the record is missing).
   */
  recordFailure: (threadRef: string, reason: string) => number
  /** FA-H5: a successful dispatch clears failure state and the lease. */
  recordSuccess: (threadRef: string) => void
  bindWorkspace: (threadRef: string, workspaceRef: string) => FullAutoRecord | null
  bindProfile: (threadRef: string, profile: FullAutoProfile) => FullAutoRecord | null
}>

export class FullAutoRegistryError extends Error {
  readonly _tag = "FullAutoRegistryError"
  override readonly name = "FullAutoRegistryError"

  constructor(
    readonly reason: "storage_unavailable",
    message: string,
  ) {
    super(message)
  }
}

const ensurePrivateParent = (filePath: string): void => {
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
}

const writePrivateAtomic = (filePath: string, value: unknown): void => {
  ensurePrivateParent(filePath)
  const pending = `${filePath}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new FullAutoRegistryError(
      "storage_unavailable",
      error instanceof Error ? error.message : "full auto registry unavailable",
    )
  }
}

/**
 * FA-H10 (#8883): a corrupt or schema-invalid registry file must never block
 * app initialization -- Full Auto is a non-critical automation preference, so
 * decode failure fails closed for the feature (empty registry, nothing
 * enabled) and open for the app. The bad file is quarantined beside the
 * registry (best-effort) so the evidence survives for diagnosis instead of
 * being silently overwritten by the next persist.
 */
const decodeFile = (filePath: string, now: () => Date): ReadonlyArray<FullAutoRecord> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(FullAutoRegistryFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.records
  } catch (error) {
    const quarantinePath = `${filePath}.quarantined-${now().toISOString()}`
    try {
      renameSync(filePath, quarantinePath)
      console.error(
        `full auto registry failed validation; quarantined the corrupt file at ${quarantinePath} and starting with an empty registry (Full Auto disabled for all threads)`,
        error,
      )
    } catch {
      console.error(
        `full auto registry failed validation and the corrupt file at ${filePath} could not be quarantined; starting with an empty registry (Full Auto disabled for all threads)`,
        error,
      )
    }
    return []
  }
}

/** Drop `undefined`-valued keys so exact-optional record decodes stay clean
 * and cleared fields disappear from the durable file instead of persisting
 * as explicit nulls forever. */
const compactRecordInput = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined))

export const openFullAutoRegistry = (file: string, now: () => Date = () => new Date()): FullAutoRegistry => {
  const filePath = path.resolve(file)
  let records = [...decodeFile(filePath, now)]

  /**
   * FA-H10 (#8883): eviction never drops an `enabled: true` record -- an
   * owner-enabled thread must survive to the next restart no matter how many
   * other records were touched more recently. Only the disabled tail is
   * bounded: all enabled records are kept, then remaining capacity is filled
   * with the most-recently-updated disabled records.
   */
  const persist = (): void => {
    const sorted = [...records].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    const enabled = sorted.filter(record => record.enabled)
    const disabled = sorted.filter(record => !record.enabled)
    records = [...enabled, ...disabled.slice(0, Math.max(0, FULL_AUTO_RECORD_LIMIT - enabled.length))]
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    writePrivateAtomic(filePath, { schema: FULL_AUTO_REGISTRY_SCHEMA, records })
  }
  const findIndex = (threadRef: string): number => records.findIndex(record => record.threadRef === threadRef)

  const update = (index: number, patch: Record<string, unknown>): FullAutoRecord => {
    const next = Schema.decodeUnknownSync(FullAutoRecordSchema)(
      compactRecordInput({ ...records[index]!, ...patch, updatedAt: now().toISOString() }),
    )
    records[index] = next
    persist()
    return next
  }

  return {
    list: () => [...records],
    get: threadRef => {
      const index = findIndex(threadRef)
      return index === -1 ? false : records[index]!.enabled
    },
    record: threadRef => {
      const index = findIndex(threadRef)
      return index === -1 ? null : records[index]!
    },
    enabledThreads: () => records.filter(record => record.enabled).map(record => record.threadRef),
    set: (threadRef, enabled, options) => {
      const index = findIndex(threadRef)
      const existing = index === -1 ? null : records[index]!
      const timestamp = now().toISOString()
      // Enabling is a fresh grant: failure/backoff state clears so a
      // previously blocked loop the owner re-enables can run immediately.
      // Disabling zeroes the cap counter (FA-H7 pinned semantic), releases
      // any lease, and records the typed blockedReason when the disable was
      // a policy stop (workspace mismatch, failure limit, cap) rather than
      // an owner toggle-off (which passes no reason and clears it).
      const next = Schema.decodeUnknownSync(FullAutoRecordSchema)(compactRecordInput({
        threadRef,
        enabled,
        continuationCount: existing === null || !enabled ? 0 : existing.continuationCount,
        updatedAt: timestamp,
        workspaceRef: options?.workspaceRef ?? existing?.workspaceRef,
        profile: options?.profile ?? existing?.profile,
        pendingTurnRef: enabled ? existing?.pendingTurnRef ?? undefined : undefined,
        pendingStartedAt: enabled ? existing?.pendingStartedAt ?? undefined : undefined,
        lastFailureAt: enabled ? undefined : existing?.lastFailureAt,
        consecutiveFailures: enabled ? undefined : existing?.consecutiveFailures,
        blockedReason: enabled ? undefined : options?.blockedReason,
      }))
      if (index === -1) records.push(next)
      else records[index] = next
      persist()
      return next
    },
    incrementContinuation: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return 0
      return update(index, { continuationCount: records[index]!.continuationCount + 1 }).continuationCount
    },
    claimPending: (threadRef, turnRef) => {
      const index = findIndex(threadRef)
      if (index === -1) return false
      const existing = records[index]!
      if (typeof existing.pendingTurnRef === "string") return false
      update(index, { pendingTurnRef: turnRef, pendingStartedAt: now().toISOString() })
      return true
    },
    clearPending: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return
      if (records[index]!.pendingTurnRef === undefined && records[index]!.pendingStartedAt === undefined) return
      update(index, { pendingTurnRef: undefined, pendingStartedAt: undefined })
    },
    recordFailure: (threadRef, reason) => {
      const index = findIndex(threadRef)
      if (index === -1) return 0
      const failures = (records[index]!.consecutiveFailures ?? 0) + 1
      update(index, {
        consecutiveFailures: failures,
        lastFailureAt: now().toISOString(),
        blockedReason: reason.slice(0, FULL_AUTO_BLOCKED_REASON_LIMIT),
        pendingTurnRef: undefined,
        pendingStartedAt: undefined,
      })
      return failures
    },
    recordSuccess: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return
      update(index, {
        consecutiveFailures: undefined,
        lastFailureAt: undefined,
        blockedReason: undefined,
        pendingTurnRef: undefined,
        pendingStartedAt: undefined,
      })
    },
    bindWorkspace: (threadRef, workspaceRef) => {
      const index = findIndex(threadRef)
      return index === -1 ? null : update(index, { workspaceRef })
    },
    bindProfile: (threadRef, profile) => {
      const index = findIndex(threadRef)
      return index === -1 ? null : update(index, { profile })
    },
  }
}
