import { describe, expect, test } from "bun:test"

import { renderMarkdownForTerminal } from "./terminal.js"

describe("terminal Markdown rendering", () => {
  test("collapses repeated blank lines between prose paragraphs", () => {
    expect(renderMarkdownForTerminal("First paragraph.\n\n\n\nSecond paragraph.")).toBe(
      "First paragraph.\n\nSecond paragraph.",
    )
  })

  test("preserves blank lines inside fenced code blocks", () => {
    expect(renderMarkdownForTerminal("```txt\none\n\n\nthree\n```\n\n\nDone.")).toBe(
      "```txt\none\n\n\nthree\n```\n\nDone.",
    )
  })
})
