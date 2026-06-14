// CL-48: unit tests for the approvals card pure logic.
// These tests are DOM-free — only the exported pure helper is tested here.

import { describe, expect, test } from "bun:test"
import { approvalLabel } from "../src/ui/cards/approvals"

describe("approvalLabel (CL-48)", () => {
  test("returns prompt when prompt is non-empty", () => {
    expect(approvalLabel({ prompt: "Run rm -rf /tmp/cache?", kind: "shell_exec" })).toBe(
      "Run rm -rf /tmp/cache?",
    )
  })

  test("falls back to kind when prompt is empty string", () => {
    expect(approvalLabel({ prompt: "", kind: "shell_exec" })).toBe("shell_exec")
  })

  test("falls back to kind when prompt is only whitespace", () => {
    expect(approvalLabel({ prompt: "   ", kind: "file_write" })).toBe("file_write")
  })

  test("trims but still uses a prompt that has surrounding whitespace", () => {
    // The label itself is not trimmed — we only use trim() to detect emptiness.
    // So a non-blank (after trim) prompt is returned as-is.
    expect(approvalLabel({ prompt: "  write file  ", kind: "file_write" })).toBe("  write file  ")
  })

  test("returns kind when both prompt and kind are empty (edge case)", () => {
    expect(approvalLabel({ prompt: "", kind: "" })).toBe("")
  })

  test("handles a multi-line prompt (real agent output)", () => {
    const prompt = "Allow bash command:\n  git push origin main\n"
    expect(approvalLabel({ prompt, kind: "shell_exec" })).toBe(prompt)
  })
})
