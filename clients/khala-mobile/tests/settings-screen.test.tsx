import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

import { mobileFixtureOwnerUserId, mobileModelPreference } from "./fixtures/mobile-screen-fixtures"

mock.module("../src/ignite/components/Screen", () => ({
  Screen: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
}))
mock.module("../src/ignite/components/Header", () => ({
  Header: ({ title }: { title?: string }) => React.createElement("Header", null, title),
}))
mock.module("react-native-safe-area-context", () => ({
  SafeAreaView: ({ children }: { children?: React.ReactNode }) => React.createElement("SafeAreaView", null, children),
  useSafeAreaInsets: () => ({ bottom: 0, left: 0, right: 0, top: 0 }),
}))
mock.module("expo-constants", () => ({
  default: { expoConfig: { android: { versionCode: 37 }, ios: { buildNumber: "37" }, version: "1.2.3" } },
}))
mock.module("expo-notifications", () => ({
  getPermissionsAsync: async () => ({ canAskAgain: true, granted: false }),
}))
mock.module("../src/push/push-notifications-client", () => ({
  registerForPushNotificationsAsync: async () => undefined,
}))
mock.module("../src/native/use-on-device-readiness", () => ({
  useOnDeviceReadiness: () => ({
    rows: [
      { detail: "fixture sqlite", key: "sqlite", label: "SQLite", status: "ready", tone: "success" },
      { detail: "fixture notifications", key: "notifications", label: "Notifications", status: "pending", tone: "warning" },
    ],
    status: "ready",
  }),
}))
mock.module("../src/auth/mobile-openauth", () => ({
  KHALA_ACCOUNT_DELETION_POLICY_COPY: "Fixture deletion policy copy.",
}))
mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({
    baseUrl: "https://openagents.test",
    deleteAccount: async () => undefined,
    ownerUserId: mobileFixtureOwnerUserId,
    signOut: () => undefined,
    token: "token_test",
  }),
}))
mock.module("../src/sync/khala-mobile-credits-api", () => ({
  fetchKhalaMobileCreditsBalance: async () => ({ ok: true, value: 875 }),
  fetchKhalaMobileCreditsTransactions: async () => ({
    ok: true,
    value: { nextCursor: null, transactions: [] },
  }),
}))
mock.module("../src/sync/khala-mobile-model-preference-api", () => ({
  fetchKhalaMobileModelPreference: async () => ({ ok: true, value: mobileModelPreference }),
  putKhalaMobileModelPreference: async () => ({ ok: true, value: mobileModelPreference }),
}))

const { SettingsScreen } = await import("../src/screens/settings-screen")

type AnyNode = { props: Record<string, unknown>; type: unknown }

const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0))

const mountSettings = async () => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(
      React.createElement(SettingsScreen, {
        navigation: {
          navigate: () => undefined,
          openDrawer: () => undefined,
        },
      } as never),
    )
  })
  for (let attempt = 0; attempt < 10; attempt++) {
    await act(async () => {
      await tick()
    })
  }
  return renderer!
}

const textContent = (value: unknown): string => {
  if (typeof value === "string" || typeof value === "number") return String(value)
  if (Array.isArray(value)) return value.map(textContent).join("")
  if (React.isValidElement(value)) return textContent((value.props as { children?: unknown }).children)
  return ""
}

const hasText = (renderer: ReturnType<typeof createTestRenderer>, text: string): boolean =>
  renderer.root.findAll(
    (node: AnyNode) =>
      typeof node.type === "string" &&
      node.type === "Text" &&
      textContent(node.props.children).includes(text),
  ).length > 0

describe("contract khala_mobile.settings.rn_component_mount_coverage.v1 — SettingsScreen", () => {
  test("mounts all launch settings sections against typed account, credits, model, push, and readiness fixtures", async () => {
    const renderer = await mountSettings()

    for (const heading of ["Account", "Credits", "Models", "Notifications", "About & diagnostics", "Danger zone"]) {
      expect(hasText(renderer, heading)).toBe(true)
    }

    expect(hasText(renderer, mobileFixtureOwnerUserId)).toBe(true)
    expect(hasText(renderer, "Balance: $8.75")).toBe(true)
    expect(hasText(renderer, "GPT 5 (active)")).toBe(true)
    expect(hasText(renderer, "Get notified when a task finishes or needs your input.")).toBe(true)
    expect(hasText(renderer, "Khala Code 1.2.3 (ios 37)")).toBe(true)
    expect(hasText(renderer, "SQLite")).toBe(true)
    expect(hasText(renderer, "Delete account")).toBe(true)
  })
})
