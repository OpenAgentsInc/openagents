import * as Predicate from "effect/Predicate"
import type * as AiResponse from "../../core/AiResponse.js"

/** @internal */
export const ProviderMetadataKey = "@effect/ai-openai/OpenAiLanguageModel/ProviderMetadata"

const finishReasonMap: Record<string, AiResponse.FinishReason> = {
  content_filter: "content-filter",
  function_call: "tool-calls",
  length: "length",
  stop: "stop",
  tool_calls: "tool-calls"
}

/** @internal */
export const resolveFinishReason = (finishReason: string): AiResponse.FinishReason => {
  const reason = finishReasonMap[finishReason]
  return Predicate.isUndefined(reason) ? "unknown" : reason
}
