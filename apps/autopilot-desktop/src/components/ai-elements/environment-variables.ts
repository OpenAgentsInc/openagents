import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { Button } from "../ui/button.js"
import { Switch } from "../ui/switch.js"
import { cx, type AIChildren } from "./utils.js"

export type EnvironmentVariablesProps = {
  readonly className?: string
  readonly showValues?: boolean
  readonly children?: AIChildren
}

export const EnvironmentVariables = ({ className, children }: EnvironmentVariablesProps): TemplateResult => html`
  <div class="${cx("rounded-lg border bg-background", className)}">${children ?? ""}</div>
`

export type EnvironmentVariablesHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariablesHeader = ({ className, children }: EnvironmentVariablesHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between border-b px-4 py-3", className)}">${children ?? ""}</div>
`

export type EnvironmentVariablesTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariablesTitle = ({ className, children }: EnvironmentVariablesTitleProps): TemplateResult => html`
  <h3 class="${cx("font-medium text-sm", className)}">${children ?? "Environment Variables"}</h3>
`

export type EnvironmentVariablesToggleProps = {
  readonly className?: string
  readonly showValues?: boolean
}

export const EnvironmentVariablesToggle = ({ className, showValues = false }: EnvironmentVariablesToggleProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2", className)}">
    <span class="text-muted-foreground text-xs">${showValues ? "show" : "hide"}</span>
    ${Switch({ checked: showValues })}
  </div>
`

export type EnvironmentVariablesContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariablesContent = ({ className, children }: EnvironmentVariablesContentProps): TemplateResult => html`
  <div class="${cx("divide-y", className)}">${children ?? ""}</div>
`

export type EnvironmentVariableProps = {
  readonly name: string
  readonly value: string
  readonly className?: string
  readonly children?: AIChildren
  readonly showValues?: boolean
}

export const EnvironmentVariable = ({
  name,
  value,
  className,
  children,
  showValues = false,
}: EnvironmentVariableProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between gap-4 px-4 py-3", className)}">
    ${children ?? html`
      <div class="flex items-center gap-2">
        ${EnvironmentVariableName({ name })}
      </div>
      ${EnvironmentVariableValue({ value, showValues })}
    `}
  </div>
`

export type EnvironmentVariableGroupProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariableGroup = ({ className, children }: EnvironmentVariableGroupProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2", className)}">${children ?? ""}</div>
`

export type EnvironmentVariableNameProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly name?: string
}

export const EnvironmentVariableName = ({ className, children, name }: EnvironmentVariableNameProps): TemplateResult => html`
  <span class="${cx("font-mono text-sm", className)}">${children ?? name ?? ""}</span>
`

export type EnvironmentVariableValueProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly value?: string
  readonly showValues?: boolean
}

export const EnvironmentVariableValue = ({ className, children, value = "", showValues = false }: EnvironmentVariableValueProps): TemplateResult => {
  const displayValue = showValues ? value : "*".repeat(Math.min(value.length, 20))
  return html`
    <span class="${cx("font-mono text-muted-foreground text-sm", !showValues ? "select-none" : "", className)}">
      ${children ?? displayValue}
    </span>
  `
}

export type EnvironmentVariableCopyButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariableCopyButton = ({ className, children }: EnvironmentVariableCopyButtonProps): TemplateResult =>
  Button({ className: cx("size-7", className), size: "icon", type: "button", variant: "ghost", children: children ?? "copy" })

export type EnvironmentVariableRequiredProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const EnvironmentVariableRequired = ({ className, children }: EnvironmentVariableRequiredProps): TemplateResult =>
  Badge({ className: cx("text-xs", className), variant: "secondary", children: children ?? "required" })
