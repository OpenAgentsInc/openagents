import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  ComponentValueBinding,
  defineIntent,
  IntentRef,
  type IntentHandlers,
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
import type { ScopeSyncState } from "@openagentsinc/khala-sync-client"

import {
  initialKhalaState,
  khalaHandlers,
  khalaIntentDefinitions,
  renderKhalaSurface,
  type KhalaState,
  type KhalaTurnClient,
} from "./khala-core"

/**
 * Persona-neutral mobile home. Sol roadmap rev-24 explicitly pauses named
 * assistants as a product front door: mobile owns truthful supervision and continuity, not
 * relationship state or presentation demos. Until authoritative Sync/Fleet
 * projections land, Khala is the one real conversation surface.
 */
export type SurfaceMode = "openagents" | "khala"

export interface SurfaceModeOption {
  readonly id: SurfaceMode
  readonly label: string
}

export const surfaceModeOptions: ReadonlyArray<SurfaceModeOption> = [
  { id: "openagents", label: "OpenAgents" },
  { id: "khala", label: "Khala" },
]

export interface HomeState {
  readonly drawerOpen: boolean
  readonly surfaceMode: SurfaceMode
  readonly modeMenuOpen: boolean
  readonly syncPhase: MobileSyncPhase
  readonly khala: KhalaState
}

export type MobileSyncPhase = ScopeSyncState["phase"] | "unconfigured" | "unavailable" | "stale"

export interface SyncStatusCopy {
  readonly title: string
  readonly detail: string
}

const syncStatusCopyByPhase: Record<MobileSyncPhase, SyncStatusCopy> = {
  unconfigured: {
    title: "Sync not configured",
    detail: "Connect an OpenAgents session to view shared work, repositories, and Fleet state.",
  },
  idle: {
    title: "Sync idle",
    detail: "Sync is ready to connect. Shared work is not loaded yet.",
  },
  bootstrapping: {
    title: "Loading shared work",
    detail: "Fetching the current authorized projection.",
  },
  catching_up: {
    title: "Catching up",
    detail: "Applying confirmed updates before shared work is ready.",
  },
  live: {
    title: "Sync live",
    detail: "Shared work is current.",
  },
  stale: {
    title: "Sync stale",
    detail: "Shared work may be outdated. Controls stay unavailable until it reconnects.",
  },
  must_refetch: {
    title: "Sync needs refresh",
    detail: "The authorized projection must be fetched again before it can be used.",
  },
  denied: {
    title: "Sync access removed",
    detail: "This device can no longer show shared work for the previous session.",
  },
  unavailable: {
    title: "Sync unavailable",
    detail: "Shared work cannot be loaded right now.",
  },
}

export const syncStatusCopy = (phase: MobileSyncPhase): SyncStatusCopy => syncStatusCopyByPhase[phase]

export const initialHomeState: HomeState = {
  drawerOpen: false,
  surfaceMode: "khala",
  modeMenuOpen: false,
  syncPhase: "unconfigured",
  khala: initialKhalaState,
}

/** Visible embedded-binary tag; build 116 removes the named-persona front door. */
export const BUNDLE_TAG = "2026-07-10.embedded-116"

const EmptyPayload = Schema.Struct({})

export const DrawerToggled = defineIntent("DrawerToggled", EmptyPayload)
export const NewChatPressed = defineIntent("NewChatPressed", EmptyPayload)
export const SettingsPressed = defineIntent("SettingsPressed", EmptyPayload)
export const SurfaceModeSelected = defineIntent(
  "SurfaceModeSelected",
  Schema.Struct({ mode: Schema.Literals(["openagents", "khala"]) }),
)

export const homeIntentDefinitions = [
  DrawerToggled,
  NewChatPressed,
  SettingsPressed,
  SurfaceModeSelected,
  ...khalaIntentDefinitions.map((definition) => defineIntent(definition.name, definition.payload)),
] as const

export interface ChromeProps {
  readonly pillLabel: string
  readonly composerPlaceholder: string
  readonly chromeVisible: boolean
  readonly glassComposerVisible: boolean
  readonly surfaceMode: SurfaceMode
  readonly draft: string
  readonly sending: boolean
}

export const chromeProps = (state: HomeState): ChromeProps => ({
  pillLabel: surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ?? "OpenAgents",
  composerPlaceholder: "Message Khala",
  chromeVisible: !state.drawerOpen,
  glassComposerVisible: !state.drawerOpen && state.surfaceMode === "khala",
  surfaceMode: state.surfaceMode,
  draft: state.khala.draft,
  sending: state.khala.pending,
})

/** The retained native composer sits above this content; do not add a second
 * input here. Its SwiftUI events enter the Khala typed intent boundary. */
export const renderContentView = (state: HomeState): View =>
  Stack(
    {
      key: "home-root",
      direction: "column",
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    state.surfaceMode === "khala"
      ? [renderKhalaSurface(state.khala)]
      : [
          Spacer({ key: "openagents-top-space", size: "16" }),
          Text({ key: "openagents-title", content: "OpenAgents", variant: "title", color: "textPrimary" }),
          Text({
            key: "openagents-sync-title",
            content: syncStatusCopy(state.syncPhase).title,
            variant: "heading",
            color: state.syncPhase === "unconfigured" ? "warning" : "textPrimary",
          }),
          Text({
            key: "openagents-sync-detail",
            content: syncStatusCopy(state.syncPhase).detail,
            variant: "body",
            color: "textMuted",
          }),
        ],
  )

const drawerRow = (input: { readonly key: string; readonly label: string; readonly onPress: ReturnType<typeof IntentRef>; readonly selected?: boolean }): View =>
  Button({
    key: input.key,
    label: input.label,
    variant: input.selected === true ? "secondary" : "ghost",
    onPress: input.onPress,
    style: { width: "full", ...(input.selected === true ? { backgroundColor: "surfaceRaised" } : {}) },
  })

export const renderDrawerView = (state: HomeState): View =>
  Stack(
    { key: "drawer-root", direction: "column", gap: "2", padding: "4", style: { width: "full", height: "full", backgroundColor: "surface" } },
    [
      Spacer({ key: "drawer-top-space", size: "10" }),
      drawerRow({ key: "drawer-new-chat", label: "New chat", onPress: IntentRef("NewChatPressed", StaticPayload({})), selected: state.surfaceMode === "khala" && state.khala.entries.length === 0 }),
      drawerRow({ key: "drawer-khala", label: "Khala", onPress: IntentRef("SurfaceModeSelected", StaticPayload({ mode: "khala" })), selected: state.surfaceMode === "khala" }),
      Spacer({ key: "drawer-flex-space", size: "8" }),
      drawerRow({ key: "drawer-settings", label: "Settings", onPress: IntentRef("SettingsPressed", StaticPayload({})) }),
      Text({ key: "drawer-bundle", content: `Bundle ${BUNDLE_TAG}`, variant: "caption", color: "textMuted" }),
    ],
  )

export interface HomeProgramOptions {
  readonly khalaTurn?: KhalaTurnClient
}

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  options: HomeProgramOptions = {},
): IntentHandlers<typeof homeIntentDefinitions> => ({
  DrawerToggled: () => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: !current.drawerOpen })),
  NewChatPressed: () => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: "khala" as const, khala: initialKhalaState })),
  SettingsPressed: () => Effect.void,
  SurfaceModeSelected: (payload) => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: payload.mode as SurfaceMode })),
  ...khalaHandlers(state, options.khalaTurn),
})

export interface HomeProgramHandle {
  readonly contentViewStream: Stream.Stream<View>
  readonly drawerViewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly stateChanges: Stream.Stream<HomeState>
  readonly chrome: {
    readonly toggleDrawer: () => void
    readonly pressNewChat: () => void
    readonly selectSurfaceMode: (mode: SurfaceMode) => void
  }
  readonly khala: {
    readonly draftChanged: (text: string) => void
    readonly submitTurn: (text: string) => void
  }
}

export const buildHomeProgram = (options: HomeProgramOptions = {}): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<HomeState>(initialHomeState)
      const registry = yield* makeIntentRegistry(homeIntentDefinitions, makeHomeHandlers(state, options))
      const report: IntentReporter = (ref, runtimeValue) => registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const fireRef = (ref: ReturnType<typeof IntentRef>): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref))))
      }
      const fireText = (ref: ReturnType<typeof IntentRef>, value: string): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref, value))))
      }
      const fire = (name: string) => (): void => fireRef(IntentRef(name, StaticPayload({})))
      const submitKhala = (text: string): void => fireText(IntentRef("KhalaTurnSubmitted", ComponentValueBinding()), text)
      return {
        contentViewStream: makeViewProgramFromState(state, renderContentView).viewStream,
        drawerViewStream: makeViewProgramFromState(state, renderDrawerView).viewStream,
        report,
        stateChanges: SubscriptionRef.changes(state),
        chrome: {
          toggleDrawer: fire("DrawerToggled"),
          pressNewChat: fire("NewChatPressed"),
          selectSurfaceMode: (mode) => fireRef(IntentRef("SurfaceModeSelected", StaticPayload({ mode }))),
        },
        khala: {
          draftChanged: (text) => fireText(IntentRef("KhalaDraftChanged", ComponentValueBinding()), text),
          submitTurn: submitKhala,
        },
      }
    }),
  )
