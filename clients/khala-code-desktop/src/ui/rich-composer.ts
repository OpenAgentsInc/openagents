export type KhalaRichComposerMode = "normal" | "shell"

export type KhalaRichComposerKeyCommand =
  | "history-next"
  | "history-previous"
  | "newline"
  | "submit"

export type KhalaRichComposerKeyEvent = Readonly<{
  altKey?: boolean
  ctrlKey?: boolean
  isComposing?: boolean
  key: string
  metaKey?: boolean
  shiftKey?: boolean
}>

type KhalaComposerHistoryState = {
  cursor: number | null
  entries: string[]
}

const composerModes = ["normal", "shell"] as const
const elementNodeType = 1
const textNodeType = 3

const blockElementNames = new Set([
  "ADDRESS",
  "ARTICLE",
  "ASIDE",
  "BLOCKQUOTE",
  "DD",
  "DIV",
  "DL",
  "DT",
  "FIELDSET",
  "FIGCAPTION",
  "FIGURE",
  "FOOTER",
  "FORM",
  "H1",
  "H2",
  "H3",
  "H4",
  "H5",
  "H6",
  "HEADER",
  "HR",
  "LI",
  "MAIN",
  "NAV",
  "OL",
  "P",
  "PRE",
  "SECTION",
  "TABLE",
  "UL",
])

export const normalizeKhalaComposerPlainText = (value: string): string =>
  value.replace(/\r\n?/gu, "\n").replace(/\u0000/gu, "")

export const normalizeKhalaComposerPasteText = (value: string): string =>
  normalizeKhalaComposerPlainText(value).replace(/\u00a0/gu, " ")

const isElementNode = (node: Node): node is Element =>
  node.nodeType === elementNodeType

const isBlockElement = (node: Node): node is Element =>
  isElementNode(node) && blockElementNames.has(node.tagName)

const readNodeText = (node: Node): string => {
  if (node.nodeType === textNodeType) {
    return (node.textContent ?? "")
  }
  if (!isElementNode(node)) return ""
  if (node.tagName === "BR") return "\n"
  const firstChild = node.firstChild
  if (
    isBlockElement(node) &&
    node.childNodes.length === 1 &&
    firstChild !== null &&
    isElementNode(firstChild) &&
    firstChild.tagName === "BR"
  ) {
    return ""
  }
  return Array.from(node.childNodes).map(readNodeText).join("")
}

export const readKhalaComposerPlainText = (element: HTMLElement): string => {
  const children = Array.from(element.childNodes)
  const value = children.reduce((output, child, index) => {
    const next = `${output}${readNodeText(child)}`
    if (
      isBlockElement(child) &&
      index < children.length - 1 &&
      !next.endsWith("\n")
    ) {
      return `${next}\n`
    }
    return next
  }, "")
  return normalizeKhalaComposerPlainText(value)
}

const textNodesForValue = (
  document: Document,
  value: string,
): { fragment: DocumentFragment; lastNode: Node | null } => {
  const fragment = document.createDocumentFragment()
  let lastNode: Node | null = null
  const lines = normalizeKhalaComposerPlainText(value).split("\n")
  lines.forEach((line, index) => {
    if (line.length > 0) {
      const text = document.createTextNode(line)
      fragment.append(text)
      lastNode = text
    }
    if (index < lines.length - 1) {
      const br = document.createElement("br")
      fragment.append(br)
      lastNode = br
    }
  })
  return { fragment, lastNode }
}

export const syncKhalaComposerEmptyState = (element: HTMLElement): void => {
  element.dataset.empty =
    readKhalaComposerPlainText(element).length === 0 ? "true" : "false"
}

export const writeKhalaComposerPlainText = (
  element: HTMLElement,
  value: string,
): void => {
  const normalized = normalizeKhalaComposerPlainText(value)
  element.replaceChildren()
  if (normalized.length > 0) {
    element.append(textNodesForValue(element.ownerDocument, normalized).fragment)
  }
  element.dataset.empty = normalized.length === 0 ? "true" : "false"
}

const nodeBelongsToElement = (element: HTMLElement, node: Node | null): boolean =>
  node !== null && (node === element || element.contains(node))

const selectionForElement = (element: HTMLElement): Selection | null =>
  element.ownerDocument.getSelection?.() ??
  (typeof window === "undefined" ? null : window.getSelection())

export const setKhalaComposerCaretToEnd = (element: HTMLElement): void => {
  const selection = selectionForElement(element)
  if (selection === null) return
  const range = element.ownerDocument.createRange()
  range.selectNodeContents(element)
  range.collapse(false)
  selection.removeAllRanges()
  selection.addRange(range)
}

export const insertKhalaComposerTextAtSelection = (
  element: HTMLElement,
  value: string,
): void => {
  const normalized = normalizeKhalaComposerPlainText(value)
  if (normalized.length === 0) return
  const selection = selectionForElement(element)
  const range = element.ownerDocument.createRange()
  if (
    selection !== null &&
    selection.rangeCount > 0 &&
    nodeBelongsToElement(element, selection.anchorNode) &&
    nodeBelongsToElement(element, selection.focusNode)
  ) {
    range.setStart(selection.getRangeAt(0).startContainer, selection.getRangeAt(0).startOffset)
    range.setEnd(selection.getRangeAt(0).endContainer, selection.getRangeAt(0).endOffset)
  } else {
    range.selectNodeContents(element)
    range.collapse(false)
  }

  range.deleteContents()
  const { fragment, lastNode } = textNodesForValue(element.ownerDocument, normalized)
  range.insertNode(fragment)
  if (selection !== null && lastNode !== null) {
    const nextRange = element.ownerDocument.createRange()
    nextRange.setStartAfter(lastNode)
    nextRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(nextRange)
  }
  syncKhalaComposerEmptyState(element)
}

export const createKhalaComposerPromptHistory = (
  limit = 50,
): {
  all: (mode: KhalaRichComposerMode) => readonly string[]
  next: (mode: KhalaRichComposerMode) => string | null
  previous: (mode: KhalaRichComposerMode, currentValue: string) => string | null
  push: (mode: KhalaRichComposerMode, value: string) => void
} => {
  const state: Record<KhalaRichComposerMode, KhalaComposerHistoryState> = {
    normal: { cursor: null, entries: [] },
    shell: { cursor: null, entries: [] },
  }

  const clampHistory = (mode: KhalaRichComposerMode): void => {
    const bucket = state[mode]
    if (bucket.entries.length <= limit) return
    bucket.entries = bucket.entries.slice(bucket.entries.length - limit)
  }

  return {
    all: mode => [...state[mode].entries],
    next: mode => {
      const bucket = state[mode]
      if (bucket.cursor === null) return null
      const nextCursor = bucket.cursor + 1
      if (nextCursor >= bucket.entries.length) {
        bucket.cursor = null
        return ""
      }
      bucket.cursor = nextCursor
      return bucket.entries[nextCursor] ?? null
    },
    previous: (mode, _currentValue) => {
      const bucket = state[mode]
      if (bucket.entries.length === 0) return null
      const cursor =
        bucket.cursor === null
          ? bucket.entries.length - 1
          : Math.max(0, bucket.cursor - 1)
      bucket.cursor = cursor
      return bucket.entries[cursor] ?? null
    },
    push: (mode, value) => {
      const normalized = normalizeKhalaComposerPlainText(value)
      const bucket = state[mode]
      bucket.cursor = null
      if (normalized.trim() === "") return
      if (bucket.entries[bucket.entries.length - 1] === normalized) return
      bucket.entries.push(normalized)
      clampHistory(mode)
    },
  }
}

export const isKhalaRichComposerMode = (
  value: string,
): value is KhalaRichComposerMode =>
  composerModes.includes(value as KhalaRichComposerMode)

export const khalaRichComposerCommandForKey = (
  event: KhalaRichComposerKeyEvent,
  currentText: string,
): KhalaRichComposerKeyCommand | null => {
  if (event.isComposing === true) return null
  if (event.key === "Enter") {
    if (event.metaKey === true || event.ctrlKey === true || event.altKey === true) {
      return null
    }
    return event.shiftKey === true ? "newline" : "submit"
  }
  if (
    event.key === "ArrowUp" &&
    event.metaKey !== true &&
    event.ctrlKey !== true &&
    (event.altKey === true || currentText.trim() === "")
  ) {
    return "history-previous"
  }
  if (
    event.key === "ArrowDown" &&
    event.metaKey !== true &&
    event.ctrlKey !== true &&
    (event.altKey === true || currentText.trim() === "")
  ) {
    return "history-next"
  }
  return null
}
