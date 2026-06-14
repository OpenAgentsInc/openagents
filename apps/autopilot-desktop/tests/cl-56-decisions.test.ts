// CL-56: unit tests for the decisions pane pure logic.
// These tests are DOM-free — only the exported pure helper is tested here.

import { describe, expect, test } from "bun:test"
import { approvalLabel } from "../src/ui/panes/decisions"

describe("approvalLabel (CL-56)", () => {
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

  test("trims prompt for emptiness check but returns the original value", () => {
    // Non-blank after trim → return as-is (not trimmed).
    expect(approvalLabel({ prompt: "  write file  ", kind: "file_write" })).toBe("  write file  ")
  })

  test("returns kind when both prompt and kind are empty (edge case)", () => {
    expect(approvalLabel({ prompt: "", kind: "" })).toBe("")
  })

  test("handles a multi-line prompt (real agent output)", () => {
    const prompt = "Allow bash command:\n  git push origin main\n"
    expect(approvalLabel({ prompt, kind: "shell_exec" })).toBe(prompt)
  })

  test("uses prompt for a file_write kind with a path prompt", () => {
    expect(approvalLabel({ prompt: "Write to /etc/hosts?", kind: "file_write" })).toBe(
      "Write to /etc/hosts?",
    )
  })

  test("uses kind as fallback for unknown approval kinds", () => {
    expect(approvalLabel({ prompt: "", kind: "network_fetch" })).toBe("network_fetch")
  })
})
