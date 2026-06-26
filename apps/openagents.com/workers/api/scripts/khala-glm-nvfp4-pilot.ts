#!/usr/bin/env bun
import {
  GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
  GLM_NVFP4_PILOT_MODEL,
  GlmNvfp4PilotNotArmedError,
  buildGlmNvfp4PilotResult,
  collectGlmNvfp4PilotObservation,
  makeOpenAiCompatibleGlmNvfp4PilotExecutor,
  type GlmNvfp4PilotConfig,
  type GlmNvfp4PilotObservation,
} from '../src/inference/glm-nvfp4-pilot'

const args = process.argv.slice(2)

const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  const value = args[index + 1]
  return value === undefined || value.startsWith('--') ? undefined : value
}

const flag = (name: string): boolean => args.includes(name)

const numberOption = (name: string): number | null => {
  const value = option(name)
  if (value === undefined) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

const boolFromEnv = (value: string | undefined): boolean =>
  value === '1' || value === 'true' || value === 'yes'

const endpointUrl =
  option('--endpoint-url') ?? Bun.env.KHALA_GLM_NVFP4_ENDPOINT_URL ?? null
const endpointRef =
  option('--endpoint-ref') ?? Bun.env.KHALA_GLM_NVFP4_ENDPOINT_REF ?? null
const ownerApprovalRef =
  option('--owner-approval-ref') ??
  Bun.env.KHALA_GLM_NVFP4_OWNER_APPROVAL_REF ??
  null
const decisionRef =
  option('--decision-ref') ?? Bun.env.KHALA_GLM_NVFP4_DECISION_REF ?? null
const apiKey = Bun.env.KHALA_GLM_NVFP4_API_KEY ?? undefined
const samplesFromEnv = Number(Bun.env.KHALA_GLM_NVFP4_TOOL_LOOP_SAMPLES ?? '')
const samples =
  numberOption('--samples') ??
  (Number.isFinite(samplesFromEnv) && samplesFromEnv > 0
    ? samplesFromEnv
    : GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES)

const config: GlmNvfp4PilotConfig = {
  generatedAt: new Date().toISOString(),
  ownerArmed:
    flag('--arm') || boolFromEnv(Bun.env.KHALA_GLM_NVFP4_PILOT_ARM),
  ownerApprovalRef,
  endpointUrl,
  endpointRef,
  model: option('--model') ?? Bun.env.KHALA_GLM_NVFP4_MODEL ?? GLM_NVFP4_PILOT_MODEL,
  decisionRef,
  measuredMaxModelLen:
    numberOption('--measured-max-model-len') ??
    (() => {
      const value = Number(Bun.env.KHALA_GLM_NVFP4_MEASURED_MAX_MODEL_LEN ?? '')
      return Number.isFinite(value) && value > 0 ? value : null
    })(),
  measuredMaxModelLenEvidenceRef:
    option('--measured-max-model-len-evidence-ref') ??
    Bun.env.KHALA_GLM_NVFP4_MAX_MODEL_LEN_EVIDENCE_REF ??
    null,
  qualityParity:
    (option('--quality-parity') ??
      Bun.env.KHALA_GLM_NVFP4_QUALITY_PARITY ??
      'not_measured') as GlmNvfp4PilotConfig['qualityParity'],
  qualityEvidenceRef:
    option('--quality-evidence-ref') ??
    Bun.env.KHALA_GLM_NVFP4_QUALITY_EVIDENCE_REF ??
    null,
  reapBaselineTps:
    numberOption('--reap-baseline-tps') ??
    (() => {
      const value = Number(Bun.env.KHALA_GLM_NVFP4_REAP_BASELINE_TPS ?? '')
      return Number.isFinite(value) && value > 0 ? value : null
    })(),
  requiredToolLoopSamples: samples,
}

const callEndpoint = async (
  path: string,
  body: Record<string, unknown>,
): Promise<Record<string, unknown>> => {
  if (endpointUrl === null || endpointUrl.trim() === '') {
    throw new GlmNvfp4PilotNotArmedError()
  }
  const response = await fetch(new URL(path, endpointUrl).toString(), {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(apiKey === undefined || apiKey.trim() === ''
        ? {}
        : { authorization: `Bearer ${apiKey}` }),
    },
    body: JSON.stringify(body),
  })
  const json = await response.json().catch(() => ({}))
  if (!response.ok) {
    return {
      error: {
        type: 'pilot_endpoint_http_error',
        status: response.status,
      },
    }
  }
  return typeof json === 'object' && json !== null
    ? (json as Record<string, unknown>)
    : {}
}

let observation: GlmNvfp4PilotObservation | undefined = undefined
try {
  observation = await collectGlmNvfp4PilotObservation({
    config,
    executor: makeOpenAiCompatibleGlmNvfp4PilotExecutor({
      http: callEndpoint,
      samples,
      evidenceSeed: config.generatedAt,
    }),
  })
} catch (error) {
  if (!(error instanceof GlmNvfp4PilotNotArmedError)) {
    throw error
  }
}

const result = buildGlmNvfp4PilotResult({ config, observation })
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
if (result.decision !== 'go') {
  process.exitCode = 2
}
