import { describe, expect, test } from 'vitest'

import {
  BUNDLED_GYM_EXPERIMENT,
  GYM_ENVIRONMENT_REGISTRY,
  KHALA_CODE_GYM_EXPERIMENT,
  LONG_CONTEXT_CODEBASE_QA_GYM_EXPERIMENT,
  M8_HEAD_TO_HEAD_GYM_EXPERIMENT,
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  PHASE_1_GYM_ENVIRONMENT_FIXTURE_EXPERIMENTS,
  TERMINAL_BENCH_GYM_EXPERIMENT,
  THROUGHPUT_CONCURRENCY_GYM_EXPERIMENT,
  compileGymExperiment,
  decodeGymExperiment,
  encodeGymExperiment,
  getGymEnvironmentDefinition,
  listGymEnvironmentDefinitions,
  runGymFixtureExperiment,
} from './experiment'

describe('OpenAgents Gym experiment schema', () => {
  test('round-trips the bundled fixture experiment', () => {
    const decoded = decodeGymExperiment(BUNDLED_GYM_EXPERIMENT)
    const encoded = encodeGymExperiment(decoded)
    expect(encoded).toEqual(BUNDLED_GYM_EXPERIMENT)
  })

  test('round-trips the OpenCode head-to-head fixture experiment', () => {
    const decoded = decodeGymExperiment(OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT)
    const encoded = encodeGymExperiment(decoded)
    expect(encoded).toEqual(OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT)
  })

  test('round-trips the Phase 1 environment fixture experiments', () => {
    for (const experiment of PHASE_1_GYM_ENVIRONMENT_FIXTURE_EXPERIMENTS) {
      const decoded = decodeGymExperiment(experiment)
      const encoded = encodeGymExperiment(decoded)
      expect(encoded).toEqual(experiment)
    }
  })

  test('round-trips the throughput/concurrency fixture experiment', () => {
    const decoded = decodeGymExperiment(THROUGHPUT_CONCURRENCY_GYM_EXPERIMENT)
    const encoded = encodeGymExperiment(decoded)
    expect(encoded).toEqual(THROUGHPUT_CONCURRENCY_GYM_EXPERIMENT)
  })

  test('rejects malformed configs at the schema boundary', () => {
    expect(() =>
      decodeGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        environment: 'unknown-environment',
      }),
    ).toThrow()
    expect(() =>
      decodeGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        samplesPerCell: 4,
      }),
    ).toThrow()
  })
})

describe('Gym environment registry', () => {
  test('registers the Phase 1 environments with task sets, verifiers, contracts, and default shapes', () => {
    expect(
      listGymEnvironmentDefinitions().map(definition => definition.ref),
    ).toEqual([
      'bundled-decision-suite-v1',
      'opencode-head-to-head-v1',
      'terminal-bench',
      'khala-code',
      'long-context-codebase-qa',
      'm8-head-to-head',
      'throughput-concurrency',
    ])

    const terminalBench = getGymEnvironmentDefinition('terminal-bench')
    expect(terminalBench.taskSet.harborDataset).toBe('terminal-bench@2.0')
    expect(terminalBench.taskSet.publicSafeTaskRefs).toContain(
      'configure-git-webserver',
    )
    expect(terminalBench.verifier.mode).toBe('harbor-separate')
    expect(terminalBench.acceptance.verifierRef).toBe(
      terminalBench.verifier.ref,
    )

    const throughput = getGymEnvironmentDefinition('throughput-concurrency')
    expect(throughput.surface).toBe('throughput-concurrency')
    expect(throughput.verifier.mode).toBe('telemetry-reconciliation')
    expect(throughput.acceptance.requiresExecutedVerifier).toBe(false)
    expect(throughput.defaultShapes.map(shape => shape.concurrency)).toEqual([
      1, 2, 4, 8,
    ])

    for (const definition of listGymEnvironmentDefinitions()) {
      expect(definition.verifier.ref).not.toBe('')
      expect(definition.acceptance.ref).not.toBe('')
      expect(definition.acceptance.verifierRef).toBe(definition.verifier.ref)
      expect(definition.workloads.length).toBeGreaterThan(0)
      expect(definition.defaultShapes.length).toBeGreaterThan(0)
    }
  })
})

describe('compileGymExperiment', () => {
  test('expands the bundled fixture experiment to the expected matrix cells', () => {
    const compiled = compileGymExperiment(BUNDLED_GYM_EXPERIMENT)
    // 4 lanes × 4 workloads × 3 shapes × 1 transport × 1 sampling.
    expect(compiled.expectedCellCount).toBe(48)
    expect(compiled.matrixConfig.id).toBe('gym:gym-fixture-decision-suite-v1')
    expect(compiled.matrixConfig.samplesPerCell).toBe(5)
    expect(compiled.policySelection.environment.verifierRef).toBe(
      'verifier.fixture.khala-decision-suite.v1',
    )
    expect(compiled.policySelection.coordinator).toBe('heuristic-v0')
    expect(compiled.policySelection.fanout.lanes).toEqual([
      'fireworks',
      'vertex-anthropic',
      'pylon-whole-small',
      'psionic-shard-wan',
    ])
    expect(compiled.policySelection.skippedCells.length).toBe(24)
  })

  test('expands the OpenCode fixture experiment to Khala vs BigPickle', () => {
    const compiled = compileGymExperiment(OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT)
    expect(compiled.expectedCellCount).toBe(2)
    expect(compiled.matrixConfig.workloads).toEqual(['opencode-coding-task'])
    expect(compiled.policySelection.fanout.lanes).toEqual([
      'khala',
      'bigpickle',
    ])
    expect(compiled.policySelection.tools).toBe('opencode-client-tools')
    expect(compiled.policySelection.skippedCells).toEqual([])
  })

  test('expands each Phase 1 environment through its registered verifier binding', () => {
    const expectations = [
      {
        experiment: TERMINAL_BENCH_GYM_EXPERIMENT,
        workload: 'verifier-run',
        verifier: 'verifier.harbor.terminal_bench.summary.v1',
      },
      {
        experiment: KHALA_CODE_GYM_EXPERIMENT,
        workload: 'khala-code-artifact-gen',
        verifier: 'verifier.khala_code.executed_acceptance_suite.v1',
      },
      {
        experiment: LONG_CONTEXT_CODEBASE_QA_GYM_EXPERIMENT,
        workload: 'long-context-codebase-question',
        verifier: 'verifier.seeded.long_context_answer.v1',
      },
      {
        experiment: M8_HEAD_TO_HEAD_GYM_EXPERIMENT,
        workload: 'khala-code-artifact-gen',
        verifier: 'verifier.khala_code.executed_acceptance_suite.v1',
      },
    ] as const

    for (const { experiment, workload, verifier } of expectations) {
      const compiled = compileGymExperiment(experiment)
      expect(compiled.matrixConfig.workloads).toEqual([workload])
      expect(compiled.matrixConfig.shapes).toEqual(
        GYM_ENVIRONMENT_REGISTRY[experiment.environment].defaultShapes,
      )
      expect(compiled.policySelection.environment.verifierRef).toBe(verifier)
      expect(compiled.policySelection.environment.acceptanceContractRef).toBe(
        GYM_ENVIRONMENT_REGISTRY[experiment.environment].acceptance.ref,
      )
    }
  })

  test('expands the throughput/concurrency environment as a typed ramp', () => {
    const compiled = compileGymExperiment(THROUGHPUT_CONCURRENCY_GYM_EXPERIMENT)

    expect(compiled.matrixConfig.workloads).toEqual(['chat'])
    expect(compiled.matrixConfig.shapes.map(shape => shape.concurrency)).toEqual(
      [1, 2, 4, 8],
    )
    expect(compiled.policySelection.fanout.lanes).toEqual([
      'gpt-oss-20b',
      'glm-52',
    ])
    expect(compiled.policySelection.environment.verifierRef).toBe(
      'verifier.gym.throughput.telemetry_reconciliation.v1',
    )
    expect(compiled.policySelection.serving.speculation).toEqual({
      mode: 'ngram',
      draftModelRef: 'glm-52.mtp2',
    })
  })

  test('compiles a real-seam experiment without spending', () => {
    const compiled = compileGymExperiment({
      ...BUNDLED_GYM_EXPERIMENT,
      budget: {
        ...BUNDLED_GYM_EXPERIMENT.budget,
        seam: 'real',
        ownerApprovalRef: 'approval:future',
      },
    })

    expect(compiled.matrixConfig.id).toBe('gym:gym-fixture-decision-suite-v1')
  })

  test('rejects empty fanout and empty shapes with typed compile errors', () => {
    expect(() =>
      compileGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        policy: {
          ...BUNDLED_GYM_EXPERIMENT.policy,
          fanout: {
            ...BUNDLED_GYM_EXPERIMENT.policy.fanout,
            lanes: [],
          },
        },
      }),
    ).toThrow(/fanout/)
    expect(() =>
      compileGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        shapes: [],
      }),
    ).toThrow(/shapes/)
  })

  test('rejects an environment registry entry without a bound verifier', () => {
    const brokenRegistry = {
      ...GYM_ENVIRONMENT_REGISTRY,
      'terminal-bench': {
        ...GYM_ENVIRONMENT_REGISTRY['terminal-bench'],
        verifier: {
          ...GYM_ENVIRONMENT_REGISTRY['terminal-bench'].verifier,
          ref: '',
        },
      },
    }

    expect(() =>
      compileGymExperiment(TERMINAL_BENCH_GYM_EXPERIMENT, brokenRegistry),
    ).toThrow(/verifier/)
  })
})

describe('runGymFixtureExperiment', () => {
  test('is deterministic: same config produces byte-identical records and report', () => {
    const a = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    const b = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    expect(JSON.stringify(a.runSet)).toBe(JSON.stringify(b.runSet))
    expect(JSON.stringify(a.report)).toBe(JSON.stringify(b.report))
  })

  test('produces a public-safe illustrative report, not a decision-grade claim', () => {
    const result = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    expect(result.runSet.seamId).toBe('fixture')
    expect(result.runSet.seamCanSpend).toBe(false)
    expect(result.report.decisionGrade).toBe(false)
    expect(result.report.illustrativeNotice).toContain('ILLUSTRATIVE ONLY')
    expect(result.publicSafety.safe).toBe(true)
    expect(result.publicSafety.violations).toEqual([])
  })

  test('runs the OpenCode Khala vs BigPickle fixture through the report path', () => {
    const result = runGymFixtureExperiment(OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT)
    const khala = result.report.groups.find(
      group =>
        group.lane === 'khala' && group.workload === 'opencode-coding-task',
    )
    const bigpickle = result.report.groups.find(
      group =>
        group.lane === 'bigpickle' &&
        group.workload === 'opencode-coding-task',
    )

    expect(result.report.decisionGrade).toBe(false)
    expect(khala?.toolCallSuccessRate).toBe(1)
    expect(khala?.verificationRate).toBe(1)
    expect(bigpickle?.toolCallSuccessRate).toBeCloseTo(2 / 3, 6)
    expect(bigpickle?.verificationRate).toBe(0)
    expect(result.publicSafety.safe).toBe(true)
  })

  test('runs every Phase 1 environment fixture with its grader bound', () => {
    for (const experiment of PHASE_1_GYM_ENVIRONMENT_FIXTURE_EXPERIMENTS) {
      const result = runGymFixtureExperiment(experiment)
      const environment = result.compiled.policySelection.environment
      expect(environment.verifierRef).toBe(
        GYM_ENVIRONMENT_REGISTRY[experiment.environment].verifier.ref,
      )
      expect(environment.acceptanceContractRef).toBe(
        GYM_ENVIRONMENT_REGISTRY[experiment.environment].acceptance.ref,
      )
      expect(result.runSet.seamId).toBe('fixture')
      expect(result.report.decisionGrade).toBe(false)
      expect(result.publicSafety.safe).toBe(true)
    }
  })
})
