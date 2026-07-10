import { describe, expect, test } from "bun:test"
// effect via the bridge — same effect copy as @effect-native/* (see the core
// module for why); Effect values must unify with the renderer's mount Effect.
import { Effect, Stream } from "@effect-native/core/effect"
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
  surfaceModeOptions,
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

// Current program state via the changes stream (emits the current value on
// subscribe) — the main surface is deliberately text-free, so loop tests
// assert STATE rather than rendered text.
const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") {
      throw new Error("expected a current state value")
    }
    return option.value
  })

describe("contract openagents_mobile.home_shell.view_program.v1", () => {
  test("content view is a clean surface: NO status text on the main surface (owner direction 2026-07-09)", () => {
    const serialized = JSON.stringify(renderContentView(initialHomeState))
    expect(serialized).not.toContain("Chrome intents")
    expect(serialized).not.toContain("Welcome")
    expect(serialized).not.toContain("Typed Effect Native")
    // The surface itself remains: opaque Protoss background by default.
    expect(serialized).toContain('"backgroundColor":"background"')
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
    expect(serialized).toContain("Bundle 2026-07-09.embedded-111")
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

  test("surface-mode dropdown: pill label follows the mode; sarah mode makes the content root transparent for the video layer", async () => {
    // Projection: label swaps with the mode.
    expect(chromeProps({ ...initialHomeState, surfaceMode: "sarah" }).pillLabel).toBe("Sarah")
    expect(surfaceModeOptions.map((option) => option.id)).toEqual(["openagents", "sarah"])

    // openagents mode: opaque Protoss background; sarah mode: NO background
    // (the shell's fullscreen video shows through — glass-over-video depth).
    const opaque = JSON.stringify(renderContentView(initialHomeState))
    expect(opaque).toContain('"backgroundColor":"background"')
    const transparent = JSON.stringify(
      renderContentView({ ...initialHomeState, surfaceMode: "sarah" }),
    )
    expect(transparent).not.toContain('"backgroundColor":"background"')
    // The ask-video takeover ALSO clears the surface so the video (a shell
    // layer below, under the chrome) shows through.
    const askTransparent = JSON.stringify(
      renderContentView({ ...initialHomeState, askVideoPlaying: true }),
    )
    expect(askTransparent).not.toContain('"backgroundColor":"background"')

    // Typed round-trip through the REAL renderer: the SwiftUI menu selection
    // (exact shell wiring: chrome.selectSurfaceMode) re-renders the tree.
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const program = buildHomeProgram()
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaTheme,
            platform: "ios",
          })
          const surface = yield* renderer.mount(
            { render: () => undefined },
            program.contentViewStream,
            program.report,
          )
          expect(JSON.stringify(yield* surface.currentElement)).toContain(
            khalaTheme.color.background,
          )
          program.chrome.selectSurfaceMode("sarah")
          yield* settle
          const after = JSON.stringify(yield* surface.currentElement)
          expect(after).not.toContain(`"backgroundColor":"${khalaTheme.color.background}"`)
          program.chrome.selectSurfaceMode("openagents")
          yield* settle
          expect(JSON.stringify(yield* surface.currentElement)).toContain(
            khalaTheme.color.background,
          )
          yield* surface.unmount
        }),
      ),
    )
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

          // Selection landed in STATE (the main surface is deliberately
          // text-free): active recent switched and the drawer closed.
          const afterSelect = yield* lastState(program)
          expect(afterSelect.activeRecentId).toBe("glass-shell")
          expect(afterSelect.drawerOpen).toBe(false)

          // (3) chrome dispatchers land in state.
          program.chrome.pressComposer()
          yield* settle
          program.chrome.pressMic()
          yield* settle
          program.chrome.pressMic()
          yield* settle
          const afterTaps = yield* lastState(program)
          expect(afterTaps.composerTaps).toBe(1)
          expect(afterTaps.micTaps).toBe(2)
          // Composer tap also starts the ask-video takeover (audio-on reply
          // video, owner direction); dismissal is a typed intent.
          expect(afterTaps.askVideoPlaying).toBe(true)

          // Minerals fly-up (owner direction): shell opens it midway through
          // the video; selecting a pack (or Not now) closes it; ending the
          // video closes BOTH so the original surface resumes.
          program.chrome.openMineralsSheet()
          yield* settle
          expect((yield* lastState(program)).mineralsSheetOpen).toBe(true)
          program.chrome.selectMineralPack("pack-550")
          yield* settle
          const afterPack = yield* lastState(program)
          expect(afterPack.mineralsSheetOpen).toBe(false)
          expect(afterPack.lastMineralPackId).toBe("pack-550")
          program.chrome.openMineralsSheet()
          yield* settle
          program.chrome.dismissAskVideo()
          yield* settle
          const afterEnd = yield* lastState(program)
          expect(afterEnd.askVideoPlaying).toBe(false)
          expect(afterEnd.mineralsSheetOpen).toBe(false)

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
