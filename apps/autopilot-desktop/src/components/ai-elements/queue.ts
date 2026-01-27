import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { ScrollArea } from "../ui/scroll-area.js"
import { cx, type AIChildren } from "./utils.js"

export interface QueueMessagePart {
  type: string
  text?: string
  url?: string
  filename?: string
  mediaType?: string
}

export interface QueueMessage {
  id: string
  parts: QueueMessagePart[]
}

export interface QueueTodo {
  id: string
  title: string
  description?: string
  status?: "pending" | "completed"
}

export type QueueItemProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItem = ({ className, children }: QueueItemProps): TemplateResult => html`
  <li class="${cx("group flex flex-col gap-1 rounded-md px-3 py-1 text-sm transition-colors hover:bg-muted", className)}">
    ${children ?? ""}
  </li>
`

export type QueueItemIndicatorProps = {
  readonly completed?: boolean
  readonly className?: string
}

export const QueueItemIndicator = ({ completed = false, className }: QueueItemIndicatorProps): TemplateResult => html`
  <span
    class="${cx(
      "mt-0.5 inline-block size-2.5 rounded-full border",
      completed ? "border-muted-foreground/20 bg-muted-foreground/10" : "border-muted-foreground/50",
      className
    )}"
  ></span>
`

export type QueueItemContentProps = {
  readonly completed?: boolean
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemContent = ({ completed = false, className, children }: QueueItemContentProps): TemplateResult => html`
  <span
    class="${cx(
      "line-clamp-1 grow break-words",
      completed ? "text-muted-foreground/50 line-through" : "text-muted-foreground",
      className
    )}"
  >
    ${children ?? ""}
  </span>
`

export type QueueItemDescriptionProps = {
  readonly completed?: boolean
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemDescription = ({ completed = false, className, children }: QueueItemDescriptionProps): TemplateResult => html`
  <div
    class="${cx(
      "ml-6 text-xs",
      completed ? "text-muted-foreground/40 line-through" : "text-muted-foreground",
      className
    )}"
  >
    ${children ?? ""}
  </div>
`

export type QueueItemActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemActions = ({ className, children }: QueueItemActionsProps): TemplateResult => html`
  <div class="${cx("flex gap-1", className)}">${children ?? ""}</div>
`

export type QueueItemActionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemAction = ({ className, children }: QueueItemActionProps): TemplateResult =>
  Button({
    className: cx(
      "size-auto rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:bg-muted-foreground/10 hover:text-foreground group-hover:opacity-100",
      className
    ),
    size: "icon",
    type: "button",
    variant: "ghost",
    children: children ?? "...",
  })

export type QueueItemAttachmentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemAttachment = ({ className, children }: QueueItemAttachmentProps): TemplateResult => html`
  <div class="${cx("mt-1 flex flex-wrap gap-2", className)}">${children ?? ""}</div>
`

export type QueueItemImageProps = {
  readonly className?: string
  readonly src?: string
}

export const QueueItemImage = ({ className, src }: QueueItemImageProps): TemplateResult => html`
  <img alt="" class="${cx("h-8 w-8 rounded border object-cover", className)}" height="32" width="32" src="${src ?? ""}" />
`

export type QueueItemFileProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueItemFile = ({ className, children }: QueueItemFileProps): TemplateResult => html`
  <span class="${cx("flex items-center gap-1 rounded border bg-muted px-2 py-1 text-xs", className)}">
    <span class="size-3">file</span>
    <span class="max-w-[100px] truncate">${children ?? ""}</span>
  </span>
`

export type QueueListProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueList = ({ className, children }: QueueListProps): TemplateResult =>
  ScrollArea({
    className: cx("mt-2 -mb-1", className),
    children: html`<div class="max-h-40 pr-4"><ul>${children ?? ""}</ul></div>`,
  })

export type QueueSectionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueSection = ({ className, children }: QueueSectionProps): TemplateResult =>
  Collapsible({ className: cx("rounded-md border", className), children })

export type QueueSectionHeaderProps = {
  readonly className?: string
  readonly title?: string
  readonly children?: AIChildren
}

export const QueueSectionHeader = ({ className, title, children }: QueueSectionHeaderProps): TemplateResult =>
  CollapsibleTrigger({
    children: html`
      <div class="${cx("flex w-full items-center justify-between gap-2 px-3 py-2 text-xs uppercase tracking-wide text-muted-foreground", className)}">
        <div class="flex items-center gap-2">
          <span>${children ?? title ?? "Queue"}</span>
        </div>
        <span class="size-4">v</span>
      </div>
    `,
  })

export type QueueSectionContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const QueueSectionContent = ({ className, children }: QueueSectionContentProps): TemplateResult =>
  CollapsibleContent({ className: cx("px-2 pb-2", className), children })
