import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type AvatarSize = "default" | "sm" | "lg"

export type AvatarProps = {
  readonly className?: string
  readonly size?: AvatarSize
  readonly children?: UIChildren
}

export type AvatarImageProps = {
  readonly className?: string
  readonly src?: string
  readonly alt?: string
}

export type AvatarFallbackProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AvatarBadgeProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AvatarGroupProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type AvatarGroupCountProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const Avatar = ({
  className,
  size = "default",
  children,
}: AvatarProps): TemplateResult => {
  return html`
    <div
      data-slot="avatar"
      data-size="${size}"
      class="${cx(
        "group/avatar relative flex size-8 shrink-0 overflow-hidden rounded-full select-none data-[size=lg]:size-10 data-[size=sm]:size-6",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export const AvatarImage = ({
  className,
  src,
  alt,
}: AvatarImageProps): TemplateResult => {
  return html`
    <img
      data-slot="avatar-image"
      class="${cx("aspect-square size-full", className)}"
      src="${src ?? ""}"
      alt="${alt ?? ""}"
    />
  `
}

export const AvatarFallback = ({
  className,
  children,
}: AvatarFallbackProps): TemplateResult => {
  return html`
    <div
      data-slot="avatar-fallback"
      class="${cx(
        "bg-muted text-muted-foreground flex size-full items-center justify-center rounded-full text-sm group-data-[size=sm]/avatar:text-xs",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export const AvatarBadge = ({
  className,
  children,
}: AvatarBadgeProps): TemplateResult => {
  return html`
    <span
      data-slot="avatar-badge"
      class="${cx(
        "bg-primary text-primary-foreground ring-background absolute right-0 bottom-0 z-10 inline-flex items-center justify-center rounded-full ring-2 select-none",
        "group-data-[size=sm]/avatar:size-2 group-data-[size=sm]/avatar:[&>svg]:hidden",
        "group-data-[size=default]/avatar:size-2.5 group-data-[size=default]/avatar:[&>svg]:size-2",
        "group-data-[size=lg]/avatar:size-3 group-data-[size=lg]/avatar:[&>svg]:size-2",
        className
      )}"
    >
      ${children ?? ""}
    </span>
  `
}

export const AvatarGroup = ({ className, children }: AvatarGroupProps): TemplateResult => {
  return html`
    <div
      data-slot="avatar-group"
      class="${cx(
        "*:data-[slot=avatar]:ring-background group/avatar-group flex -space-x-2 *:data-[slot=avatar]:ring-2",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export const AvatarGroupCount = ({
  className,
  children,
}: AvatarGroupCountProps): TemplateResult => {
  return html`
    <div
      data-slot="avatar-group-count"
      class="${cx(
        "bg-muted text-muted-foreground ring-background relative flex size-8 shrink-0 items-center justify-center rounded-full text-sm ring-2 group-has-data-[size=lg]/avatar-group:size-10 group-has-data-[size=sm]/avatar-group:size-6 [&>svg]:size-4 group-has-data-[size=lg]/avatar-group:[&>svg]:size-5 group-has-data-[size=sm]/avatar-group:[&>svg]:size-3",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}
