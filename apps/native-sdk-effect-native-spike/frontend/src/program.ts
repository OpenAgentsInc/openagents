import { makeIntentRegistry, makeViewProgramFromState, Stack, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"
import {
  desktopShellIntents,
  desktopShellMainView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  withThreads,
  type ChatHost,
  type DesktopShellState,
  type DesktopThread,
} from "@openagentsinc/openagents-desktop/renderer-portable"

export const fixtureSessions = [
  { ref: "session.parity", title: "Native parity pass" },
  { ref: "session.renderer", title: "Renderer boundary" },
  { ref: "session.audit", title: "SDK adoption audit" },
] as const

export type FixtureSessionRef = (typeof fixtureSessions)[number]["ref"]
export type NativeWorkspace = "chat" | "home" | "settings"

export type NativeStartupState = Readonly<{
  revision: number
  acknowledgedNativeSequence: number
  workspace: NativeWorkspace
  selectedSessionRef: FixtureSessionRef | null
}>

const initialNotes = [
  {
    key: "fixture-user",
    role: "user" as const,
    text: "Bring the Native SDK host one step closer to the real desktop app.",
    timestamp: "10:42",
  },
  {
    key: "fixture-assistant",
    role: "assistant" as const,
    text: "This center pane is now the production Desktop Effect Native view and intent program.",
    timestamp: "10:42",
  },
]

export const fixtureThreads: ReadonlyArray<DesktopThread> = [
  {
    id: fixtureSessions[0].ref,
    title: fixtureSessions[0].title,
    updatedAt: "2026-07-14T15:03:00.000Z",
    notes: initialNotes,
  },
  {
    id: fixtureSessions[1].ref,
    title: fixtureSessions[1].title,
    updatedAt: "2026-07-14T15:02:00.000Z",
    notes: [],
  },
  {
    id: fixtureSessions[2].ref,
    title: fixtureSessions[2].title,
    updatedAt: "2026-07-14T15:01:00.000Z",
    notes: [],
  },
]

const fixtureSessionRefs = new Set(fixtureSessions.map((session) => session.ref))

export const isFixtureSessionRef = (value: string | null): value is FixtureSessionRef =>
  value !== null && fixtureSessionRefs.has(value as FixtureSessionRef)

export const initialSpikeState = (
  startup: NativeStartupState = { revision: 1, acknowledgedNativeSequence: 0, workspace: "chat", selectedSessionRef: fixtureSessions[0].ref },
): DesktopShellState => {
  const base = initialDesktopShellState("native-sdk/darwin", "10:42")
  const selected = startup.selectedSessionRef === null
    ? base
    : { ...base, activeThreadId: startup.selectedSessionRef }
  const withFixtureThreads = withThreads(selected, fixtureThreads)
  return {
    ...withFixtureThreads,
    workspace: startup.workspace,
    harnessLanes: {
      fable: { available: false, reason: "Fable is unavailable in the Native SDK parity fixture" },
      codex: { available: true, reason: null },
    },
    ...(startup.selectedSessionRef === null
      ? { activeThreadId: null, notes: [] }
      : {}),
  }
}

const makeFixtureChatHost = (): ChatHost => {
  const threads = new Map(fixtureThreads.map((thread) => [thread.id, thread] as const))
  let nextThread = 0
  let nextMessage = 0
  return {
    listThreads: async () => [...threads.values()],
    newThread: async () => {
      const id = `native.fixture.new.${++nextThread}`
      const thread: DesktopThread = {
        id,
        title: "New chat",
        updatedAt: `2026-07-14T16:${String(nextThread).padStart(2, "0")}:00.000Z`,
        notes: [],
      }
      threads.set(id, thread)
      return thread
    },
    openThread: async (id) => threads.get(id) ?? null,
    sendMessage: async ({ id, message }) => {
      const current = threads.get(id)
      if (current === undefined) return { ok: false, error: "The Native fixture thread is unavailable." }
      const sequence = ++nextMessage
      const thread: DesktopThread = {
        ...current,
        updatedAt: `2026-07-14T17:${String(sequence).padStart(2, "0")}:00.000Z`,
        notes: [
          ...current.notes,
          { key: `native-user-${sequence}`, role: "user", text: message, timestamp: "now" },
          {
            key: `native-assistant-${sequence}`,
            role: "assistant",
            text: "Fixture response from the bounded Native host adapter; no provider was called.",
            timestamp: "now",
          },
        ],
      }
      threads.set(id, thread)
      return { ok: true, thread }
    },
    interruptActive: async () => false,
  }
}

/** Host-only sizing wrapper; product content remains the exact Desktop pane. */
export const spikeView = (state: DesktopShellState): View => Stack({
  key: "native-desktop-center",
  direction: "column",
  style: { width: "full", height: "full", minHeight: 0 },
}, [desktopShellMainView(state)])
export type SpikeState = DesktopShellState

export const makeSpikeRuntime = (restoredState: DesktopShellState = initialSpikeState()) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(restoredState)
    const registry = yield* makeIntentRegistry(
      desktopShellIntents,
      makeDesktopShellHandlers(
        state,
        () => "10:42",
        undefined,
        makeFixtureChatHost(),
      ),
    )
    return {
      state,
      registry,
      program: makeViewProgramFromState(state, spikeView),
    }
  })
