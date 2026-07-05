import { describe, expect, test } from "bun:test"

import { buildCopyMarkdown, buildCopyText } from "../src/sync/blurred-popup-menu-core"
import type { TranscriptPart } from "../src/sync/khala-runtime-transcript-core"

describe("buildCopyText", () => {
  test("copies a text part verbatim", () => {
    const part: TranscriptPart = { id: "m1", kind: "text", text: "Hello there" }
    expect(buildCopyText(part)).toBe("Hello there")
  })

  test("copies a reasoning part verbatim (no blockquote decoration)", () => {
    const part: TranscriptPart = { id: "m1", kind: "reasoning", text: "thinking it through" }
    expect(buildCopyText(part)).toBe("thinking it through")
  })

  test("copies a tool part by name and status when there's no error", () => {
    const part: TranscriptPart = {
      id: "m1",
      kind: "tool",
      toolCallId: "call1",
      toolName: "search",
      status: "completed"
    }
    expect(buildCopyText(part)).toBe("search (completed)")
  })

  test("includes the error message for a failed tool part", () => {
    const part: TranscriptPart = {
      id: "m1",
      kind: "tool",
      toolCallId: "call1",
      toolName: "search",
      status: "failed",
      errorMessageSafe: "timed out"
    }
    expect(buildCopyText(part)).toBe("search (failed): timed out")
  })

  test("returns undefined for usage and turn-status parts", () => {
    expect(buildCopyText({ id: "u1", kind: "usage" })).toBeUndefined()
    expect(buildCopyText({ id: "t1", kind: "turn-status", status: "running", lane: "ai_sdk_core" })).toBeUndefined()
  })
})

describe("buildCopyMarkdown", () => {
  test("copies a text part verbatim (no decoration needed)", () => {
    const part: TranscriptPart = { id: "m1", kind: "text", text: "Hello there" }
    expect(buildCopyMarkdown(part)).toBe("Hello there")
  })

  test("wraps a reasoning part in a markdown blockquote", () => {
    const part: TranscriptPart = { id: "m1", kind: "reasoning", text: "thinking it through" }
    expect(buildCopyMarkdown(part)).toBe("> thinking it through")
  })

  test("renders a tool part's name as inline code", () => {
    const part: TranscriptPart = {
      id: "m1",
      kind: "tool",
      toolCallId: "call1",
      toolName: "search",
      status: "completed"
    }
    expect(buildCopyMarkdown(part)).toBe("`search` — completed")
  })

  test("includes the error message for a failed tool part", () => {
    const part: TranscriptPart = {
      id: "m1",
      kind: "tool",
      toolCallId: "call1",
      toolName: "search",
      status: "failed",
      errorMessageSafe: "timed out"
    }
    expect(buildCopyMarkdown(part)).toBe("`search` — failed: timed out")
  })

  test("returns undefined for usage and turn-status parts", () => {
    expect(buildCopyMarkdown({ id: "u1", kind: "usage" })).toBeUndefined()
    expect(
      buildCopyMarkdown({ id: "t1", kind: "turn-status", status: "completed", lane: "codex_app_server" })
    ).toBeUndefined()
  })
})
