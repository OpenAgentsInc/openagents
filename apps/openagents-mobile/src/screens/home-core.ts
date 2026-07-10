// Import effect through the @effect-native/core/effect bridge so this module
// uses the SAME effect instance the vendored @effect-native/* packages pin
// (effect beta.94), not the repo catalog effect (beta.70). Mixing the two
// effect copies makes SubscriptionRef/Effect/Schema types fail to unify across
// the adapter boundary. (Same bridge the web Effect Native surfaces use.)
import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  defineIntent,
  type IntentHandlers,
  IntentRef,
  type IntentReporter,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"

/**
 * OpenAgents mobile (#8597, GL-2 #8648) — the PURE half of the Home screen:
 * the ChatGPT-style glass shell as ONE typed Effect Native program. This
 * module imports only `@effect-native/core` (+ its effect bridge), never
 * `react`/`react-native`. Two view projections render from the same state:
 *
 * - `renderContentView` — the main surface (under the floating glass chrome).
 * - `renderDrawerView` — the left nav flyout panel (rendered by the shell in
 *   an overlay when `drawerOpen`; overlay POSITIONING is host machinery
 *   because the v26 typed style system deliberately has no absolute
 *   positioning).
 *
 * The floating glass chrome (pill, circular icon buttons, composer bar) is
 * SwiftUI (iOS 26 Liquid Glass) mounted by the shell at the audit's island
 * boundary; its props are `chromeProps(state)` projections and every tap
 * dispatches one of the typed intents below through the SAME registry the
 * renderer's reporter uses. Per the hybrid decision
 * (docs/fable/2026-07-09-swiftui-expo-ui-and-the-effect-native-stdlib.md §6):
 * state-in-props, intents-out, always.
 */

export interface RecentChat {
  readonly id: string
  readonly title: string
}

/** The pill dropdown's surface modes: the plain OpenAgents surface (Protoss
 * background) or the Sarah demo-video surface (fullscreen looping video the
 * glass chrome layers over). */
export type SurfaceMode = "openagents" | "sarah"

export interface SurfaceModeOption {
  readonly id: SurfaceMode
  readonly label: string
}

export const surfaceModeOptions: ReadonlyArray<SurfaceModeOption> = [
  { id: "openagents", label: "OpenAgents" },
  { id: "sarah", label: "Sarah" },
]

export interface HomeState {
  readonly drawerOpen: boolean
  readonly surfaceMode: SurfaceMode
  /** The active conversation; undefined = fresh "new chat" surface. */
  readonly activeRecentId: string | undefined
  readonly recents: ReadonlyArray<RecentChat>
  readonly composerTaps: number
  readonly micTaps: number
  readonly searchTaps: number
  readonly pillTaps: number
  readonly settingsTaps: number
}

/** Seed conversations so the drawer's Recents section and selection highlight
 * are real state-driven UI (local placeholder data until Sarah conversation
 * state lands per #8597 scope 2/3). */
export const seedRecents: ReadonlyArray<RecentChat> = [
  { id: "welcome", title: "Welcome to OpenAgents" },
  { id: "glass-shell", title: "Glass shell design" },
  { id: "fleet-notes", title: "Fleet supervision notes" },
]

export const initialHomeState: HomeState = {
  drawerOpen: false,
  surfaceMode: "openagents",
  activeRecentId: "welcome",
  recents: seedRecents,
  composerTaps: 0,
  micTaps: 0,
  searchTaps: 0,
  pillTaps: 0,
  settingsTaps: 0,
}

/** Visible JS-bundle tag (OTA proof surface). Bump when publishing an OTA so
 * the owner can SEE the over-the-air bundle swap land (embedded build 107
 * ships the tag below; a published OTA with a bumped tag should appear within
 * ~3s via the temporary poll loop and reload). Rendered in the drawer footer. */
export const BUNDLE_TAG = "2026-07-09.embedded-109"

// ---------------------------------------------------------------------------
// Typed intents — the ONLY way anything (EN tree, SwiftUI chrome, scrim)
// mutates Home state.
// ---------------------------------------------------------------------------

const EmptyPayload = Schema.Struct({})

export const DrawerToggled = defineIntent("DrawerToggled", EmptyPayload)
export const NewChatPressed = defineIntent("NewChatPressed", EmptyPayload)
export const RecentSelected = defineIntent(
  "RecentSelected",
  Schema.Struct({ id: Schema.NonEmptyString }),
)
export const SearchPressed = defineIntent("SearchPressed", EmptyPayload)
export const SettingsPressed = defineIntent("SettingsPressed", EmptyPayload)
export const ChatPillPressed = defineIntent("ChatPillPressed", EmptyPayload)
export const ComposerPressed = defineIntent("ComposerPressed", EmptyPayload)
export const MicPressed = defineIntent("MicPressed", EmptyPayload)
export const SurfaceModeSelected = defineIntent(
  "SurfaceModeSelected",
  Schema.Struct({ mode: Schema.Literals(["openagents", "sarah"]) }),
)

export const homeIntentDefinitions = [
  DrawerToggled,
  NewChatPressed,
  RecentSelected,
  SearchPressed,
  SettingsPressed,
  ChatPillPressed,
  ComposerPressed,
  MicPressed,
  SurfaceModeSelected,
] as const

export const drawerToggledRef = IntentRef("DrawerToggled", StaticPayload({}))
const recentRef = (id: string) => IntentRef("RecentSelected", StaticPayload({ id }))

// ---------------------------------------------------------------------------
// Chrome projections — serializable props for the SwiftUI glass islands.
// ---------------------------------------------------------------------------

export interface ChromeProps {
  readonly pillLabel: string
  readonly composerPlaceholder: string
  readonly chromeVisible: boolean
  readonly surfaceMode: SurfaceMode
}

export const chromeProps = (state: HomeState): ChromeProps => ({
  pillLabel:
    surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ??
    "OpenAgents",
  composerPlaceholder: "Ask anything",
  chromeVisible: !state.drawerOpen,
  surfaceMode: state.surfaceMode,
})

export const activeRecentTitle = (state: HomeState): string => {
  const active = state.recents.find((recent) => recent.id === state.activeRecentId)
  return active === undefined ? "New chat" : active.title
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

/** Main surface, rendered under the floating glass chrome. DELIBERATELY
 * EMPTY (owner direction 2026-07-09: no status text on the main surface) —
 * just the surface itself: opaque Protoss background in "openagents" mode;
 * TRANSPARENT in "sarah" mode so the fullscreen demo video (a shell layer
 * below) shows through and the glass chrome floats over it. Conversation
 * content mounts here when the Sarah surface lands. */
export const renderContentView = (state: HomeState): View =>
  Stack(
    {
      key: "home-root",
      direction: "column",
      style: {
        width: "full",
        height: "full",
        ...(state.surfaceMode === "openagents"
          ? { backgroundColor: "background" as const }
          : {}),
      },
    },
    [],
  )

const drawerRow = (input: {
  readonly key: string
  readonly label: string
  readonly onPress: ReturnType<typeof IntentRef>
  readonly selected?: boolean
}): View =>
  Button({
    key: input.key,
    label: input.label,
    variant: input.selected === true ? "secondary" : "ghost",
    onPress: input.onPress,
    style: {
      width: "full",
      ...(input.selected === true ? { backgroundColor: "surfaceRaised" } : {}),
    },
  })

/** Left nav flyout panel (EN composition per the decision doc §6 — drawer
 * state interleaves with the whole screen, which islands are worst at). The
 * shell overlays this next to a scrim when `drawerOpen`. Rows are v26
 * `Button`s; they upgrade to the v27 `IconButton`/row contract when the GL-1
 * catalog bump is vendored (#8647). */
export const renderDrawerView = (state: HomeState): View =>
  Stack(
    {
      key: "drawer-root",
      direction: "column",
      gap: "2",
      padding: "4",
      style: { width: "full", height: "full", backgroundColor: "surface" },
    },
    [
      Spacer({ key: "drawer-top-space", size: "10" }),
      drawerRow({
        key: "drawer-search",
        label: "Search",
        onPress: IntentRef("SearchPressed", StaticPayload({})),
      }),
      drawerRow({
        key: "drawer-new-chat",
        label: "New chat",
        onPress: IntentRef("NewChatPressed", StaticPayload({})),
        selected: state.activeRecentId === undefined,
      }),
      Spacer({ key: "drawer-recents-space", size: "3" }),
      Text({
        key: "drawer-recents-label",
        content: "Recents",
        variant: "label",
        color: "textMuted",
      }),
      ...state.recents.map((recent) =>
        drawerRow({
          key: `drawer-recent-${recent.id}`,
          label: recent.title,
          onPress: recentRef(recent.id),
          selected: state.activeRecentId === recent.id,
        }),
      ),
      Spacer({ key: "drawer-flex-space", size: "8" }),
      drawerRow({
        key: "drawer-settings",
        label: "Settings",
        onPress: IntentRef("SettingsPressed", StaticPayload({})),
      }),
      Text({
        key: "drawer-bundle",
        content: `Bundle ${BUNDLE_TAG}`,
        variant: "caption",
        color: "textMuted",
      }),
    ],
  )

// ---------------------------------------------------------------------------
// Handlers + program
// ---------------------------------------------------------------------------

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
): IntentHandlers<typeof homeIntentDefinitions> => ({
  DrawerToggled: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      drawerOpen: !current.drawerOpen,
    })),
  NewChatPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      activeRecentId: undefined,
      drawerOpen: false,
    })),
  RecentSelected: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      activeRecentId: payload.id,
      drawerOpen: false,
    })),
  SearchPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      searchTaps: current.searchTaps + 1,
    })),
  SettingsPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      settingsTaps: current.settingsTaps + 1,
    })),
  ChatPillPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      pillTaps: current.pillTaps + 1,
    })),
  ComposerPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      composerTaps: current.composerTaps + 1,
    })),
  MicPressed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      micTaps: current.micTaps + 1,
    })),
  SurfaceModeSelected: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      surfaceMode: payload.mode,
    })),
})

/** Fire-and-forget typed dispatchers for the SwiftUI chrome + shell scrim —
 * the ONLY seam native events enter the program through. Soft failure: an
 * intent error must never crash a native event path. */
export interface ChromeDispatchers {
  readonly toggleDrawer: () => void
  readonly pressPill: () => void
  readonly pressSearch: () => void
  readonly pressNewChat: () => void
  readonly pressComposer: () => void
  readonly pressMic: () => void
  readonly selectSurfaceMode: (mode: SurfaceMode) => void
}

export interface HomeProgramHandle {
  readonly contentViewStream: Stream.Stream<View>
  readonly drawerViewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly stateChanges: Stream.Stream<HomeState>
  readonly chrome: ChromeDispatchers
}

export const buildHomeProgram = (): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<HomeState>(initialHomeState)
      const registry = yield* makeIntentRegistry(
        homeIntentDefinitions,
        makeHomeHandlers(state),
      )
      const report: IntentReporter = (ref, runtimeValue) =>
        registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const fireRef = (ref: ReturnType<typeof IntentRef>): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref))))
      }
      const fire = (name: string) => (): void => {
        fireRef(IntentRef(name, StaticPayload({})))
      }
      const content = makeViewProgramFromState(state, renderContentView)
      const drawer = makeViewProgramFromState(state, renderDrawerView)
      return {
        contentViewStream: content.viewStream,
        drawerViewStream: drawer.viewStream,
        report,
        stateChanges: SubscriptionRef.changes(state),
        chrome: {
          toggleDrawer: fire("DrawerToggled"),
          pressPill: fire("ChatPillPressed"),
          pressSearch: fire("SearchPressed"),
          pressNewChat: fire("NewChatPressed"),
          pressComposer: fire("ComposerPressed"),
          pressMic: fire("MicPressed"),
          selectSurfaceMode: (mode) => {
            fireRef(IntentRef("SurfaceModeSelected", StaticPayload({ mode })))
          },
        },
      }
    }),
  )
