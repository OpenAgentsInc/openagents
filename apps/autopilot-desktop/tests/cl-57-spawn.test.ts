import { describe, expect, test } from "bun:test"
import { parseVerifyLines } from "../src/ui/panes/spawn"

describe("CL-57 parseVerifyLines", () => {
  test("returns empty array for empty string", () => {
    expect(parseVerifyLines("")).toEqual([])
  })

  test("returns empty array for whitespace-only string", () => {
    expect(parseVerifyLines("   \n\t\n  ")).toEqual([])
  })

  test("splits on newlines and trims each line", () => {
    expect(parseVerifyLines("  bun test  \n  bun run typecheck  ")).toEqual([
      "bun test",
      "bun run typecheck",
    ])
  })

  test("drops empty lines between content", () => {
    expect(parseVerifyLines("bun test\n\nbun run typecheck\n")).toEqual([
      "bun test",
      "bun run typecheck",
    ])
  })

  test("handles a single command without a trailing newline", () => {
    expect(parseVerifyLines("bun test")).toEqual(["bun test"])
  })

  test("handles a single command with a trailing newline", () => {
    expect(parseVerifyLines("bun test\n")).toEqual(["bun test"])
  })

  test("handles many blank lines between commands", () => {
    expect(parseVerifyLines("bun test\n\n\n\nbun run build")).toEqual([
      "bun test",
      "bun run build",
    ])
  })

  test("trims lines with tabs", () => {
    expect(parseVerifyLines("\tbun test\t\n\tbun run build\t")).toEqual([
      "bun test",
      "bun run build",
    ])
  })

  test("preserves internal spaces within a command", () => {
    expect(parseVerifyLines("bun run typecheck --noEmit")).toEqual([
      "bun run typecheck --noEmit",
    ])
  })

  test("returns all non-empty lines as separate entries", () => {
    const result = parseVerifyLines("a\nb\nc")
    expect(result).toHaveLength(3)
    expect(result).toEqual(["a", "b", "c"])
  })
})
