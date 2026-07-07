import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * Real React Native component-mount test for the Settings `AccountSection`'s
 * "Sign out" button — the SAME `bun test` RN harness
 * (`tests/support/rn-test-environment.ts`) that `chat-composer.test.tsx`,
 * `repo-picker-screen.test.tsx`, and `onboarding-welcome-cta.test.tsx` proved
 * out.
 *
 * It pins the Fabric no-fill button fix (owner report 2026-07-07, identical to
 * the login/"Get started" bug): under the New Architecture a `Pressable` with
 * a function `style` does NOT paint its own `backgroundColor`/`borderColor`,
 * so the visible fill MUST live on an INNER plain `<View>`. The Ignite
 * `Button preset="reversed"` this replaced hit exactly that bug — a dark label
 * on the unpainted dark-navy card, rendered invisible. This mounts the REAL
 * `AccountSection` and asserts the neutral pill fill is on the inner View, not
 * the Pressable.
 *
 * Mocks follow the same "only mock what touches the dead native bridge or is
 * covered elsewhere" rule the neighbouring mount tests document.
 */

// Ignite `Screen`/`Header`/`Card` pull @react-navigation/native + native shell
// wrappers at module-eval; AccountSection only needs Card as a passthrough
// wrapper, so stand them in with compatible shapes (same idea as the
// repo-picker/onboarding mount tests).
mock.module("../src/ignite/components/Screen", () => ({
  Screen: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
}))
mock.module("../src/ignite/components/Header", () => ({
  Header: ({ title }: { title?: string }) => React.createElement("Header", null, title),
}))
mock.module("../src/ignite/components/Card", () => ({
  Card: ({ ContentComponent, heading }: { ContentComponent?: React.ReactNode; heading?: string }) =>
    React.createElement(React.Fragment, null, React.createElement("Text", null, heading), ContentComponent),
}))

// The `../ignite` barrel re-exports `useSafeAreaInsetsStyle` (native). Same
// stand-in the other mount tests use.
mock.module("react-native-safe-area-context", () => ({
  useSafeAreaInsets: () => ({ bottom: 0, left: 0, right: 0, top: 0 }),
}))

// Native-bridge / expo modules the settings-screen module graph pulls at eval
// but that AccountSection's Sign out button does not exercise.
mock.module("expo-constants", () => ({ default: { expoConfig: { version: "0.0.0" } } }))
mock.module("expo-notifications", () => ({
  getPermissionsAsync: async () => ({ canAskAgain: true, granted: false }),
}))
mock.module("../src/push/push-notifications-client", () => ({
  registerForPushNotificationsAsync: async () => undefined,
}))
mock.module("../src/native/use-on-device-readiness", () => ({
  useOnDeviceReadiness: () => ({ status: "loading" }),
}))
mock.module("../src/sync/khala-mobile-credits-api", () => ({
  fetchKhalaMobileCreditsBalance: async () => ({ ok: false }),
}))
mock.module("../src/sync/khala-mobile-model-preference-api", () => ({
  fetchKhalaMobileModelPreference: async () => ({ ok: false }),
  putKhalaMobileModelPreference: async () => ({ ok: false }),
}))
mock.module("../src/auth/mobile-openauth", () => ({
  KHALA_ACCOUNT_DELETION_POLICY_COPY: "…",
}))

const signOut = mock(() => undefined)
mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({
    baseUrl: "https://openagents.test",
    deleteAccount: async () => undefined,
    ownerUserId: "user_test",
    signOut,
    token: "token_test",
  }),
}))

const { AccountSection } = await import("../src/screens/settings-screen")

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

const mountAccountSection = async () => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(React.createElement(AccountSection))
  })
  return renderer!
}

// Oracle for khala_mobile.settings.sign_out_button_fill_on_inner_view.v1
describe("contract khala_mobile.settings.sign_out_button_fill_on_inner_view.v1 — AccountSection Sign out (real component mount)", () => {
  test("renders a 'Sign out' label inside a filled inner View, with the Pressable owning only the touch target", async () => {
    const renderer = await mountAccountSection()

    // The label is a real react-native Text (host "Text") with string children.
    const labelNodes = renderer.root.findAll(
      (node: AnyNode) => typeof node.type === "string" && node.type === "Text" && node.props.children === "Sign out",
    )
    expect(labelNodes.length).toBe(1)

    // A single plain host View carries the neutral pill fill — the
    // guaranteed-paint element under Fabric. (The Pressable must NOT.)
    const filledViews = renderer.root.findAll(
      (node: AnyNode) =>
        typeof node.type === "string" &&
        node.type === "View" &&
        flattenStyle(node.props.style).backgroundColor === "#141d33",
    )
    expect(filledViews.length).toBe(1)
    const innerStyle = flattenStyle(filledViews[0]!.props.style)
    // Obviously-a-button proportions (not bare text), with a visible border.
    expect(innerStyle.minHeight).toBe(48)
    expect(innerStyle.borderRadius).toBe(10)
    expect(innerStyle.borderWidth).toBe(1)

    // And that filled View is the one wrapping the "Sign out" label.
    const labelInsideFill = filledViews[0]!.findAll(
      (node: AnyNode) => typeof node.type === "string" && node.type === "Text" && node.props.children === "Sign out",
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

    // Legible light label against the dark navy fill.
    const labelStyle = flattenStyle(labelNodes[0]!.props.style)
    expect(labelStyle.color).toBe("#e6e9f2")
    expect(labelStyle.fontWeight).toBe("600")
  })
})
