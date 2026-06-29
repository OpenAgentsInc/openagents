import {
  type CodeTokenKind,
  type DiffRow,
  parseUnifiedDiff,
  tokenizeCode,
  tokenizeCodeLines,
} from "@openagentsinc/ui/ai-elements/code-highlight"

const EXT_LANGUAGE: Readonly<Record<string, string>> = {
  bash: "bash",
  cjs: "javascript",
  cts: "typescript",
  go: "go",
  js: "javascript",
  json: "json",
  jsonc: "json",
  jsx: "javascript",
  mjs: "javascript",
  mts: "typescript",
  py: "python",
  rs: "rust",
  sh: "bash",
  ts: "typescript",
  tsx: "typescript",
  zsh: "bash",
}

export type MessageSegment =
  | { readonly kind: "prose"; readonly text: string }
  | { readonly kind: "code"; readonly text: string; readonly language?: string }
  | { readonly kind: "diff"; readonly text: string }

const languageForFilename = (filename: string | undefined): string | undefined => {
  if (filename === undefined) return undefined
  const ext = filename.split(".").pop()?.toLowerCase()
  return ext === undefined ? undefined : EXT_LANGUAGE[ext]
}

const tokenSpan = (kind: CodeTokenKind, text: string): HTMLElement => {
  const span = document.createElement("span")
  span.className = `cb-tok cb-tok--${kind}`
  span.textContent = text
  return span
}

const tokenizeInto = (
  parent: HTMLElement,
  text: string,
  language: string | undefined,
): void => {
  for (const token of tokenizeCode(text, language)) {
    parent.append(tokenSpan(token.kind, token.text))
  }
}

export const codeBlockElement = (input: {
  readonly code: string
  readonly language?: string
  readonly filename?: string
}): HTMLElement => {
  const root = document.createElement("div")
  root.className = "cb"

  if (input.filename !== undefined || input.language !== undefined) {
    const header = document.createElement("div")
    header.className = "cb-header"
    if (input.filename !== undefined) {
      const file = document.createElement("span")
      file.className = "cb-file"
      file.textContent = input.filename
      header.append(file)
    }
    if (input.language !== undefined) {
      const lang = document.createElement("span")
      lang.className = "cb-lang"
      lang.textContent = input.language
      header.append(lang)
    }
    root.append(header)
  }

  const pre = document.createElement("pre")
  pre.className = "cb-body"
  const code = document.createElement("code")
  code.className = "cb-code-grid"

  for (const lineTokens of tokenizeCodeLines(input.code, input.language)) {
    const line = document.createElement("span")
    line.className = "cb-line"
    for (const token of lineTokens) line.append(tokenSpan(token.kind, token.text))
    code.append(line)
  }

  pre.append(code)
  root.append(pre)
  return root
}

const diffGutter = (
  kind: DiffRow["kind"],
  value: number | undefined,
): HTMLElement => {
  const span = document.createElement("span")
  span.className = `cb-diff-gutter cb-diff-gutter--${kind}`
  span.setAttribute("aria-hidden", "true")
  span.textContent = value === undefined ? "" : String(value)
  return span
}

export const diffElement = (input: {
  readonly patch: string
  readonly language?: string
  readonly filename?: string
}): HTMLElement => {
  const parsed = parseUnifiedDiff(input.patch, input.filename)
  const language = input.language ?? languageForFilename(parsed.filename)

  const root = document.createElement("div")
  root.className = "cb cb-diff"

  const header = document.createElement("div")
  header.className = "cb-header cb-diff-header"
  const file = document.createElement("span")
  file.className = "cb-file"
  file.textContent = parsed.filename ?? "diff"
  const stats = document.createElement("span")
  stats.className = "cb-diff-stats"
  const add = document.createElement("span")
  add.className = "cb-stat-add"
  add.textContent = `+${parsed.added}`
  const rem = document.createElement("span")
  rem.className = "cb-stat-rem"
  rem.textContent = `-${parsed.removed}`
  stats.append(add, rem)
  header.append(file, stats)
  root.append(header)

  const pre = document.createElement("pre")
  pre.className = "cb-body cb-diff-body"
  const code = document.createElement("code")
  code.className = "cb-code-grid"

  for (const row of parsed.rows) {
    if (row.kind === "hunk") {
      const hunk = document.createElement("span")
      hunk.className = "cb-diff-hunk"
      hunk.textContent = row.text
      code.append(hunk)
      continue
    }

    const line = document.createElement("span")
    line.className = "cb-diff-line"
    line.dataset.kind = row.kind
    line.append(diffGutter(row.kind, row.oldNo))
    line.append(diffGutter(row.kind, row.newNo))

    const sign = document.createElement("span")
    sign.className = "cb-diff-sign"
    sign.setAttribute("aria-hidden", "true")
    sign.textContent = row.kind === "add" ? "+" : row.kind === "remove" ? "-" : " "
    line.append(sign)

    const content = document.createElement("span")
    content.className = "cb-diff-code"
    tokenizeInto(content, row.text, language)
    line.append(content)
    code.append(line)
  }

  pre.append(code)
  root.append(pre)
  return root
}

export const looksLikeUnifiedDiff = (text: string): boolean => {
  const first = text.split("\n").find(line => line.trim().length > 0) ?? ""
  return (
    first.startsWith("diff --git") ||
    first.startsWith("--- ") ||
    first.startsWith("*** ") ||
    /^@@ -\d/.test(first)
  )
}

export const parseMessageSegments = (text: string): readonly MessageSegment[] => {
  if (looksLikeUnifiedDiff(text)) {
    return [{ kind: "diff", text }]
  }

  const segments: MessageSegment[] = []
  const fence = /```([^\n`]*)\n([\s\S]*?)```/g
  let lastIndex = 0
  let match: RegExpExecArray | null

  while ((match = fence.exec(text)) !== null) {
    if (match.index > lastIndex) {
      const prose = text.slice(lastIndex, match.index).trim()
      if (prose.length > 0) segments.push({ kind: "prose", text: prose })
    }
    const info = (match[1] ?? "").trim().toLowerCase()
    const body = match[2] ?? ""
    if (info === "diff" || info === "patch" || looksLikeUnifiedDiff(body)) {
      segments.push({ kind: "diff", text: body })
    } else {
      segments.push({
        kind: "code",
        text: body.replace(/\n$/, ""),
        ...(info === "" ? {} : { language: info }),
      })
    }
    lastIndex = fence.lastIndex
  }

  if (lastIndex < text.length) {
    const tail = text.slice(lastIndex).trim()
    if (tail.length > 0) segments.push({ kind: "prose", text: tail })
  }

  if (segments.length === 0) segments.push({ kind: "prose", text })

  return segments
}

export const renderMessageBody = (text: string): readonly HTMLElement[] =>
  parseMessageSegments(text).map(segment => {
    if (segment.kind === "diff") return diffElement({ patch: segment.text })
    if (segment.kind === "code") {
      return codeBlockElement({
        code: segment.text,
        ...(segment.language === undefined ? {} : { language: segment.language }),
      })
    }
    const p = document.createElement("p")
    p.className = "message-prose"
    p.textContent = segment.text
    return p
  })
