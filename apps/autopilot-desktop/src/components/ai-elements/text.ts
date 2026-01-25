import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type TextTone = "default" | "muted" | "accent" | "danger"
export type TextSize = "xs" | "sm" | "md"

const toneClasses: Record<TextTone, string> = {
  default: "text-foreground",
  muted: "text-muted-foreground",
  accent: "text-accent",
  danger: "text-destructive",
}

const sizeClasses: Record<TextSize, string> = {
  xs: "text-[10px]",
  sm: "text-[11px]",
  md: "text-[12px]",
}

export type TextProps = {
  readonly text: string
  readonly tone?: TextTone
  readonly size?: TextSize
}

export const Text = ({
  text,
  tone = "default",
  size = "md",
}: TextProps): TemplateResult => html`
  <span class="${toneClasses[tone]} ${sizeClasses[size]}">${text}</span>
`

export type HeadingProps = {
  readonly text: string
  readonly level?: 1 | 2 | 3 | 4
}

export const Heading = ({ text, level = 2 }: HeadingProps): TemplateResult => {
  const Tag = `h${level}` as const
  const classes =
    level === 1
      ? "text-lg font-semibold"
      : level === 2
        ? "text-base font-semibold"
        : level === 3
          ? "text-sm font-semibold"
          : "text-xs font-semibold uppercase"

  return html`<${Tag} class="${classes} text-foreground">${text}</${Tag}>`
}
