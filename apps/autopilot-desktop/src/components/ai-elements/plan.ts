import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { CodexPlan, CodexPlanStep } from "../../types/codex.js"
import { renderMarkdown, renderInlineMarkdown } from "./markdown.js"

export type PlanProps = Pick<CodexPlan, "explanation" | "steps"> & {
  readonly isStreaming?: boolean
  readonly open?: boolean
}

const formatStatus = (status: string) =>
  status.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase())

const statusTone = (status: string) => {
  switch (status) {
    case "completed":
      return "text-status-connected border-status-connected/40"
    case "in_progress":
      return "text-accent border-accent/40"
    case "pending":
    default:
      return "text-muted-foreground border-border"
  }
}

const getHeaderStatus = (steps: CodexPlanStep[]) => {
  if (!steps.length) {
    return "Pending"
  }
  const hasInProgress = steps.some((step) => step.status === "in_progress")
  const allCompleted = steps.every((step) => step.status === "completed")
  if (hasInProgress) {
    return "In Progress"
  }
  if (allCompleted) {
    return "Completed"
  }
  return "Pending"
}

export const Plan = ({
  explanation,
  steps,
  isStreaming = false,
  open,
}: PlanProps): TemplateResult => {
  const headerStatus = getHeaderStatus(steps)
  return html`
    <details
      class="border border-border bg-background px-3 py-2 text-xs text-foreground"
      ${open || isStreaming ? "open" : ""}
    >
      <summary class="flex cursor-pointer list-none items-center justify-between text-[10px] text-muted-foreground">
        <span class="flex items-center gap-2">
          <span
            class="inline-flex h-2 w-2 rounded-full ${
              isStreaming ? "bg-accent" : "bg-muted-foreground"
            }"
          ></span>
          ${renderInlineMarkdown(`Plan (${steps.length})`)}
        </span>
        <span class="text-[10px] font-semibold text-muted-foreground">
          ${headerStatus}
        </span>
      </summary>
      ${
        explanation
          ? html`<div class="mt-2 text-xs text-muted-foreground">
              ${renderMarkdown(explanation)}
            </div>`
          : ""
      }
      ${
        steps.length
          ? html`<ol class="mt-3 space-y-2">
              ${steps.map(
                (step, index) => html`<li class="flex items-start gap-3">
                  <span class="mt-[2px] w-5 text-[10px] text-muted-foreground"
                    >${index + 1}.</span
                  >
                  <div class="flex flex-1 items-start gap-2">
                    <span
                      class="rounded border px-1.5 py-[2px] text-[10px] ${statusTone(step.status)}"
                    >
                      ${formatStatus(step.status)}
                    </span>
                    <div class="flex-1 text-foreground">
                      ${renderMarkdown(step.step)}
                    </div>
                  </div>
                </li>`
              )}
            </ol>`
          : html`<div class="mt-2 text-xs text-muted-foreground">
              No plan steps yet.
            </div>`
      }
    </details>
  `
}
