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
  buildCreditsHistoryProgram,
  type CreditsHistoryCallbacks,
  type CreditsHistoryViewModel,
  renderCreditsHistoryView,
} from "../src/screens/credits-history-effect-native-core"

/**
 * MB-EN (#8597) renderer-adapter contract for the converted Credits History
 * screen: the authored Effect Native view tree renders through the REAL React
 * Native renderer (`@effect-native/render-rn`) driven with a string-typed RN
 * host shim, so it runs deterministically in bun with no native modules. Proves:
 * (1) the pure view authors the four load states with the catalog component set,
 * (2) the Protoss-blue `khalaEffectNativeTheme` resolves into the tree, and
 * (3) the typed Back / Load more intents flow intent -> handler -> the shell's
 * imperative callbacks through the adapter (the seam that keeps DATA/NAV in the
 * screen shell while PRESENTATION is Effect Native).
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

const populatedViewModel: CreditsHistoryViewModel = {
  status: "ready",
  hasMore: true,
  transactions: [
    {
      id: "fixture-grant",
      description: "Launch credit",
      kind: "grant",
      amountUsdCents: 1000,
      occurredAt: "2026-07-01T00:00:00.000Z",
    },
    {
      id: "fixture-charge",
      description: "Codex mobile fixture run",
      kind: "charge",
      amountUsdCents: 25,
      occurredAt: "2026-07-02T00:00:00.000Z",
    },
  ],
}

describe("contract khala_mobile.credits_history.effect_native_adapter_render.v1", () => {
  test("pure view authors each load state as a typed Effect Native tree", () => {
    const loading = renderCreditsHistoryView({ status: "loading" })
    expect(loading._tag).toBe("Stack")
    // The header (Back button + title) renders unconditionally across states.
    expect(JSON.stringify(loading)).toContain("CreditsHistoryBack")
    expect(JSON.stringify(loading)).toContain("Credit history")

    expect(JSON.stringify(renderCreditsHistoryView({ status: "unavailable" }))).toContain(
      "History not yet available",
    )
    expect(JSON.stringify(renderCreditsHistoryView({ status: "error" }))).toContain(
      "History unavailable",
    )
    expect(
      JSON.stringify(renderCreditsHistoryView({ status: "ready", hasMore: false, transactions: [] })),
    ).toContain("No transactions yet")

    const populated = JSON.stringify(renderCreditsHistoryView(populatedViewModel))
    expect(populated).toContain('"_tag":"List"')
    expect(populated).toContain("Launch credit")
    expect(populated).toContain("Codex mobile fixture run")
    // hasMore renders a typed Load-more intent, never an inline handler.
    expect(populated).toContain("CreditsHistoryLoadMore")
  })

  test("renders through render-rn with the khala theme and dispatches typed intents to shell callbacks", async () => {
    let backPresses = 0
    let loadMorePresses = 0
    const callbacks: CreditsHistoryCallbacks = {
      onBack: () => {
        backPresses += 1
      },
      onLoadMore: () => {
        loadMorePresses += 1
      },
    }

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const program = buildCreditsHistoryProgram(callbacks)
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaEffectNativeTheme,
            platform: "ios",
          })
          const surface = yield* renderer.mount(
            { render: () => {} },
            program.viewStream,
            program.report,
          )

          // Drive the surface to the populated state (the shell's setViewModel
          // push — data stays in the shell, presentation is Effect Native).
          program.setViewModel(populatedViewModel)
          yield* nextTask
          yield* Effect.yieldNow

          const current = yield* surface.currentElement
          const structure = reactNativeStructure(current)
          expect(structure?.tag).toBe("Stack")
          expect(structure?.key).toBe("credits-history-root")

          // (2) the Protoss-blue theme resolved into the tree: the `background`
          // color token became the khala background hex in the rendered styles.
          expect(JSON.stringify(current)).toContain(khalaEffectNativeTheme.color.background)
          expect(khalaEffectNativeTheme.color.background).toBe("#02060d")

          // (3) typed Load-more intent reaches the shell callback.
          const loadMore = findByNativeId(current, "effect-native:Button:credits-history-load-more")
          const onLoadMore = loadMore?.props.onPress
          if (typeof onLoadMore !== "function") {
            throw new Error("expected the Load more Button to bind onPress")
          }
          onLoadMore()
          yield* nextTask
          expect(loadMorePresses).toBe(1)

          // (3) typed Back intent reaches the shell callback.
          const back = findByNativeId(current, "effect-native:Button:credits-history-back")
          const onBack = back?.props.onPress
          if (typeof onBack !== "function") {
            throw new Error("expected the Back Button to bind onPress")
          }
          onBack()
          yield* nextTask
          expect(backPresses).toBe(1)

          yield* surface.unmount
        }),
      ),
    )
  })
})
