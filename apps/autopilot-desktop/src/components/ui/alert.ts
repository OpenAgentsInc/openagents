import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type AlertVariant = "default" | "destructive"

export type AlertProps = {
  readonly className?: string
  readonly variant?: AlertVariant
  readonly children?: UIChildren
}

export type AlertTitleProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AlertDescriptionProps = {
  readonly className?: string
  readonly children?: UIChildren
}

const baseClasses =
  "relative w-full rounded-lg border px-4 py-3 text-sm grid has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] grid-cols-[0_1fr] has-[>svg]:gap-x-3 gap-y-0.5 items-start [&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current"

const variantClasses: Record<AlertVariant, string> = {
  default: "bg-card text-card-foreground",
  destructive:
    "text-destructive bg-card [&>svg]:text-current *:data-[slot=alert-description]:text-destructive/90",
}

export const Alert = ({
  className,
  variant = "default",
  children,
}: AlertProps): TemplateResult => {
  return html`
    <div
      data-slot="alert"
      role="alert"
      class="${cx(baseClasses, variantClasses[variant], className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const AlertTitle = ({ className, children }: AlertTitleProps): TemplateResult => {
  return html`
    <div
      data-slot="alert-title"
      class="${cx("col-start-2 line-clamp-1 min-h-4 font-medium tracking-tight", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const AlertDescription = ({
  className,
  children,
}: AlertDescriptionProps): TemplateResult => {
  return html`
    <div
      data-slot="alert-description"
      class="${cx(
        "text-muted-foreground col-start-2 grid justify-items-start gap-1 text-sm [&_p]:leading-relaxed",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}
