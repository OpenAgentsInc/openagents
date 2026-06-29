import { recordFromUnknown } from '../json-boundary'
import type {
  InferenceMessage,
  InferenceToolCall,
  InferenceToolCallDelta,
} from './provider-adapter'

const presentString = (value: unknown): string | undefined =>
  typeof value === 'string' && value !== '' ? value : undefined

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' ? value : undefined

export const inferenceToolCallsFromUnknown = (
  value: unknown,
): ReadonlyArray<InferenceToolCall> | undefined => {
  if (!Array.isArray(value)) {
    return undefined
  }
  const parsed: Array<InferenceToolCall> = []
  for (const item of value) {
    const record = recordFromUnknown(item)
    const fn = recordFromUnknown(record?.['function'])
    const id = presentString(record?.['id'])
    const type = record?.['type']
    const name = presentString(fn?.['name'])
    const args = stringValue(fn?.['arguments'])
    if (
      id === undefined ||
      type !== 'function' ||
      name === undefined ||
      args === undefined
    ) {
      return undefined
    }
    parsed.push({
      function: { arguments: args, name },
      id,
      type: 'function',
    })
  }
  return parsed
}

export const inferenceToolCallDeltasFromUnknown = (
  value: unknown,
): ReadonlyArray<InferenceToolCallDelta> | undefined => {
  if (!Array.isArray(value)) {
    return undefined
  }
  const parsed: Array<InferenceToolCallDelta> = []
  for (const item of value) {
    const record = recordFromUnknown(item)
    if (record === undefined) {
      return undefined
    }
    const index = record['index']
    if (
      typeof index !== 'number' ||
      !Number.isInteger(index) ||
      index < 0
    ) {
      return undefined
    }
    const fn = recordFromUnknown(record['function'])
    const id = presentString(record['id'])
    const type = record['type'] === 'function' ? 'function' : undefined
    const name = stringValue(fn?.['name'])
    const args = stringValue(fn?.['arguments'])
    const delta: InferenceToolCallDelta = {
      index,
      ...(id === undefined ? {} : { id }),
      ...(type === undefined ? {} : { type }),
      ...(name === undefined && args === undefined
        ? {}
        : {
            function: {
              ...(name === undefined ? {} : { name }),
              ...(args === undefined ? {} : { arguments: args }),
            },
          }),
    }
    if (
      delta.id === undefined &&
      delta.type === undefined &&
      delta.function === undefined
    ) {
      return undefined
    }
    parsed.push(delta)
  }
  return parsed
}

export const openAiWireMessageFromInferenceMessage = (
  message: InferenceMessage,
): Record<string, unknown> => ({
  content: message.content,
  role: message.role,
  ...(message.name === undefined ? {} : { name: message.name }),
  ...(message.toolCallId === undefined
    ? {}
    : { tool_call_id: message.toolCallId }),
  ...(message.toolCalls === undefined || message.toolCalls.length === 0
    ? {}
    : { tool_calls: message.toolCalls }),
})
