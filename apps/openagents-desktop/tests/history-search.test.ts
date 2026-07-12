import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, utimesSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeCodexHistoryCatalog, decodeCodexHistorySearchResponse } from "../src/codex-history-contract.ts"
import { buildHistorySearchDocuments, buildMergedHistoryGraphs, readMergedHistoryCatalog, readMergedHistoryPage, searchMergedHistory } from "../src/merged-history.ts"
import { searchHistoryDocuments, type HistorySearchDocument } from "../src/history-search.ts"

// --- Codex fixture (sessions root) -----------------------------------------
const codexRootDir = () => { const value = path.join(mkdtempSync(path.join(tmpdir(), "oa-mix-cx-")), "sessions"); mkdirSync(value, { recursive: true }); return value }
const stamp = (file: string, at: string): void => { const date = new Date(at); utimesSync(file, date, date) }
const codexSession = (root: string, id: string, at: string, title: string, body: string) => {
  const dir = path.join(root, "2026", "07", "10"); mkdirSync(dir, { recursive: true })
  const file = path.join(dir, `${id}.jsonl`)
  writeFileSync(file, [
    JSON.stringify({ timestamp: at, type: "session_meta", payload: { id, source: "cli" } }),
    JSON.stringify({ timestamp: at, type: "response_item", payload: { type: "message", role: "user", content: [{ type: "input_text", text: title }] } }),
    JSON.stringify({ timestamp: at, type: "response_item", payload: { type: "message", role: "assistant", content: [{ type: "output_text", text: body }] } }),
  ].join("\n") + "\n")
  stamp(file, at)
}

// --- Claude fixture (projects root) ----------------------------------------
const claudeRootDir = () => { const value = path.join(mkdtempSync(path.join(tmpdir(), "oa-mix-cl-")), "projects"); mkdirSync(value, { recursive: true }); return value }
const claudeSession = (root: string, id: string, at: string, title: string, body: string) => {
  const dir = path.join(root, "proj"); mkdirSync(dir, { recursive: true })
  const file = path.join(dir, `${id}.jsonl`)
  writeFileSync(file, [
    JSON.stringify({ type: "user", uuid: "u1", parentUuid: null, timestamp: at, sessionId: id, message: { role: "user", content: title } }),
    JSON.stringify({ type: "assistant", uuid: "a1", parentUuid: "u1", timestamp: at, message: { role: "assistant", content: [{ type: "text", text: body }] } }),
  ].join("\n") + "\n")
  stamp(file, at)
}

const buildMix = () => {
  const codex = codexRootDir(); const claude = claudeRootDir()
  codexSession(codex, "cx0001", "2026-07-10T09:00:00.000Z", "Ship the codex parser", "inspect the quantum kernel")
  claudeSession(claude, "cl0001", "2026-07-10T11:00:00.000Z", "Ship the claude sidebar", "trace the photon mapping")
  return { codex, claude }
}

describe("merged codex + claude catalog (#8712 H3)", () => {
  test("shows both sources tagged, sorted by recency, and routes pages by ref", () => {
    const { codex, claude } = buildMix()
    const catalog = readMergedHistoryCatalog(codex, claude)
    expect(catalog.roots.map(root => root.source)).toEqual(["claude", "codex"]) // claude newer first
    expect(catalog.roots.map(root => root.threadRef)).toEqual(["claude:cl0001", "cx0001"])
    expect(decodeCodexHistoryCatalog(catalog)).not.toBeNull()
    // Page routing: claude-namespaced ref -> claude reader; bare ref -> codex.
    expect(readMergedHistoryPage({ codexRoot: codex, claudeRoot: claude, threadRef: "claude:cl0001" })!.items[0]?.kind).toBe("user_message")
    expect(readMergedHistoryPage({ codexRoot: codex, claudeRoot: claude, threadRef: "cx0001" })!.items[0]?.kind).toBe("session")
    expect(readMergedHistoryPage({ codexRoot: codex, claudeRoot: claude, threadRef: "claude:missing" })).toBeNull()
  })

  test("works when one source is empty (additive, never blocks the other)", () => {
    const { codex } = buildMix()
    const catalog = readMergedHistoryCatalog(codex, null)
    expect(catalog.roots.every(root => root.source === "codex")).toBe(true)
    expect(catalog.roots.length).toBe(1)
  })
})

describe("free-text history search (#8712 H4)", () => {
  test("title match, content match with open-at-item, and cross-source ranking", () => {
    const { codex, claude } = buildMix()
    const graphs = buildMergedHistoryGraphs(codex, claude)
    const index = buildHistorySearchDocuments(codex, claude, graphs)

    const titleHit = searchMergedHistory({ query: "sidebar" }, index)
    expect(titleHit.results).toHaveLength(1)
    expect(titleHit.results[0]).toMatchObject({ threadRef: "claude:cl0001", matchKind: "title", matchItemRef: null })
    expect(decodeCodexHistorySearchResponse(titleHit)).not.toBeNull()

    const contentHit = searchMergedHistory({ query: "quantum" }, index)
    expect(contentHit.results).toHaveLength(1)
    expect(contentHit.results[0]).toMatchObject({ threadRef: "cx0001", source: "codex", matchKind: "content" })
    // Open-at-item: the result names the exact matching item to window on.
    expect(contentHit.results[0]?.matchItemRef).toBe("cx0001:2")
    expect(contentHit.results[0]?.matchSequence).toBe(2)
    expect(contentHit.results[0]?.snippet).toContain("quantum")

    const claudeContent = searchMergedHistory({ query: "photon" }, index)
    expect(claudeContent.results[0]).toMatchObject({ threadRef: "claude:cl0001", matchKind: "content" })
  })

  test("empty query and no-match return no results; index tracks bounds", () => {
    const { codex, claude } = buildMix()
    const index = buildHistorySearchDocuments(codex, claude, buildMergedHistoryGraphs(codex, claude))
    expect(searchMergedHistory({ query: "" }, index).results).toEqual([])
    expect(searchMergedHistory({ query: "nonexistent-token-xyz" }, index).results).toEqual([])
    expect(index.indexedSessions).toBe(2)
    expect(index.truncated).toBe(false)
  })

  test("index is rebuildable — a fresh build yields identical ranked results", () => {
    const { codex, claude } = buildMix()
    const first = searchMergedHistory({ query: "ship" }, buildHistorySearchDocuments(codex, claude, buildMergedHistoryGraphs(codex, claude)))
    const second = searchMergedHistory({ query: "ship" }, buildHistorySearchDocuments(codex, claude, buildMergedHistoryGraphs(codex, claude)))
    expect(second.results).toEqual(first.results)
    // Both title-match; ranking is recency-ordered (claude newer first).
    expect(first.results.map(result => result.threadRef)).toEqual(["claude:cl0001", "cx0001"])
  })

  test("pure ranking: titles outrank content, recency breaks ties", () => {
    const docs: HistorySearchDocument[] = [
      { threadRef: "a", rootThreadRef: "a", source: "codex", title: "old alpha report", updatedAt: "2026-01-01T00:00:00.000Z", items: [] },
      { threadRef: "b", rootThreadRef: "b", source: "claude", title: "new alpha report", updatedAt: "2026-07-01T00:00:00.000Z", items: [] },
      { threadRef: "c", rootThreadRef: "c", source: "codex", title: "unrelated", updatedAt: "2026-07-10T00:00:00.000Z", items: [{ itemRef: "c:5", sequence: 5, text: "buried alpha mention" }] },
    ]
    const results = searchHistoryDocuments(docs, "alpha")
    expect(results.map(result => result.threadRef)).toEqual(["b", "a", "c"]) // titles (recency) then content
    expect(results[2]).toMatchObject({ matchKind: "content", matchItemRef: "c:5", matchSequence: 5 })
  })
})
