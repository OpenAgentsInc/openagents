import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { Carousel, CarouselContent, CarouselItem, CarouselPrevious, CarouselNext } from "../ui/carousel.js"
import { HoverCard, HoverCardContent, HoverCardTrigger } from "../ui/hover-card.js"
import { cx, type AIChildren } from "./utils.js"

export type InlineCitationProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitation = ({ className, children }: InlineCitationProps): TemplateResult => html`
  <span class="${cx("group inline items-center gap-1", className)}">${children ?? ""}</span>
`

export type InlineCitationTextProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationText = ({ className, children }: InlineCitationTextProps): TemplateResult => html`
  <span class="${cx("transition-colors group-hover:bg-accent", className)}">${children ?? ""}</span>
`

export type InlineCitationCardProps = {
  readonly children?: AIChildren
}

export const InlineCitationCard = ({ children }: InlineCitationCardProps): TemplateResult =>
  HoverCard({ closeDelay: 0, openDelay: 0, children })

export type InlineCitationCardTriggerProps = {
  readonly sources: string[]
  readonly className?: string
}

export const InlineCitationCardTrigger = ({ sources, className }: InlineCitationCardTriggerProps): TemplateResult => {
  const hostname = sources[0] ? new URL(sources[0]).hostname : "unknown"
  const extra = sources.length > 1 ? `+${sources.length - 1}` : ""
  return HoverCardTrigger({
    children: Badge({ className: cx("ml-1 rounded-full", className), variant: "secondary", children: `${hostname} ${extra}`.trim() }),
  })
}

export type InlineCitationCardBodyProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationCardBody = ({ className, children }: InlineCitationCardBodyProps): TemplateResult =>
  HoverCardContent({ className: cx("relative w-80 p-0", className), children })

export type InlineCitationCarouselProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationCarousel = ({ className, children }: InlineCitationCarouselProps): TemplateResult =>
  Carousel({ className: cx("w-full", className), children })

export type InlineCitationCarouselContentProps = {
  readonly children?: AIChildren
}

export const InlineCitationCarouselContent = ({ children }: InlineCitationCarouselContentProps): TemplateResult =>
  CarouselContent({ children })

export type InlineCitationCarouselItemProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationCarouselItem = ({ className, children }: InlineCitationCarouselItemProps): TemplateResult =>
  CarouselItem({ className: cx("w-full space-y-2 p-4 pl-8", className), children })

export type InlineCitationCarouselHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationCarouselHeader = ({ className, children }: InlineCitationCarouselHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between gap-2 rounded-t-md bg-secondary p-2", className)}">${children ?? ""}</div>
`

export type InlineCitationCarouselIndexProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationCarouselIndex = ({ className, children }: InlineCitationCarouselIndexProps): TemplateResult => html`
  <div class="${cx("flex flex-1 items-center justify-end px-3 py-1 text-muted-foreground text-xs", className)}">${children ?? "1/1"}</div>
`

export type InlineCitationCarouselPrevProps = {
  readonly className?: string
}

export const InlineCitationCarouselPrev = ({ className }: InlineCitationCarouselPrevProps): TemplateResult =>
  CarouselPrevious({ className: cx("shrink-0", className), children: "<-" })

export type InlineCitationCarouselNextProps = {
  readonly className?: string
}

export const InlineCitationCarouselNext = ({ className }: InlineCitationCarouselNextProps): TemplateResult =>
  CarouselNext({ className: cx("shrink-0", className), children: "->" })

export type InlineCitationSourceProps = {
  readonly className?: string
  readonly title?: string
  readonly url?: string
  readonly description?: string
  readonly children?: AIChildren
}

export const InlineCitationSource = ({ title, url, description, className, children }: InlineCitationSourceProps): TemplateResult => html`
  <div class="${cx("space-y-1", className)}">
    ${title ? html`<h4 class="truncate font-medium text-sm leading-tight">${title}</h4>` : ""}
    ${url ? html`<p class="truncate break-all text-muted-foreground text-xs">${url}</p>` : ""}
    ${description ? html`<p class="line-clamp-3 text-muted-foreground text-sm leading-relaxed">${description}</p>` : ""}
    ${children ?? ""}
  </div>
`

export type InlineCitationQuoteProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const InlineCitationQuote = ({ className, children }: InlineCitationQuoteProps): TemplateResult => html`
  <blockquote class="${cx("border-muted border-l-2 pl-3 text-muted-foreground text-sm italic", className)}">${children ?? ""}</blockquote>
`
