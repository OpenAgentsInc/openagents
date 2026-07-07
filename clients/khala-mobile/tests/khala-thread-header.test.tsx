import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * Real React Native component-mount tests for `KhalaThreadHeader`, plus
 * source-level wiring checks, covering the two "you're not trapped" behavior
 * contracts filed from the owner's 2026-07-06 thread-view report ("just the
 * one message and button shows stop, cant do anything, also no way to start a
 * new thread"):
 *
 * - khala_mobile.thread.new_thread_action_always_reachable.v1
 * - khala_mobile.thread.active_turn_never_traps_user.v1
 *
 * `TouchableFeedback` is stood in exactly as the other component-mount tests
 * in this package do (`tests/khala-ui-primitives.test.tsx`,
 * `tests/chat-composer.test.tsx`): the real one is a
 * `react-native-gesture-handler` + Reanimated press cross-fade with no
 * meaning under `bun test`. The stand-in forwards
 * `accessibilityLabel`/`onPress`/`disabled` straight through, so the header's
 * own button-wiring is still exercised for real.
 */
mock.module("../src/components/touchable-feedback", () => ({
  TouchableFeedback: ({
    accessibilityLabel,
    accessibilityRole,
    accessibilityState,
    children,
    disabled,
    onPress,
    testID,
  }: {
    accessibilityLabel?: string
    accessibilityRole?: "button" | "link" | "none"
    accessibilityState?: Record<string, unknown>
    children?: React.ReactNode
    disabled?: boolean
    onPress?: () => void
    testID?: string
  }) =>
    React.createElement(
      "TouchableFeedback",
      {
        accessibilityLabel,
        accessibilityRole,
        accessibilityState: { ...accessibilityState, disabled },
        onPress: disabled ? undefined : onPress,
        testID,
      },
      children,
    ),
}))

// `../src/theme/typography` (pulled in transitively by `KhalaText`) imports
// `@expo-google-fonts/*`, whose barrel reaches `expo-font` -> `expo-modules-core`,
// which dereferences `globalThis.expo.EventEmitter` at module-evaluation time —
// dead outside a native host. Same stand-in the composer mount test uses; no
// real font rendering is needed for this header-structure test.
mock.module("../src/theme/typography", () => ({
  khalaMobileFontsToLoad: {},
  khalaMobileTextSizes: {
    lg: { fontSize: 20, lineHeight: 32 },
    md: { fontSize: 18, lineHeight: 26 },
    sm: { fontSize: 16, lineHeight: 24 },
    xl: { fontSize: 24, lineHeight: 34 },
    xs: { fontSize: 14, lineHeight: 21 },
    xxl: { fontSize: 36, lineHeight: 44 },
    xxs: { fontSize: 12, lineHeight: 18 },
  },
  khalaMobileTypography: {
    code: { bold: "test-mono-bold", normal: "test-mono" },
    display: "test-display",
    primary: {
      bold: "test-sans-bold",
      light: "test-sans-light",
      medium: "test-sans-medium",
      normal: "test-sans",
      semiBold: "test-sans-semibold",
    },
  },
}))

const { KhalaThreadHeader } = await import("../src/components/khala-thread-header")

const repoPath = (ref: string): string => new URL(`../../../${ref}`, import.meta.url).pathname

type AnyNode = { type: unknown; props: Record<string, unknown> }

const findButton = (root: { findAll: (predicate: (node: AnyNode) => boolean) => AnyNode[] }, label: string) =>
  root.findAll(node => typeof node.type === "string" && node.props?.accessibilityLabel === label)

const mount = async (element: React.ReactElement) => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(element)
    await Promise.resolve()
  })
  return renderer!
}

// Oracle for khala_mobile.thread.new_thread_action_always_reachable.v1
describe("contract khala_mobile.thread.new_thread_action_always_reachable.v1 — KhalaThreadHeader new-thread affordance", () => {
  test("new_thread_button_present_and_calls_handler.unit — the New thread button renders enabled and fires its handler on press", async () => {
    const onNewThread = mock(() => undefined)
    const renderer = await mount(
      React.createElement(KhalaThreadHeader, {
        onOpenMenu: () => undefined,
        onNewThread,
        subtitle: "work · Khala Mobile",
        title: "Thread",
      }),
    )

    const buttons = findButton(renderer.root as never, "New thread")
    expect(buttons.length).toBe(1)
    const onPress = buttons[0]!.props.onPress as (() => void) | undefined
    expect(typeof onPress).toBe("function")
    act(() => onPress?.())
    expect(onNewThread).toHaveBeenCalledTimes(1)
  })

  test("new_thread_button_disabled_without_runtime.unit — with no handler the button still renders (never hidden) but is disabled", async () => {
    const renderer = await mount(
      React.createElement(KhalaThreadHeader, {
        onOpenMenu: () => undefined,
        subtitle: "work · Khala Mobile",
        title: "Thread",
      }),
    )

    const buttons = findButton(renderer.root as never, "New thread")
    // Affordance is always present so it never disappears mid-session…
    expect(buttons.length).toBe(1)
    // …but non-functional until the sync runtime is ready.
    expect(buttons[0]!.props.onPress).toBeUndefined()
    expect((buttons[0]!.props.accessibilityState as { disabled?: boolean }).disabled).toBe(true)
  })

  test("thread_screen_wires_new_thread_action.source — the thread view passes onNewThread and creates+navigates to a fresh thread", async () => {
    const source = await Bun.file(repoPath("clients/khala-mobile/src/screens/thread-messages-screen.tsx")).text()
    expect(source).toContain("onNewThread=")
    expect(source).toContain("startNewThread")
    expect(source).toContain("runtime.createThread")
    expect(source).toContain('navigation.replace("ThreadMessages"')
  })
})

// Oracle for khala_mobile.thread.active_turn_never_traps_user.v1
describe("contract khala_mobile.thread.active_turn_never_traps_user.v1 — escape hatches survive an in-flight turn", () => {
  test("header_escape_hatches_always_render.unit — Open menu and New thread both render, independent of any turn state", async () => {
    const renderer = await mount(
      React.createElement(KhalaThreadHeader, {
        onOpenMenu: () => undefined,
        onNewThread: () => undefined,
        subtitle: "work · Khala Mobile",
        title: "Thread",
      }),
    )

    // The header takes NO turn/props gating, so both ways out of a thread are
    // structurally unconditional — a running turn can never hide them.
    expect(findButton(renderer.root as never, "Open menu").length).toBe(1)
    expect(findButton(renderer.root as never, "New thread").length).toBe(1)
  })

  test("stop_interrupts_and_composer_reverts_when_idle.source — Stop dispatches an interrupt, and an idle composer is editable/Send", async () => {
    const composer = await Bun.file(repoPath("clients/khala-mobile/src/components/chat-composer.tsx")).text()
    // Stop actually cancels the turn (not just a visual no-op).
    expect(composer).toContain("buildInterruptTurnIntentArgs")
    expect(composer).toContain("runtime.interruptTurn")
    // The header (escape hatch) is rendered above the only turn-gated element,
    // and the composer flips back to an editable Send once no turn is active
    // (transition proven live by khala_mobile.composer.rn_component_mount_coverage.v1).
    expect(composer).toContain('accessibilityLabel="Send"')
    const screen = await Bun.file(repoPath("clients/khala-mobile/src/screens/thread-messages-screen.tsx")).text()
    expect(screen).toContain("KhalaThreadHeader")
  })
})

// Oracle for khala_mobile.thread.header_menu_opens_drawer.v1
describe("contract khala_mobile.thread.header_menu_opens_drawer.v1 — chat header left button opens the drawer", () => {
  test("menu_button_present_and_calls_handler.unit — the header renders a single 'Open menu' hamburger that fires onOpenMenu, and no 'Back' button", async () => {
    const onOpenMenu = mock(() => undefined)
    const renderer = await mount(
      React.createElement(KhalaThreadHeader, {
        onOpenMenu,
        onNewThread: () => undefined,
        subtitle: "work · Khala Mobile",
        title: "Thread",
      }),
    )

    // The old broken back chevron is gone; the left action is the hamburger.
    expect(findButton(renderer.root as never, "Back").length).toBe(0)
    const menuButtons = findButton(renderer.root as never, "Open menu")
    expect(menuButtons.length).toBe(1)
    const onPress = menuButtons[0]!.props.onPress as (() => void) | undefined
    expect(typeof onPress).toBe("function")
    act(() => onPress?.())
    expect(onOpenMenu).toHaveBeenCalledTimes(1)
  })

  test("thread_screen_wires_menu_to_open_drawer.source — the thread screen opens the root Drawer from the header hamburger", async () => {
    const source = await Bun.file(repoPath("clients/khala-mobile/src/screens/thread-messages-screen.tsx")).text()
    expect(source).toContain("onOpenMenu=")
    expect(source).toContain("openDrawer()")
    // The thread view is a native-stack screen inside the Drawer, so the
    // drawer is reached one level up via getParent().
    expect(source).toContain("getParent")
  })
})
