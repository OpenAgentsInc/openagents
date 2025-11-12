import type { ThreadMessageLike, ThreadAssistantMessagePart, ThreadUserMessagePart } from "@/ui/types/thread";
import type { ContentBlock } from "@agentclientprotocol/sdk";

export function convertACPToThreadMessage(acpMessage: { id: string; role: "user" | "assistant" | "system"; content: ContentBlock[]; createdAt?: string; metadata?: unknown }): ThreadMessageLike {
  const base = { id: acpMessage.id, createdAt: acpMessage.createdAt ? new Date(acpMessage.createdAt) : new Date() };
  if (acpMessage.role === "user") {
    return { ...base, role: "user", content: acpMessage.content.map(toUserPart) } as const;
  }
  if (acpMessage.role === "assistant") {
    return { ...base, role: "assistant", content: acpMessage.content.map(toAssistantPart) } as const;
  }
  return { ...base, role: "system", content: acpMessage.content.filter((c) => c.type === "text").map((c) => ({ type: "text", text: (c as any).text })) } as const;
}

function toUserPart(content: ContentBlock): ThreadUserMessagePart {
  switch (content.type) {
    case "text":
      return { type: "text", text: content.text };
    case "image":
      return { type: "image", image: content.data.startsWith("data:") ? content.data : `data:${content.mimeType};base64,${content.data}` } as any;
    default:
      return { type: "text", text: "[Unsupported content]" };
  }
}

function toAssistantPart(content: ContentBlock): ThreadAssistantMessagePart {
  switch (content.type) {
    case "text":
      return { type: "text", text: content.text };
    case "image":
      return { type: "image", image: content.data.startsWith("data:") ? content.data : `data:${content.mimeType};base64,${content.data}` } as any;
    default:
      return { type: "text", text: "[Unsupported content]" } as any;
  }
}
