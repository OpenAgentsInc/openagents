export type MarkdownBlock =
  | Readonly<{ kind: "heading"; level: 1 | 2 | 3; text: string }>
  | Readonly<{ kind: "paragraph"; text: string }>
  | Readonly<{ kind: "unordered-list"; items: ReadonlyArray<string> }>
  | Readonly<{ kind: "ordered-list"; items: ReadonlyArray<string> }>
  | Readonly<{ kind: "blockquote"; text: string }>
  | Readonly<{ kind: "code"; language: string | undefined; code: string }>
  | Readonly<{ kind: "rule" }>

export type MarkdownInlinePart =
  | Readonly<{ kind: "text"; text: string }>
  | Readonly<{ kind: "code"; text: string }>
  | Readonly<{ kind: "link"; href: string; children: ReadonlyArray<MarkdownInlinePart> }>
  | Readonly<{ kind: "strong"; children: ReadonlyArray<MarkdownInlinePart> }>
  | Readonly<{ kind: "emphasis"; children: ReadonlyArray<MarkdownInlinePart> }>

export const isSafeMarkdownHref = (href: string): boolean => {
  const trimmed = href.trim()
  if (trimmed === "") return false
  return (
    /^https?:\/\//i.test(trimmed) ||
    /^mailto:/i.test(trimmed) ||
    trimmed.startsWith("/") ||
    trimmed.startsWith("#")
  )
}

export const parseMarkdownInline = (text: string): ReadonlyArray<MarkdownInlinePart> => {
  const nodes: MarkdownInlinePart[] = []
  let buffer = ""
  let index = 0

  const flush = (): void => {
    if (buffer.length === 0) return
    nodes.push({ kind: "text", text: buffer })
    buffer = ""
  }

  while (index < text.length) {
    const rest = text.slice(index)

    if (rest.startsWith("`")) {
      const end = rest.indexOf("`", 1)
      if (end > 0) {
        flush()
        nodes.push({ kind: "code", text: rest.slice(1, end) })
        index += end + 1
        continue
      }
      buffer += "`"
      index += 1
      continue
    }

    if (rest.startsWith("[")) {
      const labelEnd = rest.indexOf("]")
      if (labelEnd > 0 && rest[labelEnd + 1] === "(") {
        const hrefEnd = rest.indexOf(")", labelEnd + 2)
        if (hrefEnd > labelEnd + 1) {
          const label = rest.slice(1, labelEnd)
          const href = rest.slice(labelEnd + 2, hrefEnd).trim()
          flush()
          const children = parseMarkdownInline(label)
          if (isSafeMarkdownHref(href)) {
            nodes.push({ kind: "link", href, children })
          } else {
            nodes.push(...children)
          }
          index += hrefEnd + 1
          continue
        }
      }
      buffer += "["
      index += 1
      continue
    }

    const boldMarker = rest.startsWith("**") ? "**" : rest.startsWith("__") ? "__" : ""
    if (boldMarker.length > 0) {
      const end = rest.indexOf(boldMarker, boldMarker.length)
      if (end > 0) {
        flush()
        nodes.push({
          children: parseMarkdownInline(rest.slice(boldMarker.length, end)),
          kind: "strong",
        })
        index += end + boldMarker.length
        continue
      }
      buffer += boldMarker
      index += boldMarker.length
      continue
    }

    const italicMarker = rest.startsWith("*") ? "*" : rest.startsWith("_") ? "_" : ""
    if (italicMarker.length > 0) {
      const end = rest.indexOf(italicMarker, 1)
      if (end > 0) {
        flush()
        nodes.push({
          children: parseMarkdownInline(rest.slice(1, end)),
          kind: "emphasis",
        })
        index += end + 1
        continue
      }
      buffer += italicMarker
      index += 1
      continue
    }

    buffer += text[index] ?? ""
    index += 1
  }

  flush()
  return nodes
}

const headingMatch = (line: string): { level: 1 | 2 | 3; text: string } | undefined => {
  const match = /^(#{1,6})\s+(.*)$/.exec(line)
  if (match === null) {
    return undefined
  }
  const hashes = match[1] ?? ""
  const level = (hashes.length >= 3 ? 3 : hashes.length) as 1 | 2 | 3
  return { level, text: (match[2] ?? "").trim() }
}

const unorderedItemMatch = (line: string): string | undefined => {
  const match = /^\s*[-*+]\s+(.*)$/.exec(line)
  return match === null ? undefined : (match[1] ?? "")
}

const orderedItemMatch = (line: string): string | undefined => {
  const match = /^\s*\d+[.)]\s+(.*)$/.exec(line)
  return match === null ? undefined : (match[1] ?? "")
}

const isRule = (line: string): boolean => /^\s*([-*_])(\s*\1){2,}\s*$/.test(line)

const isBlank = (line: string): boolean => line.trim() === ""

// Streaming-tolerant block parser for the common assistant Markdown subset.
export const parseMarkdownBlocks = (text: string): ReadonlyArray<MarkdownBlock> => {
  const lines = text.replace(/\r\n/g, "\n").split("\n")
  const blocks: MarkdownBlock[] = []
  let lineIndex = 0

  while (lineIndex < lines.length) {
    const line = lines[lineIndex] ?? ""

    if (isBlank(line)) {
      lineIndex += 1
      continue
    }

    const fenceMatch = /^\s*```(.*)$/.exec(line)
    if (fenceMatch !== null) {
      const language = (fenceMatch[1] ?? "").trim()
      const codeLines: string[] = []
      lineIndex += 1
      while (lineIndex < lines.length) {
        const codeLine = lines[lineIndex] ?? ""
        if (/^\s*```\s*$/.test(codeLine)) {
          lineIndex += 1
          break
        }
        codeLines.push(codeLine)
        lineIndex += 1
      }
      blocks.push({
        code: codeLines.join("\n"),
        kind: "code",
        language: language === "" ? undefined : language,
      })
      continue
    }

    if (/^ {4}\S/.test(line)) {
      const codeLines: string[] = []
      while (lineIndex < lines.length && /^ {4}/.test(lines[lineIndex] ?? "")) {
        codeLines.push((lines[lineIndex] ?? "").slice(4))
        lineIndex += 1
      }
      blocks.push({ code: codeLines.join("\n"), kind: "code", language: undefined })
      continue
    }

    if (isRule(line)) {
      blocks.push({ kind: "rule" })
      lineIndex += 1
      continue
    }

    const heading = headingMatch(line)
    if (heading !== undefined) {
      blocks.push({ kind: "heading", level: heading.level, text: heading.text })
      lineIndex += 1
      continue
    }

    if (unorderedItemMatch(line) !== undefined) {
      const items: string[] = []
      while (lineIndex < lines.length) {
        const item = unorderedItemMatch(lines[lineIndex] ?? "")
        if (item === undefined) break
        items.push(item)
        lineIndex += 1
      }
      blocks.push({ items, kind: "unordered-list" })
      continue
    }

    if (orderedItemMatch(line) !== undefined) {
      const items: string[] = []
      while (lineIndex < lines.length) {
        const item = orderedItemMatch(lines[lineIndex] ?? "")
        if (item === undefined) break
        items.push(item)
        lineIndex += 1
      }
      blocks.push({ items, kind: "ordered-list" })
      continue
    }

    if (/^\s*>\s?/.test(line)) {
      const quoteLines: string[] = []
      while (lineIndex < lines.length && /^\s*>\s?/.test(lines[lineIndex] ?? "")) {
        quoteLines.push((lines[lineIndex] ?? "").replace(/^\s*>\s?/, ""))
        lineIndex += 1
      }
      blocks.push({ kind: "blockquote", text: quoteLines.join("\n") })
      continue
    }

    const paragraphLines: string[] = []
    while (lineIndex < lines.length) {
      const candidate = lines[lineIndex] ?? ""
      if (
        isBlank(candidate) ||
        /^\s*```/.test(candidate) ||
        headingMatch(candidate) !== undefined ||
        unorderedItemMatch(candidate) !== undefined ||
        orderedItemMatch(candidate) !== undefined ||
        /^\s*>\s?/.test(candidate) ||
        isRule(candidate)
      ) {
        break
      }
      paragraphLines.push(candidate.trim())
      lineIndex += 1
    }
    if (paragraphLines.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraphLines.join(" ") })
    }
  }

  return blocks
}
