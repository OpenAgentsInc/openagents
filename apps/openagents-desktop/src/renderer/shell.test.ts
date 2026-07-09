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
  initialDesktopShellState,
  makeDesktopShellHandlers,
  noteMessage,
  withInput,
  withLoopProof,
  withNote,
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

const baseState: DesktopShellState = initialDesktopShellState("electron/darwin")

describe("desktopShellView (state -> component tree)", () => {
  test("renders the honest shell: title, status badges, transcript, composer", () => {
    const view = desktopShellView(baseState)

    const title = nodeByKey(view, "shell-title")
    expect(title?._tag).toBe("Text")
    expect(title?.content).toBe("OpenAgents")

    const surface = nodeByKey(view, "shell-surface")
    expect(surface?._tag).toBe("Badge")
    expect(surface?.label).toBe("DESKTOP")

    const status = nodeByKey(view, "shell-status")
    expect(status?.label).toBe("READY")
    expect(status?.tone).toBe("success")

    const host = nodeByKey(view, "shell-host")
    expect(host?.label).toBe("host: electron/darwin")

    const transcript = nodeByKey(view, "shell-transcript")
    expect(transcript?._tag).toBe("Transcript")
    expect(transcript?.pinToEnd).toBe(true)
    expect((transcript?.messages as Array<unknown>).length).toBe(2)

    expect(nodeByKey(view, "shell-input")?._tag).toBe("TextField")
    expect(nodeByKey(view, "shell-note")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-ping")?._tag).toBe("Button")
  })

  test("loop-proof badge reflects state and flips tone once proven", () => {
    const zero = nodeByKey(desktopShellView(baseState), "shell-ping-count")
    expect(zero?.label).toBe("loop proofs: 0")
    expect(zero?.tone).toBe("neutral")

    const proven = nodeByKey(
      desktopShellView({ ...baseState, loopProofs: 3 }),
      "shell-ping-count",
    )
    expect(proven?.label).toBe("loop proofs: 3")
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

  test("transcript message shape matches the Sarah surface composition (shared catalog, keyed role-tagged Card body)", () => {
    const message = noteMessage({ key: "boot-0", role: "system", text: "hello" })
    expect(message.key).toBe("boot-0")
    expect(message.role).toBe("system")
    expect(message.body.length).toBe(1)
    const card = message.body[0] as unknown as AnyNode
    expect(card._tag).toBe("Card")
    expect(card.key).toBe("boot-0-card")
    const texts = collectNodes(card).filter((node) => node._tag === "Text")
    expect(texts.map((node) => node.key)).toEqual(["boot-0-role", "boot-0-text"])
  })
})

describe("pure transitions", () => {
  test("withNote trims, clears the composer, and appends a user note", () => {
    const next = withNote(withInput(baseState, "  hello desktop  "), "  hello desktop  ")
    expect(next.input).toBe("")
    expect(next.notes.length).toBe(3)
    expect(next.notes[2]).toEqual({ key: "note-2", role: "user", text: "hello desktop" })
  })

  test("withNote ignores empty input", () => {
    expect(withNote(baseState, "   ")).toBe(baseState)
  })

  test("withLoopProof increments and appends a system note", () => {
    const next = withLoopProof(baseState)
    expect(next.loopProofs).toBe(1)
    expect(next.notes.length).toBe(3)
    expect(next.notes[2]?.role).toBe("system")
  })
})

describe("typed intent loop end-to-end (registry -> state -> re-render)", () => {
  test("DesktopLoopPinged round trip re-renders the badge and transcript", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state),
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
        expect(nodeByKey(rerendered, "shell-ping-count")?.label).toBe("loop proofs: 1")
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
          makeDesktopShellHandlers(state),
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
        expect(next.notes[2]).toEqual({ key: "note-2", role: "user", text: "ship the shell" })
      }),
    )
  })
})

describe("theme parity (one Protoss-blue theme, many hosts)", () => {
  test("desktop theme matches the Sarah surface token values", () => {
    expect(openagentsDesktopTheme.color.background).toBe("#03060b")
    expect(openagentsDesktopTheme.color.accent).toBe("#3a7bff")
    expect(openagentsDesktopTheme.color.border).toBe("#17315f")
    expect(openagentsDesktopTheme.color.focus).toBe("#4fd0ff")
    expect(openagentsDesktopTheme.color.textPrimary).toBe("#f1efe8")
  })
})
