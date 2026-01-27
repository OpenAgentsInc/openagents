import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type CollapsibleProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly state?: "open" | "closed"
}

export const Collapsible = ({
  className,
  children,
  state = "closed",
}: CollapsibleProps): TemplateResult => {
  return html`
    <div data-slot="collapsible" data-state="${state}" class="${cx(className)}">
      ${children ?? ""}
    </div>
  `
}

export type CollapsibleTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CollapsibleTrigger = ({
  className,
  children,
}: CollapsibleTriggerProps): TemplateResult => {
  return html`
    <button data-slot="collapsible-trigger" class="${cx(className)}" type="button">
      ${children ?? ""}
    </button>
  `
}

export type CollapsibleContentProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CollapsibleContent = ({
  className,
  children,
}: CollapsibleContentProps): TemplateResult => {
  return html`
    <div data-slot="collapsible-content" class="${cx(className)}">
      ${children ?? ""}
    </div>
  `
}
