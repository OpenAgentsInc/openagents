import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  KhalaFleetEventStore,
  OpenAgentsDesktopFleetManager,
  redactKhalaFleetLogText,
} from "../src/shared/khala-fleet-manager.js"

const tempPaths: string[] = []
const tempStorePaths: string[] = []

const tempDbPath = (): string => {
  const directory = mkdtempSync(join(tmpdir(), "openagents-desktop-fleet-"))
  tempPaths.push(directory)
  const path = join(directory, "fleet.sqlite")
  tempStorePaths.push(path)
  return path
}

afterEach(() => {
  for (const path of tempStorePaths.splice(0)) {
    OpenAgentsDesktopFleetManager.release(path)
  }
  for (const directory of tempPaths.splice(0)) {
    rmSync(directory, { force: true, recursive: true })
  }
})

describe("openagents desktop Khala fleet manager", () => {
  test("records every local lifecycle state as structured rows", () => {
    const manager = OpenAgentsDesktopFleetManager.acquire({ path: tempDbPath() })
    const planned = manager.plan(
      {
        accountRef: "codex-3",
        claimRef: "claim.issue.7591",
        issueRef: "7591",
        originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
        prRef: null,
        queueDecision: "selected urgent desktop issue",
        queueLane: "khala-code",
        verifier: "bun run --cwd clients/openagents-desktop test",
      },
      new Date("2026-06-29T15:00:00.000Z"),
    )

    expect(planned).toMatchObject({
      accountRef: "codex-3",
      claimRef: "claim.issue.7591",
      issueRef: "7591",
      queueDecision: "selected urgent desktop issue",
      queueLane: "khala-code",
      state: "planned",
    })
    expect(planned.plannedAt).toBe("2026-06-29T15:00:00.000Z")

    const dispatched = manager.dispatch(
      planned.id,
      { assignmentRef: "assignment.one" },
      new Date("2026-06-29T15:01:00.000Z"),
    )
    const accepted = manager.accept(
      planned.id,
      {},
      new Date("2026-06-29T15:02:00.000Z"),
    )
    const executing = manager.execute(
      planned.id,
      { pid: 4242 },
      new Date("2026-06-29T15:03:00.000Z"),
    )
    const completed = manager.complete(
      planned.id,
      {},
      new Date("2026-06-29T15:04:00.000Z"),
    )
    const rejected = manager.reject(
      planned.id,
      {
        reasonDetail: "provider refused the execution",
        reasonKind: "execution_refused",
      },
      new Date("2026-06-29T15:05:00.000Z"),
    )
    const retryable = manager.retry(
      planned.id,
      {
        reasonDetail: "capacity is cooling down",
        reasonKind: "capacity_unavailable",
      },
      new Date("2026-06-29T15:06:00.000Z"),
    )
    const blocked = manager.block(
      planned.id,
      {
        reasonDetail: "token usage did not reconcile",
        reasonKind: "token_reconciliation_failed",
      },
      new Date("2026-06-29T15:07:00.000Z"),
    )

    expect([
      planned.state,
      dispatched.state,
      accepted.state,
      executing.state,
      completed.state,
      rejected.state,
      retryable.state,
      blocked.state,
    ]).toEqual([
      "planned",
      "dispatched",
      "accepted",
      "executing",
      "completed",
      "rejected",
      "retryable",
      "blocked",
    ])
    expect(executing.pid).toBe(4242)
    expect(dispatched.assignmentRef).toBe("assignment.one")
    expect(blocked).toMatchObject({
      reasonKind: "token_reconciliation_failed",
      state: "blocked",
    })

    const tokenFailed = manager.recordTokenFailure(planned.id)
    expect(tokenFailed.tokenFailureCount).toBe(1)
    const counted = manager.reconcileTokens(planned.id, {
      inputTokens: 100,
      outputTokens: 40,
      reasoningTokens: 10,
      totalTokens: 150,
    })
    expect(counted).toMatchObject({
      tokenInputTokens: 100,
      tokenOutputTokens: 40,
      tokenReasoningTokens: 10,
      tokenTotalTokens: 150,
    })
    expect(counted.tokenReconciledAt).not.toBeNull()
  })

  test("keeps one controller instance per desktop fleet store", () => {
    const path = tempDbPath()
    const first = OpenAgentsDesktopFleetManager.acquire({ path })
    const second = OpenAgentsDesktopFleetManager.acquire({ path })

    expect(second).toBe(first)
    expect(first.reconstructActiveState().controller).toMatchObject({
      singletonActive: true,
      storePath: path,
    })

    OpenAgentsDesktopFleetManager.release(path)
    const third = OpenAgentsDesktopFleetManager.acquire({ path })
    expect(third).not.toBe(first)
  })

  test("reconstructs active, retryable, blocked, and completed rows after restart", () => {
    const path = tempDbPath()
    const firstStore = new KhalaFleetEventStore({ path })
    const active = firstStore.plan({
      accountRef: "codex-4",
      issueRef: "7591",
      originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
      prRef: null,
      verifier: "bun run --cwd clients/openagents-desktop typecheck",
    })
    firstStore.transition(active.id, "executing", {
      assignmentRef: "assignment.active",
      pid: 5001,
    })

    const retryable = firstStore.plan({
      accountRef: "codex-5",
      issueRef: null,
      originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
      prRef: "7557",
      verifier: "bun scripts/check-conflict-markers.mjs",
    })
    firstStore.transition(retryable.id, "retryable", {
      reasonKind: "capacity_unavailable",
    })

    const blocked = firstStore.plan({
      accountRef: "codex-6",
      issueRef: "7590",
      originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
      prRef: null,
      verifier: "bun test",
    })
    firstStore.transition(blocked.id, "blocked", {
      reasonKind: "credentials_missing",
    })

    const completed = firstStore.plan({
      accountRef: "codex-7",
      issueRef: "7589",
      originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
      prRef: null,
      verifier: "bun test",
    })
    firstStore.transition(completed.id, "completed")
    firstStore.close()

    const restartedStore = new KhalaFleetEventStore({ path })
    const snapshot = restartedStore.reconstructActiveState(
      new Date("2026-06-29T16:00:00.000Z"),
    )

    expect(snapshot.observedAt).toBe("2026-06-29T16:00:00.000Z")
    expect(snapshot.activeRows.map(row => row.assignmentRef)).toEqual([
      "assignment.active",
    ])
    expect(snapshot.retryableRows.map(row => row.reasonKind)).toEqual([
      "capacity_unavailable",
    ])
    expect(snapshot.blockedRows.map(row => row.reasonKind)).toEqual([
      "credentials_missing",
    ])
    expect(snapshot.completedRows.map(row => row.state)).toEqual(["completed"])
    restartedStore.close()
  })

  test("redacts secrets and local auth paths from event logs", () => {
    const store = new KhalaFleetEventStore({ path: tempDbPath() })
    const row = store.plan({
      accountRef: "codex-3",
      issueRef: "7591",
      originMainCommit: "0549290f5974d2a86be4ca8764c29728070038f3",
      prRef: null,
      verifier: "bun test",
    })

    const log = store.appendLog(row.id, {
      assignmentRef: "assignment.secret",
      eventType: "worker.output",
      message:
        "Bearer abc.def OPENAGENTS_AGENT_TOKEN=secret /Users/alice/.codex/auth.json",
      payload: {
        access_token: "secret-access-token",
        nested: "OPENAI_API_KEY=sk-secret",
      },
    })

    expect(log.message).toBe(
      "Bearer [REDACTED] OPENAGENTS_AGENT_TOKEN=[REDACTED] [REDACTED_CODEX_AUTH]",
    )
    expect(log.payloadJson).toContain("\"access_token\":\"[REDACTED]\"")
    expect(log.payloadJson).toContain("OPENAI_API_KEY=[REDACTED]")
    expect(log.payloadJson).not.toContain("secret")
    expect(redactKhalaFleetLogText("GITHUB_TOKEN=ghp_example")).toBe(
      "GITHUB_TOKEN=[REDACTED]",
    )

    store.close()
  })
})
