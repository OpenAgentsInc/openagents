import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { HoverCard, HoverCardContent, HoverCardTrigger } from "../ui/hover-card.js"
import { cx, type AIChildren } from "./utils.js"

export type AttachmentData = {
  readonly id?: string
  readonly type: string
  readonly filename?: string | null
  readonly title?: string | null
  readonly url?: string | null
  readonly base64?: string | null
  readonly mediaType?: string | null
  readonly size?: number | null
}

export type AttachmentMediaCategory = "image" | "video" | "audio" | "document" | "source" | "unknown"

export type AttachmentVariant = "grid" | "inline" | "list"

export const getMediaCategory = (data: AttachmentData): AttachmentMediaCategory => {
  if (data.type === "source-document") {
    return "source"
  }
  const mediaType = data.mediaType ?? ""
  if (mediaType.startsWith("image/")) return "image"
  if (mediaType.startsWith("video/")) return "video"
  if (mediaType.startsWith("audio/")) return "audio"
  if (mediaType.startsWith("application/") || mediaType.startsWith("text/")) return "document"
  return "unknown"
}

export const getAttachmentLabel = (data: AttachmentData): string => {
  if (data.type === "source-document") {
    return data.title || data.filename || "Source"
  }
  const category = getMediaCategory(data)
  return data.filename || (category === "image" ? "Image" : "Attachment")
}

export type AttachmentsProps = {
  readonly variant?: AttachmentVariant
  readonly className?: string
  readonly children?: AIChildren
}

export const Attachments = ({
  variant = "grid",
  className,
  children,
}: AttachmentsProps): TemplateResult => {
  return html`
    <div
      class="${cx(
        "flex items-start",
        variant === "list" ? "flex-col gap-2" : "flex-wrap gap-2",
        variant === "grid" ? "ml-auto w-fit" : "",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type AttachmentProps = {
  readonly data: AttachmentData
  readonly variant?: AttachmentVariant
  readonly onRemove?: () => void
  readonly className?: string
  readonly children?: AIChildren
}

export const Attachment = ({
  data,
  variant = "grid",
  className,
  children,
}: AttachmentProps): TemplateResult => {
  const mediaCategory = getMediaCategory(data)
  return html`
    <div
      data-media="${mediaCategory}"
      class="${cx(
        "group relative",
        variant === "grid" ? "size-24 overflow-hidden rounded-lg" : "",
        variant === "inline"
          ? "flex h-8 cursor-pointer select-none items-center gap-1.5 rounded-md border border-border px-1.5 font-medium text-sm transition-all hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50"
          : "",
        variant === "list" ? "flex w-full items-center gap-3 rounded-lg border p-3 hover:bg-accent/50" : "",
        className
      )}"
    >
      ${children ?? html`${AttachmentPreview({ data, variant })}${AttachmentInfo({ data, variant })}`}
    </div>
  `
}

export type AttachmentPreviewProps = {
  readonly data: AttachmentData
  readonly variant?: AttachmentVariant
  readonly className?: string
  readonly fallbackIcon?: AIChildren
}

const getPreviewIcon = (category: AttachmentMediaCategory) => {
  switch (category) {
    case "image":
      return "img"
    case "video":
      return "vid"
    case "audio":
      return "aud"
    case "document":
      return "file"
    case "source":
      return "web"
    default:
      return "file"
  }
}

export const AttachmentPreview = ({
  data,
  variant = "grid",
  className,
  fallbackIcon,
}: AttachmentPreviewProps): TemplateResult => {
  const category = getMediaCategory(data)
  const iconSize = variant === "inline" ? "size-3" : "size-4"
  const src = data.url ?? (data.base64 && data.mediaType ? `data:${data.mediaType};base64,${data.base64}` : null)

  if (category === "image" && src) {
    return html`
      <div class="${cx("flex items-center justify-center", className)}">
        <img
          alt="${data.filename ?? "Image"}"
          class="${variant === "grid" ? "size-full object-cover" : "size-full rounded object-cover"}"
          height="${variant === "grid" ? 96 : 20}"
          width="${variant === "grid" ? 96 : 20}"
          src="${src}"
        />
      </div>
    `
  }

  return html`
    <div class="${cx("flex items-center justify-center", className)}">
      <span class="${cx(iconSize, "text-muted-foreground")}">${fallbackIcon ?? getPreviewIcon(category)}</span>
    </div>
  `
}

export type AttachmentInfoProps = {
  readonly data: AttachmentData
  readonly variant?: AttachmentVariant
  readonly className?: string
  readonly children?: AIChildren
}

const formatFileSize = (size?: number | null) => {
  if (!size && size !== 0) return ""
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / (1024 * 1024)).toFixed(1)} MB`
}

export const AttachmentInfo = ({
  data,
  variant = "grid",
  className,
  children,
}: AttachmentInfoProps): TemplateResult => {
  if (variant === "grid") {
    return html``
  }

  return html`
    <div class="${cx("flex flex-1 items-center justify-between gap-2", className)}">
      ${children ?? html`
        <div class="flex flex-col">
          <span class="text-sm font-medium">${getAttachmentLabel(data)}</span>
          ${data.size ? html`<span class="text-xs text-muted-foreground">${formatFileSize(data.size)}</span>` : ""}
        </div>
      `}
    </div>
  `
}

export type AttachmentRemoveProps = {
  readonly className?: string
  readonly label?: string
}

export const AttachmentRemove = ({ className, label = "Remove" }: AttachmentRemoveProps): TemplateResult => {
  return Button({
    className: cx("size-6 p-0 text-muted-foreground hover:text-foreground", className),
    size: "icon-xs",
    type: "button",
    variant: "ghost",
    children: html`x<span class="sr-only">${label}</span>`,
  })
}

export type AttachmentHoverCardProps = {
  readonly children?: AIChildren
}

export const AttachmentHoverCard = ({ children }: AttachmentHoverCardProps): TemplateResult =>
  HoverCard({ children })

export type AttachmentHoverCardTriggerProps = {
  readonly children?: AIChildren
}

export const AttachmentHoverCardTrigger = ({ children }: AttachmentHoverCardTriggerProps): TemplateResult =>
  HoverCardTrigger({ children })

export type AttachmentHoverCardContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AttachmentHoverCardContent = ({ className, children }: AttachmentHoverCardContentProps): TemplateResult =>
  HoverCardContent({ className, children })

export type AttachmentEmptyProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AttachmentEmpty = ({ className, children }: AttachmentEmptyProps): TemplateResult => html`
  <div class="${cx("flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed p-6 text-center", className)}">
    ${children ?? html`
      <p class="text-sm text-muted-foreground">No attachments yet.</p>
    `}
  </div>
`
