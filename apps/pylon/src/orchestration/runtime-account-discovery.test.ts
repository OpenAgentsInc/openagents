import { join } from "node:path"
import { mkdtemp, mkdir, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { discoverPylonSiblingAccountHomes } from "../account-registry.js"

import { runtimeSiblingAccountDiscoveryEnv } from "./runtime-account-discovery.js"

describe("runtimeSiblingAccountDiscoveryEnv", () => {
  test("confines implicit sibling discovery to the declared Pylon account root", () => {
    const env = runtimeSiblingAccountDiscoveryEnv("/private/pylon-fable", {
      HOME: "/Users/owner",
    })
    expect(env.PYLON_ACCOUNT_HOME_ROOT).toBe(
      join("/private/pylon-fable", "accounts"),
    )
    expect(env.PYLON_ACCOUNT_HOME_ROOT).not.toBe("/Users/owner")
  })

  test("preserves an explicit bounded discovery root", () => {
    const env = runtimeSiblingAccountDiscoveryEnv("/private/pylon-fable", {
      PYLON_ACCOUNT_HOME_ROOT: "/private/approved-account-homes",
    })
    expect(env.PYLON_ACCOUNT_HOME_ROOT).toBe("/private/approved-account-homes")
  })

  test("does not discover a default-style sibling outside the Pylon account root", async () => {
    const root = await mkdtemp(join(tmpdir(), "runtime-account-custody-"))
    const pylonHome = join(root, ".pylon-fable")
    await mkdir(join(pylonHome, "accounts"), { recursive: true })
    await mkdir(join(root, ".codex"))
    try {
      const discovered = await discoverPylonSiblingAccountHomes(
        runtimeSiblingAccountDiscoveryEnv(pylonHome, {}),
      )
      expect(discovered).toEqual([])
      const explicitlyWidened = await discoverPylonSiblingAccountHomes({
        PYLON_ACCOUNT_HOME_ROOT: root,
      })
      expect(explicitlyWidened.map(account => account.home)).toEqual([
        join(root, ".codex"),
      ])
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
