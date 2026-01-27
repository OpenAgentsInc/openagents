import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Separator } from "../ui/separator.js"
import { Tooltip, TooltipContent, TooltipTrigger } from "../ui/tooltip.js"
import { cx, type AIChildren } from "./utils.js"

export type CheckpointProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Checkpoint = ({ className, children }: CheckpointProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-0.5 overflow-hidden text-muted-foreground", className)}">
    ${children ?? ""}
    ${Separator({})}
  </div>
`

export type CheckpointIconProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CheckpointIcon = ({ className, children }: CheckpointIconProps): TemplateResult =>
  html`${children ?? html`<span class="${cx("size-4 shrink-0", className)}">mark</span>`}`

export type CheckpointTriggerProps = {
  readonly className?: string
  readonly tooltip?: string
  readonly children?: AIChildren
  readonly variant?: "ghost" | "outline" | "default" | "secondary" | "destructive" | "link"
  readonly size?: "sm" | "icon" | "icon-sm" | "icon-xs" | "default" | "lg"
}

export const CheckpointTrigger = ({
  children,
  className,
  variant = "ghost",
  size = "sm",
  tooltip,
}: CheckpointTriggerProps): TemplateResult => {
  const button = Button({
    className,
    size: size as any,
    type: "button",
    variant: variant as any,
    children,
  })

  if (!tooltip) {
    return button
  }

  return Tooltip({
    children: html`
      ${TooltipTrigger({ children: button })}
      ${TooltipContent({ side: "bottom", children: tooltip })}
    `,
  })
}
