import { html } from "../../effuse/template/html"
import { Message } from "./message"

export default {
  title: "ai/Message",
  component: Message,
}

const baseContent = "Root is a Rust Tauri project with src modules and config files."

export const All = {
  render: () => html`
    <div class="flex flex-col gap-8 p-4">
      <!-- User Group -->
      <div class="space-y-4">
        <div class="text-xs font-medium text-white/30 uppercase tracking-wider">User</div>
        
        <div class="space-y-6">
          ${Message({
            role: "user",
            text: "Show me the latest session summary.",
            isStreaming: false,
          })}

          ${Message({
            role: "user",
            text: "Streaming user prompt...",
            isStreaming: true,
          })}
        </div>
      </div>

      <!-- Assistant Group -->
      <div class="space-y-4">
        <div class="text-xs font-medium text-white/30 uppercase tracking-wider">Assistant</div>
        
        <div class="space-y-6">
          ${Message({
            role: "assistant",
            text: baseContent,
            isStreaming: false,
          })}

          ${Message({
            role: "assistant",
            text: "Streaming response chunk...",
            isStreaming: true,
          })}
        </div>
      </div>

      <!-- System Group -->
      <div class="space-y-4">
        <div class="text-xs font-medium text-white/30 uppercase tracking-wider">System</div>
        
        <div class="space-y-6">
          ${Message({
            role: "system",
            text: "System notice: session connected.",
            isStreaming: false,
          })}

          ${Message({
            role: "system",
            text: "System status updating...",
            isStreaming: true,
          })}
        </div>
      </div>
    </div>
  `,
}
