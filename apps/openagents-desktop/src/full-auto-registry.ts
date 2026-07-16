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
 */
export const FULL_AUTO_REGISTRY_SCHEMA = "openagents.desktop.full_auto_registry.v1" as const
export const FULL_AUTO_RECORD_LIMIT = 128

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Cursor = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

export const FullAutoRecordSchema = Schema.Struct({
  threadRef: Ref,
  enabled: Schema.Boolean,
  /** Consecutive auto-dispatched continuations since the last manual send or reset. */
  continuationCount: Cursor,
  updatedAt: Schema.String,
})
export type FullAutoRecord = typeof FullAutoRecordSchema.Type

const FullAutoRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_REGISTRY_SCHEMA),
  records: Schema.Array(FullAutoRecordSchema).check(Schema.isMaxLength(FULL_AUTO_RECORD_LIMIT)),
})

export type FullAutoRegistry = Readonly<{
  list: () => ReadonlyArray<FullAutoRecord>
  get: (threadRef: string) => boolean
  enabledThreads: () => ReadonlyArray<string>
  set: (threadRef: string, enabled: boolean) => FullAutoRecord
  incrementContinuation: (threadRef: string) => number
  resetContinuation: (threadRef: string) => void
}>

export class FullAutoRegistryError extends Error {
  readonly _tag = "FullAutoRegistryError"
  override readonly name = "FullAutoRegistryError"

  constructor(
    readonly reason: "invalid_registry" | "storage_unavailable",
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

const decodeFile = (filePath: string): ReadonlyArray<FullAutoRecord> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(FullAutoRegistryFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.records
  } catch {
    throw new FullAutoRegistryError(
      "invalid_registry",
      "full auto registry failed validation",
    )
  }
}

export const openFullAutoRegistry = (file: string, now: () => Date = () => new Date()): FullAutoRegistry => {
  const filePath = path.resolve(file)
  let records = [...decodeFile(filePath)]

  const persist = (): void => {
    records = records
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, FULL_AUTO_RECORD_LIMIT)
    writePrivateAtomic(filePath, { schema: FULL_AUTO_REGISTRY_SCHEMA, records })
  }
  const findIndex = (threadRef: string): number => records.findIndex(record => record.threadRef === threadRef)

  return {
    list: () => [...records],
    get: threadRef => {
      const index = findIndex(threadRef)
      return index === -1 ? false : records[index]!.enabled
    },
    enabledThreads: () => records.filter(record => record.enabled).map(record => record.threadRef),
    set: (threadRef, enabled) => {
      const index = findIndex(threadRef)
      const timestamp = now().toISOString()
      const next = Schema.decodeUnknownSync(FullAutoRecordSchema)({
        threadRef,
        enabled,
        continuationCount: index === -1 || !enabled ? 0 : records[index]!.continuationCount,
        updatedAt: timestamp,
      })
      if (index === -1) records.push(next)
      else records[index] = next
      persist()
      return next
    },
    incrementContinuation: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return 0
      const next = { ...records[index]!, continuationCount: records[index]!.continuationCount + 1, updatedAt: now().toISOString() }
      records[index] = Schema.decodeUnknownSync(FullAutoRecordSchema)(next)
      persist()
      return next.continuationCount
    },
    resetContinuation: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return
      records[index] = { ...records[index]!, continuationCount: 0, updatedAt: now().toISOString() }
      persist()
    },
  }
}
