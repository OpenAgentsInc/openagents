import {
  GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
  GLM_NVFP4_PILOT_MODEL,
  summarizeGlmNvfp4PilotResult,
  type GlmNvfp4PilotConfig,
  type GlmNvfp4PilotEvidenceRefField,
  type GlmNvfp4PilotPublicSummary,
  type GlmNvfp4PilotResult,
} from './glm-nvfp4-pilot'

export const GLM_NVFP4_PILOT_OPERATOR_BUNDLE_SCHEMA =
  'openagents.khala.glm_nvfp4_pilot_operator_bundle.v1' as const

export type GlmNvfp4PilotOperatorInput = Readonly<{
  env: string
  flag: string
  label: string
  status: 'missing' | 'rejected_unsafe'
}>

export type GlmNvfp4PilotOwnerArmedCommandInput = Readonly<{
  outputDir?: string | undefined
  samples?: number | undefined
  measuredMaxModelLen?: number | undefined
}>

export type GlmNvfp4PilotOperatorBundle = Readonly<{
  schemaVersion: typeof GLM_NVFP4_PILOT_OPERATOR_BUNDLE_SCHEMA
  generatedAt: string
  issueRef: 'github.issue.OpenAgentsInc.openagents.6323'
  publicSafe: true
  result: GlmNvfp4PilotResult
  summary: GlmNvfp4PilotPublicSummary
  missingOperatorInputs: ReadonlyArray<GlmNvfp4PilotOperatorInput>
  ownerArmedCommand: string
  retentionNotes: ReadonlyArray<string>
}>

const blank = (value: string | null | undefined): boolean =>
  value === null || value === undefined || value.trim() === ''

const positiveNumberOrUndefined = (
  value: number | null | undefined,
): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) && value > 0
    ? value
    : undefined

const DEFAULT_PUBLIC_OUTPUT_DIR = '.pilot-evidence/glm-nvfp4-6323'
const safeRelativeOutputDirPattern =
  /^(?:\.?[a-z0-9][a-z0-9._-]*)(?:\/[a-z0-9][a-z0-9._-]*){0,8}$/i

const publicSafeOutputDirForCommand = (
  outputDir: string | undefined,
): string => {
  const trimmed = outputDir?.trim()
  if (
    trimmed === undefined ||
    trimmed === '' ||
    trimmed.startsWith('/') ||
    trimmed.startsWith('~') ||
    trimmed.includes('..') ||
    trimmed.includes('://') ||
    !safeRelativeOutputDirPattern.test(trimmed)
  ) {
    return DEFAULT_PUBLIC_OUTPUT_DIR
  }
  return trimmed
}

const evidenceInputByField: Record<
  GlmNvfp4PilotEvidenceRefField,
  Readonly<{
    env: string
    flag: string
    label: string
  }>
> = {
  ownerApprovalRef: {
    env: 'KHALA_GLM_NVFP4_OWNER_APPROVAL_REF',
    flag: '--owner-approval-ref',
    label: 'owner public approval ref',
  },
  endpointRef: {
    env: 'KHALA_GLM_NVFP4_ENDPOINT_REF',
    flag: '--endpoint-ref',
    label: 'public endpoint ref for the isolated pilot endpoint',
  },
  decisionRef: {
    env: 'KHALA_GLM_NVFP4_DECISION_REF',
    flag: '--decision-ref',
    label: 'public #6323 decision ref',
  },
  measuredMaxModelLenEvidenceRef: {
    env: 'KHALA_GLM_NVFP4_MAX_MODEL_LEN_EVIDENCE_REF',
    flag: '--measured-max-model-len-evidence-ref',
    label: 'public measured max-model-len evidence ref',
  },
  qualityEvidenceRef: {
    env: 'KHALA_GLM_NVFP4_QUALITY_EVIDENCE_REF',
    flag: '--quality-evidence-ref',
    label: 'public quality parity evidence ref',
  },
  toolLoopEvidenceRef: {
    env: 'generated_by_cli',
    flag: 'generated_by_cli',
    label: 'public tool-loop evidence ref produced by the pilot CLI',
  },
  throughputEvidenceRef: {
    env: 'generated_by_cli',
    flag: 'generated_by_cli',
    label: 'public throughput evidence ref produced by the pilot CLI',
  },
}

const missingInput = (
  input: Omit<GlmNvfp4PilotOperatorInput, 'status'>,
): GlmNvfp4PilotOperatorInput => ({
  ...input,
  status: 'missing',
})

const ownerArmedCommandLines = (
  input: Required<GlmNvfp4PilotOwnerArmedCommandInput>,
): ReadonlyArray<string> => [
  'KHALA_GLM_NVFP4_PILOT_ARM=1 \\',
  'KHALA_GLM_NVFP4_ENDPOINT_URL="<redacted OpenAI-compatible pilot base URL>" \\',
  'KHALA_GLM_NVFP4_API_KEY="<redacted pilot API key>" \\',
  'KHALA_GLM_NVFP4_ENDPOINT_REF="endpoint.public.khala.glm_nvfp4.single_host_8x.<owner-issued>" \\',
  'KHALA_GLM_NVFP4_OWNER_APPROVAL_REF="approval.public.khala.glm_nvfp4.owner_armed.<owner-issued>" \\',
  'KHALA_GLM_NVFP4_DECISION_REF="decision.public.khala.glm_nvfp4.issue_6323.<owner-issued>" \\',
  `KHALA_GLM_NVFP4_MEASURED_MAX_MODEL_LEN=${input.measuredMaxModelLen} \\`,
  'KHALA_GLM_NVFP4_MAX_MODEL_LEN_EVIDENCE_REF="evidence.public.khala.glm_nvfp4.max_model_len.<owner-issued>" \\',
  'KHALA_GLM_NVFP4_QUALITY_PARITY=passed \\',
  'KHALA_GLM_NVFP4_QUALITY_EVIDENCE_REF="evidence.public.khala.glm_nvfp4.quality_parity.<owner-issued>" \\',
  'KHALA_GLM_NVFP4_MODEL="nvidia/GLM-5.2-NVFP4" \\',
  `bun run --cwd apps/openagents.com/workers/api pilot:glm-nvfp4 --arm --samples ${input.samples} --output-dir ${input.outputDir}`,
]

export const buildGlmNvfp4PilotOwnerArmedCommand = (
  input: GlmNvfp4PilotOwnerArmedCommandInput = {},
): string =>
  ownerArmedCommandLines({
    outputDir: publicSafeOutputDirForCommand(input.outputDir),
    samples: input.samples ?? GLM_NVFP4_MIN_TOOL_LOOP_SAMPLES,
    measuredMaxModelLen: input.measuredMaxModelLen ?? 65536,
  }).join('\n')

export const collectGlmNvfp4PilotMissingOperatorInputs = (input: {
  config: GlmNvfp4PilotConfig
  result: GlmNvfp4PilotResult
}): ReadonlyArray<GlmNvfp4PilotOperatorInput> => {
  const missingInputs: Array<GlmNvfp4PilotOperatorInput> = []

  if (input.config.ownerArmed !== true) {
    missingInputs.push(
      missingInput({
        env: 'KHALA_GLM_NVFP4_PILOT_ARM',
        flag: '--arm',
        label: 'owner arm for the isolated pilot run',
      }),
    )
  }
  if (blank(input.config.endpointUrl)) {
    missingInputs.push(
      missingInput({
        env: 'KHALA_GLM_NVFP4_ENDPOINT_URL',
        flag: '--endpoint-url',
        label: 'private OpenAI-compatible base URL for the isolated endpoint',
      }),
    )
  }
  if ((input.config.model ?? GLM_NVFP4_PILOT_MODEL) !== GLM_NVFP4_PILOT_MODEL) {
    missingInputs.push(
      missingInput({
        env: 'KHALA_GLM_NVFP4_MODEL',
        flag: '--model',
        label: 'exact isolated model id nvidia/GLM-5.2-NVFP4',
      }),
    )
  }
  if (
    input.config.measuredMaxModelLen === null ||
    input.config.measuredMaxModelLen === undefined ||
    input.config.measuredMaxModelLen <= 0
  ) {
    missingInputs.push(
      missingInput({
        env: 'KHALA_GLM_NVFP4_MEASURED_MAX_MODEL_LEN',
        flag: '--measured-max-model-len',
        label: 'measured context ceiling on the 8x RTX PRO 6000 host',
      }),
    )
  }
  if (input.config.qualityParity !== 'passed') {
    missingInputs.push(
      missingInput({
        env: 'KHALA_GLM_NVFP4_QUALITY_PARITY',
        flag: '--quality-parity',
        label: 'owner-reviewed quality parity verdict after the real run',
      }),
    )
  }

  input.result.evidenceRefAudit.forEach(row => {
    if (row.status === 'accepted') {
      return
    }
    const operatorInput = evidenceInputByField[row.field]
    missingInputs.push({
      env: operatorInput.env,
      flag: operatorInput.flag,
      label: operatorInput.label,
      status: row.status,
    })
  })

  return [
    ...new Map(
      missingInputs.map(operatorInput => [
        `${operatorInput.env}:${operatorInput.status}`,
        operatorInput,
      ]),
    ).values(),
  ]
}

export const buildGlmNvfp4PilotOperatorBundle = (input: {
  config: GlmNvfp4PilotConfig
  result: GlmNvfp4PilotResult
  outputDir?: string | undefined
}): GlmNvfp4PilotOperatorBundle => {
  const summary = summarizeGlmNvfp4PilotResult(input.result)
  return {
    schemaVersion: GLM_NVFP4_PILOT_OPERATOR_BUNDLE_SCHEMA,
    generatedAt: input.result.generatedAt,
    issueRef: input.result.issueRef,
    publicSafe: true,
    result: input.result,
    summary,
    missingOperatorInputs: collectGlmNvfp4PilotMissingOperatorInputs(input),
    ownerArmedCommand: buildGlmNvfp4PilotOwnerArmedCommand({
      outputDir: input.outputDir,
      samples: input.config.requiredToolLoopSamples,
      measuredMaxModelLen: positiveNumberOrUndefined(
        input.config.measuredMaxModelLen,
      ),
    }),
    retentionNotes: [
      'This bundle intentionally contains public refs, gate summaries, blockers, launch flags, and scrubbed measurements only.',
      'It does not contain endpoint URLs, API keys, raw prompts, raw model output, checkpoint paths, host paths, wallet material, or private traces.',
      'A no_go bundle is evidence that the pilot remains blocked; it is not evidence that nvidia/GLM-5.2-NVFP4 passed.',
      'This bundle does not authorize or perform any live Khala routing change.',
    ],
  }
}

export const formatGlmNvfp4PilotOperatorReadme = (
  bundle: GlmNvfp4PilotOperatorBundle,
): string => {
  const missingInputs =
    bundle.missingOperatorInputs.length === 0
      ? '- none'
      : bundle.missingOperatorInputs
          .map(
            input =>
              `- ${input.env} (${input.flag}): ${input.status}; ${input.label}`,
          )
          .join('\n')

  return [
    '# GLM NVFP4 Pilot Evidence Bundle',
    '',
    `Generated: ${bundle.generatedAt}`,
    `Issue: ${bundle.issueRef}`,
    `Decision: ${bundle.summary.decision}`,
    `Can route coding lane: ${String(bundle.summary.canRouteCodingLane)}`,
    '',
    '## Missing Operator Inputs',
    '',
    missingInputs,
    '',
    '## Owner-Armed Command Template',
    '',
    '```sh',
    bundle.ownerArmedCommand,
    '```',
    '',
    '## Retention Notes',
    '',
    bundle.retentionNotes.map(note => `- ${note}`).join('\n'),
    '',
  ].join('\n')
}
