import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  claudeAccountCapacityRefs,
  claudeBusyByAccount,
  claudePerAccountConcurrency,
  codexAccountCapacities,
  codexAccountCapacityKey,
  codingServiceCapacityFromRuntime,
  localCodingServiceReadyCounts,
  localClaudeAccountCapacities,
  localClaudeAccountReadiness,
} from "./presence.js"
import { UNKEYED_ACTIVE_RUN_ACCOUNT } from "./active-assignment-runs.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { hashPylonAccountRef } from "./account-registry.js"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent.js"
import { ensurePylonLocalState } from "./state.js"

const HASH_A = "account.pylon.claude_agent.aaaaaaaaaaaa"
const KEY_A = "aaaaaaaaaaaa"
const HASH_B = "account.pylon.claude_agent.bbbbbbbbbbbb"
const KEY_B = "bbbbbbbbbbbb"

describe("#6421 per-account Claude capacity (Pylon side)", () => {
  test("discovers only authenticated Claude sibling homes for per-account readiness", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-capacity-"))
    try {
      const root = join(home, "scan-root")
      const realHome = join(root, ".claude-pylon-2")
      const supervisorHome = join(root, ".claude-supervisor")
      await mkdir(realHome, { recursive: true })
      await mkdir(supervisorHome, { recursive: true })
      await writeFile(join(realHome, "claude-oauth-token"), "sk-ant-oat-real\n")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })

      const readiness = await localClaudeAccountReadiness(summary, {
        PYLON_ACCOUNT_HOME_ROOT: root,
      } as NodeJS.ProcessEnv)

      expect(readiness).toContainEqual({
        accountRefHash: hashPylonAccountRef("claude_agent", realHome),
        ready: true,
      })
      expect(readiness).toContainEqual({
        accountRefHash: hashPylonAccountRef("claude_agent", supervisorHome),
        ready: false,
      })
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("a paused registered Claude account advertises no runnable slots while retaining active busy load", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-paused-capacity-"))
    try {
      const accountRef = "claude-paused"
      const accountHome = join(home, "accounts", "claude", accountRef)
      await mkdir(accountHome, { recursive: true })
      await writeFile(join(accountHome, "claude-oauth-token"), "credential-fixture\n")
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--json",
          "--capability-ref",
          CLAUDE_AGENT_CAPABILITY_REF,
        ]),
        { PYLON_HOME: home },
      )
      const state = await ensurePylonLocalState(summary)
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [{
            provider: "claude_agent",
            ref: accountRef,
            home: accountHome,
            paused: true,
          }],
        },
      }))
      const accountRefHash = hashPylonAccountRef("claude_agent", accountRef)
      const accountKey = codexAccountCapacityKey(accountRefHash)
      if (accountKey === null) throw new Error("expected a public-safe account capacity key")
      const env = {
        CODEX_HOME: join(home, "no-codex-login"),
        OPENAGENTS_PYLON_CLAUDE_BUSY: "0",
        OPENAGENTS_PYLON_CLAUDE_CONCURRENCY: "3",
        OPENAGENTS_PYLON_CLAUDE_QUEUED: "0",
        PYLON_ACCOUNT_HOME_ROOT: join(home, "scan-root"),
      } as NodeJS.ProcessEnv

      expect(await localClaudeAccountReadiness(summary, env)).toEqual([{
        accountRefHash,
        paused: true,
        ready: false,
      }])

      const accounts = await localClaudeAccountCapacities(
        state,
        summary,
        env,
        { [accountRefHash]: 2 },
      )
      expect(accounts).toEqual([{
        accountKey,
        accountRefHash,
        available: 0,
        busy: 2,
        queued: 0,
        ready: 0,
      }])
      const refs = claudeAccountCapacityRefs(accounts)
      expect(refs.capacityRefs).toEqual([
        `capacity.coding.claude.account.${accountKey}.ready=0`,
        `capacity.coding.claude.account.${accountKey}.available=0`,
      ])
      expect(refs.loadRefs).toEqual([
        `load.coding.claude.account.${accountKey}.busy=2`,
        `load.coding.claude.account.${accountKey}.queued=0`,
      ])
      expect(JSON.stringify(refs)).not.toContain(accountRef)
      expect(JSON.stringify(refs)).not.toContain(accountHome)

      const readyCounts = await localCodingServiceReadyCounts(summary, env)
      expect(readyCounts).toEqual({ claude: 0 })
      expect(
        codingServiceCapacityFromRuntime(state, env, readyCounts, { claude: 2 }),
      ).toEqual([{
        available: 0,
        busy: 2,
        queued: 0,
        ready: 0,
        service: "claude",
      }])
      expect(
        codingServiceCapacityFromRuntime(state, env, { claude: 1 }, { claude: 1 }),
      ).toEqual([{
        available: 2,
        busy: 1,
        queued: 0,
        ready: 3,
        service: "claude",
      }])
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("capacity key is the public-safe trailing hex of the claude account-ref hash", () => {
    expect(codexAccountCapacityKey(HASH_A)).toBe(KEY_A)
  })

  test("each ready Claude account advertises its own slots minus its own busy load", () => {
    // The capacity arithmetic is shared with Codex (codexAccountCapacities); the
    // service only differs in the ref string. One account's busy load never
    // lowers another account's available slots.
    const accounts = codexAccountCapacities({
      busyByAccount: { [HASH_A]: 2, [HASH_B]: 0 },
      perAccountConcurrency: 2,
      readiness: [
        { accountRefHash: HASH_A, ready: true },
        { accountRefHash: HASH_B, ready: true },
      ],
    })
    expect(accounts).toEqual([
      { accountKey: KEY_A, accountRefHash: HASH_A, available: 0, busy: 2, queued: 0, ready: 2 },
      { accountKey: KEY_B, accountRefHash: HASH_B, available: 2, busy: 0, queued: 0, ready: 2 },
    ])
  })

  test("capacity/load refs use the counted per-account Claude ref shape", () => {
    const refs = claudeAccountCapacityRefs([
      { accountKey: KEY_A, accountRefHash: HASH_A, available: 1, busy: 1, queued: 0, ready: 2 },
    ])
    expect(refs.capacityRefs).toEqual([
      `capacity.coding.claude.account.${KEY_A}.ready=2`,
      `capacity.coding.claude.account.${KEY_A}.available=1`,
    ])
    expect(refs.loadRefs).toEqual([
      `load.coding.claude.account.${KEY_A}.busy=1`,
      `load.coding.claude.account.${KEY_A}.queued=0`,
    ])
  })

  test("claudeBusyByAccount reads the claude bucket and drops the unkeyed bucket", () => {
    expect(
      claudeBusyByAccount({
        claude: { [HASH_A]: 1, [UNKEYED_ACTIVE_RUN_ACCOUNT]: 4 },
        codex: { [HASH_B]: 9 },
      }),
    ).toEqual({ [HASH_A]: 1 })
  })

  test("per-account Claude concurrency falls back to the pooled concurrency env then 1", () => {
    expect(claudePerAccountConcurrency({})).toBe(1)
    expect(
      claudePerAccountConcurrency({ OPENAGENTS_PYLON_CLAUDE_CONCURRENCY: "4" }),
    ).toBe(4)
    expect(
      claudePerAccountConcurrency({
        OPENAGENTS_PYLON_CLAUDE_CONCURRENCY: "4",
        OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY: "2",
      }),
    ).toBe(2)
  })
})
