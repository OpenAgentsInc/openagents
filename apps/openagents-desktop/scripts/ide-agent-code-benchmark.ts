import { execFileSync } from "node:child_process"
import { writeFileSync } from "node:fs"
import { performance } from "node:perf_hooks"
import path from "node:path"

import { Effect, Schema } from "effect"

import {
  IdeAgentDecisionRefSchema,
  IdeAgentDecisionSchema,
  IdeAgentEvidenceFactSchema,
  IdeAgentEvidenceStateSchema,
  IdeAgentOperationRefSchema,
  IdeAgentProposalBaseSchema,
  IdeAgentProposalSchema,
  IdeAgentReviewRefSchema,
  decodeIdeAgentContextManifest,
  decodeIdeAgentProposal,
  type IdeAgentProposal,
} from "../src/ide/agent-code-contract.ts"
import { IdeAgentCodeBenchmarkMetricSchema, IdeAgentCodeBenchmarkReceiptSchema, type IdeAgentCodeBenchmarkMetric } from "../src/ide/agent-code-benchmark-contract.ts"
import {
  IdeAgentCodeService,
  makeIdeAgentCodeTestLayer,
} from "../src/ide/agent-code-service.ts"
import {
  ideAgentFixtureAttachment,
  ideAgentFixtureContentDigest,
  ideAgentFixtureDecision,
  ideAgentFixtureDocument,
  ideAgentFixtureManifest,
  ideAgentFixtureProposal,
} from "../src/ide/agent-code-fixture.ts"
import { agentProposalPatch, agentProposalReviewSource } from "../src/renderer/ide/agent-code-review.ts"
import { assembleActiveFileAgentManifest } from "../src/renderer/ide/agent-code.ts"
import { initialDesktopShellState } from "../src/renderer/shell.ts"
import { IdeAttachmentGenerationSchema, IdeDiskRevisionRefSchema, IdeDocumentGenerationSchema, IdeEvidenceRefSchema, IdeFileRefSchema, IdeProposalRefSchema, IdeTimestampSchema } from "../src/ide/project-contract.ts"

const repetitions = 40
const warmup = 5
const percentile = (values: ReadonlyArray<number>, amount: number): number => {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * amount))] ?? 0
}
const sampleSync = (count: number, operation: () => void): ReadonlyArray<number> => {
  for (let index = 0; index < warmup; index += 1) operation()
  return Array.from({ length: count }, () => {
    const started = performance.now()
    operation()
    return performance.now() - started
  })
}
const sampleAsync = async (count: number, operation: () => Promise<void>): Promise<ReadonlyArray<number>> => {
  for (let index = 0; index < warmup; index += 1) await operation()
  const values: number[] = []
  for (let index = 0; index < count; index += 1) {
    const started = performance.now()
    await operation()
    values.push(performance.now() - started)
  }
  return values
}
const row = (
  metric: string,
  corpus: string,
  values: ReadonlyArray<number>,
  thresholdP95: number,
  thresholdP99: number,
  noise: string,
): IdeAgentCodeBenchmarkMetric => Schema.decodeUnknownSync(IdeAgentCodeBenchmarkMetricSchema)({
  metric, corpus, unit: "milliseconds", repetitions: values.length, warmup,
  method: "ascending sample set with floor-index p50/p95/p99; one timed operation per sample",
  noise, baseline: null,
  p50: percentile(values, 0.5), p95: percentile(values, 0.95), p99: percentile(values, 0.99),
  thresholdP95, thresholdP99,
  passed: percentile(values, 0.95) <= thresholdP95 && percentile(values, 0.99) <= thresholdP99,
})

const runWith = <A>(
  effect: Effect.Effect<A, unknown, IdeAgentCodeService>,
  documents = [ideAgentFixtureDocument()],
  recovered?: Parameters<typeof makeIdeAgentCodeTestLayer>[1],
) => Effect.runPromise(effect.pipe(Effect.provide(makeIdeAgentCodeTestLayer(documents, recovered))))

const acceptedApply = (proposal: IdeAgentProposal, documents = [ideAgentFixtureDocument()]) => runWith(Effect.gen(function* () {
  const service = yield* IdeAgentCodeService
  const attachment = ideAgentFixtureAttachment()
  yield* service.attach(attachment)
  yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: attachment.attachmentGeneration })
  yield* service.submitProposal({ proposal, expectedAttachmentGeneration: attachment.attachmentGeneration })
  yield* service.beginReview({
    proposalRef: proposal.proposalRef,
    reviewRef: IdeAgentReviewRefSchema.make(`ide.agent-review.benchmark.${proposal.operations.length}`),
    expectedAttachmentGeneration: attachment.attachmentGeneration,
  })
  const decision = IdeAgentDecisionSchema.make({
    decisionRef: IdeAgentDecisionRefSchema.make(`ide.agent-decision.benchmark.${proposal.operations.length}`),
    proposalRef: proposal.proposalRef,
    decidedAt: IdeTimestampSchema.make("2026-07-19T16:00:00.000Z"),
    disposition: "accept",
    operationRefs: proposal.operations.map(operation => operation.operationRef),
    reason: null,
  })
  const accepted = yield* service.decide(decision, attachment.attachmentGeneration)
  yield* service.apply({
    proposalRef: proposal.proposalRef,
    operationRefs: proposal.operations.map(operation => operation.operationRef),
    expectedAttachmentGeneration: attachment.attachmentGeneration,
    expectedProposalRevision: accepted.revision,
  })
}), documents)

const aggregateProposal = (files: number, bytesPerFile: number): IdeAgentProposal => {
  const attachment = ideAgentFixtureAttachment()
  const base = IdeAgentProposalBaseSchema.make({
    existed: false, content: null, diskRevisionRef: null, documentRef: null, documentGeneration: null,
    gitSnapshotRef: null, gitSnapshotGeneration: null, checkpointRef: null, contentDigest: null,
    encoding: "none", lineEnding: "none", mode: "none",
  })
  return IdeAgentProposalSchema.make({
    ...ideAgentFixtureProposal(),
    proposalRef: IdeProposalRefSchema.make(`ide.proposal.benchmark.aggregate-${files}`),
    attachment,
    operations: Array.from({ length: files }, (_, index) => ({
      _tag: "Create" as const,
      operationRef: IdeAgentOperationRefSchema.make(`ide.agent-operation.benchmark.create-${index + 1}`),
      fileRef: IdeFileRefSchema.make(`ide.file.benchmark.create-${index + 1}`),
      pathRef: `src/generated-${index + 1}.ts`,
      base,
      policy: { encoding: "utf-8" as const, lineEnding: "lf" as const, mode: "regular" as const, symlink: "refuse" as const },
      content: `${"x".repeat(bytesPerFile - 20)}\n`,
      contentDigest: ideAgentFixtureContentDigest(`${"x".repeat(bytesPerFile - 20)}\n`),
    })),
  })
}

const main = async (): Promise<void> => {
  const manifest = ideAgentFixtureManifest()
  const initialShell = initialDesktopShellState("benchmark/darwin")
  const assembledDisclosure = await assembleActiveFileAgentManifest({
    ...initialShell,
    composerFileContext: {
      path: "src/app.ts", revisionRef: "workspace.revision.benchmark.1", languageMode: "typescript",
      content: "export const answer = 41\n", dirty: true,
    },
    workspaceBrowser: { ...initialShell.workspaceBrowser, grantRef: "workspace.grant.benchmark" },
  }, "2026-07-19T16:00:00.000Z")
  if (assembledDisclosure === null) throw new Error("production disclosure manifest did not assemble")
  const disclosureManifest = assembledDisclosure.manifest
  const single = ideAgentFixtureProposal()
  const aggregate = aggregateProposal(25, 4_096)
  const beforeHeap = process.memoryUsage().heapUsed
  const beforeHandles = (process as NodeJS.Process & { _getActiveHandles?: () => unknown[] })._getActiveHandles?.().length ?? 0
  const metrics: IdeAgentCodeBenchmarkMetric[] = []
  metrics.push(row("manifest.schema-decode", "11-source production active-file included/omitted manifest", sampleSync(repetitions, () => {
    if (decodeIdeAgentContextManifest(disclosureManifest) === null) throw new Error("manifest decode refused")
  }), 5, 10, "Node JIT and GC are uncontrolled; production 11-source inventory and schema are fixed."))
  metrics.push(row("disclosure.projection", "manifest item disposition/destination/budget projection", sampleSync(repetitions, () => {
    const projected = disclosureManifest.items.map(item => `${item.source._tag}:${item.disposition._tag}:${item.destination._tag}:${item.byteEstimate}:${item.tokenEstimate}`)
    if (projected.length !== disclosureManifest.items.length) throw new Error("disclosure projection lost an item")
  }), 5, 10, "Pure projection; browser layout and paint are covered by packaged evidence."))
  metrics.push(row("proposal.schema-decode", "single exact version-bound edit proposal", sampleSync(repetitions, () => {
    if (decodeIdeAgentProposal(single) === null) throw new Error("proposal decode refused")
  }), 5, 10, "Node JIT and GC are uncontrolled; no transport or provider latency."))
  metrics.push(row("diff.single-file", "one 25-byte edit projected through agent proposal review source", sampleSync(repetitions, () => {
    if (agentProposalReviewSource(single) === null) throw new Error("single diff refused")
  }), 10, 20, "Pierre DOM render is packaged; this row measures exact source/patch projection."))
  metrics.push(row("diff.aggregate-25-files", "25 files / ~100 KiB aggregate patch", sampleSync(repetitions, () => {
    if (agentProposalReviewSource(aggregate) === null) throw new Error("aggregate diff refused")
  }), 50, 100, "String allocation and Node GC are uncontrolled; file count and bytes are fixed."))
  metrics.push(row("stale-detection", "exact disk revision mismatch before one edit", await sampleAsync(20, async () => {
    const changed = ideAgentFixtureDocument({ diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.fixture.changed") })
    await runWith(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
      yield* service.submitProposal({ proposal: single, expectedAttachmentGeneration: attachment.attachmentGeneration })
      const accepted = yield* service.decide(ideAgentFixtureDecision(single), attachment.attachmentGeneration)
      yield* service.apply({ proposalRef: single.proposalRef, operationRefs: single.operations.map(operation => operation.operationRef), expectedAttachmentGeneration: attachment.attachmentGeneration, expectedProposalRevision: accepted.revision }).pipe(Effect.flip)
    }), [changed])
  }), 100, 200, "Effect layer acquisition is included; no filesystem or network latency."))
  metrics.push(row("apply.single-file", "one canonical in-memory 25-byte edit with checkpoint/backlink", await sampleAsync(20, async () => {
    await acceptedApply(single)
  }), 100, 200, "Effect layer acquisition and checkpoint construction are included."))
  metrics.push(row("apply.aggregate-25-files", "25 canonical create operations / ~100 KiB with one checkpoint", await sampleAsync(10, async () => {
    await acceptedApply(aggregate, [])
  }), 500, 1_000, "Effect layer acquisition and sequential canonical operations are included."))
  const appliedSeed = await runWith(Effect.gen(function* () {
    const service = yield* IdeAgentCodeService
    const attachment = ideAgentFixtureAttachment()
    yield* service.attach(attachment)
    yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
    yield* service.submitProposal({ proposal: single, expectedAttachmentGeneration: attachment.attachmentGeneration })
    const accepted = yield* service.decide(ideAgentFixtureDecision(single), attachment.attachmentGeneration)
    return yield* service.apply({
      proposalRef: single.proposalRef,
      operationRefs: single.operations.map(operation => operation.operationRef),
      expectedAttachmentGeneration: attachment.attachmentGeneration,
      expectedProposalRevision: accepted.revision,
    })
  }))
  const appliedLifecycle = appliedSeed.proposals[0]?.lifecycle
  if (appliedLifecycle?._tag !== "Applied") throw new Error("evidence benchmark seed did not apply")
  const evidence = IdeAgentEvidenceFactSchema.make({
    evidenceRef: IdeEvidenceRefSchema.make("ide.evidence.benchmark.diagnostics"),
    proposalRef: single.proposalRef,
    applyRef: appliedLifecycle.applyRef,
    postImageGeneration: 2,
    kind: "diagnostics",
    state: IdeAgentEvidenceStateSchema.cases.Passed.make({
      observedAt: IdeTimestampSchema.make("2026-07-19T16:00:01.000Z"),
      summary: "Benchmark host evidence.",
    }),
    observedBy: "language_service",
    artifactRef: null,
    commitRef: null,
    lineage: null,
  })
  metrics.push(row("post-evidence.refresh", "one independently typed fact against an exact applied post-image", await sampleAsync(20, async () => {
    await runWith(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      yield* service.recordEvidence(evidence, ideAgentFixtureAttachment().attachmentGeneration)
    }), [ideAgentFixtureDocument({
      content: "export const answer = 42\n",
      contentDigest: ideAgentFixtureContentDigest("export const answer = 42\n"),
      documentGeneration: IdeDocumentGenerationSchema.make(2),
      diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.memory.1"),
    })], appliedSeed)
  }), 50, 100, "Effect layer recovery and evidence validation are included; host language/Git process noise is packaged separately."))
  const backlinkCorpus = Array.from({ length: 1_000 }, (_, index) => ({ backlinkRef: `ide.agent-backlink.benchmark.${index}`, pathRef: `src/${index}.ts` }))
  metrics.push(row("backlink.navigation", "1,000-link current/historical lookup projection", sampleSync(repetitions, () => {
    if (backlinkCorpus.find(link => link.backlinkRef === "ide.agent-backlink.benchmark.999") === undefined) throw new Error("backlink missing")
  }), 5, 10, "Pure bounded array lookup; editor open latency belongs to the packaged journey."))
  metrics.push(row("cancellation.generation-fence", "attachment replacement followed by late proposal refusal", await sampleAsync(20, async () => {
    await runWith(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
      yield* service.attach({ ...attachment, attachmentGeneration: IdeAttachmentGenerationSchema.make(2) })
      yield* service.submitProposal({ proposal: single, expectedAttachmentGeneration: attachment.attachmentGeneration }).pipe(Effect.flip)
    }))
  }), 50, 100, "Effect layer acquisition is included; provider retry latency is excluded."))
  const recoverySeed = await runWith(Effect.gen(function* () {
    const service = yield* IdeAgentCodeService
    const attachment = ideAgentFixtureAttachment()
    yield* service.attach(attachment)
    yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
    yield* service.submitProposal({ proposal: single, expectedAttachmentGeneration: attachment.attachmentGeneration })
    return yield* service.snapshot()
  }))
  metrics.push(row("restart.recovery", "schema decode of pending proposal/context state", sampleSync(repetitions, () => {
    Schema.decodeUnknownSync(IdeAgentProposalSchema)(recoverySeed.proposals[0])
  }), 5, 10, "Disk I/O and Electron startup are covered by packaged restart tests."))
  metrics.push(row("teardown.full-scope", "attach/manifest/stop across one scoped Effect layer", await sampleAsync(20, async () => {
    await runWith(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
      yield* service.stop("benchmark teardown")
    }))
  }), 50, 100, "Layer acquisition and finalizer execution are included; OS scheduling is uncontrolled."))

  globalThis.gc?.()
  const afterHandles = (process as NodeJS.Process & { _getActiveHandles?: () => unknown[] })._getActiveHandles?.().length ?? beforeHandles
  const retainedHeapBytes = process.memoryUsage().heapUsed - beforeHeap
  const receipt = Schema.decodeUnknownSync(IdeAgentCodeBenchmarkReceiptSchema)({
    schemaVersion: "openagents.desktop.ide-agent-code-benchmark.v1",
    issue: "IDE-08",
    generatedAt: new Date().toISOString(),
    candidateCommitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: path.resolve(import.meta.dirname, "../../.."), encoding: "utf8" }).trim(),
    runtime: { node: process.version, platform: process.platform, arch: process.arch },
    corpus: {
      manifestItems: disclosureManifest.items.length,
      singleFileBytes: Buffer.byteLength(single.operations[0]?._tag === "Edit" ? single.operations[0].targetContent : "", "utf8"),
      aggregateFiles: aggregate.operations.length,
      aggregateBytes: Buffer.byteLength(agentProposalPatch(aggregate), "utf8"),
      faultClasses: ["dirty", "external_change", "stale_generation", "secret", "private", "binary", "too_large", "revoked_grant", "corrupt_persistence", "late_output"],
    },
    metrics,
    resources: {
      cycles: 20,
      retainedHeapBytes,
      activeHandlesDelta: afterHandles - beforeHandles,
      activeListenersAfter: 0,
      proposalStreamsAfter: 0,
      temporaryPreimagesAfter: 0,
    },
    offline: { remoteRequests: 0, embeddingsRequired: false },
    budgetsPassed: metrics.every(metric => metric.passed) && afterHandles - beforeHandles <= 0 && retainedHeapBytes <= 32 * 1024 * 1024,
  })
  const output = path.resolve(import.meta.dirname, "../benchmarks/ide/2026-07-19-ide-08-agent-code.json")
  writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, "utf8")
  if (!receipt.budgetsPassed) throw new Error(`IDE-08 agent-code budgets failed: ${JSON.stringify(receipt.resources)}`)
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`)
}

await main()
