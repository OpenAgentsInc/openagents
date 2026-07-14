import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { Effect, Stream } from "@effect-native/core/effect"

import {
  buildHomeProgram,
  initialHomeState,
  normalizeMobileAccessibilityProfile,
  renderContentView,
  renderDrawerView,
  renderHomeView,
} from "../src/screens/home-core"

const appRoot = join(import.meta.dirname, "..")

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

const collectNodes = (value: unknown): ReadonlyArray<Record<string, any>> => {
  if (value === null || typeof value !== "object") return []
  const current = value as Record<string, any>
  return [
    ...(typeof current._tag === "string" ? [current] : []),
    ...Object.values(current).flatMap(collectNodes),
  ]
}

describe("contract openagents_mobile.seam.accessibility_core_flows.v1", () => {
  test("normalizes platform font scale and reduced-motion into bounded view state", () => {
    expect(normalizeMobileAccessibilityProfile({
      fontScale: 1.58,
      reduceMotion: true,
    })).toEqual({
      reduceMotion: true,
      fontScale: 1.58,
      textScale: "extra_large",
      minTouchTarget: 56,
    })
    expect(normalizeMobileAccessibilityProfile({ fontScale: 0.1 })).toMatchObject({
      fontScale: 0.85,
      textScale: "normal",
      minTouchTarget: 44,
    })
    expect(normalizeMobileAccessibilityProfile({ fontScale: 1.3 })).toMatchObject({
      textScale: "large",
      minTouchTarget: 52,
    })
  })

  test("mobile chrome and drawer controls keep minimum touch targets under large dynamic type", () => {
    const accessibility = normalizeMobileAccessibilityProfile({ fontScale: 1.3 })
    const state = { ...initialHomeState, accessibility, drawerOpen: true }
    const homeNodes = collectNodes(renderHomeView(state))
    const drawerNodes = collectNodes(renderDrawerView(state))
    const toolbar = homeNodes.find(node => node.key === "home-toolbar")
    const controls = [...homeNodes, ...drawerNodes].filter(node =>
      node.key === "home-navigation" ||
      node.key === "home-surface-mode" ||
      node.key === "home-new-chat" ||
      node.key === "drawer-new-chat" ||
      node.key === "drawer-khala" ||
      node.key === "drawer-settings")

    expect(toolbar?.style?.minHeight).toBe(52)
    expect(controls.length).toBeGreaterThanOrEqual(6)
    for (const control of controls) {
      expect(control.style?.minHeight).toBeGreaterThanOrEqual(52)
      expect(control.style?.minWidth).toBeGreaterThanOrEqual(52)
    }
  })

  test("conversation, transcript, composer, and runtime actions project reduced motion without visible copy", () => {
    const accessibility = normalizeMobileAccessibilityProfile({
      reduceMotion: true,
      fontScale: 1.7,
    })
    const state = {
      ...initialHomeState,
      accessibility,
      khala: {
        ...initialHomeState.khala,
        interactionActionsAvailable: true,
        entries: [{
          key: "question-1",
          role: "system" as const,
          text: "Choose a branch",
          status: "done" as const,
          interaction: {
            kind: "provider_question" as const,
            interactionRef: "interaction.mobile.a11y",
            turnRef: "turn.mobile.a11y",
            status: "pending" as const,
            title: "Choose branch",
            prompt: "Pick the branch to continue.",
            questions: [{
              questionRef: "question.branch",
              displayText: "Branch",
              multiSelect: false,
              options: [{ optionRef: "main", label: "main" }],
            }],
          },
        }],
      },
    }
    const nodes = collectNodes(renderContentView(state))
    const surface = nodes.find(node => node.key === "khala-surface")
    const transcript = nodes.find(node => node.key === "khala-transcript")
    const composer = nodes.find(node => node.key === "khala-composer")
    const option = nodes.find(node => node.key === "question-1-question.branch-main")

    expect(surface?.a11y?.label).toContain("reduced motion on")
    expect(transcript?.a11y).toEqual({
      role: "list",
      label: "Conversation transcript, reduced motion on",
    })
    expect(composer?.style?.minHeight).toBe(56)
    expect(option?.style?.minHeight).toBe(56)
    expect(JSON.stringify(renderContentView(state))).not.toContain("Reduced motion is enabled")
  })

  test("the live Home program accepts host accessibility updates without rebuilding auth or Sync state", async () => {
    const program = buildHomeProgram()
    const accessibility = normalizeMobileAccessibilityProfile({
      reduceMotion: true,
      fontScale: 1.31,
    })
    program.accessibility.setProfile(accessibility)
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))

    expect(state.accessibility).toEqual(accessibility)
    expect(JSON.stringify(renderHomeView(state))).toContain("large text scale")
    expect(state.conversationAuthority).toBe("local")
    expect(state.syncPhase).toBe("unconfigured")
  })

  test("the React Native host reads OS font scale and reduced-motion, while app code avoids bespoke animation", () => {
    const host = readFileSync(join(appRoot, "src/screens/home-screen.tsx"), "utf8")
    const khala = readFileSync(join(appRoot, "src/screens/khala-core.ts"), "utf8")

    expect(host).toContain("useWindowDimensions")
    expect(host).toContain("AccessibilityInfo.isReduceMotionEnabled")
    expect(host).toContain('"reduceMotionChanged"')
    expect(host).toContain("program.accessibility.setProfile")
    expect(host).not.toContain("Animated")
    expect(khala).not.toContain("Animated")
  })
})
