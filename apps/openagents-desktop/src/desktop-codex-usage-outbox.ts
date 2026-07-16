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

export const DESKTOP_CODEX_USAGE_OUTBOX_SCHEMA =
  "openagents.desktop.codex_usage_outbox.v1" as const
export const DESKTOP_CODEX_USAGE_OUTBOX_LIMIT = 256

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
const Count = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const Usage = Schema.Struct({
  inputTokens: Count,
  cachedInputTokens: Count,
  outputTokens: Count,
  reasoningTokens: Count,
  totalTokens: Count,
})
const Report = Schema.Struct({ observedAt: Schema.String, usage: Usage })
const Entry = Schema.Struct({
  admissionRef: Ref,
  turnRef: Ref,
  model: Ref,
  admittedAt: Schema.String,
  expiresAt: Schema.String,
  report: Schema.NullOr(Report),
  attempts: Count,
  nextAttemptAt: Schema.String,
})
const Document = Schema.Struct({
  schema: Schema.Literal(DESKTOP_CODEX_USAGE_OUTBOX_SCHEMA),
  entries: Schema.Array(Entry).check(Schema.isMaxLength(DESKTOP_CODEX_USAGE_OUTBOX_LIMIT)),
})

export type DesktopCodexUsageOutboxEntry = typeof Entry.Type

export type DesktopCodexUsageOutbox = Readonly<{
  recordAdmission: (input: Readonly<{
    admissionRef: string
    turnRef: string
    model: string
    admittedAt: string
    expiresAt: string
  }>) => void
  complete: (turnRef: string, report: NonNullable<DesktopCodexUsageOutboxEntry["report"]>) => boolean
  due: () => ReadonlyArray<DesktopCodexUsageOutboxEntry>
  success: (admissionRef: string) => void
  drop: (admissionRef: string) => void
  retry: (admissionRef: string) => void
  clear: () => void
  snapshot: () => ReadonlyArray<DesktopCodexUsageOutboxEntry>
}>

const writePrivateAtomic = (filePath: string, value: unknown): void => {
  mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(filePath), 0o700)
  const pending = `${filePath}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw error
  }
}

const readEntries = (filePath: string): Array<DesktopCodexUsageOutboxEntry> => {
  if (!existsSync(filePath)) return []
  try {
    return [...Schema.decodeUnknownSync(Document)(JSON.parse(readFileSync(filePath, "utf8"))).entries]
  } catch {
    try {
      renameSync(filePath, `${filePath}.quarantined-${new Date().toISOString().replaceAll(":", "-")}`)
    } catch {
      // Best effort quarantine. A malformed outbox must never stop Desktop.
    }
    return []
  }
}

export const openDesktopCodexUsageOutbox = (
  file: string,
  now: () => Date = () => new Date(),
): DesktopCodexUsageOutbox => {
  const filePath = path.resolve(file)
  let entries = readEntries(filePath)

  const persist = (): void => {
    const nowMs = now().getTime()
    entries = entries
      .filter(entry => Date.parse(entry.expiresAt) > nowMs)
      .sort((left, right) => right.admittedAt.localeCompare(left.admittedAt))
      .slice(0, DESKTOP_CODEX_USAGE_OUTBOX_LIMIT)
    writePrivateAtomic(filePath, { schema: DESKTOP_CODEX_USAGE_OUTBOX_SCHEMA, entries })
  }
  const remove = (admissionRef: string): void => {
    entries = entries.filter(entry => entry.admissionRef !== admissionRef)
    persist()
  }

  return {
    recordAdmission: input => {
      const record = Schema.decodeUnknownSync(Entry)({
        ...input,
        report: null,
        attempts: 0,
        nextAttemptAt: input.admittedAt,
      })
      entries = [record, ...entries.filter(entry => entry.admissionRef !== record.admissionRef)]
      persist()
    },
    complete: (turnRef, report) => {
      const index = entries.findIndex(entry => entry.turnRef === turnRef)
      if (index === -1) return false
      entries[index] = Schema.decodeUnknownSync(Entry)({
        ...entries[index]!,
        report,
        nextAttemptAt: now().toISOString(),
      })
      persist()
      return true
    },
    due: () => {
      const nowMs = now().getTime()
      return entries.filter(entry =>
        entry.report !== null &&
        Date.parse(entry.expiresAt) > nowMs &&
        Date.parse(entry.nextAttemptAt) <= nowMs,
      )
    },
    success: remove,
    drop: remove,
    retry: admissionRef => {
      const index = entries.findIndex(entry => entry.admissionRef === admissionRef)
      if (index === -1) return
      const entry = entries[index]!
      const attempts = entry.attempts + 1
      const delayMs = Math.min(30 * 60_000, 30_000 * 2 ** Math.min(attempts - 1, 10))
      entries[index] = Schema.decodeUnknownSync(Entry)({
        ...entry,
        attempts,
        nextAttemptAt: new Date(now().getTime() + delayMs).toISOString(),
      })
      persist()
    },
    clear: () => {
      entries = []
      persist()
    },
    snapshot: () => [...entries],
  }
}
