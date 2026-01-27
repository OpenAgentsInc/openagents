import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../ui/tabs.js"
import { cx, type AIChildren } from "./utils.js"
import { getStatusBadge, type ToolState } from "./tool.js"

export type SandboxProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Sandbox = ({ className, children }: SandboxProps): TemplateResult =>
  Collapsible({
    className: cx("not-prose group mb-4 w-full overflow-hidden rounded-md border", className),
    children,
  })

export type SandboxHeaderProps = {
  readonly title?: string
  readonly state?: ToolState
  readonly className?: string
}

export const SandboxHeader = ({ className, title = "Sandbox", state = "output-available" }: SandboxHeaderProps): TemplateResult =>
  CollapsibleTrigger({
    className: cx("flex w-full items-center justify-between gap-4 p-3", className),
    children: html`
      <div class="flex items-center gap-2">
        <span class="size-4 text-muted-foreground">cmd</span>
        <span class="font-medium text-sm">${title}</span>
        ${getStatusBadge(state)}
      </div>
      <span class="size-4 text-muted-foreground transition-transform group-data-[state=open]:rotate-180">v</span>
    `,
  })

export type SandboxContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxContent = ({ className, children }: SandboxContentProps): TemplateResult =>
  CollapsibleContent({
    className: cx(
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className
    ),
    children,
  })

export type SandboxTabsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxTabs = ({ className, children }: SandboxTabsProps): TemplateResult =>
  Tabs({ className: cx("w-full gap-0", className), children })

export type SandboxTabsBarProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxTabsBar = ({ className, children }: SandboxTabsBarProps): TemplateResult => html`
  <div class="${cx("flex w-full items-center border-border border-t border-b", className)}">${children ?? ""}</div>
`

export type SandboxTabsListProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxTabsList = ({ className, children }: SandboxTabsListProps): TemplateResult =>
  TabsList({ className: cx("h-auto rounded-none border-0 bg-transparent p-0", className), children })

export type SandboxTabsTriggerProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxTabsTrigger = ({ className, children }: SandboxTabsTriggerProps): TemplateResult =>
  TabsTrigger({
    className: cx(
      "rounded-none border-0 border-transparent border-b-2 px-4 py-2 font-medium text-muted-foreground text-sm transition-colors data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none",
      className
    ),
    children,
  })

export type SandboxTabContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SandboxTabContent = ({ className, children }: SandboxTabContentProps): TemplateResult =>
  TabsContent({ className: cx("mt-0 text-sm", className), children })
