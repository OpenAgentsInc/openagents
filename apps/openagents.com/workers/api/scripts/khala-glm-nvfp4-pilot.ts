#!/usr/bin/env bun
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

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
import {
  buildGlmNvfp4PilotOperatorBundle,
  buildGlmNvfp4PilotOwnerArmedCommand,
  formatGlmNvfp4PilotOperatorReadme,
} from '../src/inference/glm-nvfp4-pilot-operator'

const args = process.argv.slice(2)

const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  const value = args[index + 1]
  return value === undefined || value.startsWith('--') ? undefined : value
}

const flag = (name: string): boolean => args.includes(name)

const help = (): string => [
  'Usage: bun run scripts/khala-glm-nvfp4-pilot.ts [options]',
  '',
  'Owner-armed #6323 pilot runner for nvidia/GLM-5.2-NVFP4.',
  'Without owner arm, endpoint, and public refs it emits a public-safe no_go result and exits 2.',
  '',
  'Options:',
  '  --arm',
  '  --endpoint-url <private OpenAI-compatible base URL>',
  '  --endpoint-ref <public ref>',
  '  --owner-approval-ref <public ref>',
  '  --decision-ref <public ref>',
  '  --boot-load-evidence-ref <public ref>',
  '  --measured-max-model-len <number>',
  '  --measured-max-model-len-evidence-ref <public ref>',
  '  --quality-parity <passed|failed|not_measured>',
  '  --quality-evidence-ref <public ref>',
  '  --samples <number>',
  '  --output-dir <dir>       Write public-safe result, summary, bundle, and README files.',
  '  --summary                Print the public summary JSON to stdout instead of the full result.',
  '  --print-owner-command    Print the owner-armed command template and exit 0.',
  '  --help',
  '',
  'Owner-armed command template:',
  '',
  buildGlmNvfp4PilotOwnerArmedCommand(),
  '',
].join('\n')

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
const bootLoadEvidenceRef =
  option('--boot-load-evidence-ref') ??
  Bun.env.KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF ??
  null
const apiKey = Bun.env.KHALA_GLM_NVFP4_API_KEY ?? undefined
const samplesFromEnv = Number(Bun.env.KHALA_GLM_NVFP4_TOOL_LOOP_SAMPLES ?? '')
const samples =
  numberOption('--samples') ??
  (Number.isFinite(samplesFromEnv) && samplesFromEnv > 0
    ? samplesFromEnv
    : GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES)
const outputDir = option('--output-dir')

if (flag('--help')) {
  process.stdout.write(help())
  process.exit(0)
}

if (flag('--print-owner-command')) {
  process.stdout.write(`${buildGlmNvfp4PilotOwnerArmedCommand({
    outputDir,
    samples,
    measuredMaxModelLen:
      numberOption('--measured-max-model-len') ?? undefined,
  })}\n`)
  process.exit(0)
}

const config: GlmNvfp4PilotConfig = {
  generatedAt: new Date().toISOString(),
  ownerArmed:
    flag('--arm') || boolFromEnv(Bun.env.KHALA_GLM_NVFP4_PILOT_ARM),
  ownerApprovalRef,
  endpointUrl,
  endpointRef,
  model: option('--model') ?? Bun.env.KHALA_GLM_NVFP4_MODEL ?? GLM_NVFP4_PILOT_MODEL,
  decisionRef,
  bootLoadEvidenceRef,
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
const bundle = buildGlmNvfp4PilotOperatorBundle({
  config,
  result,
  outputDir,
})

if (outputDir !== undefined) {
  await mkdir(outputDir, { recursive: true })
  await Promise.all([
    Bun.write(
      join(outputDir, 'glm-nvfp4-pilot-result.public.json'),
      `${JSON.stringify(bundle.result, null, 2)}\n`,
    ),
    Bun.write(
      join(outputDir, 'glm-nvfp4-pilot-summary.public.json'),
      `${JSON.stringify(bundle.summary, null, 2)}\n`,
    ),
    Bun.write(
      join(outputDir, 'glm-nvfp4-pilot-operator-bundle.public.json'),
      `${JSON.stringify(bundle, null, 2)}\n`,
    ),
    Bun.write(
      join(outputDir, 'README.public.md'),
      formatGlmNvfp4PilotOperatorReadme(bundle),
    ),
  ])
}

process.stderr.write(
  [
    `[glm-nvfp4-pilot] decision=${bundle.summary.decision}`,
    `[glm-nvfp4-pilot] canRouteCodingLane=${String(bundle.summary.canRouteCodingLane)}`,
    `[glm-nvfp4-pilot] missingOperatorInputs=${bundle.missingOperatorInputs.map(input => input.env).join(', ') || 'none'}`,
    '[glm-nvfp4-pilot] liveRoutingChanged=false',
    ...(outputDir === undefined
      ? []
      : [`[glm-nvfp4-pilot] wrotePublicSafeEvidenceDir=${outputDir}`]),
  ].join('\n') + '\n',
)

process.stdout.write(
  `${JSON.stringify(flag('--summary') ? bundle.summary : result, null, 2)}\n`,
)
if (result.decision !== 'go') {
  process.exitCode = 2
}
