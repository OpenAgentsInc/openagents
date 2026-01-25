import { html } from "../../effuse/template/html"
import { ConversationEmptyState } from "./conversation"

export default {
  title: "ai/ConversationEmptyState",
  component: ConversationEmptyState,
}

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Default</div>
      ${ConversationEmptyState({})}

      <div class="text-xs text-muted-foreground">Custom Copy</div>
      ${ConversationEmptyState({
        title: "No stream yet",
        description: "Send a prompt to start receiving events.",
      })}
    </div>
  `,
}
