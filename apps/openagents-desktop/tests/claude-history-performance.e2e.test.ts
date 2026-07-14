import { expect, test } from "vite-plus/test"
import { appendFileSync, mkdirSync, mkdtempSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { buildClaudeHistoryGraph, claudeHistoryTopology, readClaudeHistoryCatalog, readClaudeHistoryPage } from "../src/claude-history.ts"

// #8712 H3 loss-accounting oracle at 100+MB / many-children scale (like #8674).
test("valid 100 MiB / 100-child / 100k-record Claude history stays bounded and loss-accounted", () => {
  const projects = mkdtempSync(path.join(tmpdir(), "oa-claude-scale-"))
  const project = path.join(projects, "proj"); mkdirSync(project, { recursive: true })
  const sessionId = "session-scale-0001"
  const rootFile = path.join(project, `${sessionId}.jsonl`)
  writeFileSync(rootFile, JSON.stringify({ type: "user", uuid: "u0", parentUuid: null, timestamp: "2026-07-10T00:00:00.000Z", sessionId, message: { role: "user", content: "Kick off the 100-child scale run" } }) + "\n")
  const padding = "x".repeat(1040)
  // 100k assistant records (~100 MiB) interleaved with 100 Agent spawn edges.
  for (let base = 0; base < 100_000; base += 1000) {
    let chunk = ""
    for (let i = base; i < base + 1000; i++) {
      chunk += JSON.stringify({ type: "assistant", uuid: `a${i}`, parentUuid: `a${i - 1}`, timestamp: "2026-07-10T00:00:01.000Z", message: { role: "assistant", model: "claude-opus-4-8", content: [{ type: "text", text: `${i}:${padding}` }] } }) + "\n"
      if (i % 1000 === 0 && i < 100_000) {
        const child = i / 1000
        chunk += JSON.stringify({ type: "user", uuid: `t${i}`, parentUuid: `a${i}`, timestamp: "2026-07-10T00:00:02.000Z", message: { role: "user", content: [{ type: "tool_result", tool_use_id: `tool${child}` }] }, toolUseResult: { agentId: `child-${child}`, tool_use_id: `tool${child}`, status: "completed" } }) + "\n"
      }
    }
    appendFileSync(rootFile, chunk)
  }
  const subagents = path.join(project, sessionId, "subagents"); mkdirSync(subagents, { recursive: true })
  for (let i = 0; i < 100; i++) writeFileSync(path.join(subagents, `agent-child-${i}.jsonl`), JSON.stringify({ type: "user", uuid: "cu", parentUuid: null, timestamp: "2026-07-10T00:00:03.000Z", isSidechain: true, sessionId, message: { role: "user", content: `child ${i}` } }) + "\n")
  expect(statSync(rootFile).size).toBeGreaterThanOrEqual(100 * 1024 * 1024)

  const graphStarted = performance.now()
  const graph = buildClaudeHistoryGraph(projects)
  const graphMs = performance.now() - graphStarted
  const catalog = readClaudeHistoryCatalog(projects, graph)
  // Every child linked through its structured Agent edge; no orphans; the root
  // owns all 100.
  expect(catalog.roots[0]?.descendantCount).toBe(100)
  expect(claudeHistoryTopology(graph)).toEqual({ childFiles: 100, linked: 100, orphans: 0 })
  expect(graphMs).toBeLessThan(12_000)

  const pageStarted = performance.now()
  const page = readClaudeHistoryPage({ projectsRoot: projects, threadRef: "claude:" + sessionId, offset: 0, limit: 200 }, graph)!
  const pageMs = performance.now() - pageStarted
  expect(page.totalItems).toBe(100_101) // 1 user + 100k assistant + 100 tool_result
  expect(page.items).toHaveLength(200)
  // Whole-conversation completeness holds even though only a window rendered.
  expect(page.completeness.source).toBe(page.completeness.rendered + page.completeness.redactions + page.completeness.gaps)
  expect(page.completeness.source).toBe(100_101)
  expect(pageMs).toBeLessThan(15_000)
}, 120_000)
