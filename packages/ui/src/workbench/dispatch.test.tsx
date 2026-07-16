import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vite-plus/test"

import { dispatchWorkbenchItem, type WorkbenchMeterDispatchItem } from "./dispatch.tsx"

/**
 * Scope: this file only exercises the "meter" branch (T11 #8868). Every
 * other `dispatchWorkbenchItem` branch is owned by its own Wave-2 lane
 * (#8861-8867, #8869) and is intentionally NOT covered here to avoid two
 * lanes racing on the same test file.
 */
describe("dispatchWorkbenchItem meter branch (#8868, T11)", () => {
  it("renders a historical ContextMeter snapshot with the item's exact token fields", () => {
    const item: WorkbenchMeterDispatchItem = {
      kind: "meter",
      source: "codex",
      inputTokens: 100,
      cachedInputTokens: 20,
      outputTokens: 30,
      reasoningTokens: 5,
      totalTokens: 155,
    }
    const html = renderToStaticMarkup(dispatchWorkbenchItem(item, { itemKey: "meter-1" }))
    expect(html).toContain('data-timeline-key="meter-1"')
    expect(html).toContain('data-historical="true"')
    expect(html).toContain("155")
    expect(html).toContain("100")
    expect(html).toContain("20")
    expect(html).toContain("30")
  })

  it("renders the honest NO DATA state for a meter item with no token fields", () => {
    const item: WorkbenchMeterDispatchItem = { kind: "meter", source: "claude" }
    const html = renderToStaticMarkup(dispatchWorkbenchItem(item, { itemKey: "meter-empty" }))
    expect(html).toContain("NO DATA")
  })
})
