import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type TabsOrientation = "horizontal" | "vertical"
export type TabsVariant = "default" | "line"

export type TabsProps = {
  readonly className?: string
  readonly orientation?: TabsOrientation
  readonly defaultValue?: string
  readonly children?: UIChildren
}

export const Tabs = ({
  className,
  orientation = "horizontal",
  defaultValue,
  children,
}: TabsProps): TemplateResult => {
  return html`
    <div
      data-slot="tabs"
      data-orientation="${orientation}"
      data-value="${defaultValue ?? ""}"
      class="${cx("group/tabs flex gap-2 data-[orientation=horizontal]:flex-col", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

const tabsListBase =
  "rounded-lg p-[3px] group-data-[orientation=horizontal]/tabs:h-9 data-[variant=line]:rounded-none group/tabs-list text-muted-foreground inline-flex w-fit items-center justify-center group-data-[orientation=vertical]/tabs:h-fit group-data-[orientation=vertical]/tabs:flex-col"

const tabsListVariant: Record<TabsVariant, string> = {
  default: "bg-muted",
  line: "gap-1 bg-transparent",
}

export type TabsListProps = {
  readonly className?: string
  readonly variant?: TabsVariant
  readonly children?: UIChildren
}

export const TabsList = ({
  className,
  variant = "default",
  children,
}: TabsListProps): TemplateResult => {
  return html`
    <div
      data-slot="tabs-list"
      data-variant="${variant}"
      class="${cx(tabsListBase, tabsListVariant[variant], className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export type TabsTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly active?: boolean
  readonly value?: string
}

export const TabsTrigger = ({
  className,
  children,
  active = false,
  value,
}: TabsTriggerProps): TemplateResult => {
  return html`
    <button
      data-slot="tabs-trigger"
      data-state="${active ? "active" : "inactive"}"
      data-value="${value ?? ""}"
      aria-selected="${active ? "true" : "false"}"
      class="${cx(
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:outline-ring text-foreground/60 hover:text-foreground dark:text-muted-foreground dark:hover:text-foreground relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-2 py-1 text-sm font-medium whitespace-nowrap transition-all group-data-[orientation=vertical]/tabs:w-full group-data-[orientation=vertical]/tabs:justify-start focus-visible:ring-[3px] focus-visible:outline-1 disabled:pointer-events-none disabled:opacity-50 group-data-[variant=default]/tabs-list:data-[state=active]:shadow-sm group-data-[variant=line]/tabs-list:data-[state=active]:shadow-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        "group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:border-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent",
        "data-[state=active]:bg-background dark:data-[state=active]:text-foreground dark:data-[state=active]:border-input dark:data-[state=active]:bg-input/30 data-[state=active]:text-foreground",
        "after:bg-foreground after:absolute after:opacity-0 after:transition-opacity group-data-[orientation=horizontal]/tabs:after:inset-x-0 group-data-[orientation=horizontal]/tabs:after:bottom-[-5px] group-data-[orientation=horizontal]/tabs:after:h-0.5 group-data-[orientation=vertical]/tabs:after:inset-y-0 group-data-[orientation=vertical]/tabs:after:-right-1 group-data-[orientation=vertical]/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-[state=active]:after:opacity-100",
        className
      )}"
      type="button"
    >
      ${children ?? ""}
    </button>
  `
}

export type TabsContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly value?: string
  readonly active?: boolean
}

export const TabsContent = ({
  className,
  children,
  value,
  active = false,
}: TabsContentProps): TemplateResult => {
  return html`
    <div
      data-slot="tabs-content"
      data-state="${active ? "active" : "inactive"}"
      data-value="${value ?? ""}"
      class="${cx("flex-1 outline-none", className)}"
    >
      ${children ?? ""}
    </div>
  `
}
