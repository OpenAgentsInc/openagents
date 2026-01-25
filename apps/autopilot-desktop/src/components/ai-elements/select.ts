import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { Action } from "../../effuse/ui/index.js"
import { actionAttributes } from "./action-attrs.js"

export type SelectOption = {
  readonly label: string
  readonly value: string
}

export type SelectProps = {
  readonly name: string
  readonly label?: string
  readonly value?: string
  readonly options: readonly SelectOption[]
  readonly disabled?: boolean
  readonly action?: Action
  readonly trigger?: "change"
}

export const Select = ({
  name,
  label,
  value,
  options,
  disabled = false,
  action,
  trigger = "change",
}: SelectProps): TemplateResult => {
  const select = html`
    <select
      class="h-9 rounded-md border border-border bg-background px-3 text-[12px] text-foreground outline-none focus-visible:ring-1 focus-visible:ring-accent"
      name="${name}"
      ${disabled ? "disabled" : ""}
      ${actionAttributes(action, { trigger })}
    >
      ${options.map(
        (option) => html`
          <option value="${option.value}" ${option.value === value ? "selected" : ""}>
            ${option.label}
          </option>
        `
      )}
    </select>
  `

  if (!label) {
    return select
  }

  return html`
    <label class="flex flex-col gap-1 text-[10px] uppercase text-muted-foreground">
      <span>${label}</span>
      ${select}
    </label>
  `
}
