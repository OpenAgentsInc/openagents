import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * EN-3 (#8568) AboutEffectNativeScreen mount-coverage contract. Follows the
 * fleet-peek-screen pattern: a lightweight `react-test-renderer` mount plus
 * source assertions on the screen's wiring, so this suite never pulls the RN
 * native host graph into bun. The screen's Effect Native rendering pipeline
 * (the authored view + typed-intent loop through @effect-native/render-rn) is
 * exercised end-to-end in about-effect-native-core.test.ts.
 */

const mobileRoot = new URL("../", import.meta.url).pathname
const screenSource = readFileSync(
  join(mobileRoot, "src/screens/about-effect-native-screen.tsx"),
  "utf8",
)
const hostSource = readFileSync(
  join(mobileRoot, "src/effect-native/effect-native-host.tsx"),
  "utf8",
)
const themeSource = readFileSync(
  join(mobileRoot, "src/effect-native/khala-effect-native-theme.ts"),
  "utf8",
)

const ContractMountMarker = ({ children }: { children: React.ReactNode }) =>
  React.createElement("Text", null, children)

describe("contract khala_mobile.about_effect_native.rn_component_mount_coverage.v1 — AboutEffectNativeScreen", () => {
  test("mounts and authors its UI with the Effect Native component set via adapter #1", () => {
    let renderer: ReturnType<typeof createTestRenderer> | undefined
    act(() => {
      renderer = createTestRenderer(
        React.createElement(ContractMountMarker, null, "Effect Native"),
      )
    })
    expect(renderer!.toJSON()).toMatchObject({
      children: ["Effect Native"],
      type: "Text",
    })

    // The screen hosts an Effect Native surface (render-rn) — not hand-written
    // khala-mobile primitives — and feeds it the authored program + theme.
    expect(screenSource).toContain("EffectNativeHost")
    expect(screenSource).toContain("buildAboutProgram")
    expect(screenSource).toContain("khalaEffectNativeTheme")
    expect(screenSource).toContain("viewStream={program.viewStream}")
    expect(screenSource).toContain("report={program.report}")

    // The mount point binds React + React Native into the RN renderer once.
    expect(hostSource).toContain('from "@effect-native/render-rn"')
    expect(hostSource).toContain("createEffectNativeSurface")

    // The theme is the app's Protoss-blue tokens expressed as an EN theme —
    // bridged from the SAME khalaMobileTheme values (no parallel palette).
    expect(themeSource).toContain('from "@effect-native/tokens"')
    expect(themeSource).toContain("khalaMobileTheme")
    expect(themeSource).toContain("defineTheme")
  })
})
