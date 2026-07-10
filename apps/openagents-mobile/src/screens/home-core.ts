// Import effect through the @effect-native/core/effect bridge so this module
// uses the SAME effect instance the vendored @effect-native/* packages pin
// (effect beta.94), not the repo catalog effect (beta.70). Mixing the two
// effect copies makes SubscriptionRef/Effect/Schema types fail to unify across
// the adapter boundary. (Same bridge the web Effect Native surfaces use.)
import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  ComponentValueBinding,
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

import {
  boundedEntries,
  clipSarahText,
  turnCounterFromEntries,
  initialSarahState,
  isDuplicateTranscriptEvent,
  MAX_SARAH_CARDS,
  prospectRefFromThreadId,
  renderSarahSurface,
  SARAH_TURN_FAILED_TEXT,
  SarahEventPayload,
  SarahSessionReadyPayload,
  SarahSessionUnavailablePayload,
  SarahStreamStatusPayload,
  type SarahEntry,
  type SarahState,
  type SarahTurnClient,
} from "./sarah-core"

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

/** Example in-app-purchase price points for the minerals fly-up sheet
 * (owner direction 2026-07-09). DEMO options only — no StoreKit wiring yet. */
export interface MineralPack {
  readonly id: string
  readonly label: string
  readonly price: string
}

export const mineralPacks: ReadonlyArray<MineralPack> = [
  { id: "pack-100", label: "100 Minerals", price: "$0.99" },
  { id: "pack-550", label: "550 Minerals", price: "$4.99" },
  { id: "pack-1200", label: "1,200 Minerals", price: "$9.99" },
  { id: "pack-3000", label: "3,000 Minerals", price: "$19.99" },
]

export interface HomeState {
  readonly drawerOpen: boolean
  readonly surfaceMode: SurfaceMode
  /** Composer-tap takeover: fullscreen Sarah reply video WITH audio (owner
   * direction 2026-07-09); ends on play-to-end (AskVideoEnded) or user tap
   * (AskVideoDismissed). Ending the video NEVER touches the minerals sheet. */
  readonly askVideoPlaying: boolean
  /** Liquid Glass fly-up sheet (bottom third) offering example mineral
   * packs; opened by the shell midway through the ask video. USER-owned
   * lifecycle (owner P0, build 111 feedback 2026-07-09): only
   * MineralPackSelected or MineralsSheetDismissed closes it — a video
   * ended/looped playback event must never close the sheet. */
  readonly mineralsSheetOpen: boolean
  readonly lastMineralPackId: string | undefined
  /** The active conversation; undefined = fresh "new chat" surface. */
  readonly activeRecentId: string | undefined
  readonly recents: ReadonlyArray<RecentChat>
  readonly composerTaps: number
  readonly micTaps: number
  readonly searchTaps: number
  readonly pillTaps: number
  readonly settingsTaps: number
  /** GL-3 (#8649): the Sarah conversation slice — one program, one state. */
  readonly sarah: SarahState
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
  askVideoPlaying: false,
  mineralsSheetOpen: false,
  lastMineralPackId: undefined,
  activeRecentId: "welcome",
  recents: seedRecents,
  composerTaps: 0,
  micTaps: 0,
  searchTaps: 0,
  pillTaps: 0,
  settingsTaps: 0,
  sarah: initialSarahState,
}

/** Visible JS-bundle tag (OTA proof surface). Bump when publishing an OTA so
 * the owner can SEE the over-the-air bundle swap land (embedded build 107
 * ships the tag below; a published OTA with a bumped tag should appear within
 * ~3s via the temporary poll loop and reload). Rendered in the drawer footer. */
export const BUNDLE_TAG = "2026-07-10.embedded-113"

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
export const AskVideoDismissed = defineIntent("AskVideoDismissed", EmptyPayload)
/** Playback-lifecycle event (playToEnd/loop boundary) — distinct from the
 * USER dismissal above so the sheet lifecycle can never couple to playback. */
export const AskVideoEnded = defineIntent("AskVideoEnded", EmptyPayload)
export const MineralsSheetOpened = defineIntent("MineralsSheetOpened", EmptyPayload)
export const MineralsSheetDismissed = defineIntent("MineralsSheetDismissed", EmptyPayload)
export const MineralPackSelected = defineIntent(
  "MineralPackSelected",
  Schema.Struct({ id: Schema.NonEmptyString }),
)
export const SurfaceModeSelected = defineIntent(
  "SurfaceModeSelected",
  Schema.Struct({ mode: Schema.Literals(["openagents", "sarah"]) }),
)

// GL-3 (#8649) Sarah conversation intents. Session/stream/event intents are
// dispatched by the HOST from the effectful client (../sarah/sarah-client);
// draft/turn intents come from the EN Composer inside the surface itself.
export const SarahSessionReady = defineIntent(
  "SarahSessionReady",
  SarahSessionReadyPayload,
)
export const SarahSessionUnavailable = defineIntent(
  "SarahSessionUnavailable",
  SarahSessionUnavailablePayload,
)
/** ComponentValueBinding payload: the composer's normalized plaintext. */
export const SarahDraftChanged = defineIntent("SarahDraftChanged", Schema.String)
export const SarahTurnSubmitted = defineIntent("SarahTurnSubmitted", Schema.String)
export const SarahStreamStatusChanged = defineIntent(
  "SarahStreamStatusChanged",
  SarahStreamStatusPayload,
)
export const SarahEventReceived = defineIntent("SarahEventReceived", SarahEventPayload)

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
  AskVideoDismissed,
  AskVideoEnded,
  MineralsSheetOpened,
  MineralsSheetDismissed,
  MineralPackSelected,
  SarahSessionReady,
  SarahSessionUnavailable,
  SarahDraftChanged,
  SarahTurnSubmitted,
  SarahStreamStatusChanged,
  SarahEventReceived,
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
  /** GL-3: the tap-only SwiftUI glass composer belongs to the demo surface.
   * In Sarah mode the EN Composer (a real bound text input) lives INSIDE the
   * conversation surface, so the glass bar hides. */
  readonly glassComposerVisible: boolean
  readonly surfaceMode: SurfaceMode
}

export const chromeProps = (state: HomeState): ChromeProps => ({
  pillLabel:
    surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ??
    "OpenAgents",
  composerPlaceholder: "Ask anything",
  chromeVisible: !state.drawerOpen,
  glassComposerVisible: !state.drawerOpen && state.surfaceMode !== "sarah",
  surfaceMode: state.surfaceMode,
})

export const activeRecentTitle = (state: HomeState): string => {
  const active = state.recents.find((recent) => recent.id === state.activeRecentId)
  return active === undefined ? "New chat" : active.title
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

/** Main surface, rendered under the floating glass chrome. In "openagents"
 * mode it stays DELIBERATELY EMPTY (owner direction 2026-07-09: no status
 * text) — opaque Protoss background, transparent while the ask video plays.
 * In "sarah" mode (GL-3 #8649) it is the REAL Sarah conversation: transparent
 * root (the muted demo loop plays beneath as AMBIENT BACKGROUND ONLY) with
 * the typed transcript, cards, and the EN Composer rendered over it. */
export const renderContentView = (state: HomeState): View =>
  Stack(
    {
      key: "home-root",
      direction: "column",
      style: {
        width: "full",
        height: "full",
        ...(state.surfaceMode === "openagents" && !state.askVideoPlaying
          ? { backgroundColor: "background" as const }
          : {}),
      },
    },
    state.surfaceMode === "sarah" && !state.askVideoPlaying
      ? [renderSarahSurface(state.sarah)]
      : [],
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

/** Effect seams the program needs but never owns: the Sarah turn client is
 * injected (production: ../sarah/sarah-client sendSarahTurn; tests: a
 * deterministic fake). No client => turns fail honestly to the typed
 * degradation entry. */
export interface HomeProgramOptions {
  readonly sarahTurn?: SarahTurnClient
}

const updateSarah = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  update: (sarah: SarahState) => SarahState,
) =>
  SubscriptionRef.update(state, (current) => ({
    ...current,
    sarah: update(current.sarah),
  }))

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  options: HomeProgramOptions = {},
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
      // The demo reply-video takeover belongs to the "openagents" surface;
      // in Sarah mode the composer is a REAL conversation input (GL-3) and
      // never triggers presentation playback.
      askVideoPlaying:
        current.surfaceMode === "sarah" ? current.askVideoPlaying : true,
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
  // Ending the takeover (user tap OR playback end) NEVER touches the
  // minerals sheet (owner P0, build 111 feedback 2026-07-09): the sheet
  // stays open until the USER dismisses it via MineralPackSelected or
  // MineralsSheetDismissed. The original surface (Sarah loop / black)
  // resumes untouched underneath the still-open sheet.
  AskVideoDismissed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      askVideoPlaying: false,
    })),
  AskVideoEnded: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      askVideoPlaying: false,
    })),
  MineralsSheetOpened: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      mineralsSheetOpen: true,
    })),
  MineralsSheetDismissed: () =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      mineralsSheetOpen: false,
    })),
  MineralPackSelected: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      mineralsSheetOpen: false,
      lastMineralPackId: payload.id,
    })),
  // --- GL-3 Sarah conversation handlers -----------------------------------
  SarahSessionReady: (payload) =>
    updateSarah(state, (sarah) => {
      // Restored transcript (persisted relationship) seeds ONLY an empty
      // surface — a live conversation is never clobbered by a late restore.
      const entries =
        sarah.entries.length === 0
          ? boundedEntries(
              payload.entries.map((entry): SarahEntry => ({
                key: entry.key,
                role: entry.role,
                text: clipSarahText(entry.text),
                status: "done",
              })),
            )
          : sarah.entries
      return {
        ...sarah,
        phase: "ready",
        prospectRef: payload.prospectRef,
        threadId: payload.threadId,
        restored: payload.restored,
        lastFailure: null,
        entries,
        // New turns must never reuse a restored turn key (a collision would
        // rewrite a restored bubble with the new reply).
        turnCounter: Math.max(sarah.turnCounter, turnCounterFromEntries(entries)),
      }
    }),
  SarahSessionUnavailable: (payload) =>
    updateSarah(state, (sarah) => ({
      ...sarah,
      phase: "unavailable",
      lastFailure: payload.reason,
    })),
  SarahDraftChanged: (text) =>
    updateSarah(state, (sarah) => ({
      ...sarah,
      draft: typeof text === "string" ? clipSarahText(text) : "",
    })),
  SarahTurnSubmitted: (raw) =>
    Effect.gen(function* () {
      const message = (typeof raw === "string" ? raw : "").trim()
      if (message === "") return
      const before = yield* SubscriptionRef.get(state)
      if (before.sarah.turnPending) return
      const turn = before.sarah.turnCounter + 1
      const userKey = `turn-${turn}-user`
      const replyKey = `turn-${turn}-reply`
      yield* updateSarah(state, (sarah) => ({
        ...sarah,
        draft: "",
        turnPending: true,
        turnCounter: turn,
        entries: boundedEntries([
          ...sarah.entries,
          { key: userKey, role: "user", text: clipSarahText(message), status: "done" },
          { key: replyKey, role: "assistant", text: "", status: "thinking" },
        ]),
      }))
      const client = options.sarahTurn
      const result =
        client === undefined
          ? null
          : yield* Effect.tryPromise({
              try: () =>
                client.sendTurn({
                  message,
                  prospectRef: before.sarah.prospectRef,
                  threadId: before.sarah.threadId,
                }),
              catch: () => new Error("turn_failed"),
            }).pipe(Effect.catch(() => Effect.succeed(null)))
      yield* updateSarah(state, (sarah) => {
        const adoptedRef =
          sarah.prospectRef ??
          (result === null ? null : prospectRefFromThreadId(result.threadId))
        return {
          ...sarah,
          turnPending: false,
          // A turn that reached the server can bootstrap the session (the
          // server mints the prospect relationship on first contact).
          prospectRef: adoptedRef,
          threadId:
            sarah.threadId ?? (result === null ? null : result.threadId),
          phase: result === null ? sarah.phase : "ready",
          lastFailure: result === null ? "turn_failed" : sarah.lastFailure,
          entries: sarah.entries.map((entry) =>
            entry.key === replyKey
              ? result === null
                ? { ...entry, text: SARAH_TURN_FAILED_TEXT, status: "failed" as const }
                : { ...entry, text: clipSarahText(result.reply), status: "done" as const }
              : entry,
          ),
        }
      })
    }),
  SarahStreamStatusChanged: (payload) =>
    updateSarah(state, (sarah) => ({ ...sarah, stream: payload.phase })),
  SarahEventReceived: (event) =>
    updateSarah(state, (sarah) => {
      sarah = { ...sarah, eventCounter: sarah.eventCounter + 1 }
      if (
        event.type === "transcript" &&
        (event.role === "user" || event.role === "assistant") &&
        typeof event.text === "string" &&
        event.text.length > 0
      ) {
        const text = clipSarahText(event.text)
        if (isDuplicateTranscriptEvent(sarah.entries, event.role, text)) return sarah
        return {
          ...sarah,
          entries: boundedEntries([
            ...sarah.entries,
            {
              key: `sse-${sarah.eventCounter}`,
              role: event.role,
              text,
              status: "done",
            },
          ]),
        }
      }
      if (
        (event.type === "card" || event.type === "guard_refusal") &&
        typeof event.title === "string" &&
        event.title.length > 0
      ) {
        return {
          ...sarah,
          cards: [
            ...sarah.cards,
            {
              key: `card-${sarah.eventCounter}`,
              title: clipSarahText(event.title),
              body: clipSarahText(typeof event.body === "string" ? event.body : ""),
            },
          ].slice(-MAX_SARAH_CARDS),
        }
      }
      // Unknown/other event types (session, blueprint_delta) are counted but
      // not rendered in v1 — bounded and honest.
      return sarah
    }),
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
  readonly dismissAskVideo: () => void
  /** Playback event (playToEnd/loop) — never a user intent, never closes
   * the minerals sheet. */
  readonly askVideoEnded: () => void
  readonly openMineralsSheet: () => void
  readonly dismissMineralsSheet: () => void
  readonly selectMineralPack: (id: string) => void
}

/** Host-side dispatchers for the effectful Sarah client (session boot, SSE
 * loop). Same fire-and-forget posture as the chrome: typed intents only. */
export interface SarahDispatchers {
  readonly sessionReady: (payload: {
    readonly prospectRef: string
    readonly threadId: string
    readonly restored: boolean
    readonly entries: ReadonlyArray<{
      readonly key: string
      readonly role: "user" | "assistant"
      readonly text: string
    }>
  }) => void
  readonly sessionUnavailable: (reason: string) => void
  readonly streamStatus: (
    phase: "idle" | "connecting" | "live" | "reconnecting" | "unavailable",
  ) => void
  readonly eventReceived: (event: {
    readonly type: string
    readonly role?: string
    readonly text?: string
    readonly title?: string
    readonly body?: string
  }) => void
  /** Exact same intent the EN Composer's onSubmit dispatches. */
  readonly submitTurn: (text: string) => void
}

export interface HomeProgramHandle {
  readonly contentViewStream: Stream.Stream<View>
  readonly drawerViewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly stateChanges: Stream.Stream<HomeState>
  readonly chrome: ChromeDispatchers
  readonly sarah: SarahDispatchers
}

export const buildHomeProgram = (
  options: HomeProgramOptions = {},
): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<HomeState>(initialHomeState)
      const registry = yield* makeIntentRegistry(
        homeIntentDefinitions,
        makeHomeHandlers(state, options),
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
          dismissAskVideo: fire("AskVideoDismissed"),
          askVideoEnded: fire("AskVideoEnded"),
          openMineralsSheet: fire("MineralsSheetOpened"),
          dismissMineralsSheet: fire("MineralsSheetDismissed"),
          selectMineralPack: (id) => {
            fireRef(IntentRef("MineralPackSelected", StaticPayload({ id })))
          },
        },
        sarah: {
          sessionReady: (payload) => {
            fireRef(
              IntentRef(
                "SarahSessionReady",
                StaticPayload({
                  prospectRef: payload.prospectRef,
                  threadId: payload.threadId,
                  restored: payload.restored,
                  entries: payload.entries.map((entry) => ({ ...entry })),
                }),
              ),
            )
          },
          sessionUnavailable: (reason) => {
            fireRef(IntentRef("SarahSessionUnavailable", StaticPayload({ reason })))
          },
          streamStatus: (phase) => {
            fireRef(IntentRef("SarahStreamStatusChanged", StaticPayload({ phase })))
          },
          eventReceived: (event) => {
            fireRef(IntentRef("SarahEventReceived", StaticPayload({ ...event })))
          },
          submitTurn: (text) => {
            Effect.runFork(
              Effect.exit(
                registry.dispatch(
                  resolveIntentRef(
                    IntentRef("SarahTurnSubmitted", ComponentValueBinding()),
                    text,
                  ),
                ),
              ),
            )
          },
        },
      }
    }),
  )
