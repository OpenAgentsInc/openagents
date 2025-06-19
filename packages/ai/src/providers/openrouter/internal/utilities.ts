/**
 * @since 1.0.0
 * @internal
 */
import * as ReadonlyArray from "effect/Array"
import * as Option from "effect/Option"
import * as Predicate from "effect/Predicate"
import * as Schema from "effect/Schema"
import type * as AiInput from "../../../core/AiInput.js"
import type * as AiResponse from "../../../core/AiResponse.js"

export const ProviderMetadataKey = "openrouter" as const

export const resolveFinishReason = (reason: string): AiResponse.FinishReason => {
  switch (reason) {
    case "stop":
      return "stop"
    case "length":
      return "length"
    case "content_filter":
      return "content-filter"
    case "tool_calls":
    case "function_call":
      return "tool-calls"
    default:
      return "unknown"
  }
}

export const ImageDetail = Schema.Literal("auto", "low", "high")

export const MessageContentPart = Schema.Union(
  Schema.Struct({
    type: Schema.Literal("text"),
    text: Schema.String
  }),
  Schema.Struct({
    type: Schema.Literal("image_url"),
    image_url: Schema.Struct({
      url: Schema.String,
      detail: Schema.optional(ImageDetail)
    })
  })
)

export const convertToOpenRouterMessages = (
  messages: ReadonlyArray<AiInput.Message>
): ReadonlyArray<unknown> =>
  ReadonlyArray.filterMap(messages, (message) => {
    if (message._tag === "UserMessage") {
      const parts = message.parts
      if (parts.length === 0) {
        return Option.none()
      }

      const content: Array<{ type: string; text?: string; image_url?: { url: string; detail?: string } }> = []

      for (const part of parts) {
        if (part._tag === "TextPart") {
          content.push({
            type: "text",
            text: part.text
          })
        } else if (part._tag === "ImagePart") {
          content.push({
            type: "image_url",
            image_url: {
              url: `data:${part.mediaType || "image/png"};base64,${Buffer.from(part.data).toString("base64")}`
            }
          })
        } else if (part._tag === "ImageUrlPart") {
          content.push({
            type: "image_url",
            image_url: {
              url: part.url.toString()
            }
          })
        } else if (part._tag === "FilePart") {
          content.push({
            type: "image_url",
            image_url: {
              url: `data:${part.mediaType || "application/octet-stream"};base64,${
                Buffer.from(part.data).toString("base64")
              }`
            }
          })
        } else if (part._tag === "FileUrlPart") {
          content.push({
            type: "image_url",
            image_url: {
              url: part.url.toString()
            }
          })
        }
      }

      if (content.length === 0) {
        return Option.none()
      } else if (content.length === 1 && content[0].type === "text") {
        return Option.some({
          role: "user",
          content: content[0].text!,
          name: message.userName
        })
      } else {
        return Option.some({
          role: "user",
          content,
          name: message.userName
        })
      }
    } else if (message._tag === "AssistantMessage") {
      const parts = message.parts
      const toolCalls: Array<{
        id: string
        type: "function"
        function: { name: string; arguments: string }
      }> = []

      let textContent = ""

      for (const part of parts) {
        if (part._tag === "TextPart") {
          textContent += part.text
        } else if (part._tag === "ToolCallPart") {
          toolCalls.push({
            id: part.id,
            type: "function",
            function: {
              name: part.name,
              arguments: JSON.stringify(part.params)
            }
          })
        }
      }

      if (toolCalls.length > 0) {
        return Option.some({
          role: "assistant",
          content: textContent || null,
          tool_calls: toolCalls
        })
      } else if (textContent) {
        return Option.some({
          role: "assistant",
          content: textContent
        })
      } else {
        return Option.none()
      }
    } else if (message._tag === "ToolMessage") {
      const parts = message.parts
      if (parts.length === 0) {
        return Option.none()
      }

      const results: Array<unknown> = []

      for (const part of parts) {
        if (part._tag === "ToolCallResultPart") {
          results.push({
            tool_call_id: part.id,
            role: "tool",
            content: Predicate.isString(part.result) ? part.result : JSON.stringify(part.result)
          })
        }
      }

      return results.length > 0 ? Option.some(results) : Option.none()
    }

    return Option.none()
  }).flat()
