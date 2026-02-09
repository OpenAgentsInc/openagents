import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import type { FlowNodeBadgeTone, FlowNodeStatus } from "./types.js"

const cx = (...parts: Array<string | null | undefined | false>): string =>
  parts.filter(Boolean).join(" ")

export function StatusDot(input: { readonly status?: FlowNodeStatus; readonly className?: string }): TemplateResult {
  const status = input.status ?? "ok"
  return html`<span class="${cx("oa-flow-status-dot", input.className)}" data-status="${status}" aria-hidden="true"></span>`
}

export function Pill(input: {
  readonly tone?: FlowNodeBadgeTone
  readonly className?: string
  readonly children: string
}): TemplateResult {
  const tone = input.tone ?? "neutral"
  return html`<span class="${cx("oa-flow-pill", input.className)}" data-tone="${tone}">${input.children}</span>`
}

export function StatusPill(input: { readonly status?: FlowNodeStatus }): TemplateResult {
  const status = input.status ?? "ok"
  const tone: FlowNodeBadgeTone =
    status === "error"
      ? "destructive"
      : status === "pending"
        ? "warning"
        : status === "running"
          ? "success"
          : status === "live"
            ? "info"
            : "neutral"

  return Pill({ tone, children: status.toUpperCase() })
}
