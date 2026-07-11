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

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
  MobileConversationThreadSummary,
} from "../conversation/mobile-conversation"

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
 * relationship state or presentation demos. The existing conversation surface
 * is driven by confirmed personal Sync when live and the public Khala client
 * when startup selects the explicit local fallback.
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
  readonly conversationAuthority: "local" | "sync"
  readonly conversationThreads: ReadonlyArray<MobileConversationThreadSummary>
  readonly activeThreadRef: string | null
  readonly khala: KhalaState
}

export type MobileSyncPhase =
  | ScopeSyncState["phase"]
  | "authenticating"
  | "credential_present_unverified"
  | "local_ready"
  | "session_ready"
  | "unconfigured"
  | "unavailable"
  | "stale"

export interface SyncStatusCopy {
  readonly title: string
  readonly detail: string
}

const syncStatusCopyByPhase: Record<MobileSyncPhase, SyncStatusCopy> = {
  authenticating: {
    title: "Updating session",
    detail: "Complete the secure browser step. Shared work stays hidden until OpenAgents verifies the session.",
  },
  credential_present_unverified: {
    title: "Session verification required",
    detail: "Stored credentials remain private. Shared work stays hidden until the server verifies them.",
  },
  local_ready: {
    title: "Local device ready",
    detail: "Coding, conversations, and fleets work without an account. Link OpenAgents only for cross-device Sync and network features.",
  },
  session_ready: {
    title: "Session verified",
    detail: "OpenAgents accepted this session. Shared work is ready to connect.",
  },
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
  conversationAuthority: "local",
  conversationThreads: [],
  activeThreadRef: null,
  khala: initialKhalaState,
}

/** Visible embedded-binary tag; build 116 removes the named-persona front door. */
export const BUNDLE_TAG = "2026-07-10.embedded-116"

const EmptyPayload = Schema.Struct({})

export const DrawerToggled = defineIntent("DrawerToggled", EmptyPayload)
export const NewChatPressed = defineIntent("NewChatPressed", EmptyPayload)
export const SettingsPressed = defineIntent("SettingsPressed", EmptyPayload)
export const OpenAgentsSignInPressed = defineIntent("OpenAgentsSignInPressed", EmptyPayload)
export const OpenAgentsSignOutPressed = defineIntent("OpenAgentsSignOutPressed", EmptyPayload)
export const SurfaceModeSelected = defineIntent(
  "SurfaceModeSelected",
  Schema.Struct({ mode: Schema.Literals(["openagents", "khala"]) }),
)
export const ConversationThreadSelected = defineIntent(
  "ConversationThreadSelected",
  Schema.Struct({ threadRef: Schema.String }),
)

export const homeIntentDefinitions = [
  DrawerToggled,
  NewChatPressed,
  SettingsPressed,
  OpenAgentsSignInPressed,
  OpenAgentsSignOutPressed,
  SurfaceModeSelected,
  ConversationThreadSelected,
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
  pillLabel: state.conversationAuthority === "sync" && state.surfaceMode === "khala"
    ? "OpenAgents"
    : surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ?? "OpenAgents",
  composerPlaceholder: state.conversationAuthority === "sync" ? "Continue conversation" : "Message Khala",
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
      ? [renderKhalaSurface(state.khala, state.conversationAuthority)]
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
          ...(state.syncPhase === "session_ready"
            ? [Button({
                key: "openagents-sign-out",
                label: "Sign out",
                variant: "secondary",
                onPress: IntentRef("OpenAgentsSignOutPressed", StaticPayload({})),
              })]
            : state.syncPhase === "authenticating"
              ? []
              : [Button({
                  key: "openagents-sign-in",
                  label: "Link OpenAgents account",
                  variant: "primary",
                  onPress: IntentRef("OpenAgentsSignInPressed", StaticPayload({})),
                })]),
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
      drawerRow({ key: "drawer-khala", label: state.conversationAuthority === "sync" ? "OpenAgents" : "Khala", onPress: IntentRef("SurfaceModeSelected", StaticPayload({ mode: "khala" })), selected: state.surfaceMode === "khala" }),
      ...state.conversationThreads.map(thread => drawerRow({
        key: `drawer-thread-${thread.threadRef}`,
        label: thread.title,
        onPress: IntentRef("ConversationThreadSelected", StaticPayload({ threadRef: thread.threadRef })),
        selected: state.activeThreadRef === thread.threadRef,
      })),
      Spacer({ key: "drawer-flex-space", size: "8" }),
      drawerRow({ key: "drawer-settings", label: "Settings", onPress: IntentRef("SettingsPressed", StaticPayload({})) }),
      Text({ key: "drawer-bundle", content: `Bundle ${BUNDLE_TAG}`, variant: "caption", color: "textMuted" }),
    ],
  )

export interface HomeProgramOptions {
  readonly khalaTurn?: KhalaTurnClient
  readonly sessionActions?: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
}

const confirmedKhalaState = (
  thread: MobileConversationThread | null,
  turnCounter = 0,
): KhalaState => ({
  draft: "",
  entries: (thread?.messages ?? []).map(message => ({
    key: message.messageRef,
    role: "user" as const,
    text: message.body,
    status: "done" as const,
    createdAt: message.createdAt,
    version: message.version,
  })),
  pending: false,
  turnCounter,
})

const withConfirmedThread = (
  state: HomeState,
  thread: MobileConversationThread,
): HomeState => ({
  ...state,
  drawerOpen: false,
  surfaceMode: "khala",
  activeThreadRef: thread.threadRef,
  conversationThreads: [
    {
      threadRef: thread.threadRef,
      title: thread.title,
      messageCount: thread.messageCount,
      lastMessageAt: thread.lastMessageAt,
      updatedAt: thread.updatedAt,
      version: thread.version,
    },
    ...state.conversationThreads.filter(item => item.threadRef !== thread.threadRef),
  ],
  khala: confirmedKhalaState(thread, state.khala.turnCounter),
})

const failedConversationState = (
  state: HomeState,
  error: string,
): HomeState => ({
  ...state,
  khala: {
    ...state.khala,
    pending: false,
    entries: [
      ...state.khala.entries.filter(entry => entry.status !== "pending"),
      {
        key: `sync-error-${state.khala.turnCounter}`,
        role: "system",
        text: error,
        status: "failed",
      },
    ],
  },
})

export const initialHomeStateForConversation = (
  selection: HomeProgramOptions["conversation"],
): HomeState => selection === undefined
  ? initialHomeState
  : {
      ...initialHomeState,
      syncPhase: "live",
      conversationAuthority: "sync",
      conversationThreads: selection.threads,
      activeThreadRef: selection.activeThread?.threadRef ?? null,
      khala: confirmedKhalaState(selection.activeThread),
    }

const makeSyncedConversationHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  host: MobileConversationHost,
) => ({
  NewChatPressed: () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    if (before.khala.pending) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      surfaceMode: "khala" as const,
      khala: {
        ...current.khala,
        pending: true,
        entries: [{
          key: `pending-new-thread-${current.khala.turnCounter + 1}`,
          role: "system" as const,
          text: "Creating chat…",
          status: "pending" as const,
        }],
      },
    }))
    const result = yield* Effect.promise(host.newThread)
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, result.thread)
      : failedConversationState(current, result.error))
  }),
  ConversationThreadSelected: (payload: { readonly threadRef: string }) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    if (before.khala.pending) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      khala: { ...current.khala, pending: true },
    }))
    const thread = yield* Effect.promise(() => host.openThread(payload.threadRef))
    yield* SubscriptionRef.update(state, current => thread === null
      ? failedConversationState(current, "Conversation is still pending reconciliation.")
      : withConfirmedThread(current, thread))
  }),
  KhalaDraftChanged: (text: string) => SubscriptionRef.update(state, current => ({
    ...current,
    khala: { ...current.khala, draft: text.length > 4_000 ? `${text.slice(0, 4_000)}…` : text },
  })),
  KhalaTurnSubmitted: (raw: string) => Effect.gen(function* () {
    const message = raw.trim()
    if (message === "") return
    const before = yield* SubscriptionRef.get(state)
    if (before.khala.pending) return
    const turn = before.khala.turnCounter + 1
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      khala: {
        ...current.khala,
        draft: "",
        pending: true,
        turnCounter: turn,
        entries: [
          ...current.khala.entries,
          {
            key: `pending-mobile-${turn}`,
            role: "user" as const,
            text: message.length > 4_000 ? `${message.slice(0, 4_000)}…` : message,
            status: "pending" as const,
          },
        ],
      },
    }))

    let threadRef = before.activeThreadRef
    if (threadRef === null) {
      const created = yield* Effect.promise(host.newThread)
      if (!created.ok) {
        yield* SubscriptionRef.update(state, current => failedConversationState(current, created.error))
        return
      }
      threadRef = created.thread.threadRef
      yield* SubscriptionRef.update(state, current => ({
        ...withConfirmedThread(current, created.thread),
        khala: {
          ...confirmedKhalaState(created.thread, turn),
          pending: true,
          entries: current.khala.entries,
        },
      }))
    }

    const result = yield* Effect.promise(() => host.sendMessage({ threadRef, body: message }))
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, result.thread)
      : failedConversationState(current, result.error))
  }),
})

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  options: HomeProgramOptions = {},
): IntentHandlers<typeof homeIntentDefinitions> => {
  const synced = options.conversation === undefined
    ? undefined
    : makeSyncedConversationHandlers(state, options.conversation.host)
  return {
    DrawerToggled: () => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: !current.drawerOpen })),
    NewChatPressed: synced?.NewChatPressed ??
      (() => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: "khala" as const, khala: initialKhalaState }))),
    SettingsPressed: () => Effect.void,
    OpenAgentsSignInPressed: () => options.sessionActions === undefined
      ? Effect.void
      : Effect.promise(options.sessionActions.signIn),
    OpenAgentsSignOutPressed: () => options.sessionActions === undefined
      ? Effect.void
      : Effect.promise(options.sessionActions.signOut),
    SurfaceModeSelected: (payload) => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: payload.mode as SurfaceMode })),
    ConversationThreadSelected: synced?.ConversationThreadSelected ?? (() => Effect.void),
    ...(synced === undefined
      ? khalaHandlers(state, options.khalaTurn)
      : {
          KhalaDraftChanged: synced.KhalaDraftChanged,
          KhalaTurnSubmitted: synced.KhalaTurnSubmitted,
        }),
  }
}

export interface HomeProgramHandle {
  readonly initialState: HomeState
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
  readonly sync: {
    readonly setPhase: (phase: MobileSyncPhase) => void
  }
  readonly session: {
    readonly signIn: () => void
    readonly signOut: () => void
  }
}

export const buildHomeProgram = (options: HomeProgramOptions = {}): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const programInitialState = initialHomeStateForConversation(options.conversation)
      const state = yield* SubscriptionRef.make<HomeState>(programInitialState)
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
        initialState: programInitialState,
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
        sync: {
          setPhase: phase => {
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.conversationAuthority === "sync" && phase !== "live" && phase !== "catching_up"
                ? {
                    ...current,
                    syncPhase: phase,
                    conversationThreads: [],
                    activeThreadRef: null,
                    khala: initialKhalaState,
                  }
                : { ...current, syncPhase: phase }))
          },
        },
        session: {
          signIn: fire("OpenAgentsSignInPressed"),
          signOut: fire("OpenAgentsSignOutPressed"),
        },
      }
    }),
  )
