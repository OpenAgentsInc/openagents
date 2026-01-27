import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { CodexReasoning } from "../../types/codex.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { renderInlineMarkdown, renderMarkdown } from "./markdown.js"
import { Shimmer } from "./shimmer.js"
import { cx, type AIChildren } from "./utils.js"

export type ReasoningLegacyProps = Pick<CodexReasoning, "summary" | "content"> & {
  readonly isStreaming?: boolean
  readonly open?: boolean
}

const ReasoningSummary = ({
  summary = "Reasoning",
  content,
  isStreaming = false,
  open,
}: ReasoningLegacyProps): TemplateResult => html`
  <details
    class="block w-full border border-border bg-background px-3 py-2 text-xs text-foreground"
    ${open || isStreaming ? "open" : ""}
  >
    <summary class="flex w-full cursor-pointer list-none items-center justify-between text-[10px] text-muted-foreground">
      <span class="flex items-center gap-2">
        <span class="inline-flex h-2 w-2 rounded-full ${isStreaming ? "bg-accent" : "bg-muted-foreground"}"></span>
        ${renderInlineMarkdown(summary)}
      </span>
      <span class="text-[10px] font-semibold ${isStreaming ? "text-accent" : "text-muted-foreground"}">
        ${isStreaming ? "Thinking" : "Complete"}
      </span>
    </summary>
    <div class="mt-2 break-words text-xs leading-relaxed text-foreground">
      ${renderMarkdown(content ?? "No reasoning captured yet.")}
    </div>
  </details>
`

export type ReasoningProps = (ReasoningLegacyProps & { readonly children?: never }) | {
  readonly className?: string
  readonly children?: AIChildren
}

export const Reasoning = (props: ReasoningProps): TemplateResult => {
  if ("content" in props || "summary" in props) {
    return ReasoningSummary(props as ReasoningLegacyProps)
  }
  const { className, children } = props
  return Collapsible({ className: cx("not-prose mb-4", className), children })
}

export type ReasoningTriggerProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly getThinkingMessage?: (isStreaming: boolean, duration?: number) => AIChildren
  readonly isStreaming?: boolean
  readonly duration?: number
  readonly isOpen?: boolean
}

const defaultGetThinkingMessage = (isStreaming: boolean, duration?: number) => {
  if (isStreaming || duration === 0) {
    return Shimmer({ duration: 1, children: "Thinking..." })
  }
  if (duration === undefined) {
    return html`<p>Thought for a few seconds</p>`
  }
  return html`<p>Thought for ${duration} seconds</p>`
}

export const ReasoningTrigger = ({
  className,
  children,
  getThinkingMessage = defaultGetThinkingMessage,
  isStreaming = false,
  duration,
  isOpen = false,
}: ReasoningTriggerProps): TemplateResult =>
  CollapsibleTrigger({
    className: cx(
      "flex w-full items-center gap-2 text-muted-foreground text-sm transition-colors hover:text-foreground",
      className
    ),
    children: children ?? html`
      <span class="size-4">think</span>
      ${getThinkingMessage(isStreaming, duration)}
      <span class="size-4 ${isOpen ? "rotate-180" : "rotate-0"}">v</span>
    `,
  })

export type ReasoningContentProps = {
  readonly className?: string
  readonly children: string
}

export const ReasoningContent = ({ className, children }: ReasoningContentProps): TemplateResult =>
  CollapsibleContent({
    className: cx(
      "mt-4 text-sm",
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-muted-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className
    ),
    children: renderMarkdown(children),
  })
