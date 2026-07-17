import {
  CodeBlock,
  CopyButton,
  Markdown,
  Stack,
  type MarkdownBlock,
  type MarkdownInline,
  type View,
} from "@effect-native/core"

const MAX_SOURCE_LENGTH = 20_000
const MAX_BLOCKS = 128
const MAX_CODE_LINES = 400
const MAX_LINE_LENGTH = 1_000

const boundedSource = (source: string): string =>
  source.length <= MAX_SOURCE_LENGTH ? source : `${source.slice(0, MAX_SOURCE_LENGTH)}…`

const safeMarkdownHref = (candidate: string): string | null => {
  const value = candidate.trim()
  if (value.startsWith("/") || value.startsWith("#")) return value
  try {
    const url = new URL(value)
    return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : null
  } catch {
    return null
  }
}

const inlineToken = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/g

export const parseMobileMarkdownInline = (source: string): ReadonlyArray<MarkdownInline> => {
  const result: Array<MarkdownInline> = []
  let cursor = 0
  for (const match of source.matchAll(inlineToken)) {
    const index = match.index ?? cursor
    if (index > cursor) result.push({ kind: "text", text: source.slice(cursor, index) })
    const token = match[0]
    if (token.startsWith("`")) {
      result.push({ kind: "code", text: token.slice(1, -1) })
    } else if (token.startsWith("**")) {
      result.push({ kind: "strong", children: [{ kind: "text", text: token.slice(2, -2) }] })
    } else if (token.startsWith("*")) {
      result.push({ kind: "emphasis", children: [{ kind: "text", text: token.slice(1, -1) }] })
    } else {
      const labelEnd = token.indexOf("](")
      const label = token.slice(1, labelEnd)
      const href = safeMarkdownHref(token.slice(labelEnd + 2, -1))
      result.push(href === null
        ? { kind: "text", text: label }
        : { kind: "link", href, children: [{ kind: "text", text: label }] })
    }
    cursor = index + token.length
  }
  if (cursor < source.length) result.push({ kind: "text", text: source.slice(cursor) })
  return result.length === 0 ? [{ kind: "text", text: source }] : result
}

const markdownBlocks = (source: string): ReadonlyArray<MarkdownBlock> => {
  const lines = source.split("\n")
  const blocks: Array<MarkdownBlock> = []
  let index = 0
  while (index < lines.length && blocks.length < MAX_BLOCKS) {
    const line = lines[index]?.slice(0, MAX_LINE_LENGTH) ?? ""
    if (line.trim() === "") {
      index += 1
      continue
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line)
    if (heading !== null) {
      const marker = heading[1] ?? "#"
      const content = heading[2] ?? ""
      blocks.push({
        kind: "heading",
        level: marker.length as 1 | 2 | 3 | 4 | 5 | 6,
        children: parseMobileMarkdownInline(content),
      })
      index += 1
      continue
    }
    const quoteLines: Array<string> = []
    while (index < lines.length && /^>\s?/.test(lines[index] ?? "")) {
      quoteLines.push((lines[index] ?? "").replace(/^>\s?/, "").slice(0, MAX_LINE_LENGTH))
      index += 1
    }
    if (quoteLines.length > 0) {
      blocks.push({ kind: "blockquote", children: markdownBlocks(quoteLines.join("\n")) })
      continue
    }
    const listMatch = /^(\s*)([-*+] |\d+\. )(.+)$/.exec(line)
    if (listMatch !== null) {
      const ordered = /\d+\. /.test(listMatch[2] ?? "")
      const items: Array<ReadonlyArray<MarkdownBlock>> = []
      while (index < lines.length) {
        const item = /^(\s*)([-*+] |\d+\. )(.+)$/.exec(lines[index] ?? "")
        if (item === null || /\d+\. /.test(item[2] ?? "") !== ordered) break
        items.push([{
          kind: "paragraph",
          children: parseMobileMarkdownInline((item[3] ?? "").slice(0, MAX_LINE_LENGTH)),
        }])
        index += 1
      }
      blocks.push({ kind: "list", ordered, items })
      continue
    }
    const paragraph: Array<string> = [line]
    index += 1
    while (
      index < lines.length &&
      (lines[index] ?? "").trim() !== "" &&
      !/^(#{1,6})\s+/.test(lines[index] ?? "") &&
      !/^>\s?/.test(lines[index] ?? "") &&
      !/^(\s*)([-*+] |\d+\. )/.test(lines[index] ?? "")
    ) {
      paragraph.push((lines[index] ?? "").slice(0, MAX_LINE_LENGTH))
      index += 1
    }
    blocks.push({ kind: "paragraph", children: parseMobileMarkdownInline(paragraph.join("\n")) })
  }
  return blocks
}

type TranscriptSegment =
  | Readonly<{ kind: "markdown"; source: string }>
  | Readonly<{ kind: "code"; language?: string; source: string }>

const transcriptSegments = (source: string): ReadonlyArray<TranscriptSegment> => {
  const lines = boundedSource(source).split("\n")
  const segments: Array<TranscriptSegment> = []
  let markdown: Array<string> = []
  let code: Array<string> | null = null
  let language: string | undefined
  const flushMarkdown = () => {
    if (markdown.length > 0) segments.push({ kind: "markdown", source: markdown.join("\n") })
    markdown = []
  }
  const flushCode = () => {
    if (code === null) return
    segments.push({
      kind: "code",
      ...(language === undefined || language === "" ? {} : { language }),
      source: code.slice(0, MAX_CODE_LINES).map(line => line.slice(0, MAX_LINE_LENGTH)).join("\n"),
    })
    code = null
    language = undefined
  }
  for (const line of lines) {
    const fence = /^```([^\s`]*)\s*$/.exec(line)
    if (fence !== null) {
      if (code === null) {
        flushMarkdown()
        code = []
        language = fence[1]
      } else {
        flushCode()
      }
      continue
    }
    if (code === null) markdown.push(line)
    else code.push(line)
  }
  if (code !== null) flushCode()
  flushMarkdown()
  return segments.slice(0, MAX_BLOCKS)
}

export const mobileRichContentViews = (
  key: string,
  source: string,
  copyAccessibilityLabel: string,
): ReadonlyArray<View> => {
  const bounded = boundedSource(source)
  const content = transcriptSegments(bounded).flatMap((segment, index): ReadonlyArray<View> => {
    if (segment.kind === "markdown") {
      const blocks = markdownBlocks(segment.source)
      return blocks.length === 0 ? [] : [Markdown({
        key: `${key}-markdown-${index}`,
        blocks,
        style: { width: "full" },
      })]
    }
    const lines = segment.source.split("\n").map(line => ({
      tokens: [{ kind: "plain" as const, text: line }],
    }))
    return [Stack({
      key: `${key}-code-${index}`,
      direction: "column",
      gap: "1",
      style: { width: "full" },
    }, [
      CodeBlock({
        key: `${key}-code-block-${index}`,
        ...(segment.language === undefined ? {} : { language: segment.language }),
        lines,
        showLineNumbers: lines.length > 1,
        style: { width: "full", borderRadius: "md", padding: "2" },
      }),
      CopyButton({
        key: `${key}-copy-code-${index}`,
        content: segment.source,
        label: "Copy code",
        accessibilityLabel: `Copy ${segment.language ?? "code"} block`,
        size: "sm",
        variant: "ghost",
      }),
    ])]
  })
  return [
    ...content,
    CopyButton({
      key: `${key}-copy-message`,
      content: bounded,
      accessibilityLabel: copyAccessibilityLabel,
      size: "sm",
      variant: "ghost",
    }),
  ]
}

export const mobileAssistantContentViews = (
  key: string,
  source: string,
): ReadonlyArray<View> => mobileRichContentViews(key, source, "Copy assistant message")
