/**
 * Loss-accounted history catalog at REAL ~/.codex scale (#8789, rc.10 owner
 * incident: "That says coding history all time, but it only has five chats,
 * so that's definitely not all time.").
 *
 * Root cause (verified against the owner's actual 20 GB / ~1,500-rollout
 * store): the catalog graph build read WHOLE rollout files with readFileSync
 * to derive display titles; a 4.5 GB rollout ENOMEMed the read, the whole
 * catalog threw, and the sidebar silently fell back to the 24-hour recent
 * list — five chats under an "all time" header. These oracles pin the bounded
 * behavior that replaces it: catalogs survive oversized sessions, page reads
 * stream with bounded memory but whole-conversation accounting, the search
 * index reads bounded heads, and a >page-size multi-workspace store keeps
 * every session countable (no silent truncation).
 */
import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, utimesSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeCodexHistoryCatalog, decodeCodexHistoryPage } from "../src/codex-history-contract.ts"
import { buildCodexHistoryGraph, readCodexHistoryCatalog, readCodexHistoryHeadItems, readCodexHistoryPage } from "../src/codex-history.ts"
import { buildHistorySearchDocuments, buildMergedHistoryGraphs, searchMergedHistory } from "../src/merged-history.ts"
import { historyCatalogPageSize } from "../src/renderer/history-workspace.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

const sessionsRoot = () => {
  const value = path.join(mkdtempSync(path.join(tmpdir(), "oa-scale-cx-")), "sessions")
  mkdirSync(value, { recursive: true })
  return value
}
const stamp = (file: string, at: string): void => { const date = new Date(at); utimesSync(file, date, date) }
const writeSession = (root: string, id: string, at: string, rows: unknown[]): string => {
  const dir = path.join(root, "2026", "07", "10")
  mkdirSync(dir, { recursive: true })
  const file = path.join(dir, `${id}.jsonl`)
  writeFileSync(file, `${rows.map(row => JSON.stringify(row)).join("\n")}\n`)
  stamp(file, at)
  return file
}
const meta = (id: string, at: string, extra: Record<string, unknown> = {}) => ({ timestamp: at, type: "session_meta", payload: { id, source: "cli", ...extra } })
const userMessage = (at: string, text: string) => ({ timestamp: at, type: "response_item", payload: { type: "message", role: "user", content: [{ type: "input_text", text }] } })
const assistantMessage = (at: string, text: string) => ({ timestamp: at, type: "response_item", payload: { type: "message", role: "assistant", content: [{ type: "output_text", text }] } })

describe("bounded catalog against oversized rollouts (#8789)", () => {
  test("registry records the enforced truthful-header contract", () => {
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      (contract) => contract.contractId === "openagents_desktop.history.sidebar_header_truthful_scope.v1",
    )?.state).toBe("enforced")
  })

  test(">page-size multi-workspace store: every root catalogued, recency-ordered, nothing silently truncated", () => {
    const root = sessionsRoot()
    const total = historyCatalogPageSize + 5 // 45 — beyond one sidebar page
    for (let index = 0; index < total; index++) {
      const at = `2026-07-10T${String(8 + Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}:00.000Z`
      writeSession(root, `sess-${String(index).padStart(3, "0")}`, at, [
        meta(`sess-${String(index).padStart(3, "0")}`, at, { cwd: `/work/workspace-${index % 7}` }),
        userMessage(at, `Session ${index} objective`),
      ])
    }
    // A child session must never pollute the root catalog — but is never lost.
    writeSession(root, "sess-child", "2026-07-10T09:30:00.000Z", [
      meta("sess-child", "2026-07-10T09:30:00.000Z", { parent_thread_id: "sess-000", cwd: "/work/workspace-0" }),
      userMessage("2026-07-10T09:30:01.000Z", "child work"),
    ])
    const catalog = readCodexHistoryCatalog(root)
    expect(catalog.roots).toHaveLength(total)
    expect(catalog.agents).toHaveLength(total + 1)
    expect(catalog.roots.map(item => item.threadRef)).not.toContain("sess-child")
    // Recent-first, no age ceiling: the newest root leads, the oldest is present.
    expect(catalog.roots[0]?.threadRef).toBe(`sess-${String(total - 1).padStart(3, "0")}`)
    expect(catalog.roots.at(-1)?.threadRef).toBe("sess-000")
    expect(decodeCodexHistoryCatalog(catalog)).not.toBeNull()
  })

  test("a session whose authored title lies beyond the bounded head scan degrades to a fallback title — and cannot take down the catalog", () => {
    const root = sessionsRoot()
    // ~9 MB of non-authored preamble pushes the authored message past the
    // 8 MB title cap. Before the fix this file was read WHOLE via
    // readFileSync (ENOMEM at real scale); now the scan is byte-bounded.
    const filler = "x".repeat(64 * 1024)
    const preamble = Array.from({ length: 144 }, (_, index) => assistantMessage("2026-07-10T10:00:01.000Z", `${index} ${filler}`))
    writeSession(root, "sess-giant", "2026-07-10T10:00:00.000Z", [
      meta("sess-giant", "2026-07-10T10:00:00.000Z", { cwd: "/work/giant" }),
      ...preamble,
      userMessage("2026-07-10T10:05:00.000Z", "The buried authored title"),
    ])
    writeSession(root, "sess-normal", "2026-07-10T11:00:00.000Z", [
      meta("sess-normal", "2026-07-10T11:00:00.000Z", { cwd: "/work/normal" }),
      userMessage("2026-07-10T11:00:01.000Z", "Ordinary session"),
    ])
    const catalog = readCodexHistoryCatalog(root)
    expect(catalog.roots).toHaveLength(2)
    expect(catalog.roots.find(item => item.threadRef === "sess-normal")?.title).toBe("Ordinary session")
    expect(catalog.roots.find(item => item.threadRef === "sess-giant")?.title).toBe("Untitled Codex chat")
  })

  test("streaming page read keeps whole-conversation totals and accounting while returning only the requested window", () => {
    const root = sessionsRoot()
    const rows: unknown[] = [meta("sess-a", "2026-07-10T10:00:00.000Z", { cwd: "/work/a" })]
    for (let index = 0; index < 120; index++) rows.push(userMessage("2026-07-10T10:01:00.000Z", `message ${index}`))
    writeSession(root, "sess-a", "2026-07-10T10:02:00.000Z", rows)
    const graph = buildCodexHistoryGraph(root)
    const page = readCodexHistoryPage({ sessionsRoot: root, threadRef: "sess-a", offset: 50, limit: 25 }, graph)!
    expect(page.items).toHaveLength(25)
    expect(page.items[0]?.summary).toBe("message 49") // sequence 50 = 49th message after session_meta
    expect(page.offset).toBe(50)
    expect(page.totalItems).toBe(121)
    expect(page.hasPrevious).toBe(true)
    expect(page.hasNext).toBe(true)
    expect(page.completeness).toMatchObject({ source: 121, gaps: 0, redactions: 0 })
    expect(decodeCodexHistoryPage(page)).not.toBeNull()
    // Beyond-end offsets clamp honestly to an empty window.
    const beyond = readCodexHistoryPage({ sessionsRoot: root, threadRef: "sess-a", offset: 10_000, limit: 25 }, graph)!
    expect(beyond.items).toHaveLength(0)
    expect(beyond.totalItems).toBe(121)
    expect(beyond.offset).toBe(121)
  })

  test("search-index head reader is item- and byte-bounded, never a whole-file read", () => {
    const root = sessionsRoot()
    const rows: unknown[] = [meta("sess-b", "2026-07-10T10:00:00.000Z", { cwd: "/work/b" })]
    for (let index = 0; index < 500; index++) rows.push(userMessage("2026-07-10T10:01:00.000Z", `entry ${index}`))
    const file = writeSession(root, "sess-b", "2026-07-10T10:02:00.000Z", rows)
    expect(readCodexHistoryHeadItems(file, "sess-b", 10)).toHaveLength(10)
    // A tiny byte cap stops the scan long before 500 rows.
    expect(readCodexHistoryHeadItems(file, "sess-b", 500, 2_048).length).toBeLessThan(50)
  })

  test("search matches the workspace label alongside titles (#8788)", () => {
    const root = sessionsRoot()
    writeSession(root, "sess-ws", "2026-07-10T10:00:00.000Z", [
      meta("sess-ws", "2026-07-10T10:00:00.000Z", { cwd: "/Users/owner/work/peregrine-lab" }),
      userMessage("2026-07-10T10:00:01.000Z", "Unrelated title"),
    ])
    const graphs = buildMergedHistoryGraphs(root, null)
    const index = buildHistorySearchDocuments(root, null, graphs)
    expect(index.documents[0]?.workspaceLabel).toBe("peregrine-lab")
    const response = searchMergedHistory({ query: "peregrine-la" }, index)
    expect(response.results).toHaveLength(1)
    expect(response.results[0]).toMatchObject({ threadRef: "sess-ws", matchKind: "title" })
  })
})
