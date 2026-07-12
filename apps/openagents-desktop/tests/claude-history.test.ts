import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeCodexHistoryCatalog, decodeCodexHistoryPage } from "../src/codex-history-contract.ts"
import { buildClaudeHistoryGraph, claudeHistoryTopology, readClaudeHistoryCatalog, readClaudeHistoryPage } from "../src/claude-history.ts"

// --- Synthetic ~/.claude/projects corpus builder ---------------------------
const projectsRoot = () => { const value = path.join(mkdtempSync(path.join(tmpdir(), "oa-claude-")), "projects"); mkdirSync(value, { recursive: true }); return value }
const line = (row: unknown): string => JSON.stringify(row)
const writeParent = (root: string, project: string, sessionId: string, rows: unknown[]): void => {
  const dir = path.join(root, project); mkdirSync(dir, { recursive: true })
  writeFileSync(path.join(dir, `${sessionId}.jsonl`), rows.map(line).join("\n") + "\n")
}
const writeChild = (root: string, project: string, sessionId: string, agentId: string, rows: unknown[], workflow?: string): void => {
  const dir = workflow === undefined ? path.join(root, project, sessionId, "subagents") : path.join(root, project, sessionId, "subagents", "workflows", workflow)
  mkdirSync(dir, { recursive: true })
  writeFileSync(path.join(dir, `agent-${agentId}.jsonl`), rows.map(line).join("\n") + "\n")
}
const user = (uuid: string, parent: string | null, content: unknown, extra: Record<string, unknown> = {}) => ({ type: "user", uuid, parentUuid: parent, timestamp: "2026-07-10T00:00:00.000Z", sessionId: "S", cwd: "/safe/repo", gitBranch: "main", version: "2.1.200", message: { role: "user", content }, ...extra })
const assistant = (uuid: string, parent: string | null, content: unknown[]) => ({ type: "assistant", uuid, parentUuid: parent, timestamp: "2026-07-10T00:00:01.000Z", message: { role: "assistant", model: "claude-opus-4-8", content } })

describe("claude history import (#8712 H3)", () => {
  test("reconstructs the Agent-edge graph, tags source, and namespaces refs", () => {
    const root = projectsRoot()
    writeParent(root, "proj", "S", [
      user("u1", null, "Ship the Claude sidebar"),
      assistant("a1", "u1", [{ type: "text", text: "On it" }, { type: "tool_use", id: "tool1", name: "Agent", input: { description: "roadmap audit", subagent_type: "general-purpose", prompt: "audit" } }]),
      user("u2", "a1", [{ type: "tool_result", tool_use_id: "tool1", content: "done" }], { toolUseResult: { agentId: "child1", status: "completed", totalToolUseCount: 4 } }),
    ])
    writeChild(root, "proj", "S", "child1", [
      user("cu1", null, "audit the roadmap", { isSidechain: true }),
      assistant("ca1", "cu1", [{ type: "thinking", thinking: "private plan" }, { type: "text", text: "Roadmap looks good" }]),
    ])
    const graph = buildClaudeHistoryGraph(root)
    const catalog = readClaudeHistoryCatalog(root, graph)
    expect(catalog.roots).toHaveLength(1)
    expect(catalog.roots[0]).toMatchObject({ threadRef: "claude:S", title: "Ship the Claude sidebar", source: "claude", descendantCount: 1 })
    const child = catalog.agents.find(agent => agent.threadRef === "claude:child1")!
    expect(child).toMatchObject({ parentThreadRef: "claude:S", source: "claude", depth: 1, role: "subagent" })
    expect(child.orphan).toBeUndefined()
    expect(decodeCodexHistoryCatalog(catalog)).not.toBeNull()
  })

  test("projects rich items, redacts credentials, and links the subagent preview", () => {
    const root = projectsRoot()
    writeParent(root, "proj", "S", [
      user("u1", null, "Start"),
      assistant("a1", "u1", [{ type: "text", text: "Reading files" }]),
      assistant("a2", "a1", [{ type: "tool_use", id: "b1", name: "Bash", input: { command: "curl -H 'Authorization: Bearer abcdefghijklmnop' x" } }]),
      user("u3", "a2", [{ type: "tool_result", tool_use_id: "b1", content: "ok" }]),
      assistant("a3", "u3", [{ type: "tool_use", id: "tool1", name: "Agent", input: { description: "audit", subagent_type: "explore" } }]),
      user("u4", "a3", [{ type: "tool_result", tool_use_id: "tool1", content: "spawned" }], { toolUseResult: { agentId: "child1", tool_use_id: "tool1", status: "completed" } }),
      { type: "summary", summary: "compacted earlier turns", leafUuid: "u1" },
      "{broken json",
    ])
    writeChild(root, "proj", "S", "child1", [user("cu1", null, "go", { isSidechain: true }), assistant("ca1", "cu1", [{ type: "text", text: "Audited the code" }])])
    const page = readClaudeHistoryPage({ projectsRoot: root, threadRef: "claude:S", limit: 200 })!
    expect(page.items.map(item => item.kind)).toEqual(["user_message", "assistant_message", "tool_call", "tool_result", "collaboration", "tool_result", "context", "gap"])
    // Credential scrubbed inline, record still rendered.
    expect(JSON.stringify(page)).not.toContain("abcdefghijklmnop")
    expect(page.items[2]?.redacted).toBe(true)
    // Agent spawn links its child preview.
    expect(page.items[4]?.relatedAgent).toMatchObject({ threadRef: "claude:child1", latest: { summary: "Audited the code" } })
    // Completeness equation whole-conversation.
    expect(page.completeness).toEqual({ source: 8, rendered: 7, redactions: 0, gaps: 1, complete: true })
    expect(page.completeness.source).toBe(page.completeness.rendered + page.completeness.redactions + page.completeness.gaps)
    expect(decodeCodexHistoryPage(page)).not.toBeNull()
  })

  test("represents a rootless child as an explicit orphan/topology gap, never hidden", () => {
    const root = projectsRoot()
    // Session S2 has a root file, so its child attaches to the session root.
    writeParent(root, "proj", "S2", [user("v1", null, "Root2"), assistant("v2", "v1", [{ type: "text", text: "hi" }])])
    writeChild(root, "proj", "S2", "linked", [user("lu", null, "go", { isSidechain: true })])
    // Orphan child: a subagent file whose session root JSONL is ABSENT — its
    // topology cannot be recovered, so it surfaces as a rootless gap node.
    writeChild(root, "proj", "S3", "orphan1", [user("ou", null, "orphaned work", { isSidechain: true })])
    const graph = buildClaudeHistoryGraph(root)
    const topo = claudeHistoryTopology(graph)
    expect(topo).toEqual({ childFiles: 2, linked: 1, orphans: 1 })
    const catalog = readClaudeHistoryCatalog(root, graph)
    const orphan = catalog.agents.find(agent => agent.threadRef === "claude:orphan1")!
    expect(orphan.orphan).toBe(true)
    expect(orphan.parentThreadRef).toBeNull() // rootless — shown as its own node
    expect(orphan.status).toBe("unknown")
    // The rooted child attaches to its session root and is not flagged.
    const linked = catalog.agents.find(agent => agent.threadRef === "claude:linked")!
    expect(linked.orphan).toBeUndefined()
    expect(linked.parentThreadRef).toBe("claude:S2")
  })

  test("pages without overlap or omission across a large child transcript", () => {
    const root = projectsRoot()
    writeParent(root, "proj", "S", [user("u1", null, "Root"), ...Array.from({ length: 501 }, (_, i) => assistant(`a${i}`, "u1", [{ type: "text", text: `turn ${i}` }]))])
    const pages = [0, 200, 400].map(offset => readClaudeHistoryPage({ projectsRoot: root, threadRef: "claude:S", offset, limit: 200 })!)
    const refs = pages.flatMap(page => page.items.map(item => item.itemRef))
    expect(new Set(refs).size).toBe(502)
    expect(pages[0]?.hasPrevious).toBe(false)
    expect(pages[2]?.hasNext).toBe(false)
    expect(pages[0]?.completeness.source).toBe(502)
  })

  test("is empty and usable when the Claude root is absent or malformed", () => {
    expect(readClaudeHistoryCatalog(path.join(tmpdir(), "missing-oa-claude")).roots).toEqual([])
    const root = projectsRoot(); writeParent(root, "proj", "S", [{ nope: true }, "{bad"])
    // A session with no user text still surfaces with a fallback title.
    expect(readClaudeHistoryCatalog(root).roots[0]).toMatchObject({ title: "Untitled Claude chat", source: "claude" })
  })
})
