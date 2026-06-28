const reset = "\x1b[0m"

const ansi = {
  bold: "\x1b[1m",
  code: "\x1b[38;5;114m",
  heading: "\x1b[38;5;140m\x1b[1m",
  link: "\x1b[38;5;216m",
  linkText: "\x1b[38;5;80m",
  list: "\x1b[38;5;216m",
  meta: "\x1b[90m",
  quote: "\x1b[38;5;180m",
  reasoning: "\x1b[38;5;244m",
  strong: "\x1b[38;5;215m\x1b[1m",
  user: "\x1b[38;2;127;220;155m\x1b[1m",
  assistant: "\x1b[38;2;58;123;255m\x1b[1m",
} as const

export function colorEnabled(): boolean {
  return process.stdout.isTTY && process.env.NO_COLOR !== "1" && process.env.TERM !== "dumb"
}

export function colorize(style: keyof typeof ansi, text: string): string {
  if (!colorEnabled() || text.length === 0) return text
  return `${ansi[style]}${text}${reset}`
}

export const terminalStyle = {
  assistant: (text: string): string => colorize("assistant", text),
  code: (text: string): string => colorize("code", text),
  heading: (text: string): string => colorize("heading", text),
  list: (text: string): string => colorize("list", text),
  meta: (text: string): string => colorize("meta", text),
  quote: (text: string): string => colorize("quote", text),
  reasoning: (text: string): string => colorize("reasoning", text),
  strong: (text: string): string => colorize("strong", text),
  user: (text: string): string => colorize("user", text),
}

export function renderMarkdownForTerminal(markdown: string): string {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n")
  const rendered: Array<string> = []
  let inFence = false
  let previousBlankOutsideFence = false

  for (const line of lines) {
    const fence = line.match(/^\s*```/)
    if (fence !== null) {
      inFence = !inFence
      rendered.push(terminalStyle.meta(line))
      previousBlankOutsideFence = false
      continue
    }

    if (inFence) {
      rendered.push(terminalStyle.code(line))
      continue
    }

    if (line.trim() === "") {
      if (!previousBlankOutsideFence) {
        rendered.push("")
      }
      previousBlankOutsideFence = true
      continue
    }
    previousBlankOutsideFence = false

    const heading = line.match(/^\s{0,3}(#{1,6})\s+(.+?)\s*#*\s*$/)
    if (heading !== null) {
      rendered.push(terminalStyle.heading(heading[2] ?? ""))
      continue
    }

    const unordered = line.match(/^(\s*)[-*+]\s+(.+)$/)
    if (unordered !== null) {
      rendered.push(`${unordered[1] ?? ""}${terminalStyle.list("-")} ${renderInlineMarkdown(unordered[2] ?? "")}`)
      continue
    }

    const ordered = line.match(/^(\s*)(\d+[.)])\s+(.+)$/)
    if (ordered !== null) {
      rendered.push(`${ordered[1] ?? ""}${terminalStyle.list(ordered[2] ?? "")} ${renderInlineMarkdown(ordered[3] ?? "")}`)
      continue
    }

    const quote = line.match(/^\s{0,3}>\s?(.*)$/)
    if (quote !== null) {
      rendered.push(`${terminalStyle.quote("|")} ${renderInlineMarkdown(quote[1] ?? "")}`)
      continue
    }

    const rule = line.match(/^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/)
    if (rule !== null) {
      rendered.push(terminalStyle.meta("-----"))
      continue
    }

    rendered.push(renderInlineMarkdown(line))
  }

  return rendered.join("\n")
}

export function renderMarkdownDeltaForTerminal(markdownDelta: string): string {
  return renderInlineMarkdown(markdownDelta)
}

export function renderReasoningMarkdownDeltaForTerminal(markdownDelta: string): string {
  return terminalStyle.reasoning(renderInlineMarkdown(markdownDelta))
}

function renderInlineMarkdown(input: string): string {
  let output = input
  output = output.replace(/`([^`]+)`/g, (_, code: string) => terminalStyle.code(code))
  output = output.replace(/\*\*([^*]+)\*\*/g, (_, text: string) => terminalStyle.strong(text))
  output = output.replace(/__([^_]+)__/g, (_, text: string) => terminalStyle.strong(text))
  output = output.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, text: string, url: string) =>
    `${colorize("linkText", text)} ${colorize("link", `(${url})`)}`,
  )
  output = output.replace(/(^|[^\*])\*([^*\n]+)\*(?!\*)/g, (_, prefix: string, text: string) =>
    `${prefix}${terminalStyle.quote(text)}`,
  )
  return output
}
