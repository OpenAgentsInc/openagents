import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type TaskItemFileProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TaskItemFile = ({ className, children }: TaskItemFileProps): TemplateResult =>
  html`<div class="${cx("inline-flex items-center gap-1 rounded-md border bg-secondary px-1.5 py-0.5 text-foreground text-xs", className)}">${children ?? ""}</div>`

export type TaskItemProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TaskItem = ({ className, children }: TaskItemProps): TemplateResult =>
  html`<div class="${cx("text-muted-foreground text-sm", className)}">${children ?? ""}</div>`

export type TaskProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Task = ({ className, children }: TaskProps): TemplateResult =>
  Collapsible({ className: cx(className ?? ""), children })

export type TaskTriggerProps = {
  readonly className?: string
  readonly title: string
  readonly children?: AIChildren
}

export const TaskTrigger = ({ className, title, children }: TaskTriggerProps): TemplateResult =>
  CollapsibleTrigger({
    className: cx("group", className),
    children: children ?? html`
      <div class="flex w-full cursor-pointer items-center gap-2 text-muted-foreground text-sm transition-colors hover:text-foreground">
        <span class="size-4">search</span>
        <p class="text-sm">${title}</p>
        <span class="size-4 transition-transform group-data-[state=open]:rotate-180">v</span>
      </div>
    `,
  })

export type TaskContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TaskContent = ({ className, children }: TaskContentProps): TemplateResult =>
  CollapsibleContent({
    className: cx(
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-popover-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className
    ),
    children: html`<div class="mt-4 space-y-2 border-muted border-l-2 pl-4">${children ?? ""}</div>`,
  })
