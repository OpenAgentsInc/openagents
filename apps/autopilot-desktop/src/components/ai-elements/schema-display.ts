import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE"

export interface SchemaParameter {
  name: string
  type: string
  required?: boolean
  description?: string
  location?: "path" | "query" | "header"
}

export interface SchemaProperty {
  name: string
  type: string
  required?: boolean
  description?: string
  properties?: SchemaProperty[]
  items?: SchemaProperty
}

export type SchemaDisplayProps = {
  readonly method: HttpMethod
  readonly path: string
  readonly description?: string
  readonly parameters?: SchemaParameter[]
  readonly requestBody?: SchemaProperty[]
  readonly responseBody?: SchemaProperty[]
  readonly className?: string
  readonly children?: AIChildren
}

const methodStyles: Record<HttpMethod, string> = {
  GET: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  POST: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  PUT: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
  PATCH: "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
  DELETE: "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
}

export const SchemaDisplay = ({
  method,
  path,
  description,
  parameters,
  requestBody,
  responseBody,
  className,
  children,
}: SchemaDisplayProps): TemplateResult => html`
  <div class="${cx("overflow-hidden rounded-lg border bg-background", className)}">
    ${children ?? html`
      ${SchemaDisplayHeader({ children: html`
        <div class="flex items-center gap-3">
          ${SchemaDisplayMethod({ method })}
          ${SchemaDisplayPath({ path })}
        </div>
      ` })}
      ${description ? SchemaDisplayDescription({ description }) : ""}
      ${SchemaDisplayContent({
        children: html`
          ${parameters && parameters.length ? SchemaDisplayParameters({ parameters }) : ""}
          ${requestBody && requestBody.length ? SchemaDisplayRequest({ properties: requestBody }) : ""}
          ${responseBody && responseBody.length ? SchemaDisplayResponse({ properties: responseBody }) : ""}
        `,
      })}
    `}
  </div>
`

export type SchemaDisplayHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SchemaDisplayHeader = ({ className, children }: SchemaDisplayHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-3 border-b px-4 py-3", className)}">${children ?? ""}</div>
`

export type SchemaDisplayMethodProps = {
  readonly method: HttpMethod
  readonly className?: string
}

export const SchemaDisplayMethod = ({ method, className }: SchemaDisplayMethodProps): TemplateResult =>
  Badge({ className: cx("font-mono text-xs", methodStyles[method], className), variant: "secondary", children: method })

export type SchemaDisplayPathProps = {
  readonly path: string
  readonly className?: string
}

export const SchemaDisplayPath = ({ path, className }: SchemaDisplayPathProps): TemplateResult => html`
  <span class="${cx("font-mono text-xs text-muted-foreground", className)}">${path}</span>
`

export type SchemaDisplayDescriptionProps = {
  readonly description: string
  readonly className?: string
}

export const SchemaDisplayDescription = ({ description, className }: SchemaDisplayDescriptionProps): TemplateResult => html`
  <div class="${cx("px-4 py-2 text-sm text-muted-foreground", className)}">${description}</div>
`

export type SchemaDisplayContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SchemaDisplayContent = ({ className, children }: SchemaDisplayContentProps): TemplateResult => html`
  <div class="${cx("divide-y", className)}">${children ?? ""}</div>
`

export type SchemaDisplayParametersProps = {
  readonly parameters: SchemaParameter[]
  readonly className?: string
}

export const SchemaDisplayParameters = ({ parameters, className }: SchemaDisplayParametersProps): TemplateResult => html`
  <div class="${cx("px-4 py-3", className)}">
    <h4 class="text-xs font-medium text-muted-foreground">Parameters</h4>
    <div class="mt-2 space-y-2">
      ${parameters.map((param) => SchemaDisplayParameter({ parameter: param }))}
    </div>
  </div>
`

export type SchemaDisplayParameterProps = {
  readonly parameter: SchemaParameter
  readonly className?: string
}

export const SchemaDisplayParameter = ({ parameter, className }: SchemaDisplayParameterProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2 text-xs", className)}">
    <span class="font-mono">${parameter.name}</span>
    <span class="text-muted-foreground">${parameter.type}</span>
    ${parameter.required ? Badge({ className: "text-[10px]", variant: "secondary", children: "required" }) : ""}
  </div>
`

export type SchemaDisplayRequestProps = {
  readonly properties: SchemaProperty[]
  readonly className?: string
}

export const SchemaDisplayRequest = ({ properties, className }: SchemaDisplayRequestProps): TemplateResult =>
  SchemaDisplayBody({ title: "Request", properties, className })

export type SchemaDisplayResponseProps = {
  readonly properties: SchemaProperty[]
  readonly className?: string
}

export const SchemaDisplayResponse = ({ properties, className }: SchemaDisplayResponseProps): TemplateResult =>
  SchemaDisplayBody({ title: "Response", properties, className })

export type SchemaDisplayBodyProps = {
  readonly title: string
  readonly properties: SchemaProperty[]
  readonly className?: string
}

export const SchemaDisplayBody = ({ title, properties, className }: SchemaDisplayBodyProps): TemplateResult => html`
  <div class="${cx("px-4 py-3", className)}">
    <Collapsible>
      ${CollapsibleTrigger({
        className: "flex items-center gap-2 text-xs font-medium text-muted-foreground",
        children: html`<span>></span>${title}`,
      })}
      ${CollapsibleContent({
        className: "mt-2",
        children: html`
          <div class="space-y-2">
            ${properties.map((prop) => SchemaDisplayProperty({ property: prop }))}
          </div>
        `,
      })}
    </Collapsible>
  </div>
`

export type SchemaDisplayPropertyProps = {
  readonly property: SchemaProperty
  readonly className?: string
}

export const SchemaDisplayProperty = ({ property, className }: SchemaDisplayPropertyProps): TemplateResult => html`
  <div class="${cx("flex items-start gap-2 text-xs", className)}">
    <span class="font-mono">${property.name}</span>
    <span class="text-muted-foreground">${property.type}</span>
    ${property.required ? Badge({ className: "text-[10px]", variant: "secondary", children: "required" }) : ""}
  </div>
`

export type SchemaDisplayExampleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const SchemaDisplayExample = ({ className, children }: SchemaDisplayExampleProps): TemplateResult => html`
  <div class="${cx("rounded-md border bg-muted/30 p-3 text-xs", className)}">${children ?? ""}</div>
`
