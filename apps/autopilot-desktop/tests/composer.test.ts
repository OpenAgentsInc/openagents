// #5355: coding-composer reducer + helper tests.
//
// The composer is the foreground "code in the app" loop on the EXISTING control
// protocol (session.spawn / events / cancel + approvals). These tests drive the
// pure reducer (update.ts) and the pure helpers (helpers.ts) through the whole
// loop — spawn → succeeded → reply/continue → cancel → new-thread — and the
// transcript/approval projections, without a DOM or a runtime, the same way the
// CL-53 tests cover the other panes.

import { describe, expect, test } from "bun:test"

import type { NodeStateMessage } from "../src/shared/rpc"
import {
  buildComposerContinuationObjective,
  composerCanReply,
  composerTurnSummary,
  isComposerTranscriptEvent,
} from "../src/ui/helpers"
import { initialModel, Model, modelNode } from "../src/ui/model"
import {
  ChangedComposerRepoPath,
  ChangedComposerReply,
  ChangedSpawnObjective,
  ClickedComposerNewThread,
  ClickedComposerReply,
  ClickedComposerSpawn,
  FailedComposerTurn,
  GotNodeState,
  NavigatedTo,
  SucceededComposerTurn,
} from "../src/ui/message"
import { update } from "../src/ui/update"

const eventRow = (
  over: Partial<{
    eventIndex: number
    phase: string
    state: string
    observedAt: string
    detail: string
    full: string
  }> = {},
) => ({
  eventIndex: over.eventIndex ?? 0,
  phase: over.phase ?? "progress",
  state: over.state ?? "running",
  observedAt: over.observedAt ?? "2026-06-18T12:00:00.000Z",
  detail: over.detail ?? "",
  full: over.full ?? "",
})

describe("composer helpers (#5355)", () => {
  test("isComposerTranscriptEvent: lifecycle-only rows are not transcript", () => {
    expect(isComposerTranscriptEvent(eventRow({ detail: "", full: "" }))).toBe(false)
    expect(isComposerTranscriptEvent(eventRow({ detail: "edited src/x.ts" }))).toBe(true)
    expect(isComposerTranscriptEvent(eventRow({ detail: "", full: "long diff" }))).toBe(true)
  })

  test("buildComposerContinuationObjective: first turn is just the follow-up", () => {
    expect(buildComposerContinuationObjective([], "add a test")).toBe("add a test")
  })

  test("buildComposerContinuationObjective: carries prior turns as context", () => {
    const objective = buildComposerContinuationObjective(
      ["add a /health route", "wire it into the router"],
      "now add a test for it",
    )
    expect(objective).toContain("Earlier turns in this thread")
    expect(objective).toContain("1. add a /health route")
    expect(objective).toContain("2. wire it into the router")
    expect(objective).toContain("Next instruction:")
    expect(objective).toContain("now add a test for it")
  })

  test("buildComposerContinuationObjective: bounded to the last 4 turns", () => {
    const objective = buildComposerContinuationObjective(
      ["t1", "t2", "t3", "t4", "t5"],
      "next",
    )
    expect(objective).not.toContain("t1")
    expect(objective).toContain("t2")
    expect(objective).toContain("t5")
  })

  test("composerCanReply: only terminal turns unlock a follow-up", () => {
    expect(composerCanReply(null)).toBe(false)
    expect(composerCanReply("queued")).toBe(false)
    expect(composerCanReply("running")).toBe(false)
    expect(composerCanReply("completed")).toBe(true)
    expect(composerCanReply("failed")).toBe(true)
    expect(composerCanReply("cancelled")).toBe(true)
  })

  test("composerTurnSummary: reflects state + turn count", () => {
    expect(composerTurnSummary(null, 0)).toBe("no session yet")
    expect(composerTurnSummary("running", 1)).toBe("running · 1 turn")
    expect(composerTurnSummary("completed", 2)).toContain("reply to continue")
    expect(composerTurnSummary("failed", 3)).toContain("reply to retry")
  })
})

describe("composer reducer (#5355)", () => {
  test("ClickedComposerSpawn validates the objective", () => {
    const [model, commands] = update(
      Model.make({ ...initialModel, spawnObjective: "   " }),
      ClickedComposerSpawn(),
    )
    expect(model.composerStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })

  test("ClickedComposerSpawn dispatches a spawn turn with repo path + records the turn", () => {
    const start = Model.make({
      ...initialModel,
      spawnObjective: "add a /health route and a test",
      composerRepoPath: "/Users/me/code/repo",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)
    expect(model.composerStatus.tone).toBe("info")
    expect(model.composerTurns).toEqual(["add a /health route and a test"])
    expect(commands).toHaveLength(1)
    // The command is a SpawnComposerTurn carrying the worktree path (no new contract).
    const cmd = commands[0] as unknown as { args?: { worktreePath?: string | null; objective?: string } }
    expect(cmd.args?.worktreePath).toBe("/Users/me/code/repo")
    expect(cmd.args?.objective).toBe("add a /health route and a test")
  })

  test("SucceededComposerTurn binds the active session and clears the objective box", () => {
    const spawned = Model.make({
      ...initialModel,
      spawnObjective: "do the thing",
      composerPending: true,
      composerTurns: ["do the thing"],
    })
    const [model] = update(
      spawned,
      SucceededComposerTurn({ sessionRef: "session.pylon.codex.abc" }),
    )
    expect(model.composerPending).toBe(false)
    expect(model.composerSessionRef).toBe("session.pylon.codex.abc")
    expect(model.composerStatus.tone).toBe("success")
    expect(model.spawnObjective).toBe("")
  })

  test("FailedComposerTurn surfaces the error and clears pending", () => {
    const [model] = update(
      Model.make({ ...initialModel, composerPending: true }),
      FailedComposerTurn({ error: "control 500" }),
    )
    expect(model.composerPending).toBe(false)
    expect(model.composerStatus.tone).toBe("error")
    expect(model.composerStatus.text).toBe("control 500")
  })

  test("ClickedComposerReply requires non-empty follow-up", () => {
    const active = Model.make({
      ...initialModel,
      composerSessionRef: "session.pylon.codex.abc",
      composerReply: "   ",
      composerTurns: ["first turn"],
    })
    const [model, commands] = update(active, ClickedComposerReply())
    expect(model.composerStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })

  test("ClickedComposerReply dispatches a continuation turn carrying prior context", () => {
    const active = Model.make({
      ...initialModel,
      composerSessionRef: "session.pylon.codex.abc",
      composerRepoPath: "/repo",
      composerReply: "now add a test",
      composerTurns: ["add a /health route"],
    })
    const [model, commands] = update(active, ClickedComposerReply())
    expect(model.composerPending).toBe(true)
    expect(model.composerReply).toBe("")
    expect(model.composerTurns).toEqual(["add a /health route", "now add a test"])
    expect(commands).toHaveLength(1)
    const cmd = commands[0] as unknown as { args?: { objective?: string; worktreePath?: string | null } }
    expect(cmd.args?.objective).toContain("add a /health route")
    expect(cmd.args?.objective).toContain("now add a test")
    expect(cmd.args?.worktreePath).toBe("/repo")
  })

  test("ClickedComposerNewThread clears the active session + history", () => {
    const active = Model.make({
      ...initialModel,
      composerSessionRef: "session.pylon.codex.abc",
      composerTurns: ["a", "b"],
      composerReply: "draft",
      composerStatus: { text: "running", tone: "success" },
      spawnObjective: "leftover",
    })
    const [model] = update(active, ClickedComposerNewThread())
    expect(model.composerSessionRef).toBe(null)
    expect(model.composerTurns).toEqual([])
    expect(model.composerReply).toBe("")
    expect(model.composerStatus.tone).toBe("idle")
    expect(model.spawnObjective).toBe("")
  })

  test("ChangedComposerRepoPath + ChangedComposerReply update the form", () => {
    const [withPath] = update(
      initialModel,
      ChangedComposerRepoPath({ value: "/x" }),
    )
    expect(withPath.composerRepoPath).toBe("/x")
    const [withReply] = update(
      initialModel,
      ChangedComposerReply({ value: "hi" }),
    )
    expect(withReply.composerReply).toBe("hi")
  })

  test("NavigatedTo composer loads the managed-account registry (CS-A1)", () => {
    const [model, commands] = update(initialModel, NavigatedTo({ pane: "composer" }))
    expect(model.pane).toBe("composer")
    // CS-A1: the composer hosts the per-session account picker + management UI,
    // so opening it refreshes the node's managed accounts.
    expect(commands).toHaveLength(1)
    expect(model.managedAccountsPending).toBe(true)
  })

  test("full loop: spawn → succeeded → live transcript via node poll → reply", () => {
    // Start a turn.
    let [model] = update(
      Model.make({ ...initialModel, spawnObjective: "add /health" }),
      ChangedSpawnObjective({ value: "add /health" }),
    )
    ;[model] = update(model, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)

    // Node accepts the spawn.
    ;[model] = update(
      model,
      SucceededComposerTurn({ sessionRef: "session.pylon.codex.live" }),
    )
    expect(model.composerSessionRef).toBe("session.pylon.codex.live")

    // A node poll lands carrying the live transcript + a completed terminal state.
    const node: NodeStateMessage = {
      ok: true,
      schema: "openagents.pylon.control.v0.3",
      sessions: [
        {
          sessionRef: "session.pylon.codex.live",
          adapter: "codex",
          state: "completed",
          accountRefHash: null,
          updatedAt: "2026-06-18T12:01:00.000Z",
        },
      ],
      events: {
        "session.pylon.codex.live": [
          eventRow({ eventIndex: 0, phase: "started", detail: "" }),
          eventRow({ eventIndex: 1, detail: "edited src/health.ts", state: "running" }),
          eventRow({ eventIndex: 2, phase: "completed", detail: "verify passed", state: "completed" }),
        ],
      },
      approvals: [
        { approvalRef: "ap-1", kind: "tool", prompt: "Run bun test?", createdAt: "2026-06-18T12:00:30.000Z" },
      ],
    }
    ;[model] = update(model, GotNodeState({ node }))
    const stored = modelNode(model)
    expect(stored?.sessions[0]?.state).toBe("completed")
    expect(stored?.events?.["session.pylon.codex.live"]?.length).toBe(3)
    // The completed turn unlocks a reply.
    expect(composerCanReply(stored?.sessions[0]?.state ?? null)).toBe(true)
    // One pending approval surfaces in-pane.
    expect(stored?.approvals?.length).toBe(1)

    // Continue the thread.
    ;[model] = update(model, ChangedComposerReply({ value: "now add a test" }))
    const [replied, replyCommands] = update(model, ClickedComposerReply())
    expect(replied.composerTurns).toEqual(["add /health", "now add a test"])
    expect(replyCommands).toHaveLength(1)
  })
})
