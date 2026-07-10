import { describe, expect, test } from "bun:test"
import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"

import { CatalogVersion } from "@effect-native/core"

import { initialHomeState, renderContentView, renderDrawerView } from "../src/screens/home-core"

const appRoot = join(import.meta.dir, "..")

describe("contract openagents_mobile.persona_neutral_catalog.v1", () => {
  test("the pure program stays host-agnostic and contains only Khala conversation state", () => {
    for (const module of ["src/screens/home-core.ts", "src/screens/khala-core.ts"]) {
      const source = readFileSync(join(appRoot, module), "utf8")
      expect(source).not.toContain('from "react"')
      expect(source).not.toContain('from "react-native"')
      expect(source).not.toContain("Sarah")
    }
    expect(JSON.stringify(renderContentView(initialHomeState))).toContain(`"catalogVersion":"${CatalogVersion}"`)
    expect(JSON.stringify(renderDrawerView(initialHomeState))).toContain(`"catalogVersion":"${CatalogVersion}"`)
  })

  test("the iOS SwiftUI island is the sole real composer and forwards typed text/submit events", () => {
    const swift = readFileSync(join(appRoot, "modules/openagents-liquid-glass/ios/OpenAgentsLiquidGlassView.swift"), "utf8")
    const module = readFileSync(join(appRoot, "modules/openagents-liquid-glass/ios/OpenAgentsLiquidGlassModule.swift"), "utf8")
    const shell = readFileSync(join(appRoot, "src/screens/home-screen.tsx"), "utf8")
    expect(swift).toContain("TextField(state.placeholder")
    expect(swift).toContain("onTextChange")
    expect(swift).toContain("onSubmit")
    expect(module).toContain('Events("onTextChange", "onSubmit", "onTapPlus")')
    expect(shell).toContain("<GlassComposer")
    expect(shell).not.toContain("onTapComposer")
    expect(shell).not.toContain("onTapMic")
  })

  test("the mobile package no longer carries Sarah source, tests, or demo media", () => {
    expect(existsSync(join(appRoot, "src/screens/sarah-core.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "src/sarah/sarah-client.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "tests/sarah-surface.test.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "assets/videos/sarah-demo.mp4"))).toBe(false)
    expect(existsSync(join(appRoot, "assets/videos/ask-anything.mp4"))).toBe(false)
  })
})
