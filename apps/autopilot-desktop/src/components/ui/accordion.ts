import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type AccordionProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AccordionItemProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AccordionTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AccordionContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly state?: "open" | "closed"
}

export const Accordion = ({ className, children }: AccordionProps): TemplateResult => {
  return html`
    <div data-slot="accordion" class="${cx(className)}">
      ${children ?? ""}
    </div>
  `
}

export const AccordionItem = ({ className, children }: AccordionItemProps): TemplateResult => {
  return html`
    <div data-slot="accordion-item" class="${cx("border-b last:border-b-0", className)}">
      ${children ?? ""}
    </div>
  `
}

export const AccordionTrigger = ({ className, children }: AccordionTriggerProps): TemplateResult => {
  return html`
    <div class="flex">
      <button
        data-slot="accordion-trigger"
        class="${cx(
          "focus-visible:border-ring focus-visible:ring-ring/50 flex flex-1 items-start justify-between gap-4 rounded-md py-4 text-left text-sm font-medium transition-all outline-none hover:underline focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50 [&[data-state=open]>svg]:rotate-180",
          className
        )}"
        type="button"
      >
        ${children ?? ""}
        <span class="text-muted-foreground pointer-events-none size-4 shrink-0 translate-y-0.5 transition-transform duration-200">▾</span>
      </button>
    </div>
  `
}

export const AccordionContent = ({
  className,
  children,
  state = "closed",
}: AccordionContentProps): TemplateResult => {
  return html`
    <div
      data-slot="accordion-content"
      data-state="${state}"
      class="data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down overflow-hidden text-sm"
    >
      <div class="${cx("pt-0 pb-4", className)}">${children ?? ""}</div>
    </div>
  `
}
