import { afterAll, afterEach, beforeEach, describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { View as RNView } from "react-native"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * Real React Native component-mount test for `RepoPickerScreen` — MM-I3
 * (#8492)'s straight-line E2E work. Extends the SAME `bun test` React Native
 * harness (`tests/support/rn-test-environment.ts`) that
 * `tests/chat-composer.test.tsx` proved out first, per the mobile QA swarm
 * audit's own stated next priority ("extending the SAME harness to the next
 * screen/component is the next highest-leverage step").
 *
 * This mounts the REAL, unmodified `RepoPickerScreen` from
 * `src/screens/repo-picker-screen.tsx` — the "pick a repo" step of the
 * mobile-only MVP's straight line (audit `docs/fable/2026-07-05-khala-code-
 * mobile-only-mvp-launch-audit.md` §4 step 4). It proves real loading ->
 * ready state transitions, a real fetch through the REAL (unmocked)
 * `khala-mobile-repos-api` client, real search-filter wiring against the
 * real pure `khala-mobile-repo-search-core` functions, and a real
 * repo-select -> `bindThreadRepo()` call — the exact seam the cloud-execution
 * lane consumes. It also mounts the REAL `KhalaListItem` (not a stand-in),
 * proving RepoPickerScreen's row-press wiring through KhalaListItem's own
 * real `onPress`/`accessibilityLabel` contract.
 *
 * `bun:test`'s `mock.module` mutates the GLOBAL module registry for the
 * WHOLE test process, not just this file — confirmed empirically while
 * writing this test: an earlier draft globally mocked `khala-list-item` and
 * `khala-mobile-repos-api`, which silently broke `khala-ui-primitives.test.tsx`
 * (needs the REAL `KhalaListItem`) and `khala-mobile-repos-api.test.ts`
 * (needs the REAL client) when the full `bun test` suite ran, even though
 * this file passed in isolation. The rule this test follows: only
 * `mock.module` a dependency that NO other test file in the package needs
 * for real (verified by grep before writing each mock below); everything
 * else gets a locally-scoped fake (a `globalThis.fetch` swap, restored in
 * `afterAll`) instead of a module-level mock.
 *
 * What's mocked here, and why:
 * - `../components/app-header`: real `AppHeader` calls `useDrawerStatus()`
 *   (throws outside a mounted drawer navigator) and `useNavigation()`; this
 *   screen's own state logic doesn't touch either, so a plain stand-in
 *   (rendering its `title` as text) is a safe, uninvolved swap. No other
 *   test file imports `app-header` for real.
 * - `../components/touchable-feedback`: the REAL `KhalaListItem` renders
 *   through this (needs `react-native-gesture-handler` +
 *   `react-native-reanimated`, no meaning under `bun test`, same class of
 *   problem as ChatComposer's Reanimated dependency) — mocked with the EXACT
 *   same shape `tests/khala-ui-primitives.test.tsx` already uses for the
 *   same reason, so both files' mocks are compatible/idempotent rather than
 *   conflicting.
 * - `../auth/khala-auth-context`: static `baseUrl`/`token`, same shape
 *   `tests/chat-composer.test.tsx` already uses (compatible, not conflicting).
 * - `../sync/khala-mobile-sync-runtime-context`: real runtime needs Expo
 *   SQLite + a durable-cursor session; this fake returns a `ready` state with
 *   a spy `bindThreadRepo`. No other test file imports this for real.
 * - `../components/khala-screen`: real `KhalaScreen` renders `SafeAreaView`
 *   from `react-native-safe-area-context`, a native module with no meaning
 *   under `bun test`. `KhalaScreen` carries no picker logic of its own, so
 *   this stand-in just renders `children` directly (a Fragment). The same
 *   module is ALSO safely mocked this way by `tests/crash-reporting.test.tsx`
 *   (compatible shape) — nothing else needs the real executable module
 *   (`provider-primitives-architecture.test.ts` only reads it as source
 *   text, never executes it).
 * - `globalThis.fetch`: swapped for the duration of this file only (restored
 *   in `afterAll`) so the REAL `fetchKhalaMobileRepositories` client exercises
 *   its real parsing/error-mapping logic against scripted HTTP responses,
 *   instead of hitting a live Worker. `khala-mobile-repos-api.test.ts`
 *   already tests this same client via its `fetchImpl` injection parameter
 *   instead of a global swap — RepoPickerScreen itself calls the client with
 *   NO custom `fetchImpl`, so it always resolves through `globalThis.fetch`,
 *   which is what this test needs to intercept.
 *
 * Everything else RepoPickerScreen imports — `khala-empty-state`,
 * `khala-list-item` (real), `khala-text`, `khala-text-field`, the real
 * `khala-mobile-repos-api` client, and the REAL pure
 * `khala-mobile-repo-search-core` filter/sort/dedupe functions — is the
 * real, unmocked module.
 */

mock.module("../src/components/khala-screen", () => ({
  KhalaScreen: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
}))

mock.module("../src/components/app-header", () => ({
  AppHeader: ({ title }: { title: string; showBack?: boolean }) =>
    React.createElement("AppHeader", null, title),
}))

// Arcade-fidelity audit (2026-07-06) added a real `react-native-reanimated`
// `Animated.View`/`FadeIn` stagger directly to `RepoPickerScreen` (matching
// thread-messages-screen.tsx's existing pattern). Real Reanimated needs a
// native worklet runtime with no meaning under `bun test`. This is a
// separate, minimal mock rather than reusing `chat-composer.test.tsx`'s
// (that one's factory calls `require("react-native")` at invocation time,
// which throws "Requested module is already fetched" if Bun's global
// mock.module registry re-invokes it from a different file's context —
// confirmed empirically while fixing this). Only the two APIs this screen
// actually uses are stood in.
mock.module("react-native-reanimated", () => {
  const chainable = () => ({ duration: () => chainable() })
  return {
    default: { View: RNView },
    FadeIn: { delay: () => chainable() },
  }
})

// Same shape as tests/khala-ui-primitives.test.tsx's mock — kept in sync so
// both files' mocks are compatible, not conflicting, in a shared process.
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

mock.module("../src/auth/khala-auth-context", () => ({
  useKhalaAuth: () => ({
    baseUrl: "https://openagents.test",
    ownerUserId: "user_test",
    status: "signed_in",
    token: "token_test",
  }),
}))

export const bindThreadRepoMock = mock(() => Promise.resolve({ ok: true as const }))
mock.module("../src/sync/khala-mobile-sync-runtime-context", () => ({
  useKhalaMobileSyncRuntime: () => ({
    runtime: { bindThreadRepo: bindThreadRepoMock },
    status: "ready",
  }),
}))

const { RepoPickerScreen } = await import("../src/screens/repo-picker-screen")

const sampleRepo = (overrides: Record<string, unknown> = {}) => ({
  defaultBranch: "main",
  description: null,
  fullName: "acme/widgets",
  htmlUrl: "https://github.com/acme/widgets",
  id: "repo_widgets",
  name: "widgets",
  owner: "acme",
  private: false,
  provider: "github" as const,
  ...overrides,
})

const scriptedRepos = [
  sampleRepo({ fullName: "acme/widgets", id: "repo_widgets", name: "widgets" }),
  sampleRepo({
    description: "Docs site",
    fullName: "acme/docs",
    htmlUrl: "https://github.com/acme/docs",
    id: "repo_docs",
    name: "docs",
  }),
]

const originalFetch = globalThis.fetch
let nextResponseBody: unknown = { hasNextPage: false, page: 1, perPage: 100, repositories: scriptedRepos }
let nextResponseOk = true
export const fetchSpy = mock((..._args: Array<unknown>) =>
  Promise.resolve({ json: () => Promise.resolve(nextResponseBody), ok: nextResponseOk } as Response),
)

beforeEach(() => {
  nextResponseBody = { hasNextPage: false, page: 1, perPage: 100, repositories: scriptedRepos }
  nextResponseOk = true
  fetchSpy.mockClear()
  globalThis.fetch = fetchSpy as unknown as typeof fetch
})

afterEach(() => {
  globalThis.fetch = originalFetch
})

afterAll(() => {
  globalThis.fetch = originalFetch
})

type TestInstance = ReturnType<ReturnType<typeof createTestRenderer>["root"]["findByType"]>

const findByProp = (root: TestInstance["parent"] extends never ? never : any, propName: string, value: unknown) =>
  root.findAll((node: any) => typeof node.type === "string" && node.props?.[propName] === value)

const mountRepoPicker = async () => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(
      React.createElement(RepoPickerScreen, {
        navigation: { canGoBack: () => true, goBack: () => undefined } as never,
        route: { params: { threadId: "thread_1" } } as never,
      }),
    )
  })
  // Flush the effect's `fetchKhalaMobileRepositories` promise chain (the real
  // client does its own `await response.json()` after the fetch resolves,
  // and React's own act-scheduling can involve a macrotask, not just
  // microtasks). Bounded-poll with a real macrotask tick rather than a fixed
  // microtask-tick count: empirically, a fixed microtask-only count that's
  // reliable running this file alone is NOT always enough when the full
  // `bun test` suite's shared event loop is under load from other files.
  const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0))
  for (let attempt = 0; attempt < 50; attempt++) {
    const stillLoading = renderer!.root.findAll(
      (node: { type: unknown }) => typeof node.type === "string" && node.type === "ActivityIndicator",
    )
    if (fetchSpy.mock.calls.length > 0 && stillLoading.length === 0) break
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await tick()
    })
  }
  return renderer!
}

// Oracle for khala_mobile.repo_picker.rn_component_mount_coverage.v1
describe("contract khala_mobile.repo_picker.rn_component_mount_coverage.v1 — RepoPickerScreen (real component mount)", () => {
  test("mounts, loads through the real fetch client, and renders both repos via the real KhalaListItem", async () => {
    const renderer = await mountRepoPicker()
    expect(fetchSpy).toHaveBeenCalledTimes(1)
    const [calledUrl] = fetchSpy.mock.calls[0] as unknown as Array<string>
    expect(calledUrl).toContain("/api/mobile/repos")

    const widgetsRow = findByProp(renderer.root, "accessibilityLabel", "acme/widgets")
    const docsRow = findByProp(renderer.root, "accessibilityLabel", "acme/docs")
    expect(widgetsRow.length).toBe(1)
    expect(docsRow.length).toBe(1)
  })

  test("typing in search filters the real (unmocked) repo list via khala-mobile-repo-search-core", async () => {
    const renderer = await mountRepoPicker()

    const searchInputs = renderer.root.findAllByType("TextInput" as unknown as React.ComponentType)
    expect(searchInputs.length).toBe(1)
    await act(() => {
      ;(searchInputs[0]!.props as { onChangeText: (text: string) => void }).onChangeText("docs")
    })

    expect(findByProp(renderer.root, "accessibilityLabel", "acme/docs").length).toBe(1)
    expect(findByProp(renderer.root, "accessibilityLabel", "acme/widgets").length).toBe(0)
  })

  test("selecting a repo calls the real runtime's bindThreadRepo() with the picked repo and threadId", async () => {
    bindThreadRepoMock.mockClear()
    const renderer = await mountRepoPicker()

    const widgetsRow = findByProp(renderer.root, "accessibilityLabel", "acme/widgets")[0]!
    await act(async () => {
      await (widgetsRow.props as { onPress: () => Promise<void> }).onPress()
    })

    expect(bindThreadRepoMock).toHaveBeenCalledTimes(1)
    const call = bindThreadRepoMock.mock.calls[0]! as unknown as Array<{
      repo: { defaultBranch: string; name: string; owner: string }
      threadId: string
    }>
    expect(call[0]).toEqual({
      repo: { defaultBranch: "main", name: "widgets", owner: "acme" },
      threadId: "thread_1",
    })
  })

  test("a failed fetch renders the real error branch through the real client's error mapping, not a silent blank screen", async () => {
    nextResponseOk = false
    nextResponseBody = { error: "unauthorized" }
    const renderer = await mountRepoPicker()

    // Search every node's string children rather than a specific host type
    // name: `KhalaText` renders as real `Text` here, but ANOTHER test file
    // in the full suite (`tests/crash-reporting.test.tsx`) globally mocks
    // `khala-text` to render a `"KhalaText"` host tag instead — a real
    // instance of the SAME cross-file `mock.module` leakage class this
    // file's other mocks were rewritten to avoid, this time from a module
    // this file never touches. Matching on content, not tag name, is
    // robust to either rendering.
    const allText = renderer.root
      .findAll(node => typeof node.props?.children === "string")
      .map(node => node.props.children as string)
      .join(" ")
    expect(allText).toContain("Repositories unavailable")
  })
})
