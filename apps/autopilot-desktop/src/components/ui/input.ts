import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type InputProps = {
  readonly className?: string
  readonly type?: string
  readonly name?: string
  readonly value?: string
  readonly placeholder?: string
  readonly disabled?: boolean
  readonly id?: string
}

export const Input = ({
  className,
  type = "text",
  name,
  value,
  placeholder,
  disabled = false,
  id,
}: InputProps): TemplateResult => {
  return html`
    <input
      data-slot="input"
      type="${type}"
      name="${name ?? ""}"
      id="${id ?? ""}"
      value="${value ?? ""}"
      placeholder="${placeholder ?? ""}"
      class="${cx(
        "file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        className
      )}"
      ${disabled ? "disabled" : ""}
    />
  `
}
