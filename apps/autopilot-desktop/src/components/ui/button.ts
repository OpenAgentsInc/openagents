import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type ButtonVariant =
  | "default"
  | "destructive"
  | "outline"
  | "secondary"
  | "ghost"
  | "link"

export type ButtonSize =
  | "default"
  | "xs"
  | "sm"
  | "lg"
  | "icon"
  | "icon-xs"
  | "icon-sm"
  | "icon-lg"

export type ButtonProps = {
  readonly className?: string
  readonly variant?: ButtonVariant
  readonly size?: ButtonSize
  readonly disabled?: boolean
  readonly type?: "button" | "submit" | "reset"
  readonly dataSlot?: string
  readonly dataRole?: string
  readonly dataUi?: string
  readonly dataUiStop?: boolean
  readonly dataCopyTarget?: string
  readonly dataCopyValue?: string
  readonly ariaLabel?: string
  readonly title?: string
  readonly children?: UIChildren
}

const baseClasses =
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive"

const variantClasses: Record<ButtonVariant, string> = {
  default: "bg-primary text-primary-foreground hover:bg-primary/90",
  destructive:
    "bg-destructive text-white hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
  outline:
    "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
  secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
  ghost: "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
  link: "text-primary underline-offset-4 hover:underline",
}

const sizeClasses: Record<ButtonSize, string> = {
  default: "h-9 px-4 py-2 has-[>svg]:px-3",
  xs: "h-6 gap-1 rounded-md px-2 text-xs has-[>svg]:px-1.5 [&_svg:not([class*='size-'])]:size-3",
  sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
  lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
  icon: "size-9",
  "icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
  "icon-sm": "size-8",
  "icon-lg": "size-10",
}

export const Button = ({
  className,
  variant = "default",
  size = "default",
  disabled = false,
  type = "button",
  dataSlot,
  dataRole,
  dataUi,
  dataUiStop = false,
  dataCopyTarget,
  dataCopyValue,
  ariaLabel,
  title,
  children,
}: ButtonProps): TemplateResult => {
  return html`
    <button
      data-slot="${dataSlot ?? "button"}"
      data-variant="${variant}"
      data-size="${size}"
      data-role="${dataRole ?? ""}"
      data-ui="${dataUi ?? ""}"
      data-ui-stop="${dataUiStop ? "true" : ""}"
      data-copy-target="${dataCopyTarget ?? ""}"
      data-copy-value="${dataCopyValue ?? ""}"
      aria-label="${ariaLabel ?? ""}"
      title="${title ?? ""}"
      type="${type}"
      class="${cx(baseClasses, variantClasses[variant], sizeClasses[size], className)}"
      ${disabled ? "disabled" : ""}
    >
      ${children ?? ""}
    </button>
  `
}
