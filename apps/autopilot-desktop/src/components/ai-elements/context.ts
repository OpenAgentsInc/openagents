import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { HoverCard, HoverCardContent, HoverCardTrigger } from "../ui/hover-card.js"
import { Progress } from "../ui/progress.js"
import { cx, type AIChildren } from "./utils.js"

export type ContextProps = {
  readonly usedTokens: number
  readonly maxTokens: number
  readonly usage?: {
    inputTokens?: number
    outputTokens?: number
    reasoningTokens?: number
    cachedTokens?: number
  }
  readonly modelId?: string
  readonly children?: AIChildren
}

export const Context = ({ usedTokens, maxTokens, usage, modelId, children }: ContextProps): TemplateResult =>
  HoverCard({
    children: children ?? html`
      ${ContextTrigger({ usedTokens, maxTokens })}
      ${ContextContent({ usedTokens, maxTokens, usage, modelId })}
    `,
  })

const formatPercent = (used: number, max: number) => {
  if (!max) return "0%"
  return new Intl.NumberFormat("en-US", { style: "percent", maximumFractionDigits: 1 }).format(used / max)
}

export type ContextTriggerProps = {
  readonly usedTokens: number
  readonly maxTokens: number
  readonly children?: AIChildren
}

export const ContextTrigger = ({ usedTokens, maxTokens, children }: ContextTriggerProps): TemplateResult =>
  HoverCardTrigger({
    children: children ??
      Button({
        type: "button",
        variant: "ghost",
        children: html`<span class="font-medium text-muted-foreground">${formatPercent(usedTokens, maxTokens)}</span>`,
      }),
  })

export type ContextContentProps = {
  readonly usedTokens: number
  readonly maxTokens: number
  readonly usage?: ContextProps["usage"]
  readonly modelId?: string
  readonly className?: string
}

export const ContextContent = ({ usedTokens, maxTokens, usage, modelId, className }: ContextContentProps): TemplateResult => {
  const percent = Math.round((usedTokens / Math.max(maxTokens, 1)) * 100)
  return HoverCardContent({
    className: cx("w-64 space-y-4", className),
    children: html`
      ${ContextContentHeader({ modelId })}
      ${ContextContentBody({ usedTokens, maxTokens, percent, usage })}
      ${ContextContentFooter({})}
    `,
  })
}

export type ContextContentHeaderProps = {
  readonly className?: string
  readonly modelId?: string
}

export const ContextContentHeader = ({ className, modelId }: ContextContentHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between", className)}">
    <div class="text-sm font-medium">Context Usage</div>
    ${modelId ? html`<span class="text-xs text-muted-foreground">${modelId}</span>` : ""}
  </div>
`

export type ContextContentBodyProps = {
  readonly className?: string
  readonly usedTokens: number
  readonly maxTokens: number
  readonly percent: number
  readonly usage?: ContextProps["usage"]
}

export const ContextContentBody = ({
  className,
  usedTokens,
  maxTokens,
  percent,
  usage,
}: ContextContentBodyProps): TemplateResult => html`
  <div class="${cx("space-y-3", className)}">
    <div class="space-y-1">
      <div class="flex items-center justify-between text-xs text-muted-foreground">
        <span>${usedTokens} / ${maxTokens} tokens</span>
        <span>${percent}%</span>
      </div>
      ${Progress({ value: percent })}
    </div>
    ${usage
      ? html`
          ${ContextInputUsage({ value: usage.inputTokens ?? 0 })}
          ${ContextOutputUsage({ value: usage.outputTokens ?? 0 })}
          ${ContextReasoningUsage({ value: usage.reasoningTokens ?? 0 })}
          ${ContextCacheUsage({ value: usage.cachedTokens ?? 0 })}
        `
      : ""}
  </div>
`

export type ContextContentFooterProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ContextContentFooter = ({ className, children }: ContextContentFooterProps): TemplateResult => html`
  <div class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Context is recalculated each turn."}</div>
`

const usageRow = (label: string, value: number) => html`
  <div class="flex items-center justify-between text-xs">
    <span class="text-muted-foreground">${label}</span>
    <span class="font-medium">${value}</span>
  </div>
`

export type ContextUsageProps = {
  readonly value: number
}

export const ContextInputUsage = ({ value }: ContextUsageProps): TemplateResult => usageRow("Input", value)

export const ContextOutputUsage = ({ value }: ContextUsageProps): TemplateResult => usageRow("Output", value)

export const ContextReasoningUsage = ({ value }: ContextUsageProps): TemplateResult => usageRow("Reasoning", value)

export const ContextCacheUsage = ({ value }: ContextUsageProps): TemplateResult => usageRow("Cache", value)
