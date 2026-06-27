import { createHash } from 'node:crypto'
import { Schema as S } from 'effect'

export const GLM_NVFP4_PILOT_RESULT_SCHEMA =
  'openagents.khala.glm_nvfp4_pilot_result.v1' as const
export const GLM_NVFP4_PILOT_MODEL = 'nvidia/GLM-5.2-NVFP4' as const
export const GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES = 20
export const GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS = 47
export const GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME =
  'public_fixture_return_number' as const

export const GLM_NVFP4_EXACT_VLLM_FLAGS = [
  { name: '--tensor-parallel-size', value: '8' },
  { name: '--quantization', value: 'modelopt_fp4' },
  { name: '--tool-call-parser', value: 'glm47' },
  { name: '--reasoning-parser', value: 'glm45' },
  { name: '--trust-remote-code' },
  { name: '--chunked-prefill-size', value: '131072' },
  { name: '--mem-fraction-static', value: '0.80' },
] as const

export const GlmNvfp4PilotBlocker = S.Literals([
  'owner_arm_missing',
  'owner_approval_ref_missing',
  'owner_approval_ref_unsafe',
  'endpoint_ref_missing',
  'endpoint_ref_unsafe',
  'endpoint_url_missing',
  'model_mismatch',
  'boot_load_evidence_ref_missing',
  'boot_load_evidence_ref_unsafe',
  'boot_load_failed',
  'serving_stack_evidence_ref_unsafe',
  'measured_max_model_len_missing',
  'measured_max_model_len_evidence_ref_missing',
  'measured_max_model_len_evidence_ref_unsafe',
  'tool_loop_evidence_missing',
  'tool_loop_evidence_ref_unsafe',
  'tool_loop_sample_count_too_low',
  'tool_loop_provider_error',
  'tool_loop_missing_tool_calls',
  'quality_parity_missing',
  'quality_evidence_ref_missing',
  'quality_evidence_ref_unsafe',
  'tps_measurement_missing',
  'tps_evidence_ref_unsafe',
  'tps_not_finite',
  'decision_ref_missing',
  'decision_ref_unsafe',
  'unsafe_public_ref',
])
export type GlmNvfp4PilotBlocker = typeof GlmNvfp4PilotBlocker.Type

export const GlmNvfp4PilotDecision = S.Literals(['go', 'no_go'])
export type GlmNvfp4PilotDecision = typeof GlmNvfp4PilotDecision.Type

export const GlmNvfp4BootLoadStatus = S.Literals([
  'not_attempted',
  'failed',
  'passed',
])
export type GlmNvfp4BootLoadStatus = typeof GlmNvfp4BootLoadStatus.Type

export const GlmNvfp4ServingStackEngine = S.Literals(['vllm', 'sglang'])
export type GlmNvfp4ServingStackEngine =
  typeof GlmNvfp4ServingStackEngine.Type

export const GlmNvfp4ServingStackAttemptStatus = S.Literals([
  'not_attempted',
  'failed_before_endpoint',
  'endpoint_healthy',
])
export type GlmNvfp4ServingStackAttemptStatus =
  typeof GlmNvfp4ServingStackAttemptStatus.Type

export const GlmNvfp4ServingStackFailureCode = S.Literals([
  'vllm_sparse_mla_backend_unavailable',
  'sglang_moe_w13_shape_mismatch',
  'unknown',
])
export type GlmNvfp4ServingStackFailureCode =
  typeof GlmNvfp4ServingStackFailureCode.Type

export const GlmNvfp4ServingStackFinding = S.Struct({
  engine: GlmNvfp4ServingStackEngine,
  status: GlmNvfp4ServingStackAttemptStatus,
  failureCode: S.Union([GlmNvfp4ServingStackFailureCode, S.Null]),
  evidenceRef: S.Union([S.String, S.Null]),
})
export type GlmNvfp4ServingStackFinding =
  typeof GlmNvfp4ServingStackFinding.Type

export const GlmNvfp4PilotIssueGate = S.Literals([
  'isolated_owner_armed_endpoint_context',
  'tool_loop_proof',
  'quality_parity',
  'throughput_context_tradeoff',
])
export type GlmNvfp4PilotIssueGate = typeof GlmNvfp4PilotIssueGate.Type

export const GlmNvfp4PilotIssueGateStatus = S.Literals(['passed', 'blocked'])
export type GlmNvfp4PilotIssueGateStatus =
  typeof GlmNvfp4PilotIssueGateStatus.Type

export const GlmNvfp4PilotEvidenceRefField = S.Literals([
  'ownerApprovalRef',
  'endpointRef',
  'decisionRef',
  'bootLoadEvidenceRef',
  'measuredMaxModelLenEvidenceRef',
  'qualityEvidenceRef',
  'toolLoopEvidenceRef',
  'throughputEvidenceRef',
])
export type GlmNvfp4PilotEvidenceRefField =
  typeof GlmNvfp4PilotEvidenceRefField.Type

export const GlmNvfp4PilotEvidenceRefAuditStatus = S.Literals([
  'accepted',
  'missing',
  'rejected_unsafe',
])
export type GlmNvfp4PilotEvidenceRefAuditStatus =
  typeof GlmNvfp4PilotEvidenceRefAuditStatus.Type

export const GlmNvfp4PilotEvidenceRefAudit = S.Struct({
  field: GlmNvfp4PilotEvidenceRefField,
  status: GlmNvfp4PilotEvidenceRefAuditStatus,
  publicRef: S.Union([S.String, S.Null]),
})
export type GlmNvfp4PilotEvidenceRefAudit =
  typeof GlmNvfp4PilotEvidenceRefAudit.Type

export const GlmNvfp4PilotIssueGateSummary = S.Struct({
  gate: GlmNvfp4PilotIssueGate,
  status: GlmNvfp4PilotIssueGateStatus,
  blockerRefs: S.Array(GlmNvfp4PilotBlocker),
  evidenceRefs: S.Array(S.String),
})
export type GlmNvfp4PilotIssueGateSummary =
  typeof GlmNvfp4PilotIssueGateSummary.Type

export const GlmNvfp4PilotVllmFlag = S.Struct({
  name: S.String,
  value: S.optional(S.String),
})
export type GlmNvfp4PilotVllmFlag = typeof GlmNvfp4PilotVllmFlag.Type

export const GlmNvfp4ToolLoopEvidence = S.Struct({
  sampleCount: S.Number,
  providerErrorCount: S.Number,
  toolCallsAttempted: S.Number,
  toolCallsSucceeded: S.Number,
  hallucinatedToolCallCount: S.Number,
  evidenceRef: S.Union([S.String, S.Null]),
})
export type GlmNvfp4ToolLoopEvidence =
  typeof GlmNvfp4ToolLoopEvidence.Type

export const GlmNvfp4ThroughputEvidence = S.Struct({
  outputTokens: S.Number,
  wallClockMs: S.Number,
  measuredTps: S.Union([S.Number, S.Null]),
  reapBaselineTps: S.Number,
  evidenceRef: S.Union([S.String, S.Null]),
})
export type GlmNvfp4ThroughputEvidence =
  typeof GlmNvfp4ThroughputEvidence.Type

export const GlmNvfp4PilotResult = S.Struct({
  schemaVersion: S.Literal(GLM_NVFP4_PILOT_RESULT_SCHEMA),
  generatedAt: S.String,
  issueRef: S.Literal('github.issue.OpenAgentsInc.openagents.6323'),
  publicSafe: S.Literal(true),
  ownerArmed: S.Boolean,
  decision: GlmNvfp4PilotDecision,
  canRouteCodingLane: S.Boolean,
  model: S.Literal(GLM_NVFP4_PILOT_MODEL),
  endpointRef: S.Union([S.String, S.Null]),
  ownerApprovalRef: S.Union([S.String, S.Null]),
  decisionRef: S.Union([S.String, S.Null]),
  bootLoadStatus: GlmNvfp4BootLoadStatus,
  bootLoadEvidenceRef: S.Union([S.String, S.Null]),
  servingStackFindings: S.Array(GlmNvfp4ServingStackFinding),
  measuredMaxModelLen: S.Union([S.Number, S.Null]),
  measuredMaxModelLenEvidenceRef: S.Union([S.String, S.Null]),
  qualityParity: S.Literals(['passed', 'failed', 'not_measured']),
  qualityEvidenceRef: S.Union([S.String, S.Null]),
  runtimeRequirements: S.Struct({
    hostClassRef: S.Literal('g4-standard-384.8x-rtx-pro-6000.blackwell'),
    tensorParallelSize: S.Literal(8),
    transformersMinimumVersion: S.Literal('5.3.0'),
    isolatedEndpointRequired: S.Literal(true),
  }),
  vllmFlags: S.Array(GlmNvfp4PilotVllmFlag),
  toolLoop: GlmNvfp4ToolLoopEvidence,
  throughput: GlmNvfp4ThroughputEvidence,
  routingOutcome: S.Literals([
    'not_recorded',
    'keep_reap_live_lane',
    'route_coding_tools_to_full_model_then_reap_overflow',
  ]),
  blockerRefs: S.Array(GlmNvfp4PilotBlocker),
  evidenceRefAudit: S.Array(GlmNvfp4PilotEvidenceRefAudit),
  evidenceRefs: S.Array(S.String),
  authorityBoundary: S.String,
  contentRedacted: S.Literal(true),
})
export type GlmNvfp4PilotResult = typeof GlmNvfp4PilotResult.Type

export const GlmNvfp4PilotPublicSummary = S.Struct({
  schemaVersion: S.Literal('openagents.khala.glm_nvfp4_pilot_public_summary.v1'),
  generatedAt: S.String,
  issueRef: S.Literal('github.issue.OpenAgentsInc.openagents.6323'),
  publicSafe: S.Literal(true),
  decision: GlmNvfp4PilotDecision,
  canRouteCodingLane: S.Boolean,
  gates: S.Array(GlmNvfp4PilotIssueGateSummary),
  blockerRefs: S.Array(GlmNvfp4PilotBlocker),
  evidenceRefs: S.Array(S.String),
  contentRedacted: S.Literal(true),
})
export type GlmNvfp4PilotPublicSummary =
  typeof GlmNvfp4PilotPublicSummary.Type

export const decodeGlmNvfp4PilotResult = S.decodeUnknownSync(
  GlmNvfp4PilotResult,
)

export const decodeGlmNvfp4PilotPublicSummary = S.decodeUnknownSync(
  GlmNvfp4PilotPublicSummary,
)

export type GlmNvfp4PilotConfig = Readonly<{
  generatedAt: string
  ownerArmed?: boolean
  ownerApprovalRef?: string | null
  endpointUrl?: string | null
  endpointRef?: string | null
  model?: string | null
  decisionRef?: string | null
  bootLoadStatus?: GlmNvfp4BootLoadStatus | null
  bootLoadEvidenceRef?: string | null
  servingStackFindings?: ReadonlyArray<GlmNvfp4ServingStackFinding> | undefined
  measuredMaxModelLen?: number | null
  measuredMaxModelLenEvidenceRef?: string | null
  qualityParity?: 'passed' | 'failed' | 'not_measured'
  qualityEvidenceRef?: string | null
  reapBaselineTps?: number | null
  requiredToolLoopSamples?: number
}>

export type GlmNvfp4PilotObservation = Readonly<{
  toolLoop?: GlmNvfp4ToolLoopEvidence | undefined
  throughput?: GlmNvfp4ThroughputEvidence | undefined
}>

export type GlmNvfp4PilotExecutor = (
  config: GlmNvfp4PilotConfig,
) => Promise<GlmNvfp4PilotObservation>

export class GlmNvfp4PilotNotArmedError extends Error {
  readonly _tag = 'GlmNvfp4PilotNotArmedError'
  constructor() {
    super(
      'GLM NVFP4 pilot is fail-closed until owner arm, endpoint, and public refs are present.',
    )
    this.name = 'GlmNvfp4PilotNotArmedError'
  }
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(:\/\/|\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|cookie|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const nullIfBlank = (value: string | null | undefined): string | null => {
  if (value === null || value === undefined) return null
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const isSafeRef = (value: string): boolean =>
  publicRefPattern.test(value) && !unsafeRefPattern.test(value)

const safeRefOrNull = (value: string | null | undefined): string | null => {
  const ref = nullIfBlank(value)
  return ref !== null && isSafeRef(ref) ? ref : null
}

const evidenceRefAudit = (input: {
  field: GlmNvfp4PilotEvidenceRefField
  rawRef: string | null | undefined
  publicRef: string | null
}): GlmNvfp4PilotEvidenceRefAudit => {
  const normalized = nullIfBlank(input.rawRef)
  return {
    field: input.field,
    status:
      normalized === null
        ? 'missing'
        : input.publicRef === null
          ? 'rejected_unsafe'
          : 'accepted',
    publicRef: input.publicRef,
  }
}

const unsafeBlockerForEvidenceField = (
  field: GlmNvfp4PilotEvidenceRefField,
): GlmNvfp4PilotBlocker => {
  const blockersByField: Record<
    GlmNvfp4PilotEvidenceRefField,
    GlmNvfp4PilotBlocker
  > = {
    ownerApprovalRef: 'owner_approval_ref_unsafe',
    endpointRef: 'endpoint_ref_unsafe',
    decisionRef: 'decision_ref_unsafe',
    bootLoadEvidenceRef: 'boot_load_evidence_ref_unsafe',
    measuredMaxModelLenEvidenceRef:
      'measured_max_model_len_evidence_ref_unsafe',
    qualityEvidenceRef: 'quality_evidence_ref_unsafe',
    toolLoopEvidenceRef: 'tool_loop_evidence_ref_unsafe',
    throughputEvidenceRef: 'tps_evidence_ref_unsafe',
  }
  return blockersByField[field]
}

const GLM_NVFP4_PILOT_ISSUE_GATES: ReadonlyArray<GlmNvfp4PilotIssueGate> = [
  'isolated_owner_armed_endpoint_context',
  'tool_loop_proof',
  'quality_parity',
  'throughput_context_tradeoff',
]

const GLM_NVFP4_PILOT_BLOCKER_ISSUE_GATE: Record<
  GlmNvfp4PilotBlocker,
  GlmNvfp4PilotIssueGate
> = {
  owner_arm_missing: 'isolated_owner_armed_endpoint_context',
  owner_approval_ref_missing: 'isolated_owner_armed_endpoint_context',
  owner_approval_ref_unsafe: 'isolated_owner_armed_endpoint_context',
  endpoint_ref_missing: 'isolated_owner_armed_endpoint_context',
  endpoint_ref_unsafe: 'isolated_owner_armed_endpoint_context',
  endpoint_url_missing: 'isolated_owner_armed_endpoint_context',
  model_mismatch: 'isolated_owner_armed_endpoint_context',
  boot_load_evidence_ref_missing: 'isolated_owner_armed_endpoint_context',
  boot_load_evidence_ref_unsafe: 'isolated_owner_armed_endpoint_context',
  boot_load_failed: 'isolated_owner_armed_endpoint_context',
  serving_stack_evidence_ref_unsafe: 'isolated_owner_armed_endpoint_context',
  measured_max_model_len_missing: 'throughput_context_tradeoff',
  measured_max_model_len_evidence_ref_missing: 'throughput_context_tradeoff',
  measured_max_model_len_evidence_ref_unsafe: 'throughput_context_tradeoff',
  tool_loop_evidence_missing: 'tool_loop_proof',
  tool_loop_evidence_ref_unsafe: 'tool_loop_proof',
  tool_loop_sample_count_too_low: 'tool_loop_proof',
  tool_loop_provider_error: 'tool_loop_proof',
  tool_loop_missing_tool_calls: 'tool_loop_proof',
  quality_parity_missing: 'quality_parity',
  quality_evidence_ref_missing: 'quality_parity',
  quality_evidence_ref_unsafe: 'quality_parity',
  tps_measurement_missing: 'throughput_context_tradeoff',
  tps_evidence_ref_unsafe: 'throughput_context_tradeoff',
  tps_not_finite: 'throughput_context_tradeoff',
  decision_ref_missing: 'isolated_owner_armed_endpoint_context',
  decision_ref_unsafe: 'isolated_owner_armed_endpoint_context',
  unsafe_public_ref: 'isolated_owner_armed_endpoint_context',
}

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash('sha256').update(value).digest('hex').slice(0, 24)}`

const toolCallFunctionName = (value: unknown): string | null => {
  if (typeof value !== 'object' || value === null) return null
  const toolCall = value as Record<string, unknown>
  const fn = toolCall.function
  if (typeof fn !== 'object' || fn === null) return null
  const name = (fn as Record<string, unknown>).name
  return typeof name === 'string' && name.trim() !== '' ? name : null
}

const toolCallId = (value: unknown): string | null => {
  if (typeof value !== 'object' || value === null) return null
  const id = (value as Record<string, unknown>).id
  return typeof id === 'string' && id.trim() !== '' ? id : null
}

const responseHasProviderError = (response: Record<string, unknown>): boolean =>
  typeof response.error === 'object' && response.error !== null

const responseCompletionTokens = (response: Record<string, unknown>): number => {
  const usage =
    typeof response.usage === 'object' && response.usage !== null
      ? (response.usage as Record<string, unknown>)
      : {}
  return typeof usage.completion_tokens === 'number'
    ? usage.completion_tokens
    : 0
}

const defaultToolLoop = (): GlmNvfp4ToolLoopEvidence => ({
  sampleCount: 0,
  providerErrorCount: 0,
  toolCallsAttempted: 0,
  toolCallsSucceeded: 0,
  hallucinatedToolCallCount: 0,
  evidenceRef: null,
})

const defaultThroughput = (
  reapBaselineTps: number,
): GlmNvfp4ThroughputEvidence => ({
  outputTokens: 0,
  wallClockMs: 0,
  measuredTps: null,
  reapBaselineTps,
  evidenceRef: null,
})

const servingStackFindings = (
  input: ReadonlyArray<GlmNvfp4ServingStackFinding> | undefined,
): Readonly<{
  publicFindings: ReadonlyArray<GlmNvfp4ServingStackFinding>
  hasUnsafeEvidenceRef: boolean
  hasFailedBeforeEndpoint: boolean
}> => {
  const findings = input ?? []
  const publicFindings = findings.map(finding => ({
    ...finding,
    evidenceRef: safeRefOrNull(finding.evidenceRef),
  }))
  return {
    publicFindings,
    hasUnsafeEvidenceRef: findings.some(
      finding =>
        nullIfBlank(finding.evidenceRef) !== null &&
        safeRefOrNull(finding.evidenceRef) === null,
    ),
    hasFailedBeforeEndpoint: publicFindings.some(
      finding => finding.status === 'failed_before_endpoint',
    ),
  }
}

const finitePositiveInteger = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : null

const finitePositiveNumber = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value > 0
    ? value
    : null

const publicSafeRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.flatMap(ref => {
    const publicRef = safeRefOrNull(ref)
    return publicRef === null ? [] : [publicRef]
  }))].sort()

const pilotSummaryGateEvidenceRefs = (
  result: GlmNvfp4PilotResult,
  gate: GlmNvfp4PilotIssueGate,
): ReadonlyArray<string> => {
  const refsByGate: Record<
    GlmNvfp4PilotIssueGate,
    ReadonlyArray<string | null>
  > = {
    isolated_owner_armed_endpoint_context: [
      result.ownerApprovalRef,
      result.endpointRef,
      result.decisionRef,
      result.bootLoadEvidenceRef,
      ...result.servingStackFindings.map(finding => finding.evidenceRef),
    ],
    tool_loop_proof: [result.toolLoop.evidenceRef],
    quality_parity: [result.qualityEvidenceRef],
    throughput_context_tradeoff: [
      result.measuredMaxModelLenEvidenceRef,
      result.throughput.evidenceRef,
    ],
  }
  return publicSafeRefs(refsByGate[gate])
}

const pilotSummaryGateHasRequiredEvidence = (
  result: GlmNvfp4PilotResult,
  gate: GlmNvfp4PilotIssueGate,
): boolean => {
  const measuredTps = finitePositiveNumber(result.throughput.measuredTps)
  const requiredByGate: Record<GlmNvfp4PilotIssueGate, boolean> = {
    isolated_owner_armed_endpoint_context:
      result.ownerArmed === true &&
      safeRefOrNull(result.ownerApprovalRef) !== null &&
      safeRefOrNull(result.endpointRef) !== null &&
      safeRefOrNull(result.decisionRef) !== null &&
      safeRefOrNull(result.bootLoadEvidenceRef) !== null &&
      result.bootLoadStatus === 'passed',
    tool_loop_proof:
      safeRefOrNull(result.toolLoop.evidenceRef) !== null &&
      result.toolLoop.sampleCount >= GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES &&
      result.toolLoop.providerErrorCount === 0 &&
      result.toolLoop.toolCallsAttempted >= GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES &&
      result.toolLoop.toolCallsSucceeded >= GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES &&
      result.toolLoop.hallucinatedToolCallCount === 0,
    quality_parity:
      result.qualityParity === 'passed' &&
      safeRefOrNull(result.qualityEvidenceRef) !== null,
    throughput_context_tradeoff:
      result.measuredMaxModelLen !== null &&
      safeRefOrNull(result.measuredMaxModelLenEvidenceRef) !== null &&
      safeRefOrNull(result.throughput.evidenceRef) !== null &&
      result.throughput.outputTokens > 0 &&
      measuredTps !== null,
  }
  return requiredByGate[gate]
}

export const summarizeGlmNvfp4PilotResult = (
  input: unknown,
): GlmNvfp4PilotPublicSummary => {
  const result = decodeGlmNvfp4PilotResult(input)
  const gates = GLM_NVFP4_PILOT_ISSUE_GATES.map(gate => {
    const blockerRefs = result.blockerRefs.filter(
      blocker => GLM_NVFP4_PILOT_BLOCKER_ISSUE_GATE[blocker] === gate,
    )
    const status: GlmNvfp4PilotIssueGateStatus =
      blockerRefs.length === 0 &&
      pilotSummaryGateHasRequiredEvidence(result, gate)
        ? 'passed'
        : 'blocked'
    return {
      gate,
      status,
      blockerRefs,
      evidenceRefs: pilotSummaryGateEvidenceRefs(result, gate),
    }
  })
  const allGatesPassed = gates.every(gate => gate.status === 'passed')
  const decision: GlmNvfp4PilotDecision =
    allGatesPassed && result.decision === 'go' && result.canRouteCodingLane
      ? 'go'
      : 'no_go'

  return decodeGlmNvfp4PilotPublicSummary({
    schemaVersion: 'openagents.khala.glm_nvfp4_pilot_public_summary.v1',
    generatedAt: result.generatedAt,
    issueRef: result.issueRef,
    publicSafe: true,
    decision,
    canRouteCodingLane: decision === 'go',
    gates,
    blockerRefs: result.blockerRefs,
    evidenceRefs: publicSafeRefs(gates.flatMap(gate => gate.evidenceRefs)),
    contentRedacted: true,
  })
}

export const buildGlmNvfp4PilotLaunchFlags = (
  measuredMaxModelLen: number | null,
): ReadonlyArray<GlmNvfp4PilotVllmFlag> => [
  ...GLM_NVFP4_EXACT_VLLM_FLAGS,
  ...(measuredMaxModelLen === null
    ? []
    : [{ name: '--max-model-len', value: String(measuredMaxModelLen) }]),
]

export const buildGlmNvfp4PilotResult = (input: {
  config: GlmNvfp4PilotConfig
  observation?: GlmNvfp4PilotObservation
}): GlmNvfp4PilotResult => {
  const config = input.config
  const blockerRefs = new Set<GlmNvfp4PilotBlocker>()
  const ownerApprovalRef = safeRefOrNull(config.ownerApprovalRef)
  const endpointRef = safeRefOrNull(config.endpointRef)
  const decisionRef = safeRefOrNull(config.decisionRef)
  const bootLoadEvidenceRef = safeRefOrNull(config.bootLoadEvidenceRef)
  const stackFindings = servingStackFindings(config.servingStackFindings)
  const bootLoadStatus: GlmNvfp4BootLoadStatus =
    config.bootLoadStatus === 'failed' || stackFindings.hasFailedBeforeEndpoint
      ? 'failed'
      : config.bootLoadStatus === 'passed' ||
          (config.bootLoadStatus !== 'not_attempted' &&
            bootLoadEvidenceRef !== null)
        ? 'passed'
        : 'not_attempted'
  const measuredMaxModelLen = finitePositiveInteger(config.measuredMaxModelLen)
  const measuredMaxModelLenEvidenceRef = safeRefOrNull(
    config.measuredMaxModelLenEvidenceRef,
  )
  const qualityEvidenceRef = safeRefOrNull(config.qualityEvidenceRef)
  const qualityParity = config.qualityParity ?? 'not_measured'
  const reapBaselineTps =
    finitePositiveNumber(config.reapBaselineTps) ??
    GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS
  const toolLoop = input.observation?.toolLoop ?? defaultToolLoop()
  const throughput =
    input.observation?.throughput ?? defaultThroughput(reapBaselineTps)
  const requiredToolLoopSamples =
    config.requiredToolLoopSamples ?? GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES
  const toolLoopEvidenceRef = safeRefOrNull(toolLoop.evidenceRef)
  const throughputEvidenceRef = safeRefOrNull(throughput.evidenceRef)
  const publicToolLoop: GlmNvfp4ToolLoopEvidence = {
    ...toolLoop,
    evidenceRef: toolLoopEvidenceRef,
  }
  const publicThroughput: GlmNvfp4ThroughputEvidence = {
    ...throughput,
    evidenceRef: throughputEvidenceRef,
  }
  const evidenceRefAuditRows = [
    evidenceRefAudit({
      field: 'ownerApprovalRef',
      rawRef: config.ownerApprovalRef,
      publicRef: ownerApprovalRef,
    }),
    evidenceRefAudit({
      field: 'endpointRef',
      rawRef: config.endpointRef,
      publicRef: endpointRef,
    }),
    evidenceRefAudit({
      field: 'decisionRef',
      rawRef: config.decisionRef,
      publicRef: decisionRef,
    }),
    evidenceRefAudit({
      field: 'bootLoadEvidenceRef',
      rawRef: config.bootLoadEvidenceRef,
      publicRef: bootLoadEvidenceRef,
    }),
    evidenceRefAudit({
      field: 'measuredMaxModelLenEvidenceRef',
      rawRef: config.measuredMaxModelLenEvidenceRef,
      publicRef: measuredMaxModelLenEvidenceRef,
    }),
    evidenceRefAudit({
      field: 'qualityEvidenceRef',
      rawRef: config.qualityEvidenceRef,
      publicRef: qualityEvidenceRef,
    }),
    evidenceRefAudit({
      field: 'toolLoopEvidenceRef',
      rawRef: toolLoop.evidenceRef,
      publicRef: toolLoopEvidenceRef,
    }),
    evidenceRefAudit({
      field: 'throughputEvidenceRef',
      rawRef: throughput.evidenceRef,
      publicRef: throughputEvidenceRef,
    }),
  ]

  if (config.ownerArmed !== true) blockerRefs.add('owner_arm_missing')
  if (ownerApprovalRef === null) blockerRefs.add('owner_approval_ref_missing')
  if (endpointRef === null) blockerRefs.add('endpoint_ref_missing')
  if (nullIfBlank(config.endpointUrl) === null) {
    blockerRefs.add('endpoint_url_missing')
  }
  if ((config.model ?? GLM_NVFP4_PILOT_MODEL) !== GLM_NVFP4_PILOT_MODEL) {
    blockerRefs.add('model_mismatch')
  }
  if (bootLoadEvidenceRef === null) {
    blockerRefs.add('boot_load_evidence_ref_missing')
  }
  if (bootLoadStatus === 'failed') {
    blockerRefs.add('boot_load_failed')
  }
  if (stackFindings.hasUnsafeEvidenceRef) {
    blockerRefs.add('unsafe_public_ref')
    blockerRefs.add('serving_stack_evidence_ref_unsafe')
  }
  if (measuredMaxModelLen === null) {
    blockerRefs.add('measured_max_model_len_missing')
  }
  if (measuredMaxModelLenEvidenceRef === null) {
    blockerRefs.add('measured_max_model_len_evidence_ref_missing')
  }
  if (toolLoopEvidenceRef === null) {
    blockerRefs.add('tool_loop_evidence_missing')
  }
  if (toolLoop.sampleCount < requiredToolLoopSamples) {
    blockerRefs.add('tool_loop_sample_count_too_low')
  }
  if (toolLoop.providerErrorCount > 0) blockerRefs.add('tool_loop_provider_error')
  if (
    toolLoop.toolCallsAttempted < requiredToolLoopSamples ||
    toolLoop.toolCallsSucceeded < requiredToolLoopSamples ||
    toolLoop.hallucinatedToolCallCount > 0
  ) {
    blockerRefs.add('tool_loop_missing_tool_calls')
  }
  if (qualityParity !== 'passed') blockerRefs.add('quality_parity_missing')
  if (qualityEvidenceRef === null) blockerRefs.add('quality_evidence_ref_missing')
  if (throughput.measuredTps === null || throughput.outputTokens <= 0) {
    blockerRefs.add('tps_measurement_missing')
  }
  if (
    throughput.measuredTps !== null &&
    (!Number.isFinite(throughput.measuredTps) || throughput.measuredTps < 0)
  ) {
    blockerRefs.add('tps_not_finite')
  }
  if (decisionRef === null) blockerRefs.add('decision_ref_missing')
  evidenceRefAuditRows
    .filter(row => row.status === 'rejected_unsafe')
    .forEach(row => {
      blockerRefs.add('unsafe_public_ref')
      blockerRefs.add(unsafeBlockerForEvidenceField(row.field))
    })

  const decision: GlmNvfp4PilotDecision =
    blockerRefs.size === 0 ? 'go' : 'no_go'
  const evidenceRefs = [
    ownerApprovalRef,
    endpointRef,
    decisionRef,
    bootLoadEvidenceRef,
    ...stackFindings.publicFindings.map(finding => finding.evidenceRef),
    measuredMaxModelLenEvidenceRef,
    qualityEvidenceRef,
    publicToolLoop.evidenceRef,
    publicThroughput.evidenceRef,
  ].filter((ref): ref is string => ref !== null)

  return decodeGlmNvfp4PilotResult({
    schemaVersion: GLM_NVFP4_PILOT_RESULT_SCHEMA,
    generatedAt: config.generatedAt,
    issueRef: 'github.issue.OpenAgentsInc.openagents.6323',
    publicSafe: true,
    ownerArmed: config.ownerArmed === true,
    decision,
    canRouteCodingLane: decision === 'go',
    model: GLM_NVFP4_PILOT_MODEL,
    endpointRef,
    ownerApprovalRef,
    decisionRef,
    bootLoadStatus,
    bootLoadEvidenceRef,
    servingStackFindings: stackFindings.publicFindings,
    measuredMaxModelLen,
    measuredMaxModelLenEvidenceRef,
    qualityParity,
    qualityEvidenceRef,
    runtimeRequirements: {
      hostClassRef: 'g4-standard-384.8x-rtx-pro-6000.blackwell',
      tensorParallelSize: 8,
      transformersMinimumVersion: '5.3.0',
      isolatedEndpointRequired: true,
    },
    vllmFlags: buildGlmNvfp4PilotLaunchFlags(measuredMaxModelLen),
    toolLoop: publicToolLoop,
    throughput: {
      ...publicThroughput,
      reapBaselineTps,
    },
    routingOutcome:
      decision === 'go'
        ? 'route_coding_tools_to_full_model_then_reap_overflow'
        : 'keep_reap_live_lane',
    blockerRefs: [...blockerRefs].sort(),
    evidenceRefAudit: evidenceRefAuditRows,
    evidenceRefs: [...new Set(evidenceRefs)].sort(),
    authorityBoundary:
      'Owner-armed, public-safe pilot/preflight record for issue #6323. It does not expose endpoint URLs, API keys, raw prompts, raw model output, checkpoint paths, host paths, wallet material, or private traces; it does not repoint live Khala routing by itself.',
    contentRedacted: true,
  })
}

export const collectGlmNvfp4PilotObservation = async (input: {
  config: GlmNvfp4PilotConfig
  executor?: GlmNvfp4PilotExecutor
}): Promise<GlmNvfp4PilotObservation> => {
  if (
    input.config.ownerArmed !== true ||
    nullIfBlank(input.config.endpointUrl) === null ||
    safeRefOrNull(input.config.endpointRef) === null ||
    safeRefOrNull(input.config.ownerApprovalRef) === null ||
    (input.config.model ?? GLM_NVFP4_PILOT_MODEL) !== GLM_NVFP4_PILOT_MODEL ||
    input.executor === undefined
  ) {
    throw new GlmNvfp4PilotNotArmedError()
  }
  return input.executor(input.config)
}

export type OpenAiCompatiblePilotHttp = (
  path: string,
  body: Record<string, unknown>,
) => Promise<Record<string, unknown>>

export const makeOpenAiCompatibleGlmNvfp4PilotExecutor =
  (input: {
    http: OpenAiCompatiblePilotHttp
    samples: number
    evidenceSeed: string
  }): GlmNvfp4PilotExecutor =>
  async (config) => {
    const started = performance.now()
    const observations = await Promise.all(
      Array.from({ length: input.samples }, async (_, index) => {
        const response = await input.http('/v1/chat/completions', {
          model: config.model ?? GLM_NVFP4_PILOT_MODEL,
          messages: [
            {
              role: 'user',
              content:
                'Use the provided public fixture tool to return the number 7.',
            },
          ],
          tools: [
            {
              type: 'function',
              function: {
                name: GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
                description: 'Return a public fixture number.',
                parameters: {
                  type: 'object',
                  properties: {
                    value: { type: 'number' },
                  },
                  required: ['value'],
                },
              },
            },
          ],
          tool_choice: 'auto',
          temperature: 0,
          max_tokens: 64,
          metadata: {
            public_pilot_ref: 'github.issue.OpenAgentsInc.openagents.6323',
            sample_index: index,
          },
        })
        const error = responseHasProviderError(response)
        const choice = Array.isArray(response.choices)
          ? (response.choices[0] as Record<string, unknown> | undefined)
          : undefined
        const message =
          typeof choice?.message === 'object' && choice.message !== null
            ? (choice.message as Record<string, unknown>)
            : {}
        const toolCalls = Array.isArray(message.tool_calls)
          ? message.tool_calls
          : []
        const toolCallNames = toolCalls.flatMap(toolCall => {
          const name = toolCallFunctionName(toolCall)
          return name === null ? [] : [name]
        })
        const expectedToolCallCount = toolCallNames.filter(
          name => name === GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
        ).length
        const hallucinatedToolCallCount = toolCallNames.filter(
          name => name !== GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
        ).length
        const expectedToolCall = toolCalls.find(
          toolCall =>
            toolCallFunctionName(toolCall) === GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
        )
        const expectedToolCallId = toolCallId(expectedToolCall)
        const shouldRoundTrip =
          !error &&
          expectedToolCall !== undefined &&
          expectedToolCallId !== null &&
          expectedToolCallCount > 0 &&
          hallucinatedToolCallCount === 0
        const roundTripResponse = shouldRoundTrip
          ? await input.http('/v1/chat/completions', {
              model: config.model ?? GLM_NVFP4_PILOT_MODEL,
              messages: [
                {
                  role: 'user',
                  content:
                    'Use the provided public fixture tool to return the number 7.',
                },
                {
                  role: 'assistant',
                  tool_calls: [expectedToolCall],
                },
                {
                  role: 'tool',
                  tool_call_id: expectedToolCallId,
                  content: '{"value":7}',
                },
              ],
              temperature: 0,
              max_tokens: 64,
              metadata: {
                public_pilot_ref: 'github.issue.OpenAgentsInc.openagents.6323',
                sample_index: index,
                tool_round_trip: true,
              },
            })
          : null
        const roundTripError =
          roundTripResponse !== null && responseHasProviderError(roundTripResponse)
        const completionTokens =
          responseCompletionTokens(response) +
          (roundTripResponse === null
            ? 0
            : responseCompletionTokens(roundTripResponse))
        return {
          error,
          roundTripError,
          expectedToolCallCount,
          hallucinatedToolCallCount,
          completionTokens,
          roundTripped: shouldRoundTrip && !roundTripError,
        }
      }),
    )
    const wallClockMs = Math.max(1, Math.round(performance.now() - started))
    const outputTokens = observations.reduce(
      (sum, observation) => sum + observation.completionTokens,
      0,
    )
    const toolCallsSucceeded = observations.filter(
      observation =>
        observation.expectedToolCallCount > 0 &&
        observation.hallucinatedToolCallCount === 0 &&
        !observation.error &&
        observation.roundTripped,
    ).length
    const hallucinatedToolCallCount = observations.reduce(
      (sum, observation) => sum + observation.hallucinatedToolCallCount,
      0,
    )
    const seed = `${input.evidenceSeed}:${config.endpointRef}:${input.samples}:${outputTokens}:${toolCallsSucceeded}`
    return {
      toolLoop: {
        sampleCount: observations.length,
        providerErrorCount: observations.filter(
          observation => observation.error || observation.roundTripError,
        ).length,
        toolCallsAttempted: observations.length,
        toolCallsSucceeded,
        hallucinatedToolCallCount,
        evidenceRef: stableRef('evidence.public.khala.glm_nvfp4.tool_loop', seed),
      },
      throughput: {
        outputTokens,
        wallClockMs,
        measuredTps: outputTokens / (wallClockMs / 1000),
        reapBaselineTps:
          finitePositiveNumber(config.reapBaselineTps) ??
          GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
        evidenceRef: stableRef('evidence.public.khala.glm_nvfp4.tps', seed),
      },
    }
  }
