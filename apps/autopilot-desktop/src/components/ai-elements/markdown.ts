import { rawHtml } from "../../effuse/template/html.js"
import { escapeHtml } from "../../effuse/template/escape.js"
import type { TemplateResult } from "../../effuse/template/types.js"

type MarkdownOptions = {
  readonly inline?: boolean
}

const applyMarkdown = (input: string, options: MarkdownOptions = {}) => {
  let output = escapeHtml(input)
  output = output.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
  output = output.replace(/\*(.+?)\*/g, "<strong>$1</strong>")
  output = output.replace(/`([^`]+)`/g, "<code>$1</code>")
  output = options.inline
    ? output.replace(/\r?\n/g, " ")
    : output.replace(/\r?\n/g, "<br />")
  return output
}

export const renderMarkdown = (
  text: string,
  options?: MarkdownOptions
): TemplateResult => rawHtml(applyMarkdown(text, options))

export const renderInlineMarkdown = (text: string): TemplateResult =>
  renderMarkdown(text, { inline: true })
