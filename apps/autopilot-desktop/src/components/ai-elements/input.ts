import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { Action } from "../../effuse/ui/index.js"
import { actionAttributes } from "./action-attrs.js"

export type InputProps = {
  readonly name: string
  readonly label?: string
  readonly value?: string
  readonly placeholder?: string
  readonly type?: "text" | "number" | "password" | "search"
  readonly disabled?: boolean
  readonly action?: Action
  readonly trigger?: "change" | "input"
}

export const Input = ({
  name,
  label,
  value,
  placeholder,
  type = "text",
  disabled = false,
  action,
  trigger = "change",
}: InputProps): TemplateResult => {
  const input = html`
    <input
      class="h-9 rounded-md border border-border bg-background px-3 text-[12px] text-foreground outline-none focus-visible:ring-1 focus-visible:ring-accent"
      name="${name}"
      type="${type}"
      value="${value ?? ""}"
      placeholder="${placeholder ?? ""}"
      ${disabled ? "disabled" : ""}
      ${actionAttributes(action, { trigger })}
    />
  `

  if (!label) {
    return input
  }

  return html`
    <label class="flex flex-col gap-1 text-[10px] uppercase text-muted-foreground">
      <span>${label}</span>
      ${input}
    </label>
  `
}
