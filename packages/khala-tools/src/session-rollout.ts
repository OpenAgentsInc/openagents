import { randomUUID } from "node:crypto"
import { mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { Schema as S } from "effect"
import { KhalaToolEvent } from "./index.js"

export const KhalaSessionRolloutSchemaVersion = "khala.session_rollout.v1" as const

export const KhalaSessionModelItemRole = S.Literals(["system", "user", "assistant", "tool"])
export type KhalaSessionModelItemRole = typeof KhalaSessionModelItemRole.Type

export const KhalaSessionModelItem = S.Struct({
  body: S.String,
  id: S.String,
  role: KhalaSessionModelItemRole,
  toolCallId: S.optional(S.String),
})
export type KhalaSessionModelItem = typeof KhalaSessionModelItem.Type

export const KhalaSessionRolloutRecordKind = S.Literals([
  "session_started",
  "session_resumed",
  "session_forked",
  "model_item",
  "tool_event",
])
export type KhalaSessionRolloutRecordKind = typeof KhalaSessionRolloutRecordKind.Type

export const KhalaSessionRolloutRecord = S.Struct({
  createdAt: S.String,
  kind: KhalaSessionRolloutRecordKind,
  parentSessionId: S.optional(S.String),
  payload: S.Unknown,
  recordId: S.String,
  schemaVersion: S.Literal(KhalaSessionRolloutSchemaVersion),
  sequence: S.Int,
  sessionId: S.String,
})
export type KhalaSessionRolloutRecord = typeof KhalaSessionRolloutRecord.Type

export type KhalaSessionRolloutLoaded = Readonly<{
  corruptLineCount: number
  path: string
  records: ReadonlyArray<KhalaSessionRolloutRecord>
  sessionId: string
}>

export type KhalaSessionRolloutSummary = Readonly<{
  lastUpdatedAt: string
  modelItemCount: number
  path: string
  recordCount: number
  sessionId: string
}>

export type KhalaSessionRolloutCreateOptions = Readonly<{
  createdAt?: string | undefined
  parentSessionId?: string | undefined
  sessionId?: string | undefined
  stateDir: string
}>

export type KhalaSessionRolloutAppendInput = Readonly<{
  kind: KhalaSessionRolloutRecordKind
  parentSessionId?: string | undefined
  payload: unknown
}>

export type KhalaSessionRolloutAppendOptions = Readonly<{
  createdAt?: string | undefined
  stateDir: string
}>

export type KhalaSessionRolloutForkOptions = Readonly<{
  createdAt?: string | undefined
  fromSessionId: string
  newSessionId?: string | undefined
  stateDir: string
}>

export async function createKhalaSessionRollout(
  options: KhalaSessionRolloutCreateOptions,
): Promise<KhalaSessionRolloutLoaded> {
  const sessionId = normalizeSessionId(options.sessionId ?? createKhalaSessionId())
  const path = khalaSessionRolloutPath(options.stateDir, sessionId)
  await mkdir(dirname(path), { mode: 0o700, recursive: true })
  const record = makeRolloutRecord({
    createdAt: options.createdAt,
    kind: options.parentSessionId === undefined ? "session_started" : "session_forked",
    parentSessionId: options.parentSessionId,
    payload: options.parentSessionId === undefined
      ? { sessionId }
      : { fromSessionId: options.parentSessionId, sessionId },
    sequence: 0,
    sessionId,
  })
  await writeFile(path, `${JSON.stringify(record)}\n`, { mode: 0o600, flag: "wx" })
  return { corruptLineCount: 0, path, records: [record], sessionId }
}

export async function appendKhalaSessionRolloutRecord(
  stateDir: string,
  sessionId: string,
  input: KhalaSessionRolloutAppendInput,
  options: KhalaSessionRolloutAppendOptions = { stateDir },
): Promise<KhalaSessionRolloutRecord> {
  const loaded = await readKhalaSessionRollout(options.stateDir, sessionId)
  const record = makeRolloutRecord({
    createdAt: options.createdAt,
    kind: input.kind,
    parentSessionId: input.parentSessionId,
    payload: input.payload,
    sequence: nextSequence(loaded.records),
    sessionId: loaded.sessionId,
  })
  await writeFile(loaded.path, `${JSON.stringify(record)}\n`, { flag: "a", mode: 0o600 })
  return record
}

export async function appendKhalaSessionModelItem(
  stateDir: string,
  sessionId: string,
  item: KhalaSessionModelItem,
  options: Omit<KhalaSessionRolloutAppendOptions, "stateDir"> = {},
): Promise<KhalaSessionRolloutRecord> {
  const payload = S.decodeUnknownSync(KhalaSessionModelItem)(item)
  return appendKhalaSessionRolloutRecord(stateDir, sessionId, { kind: "model_item", payload }, {
    ...options,
    stateDir,
  })
}

export async function appendKhalaSessionToolEvent(
  stateDir: string,
  sessionId: string,
  event: typeof KhalaToolEvent.Type,
  options: Omit<KhalaSessionRolloutAppendOptions, "stateDir"> = {},
): Promise<KhalaSessionRolloutRecord> {
  const payload = S.decodeUnknownSync(KhalaToolEvent)(event)
  return appendKhalaSessionRolloutRecord(stateDir, sessionId, { kind: "tool_event", payload }, {
    ...options,
    stateDir,
  })
}

export async function readKhalaSessionRollout(
  stateDir: string,
  sessionId: string,
): Promise<KhalaSessionRolloutLoaded> {
  const normalized = normalizeSessionId(sessionId)
  const path = khalaSessionRolloutPath(stateDir, normalized)
  const text = await readFile(path, "utf8")
  const parsed = parseKhalaSessionRolloutText(text, path)
  if (parsed.records.length === 0) {
    throw new Error(`Khala session rollout has no readable records: ${normalized}`)
  }
  const firstSessionId = parsed.records[0]?.sessionId
  if (firstSessionId !== normalized) {
    throw new Error(`Khala session rollout id mismatch: expected ${normalized}, found ${firstSessionId ?? "unknown"}`)
  }
  return {
    corruptLineCount: parsed.corruptLineCount,
    path,
    records: parsed.records,
    sessionId: normalized,
  }
}

export async function forkKhalaSessionRollout(
  options: KhalaSessionRolloutForkOptions,
): Promise<KhalaSessionRolloutLoaded> {
  const source = await readKhalaSessionRollout(options.stateDir, options.fromSessionId)
  const fork = await createKhalaSessionRollout({
    createdAt: options.createdAt,
    parentSessionId: source.sessionId,
    sessionId: options.newSessionId,
    stateDir: options.stateDir,
  })
  const inherited = source.records
    .filter(record => record.kind === "model_item" || record.kind === "tool_event")
    .map((record, index) => makeRolloutRecord({
      createdAt: options.createdAt,
      kind: record.kind,
      parentSessionId: source.sessionId,
      payload: record.payload,
      sequence: index + 1,
      sessionId: fork.sessionId,
    }))
  if (inherited.length > 0) {
    await writeFile(
      fork.path,
      `${inherited.map(record => JSON.stringify(record)).join("\n")}\n`,
      { flag: "a", mode: 0o600 },
    )
  }
  return {
    corruptLineCount: 0,
    path: fork.path,
    records: [...fork.records, ...inherited],
    sessionId: fork.sessionId,
  }
}

export async function listKhalaSessionRollouts(stateDir: string): Promise<ReadonlyArray<KhalaSessionRolloutSummary>> {
  const dir = join(stateDir, "sessions")
  let names: string[]
  try {
    names = await readdir(dir)
  } catch {
    return []
  }
  const summaries = await Promise.all(names
    .filter(name => name.endsWith(".jsonl"))
    .map(async name => {
      const sessionId = name.slice(0, -".jsonl".length)
      try {
        const loaded = await readKhalaSessionRollout(stateDir, sessionId)
        const last = loaded.records[loaded.records.length - 1]
        return {
          lastUpdatedAt: last?.createdAt ?? new Date(0).toISOString(),
          modelItemCount: khalaSessionModelItems(loaded).length,
          path: loaded.path,
          recordCount: loaded.records.length,
          sessionId: loaded.sessionId,
        } satisfies KhalaSessionRolloutSummary
      } catch {
        return undefined
      }
    }))
  return summaries
    .filter((summary): summary is KhalaSessionRolloutSummary => summary !== undefined)
    .sort((a, b) => b.lastUpdatedAt.localeCompare(a.lastUpdatedAt))
}

export function khalaSessionModelItems(
  rollout: Pick<KhalaSessionRolloutLoaded, "records">,
): ReadonlyArray<KhalaSessionModelItem> {
  const items: KhalaSessionModelItem[] = []
  for (const record of rollout.records) {
    if (record.kind !== "model_item") continue
    try {
      items.push(S.decodeUnknownSync(KhalaSessionModelItem)(record.payload))
    } catch {
      continue
    }
  }
  return items
}

export function khalaSessionToolEvents(
  rollout: Pick<KhalaSessionRolloutLoaded, "records">,
): ReadonlyArray<typeof KhalaToolEvent.Type> {
  const events: Array<typeof KhalaToolEvent.Type> = []
  for (const record of rollout.records) {
    if (record.kind !== "tool_event") continue
    try {
      events.push(S.decodeUnknownSync(KhalaToolEvent)(record.payload))
    } catch {
      continue
    }
  }
  return events
}

export function khalaSessionRolloutPath(stateDir: string, sessionId: string): string {
  return join(stateDir, "sessions", `${normalizeSessionId(sessionId)}.jsonl`)
}

export function parseKhalaSessionRolloutText(
  text: string,
  path = "<memory>",
): Readonly<{ corruptLineCount: number; records: ReadonlyArray<KhalaSessionRolloutRecord> }> {
  const records: KhalaSessionRolloutRecord[] = []
  let corruptLineCount = 0
  const lines = text.split(/\n/)
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]?.trim()
    if (line === undefined || line.length === 0) continue
    try {
      records.push(S.decodeUnknownSync(KhalaSessionRolloutRecord)(JSON.parse(line)))
    } catch {
      if (index === lines.length - 1) {
        corruptLineCount += 1
        continue
      }
      throw new Error(`Invalid Khala session rollout record in ${path} at line ${index + 1}`)
    }
  }
  return { corruptLineCount, records }
}

export async function compactKhalaSessionRollout(stateDir: string, sessionId: string): Promise<KhalaSessionRolloutLoaded> {
  const loaded = await readKhalaSessionRollout(stateDir, sessionId)
  const tempPath = `${loaded.path}.${process.pid}.${Date.now()}.tmp`
  await writeFile(tempPath, `${loaded.records.map(record => JSON.stringify(record)).join("\n")}\n`, { mode: 0o600 })
  await rename(tempPath, loaded.path)
  return { ...loaded, corruptLineCount: 0 }
}

function makeRolloutRecord(input: {
  readonly createdAt?: string | undefined
  readonly kind: KhalaSessionRolloutRecordKind
  readonly parentSessionId?: string | undefined
  readonly payload: unknown
  readonly sequence: number
  readonly sessionId: string
}): KhalaSessionRolloutRecord {
  return S.decodeUnknownSync(KhalaSessionRolloutRecord)({
    createdAt: input.createdAt ?? new Date().toISOString(),
    kind: input.kind,
    ...(input.parentSessionId === undefined ? {} : { parentSessionId: normalizeSessionId(input.parentSessionId) }),
    payload: input.payload,
    recordId: `rollout_record.${randomUUID()}`,
    schemaVersion: KhalaSessionRolloutSchemaVersion,
    sequence: input.sequence,
    sessionId: normalizeSessionId(input.sessionId),
  })
}

function nextSequence(records: ReadonlyArray<KhalaSessionRolloutRecord>): number {
  return records.reduce((max, record) => Math.max(max, record.sequence), -1) + 1
}

function createKhalaSessionId(): string {
  return `khala_session.${randomUUID()}`
}

function normalizeSessionId(sessionId: string): string {
  const trimmed = sessionId.trim()
  if (!/^[A-Za-z0-9._-]+$/.test(trimmed)) {
    throw new Error("Khala session id may only contain letters, numbers, dot, underscore, and dash.")
  }
  return trimmed
}
