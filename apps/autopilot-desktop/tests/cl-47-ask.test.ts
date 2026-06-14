import { describe, expect, test } from "bun:test"
import { shipStatusLine } from "../src/ui/cards/ask"

describe("CL-47 shipStatusLine", () => {
  test("received → non-terminal muted label", () => {
    const s = shipStatusLine("received")
    expect(s.text).toBe("received")
    expect(s.terminal).toBe(false)
  })

  test("planning → non-terminal ellipsis label", () => {
    const s = shipStatusLine("planning")
    expect(s.text).toBe("planning…")
    expect(s.terminal).toBe(false)
  })

  test("fanning_out → non-terminal agents label", () => {
    const s = shipStatusLine("fanning_out")
    expect(s.text).toBe("agents working…")
    expect(s.terminal).toBe(false)
  })

  test("shipping → non-terminal shipping label", () => {
    const s = shipStatusLine("shipping")
    expect(s.text).toBe("shipping…")
    expect(s.terminal).toBe(false)
  })

  test("shipped → terminal success label", () => {
    const s = shipStatusLine("shipped")
    expect(s.text).toBe("✓ shipped")
    expect(s.terminal).toBe(true)
  })

  test("failed → terminal failure label", () => {
    const s = shipStatusLine("failed")
    expect(s.text).toBe("✗ failed")
    expect(s.terminal).toBe(true)
  })

  test("unknown status → passthrough with non-terminal", () => {
    const s = shipStatusLine("some_future_state")
    expect(s.text).toBe("some_future_state")
    expect(s.terminal).toBe(false)
  })

  test("shipped dot color is green", () => {
    expect(shipStatusLine("shipped").dotColor).toBe("#3fb950")
  })

  test("failed dot color is red", () => {
    expect(shipStatusLine("failed").dotColor).toBe("#f85149")
  })

  test("received dot color is muted", () => {
    expect(shipStatusLine("received").dotColor).toBe("#8b949e")
  })
})
