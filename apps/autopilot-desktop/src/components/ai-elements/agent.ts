import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "../ui/accordion.js"
import { Badge } from "../ui/badge.js"
import { CodeBlock } from "./code-block.js"
import { cx, type AIChildren } from "./utils.js"

export type AgentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Agent = ({ className, children }: AgentProps): TemplateResult => html`
  <div class="${cx("not-prose w-full rounded-md border", className)}">${children ?? ""}</div>
`

export type AgentHeaderProps = {
  readonly className?: string
  readonly name: string
  readonly model?: string
  readonly children?: AIChildren
}

export const AgentHeader = ({ className, name, model, children }: AgentHeaderProps): TemplateResult => html`
  <div class="${cx("flex w-full items-center justify-between gap-4 p-3", className)}">
    ${children ?? html`
      <div class="flex items-center gap-2">
        <span class="size-4 text-muted-foreground" aria-hidden="true">bot</span>
        <span class="font-medium text-sm">${name}</span>
        ${model
          ? Badge({ className: "font-mono text-xs", variant: "secondary", children: model })
          : ""}
      </div>
    `}
  </div>
`

export type AgentContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AgentContent = ({ className, children }: AgentContentProps): TemplateResult => html`
  <div class="${cx("space-y-4 p-4 pt-0", className)}">${children ?? ""}</div>
`

export type AgentInstructionsProps = {
  readonly className?: string
  readonly children: string
}

export const AgentInstructions = ({ className, children }: AgentInstructionsProps): TemplateResult => html`
  <div class="${cx("space-y-2", className)}">
    <span class="font-medium text-muted-foreground text-sm">Instructions</span>
    <div class="rounded-md bg-muted/50 p-3 text-muted-foreground text-sm">
      <p>${children}</p>
    </div>
  </div>
`

export type AgentToolsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AgentTools = ({ className, children }: AgentToolsProps): TemplateResult => html`
  <div class="${cx("space-y-2", className)}">
    <span class="font-medium text-muted-foreground text-sm">Tools</span>
    ${Accordion({ className: "rounded-md border", children })}
  </div>
`

export type AgentToolProps = {
  readonly className?: string
  readonly tool: { description?: string | null; inputSchema?: unknown; jsonSchema?: unknown }
  readonly value?: string
}

export const AgentTool = ({ className, tool, value }: AgentToolProps): TemplateResult => {
  const schema = "jsonSchema" in tool && tool.jsonSchema ? tool.jsonSchema : tool.inputSchema
  return AccordionItem({
    className: cx("border-b last:border-b-0", className),
    children: html`
      ${AccordionTrigger({ className: "px-3 py-2 text-sm hover:no-underline", children: tool.description ?? "No description" })}
      ${AccordionContent({ className: "px-3 pb-3", children: html`
        <div class="rounded-md bg-muted/50">
          ${CodeBlock({ code: JSON.stringify(schema ?? {}, null, 2), language: "json" })}
        </div>
      ` })}
    `,
  })
}

export type AgentOutputProps = {
  readonly className?: string
  readonly schema: string
}

export const AgentOutput = ({ className, schema }: AgentOutputProps): TemplateResult => html`
  <div class="${cx("space-y-2", className)}">
    <span class="font-medium text-muted-foreground text-sm">Output Schema</span>
    <div class="rounded-md bg-muted/50">
      ${CodeBlock({ code: schema, language: "typescript" })}
    </div>
  </div>
`
