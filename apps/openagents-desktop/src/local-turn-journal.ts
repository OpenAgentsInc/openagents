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

export const LOCAL_TURN_JOURNAL_SCHEMA = "openagents.desktop.local_turn_journal.v1" as const
export const LOCAL_TURN_RECORD_SCHEMA = "openagents.desktop.local_turn_record.v1" as const
export const LOCAL_TURN_TEXT_LIMIT = 32_000
export const LOCAL_TURN_RECORD_LIMIT = 128

/**
 * Provider lane SPI (L1 #8899): the journal records turns for ANY registered
 * lane, keyed by its bounded lane ref — the built-in `fable-local` /
 * `codex-local` values plus future SPI lanes (ACP peers, fixtures). Widened
 * from the original two-literal union; every previously valid journal file
 * still decodes, and restart recovery fails closed for any lane that does not
 * own provider-session replay (see ./local-turn-recovery.ts).
 */
export const LocalTurnLaneSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64))
export type LocalTurnLane = typeof LocalTurnLaneSchema.Type

export const LocalTurnPhaseSchema = Schema.Literals([
  "accepted",
  "dispatching",
  "attached",
  "streaming",
  "recovering",
  "completed",
  "failed",
  "interrupted",
  "interrupted_by_restart",
])
export type LocalTurnPhase = typeof LocalTurnPhaseSchema.Type

export const LocalTurnDispositionSchema = Schema.Literals([
  "completed",
  "failed",
  "owner_interrupted",
  "resumed_after_restart",
  "interrupted_by_restart",
])
export type LocalTurnDisposition = typeof LocalTurnDispositionSchema.Type

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const OptionalRef = Schema.NullOr(Ref)
const Cursor = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const LocalTurnAssistantSegmentSchema = Schema.Struct({
  key: Ref,
  text: Schema.String.check(Schema.isMaxLength(LOCAL_TURN_TEXT_LIMIT)),
})

export const LocalTurnRecordSchema = Schema.Struct({
  schema: Schema.Literal(LOCAL_TURN_RECORD_SCHEMA),
  threadRef: Ref,
  turnRef: Ref,
  lane: LocalTurnLaneSchema,
  userMessageKey: Ref,
  assistantMessageKey: Ref,
  accountRef: OptionalRef,
  providerSessionRef: OptionalRef,
  model: OptionalRef,
  phase: LocalTurnPhaseSchema,
  persistedCursor: Cursor,
  assistantText: Schema.String.check(Schema.isMaxLength(LOCAL_TURN_TEXT_LIMIT)),
  assistantSegments: Schema.Array(LocalTurnAssistantSegmentSchema).check(Schema.isMaxLength(256)),
  recoveryGeneration: Cursor,
  disposition: Schema.NullOr(LocalTurnDispositionSchema),
  createdAt: Schema.String,
  updatedAt: Schema.String,
})
export type LocalTurnRecord = typeof LocalTurnRecordSchema.Type

const LocalTurnJournalFileSchema = Schema.Struct({
  schema: Schema.Literal(LOCAL_TURN_JOURNAL_SCHEMA),
  records: Schema.Array(LocalTurnRecordSchema).check(Schema.isMaxLength(LOCAL_TURN_RECORD_LIMIT)),
})

export type LocalTurnKey = Readonly<{
  threadRef: string
  turnRef: string
  lane: LocalTurnLane
}>

export type LocalTurnJournal = Readonly<{
  list: () => ReadonlyArray<LocalTurnRecord>
  nonterminal: () => ReadonlyArray<LocalTurnRecord>
  get: (key: LocalTurnKey) => LocalTurnRecord | null
  accept: (input: LocalTurnKey & Readonly<{
    userMessageKey: string
    assistantMessageKey: string
    accountRef?: string | null
    model?: string | null
  }>) => Readonly<{ accepted: boolean; record: LocalTurnRecord }>
  recordDispatch: (key: LocalTurnKey, accountRef: string) => LocalTurnRecord | null
  recordProviderSession: (key: LocalTurnKey, input: Readonly<{
    accountRef: string
    providerSessionRef: string
  }>) => LocalTurnRecord | null
  appendAssistantText: (key: LocalTurnKey, text: string, segmentKey?: string) => LocalTurnRecord | null
  setAssistantText: (key: LocalTurnKey, text: string) => LocalTurnRecord | null
  beginRecovery: (key: LocalTurnKey) => LocalTurnRecord | null
  terminal: (
    key: LocalTurnKey,
    phase: Extract<LocalTurnPhase, "completed" | "failed" | "interrupted" | "interrupted_by_restart">,
    disposition: LocalTurnDisposition,
  ) => LocalTurnRecord | null
}>

export class LocalTurnJournalError extends Error {
  readonly _tag = "LocalTurnJournalError"
  override readonly name = "LocalTurnJournalError"

  constructor(
    readonly reason: "invalid_journal" | "storage_unavailable" | "conflicting_turn",
    message: string,
  ) {
    super(message)
  }
}

const keyOf = (value: LocalTurnKey): string =>
  `${value.threadRef}\u0000${value.turnRef}\u0000${value.lane}`

const isTerminal = (record: LocalTurnRecord): boolean =>
  record.disposition !== null || ["completed", "failed", "interrupted", "interrupted_by_restart"].includes(record.phase)

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
    throw new LocalTurnJournalError(
      "storage_unavailable",
      error instanceof Error ? error.message : "local turn journal unavailable",
    )
  }
}

const decodeFile = (filePath: string): ReadonlyArray<LocalTurnRecord> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(LocalTurnJournalFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.records
  } catch {
    throw new LocalTurnJournalError(
      "invalid_journal",
      "local turn recovery journal failed validation",
    )
  }
}

const boundedText = (value: string): string =>
  value.length <= LOCAL_TURN_TEXT_LIMIT ? value : value.slice(0, LOCAL_TURN_TEXT_LIMIT)

export const openLocalTurnJournal = (file: string, now: () => Date = () => new Date()): LocalTurnJournal => {
  const filePath = path.resolve(file)
  let records = [...decodeFile(filePath)]

  const persist = (): void => {
    records = records
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, LOCAL_TURN_RECORD_LIMIT)
    writePrivateAtomic(filePath, { schema: LOCAL_TURN_JOURNAL_SCHEMA, records })
  }
  const findIndex = (key: LocalTurnKey): number => records.findIndex(record => keyOf(record) === keyOf(key))
  const update = (key: LocalTurnKey, change: (record: LocalTurnRecord) => LocalTurnRecord): LocalTurnRecord | null => {
    const index = findIndex(key)
    if (index === -1) return null
    const current = records[index]!
    const next = Schema.decodeUnknownSync(LocalTurnRecordSchema)(change(current))
    records[index] = next
    persist()
    return next
  }

  return {
    list: () => [...records],
    nonterminal: () => records.filter(record => !isTerminal(record)),
    get: key => records.find(record => keyOf(record) === keyOf(key)) ?? null,
    accept: input => {
      const existing = records.find(record => keyOf(record) === keyOf(input))
      if (existing !== undefined) {
        if (existing.userMessageKey !== input.userMessageKey || existing.assistantMessageKey !== input.assistantMessageKey) {
          throw new LocalTurnJournalError("conflicting_turn", "turn identity conflicts with durable recovery state")
        }
        return { accepted: false, record: existing }
      }
      const timestamp = now().toISOString()
      const record = Schema.decodeUnknownSync(LocalTurnRecordSchema)({
        schema: LOCAL_TURN_RECORD_SCHEMA,
        threadRef: input.threadRef,
        turnRef: input.turnRef,
        lane: input.lane,
        userMessageKey: input.userMessageKey,
        assistantMessageKey: input.assistantMessageKey,
        accountRef: input.accountRef ?? null,
        providerSessionRef: null,
        model: input.model ?? null,
        phase: "accepted",
        persistedCursor: 0,
        assistantText: "",
        assistantSegments: [],
        recoveryGeneration: 0,
        disposition: null,
        createdAt: timestamp,
        updatedAt: timestamp,
      })
      records.push(record)
      persist()
      return { accepted: true, record }
    },
    recordDispatch: (key, accountRef) => update(key, record => isTerminal(record) ? record : {
      ...record,
      accountRef,
      phase: "dispatching",
      updatedAt: now().toISOString(),
    }),
    recordProviderSession: (key, input) => update(key, record => isTerminal(record) ? record : {
      ...record,
      accountRef: input.accountRef,
      providerSessionRef: input.providerSessionRef,
      phase: record.assistantText === "" ? "attached" : "streaming",
      updatedAt: now().toISOString(),
    }),
    appendAssistantText: (key, text, segmentKey) => update(key, record => {
      if (isTerminal(record) || text === "") return record
      const chunk = text.slice(0, Math.max(0, LOCAL_TURN_TEXT_LIMIT - record.assistantText.length))
      if (chunk === "") return record
      const targetKey = segmentKey ?? record.assistantMessageKey
      const index = record.assistantSegments.findIndex(segment => segment.key === targetKey)
      const assistantSegments = index === -1
        ? [...record.assistantSegments, { key: targetKey, text: chunk }]
        : record.assistantSegments.map((segment, position) => position === index
            ? { ...segment, text: boundedText(segment.text + chunk) }
            : segment)
      return {
        ...record,
        assistantText: record.assistantText + chunk,
        assistantSegments,
        persistedCursor: record.persistedCursor + 1,
        phase: "streaming",
        updatedAt: now().toISOString(),
      }
    }),
    setAssistantText: (key, text) => update(key, record => isTerminal(record) ? record : {
      ...record,
      assistantText: boundedText(text),
      assistantSegments: text === "" ? [] : [{ key: record.assistantMessageKey, text: boundedText(text) }],
      persistedCursor: record.persistedCursor + 1,
      phase: "streaming",
      updatedAt: now().toISOString(),
    }),
    beginRecovery: key => update(key, record => {
      if (isTerminal(record)) return record
      if (record.recoveryGeneration > 0) {
        return {
          ...record,
          phase: "interrupted_by_restart",
          disposition: "interrupted_by_restart",
          updatedAt: now().toISOString(),
        }
      }
      return {
        ...record,
        phase: "recovering",
        recoveryGeneration: 1,
        updatedAt: now().toISOString(),
      }
    }),
    terminal: (key, phase, disposition) => update(key, record => isTerminal(record) ? record : {
      ...record,
      phase,
      disposition,
      updatedAt: now().toISOString(),
    }),
  }
}
