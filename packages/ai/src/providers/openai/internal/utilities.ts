import * as Predicate from "effect/Predicate"

/** @internal */
export const ProviderMetadataKey = "@effect/ai-openai/OpenAiLanguageModel/ProviderMetadata"

type FinishReason = "stop" | "length" | "content-filter" | "tool-calls" | "error" | "other" | "unknown"

const finishReasonMap: Record<string, FinishReason> = {
  content_filter: "content-filter",
  function_call: "tool-calls",
  length: "length",
  stop: "stop",
  tool_calls: "tool-calls"
}

/** @internal */
export const resolveFinishReason = (finishReason: string): FinishReason => {
  const reason = finishReasonMap[finishReason]
  return Predicate.isUndefined(reason) ? "unknown" : reason
}
