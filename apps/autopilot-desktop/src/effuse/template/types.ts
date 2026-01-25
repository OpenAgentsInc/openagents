/**
 * Template system types
 */

export type TemplatePart =
  | { readonly _tag: "Text"; readonly value: string }
  | { readonly _tag: "Html"; readonly value: string }
  | { readonly _tag: "Template"; readonly value: TemplateResult }

export type TemplateResult = {
  readonly _tag: "TemplateResult"
  readonly parts: readonly TemplatePart[]
}

export type TemplateValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | TemplateResult
  | readonly TemplateValue[]

export const isTemplateResult = (value: unknown): value is TemplateResult => {
  return (
    typeof value === "object" &&
    value !== null &&
    "_tag" in value &&
    (value as { _tag: unknown })._tag === "TemplateResult"
  )
}
