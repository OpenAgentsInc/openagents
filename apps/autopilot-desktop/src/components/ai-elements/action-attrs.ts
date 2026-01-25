import { rawHtml } from "../../effuse/template/html.js"
import { escapeHtml } from "../../effuse/template/escape.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { Action } from "../../effuse/ui/index.js"

export type ActionAttributeOptions = {
  trigger?: "click" | "submit" | "change" | "input"
  target?: string
  swap?: "inner" | "outer" | "beforeend" | "afterbegin" | "delete" | "replace"
}

export const actionAttributes = (
  action?: Action,
  options?: ActionAttributeOptions
): TemplateResult | "" => {
  if (!action) {
    return ""
  }
  const payload = escapeHtml(JSON.stringify(action))
  const trigger = options?.trigger ? ` data-ez-trigger="${options.trigger}"` : ""
  const target = options?.target ? ` data-ez-target="${escapeHtml(options.target)}"` : ""
  const swap = options?.swap ? ` data-ez-swap="${options.swap}"` : ""
  return rawHtml(
    ` data-ez="ui.action" data-ez-vals="${payload}" data-ez-disable${trigger}${target}${swap}`
  )
}
