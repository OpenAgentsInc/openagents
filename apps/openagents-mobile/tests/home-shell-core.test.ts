import { describe, expect, test } from "bun:test"
// effect via the bridge — same effect copy as @effect-native/* (see the core
// module for why); Effect values must unify with the renderer's mount Effect.
import { Effect, Stream } from "@effect-native/core/effect"
import {
  makeReactNativeRenderer,
  renderReactNativeView,
  type ExpoUiSwiftUiRuntime,
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
  renderChromeComposerView,
  renderChromeMenuButtonView,
  renderChromeNewChatView,
  renderChromePillView,
  renderContentView,
  renderDrawerView,
  renderMineralsSheetView,
  renderModeMenuView,
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

  test("drawer view: search/new-chat rows, real Recents section, selection highlight, settings + bundle footer", () => {
    const realRecents = [
      { id: "thread-1", title: "Review the project" },
      { id: "thread-2", title: "Ship mobile parity" },
    ]
    const state = { ...initialHomeState, recents: realRecents, activeRecentId: "thread-1" }
    const serialized = JSON.stringify(renderDrawerView(state))
    expect(serialized).toContain('"label":"Search"')
    expect(serialized).toContain('"label":"New chat"')
    expect(serialized).toContain("Recents")
    for (const recent of realRecents) {
      expect(serialized).toContain(recent.title)
    }
    expect(serialized).toContain('"label":"Settings"')
    expect(serialized).toContain("Bundle 2026-07-10.embedded-114")
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

  test("surface-mode dropdown: labels follow modes; Khala is a real typed surface and Sarah keeps its video layer", async () => {
    // Projection: label swaps with the mode.
    expect(chromeProps({ ...initialHomeState, surfaceMode: "sarah" }).pillLabel).toBe("Sarah")
    expect(chromeProps({ ...initialHomeState, surfaceMode: "khala" }).pillLabel).toBe("Khala")
    expect(surfaceModeOptions.map((option) => option.id)).toEqual(["openagents", "khala", "sarah"])

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
    const khala = JSON.stringify(renderContentView({ ...initialHomeState, surfaceMode: "khala" }))
    expect(khala).toContain('"content":"Khala"')
    expect(khala).toContain("KhalaTurnSubmitted")

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
          program.recents.hydrate([
            { id: "thread-1", title: "Review the project" },
            { id: "thread-2", title: "Ship mobile parity" },
          ])
          yield* settle
          program.chrome.toggleDrawer()
          yield* settle
          // (2) tap a drawer recent row (find its Pressable in the rendered tree).
          const drawerTree = yield* drawer.currentElement
          const row = findByNativeId(
            drawerTree,
            "effect-native:Button:drawer-recent-thread-2",
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
          expect(afterSelect.activeRecentId).toBe("thread-2")
          expect(afterSelect.drawerOpen).toBe(false)
          expect(afterSelect.conversationSource).toBe("recent")
          expect(afterSelect.surfaceMode).toBe("sarah")

          // (3) chrome dispatchers land in state.
          // Return to the non-conversation surface: only there does the
          // presentation-only composer start the ask-video takeover.
          program.chrome.selectSurfaceMode("openagents")
          yield* settle
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
          // video ends ONLY the takeover — the sheet stays open until the
          // user dismisses it (owner P0, build 111 feedback 2026-07-09).
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
          expect(afterEnd.mineralsSheetOpen).toBe(true)
          program.chrome.dismissMineralsSheet()
          yield* settle
          expect((yield* lastState(program)).mineralsSheetOpen).toBe(false)

          yield* content.unmount
          yield* drawer.unmount
        }),
      ),
    )
  })

  // Behavior contract openagents_mobile.minerals_sheet_user_dismiss_only.v1
  // (owner P0, build 111 feedback 2026-07-09): the Buy Minerals sheet's
  // lifecycle is decoupled from playback state ENTIRELY. A video ended/looped
  // playback event must never close the sheet; only the USER closes it —
  // selecting a price pack or "Not now".
  test("minerals sheet survives video end/dismiss; ONLY user intents close it", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const program = buildHomeProgram()

        // Takeover starts (composer tap) and the shell opens the sheet
        // midway through the video.
        program.chrome.pressComposer()
        yield* settle
        program.chrome.openMineralsSheet()
        yield* settle
        expect((yield* lastState(program)).mineralsSheetOpen).toBe(true)

        // (1) PLAYBACK event — playToEnd/loop boundary: the takeover ends,
        // the sheet STAYS OPEN over the resumed surface.
        program.chrome.askVideoEnded()
        yield* settle
        const afterEnded = yield* lastState(program)
        expect(afterEnded.askVideoPlaying).toBe(false)
        expect(afterEnded.mineralsSheetOpen).toBe(true)

        // Ending the video again (spurious repeat playback events) still
        // never touches the sheet.
        program.chrome.askVideoEnded()
        yield* settle
        expect((yield* lastState(program)).mineralsSheetOpen).toBe(true)

        // (2) USER tap-dismisses a replaying video: sheet still stays open.
        program.chrome.pressComposer()
        yield* settle
        program.chrome.dismissAskVideo()
        yield* settle
        const afterTapDismiss = yield* lastState(program)
        expect(afterTapDismiss.askVideoPlaying).toBe(false)
        expect(afterTapDismiss.mineralsSheetOpen).toBe(true)

        // (3) USER intent "Not now" closes it.
        program.chrome.dismissMineralsSheet()
        yield* settle
        expect((yield* lastState(program)).mineralsSheetOpen).toBe(false)

        // (4) USER intent pack selection closes it too.
        program.chrome.openMineralsSheet()
        yield* settle
        program.chrome.selectMineralPack("pack-1200")
        yield* settle
        const afterPack = yield* lastState(program)
        expect(afterPack.mineralsSheetOpen).toBe(false)
        expect(afterPack.lastMineralPackId).toBe("pack-1200")
      }),
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

// ---------------------------------------------------------------------------
// GL-1 (#8647): the glass chrome as typed EN trees through the render-rn
// @expo/ui lowering seam. The app-local openagents-liquid-glass expo-module
// island is DELETED; these oracles pin its replacement: catalog components
// with `surface: "glass"`, lowered INTERNALLY by render-rn (SwiftUI Liquid
// Glass on iOS 26+ via an injected fake runtime here; honest material
// approximation otherwise). App code never imports @expo/ui.
// ---------------------------------------------------------------------------

const fakeExpoUi: ExpoUiSwiftUiRuntime = {
  Host: "ExpoUi.Host",
  HStack: "ExpoUi.HStack",
  VStack: "ExpoUi.VStack",
  Button: "ExpoUi.Button",
  Image: "ExpoUi.Image",
  Text: "ExpoUi.Text",
  Spacer: "ExpoUi.Spacer",
  modifiers: {
    glassEffect: (params) => ({ $type: "glassEffect", ...params }),
    foregroundStyle: (style) => ({ $type: "foregroundStyle", style }),
    frame: (params) => ({ $type: "frame", ...params }),
    padding: (params) => ({ $type: "padding", ...params }),
    disabled: (disabled) => ({ $type: "disabled", disabled }),
  },
}

const ios26Dependencies: ReactNativeDependencies = {
  React: { createElement },
  ReactNative: { ...host, Platform: { OS: "ios", Version: "26.0" } },
}

const findByType = (node: ReactNodeLike, type: unknown): ReactElementLike | undefined => {
  if (!isElement(node)) return undefined
  if (node.type === type) return node
  for (const child of childrenOf(node)) {
    const found = findByType(child, type)
    if (found !== undefined) return found
  }
  return undefined
}

const findAllByType = (node: ReactNodeLike, type: unknown): ReadonlyArray<ReactElementLike> => {
  if (!isElement(node)) return []
  const own = node.type === type ? [node] : []
  return [...own, ...childrenOf(node).flatMap((child) => findAllByType(child, type))]
}

describe("contract openagents_mobile.glass_chrome.en_seam.v1", () => {
  test("the chrome is typed glass catalog data: IconButton/Button/Toolbar with surface glass + intents (no island props)", () => {
    const menuButton = JSON.stringify(renderChromeMenuButtonView(initialHomeState))
    expect(menuButton).toContain('"_tag":"IconButton"')
    expect(menuButton).toContain('"surface":"glass"')
    expect(menuButton).toContain('"icon":"Menu"')
    expect(menuButton).toContain("DrawerToggled")

    const pill = JSON.stringify(renderChromePillView(initialHomeState))
    expect(pill).toContain('"_tag":"Button"')
    expect(pill).toContain('"surface":"glass"')
    expect(pill).toContain('"label":"OpenAgents"')
    expect(pill).toContain("ChatPillPressed")

    const newChat = JSON.stringify(renderChromeNewChatView(initialHomeState))
    expect(newChat).toContain('"icon":"Compose"')
    expect(newChat).toContain("NewChatPressed")

    const composer = JSON.stringify(renderChromeComposerView(initialHomeState))
    expect(composer).toContain('"_tag":"Toolbar"')
    expect(composer).toContain('"surface":"glass"')
    expect(composer).toContain('"label":"Ask anything"')
    expect(composer).toContain("ComposerPressed")
    expect(composer).toContain("MicPressed")

    const sheet = JSON.stringify(renderMineralsSheetView(initialHomeState))
    expect(sheet).toContain('"surface":"glass"')
    expect(sheet).toContain("Buy Minerals")
    expect(sheet).toContain("MineralPackSelected")
    expect(sheet).toContain("MineralsSheetDismissed")
  })

  test("iOS 26 + @expo/ui runtime: the chrome lowers to SwiftUI Liquid Glass and round-trips typed intents through the NEW seam", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const program = buildHomeProgram()

        // Menu icon button: Host > Button(glassEffect circle) > SF Symbol.
        const menuElement = renderReactNativeView(
          renderChromeMenuButtonView(initialHomeState),
          ios26Dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios", expoUi: fakeExpoUi },
        )
        expect(findByType(menuElement, fakeExpoUi.Host)).toBeDefined()
        const menuNative = findByType(menuElement, fakeExpoUi.Button)
        const menuMods = (menuNative?.props.modifiers ?? []) as ReadonlyArray<Record<string, unknown>>
        expect(
          menuMods.some((mod) => mod.$type === "glassEffect" && mod.shape === "circle"),
        ).toBe(true)
        expect(findByType(menuElement, fakeExpoUi.Image)?.props.systemName).toBe(
          "line.3.horizontal",
        )

        // Typed intent round trip: SwiftUI onPress -> DrawerToggled -> state.
        ;(menuNative?.props.onPress as () => void)()
        yield* settle
        expect((yield* lastState(program)).drawerOpen).toBe(true)
        ;(menuNative?.props.onPress as () => void)()
        yield* settle
        expect((yield* lastState(program)).drawerOpen).toBe(false)

        // Composer bar: ONE SwiftUI subtree (HStack under a shared capsule)
        // whose native buttons dispatch the same typed intents the island's
        // EventDispatchers used.
        const composerElement = renderReactNativeView(
          renderChromeComposerView(initialHomeState),
          ios26Dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios", expoUi: fakeExpoUi },
        )
        const stack = findByType(composerElement, fakeExpoUi.HStack)
        expect(stack).toBeDefined()
        const stackMods = (stack?.props.modifiers ?? []) as ReadonlyArray<Record<string, unknown>>
        expect(
          stackMods.some((mod) => mod.$type === "glassEffect" && mod.shape === "capsule"),
        ).toBe(true)
        const buttons = findAllByType(composerElement, fakeExpoUi.Button)
        expect(buttons).toHaveLength(3)
        // plus / ask / mic in order.
        ;(buttons[1]?.props.onPress as () => void)()
        yield* settle
        ;(buttons[2]?.props.onPress as () => void)()
        yield* settle
        const after = yield* lastState(program)
        expect(after.composerTaps).toBe(1)
        expect(after.askVideoPlaying).toBe(true)
        expect(after.micTaps).toBe(1)
      }),
    )
  })

  test("the pill opens the TYPED mode menu; menu selection switches the surface and closes it", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const program = buildHomeProgram()

        // Pill tap (through the SwiftUI lowering) opens the typed menu.
        const pillElement = renderReactNativeView(
          renderChromePillView(initialHomeState),
          ios26Dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios", expoUi: fakeExpoUi },
        )
        const pillNative = findByType(pillElement, fakeExpoUi.Button)
        expect(findByType(pillElement, fakeExpoUi.Text)?.props.children).toBe("OpenAgents")
        ;(pillNative?.props.onPress as () => void)()
        yield* settle
        const open = yield* lastState(program)
        expect(open.modeMenuOpen).toBe(true)
        expect(open.pillTaps).toBe(1)

        // The menu itself is a typed EN DropdownMenu (RN Modal lowering);
        // selecting "Sarah" dispatches SurfaceModeMenuItemSelected.
        const menuElement = renderReactNativeView(
          renderModeMenuView(open),
          dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios" },
        )
        const sarahRow = findByTestId(menuElement, "en-menu-item:sarah")
        ;(sarahRow?.props.onPress as () => void)()
        yield* settle
        const selected = yield* lastState(program)
        expect(selected.surfaceMode).toBe("sarah")
        expect(selected.modeMenuOpen).toBe(false)

        // Backdrop dismissal closes without changing the mode.
        ;(pillNative?.props.onPress as () => void)()
        yield* settle
        const reopened = yield* lastState(program)
        const reopenedMenu = renderReactNativeView(
          renderModeMenuView(reopened),
          dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios" },
        )
        const backdrop = findByTestId(reopenedMenu, "en-dropdown-backdrop")
        ;(backdrop?.props.onPress as () => void)()
        yield* settle
        const dismissed = yield* lastState(program)
        expect(dismissed.modeMenuOpen).toBe(false)
        expect(dismissed.surfaceMode).toBe("sarah")
      }),
    )
  })

  test("without the @expo/ui runtime the chrome renders the HONEST material approximation (never fake glass, never a crash)", () => {
    const element = renderReactNativeView(
      renderChromeMenuButtonView(initialHomeState),
      dependencies,
      (() => Effect.succeed(undefined)) as never,
      { theme: khalaTheme, platform: "ios" },
    )
    expect(element.type).toBe(host.Pressable)
    const style = element.props.style as Record<string, unknown>
    // khalaTheme surface #0b1220 at the documented 0.72 material opacity.
    expect(style.backgroundColor).toBe("rgba(11, 18, 32, 0.72)")
    expect(style.borderWidth).toBe(1)
  })

  test("minerals sheet: typed pack selection through the EN tree closes the sheet and records the pack", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const program = buildHomeProgram()
        program.chrome.openMineralsSheet()
        yield* settle

        const sheetElement = renderReactNativeView(
          renderMineralsSheetView(yield* lastState(program)),
          dependencies,
          program.report,
          { theme: khalaTheme, platform: "ios" },
        )
        const pack = findByNativeId(sheetElement, "effect-native:Button:minerals-pack-550")
        const onPress = pack?.props.onPress
        if (typeof onPress !== "function") {
          throw new Error("expected the pack row to bind onPress")
        }
        onPress()
        yield* settle
        const after = yield* lastState(program)
        expect(after.mineralsSheetOpen).toBe(false)
        expect(after.lastMineralPackId).toBe("pack-550")
      }),
    )
  })
})

const findByTestId = (node: ReactNodeLike, testID: string): ReactElementLike | undefined => {
  if (!isElement(node)) return undefined
  if (node.props.testID === testID) return node
  for (const child of childrenOf(node)) {
    const found = findByTestId(child, testID)
    if (found !== undefined) return found
  }
  return undefined
}
