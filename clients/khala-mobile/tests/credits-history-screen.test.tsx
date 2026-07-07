import { beforeEach, describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { View as RNView } from "react-native"
import { act, create as createTestRenderer } from "react-test-renderer"

import { mobileCreditsTransactions } from "./fixtures/mobile-screen-fixtures"

mock.module("../src/ignite/components/Screen", () => ({
  Screen: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
}))
mock.module("../src/ignite/components/Header", () => ({
  Header: ({ title }: { title?: string }) => React.createElement("Header", null, title),
}))
mock.module("@react-navigation/native", () => ({
  useNavigation: () => ({ canGoBack: () => true, goBack: () => undefined }),
}))
mock.module("react-native-reanimated", () => {
  const chainable = () => ({ duration: () => chainable() })
  return { default: { View: RNView }, FadeIn: { delay: () => chainable() } }
})
mock.module("react-native-safe-area-context", () => ({
  SafeAreaView: ({ children }: { children?: React.ReactNode }) => React.createElement("SafeAreaView", null, children),
  useSafeAreaInsets: () => ({ bottom: 0, left: 0, right: 0, top: 0 }),
}))
mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({ baseUrl: "https://openagents.test", token: "token_test" }),
}))

type CreditsResponse =
  | Readonly<{ ok: false; kind: "network" | "unavailable" }>
  | Readonly<{ ok: true; value: { nextCursor: string | null; transactions: typeof mobileCreditsTransactions } }>

let creditsResponse: CreditsResponse = {
  ok: true,
  value: { nextCursor: null, transactions: mobileCreditsTransactions },
}

mock.module("../src/sync/khala-mobile-credits-api", () => ({
  fetchKhalaMobileCreditsBalance: async () => ({ ok: true, value: 875 }),
  fetchKhalaMobileCreditsTransactions: async () => creditsResponse,
}))

const { CreditsHistoryScreen } = await import("../src/screens/credits-history-screen")

type AnyNode = { props: Record<string, unknown>; type: unknown }

const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0))

const mountCredits = async () => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(React.createElement(CreditsHistoryScreen))
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

beforeEach(() => {
  creditsResponse = {
    ok: true,
    value: { nextCursor: null, transactions: mobileCreditsTransactions },
  }
})

describe("contract khala_mobile.credits_history.rn_component_mount_coverage.v1 — CreditsHistoryScreen", () => {
  test("renders unavailable, error, empty, and populated transaction states from typed fixtures", async () => {
    creditsResponse = { kind: "unavailable", ok: false }
    expect(hasText(await mountCredits(), "History not yet available")).toBe(true)

    creditsResponse = { kind: "network", ok: false }
    expect(hasText(await mountCredits(), "History unavailable")).toBe(true)

    creditsResponse = { ok: true, value: { nextCursor: null, transactions: [] } }
    expect(hasText(await mountCredits(), "No transactions yet")).toBe(true)

    creditsResponse = {
      ok: true,
      value: { nextCursor: null, transactions: mobileCreditsTransactions },
    }
    const populated = await mountCredits()
    expect(hasText(populated, "Launch credit")).toBe(true)
    expect(hasText(populated, "Codex mobile fixture run")).toBe(true)
  })
})
