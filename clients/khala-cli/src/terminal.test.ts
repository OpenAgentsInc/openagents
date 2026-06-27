import { describe, expect, test } from "bun:test"

import { normalizeMarkdownSpacing } from "./terminal.js"

describe("terminal Markdown spacing", () => {
  test("collapses excessive paragraph gaps outside code fences", () => {
    expect(normalizeMarkdownSpacing([
      "First paragraph.",
      "",
      "",
      "",
      "Second paragraph.",
      "```",
      "one",
      "",
      "",
      "two",
      "```",
      "",
      "",
      "Third paragraph.",
    ].join("\n"))).toBe([
      "First paragraph.",
      "",
      "Second paragraph.",
      "```",
      "one",
      "",
      "",
      "two",
      "```",
      "",
      "Third paragraph.",
    ].join("\n"))
  })
})
