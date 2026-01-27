import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type SwitchSize = "sm" | "default"

export type SwitchProps = {
  readonly className?: string
  readonly size?: SwitchSize
  readonly checked?: boolean
  readonly disabled?: boolean
}

export const Switch = ({
  className,
  size = "default",
  checked = false,
  disabled = false,
}: SwitchProps): TemplateResult => {
  const state = checked ? "checked" : "unchecked"
  return html`
    <button
      data-slot="switch"
      data-size="${size}"
      data-state="${state}"
      class="${cx(
        "peer data-[state=checked]:bg-primary data-[state=unchecked]:bg-input focus-visible:border-ring focus-visible:ring-ring/50 dark:data-[state=unchecked]:bg-input/80 group/switch inline-flex shrink-0 items-center rounded-full border border-transparent shadow-xs transition-all outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-[1.15rem] data-[size=default]:w-8 data-[size=sm]:h-3.5 data-[size=sm]:w-6",
        className
      )}"
      type="button"
      ${disabled ? "disabled" : ""}
    >
      <span
        data-slot="switch-thumb"
        data-state="${state}"
        class="bg-background dark:data-[state=unchecked]:bg-foreground dark:data-[state=checked]:bg-primary-foreground pointer-events-none block rounded-full ring-0 transition-transform group-data-[size=default]/switch:size-4 group-data-[size=sm]/switch:size-3 data-[state=checked]:translate-x-[calc(100%-2px)] data-[state=unchecked]:translate-x-0"
      ></span>
    </button>
  `
}
