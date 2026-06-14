import { describe, expect, test } from "bun:test"

import { coordinatorToggleLabel } from "../src/ui/cards/cloud"

describe("coordinatorToggleLabel", () => {
  test('returns "▶ Resume" when paused is true', () => {
    expect(coordinatorToggleLabel(true)).toBe("▶ Resume")
  })

  test('returns "⏸ Pause" when paused is false', () => {
    expect(coordinatorToggleLabel(false)).toBe("⏸ Pause")
  })
})
