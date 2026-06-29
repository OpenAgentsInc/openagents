import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { Effect } from "effect"

import {
  hashPylonAccountRef,
  loadPylonAccountRegistry,
  loadPylonAccountRegistryEffect,
  publicPylonAccountSelection,
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
} from "../src/account-registry"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe } from "../src/state"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-registry-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("pylon account registry", () => {
  test("strict Effect loader distinguishes missing and malformed registry config", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })

      await expect(Effect.runPromise(loadPylonAccountRegistryEffect(summary))).rejects.toMatchObject({
        kind: "not_found",
        operation: "load_account_registry",
      })

      await writeFile(summary.paths.config, "{not json")
      await expect(Effect.runPromise(loadPylonAccountRegistryEffect(summary))).rejects.toMatchObject({
        kind: "malformed",
        operation: "load_account_registry",
      })

      const failSoft = await loadPylonAccountRegistry(summary)
      expect(failSoft).toEqual([])
    })
  })

  test("loads named credential homes from dev.accounts without projecting paths", async () => {
    await withHome(async (home) => {
      const codexHome = join(home, "codex-a")
      const claudeHome = join(home, "claude-a")
      await mkdir(codexHome, { recursive: true })
      await mkdir(claudeHome, { recursive: true })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                { ref: "codex-a", provider: "codex", home: codexHome },
                { ref: "claude-a", provider: "claude_agent", home: claudeHome },
                { ref: "bad provider", provider: "codex", home: "/ignored" },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )

      const entries = await loadPylonAccountRegistry(summary)
      expect(entries.map(entry => `${entry.provider}:${entry.ref}`)).toEqual([
        "codex:codex-a",
        "claude_agent:claude-a",
      ])

      const resolved = await resolvePylonAccountSelection(summary, {
        provider: "codex",
        accountRef: "codex-a",
      })
      expect(resolved?.selector).toBe("registry_ref")
      expect(resolved?.home).toBe(codexHome)
      expect(resolved?.accountRefHash).toBe(hashPylonAccountRef("codex", "codex-a"))

      const publicSelection = publicPylonAccountSelection(resolved)
      expect(publicSelection).toEqual({
        provider: "codex",
        selector: "registry_ref",
        accountRefHash: hashPylonAccountRef("codex", "codex-a"),
      })
      expect(JSON.stringify(publicSelection)).not.toContain(codexHome)
      assertPublicProjectionSafe(publicSelection)
      const retainedProofFragment = JSON.stringify({ account: publicSelection })
      expect(retainedProofFragment).toContain("account.pylon.codex.")
      expect(retainedProofFragment).not.toContain(codexHome)
    })
  })

  test("resolves direct homes and builds provider-specific child env", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const resolved = await resolvePylonAccountSelection(summary, {
        provider: "claude_agent",
        accountHome: home,
      })
      const env = pylonAccountEnvironment({ PATH: "/bin", HOME: "/base" }, resolved)
      expect(env).toMatchObject({
        PATH: "/bin",
        HOME: "/base",
        CLAUDE_CONFIG_DIR: home,
      })
      expect(env.CODEX_HOME).toBeUndefined()
      expect(publicPylonAccountSelection(resolved)?.accountRefHash).toBe(
        hashPylonAccountRef("claude_agent", home),
      )
    })
  })

  test("injects a pooled Claude account's OAuth token without projecting it", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const resolved = await resolvePylonAccountSelection(summary, {
        provider: "claude_agent",
        accountHome: home,
      })

      // No token file: only CLAUDE_CONFIG_DIR is set (prior behavior).
      const baseline = pylonAccountEnvironment({ PATH: "/bin" }, resolved)
      expect(baseline.CLAUDE_CONFIG_DIR).toBe(home)
      expect(baseline.CLAUDE_CODE_OAUTH_TOKEN).toBeUndefined()

      // With a token file: CLAUDE_CODE_OAUTH_TOKEN is injected into the env only.
      await writeFile(join(home, "claude-oauth-token"), "sk-ant-oat-test-token-value\n")
      const env = pylonAccountEnvironment({ PATH: "/bin" }, resolved)
      expect(env.CLAUDE_CONFIG_DIR).toBe(home)
      expect(env.CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat-test-token-value")

      // The token never reaches the resolved or public projection of the account.
      expect(JSON.stringify(resolved)).not.toContain("sk-ant-oat-test-token-value")
      const publicSelection = publicPylonAccountSelection(resolved)
      expect(JSON.stringify(publicSelection)).not.toContain("sk-ant-oat-test-token-value")
      assertPublicProjectionSafe(publicSelection)
    })
  })

  test("resolves discovered Claude account refs against sibling account homes", async () => {
    await withHome(async (home) => {
      const root = join(home, "scan-root")
      const claudeHome = join(root, ".claude-pylon-2")
      await mkdir(claudeHome, { recursive: true })
      await writeFile(join(claudeHome, "claude-oauth-token"), "sk-ant-oat-discovered\n")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const previousRoot = process.env.PYLON_ACCOUNT_HOME_ROOT
      process.env.PYLON_ACCOUNT_HOME_ROOT = root
      try {
        const resolved = await resolvePylonAccountSelection(summary, {
          provider: "claude_agent",
          accountRef: "claude-pylon-2",
        })
        expect(resolved).toMatchObject({
          provider: "claude_agent",
          selector: "registry_ref",
          accountRef: "claude-pylon-2",
          accountRefHash: hashPylonAccountRef("claude_agent", claudeHome),
          home: claudeHome,
        })

        const env = pylonAccountEnvironment({ PATH: "/bin" }, resolved)
        expect(env.CLAUDE_CONFIG_DIR).toBe(claudeHome)
        expect(env.CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat-discovered")
      } finally {
        if (previousRoot === undefined) {
          delete process.env.PYLON_ACCOUNT_HOME_ROOT
        } else {
          process.env.PYLON_ACCOUNT_HOME_ROOT = previousRoot
        }
      }
    })
  })

  test("refuses unknown refs, ambiguous selectors, and missing homes", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          dev: {
            accounts: [{ ref: "missing", provider: "codex", home: join(home, "missing") }],
          },
        }),
      )

      await expect(
        resolvePylonAccountSelection(summary, { provider: "codex", accountRef: "nope" }),
      ).rejects.toMatchObject({ blockerRef: "blocker.pylon.account_ref_unknown" })
      await expect(
        resolvePylonAccountSelection(summary, {
          provider: "codex",
          accountRef: "missing",
        }),
      ).rejects.toMatchObject({ blockerRef: "blocker.pylon.account_home_missing" })
      await expect(
        resolvePylonAccountSelection(summary, {
          provider: "codex",
          accountRef: "missing",
          accountHome: home,
        }),
      ).rejects.toMatchObject({ blockerRef: "blocker.pylon.account_selector_ambiguous" })
    })
  })
})
