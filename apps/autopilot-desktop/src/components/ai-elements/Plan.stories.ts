import { html } from "../../effuse/template/html"
import { Plan } from "./plan"

export default {
  title: "ai/Plan",
  component: Plan,
}

const sampleSteps = [
  { step: "Draft the demo goal and expected outcome.", status: "pending" },
  { step: "List the key files or commands the demo would touch.", status: "in_progress" },
  { step: "Define how the demo is verified (test or output).", status: "completed" },
]

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Default</div>
      ${Plan({
        explanation: "3-point demo plan (no execution).",
        steps: sampleSteps,
        open: true,
      })}

      <div class="text-xs text-muted-foreground">Collapsed</div>
      ${Plan({
        explanation: "Collapsed plan example.",
        steps: sampleSteps,
        open: false,
      })}

      <div class="text-xs text-muted-foreground">Streaming</div>
      ${Plan({
        explanation: "Streaming plan update.",
        steps: sampleSteps.slice(0, 2),
        isStreaming: true,
        open: false,
      })}

      <div class="text-xs text-muted-foreground">Empty</div>
      ${Plan({
        explanation: "",
        steps: [],
        open: true,
      })}
    </div>
  `,
}
