import { describe, expect, test } from "bun:test"

import {
  claudeAccountCapacityRefs,
  claudeBusyByAccount,
  claudePerAccountConcurrency,
  codexAccountCapacities,
  codexAccountCapacityKey,
} from "./presence.js"
import { UNKEYED_ACTIVE_RUN_ACCOUNT } from "./active-assignment-runs.js"

const HASH_A = "account.pylon.claude_agent.aaaaaaaaaaaa"
const KEY_A = "aaaaaaaaaaaa"
const HASH_B = "account.pylon.claude_agent.bbbbbbbbbbbb"
const KEY_B = "bbbbbbbbbbbb"

describe("#6421 per-account Claude capacity (Pylon side)", () => {
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
