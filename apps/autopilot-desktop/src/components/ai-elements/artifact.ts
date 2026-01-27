import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip.js"
import { cx, type AIChildren } from "./utils.js"

export type ArtifactProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Artifact = ({ className, children }: ArtifactProps): TemplateResult => html`
  <div class="${cx("flex flex-col overflow-hidden rounded-lg border bg-background shadow-sm", className)}">
    ${children ?? ""}
  </div>
`

export type ArtifactHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ArtifactHeader = ({ className, children }: ArtifactHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between border-b bg-muted/50 px-4 py-3", className)}">
    ${children ?? ""}
  </div>
`

export type ArtifactCloseProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly size?: "sm" | "icon" | "icon-sm" | "icon-xs" | "default" | "lg"
  readonly variant?: "ghost" | "outline" | "default" | "secondary" | "destructive" | "link"
}

export const ArtifactClose = ({
  className,
  children,
  size = "sm",
  variant = "ghost",
}: ArtifactCloseProps): TemplateResult => {
  return Button({
    className: cx("size-8 p-0 text-muted-foreground hover:text-foreground", className),
    size: size as any,
    type: "button",
    variant: variant as any,
    children: html`${children ?? "x"}<span class="sr-only">Close</span>`,
  })
}

export type ArtifactTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ArtifactTitle = ({ className, children }: ArtifactTitleProps): TemplateResult => html`
  <p class="${cx("font-medium text-foreground text-sm", className)}">${children ?? ""}</p>
`

export type ArtifactDescriptionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ArtifactDescription = ({ className, children }: ArtifactDescriptionProps): TemplateResult => html`
  <p class="${cx("text-muted-foreground text-sm", className)}">${children ?? ""}</p>
`

export type ArtifactActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ArtifactActions = ({ className, children }: ArtifactActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-1", className)}">${children ?? ""}</div>
`

export type ArtifactActionProps = {
  readonly className?: string
  readonly tooltip?: string
  readonly label?: string
  readonly children?: AIChildren
  readonly size?: "sm" | "icon" | "icon-sm" | "icon-xs" | "default" | "lg"
  readonly variant?: "ghost" | "outline" | "default" | "secondary" | "destructive" | "link"
}

export const ArtifactAction = ({
  tooltip,
  label,
  children,
  className,
  size = "sm",
  variant = "ghost",
}: ArtifactActionProps): TemplateResult => {
  const button = Button({
    className: cx("size-8 p-0 text-muted-foreground hover:text-foreground", className),
    size: size as any,
    type: "button",
    variant: variant as any,
    children: html`${children ?? "*"}<span class="sr-only">${label || tooltip || ""}</span>`,
  })

  if (!tooltip) {
    return button
  }

  return TooltipProvider({
    children: Tooltip({
      children: html`
        ${TooltipTrigger({ children: button })}
        ${TooltipContent({ children: html`<p>${tooltip}</p>` })}
      `,
    }),
  })
}

export type ArtifactContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ArtifactContent = ({ className, children }: ArtifactContentProps): TemplateResult => html`
  <div class="${cx("flex-1 overflow-auto p-4", className)}">${children ?? ""}</div>
`
