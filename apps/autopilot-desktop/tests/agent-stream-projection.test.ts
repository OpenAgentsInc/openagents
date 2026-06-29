// VCODE-07 (#5924): Codex Agent Stream projection.
//
// The stream rows are a public-safe projection over existing Pylon session
// summaries and event tails. They must stay stable across replayed batches and
// must not surface raw prompts, local paths, secrets, provider payloads, or full
// account hashes in the compact default UI contract.

import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow, SessionEventRow } from "../src/shared/rpc"
import {
  projectAgentStreamRows,
  sanitizeAgentStreamText,
} from "../src/ui/agent-stream-projection"

const session: SessionSummary = {
  sessionRef: "session.pylon.codex.live",
  adapter: "codex",
  state: "running",
  objectiveRef: "objective.pylon.codex.0123456789abcdef0123456789abcdef",
  accountRefHash: "account.pylon.codex.work.abcdef0123456789abcdef0123456789",
  latestActivity: "RAW PROMPT: add a secret token to /Users/me/project",
  updatedAt: "2026-06-21T18:00:00.000Z",
}

const account: AccountRow = {
  provider: "codex",
  homeState: "present",
  ready: true,
  accountRef: "work",
  accountRefHash: "account.pylon.codex.work.abcdef0123456789abcdef0123456789",
  selector: "registry_ref",
  blockerRefs: [],
  priority: 1,
}

const event = (
  eventIndex: number,
  phase: string,
  detail: string,
  state = "running",
): SessionEventRow => ({
  eventIndex,
  phase,
  state,
  observedAt: "2026-06-21T18:00:00.000Z",
  detail,
})

const events: SessionEventRow[] = [
  event(0, "progress", "thinking: inspect the route and update tests"),
  event(1, "progress", "web search: current Codex docs"),
  event(2, "progress", "edited /Users/me/project/src/secret.ts with sk-test-secret-token-123456789"),
  event(3, "progress", "running: bun test exit 0"),
  event(4, "decision_requested", "approval required: run deploy"),
  event(5, "failed", "error: provider payload {\"raw\":\"".concat("x".repeat(100), "\"}")),
  event(6, "completed", "turn completed", "completed"),
]

describe("Agent Stream projection (#5924)", () => {
  test("classifies fixture Codex events into compact stable row types", () => {
    const rows = projectAgentStreamRows({ session, events, accounts: [account] })
    expect(rows.map((row) => row.kind)).toEqual([
      "objective",
      "plan",
      "tool",
      "file",
      "check",
      "approval",
      "error",
      "done",
    ])
    expect(rows.map((row) => row.key)).toEqual(
      projectAgentStreamRows({ session, events, accounts: [account] }).map((row) => row.key),
    )
    expect(rows[0]?.body).toBe("objective #89abcdef")
    expect(rows[0]?.accountLabel).toBe("codex work")
    expect(rows[0]?.accountRefHash).toBe("#23456789")
  })

  test("redacts raw prompts, local paths, secrets, provider payloads, and full hashes", () => {
    const rows = projectAgentStreamRows({ session, events, accounts: [account] })
    const rendered = JSON.stringify(rows)
    expect(rendered).not.toContain("RAW PROMPT")
    expect(rendered).not.toContain("/Users/me/project")
    expect(rendered).not.toContain("sk-test-secret-token")
    expect(rendered).not.toContain("abcdef0123456789abcdef0123456789")
    expect(rendered).not.toContain("\"raw\"")
    expect(rendered).toContain("[local path]")
    expect(rendered).toContain("[secret]")
    expect(rendered).toContain("[provider payload]")
  })

  test("sanitizer keeps short readable rows without leaking long machine refs", () => {
    expect(
      sanitizeAgentStreamText(
        "check digest 0123456789abcdef0123456789abcdef at /Users/me/repo with ghp_1234567890abcdef",
      ),
    ).toBe("check digest #89abcdef at [local path] with [secret]")
  })
})
