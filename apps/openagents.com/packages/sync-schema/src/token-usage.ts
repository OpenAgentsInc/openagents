import {
  isRecord,
  nestedUnknown,
  parseEmbeddedJsonRecord,
  parseJsonValue,
} from './json-boundary'

export type AutopilotTokenUsage = Readonly<{
  provider: string | null
  model: string | null
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  cacheReadTokens: number
  cacheWrite5mTokens: number
  cacheWrite1hTokens: number
  totalTokens: number
}>

const asInteger = (value: unknown): number => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value))
  }

  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value)

    return Number.isFinite(parsed) ? Math.max(0, Math.trunc(parsed)) : 0
  }

  return 0
}

const asText = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : null

const firstNumber = (
  value: Record<string, unknown>,
  paths: ReadonlyArray<ReadonlyArray<string>>,
): number => {
  for (const path of paths) {
    const number = asInteger(nestedUnknown(value, path))

    if (number > 0) {
      return number
    }
  }

  return 0
}

const hasPositiveNumber = (
  value: Record<string, unknown>,
  paths: ReadonlyArray<ReadonlyArray<string>>,
): boolean => firstNumber(value, paths) > 0

const firstText = (
  value: Record<string, unknown>,
  paths: ReadonlyArray<ReadonlyArray<string>>,
): string | null => {
  for (const path of paths) {
    const text = asText(nestedUnknown(value, path))

    if (text !== null) {
      return text
    }
  }

  return null
}

const embeddedRecord = (
  value: unknown,
): Record<string, unknown> | undefined => {
  if (isRecord(value)) {
    return value
  }

  if (typeof value === 'string') {
    return parseEmbeddedJsonRecord(value)
  }

  return undefined
}

const expandEmbeddedRecords = (
  record: Record<string, unknown>,
  depth = 0,
): ReadonlyArray<Record<string, unknown>> => {
  if (depth >= 4) {
    return [record]
  }

  const records: Array<Record<string, unknown>> = [record]
  const paths = [
    ['dataJson'],
    ['detail'],
    ['payloadJson'],
    ['payload'],
    ['body'],
    ['event'],
    ['message'],
    ['data'],
  ] as const

  for (const path of paths) {
    const parsed = embeddedRecord(nestedUnknown(record, path))

    if (parsed !== undefined) {
      records.push(...expandEmbeddedRecords(parsed, depth + 1))
    }
  }

  return records
}

const uniqueRecords = (
  records: ReadonlyArray<Record<string, unknown>>,
): ReadonlyArray<Record<string, unknown>> => {
  const seen = new Set<string>()
  const unique: Array<Record<string, unknown>> = []

  for (const record of records) {
    const key = JSON.stringify(record)

    if (!seen.has(key)) {
      seen.add(key)
      unique.push(record)
    }
  }

  return unique
}

const candidateRecords = (
  payload: Record<string, unknown>,
): ReadonlyArray<Record<string, unknown>> => {
  const candidates: Array<Record<string, unknown>> = [payload]
  const paths = [
    ['dataJson'],
    ['detail'],
    ['event'],
    ['payload'],
    ['body'],
    ['usage'],
    ['usageMetadata'],
    ['usage_metadata'],
    ['tokens'],
    ['tokenUsage', 'last'],
    ['token_usage', 'last'],
    ['token_usage', 'last_token_usage'],
    ['lastTokenUsage'],
    ['last_token_usage'],
    ['message', 'usage'],
    ['message', 'tokens'],
    ['message', 'info'],
    ['message', 'info', 'tokens'],
    ['part'],
    ['part', 'usage'],
    ['part', 'tokens'],
    ['response', 'usage'],
    ['response', 'tokens'],
    ['result', 'usage'],
    ['result', 'tokens'],
    ['data'],
    ['data', 'usage'],
    ['data', 'usageMetadata'],
    ['data', 'usage_metadata'],
    ['data', 'tokens'],
    ['model_usage'],
    ['modelUsage'],
  ] as const
  const hasLastTurnUsage =
    isRecord(nestedUnknown(payload, ['lastTokenUsage'])) ||
    isRecord(nestedUnknown(payload, ['last_token_usage'])) ||
    isRecord(nestedUnknown(payload, ['tokenUsage', 'last'])) ||
    isRecord(nestedUnknown(payload, ['token_usage', 'last'])) ||
    isRecord(nestedUnknown(payload, ['token_usage', 'last_token_usage']))
  const totalUsagePaths = hasLastTurnUsage
    ? []
    : ([['totalTokenUsage'], ['total_token_usage']] as const)

  for (const path of [...paths, ...totalUsagePaths]) {
    const value = nestedUnknown(payload, path)

    if (isRecord(value)) {
      candidates.push(value)
    }
  }

  return candidates
}

const usageWeight = (usage: AutopilotTokenUsage): number =>
  usage.inputTokens +
  usage.outputTokens +
  usage.reasoningTokens +
  usage.cacheReadTokens +
  usage.cacheWrite5mTokens +
  usage.cacheWrite1hTokens +
  usage.totalTokens

const normalizeUsage = (
  candidate: Record<string, unknown>,
  root: Record<string, unknown>,
): AutopilotTokenUsage | undefined => {
  const rawInputTokens = firstNumber(candidate, [
    ['input'],
    ['input_tokens'],
    ['inputTokens'],
    ['prompt_tokens'],
    ['promptTokens'],
    ['promptTokenCount'],
    ['prompt_token_count'],
    ['tokens_input'],
  ])
  const geminiCacheReadTokens = firstNumber(candidate, [
    ['cachedContentTokenCount'],
    ['cached_content_token_count'],
  ])
  const inputTokens = Math.max(0, rawInputTokens - geminiCacheReadTokens)
  const outputTokens = firstNumber(candidate, [
    ['output'],
    ['output_tokens'],
    ['outputTokens'],
    ['completion_tokens'],
    ['completionTokens'],
    ['completionTokenCount'],
    ['candidatesTokenCount'],
    ['candidates_token_count'],
    ['tokens_output'],
  ])
  const reasoningTokens = firstNumber(candidate, [
    ['reasoning'],
    ['reasoning_tokens'],
    ['reasoningTokens'],
    ['reasoningTokens'],
    ['reasoning_output_tokens'],
    ['reasoningOutputTokens'],
    ['thoughtsTokenCount'],
    ['thoughts_token_count'],
    ['thinkingTokenCount'],
    ['thinking_token_count'],
    ['tokens_reasoning'],
    ['completion_tokens_details', 'reasoning_tokens'],
    ['completion_tokens_details', 'reasoningTokens'],
    ['output_tokens_details', 'reasoning_tokens'],
    ['output_tokens_details', 'reasoningTokens'],
  ])
  const cacheReadTokens = firstNumber(candidate, [
    ['cache_read'],
    ['cache_read_tokens'],
    ['cacheReadTokens'],
    ['cache_read_input_tokens'],
    ['cacheReadInputTokens'],
    ['cached_input_tokens'],
    ['cachedInputTokens'],
    ['cachedContentTokenCount'],
    ['cached_content_token_count'],
    ['tokens_cache_read'],
    ['cache', 'read'],
    ['input_tokens_details', 'cached_tokens'],
    ['input_tokens_details', 'cachedTokens'],
    ['prompt_tokens_details', 'cached_tokens'],
    ['prompt_tokens_details', 'cachedTokens'],
  ])
  const cacheWrite5mTokens = firstNumber(candidate, [
    ['cache_write_5m'],
    ['cache_write_5m_tokens'],
    ['cacheWrite5mTokens'],
    ['cache_creation', 'ephemeral_5m_input_tokens'],
    ['cacheCreation', 'ephemeral5mInputTokens'],
    ['tokens_cache_write_5m'],
  ])
  const cacheWrite1hTokens =
    firstNumber(candidate, [
      ['cache_write_1h'],
      ['cache_write_1h_tokens'],
      ['cacheWrite1hTokens'],
      ['cache_creation', 'ephemeral_1h_input_tokens'],
      ['cacheCreation', 'ephemeral1hInputTokens'],
      ['tokens_cache_write_1h'],
    ]) ||
    firstNumber(candidate, [
      ['cache_write'],
      ['cache_write_tokens'],
      ['cacheWriteTokens'],
      ['cache_creation_input_tokens'],
      ['cacheCreationInputTokens'],
      ['prompt_tokens_details', 'cache_creation_input_tokens'],
      ['prompt_tokens_details', 'cacheCreationInputTokens'],
      ['tokens_cache_write'],
      ['cache', 'write'],
    ])
  const cacheReadAlreadyInInput = hasPositiveNumber(candidate, [
    ['cached_input_tokens'],
    ['cachedInputTokens'],
    ['input_tokens_details', 'cached_tokens'],
    ['input_tokens_details', 'cachedTokens'],
    ['prompt_tokens_details', 'cached_tokens'],
    ['prompt_tokens_details', 'cachedTokens'],
  ])
  const computedTotal =
    inputTokens +
    outputTokens +
    reasoningTokens +
    (cacheReadAlreadyInInput ? 0 : cacheReadTokens) +
    cacheWrite5mTokens +
    cacheWrite1hTokens
  const explicitTotal = firstNumber(candidate, [
    ['total'],
    ['total_tokens'],
    ['totalTokens'],
    ['totalTokenCount'],
    ['total_token_count'],
    ['tokens_total'],
  ])
  const totalTokens = explicitTotal > 0 ? explicitTotal : computedTotal

  if (
    inputTokens === 0 &&
    outputTokens === 0 &&
    reasoningTokens === 0 &&
    cacheReadTokens === 0 &&
    cacheWrite5mTokens === 0 &&
    cacheWrite1hTokens === 0 &&
    totalTokens === 0
  ) {
    return undefined
  }

  return {
    provider:
      firstText(candidate, [['provider'], ['provider_model']]) ??
      firstText(root, [['provider'], ['provider_model']]),
    model:
      firstText(candidate, [
        ['model'],
        ['modelVersion'],
        ['model_version'],
        ['provider_model'],
        ['providerModel'],
      ]) ??
      firstText(root, [
        ['model'],
        ['modelVersion'],
        ['model_version'],
        ['provider_model'],
        ['providerModel'],
      ]),
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    cacheWrite5mTokens,
    cacheWrite1hTokens,
    totalTokens,
  }
}

export const extractAutopilotTokenUsage = (
  payload: unknown,
): AutopilotTokenUsage | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }

  const roots = uniqueRecords(expandEmbeddedRecords(payload))
  const normalized = roots
    .flatMap(root =>
      candidateRecords(root).map(candidate => normalizeUsage(candidate, root)),
    )
    .filter((usage): usage is AutopilotTokenUsage => usage !== undefined)
    .sort((left, right) => usageWeight(right) - usageWeight(left))

  return normalized[0]
}

export const extractAutopilotTokenUsageFromJson = (
  payloadJson: string | null,
): AutopilotTokenUsage | undefined => {
  if (payloadJson === null) {
    return undefined
  }

  return extractAutopilotTokenUsage(parseJsonValue(payloadJson))
}
