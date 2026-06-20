// ZERO-BASE SHELL (owner directive, 2026-06-19): the desktop launches to a
// dead-simple shell — a black screen with NOTHING on it except a single text
// bar at the bottom (and the clean conversation above it once there is a
// response). Everything else (the full multi-pane UI) is KEPT and still mounts,
// but is hidden behind an explicit open (Cmd-K palette / "open panes").
//
// These tests pin: (1) the minimal default shell on launch (shell pane, no
// warm-up commands, no panes rendered), (2) the text-bar → response loop (a
// pure reducer + one loopback command), (3) programmatic-control parity (the
// transcript projection matches what the view shows), and (4) the open/close
// panes toggle (the hidden UI is reachable and the panes still mount).

import { describe, expect, test } from "bun:test"
import { initialRuntimeState } from "../src/ui/initial-state"
import {
  initialModel,
  Model,
  PaneId,
  shellTranscriptText,
} from "../src/ui/model"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import {
  ChangedShellInput,
  ClosedPanes,
  OpenedPanes,
  RespondedShell,
  SubmittedShell,
} from "../src/ui/message"
import { shellLoopbackReply } from "../src/ui/commands"
import { interpretKey } from "../src/ui/keyboard"

// Serialize a rendered view tree to a string so we can assert what is / is NOT
// present without a DOM. The tree is plain objects (foldkit Html) — JSON.stringify
// with a cycle-safe replacer is enough to scan for class names / text.
const serialize = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_k, v) => {
    if (typeof v === "function") return "[fn]"
    if (typeof v === "object" && v !== null) {
      if (seen.has(v)) return "[cycle]"
      seen.add(v)
    }
    return v
  })
}

describe("zero-base shell: the minimal default surface", () => {
  test("launch lands on the `shell` pane with NO warm-up commands", () => {
    const [model, commands] = initialRuntimeState()
    expect(model.pane).toBe("shell")
    // Quiet + black: nothing is loaded at entry (panes warm lazily on open).
    expect(commands).toHaveLength(0)
    // The shell starts empty (truly black: no conversation, empty input).
    expect(model.shellTurns).toHaveLength(0)
    expect(model.shellInput).toBe("")
    expect(model.shellPending).toBe(false)
  })

  test("the default shell view mounts a Document with the text bar and NO nav/sidebar/panes", () => {
    const [model] = initialRuntimeState()
    const doc = view(model) as { title: string; body: unknown }
    expect(doc.title).toBe("Autopilot")
    expect(doc.body).toBeDefined()
    const tree = serialize(doc.body)
    // The one surface: the bottom text bar exists.
    expect(tree).toContain("shell-input")
    expect(tree).toContain("shell-bar")
    // The hidden full UI does NOT render by default: no sidebar, no pane chrome,
    // no settings surface on the default screen.
    expect(tree).not.toContain("sidebar")
    expect(tree).not.toContain("settings-pane")
    expect(tree).not.toContain("composer-pane")
  })

  test("on a fresh launch (no response yet) the conversation area is absent — pure black", () => {
    const [model] = initialRuntimeState()
    const tree = serialize(view(model).body)
    expect(tree).not.toContain("shell-conversation")
  })
})

describe("zero-base shell: text bar → response loop", () => {
  test("typing tracks the input; an empty submit is a no-op (no error chrome)", () => {
    const [m0] = initialRuntimeState()
    const [m1, c1] = update(m0, ChangedShellInput({ value: "  " }))
    expect(m1.shellInput).toBe("  ")
    const [m2, c2] = update(m1, SubmittedShell())
    expect(m2.shellTurns).toHaveLength(0)
    expect(c2).toHaveLength(0)
    void c1
  })

  test("submitting records the user turn, sets pending, and dispatches the loopback command", () => {
    const [m0] = initialRuntimeState()
    const [m1] = update(m0, ChangedShellInput({ value: "hello there" }))
    const [m2, commands] = update(m1, SubmittedShell())
    expect(m2.shellInput).toBe("")
    expect(m2.shellPending).toBe(true)
    expect(m2.shellTurns).toHaveLength(1)
    expect(m2.shellTurns[0]).toMatchObject({ role: "you", text: "hello there" })
    // Exactly one command: the response seam (RespondToShellInput).
    expect(commands).toHaveLength(1)
  })

  test("a submit while pending is a no-op — so the input can stay enabled and KEEP FOCUS", () => {
    // The input is intentionally NOT disabled while pending (a disabled input is
    // blurred, dropping focus from the chat box after every send). That's only
    // safe because the reducer guards a second submit while pending.
    const [m0] = initialRuntimeState()
    const [m1] = update(m0, ChangedShellInput({ value: "first" }))
    const [m2] = update(m1, SubmittedShell())
    expect(m2.shellPending).toBe(true)
    const [m3] = update(m2, ChangedShellInput({ value: "second" }))
    const [m4, c4] = update(m3, SubmittedShell())
    // The pending submit did nothing: still just the first turn, no command.
    expect(m4.shellTurns).toHaveLength(1)
    expect(c4).toHaveLength(0)
  })

  test("the response lands as an Autopilot turn and clears pending", () => {
    const [m0] = initialRuntimeState()
    const [m1] = update(m0, ChangedShellInput({ value: "ping" }))
    const [m2] = update(m1, SubmittedShell())
    // Drive the loopback exactly as the command would (deterministic seam).
    const [m3] = update(
      m2,
      RespondedShell({ prompt: "ping", text: shellLoopbackReply("ping") }),
    )
    expect(m3.shellPending).toBe(false)
    expect(m3.shellTurns).toHaveLength(2)
    expect(m3.shellTurns[1]).toMatchObject({ role: "autopilot" })
    expect(m3.shellTurns[1].text).toBe(shellLoopbackReply("ping"))
    // The conversation now renders above the bar.
    const tree = serialize(view(m3).body)
    expect(tree).toContain("shell-conversation")
    expect(tree).toContain("ping")
  })
})

describe("zero-base shell: programmatic-control parity", () => {
  test("shellTranscriptText projects EXACTLY what the user sees (you → Autopilot)", () => {
    const [m0] = initialRuntimeState()
    const [m1] = update(m0, ChangedShellInput({ value: "drive me" }))
    const [m2] = update(m1, SubmittedShell())
    const [m3] = update(
      m2,
      RespondedShell({ prompt: "drive me", text: "ok" }),
    )
    // A driver (Claude) reads this and sees the same state the owner does.
    expect(shellTranscriptText(m3)).toBe("you: drive me\nautopilot: ok")
    // And every line it reads is present in the rendered view (true parity).
    const tree = serialize(view(m3).body)
    expect(tree).toContain("drive me")
    expect(tree).toContain("ok")
  })
})

describe("zero-base shell: open / close the hidden full UI", () => {
  test("OpenedPanes reveals the full UI (lands on chat) and the chat pane mounts", () => {
    const [m0] = initialRuntimeState()
    const [m1] = update(m0, OpenedPanes())
    expect(m1.pane).toBe("chat")
    const tree = serialize(view(m1).body)
    // The full UI is back: sidebar + the chat pane render when opened.
    expect(tree).toContain("sidebar")
    expect(tree).toContain("chat-pane")
  })

  test("ClosedPanes returns to the black shell and closes the palette", () => {
    const opened = Model.make({
      ...initialModel,
      pane: "settings",
      commandPaletteOpen: true,
    })
    const [closed] = update(opened, ClosedPanes())
    expect(closed.pane).toBe("shell")
    expect(closed.commandPaletteOpen).toBe(false)
  })

  test("Escape from any pane returns to the shell — you can never get trapped", () => {
    // The bug: opening the full UI (Cmd-K / open-panes) left no way back.
    const inPane = Model.make({ ...initialModel, pane: "chat", commandPaletteOpen: false })
    const esc = { key: "Escape", meta: false, ctrl: false, shift: false, inEditable: false }
    expect(interpretKey(inPane, esc)).toEqual({ kind: "back-to-shell" })
    // ...and the reducer takes it home (ClosedPanes → shell).
    const [home] = update(inPane, ClosedPanes())
    expect(home.pane).toBe("shell")
    // On the shell itself, Escape is not a back-to-shell (nothing to escape).
    const onShell = Model.make({ ...initialModel, pane: "shell", commandPaletteOpen: false })
    expect(interpretKey(onShell, esc)).not.toEqual({ kind: "back-to-shell" })
  })

  test("the full UI always renders a visible 'back to shell' control", () => {
    const tree = serialize(view(Model.make({ ...initialModel, pane: "chat" })).body)
    expect(tree).toContain("shell-return")
  })

  test("ALL panes still mount when opened (the kept UI is not broken)", () => {
    // The black-screen guard already covers this for every PaneId; assert here
    // too that the new `shell` literal is part of the closed set and renders.
    expect(PaneId.literals).toContain("shell")
    for (const pane of PaneId.literals) {
      const doc = view(Model.make({ ...initialModel, pane })) as {
        body: unknown
      }
      expect(doc.body).toBeDefined()
    }
  })
})
