import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button, type ButtonVariant, type ButtonSize } from "./button.js"
import { cx, type UIChildren } from "./utils.js"

export type CarouselOrientation = "horizontal" | "vertical"

export type CarouselProps = {
  readonly className?: string
  readonly orientation?: CarouselOrientation
  readonly children?: UIChildren
}

export type CarouselContentProps = {
  readonly className?: string
  readonly orientation?: CarouselOrientation
  readonly children?: UIChildren
}

export type CarouselItemProps = {
  readonly className?: string
  readonly orientation?: CarouselOrientation
  readonly children?: UIChildren
}

export type CarouselNavProps = {
  readonly className?: string
  readonly orientation?: CarouselOrientation
  readonly variant?: ButtonVariant
  readonly size?: ButtonSize
  readonly disabled?: boolean
  readonly label?: string
}

export const Carousel = ({
  className,
  orientation = "horizontal",
  children,
}: CarouselProps): TemplateResult => {
  return html`
    <div
      role="region"
      aria-roledescription="carousel"
      data-slot="carousel"
      data-orientation="${orientation}"
      class="${cx("relative", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const CarouselContent = ({
  className,
  orientation = "horizontal",
  children,
}: CarouselContentProps): TemplateResult => {
  const layoutClass = orientation === "horizontal" ? "-ml-4" : "-mt-4 flex-col"
  return html`
    <div data-slot="carousel-content" class="overflow-hidden">
      <div class="${cx("flex", layoutClass, className)}">${children ?? ""}</div>
    </div>
  `
}

export const CarouselItem = ({
  className,
  orientation = "horizontal",
  children,
}: CarouselItemProps): TemplateResult => {
  const spacing = orientation === "horizontal" ? "pl-4" : "pt-4"
  return html`
    <div
      role="group"
      aria-roledescription="slide"
      data-slot="carousel-item"
      class="${cx("min-w-0 shrink-0 grow-0 basis-full", spacing, className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const CarouselPrevious = ({
  className,
  orientation = "horizontal",
  variant = "outline",
  size = "icon",
  disabled = false,
  label = "Previous slide",
}: CarouselNavProps): TemplateResult => {
  const positionClass =
    orientation === "horizontal"
      ? "top-1/2 -left-12 -translate-y-1/2"
      : "-top-12 left-1/2 -translate-x-1/2 rotate-90"
  return Button({
    variant,
    size,
    disabled,
    className: cx("absolute size-8 rounded-full", positionClass, className),
    children: html`<span aria-hidden="true">←</span><span class="sr-only">${label}</span>`,
  })
}

export const CarouselNext = ({
  className,
  orientation = "horizontal",
  variant = "outline",
  size = "icon",
  disabled = false,
  label = "Next slide",
}: CarouselNavProps): TemplateResult => {
  const positionClass =
    orientation === "horizontal"
      ? "top-1/2 -right-12 -translate-y-1/2"
      : "-bottom-12 left-1/2 -translate-x-1/2 rotate-90"
  return Button({
    variant,
    size,
    disabled,
    className: cx("absolute size-8 rounded-full", positionClass, className),
    children: html`<span aria-hidden="true">→</span><span class="sr-only">${label}</span>`,
  })
}
