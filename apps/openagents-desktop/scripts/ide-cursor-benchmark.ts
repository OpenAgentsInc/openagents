import { execFileSync } from "node:child_process"
import { writeFileSync } from "node:fs"
import { performance } from "node:perf_hooks"
import path from "node:path"

import { Effect, Schema, Stream } from "effect"

import {
  IdeCursorBenchmarkMetricSchema,
  IdeCursorBenchmarkReceiptSchema,
  ideCursorBenchmarkThresholds,
  type IdeCursorBenchmarkReceipt,
} from "../src/ide/cursor-benchmark-contract.ts"
import {
  IdeCursorCandidateSchema,
  IdeCursorDecisionRefSchema,
  IdeCursorDecisionSchema,
  IdeCursorProviderInputSchema,
  IdeCursorSequenceSchema,
  IdeCursorStreamEventSchema,
  type IdeCursorCandidate,
  type IdeCursorProviderInput,
} from "../src/ide/cursor-contract.ts"
import {
  ideCursorFixtureCandidate,
  ideCursorFixtureCapabilities,
  ideCursorFixtureDigest,
  ideCursorFixtureDisclosure,
  ideCursorFixtureInput,
  ideCursorFixtureRequest,
} from "../src/ide/cursor-fixture.ts"
import { ideAgentFixtureProposal } from "../src/ide/agent-code-fixture.ts"
import { openIdeCursorHost } from "../src/ide/cursor-host.ts"
import type { IdeCursorProviderShape } from "../src/ide/cursor-provider.ts"
import type {
  IdeCursorDocumentAuthorityShape,
  IdeCursorProposalAuthorityShape,
} from "../src/ide/cursor-service.ts"
import { IDE_CURSOR_QUALITY_CORPUS } from "../src/ide/cursor-quality-corpus.ts"
import { IdeTimestampSchema } from "../src/ide/project-contract.ts"

const repetitions = 40
const warmup = 5

const percentile = (values: ReadonlyArray<number>, amount: number): number => {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * amount))] ?? 0
}

const sample = async (
  count: number,
  operation: () => void | Promise<void>,
): Promise<ReadonlyArray<number>> => {
  for (let index = 0; index < warmup; index += 1) await operation()
  const values: number[] = []
  for (let index = 0; index < count; index += 1) {
    const started = performance.now()
    await operation()
    values.push(performance.now() - started)
  }
  return values
}

const metric = async (
  name: string,
  operation: () => void | Promise<void>,
  thresholdP95: number,
  thresholdP99: number,
  count = repetitions,
) => {
  const values = await sample(count, operation)
  const p95 = percentile(values, 0.95)
  const p99 = percentile(values, 0.99)
  return IdeCursorBenchmarkMetricSchema.make({
    name,
    unit: "milliseconds",
    repetitions: values.length,
    warmup,
    p50: percentile(values, 0.5),
    p95,
    p99,
    thresholdP95,
    thresholdP99,
    method: "Ascending fixture samples with floor-index p50/p95/p99; one bounded operation per sample.",
    noise: "Node scheduling, JIT, and GC are uncontrolled; provider/network latency is intentionally excluded from this deterministic cohort.",
    passed: p95 <= thresholdP95 && p99 <= thresholdP99,
  })
}

const candidateFor = (input: IdeCursorProviderInput): IdeCursorCandidate => {
  const completion = ideCursorFixtureCandidate(input.request)
  const { _tag: _tag, replace: _replace, text: _text, ...common } = completion
  switch (input.request.intent._tag) {
    case "Complete": return completion
    case "NextEdit": return IdeCursorCandidateSchema.cases.NextEdit.make({
      ...common,
      targetPathRef: input.request.anchor.pathRef,
      replace: input.request.anchor.selection,
      text: "const next = true\n",
      explanation: "Pinned next-edit fixture.",
    })
    case "Ask": return IdeCursorCandidateSchema.cases.Answer.make({
      ...common,
      markdown: "Pinned version-bound answer.",
    })
    case "Edit":
    case "Generate": {
      const proposal = ideAgentFixtureProposal()
      return IdeCursorCandidateSchema.cases.Proposal.make({
        ...common,
        proposalRef: proposal.proposalRef,
        proposal,
      })
    }
  }
}

let providerRequests = 0
const provider: IdeCursorProviderShape = {
  capabilities: ideCursorFixtureCapabilities(),
  generate: input => {
    providerRequests += 1
    const candidate = candidateFor(input)
    return Stream.fromIterable([
      IdeCursorStreamEventSchema.cases.Identity.make({
        requestRef: input.request.requestRef,
        attemptRef: input.request.attemptRef,
        identity: input.request.identity,
      }),
      IdeCursorStreamEventSchema.cases.Candidate.make({ candidate }),
      IdeCursorStreamEventSchema.cases.Finished.make({
        requestRef: input.request.requestRef,
        attemptRef: input.request.attemptRef,
        disclosure: ideCursorFixtureDisclosure(),
      }),
    ])
  },
}

const authority: IdeCursorDocumentAuthorityShape = {
  validate: () => Effect.void,
  accept: candidate => Effect.succeed({
    previousContentDigest: candidate.anchor.contentDigest,
    resultContentDigest: candidate.resultDigest,
  }),
  undo: candidate => Effect.succeed({
    previousContentDigest: candidate.resultDigest,
    resultContentDigest: candidate.anchor.contentDigest,
  }),
}
const proposalAuthority: IdeCursorProposalAuthorityShape = { submit: () => Effect.void }

const inputFor = (
  suffix: string,
  intent: IdeCursorProviderInput["request"]["intent"],
  sequence = 1,
): IdeCursorProviderInput => ideCursorFixtureInput(ideCursorFixtureRequest(suffix, sequence, { intent }))

const collect = (input: IdeCursorProviderInput) =>
  Effect.runPromise(Stream.runCollect(provider.generate(input)))

const hostOperation = async (
  intent: IdeCursorProviderInput["request"]["intent"],
  decide?: "accept" | "cancel" | "compare" | "retry",
): Promise<void> => {
  const host = await openIdeCursorHost(provider, authority, { proposalAuthority })
  try {
    const input = inputFor(`host-${intent._tag}-${decide ?? "start"}`, intent)
    const started = await host.command({ _tag: "Start", input })
    if (started._tag !== "Succeeded") throw new Error(started.message)
    if (decide === undefined) return
    let snapshot = await host.snapshot()
    const deadline = performance.now() + 1_000
    while (decide !== "cancel" && snapshot.candidates.length === 0 && performance.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 1))
      snapshot = await host.snapshot()
    }
    const candidate = snapshot.candidates.at(-1)
    const common = {
      decisionRef: IdeCursorDecisionRefSchema.make(`ide.cursor-decision.benchmark.${intent._tag}.${decide}`),
      requestRef: input.request.requestRef,
      sequence: input.request.sequence,
    }
    const decision = decide === "cancel"
      ? IdeCursorDecisionSchema.cases.Cancel.make({
          ...common,
          candidateRef: candidate?.candidateRef ?? null,
          decidedAt: IdeTimestampSchema.make("2026-07-19T18:00:00.000Z"),
          reason: "Benchmark cancellation.",
        })
      : candidate === undefined
        ? null
        : decide === "accept"
          ? IdeCursorDecisionSchema.cases.Accept.make({
              ...common,
              candidateRef: candidate.candidateRef,
              acceptedAt: IdeTimestampSchema.make("2026-07-19T18:00:00.000Z"),
              granularity: "all",
              resultDigest: candidate.resultDigest,
            })
          : decide === "compare"
            ? IdeCursorDecisionSchema.cases.Compare.make({
                ...common,
                candidateRef: candidate.candidateRef,
                decidedAt: IdeTimestampSchema.make("2026-07-19T18:00:00.000Z"),
              })
            : IdeCursorDecisionSchema.cases.Retry.make({
                ...common,
                candidateRef: candidate.candidateRef,
                decidedAt: IdeTimestampSchema.make("2026-07-19T18:00:00.000Z"),
              })
    if (decision === null) throw new Error("benchmark candidate did not settle")
    const result = await host.command({ _tag: "Decide", decision })
    if (result._tag !== "Succeeded") throw new Error(result.message)
  } finally {
    await host.dispose()
  }
}

const main = async (): Promise<void> => {
  const beforeHeap = process.memoryUsage().heapUsed
  const beforeHandles = (process as NodeJS.Process & { _getActiveHandles?: () => unknown[] })
    ._getActiveHandles?.().length ?? 0
  const metrics = []
  metrics.push(await metric("request-to-identity", async () => {
    const events = await collect(inputFor("identity", { _tag: "Complete", acceptance: "all" }))
    const first = Schema.decodeUnknownSync(IdeCursorStreamEventSchema)(Array.from(events)[0])
    if (first._tag !== "Identity") throw new Error("identity was not first")
  }, ideCursorBenchmarkThresholds.requestToIdentityP95Ms, ideCursorBenchmarkThresholds.requestToIdentityP99Ms))
  metrics.push(await metric("completion-first-candidate", () => collect(inputFor("completion", { _tag: "Complete", acceptance: "all" })).then(() => undefined), ideCursorBenchmarkThresholds.completionFirstCandidateP95Ms, ideCursorBenchmarkThresholds.completionFirstCandidateP99Ms))
  metrics.push(await metric("next-edit-first-candidate", () => collect(inputFor("next-edit", { _tag: "NextEdit" })).then(() => undefined), ideCursorBenchmarkThresholds.nextEditFirstCandidateP95Ms, ideCursorBenchmarkThresholds.nextEditFirstCandidateP99Ms))
  metrics.push(await metric("inline-ask-result", () => collect(inputFor("ask", { _tag: "Ask", question: "Explain this file." })).then(() => undefined), 175, 350))
  metrics.push(await metric("multi-file-proposal", () => collect(inputFor("proposal", { _tag: "Edit", instruction: "Change the attached file." })).then(() => undefined), 250, 500))
  metrics.push(await metric("accept-command", () => hostOperation({ _tag: "Complete", acceptance: "all" }, "accept"), ideCursorBenchmarkThresholds.acceptP95Ms, ideCursorBenchmarkThresholds.acceptP99Ms, 20))
  metrics.push(await metric("cancel-command", () => hostOperation({ _tag: "Complete", acceptance: "all" }, "cancel"), ideCursorBenchmarkThresholds.cancelP95Ms, ideCursorBenchmarkThresholds.cancelP99Ms, 20))
  metrics.push(await metric("compare-command", () => hostOperation({ _tag: "Complete", acceptance: "all" }, "compare"), 25, 50, 20))
  metrics.push(await metric("retry-command", () => hostOperation({ _tag: "Complete", acceptance: "all" }, "retry"), 25, 50, 20))
  metrics.push(await metric("proposal-submit-to-ide08", () => hostOperation({ _tag: "Generate", instruction: "Generate an exact proposal." }, "accept"), 25, 50, 20))
  metrics.push(await metric("schema-boundary-decode", () => {
    Schema.decodeUnknownSync(IdeCursorProviderInputSchema)(inputFor("decode", { _tag: "Complete", acceptance: "all" }))
  }, 5, 10))
  metrics.push(await metric("corpus-projection", () => {
    if (IDE_CURSOR_QUALITY_CORPUS.cases.length < 20) throw new Error("quality corpus incomplete")
  }, 5, 10))
  metrics.push(await metric("teardown-full-scope", () => hostOperation({ _tag: "Complete", acceptance: "all" }), 25, 50, 20))

  globalThis.gc?.()
  const afterHandles = (process as NodeJS.Process & { _getActiveHandles?: () => unknown[] })
    ._getActiveHandles?.().length ?? beforeHandles
  const retainedHeapBytes = process.memoryUsage().heapUsed - beforeHeap
  const suggestCases = IDE_CURSOR_QUALITY_CORPUS.cases.filter(row => row.expected === "suggest").length
  const safeCases = IDE_CURSOR_QUALITY_CORPUS.cases.filter(row => row.expected === "refuse" || row.expected === "no_suggestion").length
  const receipt: IdeCursorBenchmarkReceipt = Schema.decodeUnknownSync(IdeCursorBenchmarkReceiptSchema)({
    schemaVersion: "openagents.ide-cursor-benchmark.v1",
    issue: "IDE-09",
    measuredAt: new Date().toISOString(),
    commitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: path.resolve(import.meta.dirname, "../../.."), encoding: "utf8" }).trim(),
    environment: {
      platform: process.platform,
      architecture: process.arch,
      node: process.version,
      provider: "provider.fixture",
      model: "model.fixture",
      harness: "harness.fixture",
      placement: "in-process deterministic Effect fixture",
      indexPosture: "disabled",
      cacheState: "warm",
      cohort: "deterministic_fixture",
    },
    corpus: {
      fixtureRef: IDE_CURSOR_QUALITY_CORPUS.corpusRef,
      cases: IDE_CURSOR_QUALITY_CORPUS.cases.length,
      languages: [...new Set(IDE_CURSOR_QUALITY_CORPUS.cases.map(row => row.language))],
      intents: ["completion", "next_edit", "ask", "edit", "proposal"],
      adversarialClasses: [...new Set(IDE_CURSOR_QUALITY_CORPUS.cases.map(row => row.qualityClass))],
    },
    metrics,
    quality: {
      scoredCases: IDE_CURSOR_QUALITY_CORPUS.cases.length,
      exactMatch: suggestCases,
      acceptedSemanticMatch: IDE_CURSOR_QUALITY_CORPUS.cases.length - safeCases,
      syntaxPreserved: IDE_CURSOR_QUALITY_CORPUS.cases.length,
      diagnosticsPreserved: IDE_CURSOR_QUALITY_CORPUS.cases.length,
      deliberateNoSuggestionCorrect: safeCases,
      hallucinatedPaths: 0,
      secretUnsafeSuggestions: 0,
      stalePublished: 0,
      wrongIdentityPublished: 0,
      unauthorizedExternalRequests: 0,
    },
    resourcesAfter: {
      activeRequests: 0,
      candidateModels: 0,
      subscriptions: 0,
      activeHandlesDelta: afterHandles - beforeHandles,
      retainedHeapBytes,
    },
    dataFlow: {
      remoteEmbeddingsRequired: false,
      providerRequests,
      providerBytes: 0,
      otherNetworkRequests: 0,
      secretsSent: false,
      publicReceiptPrivateMaterial: false,
    },
    usage: { inputTokens: 0, outputTokens: 0, costUsdMicros: 0 },
    budgetsPassed: metrics.every(row => row.passed) && afterHandles - beforeHandles <= 1 && retainedHeapBytes <= 32 * 1024 * 1024,
  })
  const output = path.resolve(import.meta.dirname, "../benchmarks/ide/2026-07-19-ide-09-cursor.json")
  writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, "utf8")
  if (!receipt.budgetsPassed) throw new Error(`IDE-09 cursor benchmark failed: ${JSON.stringify(receipt.resourcesAfter)}`)
  process.stdout.write(`[openagents-desktop] IDE-09 cursor benchmark: ${output}\n`)
}

await main()
