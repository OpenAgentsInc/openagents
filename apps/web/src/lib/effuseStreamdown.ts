import { escapeHtml, html, rawHtml } from "@openagentsinc/effuse"
import remend from "remend"
import { Lexer, Renderer, marked } from "marked"

import type { TemplateResult } from "@openagentsinc/effuse"

export type EffuseStreamdownCaret = "block" | "circle"
export type EffuseStreamdownMode = "static" | "streaming"

export type EffuseStreamdownOptions = {
  readonly mode?: EffuseStreamdownMode
  readonly isAnimating?: boolean
  readonly caret?: EffuseStreamdownCaret
  readonly parseIncompleteMarkdown?: boolean
}

const CARETS: Record<EffuseStreamdownCaret, string> = {
  block: " ▋",
  circle: " ●",
}

const safeHref = (href: string): string | null => {
  const raw = href.trim()
  if (!raw) return null
  if (raw.startsWith("#")) return raw
  if (raw.startsWith("/")) return raw
  if (raw.startsWith("./") || raw.startsWith("../")) return raw

  try {
    const url = new URL(raw)
    const protocol = url.protocol.toLowerCase()
    if (protocol === "http:" || protocol === "https:" || protocol === "mailto:" || protocol === "tel:") {
      return url.toString()
    }
    return null
  } catch {
    return null
  }
}

const classAttr = (value: string | null | undefined): string => (value ? ` class="${value}"` : "")

// A minimal, safe Markdown renderer tuned for streaming UX:
// - no raw HTML passthrough (html tokens are escaped)
// - safe link protocols only
// - Tailwind utility classes aligned with Streamdown defaults (lists, headings, etc.)
const createRenderer = (): Renderer => {
  const r = new Renderer()

  r.html = ({ text }) => escapeHtml(text)

  r.paragraph = function ({ tokens }) {
    const inner = this.parser.parseInline(tokens)
    return `<p>${inner}</p>`
  }

  r.heading = function ({ tokens, depth }) {
    const inner = this.parser.parseInline(tokens)
    const cls =
      depth === 1
        ? "mt-6 mb-2 font-semibold text-3xl"
        : depth === 2
          ? "mt-6 mb-2 font-semibold text-2xl"
          : depth === 3
            ? "mt-6 mb-2 font-semibold text-xl"
            : depth === 4
              ? "mt-6 mb-2 font-semibold text-lg"
              : depth === 5
                ? "mt-6 mb-2 font-semibold text-base"
                : "mt-6 mb-2 font-semibold text-sm"
    return `<h${depth}${classAttr(cls)} data-streamdown="heading-${depth}">${inner}</h${depth}>`
  }

  r.hr = () => `<hr class="my-6 border-border" data-streamdown="horizontal-rule" />`

  r.strong = function ({ tokens }) {
    const inner = this.parser.parseInline(tokens)
    return `<strong class="font-semibold" data-streamdown="strong">${inner}</strong>`
  }

  r.em = function ({ tokens }) {
    const inner = this.parser.parseInline(tokens)
    return `<em>${inner}</em>`
  }

  r.codespan = ({ text }) =>
    `<code class="rounded border border-border-dark bg-bg-secondary px-1 py-0.5 text-xs" data-streamdown="inline-code">${escapeHtml(
      text,
    )}</code>`

  r.code = ({ text, lang }) => {
    const language = typeof lang === "string" && lang.trim() ? lang.trim() : ""
    const langClass = language ? ` language-${escapeHtml(language)}` : ""
    return [
      `<pre class="overflow-x-auto rounded border border-border-dark bg-surface-primary/35 px-3 py-2 text-xs shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]" data-streamdown="code-block">`,
      `<code class="font-mono${langClass}">${escapeHtml(text)}</code>`,
      `</pre>`,
    ].join("")
  }

  r.blockquote = function ({ tokens }) {
    const inner = this.parser.parse(tokens)
    return `<blockquote class="my-4 border-muted-foreground/30 border-l-4 pl-4 text-muted-foreground italic" data-streamdown="blockquote">${inner}</blockquote>`
  }

  r.list = function (token) {
    const tag = token.ordered ? "ol" : "ul"
    const cls = token.ordered
      ? 'list-inside list-decimal whitespace-normal [li_&]:pl-6'
      : 'list-inside list-disc whitespace-normal [li_&]:pl-6'
    const type = token.ordered ? "ordered-list" : "unordered-list"
    const body = token.items.map((item) => this.listitem(item)).join("")
    return `<${tag} class="${cls}" data-streamdown="${type}">${body}</${tag}>`
  }

  r.listitem = function (item) {
    const inner = this.parser.parse(item.tokens)
    return `<li class="py-1 [&>p]:inline" data-streamdown="list-item">${inner}</li>`
  }

  r.link = function ({ href, title, tokens }) {
    const inner = this.parser.parseInline(tokens)
    const safe = safeHref(href)
    if (!safe) {
      return `<span class="text-text-muted underline decoration-dotted" data-streamdown="link-blocked">${inner}</span>`
    }
    const titleAttr = title ? ` title="${escapeHtml(title)}"` : ""
    return `<a class="wrap-anywhere font-medium text-primary underline" data-streamdown="link" href="${escapeHtml(
      safe,
    )}" rel="noreferrer" target="_blank"${titleAttr}>${inner}</a>`
  }

  r.image = ({ href, text }) => {
    const safe = safeHref(href)
    if (!safe || safe.startsWith("mailto:") || safe.startsWith("tel:")) {
      return `<span class="text-text-muted" data-streamdown="image-blocked">[image]</span>`
    }
    const alt = escapeHtml(text ?? "")
    return `<img src="${escapeHtml(safe)}" alt="${alt}" class="max-w-full rounded border border-border-dark" data-streamdown="image" />`
  }

  // Basic tables (no interactive controls; just styling).
  r.table = function (token) {
    const header = token.header.map((cell) => this.tablecell(cell)).join("")
    const headerRow = this.tablerow({ text: header } as any)
    const bodyRows = token.rows.map((row) => this.tablerow({ text: row.map((c) => this.tablecell(c)).join("") } as any)).join("")
    return [
      `<table class="w-full border-collapse overflow-hidden rounded border border-border-dark" data-streamdown="table">`,
      `<thead class="bg-muted/80" data-streamdown="table-header">${headerRow}</thead>`,
      `<tbody class="divide-y divide-border bg-muted/40" data-streamdown="table-body">${bodyRows}</tbody>`,
      `</table>`,
    ].join("")
  }

  r.tablerow = ({ text }) => `<tr class="border-border border-b" data-streamdown="table-row">${text}</tr>`

  r.tablecell = function (token) {
    const inner = this.parser.parseInline(token.tokens)
    const align = token.align ? ` style="text-align:${token.align}"` : ""
    if (token.header) {
      return `<th class="whitespace-nowrap px-4 py-2 text-left font-semibold text-sm" data-streamdown="table-header-cell"${align}>${inner}</th>`
    }
    return `<td class="px-4 py-2 text-sm" data-streamdown="table-cell"${align}>${inner}</td>`
  }

  return r
}

const renderer = createRenderer()

const parseMarkdownIntoBlocks = (markdown: string): string[] => {
  const tokens = Lexer.lex(markdown, { gfm: true }) as Array<{ readonly raw?: string }>
  return tokens
    .map((t) => (typeof t?.raw === "string" ? t.raw : ""))
    .filter((raw) => raw.trim() !== "")
}

const renderMarkdownBlockToHtml = (markdown: string): string => {
  // `marked.parse` returns string when async=false (default).
  return marked.parse(markdown, {
    gfm: true,
    breaks: false,
    renderer,
  }) as string
}

/**
 * Streamdown-like Markdown renderer for Effuse:
 * - Splits Markdown into blocks so incomplete trailing Markdown doesn't break earlier blocks.
 * - Optionally applies `remend` during streaming to improve unterminated block parsing.
 * - Adds a caret during streaming (optional).
 */
export const streamdown = (markdown: string, options?: EffuseStreamdownOptions): TemplateResult => {
  const mode = options?.mode ?? "streaming"
  const isAnimating = options?.isAnimating ?? false
  const caret = options?.caret
  const parseIncompleteMarkdown = options?.parseIncompleteMarkdown ?? true

  const processed =
    mode === "streaming" && parseIncompleteMarkdown ? remend(markdown ?? "") : (markdown ?? "")

  const blocks = parseMarkdownIntoBlocks(processed)

  const caretChar = caret ? CARETS[caret] : ""
  const caretEnabled = mode === "streaming" && isAnimating && Boolean(caretChar)

  const wrapperClass = [
    "space-y-4 whitespace-normal *:first:mt-0 *:last:mb-0",
    caretEnabled
      ? "*:last:after:inline *:last:after:align-baseline *:last:after:content-[var(--streamdown-caret)]"
      : null,
  ]
    .filter(Boolean)
    .join(" ")

  const style = caretEnabled ? `--streamdown-caret: "${caretChar}";` : ""

  const blockEls =
    blocks.length === 0 && caretEnabled
      ? [html`<span></span>`]
      : blocks.map((block) => rawHtml(renderMarkdownBlockToHtml(block)))

  return html`<div class="${wrapperClass}" style="${style}" data-effuse-streamdown="1">${blockEls}</div>`
}
