import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "../ui/card.js"
import { cx, type AIChildren } from "./utils.js"

export type NodeProps = {
  readonly handles: {
    readonly target?: boolean
    readonly source?: boolean
    readonly top?: boolean
    readonly bottom?: boolean
  }
  readonly className?: string
  readonly children?: AIChildren
}

export const Node = ({ handles, className, children }: NodeProps): TemplateResult => {
  const handleMarkup = html`
    ${handles?.target ? html`<span class="node-handle node-handle--target" aria-hidden="true"></span>` : ""}
    ${handles?.source ? html`<span class="node-handle node-handle--source" aria-hidden="true"></span>` : ""}
    ${handles?.top ? html`<span class="node-handle node-handle--top" aria-hidden="true"></span>` : ""}
    ${handles?.bottom ? html`<span class="node-handle node-handle--bottom" aria-hidden="true"></span>` : ""}
  `

  return Card({
    className: cx("node-container relative size-full h-auto w-sm gap-0 rounded-md p-0", className),
    children: html`${handleMarkup}${children ?? ""}`,
  })
}

export type NodeHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const NodeHeader = ({ className, children }: NodeHeaderProps): TemplateResult =>
  CardHeader({ className: cx("gap-0.5 rounded-t-md border-b bg-secondary p-3!", className), children })

export type NodeTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly text?: string
}

export const NodeTitle = ({ className, children, text }: NodeTitleProps): TemplateResult =>
  CardTitle({ className, children: children ?? text ?? "" })

export type NodeDescriptionProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly text?: string
}

export const NodeDescription = ({ className, children, text }: NodeDescriptionProps): TemplateResult =>
  CardDescription({ className, children: children ?? text ?? "" })

export type NodeActionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const NodeAction = ({ className, children }: NodeActionProps): TemplateResult =>
  CardAction({ className, children })

export type NodeContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const NodeContent = ({ className, children }: NodeContentProps): TemplateResult =>
  CardContent({ className: cx("p-3", className), children })

export type NodeFooterProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const NodeFooter = ({ className, children }: NodeFooterProps): TemplateResult =>
  CardFooter({ className: cx("rounded-b-md border-t bg-secondary p-3!", className), children })
