/**
 * HTML template tagged literal
 */

import { escapeHtml } from "./escape.js"
import { isTemplateResult, type TemplatePart, type TemplateResult, type TemplateValue } from "./types.js"

const processValue = (value: TemplateValue): TemplatePart[] => {
  if (value === null || value === undefined) {
    return []
  }

  if (typeof value === "string") {
    return [{ _tag: "Text", value: escapeHtml(value) }]
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return [{ _tag: "Text", value: String(value) }]
  }

  if (isTemplateResult(value)) {
    return [{ _tag: "Template", value }]
  }

  if (Array.isArray(value)) {
    return value.flatMap(processValue)
  }

  return []
}

export const html = (
  strings: TemplateStringsArray,
  ...values: readonly TemplateValue[]
): TemplateResult => {
  const parts: TemplatePart[] = []

  for (let i = 0; i < strings.length; i++) {
    if (strings[i]) {
      parts.push({ _tag: "Html", value: strings[i] as string })
    }

    if (i < values.length) {
      parts.push(...processValue(values[i] as TemplateValue))
    }
  }

  return {
    _tag: "TemplateResult",
    parts,
  }
}

export const rawHtml = (html: string): TemplateResult => {
  return {
    _tag: "TemplateResult",
    parts: [{ _tag: "Html", value: html }],
  }
}

export const joinTemplates = (templates: readonly TemplateResult[]): TemplateResult => {
  return {
    _tag: "TemplateResult",
    parts: templates.flatMap((t) => t.parts),
  }
}
