import { html } from "../../effuse/template/html"
import { Reasoning } from "./reasoning"

export default {
  title: "ai/Reasoning",
  component: Reasoning,
}

const sample =
  "Evaluating the request, selecting tools, and preparing a concise response."

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Completed (Closed)</div>
      ${Reasoning({
        summary: "Evaluation",
        content: sample,
        isStreaming: false,
        open: false,
      })}

      <div class="text-xs text-muted-foreground">Completed (Open)</div>
      ${Reasoning({
        summary: "Analysis",
        content: sample,
        isStreaming: false,
        open: true,
      })}

      <div class="text-xs text-muted-foreground">Streaming (Open)</div>
      ${Reasoning({
        summary: "Thinking",
        content: "Streaming reasoning...",
        isStreaming: true,
        open: true,
      })}

      <div class="text-xs text-muted-foreground">Streaming (Auto Open)</div>
      ${Reasoning({
        content: "Streaming reasoning with open=false.",
        isStreaming: true,
        open: false,
      })}

      <div class="text-xs text-muted-foreground">Empty (Closed)</div>
      ${Reasoning({ content: "", isStreaming: false, open: false })}

      <div class="text-xs text-muted-foreground">Empty (Open)</div>
      ${Reasoning({ content: "", isStreaming: false, open: true })}

      <div class="text-xs text-muted-foreground">Empty Streaming (Open)</div>
      ${Reasoning({ content: "", isStreaming: true, open: true })}

      <div class="text-xs text-muted-foreground">Empty Streaming (Auto Open)</div>
      ${Reasoning({ content: "", isStreaming: true, open: false })}
    </div>
  `,
}
