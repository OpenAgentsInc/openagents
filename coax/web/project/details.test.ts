import { expect, test } from "bun:test"
import { escapeDetail, escapeList, splitAcceptance, unescapeDetail } from "./details"

test("a field never holds a raw '|' or newline, and decodes back", () => {
  const nasty = ["a|b", "two\nlines", "back\\slash", "\\.", "\\:", "\\n literally", "|\n|\\", "", "trailing\\"]
  for (const s of nasty) {
    const f = escapeDetail(s)
    expect(f.includes("|") || f.includes("\n")).toBe(false)
    expect(unescapeDetail(f)).toEqual([[s]])
  }
})

test("lists of parts round-trip, whatever the text holds", () => {
  const notes = [["10-28 14:52 CDT", "claude-queue-1", "took it | for real\nsecond line \\. not a break"], ["10-28 15:00 CDT", "", "x\\:y"]]
  const f = escapeList(notes)
  expect(f.includes("|") || f.includes("\n")).toBe(false)
  expect(unescapeDetail(f)).toEqual(notes)
  const checks = [["one"], ["two | three"], ["four\\"]]
  expect(unescapeDetail(escapeList(checks))).toEqual(checks)
})

test("the acceptance section comes out of the body as a list", () => {
  const s = splitAcceptance("Do the thing.\n\n## Acceptance\n\n- it works\n- it is fast\n  and small\n\n## Files\n\n- `a.coax`")
  expect(s.acceptance).toEqual(["it works", "it is fast and small"])
  expect(s.body).toBe("Do the thing.\n\n## Files\n\n- `a.coax`")
  expect(splitAcceptance("plain").acceptance).toEqual([])
})
