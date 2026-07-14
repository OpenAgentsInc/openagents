import { describe, expect, test } from "vite-plus/test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { hashPylonAccountRef } from "../account-registry.js"
import { loadCodexAccountHealthRecord } from "../codex-account-health-ledger.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../bootstrap.js"
import {
  PYLON_DEV_CHECK_SCHEMA,
  type PylonDevCheckProjection,
} from "../dev-loop.js"
import {
  createControlSessionActions,
  type ControlSessionActions,
  type ControlSessionExecutor,
} from "./control-sessions.js"

const passedDevCheck = (): PylonDevCheckProjection => ({
  schema: PYLON_DEV_CHECK_SCHEMA,
  observedAt: "2026-07-03T12:00:00.000Z",
  action: "check",
  state: "passed",
  changeSummary: {
    repo: {
      state: "not_git",
      rootRef: null,
      branch: null,
      commit: null,
    },
    dirty: {
      state: "unknown",
      changedCount: 0,
      stagedCount: 0,
      unstagedCount: 0,
      untrackedCount: 0,
    },
    changedFileRefs: [],
    areaRefs: [],
    blockerRefs: [],
  },
  checkPlan: {
    state: "ready",
    commandRefs: ["command.test.true"],
    blockerRefs: [],
  },
  commandResults: [
    {
      commandRef: "command.test.true",
      reasonRef: "check.test.true",
      cwdRef: "command.cwd.test",
      argvRef: "command.argv.true",
      exitCode: 0,
      status: "passed",
      durationMs: 0,
      stdoutBytes: 0,
      stderrBytes: 0,
      stdoutDigestRef: null,
      stderrDigestRef: null,
    },
  ],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: [],
})

const waitForTerminal = async (
  actions: ControlSessionActions,
  sessionRef: string,
) => {
  for (let index = 0; index < 50; index += 1) {
    const row = (await actions.list()).find((entry) => entry.sessionRef === sessionRef)
    if (
      row?.state === "completed" ||
      row?.state === "failed" ||
      row?.state === "cancelled"
    ) {
      return row
    }
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error("session did not finish")
}

describe("control session Codex account failover", () => {
  test("retries on another connected Codex account when the first account is exhausted", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-control-session-failover-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const accountA = join(home, "accounts", "codex", "codex-a")
      const accountB = join(home, "accounts", "codex", "codex-b")
      await mkdir(accountA, { recursive: true })
      await mkdir(accountB, { recursive: true })
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            { provider: "codex", ref: "codex-a", home: accountA },
            { provider: "codex", ref: "codex-b", home: accountB },
          ],
        },
      }))

      const attempts: Array<string | null> = []
      const executor: ControlSessionExecutor = async (input) => {
        attempts.push(input.account?.accountRef ?? null)
        if (input.account?.accountRef === "codex-a") {
          throw new Error("You have hit your usage limit. Please try again later.")
        }
        return {
          commandCount: 0,
          devCheck: passedDevCheck(),
          editedFileCount: 0,
          eventCount: 0,
          externalSessionRef: null,
          responseDigestRef: null,
          totalTokens: 0,
        }
      }

      const actions = createControlSessionActions({
        env: {},
        executor,
        summary,
      })
      const spawned = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        accountRef: "codex-a",
        objective: "Run a bounded failover proof.",
        verify: ["true"],
      })
      const terminal = await waitForTerminal(actions, spawned.sessionRef)
      const events = await actions.events(spawned.sessionRef)

      expect(terminal.state).toBe("completed")
      expect(terminal.accountRefHash).toBe(hashPylonAccountRef("codex", "codex-b"))
      expect(attempts).toEqual(["codex-a", "codex-b"])
      expect(events.recentEvents.some((event) =>
        event.messageText?.includes("retrying with another connected account")
      )).toBe(true)
      expect(
        (await loadCodexAccountHealthRecord(
          summary,
          hashPylonAccountRef("codex", "codex-a"),
        ))?.reason,
      ).toBe("usage_limited")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})

describe("control session Claude owner-local authority", () => {
  test("only an internal loopback launch can honor the local danger overlay", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-control-session-claude-authority-"))
    try {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json", "--pylon-ref", "pylon.owner.control"]),
        { PYLON_HOME: home },
      )
      const accountHome = join(home, "accounts", "claude_agent", "claude-named")
      await mkdir(accountHome, { recursive: true })
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          claudeExecutionMode: "local_supervised_danger",
          accounts: [
            { provider: "claude_agent", ref: "claude-named", home: accountHome },
          ],
        },
      }))

      const authorityRefs: Array<string | null> = []
      const executor: ControlSessionExecutor = async input => {
        authorityRefs.push(
          input.claudeOwnerLocalPermissionControl?.authority.authorityRef ?? null,
        )
        return {
          commandCount: 0,
          devCheck: passedDevCheck(),
          editedFileCount: 0,
          eventCount: 0,
          externalSessionRef: null,
          responseDigestRef: null,
          totalTokens: 0,
        }
      }
      const actions = createControlSessionActions({ env: {}, executor, summary })
      const bridgeLike = await actions.spawn({
        type: "session.spawn",
        adapter: "claude_agent",
        accountRef: "claude-named",
        objective: "Bridge-like bounded launch.",
        verify: ["true"],
      })
      const loopback = await actions.spawn({
        type: "session.spawn",
        adapter: "claude_agent",
        accountRef: "claude-named",
        objective: "Owner-local loopback launch.",
        verify: ["true"],
      }, { ownerLocalLoopback: true })
      await waitForTerminal(actions, bridgeLike.sessionRef)
      await waitForTerminal(actions, loopback.sessionRef)

      expect(authorityRefs[0]).toBeNull()
      expect(authorityRefs[1]).toMatch(
        /^authority\.pylon\.claude_owner_local\.[0-9a-f]{24}$/,
      )
      expect(JSON.stringify(await actions.list())).not.toContain("bypassPermissions")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})
