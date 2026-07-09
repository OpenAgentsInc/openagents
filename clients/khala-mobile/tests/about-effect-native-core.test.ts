import { describe, expect, test } from "bun:test"
// effect via the bridge — same effect copy as @effect-native/* (see the core
// module for why); Effect values must unify with the renderer's mount Effect.
import { Effect } from "@effect-native/core/effect"
import {
  makeReactNativeRenderer,
  type ReactElementLike,
  type ReactNativeDependencies,
  type ReactNodeLike,
  reactNativeStructure,
} from "@effect-native/render-rn"

import { khalaEffectNativeTheme } from "../src/effect-native/khala-effect-native-theme"
import {
  buildAboutProgram,
  initialAboutState,
  renderAboutView,
} from "../src/screens/about-effect-native-core"

/**
 * EN-3 (#8568) renderer-adapter-#1 contract: the About screen's Effect Native
 * view tree renders through the REAL React Native renderer
 * (`@effect-native/render-rn`) — here driven with a string-typed RN host shim
 * (View/Text/Pressable/...) so it runs deterministically in bun with no native
 * modules, exactly the way the renderer maps to real RN host components on
 * device. Proves: (1) the authored component set renders, (2) the Protoss-blue
 * `khalaEffectNativeTheme` resolves into the tree, (3) a typed intent press
 * flows intent -> handler -> state -> re-render through the adapter.
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

const isElement = (node: ReactNodeLike): node is ReactElementLike =>
  typeof node === "object" && node !== null && "props" in node

const childrenOf = (node: ReactElementLike): ReadonlyArray<ReactNodeLike> => {
  const value = node.props.children
  if (value === undefined || value === null) return []
  return Array.isArray(value) ? (value as ReadonlyArray<ReactNodeLike>) : [value as ReactNodeLike]
}

const findByNativeId = (
  node: ReactNodeLike,
  nativeID: string,
): ReactElementLike | undefined => {
  if (!isElement(node)) return undefined
  if (node.props.nativeID === nativeID) return node
  for (const child of childrenOf(node)) {
    const found = findByNativeId(child, nativeID)
    if (found !== undefined) return found
  }
  return undefined
}

describe("contract khala_mobile.about_effect_native.rn_adapter_render.v1", () => {
  test("pure view renders as a typed Effect Native tree (Stack -> Text/Card/Button)", () => {
    // The authored view is a typed catalog tree, independent of any renderer.
    const view = renderAboutView(initialAboutState)
    expect(view._tag).toBe("Stack")
    const serialized = JSON.stringify(view)
    expect(serialized).toContain('"_tag":"Text"')
    expect(serialized).toContain('"_tag":"Card"')
    expect(serialized).toContain('"_tag":"Button"')
    expect(serialized).toContain("Effect Native")
    // onPress carries a typed intent ref, not an imperative handler.
    expect(serialized).toContain("AboutPinged")
  })

  test("renders through @effect-native/render-rn with the khala theme and dispatches typed intents", async () => {
    const renders: Array<ReactNodeLike | undefined> = []

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const program = buildAboutProgram()
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaEffectNativeTheme,
            platform: "ios",
          })
          const surface = yield* renderer.mount(
            { render: (element) => renders.push(element) },
            program.viewStream,
            program.report,
          )

          // (1) renders through the real RN renderer to the host shim.
          const initial = yield* surface.currentElement
          const structure = reactNativeStructure(initial)
          expect(structure?.tag).toBe("Stack")
          expect(structure?.key).toBe("about-root")
          expect(renders.length).toBeGreaterThan(0)

          // (2) the Protoss-blue theme resolved into the rendered tree: the
          // `accent` color token became the khala accent hex (#4fd0ff).
          expect(JSON.stringify(initial)).toContain(
            khalaEffectNativeTheme.color.accent,
          )
          expect(khalaEffectNativeTheme.color.accent).toBe("#4fd0ff")

          // (3) the typed-intent loop: press dispatches AboutPinged, the
          // handler updates state, the view stream re-renders the count.
          const pingsBefore = findByNativeId(
            initial,
            "effect-native:Text:about-pings-value",
          )
          expect(pingsBefore?.props.children).toBe("0")

          const button = findByNativeId(
            initial,
            "effect-native:Button:about-ping",
          )
          const onPress = button?.props.onPress
          if (typeof onPress !== "function") {
            throw new Error("expected the Effect Native Button to bind onPress")
          }
          onPress()
          yield* nextTask
          yield* Effect.yieldNow

          const pingsAfter = findByNativeId(
            yield* surface.currentElement,
            "effect-native:Text:about-pings-value",
          )
          expect(pingsAfter?.props.children).toBe("1")

          yield* surface.unmount
        }),
      ),
    )
  })
})
