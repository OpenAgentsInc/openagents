import { describe, expect, test } from "bun:test"

import {
  assertPart2UiPublicSafeText,
  PART2_UI_RECORDING_SMOKE_HARNESS,
  part2UiSmokeViewports,
} from "../scripts/part2-ui-recording-smoke"
import {
  assertKhalaCodePublicSafeValue,
  khalaCodeUnsafeTextPattern,
} from "../scripts/public-safety-oracle"

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

  test("rejects unsafe screenshot-adjacent metadata in smoke summaries", () => {
    expect(() =>
      assertKhalaCodePublicSafeValue({
        harness: PART2_UI_RECORDING_SMOKE_HARNESS,
        screenshots: ["part2-ui-fleet-desktop.png"],
        visualBaseline: { status: "matched", baseline: "screenshots/part2.png" },
      }),
    ).not.toThrow()
    expect(() =>
      assertKhalaCodePublicSafeValue({
        harness: PART2_UI_RECORDING_SMOKE_HARNESS,
        screenshots: ["/Users/operator/.codex/auth.json"],
      }),
    ).toThrow("private or raw")
    expect(khalaCodeUnsafeTextPattern.test("raw_prompt.body")).toBe(true)
  })

  test("wires every Mode D visual smoke through console oracles and seed RPC mocks", async () => {
    const scriptUrls = [
      "../scripts/part2-ui-recording-smoke.ts",
      "../scripts/cockpit-visual-smoke.ts",
      "../scripts/composer-visual-smoke.ts",
      "../scripts/part2-fleet-gym-visual-smoke.ts",
    ].map(path => new URL(path, import.meta.url))

    for (const scriptUrl of scriptUrls) {
      const source = await Bun.file(scriptUrl).text()
      expect(source).toContain("installKhalaQaConsoleErrorOracle")
      expect(source).toContain("installKhalaCodeVisualSmokeRpcMocks")
      expect(source).toContain("assertKhalaCodePublicSafe")
    }

    const rpcMocks = await Bun.file(
      new URL("../scripts/visual-smoke-rpc-mocks.ts", import.meta.url),
    ).text()
    expect(rpcMocks).toContain("makeKhalaCodeQaSeedCorpusFixtureFetch")
    expect(rpcMocks).toContain('method === "events"')
    expect(rpcMocks).not.toContain("status: 500")
  })
})
