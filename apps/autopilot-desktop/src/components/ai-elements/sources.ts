import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type SourcesProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Sources = ({ className, children }: SourcesProps): TemplateResult =>
  Collapsible({ className: cx("not-prose mb-4 text-primary text-xs", className), children })

export type SourcesTriggerProps = {
  readonly className?: string
  readonly count: number
  readonly children?: AIChildren
}

export const SourcesTrigger = ({ className, count, children }: SourcesTriggerProps): TemplateResult =>
  CollapsibleTrigger({
    className: cx("flex items-center gap-2", className),
    children: children ?? html`<p class="font-medium">Used ${count} sources</p><span class="h-4 w-4">v</span>`,
  })

export type SourcesContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SourcesContent = ({ className, children }: SourcesContentProps): TemplateResult =>
  CollapsibleContent({
    className: cx(
      "mt-3 flex w-fit flex-col gap-2",
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className
    ),
    children,
  })

export type SourceProps = {
  readonly href?: string
  readonly title?: string
  readonly children?: AIChildren
}

export const Source = ({ href, title, children }: SourceProps): TemplateResult =>
  html`<a class="flex items-center gap-2" href="${href ?? "#"}" rel="noreferrer" target="_blank">${
    children ?? html`<span class="h-4 w-4">src</span><span class="block font-medium">${title ?? "Source"}</span>`
  }</a>`
