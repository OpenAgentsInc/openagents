import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { makeCodexThreadLifecycle } from "./codex-thread-lifecycle.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

const fakeLease = (options: Readonly<{ experimentalPagination?: boolean; extraItems?: ReadonlyArray<Record<string, unknown>>; unnamedPreview?: string }> = {}) => {
  let generation = 1
  const requests: Array<{ method: string; params: unknown }> = []
  const listeners = new Set<(notification: CodexAppServerNotification) => void>()
  const visible = new Set<string>()
  const threads = new Map<string, Record<string, unknown>>([
    ["thread-1", { id: "thread-1", name: options.unnamedPreview === undefined ? "One" : null, preview: options.unnamedPreview ?? "First prompt", status: { type: "idle" }, createdAt: 1_700_000_000, updatedAt: 1_700_000_001, ephemeral: false, parentThreadId: null, goal: { text: "ship" }, settings: { mode: "pair" }, memoryMode: "auto", metadata: { private: "memory-only" }, turns: [
      { id: "turn-1", status: "completed", items: [{ id: "item-1", turnId: "turn-1", type: "userMessage", status: "completed" }] },
      { id: "turn-2", status: "inProgress", items: [{ id: "item-2", turnId: "turn-2", type: "agentMessage", status: "inProgress" }, ...(options.extraItems ?? [])] },
    ] }],
    ["thread-2", { id: "thread-2", name: "Ephemeral", status: { type: "idle" }, createdAt: 1_700_000_002, updatedAt: 1_700_000_003, ephemeral: true, parentThreadId: "thread-1", turns: [] }],
  ])
  const response = (method: string, params: unknown): unknown => {
    const p = params as Record<string, unknown>
    const threadId = typeof p.threadId === "string" ? p.threadId : "thread-1"
    const thread = threads.get(threadId)
    switch (method) {
      case "thread/list": return p.cursor === "page-2" ? { data: [threads.get("thread-2")], nextCursor: null } : { data: [threads.get("thread-1"), threads.get("thread-1")], nextCursor: "page-2" }
      case "thread/search": return { data: [...threads.values()].filter(value => String(value.name).toLowerCase().includes(String(p.searchTerm).toLowerCase())), nextCursor: null }
      case "thread/loaded/list": return { data: [...threads.values()] }
      case "thread/read": return { thread }
      case "thread/turns/list": {
        if (options.experimentalPagination === false) throw new Error("experimental method unavailable")
        const turns = (thread?.turns ?? []) as unknown[]
        return p.cursor === "turn-page-2" ? { data: turns.slice(1), nextCursor: null } : { data: [turns[0], turns[0]].filter(Boolean), nextCursor: turns.length > 1 ? "turn-page-2" : null }
      }
      case "thread/items/list": {
        if (options.experimentalPagination === false) throw new Error("experimental method unavailable")
        const items = ((thread?.turns ?? []) as Array<Record<string, unknown>>).flatMap(turn => turn.items as unknown[])
        return p.cursor === "item-page-2" ? { data: items.slice(1), nextCursor: null } : { data: [items[0], items[0]].filter(Boolean), nextCursor: items.length > 1 ? "item-page-2" : null }
      }
      case "thread/start": threads.set("thread-3", { id: "thread-3", name: "Three", status: { type: "idle" }, createdAt: 1_700_000_004, updatedAt: 1_700_000_004, ephemeral: false, turns: [] }); return { thread: threads.get("thread-3") }
      case "thread/fork": threads.set("thread-fork", { ...(thread ?? {}), id: "thread-fork", parentThreadId: threadId }); return { thread: threads.get("thread-fork") }
      case "thread/name/set": if (thread) thread.name = p.name; return {}
      case "thread/metadata/update": if (thread) thread.metadata = p.gitInfo; return {}
      case "thread/settings/update": if (thread) thread.settings = { model: p.model, effort: p.effort }; return {}
      case "thread/goal/set": if (thread) thread.goal = { objective: p.objective, status: p.status }; return {}
      case "thread/goal/clear": if (thread) thread.goal = null; return {}
      case "thread/memoryMode/set": if (thread) thread.memoryMode = p.mode; return {}
      case "thread/delete": threads.delete(threadId); return {}
      default: return {}
    }
  }
  const lease = {
    key: "lifecycle", identity: { binary: "/codex", binarySha256: "hash", codexHome: null, accountRef: "account", hostTarget: "desktop" },
    state: () => ({ status: "ready", generation }) as const,
    request: async (method: string, params: unknown) => { requests.push({ method, params }); return response(method, params) },
    notify: async () => undefined,
    subscribe: (listener: (notification: CodexAppServerNotification) => void) => { listeners.add(listener); return () => listeners.delete(listener) },
    subscribeCompatibility: () => () => undefined, nativeEnvelopes: () => [], compatibilityReceipts: () => [], nativeJournal: () => [],
    registerVisibleThread: (threadId: string) => { visible.add(threadId); return () => visible.delete(threadId) },
    registerReverseHandler: () => () => undefined, release: () => undefined,
  } as unknown as CodexAppServerLease
  return { lease, requests, visible, setGeneration: (value: number) => { generation = value }, emit: (method: string, params: unknown) => { for (const listener of listeners) listener({ generation, message: { method, params } }) } }
}

describe("Codex app-server thread lifecycle", () => {
  test("pages without gaps/duplicates and exposes ephemeral limitations", async () => {
    const fake = fakeLease()
    const lifecycle = makeCodexThreadLifecycle({ lease: fake.lease })
    const snapshot = await lifecycle.initialize()
    expect(snapshot.threads.map(thread => thread.id)).toEqual(["thread-1", "thread-2"])
    expect(snapshot.threads.find(thread => thread.id === "thread-2")?.limitations).toContain("ephemeral_history")
    await lifecycle.read("thread-1")
    await lifecycle.read("thread-2")
    expect(fake.visible).toEqual(new Set(["thread-1"]))
    expect((await lifecycle.pageTurns("thread-1")).map(turn => (turn as { id: string }).id)).toEqual(["turn-1", "turn-2"])
    expect((await lifecycle.pageItems("thread-1")).map(item => item.id)).toEqual(["item-1", "item-2"])
    const catalog = await lifecycle.runHistory({ kind: "history_catalog", sessionsRoot: "/unused" }) as { roots: Array<{ threadRef: string }>; agents: unknown[] }
    expect(catalog.roots.map(root => root.threadRef)).toEqual(["thread-1"])
    const page = await lifecycle.runHistory({ kind: "history_page", sessionsRoot: "/unused", threadRef: "thread-1", offset: 0, limit: 1 }) as { items: unknown[]; totalItems: number; hasNext: boolean }
    expect(page).toMatchObject({ totalItems: 2, hasNext: true })
    expect(page.items).toHaveLength(1)
    const search = await lifecycle.runHistory({ kind: "history_search", sessionsRoot: "/unused", query: "one", limit: 10 }) as { results: Array<{ threadRef: string }> }
    expect(search.results.map(result => result.threadRef)).toEqual(["thread-1"])
    const recent = await lifecycle.runHistory({ kind: "list", sessionsRoot: "/unused", limit: 1 }) as Array<{ id: string; notes: unknown[] }>
    expect(recent[0]).toMatchObject({ id: "thread-1" })
    expect(recent[0]?.notes).toHaveLength(2)
    lifecycle.close()
  })

  test("uses the app-server first-user preview when a native thread name is absent", async () => {
    const fake = fakeLease({ unnamedPreview: "  Diagnose   untitled Codex history  " })
    const lifecycle = makeCodexThreadLifecycle({ lease: fake.lease })
    await lifecycle.initialize()
    const catalog = await lifecycle.runHistory({
      kind: "history_catalog",
      sessionsRoot: "/unused",
    }) as { roots: Array<{ title: string }> }
    expect(catalog.roots[0]?.title).toBe("Diagnose untitled Codex history")
    const recent = await lifecycle.runHistory({
      kind: "list",
      sessionsRoot: "/unused",
      limit: 1,
    }) as Array<{ title: string }>
    expect(recent[0]?.title).toBe("Diagnose untitled Codex history")
    lifecycle.close()
  })

  test("round-trips lifecycle controls and receipts every dangerous mutation", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-lifecycle-")); roots.push(root)
    const receiptPath = join(root, "receipts.json")
    const fake = fakeLease()
    const lifecycle = makeCodexThreadLifecycle({ lease: fake.lease, receiptPath })
    await lifecycle.initialize()
    await lifecycle.start({ model: "gpt-5.6-sol", cwd: "/workspace" })
    await lifecycle.resume("thread-1")
    await lifecycle.fork("thread-1")
    await lifecycle.setName("thread-1", "Renamed")
    await lifecycle.updateMetadata("thread-1", { owner: true })
    await lifecycle.updateSettings("thread-1", { model: "gpt-5.6-sol", effort: "medium" })
    await lifecycle.setGoal("thread-1", { objective: "done", status: "active" })
    await lifecycle.clearGoal("thread-1")
    await lifecycle.setMemoryMode("thread-1", "disabled")
    await lifecycle.compact("thread-1")
    await lifecycle.unsubscribe("thread-1")
    const dangerous = async (kind: Parameters<typeof lifecycle.authorize>[0], action: (authority: ReturnType<typeof lifecycle.authorize>) => Promise<unknown>) => action(lifecycle.authorize(kind, "thread-1", lifecycle.snapshot().revision))
    await dangerous("archive", authority => lifecycle.archive(authority))
    await dangerous("unarchive", authority => lifecycle.unarchive(authority))
    await dangerous("rollback", authority => lifecycle.rollback(1, authority))
    await dangerous("inject_items", authority => lifecycle.injectItems([{ type: "text", text: "private" }], authority))
    await dangerous("guardian_approval", authority => lifecycle.approveGuardianDeniedAction("action-1", authority))
    await dangerous("delete", authority => lifecycle.delete(authority))
    expect(lifecycle.receipts()).toHaveLength(6)
    const disk = readFileSync(receiptPath, "utf8")
    expect(disk).not.toContain("thread-1")
    expect(disk).not.toContain("private")
    lifecycle.close()
  })

  test("projects subagent activity through the typed delegated-agent component contract", async () => {
    const fake = fakeLease({ extraItems: [{
      id: "subagent-activity-1", turnId: "turn-2", type: "subAgentActivity",
      agentPath: "reviewer", agentThreadId: "child-thread-1", kind: "interacted",
    }] })
    const lifecycle = makeCodexThreadLifecycle({ lease: fake.lease })
    await lifecycle.initialize()
    const page = await lifecycle.runHistory({
      kind: "history_page", sessionsRoot: "/unused", threadRef: "thread-1", offset: 0, limit: 10,
    }) as { items: Array<Record<string, unknown>> }
    expect(page.items.find(item => item.itemRef === "subagent-activity-1")).toMatchObject({
      kind: "collaboration",
      item: {
        kind: "agent", source: "codex", activityKind: "interacted", agentPath: "reviewer",
        children: [{ threadRef: "child-thread-1", status: "running" }],
      },
    })
    lifecycle.close()
  })

  test("repairs durable state on reconnect and labels unreplayable gaps", async () => {
    const fake = fakeLease({ experimentalPagination: false })
    const lifecycle = makeCodexThreadLifecycle({ lease: fake.lease })
    await lifecycle.initialize()
    await lifecycle.read("thread-1")
    expect(await lifecycle.pageItems("thread-1")).toHaveLength(2)
    expect(lifecycle.snapshot().limitations.some(value => value.limitation === "experimental_pagination_unavailable")).toBe(true)
    fake.setGeneration(2)
    fake.emit("thread/status/changed", { threadId: "thread-1" })
    await new Promise(resolve => setTimeout(resolve, 20))
    expect(lifecycle.snapshot().generation).toBe(2)
    expect(lifecycle.snapshot().repairState).toBe("current")
    expect(lifecycle.snapshot().limitations).toContainEqual(expect.objectContaining({ threadId: "thread-1", limitation: "transient_gap" }))
    lifecycle.close()
  })
})
