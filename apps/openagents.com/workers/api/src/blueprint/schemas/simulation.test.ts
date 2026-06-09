import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintSimulationBranch,
  blueprintScenarioForkHasProductionEffects,
  blueprintSimulationBranchHasProductionEffects,
  blueprintSimulationBranchProjection,
  BlueprintSimulationBranch as BlueprintSimulationBranchSchema,
} from './simulation'

const branch: BlueprintSimulationBranch = {
  createdAt: '2026-06-05T00:00:00.000Z',
  id: 'simulation_branch.autonomy_promotion_1',
  purpose: 'autonomy_promotion',
  rollbackEvidenceRefs: ['receipt.rollback_plan_1'],
  scenarioForks: [
    {
      baseStateRef: 'state.production_snapshot_1',
      effectIsolation: 'simulated_only',
      forkStateRef: 'state.simulated_fork_1',
      id: 'scenario_fork.autonomy_promotion_1',
      productionEffectRefs: [],
      scenarioRef: 'scenario.first_batch_sites_continue',
      simulatedEffectRefs: ['simulated_effect.deploy_candidate'],
      simulationBranchId: 'simulation_branch.autonomy_promotion_1',
    },
  ],
  status: 'passed',
  targetRef: 'release_gate.autopilot.continue.v1',
  updatedAt: '2026-06-05T00:00:00.000Z',
}

describe('Blueprint Simulation Branch and Scenario Fork schemas', () => {
  test('decodes isolated simulation branches', () => {
    expect(S.decodeUnknownSync(BlueprintSimulationBranchSchema)(branch)).toEqual(
      branch,
    )
  })

  test('projects no-production-effect state for isolated forks', () => {
    expect(blueprintSimulationBranchProjection(branch)).toEqual({
      id: 'simulation_branch.autonomy_promotion_1',
      noProductionEffects: true,
      purpose: 'autonomy_promotion',
      scenarioForkCount: 1,
      status: 'passed',
      targetRef: 'release_gate.autopilot.continue.v1',
    })
    expect(blueprintSimulationBranchHasProductionEffects(branch)).toBe(false)
  })

  test('detects production-effect leakage in scenario forks', () => {
    const leakingFork = {
      ...branch.scenarioForks[0]!,
      productionEffectRefs: ['production_effect.deploy_live'],
    }

    expect(blueprintScenarioForkHasProductionEffects(leakingFork)).toBe(true)
    expect(
      blueprintSimulationBranchHasProductionEffects({
        ...branch,
        scenarioForks: [leakingFork],
      }),
    ).toBe(true)
  })
})
