import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../ui/select.js"
import { cx, type AIChildren } from "./utils.js"

export type CodeBlockContainerProps = {
  readonly className?: string
  readonly language: string
  readonly style?: string
  readonly copyValue?: string
  readonly children?: AIChildren
}

export const CodeBlockContainer = ({
  className,
  language,
  copyValue,
  children,
}: CodeBlockContainerProps): TemplateResult => html`
  <div
    data-slot="code-block"
    class="${cx("group relative w-full overflow-hidden rounded-md border bg-background text-foreground", className)}"
    data-language="${language}"
    data-copy-value="${copyValue ?? ""}"
  >
    ${children ?? ""}
  </div>
`

export type CodeBlockHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockHeader = ({ className, children }: CodeBlockHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between bg-muted/80 px-3 py-2 text-muted-foreground text-xs", className)}">
    ${children ?? ""}
  </div>
`

export type CodeBlockTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockTitle = ({ className, children }: CodeBlockTitleProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2", className)}">${children ?? ""}</div>
`

export type CodeBlockFilenameProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockFilename = ({ className, children }: CodeBlockFilenameProps): TemplateResult => html`
  <span class="${cx("font-mono", className)}">${children ?? ""}</span>
`

export type CodeBlockActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockActions = ({ className, children }: CodeBlockActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2", className)}">${children ?? ""}</div>
`

export type CodeBlockContentProps = {
  readonly code: string
  readonly language: string
  readonly showLineNumbers?: boolean
}

export const CodeBlockContent = ({
  code,
  language,
  showLineNumbers = false,
}: CodeBlockContentProps): TemplateResult => {
  const lines = code.split("\n")
  return html`
    <div class="relative overflow-auto">
      <pre class="whitespace-pre-wrap break-words p-3 text-xs text-foreground" data-language="${language}">
        ${lines
          .map((line, index) =>
            showLineNumbers
              ? html`<div class="grid grid-cols-[2rem_1fr] gap-2">
                  <span class="text-muted-foreground">${index + 1}</span>
                  <code>${line || " "}</code>
                </div>`
              : html`<code class="block">${line || " "}</code>`
          )}
      </pre>
    </div>
  `
}

export type CodeBlockProps = {
  readonly code: string
  readonly language: string
  readonly showLineNumbers?: boolean
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlock = ({
  code,
  language,
  showLineNumbers = false,
  className,
  children,
}: CodeBlockProps): TemplateResult => html`
  ${CodeBlockContainer({ className, language, copyValue: code, children: html`
    ${children ?? ""}
    ${CodeBlockContent({ code, language, showLineNumbers })}
  ` })}
`

export type CodeBlockCopyButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockCopyButton = ({ className, children }: CodeBlockCopyButtonProps): TemplateResult =>
  Button({
    className: cx("shrink-0", className),
    size: "icon",
    type: "button",
    variant: "ghost",
    dataUi: "copy",
    dataCopyTarget: "closest([data-slot='code-block'])",
    children: children ?? "copy",
  })

export type CodeBlockLanguageSelectorProps = {
  readonly children?: AIChildren
}

export const CodeBlockLanguageSelector = ({ children }: CodeBlockLanguageSelectorProps): TemplateResult =>
  Select({ children })

export type CodeBlockLanguageSelectorTriggerProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const CodeBlockLanguageSelectorTrigger = ({ className, children }: CodeBlockLanguageSelectorTriggerProps): TemplateResult =>
  SelectTrigger({
    className: cx("h-7 border-none bg-transparent px-2 text-xs shadow-none", className),
    size: "sm",
    children,
  })

export type CodeBlockLanguageSelectorValueProps = {
  readonly children?: AIChildren
}

export const CodeBlockLanguageSelectorValue = ({ children }: CodeBlockLanguageSelectorValueProps): TemplateResult =>
  SelectValue({ children })

export type CodeBlockLanguageSelectorContentProps = {
  readonly align?: "start" | "center" | "end"
  readonly children?: AIChildren
}

export const CodeBlockLanguageSelectorContent = ({ align = "end", children }: CodeBlockLanguageSelectorContentProps): TemplateResult =>
  SelectContent({ align, children })

export type CodeBlockLanguageSelectorItemProps = {
  readonly value?: string
  readonly children?: AIChildren
}

export const CodeBlockLanguageSelectorItem = ({ value, children }: CodeBlockLanguageSelectorItemProps): TemplateResult =>
  SelectItem({ value, children })
