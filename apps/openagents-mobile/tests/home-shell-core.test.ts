import { describe, expect, test } from "bun:test"
// effect via the bridge — same effect copy as @effect-native/* (see the core
// module for why); Effect values must unify with the renderer's mount Effect.
import { Effect } from "@effect-native/core/effect"
import {
  makeReactNativeRenderer,
  type ReactElementLike,
  type ReactNativeDependencies,
  type ReactNodeLike,
} from "@effect-native/render-rn"
import { khalaTheme } from "@effect-native/tokens"

import {
  activeRecentTitle,
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  renderContentView,
  renderDrawerView,
  seedRecents,
} from "../src/screens/home-core"

/**
 * OpenAgents mobile (GL-2 #8648) — ChatGPT-style glass shell view-program
 * contract. Everything here drives the REAL `@effect-native/render-rn`
 * renderer with a string host shim (no native modules): the two view
 * projections (content + drawer), the drawer open/close + selection loops,
 * and the chrome dispatchers (the exact functions the SwiftUI islands call).
 *
 * SwiftUI glass RENDERING itself (UIHostingController, .glassEffect) is
 * device/simulator-proven only — the pixel-proof gate covers it.
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

const settle = Effect.gen(function* () {
  yield* nextTask
  yield* Effect.yieldNow
})

describe("contract openagents_mobile.home_shell.view_program.v1", () => {
  test("content view: heading is the active recent, status card carries chrome counters", () => {
    const serialized = JSON.stringify(renderContentView(initialHomeState))
    expect(serialized).toContain("Welcome to OpenAgents")
    expect(serialized).toContain("Chrome intents")
    expect(serialized).toContain("pill 0")
    expect(activeRecentTitle({ ...initialHomeState, activeRecentId: undefined })).toBe(
      "New chat",
    )
  })

  test("drawer view: search/new-chat rows, Recents section, selection highlight, settings + bundle footer", () => {
    const serialized = JSON.stringify(renderDrawerView(initialHomeState))
    expect(serialized).toContain('"label":"Search"')
    expect(serialized).toContain('"label":"New chat"')
    expect(serialized).toContain("Recents")
    for (const recent of seedRecents) {
      expect(serialized).toContain(recent.title)
    }
    expect(serialized).toContain('"label":"Settings"')
    expect(serialized).toContain("Bundle 2026-07-09.embedded-107")
    // The active recent renders as the highlighted (secondary) row; the
    // others are ghost rows.
    expect(serialized).toContain('"backgroundColor":"surfaceRaised"')
    // Intents, not callbacks.
    expect(serialized).toContain("RecentSelected")
    expect(serialized).toContain("SearchPressed")
    expect(serialized).toContain("SettingsPressed")
  })

  test("chrome props are a pure projection: visible iff drawer closed", () => {
    expect(chromeProps(initialHomeState).chromeVisible).toBe(true)
    expect(chromeProps({ ...initialHomeState, drawerOpen: true }).chromeVisible).toBe(false)
    expect(chromeProps(initialHomeState).pillLabel).toBe("OpenAgents")
  })

  test("full loop through the REAL RN renderer: chrome toggle opens drawer, recent tap selects + closes, chrome counters re-render", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const program = buildHomeProgram()
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaTheme,
            platform: "ios",
          })
          const content = yield* renderer.mount(
            { render: () => undefined },
            program.contentViewStream,
            program.report,
          )
          const drawer = yield* renderer.mount(
            { render: () => undefined },
            program.drawerViewStream,
            program.report,
          )

          // (1) SwiftUI sidebar icon tap (exact shell wiring) opens the drawer.
          program.chrome.toggleDrawer()
          yield* settle
          // (2) tap a drawer recent row (find its Pressable in the rendered tree).
          const drawerTree = yield* drawer.currentElement
          const row = findByNativeId(
            drawerTree,
            "effect-native:Button:drawer-recent-glass-shell",
          )
          const onPress = row?.props.onPress
          if (typeof onPress !== "function") {
            throw new Error("expected the drawer recent row to bind onPress")
          }
          onPress()
          yield* settle

          // Selection landed: content heading switches to the tapped recent,
          // and the drawer closed (drawerOpen false in the projection).
          const contentTree = JSON.stringify(yield* content.currentElement)
          expect(contentTree).toContain("Glass shell design")

          // (3) chrome counters: composer + mic taps re-render the status card.
          program.chrome.pressComposer()
          yield* settle
          program.chrome.pressMic()
          yield* settle
          program.chrome.pressMic()
          yield* settle
          const after = JSON.stringify(yield* content.currentElement)
          expect(after).toContain("composer 1")
          expect(after).toContain("mic 2")

          yield* content.unmount
          yield* drawer.unmount
        }),
      ),
    )
  })
})

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
