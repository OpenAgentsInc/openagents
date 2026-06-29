export type SessionMarkdownExportInput = {
  sessionRef: string
  title?: string
  events: {
    phase: string
    messageText: string
    observedAt: string
  }[]
}

function inlineText(value: string): string {
  return value.replace(/\s+/g, " ").trim()
}

export function exportSessionMarkdown(input: SessionMarkdownExportInput): string {
  const title = inlineText(input.title ?? "") || inlineText(input.sessionRef)
  const lines = [`# ${title}`, ""]

  for (const event of input.events) {
    const observedAt = inlineText(event.observedAt)
    const phase = inlineText(event.phase)
    const message = inlineText(event.messageText)

    lines.push(`- ${observedAt} [${phase}] ${message}`)
  }

  return `${lines.join("\n").trimEnd()}\n`
}
