/**
 * Effuse HTML Template System
 *
 * Type-safe HTML templates with automatic escaping.
 * Inspired by Typed's template system but simplified for our use case.
 */

import type { TemplateResult, TemplateValue } from "./types.js"
import { isTemplateResult } from "./types.js"
import { escapeHtml } from "./escape.js"

/**
 * Render a single value to an escaped string.
 */
const renderValue = (value: unknown): string => {
  // Null/undefined render as empty
  if (value === null || value === undefined) {
    return ""
  }

  // Primitives get escaped
  if (typeof value === "string") {
    return escapeHtml(value)
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value)
  }

  // TemplateResults are already escaped, use their string form
  if (isTemplateResult(value)) {
    return value.toString()
  }

  // Arrays: render each element
  if (Array.isArray(value)) {
    return value.map(renderValue).join("")
  }

  // Fallback: escape string representation
  return escapeHtml(String(value))
}

/**
 * Tagged template literal for creating type-safe HTML.
 *
 * All interpolated values are automatically escaped to prevent XSS.
 * Nested TemplateResults are rendered without double-escaping.
 *
 * @example
 * ```typescript
 * const name = "<script>alert('xss')</script>"
 * const template = html`<div>Hello, ${name}!</div>`
 * // Renders: <div>Hello, &lt;script&gt;alert('xss')&lt;/script&gt;!</div>
 * ```
 *
 * @example
 * ```typescript
 * const items = ['a', 'b', 'c']
 * const list = html`<ul>${items.map(i => html`<li>${i}</li>`)}</ul>`
 * // Renders: <ul><li>a</li><li>b</li><li>c</li></ul>
 * ```
 */
export function html(
  strings: TemplateStringsArray,
  ...values: TemplateValue[]
): TemplateResult {
  return {
    _tag: "TemplateResult",
    strings,
    values,
    toString() {
      let result = ""
      for (let i = 0; i < strings.length; i++) {
        result += strings[i]
        if (i < values.length) {
          result += renderValue(values[i])
        }
      }
      return result
    },
  }
}

/**
 * Create a raw (unescaped) HTML template.
 * USE WITH CAUTION - only for trusted content like SVG paths.
 */
export function rawHtml(content: string): TemplateResult {
  return {
    _tag: "TemplateResult",
    strings: [content] as unknown as TemplateStringsArray,
    values: [],
    toString() {
      return content
    },
  }
}

/**
 * Join multiple TemplateResults with a separator.
 */
export function joinTemplates(
  templates: TemplateResult[],
  separator: string = ""
): TemplateResult {
  const joined = templates.map((t) => t.toString()).join(separator)
  return rawHtml(joined)
}
