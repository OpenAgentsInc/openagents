import { Schema as S } from 'effect'

export const BlueprintSimulationPurpose = S.Literals([
  'autonomy_promotion',
  'destructive_action_suite',
  'migration',
  'risky_workflow',
])
export type BlueprintSimulationPurpose =
  typeof BlueprintSimulationPurpose.Type

export const BlueprintSimulationStatus = S.Literals([
  'draft',
  'running',
  'passed',
  'failed',
  'archived',
])
export type BlueprintSimulationStatus =
  typeof BlueprintSimulationStatus.Type

export const BlueprintEffectIsolation = S.Literals([
  'simulated_only',
])
export type BlueprintEffectIsolation = typeof BlueprintEffectIsolation.Type

export const BlueprintScenarioFork = S.Struct({
  baseStateRef: S.String,
  effectIsolation: BlueprintEffectIsolation,
  forkStateRef: S.String,
  id: S.String,
  productionEffectRefs: S.Array(S.String),
  scenarioRef: S.String,
  simulatedEffectRefs: S.Array(S.String),
  simulationBranchId: S.String,
})
export type BlueprintScenarioFork = typeof BlueprintScenarioFork.Type

export const BlueprintSimulationBranch = S.Struct({
  createdAt: S.String,
  id: S.String,
  purpose: BlueprintSimulationPurpose,
  rollbackEvidenceRefs: S.Array(S.String),
  scenarioForks: S.Array(BlueprintScenarioFork),
  status: BlueprintSimulationStatus,
  targetRef: S.String,
  updatedAt: S.String,
})
export type BlueprintSimulationBranch =
  typeof BlueprintSimulationBranch.Type

export const blueprintScenarioForkHasProductionEffects = (
  fork: BlueprintScenarioFork,
): boolean =>
  fork.effectIsolation !== 'simulated_only' ||
  fork.productionEffectRefs.length > 0

export const blueprintSimulationBranchHasProductionEffects = (
  branch: BlueprintSimulationBranch,
): boolean =>
  branch.scenarioForks.some(fork =>
    blueprintScenarioForkHasProductionEffects(fork),
  )

export const blueprintSimulationBranchProjection = (
  branch: BlueprintSimulationBranch,
) => ({
  id: branch.id,
  noProductionEffects: !blueprintSimulationBranchHasProductionEffects(branch),
  purpose: branch.purpose,
  scenarioForkCount: branch.scenarioForks.length,
  status: branch.status,
  targetRef: branch.targetRef,
})
