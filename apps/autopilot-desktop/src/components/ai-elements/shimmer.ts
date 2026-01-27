import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export interface TextShimmerProps {
  readonly children: string
  readonly className?: string
  readonly duration?: number
  readonly spread?: number
}

export const Shimmer = ({ children, className, duration = 2, spread = 2 }: TextShimmerProps): TemplateResult => {
  const dynamicSpread = (children?.length ?? 0) * spread
  return html`
    <span
      class="${cx(
        "relative inline-block bg-[length:250%_100%,auto] bg-clip-text text-transparent",
        "[--bg:linear-gradient(90deg,#0000_calc(50%-var(--spread)),var(--color-background),#0000_calc(50%+var(--spread)))] [background-repeat:no-repeat,padding-box]",
        className
      )}"
      style="--spread: ${dynamicSpread}px; background-image: var(--bg), linear-gradient(var(--color-muted-foreground), var(--color-muted-foreground)); animation: shimmer ${duration}s linear infinite;"
    >
      ${children}
    </span>
  `
}
