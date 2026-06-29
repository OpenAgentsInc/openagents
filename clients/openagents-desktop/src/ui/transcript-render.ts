import {
  type CodeTokenKind,
  type DiffRow,
  parseUnifiedDiff,
  tokenizeCode,
  tokenizeCodeLines,
} from "@openagentsinc/ui/ai-elements/code-highlight"

// Shared code/diff rendering for the desktop Codex transcript.
//
// The web `@openagentsinc/ui` ai-elements (code block + diff) are Foldkit views;
// this app is vanilla DOM. To keep ONE visual language across web and desktop we
// reuse the pure tokenizer + unified-diff parser from `@openagentsinc/ui` and
// render the same structure here as DOM, styled to match (see styles.css
// `.cb`/`.cb-diff` — the Protoss-glow code surface, cool-blue tokens, and
// green/red change tints). Token text is set via textContent, never innerHTML.

const EXT_LANGUAGE: Readonly<Record<string, string>> = {
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  py: "python",
  rs: "rust",
  go: "go",
  json: "json",
  jsonc: "json",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
}

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

// A framed, syntax-highlighted code block (mirrors ai-elements code-block).
export const codeBlockElement = (input: {
  code: string
  language?: string
  filename?: string
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
    if (lineTokens.length === 0) {
      // keep blank-line height
      line.append(document.createTextNode(""))
    } else {
      for (const token of lineTokens) line.append(tokenSpan(token.kind, token.text))
    }
    code.append(line)
  }

  pre.append(code)
  root.append(pre)
  return root
}

const diffGutter = (kind: DiffRow["kind"], value: number | undefined): HTMLElement => {
  const span = document.createElement("span")
  span.className = `cb-diff-gutter cb-diff-gutter--${kind}`
  span.setAttribute("aria-hidden", "true")
  span.textContent = value === undefined ? "" : String(value)
  return span
}

// A framed unified diff (mirrors ai-elements diff): old|new gutter, +/- signs,
// green/red line tints, per-line syntax highlighting.
export const diffElement = (input: {
  patch: string
  language?: string
  filename?: string
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
  rem.textContent = `−${parsed.removed}`
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

type Segment =
  | { kind: "prose"; text: string }
  | { kind: "code"; text: string; language?: string }
  | { kind: "diff"; text: string }

const looksLikeUnifiedDiff = (text: string): boolean => {
  const first = text.split("\n").find((line) => line.trim().length > 0) ?? ""
  return (
    first.startsWith("diff --git") ||
    first.startsWith("--- ") ||
    first.startsWith("*** ") ||
    /^@@ -\d/.test(first)
  )
}

// Split a message body into prose / fenced-code / diff segments. Triple-backtick
// fences carry a language; a `diff`/`patch` fence (or diff-shaped content) is a
// diff. A whole message that is itself a unified diff is rendered as one diff.
export const parseMessageSegments = (text: string): readonly Segment[] => {
  if (looksLikeUnifiedDiff(text)) {
    return [{ kind: "diff", text }]
  }

  const segments: Segment[] = []
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

  if (segments.length === 0) {
    segments.push({ kind: "prose", text })
  }

  return segments
}

// Render a transcript message body into the shared code language. Returns the
// list of block elements to append (prose paragraphs, code blocks, diffs).
export const renderMessageBody = (input: {
  text: string
  role: string
  status: string
}): readonly HTMLElement[] => {
  // Tool stdout / errors are not prose: render as a plain mono output block
  // unless they are clearly a diff.
  if (
    (input.role === "tool" || input.status === "error") &&
    !looksLikeUnifiedDiff(input.text)
  ) {
    const pre = document.createElement("pre")
    pre.className = "coding-message-output"
    pre.dataset.status = input.status
    pre.textContent = input.text
    return [pre]
  }

  return parseMessageSegments(input.text).map((segment) => {
    if (segment.kind === "diff") {
      return diffElement({ patch: segment.text })
    }
    if (segment.kind === "code") {
      return codeBlockElement({
        code: segment.text,
        ...(segment.language === undefined ? {} : { language: segment.language }),
      })
    }
    const p = document.createElement("p")
    p.className = "coding-message-prose"
    p.textContent = segment.text
    return p
  })
}
