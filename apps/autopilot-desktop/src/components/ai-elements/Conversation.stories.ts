import { html } from "../../effuse/template/html"
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
} from "./conversation"
import { Message } from "./message"
import { Reasoning } from "./reasoning"

export default {
  title: "ai/Conversation",
  component: Conversation,
}

const sampleThread = ConversationContent({
  children: [
    Message({
      role: "user",
      text: "Summarize the repository layout.",
    }),
    Reasoning({
      summary: "Planning",
      content: "Scanning the root folder and planning the summary.",
      isStreaming: true,
    }),
    Message({
      role: "assistant",
      text: "Root includes Cargo config, Tauri settings, src modules, and generated build artifacts.",
    }),
  ],
})

const compactThread = ConversationContent({
  children: [
    Message({ role: "system", text: "System notice: streaming enabled." }),
    Message({ role: "assistant", text: "Ready to run the next command." }),
  ],
})

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Empty</div>
      ${Conversation({
        children: ConversationContent({ children: ConversationEmptyState({}) }),
      })}

      <div class="text-xs text-muted-foreground">With Messages</div>
      ${Conversation({ children: sampleThread })}

      <div class="text-xs text-muted-foreground">With System Notice</div>
      ${Conversation({ children: compactThread })}

      <div class="text-xs text-muted-foreground">With Inline Content</div>
      ${Conversation({
        children: ConversationContent({
          children: html`<div class="border border-border bg-surface-muted p-3 text-xs text-foreground">
            Inline content block inside conversation.
          </div>`,
        }),
      })}
    </div>
  `,
}
