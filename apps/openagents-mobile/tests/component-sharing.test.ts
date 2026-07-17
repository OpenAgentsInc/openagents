import { describe, expect, test } from "vite-plus/test"
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs"
import { join } from "node:path"

import { CatalogVersion } from "@effect-native/core"

import { initialHomeState, renderContentView, renderDrawerView, renderHomeView } from "../src/screens/home-core"

const appRoot = join(import.meta.dirname, "..")

const sourceFiles = (directory: string): ReadonlyArray<string> =>
  readdirSync(directory).flatMap((entry) => {
    const path = join(directory, entry)
    return statSync(path).isDirectory()
      ? sourceFiles(path)
      : /\.(?:ts|tsx)$/.test(entry) ? [path] : []
  })

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
    expect(JSON.stringify(renderHomeView(initialHomeState))).toContain(`"catalogVersion":"${CatalogVersion}"`)
  })

  test("the Effect Native tree is the sole composer and app code cannot import native UI islands", () => {
    const view = JSON.stringify(renderHomeView(initialHomeState))
    const shell = readFileSync(join(appRoot, "src/screens/home-screen.tsx"), "utf8")
    expect(existsSync(join(appRoot, "modules/openagents-liquid-glass/package.json"))).toBe(false)
    expect(existsSync(join(appRoot, "modules/openagents-liquid-glass/ios/OpenAgentsLiquidGlassModule.swift"))).toBe(false)
    expect(readFileSync(join(appRoot, "package.json"), "utf8")).not.toContain("openagents-liquid-glass")
    expect(view.match(/"_tag":"Composer"/g)?.length).toBe(1)
    expect(view).toContain('"name":"KhalaDraftChanged"')
    expect(view).toContain('"name":"KhalaTurnSubmitted"')
    expect(view).toContain('"_tag":"Toolbar"')
    expect(view).toContain('"surface":"glass"')
    expect(shell).toContain("<EffectNativeHost")
    expect(shell).not.toContain("Pressable")
    expect(shell).not.toContain("TextInput")
    for (const file of sourceFiles(join(appRoot, "src"))) {
      const source = readFileSync(file, "utf8")
      expect(source).not.toContain("openagents-liquid-glass")
      expect(source).not.toMatch(/from ["']@expo\/ui/)
    }
  })

  test("the mobile package no longer carries Sarah source, tests, or demo media", () => {
    expect(existsSync(join(appRoot, "src/screens/sarah-core.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "src/sarah/sarah-client.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "tests/sarah-surface.test.ts"))).toBe(false)
    expect(existsSync(join(appRoot, "assets/videos/sarah-demo.mp4"))).toBe(false)
    expect(existsSync(join(appRoot, "assets/videos/ask-anything.mp4"))).toBe(false)
  })

  test("the vendored RN transcript keeps long messages inside the viewport", () => {
    const renderer = readFileSync(
      join(appRoot, "../openagents.com/packages/effect-native-render-rn/src/index.ts"),
      "utf8",
    )
    expect(renderer).toContain('testID: `en-message-row:${message.key}`')
    expect(renderer).toContain('width: "100%"')
    expect(renderer).toContain('maxWidth: message.role === "user" ? "85%" : "100%"')
    expect(renderer).toContain('backgroundColor: "#0a84ff"')
    expect(renderer).toContain('borderRadius: 20')
    expect(renderer).toContain("minWidth: 0")
    expect(renderer).toContain("flexShrink: 1")
  })

  test("Metro resolves NodeNext .js specifiers to workspace TypeScript sources", () => {
    const metro = readFileSync(join(appRoot, "metro.config.cjs"), "utf8")
    expect(metro).toContain('moduleName.endsWith(".js")')
    expect(metro).toContain('`${withoutJs}.ts`')
    expect(metro).toContain('`${withoutJs}.tsx`')
    expect(metro).toContain("context.resolveRequest")
  })

  test("the host keeps the Effect Native surface above the keyboard and the renderer owns submit blur", () => {
    const shell = readFileSync(join(appRoot, "src/screens/home-screen.tsx"), "utf8")
    const renderer = readFileSync(
      join(appRoot, "../openagents.com/packages/effect-native-render-rn/src/index.ts"),
      "utf8",
    )
    expect(shell).toContain("<KeyboardAvoidingView")
    expect(shell).toContain('Platform.OS === "ios" ? "padding" : "height"')
    expect(shell).not.toContain("Keyboard.dismiss")
    expect(renderer).toContain('submitBehavior: "blurAndSubmit"')
    expect(renderer).toContain('returnKeyType: "send"')
    expect(renderer).toContain('testID: "en-composer-input"')
  })

  test("CUT-16 interaction controls share keyboard, screen-reader, and reduced-motion semantics across native hosts", () => {
    const interaction = readFileSync(join(appRoot, "src/screens/khala-core.ts"), "utf8")
    const rn = readFileSync(
      join(appRoot, "../openagents.com/packages/effect-native-render-rn/src/index.ts"),
      "utf8",
    )
    const dom = readFileSync(
      join(appRoot, "../openagents.com/packages/effect-native-render-dom/src/index.ts"),
      "utf8",
    )
    // Product code stays on one semantic Button primitive: no hidden native
    // island or bespoke animation can diverge question/approval behavior.
    expect(interaction).toContain('Button({')
    expect(interaction).not.toContain("Pressable")
    expect(interaction).not.toContain("Touchable")
    expect(interaction).not.toContain("Animated")
    // React Native exposes a real screen-reader button and disabled state;
    // its Text child is the spoken visible label.
    expect(rn).toContain('accessibilityRole: "button"')
    expect(rn).toContain('accessibilityState: { disabled: view.disabled === true }')
    expect(rn).toContain('view.label')
    // Desktop lowers the same primitive to a native keyboard-operable button.
    expect(dom).toContain('state.keyedElement(view, "button")')
    expect(dom).toContain('element.textContent = view.label')
    expect(dom).toContain('element.disabled = view.disabled === true')
    // Interaction cards themselves animate nothing. Any surrounding DOM
    // motion is globally collapsed when the OS requests reduced motion.
    expect(dom).toContain('@media (prefers-reduced-motion: reduce)')
  })
})
