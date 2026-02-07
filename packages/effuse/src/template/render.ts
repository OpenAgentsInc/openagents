import type { TemplateResult } from "./types.js"

/**
 * Convert an Effuse `TemplateResult` into a string of HTML.
 *
 * Important:
 * - This must be SSR-safe (no DOM).
 * - `Text` parts are assumed already escaped (via `html` value processing).
 */
export const renderToString = (template: TemplateResult): string => {
  let out = ""

  for (const part of template.parts) {
    switch (part._tag) {
      case "Text":
      case "Html":
        out += part.value
        break
      case "Template":
        out += renderToString(part.value)
        break
    }
  }

  return out
}

