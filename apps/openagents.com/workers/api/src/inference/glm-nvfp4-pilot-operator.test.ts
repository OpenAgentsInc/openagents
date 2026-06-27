import { describe, expect, test } from 'vitest'

import {
  GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
  GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
  GLM_NVFP4_PILOT_MODEL,
  buildGlmNvfp4PilotResult,
  type GlmNvfp4PilotConfig,
  type GlmNvfp4PilotObservation,
} from './glm-nvfp4-pilot'
import {
  buildGlmNvfp4PilotOperatorBundle,
  buildGlmNvfp4PilotOwnerArmedCommand,
  formatGlmNvfp4PilotOperatorReadme,
} from './glm-nvfp4-pilot-operator'

const config = (
  overrides: Partial<GlmNvfp4PilotConfig> = {},
): GlmNvfp4PilotConfig => ({
  generatedAt: '2026-06-26T23:55:00.000Z',
  ownerArmed: false,
  ownerApprovalRef: null,
  endpointUrl: null,
  endpointRef: null,
  model: GLM_NVFP4_PILOT_MODEL,
  decisionRef: null,
  bootLoadEvidenceRef: null,
  measuredMaxModelLen: null,
  measuredMaxModelLenEvidenceRef: null,
  qualityParity: 'not_measured',
  qualityEvidenceRef: null,
  reapBaselineTps: GLM_NVFP4_DEFAULT_REAP_BASELINE_TPS,
  requiredToolLoopSamples: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
  ...overrides,
})

const observation = (
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

describe('GLM NVFP4 pilot operator path (#6323)', () => {
  test('turns the unarmed local environment into an explicit public-safe no-go bundle', () => {
    const localConfig = config()
    const result = buildGlmNvfp4PilotResult({ config: localConfig })
    const bundle = buildGlmNvfp4PilotOperatorBundle({
      config: localConfig,
      result,
      outputDir: '.pilot-evidence/glm-nvfp4-6323',
    })
    const missingEnvs = bundle.missingOperatorInputs.map(input => input.env)

    expect(bundle.summary.decision).toBe('no_go')
    expect(bundle.summary.canRouteCodingLane).toBe(false)
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_PILOT_ARM')
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_ENDPOINT_URL')
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_ENDPOINT_REF')
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_OWNER_APPROVAL_REF')
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_DECISION_REF')
    expect(missingEnvs).toContain('KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF')
    expect(bundle.ownerArmedCommand).toContain(
      'bun run --cwd apps/openagents.com/workers/api pilot:glm-nvfp4',
    )
    expect(bundle.ownerArmedCommand).toContain(
      '--output-dir .pilot-evidence/glm-nvfp4-6323',
    )
    expect(bundle.retentionNotes.join('\n')).toContain(
      'does not authorize or perform any live Khala routing change',
    )
  })

  test('formats the exact owner-armed command without embedding live refs', () => {
    const command = buildGlmNvfp4PilotOwnerArmedCommand({
      outputDir: '.evidence/glm',
      samples: 24,
      measuredMaxModelLen: 65536,
    })

    expect(command).toContain('KHALA_GLM_NVFP4_PILOT_ARM=1')
    expect(command).toContain(
      'KHALA_GLM_NVFP4_ENDPOINT_URL="<redacted OpenAI-compatible pilot base URL>"',
    )
    expect(command).toContain(
      'KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF="evidence.public.khala.glm_nvfp4.boot_load.<owner-issued>"',
    )
    expect(command).toContain('KHALA_GLM_NVFP4_MODEL="nvidia/GLM-5.2-NVFP4"')
    expect(command).toContain('--samples 24')
    expect(command).toContain('--output-dir .evidence/glm')
    expect(command).not.toContain('https://')
    expect(command).not.toContain('sk-')
  })

  test('redacts absolute output paths from retained owner command templates', () => {
    const command = buildGlmNvfp4PilotOwnerArmedCommand({
      outputDir: '/Users/operator/private/glm-evidence',
    })

    expect(command).toContain('--output-dir .pilot-evidence/glm-nvfp4-6323')
    expect(command).not.toContain('/Users/operator')
    expect(command).not.toContain('private')
  })

  test('retained evidence and README exclude unsafe refs and private endpoint values', () => {
    const localConfig = config({
      ownerArmed: true,
      ownerApprovalRef: 'approval.public.khala.glm_nvfp4.owner_armed.001',
      endpointUrl: 'https://pilot.internal.example.invalid/v1',
      endpointRef: 'https://pilot.internal.example.invalid/endpoint',
      decisionRef: 'decision.public.khala.glm_nvfp4.issue_6323.001',
      bootLoadEvidenceRef: 'wallet.private.boot.load',
      measuredMaxModelLen: 65536,
      measuredMaxModelLenEvidenceRef:
        'evidence.public.khala.glm_nvfp4.max_model_len.65536.001',
      qualityParity: 'passed',
      qualityEvidenceRef: 'prompt.private.quality',
    })
    const result = buildGlmNvfp4PilotResult({
      config: localConfig,
      observation: observation({
        toolLoop: {
          sampleCount: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          providerErrorCount: 0,
          toolCallsAttempted: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          toolCallsSucceeded: GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
          hallucinatedToolCallCount: 0,
          evidenceRef: '/Users/operator/raw-tool-loop.json',
        },
      }),
    })
    const bundle = buildGlmNvfp4PilotOperatorBundle({
      config: localConfig,
      result,
      outputDir: '.pilot-evidence/glm-nvfp4-6323',
    })
    const readme = formatGlmNvfp4PilotOperatorReadme(bundle)
    const retained = JSON.stringify(bundle) + readme
    const rejectedEnvs = bundle.missingOperatorInputs
      .filter(input => input.status === 'rejected_unsafe')
      .map(input => input.env)

    expect(bundle.summary.decision).toBe('no_go')
    expect(rejectedEnvs).toContain('KHALA_GLM_NVFP4_ENDPOINT_REF')
    expect(rejectedEnvs).toContain('KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF')
    expect(rejectedEnvs).toContain('KHALA_GLM_NVFP4_QUALITY_EVIDENCE_REF')
    expect(rejectedEnvs).toContain('generated_by_cli')
    expect(retained).not.toContain('pilot.internal.example.invalid')
    expect(retained).not.toContain('/Users/operator')
    expect(retained).not.toContain('prompt.private.quality')
    expect(retained).not.toContain('wallet.private.boot.load')
  })
})
