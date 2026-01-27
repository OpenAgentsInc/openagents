import type { TemplateResult } from "../../effuse/template/types.js"

export type UIChild = TemplateResult | string | number | boolean | null | undefined
export type UIChildren = UIChild | UIChild[]

export const cx = (...classes: Array<string | false | null | undefined>) =>
  classes.filter(Boolean).join(" ")
