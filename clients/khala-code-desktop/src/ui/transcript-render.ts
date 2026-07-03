import {
  type CodeTokenKind,
  type DiffRow,
  parseUnifiedDiff,
  tokenizeCode,
  tokenizeCodeLines,
} from "@openagentsinc/ui/ai-elements/code-highlight"
import { Schema as S } from "effect"
import {
  parseMarkdownBlocks,
  parseMarkdownInline,
  type MarkdownInlinePart,
} from "@openagentsinc/ui/ai-elements/markdown"
import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import type {
  KhalaCodeDesktopCodexItemCard,
  KhalaCodeDesktopMessageRole,
} from "../shared/rpc"
import type { KhalaCodeDesktopCodexApprovalAction } from "../shared/codex-approval-decisions"
import {
  KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
  KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA,
  KhalaCodeJudgeDiffVerdictSchema,
  khalaCodeJudgeDiffFindingToReviewDetail,
  khalaCodeDiffReviewLineLabel,
  type KhalaCodeDiffReviewLineKind,
  type KhalaCodeDiffReviewLineSide,
  type KhalaCodeDiffReviewSubmitDetail,
  type KhalaCodeJudgeDiffVerdict,
  type KhalaCodeJudgeDiffVerdictFinding,
} from "../shared/diff-review"
import {
  KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT,
  khalaCodeSourceControlActionLabel,
  type KhalaCodeSourceControlActionKind,
  type KhalaCodeSourceControlActionSubmitDetail,
} from "../shared/source-control-action"
import { displayLocalPathsForKhalaCode } from "../shared/display-paths"

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
  | { readonly kind: "judge-diff-verdict"; readonly verdict: KhalaCodeJudgeDiffVerdict }

export type ToolTranscriptParts = {
  readonly output: string
  readonly status: string
  readonly toolName: string
}

type MarkdownBlock = ReturnType<typeof parseMarkdownBlocks>[number]

const MAX_COMPACT_SUMMARY_CHARS = 160

const compactLine = (text: string): string =>
  text.replace(/`{3,}/g, "").replace(/\s+/g, " ").trim()

const compactSummaryLineSkipped = (line: string): boolean => {
  const normalized = line.trim()
  if (normalized.length === 0) return true
  if (/^```/u.test(normalized)) return true
  if (/^[{}[\],]$/u.test(normalized)) return true
  return /^(cwd:|Output|Arguments|Result|Error|Content|Network|Permissions|Available decisions|Agent states|Review|JSON|BASH)$/iu
    .test(normalized)
}

const truncateCompactSummary = (summary: string, fallback: string): string => {
  const compact = compactLine(summary)
  const value = compact.length === 0 ? compactLine(fallback) : compact
  if (value.length <= MAX_COMPACT_SUMMARY_CHARS) return value
  return `${value.slice(0, MAX_COMPACT_SUMMARY_CHARS - 3)}...`
}

export const compactToolSummary = (text: string, fallback = "Details available"): string => {
  const normalized = displayLocalPathsForKhalaCode(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n")
  const command = /```(?:bash|sh|zsh)\n([\s\S]*?)\n```/u.exec(normalized)
  if (command?.[1] !== undefined) return truncateCompactSummary(command[1], fallback)

  for (const rawLine of normalized.split("\n")) {
    const line = rawLine.trim().replace(/^#{1,6}\s+/u, "")
    if (compactSummaryLineSkipped(line)) continue
    return truncateCompactSummary(line, fallback)
  }

  return truncateCompactSummary("", fallback)
}

const setToolCardExpanded = (
  root: HTMLElement,
  header: HTMLElement,
  expanded: boolean,
): void => {
  root.dataset.expanded = expanded ? "true" : "false"
  header.setAttribute("aria-expanded", expanded ? "true" : "false")
}

const bindExpandableToolCard = (
  root: HTMLElement,
  header: HTMLElement,
  title = "Click to expand",
): void => {
  header.classList.add("tool-card-header--toggle")
  header.setAttribute("role", "button")
  header.setAttribute("tabindex", "0")
  header.setAttribute("aria-expanded", "false")
  header.title = title

  const toggle = (): void => {
    setToolCardExpanded(root, header, root.dataset.expanded !== "true")
  }

  header.addEventListener("click", toggle)
  header.addEventListener("keydown", event => {
    if (event.key !== "Enter" && event.key !== " ") return
    event.preventDefault()
    toggle()
  })
}

const iconForCodexItem = (itemType: string): IconName => {
  switch (itemType) {
    case "commandExecution":
      return "Terminal"
    case "mcpToolCall":
    case "dynamicToolCall":
    case "collabAgentToolCall":
      return "Tools"
    case "fileChange":
      return "FileCode"
    case "webSearch":
      return "Globe"
    case "imageGeneration":
    case "imageView":
      return "ImageSquare"
    case "sleep":
      return "Sleep"
    case "approval":
    case "approvalReview":
      return "CheckCircle"
    default:
      return "Code"
  }
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

const inlineNodes = (parts: readonly MarkdownInlinePart[]): readonly Node[] => {
  const nodes: Node[] = []
  for (const part of parts) {
    switch (part.kind) {
      case "text":
        nodes.push(document.createTextNode(part.text))
        break
      case "code": {
        const code = document.createElement("code")
        code.className = "md-inline-code"
        code.textContent = part.text
        nodes.push(code)
        break
      }
      case "link": {
        const anchor = document.createElement("a")
        anchor.className = "md-link"
        anchor.href = part.href
        anchor.target = "_blank"
        anchor.rel = "noopener noreferrer"
        anchor.append(...inlineNodes(part.children))
        nodes.push(anchor)
        break
      }
      case "strong": {
        const strong = document.createElement("strong")
        strong.className = "md-strong"
        strong.append(...inlineNodes(part.children))
        nodes.push(strong)
        break
      }
      case "emphasis": {
        const em = document.createElement("em")
        em.className = "md-emphasis"
        em.append(...inlineNodes(part.children))
        nodes.push(em)
        break
      }
    }
  }
  return nodes
}

const appendInlineMarkdown = (element: HTMLElement, text: string): void => {
  element.append(...inlineNodes(parseMarkdownInline(text)))
}

const decodeJudgeDiffVerdict = (text: string): KhalaCodeJudgeDiffVerdict | null => {
  const trimmed = text.trim()
  if (!trimmed.includes(KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA)) return null
  try {
    return S.decodeUnknownSync(KhalaCodeJudgeDiffVerdictSchema)(JSON.parse(trimmed))
  } catch {
    return null
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

const diffReviewSide = (row: DiffRow): KhalaCodeDiffReviewLineSide =>
  row.kind === "remove" ? "old" : "new"

const diffReviewLineNo = (row: DiffRow): number | null => {
  if (row.kind === "remove") return row.oldNo ?? null
  return row.newNo ?? row.oldNo ?? null
}

const diffReviewKind = (row: DiffRow): KhalaCodeDiffReviewLineKind | null => {
  if (row.kind === "add" || row.kind === "context" || row.kind === "remove") {
    return row.kind
  }
  return null
}

const removeOpenDiffReviewEditors = (root: HTMLElement): void => {
  for (const editor of root.querySelectorAll(".cb-diff-review-editor")) {
    editor.remove()
  }
}

const openDiffReviewEditor = (
  root: HTMLElement,
  line: HTMLElement,
  detail: Omit<KhalaCodeDiffReviewSubmitDetail, "body">,
): void => {
  removeOpenDiffReviewEditors(root)

  const editor = document.createElement("span")
  editor.className = "cb-diff-review-editor"
  editor.dataset.patchRef = detail.patchRef

  const textarea = document.createElement("textarea")
  textarea.className = "cb-diff-review-textarea"
  textarea.name = "khala-diff-review-comment"
  textarea.rows = 3
  textarea.placeholder = "Comment for this line"
  textarea.setAttribute("aria-label", `Comment for ${khalaCodeDiffReviewLineLabel(detail)}`)

  const actions = document.createElement("span")
  actions.className = "cb-diff-review-actions"

  const submit = document.createElement("button")
  submit.type = "button"
  submit.className = "cb-diff-review-submit"
  submit.textContent = "Send"

  const cancel = document.createElement("button")
  cancel.type = "button"
  cancel.className = "cb-diff-review-cancel"
  cancel.textContent = "Cancel"

  submit.addEventListener("click", event => {
    event.preventDefault()
    event.stopPropagation()
    const body = textarea.value.trim()
    if (body.length === 0) {
      textarea.focus({ preventScroll: true })
      return
    }
    root.dispatchEvent(new CustomEvent<KhalaCodeDiffReviewSubmitDetail>(
      KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
      {
        bubbles: true,
        detail: {
          ...detail,
          body,
        },
      },
    ))
    editor.dataset.sent = "true"
    textarea.disabled = true
    submit.disabled = true
  })

  cancel.addEventListener("click", event => {
    event.preventDefault()
    event.stopPropagation()
    editor.remove()
  })

  textarea.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      event.preventDefault()
      editor.remove()
    }
  })

  actions.append(submit, cancel)
  editor.append(textarea, actions)
  line.after(editor)
  textarea.focus({ preventScroll: true })
}

const diffReviewButton = (
  root: HTMLElement,
  line: HTMLElement,
  detail: Omit<KhalaCodeDiffReviewSubmitDetail, "body">,
): HTMLButtonElement => {
  const button = document.createElement("button")
  button.type = "button"
  button.className = "cb-diff-comment-button"
  button.title = "Annotate diff line"
  button.setAttribute("aria-label", `Annotate ${khalaCodeDiffReviewLineLabel(detail)}`)
  button.dataset.patchRef = detail.patchRef
  button.dataset.filePath = detail.filePath
  button.dataset.lineKind = detail.lineKind
  button.dataset.lineNo = String(detail.lineNo)
  button.dataset.lineSide = detail.lineSide
  button.textContent = "Comment"
  button.addEventListener("click", event => {
    event.preventDefault()
    event.stopPropagation()
    openDiffReviewEditor(root, line, detail)
  })
  return button
}

const sourceControlActionButtons: ReadonlyArray<{
  readonly action: KhalaCodeSourceControlActionKind
  readonly icon: IconName
  readonly label: string
  readonly title: string
}> = [
  {
    action: "commit_message",
    icon: "Commit",
    label: "Commit",
    title: "Draft a commit message",
  },
  {
    action: "pr_body",
    icon: "PullRequestOpen",
    label: "PR body",
    title: "Draft a pull request body",
  },
  {
    action: "fix_checks",
    icon: "Bug",
    label: "Fix checks",
    title: "Prompt a check-fix pass",
  },
]

const sourceControlActionButton = (
  root: HTMLElement,
  detail: KhalaCodeSourceControlActionSubmitDetail,
  buttonConfig: typeof sourceControlActionButtons[number],
): HTMLButtonElement => {
  const button = document.createElement("button")
  button.type = "button"
  button.className = "cb-diff-source-action-button"
  button.title = buttonConfig.title
  button.setAttribute(
    "aria-label",
    `${buttonConfig.title} for ${detail.filePath ?? "this diff"}`,
  )
  button.dataset.action = buttonConfig.action
  button.dataset.sourceRef = detail.sourceRef

  const label = document.createElement("span")
  label.className = "cb-diff-source-action-label"
  label.textContent = buttonConfig.label
  button.replaceChildren(
    iconElement(buttonConfig.icon, { className: "cb-diff-source-action-icon" }),
    label,
  )

  button.addEventListener("click", event => {
    event.preventDefault()
    event.stopPropagation()
    root.dispatchEvent(new CustomEvent<KhalaCodeSourceControlActionSubmitDetail>(
      KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT,
      {
        bubbles: true,
        detail: {
          ...detail,
          action: buttonConfig.action,
        },
      },
    ))
  })

  return button
}

const sourceControlActions = (
  root: HTMLElement,
  filePath: string,
): HTMLElement => {
  const actions = document.createElement("span")
  actions.className = "cb-diff-source-actions"
  actions.setAttribute("aria-label", "Source-control AI actions")
  const baseDetail = {
    filePath,
    sourceRef: `diff.${filePath}`,
  }
  for (const button of sourceControlActionButtons) {
    actions.append(sourceControlActionButton(root, {
      ...baseDetail,
      action: button.action,
    }, button))
  }
  actions.title = sourceControlActionButtons
    .map(button => khalaCodeSourceControlActionLabel(button.action))
    .join(", ")
  return actions
}

const judgeVerdictLabel = (verdict: KhalaCodeJudgeDiffVerdict["verdict"]): string => {
  switch (verdict) {
    case "accept":
      return "Accept"
    case "request_changes":
      return "Request changes"
    case "replan":
      return "Replan"
  }
}

const judgeFindingLineLabel = (finding: KhalaCodeJudgeDiffVerdictFinding): string => {
  const range = finding.lineEnd === undefined || finding.lineEnd === finding.lineStart
    ? String(finding.lineStart)
    : `${finding.lineStart}-${finding.lineEnd}`
  return `${finding.filePath}:${range}`
}

const judgeFindingElement = (
  root: HTMLElement,
  finding: KhalaCodeJudgeDiffVerdictFinding,
  verdictKind: KhalaCodeJudgeDiffVerdict["verdict"],
): HTMLElement => {
  const item = document.createElement("li")
  item.className = "judge-verdict-finding"
  item.dataset.priority = finding.priority

  const head = document.createElement("div")
  head.className = "judge-verdict-finding-head"

  const priority = document.createElement("span")
  priority.className = "judge-verdict-priority"
  priority.textContent = finding.priority

  const title = document.createElement("span")
  title.className = "judge-verdict-finding-title"
  title.textContent = finding.title

  const confidence = document.createElement("span")
  confidence.className = "judge-verdict-confidence"
  confidence.textContent = `${Math.round(finding.confidence * 100)}%`
  confidence.title = "Judge confidence"

  head.append(priority, title, confidence)

  const anchor = document.createElement("div")
  anchor.className = "judge-verdict-anchor"
  anchor.textContent = judgeFindingLineLabel(finding)

  const body = document.createElement("p")
  body.className = "judge-verdict-finding-body"
  body.textContent = finding.body

  item.append(head, anchor, body)

  if (verdictKind === "request_changes") {
    const detail = khalaCodeJudgeDiffFindingToReviewDetail(finding)
    const action = document.createElement("button")
    action.type = "button"
    action.className = "judge-verdict-steer-button"
    action.textContent = "Send to coder"
    action.title = "Feed this judge finding into the diff-annotation steering loop"
    action.addEventListener("click", event => {
      event.preventDefault()
      event.stopPropagation()
      root.dispatchEvent(new CustomEvent<KhalaCodeDiffReviewSubmitDetail>(
        KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
        {
          bubbles: true,
          detail,
        },
      ))
    })
    item.append(action)
  }

  return item
}

export const judgeDiffVerdictElement = (
  verdict: KhalaCodeJudgeDiffVerdict,
): HTMLElement => {
  const root = document.createElement("section")
  root.className = "judge-verdict-card"
  root.dataset.verdict = verdict.verdict
  root.dataset.schema = verdict.schema

  const header = document.createElement("div")
  header.className = "judge-verdict-header"

  const role = document.createElement("span")
  role.className = "judge-verdict-role"
  role.textContent = "Judge"

  const kind = document.createElement("span")
  kind.className = "judge-verdict-kind"
  kind.textContent = judgeVerdictLabel(verdict.verdict)

  const confidence = document.createElement("span")
  confidence.className = "judge-verdict-confidence judge-verdict-confidence--overall"
  confidence.textContent = `${Math.round(verdict.confidence * 100)}%`
  confidence.title = "Overall judge confidence"

  header.append(role, kind, confidence)

  const summary = document.createElement("p")
  summary.className = "judge-verdict-summary"
  summary.textContent = verdict.summary

  const authority = document.createElement("p")
  authority.className = "judge-verdict-authority"
  authority.textContent = "Advisory verdict only. The verify command remains the merge authority."

  root.append(header, summary, authority)

  if (verdict.findings.length > 0) {
    const findings = document.createElement("ul")
    findings.className = "judge-verdict-findings"
    findings.append(...verdict.findings.map(finding =>
      judgeFindingElement(root, finding, verdict.verdict)))
    root.append(findings)
  }

  return root
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
  const filePath = parsed.filename ?? "diff"

  const header = document.createElement("div")
  header.className = "cb-header cb-diff-header"
  const file = document.createElement("span")
  file.className = "cb-file"
  file.textContent = filePath
  const meta = document.createElement("span")
  meta.className = "cb-diff-header-meta"
  const stats = document.createElement("span")
  stats.className = "cb-diff-stats"
  const add = document.createElement("span")
  add.className = "cb-stat-add"
  add.textContent = `+${parsed.added}`
  const rem = document.createElement("span")
  rem.className = "cb-stat-rem"
  rem.textContent = `-${parsed.removed}`
  stats.append(add, rem)
  meta.append(sourceControlActions(root, filePath), stats)
  header.append(file, meta)
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
    const lineNo = diffReviewLineNo(row)
    const lineKind = diffReviewKind(row)
    if (lineNo !== null && lineKind !== null) {
      line.append(diffReviewButton(root, line, {
        filePath,
        lineKind,
        lineNo,
        lineSide: diffReviewSide(row),
        patchRef: `diff.${filePath}.${lineKind}.${lineNo}`,
      }))
    }
    code.append(line)
  }

  pre.append(code)
  root.append(pre)
  return root
}

const headingElement = (block: Extract<MarkdownBlock, { readonly kind: "heading" }>): HTMLElement => {
  const tag = block.level === 1 ? "h3" : block.level === 2 ? "h4" : "h5"
  const heading = document.createElement(tag)
  heading.className = `md-heading md-heading--${block.level}`
  appendInlineMarkdown(heading, block.text)
  return heading
}

const listElement = (
  tag: "ol" | "ul",
  block: Extract<MarkdownBlock, { readonly kind: "ordered-list" | "unordered-list" }>,
): HTMLElement => {
  const list = document.createElement(tag)
  list.className = `md-list md-list--${tag === "ol" ? "ordered" : "unordered"}`
  for (const item of block.items) {
    const li = document.createElement("li")
    li.className = "md-list-item"
    appendInlineMarkdown(li, item)
    list.append(li)
  }
  return list
}

const markdownBlockElement = (block: MarkdownBlock): HTMLElement => {
  switch (block.kind) {
    case "heading":
      return headingElement(block)
    case "paragraph": {
      const p = document.createElement("p")
      p.className = "md-paragraph"
      appendInlineMarkdown(p, block.text)
      return p
    }
    case "unordered-list":
      return listElement("ul", block)
    case "ordered-list":
      return listElement("ol", block)
    case "blockquote": {
      const quote = document.createElement("blockquote")
      quote.className = "md-blockquote"
      appendInlineMarkdown(quote, block.text)
      return quote
    }
    case "code":
      return codeBlockElement({
        code: block.code,
        ...(block.language === undefined ? {} : { language: block.language }),
      })
    case "rule": {
      const rule = document.createElement("hr")
      rule.className = "md-rule"
      return rule
    }
  }
}

export const markdownElement = (input: { readonly markdown: string }): HTMLElement => {
  const root = document.createElement("div")
  root.className = "message-prose message-markdown"
  root.append(...parseMarkdownBlocks(input.markdown).map(markdownBlockElement))
  return root
}

const plainTextElement = (text: string): HTMLElement => {
  const root = document.createElement("div")
  root.className = "message-prose message-plain"
  const paragraphs = text
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .split(/\n{2,}/)
  for (const paragraph of paragraphs) {
    if (paragraph.trim().length === 0) continue
    const p = document.createElement("p")
    p.className = "md-paragraph"
    p.textContent = paragraph
    root.append(p)
  }
  if (root.childNodes.length === 0) {
    const p = document.createElement("p")
    p.className = "md-paragraph"
    p.textContent = text
    root.append(p)
  }
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
  const verdict = decodeJudgeDiffVerdict(text)
  if (verdict !== null) return [{ kind: "judge-diff-verdict", verdict }]

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
    const bodyVerdict = decodeJudgeDiffVerdict(body)
    if (bodyVerdict !== null && (info === "" || info === "json")) {
      segments.push({ kind: "judge-diff-verdict", verdict: bodyVerdict })
    } else if (info === "diff" || info === "patch" || looksLikeUnifiedDiff(body)) {
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

export const parseToolTranscript = (text: string): ToolTranscriptParts => {
  const normalized = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n")
  const firstLineEnd = normalized.indexOf("\n")
  const headline = firstLineEnd < 0 ? normalized : normalized.slice(0, firstLineEnd)
  const body = firstLineEnd < 0 ? "" : normalized.slice(firstLineEnd + 1).replace(/^\n/, "")
  const match = /^([A-Za-z][\w.-]*):\s*(.+?)\s*$/.exec(headline.trim())
  if (match === null) {
    return {
      output: normalized,
      status: "output",
      toolName: "tool",
    }
  }
  const rawStatus = match[2] ?? "output"
  return {
    output: body,
    status: normalizedToolStatus(rawStatus, body),
    toolName: match[1] ?? "tool",
  }
}

const normalizedToolStatus = (status: string, output: string): string => {
  if (status.toLowerCase() !== "ok") return status
  const normalizedOutput = output.toLowerCase()
  if (/\bcommand timed out\b/u.test(normalizedOutput)) return "failed"
  if (/\baccepted\s+0\/[1-9]\d*\b/u.test(normalizedOutput) && /\bfailed\b/u.test(normalizedOutput)) {
    return "failed"
  }
  return status
}

const toolTranscriptElement = (text: string): HTMLElement => {
  const parts = parseToolTranscript(text)
  const displayOutput = displayLocalPathsForKhalaCode(parts.output)
  const root = document.createElement("div")
  root.className = "tool-card"
  root.dataset.status = parts.status

  const header = document.createElement("div")
  header.className = "tool-card-header"

  const icon = iconElement("Tools", {
    ariaHidden: true,
    className: "tool-card-icon",
  })

  const name = document.createElement("span")
  name.className = "tool-card-name"
  name.textContent = parts.toolName

  const summary = document.createElement("span")
  summary.className = "tool-card-summary"
  summary.textContent = compactToolSummary(displayOutput, parts.status)

  const status = document.createElement("span")
  status.className = "tool-card-status"
  status.setAttribute("aria-label", parts.status)
  status.setAttribute("role", "img")
  status.title = parts.status

  header.append(name, icon, summary, status)
  root.append(header)

  if (displayOutput.trim().length > 0) {
    const chevron = document.createElement("span")
    chevron.className = "tool-card-chevron"
    chevron.setAttribute("aria-hidden", "true")
    header.append(chevron)
    bindExpandableToolCard(root, header)

    const pre = document.createElement("pre")
    pre.className = "tool-card-output"
    pre.textContent = displayOutput
    root.append(pre)
    // Live feed: keep the latest line in view while the box is compact.
    requestAnimationFrame(() => {
      if (root.dataset.expanded !== "true") pre.scrollTop = pre.scrollHeight
    })
  }

  return root
}

const codexItemElement = (input: {
  readonly codexItem: KhalaCodeDesktopCodexItemCard
  readonly role: KhalaCodeDesktopMessageRole
  readonly text: string
}): HTMLElement => {
  const root = document.createElement("div")
  root.className = "tool-card codex-item-card"
  root.dataset.codexItemType = input.codexItem.itemType
  root.dataset.itemId = input.codexItem.itemId
  root.dataset.status = input.codexItem.status
  if (input.codexItem.threadId !== undefined) root.dataset.threadId = input.codexItem.threadId
  if (input.codexItem.turnId !== undefined) root.dataset.turnId = input.codexItem.turnId

  const header = document.createElement("div")
  header.className = "tool-card-header codex-item-card-header tool-card-header--toggle"
  header.title = input.codexItem.title

  const icon = iconElement(iconForCodexItem(input.codexItem.itemType), {
    ariaHidden: true,
    className: "tool-card-icon codex-item-card-icon",
  })

  const title = document.createElement("span")
  title.className = "tool-card-name codex-item-card-title"
  title.textContent = input.codexItem.title

  const summary = document.createElement("span")
  summary.className = "tool-card-summary codex-item-card-summary"
  summary.textContent = compactToolSummary(input.text, input.codexItem.subtitle ?? input.codexItem.itemType)

  const meta = document.createElement("span")
  meta.className = "codex-item-card-meta"
  meta.textContent = input.codexItem.subtitle ?? input.codexItem.itemType

  const status = document.createElement("span")
  status.className = "tool-card-status codex-item-card-status"
  status.setAttribute("aria-label", input.codexItem.status)
  status.setAttribute("role", "img")
  status.title = input.codexItem.status

  const copy = document.createElement("button")
  copy.type = "button"
  copy.className = "codex-item-card-copy"
  copy.title = "Copy item output"
  copy.textContent = "Copy"
  copy.addEventListener("click", event => {
    event.stopPropagation()
    void navigator.clipboard?.writeText(input.text).catch(() => undefined)
  })

  const chevron = document.createElement("span")
  chevron.className = "tool-card-chevron"
  chevron.setAttribute("aria-hidden", "true")

  header.append(title, icon, summary, meta, status, copy, chevron)
  bindExpandableToolCard(root, header, input.codexItem.title)
  root.append(header)

  if (input.text.trim().length > 0) {
    const body = document.createElement("div")
    body.className = "codex-item-card-body"
    body.append(...parseMessageSegments(input.text).map(segment => {
      if (segment.kind === "judge-diff-verdict") return judgeDiffVerdictElement(segment.verdict)
      if (segment.kind === "diff") return diffElement({ patch: segment.text })
      if (segment.kind === "code") {
        return codeBlockElement({
          code: segment.text,
          ...(segment.language === undefined ? {} : { language: segment.language }),
        })
      }
      return input.role === "system"
        ? plainTextElement(segment.text)
        : markdownElement({ markdown: segment.text })
    }))
    root.append(body)
  }

  const approvalControls = codexApprovalControlsElement(input.codexItem)
  if (approvalControls !== null) root.append(approvalControls)

  return root
}

const approvalDecisionKey = (decision: unknown): string | null => {
  if (typeof decision === "string") return decision
  if (typeof decision !== "object" || decision === null || Array.isArray(decision)) return null
  const [key] = Object.keys(decision)
  return key ?? null
}

const actionDecisionKey = (action: KhalaCodeDesktopCodexApprovalAction): string => {
  switch (action) {
    case "acceptWithExecpolicyAmendment":
      return "acceptWithExecpolicyAmendment"
    case "applyNetworkPolicyAmendment":
      return "applyNetworkPolicyAmendment"
    case "grantPermissions":
    case "grantPermissionsForSession":
    case "grantPermissionsWithStrictReview":
      return "accept"
    default:
      return action
  }
}

const approvalActionAvailable = (
  codexItem: KhalaCodeDesktopCodexItemCard,
  action: KhalaCodeDesktopCodexApprovalAction,
): boolean => {
  const decisions = codexItem.approval?.availableDecisions
  if (decisions === undefined || decisions.length === 0) return true
  const keys = new Set(decisions.map(approvalDecisionKey).filter((key): key is string => key !== null))
  return keys.has(actionDecisionKey(action))
}

const codexApprovalButton = (
  codexItem: KhalaCodeDesktopCodexItemCard,
  label: string,
  action: KhalaCodeDesktopCodexApprovalAction,
  options: {
    readonly execpolicyAmendment?: readonly string[]
    readonly networkPolicyAmendment?: unknown
    readonly permissions?: unknown
  } = {},
): HTMLButtonElement | null => {
  const approval = codexItem.approval
  if (approval === undefined || !approvalActionAvailable(codexItem, action)) return null
  const button = document.createElement("button")
  button.type = "button"
  button.className = "codex-approval-button"
  button.textContent = label
  button.title = label
  button.dataset.codexApprovalAction = action
  button.dataset.codexApprovalMethod = approval.method
  button.dataset.codexApprovalRequestId = JSON.stringify(approval.requestId)
  if (options.execpolicyAmendment !== undefined) {
    button.dataset.codexApprovalExecpolicyAmendment = JSON.stringify(options.execpolicyAmendment)
  }
  if (options.networkPolicyAmendment !== undefined) {
    button.dataset.codexApprovalNetworkPolicyAmendment = JSON.stringify(options.networkPolicyAmendment)
  }
  if (options.permissions !== undefined) {
    button.dataset.codexApprovalPermissions = JSON.stringify(options.permissions)
  }
  return button
}

const codexApprovalControlsElement = (
  codexItem: KhalaCodeDesktopCodexItemCard,
): HTMLElement | null => {
  const approval = codexItem.approval
  if (approval === undefined || codexItem.status !== "pending") return null
  const controls = document.createElement("div")
  controls.className = "codex-approval-controls"

  if (approval.method === "item/permissions/requestApproval") {
    const permissions = approval.permissions ?? {}
    controls.append(
      ...[
        codexApprovalButton(codexItem, "Grant", "grantPermissions", { permissions }),
        codexApprovalButton(codexItem, "Session", "grantPermissionsForSession", { permissions }),
        codexApprovalButton(codexItem, "Review", "grantPermissionsWithStrictReview", { permissions }),
        codexApprovalButton(codexItem, "Decline", "decline"),
      ].filter((button): button is HTMLButtonElement => button !== null),
    )
    return controls
  }

  controls.append(
    ...[
      codexApprovalButton(codexItem, "Accept", "accept"),
      codexApprovalButton(codexItem, "Session", "acceptForSession"),
      ...(approval.proposedExecpolicyAmendment === undefined
        ? []
        : [
            codexApprovalButton(codexItem, "Execpolicy", "acceptWithExecpolicyAmendment", {
              execpolicyAmendment: approval.proposedExecpolicyAmendment,
            }),
          ]),
      ...(approval.proposedNetworkPolicyAmendments ?? []).map(amendment =>
        codexApprovalButton(codexItem, "Network", "applyNetworkPolicyAmendment", {
          networkPolicyAmendment: amendment,
        })),
      codexApprovalButton(codexItem, "Decline", "decline"),
      codexApprovalButton(codexItem, "Cancel", "cancel"),
    ].filter((button): button is HTMLButtonElement => button !== null),
  )
  return controls
}

export const renderMessageBody = (
  text: string,
  role: KhalaCodeDesktopMessageRole = "assistant",
  codexItem?: KhalaCodeDesktopCodexItemCard,
): readonly HTMLElement[] => {
  if (codexItem !== undefined && codexItem.itemType !== "agentMessage" && codexItem.itemType !== "userMessage") {
    return [codexItemElement({ codexItem, role, text })]
  }
  if (role === "tool") return [toolTranscriptElement(text)]
  if (role === "system") return [plainTextElement(text)]
  return parseMessageSegments(text).map(segment => {
    if (segment.kind === "judge-diff-verdict") return judgeDiffVerdictElement(segment.verdict)
    if (segment.kind === "diff") return diffElement({ patch: segment.text })
    if (segment.kind === "code") {
      return codeBlockElement({
        code: segment.text,
        ...(segment.language === undefined ? {} : { language: segment.language }),
      })
    }
    return markdownElement({ markdown: segment.text })
  })
}
