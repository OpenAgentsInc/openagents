import { createHash, randomUUID } from "node:crypto"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"

import {
  codexAppServerPoolKey,
  type CodexAppServerLease,
  type CodexAppServerPoolTarget,
  type CodexAppServerSupervisor,
} from "./codex-app-server-supervisor.ts"
import type { DesktopThread } from "./chat-contract.ts"
import type { CodexHistoryRequest } from "./codex-history-host.ts"
import type { CodexHistoryAgent, CodexHistoryCatalog, CodexHistoryItem, CodexHistoryPage, CodexHistorySearchResponse } from "./codex-history-contract.ts"
import { redactCodexHistoryText } from "./codex-history.ts"

type ObjectValue = Readonly<Record<string, unknown>>
const object = (value: unknown): ObjectValue | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as ObjectValue : null
const array = (value: unknown): ReadonlyArray<unknown> => Array.isArray(value) ? value : []
const string = (value: unknown): string | null => typeof value === "string" ? value : null
const number = (value: unknown): number | null => typeof value === "number" && Number.isFinite(value) ? value : null
const bool = (value: unknown): boolean => value === true
const idOf = (value: unknown): string | null => {
  const row = object(value)
  return string(row?.id) ?? string(row?.threadId) ?? string(row?.turnId) ?? string(row?.itemId)
}
const iso = (value: unknown): string => {
  if (typeof value === "string" && !Number.isNaN(Date.parse(value))) return new Date(value).toISOString()
  if (typeof value === "number" && Number.isFinite(value)) return new Date(value > 10_000_000_000 ? value : value * 1_000).toISOString()
  return new Date(0).toISOString()
}

export type CodexThreadLimitation = "ephemeral_history" | "transient_gap" | "experimental_pagination_unavailable"
export type CodexLifecycleThread = Readonly<{
  id: string
  name: string | null
  status: string
  createdAt: string
  updatedAt: string
  ephemeral: boolean
  parentThreadId: string | null
  goal: unknown
  settings: unknown
  memoryMode: string | null
  metadata: unknown
  cwd: string | null
  model: string | null
  limitations: ReadonlyArray<CodexThreadLimitation>
}>
export type CodexLifecycleItem = Readonly<{ id: string; threadId: string; turnId: string | null; type: string; status: string | null; payload: unknown }>
export type CodexLifecycleSnapshot = Readonly<{
  revision: number
  generation: number
  observedAt: string
  repairState: "current" | "repairing" | "degraded"
  threads: ReadonlyArray<CodexLifecycleThread>
  turnsByThread: Readonly<Record<string, ReadonlyArray<unknown>>>
  itemsByThread: Readonly<Record<string, ReadonlyArray<CodexLifecycleItem>>>
  limitations: ReadonlyArray<Readonly<{ threadId: string; limitation: CodexThreadLimitation; detail: string }>>
}>

export type CodexLifecycleDangerousKind = "archive" | "unarchive" | "delete" | "rollback" | "inject_items" | "guardian_approval"
export type CodexLifecycleAuthority = Readonly<{ token: string; kind: CodexLifecycleDangerousKind; threadId: string; revision: number; expiresAt: string }>
export type CodexLifecycleReceipt = Readonly<{
  authorityHash: string
  kind: CodexLifecycleDangerousKind
  method: string
  threadIdHash: string
  revision: number
  outcome: "accepted" | "failed" | "stale" | "expired"
  observedAt: string
}>

export class CodexThreadLifecycleError extends Error {
  readonly _tag = "CodexThreadLifecycleError"
  override readonly name = "CodexThreadLifecycleError"
  constructor(readonly reason: "closed" | "stale" | "expired" | "reused" | "pagination_loop" | "invalid_response", message: string) { super(message) }
}

export type CodexThreadLifecycle = Readonly<{
  initialize: () => Promise<CodexLifecycleSnapshot>
  snapshot: () => CodexLifecycleSnapshot
  subscribe: (listener: (snapshot: CodexLifecycleSnapshot) => void) => () => void
  list: (params?: unknown) => Promise<ReadonlyArray<CodexLifecycleThread>>
  search: (query: string, params?: unknown) => Promise<ReadonlyArray<CodexLifecycleThread>>
  loaded: () => Promise<ReadonlyArray<CodexLifecycleThread>>
  read: (threadId: string, includeTurns?: boolean) => Promise<CodexLifecycleThread>
  pageTurns: (threadId: string, limit?: number) => Promise<ReadonlyArray<unknown>>
  pageItems: (threadId: string, limit?: number) => Promise<ReadonlyArray<CodexLifecycleItem>>
  start: (params: unknown) => Promise<unknown>
  resume: (threadId: string, params?: unknown) => Promise<unknown>
  fork: (threadId: string, params?: unknown) => Promise<unknown>
  setName: (threadId: string, name: string) => Promise<unknown>
  updateMetadata: (threadId: string, gitInfo: unknown) => Promise<unknown>
  updateSettings: (threadId: string, settings: Readonly<Record<string, unknown>>) => Promise<unknown>
  setGoal: (threadId: string, goal: Readonly<{ objective?: string | null; status?: string | null; tokenBudget?: number | null }>) => Promise<unknown>
  clearGoal: (threadId: string) => Promise<unknown>
  setMemoryMode: (threadId: string, memoryMode: string) => Promise<unknown>
  compact: (threadId: string) => Promise<unknown>
  unsubscribe: (threadId: string) => Promise<unknown>
  authorize: (kind: CodexLifecycleDangerousKind, threadId: string, revision: number) => CodexLifecycleAuthority
  archive: (authority: CodexLifecycleAuthority) => Promise<unknown>
  unarchive: (authority: CodexLifecycleAuthority) => Promise<unknown>
  delete: (authority: CodexLifecycleAuthority) => Promise<unknown>
  rollback: (numTurns: number, authority: CodexLifecycleAuthority) => Promise<unknown>
  injectItems: (items: ReadonlyArray<unknown>, authority: CodexLifecycleAuthority) => Promise<unknown>
  approveGuardianDeniedAction: (event: unknown, authority: CodexLifecycleAuthority) => Promise<unknown>
  runHistory: (request: CodexHistoryRequest) => Promise<unknown>
  receipts: () => ReadonlyArray<CodexLifecycleReceipt>
  close: () => void
}>

const initial = (): CodexLifecycleSnapshot => ({ revision: 0, generation: 0, observedAt: new Date(0).toISOString(), repairState: "current", threads: [], turnsByThread: {}, itemsByThread: {}, limitations: [] })
const projectThread = (raw: unknown, inheritedLimitations: ReadonlyArray<CodexThreadLimitation> = []): CodexLifecycleThread | null => {
  const row = object(raw)
  const id = idOf(raw)
  if (row === null || id === null) return null
  const status = string(object(row.status)?.type) ?? string(row.status) ?? "unknown"
  const ephemeral = bool(row.ephemeral)
  return {
    id,
    name: string(row.name) ?? string(row.title),
    status,
    createdAt: iso(row.createdAt ?? row.created_at),
    updatedAt: iso(row.updatedAt ?? row.updated_at ?? row.createdAt ?? row.created_at),
    ephemeral,
    parentThreadId: string(row.parentThreadId) ?? string(row.parent_thread_id),
    goal: row.goal ?? null,
    settings: row.settings ?? null,
    memoryMode: string(row.memoryMode),
    metadata: row.metadata ?? null,
    cwd: string(row.cwd),
    model: string(row.model),
    limitations: [...new Set([...(ephemeral ? ["ephemeral_history" as const] : []), ...inheritedLimitations])],
  }
}
const responseRows = (response: unknown, key: string): ReadonlyArray<unknown> => {
  const row = object(response)
  return array(row?.data).length > 0 ? array(row?.data) : array(row?.[key])
}
const responseCursor = (response: unknown): string | null => string(object(response)?.nextCursor)

const readReceipts = (path: string | undefined): CodexLifecycleReceipt[] => {
  if (path === undefined) return []
  try { const value = JSON.parse(readFileSync(path, "utf8")); return Array.isArray(value.receipts) ? value.receipts : [] } catch { return [] }
}

export const makeCodexThreadLifecycle = (options: Readonly<{ lease: CodexAppServerLease; receiptPath?: string; now?: () => Date; intentTtlMs?: number }>): CodexThreadLifecycle => {
  let state = initial()
  let closed = false
  let repairingGeneration: number | null = null
  const listeners = new Set<(snapshot: CodexLifecycleSnapshot) => void>()
  const visible = new Map<string, () => void>()
  const authorities = new Map<string, CodexLifecycleAuthority>()
  const receipts = readReceipts(options.receiptPath)
  const now = () => options.now?.() ?? new Date()
  const assertOpen = () => { if (closed) throw new CodexThreadLifecycleError("closed", "Codex thread lifecycle is closed") }
  const publish = (patch: Partial<CodexLifecycleSnapshot>, generation = options.lease.state().generation): CodexLifecycleSnapshot => {
    if (generation < state.generation) return state
    state = { ...state, ...patch, revision: state.revision + 1, generation, observedAt: now().toISOString() }
    for (const listener of listeners) { try { listener(state) } catch { /* isolate */ } }
    return state
  }
  const persist = () => {
    if (options.receiptPath === undefined) return
    mkdirSync(dirname(options.receiptPath), { recursive: true })
    const temporary = `${options.receiptPath}.tmp`
    writeFileSync(temporary, `${JSON.stringify({ schema: "openagents.desktop.codex_lifecycle_receipts.v1", receipts: receipts.slice(-2_048) }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, options.receiptPath)
  }
  const record = (authority: CodexLifecycleAuthority, method: string, outcome: CodexLifecycleReceipt["outcome"]) => {
    receipts.push({ authorityHash: createHash("sha256").update(authority.token).digest("hex"), kind: authority.kind, method, threadIdHash: createHash("sha256").update(authority.threadId).digest("hex"), revision: authority.revision, outcome, observedAt: now().toISOString() })
    if (receipts.length > 2_048) receipts.splice(0, receipts.length - 2_048)
    persist()
  }
  const consume = (kind: CodexLifecycleDangerousKind, authority: CodexLifecycleAuthority, method: string) => {
    assertOpen()
    const owned = authorities.get(authority.token)
    if (owned === undefined || owned.kind !== kind || owned.threadId !== authority.threadId) throw new CodexThreadLifecycleError("reused", "Lifecycle authority is unknown, mismatched, or consumed")
    authorities.delete(authority.token)
    if (Date.parse(owned.expiresAt) <= now().getTime()) { record(authority, method, "expired"); throw new CodexThreadLifecycleError("expired", "Lifecycle authority expired") }
    if (owned.revision !== state.revision || authority.revision !== state.revision) { record(authority, method, "stale"); throw new CodexThreadLifecycleError("stale", "Lifecycle state changed before mutation") }
  }
  const paged = async (method: string, params: ObjectValue, rowKey: string, max = 10_000): Promise<ReadonlyArray<unknown>> => {
    const rows: unknown[] = []
    const seenIds = new Set<string>()
    const cursors = new Set<string>()
    let cursor: string | null = null
    do {
      const response = await options.lease.request(method, { ...params, ...(cursor === null ? {} : { cursor }), limit: Math.min(100, max - rows.length) })
      for (const value of responseRows(response, rowKey)) {
        if (value === undefined || value === null) continue
        const encoded = JSON.stringify(value)
        if (encoded === undefined) continue
        const id = idOf(value) ?? createHash("sha256").update(encoded).digest("hex")
        if (!seenIds.has(id)) { seenIds.add(id); rows.push(value) }
      }
      const next = responseCursor(response)
      if (next !== null && cursors.has(next)) throw new CodexThreadLifecycleError("pagination_loop", `${method} repeated cursor`)
      if (next !== null) cursors.add(next)
      cursor = next
    } while (cursor !== null && rows.length < max)
    return rows
  }
  const rememberThreads = (threads: ReadonlyArray<CodexLifecycleThread>) => publish({ threads: [...new Map([...state.threads, ...threads].map(thread => [thread.id, thread])).values()] })
  const list = async (params: unknown = {}): Promise<ReadonlyArray<CodexLifecycleThread>> => {
    assertOpen()
    const rows = await paged("thread/list", { sortKey: "updated_at", sortDirection: "desc", ...(object(params) ?? {}) }, "threads")
    const threads = rows.flatMap(raw => projectThread(raw) ?? [])
    rememberThreads(threads)
    return threads
  }
  const loaded = async (): Promise<ReadonlyArray<CodexLifecycleThread>> => {
    assertOpen()
    const response = await options.lease.request("thread/loaded/list", {})
    const threads = responseRows(response, "threads").flatMap(raw => projectThread(raw) ?? [])
    rememberThreads(threads)
    return threads
  }
  const read = async (threadId: string, includeTurns = true): Promise<CodexLifecycleThread> => {
    assertOpen()
    let turnsIncluded = includeTurns
    let response: unknown
    try { response = await options.lease.request("thread/read", { threadId, includeTurns }) }
    catch (error) {
      if (!includeTurns) throw error
      response = await options.lease.request("thread/read", { threadId, includeTurns: false })
      turnsIncluded = false
    }
    const raw = object(response)?.thread ?? response
    const thread = projectThread(raw)
    if (thread === null) throw new CodexThreadLifecycleError("invalid_response", "thread/read omitted thread identity")
    if (!thread.ephemeral && !visible.has(threadId)) visible.set(threadId, options.lease.registerVisibleThread(threadId))
    const turns = array(object(raw)?.turns)
    publish({ threads: [...state.threads.filter(item => item.id !== thread.id), thread], ...(turnsIncluded ? { turnsByThread: { ...state.turnsByThread, [threadId]: turns } } : {}) })
    return thread
  }
  const pageTurns = async (threadId: string, limit = 10_000): Promise<ReadonlyArray<unknown>> => {
    try {
      const turns = await paged("thread/turns/list", { threadId, sortDirection: "asc", itemsView: "full" }, "turns", limit)
      publish({ turnsByThread: { ...state.turnsByThread, [threadId]: turns } })
      return turns
    } catch {
      await read(threadId, true)
      const turns = state.turnsByThread[threadId] ?? []
      publish({ limitations: [...state.limitations.filter(item => !(item.threadId === threadId && item.limitation === "experimental_pagination_unavailable")), { threadId, limitation: "experimental_pagination_unavailable", detail: "thread/turns/list unavailable; used complete thread/read state" }] })
      return turns
    }
  }
  const pageItems = async (threadId: string, limit = 10_000): Promise<ReadonlyArray<CodexLifecycleItem>> => {
    let raw: ReadonlyArray<unknown>
    try { raw = await paged("thread/items/list", { threadId, sortDirection: "asc" }, "items", limit) }
    catch {
      const turns = await pageTurns(threadId, limit)
      raw = turns.flatMap(turn => array(object(turn)?.items))
    }
    const items = raw.flatMap(value => {
      const row = object(value); const id = idOf(value)
      if (row === null || id === null) return []
      return [{ id, threadId, turnId: string(row.turnId), type: string(row.type) ?? "unknown", status: string(row.status), payload: value }]
    })
    publish({ itemsByThread: { ...state.itemsByThread, [threadId]: items } })
    return items
  }
  const reconcile = async (generation: number) => {
    if (closed || generation <= state.generation || repairingGeneration === generation) return
    repairingGeneration = generation
    publish({ repairState: "repairing" }, generation)
    const limitations = [...state.limitations.filter(item => item.limitation !== "transient_gap")]
    try {
      await loaded()
      for (const threadId of visible.keys()) {
        limitations.push({ threadId, limitation: "transient_gap", detail: "connection generation changed; durable state reconciled without a replay cursor" })
        await read(threadId, true)
        await pageItems(threadId)
      }
      publish({ repairState: "current", limitations }, generation)
    } catch {
      publish({ repairState: "degraded", limitations }, generation)
    } finally { repairingGeneration = null }
  }
  const mutate = async (method: string, params: unknown, threadId?: string) => {
    assertOpen(); const response = await options.lease.request(method, params)
    if (threadId !== undefined && method !== "thread/delete") await read(threadId, true)
    else await list()
    return response
  }
  const dangerous = async (kind: CodexLifecycleDangerousKind, method: string, params: ObjectValue, authority: CodexLifecycleAuthority) => {
    consume(kind, authority, method)
    try { const response = await mutate(method, params, method === "thread/delete" ? undefined : authority.threadId); record(authority, method, "accepted"); return response }
    catch (error) { record(authority, method, "failed"); throw error }
  }
  const agentStatus = (status: string): CodexHistoryAgent["status"] => {
    if (["pending", "running", "waiting", "interrupted", "completed", "errored", "shutdown", "not_found"].includes(status)) return status as CodexHistoryAgent["status"]
    if (["inProgress", "active"].includes(status)) return "running"
    if (["idle", "notStarted"].includes(status)) return "pending"
    return "unknown"
  }
  const agents = (threads: ReadonlyArray<CodexLifecycleThread>): ReadonlyArray<CodexHistoryAgent> => {
    const children = new Map<string, number>()
    for (const thread of threads) if (thread.parentThreadId !== null) children.set(thread.parentThreadId, (children.get(thread.parentThreadId) ?? 0) + 1)
    const depth = (thread: CodexLifecycleThread): number => {
      let value = thread; let count = 0; const seen = new Set([value.id])
      while (value.parentThreadId !== null && count < 32) { const parent = threads.find(candidate => candidate.id === value.parentThreadId); if (parent === undefined || seen.has(parent.id)) break; seen.add(parent.id); value = parent; count += 1 }
      return count
    }
    return threads.map(thread => ({
      threadRef: thread.id, parentThreadRef: thread.parentThreadId, title: (thread.name ?? "Untitled Codex chat").slice(0, 160), status: agentStatus(thread.status),
      createdAt: thread.createdAt, updatedAt: thread.updatedAt, depth: depth(thread), descendantCount: children.get(thread.id) ?? 0,
      model: thread.model, role: null, nickname: null, agentPath: null, sourceVersion: null, reasoning: null, source: "codex" as const,
    }))
  }
  const itemText = (payload: unknown): Readonly<{ text: string; redacted: boolean }> => {
    const row = object(payload)
    const direct = string(row?.text) ?? string(row?.message) ?? string(row?.summary)
    const parts = array(row?.content).flatMap(part => {
      const value = object(part)
      return string(value?.text) ?? string(value?.inputText) ?? string(value?.outputText) ?? []
    })
    const value = direct ?? parts.join("\n")
    return redactCodexHistoryText(value === "" ? `[${string(row?.type) ?? "item"}]` : value)
  }
  const historyItems = (threadId: string, values: ReadonlyArray<CodexLifecycleItem>): ReadonlyArray<CodexHistoryItem> => values.map((item, sequence) => {
    const text = itemText(item.payload)
    const normalized = item.type.toLowerCase()
    const kind: CodexHistoryItem["kind"] = normalized.includes("user") ? "user_message"
      : normalized.includes("agent") || normalized.includes("assistant") ? "assistant_message"
        : normalized.includes("reason") ? "reasoning"
          : normalized.includes("plan") ? "plan"
            : normalized.includes("error") ? "error"
              : normalized.includes("tool") || normalized.includes("command") || normalized.includes("file") ? "tool_call"
                : "lifecycle"
    return { itemRef: item.id, threadRef: threadId, sequence, timestamp: iso(object(item.payload)?.createdAt), kind, label: item.type.slice(0, 160), summary: text.text, status: item.status, fields: [], redacted: text.redacted, sourceType: item.type.slice(0, 160) }
  })
  const catalog = async (): Promise<CodexHistoryCatalog> => {
    const threads = await list()
    const projected = agents(threads)
    return { roots: projected.filter(agent => agent.parentThreadRef === null), agents: projected }
  }
  const historyPage = async (threadId: string, offset = 0, limit = 100): Promise<CodexHistoryPage | null> => {
    try { await read(threadId, true) } catch { return null }
    const allThreads = state.threads
    const selected = allThreads.find(thread => thread.id === threadId)
    if (selected === undefined) return null
    const allItems = historyItems(threadId, await pageItems(threadId))
    const boundedLimit = Math.max(1, Math.min(500, Math.floor(limit)))
    const boundedOffset = Math.max(0, Math.min(allItems.length, Math.floor(offset)))
    const projectedAgents = agents(allThreads)
    let root = selected
    while (root.parentThreadId !== null) { const parent = allThreads.find(thread => thread.id === root.parentThreadId); if (parent === undefined) break; root = parent }
    const limitations = state.limitations.filter(item => item.threadId === threadId)
    const gaps = limitations.filter(item => item.limitation === "transient_gap").length
    return {
      rootThreadRef: root.id, selectedThreadRef: threadId, agents: projectedAgents, items: allItems.slice(boundedOffset, boundedOffset + boundedLimit),
      offset: boundedOffset, limit: boundedLimit, totalItems: allItems.length, hasPrevious: boundedOffset > 0, hasNext: boundedOffset + boundedLimit < allItems.length,
      completeness: { source: allItems.length + gaps, rendered: allItems.length, redactions: allItems.filter(item => item.redacted).length, gaps, complete: gaps === 0 && !selected.ephemeral },
    }
  }
  const runHistory = async (request: CodexHistoryRequest): Promise<unknown> => {
    if (request.kind === "history_catalog") return catalog()
    if (request.kind === "history_page") return historyPage(request.threadRef, request.offset, request.limit)
    if (request.kind === "history_search") {
      const matches = await (async () => { try { return await (paged("thread/search", { searchTerm: request.query }, "threads", request.limit ?? 40)) } catch { return [] } })()
      const threads = matches.flatMap(raw => projectThread(raw) ?? [])
      const response: CodexHistorySearchResponse = { query: request.query.slice(0, 200), results: threads.slice(0, request.limit ?? 40).map(thread => ({ threadRef: thread.id, rootThreadRef: thread.parentThreadId ?? thread.id, source: "codex", title: (thread.name ?? "Untitled Codex chat").slice(0, 160), matchKind: "title", matchItemRef: null, matchSequence: null, snippet: (thread.name ?? "Untitled Codex chat").slice(0, 240), updatedAt: thread.updatedAt, score: 1 })), indexedSessions: threads.length, truncated: false }
      return response
    }
    if (request.kind === "list") {
      const threads = (await list()).filter(thread => thread.parentThreadId === null).slice(0, request.limit ?? 100)
      return Promise.all(threads.map(async thread => {
        const page = await historyPage(thread.id, 0, 100)
        const notes = (page?.items ?? []).flatMap(item => item.kind === "user_message" || item.kind === "assistant_message" ? [{ key: item.itemRef, role: item.kind === "user_message" ? "user" as const : "assistant" as const, text: item.summary, timestamp: item.timestamp }] : [])
        return { id: thread.id, title: thread.name ?? "Untitled Codex chat", createdAt: thread.createdAt, updatedAt: thread.updatedAt, ...(thread.cwd === null ? {} : { cwd: thread.cwd }), ...(thread.model === null ? {} : { model: thread.model }), notes } satisfies DesktopThread
      }))
    }
    const thread = state.threads.find(value => value.id === request.id) ?? await read(request.id)
    const page = await historyPage(thread.id, 0, request.messageLimit ?? 100)
    if (page === null) return null
    return { id: thread.id, title: thread.name ?? "Untitled Codex chat", createdAt: thread.createdAt, updatedAt: thread.updatedAt, ...(thread.cwd === null ? {} : { cwd: thread.cwd }), ...(thread.model === null ? {} : { model: thread.model }), notes: page.items.flatMap(item => item.kind === "user_message" || item.kind === "assistant_message" ? [{ key: item.itemRef, role: item.kind === "user_message" ? "user" as const : "assistant" as const, text: item.summary, timestamp: item.timestamp }] : []) } satisfies DesktopThread
  }
  const removeNotification = options.lease.subscribe(notification => {
    if (notification.generation > state.generation) void reconcile(notification.generation)
    const params = object(notification.message.params)
    const threadId = string(params?.threadId) ?? idOf(params?.thread)
    if (threadId === null) return
    const method = string(notification.message.method)
    if (method === "thread/deleted" || method === "thread/closed") {
      publish({ threads: state.threads.filter(thread => thread.id !== threadId) }, notification.generation)
    } else if (method?.startsWith("thread/") === true) {
      void read(threadId, true).catch(() => undefined)
    } else if (method?.startsWith("turn/") === true || method?.startsWith("item/") === true) {
      void Promise.all([pageTurns(threadId), pageItems(threadId)]).catch(() => undefined)
    }
  })
  return {
    initialize: async () => { await Promise.all([list(), loaded()]); return state }, snapshot: () => state,
    subscribe: listener => { assertOpen(); listeners.add(listener); return () => listeners.delete(listener) },
    list, search: async (query, params = {}) => { const rows = await paged("thread/search", { ...(object(params) ?? {}), searchTerm: query }, "threads"); const threads = rows.flatMap(raw => projectThread(raw) ?? []); rememberThreads(threads); return threads }, loaded, read, pageTurns, pageItems,
    start: params => mutate("thread/start", params), resume: (threadId, params = {}) => mutate("thread/resume", { ...(object(params) ?? {}), threadId }, threadId), fork: (threadId, params = {}) => mutate("thread/fork", { ...(object(params) ?? {}), threadId }),
    setName: (threadId, name) => mutate("thread/name/set", { threadId, name }, threadId), updateMetadata: (threadId, gitInfo) => mutate("thread/metadata/update", { threadId, gitInfo }, threadId), updateSettings: (threadId, settings) => mutate("thread/settings/update", { threadId, ...settings }, threadId),
    setGoal: (threadId, goal) => mutate("thread/goal/set", { threadId, ...goal }, threadId), clearGoal: threadId => mutate("thread/goal/clear", { threadId }, threadId), setMemoryMode: (threadId, memoryMode) => mutate("thread/memoryMode/set", { threadId, mode: memoryMode }, threadId), compact: threadId => mutate("thread/compact/start", { threadId }, threadId), unsubscribe: threadId => mutate("thread/unsubscribe", { threadId }),
    authorize: (kind, threadId, revision) => { assertOpen(); if (revision !== state.revision) throw new CodexThreadLifecycleError("stale", "Cannot authorize stale lifecycle state"); const authority = { token: randomUUID(), kind, threadId, revision, expiresAt: new Date(now().getTime() + (options.intentTtlMs ?? 60_000)).toISOString() }; authorities.set(authority.token, authority); return authority },
    archive: authority => dangerous("archive", "thread/archive", { threadId: authority.threadId }, authority), unarchive: authority => dangerous("unarchive", "thread/unarchive", { threadId: authority.threadId }, authority), delete: authority => dangerous("delete", "thread/delete", { threadId: authority.threadId }, authority), rollback: (numTurns, authority) => dangerous("rollback", "thread/rollback", { threadId: authority.threadId, numTurns }, authority), injectItems: (items, authority) => dangerous("inject_items", "thread/inject_items", { threadId: authority.threadId, items }, authority), approveGuardianDeniedAction: (event, authority) => dangerous("guardian_approval", "thread/approveGuardianDeniedAction", { threadId: authority.threadId, event }, authority),
    runHistory,
    receipts: () => [...receipts],
    close: () => { if (closed) return; closed = true; removeNotification(); for (const remove of visible.values()) remove(); visible.clear(); authorities.clear(); listeners.clear(); options.lease.release() },
  }
}

export type CodexThreadLifecycleRegistry = Readonly<{ forTarget: (target: CodexAppServerPoolTarget) => Promise<CodexThreadLifecycle>; close: () => void }>
export const makeCodexThreadLifecycleRegistry = (options: Readonly<{ supervisor: CodexAppServerSupervisor; receiptRoot: string }>): CodexThreadLifecycleRegistry => {
  const entries = new Map<string, Promise<CodexThreadLifecycle>>()
  let closed = false
  return {
    forTarget: target => {
      if (closed) return Promise.reject(new CodexThreadLifecycleError("closed", "Lifecycle registry is closed"))
      const key = codexAppServerPoolKey(target); const existing = entries.get(key); if (existing !== undefined) return existing
      const created = options.supervisor.acquire(target).then(async lease => { const lifecycle = makeCodexThreadLifecycle({ lease, receiptPath: join(options.receiptRoot, `${createHash("sha256").update(key).digest("hex")}.json`) }); try { await lifecycle.initialize(); return lifecycle } catch (error) { lifecycle.close(); entries.delete(key); throw error } })
      entries.set(key, created); return created
    },
    close: () => { if (closed) return; closed = true; for (const entry of entries.values()) void entry.then(value => value.close(), () => undefined); entries.clear() },
  }
}
