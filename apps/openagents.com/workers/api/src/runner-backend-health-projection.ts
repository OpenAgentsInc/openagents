import { Schema as S } from 'effect'

import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerProjectionAudience,
} from './runner-backends'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsRunnerBackendAvailability = S.Literals([
  'available',
  'blocked',
  'degraded',
  'disabled',
  'unknown',
])
export type OpenAgentsRunnerBackendAvailability =
  typeof OpenAgentsRunnerBackendAvailability.Type

export const OpenAgentsRunnerBackendHealthGateKind = S.Literals([
  'billing',
  'capacity',
  'configured',
  'enabled',
  'health',
  'operator_approval',
  'staging_smoke',
  'workload_trust',
])
export type OpenAgentsRunnerBackendHealthGateKind =
  typeof OpenAgentsRunnerBackendHealthGateKind.Type

export const OpenAgentsRunnerBackendHealthGate = S.Struct({
  gateKind: OpenAgentsRunnerBackendHealthGateKind,
  operatorDiagnosticRef: S.String,
  passed: S.Boolean,
  publicCaveatRef: S.String,
})
export type OpenAgentsRunnerBackendHealthGate =
  typeof OpenAgentsRunnerBackendHealthGate.Type

export const OpenAgentsRunnerBackendHealthSnapshot = S.Struct({
  availability: OpenAgentsRunnerBackendAvailability,
  backendKind: OpenAgentsRunnerBackendKind,
  billingCaveatRefs: S.Array(S.String),
  capacityRefs: S.Array(S.String),
  coldStartRefs: S.Array(S.String),
  configured: S.Boolean,
  costTierRefs: S.Array(S.String),
  enabled: S.Boolean,
  gates: S.Array(OpenAgentsRunnerBackendHealthGate),
  healthRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  queueDepthRefs: S.Array(S.String),
  smokeRefs: S.Array(S.String),
})
export type OpenAgentsRunnerBackendHealthSnapshot =
  typeof OpenAgentsRunnerBackendHealthSnapshot.Type

export const OpenAgentsRunnerBackendHealthProjection = S.Struct({
  audience: OpenAgentsRunnerProjectionAudience,
  availability: OpenAgentsRunnerBackendAvailability,
  backendKind: OpenAgentsRunnerBackendKind,
  billingCaveatRefs: S.Array(S.String),
  capacityRefs: S.Array(S.String),
  coldStartRefs: S.Array(S.String),
  configured: S.Boolean,
  costTierRefs: S.Array(S.String),
  enabled: S.Boolean,
  gateRefs: S.Array(S.String),
  healthRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  queueDepthRefs: S.Array(S.String),
  smokeRefs: S.Array(S.String),
})
export type OpenAgentsRunnerBackendHealthProjection =
  typeof OpenAgentsRunnerBackendHealthProjection.Type

const unsafeProjectionRefPattern =
  /(bearer\s+|callback[_-]?token[_-]?(raw|secret|value)|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|failover|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|github[_-]?token(?![_-]?ref)|oauth|openagents_admin|password|preimage|private[_-]?key|provider[_-]?payload|provider[_-]?token|raw[_-]?email|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?secret|\S+@\S+)/i

const safeRef = (ref: string): boolean =>
  ref.trim() !== '' &&
  !unsafeProjectionRefPattern.test(ref) &&
  !openAgentsRunnerGatewayPayloadHasPrivateMaterial(ref)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(safeRef)

const publicGateRefs = (
  gates: ReadonlyArray<OpenAgentsRunnerBackendHealthGate>,
): ReadonlyArray<string> =>
  safeRefs(
    gates
      .filter(gate => !gate.passed)
      .map(gate => gate.publicCaveatRef),
  )

const operatorGateRefs = (
  gates: ReadonlyArray<OpenAgentsRunnerBackendHealthGate>,
): ReadonlyArray<string> =>
  safeRefs(
    gates.map(
      gate =>
        `gate.runner_backend.${gate.gateKind}.${gate.passed ? 'passed' : 'blocked'}`,
    ),
  )

export const projectOpenAgentsRunnerBackendHealth = (
  snapshot: OpenAgentsRunnerBackendHealthSnapshot,
  audience: typeof OpenAgentsRunnerProjectionAudience.Type,
): OpenAgentsRunnerBackendHealthProjection => {
  const operator = audience === 'operator'

  return {
    audience,
    availability: snapshot.availability,
    backendKind: snapshot.backendKind,
    billingCaveatRefs: operator
      ? safeRefs(snapshot.billingCaveatRefs)
      : publicGateRefs(snapshot.gates),
    capacityRefs: operator ? safeRefs(snapshot.capacityRefs) : [],
    coldStartRefs: operator ? safeRefs(snapshot.coldStartRefs) : [],
    configured: operator ? snapshot.configured : false,
    costTierRefs: operator ? safeRefs(snapshot.costTierRefs) : [],
    enabled: operator ? snapshot.enabled : false,
    gateRefs: operator
      ? operatorGateRefs(snapshot.gates)
      : publicGateRefs(snapshot.gates),
    healthRefs: operator ? safeRefs(snapshot.healthRefs) : [],
    operatorDiagnosticRefs: operator
      ? safeRefs(snapshot.operatorDiagnosticRefs)
      : [],
    publicSummaryRef: safeRefs([snapshot.publicSummaryRef])[0] ??
      'runner_backend_health.summary.redacted',
    queueDepthRefs: operator ? safeRefs(snapshot.queueDepthRefs) : [],
    smokeRefs: operator ? safeRefs(snapshot.smokeRefs) : [],
  }
}

export const openAgentsRunnerBackendHealthProjectionHasPrivateMaterial = (
  projection: OpenAgentsRunnerBackendHealthProjection,
): boolean =>
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(projection) ||
  /failover/i.test(JSON.stringify(projection))
