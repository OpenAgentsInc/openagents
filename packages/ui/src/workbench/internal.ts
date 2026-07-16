/**
 * Private DOM/CSS helpers shared across the workbench modules (#8860, epic
 * #8857 Wave 1). Not part of the public `@openagentsinc/ui/desktop-workbench`
 * surface — imported by sibling modules in this directory only.
 */
export const cx = (...values: ReadonlyArray<string | false | null | undefined>): string =>
  values.filter(Boolean).join(" ")

export const px = (value: string | number): string => typeof value === "number" ? `${value}px` : value
