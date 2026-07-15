import { randomUUID } from "node:crypto"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"

export type CodexQueuedIntentStatus = "queued" | "promoting" | "promoted" | "cancelled" | "failed"
export type CodexQueuedIntent = Readonly<{
  queueRef: string
  intentRef: string
  clientUserMessageId: string
  threadRef: string
  message: string
  position: number
  status: CodexQueuedIntentStatus
  revision: number
  quiescenceRef: string | null
  providerTurnId: string | null
  failure: string | null
  createdAt: string
  updatedAt: string
}>

export class CodexDurableQueueError extends Error {
  readonly _tag = "CodexDurableQueueError"
  override readonly name = "CodexDurableQueueError"
  constructor(readonly reason: "stale" | "not_found" | "not_queued" | "closed", message: string) { super(message) }
}

export type CodexDurableQueue = Readonly<{
  enqueue: (threadRef: string, message: string) => CodexQueuedIntent
  list: (threadRef?: string) => ReadonlyArray<CodexQueuedIntent>
  edit: (queueRef: string, message: string, expectedRevision: number) => CodexQueuedIntent
  cancel: (queueRef: string, expectedRevision: number) => CodexQueuedIntent
  claimNext: (threadRef: string, quiescenceRef: string) => CodexQueuedIntent | null
  admitPromotion: (queueRef: string, threadRef: string, clientUserMessageId: string) => CodexQueuedIntent
  complete: (queueRef: string, providerTurnId: string | null) => CodexQueuedIntent
  fail: (queueRef: string, detail: string) => CodexQueuedIntent
  close: () => void
}>

const read = (path: string): CodexQueuedIntent[] => {
  try {
    const value = JSON.parse(readFileSync(path, "utf8")) as { schema?: unknown; entries?: unknown }
    return value.schema === "openagents.desktop.codex_durable_queue.v1" && Array.isArray(value.entries)
      ? value.entries as CodexQueuedIntent[] : []
  } catch { return [] }
}

export const openCodexDurableQueue = (path: string, now: () => Date = () => new Date()): CodexDurableQueue => {
  let entries = read(path)
  let closed = false
  const assertOpen = () => { if (closed) throw new CodexDurableQueueError("closed", "Codex queue is closed") }
  const persist = () => {
    mkdirSync(dirname(path), { recursive: true, mode: 0o700 })
    const temporary = `${path}.tmp`
    writeFileSync(temporary, `${JSON.stringify({ schema: "openagents.desktop.codex_durable_queue.v1", entries: entries.slice(-10_000) }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, path)
  }
  const update = (queueRef: string, expectedRevision: number | null, change: (entry: CodexQueuedIntent) => Partial<CodexQueuedIntent>): CodexQueuedIntent => {
    assertOpen()
    const index = entries.findIndex(entry => entry.queueRef === queueRef)
    if (index < 0) throw new CodexDurableQueueError("not_found", "Queued intent does not exist")
    const current = entries[index]!
    if (expectedRevision !== null && current.revision !== expectedRevision) throw new CodexDurableQueueError("stale", "Queued intent changed")
    const next = { ...current, ...change(current), revision: current.revision + 1, updatedAt: now().toISOString() }
    entries[index] = next
    persist()
    return next
  }
  const reindex = (threadRef: string) => {
    let position = 0
    entries = entries.map(entry => entry.threadRef === threadRef && entry.status === "queued"
      ? { ...entry, position: ++position }
      : entry)
  }
  return {
    enqueue: (threadRef, message) => {
      assertOpen()
      const position = entries.filter(entry => entry.threadRef === threadRef && entry.status === "queued").length + 1
      const timestamp = now().toISOString()
      const entry: CodexQueuedIntent = { queueRef: `queue.${randomUUID()}`, intentRef: `intent.${randomUUID()}`, clientUserMessageId: `user.${randomUUID()}`, threadRef, message, position, status: "queued", revision: 0, quiescenceRef: null, providerTurnId: null, failure: null, createdAt: timestamp, updatedAt: timestamp }
      entries.push(entry); persist(); return entry
    },
    list: threadRef => entries.filter(entry => threadRef === undefined || entry.threadRef === threadRef).map(entry => ({ ...entry })),
    edit: (queueRef, message, revision) => update(queueRef, revision, entry => {
      if (entry.status !== "queued") throw new CodexDurableQueueError("not_queued", "Only queued intent can be edited")
      return { message }
    }),
    cancel: (queueRef, revision) => {
      const current = entries.find(entry => entry.queueRef === queueRef)
      if (current === undefined) throw new CodexDurableQueueError("not_found", "Queued intent does not exist")
      const next = update(queueRef, revision, entry => {
        if (entry.status !== "queued") throw new CodexDurableQueueError("not_queued", "Only queued intent can be cancelled")
        return { status: "cancelled", position: 0 }
      })
      reindex(current.threadRef); persist(); return next
    },
    claimNext: (threadRef, quiescenceRef) => {
      assertOpen()
      const sameBoundary = entries.find(entry => entry.threadRef === threadRef && entry.quiescenceRef === quiescenceRef)
      if (sameBoundary !== undefined) return sameBoundary.status === "promoting" ? { ...sameBoundary } : null
      const recovering = entries.find(entry => entry.threadRef === threadRef && entry.status === "promoting")
      if (recovering !== undefined) return { ...recovering }
      const next = entries.find(entry => entry.threadRef === threadRef && entry.status === "queued")
      if (next === undefined) return null
      const claimed = update(next.queueRef, next.revision, () => ({ status: "promoting", position: 0, quiescenceRef }))
      reindex(threadRef); persist(); return claimed
    },
    admitPromotion: (queueRef, threadRef, clientUserMessageId) => {
      const entry = entries.find(value => value.queueRef === queueRef)
      if (entry === undefined) throw new CodexDurableQueueError("not_found", "Queued intent does not exist")
      if (entry.status !== "promoting" || entry.threadRef !== threadRef || entry.clientUserMessageId !== clientUserMessageId) throw new CodexDurableQueueError("not_queued", "Promotion identity is not admitted")
      return { ...entry }
    },
    complete: (queueRef, providerTurnId) => update(queueRef, null, entry => {
      if (entry.status !== "promoting") throw new CodexDurableQueueError("not_queued", "Only promoting intent can complete")
      return { status: "promoted", providerTurnId }
    }),
    fail: (queueRef, detail) => update(queueRef, null, entry => ({ status: "failed", failure: detail.slice(0, 400), position: 0 })),
    close: () => { if (closed) return; persist(); closed = true },
  }
}
