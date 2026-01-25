import { html } from "../../effuse/template/html"
import { ConversationContent } from "./conversation"
import { Message } from "./message"
import { Reasoning } from "./reasoning"

export default {
  title: "ai/ConversationContent",
  component: ConversationContent,
}

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Empty</div>
      ${ConversationContent({ children: [] })}

      <div class="text-xs text-muted-foreground">With Messages</div>
      ${ConversationContent({
        children: [
          Message({ role: "user", text: "Show me the latest events." }),
          Message({ role: "assistant", text: "Streaming events now." }),
        ],
      })}

      <div class="text-xs text-muted-foreground">With Reasoning</div>
      ${ConversationContent({
        children: [
          Reasoning({
            summary: "Thinking",
            content: "Working through the request step by step.",
            open: true,
          }),
          Message({
            role: "assistant",
            text: "Here is the formatted output with tokens grouped.",
          }),
        ],
      })}
    </div>
  `,
}
