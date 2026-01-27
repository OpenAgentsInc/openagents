import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type TextareaProps = {
  readonly className?: string
  readonly name?: string
  readonly value?: string
  readonly placeholder?: string
  readonly disabled?: boolean
  readonly id?: string
  readonly rows?: number
}

export const Textarea = ({
  className,
  name,
  value,
  placeholder,
  disabled = false,
  id,
  rows,
}: TextareaProps): TemplateResult => {
  return html`
    <textarea
      data-slot="textarea"
      name="${name ?? ""}"
      id="${id ?? ""}"
      placeholder="${placeholder ?? ""}"
      rows="${rows ?? ""}"
      class="${cx(
        "border-input placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:bg-input/30 flex field-sizing-content min-h-16 w-full rounded-md border bg-transparent px-3 py-2 text-base shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
        className
      )}"
      ${disabled ? "disabled" : ""}
    >${value ?? ""}</textarea>
  `
}
