/**
 * META-1 (#9180): oracle tests for the one-agent identity front door.
 *
 * Behavior contract: openagents_desktop.identity.one_agent_front_door.v1
 * (src/contracts/ux-contracts.ts). The desktop conversation fronts ONE named
 * persistent agent; delegated work stays attributed inside that conversation
 * (lane/model per response, linked Full Auto run cards); the machinery views
 * remain reachable observability. Presentation only — these tests also pin
 * that no new authority or dispatch path is created.
 */
import { describe, expect, test } from "vite-plus/test"

import type { FullAutoRunProjection } from "../full-auto-run-ipc-contract.ts"
import {
  DESKTOP_AGENT_NAME,
  agentLaneAttribution,
  projectAgentConversationRunLinks,
} from "./agent-identity.ts"
import { desktopSidebarDestinationDefinitions } from "./sidebar-destinations.ts"
import { initialDesktopShellState } from "./shell.ts"

const runFixture = (extra: Partial<FullAutoRunProjection> = {}): FullAutoRunProjection => ({
  runRef: "run.fa.1",
  threadRef: "thread.local.1",
  title: "Fix the failing renderer test",
  objective: "Fix the failing renderer test and rerun the suite.",
  objectiveSource: "user",
  doneCondition: "The named verification passes.",
  workspaceRef: "workspace.test",
  lane: "codex-local",
  turnCap: 20,
  successfulAttempts: 1,
  failedAttempts: 0,
  state: "running",
  stateRevision: 3,
  terminalReason: null,
  predecessorRunRef: null,
  migratedFrom: null,
  createdAt: "2026-07-22T00:00:00.000Z",
  startedAt: "2026-07-22T00:00:01.000Z",
  lastProgressAt: "2026-07-22T00:01:00.000Z",
  pausedAt: null,
  stoppedAt: null,
  completedAt: null,
  transitions: [],
  stallCause: null,
  nextRetryAt: null,
  recoveryAction: "none",
  ...extra,
})

describe("META-1 one-agent identity (#9180)", () => {
  test("the agent identity is the existing in-product OpenAgents identity, not a new persona", () => {
    expect(DESKTOP_AGENT_NAME).toBe("OpenAgents")
  })

  test("a fresh session opens into the one-agent conversation (chat workspace, no picker)", () => {
    const state = initialDesktopShellState("electron/darwin")
    expect(state.workspace).toBe("chat")
    // No thread selection ceremony: the conversation surface is primary and
    // the composer submits into it directly.
    expect(state.activeThreadId).toBeNull()
  })

  test("the observability entries (New session, Full Auto, Settings) remain reachable — no surface is deleted", () => {
    expect(desktopSidebarDestinationDefinitions.map(destination => destination.label)).toEqual([
      "New session",
      "Full Auto",
      "Settings",
    ])
  })

  test("lane attribution is derived only from host-stamped metadata", () => {
    expect(agentLaneAttribution({ lane: "codex-local", model: "gpt-5.6-sol" })).toBe(
      "via codex-local · gpt-5.6-sol",
    )
    expect(agentLaneAttribution({ lane: "claude-local" })).toBe("via claude-local")
    expect(agentLaneAttribution({ lane: "acp:grok-cli", model: "grok-4" })).toBe(
      "via acp:grok-cli · grok-4",
    )
    // No lane recorded → no invented attribution.
    expect(agentLaneAttribution({ model: "gpt-5.6-sol" })).toBeUndefined()
    expect(agentLaneAttribution({ lane: "  " })).toBeUndefined()
    expect(agentLaneAttribution(undefined)).toBeUndefined()
  })

  test("Full Auto runs bound to the active conversation project as linked run cards", () => {
    const links = projectAgentConversationRunLinks(
      [runFixture(), runFixture({ runRef: "run.fa.2", threadRef: "thread.other" })],
      "thread.local.1",
    )
    expect(links).toEqual([
      {
        runRef: "run.fa.1",
        title: "Fix the failing renderer test",
        statusLabel: "Running",
        lane: "codex-local",
      },
    ])
    expect(projectAgentConversationRunLinks([runFixture()], null)).toEqual([])
    expect(projectAgentConversationRunLinks([runFixture({ threadRef: null })], "thread.local.1")).toEqual([])
  })
})

describe("META-1 conversation projections (#9180)", () => {
  test("the final assistant response of a turn carries its honest lane/model attribution", async () => {
    const { projectLocalTimelineRecords } = await import("./react-timeline.tsx")
    const records = projectLocalTimelineRecords([
      { key: "user-1", role: "user", text: "Ship it", timestamp: "05:40" },
      {
        key: "turn-1-assistant-0",
        role: "assistant",
        text: "Working on it.",
        timestamp: "05:40",
        meta: { lane: "codex-local", turnRef: "turn.codex.1" },
      },
      {
        key: "turn-1-assistant-1",
        role: "assistant",
        text: "Done — the tests are green.",
        timestamp: "05:41",
        meta: { lane: "codex-local", model: "gpt-5.6-sol", turnRef: "turn.codex.1" },
      },
    ])
    const finalAssistant = records.find(record => record.key === "turn-1-assistant-1")
    expect(finalAssistant?.attribution).toBe("via codex-local · gpt-5.6-sol")
    // Intermediate segments stay visually quiet — attribution is stamped on
    // the terminal assistant record of the turn only.
    expect(records.find(record => record.key === "turn-1-assistant-0")?.attribution).toBeUndefined()
    // No lane meta → no invented attribution.
    const bare = projectLocalTimelineRecords([
      { key: "user-2", role: "user", text: "hi", timestamp: "05:42" },
      { key: "turn-2-assistant-0", role: "assistant", text: "hello", timestamp: "05:42" },
    ])
    expect(bare.find(record => record.key === "turn-2-assistant-0")?.attribution).toBeUndefined()
  })

  test("delegated answers keep their #9127 subagent attribution — never overwritten by lane meta", async () => {
    const { projectLocalTimelineRecords } = await import("./react-timeline.tsx")
    const records = projectLocalTimelineRecords([
      { key: "user-1", role: "user", text: "Summarize the last reply", timestamp: "05:40" },
      {
        key: "delegation-request.claude.1",
        role: "system",
        text: "",
        timestamp: "05:41",
        runtime: {
          kind: "child",
          turnRef: "request.claude.1",
          childRef: "codex",
          status: "completed",
          title: "Claude subagent",
          detail: "Summarize the last reply",
          steered: null,
          transcript: [
            { role: "user", text: "Summarize the last reply" },
            { role: "assistant", text: "The reply says the tests are green." },
          ],
        },
      },
    ])
    const promoted = records.find(record => record.key.endsWith(":promoted-answer"))
    expect(promoted?.attribution).toBe("via Claude subagent")
  })

  test("the active conversation surfaces its bound Full Auto run as a linked card through the existing run-view intent", async () => {
    const { projectReactStatusNotices } = await import("./react-review.tsx")
    const base = initialDesktopShellState("electron/darwin")
    const state = {
      ...base,
      activeThreadId: "thread.local.1",
      harnessLanes: { ...base.harnessLanes, codex: { available: true, reason: null } },
      fullAuto: { ...base.fullAuto, runs: [runFixture()] },
    }
    const notice = projectReactStatusNotices(state).find(entry => entry.kind === "full_auto_run")
    expect(notice).toMatchObject({
      key: "full-auto-run:run.fa.1",
      title: "Full Auto run · Running",
      detail: "Fix the failing renderer test · via codex-local",
      // The link routes through the EXISTING FA-UX-01 run-view intent —
      // observability preserved, no new authority or dispatch path.
      action: { label: "Open run", intent: "DesktopFullAutoRunOpened", payload: "run.fa.1" },
    })
    // A conversation with no bound run shows no run card.
    expect(
      projectReactStatusNotices({ ...state, activeThreadId: "thread.other" }).find(
        entry => entry.kind === "full_auto_run",
      ),
    ).toBeUndefined()
  })
})
