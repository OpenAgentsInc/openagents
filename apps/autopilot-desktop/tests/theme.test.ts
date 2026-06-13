import { describe, expect, mock, test } from "bun:test"

mock.module("@openagentsinc/autopilot-ui", async () => {
  return await import("../../../packages/autopilot-ui/src/tokens")
})

describe("desktop dark theme CSS", () => {
  test("declares website dark theme variables on root", async () => {
    const [{ cssVars, darkTokens }, { darkThemeStyleCss }] = await Promise.all([
      import("@openagentsinc/autopilot-ui"),
      import("../src/ui/theme"),
    ])

    const css = darkThemeStyleCss()
    const vars = cssVars(darkTokens)

    expect(css).toContain(`--bg:${vars["--bg"]};`)
    expect(css).toContain(`--text:${vars["--text"]};`)
    expect(css).toContain(`--outline:${vars["--outline"]};`)
    expect(css).toContain(`--primary:${vars["--primary"]};`)
  })
})
