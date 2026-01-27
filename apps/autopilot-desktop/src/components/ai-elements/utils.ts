import type { TemplateResult } from "../../effuse/template/types.js"

export type AIChild = TemplateResult | string | number | boolean | null | undefined
export type AIChildren = AIChild | AIChild[]

export const cx = (...classes: Array<string | false | null | undefined>) =>
  classes.filter(Boolean).join(" ")
