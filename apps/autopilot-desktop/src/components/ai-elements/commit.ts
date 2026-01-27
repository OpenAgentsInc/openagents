import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Avatar, AvatarFallback } from "../ui/avatar.js"
import { Button } from "../ui/button.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type CommitProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Commit = ({ className, children }: CommitProps): TemplateResult =>
  Collapsible({ className: cx("rounded-lg border bg-background", className), children })

export type CommitHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitHeader = ({ className, children }: CommitHeaderProps): TemplateResult =>
  CollapsibleTrigger({
    children: html`
      <div
        class="${cx(
          "group flex cursor-pointer items-center justify-between gap-4 p-3 text-left transition-colors hover:opacity-80",
          className
        )}"
      >
        ${children ?? ""}
      </div>
    `,
  })

export type CommitHashProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitHash = ({ className, children }: CommitHashProps): TemplateResult => html`
  <span class="${cx("font-mono text-xs", className)}">
    <span class="mr-1 inline-block size-3">git</span>
    ${children ?? ""}
  </span>
`

export type CommitMessageProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitMessage = ({ className, children }: CommitMessageProps): TemplateResult => html`
  <span class="${cx("font-medium text-sm", className)}">${children ?? ""}</span>
`

export type CommitMetadataProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitMetadata = ({ className, children }: CommitMetadataProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2 text-muted-foreground text-xs", className)}">${children ?? ""}</div>
`

export type CommitSeparatorProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitSeparator = ({ className, children }: CommitSeparatorProps): TemplateResult => html`
  <span class="${className ?? ""}">${children ?? "*"}</span>
`

export type CommitInfoProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitInfo = ({ className, children }: CommitInfoProps): TemplateResult => html`
  <div class="${cx("flex flex-1 flex-col", className)}">${children ?? ""}</div>
`

export type CommitAuthorProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitAuthor = ({ className, children }: CommitAuthorProps): TemplateResult => html`
  <div class="${cx("flex items-center", className)}">${children ?? ""}</div>
`

export type CommitAuthorAvatarProps = {
  readonly initials: string
  readonly className?: string
}

export const CommitAuthorAvatar = ({ initials, className }: CommitAuthorAvatarProps): TemplateResult =>
  Avatar({
    className: cx("size-8", className),
    children: AvatarFallback({ className: "text-xs", children: initials }),
  })

export type CommitTimestampProps = {
  readonly className?: string
  readonly date: Date
  readonly children?: AIChildren
}

export const CommitTimestamp = ({ className, date, children }: CommitTimestampProps): TemplateResult => {
  const formatted = new Intl.RelativeTimeFormat("en", { numeric: "auto" }).format(
    Math.round((date.getTime() - Date.now()) / (1000 * 60 * 60 * 24)),
    "day"
  )
  return html`<time class="${cx("text-xs", className)}" datetime="${date.toISOString()}">${children ?? formatted}</time>`
}

export type CommitActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitActions = ({ className, children }: CommitActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-1", className)}" role="group">${children ?? ""}</div>
`

export type CommitCopyButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitCopyButton = ({ className, children }: CommitCopyButtonProps): TemplateResult =>
  Button({
    className: cx("size-7 shrink-0", className),
    size: "icon",
    type: "button",
    variant: "ghost",
    children: children ?? "copy",
  })

export type CommitContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitContent = ({ className, children }: CommitContentProps): TemplateResult =>
  CollapsibleContent({ className: cx("border-t p-3", className), children })

export type CommitFilesProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFiles = ({ className, children }: CommitFilesProps): TemplateResult => html`
  <div class="${cx("space-y-1", className)}">${children ?? ""}</div>
`

export type CommitFileProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFile = ({ className, children }: CommitFileProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between gap-2 rounded px-2 py-1 text-sm hover:bg-muted/50", className)}">
    ${children ?? ""}
  </div>
`

export type CommitFileInfoProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFileInfo = ({ className, children }: CommitFileInfoProps): TemplateResult => html`
  <div class="${cx("flex min-w-0 items-center gap-2", className)}">${children ?? ""}</div>
`

export type CommitFileStatusProps = {
  readonly className?: string
  readonly status: "added" | "modified" | "deleted" | "renamed"
  readonly children?: AIChildren
}

const fileStatusStyles: Record<CommitFileStatusProps["status"], string> = {
  added: "text-green-600 dark:text-green-400",
  modified: "text-yellow-600 dark:text-yellow-400",
  deleted: "text-red-600 dark:text-red-400",
  renamed: "text-blue-600 dark:text-blue-400",
}

const fileStatusLabels: Record<CommitFileStatusProps["status"], string> = {
  added: "A",
  modified: "M",
  deleted: "D",
  renamed: "R",
}

export const CommitFileStatus = ({ status, className, children }: CommitFileStatusProps): TemplateResult => html`
  <span class="${cx("font-medium font-mono text-xs", fileStatusStyles[status], className)}">${children ?? fileStatusLabels[status]}</span>
`

export type CommitFileIconProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFileIcon = ({ className, children }: CommitFileIconProps): TemplateResult => html`
  <span class="${cx("size-3.5 shrink-0 text-muted-foreground", className)}">${children ?? "file"}</span>
`

export type CommitFilePathProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFilePath = ({ className, children }: CommitFilePathProps): TemplateResult => html`
  <span class="${cx("truncate font-mono text-xs", className)}">${children ?? ""}</span>
`

export type CommitFileChangesProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CommitFileChanges = ({ className, children }: CommitFileChangesProps): TemplateResult => html`
  <div class="${cx("flex shrink-0 items-center gap-1 font-mono text-xs", className)}">${children ?? ""}</div>
`

export type CommitFileAdditionsProps = {
  readonly className?: string
  readonly count: number
  readonly children?: AIChildren
}

export const CommitFileAdditions = ({ count, className, children }: CommitFileAdditionsProps): TemplateResult => {
  if (count <= 0) {
    return html``
  }
  return html`
    <span class="${cx("text-green-600 dark:text-green-400", className)}">${children ?? html`<span class="inline-block">+</span>${count}`}</span>
  `
}

export type CommitFileDeletionsProps = {
  readonly className?: string
  readonly count: number
  readonly children?: AIChildren
}

export const CommitFileDeletions = ({ count, className, children }: CommitFileDeletionsProps): TemplateResult => {
  if (count <= 0) {
    return html``
  }
  return html`
    <span class="${cx("text-red-600 dark:text-red-400", className)}">${children ?? html`<span class="inline-block">-</span>${count}`}</span>
  `
}
