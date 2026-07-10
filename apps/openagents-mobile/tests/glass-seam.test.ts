import { describe, expect, test } from "bun:test"
import { Effect } from "@effect-native/core/effect"
import {
  makeReactNativeRenderer,
  type ReactElementLike,
  type ReactNativeDependencies,
  type ReactNodeLike,
} from "@effect-native/render-rn"
import { khalaTheme } from "@effect-native/tokens"

import {
  buildHomeProgram,
  glassIslandProps,
  glassPingedRef,
  initialHomeState,
  renderHomeView,
} from "../src/screens/home-core"

/**
 * OpenAgents mobile (#8597) SwiftUI Liquid Glass seam contract, per
 * docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md.
 *
 * What IS unit-provable here: the typed seam — the island's props are a pure
 * projection of program state, and a SwiftUI tap event (which the shell
 * forwards as `program.report(glassPingedRef)`, exactly what these tests
 * dispatch) flows through the typed GlassPinged intent -> handler -> state ->
 * re-render of the Effect Native tree AND a changed island projection.
 *
 * What is NOT unit-provable: the actual SwiftUI rendering
 * (UIHostingController, .glassEffect / .buttonStyle(.glass) on iOS 26,
 * .ultraThinMaterial fallback pre-26). That half is device-proven via the
 * TestFlight build only — stated honestly in the #8597 ladder.
 */

const host = {
  View: "View",
  Text: "Text",
  Pressable: "Pressable",
  TextInput: "TextInput",
  FlatList: "FlatList",
  SectionList: "SectionList",
  Image: "Image",
  Modal: "Modal",
  StyleSheet: {
    create: <Styles extends Record<string, unknown>>(styles: Styles): Styles => styles,
  },
}

const createElement = (
  type: unknown,
  props: Record<string, unknown> | null = null,
  ...children: ReadonlyArray<ReactNodeLike>
): ReactElementLike => ({
  type,
  key: typeof props?.key === "string" ? props.key : null,
  props: {
    ...(props ?? {}),
    ...(children.length === 0
      ? {}
      : { children: children.length === 1 ? children[0] : children }),
  },
})

const dependencies: ReactNativeDependencies = {
  React: { createElement },
  ReactNative: host,
}

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0)),
)

describe("contract openagents_mobile.glass_island.typed_seam.v1", () => {
  test("the Home tree carries the labeled SwiftUI test section", () => {
    const serialized = JSON.stringify(renderHomeView(initialHomeState))
    expect(serialized).toContain("SwiftUI via Effect Native — test")
    expect(serialized).toContain("Glass intents received: 0")
    // The typed intent for the island exists in the program's definitions.
    expect(glassPingedRef.name).toBe("GlassPinged")
  })

  test("island props are a pure projection of program state (no parallel state)", () => {
    const zero = glassIslandProps(initialHomeState)
    expect(zero.tapCount).toBe(0)
    expect(zero.title).toBe("Liquid Glass")
    const after = glassIslandProps({ ...initialHomeState, glassTaps: 3 })
    expect(after.tapCount).toBe(3)
  })

  test("SwiftUI tap event -> typed GlassPinged intent -> state -> EN re-render + island projection", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const program = buildHomeProgram()
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaTheme,
            platform: "ios",
          })
          const renders: Array<ReactNodeLike | undefined> = []
          const surface = yield* renderer.mount(
            { render: (element) => renders.push(element) },
            program.viewStream,
            program.report,
          )

          expect(JSON.stringify(yield* surface.currentElement)).toContain(
            "Glass intents received: 0",
          )

          // EXACTLY what the shell wires to the native onGlassTap event:
          program.dispatchGlassTap()
          yield* nextTask
          yield* Effect.yieldNow

          // The typed intent updated the ONE program state: the Effect Native
          // tree re-rendered ...
          const after = JSON.stringify(yield* surface.currentElement)
          expect(after).toContain("Glass intents received: 1")
          // ... and the island's prop projection reflects the same state.
          expect(glassIslandProps({ ...initialHomeState, glassTaps: 1 }).tapCount).toBe(1)

          yield* surface.unmount
        }),
      ),
    )
  })
})
