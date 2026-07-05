import { describe, expect, test } from "bun:test"

import { buildComposerTextWithQuote, buildQuoteSnippet } from "../src/sync/swipe-quote-core"
import type { TranscriptPart } from "../src/sync/khala-runtime-transcript-core"

describe("buildQuoteSnippet", () => {
  test("quotes a text part verbatim when short", () => {
    const part: TranscriptPart = { id: "m1", kind: "text", text: "Hello there" }
    expect(buildQuoteSnippet(part)).toBe("> Hello there")
  })

  test("truncates long text with an ellipsis at the requested max length", () => {
    const part: TranscriptPart = { id: "m1", kind: "text", text: "x".repeat(200) }
    const snippet = buildQuoteSnippet(part, 20)
    expect(snippet).toBe(`> ${"x".repeat(19)}…`)
    expect(snippet?.length).toBe(2 + 20)
  })

  test("collapses internal whitespace/newlines before truncating", () => {
    const part: TranscriptPart = { id: "m1", kind: "text", text: "line one\n\n  line   two" }
    expect(buildQuoteSnippet(part)).toBe("> line one line two")
  })

  test("labels a reasoning part distinctly from a text part", () => {
    const part: TranscriptPart = { id: "m1", kind: "reasoning", text: "thinking it through" }
    expect(buildQuoteSnippet(part)).toBe("> (reasoning) thinking it through")
  })

  test("quotes a tool part by name and status, not by (absent) text", () => {
    const part: TranscriptPart = {
      id: "m1",
      kind: "tool",
      toolCallId: "call1",
      toolName: "search",
      status: "completed"
    }
    expect(buildQuoteSnippet(part)).toBe("> re: search (completed)")
  })

  test("returns undefined for usage parts (nothing meaningful to quote)", () => {
    const part: TranscriptPart = { id: "m1", kind: "usage", inputTokens: 10, outputTokens: 20, totalTokens: 30 }
    expect(buildQuoteSnippet(part)).toBeUndefined()
  })

  test("returns undefined for turn-status parts (nothing meaningful to quote)", () => {
    const part: TranscriptPart = { id: "m1", kind: "turn-status", lane: "codex_app_server", status: "completed" }
    expect(buildQuoteSnippet(part)).toBeUndefined()
  })
})

describe("buildComposerTextWithQuote", () => {
  test("starts the draft with the snippet plus a trailing space when the draft is empty", () => {
    expect(buildComposerTextWithQuote("", "> quoted text")).toBe("> quoted text ")
  })

  test("treats a whitespace-only draft as empty", () => {
    expect(buildComposerTextWithQuote("   \n  ", "> quoted text")).toBe("> quoted text ")
  })

  test("prepends the snippet above an existing non-empty draft, preserving it", () => {
    expect(buildComposerTextWithQuote("my follow-up", "> quoted text")).toBe("> quoted text\nmy follow-up")
  })
})
