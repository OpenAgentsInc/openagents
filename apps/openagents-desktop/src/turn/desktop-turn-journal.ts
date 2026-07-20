import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Effect, Layer, Ref, Schema as S } from "effect"

import type { TurnRequestRef } from "@openagentsinc/agent-runtime-schema"
import {
  TurnJournal,
  TurnJournalError,
  type TurnStateRecord,
} from "@openagentsinc/agent-turn-runtime"
import {
  decodeTurnRecord,
  encodeTurnRecord,
  PersistedTurnRecord,
} from "@openagentsinc/agent-turn-store"

import type { LocalTurnJournal, LocalTurnKey } from "../local-turn-journal.ts"

/**
 * AFS-01 Desktop transition adapter: durable kernel turn journal, wrapping
 * `local-turn-journal.ts` for recovery continuity.
 *
 * The kernel record is the canonical driver-neutral turn state. This adapter
 * persists it to a private, atomically written JSON file, using the store
 * package's driver-neutral serialization. It therefore round-trips every kernel
 * record and reproduces the shared state-transition corpus exactly as the
 * in-memory adapter does.
 *
 * It also mirrors terminal lifecycle into the existing local turn journal (best
 * effort) so the legacy restart-recovery path keeps working during the
 * transition. The mirror never invents legacy identity; it only settles a legacy
 * record that already exists.
 */
export const DESKTOP_TURN_JOURNAL_FILE_SCHEMA = "openagents.desktop.agent_turn_journal.v1" as const

const DesktopTurnJournalFile = S.Struct({
  schema: S.Literal(DESKTOP_TURN_JOURNAL_FILE_SCHEMA),
  records: S.Array(PersistedTurnRecord),
})

const decodeFile = S.decodeUnknownSync(DesktopTurnJournalFile)

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
    throw error
  }
}

const readFile = (filePath: string): ReadonlyArray<PersistedTurnRecord> => {
  if (!existsSync(filePath)) return []
  return decodeFile(JSON.parse(readFileSync(filePath, "utf8"))).records
}

const isTerminalState = (record: TurnStateRecord): boolean =>
  record.state === "completed" ||
  record.state === "refused" ||
  record.state === "failed" ||
  record.state === "cancelled"

export interface DesktopTurnJournalOptions {
  readonly filePath: string
  /**
   * Optional legacy journal + lane resolver. When present, a terminal kernel
   * record settles a matching legacy record so restart recovery stays honest
   * during the transition.
   */
  readonly legacy?: {
    readonly journal: LocalTurnJournal
    readonly laneRef: (record: TurnStateRecord) => string
  }
}

const journalError = (reason: "storage_unavailable" | "invalid_record") =>
  new TurnJournalError({ reason })

const mirrorLegacyTerminal = (options: DesktopTurnJournalOptions, record: TurnStateRecord): void => {
  if (options.legacy === undefined || !isTerminalState(record)) return
  const key: LocalTurnKey = {
    threadRef: record.threadRef,
    turnRef: record.requestRef,
    lane: options.legacy.laneRef(record),
  }
  const existing = options.legacy.journal.get(key)
  if (existing === null) return
  if (record.state === "completed") options.legacy.journal.terminal(key, "completed", "completed")
  else if (record.state === "cancelled") options.legacy.journal.terminal(key, "interrupted", "owner_interrupted")
  else options.legacy.journal.terminal(key, "failed", "failed")
}

/** Wrap a Desktop turn journal file as the kernel `TurnJournal` port. */
export const desktopTurnJournalLayer = (options: DesktopTurnJournalOptions): Layer.Layer<TurnJournal> =>
  Layer.effect(
    TurnJournal,
    Effect.gen(function* () {
      const initial = new Map<string, PersistedTurnRecord>()
      for (const persisted of readFile(options.filePath)) initial.set(persisted.requestRef, persisted)
      const store = yield* Ref.make(initial)

      const persist = (map: ReadonlyMap<string, PersistedTurnRecord>): void =>
        writePrivateAtomic(options.filePath, {
          schema: DESKTOP_TURN_JOURNAL_FILE_SCHEMA,
          records: [...map.values()],
        })

      return TurnJournal.of({
        record: (state) =>
          Effect.try({ try: () => encodeTurnRecord(state), catch: () => journalError("invalid_record") }).pipe(
            Effect.flatMap((encoded) =>
              Ref.updateAndGet(store, (map) => new Map(map).set(state.requestRef, encoded)).pipe(
                Effect.flatMap((map) =>
                  Effect.try({
                    try: () => {
                      persist(map)
                      mirrorLegacyTerminal(options, state)
                    },
                    catch: () => journalError("storage_unavailable"),
                  }),
                ),
              ),
            ),
          ),
        load: (requestRef: TurnRequestRef) =>
          Ref.get(store).pipe(
            Effect.flatMap((map) => {
              const persisted = map.get(requestRef)
              return persisted === undefined
                ? Effect.succeed<TurnStateRecord | null>(null)
                : Effect.try({ try: () => decodeTurnRecord(persisted), catch: () => journalError("invalid_record") })
            }),
          ),
        list: Ref.get(store).pipe(
          Effect.flatMap((map) =>
            Effect.try({
              try: () => [...map.values()].map((persisted) => decodeTurnRecord(persisted)),
              catch: () => journalError("invalid_record"),
            }),
          ),
        ),
      })
    }),
  )

export { TurnJournal }
