import { describe, expect, test } from "bun:test"
import { createRequire } from "node:module"

import { openAgentsNativeWindTokens } from "@openagentsinc/ui/react"

import { createMobileKhalaSyncPreviewState } from "../src/sync/khala-sync-mobile"
import { khalaMobileTokens } from "../src/theme/tokens"

const require = createRequire(import.meta.url)
const tailwindConfig = require("../tailwind.config.cjs") as {
  presets: ReadonlyArray<unknown>
  theme: {
    extend: {
      borderRadius: { xl: string }
      colors: { bg: string }
    }
  }
}

describe("Khala mobile tokens and TS-3 read models", () => {
  test("uses the shared NativeWind token export", () => {
    expect(khalaMobileTokens.colors.accent).toBe(openAgentsNativeWindTokens.colors.accent)
    expect(openAgentsNativeWindTokens.colors.bg).toBe("#000")
    expect(khalaMobileTokens.colors.bg).toBe("#02060d")
    expect(tailwindConfig.theme.extend.colors.bg).toBe(khalaMobileTokens.colors.bg)
    expect(tailwindConfig.theme.extend.borderRadius.xl).toBe("8px")
    expect(tailwindConfig.presets).toHaveLength(1)
  })

  test("renders preview state from Khala Sync entity codecs", () => {
    const state = createMobileKhalaSyncPreviewState()

    expect(state.chatThreads.map(thread => thread.threadId)).toEqual([
      "thread.preview.operator",
      "thread.preview.fleet"
    ])
    expect(state.fleetRun.counters.workUnitsTotal).toBe(21)
    expect(state.fleetRun.runId).toBe("fleet.preview.ts8")
  })
})
