import { describe, expect, test } from "vite-plus/test"
import { Schema } from "effect"

import {
  IdeRunBenchmarkMetricSchema,
  IdeRunBenchmarkReceiptSchema,
  IdeRunTargetFactSchema,
} from "./run-benchmark-contract.ts"

describe("IDE-10 benchmark contract", () => {
  test("requires percentile, output-loss, resource, and six-target native-boundary facts", () => {
    const metric = IdeRunBenchmarkMetricSchema.make({
      name: "fixture",
      unit: "milliseconds",
      repetitions: 1,
      warmup: 0,
      p50: 1,
      p95: 1,
      p99: 1,
      thresholdP95: 2,
      thresholdP99: 3,
      passed: true,
    })
    const targets = [
      "macos-arm64", "macos-x64", "windows-arm64", "windows-x64", "linux-arm64", "linux-x64",
    ].map((target) => Schema.decodeUnknownSync(IdeRunTargetFactSchema)({
      target,
      nativeHelper: false,
      typescriptFallback: true,
      disposition: "not_claimed_native_helper_unnecessary",
    }))
    const receipt = Schema.decodeUnknownSync(IdeRunBenchmarkReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-run-benchmark.v1",
      issue: "IDE-10",
      measuredAt: "2026-07-19T12:00:00.000Z",
      candidateCommitSha: "a".repeat(40),
      environment: {
        platform: "darwin",
        architecture: "arm64",
        node: "v24",
        shell: "zsh",
        runtime: "Effect v4 + Node child_process",
        corpus: "deterministic declared-task and output fixture",
      },
      metrics: Array.from({ length: 8 }, (_, index) => ({ ...metric, name: `fixture-${index}` })),
      outputFacts: {
        sequenceMonotonic: true,
        boundedRetention: true,
        gapAccounted: true,
        redactionObserved: true,
        invalidEncodingAccounted: true,
        rendererReceivesEnvironmentValues: false,
        inheritedAllHostVariables: false,
      },
      resources: { activeHandlesDelta: 0, heapDeltaBytes: 0, runningProcessesAfter: 0, subscriptionsAfter: 0 },
      nativeDecision: {
        rustAdmitted: false,
        reason: "The measured TypeScript fallback passes, so no native helper is admitted.",
        targets,
      },
      passed: true,
    })
    expect(receipt.nativeDecision.targets).toHaveLength(6)
    expect(receipt.outputFacts.rendererReceivesEnvironmentValues).toBe(false)
  })
})
