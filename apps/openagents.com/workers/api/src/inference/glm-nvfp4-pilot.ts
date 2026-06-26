import { createHash } from 'node:crypto'
import { Schema as S } from 'effect'

export const GLM_NVFP4_PILOT_RESULT_SCHEMA =
  'openagents.khala.glm_nvfp4_pilot_result.v1' as const
export const GLM_NVFP4_PILOT_MODEL = 'nvidia/GLM-5.2-NVFP4' as const
export const GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES = 20
export const GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS = 47

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
  'endpoint_ref_missing',
  'endpoint_url_missing',
  'model_mismatch',
  'measured_max_model_len_missing',
  'measured_max_model_len_evidence_ref_missing',
  'tool_loop_evidence_missing',
  'tool_loop_sample_count_too_low',
  'tool_loop_provider_error',
  'tool_loop_missing_tool_calls',
  'quality_parity_missing',
  'quality_evidence_ref_missing',
  'tps_measurement_missing',
  'tps_not_finite',
  'decision_ref_missing',
  'unsafe_public_ref',
])
export type GlmNvfp4PilotBlocker = typeof GlmNvfp4PilotBlocker.Type

export const GlmNvfp4PilotDecision = S.Literals(['go', 'no_go'])
export type GlmNvfp4PilotDecision = typeof GlmNvfp4PilotDecision.Type

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
  evidenceRefs: S.Array(S.String),
  authorityBoundary: S.String,
  contentRedacted: S.Literal(true),
})
export type GlmNvfp4PilotResult = typeof GlmNvfp4PilotResult.Type

export const decodeGlmNvfp4PilotResult = S.decodeUnknownSync(
  GlmNvfp4PilotResult,
)

export type GlmNvfp4PilotConfig = Readonly<{
  generatedAt: string
  ownerArmed?: boolean
  ownerApprovalRef?: string | null
  endpointUrl?: string | null
  endpointRef?: string | null
  model?: string | null
  decisionRef?: string | null
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

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash('sha256').update(value).digest('hex').slice(0, 24)}`

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

const finitePositiveInteger = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : null

const finitePositiveNumber = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value > 0
    ? value
    : null

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
  const rawRefs = [
    config.ownerApprovalRef,
    config.endpointRef,
    config.decisionRef,
    config.measuredMaxModelLenEvidenceRef,
    config.qualityEvidenceRef,
    toolLoop.evidenceRef,
    throughput.evidenceRef,
  ].flatMap(ref => {
    const normalized = nullIfBlank(ref)
    return normalized === null ? [] : [normalized]
  })

  if (config.ownerArmed !== true) blockerRefs.add('owner_arm_missing')
  if (ownerApprovalRef === null) blockerRefs.add('owner_approval_ref_missing')
  if (endpointRef === null) blockerRefs.add('endpoint_ref_missing')
  if (nullIfBlank(config.endpointUrl) === null) {
    blockerRefs.add('endpoint_url_missing')
  }
  if ((config.model ?? GLM_NVFP4_PILOT_MODEL) !== GLM_NVFP4_PILOT_MODEL) {
    blockerRefs.add('model_mismatch')
  }
  if (measuredMaxModelLen === null) {
    blockerRefs.add('measured_max_model_len_missing')
  }
  if (measuredMaxModelLenEvidenceRef === null) {
    blockerRefs.add('measured_max_model_len_evidence_ref_missing')
  }
  if (toolLoop.evidenceRef === null) blockerRefs.add('tool_loop_evidence_missing')
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
  if (rawRefs.some(ref => !isSafeRef(ref))) blockerRefs.add('unsafe_public_ref')

  const decision: GlmNvfp4PilotDecision =
    blockerRefs.size === 0 ? 'go' : 'no_go'
  const evidenceRefs = [
    ownerApprovalRef,
    endpointRef,
    decisionRef,
    measuredMaxModelLenEvidenceRef,
    qualityEvidenceRef,
    toolLoop.evidenceRef,
    throughput.evidenceRef,
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
    toolLoop,
    throughput: {
      ...throughput,
      reapBaselineTps,
    },
    routingOutcome:
      decision === 'go'
        ? 'route_coding_tools_to_full_model_then_reap_overflow'
        : 'keep_reap_live_lane',
    blockerRefs: [...blockerRefs].sort(),
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
                name: 'public_fixture_return_number',
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
        const error = typeof response.error === 'object' && response.error !== null
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
        const usage =
          typeof response.usage === 'object' && response.usage !== null
            ? (response.usage as Record<string, unknown>)
            : {}
        const completionTokens =
          typeof usage.completion_tokens === 'number'
            ? usage.completion_tokens
            : 0
        return {
          error,
          toolCallCount: toolCalls.length,
          completionTokens,
        }
      }),
    )
    const wallClockMs = Math.max(1, Math.round(performance.now() - started))
    const outputTokens = observations.reduce(
      (sum, observation) => sum + observation.completionTokens,
      0,
    )
    const toolCallsSucceeded = observations.filter(
      observation => observation.toolCallCount > 0 && !observation.error,
    ).length
    const seed = `${input.evidenceSeed}:${config.endpointRef}:${input.samples}:${outputTokens}:${toolCallsSucceeded}`
    return {
      toolLoop: {
        sampleCount: observations.length,
        providerErrorCount: observations.filter(observation => observation.error)
          .length,
        toolCallsAttempted: observations.length,
        toolCallsSucceeded,
        hallucinatedToolCallCount: 0,
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
