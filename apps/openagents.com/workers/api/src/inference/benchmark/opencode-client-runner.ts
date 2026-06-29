import type {
  BenchmarkClientSurfaceSample,
  BenchmarkLaneSample,
} from './lane-seam'
import type { BenchmarkCell, BenchmarkLane } from './matrix'

export type OpenCodeProvisionedConfig = Readonly<{
  client: 'opencode'
  lane: BenchmarkLane
  providerId: string
  modelKey: string
  modelId: string
  modelSelector: string
  configRef: string
  availabilityNote: 'real_endpoint' | 'fixture_only'
  opencodeJson: Readonly<Record<string, unknown>>
}>

export type OpenCodeProviderUsage = Readonly<{
  prompt_tokens?: number | undefined
  completion_tokens?: number | undefined
  total_tokens?: number | undefined
}>

export type OpenCodeExtractedUsage = Readonly<{
  promptTokens: number
  completionTokens: number
  totalTokens: number
}>

export type OpenCodeRunObservation = Readonly<{
  usage: OpenCodeProviderUsage
  ttftMs: number
  totalWallClockMs: number
  generationWallClockMs: number
  providerTimeMs: number
  gatewayOverheadMs: number
  verifierTimeMs: number
  costBasisMsat: number
  cachedInputTokens?: number | undefined
  region: string
  verifierVerdict: 'passed' | 'failed'
  scalarReward: number
  toolCallsAttempted: number
  toolCallsSucceeded: number
}>

export const OPENCODE_FIXTURE_CODING_TASK = {
  taskRef: 'gym.fixture.opencode.edit-run-smoke.v1',
  verifierRef: 'verifier.fixture.patch-and-command-succeeded.v1',
  expectedToolCalls: 3,
} as const

export class OpenCodeConfigUnsupportedLaneError extends Error {
  readonly _tag = 'OpenCodeConfigUnsupportedLaneError'
  constructor(lane: BenchmarkLane) {
    super(
      `No OpenCode client-surface config profile is registered for ${lane}.`,
    )
    this.name = 'OpenCodeConfigUnsupportedLaneError'
  }
}

export class OpenCodeUsageExtractionError extends Error {
  readonly _tag = 'OpenCodeUsageExtractionError'
  constructor() {
    super(
      'OpenCode benchmark observations must include prompt_tokens, completion_tokens, and total_tokens from provider usage.',
    )
    this.name = 'OpenCodeUsageExtractionError'
  }
}

const isFiniteNonNegative = (value: number | undefined): value is number =>
  value !== undefined && Number.isFinite(value) && value >= 0

export const extractOpenCodeUsage = (
  usage: OpenCodeProviderUsage,
): OpenCodeExtractedUsage => {
  if (
    !isFiniteNonNegative(usage.prompt_tokens) ||
    !isFiniteNonNegative(usage.completion_tokens) ||
    !isFiniteNonNegative(usage.total_tokens)
  ) {
    throw new OpenCodeUsageExtractionError()
  }
  return {
    promptTokens: usage.prompt_tokens,
    completionTokens: usage.completion_tokens,
    totalTokens: usage.total_tokens,
  }
}

const openAgentsProviderJson = (
  modelKey: string,
  modelId: string,
  displayName: string,
): Readonly<Record<string, unknown>> => ({
  $schema: 'https://opencode.ai/config.json',
  provider: {
    openagents: {
      npm: '@ai-sdk/openai-compatible',
      name: 'OpenAgents',
      options: {
        baseURL: 'https://openagents.com/api/v1',
        apiKey: '{env:OPENAGENTS_API_KEY}',
      },
      models: {
        [modelKey]: {
          name: displayName,
          api: { id: modelId },
          tool_call: true,
        },
      },
    },
  },
  model: `openagents/${modelKey}`,
})

export const provisionOpenCodeConfigForLane = (
  lane: BenchmarkLane,
): OpenCodeProvisionedConfig => {
  switch (lane) {
    case 'khala':
      return {
        client: 'opencode',
        lane,
        providerId: 'openagents',
        modelKey: 'khala',
        modelId: 'openagents/khala',
        modelSelector: 'openagents/khala',
        configRef: 'opencode.config.openagents.khala.v1',
        availabilityNote: 'real_endpoint',
        opencodeJson: openAgentsProviderJson(
          'khala',
          'openagents/khala',
          'Khala',
        ),
      }
    case 'gpt-oss-20b':
      return {
        client: 'opencode',
        lane,
        providerId: 'openagents',
        modelKey: 'gpt-oss-20b',
        modelId: 'openai/gpt-oss-20b',
        modelSelector: 'openagents/gpt-oss-20b',
        configRef: 'opencode.config.openagents.gpt_oss_20b.v1',
        availabilityNote: 'real_endpoint',
        opencodeJson: openAgentsProviderJson(
          'gpt-oss-20b',
          'openai/gpt-oss-20b',
          'GPT-OSS 20B',
        ),
      }
    case 'gpt-oss-120b':
      return {
        client: 'opencode',
        lane,
        providerId: 'openagents',
        modelKey: 'gpt-oss-120b',
        modelId: 'openai/gpt-oss-120b',
        modelSelector: 'openagents/gpt-oss-120b',
        configRef: 'opencode.config.openagents.gpt_oss_120b.v1',
        availabilityNote: 'real_endpoint',
        opencodeJson: openAgentsProviderJson(
          'gpt-oss-120b',
          'openai/gpt-oss-120b',
          'GPT-OSS 120B',
        ),
      }
    case 'glm-52':
      return {
        client: 'opencode',
        lane,
        providerId: 'openagents',
        modelKey: 'glm-52',
        modelId: 'openagents/glm-5.2-reap-504b',
        modelSelector: 'openagents/glm-52',
        configRef: 'opencode.config.openagents.glm_52.v1',
        availabilityNote: 'real_endpoint',
        opencodeJson: openAgentsProviderJson(
          'glm-52',
          'openagents/glm-5.2-reap-504b',
          'GLM 5.2 REAP',
        ),
      }
    case 'bigpickle':
      return {
        client: 'opencode',
        lane,
        providerId: 'opencode',
        modelKey: 'bigpickle',
        modelId: 'bigpickle',
        modelSelector: 'opencode/bigpickle',
        configRef: 'opencode.config.fixture.bigpickle.v1',
        availabilityNote: 'fixture_only',
        opencodeJson: {
          $schema: 'https://opencode.ai/config.json',
          model: 'opencode/bigpickle',
        },
      }
    case 'gemini-free':
      return {
        client: 'opencode',
        lane,
        providerId: 'google',
        modelKey: 'gemini-free',
        modelId: 'gemini-free',
        modelSelector: 'google/gemini-free',
        configRef: 'opencode.config.fixture.gemini_free.v1',
        availabilityNote: 'fixture_only',
        opencodeJson: {
          $schema: 'https://opencode.ai/config.json',
          model: 'google/gemini-free',
        },
      }
    case 'openai-gpt':
      return {
        client: 'opencode',
        lane,
        providerId: 'openai',
        modelKey: 'gpt',
        modelId: 'openai-gpt',
        modelSelector: 'openai/gpt',
        configRef: 'opencode.config.fixture.openai_gpt.v1',
        availabilityNote: 'fixture_only',
        opencodeJson: {
          $schema: 'https://opencode.ai/config.json',
          model: 'openai/gpt',
        },
      }
    case 'claude':
      return {
        client: 'opencode',
        lane,
        providerId: 'anthropic',
        modelKey: 'claude',
        modelId: 'claude',
        modelSelector: 'anthropic/claude',
        configRef: 'opencode.config.fixture.claude.v1',
        availabilityNote: 'fixture_only',
        opencodeJson: {
          $schema: 'https://opencode.ai/config.json',
          model: 'anthropic/claude',
        },
      }
    default:
      throw new OpenCodeConfigUnsupportedLaneError(lane)
  }
}

export const modelIdForBenchmarkCell = (cell: BenchmarkCell): string => {
  if (cell.targetProfile !== undefined) {
    return cell.targetProfile.modelRef
  }
  if (cell.workload !== 'opencode-coding-task') {
    return `${cell.lane}/${cell.engine}`
  }
  return provisionOpenCodeConfigForLane(cell.lane).modelId
}

export const fixtureOpenCodeToolCallsForLane = (
  lane: BenchmarkLane,
): Readonly<{ attempted: number; succeeded: number }> => {
  if (lane === 'bigpickle') {
    return { attempted: 3, succeeded: 2 }
  }
  return { attempted: 3, succeeded: 3 }
}

export const fixtureOpenCodeVerdictForLane = (
  lane: BenchmarkLane,
): OpenCodeRunObservation['verifierVerdict'] =>
  fixtureOpenCodeToolCallsForLane(lane).attempted ===
  fixtureOpenCodeToolCallsForLane(lane).succeeded
    ? 'passed'
    : 'failed'

export const buildOpenCodeFixtureClientSurface = (
  cell: BenchmarkCell,
): BenchmarkClientSurfaceSample | undefined => {
  if (cell.workload !== 'opencode-coding-task') {
    return undefined
  }
  const config = provisionOpenCodeConfigForLane(cell.lane)
  const toolCalls = fixtureOpenCodeToolCallsForLane(cell.lane)
  return {
    client: 'opencode',
    taskRef: OPENCODE_FIXTURE_CODING_TASK.taskRef,
    configRef: config.configRef,
    toolCallsAttempted: toolCalls.attempted,
    toolCallsSucceeded: toolCalls.succeeded,
  }
}

export const openCodeSampleFromObservation = (
  cell: BenchmarkCell,
  observation: OpenCodeRunObservation,
): BenchmarkLaneSample => {
  const usage = extractOpenCodeUsage(observation.usage)
  const clientSurface = buildOpenCodeFixtureClientSurface(cell)
  return {
    promptTokens: usage.promptTokens,
    completionTokens: usage.completionTokens,
    totalTokens: usage.totalTokens,
    cachedInputTokens: observation.cachedInputTokens ?? 0,
    ttftMs: observation.ttftMs,
    totalWallClockMs: observation.totalWallClockMs,
    generationWallClockMs: observation.generationWallClockMs,
    providerTimeMs: observation.providerTimeMs,
    gatewayOverheadMs: observation.gatewayOverheadMs,
    verificationClass: 'test_passed',
    executedVerdict: observation.verifierVerdict,
    scalarReward: observation.scalarReward,
    verifierTimeMs: observation.verifierTimeMs,
    costBasisMsat: observation.costBasisMsat,
    region: observation.region,
    ...(clientSurface === undefined
      ? {}
      : {
          clientSurface: {
            ...clientSurface,
            toolCallsAttempted: observation.toolCallsAttempted,
            toolCallsSucceeded: observation.toolCallsSucceeded,
          },
        }),
  }
}
