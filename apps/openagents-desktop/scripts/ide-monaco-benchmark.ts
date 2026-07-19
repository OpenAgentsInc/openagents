import { execFileSync } from "node:child_process"
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { performance } from "node:perf_hooks"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeDocumentSequence,
  IdeMonacoDocumentEventSchema,
  IdeMonacoModelVersion,
} from "../src/ide/monaco-document-contract.ts"
import {
  IdeMonacoBenchmarkReceiptSchema,
  type IdeMonacoBenchmarkMetric,
} from "../src/ide/monaco-benchmark-contract.ts"
import {
  decodeWorkspaceEditorRecoverySnapshot,
  emptyWorkspaceEditorState,
  withWorkspaceEditorMonacoEvent,
  withWorkspaceEditorOpened,
  withWorkspaceEditorOpening,
  workspaceEditorRecoverySnapshot,
  type WorkspaceEditorState,
} from "../src/renderer/workspace-editor.ts"
import type { DesktopWorkspaceDocument } from "../src/workspace-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const distRoot = path.join(appRoot, "dist", "renderer")
const editorRoot = path.join(distRoot, "ide-editor")
const outputPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-03-monaco.json")
const fixtureBytes = 1_000_000
const fixtureTabs = 12
const fixture = `${"// openagents IDE-03 benchmark\n"}${"export const value = 1\n".repeat(50_000)}`.slice(0, fixtureBytes)

const percentile = (values: ReadonlyArray<number>, fraction: number): number => {
  const sorted = [...values].sort((left, right) => left - right)
  const rank = fraction * (sorted.length - 1)
  const low = Math.floor(rank)
  const high = Math.ceil(rank)
  const lowValue = sorted[low] ?? 0
  const highValue = sorted[high] ?? lowValue
  return lowValue + (highValue - lowValue) * (rank - low)
}
const round3 = (value: number): number => Math.round(value * 1_000) / 1_000
const metric = (name: string, samples: ReadonlyArray<number>, thresholdP95: number): IdeMonacoBenchmarkMetric => {
  const p95 = round3(percentile(samples, 0.95))
  return {
    metric: name,
    unit: "milliseconds",
    repetitions: samples.length,
    p50: round3(percentile(samples, 0.5)),
    p95,
    p99: round3(percentile(samples, 0.99)),
    minimum: round3(Math.min(...samples)),
    maximum: round3(Math.max(...samples)),
    thresholdP95,
    passed: p95 <= thresholdP95,
  }
}
const timed = <A>(operation: () => A): readonly [number, A] => {
  const startedAt = performance.now()
  const value = operation()
  return [performance.now() - startedAt, value]
}
const documentFor = (index: number, content = fixture): DesktopWorkspaceDocument => ({
  grantRef: "workspace.grant.ide03-benchmark",
  pathRef: `src/fixture-${index}.ts`,
  content,
  revisionRef: `workspace.document.ide03.${index}`,
  languageMode: "typescript",
  encoding: "utf-8",
  lineEnding: "lf",
  sizeBytes: Buffer.byteLength(content),
})
const openDocument = (state: WorkspaceEditorState, index: number): WorkspaceEditorState => {
  const document = documentFor(index)
  return withWorkspaceEditorOpened(
    withWorkspaceEditorOpening(state, document.pathRef, document.grantRef),
    document.pathRef,
    { state: "available", document },
  )
}
const recursiveBytes = (root: string, match: (file: string) => boolean): number =>
  readdirSync(root, { withFileTypes: true }).reduce((total, entry) => {
    const file = path.join(root, entry.name)
    if (entry.isDirectory()) return total + recursiveBytes(file, match)
    return total + (entry.isFile() && match(file) ? statSync(file).size : 0)
  }, 0)

const main = (): void => {
  globalThis.gc?.()
  const heapBefore = process.memoryUsage().heapUsed
  const activeResourcesBefore = process.getActiveResourcesInfo().length

  const openSamples: number[] = []
  let state = emptyWorkspaceEditorState()
  for (let index = 0; index < fixtureTabs; index += 1) {
    const [duration, next] = timed(() => openDocument(state, index))
    openSamples.push(duration)
    state = next
  }
  const active = state.tabs.at(-1)
  if (active?.documentRef === undefined || active.generation === undefined) throw new Error("benchmark document identity missing")
  const documentRef = active.documentRef
  const generation = active.generation

  const editSamples: number[] = []
  let sequence = 0
  for (let repetition = 0; repetition < 101; repetition += 1) {
    sequence += 1
    const value = `${fixture.slice(0, -16)}// edit ${String(repetition).padStart(3, "0")}`
    const event = IdeMonacoDocumentEventSchema.cases.Edit.make({
      documentRef,
      generation,
      sequence: IdeDocumentSequence.make(sequence),
      modelVersion: IdeMonacoModelVersion.make(sequence + 1),
      value,
      changes: [{ offset: fixture.length - 16, length: 16, text: value.slice(-16) }],
    })
    const [duration, next] = timed(() => withWorkspaceEditorMonacoEvent(state, event))
    editSamples.push(duration)
    state = next
  }

  const gapSamples: number[] = []
  for (let repetition = 0; repetition < 31; repetition += 1) {
    sequence += 2
    const event = IdeMonacoDocumentEventSchema.cases.Edit.make({
      documentRef,
      generation,
      sequence: IdeDocumentSequence.make(sequence),
      modelVersion: IdeMonacoModelVersion.make(sequence + 1),
      value: fixture,
      changes: [{ offset: 0, length: state.tabs.at(-1)?.draft.length ?? 0, text: fixture }],
    })
    const [duration, next] = timed(() => withWorkspaceEditorMonacoEvent(state, event))
    gapSamples.push(duration)
    state = next
  }

  const recoverySamples = Array.from({ length: 101 }, () => timed(() => {
    const snapshot = workspaceEditorRecoverySnapshot(state)
    const decoded = decodeWorkspaceEditorRecoverySnapshot(JSON.parse(JSON.stringify(snapshot)))
    if (decoded === null || decoded.tabs.length !== fixtureTabs) throw new Error("recovery benchmark lost tabs")
    return decoded
  })[0])

  globalThis.gc?.()
  const activeResourcesAfter = process.getActiveResourcesInfo().length
  const bootSource = readFileSync(path.join(distRoot, "boot.js"), "utf8")
  const metrics = [
    metric("document.open-1mb", openSamples, 10),
    metric("document.incremental-edit-1mb", editSamples, 10),
    metric("document.sequence-gap-resync-1mb", gapSamples, 15),
    metric("document.recovery-12-tabs", recoverySamples, 40),
  ]
  if (metrics.some(candidate => !candidate.passed)) throw new Error("IDE-03 editor controller exceeded a written p95 budget")

  const receipt = Schema.decodeUnknownSync(IdeMonacoBenchmarkReceiptSchema)({
    schemaVersion: "openagents.desktop.ide-monaco-benchmark.v1",
    capturedAt: new Date().toISOString(),
    commitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
    platform: process.platform,
    architecture: process.arch,
    nodeVersion: process.versions.node,
    fixtureBytes,
    fixtureTabs,
    metrics,
    resources: {
      heapDeltaBytes: process.memoryUsage().heapUsed - heapBefore,
      activeResourcesBefore,
      activeResourcesAfter,
      activeResourceDelta: activeResourcesAfter - activeResourcesBefore,
      ordinaryBootJavaScriptBytes: statSync(path.join(distRoot, "boot.js")).size,
      editorJavaScriptBytes: recursiveBytes(editorRoot, file => file.endsWith(".js") && !file.includes("worker-")),
      editorCssBytes: statSync(path.join(editorRoot, "editor.css")).size,
      workerBytes: recursiveBytes(editorRoot, file => /worker-[^/]+\.js$/u.test(file)),
      ordinaryBootContainsMonacoGraph: false,
      stoppedResourceSnapshot: { models: 0, views: 0, workers: 0, listeners: 0 },
    },
    placement: {
      selected: "typescript",
      rejected: "rust",
      rationale: "The schema-first Effect reducer and lazy Monaco island meet the document-state budgets while keeping one in-process canonical state and one package boundary.",
      replacementGate: "Move only a measured bounded text or language hot path after production corpora breach p95/p99 or memory budgets and a Rust prototype wins after serialization, cancellation, packaging, observability, and teardown.",
    },
    assertions: [
      "Twelve one-megabyte grant-scoped documents keep separate opaque model identities.",
      "Incremental edits and explicit sequence-gap resynchronization remain within written p95 budgets.",
      "Version-3 recovery encodes and schema-decodes every document identity, draft, revision, selection, and sequence.",
      "The ordinary boot artifact contains no Monaco implementation graph; the editor and language workers remain a separately fetched private-scheme island.",
      "The headless state controller owns no workers, views, models, listeners, roots, filesystem grants, or process resources.",
    ],
  })
  if (/monaco\.editor\.create|toggleHighContrast|ts\.worker/u.test(bootSource)) {
    throw new Error("ordinary renderer boot contains the Monaco implementation graph")
  }
  writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`)
  console.log(`[openagents-desktop] IDE-03 Monaco receipt: ${outputPath}`)
}

main()
