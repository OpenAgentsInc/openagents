/**
 * Structure-only real-history receipt for CUT-22 (#8702).
 *
 * Runs the landed read-only Claude importer against a real ~/.claude/projects
 * archive and emits ONLY structural counts, the loss-accounting completeness
 * equation, and timing. It never prints titles, message content, tool text,
 * file paths, or any owner-identifying string — so the receipt is safe to paste
 * into a public issue. Usage:
 *
 *   bun run scripts/claude-history-receipt.ts [projectsRoot]
 *
 * Defaults to $HOME/.claude/projects.
 */
import os from "node:os"
import path from "node:path"

import { buildClaudeHistoryGraph, claudeHistoryTopology, readClaudeHistoryCatalog, readClaudeHistoryPage } from "../src/claude-history.ts"

const projectsRoot = process.argv[2] ?? path.join(os.homedir(), ".claude", "projects")

const t0 = performance.now()
const graph = buildClaudeHistoryGraph(projectsRoot)
const buildMs = performance.now() - t0

const catalog = readClaudeHistoryCatalog(projectsRoot, graph)
const topology = claudeHistoryTopology(graph)

// Sample the completeness equation across the first N session roots (bounded;
// a page read reconstructs Agent-edge previews for that root).
const sampleRoots = catalog.roots.slice(0, 25)
let checked = 0
let equationHeld = 0
let totalSource = 0
let totalRendered = 0
let totalRedactions = 0
let totalGaps = 0
for (const root of sampleRoots) {
  const page = readClaudeHistoryPage({ projectsRoot, threadRef: root.threadRef, limit: 500 }, graph)
  if (page === null) continue
  checked += 1
  const c = page.completeness
  totalSource += c.source
  totalRendered += c.rendered
  totalRedactions += c.redactions
  totalGaps += c.gaps
  if (c.source === c.rendered + c.redactions + c.gaps) equationHeld += 1
}

const receipt = {
  artifact: "cut22-claude-history-structure-only-receipt",
  issue: "OpenAgentsInc/openagents#8702",
  nodes: graph.nodes.length,
  sessionRoots: catalog.roots.length,
  agents: catalog.agents.length,
  childFiles: topology.childFiles,
  linkedChildren: topology.linked,
  orphanTopologyGaps: topology.orphans,
  orphanRatePct: topology.childFiles === 0 ? 0 : Math.round((topology.orphans / topology.childFiles) * 1000) / 10,
  buildMs: Math.round(buildMs),
  sampledRoots: checked,
  completenessEquationHeld: `${equationHeld}/${checked}`,
  sampleSource: totalSource,
  sampleRendered: totalRendered,
  sampleRedactions: totalRedactions,
  sampleGaps: totalGaps,
  sampleEquationBalances: totalSource === totalRendered + totalRedactions + totalGaps,
}
console.log(JSON.stringify(receipt, null, 2))
