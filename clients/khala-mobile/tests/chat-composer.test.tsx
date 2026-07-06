import { beforeEach, describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

import type { RuntimeTurnEntity } from "@openagentsinc/khala-sync"

/**
 * Real React Native component-mount test for `ChatComposer` — the #1
 * prioritized follow-up from the 2026-07-05 mobile QA swarm audit
 * (`docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md`): every other
 * composer test in this package exercises only the pure intent-builder
 * functions, never the actual React component. `react-test-renderer` was a
 * listed devDependency with zero real usage before this file.
 *
 * This mounts the REAL, unmodified `ChatComposer` from
 * `src/components/chat-composer.tsx` — not a simplified stand-in — via the
 * `bun test` React Native harness in `tests/support/rn-test-environment.ts`
 * (see that file for how `react-native` itself becomes importable under
 * `bun test`, and why a few native-bridge-touching leaves are stubbed).
 *
 * What's mocked here, and why each is safe to mock for THIS test's purpose
 * (verifying ChatComposer's own state/render/effect logic, not re-proving
 * already-covered pure logic or unreachable native behavior):
 *
 * - `react-native-reanimated`: real Reanimated needs a native worklet
 *   runtime that doesn't exist under `bun test`; this mock makes
 *   `useDerivedValue`/`useAnimatedStyle`/`withTiming` resolve synchronously
 *   instead, which is exactly what the official `react-native-reanimated/
 *   mock` recommends for tests (that mock itself pulls in Reanimated's real
 *   native-touching `./index`, so this is a from-scratch equivalent scoped
 *   to only what `ChatComposer` uses).
 * - `../src/components/arwes-button`, `../src/components/background-
 *   gradient`, `../src/components/activity-indicator`: these are Skia-
 *   drawn presentational leaves (real GPU canvas rendering via
 *   `@shopify/react-native-skia`) with NO real equivalent outside a device/
 *   simulator; they carry no composer STATE logic of their own — all the
 *   Queue/Steer/Stop/lane-picker decisions live in `ChatComposer` itself, one
 *   level up. Stand-ins here forward `onPress`/`disabled`/`children` exactly
 *   like the real components' documented contracts, so `ChatComposer`'s own
 *   button-press wiring is still exercised for real.
 * - `../src/native/modules`: `usePushToTalk` (real, NOT mocked) reads
 *   `khalaNativeModules.pushToTalkStt` here; the real module calls Expo's
 *   `requireNativeModule` at import time, which throws immediately outside a
 *   native host. The push-to-talk STATE MACHINE itself
 *   (`phaseFromAvailability`, `isPushToTalkPressable`, etc.) is already
 *   covered by `tests/push-to-talk-core.test.ts`; this fake just answers
 *   `getAvailabilityAsync` so `usePushToTalk`'s real mount-effect runs.
 * - `../src/auth/khala-auth-context`: `ChatComposer` reads `baseUrl`/`token`
 *   from `useKhalaAuth()` (MM-G1, #8485) to fire the push-registration call
 *   below; the real provider needs a mounted `<KhalaAuthProvider>` plus
 *   SecureStore/expo-auth-session, which are out of scope for this
 *   composer-only test. This fake returns static values.
 * - `../src/push/push-notifications-client`: the real module imports
 *   `expo-notifications`, which calls into `expo-modules-core`'s
 *   `globalThis.expo.EventEmitter` at import time — dead outside a native
 *   host, same class of problem as the native modules above. The push
 *   registration DECISION/PERSISTENCE logic itself is already covered by
 *   `tests/push-registration-core.test.ts` and `tests/push-device-store.test.ts`;
 *   this fake just proves `ChatComposer` calls it once per new-turn send.
 *
 * Everything else `ChatComposer` imports — `push-to-talk-core`,
 * `khala-runtime-compose-core`, `khala-sync-push-core`, `swipe-quote-core`,
 * `theme/tokens` — is the REAL, unmocked module.
 */

const reanimatedMock = () => {
  const ReactNative = require("react-native")
  return {
    default: {
      View: ReactNative.View,
      createAnimatedComponent: (Component: unknown) => Component
    },
    useAnimatedStyle: (factory: () => unknown) => factory(),
    useDerivedValue: (factory: () => unknown) => ({ value: factory() }),
    useSharedValue: (initial: unknown) => ({ value: initial }),
    withTiming: (toValue: unknown) => toValue
  }
}
mock.module("react-native-reanimated", reanimatedMock)

mock.module("../src/components/arwes-button", () => ({
  ArwesButton: ({
    accessibilityLabel,
    children,
    disabled,
    onPress
  }: {
    accessibilityLabel?: string
    children?: React.ReactNode
    disabled?: boolean
    onPress?: () => void
  }) =>
    React.createElement(
      "ArwesButton",
      { accessibilityLabel, accessibilityRole: "button", disabled, onPress: disabled ? undefined : onPress },
      children
    )
}))

mock.module("../src/components/background-gradient", () => ({
  BackgroundGradient: ({ children }: { children?: React.ReactNode }) =>
    React.createElement("BackgroundGradient", null, children)
}))

mock.module("../src/components/activity-indicator", () => ({
  ActivityIndicator: () => React.createElement("ActivityIndicator", null)
}))

const pushToTalkAvailability: { status: "available" | "denied" | "unavailable"; reason?: string } = {
  status: "unavailable"
}
mock.module("../src/native/modules", () => ({
  khalaNativeModules: {
    pushToTalkStt: {
      getAvailabilityAsync: () => Promise.resolve(pushToTalkAvailability),
      startRecognitionAsync: () => Promise.reject(new Error("not implemented in test")),
      stopRecognitionAsync: () => Promise.reject(new Error("not implemented in test"))
    }
  }
}))

mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({
    baseUrl: "https://openagents.test",
    ownerUserId: "user_test",
    status: "signed_in",
    token: "token_test"
  })
}))

export const registerForPushNotificationsAsyncMock = mock(() => Promise.resolve({ ok: true, deviceId: "device_test" }))
mock.module("../src/push/push-notifications-client", () => ({
  registerForPushNotificationsAsync: registerForPushNotificationsAsyncMock
}))

// Imported AFTER the `mock.module` calls above (Bun resolves module mocks by
// the target's absolute path, matching regardless of how differently the
// two files spell the specifier — verified empirically — but the mocks must
// still be REGISTERED before `ChatComposer` first pulls in the real modules
// they replace).
const { ChatComposer } = await import("../src/components/chat-composer")

type TestInstance = ReturnType<ReturnType<typeof createTestRenderer>["root"]["findByType"]>

// Restricted to HOST instances (`typeof node.type === "string"`, e.g. the
// mocked `"ArwesButton"`/`"Pressable"` host elements from
// `tests/support/rn-test-environment.ts`), not composite component
// instances. Without this, a matching prop shows up TWICE per real button —
// once on the composite `ArwesButton`/`Pressable` component instance, once
// on the host element it renders — since this test's mocks forward
// `accessibilityLabel`/`onPress` straight through.
const findByProp = (root: TestInstance["parent"] extends never ? never : any, propName: string, value: unknown) =>
  root.findAll((node: any) => typeof node.type === "string" && node.props?.[propName] === value)

const makeTurn = (overrides: Partial<RuntimeTurnEntity> = {}): RuntimeTurnEntity =>
  ({
    lane: "codex_app_server",
    status: "running",
    threadId: "thread_test",
    turnId: "turn_test",
    ...overrides
  }) as RuntimeTurnEntity

type ChatComposerTestProps = Readonly<{
  activeTurn: RuntimeTurnEntity | undefined
  push: (mutations: ReadonlyArray<{ name: string; args: unknown }>) => Promise<unknown>
}>

/** Mounts the real `ChatComposer` inside `act`, then flushes one more
 * microtask turn so `usePushToTalk`'s real mount effect (which awaits the
 * mocked `getAvailabilityAsync()`) settles before any assertions run —
 * otherwise that state update lands just after `act`'s synchronous window
 * closes and React logs an "not wrapped in act(...)" warning. */
const mountComposer = async (props: ChatComposerTestProps) => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(React.createElement(ChatComposer, { threadId: "thread_1", ...props }))
    await Promise.resolve()
  })
  return renderer!
}

// Oracle for khala_mobile.composer.rn_component_mount_coverage.v1
describe("contract khala_mobile.composer.rn_component_mount_coverage.v1 — ChatComposer (real component mount)", () => {
  beforeEach(() => {
    pushToTalkAvailability.status = "unavailable"
  })

  test("mounts without crashing, idle state shows Send (not Stop)", async () => {
    const push = mock(() => Promise.resolve())
    const renderer = await mountComposer({ activeTurn: undefined, push })
    const tree = renderer.toJSON()
    expect(tree).toBeTruthy()

    const sendButtons = findByProp(renderer.root, "accessibilityLabel", "Send")
    const stopButtons = findByProp(renderer.root, "accessibilityLabel", "Stop")
    expect(sendButtons.length).toBe(1)
    expect(stopButtons.length).toBe(0)
  })

  test("active turn shows Stop (not Send), and the idle lane picker is hidden", async () => {
    const push = mock(() => Promise.resolve())
    const renderer = await mountComposer({ activeTurn: makeTurn(), push })

    const sendButtons = findByProp(renderer.root, "accessibilityLabel", "Send")
    const stopButtons = findByProp(renderer.root, "accessibilityLabel", "Stop")
    expect(stopButtons.length).toBe(1)
    expect(sendButtons.length).toBe(0)

    // Idle-only lane picker (`accessibilityLabel="Provider"`) must not render
    // while a turn is active (#8405 — a running turn's lane is already
    // fixed).
    const providerPicker = findByProp(renderer.root, "accessibilityLabel", "Provider")
    expect(providerPicker.length).toBe(0)
  })

  test("typing text updates the input value", async () => {
    const push = mock(() => Promise.resolve())
    const renderer = await mountComposer({ activeTurn: undefined, push })

    const inputs = renderer.root.findAllByType("TextInput" as unknown as React.ComponentType)
    expect(inputs.length).toBe(1)
    const input = inputs[0]!

    await act(() => {
      ;(input.props as { onChangeText: (text: string) => void }).onChangeText("hello from the test")
    })

    const updatedInputs = renderer.root.findAllByType("TextInput" as unknown as React.ComponentType)
    expect((updatedInputs[0]!.props as { value: string }).value).toBe("hello from the test")
  })

  test("pressing Send with idle text starts a new turn via push()", async () => {
    const push = mock(() => Promise.resolve())
    const renderer = await mountComposer({ activeTurn: undefined, push })

    const inputs = renderer.root.findAllByType("TextInput" as unknown as React.ComponentType)
    await act(() => {
      ;(inputs[0]!.props as { onChangeText: (text: string) => void }).onChangeText("ship it")
    })

    const sendButtons = findByProp(renderer.root, "accessibilityLabel", "Send")
    expect(sendButtons.length).toBe(1)
    // `onPress` triggers `sendMessage`, an async handler that awaits `push()`
    // before its `finally` clears `sending` — awaiting the returned promise
    // inside `act` keeps that whole state transition inside act's tracking
    // window instead of leaking a state update past it.
    await act(async () => {
      await (sendButtons[0]!.props as { onPress: () => Promise<void> }).onPress()
    })

    expect(push).toHaveBeenCalledTimes(1)
    const calls = push.mock.calls as unknown as Array<Array<ReadonlyArray<{ name: string; args: unknown }>>>
    const mutations = calls[0]![0]!
    expect(mutations.map(m => m.name)).toEqual(["chat.appendMessage", "runtime.startTurn"])
  })

  test("pressing Stop on an active turn calls push() with runtime.interruptTurn", async () => {
    const push = mock(() => Promise.resolve())
    const renderer = await mountComposer({ activeTurn: makeTurn(), push })

    const stopButtons = findByProp(renderer.root, "accessibilityLabel", "Stop")
    expect(stopButtons.length).toBe(1)
    await act(async () => {
      await (stopButtons[0]!.props as { onPress: () => Promise<void> }).onPress()
    })

    expect(push).toHaveBeenCalledTimes(1)
    const calls = push.mock.calls as unknown as Array<Array<ReadonlyArray<{ name: string; args: unknown }>>>
    const mutations = calls[0]![0]!
    expect(mutations.map(m => m.name)).toEqual(["runtime.interruptTurn"])
  })

  test("each turn status label renders correctly (queued / running / waiting_for_input)", async () => {
    for (const status of ["queued", "running", "waiting_for_input"] as const) {
      const push = mock(() => Promise.resolve())
      const renderer = await mountComposer({ activeTurn: makeTurn({ status }), push })
      const texts = renderer.root.findAllByType("Text" as unknown as React.ComponentType)
      const statusText = texts.map((t: TestInstance) => t.props.children as unknown).join(" ")
      expect(statusText).toContain(status === "waiting_for_input" ? "waiting for input" : status)
      // A Stop button must always be reachable while a turn is active,
      // regardless of which status it's in.
      expect(findByProp(renderer.root, "accessibilityLabel", "Stop").length).toBe(1)
    }
  })
})
