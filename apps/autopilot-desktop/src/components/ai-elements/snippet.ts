import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput, InputGroupText } from "../ui/input-group.js"
import { cx } from "./utils.js"

export type SnippetProps = {
  readonly code: string
  readonly className?: string
  readonly children?: any
}

export const Snippet = ({ code, className, children }: SnippetProps): TemplateResult =>
  InputGroup({
    className: cx("font-mono", className),
    dataRole: "snippet",
    dataCopyValue: code,
    children: children ?? html`
      ${SnippetText({ children: code })}
      ${SnippetInput({ code })}
      ${SnippetCopyButton({})}
    `,
  })

export type SnippetAddonProps = {
  readonly className?: string
  readonly children?: any
}

export const SnippetAddon = ({ className, children }: SnippetAddonProps): TemplateResult =>
  InputGroupAddon({ className, children })

export type SnippetTextProps = {
  readonly className?: string
  readonly children?: any
}

export const SnippetText = ({ className, children }: SnippetTextProps): TemplateResult =>
  InputGroupText({ className: cx("pl-2 font-normal text-muted-foreground", className), children })

export type SnippetInputProps = {
  readonly className?: string
  readonly code: string
}

export const SnippetInput = ({ className, code }: SnippetInputProps): TemplateResult =>
  InputGroupInput({ className: cx("text-foreground", className), value: code })

export type SnippetCopyButtonProps = {
  readonly className?: string
  readonly children?: any
}

export const SnippetCopyButton = ({ className, children }: SnippetCopyButtonProps): TemplateResult =>
  InputGroupButton({
    className,
    size: "icon-sm",
    dataUi: "copy",
    dataCopyTarget: "closest([data-role='snippet'])",
    ariaLabel: "Copy",
    title: "Copy",
    children: children ?? "copy",
  })
