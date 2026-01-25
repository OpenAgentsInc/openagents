/**
 * Basic Button Component
 */

import { html } from "../effuse/template/html"

export interface ButtonProps {
  label: string
  disabled?: boolean
  variant?: "primary" | "secondary" | "danger"
  size?: "sm" | "md" | "lg"
}

export const Button = (props: ButtonProps) => {
  const { label, disabled = false, variant = "primary", size = "md" } = props

  const baseClasses =
    "inline-flex items-center justify-center border border-border bg-background font-semibold uppercase transition duration-150 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent focus-visible:ring-offset-0 disabled:pointer-events-none disabled:opacity-50"

  const variantClasses = {
    primary:
      "border-accent text-accent hover:bg-surface-strong",
    secondary:
      "text-foreground hover:bg-surface-strong",
    danger:
      "border-destructive text-destructive hover:bg-surface-strong",
  }[variant]

  const sizeClasses = {
    sm: "h-7 px-2 text-[10px]",
    md: "h-8 px-3 text-[10px]",
    lg: "h-9 px-4 text-[11px]",
  }[size]

  return html`
    <button 
      class="${baseClasses} ${variantClasses} ${sizeClasses}"
      ${disabled ? "disabled" : ""}
      data-ez="button.click"
    >
      ${label}
    </button>
  `
}
