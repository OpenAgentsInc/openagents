import { describe, expect, test } from "bun:test"

import {
  assertPart2UiPublicSafeText,
  PART2_UI_RECORDING_SMOKE_HARNESS,
  part2UiSmokeViewports,
} from "../scripts/part2-ui-recording-smoke"

describe("Part 2 UI recording smoke", () => {
  test("declares a single UI-first transcript 245 smoke over desktop and mobile", () => {
    expect(PART2_UI_RECORDING_SMOKE_HARNESS).toBe(
      "khala_code_transcript_245_part2_ui_smoke",
    )
    expect(part2UiSmokeViewports()).toEqual([
      { name: "desktop", width: 1280, height: 800 },
      { name: "mobile", width: 390, height: 844 },
    ])
  })

  test("rejects unsafe UI text and the legacy 0/1 capacity dead-end", () => {
    expect(() =>
      assertPart2UiPublicSafeText(
        "Fleet delegate completed with action_submission.proposal.fixture",
      ),
    ).not.toThrow()
    expect(() =>
      assertPart2UiPublicSafeText("raw_prompt.body should never render"),
    ).toThrow("private or raw")
    expect(() =>
      assertPart2UiPublicSafeText(
        "codex_spawn_failed: No Pylon Codex assignment capacity is available right now",
      ),
    ).toThrow("legacy")
    expect(() =>
      assertPart2UiPublicSafeText("No Pylon capacity (0/1 available)."),
    ).toThrow("legacy")
  })
})
