/**
 * EN view-program unit tests (#8574): pure state -> expected component tree,
 * plus the full typed intent loop run headlessly through the real registry —
 * dispatch -> handler -> SubscriptionRef -> re-rendered view.
 */
import { describe, expect, test } from "bun:test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  desktopShellIntents,
  desktopShellView,
  formatShellTimestamp,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  noteMessage,
  withInput,
  withFleetDeploymentRequested,
  withFleetDeploymentResult,
  withFleetDesk,
  withFleetObjective,
  withNewChat,
  withChatSelected,
  withLoopProof,
  withNote,
  withPending,
  type DesktopShellState,
} from "./shell.ts"
import { openagentsDesktopTheme } from "./theme.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

/** Collect every catalog node in a view tree (children live under varying props). */
const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const baseState: DesktopShellState = initialDesktopShellState("electron/darwin", "18:04")
const fixedNow = () => "18:05"

describe("desktopShellView (state -> component tree)", () => {
  test("renders neutral chat workspace: sidebar, title, transcript, composer", () => {
    const view = desktopShellView(baseState)

    const title = nodeByKey(view, "shell-title")
    expect(title?._tag).toBe("Text")
    expect(title?.content).toBe("New conversation")

    const surface = nodeByKey(view, "shell-surface")
    expect(surface?._tag).toBe("Badge")
    expect(surface?.label).toBe("Chat")

    const status = nodeByKey(view, "shell-status")
    expect(status?.label).toBe("Local workspace")
    expect(status?.tone).toBe("success")

    const host = nodeByKey(view, "shell-host")
    expect(host?.label).toBe("electron/darwin")

    expect(nodeByKey(view, "shell-welcome-title")?.content).toBe(
      "What would you like to move today?",
    )

    const transcript = nodeByKey(view, "shell-transcript")
    expect(transcript?._tag).toBe("Transcript")
    expect(transcript?.pinToEnd).toBe(true)
    expect((transcript?.messages as Array<unknown>).length).toBe(2)

    expect(nodeByKey(view, "shell-input")?._tag).toBe("TextField")
    expect(nodeByKey(view, "shell-note")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-ping")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-fleet-toggle")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-sidebar")?._tag).toBe("Stack")
    expect(nodeByKey(view, "sidebar-new-chat")?._tag).toBe("Button")
    expect(nodeByKey(view, "sidebar-current-chat")?._tag).toBe("Button")
    expect(nodeByKey(view, "sidebar-fleet")?._tag).toBe("Button")
    expect(nodeByKey(view, "fleet-desk")).toBeUndefined()
  })

  test("loop-proof badge reflects state and flips tone once proven", () => {
    const zero = nodeByKey(desktopShellView(baseState), "shell-ping-count")
    expect(zero?.label).toBe("proofs 0")
    expect(zero?.tone).toBe("neutral")

    const proven = nodeByKey(
      desktopShellView({ ...baseState, loopProofs: 3 }),
      "shell-ping-count",
    )
    expect(proven?.label).toBe("proofs 3")
    expect(proven?.tone).toBe("success")
  })

  test("buttons carry the typed intent refs (no ad hoc handlers)", () => {
    const view = desktopShellView(baseState)
    const ping = nodeByKey(view, "shell-ping") as { onPress?: { name?: string } }
    expect(ping.onPress?.name).toBe("DesktopLoopPinged")
    const note = nodeByKey(view, "shell-note") as { onPress?: { name?: string } }
    expect(note.onPress?.name).toBe("DesktopNoteSubmitted")
    const input = nodeByKey(view, "shell-input") as {
      onChange?: { name?: string }
      onSubmit?: { name?: string }
    }
    expect(input.onChange?.name).toBe("DesktopInputChanged")
    expect(input.onSubmit?.name).toBe("DesktopNoteSubmitted")
  })

  test("messages ride the v29 chat chrome contract: typed senderLabel/timestamp, body is only the text", () => {
    const system = noteMessage({ key: "boot-0", role: "system", text: "hello", timestamp: "18:04" })
    expect(system.key).toBe("boot-0")
    expect(system.role).toBe("system")
    expect(system.senderLabel).toBe("SYSTEM")
    expect(system.timestamp).toBe("18:04")
    expect(system.body.length).toBe(1)
    const systemText = system.body[0] as unknown as AnyNode
    expect(systemText._tag).toBe("Text")
    expect(systemText.color).toBe("textMuted")
    // sender identity is typed message data, never concatenated into the body
    expect(systemText.content).toBe("hello")

    const user = noteMessage({ key: "note-2", role: "user", text: "rofl", timestamp: "18:05" })
    expect(user.senderLabel).toBe("YOU")
    expect((user.body[0] as unknown as AnyNode).color).toBe("textPrimary")
    expect((user.body[0] as unknown as AnyNode).content).toBe("rofl")

    const assistant = noteMessage({ key: "assistant-1", role: "assistant", text: "I’m here", timestamp: "18:05" })
    expect(assistant.senderLabel).toBe("ASSISTANT")
    expect((assistant.body[0] as unknown as AnyNode).content).toBe("I’m here")
  })

  test("composer rides the v29 submit lifecycle contract: clearOnSubmit + pending disables", () => {
    const idle = nodeByKey(desktopShellView(baseState), "shell-input")
    expect(idle?.clearOnSubmit).toBe(true)
    expect(idle?.disabled).toBe(false)

    const pendingView = desktopShellView(withPending(baseState, true))
    expect(nodeByKey(pendingView, "shell-input")?.disabled).toBe(true)
    expect(nodeByKey(pendingView, "shell-note")?.disabled).toBe(true)
  })
})

describe("pure transitions", () => {
  test("withNote trims, clears the composer value binding, and appends a user note", () => {
    const next = withNote(withInput(baseState, "  hello desktop  "), "  hello desktop  ", "18:05")
    expect(next.input).toBe("")
    expect(next.pending).toBe(false)
    expect(next.notes.length).toBe(4)
    expect(next.notes[2]).toEqual({
      key: "note-2",
      role: "user",
      text: "hello desktop",
      timestamp: "18:05",
    })
    expect(next.notes[3]?.role).toBe("assistant")
  })

  test("withNote ignores empty input", () => {
    expect(withNote(baseState, "   ", "18:05")).toBe(baseState)
  })

  test("withPending toggles the composer-disabling flag without touching notes", () => {
    const pending = withPending(baseState, true)
    expect(pending.pending).toBe(true)
    expect(pending.notes).toBe(baseState.notes)
    expect(withPending(pending, false).pending).toBe(false)
  })

  test("New chat resets the conversation and current-chat navigation closes Fleet", () => {
    const activeFleet = withFleetDesk(withNote(baseState, "Ship the app", "18:05"))
    expect(activeFleet.fleetDeskOpen).toBe(true)
    expect(withChatSelected(activeFleet).fleetDeskOpen).toBe(false)

    const fresh = withNewChat(activeFleet, "18:06")
    expect(fresh.fleetDeskOpen).toBe(false)
    expect(fresh.fleetObjective).toBe("")
    expect(fresh.fleetDeployment).toBe("not_requested")
    expect(fresh.notes).toHaveLength(2)
    expect(fresh.notes[0]?.role).toBe("assistant")
  })

  test("withLoopProof increments and appends a system note", () => {
    const next = withLoopProof(baseState, "18:05")
    expect(next.loopProofs).toBe(1)
    expect(next.notes.length).toBe(3)
    expect(next.notes[2]?.role).toBe("system")
    expect(next.notes[2]?.timestamp).toBe("18:05")
  })

  test("Fleet desk stages an objective without manufacturing a FleetRun", () => {
    const open = withFleetDesk(baseState)
    expect(open.fleetDeskOpen).toBe(true)
    expect(nodeByKey(desktopShellView(open), "fleet-desk")?._tag).toBe("Card")
    expect(nodeByKey(desktopShellView(open), "shell-welcome")).toBeUndefined()

    const drafted = withFleetObjective(open, "Ship the desktop fleet chat")
    const dispatching = withFleetDeploymentRequested(drafted)
    expect(dispatching.fleetDeployment).toBe("dispatching")
    const staged = withFleetDeploymentResult(dispatching, {
      state: "accepted",
      message: "Local Pylon accepted the fleet brief.",
      intentStatus: "received",
    }, "18:05")
    expect(staged.fleetDeployment).toBe("accepted")
    expect(staged.notes.at(-1)?.text).toContain("accepted")
    expect(staged.notes.at(-1)?.text).not.toContain("runRef")
  })

  test("formatShellTimestamp is a zero-padded display string", () => {
    expect(formatShellTimestamp(new Date(2026, 6, 10, 9, 5))).toBe("09:05")
    expect(formatShellTimestamp(new Date(2026, 6, 10, 18, 45))).toBe("18:45")
  })
})

describe("typed intent loop end-to-end (registry -> state -> re-render)", () => {
  test("DesktopLoopPinged round trip re-renders the badge and transcript", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        // Dispatch through the SAME IntentRef the rendered button carries.
        const view = desktopShellView(yield* SubscriptionRef.get(state))
        const ping = nodeByKey(view, "shell-ping") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(ping.onPress, null))

        const next = yield* SubscriptionRef.get(state)
        expect(next.loopProofs).toBe(1)
        const rerendered = desktopShellView(next)
        expect(nodeByKey(rerendered, "shell-ping-count")?.label).toBe("proofs 1")
        const transcript = nodeByKey(rerendered, "shell-transcript")
        expect((transcript?.messages as Array<unknown>).length).toBe(3)
      }),
    )
  })

  test("composer intents: input change then submit falls back to composer state on button press", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(baseState)
        const input = nodeByKey(view, "shell-input") as {
          onChange: Parameters<typeof resolveIntentRef>[0]
        }
        const note = nodeByKey(view, "shell-note") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        // TextField passes its component value; Button press passes null.
        yield* registry.dispatch(resolveIntentRef(input.onChange, "ship the shell"))
        yield* registry.dispatch(resolveIntentRef(note.onPress, null))

        const next = yield* SubscriptionRef.get(state)
        expect(next.input).toBe("")
        expect(next.notes[2]).toEqual({
          key: "note-2",
          role: "user",
          text: "ship the shell",
          timestamp: "18:05",
        })
        expect(next.notes[3]?.role).toBe("assistant")
      }),
    )
  })

  test("submit resets the composer value binding and the composer stays usable (clear-on-submit contract)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(baseState)
        const input = nodeByKey(view, "shell-input") as {
          onChange: Parameters<typeof resolveIntentRef>[0]
          onSubmit: Parameters<typeof resolveIntentRef>[0]
        }

        // Type then submit through the field's own intent (Enter path): the
        // submit intent must reset the composer value binding to "".
        yield* registry.dispatch(resolveIntentRef(input.onChange, "first message"))
        expect((yield* SubscriptionRef.get(state)).input).toBe("first message")
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "first message"))

        const afterFirst = yield* SubscriptionRef.get(state)
        expect(afterFirst.input).toBe("")
        // the re-rendered TextField carries the emptied value + clearOnSubmit,
        // so the DOM renderer empties the focused input too (effect-native#72)
        const rerendered = nodeByKey(desktopShellView(afterFirst), "shell-input")
        expect(rerendered?.value).toBe("")
        expect(rerendered?.clearOnSubmit).toBe(true)
        expect(rerendered?.disabled).toBe(false)

        // the composer stays usable: a second round trip works end to end
        yield* registry.dispatch(resolveIntentRef(input.onChange, "second message"))
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "second message"))
        const afterSecond = yield* SubscriptionRef.get(state)
        expect(afterSecond.input).toBe("")
        expect(afterSecond.notes.at(-2)?.text).toBe("second message")
        expect(afterSecond.notes.at(-1)?.role).toBe("assistant")
        expect(afterSecond.notes.length).toBe(6)
      }),
    )
  })

  test("submit while pending is refused (disabled-while-pending contract)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(withPending(withInput(baseState, "held"), true))
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(yield* SubscriptionRef.get(state))
        const input = nodeByKey(view, "shell-input") as {
          onSubmit: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "held"))
        const next = yield* SubscriptionRef.get(state)
        expect(next.notes.length).toBe(2)
        expect(next.input).toBe("held")
      }),
    )
  })
})

describe("theme parity (one OpenAgents blue theme, many hosts)", () => {
  test("desktop theme keeps the shared token values", () => {
    expect(openagentsDesktopTheme.color.background).toBe("#03060b")
    expect(openagentsDesktopTheme.color.accent).toBe("#3a7bff")
    expect(openagentsDesktopTheme.color.border).toBe("#17315f")
    expect(openagentsDesktopTheme.color.focus).toBe("#4fd0ff")
    expect(openagentsDesktopTheme.color.textPrimary).toBe("#f1efe8")
  })
})
