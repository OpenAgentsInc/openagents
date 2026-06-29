export type SessionSearchEvent = {
  messageText?: string
  messageFull?: string
  phase: string
}

export type SessionSearchResult = {
  index: number
  snippet: string
}

const MAX_SNIPPET_LENGTH = 120

function readString(value: unknown): string {
  return typeof value === "string" ? value : ""
}

function eventMessage(event: unknown): string {
  if (typeof event !== "object" || event === null || Array.isArray(event)) return ""

  const row = event as Record<string, unknown>
  const messageText = readString(row.messageText)
  const messageFull = readString(row.messageFull)

  if (messageText === "") return messageFull
  if (messageFull === "") return messageText
  return `${messageText}\n${messageFull}`
}

function matchingLine(text: string, matchIndex: number): { line: string; matchIndex: number } {
  const lineStartMarker = text.lastIndexOf("\n", matchIndex - 1)
  const lineStart = lineStartMarker === -1 ? 0 : lineStartMarker + 1
  const lineEndMarker = text.indexOf("\n", matchIndex)
  const lineEnd = lineEndMarker === -1 ? text.length : lineEndMarker
  let line = text.slice(lineStart, lineEnd)

  if (line.endsWith("\r")) line = line.slice(0, -1)

  return {
    line,
    matchIndex: matchIndex - lineStart,
  }
}

function cropSnippet(line: string, matchIndex: number, matchLength: number): string {
  if (line.length <= MAX_SNIPPET_LENGTH) return line

  if (matchLength >= MAX_SNIPPET_LENGTH) {
    return line.slice(matchIndex, matchIndex + MAX_SNIPPET_LENGTH)
  }

  const contextLength = MAX_SNIPPET_LENGTH - matchLength
  const beforeLength = Math.floor(contextLength / 2)
  let start = matchIndex - beforeLength
  let end = start + MAX_SNIPPET_LENGTH

  if (start < 0) {
    start = 0
    end = MAX_SNIPPET_LENGTH
  }

  if (end > line.length) {
    end = line.length
    start = Math.max(0, end - MAX_SNIPPET_LENGTH)
  }

  return line.slice(start, end)
}

export function searchSessionEvents(
  events: SessionSearchEvent[],
  query: string,
): SessionSearchResult[] {
  if (!Array.isArray(events) || query === "") return []

  const needle = query.toLowerCase()
  const results: SessionSearchResult[] = []

  events.forEach((event, index) => {
    const message = eventMessage(event)
    const matchIndex = message.toLowerCase().indexOf(needle)

    if (matchIndex === -1) return

    const lineMatch = matchingLine(message, matchIndex)
    results.push({
      index,
      snippet: cropSnippet(lineMatch.line, lineMatch.matchIndex, query.length),
    })
  })

  return results
}
