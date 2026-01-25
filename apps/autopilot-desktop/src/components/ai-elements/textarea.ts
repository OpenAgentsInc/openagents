import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { Action } from "../../effuse/ui/index.js"
import { actionAttributes } from "./action-attrs.js"

export type TextAreaProps = {
  readonly name: string
  readonly label?: string
  readonly value?: string
  readonly placeholder?: string
  readonly rows?: number
  readonly disabled?: boolean
  readonly action?: Action
  readonly trigger?: "change" | "input"
}

export const TextArea = ({
  name,
  label,
  value,
  placeholder,
  rows = 4,
  disabled = false,
  action,
  trigger = "change",
}: TextAreaProps): TemplateResult => {
  const field = html`
    <textarea
      class="min-h-[96px] w-full resize-none rounded-md border border-border bg-background px-3 py-2 text-[12px] text-foreground outline-none focus-visible:ring-1 focus-visible:ring-accent"
      name="${name}"
      rows="${rows}"
      placeholder="${placeholder ?? ""}"
      ${disabled ? "disabled" : ""}
      ${actionAttributes(action, { trigger })}
    >${value ?? ""}</textarea>
  `

  if (!label) {
    return field
  }

  return html`
    <label class="flex flex-col gap-1 text-[10px] uppercase text-muted-foreground">
      <span>${label}</span>
      ${field}
    </label>
  `
}
