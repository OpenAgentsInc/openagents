import { describe, expect, test } from "vite-plus/test"

import {
  mobileAssistantContentViews,
  parseMobileMarkdownInline,
} from "../src/screens/mobile-transcript-content"

describe("mobile transcript rich content", () => {
  test("projects readable typed Markdown, fenced code, and copy actions", () => {
    const views = mobileAssistantContentViews(
      "message.assistant.1",
      "## Result\n\nUse **Effect** and [the docs](https://example.com/docs).\n\n```ts\nconst ready = true\n```",
    )
    const serialized = JSON.stringify(views)

    expect(serialized).toContain('"_tag":"Markdown"')
    expect(serialized).toContain('"kind":"heading"')
    expect(serialized).toContain('"kind":"strong"')
    expect(serialized).toContain('"href":"https://example.com/docs"')
    expect(serialized).toContain('"_tag":"CodeBlock"')
    expect(serialized).toContain('"language":"ts"')
    expect(serialized).toContain('"label":"Copy code"')
    expect(serialized).toContain('"accessibilityLabel":"Copy assistant message"')
  })

  test("degrades unsafe links to readable text", () => {
    const inline = parseMobileMarkdownInline(
      "Read [safe](https://openagents.com) and [blocked](javascript:alert).",
    )
    const serialized = JSON.stringify(inline)

    expect(serialized).toContain('"href":"https://openagents.com/"')
    expect(serialized).toContain('"text":"blocked"')
    expect(serialized).not.toContain("javascript:")
  })

  test("bounds oversized content and unclosed fences", () => {
    const views = mobileAssistantContentViews("message.assistant.large", `\`\`\`txt\n${"x".repeat(25_000)}`)
    const serialized = JSON.stringify(views)

    expect(serialized.length).toBeLessThan(25_000)
    expect(serialized).toContain('"_tag":"CodeBlock"')
    expect(serialized).toContain('"accessibilityLabel":"Copy assistant message"')
  })
})
