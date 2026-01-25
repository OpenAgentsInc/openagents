import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { Action } from "../../effuse/ui/index.js"
import { actionAttributes } from "./action-attrs.js"

export type ToggleProps = {
  readonly name: string
  readonly label?: string
  readonly checked?: boolean
  readonly disabled?: boolean
  readonly action?: Action
  readonly trigger?: "change"
}

export const Toggle = ({
  name,
  label,
  checked = false,
  disabled = false,
  action,
  trigger = "change",
}: ToggleProps): TemplateResult => {
  return html`
    <label class="flex items-center gap-2 text-[11px] text-muted-foreground">
      <input
        type="checkbox"
        name="${name}"
        ${checked ? "checked" : ""}
        ${disabled ? "disabled" : ""}
        ${actionAttributes(action, { trigger })}
      />
      <span>${label ?? name}</span>
    </label>
  `
}
