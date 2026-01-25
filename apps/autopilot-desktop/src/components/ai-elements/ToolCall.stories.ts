import { html } from "../../effuse/template/html"
import { ToolCall } from "./tool-call"

export default {
  title: "ai/ToolCall",
  component: ToolCall,
}

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Running with Input</div>
      ${ToolCall({
        title: "shell_command",
        detail: "{\n  \"command\": \"rg --files\",\n  \"workdir\": \"/repo\"\n}",
        output: "",
        isStreaming: true,
        status: "running",
      })}

      <div class="text-xs text-muted-foreground">Running with Output</div>
      ${ToolCall({
        title: "shell_command",
        detail: "{\n  \"command\": \"rg --files\",\n  \"workdir\": \"/repo\"\n}",
        output: "src/main.ts\nsrc/index.css",
        isStreaming: true,
        status: "running",
      })}

      <div class="text-xs text-muted-foreground">Completed with Input</div>
      ${ToolCall({
        title: "list_mcp_resources",
        detail: "{\n  \"server\": \"local\"\n}",
        isStreaming: false,
        status: "completed",
        durationMs: 1250,
      })}

      <div class="text-xs text-muted-foreground">Completed with Output</div>
      ${ToolCall({
        title: "shell_command",
        detail: "{\n  \"command\": \"rg --files\",\n  \"workdir\": \"/repo\"\n}",
        output: "src/main.ts\nsrc/index.css",
        isStreaming: false,
        status: "completed",
        durationMs: 450,
      })}

      <div class="text-xs text-muted-foreground">Completed, No Input</div>
      ${ToolCall({
        title: "list_mcp_resources",
        output: "[]",
        isStreaming: false,
        status: "completed",
      })}

      <div class="text-xs text-muted-foreground">Completed, No Input/Output</div>
      ${ToolCall({
        title: "noop",
        isStreaming: false,
        status: "completed",
      })}

      <div class="text-xs text-muted-foreground">Default Status (Streaming)</div>
      ${ToolCall({
        title: "shell_command",
        detail: "{\n  \"command\": \"ls\"\n}",
        isStreaming: true,
      })}
    </div>
  `,
}
