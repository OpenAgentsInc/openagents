/**
 * @since 1.0.0
 */
import type * as AiInput from "../../../core/AiInput.js"

/**
 * @since 1.0.0
 * @category Utilities
 */
export interface CloudflareMessage {
  readonly role: string
  readonly content: string
}

/**
 * Convert OpenAgents messages to Cloudflare format
 * @since 1.0.0
 * @category Utilities
 */
export const convertToCloudflareMessages = (
  messages: ReadonlyArray<AiInput.Message>
): ReadonlyArray<CloudflareMessage> => {
  const result: Array<CloudflareMessage> = []

  for (const message of messages) {
    switch (message._tag) {
      case "UserMessage": {
        // Handle text content
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        result.push({
          role: "user",
          content: textContent || ""
        })
        break
      }
      case "AssistantMessage": {
        // Handle text content from assistant
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        if (textContent) {
          result.push({
            role: "assistant",
            content: textContent
          })
        }
        break
      }
      case "ToolMessage": {
        // Handle tool messages - convert to assistant for Cloudflare
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        if (textContent) {
          result.push({
            role: "assistant",
            content: textContent
          })
        }
        break
      }
    }
  }

  return result
}

/**
 * @since 1.0.0
 * @category Utilities
 */
export const formatPromptForNativeAPI = (
  messages: ReadonlyArray<AiInput.Message>
): string => {
  const parts: Array<string> = []

  for (const message of messages) {
    switch (message._tag) {
      case "UserMessage": {
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        if (textContent) {
          parts.push(`User: ${textContent}`)
        }
        break
      }
      case "AssistantMessage": {
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        if (textContent) {
          parts.push(`Assistant: ${textContent}`)
        }
        break
      }
      case "ToolMessage": {
        const textContent = message.parts
          .filter((part: any): part is AiInput.TextPart => part._tag === "TextPart")
          .map((part: any) => part.text)
          .join("")

        if (textContent) {
          parts.push(`Tool: ${textContent}`)
        }
        break
      }
    }
  }

  return parts.join("\n")
}

/**
 * @since 1.0.0
 * @category Utilities
 */
export const isValidCloudflareModel = (model: string): boolean => {
  return model.startsWith("@cf/")
}

/**
 * @since 1.0.0
 * @category Utilities
 */
export const getModelProvider = (model: string): string | undefined => {
  const match = model.match(/^@cf\/([^/]+)\//)
  return match?.[1]
}

/**
 * @since 1.0.0
 * @category Utilities
 */
export const getModelName = (model: string): string => {
  const parts = model.split("/")
  return parts[parts.length - 1] || model
}
