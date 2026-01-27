import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type CardProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type CardSectionProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const Card = ({ className, children }: CardProps): TemplateResult => {
  return html`
    <div
      data-slot="card"
      class="${cx(
        "bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export const CardHeader = ({ className, children }: CardSectionProps): TemplateResult => {
  return html`
    <div
      data-slot="card-header"
      class="${cx(
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export const CardTitle = ({ className, children }: CardSectionProps): TemplateResult => {
  return html`
    <div data-slot="card-title" class="${cx("leading-none font-semibold", className)}">
      ${children ?? ""}
    </div>
  `
}

export const CardDescription = ({
  className,
  children,
}: CardSectionProps): TemplateResult => {
  return html`
    <div data-slot="card-description" class="${cx("text-muted-foreground text-sm", className)}">
      ${children ?? ""}
    </div>
  `
}

export const CardAction = ({ className, children }: CardSectionProps): TemplateResult => {
  return html`
    <div
      data-slot="card-action"
      class="${cx("col-start-2 row-span-2 row-start-1 self-start justify-self-end", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const CardContent = ({ className, children }: CardSectionProps): TemplateResult => {
  return html`
    <div data-slot="card-content" class="${cx("px-6", className)}">${children ?? ""}</div>
  `
}

export const CardFooter = ({ className, children }: CardSectionProps): TemplateResult => {
  return html`
    <div
      data-slot="card-footer"
      class="${cx("flex items-center px-6 [.border-t]:pt-6", className)}"
    >
      ${children ?? ""}
    </div>
  `
}
