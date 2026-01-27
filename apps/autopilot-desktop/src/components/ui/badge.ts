import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type BadgeVariant =
  | "default"
  | "secondary"
  | "destructive"
  | "outline"
  | "ghost"
  | "link"

export type BadgeProps = {
  readonly className?: string
  readonly variant?: BadgeVariant
  readonly children?: UIChildren
  readonly href?: string
  readonly tag?: "span" | "a" | "button"
}

const baseClasses =
  "inline-flex items-center justify-center rounded-full border border-transparent px-2 py-0.5 text-xs font-medium w-fit whitespace-nowrap shrink-0 [&>svg]:size-3 gap-1 [&>svg]:pointer-events-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive transition-[color,box-shadow] overflow-hidden"

const variantClasses: Record<BadgeVariant, string> = {
  default: "bg-primary text-primary-foreground [a&]:hover:bg-primary/90",
  secondary: "bg-secondary text-secondary-foreground [a&]:hover:bg-secondary/90",
  destructive:
    "bg-destructive text-white [a&]:hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
  outline:
    "border-border text-foreground [a&]:hover:bg-accent [a&]:hover:text-accent-foreground",
  ghost: "[a&]:hover:bg-accent [a&]:hover:text-accent-foreground",
  link: "text-primary underline-offset-4 [a&]:hover:underline",
}

export const Badge = ({
  className,
  variant = "default",
  children,
  href,
  tag = href ? "a" : "span",
}: BadgeProps): TemplateResult => {
  const Tag = tag
  const attrs = Tag === "a" && href ? `href="${href}"` : ""
  return html`
    <${Tag}
      data-slot="badge"
      data-variant="${variant}"
      class="${cx(baseClasses, variantClasses[variant], className)}"
      ${attrs}
    >
      ${children ?? ""}
    </${Tag}>
  `
}
