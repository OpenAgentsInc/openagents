import { Duration, Effect, Exit, Schedule, Schema as S } from 'effect'

import type {
  InferenceMessage,
  InferenceProviderAdapter,
  InferenceToolCall,
  InferenceUsage,
} from './inference/provider-adapter'
import { parseJsonUnknown } from './json-boundary'

export const SARAH_AGENT_MAX_TOOL_ROUNDS = 6
export const SARAH_AGENT_TOOL_RESULT_MAX_CHARS = 8_000
export const SARAH_AGENT_INFERENCE_RETRY_COUNT = 1
export const SARAH_AGENT_INFERENCE_RETRY_DELAY_MS = 250

export type SarahAgentToolDefinition = Readonly<{
  name: string
  description: string
  parameters: Readonly<Record<string, unknown>>
}>

export type SarahAgentToolResult = Readonly<{
  content: string
  summary: string
  authorityReceiptRef: string
  resultRefs: ReadonlyArray<string>
  authorityAllowed?: boolean | undefined
  isError?: boolean | undefined
}>

export class SarahAgentToolError extends S.TaggedErrorClass<SarahAgentToolError>()(
  'SarahAgentToolError',
  { reason: S.String },
) {}

export type SarahAgentTool = Readonly<{
  definition: SarahAgentToolDefinition
  execute: (
    args: unknown,
    toolCall: InferenceToolCall,
  ) => Effect.Effect<SarahAgentToolResult, SarahAgentToolError>
}>

export type SarahAgentToolActivity = Readonly<{
  phase: 'started' | 'succeeded' | 'failed'
  toolCallId: string
  toolName: string
  summary: string
  authorityReceiptRef?: string | undefined
  resultRefs: ReadonlyArray<string>
  authorityAllowed?: boolean | undefined
}>

export class SarahAgentRuntimeError extends S.TaggedErrorClass<SarahAgentRuntimeError>()(
  'SarahAgentRuntimeError',
  { reason: S.String },
) {}

export type SarahAgentTurnResult = Readonly<{
  text: string
  usage: InferenceUsage
  toolCallCount: number
  servedModel: string
}>

const parseArguments = (raw: string): unknown => {
  if (raw.trim() === '') return {}
  try {
    return parseJsonUnknown(raw)
  } catch {
    return {}
  }
}

const boundToolResult = (value: string): string =>
  value.length <= SARAH_AGENT_TOOL_RESULT_MAX_CHARS
    ? value
    : `${value.slice(0, SARAH_AGENT_TOOL_RESULT_MAX_CHARS)}\n(…bounded tool result)`

const addUsage = (
  left: InferenceUsage,
  right: InferenceUsage,
): InferenceUsage => {
  const cachedPromptTokens =
    (left.cachedPromptTokens ?? 0) + (right.cachedPromptTokens ?? 0)
  const reasoningTokens =
    (left.reasoningTokens ?? 0) + (right.reasoningTokens ?? 0)
  return {
    completionTokens: left.completionTokens + right.completionTokens,
    promptTokens: left.promptTokens + right.promptTokens,
    totalTokens: left.totalTokens + right.totalTokens,
    ...(cachedPromptTokens > 0 ? { cachedPromptTokens } : {}),
    ...(reasoningTokens > 0 ? { reasoningTokens } : {}),
  }
}

const ZERO_USAGE: InferenceUsage = {
  completionTokens: 0,
  promptTokens: 0,
  totalTokens: 0,
}

const definitionsFor = (
  tools: ReadonlyArray<SarahAgentTool>,
): ReadonlyArray<Record<string, unknown>> =>
  tools.map(tool => ({
    function: tool.definition,
    type: 'function',
  }))

const runtimeFailure = (reason: string): SarahAgentRuntimeError =>
  new SarahAgentRuntimeError({ reason })

/** Bounded Sarah agent loop over one normalized inference adapter. Tool calls
 * execute sequentially so private runtime activity and target receipts retain
 * causal order. The last round suppresses tools and forces a conversational
 * answer from the gathered results. */
export const runSarahAgentTurn = (
  input: Readonly<{
    adapter: InferenceProviderAdapter
    model: string
    system: string
    prompt: string
    tools: ReadonlyArray<SarahAgentTool>
    onToolActivity?:
      ((activity: SarahAgentToolActivity) => Effect.Effect<void>) | undefined
  }>,
): Effect.Effect<SarahAgentTurnResult, SarahAgentRuntimeError> =>
  Effect.gen(function* () {
    const toolDefinitions = definitionsFor(input.tools)
    let messages: ReadonlyArray<InferenceMessage> = [
      { content: input.system, role: 'system' },
      { content: input.prompt, role: 'user' },
    ]
    let usage = ZERO_USAGE
    let toolCallCount = 0
    let servedModel = input.model
    const completedToolCalls = new Map<string, SarahAgentToolResult>()

    for (let round = 0; round <= SARAH_AGENT_MAX_TOOL_ROUNDS; round += 1) {
      const advertiseTools =
        toolDefinitions.length > 0 && round < SARAH_AGENT_MAX_TOOL_ROUNDS
      const completion = yield* input.adapter
        .complete({
          messages,
          model: input.model,
          passthroughParams: {
            max_tokens: 2_048,
            temperature: 0.3,
            ...(advertiseTools
              ? { tool_choice: 'auto', tools: toolDefinitions }
              : { tool_choice: 'none' }),
          },
          stream: false,
        })
        .pipe(
          Effect.retry({
            schedule: Schedule.spaced(
              Duration.millis(SARAH_AGENT_INFERENCE_RETRY_DELAY_MS),
            ),
            times: SARAH_AGENT_INFERENCE_RETRY_COUNT,
            while: error => error.retryable,
          }),
          Effect.mapError(error => runtimeFailure(error.reason)),
        )
      usage = addUsage(usage, completion.usage)
      servedModel = completion.servedModel
      const requested = completion.toolCalls ?? []
      if (requested.length === 0 || !advertiseTools) {
        if (completion.content.trim() === '') {
          return yield* runtimeFailure('sarah_agent_empty_reply')
        }
        return {
          servedModel,
          text: completion.content,
          toolCallCount,
          usage,
        }
      }

      const nextMessages: Array<InferenceMessage> = [
        ...messages,
        {
          content: completion.content,
          role: 'assistant',
          toolCalls: requested,
        },
      ]

      for (const toolCall of requested) {
        toolCallCount += 1
        const tool = input.tools.find(
          candidate => candidate.definition.name === toolCall.function.name,
        )
        if (tool === undefined) {
          const content = `(unknown Sarah tool: ${toolCall.function.name})`
          if (input.onToolActivity !== undefined) {
            yield* input.onToolActivity({
              phase: 'failed',
              resultRefs: ['blocker.sarah.tool.unknown'],
              summary: content,
              toolCallId: toolCall.id,
              toolName: toolCall.function.name,
            })
          }
          nextMessages.push({
            content,
            name: toolCall.function.name,
            role: 'tool',
            toolCallId: toolCall.id,
          })
          continue
        }

        if (input.onToolActivity !== undefined) {
          yield* input.onToolActivity({
            phase: 'started',
            resultRefs: [],
            summary: `Running ${tool.definition.description}`,
            toolCallId: toolCall.id,
            toolName: tool.definition.name,
          })
        }
        const parsedArguments = parseArguments(toolCall.function.arguments)
        const executionKey = `${tool.definition.name}:${JSON.stringify(parsedArguments)}`
        const cached = completedToolCalls.get(executionKey)
        const executed =
          cached === undefined
            ? yield* Effect.exit(tool.execute(parsedArguments, toolCall))
            : Exit.succeed(cached)
        if (executed._tag === 'Failure') {
          const summary = `${tool.definition.description} could not complete.`
          if (input.onToolActivity !== undefined) {
            yield* input.onToolActivity({
              phase: 'failed',
              resultRefs: ['blocker.sarah.tool.execution_failed'],
              summary,
              toolCallId: toolCall.id,
              toolName: tool.definition.name,
            })
          }
          nextMessages.push({
            content: summary,
            name: tool.definition.name,
            role: 'tool',
            toolCallId: toolCall.id,
          })
          continue
        }
        const result = executed.value
        if (cached === undefined) completedToolCalls.set(executionKey, result)
        if (input.onToolActivity !== undefined) {
          yield* input.onToolActivity({
            authorityReceiptRef: result.authorityReceiptRef,
            authorityAllowed: result.authorityAllowed ?? true,
            phase: result.isError === true ? 'failed' : 'succeeded',
            resultRefs: result.resultRefs,
            summary: result.summary,
            toolCallId: toolCall.id,
            toolName: tool.definition.name,
          })
        }
        nextMessages.push({
          content: boundToolResult(result.content),
          name: tool.definition.name,
          role: 'tool',
          toolCallId: toolCall.id,
        })
      }
      messages = nextMessages
    }

    return yield* runtimeFailure('sarah_agent_tool_round_limit')
  })
