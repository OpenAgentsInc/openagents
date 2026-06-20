// ZERO-BASE SHELL (owner directive, 2026-06-19; demoted by #5820): the fallback
// shell remains a dead-simple black screen with NOTHING on it except a single
// text bar at the bottom (and the clean conversation above it once there is a
// response). The real launch surface is now the Verse; these tests pin the
// shell when it is explicitly opened.
//
// These tests pin: (1) the minimal fallback shell (shell pane, no panes
// rendered), (2) the text-bar → response loop (a pure reducer + one loopback
// command), (3) programmatic-control parity (the transcript projection matches
// what the view shows), and (4) the open/close panes toggle (the advanced UI is
// reachable and the panes still mount).

import { describe, expect, test } from "bun:test"
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
  CycledShellTarget,
  ClosedPanes,
  FailedShellCodingTurn,
  GotNodeState,
  OpenedPanes,
  RespondedShell,
  SelectedShellTarget,
  SubmittedShell,
  SucceededShellCodingTurn,
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

const fallbackShellModel = () => Model.make({ ...initialModel, pane: "shell" })

describe("zero-base shell: the minimal fallback surface", () => {
  test("the explicit fallback shell starts quiet", () => {
    const model = fallbackShellModel()
    expect(model.pane).toBe("shell")
    // The shell starts empty (truly black: no conversation, empty input).
    expect(model.shellTurns).toHaveLength(0)
    expect(model.shellInput).toBe("")
    expect(model.shellTarget).toBe("current")
    expect(model.shellPending).toBe(false)
  })

  test("the fallback shell view mounts a Document with the text bar and NO nav/sidebar/panes", () => {
    const model = fallbackShellModel()
    const doc = view(model) as { title: string; body: unknown }
    expect(doc.title).toBe("Autopilot")
    expect(doc.body).toBeDefined()
    const tree = serialize(doc.body)
    // The one surface: the bottom text bar exists.
    expect(tree).toContain("shell-input")
    expect(tree).toContain("shell-bar")
    expect(tree).toContain("shell-target-tabs")
    expect(tree).toContain("Shift+Tab cycles shell target")
    expect(tree).toContain("hotbar-inline")
    expect(tree.indexOf("hotbar-inline")).toBeLessThan(tree.indexOf("shell-input"))
    expect(tree).not.toContain("shell-open-panes")
    // The hidden full UI does NOT render by default: no sidebar, no pane chrome,
    // no settings surface on the default screen.
    expect(tree).not.toContain("sidebar")
    expect(tree).not.toContain("settings-pane")
    expect(tree).not.toContain("composer-pane")
  })

  test("before a response, the conversation area is absent — pure black", () => {
    const model = fallbackShellModel()
    const tree = serialize(view(model).body)
    expect(tree).not.toContain("shell-conversation")
  })
})

describe("zero-base shell: text bar → response loop", () => {
  test("Shift+Tab cycles the shell target; selecting a target is pure state", () => {
    const m0 = fallbackShellModel()
    const [m1, c1] = update(m0, CycledShellTarget())
    expect(m1.shellTarget).toBe("claude_code")
    expect(c1).toHaveLength(0)
    const [m2] = update(m1, CycledShellTarget())
    expect(m2.shellTarget).toBe("codex")
    const [m3] = update(m2, CycledShellTarget())
    expect(m3.shellTarget).toBe("current")
    const [selected] = update(m3, SelectedShellTarget({ target: "codex" }))
    expect(selected.shellTarget).toBe("codex")
  })

  test("typing tracks the input; an empty submit is a no-op (no error chrome)", () => {
    const m0 = fallbackShellModel()
    const [m1, c1] = update(m0, ChangedShellInput({ value: "  " }))
    expect(m1.shellInput).toBe("  ")
    const [m2, c2] = update(m1, SubmittedShell())
    expect(m2.shellTurns).toHaveLength(0)
    expect(c2).toHaveLength(0)
    void c1
  })

  test("submitting records the user turn, sets pending, and dispatches the loopback command", () => {
    const m0 = fallbackShellModel()
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
    const m0 = fallbackShellModel()
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
    const m0 = fallbackShellModel()
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

  test("Claude Code target submits through the coding session bridge", () => {
    const m0 = fallbackShellModel()
    const [m1] = update(m0, SelectedShellTarget({ target: "claude_code" }))
    const [m2] = update(m1, ChangedShellInput({ value: "fix the crash" }))
    const [m3, commands] = update(m2, SubmittedShell())
    expect(m3.shellInput).toBe("")
    expect(m3.shellPending).toBe(true)
    expect(m3.shellTurns[0]).toMatchObject({
      role: "you",
      target: "claude_code",
      text: "fix the crash",
    })
    expect(commands).toHaveLength(1)
    expect(commands[0]?.args).toMatchObject({
      target: "claude_code",
      adapter: "claude_agent",
      prompt: "fix the crash",
      objective: "fix the crash",
      useDefaultWorktree: true,
    })

    const [m4] = update(
      m3,
      SucceededShellCodingTurn({
        target: "claude_code",
        prompt: "fix the crash",
        sessionRef: "session.pylon.claude_agent.abc",
      }),
    )
    expect(m4.shellPending).toBe(false)
    expect(m4.shellClaudeSessionRef).toBe("session.pylon.claude_agent.abc")
    expect(m4.shellCodexSessionRef).toBe(null)
    expect(m4.shellClaudeTurns).toEqual(["fix the crash"])
    expect(m4.shellTurns[1]).toMatchObject({
      role: "autopilot",
      target: "claude_code",
    })
    expect(m4.shellTurns[1]?.text).toContain("Claude Code started")
  })

  test("Codex target keeps its own continuation state", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "codex",
      shellCodexSessionRef: "session.pylon.codex.first",
      shellCodexTurns: ["first task"],
      shellInput: "follow up",
    })
    const [submitted, commands] = update(start, SubmittedShell())
    expect(commands).toHaveLength(1)
    expect(commands[0]?.args).toMatchObject({
      target: "codex",
      adapter: "codex",
      prompt: "follow up",
      useDefaultWorktree: true,
    })
    expect(String(commands[0]?.args.objective)).toContain("first task")
    expect(String(commands[0]?.args.objective)).toContain("follow up")

    const [settled] = update(
      submitted,
      SucceededShellCodingTurn({
        target: "codex",
        prompt: "follow up",
        sessionRef: "session.pylon.codex.second",
      }),
    )
    expect(settled.shellCodexSessionRef).toBe("session.pylon.codex.second")
    expect(settled.shellCodexTurns).toEqual(["first task", "follow up"])
    expect(settled.shellClaudeTurns).toEqual([])
  })

  test("Codex shell turns reconcile the visible answer from node events", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "codex",
      shellInput: "who are you",
    })
    const [submitted] = update(start, SubmittedShell())
    const sessionRef = "session.pylon.control.2fbd41b3c640b32b6b44cfb1"
    const [spawned] = update(
      submitted,
      SucceededShellCodingTurn({
        target: "codex",
        prompt: "who are you",
        sessionRef,
      }),
    )
    expect(spawned.shellTurns.at(-1)?.text).toContain("Codex started")

    const [reconciled] = update(
      spawned,
      GotNodeState({
        node: {
          ok: true,
          schema: "test.node",
          sessions: [{ sessionRef, state: "failed", errorClass: "verification_failed" }],
          events: {
            [sessionRef]: [
              {
                eventIndex: 4,
                phase: "composer_event",
                state: "running",
                observedAt: "2026-06-20T02:38:55.507Z",
                detail: "agent: I’m Codex, a coding agent based on GPT-5.",
              },
            ],
          },
        },
      }),
    )

    expect(reconciled.shellTurns.at(-1)?.text).toBe(
      "I’m Codex, a coding agent based on GPT-5.",
    )
  })

  test("Codex shell turns follow proof external-session events when control events are redacted", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "codex",
      shellInput: "who are you",
    })
    const [submitted] = update(start, SubmittedShell())
    const sessionRef = "session.pylon.control.5ba05b978a0a8a8b5cc91551"
    const externalSessionRef = "session.pylon.codex_composer.be4d2b8c1eb3512e70bf59be"
    const [spawned] = update(
      submitted,
      SucceededShellCodingTurn({
        target: "codex",
        prompt: "who are you",
        sessionRef,
      }),
    )

    const [reconciled] = update(
      spawned,
      GotNodeState({
        node: {
          ok: true,
          schema: "test.node",
          sessions: [{ sessionRef, state: "completed" }],
          events: {
            [sessionRef]: [
              {
                eventIndex: 4,
                phase: "redaction_blocked",
                state: "running",
                observedAt: "2026-06-20T02:47:08.000Z",
                detail: "",
              },
            ],
            [externalSessionRef]: [
              {
                eventIndex: 3,
                phase: "agent_message",
                state: "completed",
                observedAt: "2026-06-20T02:47:09.708Z",
                detail: "agent: I’m Codex, a GPT-5-based coding agent.",
              },
            ],
          },
          artifacts: {
            [sessionRef]: {
              kind: "proof",
              outcome: "completed",
              editedFileCount: 0,
              commandCount: 0,
              totalTokens: 16570,
              detail: {
                schema: "openagents.pylon.control_session_artifact.v0.1",
                objectiveDigestRef: null,
                verifyRef: null,
                responseDigestRef: "digest.pylon.control_session.response.f06d8db3e867dbcfae487c13",
                externalSessionRef,
                executionPathRef: "control_session.composer",
                executionMode: "local_bounded",
                sandboxMode: "workspace-write",
                permissionMode: null,
                devCheckState: "passed",
                deviationRefs: [],
                redactionState: "clean",
                errorClass: null,
                errorDigestRef: null,
                workspaceRef: "workspace.pylon.control_session.injected.test",
              },
            },
          },
        },
      }),
    )

    expect(reconciled.shellTurns.at(-1)?.text).toBe(
      "I’m Codex, a GPT-5-based coding agent.",
    )
  })

  test("Codex shell turns render streamed thinking/token rows before final text", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "codex",
      shellInput: "say ready",
    })
    const [submitted] = update(start, SubmittedShell())
    const sessionRef = "session.pylon.control.live"
    const [spawned] = update(
      submitted,
      SucceededShellCodingTurn({
        target: "codex",
        prompt: "say ready",
        sessionRef,
      }),
    )

    const [streaming] = update(
      spawned,
      GotNodeState({
        node: {
          ok: true,
          schema: "test.node",
          sessions: [{ sessionRef, state: "running" }],
          events: {
            [sessionRef]: [
              {
                eventIndex: 1,
                phase: "composer_event",
                state: "running",
                observedAt: "t1",
                detail: "thread started",
              },
              {
                eventIndex: 2,
                phase: "composer_event",
                state: "running",
                observedAt: "t2",
                detail: "agent: ready",
              },
              {
                eventIndex: 3,
                phase: "reasoning",
                state: "running",
                observedAt: "t3",
                detail: "thinking tokens: 5; output tokens: 12",
              },
            ],
          },
        },
      }),
    )

    expect(streaming.shellTurns.at(-1)?.text).toBe(
      "thinking tokens: 5; output tokens: 12\nready",
    )
    const tree = serialize(view(streaming).body)
    expect(tree).toContain("shell-stream-part-tokens")
    expect(tree).toContain("shell-stream-part-answer")
    expect(tree).not.toContain("thinking tokens: 5; output tokens: 12\\nready")
  })

  test("Codex shell stream formats tool calls and tool results as distinct rows", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "codex",
      shellInput: "run a few tool calls",
    })
    const [submitted] = update(start, SubmittedShell())
    const sessionRef = "session.pylon.control.tools"
    const [spawned] = update(
      submitted,
      SucceededShellCodingTurn({
        target: "codex",
        prompt: "run a few tool calls",
        sessionRef,
      }),
    )

    const [streaming] = update(
      spawned,
      GotNodeState({
        node: {
          ok: true,
          schema: "test.node",
          sessions: [{ sessionRef, state: "running" }],
          events: {
            [sessionRef]: [
              {
                eventIndex: 1,
                phase: "tool_use",
                state: "running",
                observedAt: "t1",
                detail: "exec_command: bun test",
                full: 'exec_command\n{"cmd":"bun test"}',
              },
              {
                eventIndex: 2,
                phase: "tool_result",
                state: "running",
                observedAt: "t2",
                detail: "result: 2 tests passed",
                full: "2 tests passed\nno failures",
              },
              {
                eventIndex: 3,
                phase: "reasoning",
                state: "running",
                observedAt: "t3",
                detail: "thinking…",
                full: "checking the tool output",
              },
              {
                eventIndex: 4,
                phase: "agent_message",
                state: "running",
                observedAt: "t4",
                detail: "agent: done",
              },
            ],
          },
        },
      }),
    )

    expect(streaming.shellTurns.at(-1)?.text).toContain("exec_command:")
    expect(streaming.shellTurns.at(-1)?.text).toContain("result: 2 tests passed")
    const tree = serialize(view(streaming).body)
    expect(tree).toContain("shell-stream-part-tool")
    expect(tree).toContain("shell-stream-part-result")
    expect(tree).toContain("shell-stream-part-reasoning")
    expect(tree).toContain("shell-stream-part-answer")
    expect(tree).toContain("no failures")
  })

  test("coding target failures clear pending without adding to continuation state", () => {
    const start = Model.make({
      ...initialModel,
      pane: "shell",
      shellTarget: "claude_code",
      shellInput: "do it",
    })
    const [submitted] = update(start, SubmittedShell())
    const [failed] = update(
      submitted,
      FailedShellCodingTurn({
        target: "claude_code",
        prompt: "do it",
        error: "no auth",
      }),
    )
    expect(failed.shellPending).toBe(false)
    expect(failed.shellClaudeTurns).toEqual([])
    expect(failed.shellTurns.at(-1)?.text).toBe("Claude Code failed: no auth")
  })
})

describe("zero-base shell: programmatic-control parity", () => {
  test("shellTranscriptText projects EXACTLY what the user sees (you → Autopilot)", () => {
    const m0 = fallbackShellModel()
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
  test("OpenedPanes reveals the advanced full UI (lands on Code composer)", () => {
    const m0 = fallbackShellModel()
    const [m1] = update(m0, OpenedPanes())
    expect(m1.pane).toBe("composer")
    const tree = serialize(view(m1).body)
    // The full UI is back: sidebar + the Code composer render when opened.
    expect(tree).toContain("sidebar")
    expect(tree).toContain("composer-pane")
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

  test("advanced panes render a visible fallback control", () => {
    const tree = serialize(view(Model.make({ ...initialModel, pane: "composer" })).body)
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
