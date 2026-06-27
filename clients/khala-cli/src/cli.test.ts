import { describe, expect, test } from "bun:test"

import { formatKhalaSpawnCapabilityAnswer } from "./cli.js"

describe("Khala CLI spawn capability answer", () => {
  test("answers the original subprocess capability question with the reviewed CLI path", () => {
    const answer = formatKhalaSpawnCapabilityAnswer()
    const lower = answer.toLowerCase()

    expect(answer).toContain("Yes.")
    expect(answer).toContain("/spawn 5")
    expect(answer).toContain('khala spawn --count 5 --objective "audit X"')
    expect(lower).toContain("supervised khala child workers")
    expect(lower).toContain("public/browser chat")
    expect(lower).toContain("cannot execute local workers on your machine")
    expect(lower).not.toContain("capability we don't yet expose")
    expect(lower).not.toContain("we do not yet expose")
  })
})
