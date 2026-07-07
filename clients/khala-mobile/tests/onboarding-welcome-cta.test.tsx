import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { View as RNView } from "react-native"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * Real React Native component-mount test for the onboarding `WelcomeStep`'s
 * "Get started" CTA — the SAME `bun test` RN harness
 * (`tests/support/rn-test-environment.ts`) that `chat-composer.test.tsx` and
 * `repo-picker-screen.test.tsx` proved out.
 *
 * It exists to pin the Fabric no-fill button fix (owner report 2026-07-07,
 * identical to the login-button bug in `sign-in-screen.tsx`): under the New
 * Architecture a `Pressable` with a function `style` does NOT paint its own
 * `backgroundColor`, so the cyan fill MUST live on an INNER plain `<View>`.
 * This mounts the REAL, unmodified `OnboardingFlow` (its initial render is
 * `WelcomeStep`) and asserts the fill is on the inner View — not the
 * Pressable — with the high-contrast cyan pill + dark bold label.
 *
 * Mocks follow the same "only mock what touches the dead native bridge or is
 * covered elsewhere, with shapes compatible with the other mount tests in this
 * shared process" rule the neighbouring mount tests document.
 */

// Ignite `Screen`/`Header` pull @react-navigation/native + the native shell
// wrappers at module-eval; WelcomeStep uses neither, so stand them in exactly
// like `repo-picker-screen.test.tsx` does (compatible shapes).
mock.module("../src/ignite/components/Screen", () => ({
  Screen: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
}))
mock.module("../src/ignite/components/Header", () => ({
  Header: ({ title }: { title?: string }) => React.createElement("Header", null, title),
}))

// The `../ignite` barrel re-exports `useSafeAreaInsetsStyle`, which imports
// `react-native-safe-area-context` (native, trips Bun's reentrant-require
// guard). Same stand-in the repo-picker mount test uses.
mock.module("react-native-safe-area-context", () => ({
  useSafeAreaInsets: () => ({ bottom: 0, left: 0, right: 0, top: 0 }),
}))

// Credits chip pulls the real Khala Sync runtime primitives; the welcome CTA
// doesn't depend on the balance readout, so a null stand-in keeps the mount
// focused on the button. No other test mounts this component for real.
mock.module("../src/components/credits-balance-chip", () => ({
  CreditsBalanceChip: () => null,
}))

// Push registration imports `expo-notifications` (dead outside a native host).
mock.module("../src/push/push-notifications-client", () => ({
  registerForPushNotificationsAsync: async () => undefined,
  unregisterPushNotificationsAsync: async () => undefined,
}))

// Real sync runtime needs Expo SQLite + a durable-cursor session. Same shape
// the repo-picker mount test uses (compatible in the shared process).
mock.module("../src/sync/khala-mobile-sync-runtime-context", () => ({
  useKhalaMobileSyncRuntime: () => ({ runtime: {}, status: "ready" }),
  useKhalaMobileSyncPrimitives: () => ({ overlay: {}, session: {}, store: {} }),
}))

// Static auth values; `githubLogin` is a harmless superset over the shapes the
// other mount tests provide. The greeting-selection logic itself is covered
// purely in `onboarding-core.test.ts`, so this file does not assert on it.
mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({
    baseUrl: "https://openagents.test",
    githubLogin: "octocat",
    ownerUserId: "user_test",
    status: "signed_in",
    token: "token_test",
  }),
}))

const { OnboardingFlow } = await import("../src/screens/onboarding-flow")

type AnyNode = {
  type: unknown
  props: Record<string, unknown>
}

/** Flattens a style prop (object | array | nested arrays | falsy) into the
 * merged style object, mirroring RN's own precedence (later wins). */
const flattenStyle = (style: unknown): Record<string, unknown> => {
  if (Array.isArray(style)) {
    return style.reduce<Record<string, unknown>>((acc, entry) => ({ ...acc, ...flattenStyle(entry) }), {})
  }
  return style && typeof style === "object" ? (style as Record<string, unknown>) : {}
}

const mountWelcome = async () => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(
      React.createElement(OnboardingFlow, { onThreadCreated: () => undefined }),
    )
  })
  return renderer!
}

// Oracle for khala_mobile.onboarding.get_started_cta_fill_on_inner_view.v1
describe("contract khala_mobile.onboarding.get_started_cta_fill_on_inner_view.v1 — WelcomeStep CTA (real component mount)", () => {
  test("renders a 'Get started' label inside a cyan-filled inner View, with the Pressable owning only the touch target", async () => {
    const renderer = await mountWelcome()

    // The label is a real react-native Text (host "Text") with string children.
    const labelNodes = renderer.root.findAll(
      (node: AnyNode) => typeof node.type === "string" && node.type === "Text" && node.props.children === "Get started",
    )
    expect(labelNodes.length).toBe(1)

    // A single plain host View carries the cyan fill — the guaranteed-paint
    // element under Fabric. (The Pressable must NOT, per the bug this guards.)
    const filledViews = renderer.root.findAll(
      (node: AnyNode) =>
        typeof node.type === "string" &&
        node.type === "View" &&
        flattenStyle(node.props.style).backgroundColor === "#4fd0ff",
    )
    expect(filledViews.length).toBe(1)
    const innerStyle = flattenStyle(filledViews[0]!.props.style)
    // High-contrast, obviously-a-button proportions (not bare text).
    expect(innerStyle.minHeight).toBe(54)
    expect(innerStyle.borderRadius).toBe(12)

    // And that filled View is the one wrapping the "Get started" label.
    const labelInsideFill = filledViews[0]!.findAll(
      (node: AnyNode) => typeof node.type === "string" && node.type === "Text" && node.props.children === "Get started",
    )
    expect(labelInsideFill.length).toBe(1)

    // The Pressable is the touch target and must NOT carry the fill itself —
    // that is exactly the Fabric no-paint bug this contract guards against.
    const pressable = renderer.root.find(
      (node: AnyNode) => typeof node.type === "string" && node.type === "Pressable",
    )
    expect(typeof (pressable.props as { onPress?: unknown }).onPress).toBe("function")
    expect((pressable.props as { accessibilityRole?: string }).accessibilityRole).toBe("button")
    expect(flattenStyle(pressable.props.style).backgroundColor).toBeUndefined()

    // Dark bold label for contrast against the cyan fill.
    const labelStyle = flattenStyle(labelNodes[0]!.props.style)
    expect(labelStyle.color).toBe("#02060d")
    expect(labelStyle.fontWeight).toBe("700")
  })
})
