import { describe, expect, test } from "bun:test"
// effect via the bridge — same effect copy as @effect-native/* (see the core
// module for why); Effect values must unify with the renderer's mount Effect.
import { Effect, Stream } from "@effect-native/core/effect"
import {
  makeReactNativeRenderer,
  type ReactElementLike,
  type ReactNativeDependencies,
  type ReactNodeLike,
} from "@effect-native/render-rn"
import { khalaTheme } from "@effect-native/tokens"

import {
  buildHomeProgram,
  initialHomeState,
  renderContentView,
} from "../src/screens/home-core"
import {
  drainSseBuffer,
  initialSarahState,
  prospectRefFromThreadId,
  renderSarahSurface,
  SARAH_TURN_FAILED_TEXT,
  SARAH_UNAVAILABLE_TITLE,
  type SarahTurnClient,
} from "../src/screens/sarah-core"

/**
 * OpenAgents mobile GL-3 (#8649) — Sarah conversation surface contract.
 *
 * Behavior contract openagents_mobile.sarah_text_surface.v1: selecting the
 * Sarah surface yields a REAL conversation over the production /sarah
 * contracts — typed turn round-trips into transcript state, typed SSE
 * events into transcript/card state with bounded dedupe, honest typed
 * degradation when the service is unreachable (never a dead composer), and
 * a persisted prospect relationship that survives restarts. The bundled
 * demo video is AMBIENT BACKGROUND ONLY and is never conversation evidence.
 *
 * Everything here drives the REAL view program (intent registry +
 * SubscriptionRef state) with a deterministic fake turn client; the
 * renderer round-trip uses the REAL @effect-native/render-rn lowering with
 * a string host shim — the exact seam the device uses.
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

const settle = Effect.gen(function* () {
  yield* nextTask
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") {
      throw new Error("expected a current state value")
    }
    return option.value
  })

/** Deterministic fake of the production turn client. */
const fakeTurnClient = (behavior?: {
  readonly fail?: boolean
  readonly reply?: string
  readonly threadId?: string
}): SarahTurnClient & { readonly calls: Array<{ message: string; prospectRef: string | null }> } => {
  const calls: Array<{ message: string; prospectRef: string | null }> = []
  return {
    calls,
    sendTurn: (input) => {
      calls.push({ message: input.message, prospectRef: input.prospectRef })
      if (behavior?.fail === true) {
        return Promise.reject(new Error("network down"))
      }
      return Promise.resolve({
        ok: true,
        reply: behavior?.reply ?? `Sarah heard: ${input.message}`,
        modelPath: "khala_gateway_live",
        threadId:
          behavior?.threadId ??
          (input.prospectRef === null
            ? "prospect:minted-by-turn"
            : `prospect:${input.prospectRef}`),
      })
    },
  }
}

const sarahOf = (state: Awaited<ReturnType<typeof effOf>>) => state.sarah
const effOf = async (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.runPromise(lastState(program))

describe("contract openagents_mobile.sarah_text_surface.v1", () => {
  test("typed turn round-trip: submit appends user + thinking, reply resolves to done transcript state", async () => {
    const client = fakeTurnClient({ reply: "Hello from Sarah." })
    const program = buildHomeProgram({ sarahTurn: client })

    program.sarah.sessionReady({
      prospectRef: "ref-1",
      threadId: "prospect:ref-1",
      restored: false,
      entries: [],
    })
    await Effect.runPromise(settle)
    program.sarah.submitTurn("hello sarah")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)

    const sarah = sarahOf(await effOf(program))
    expect(client.calls).toEqual([{ message: "hello sarah", prospectRef: "ref-1" }])
    expect(sarah.entries.map((entry) => [entry.role, entry.status, entry.text])).toEqual([
      ["user", "done", "hello sarah"],
      ["assistant", "done", "Hello from Sarah."],
    ])
    expect(sarah.turnPending).toBe(false)
    expect(sarah.draft).toBe("")
    expect(sarah.phase).toBe("ready")
  })

  test("turn failure degrades to the typed failed entry — composer alive, never dead", async () => {
    const client = fakeTurnClient({ fail: true })
    const program = buildHomeProgram({ sarahTurn: client })
    program.sarah.sessionReady({
      prospectRef: "ref-1",
      threadId: "prospect:ref-1",
      restored: false,
      entries: [],
    })
    await Effect.runPromise(settle)
    program.sarah.submitTurn("are you there?")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)

    const sarah = sarahOf(await effOf(program))
    expect(sarah.entries.at(-1)?.status).toBe("failed")
    expect(sarah.entries.at(-1)?.text).toBe(SARAH_TURN_FAILED_TEXT)
    expect(sarah.turnPending).toBe(false)
    // The surface still renders the composer (typed unavailable is a card,
    // not a dead input).
    const view = JSON.stringify(renderSarahSurface(sarah))
    expect(view).toContain('"_tag":"Composer"')
    expect(view).toContain("SarahTurnSubmitted")
  })

  test("a turn can bootstrap the session: prospect relationship adopted from the reply threadId", async () => {
    const client = fakeTurnClient({ threadId: "prospect:minted-by-turn" })
    const program = buildHomeProgram({ sarahTurn: client })
    // No session (mint failed / offline start).
    program.sarah.sessionUnavailable("session_mint_failed")
    await Effect.runPromise(settle)
    program.sarah.submitTurn("bootstrap me")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)

    const sarah = sarahOf(await effOf(program))
    expect(sarah.prospectRef).toBe("minted-by-turn")
    expect(sarah.phase).toBe("ready")
    expect(prospectRefFromThreadId("prospect:abc")).toBe("abc")
    expect(prospectRefFromThreadId("thread:abc")).toBeNull()
  })

  test("typed SSE events land in state: transcript appends (with dedupe), cards render, stream phases track", async () => {
    const program = buildHomeProgram({ sarahTurn: fakeTurnClient() })
    program.sarah.sessionReady({
      prospectRef: "ref-1",
      threadId: "prospect:ref-1",
      restored: false,
      entries: [],
    })
    await Effect.runPromise(settle)

    program.sarah.streamStatus("connecting")
    await Effect.runPromise(settle)
    program.sarah.streamStatus("live")
    await Effect.runPromise(settle)
    program.sarah.eventReceived({ type: "transcript", role: "assistant", text: "Card path works." })
    await Effect.runPromise(settle)
    // Duplicate of what the POST path already appended -> deduped.
    program.sarah.eventReceived({ type: "transcript", role: "assistant", text: "Card path works." })
    await Effect.runPromise(settle)
    program.sarah.eventReceived({ type: "card", title: "Instant answer", body: "pricing.public" })
    await Effect.runPromise(settle)
    // Reconnect lifecycle is typed state, not guesswork.
    program.sarah.streamStatus("reconnecting")
    await Effect.runPromise(settle)

    const sarah = sarahOf(await effOf(program))
    expect(sarah.entries.filter((entry) => entry.text === "Card path works.")).toHaveLength(1)
    expect(sarah.cards).toHaveLength(1)
    expect(sarah.cards[0]?.title).toBe("Instant answer")
    expect(sarah.stream).toBe("reconnecting")
    const view = JSON.stringify(renderSarahSurface(sarah))
    expect(view).toContain("Instant answer")
    expect(view).toContain("reconnecting")
  })

  test("unavailable session renders the typed card and restored sessions mark continuity", () => {
    const unavailable = JSON.stringify(
      renderSarahSurface({ ...initialSarahState, phase: "unavailable" }),
    )
    expect(unavailable).toContain(SARAH_UNAVAILABLE_TITLE)
    expect(unavailable).toContain('"_tag":"Composer"')

    const restored = JSON.stringify(
      renderSarahSurface({
        ...initialSarahState,
        phase: "ready",
        prospectRef: "ref-1",
        restored: true,
        entries: [
          { key: "turn-1-user", role: "user", text: "remember me", status: "done" },
        ],
      }),
    )
    expect(restored).toContain("continued")
    expect(restored).toContain("remember me")
  })

  test("content surface: sarah mode mounts the conversation over the ambient video; openagents mode stays the clean demo surface", () => {
    const openagents = JSON.stringify(renderContentView(initialHomeState))
    expect(openagents).not.toContain("Transcript")
    const sarah = JSON.stringify(
      renderContentView({ ...initialHomeState, surfaceMode: "sarah" }),
    )
    expect(sarah).toContain('"_tag":"Transcript"')
    expect(sarah).toContain('"_tag":"Composer"')
    // Transparent root — the muted demo loop is AMBIENT BACKGROUND ONLY.
    expect(sarah).not.toContain('"backgroundColor":"background"')
  })

  test("composer tap in Sarah mode never starts the demo reply video (presentation stays out of the conversation)", async () => {
    const program = buildHomeProgram({ sarahTurn: fakeTurnClient() })
    program.chrome.selectSurfaceMode("sarah")
    await Effect.runPromise(settle)
    program.chrome.pressComposer()
    await Effect.runPromise(settle)
    let state = await effOf(program)
    expect(state.askVideoPlaying).toBe(false)
    expect(state.composerTaps).toBe(1)

    program.chrome.selectSurfaceMode("openagents")
    await Effect.runPromise(settle)
    program.chrome.pressComposer()
    await Effect.runPromise(settle)
    state = await effOf(program)
    expect(state.askVideoPlaying).toBe(true)
  })

  test("REAL render-rn round-trip: typing into the lowered TextInput and submitting drives the transcript", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const client = fakeTurnClient({ reply: "Streamed reply." })
          const program = buildHomeProgram({ sarahTurn: client })
          const renderer = makeReactNativeRenderer({
            dependencies,
            theme: khalaTheme,
            platform: "ios",
          })
          const surface = yield* renderer.mount(
            { render: () => undefined },
            program.contentViewStream,
            program.report,
          )
          program.chrome.selectSurfaceMode("sarah")
          yield* settle
          program.sarah.sessionReady({
            prospectRef: "ref-1",
            threadId: "prospect:ref-1",
            restored: false,
            entries: [],
          })
          yield* settle

          // The EXACT device seam: the lowered RN TextInput's handlers.
          const tree = yield* surface.currentElement
          const input = findByTestId(tree, "en-composer-input")
          if (input === undefined) throw new Error("expected the EN composer input")
          const onChangeText = input.props.onChangeText as (value: string) => void
          onChangeText("hello from the shell")
          yield* settle
          const afterDraft = yield* lastState(program)
          expect(afterDraft.sarah.draft).toBe("hello from the shell")

          const treeWithDraft = yield* surface.currentElement
          const boundInput = findByTestId(treeWithDraft, "en-composer-input")
          const onSubmit = boundInput?.props.onSubmitEditing as (event: {
            nativeEvent: { text: string }
          }) => void
          onSubmit({ nativeEvent: { text: "hello from the shell" } })
          yield* settle
          yield* settle
          const after = yield* lastState(program)
          expect(after.sarah.entries.map((entry) => entry.text)).toEqual([
            "hello from the shell",
            "Streamed reply.",
          ])
          expect(client.calls).toHaveLength(1)
          yield* surface.unmount
        }),
      ),
    )
  })

  test("restored sessions resume the turn counter: a new reply never overwrites a restored bubble (key collision regression, found on-device)", async () => {
    const client = fakeTurnClient({ reply: "Fresh reply." })
    const program = buildHomeProgram({ sarahTurn: client })
    program.sarah.sessionReady({
      prospectRef: "ref-1",
      threadId: "prospect:ref-1",
      restored: true,
      entries: [
        { key: "turn-1-user", role: "user", text: "hello", status: "done" } as never,
        { key: "turn-1-reply", role: "assistant", text: "original reply", status: "done" } as never,
      ].map(({ key, role, text }) => ({ key, role, text })),
    })
    await Effect.runPromise(settle)
    program.sarah.submitTurn("second turn")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const sarah = sarahOf(await effOf(program))
    // The restored bubble is untouched; the new turn got turn-2-* keys.
    expect(sarah.entries.map((entry) => [entry.key, entry.text])).toEqual([
      ["turn-1-user", "hello"],
      ["turn-1-reply", "original reply"],
      ["turn-2-user", "second turn"],
      ["turn-2-reply", "Fresh reply."],
    ])
  })

  test("Send button carries the live draft through the same SarahTurnSubmitted intent (multiline inputs never submit on iOS return)", () => {
    const view = JSON.stringify(
      renderSarahSurface({ ...initialSarahState, phase: "ready", draft: "hello sarah" }),
    )
    expect(view).toContain('"key":"sarah-send"')
    expect(view).toContain(
      '"name":"SarahTurnSubmitted","payload":{"_tag":"StaticPayload","value":"hello sarah"}',
    )
  })

  test("SSE frame parser: data frames drain, comments/heartbeats are liveness-only, partial frames buffer", () => {
    const seen: Array<string> = []
    let rest = drainSseBuffer(
      `: connected ref-1\n\ndata: {"type":"card"}\n\ndata: {"ty`,
      (data) => seen.push(data),
    )
    expect(seen).toEqual([`{"type":"card"}`])
    expect(rest).toBe(`data: {"ty`)
    rest = drainSseBuffer(`${rest}pe":"transcript"}\n\n: hb\n\n`, (data) => seen.push(data))
    expect(seen).toEqual([`{"type":"card"}`, `{"type":"transcript"}`])
    expect(rest).toBe("")
  })
})

const isElement = (node: ReactNodeLike): node is ReactElementLike =>
  typeof node === "object" && node !== null && "props" in node

const childrenOf = (node: ReactElementLike): ReadonlyArray<ReactNodeLike> => {
  const value = node.props.children
  if (value === undefined || value === null) return []
  return Array.isArray(value) ? (value as ReadonlyArray<ReactNodeLike>) : [value as ReactNodeLike]
}

const findByTestId = (
  node: ReactNodeLike,
  testID: string,
): ReactElementLike | undefined => {
  if (!isElement(node)) return undefined
  if (node.props.testID === testID) return node
  for (const child of childrenOf(node)) {
    const found = findByTestId(child, testID)
    if (found !== undefined) return found
  }
  return undefined
}
