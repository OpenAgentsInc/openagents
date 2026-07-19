import { execFileSync } from "node:child_process"
import { writeFileSync } from "node:fs"
import path from "node:path"
import { performance } from "node:perf_hooks"

import {
  IdeNavigationEntryRefSchema,
  emptyIdeNavigationHistory,
  pushIdeNavigation,
  rankIdeQuickOpen,
} from "../src/ide/workbench-contract.ts"
import { IdeProjectRefSchema, IdeRootRefSchema, IdeWorktreeRefSchema } from "../src/ide/project-contract.ts"
import { IdeDocumentGeneration, makeIdeDocumentRef } from "../src/ide/monaco-document-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const outputPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-04-workbench.json")
const paths = Array.from({ length: 50_000 }, (_, index) =>
  `packages/domain-${String(index % 500).padStart(3, "0")}/src/feature-${String(index).padStart(5, "0")}.ts`)

const percentile = (samples: ReadonlyArray<number>, ratio: number): number => {
  const sorted = [...samples].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * ratio))] ?? 0
}

for (let index = 0; index < 4; index += 1) rankIdeQuickOpen("d042f042", paths)
const quickOpenMs = Array.from({ length: 32 }, () => {
  const started = performance.now()
  const result = rankIdeQuickOpen("d042f042", paths)
  if (result.results.length === 0) throw new Error("quick-open benchmark query did not match")
  return performance.now() - started
})

const navigationMs: number[] = []
let history = emptyIdeNavigationHistory()
for (let index = 0; index < 10_000; index += 1) {
  const started = performance.now()
  history = pushIdeNavigation(history, {
    entryRef: IdeNavigationEntryRefSchema.make(`ide.navigation.benchmark-${index}`),
    source: "quick_open",
    projectRef: IdeProjectRefSchema.make("ide.project.benchmark"),
    rootRef: IdeRootRefSchema.make("ide.root.benchmark"),
    worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.benchmark"),
    documentRef: makeIdeDocumentRef("benchmark", index),
    generation: IdeDocumentGeneration.make(index),
    pathRef: paths[index % paths.length]!,
    selection: { start: index % 1_000, end: index % 1_000 },
    state: "ready",
    reason: null,
  })
  navigationMs.push(performance.now() - started)
}

const receipt = {
  schemaVersion: "openagents.desktop.ide-workbench-benchmark.v1",
  capturedAt: new Date().toISOString(),
  commitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
  fixture: { indexedPaths: paths.length, quickOpenIterations: quickOpenMs.length, navigationPushes: navigationMs.length },
  quickOpenMs: {
    p50: percentile(quickOpenMs, 0.50),
    p95: percentile(quickOpenMs, 0.95),
    p99: percentile(quickOpenMs, 0.99),
  },
  navigationPushMs: {
    p50: percentile(navigationMs, 0.50),
    p95: percentile(navigationMs, 0.95),
    p99: percentile(navigationMs, 0.99),
  },
  retainedHistoryEntries: history.entries.length,
  thresholds: { quickOpenP95Ms: 250, navigationPushP95Ms: 2, retainedHistoryEntries: 100 },
}

if (receipt.quickOpenMs.p95 > receipt.thresholds.quickOpenP95Ms ||
  receipt.navigationPushMs.p95 > receipt.thresholds.navigationPushP95Ms ||
  receipt.retainedHistoryEntries !== receipt.thresholds.retainedHistoryEntries) {
  throw new Error(`IDE-04 workbench benchmark exceeded its admission envelope: ${JSON.stringify(receipt)}`)
}

writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`[openagents-desktop] IDE-04 workbench benchmark: ${outputPath}\n`)
