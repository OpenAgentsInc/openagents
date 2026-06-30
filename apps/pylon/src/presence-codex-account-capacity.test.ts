import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  codexAccountCapacities,
  codexAccountCapacityKey,
  codexAccountCapacityRefs,
  codexBusyByAccount,
  localCodexAccountReadiness,
  codexPerAccountConcurrency,
} from "./presence.js"
import { UNKEYED_ACTIVE_RUN_ACCOUNT } from "./active-assignment-runs.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { hashPylonAccountRef } from "./account-registry.js"
import { recordCodexAccountHealthFailure } from "./codex-account-health-ledger.js"
import { classifyCodexAccountFailure } from "./codex-account-health.js"

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

  test("revoked and usage-limited accounts are excluded with a specific readiness reason", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-account-health-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const goodHome = join(home, "accounts", "codex", "codex-good")
      const revokedHome = join(home, "accounts", "codex", "codex-revoked")
      await mkdir(goodHome, { recursive: true })
      await mkdir(revokedHome, { recursive: true })
      await writeFile(join(goodHome, "auth.json"), "{}")
      await writeFile(join(revokedHome, "auth.json"), "{}")
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            {
              provider: "codex",
              ref: "codex-good",
              home: goodHome,
              openAgentsProviderAccountRef: "provider_account.public.codex.good",
            },
            {
              provider: "codex",
              ref: "codex-revoked",
              home: revokedHome,
              openAgentsProviderAccountRef: "provider_account.public.codex.revoked",
            },
          ],
        },
      }))
      const revokedHash = hashPylonAccountRef("codex", "codex-revoked")
      await recordCodexAccountHealthFailure(summary, {
        accountRefHash: revokedHash,
        failure: classifyCodexAccountFailure("refresh token was revoked"),
        now: new Date("2026-06-28T22:00:00.000Z"),
      })

      const readiness = await localCodexAccountReadiness(summary, {
        PYLON_HOME: home,
        PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings"),
      })
      expect(readiness).toContainEqual({
        accountRefHash: hashPylonAccountRef("codex", "codex-good"),
        ready: true,
      })
      expect(readiness).toContainEqual({
        accountRefHash: revokedHash,
        ready: false,
        reason: "credentials_revoked",
      })
      expect(codexAccountCapacities({ perAccountConcurrency: 1, readiness }).map(account => account.accountRefHash)).toEqual([
        hashPylonAccountRef("codex", "codex-good"),
      ])
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("registry accounts without an OpenAgents provider-account link are not advertised", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-account-unlinked-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const linkedHome = join(home, "accounts", "codex", "codex-linked")
      const unlinkedHome = join(home, "accounts", "codex", "codex-unlinked")
      await mkdir(linkedHome, { recursive: true })
      await mkdir(unlinkedHome, { recursive: true })
      await writeFile(join(linkedHome, "auth.json"), "{}")
      await writeFile(join(unlinkedHome, "auth.json"), "{}")
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            {
              provider: "codex",
              ref: "codex-linked",
              home: linkedHome,
              openAgentsProviderAccountRef: "provider_account.public.codex.linked",
            },
            { provider: "codex", ref: "codex-unlinked", home: unlinkedHome },
          ],
        },
      }))

      const readiness = await localCodexAccountReadiness(summary, {
        PYLON_HOME: home,
        PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings"),
      })
      expect(readiness).toContainEqual({
        accountRefHash: hashPylonAccountRef("codex", "codex-linked"),
        ready: true,
      })
      expect(readiness).toContainEqual({
        accountRefHash: hashPylonAccountRef("codex", "codex-unlinked"),
        ready: false,
        reason: "account_unlinked",
      })
      expect(codexAccountCapacities({ perAccountConcurrency: 1, readiness }).map(account => account.accountRefHash)).toEqual([
        hashPylonAccountRef("codex", "codex-linked"),
      ])
    } finally {
      await rm(home, { recursive: true, force: true })
    }
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
