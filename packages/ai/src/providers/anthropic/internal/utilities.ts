import * as Predicate from "effect/Predicate"
import type * as AiResponse from "../../core/AiResponse.js"

/** @internal */
export const ProviderMetadataKey = "@effect/ai-anthropic/AnthropicLanguageModel/ProviderMetadata"

const finishReasonMap: Record<string, AiResponse.FinishReason> = {
  end_turn: "stop",
  max_tokens: "length",
  stop_sequence: "stop",
  tool_use: "tool-calls"
}

/** @internal */
export const resolveFinishReason = (finishReason: string): AiResponse.FinishReason => {
  const reason = finishReasonMap[finishReason]
  return Predicate.isUndefined(reason) ? "unknown" : reason
}
