/**
 * @since 1.0.0
 */
import * as Arr from "effect/Array"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Predicate from "effect/Predicate"
import * as GptTokenizer from "gpt-tokenizer"
import { AiError } from "../../core/AiError.js"
import type * as AiInput from "../../core/AiInput.js"
import * as Tokenizer from "../../core/Tokenizer.js"

/**
 * @since 1.0.0
 * @category Constructors
 */
export const make = (options: { readonly model: string }) =>
  Tokenizer.make({
    tokenize(input) {
      return Effect.try({
        try: () =>
          GptTokenizer.encodeChat(
            Arr.flatMap(input.messages, (message) =>
              Arr.filterMap(
                message.parts as Array<
                  | AiInput.AssistantMessagePart
                  | AiInput.ToolMessagePart
                  | AiInput.UserMessagePart
                >,
                (part) => {
                  if (
                    part._tag === "FilePart" ||
                    part._tag === "FileUrlPart" ||
                    part._tag === "ImagePart" ||
                    part._tag === "ImageUrlPart" ||
                    part._tag === "ReasoningPart" ||
                    part._tag === "RedactedReasoningPart"
                  ) return Option.none()
                  // Build message object conditionally to satisfy exactOptionalPropertyTypes
                  const role = message._tag === "UserMessage" ? "user" : "assistant"
                  const content = part._tag === "TextPart"
                    ? part.text
                    : JSON.stringify(part._tag === "ToolCallPart" ? part.params : part.result)

                  const chatMessage = message._tag === "UserMessage" && Predicate.isNotUndefined(message.userName)
                    ? { role, content, name: message.userName } as const
                    : { role, content } as const

                  return Option.some(chatMessage)
                }
              )),
            options.model as any
          ),
        catch: (cause) =>
          new AiError({
            module: "OpenAiTokenizer",
            method: "tokenize",
            description: "Could not tokenize",
            cause
          })
      })
    }
  })

/**
 * @since 1.0.0
 * @category Layers
 */
export const layer = (options: { readonly model: string }): Layer.Layer<Tokenizer.Tokenizer> =>
  Layer.succeed(Tokenizer.Tokenizer, make(options))
