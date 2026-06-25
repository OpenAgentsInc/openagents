import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import { TERMINAL_BENCH_GYM_EXPERIMENT } from './experiment'
import {
  GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA,
  HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  buildGymHarborTerminalBenchJobSpec,
  checkHydraliskTerminalBenchSummaryPublicSafety,
  dispatchGymHarborTerminalBenchRun,
  type GymHarborTerminalBenchDispatchReceipt,
  type GymHarborTerminalBenchJobSpec,
  type HydraliskHarborTerminalBenchHarness,
  type HydraliskTerminalBenchSummary,
} from './harbor-dispatch'

const sampleSummary: HydraliskTerminalBenchSummary = {
  schema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  createdAt: '2026-06-25T00:00:00Z',
  publicSafe: true,
  benchmark: {
    name: 'Terminal-Bench',
    datasetRef: 'terminal-bench@2.0',
    version: '2.0',
    repository: 'https://github.com/harbor-framework/terminal-bench-2',
    harnessRepository: 'https://github.com/harbor-framework/harbor',
  },
  runner: {
    name: 'harbor',
    version: '0.15.0',
    agent: 'terminus-2',
    model: 'openagents/khala',
    nConcurrent: 1,
    timeoutSeconds: 3600,
    maxAttempts: 5,
    retryPolicy: 'pass@1 plus up to 4 queued retries for pass@5',
  },
  model: {
    alias: 'openagents/khala',
    profileRef: 'profiles/openagents-khala.terminal-bench.v1',
    revision: 'openagents/khala@public',
    hardwareProfile: 'Hydralisk benchmark harness',
  },
  sampler: {
    minP: 0.05,
    repetitionPenalty: 1.05,
    maxTokens: 1024,
    enableThinking: false,
  },
  counts: {
    solved: 3,
    failing: 1,
    envBroken: 1,
    notStarted: 0,
    total: 5,
    attempted: 5,
    properlyAttempted: 4,
  },
  rates: {
    fullDenominatorSolved: 0.6,
    attemptedSolved: 0.6,
    properlyAttemptedSolved: 0.75,
    knownPassAt1: 0.4,
    passAtN: 0.6,
  },
  passAt: {
    passAt1Solved: 2,
    passAt1KnownTasks: 5,
    passAtNAnySolved: 3,
    maxAttempts: 5,
  },
  denominatorDefinitions: {
    total: 'all Terminal-Bench 2.0 task IDs in the run set',
    attempted: 'total minus not-started tasks',
    properlyAttempted: 'attempted minus environment-broken tasks',
    fullDenominatorSolved: 'solved / total',
    attemptedSolved: 'solved / attempted',
    properlyAttemptedSolved: 'solved / properlyAttempted',
  },
  taskIds: {
    envBroken: ['qemu-alpine-ssh'],
    notStarted: [],
    notableSolved: ['configure-git-webserver', 'pypi-server'],
  },
  claimStatus: 'preliminary_pilot_partial',
  inputSha256: 'a'.repeat(64),
  comparisonBoundary:
    'Do not compare publicly unless benchmark version, Harbor version, agent, model alias, sampler settings, retry policy, timeout, and denominator definitions are all named.',
  publicSafety: {
    containsSecrets: false,
    containsPrompts: false,
    containsResponses: false,
    containsHiddenReasoning: false,
    containsPrivateSource: false,
    containsRawBenchmarkLogs: false,
  },
}

describe('Gym Harbor Terminal-Bench dispatch seam', () => {
  test('builds a public-safe Hydralisk Harbor job spec for openagents/khala', () => {
    const { compiled, job } = buildGymHarborTerminalBenchJobSpec(
      TERMINAL_BENCH_GYM_EXPERIMENT,
      { ownerApprovalRef: 'approval.gym.harbor.terminal_bench.001' },
    )

    expect(compiled.policySelection.environment.ref).toBe('terminal-bench')
    expect(job.harborDataset).toBe('terminal-bench@2.0')
    expect(job.harborDatasetCliRef).toBe('terminal-bench/terminal-bench-2')
    expect(job.model).toBe('openagents/khala')
    expect(job.command.argv).toEqual([
      'run',
      '-d',
      'terminal-bench/terminal-bench-2',
      '--agent',
      'terminus-2',
      '--model',
      'openagents/khala',
      '--n-concurrent',
      '1',
    ])
    expect(job.artifacts.requestPublicSafeSummary).toBe(true)
    expect(job.artifacts.requestAtifTrajectory).toBe(true)
    expect(job.artifacts.requestRawHarborLogs).toBe(false)
    expect(job.publicSafetyBoundary.workerImportsHarborRuntime).toBe(false)
    expect(job.retainedPublicTaskRefs).toContain('configure-git-webserver')
  })

  test('dispatches to an injected Hydralisk harness and ingests the summary', async () => {
    const observedJobs: Array<GymHarborTerminalBenchJobSpec> = []
    const harness: HydraliskHarborTerminalBenchHarness = {
      async dispatchTerminalBenchJob(
        job,
      ): Promise<GymHarborTerminalBenchDispatchReceipt> {
        observedJobs.push(job)
        return {
          schemaVersion:
            'openagents.gym.harbor_terminal_bench_dispatch_receipt.v1',
          jobRef: job.jobRef,
          hydraliskRunRef: 'hydralisk.run.terminal_bench.khala.001',
          state: 'completed',
          summaryArtifactRef:
            'artifact.hydralisk.terminal_bench.khala.summary.001',
          atifTraceRef: 'trace.hydralisk.terminal_bench.khala.001',
          rawHarborArtifactRef: null,
        }
      },
      async readTerminalBenchSummary() {
        return sampleSummary
      },
    }

    const result = await dispatchGymHarborTerminalBenchRun(
      TERMINAL_BENCH_GYM_EXPERIMENT,
      {
        harness,
        ownerApprovalRef: 'approval.gym.harbor.terminal_bench.001',
      },
    )

    expect(observedJobs[0]?.model).toBe('openagents/khala')
    expect(result.dispatch.state).toBe('completed')
    expect(result.summary.schema).toBe(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA)
    expect(result.publicSafety).toEqual({ safe: true, violations: [] })
    expect(result.ingest.schemaVersion).toBe(
      GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA,
    )
    expect(result.ingest.acceptedOutcomes).toBe(3)
    expect(result.ingest.attemptedOutcomes).toBe(4)
    expect(result.ingest.publicClaimEligible).toBe(false)
    expect(result.ingest.decisionGradeReportReady).toBe(false)
    expect(result.ingest.caveats).toContain(
      'cost_per_accepted_outcome_mapping_deferred_to_6242',
    )
  })

  test('rejects summaries that fail the Hydralisk public-safety boundary', () => {
    const unsafeSummary = {
      ...sampleSummary,
      publicSafety: {
        ...sampleSummary.publicSafety,
        containsPrompts: true,
      },
    } as unknown as HydraliskTerminalBenchSummary

    expect(
      checkHydraliskTerminalBenchSummaryPublicSafety(unsafeSummary),
    ).toEqual({
      safe: false,
      violations: ['public_safety.containsPrompts'],
    })
  })

  test('does not import Harbor runtime code into the Worker bundle', () => {
    const source = readFileSync(
      new URL('./harbor-dispatch.ts', import.meta.url),
      'utf8',
    )
    expect(source).not.toMatch(/from ['"][^'"]*harbor[^'"]*['"]/)
    expect(source).not.toMatch(/import\([^)]*harbor[^)]*\)/)
  })
})
