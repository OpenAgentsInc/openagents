// VCODE-08 (#5925): session list/detail panes synced to Codex accounts.
//
// These tests pin the pure filter projection, reducer preservation across
// node-state polls, and the UI contract that account hashes stay out of default
// list/filter surfaces but appear in the selected-session detail pane.

import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow, NodeStateMessage } from "../src/shared/rpc"
import { initialModel, Model, modelPaneLayer } from "../src/ui/model"
import {
  ChangedSessionAccountFilter,
  ChangedSessionAdapterFilter,
  ChangedSessionFilter,
  ChangedSessionWorkspaceFilter,
  GotNodeState,
  OpenedManagedPane,
  SelectedSession,
  SelectedSessionDetailView,
} from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import {
  SESSION_FILTER_ALL,
  projectSessionPane,
} from "../src/ui/session-pane-projection"

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

const workHash = "account.pylon.codex.work.abcdef0123456789abcdef0123456789"
const homeHash = "account.pylon.codex.home.11111111111111111111111111111111"

const account = (input: Partial<AccountRow> & Pick<AccountRow, "accountRefHash">): AccountRow => ({
  provider: input.provider ?? "codex",
  homeState: input.homeState ?? "present",
  ready: input.ready ?? true,
  accountRef: input.accountRef ?? "work",
  accountRefHash: input.accountRefHash,
  selector: input.selector ?? "registry_ref",
  blockerRefs: input.blockerRefs ?? [],
  priority: input.priority ?? 1,
})

const session = (input: Partial<SessionSummary> & Pick<SessionSummary, "sessionRef">): SessionSummary => ({
  sessionRef: input.sessionRef,
  adapter: input.adapter ?? "codex",
  state: input.state ?? "running",
  accountRefHash: input.accountRefHash ?? null,
  workspaceRef: input.workspaceRef,
  latestActivity: input.latestActivity,
  updatedAt: input.updatedAt ?? "2026-06-21T19:00:00.000Z",
})

const nodeState = (sessions: ReadonlyArray<SessionSummary>): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [...sessions],
  accounts: [
    account({ accountRefHash: workHash, accountRef: "work", priority: 0 }),
    account({ accountRefHash: homeHash, accountRef: "home", priority: 1 }),
  ],
  events: {
    "session.pylon.codex.work": [
      {
        eventIndex: 0,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T19:00:00.000Z",
        detail: "thinking: inspect session panes",
      },
      {
        eventIndex: 1,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T19:00:05.000Z",
        detail: "edited src/session-pane.ts (+7 -1)",
      },
    ],
  },
  artifacts: {},
})

describe("session pane projection (#5925)", () => {
  test("filters by status, adapter, account, and workspace with short account labels", () => {
    const sessions = [
      session({
        sessionRef: "session.pylon.codex.work",
        accountRefHash: workHash,
        workspaceRef: "workspace.openagents.desktop",
      }),
      session({
        sessionRef: "session.pylon.codex.home",
        accountRefHash: homeHash,
        workspaceRef: "workspace.openagents.desktop",
        state: "completed",
      }),
      session({
        sessionRef: "session.pylon.claude.default",
        adapter: "claude_agent",
        workspaceRef: "workspace.openagents.docs",
      }),
    ]

    const projection = projectSessionPane({
      sessions,
      accounts: nodeState(sessions).accounts,
      filters: {
        status: "running",
        adapter: "codex",
        account: workHash,
        workspace: "workspace.openagents.desktop",
      },
    })

    expect(projection.sessions.map((row) => row.sessionRef)).toEqual([
      "session.pylon.codex.work",
    ])
    expect(projection.accountOptions.map((row) => row.label)).toContain("codex work")
    expect(projection.accountOptions.map((row) => row.label).join(" ")).not.toContain(workHash)
    expect(projection.workspaceOptions.map((row) => row.label)).toContain("desktop")
  })

  test("GotNodeState preserves selected session, filters, and detail view", () => {
    let model = Model.make({ ...initialModel, pane: "sessions" })
    ;[model] = update(model, ChangedSessionFilter({ filter: "running" }))
    ;[model] = update(model, ChangedSessionAdapterFilter({ adapter: "codex" }))
    ;[model] = update(model, ChangedSessionAccountFilter({ account: workHash }))
    ;[model] = update(
      model,
      ChangedSessionWorkspaceFilter({ workspace: "workspace.openagents.desktop" }),
    )
    ;[model] = update(model, SelectedSession({ sessionRef: "session.pylon.codex.work" }))
    ;[model] = update(model, SelectedSessionDetailView({ view: "terminal-log" }))

    const [afterPoll] = update(
      model,
      GotNodeState({
        node: nodeState([
          session({
            sessionRef: "session.pylon.codex.work",
            accountRefHash: workHash,
            workspaceRef: "workspace.openagents.desktop",
            latestActivity: "running tests",
          }),
        ]),
      }),
    )

    expect(afterPoll.selectedSessionRef).toBe("session.pylon.codex.work")
    expect(afterPoll.sessionFilter).toBe("running")
    expect(afterPoll.sessionAdapterFilter).toBe("codex")
    expect(afterPoll.sessionAccountFilter).toBe(workHash)
    expect(afterPoll.sessionWorkspaceFilter).toBe("workspace.openagents.desktop")
    expect(afterPoll.sessionDetailView).toBe("terminal-log")
  })

  test("session list hides account hashes while selected detail and linked panes expose them deliberately", () => {
    const sessions = [
      session({
        sessionRef: "session.pylon.codex.work",
        accountRefHash: workHash,
        workspaceRef: "workspace.openagents.desktop",
      }),
    ]
    let model = Model.make({ ...initialModel, pane: "sessions" })
    ;[model] = update(model, GotNodeState({ node: nodeState(sessions) }))

    const listTree = serializeView(view(model).body)
    const sessionsPaneTree = listTree.slice(
      listTree.indexOf("sessions-pane"),
      listTree.indexOf("hotbar"),
    )
    expect(listTree).toContain("session-filter-group")
    expect(listTree).toContain("codex work")
    expect(listTree).toContain("session-list-filtered-count")
    expect(sessionsPaneTree).not.toContain(workHash)

    ;[model] = update(model, SelectedSession({ sessionRef: "session.pylon.codex.work" }))
    const detailTree = serializeView(view(model).body)
    expect(detailTree).toContain("session-detail-pane-links")
    expect(detailTree).toContain("Agent Stream")
    expect(detailTree).toContain("Diff & Artifacts")
    expect(detailTree).toContain("Terminal / Log")
    expect(detailTree).toContain(workHash)

    ;[model] = update(model, OpenedManagedPane({ pane: "agent-stream" }))
    expect(modelPaneLayer(model).panes.some((pane) => pane.kind === "agent-stream")).toBe(true)
    expect(serializeView(view(model).body)).toContain("Thinking")
  })

  test("all filters can represent the unfiltered state", () => {
    const projection = projectSessionPane({
      sessions: [
        session({ sessionRef: "a", accountRefHash: workHash }),
        session({ sessionRef: "b", adapter: "apple_fm" }),
      ],
      accounts: [],
      filters: {
        status: "all",
        adapter: "all",
        account: SESSION_FILTER_ALL,
        workspace: SESSION_FILTER_ALL,
      },
    })
    expect(projection.sessions).toHaveLength(2)
  })
})
