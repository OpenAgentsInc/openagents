// VCODE-06 (#5923): compact Codex dock over the retained Verse scene.
//
// The dock is screen-space DOM hosted by the overlay layer. It is not a Three
// child and it does not replace the durable managed-pane inspectors. These tests
// pin the mode boundary, the active-session/permission controls, and the hide
// path that must leave both the coding session and Verse pose alone.

import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import type { NodeStateMessage } from "../src/shared/rpc"
import { initialModel, Model, modelPaneLayer } from "../src/ui/model"
import { ChangedVerseMode, GotNodeState, OpenedManagedPane } from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const activeNode = (): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [
    {
      sessionRef: "session.pylon.codex.live",
      adapter: "codex",
      state: "completed",
      accountRefHash: null,
      updatedAt: "2026-06-21T18:00:00.000Z",
    },
  ],
  approvals: [
    {
      approvalRef: "approval.codex.exec.1",
      kind: "exec",
      prompt: "Run bun test?",
      createdAt: "2026-06-21T18:01:00.000Z",
    },
  ],
  events: {
    "session.pylon.codex.live": [
      {
        eventIndex: 0,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T18:00:00.000Z",
        detail: "thinking: inspect the dock",
      },
      {
        eventIndex: 1,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T18:00:10.000Z",
        detail: "running: bun test exit 0",
      },
    ],
  },
})

describe("Verse code dock (#5923)", () => {
  test("renders only in Verse code mode and remains a DOM overlay, not a pane", () => {
    const explore = Model.make({ ...initialModel, pane: "chat", verseMode: "explore" })
    expect(serializeView(view(explore).body)).not.toContain("verse-code-dock")

    const code = Model.make({ ...explore, verseMode: "code" })
    const tree = serializeView(view(code).body)
    expect(tree).toContain("verse-code-dock")
    expect(tree).toContain("Code dock")
    expect(tree).toContain("Start Codex")
    expect(tree).not.toContain("pane-window")
  })

  test("shows active-session, follow-up, permission, and durable-inspector controls", () => {
    let model = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      composerSessionRef: "session.pylon.codex.live",
      composerTurns: ["ship a compact dock"],
      composerReply: "now add tests",
    })
    ;[model] = update(model, GotNodeState({ node: activeNode() }))

    const tree = serializeView(view(model).body)
    expect(tree).toContain("verse-code-dock-active-session")
    expect(tree).toContain("codex:live")
    expect(tree).toContain("reply to continue")
    expect(tree).toContain("Follow-up for this Codex session")
    expect(tree).toContain("Run bun test?")
    expect(tree).toContain("Allow once")
    expect(tree).toContain("Reject")
    expect(tree).toContain("Scoped always")
    expect(tree).toContain("Open Decisions")
    expect(tree).toContain("Diffs")
    expect(tree).toContain("agent-stream-row")
    expect(tree).toContain("agent-stream-row-key")
    expect(tree).toContain("Thinking")
    expect(tree).toContain("Check")
    expect(tree).not.toContain("approval.codex.exec.1")
  })

  test("hiding code mode preserves the active session, open panes, and Verse pose", () => {
    const pose = {
      regionRef: "world.region.tassadar",
      x: 3,
      y: 0,
      z: -2,
      yaw: 0.75,
      animation: "run",
      capturedAtMs: 42,
    } as const
    let model = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      composerSessionRef: "session.pylon.codex.live",
      verseSceneRestorePose: pose,
    })
    ;[model] = update(model, OpenedManagedPane({ pane: "composer" }))
    const openPaneIds = modelPaneLayer(model).panes.map((pane) => pane.id)

    const [hidden] = update(model, ChangedVerseMode({ mode: "explore" }))
    expect(hidden.verseMode).toBe("explore")
    expect(hidden.composerSessionRef).toBe("session.pylon.codex.live")
    expect(hidden.verseSceneRestorePose).toEqual(pose)
    expect(modelPaneLayer(hidden).panes.map((pane) => pane.id)).toEqual(openPaneIds)
    expect(serializeView(view(hidden).body)).not.toContain("verse-code-dock")
  })

  test("dock chrome is pass-through and only controls opt into pointer events", () => {
    const css = readFileSync(join(process.cwd(), "src/ui/styles.css"), "utf8")
    expect(css).toContain(".verse-code-dock {\n")
    expect(css).toMatch(/\.verse-code-dock\s*\{[^}]*pointer-events:\s*none;/s)
    expect(css).toMatch(/\.verse-code-dock-panel\s*\{[^}]*pointer-events:\s*none;/s)
    expect(css).toMatch(/\.verse-code-dock-textarea\s*\{[^}]*pointer-events:\s*auto;/s)
    expect(css).toMatch(/\.verse-code-dock-button,[\s\S]*?\.verse-code-dock-icon-button\s*\{[^}]*pointer-events:\s*auto;/s)
  })
})
