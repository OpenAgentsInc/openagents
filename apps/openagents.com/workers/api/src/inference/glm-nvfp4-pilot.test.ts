import { describe, expect, test } from 'vitest'

import {
  GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
  GLM_NVFP4_EXACT_VLLM_FLAGS,
  GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
  GLM_NVFP4_PILOT_MODEL,
  GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
  GlmNvfp4PilotNotArmedError,
  buildGlmNvfp4PilotLaunchFlags,
  buildGlmNvfp4PilotResult,
  collectGlmNvfp4PilotObservation,
  decodeGlmNvfp4PilotResult,
  makeOpenAiCompatibleGlmNvfp4PilotExecutor,
  type GlmNvfp4PilotConfig,
  type GlmNvfp4PilotObservation,
} from './glm-nvfp4-pilot'

const baseConfig = (
  overrides: Partial<GlmNvfp4PilotConfig> = {},
): GlmNvfp4PilotConfig => ({
  generatedAt: '2026-06-26T18:00:00.000Z',
  ownerArmed: true,
  ownerApprovalRef: 'approval.public.khala.glm_nvfp4.owner_armed.001',
  endpointUrl: 'https://glm-nvfp4-pilot.example.invalid',
  endpointRef: 'endpoint.public.khala.glm_nvfp4.single_host_8x.001',
  model: GLM_NVFP4_PILOT_MODEL,
  decisionRef: 'decision.public.khala.glm_nvfp4.issue_6323.001',
  measuredMaxModelLen: 65536,
  measuredMaxModelLenEvidenceRef:
    'evidence.public.khala.glm_nvfp4.max_model_len.65536.001',
  qualityParity: 'passed',
  qualityEvidenceRef: 'evidence.public.khala.glm_nvfp4.quality_parity.001',
  reapBaselineTps: GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
  ...overrides,
})

const passingObservation = (
  overrides: Partial<GlmNvfp4PilotObservation> = {},
): GlmNvfp4PilotObservation => ({
  toolLoop: {
    sampleCount: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
    providerErrorCount: 0,
    toolCallsAttempted: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
    toolCallsSucceeded: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
    hallucinatedToolCallCount: 0,
    evidenceRef: 'evidence.public.khala.glm_nvfp4.tool_loop.001',
  },
  throughput: {
    outputTokens: 640,
    wallClockMs: 10000,
    measuredTps: 64,
    reapBaselineTps: GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
    evidenceRef: 'evidence.public.khala.glm_nvfp4.tps.001',
  },
  ...overrides,
})

describe('GLM NVFP4 pilot preflight (#6323)', () => {
  test('fails closed when owner arm and host access are unavailable', async () => {
    const config = baseConfig({
      ownerArmed: false,
      ownerApprovalRef: null,
      endpointUrl: null,
      endpointRef: null,
      measuredMaxModelLen: null,
      measuredMaxModelLenEvidenceRef: null,
      qualityParity: 'not_measured',
      qualityEvidenceRef: null,
      decisionRef: null,
    })

    await expect(
      collectGlmNvfp4PilotObservation({ config, executor: async () => ({}) }),
    ).rejects.toThrow(GlmNvfp4PilotNotArmedError)

    const result = buildGlmNvfp4PilotResult({ config })
    expect(result.decision).toBe('no_go')
    expect(result.canRouteCodingLane).toBe(false)
    expect(result.routingOutcome).toBe('keep_reap_live_lane')
    expect(result.blockerRefs).toEqual([
      'decision_ref_missing',
      'endpoint_ref_missing',
      'endpoint_url_missing',
      'measured_max_model_len_evidence_ref_missing',
      'measured_max_model_len_missing',
      'owner_approval_ref_missing',
      'owner_arm_missing',
      'quality_evidence_ref_missing',
      'quality_parity_missing',
      'tool_loop_evidence_missing',
      'tool_loop_missing_tool_calls',
      'tool_loop_sample_count_too_low',
      'tps_measurement_missing',
    ])
  })

  test('materializes the exact NVIDIA/vLLM launch flags plus measured max-model-len', () => {
    const flags = buildGlmNvfp4PilotLaunchFlags(65536)

    expect(flags.slice(0, GLM_NVFP4_EXACT_VLLM_FLAGS.length)).toEqual(
      GLM_NVFP4_EXACT_VLLM_FLAGS,
    )
    expect(flags.at(-1)).toEqual({
      name: '--max-model-len',
      value: '65536',
    })
  })

  test('records a public-safe go decision only when tool, quality, context, and TPS evidence pass', () => {
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig(),
      observation: passingObservation(),
    })

    expect(result.decision).toBe('go')
    expect(result.canRouteCodingLane).toBe(true)
    expect(result.blockerRefs).toEqual([])
    expect(result.routingOutcome).toBe(
      'route_coding_tools_to_full_model_then_reap_overflow',
    )
    expect(result.runtimeRequirements).toEqual({
      hostClassRef: 'g4-standard-384.8x-rtx-pro-6000.blackwell',
      tensorParallelSize: 8,
      transformersMinimumVersion: '5.3.0',
      isolatedEndpointRequired: true,
    })
    expect(result.evidenceRefs).toContain(
      'evidence.public.khala.glm_nvfp4.tool_loop.001',
    )
    expect(result.evidenceRefAudit).toEqual([
      {
        field: 'ownerApprovalRef',
        status: 'accepted',
        publicRef: 'approval.public.khala.glm_nvfp4.owner_armed.001',
      },
      {
        field: 'endpointRef',
        status: 'accepted',
        publicRef: 'endpoint.public.khala.glm_nvfp4.single_host_8x.001',
      },
      {
        field: 'decisionRef',
        status: 'accepted',
        publicRef: 'decision.public.khala.glm_nvfp4.issue_6323.001',
      },
      {
        field: 'measuredMaxModelLenEvidenceRef',
        status: 'accepted',
        publicRef: 'evidence.public.khala.glm_nvfp4.max_model_len.65536.001',
      },
      {
        field: 'qualityEvidenceRef',
        status: 'accepted',
        publicRef: 'evidence.public.khala.glm_nvfp4.quality_parity.001',
      },
      {
        field: 'toolLoopEvidenceRef',
        status: 'accepted',
        publicRef: 'evidence.public.khala.glm_nvfp4.tool_loop.001',
      },
      {
        field: 'throughputEvidenceRef',
        status: 'accepted',
        publicRef: 'evidence.public.khala.glm_nvfp4.tps.001',
      },
    ])
    expect(JSON.stringify(result)).not.toContain(
      'https://glm-nvfp4-pilot.example.invalid',
    )
    expect(decodeGlmNvfp4PilotResult(result).schemaVersion).toBe(
      'openagents.khala.glm_nvfp4_pilot_result.v1',
    )
  })

  test('keeps the issue open/no-go when provider errors or insufficient samples remain', () => {
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig(),
      observation: passingObservation({
        toolLoop: {
          sampleCount: 3,
          providerErrorCount: 1,
          toolCallsAttempted: 3,
          toolCallsSucceeded: 2,
          hallucinatedToolCallCount: 0,
          evidenceRef: 'evidence.public.khala.glm_nvfp4.tool_loop.partial',
        },
      }),
    })

    expect(result.decision).toBe('no_go')
    expect(result.blockerRefs).toContain('tool_loop_provider_error')
    expect(result.blockerRefs).toContain('tool_loop_sample_count_too_low')
    expect(result.blockerRefs).toContain('tool_loop_missing_tool_calls')
  })

  test('does not call the live executor when the requested model is not the isolated NVFP4 checkpoint', async () => {
    await expect(
      collectGlmNvfp4PilotObservation({
        config: baseConfig({ model: 'openagents/glm-5.2-reap-504b' }),
        executor: async () => {
          throw new Error('executor should not be called')
        },
      }),
    ).rejects.toThrow(GlmNvfp4PilotNotArmedError)
  })

  test('blocks unsafe refs from public output and records a typed blocker', () => {
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig({
        ownerApprovalRef: '/Users/operator/secret-token',
      }),
      observation: passingObservation(),
    })

    expect(result.ownerApprovalRef).toBeNull()
    expect(result.blockerRefs).toContain('unsafe_public_ref')
    expect(result.blockerRefs).toContain('owner_approval_ref_unsafe')
    expect(result.blockerRefs).toContain('owner_approval_ref_missing')
    expect(result.evidenceRefAudit).toContainEqual({
      field: 'ownerApprovalRef',
      status: 'rejected_unsafe',
      publicRef: null,
    })
    expect(JSON.stringify(result)).not.toContain('/Users/operator')
  })

  test('blocks URL-like refs from public output', () => {
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig({
        endpointRef: 'https://pilot.internal.example.invalid/v1',
      }),
      observation: passingObservation(),
    })

    expect(result.endpointRef).toBeNull()
    expect(result.blockerRefs).toContain('unsafe_public_ref')
    expect(result.blockerRefs).toContain('endpoint_ref_unsafe')
    expect(result.blockerRefs).toContain('endpoint_ref_missing')
    expect(result.evidenceRefAudit).toContainEqual({
      field: 'endpointRef',
      status: 'rejected_unsafe',
      publicRef: null,
    })
    expect(JSON.stringify(result)).not.toContain('pilot.internal')
  })

  test('redacts unsafe live observation evidence refs before public reporting', () => {
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig(),
      observation: passingObservation({
        toolLoop: {
          sampleCount: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          providerErrorCount: 0,
          toolCallsAttempted: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          toolCallsSucceeded: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          hallucinatedToolCallCount: 0,
          evidenceRef: 'prompt.private.host.secret',
        },
        throughput: {
          outputTokens: 640,
          wallClockMs: 10000,
          measuredTps: 64,
          reapBaselineTps: GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
          evidenceRef: '/home/operator/tps.json',
        },
      }),
    })

    expect(result.decision).toBe('no_go')
    expect(result.toolLoop.evidenceRef).toBeNull()
    expect(result.throughput.evidenceRef).toBeNull()
    expect(result.evidenceRefs).not.toContain('prompt.private.host.secret')
    expect(result.evidenceRefs).not.toContain('/home/operator/tps.json')
    expect(result.blockerRefs).toContain('tool_loop_evidence_ref_unsafe')
    expect(result.blockerRefs).toContain('tps_evidence_ref_unsafe')
    expect(result.evidenceRefAudit).toContainEqual({
      field: 'toolLoopEvidenceRef',
      status: 'rejected_unsafe',
      publicRef: null,
    })
    expect(result.evidenceRefAudit).toContainEqual({
      field: 'throughputEvidenceRef',
      status: 'rejected_unsafe',
      publicRef: null,
    })
    expect(JSON.stringify(result)).not.toContain('/home/operator')
    expect(JSON.stringify(result)).not.toContain('prompt.private.host.secret')
  })

  test('live executor captures tool-call and TPS evidence without raw content', async () => {
    const executor = makeOpenAiCompatibleGlmNvfp4PilotExecutor({
      samples: 2,
      evidenceSeed: 'seed.public.fixture',
      http: async () => ({
        choices: [
          {
            message: {
              tool_calls: [
                {
                  id: 'call_fixture',
                  type: 'function',
                  function: {
                    name: GLM_NVFP4_PUBLIC_FIXTURE_TOOL_NAME,
                    arguments: '{"value":7}',
                  },
                },
              ],
            },
          },
        ],
        usage: {
          completion_tokens: 32,
        },
      }),
    })

    const observation = await executor(baseConfig({ requiredToolLoopSamples: 2 }))

    expect(observation.toolLoop?.sampleCount).toBe(2)
    expect(observation.toolLoop?.toolCallsSucceeded).toBe(2)
    expect(observation.throughput?.outputTokens).toBe(64)
    expect(observation.throughput?.measuredTps).toBeGreaterThan(0)
    expect(observation.toolLoop?.evidenceRef).toMatch(
      /^evidence\.public\.khala\.glm_nvfp4\.tool_loop\.[a-f0-9]{24}$/,
    )
    expect(JSON.stringify(observation)).not.toContain(
      'Use the provided public fixture tool',
    )
  })

  test('live executor fails closed on hallucinated tool names without exposing raw output', async () => {
    const executor = makeOpenAiCompatibleGlmNvfp4PilotExecutor({
      samples: 1,
      evidenceSeed: 'seed.public.fixture',
      http: async () => ({
        choices: [
          {
            message: {
              tool_calls: [
                {
                  id: 'call_unexpected',
                  type: 'function',
                  function: {
                    name: 'search',
                    arguments: '{"query":"private host"}',
                  },
                },
              ],
            },
          },
        ],
        usage: {
          completion_tokens: 8,
        },
      }),
    })

    const observation = await executor(baseConfig({ requiredToolLoopSamples: 1 }))
    const result = buildGlmNvfp4PilotResult({
      config: baseConfig({ requiredToolLoopSamples: 1 }),
      observation,
    })

    expect(observation.toolLoop?.toolCallsSucceeded).toBe(0)
    expect(observation.toolLoop?.hallucinatedToolCallCount).toBe(1)
    expect(result.decision).toBe('no_go')
    expect(result.blockerRefs).toContain('tool_loop_missing_tool_calls')
    expect(JSON.stringify(observation)).not.toContain('private host')
    expect(JSON.stringify(observation)).not.toContain('search')
  })
})
