import { describe, expect, test } from "vite-plus/test"

import { DESKTOP_STAGE_LABEL, resolveDesktopStageLabel } from "./branding.ts"

describe("desktop stage branding", () => {
  test("labels dev-server and packaged renderer modes without ambiguity", () => {
    expect(resolveDesktopStageLabel(true)).toBe("Dev")
    expect(resolveDesktopStageLabel(false)).toBe("Alpha")
    expect(DESKTOP_STAGE_LABEL).toBe("Dev")
  })
})
