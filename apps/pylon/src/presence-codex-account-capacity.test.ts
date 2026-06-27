import { describe, expect, test } from "bun:test"

import {
  codexAccountCapacities,
  codexAccountCapacityKey,
  codexAccountCapacityRefs,
  codexBusyByAccount,
  codexPerAccountConcurrency,
} from "./presence.js"
import { UNKEYED_ACTIVE_RUN_ACCOUNT } from "./active-assignment-runs.js"

const HASH_A = "account.pylon.codex.aaaaaaaaaaaa"
const KEY_A = "aaaaaaaaaaaa"
const HASH_B = "account.pylon.codex.bbbbbbbbbbbb"
const KEY_B = "bbbbbbbbbbbb"

describe("#6354 per-account Codex capacity (Pylon side)", () => {
  test("capacity key is the public-safe trailing hex of the account-ref hash", () => {
    expect(codexAccountCapacityKey(HASH_A)).toBe(KEY_A)
    expect(codexAccountCapacityKey("not-an-account-hash")).toBeNull()
  })

  test("each ready account advertises its own slots minus its own busy load", () => {
    const accounts = codexAccountCapacities({
      busyByAccount: { [HASH_A]: 8, [HASH_B]: 1 },
      perAccountConcurrency: 8,
      readiness: [
        { accountRefHash: HASH_A, ready: true },
        { accountRefHash: HASH_B, ready: true },
      ],
    })
    expect(accounts).toEqual([
      { accountKey: KEY_A, accountRefHash: HASH_A, available: 0, busy: 8, queued: 0, ready: 8 },
      { accountKey: KEY_B, accountRefHash: HASH_B, available: 7, busy: 1, queued: 0, ready: 8 },
    ])
  })

  test("not-ready accounts are omitted from advertised capacity", () => {
    const accounts = codexAccountCapacities({
      perAccountConcurrency: 4,
      readiness: [
        { accountRefHash: HASH_A, ready: false },
        { accountRefHash: HASH_B, ready: true },
      ],
    })
    expect(accounts.map(account => account.accountKey)).toEqual([KEY_B])
  })

  test("capacity/load refs use the counted per-account ref shape", () => {
    const refs = codexAccountCapacityRefs([
      { accountKey: KEY_A, accountRefHash: HASH_A, available: 6, busy: 2, queued: 0, ready: 8 },
    ])
    expect(refs.capacityRefs).toEqual([
      `capacity.coding.codex.account.${KEY_A}.ready=8`,
      `capacity.coding.codex.account.${KEY_A}.available=6`,
    ])
    expect(refs.loadRefs).toEqual([
      `load.coding.codex.account.${KEY_A}.busy=2`,
      `load.coding.codex.account.${KEY_A}.queued=0`,
    ])
  })

  test("codexBusyByAccount drops the unkeyed bucket", () => {
    expect(
      codexBusyByAccount({
        codex: { [HASH_A]: 3, [UNKEYED_ACTIVE_RUN_ACCOUNT]: 2 },
      }),
    ).toEqual({ [HASH_A]: 3 })
  })

  test("per-account concurrency falls back to the pooled concurrency env then 1", () => {
    expect(codexPerAccountConcurrency({})).toBe(1)
    expect(
      codexPerAccountConcurrency({ OPENAGENTS_PYLON_CODEX_CONCURRENCY: "8" }),
    ).toBe(8)
    expect(
      codexPerAccountConcurrency({
        OPENAGENTS_PYLON_CODEX_CONCURRENCY: "8",
        OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY: "3",
      }),
    ).toBe(3)
  })
})
