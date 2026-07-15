import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"

import type {
  CodexDecodedPayload,
  CodexProtocolDecodeFailure,
  CodexProtocolDecodeResult,
} from "@openagentsinc/codex-app-server-protocol/decode"

export const CODEX_NATIVE_JOURNAL_SCHEMA = "openagents.desktop.codex_native.v1" as const

export type CodexNativeIdentity = Readonly<{
  generation: number
  sequence: number
  observedAt: string
  requestId: string | number | null
  threadId: string | null
  turnId: string | null
  itemId: string | null
}>

export type CodexNativeEnvelope = Readonly<{
  _tag: "CodexNativeEnvelope"
  identity: CodexNativeIdentity
  direction: CodexDecodedPayload["direction"]
  method: string
  /** Private, generated-schema-decoded provider payload. Never serialize this field. */
  payload: unknown
  retention: "durable-semantic" | "bounded-transient"
}>

export type CodexCompatibilityReceipt = Readonly<{
  _tag: "CodexCompatibilityReceipt"
  generation: number
  observedAt: string
  direction: CodexProtocolDecodeFailure["direction"]
  method: string
  reason: CodexProtocolDecodeFailure["reason"]
  detail: string
  occurrences: number
}>

export type CodexNativeJournalEntry = Readonly<{
  generation: number
  sequence: number
  observedAt: string
  direction: CodexDecodedPayload["direction"]
  method: string
  requestId: string | number | null
  threadId: string | null
  turnId: string | null
  itemId: string | null
  itemType: string | null
  status: string | null
}>

type NativeJournal = Readonly<{
  schema: typeof CODEX_NATIVE_JOURNAL_SCHEMA
  entries: ReadonlyArray<CodexNativeJournalEntry>
}>

export type CodexNativeEventPlane = Readonly<{
  accept: (input: Readonly<{
    generation: number
    requestId?: string | number | null
    decoded: CodexProtocolDecodeResult
  }>) => CodexNativeEnvelope | CodexCompatibilityReceipt
  envelopes: (filter?: Readonly<{ threadId?: string; turnId?: string; itemId?: string; method?: string }>) => ReadonlyArray<CodexNativeEnvelope>
  receipts: () => ReadonlyArray<CodexCompatibilityReceipt>
  journal: () => ReadonlyArray<CodexNativeJournalEntry>
  thread: (threadId: string) => CodexNativeEnvelope | null
  turn: (turnId: string) => CodexNativeEnvelope | null
  item: (itemId: string) => CodexNativeEnvelope | null
}>

const asObject = (value: unknown): Readonly<Record<string, unknown>> | null =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Readonly<Record<string, unknown>>
    : null

const stringField = (value: unknown, key: string): string | null => {
  const object = asObject(value)
  return typeof object?.[key] === "string" ? object[key] : null
}

const providerIdentities = (payload: unknown): Readonly<{
  threadId: string | null
  turnId: string | null
  itemId: string | null
}> => {
  const direct = asObject(payload)
  return {
    threadId: stringField(direct, "threadId") ?? stringField(direct, "thread_id") ?? stringField(direct?.thread, "id"),
    turnId: stringField(direct, "turnId") ?? stringField(direct, "turn_id") ?? stringField(direct?.turn, "id"),
    itemId: stringField(direct, "itemId") ?? stringField(direct, "item_id") ?? stringField(direct?.item, "id"),
  }
}

const isTransient = (method: string): boolean =>
  method.includes("/delta") || method.includes("outputDelta") || method.includes("terminalInteraction") ||
  method.startsWith("thread/realtime/") || method === "rawResponseItem/completed"

const journalEntry = (envelope: CodexNativeEnvelope): CodexNativeJournalEntry => {
  const payload = asObject(envelope.payload)
  const item = asObject(payload?.item)
  const turn = asObject(payload?.turn)
  return {
    generation: envelope.identity.generation,
    sequence: envelope.identity.sequence,
    observedAt: envelope.identity.observedAt,
    direction: envelope.direction,
    method: envelope.method,
    requestId: envelope.identity.requestId,
    threadId: envelope.identity.threadId,
    turnId: envelope.identity.turnId,
    itemId: envelope.identity.itemId,
    itemType: typeof item?.type === "string" ? item.type : null,
    status: typeof item?.status === "string" ? item.status : typeof turn?.status === "string" ? turn.status : null,
  }
}

const readJournal = (path: string | undefined): CodexNativeJournalEntry[] => {
  if (path === undefined) return []
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as Partial<NativeJournal>
    if (parsed.schema !== CODEX_NATIVE_JOURNAL_SCHEMA || !Array.isArray(parsed.entries)) return []
    return parsed.entries.filter(entry => entry !== null && typeof entry === "object") as CodexNativeJournalEntry[]
  } catch {
    return []
  }
}

export const makeCodexNativeEventPlane = (options: Readonly<{
  journalPath?: string
  maxDurableEntries?: number
  maxTransientEntries?: number
  now?: () => Date
  onCompatibilityReceipt?: (receipt: CodexCompatibilityReceipt) => void
}> = {}): CodexNativeEventPlane => {
  const maxDurableEntries = Math.max(1, Math.floor(options.maxDurableEntries ?? 4_096))
  const maxTransientEntries = Math.max(1, Math.floor(options.maxTransientEntries ?? 256))
  const durable: CodexNativeEnvelope[] = []
  const transient: CodexNativeEnvelope[] = []
  const persisted = readJournal(options.journalPath).slice(-maxDurableEntries)
  const compatibility = new Map<string, CodexCompatibilityReceipt>()
  const threads = new Map<string, CodexNativeEnvelope>()
  const turns = new Map<string, CodexNativeEnvelope>()
  const items = new Map<string, CodexNativeEnvelope>()
  let sequence = persisted.reduce((maximum, entry) => Math.max(maximum, entry.sequence), 0)

  const persist = (): void => {
    if (options.journalPath === undefined) return
    mkdirSync(dirname(options.journalPath), { recursive: true })
    const temporary = `${options.journalPath}.tmp`
    writeFileSync(temporary, `${JSON.stringify({ schema: CODEX_NATIVE_JOURNAL_SCHEMA, entries: persisted }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, options.journalPath)
  }

  return {
    accept: ({ generation, requestId = null, decoded }) => {
      const observedAt = (options.now?.() ?? new Date()).toISOString()
      if (decoded._tag === "DecodeFailure") {
        const key = `${decoded.direction}\u0000${decoded.method}\u0000${decoded.reason}`
        const previous = compatibility.get(key)
        const receipt: CodexCompatibilityReceipt = {
          _tag: "CodexCompatibilityReceipt",
          generation,
          observedAt,
          direction: decoded.direction,
          method: decoded.method.slice(0, 200),
          reason: decoded.reason,
          detail: decoded.detail.slice(0, 1_000),
          occurrences: (previous?.occurrences ?? 0) + 1,
        }
        compatibility.set(key, receipt)
        if (previous === undefined) options.onCompatibilityReceipt?.(receipt)
        return receipt
      }

      const provider = providerIdentities(decoded.payload)
      const identity: CodexNativeIdentity = {
        generation,
        sequence: ++sequence,
        observedAt,
        requestId,
        ...provider,
      }
      const retention = isTransient(decoded.method) ? "bounded-transient" : "durable-semantic"
      const envelope: CodexNativeEnvelope = {
        _tag: "CodexNativeEnvelope",
        identity,
        direction: decoded.direction,
        method: decoded.method,
        payload: decoded.payload,
        retention,
      }
      const target = retention === "durable-semantic" ? durable : transient
      target.push(envelope)
      const limit = retention === "durable-semantic" ? maxDurableEntries : maxTransientEntries
      if (target.length > limit) target.splice(0, target.length - limit)
      if (retention === "durable-semantic") {
        if (identity.threadId !== null) threads.set(identity.threadId, envelope)
        if (identity.turnId !== null) turns.set(identity.turnId, envelope)
        if (identity.itemId !== null) {
          const current = items.get(identity.itemId)
          if (current?.method !== "item/completed" || envelope.method === "item/completed") {
            items.set(identity.itemId, envelope)
          }
        }
        persisted.push(journalEntry(envelope))
        if (persisted.length > maxDurableEntries) persisted.splice(0, persisted.length - maxDurableEntries)
        persist()
      }
      return envelope
    },
    envelopes: (filter = {}) => [...durable, ...transient]
      .sort((left, right) => left.identity.sequence - right.identity.sequence)
      .filter(envelope =>
        (filter.threadId === undefined || envelope.identity.threadId === filter.threadId) &&
        (filter.turnId === undefined || envelope.identity.turnId === filter.turnId) &&
        (filter.itemId === undefined || envelope.identity.itemId === filter.itemId) &&
        (filter.method === undefined || envelope.method === filter.method)),
    receipts: () => [...compatibility.values()],
    journal: () => [...persisted],
    thread: threadId => threads.get(threadId) ?? null,
    turn: turnId => turns.get(turnId) ?? null,
    item: itemId => items.get(itemId) ?? null,
  }
}
