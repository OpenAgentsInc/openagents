/**
 * @since 1.0.0
 * @internal
 */
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

type OpenRouterMessage = {
  readonly role: string
  readonly content: string | ReadonlyArray<unknown>
  readonly name?: string | undefined
  readonly tool_calls?: ReadonlyArray<unknown> | undefined
  readonly tool_call_id?: string | undefined
}

export const convertToOpenRouterMessages = (
  messages: ReadonlyArray<AiInput.Message>
): ReadonlyArray<OpenRouterMessage> => {
  const result: Array<OpenRouterMessage> = []

  for (const message of messages) {
    if (message._tag === "UserMessage") {
      const parts = message.parts
      if (parts.length === 0) {
        continue
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
        continue
      } else if (content.length === 1 && content[0].type === "text") {
        result.push({
          role: "user",
          content: content[0].text!,
          name: message.userName
        })
      } else {
        result.push({
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
        result.push({
          role: "assistant",
          content: textContent || "",
          tool_calls: toolCalls
        })
      } else if (textContent) {
        result.push({
          role: "assistant",
          content: textContent
        })
      }
    } else if (message._tag === "ToolMessage") {
      const parts = message.parts

      for (const part of parts) {
        if (part._tag === "ToolCallResultPart") {
          result.push({
            tool_call_id: part.id,
            role: "tool",
            content: Predicate.isString(part.result) ? part.result : JSON.stringify(part.result)
          })
        }
      }
    }
  }

  return result
}
