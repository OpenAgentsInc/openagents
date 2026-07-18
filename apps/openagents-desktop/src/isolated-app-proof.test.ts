import { existsSync, mkdtempSync, rmSync } from "node:fs"
import { homedir, tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { buildCodexHistoryGraph, readRecentCodexHistory } from "./codex-history.ts"
import {
  isIsolatedAppProof,
  resolveClaudeProjectsRoot,
  resolveCodexSessionsRoot,
} from "./isolated-app-proof.ts"

/**
 * Regression coverage for #8999: the isolated-app-proof profile
 * (`OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1`) correctly scoped Electron's own
 * `userData`, but the Codex-history importer computed its `sessionsRoot`
 * independently and fell straight through to the real global
 * `~/.codex/sessions` — a real UI-automation session under isolated-app-proof
 * surfaced genuine unrelated Codex session titles from the host machine's
 * real history in the sidebar.
 *
 * These tests exercise the actual production importer functions
 * (`readRecentCodexHistory`, `buildCodexHistoryGraph`) against THIS
 * machine's real `~/.codex/sessions` where present, not fixture data, per
 * the issue's explicit requirement that an empty-history environment would
 * prove nothing.
 */

const realCodexSessionsRoot = path.join(homedir(), ".codex", "sessions")
const realHistoryPresent = existsSync(realCodexSessionsRoot)

/** Reconstructs the exact PRE-FIX resolution main.ts used before #8999: no
 * isolated-app-proof branch existed at all, so anything other than smoke
 * mode fell straight through to the real home directory. Used below only to
 * demonstrate the before/after delta — never as production logic. */
const preFixCodexSessionsRoot = (input: Readonly<{
  env: NodeJS.ProcessEnv
  smokeMode: boolean
  smokeFixtureRoot: string
  realHome: string
}>): string => path.resolve(
  input.env.OPENAGENTS_DESKTOP_CODEX_SESSIONS ?? (
    input.smokeMode
      ? path.join(input.smokeFixtureRoot, "codex-smoke", "sessions")
      : path.join(input.realHome, ".codex", "sessions")
  ),
)

describe("resolveCodexSessionsRoot / resolveClaudeProjectsRoot — isolated-app-proof scoping (#8999)", () => {
  test("ordinary (non-isolated, non-smoke) launch still resolves the real ~/.codex/sessions — no regression for real users", () => {
    const resolved = resolveCodexSessionsRoot({
      env: {},
      smokeMode: false,
      isolatedAppProofMode: false,
      smokeFixtureRoot: "/unused",
      userDataPath: "/unused",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/Users/example/.codex/sessions"))
  })

  test("ordinary (non-isolated, non-smoke) launch still resolves the real ~/.claude/projects — no regression for real users", () => {
    const resolved = resolveClaudeProjectsRoot({
      env: {},
      smokeMode: false,
      isolatedAppProofMode: false,
      smokeFixtureRoot: "/unused",
      userDataPath: "/unused",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/Users/example/.claude/projects"))
  })

  test("smoke mode keeps using its fixture path regardless of isolatedAppProofMode", () => {
    const resolved = resolveCodexSessionsRoot({
      env: {},
      smokeMode: true,
      isolatedAppProofMode: true,
      smokeFixtureRoot: "/fixtures",
      userDataPath: "/unused",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/fixtures/codex-smoke/sessions"))
  })

  test("isolated-app-proof mode resolves under the isolated userData path, never the real home directory", () => {
    const resolved = resolveCodexSessionsRoot({
      env: {},
      smokeMode: false,
      isolatedAppProofMode: true,
      smokeFixtureRoot: "/unused",
      userDataPath: "/tmp/openagents-desktop-isolated-abc123",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/tmp/openagents-desktop-isolated-abc123/isolated-codex-home/sessions"))
    expect(resolved).not.toContain("/Users/example")
    expect(resolved).not.toBe(path.resolve("/Users/example/.codex/sessions"))
  })

  test("isolated-app-proof mode resolves Claude projects under the isolated userData path too", () => {
    const resolved = resolveClaudeProjectsRoot({
      env: {},
      smokeMode: false,
      isolatedAppProofMode: true,
      smokeFixtureRoot: "/unused",
      userDataPath: "/tmp/openagents-desktop-isolated-abc123",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/tmp/openagents-desktop-isolated-abc123/isolated-claude-home/projects"))
    expect(resolved).not.toContain("/Users/example")
  })

  test("an explicit OPENAGENTS_DESKTOP_CODEX_SESSIONS override still wins under isolated-app-proof mode (deliberate test fixture wiring)", () => {
    const resolved = resolveCodexSessionsRoot({
      env: { OPENAGENTS_DESKTOP_CODEX_SESSIONS: "/explicit/fixture/sessions" },
      smokeMode: false,
      isolatedAppProofMode: true,
      smokeFixtureRoot: "/unused",
      userDataPath: "/tmp/openagents-desktop-isolated-abc123",
      realHome: "/Users/example",
    })
    expect(resolved).toBe(path.resolve("/explicit/fixture/sessions"))
  })

  test("an empty-string OPENAGENTS_DESKTOP_CLAUDE_PROJECTS override disables Claude import even under isolated-app-proof mode", () => {
    const resolved = resolveClaudeProjectsRoot({
      env: { OPENAGENTS_DESKTOP_CLAUDE_PROJECTS: "" },
      smokeMode: false,
      isolatedAppProofMode: true,
      smokeFixtureRoot: "/unused",
      userDataPath: "/tmp/openagents-desktop-isolated-abc123",
      realHome: "/Users/example",
    })
    expect(resolved).toBeNull()
  })

  test("before/after: the pre-#8999 resolution logic would have leaked the real home path under isolated-app-proof mode; the fixed resolver never does", () => {
    const env = {}
    const smokeFixtureRoot = "/unused"
    const realHome = "/Users/example"
    const before = preFixCodexSessionsRoot({ env, smokeMode: false, smokeFixtureRoot, realHome })
    const after = resolveCodexSessionsRoot({
      env,
      smokeMode: false,
      isolatedAppProofMode: true,
      smokeFixtureRoot,
      userDataPath: "/tmp/openagents-desktop-isolated-xyz789",
      realHome,
    })
    // The bug: the pre-fix path had no concept of isolatedAppProofMode at
    // all, so it always resolved to the real home — identical to what an
    // ordinary, non-isolated launch resolves to.
    expect(before).toBe(path.resolve(realHome, ".codex", "sessions"))
    // The fix: under isolated-app-proof mode the resolved path must differ
    // from the pre-fix (real-home) result.
    expect(after).not.toBe(before)
  })
})

describe("Codex-history importer under isolated-app-proof scoping — real-disk regression (#8999)", () => {
  test("the isolated sessionsRoot never surfaces any session, even though a genuinely non-empty real ~/.codex/sessions exists on this host", () => {
    if (!realHistoryPresent) {
      // Portable across machines/CI without real Codex history: the
      // path-level isolation assertions above already prove the fix. This
      // test adds the strongest possible proof (real importer + real disk
      // data) only when that fixture is actually available, exactly as the
      // issue asks — never fabricated/faked as a substitute.
      return
    }
    const isolatedUserData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-isolated-app-proof-test-"))
    try {
      expect(isIsolatedAppProof({
        env: { OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1" },
        userDataPath: isolatedUserData,
        temporaryDirectory: tmpdir(),
      })).toBe(true)

      const isolatedSessionsRoot = resolveCodexSessionsRoot({
        env: {},
        smokeMode: false,
        isolatedAppProofMode: true,
        smokeFixtureRoot: "/unused",
        userDataPath: isolatedUserData,
        realHome: homedir(),
      })

      // Sanity: the isolated root really is a fresh, currently-nonexistent
      // directory distinct from the real one this host actually has data in.
      expect(existsSync(isolatedSessionsRoot)).toBe(false)
      expect(isolatedSessionsRoot).not.toBe(realCodexSessionsRoot)

      const isolatedThreads = readRecentCodexHistory({ sessionsRoot: isolatedSessionsRoot })
      const isolatedGraph = buildCodexHistoryGraph(isolatedSessionsRoot)
      // The regression this test guards: before the fix, an isolated-mode
      // caller that (accidentally or via a stale code path) resolved to the
      // real root would get back genuine session titles here. Post-fix the
      // isolated root is always empty on disk, so the importer returns
      // nothing — never real host data.
      expect(isolatedThreads).toHaveLength(0)
      expect(isolatedGraph.agents).toHaveLength(0)
    } finally {
      rmSync(isolatedUserData, { recursive: true, force: true })
    }
  })

  test("sanity: this host's real ~/.codex/sessions is a genuinely non-empty fixture (the isolation test above is not vacuous)", () => {
    if (!realHistoryPresent) return
    const realGraph = buildCodexHistoryGraph(realCodexSessionsRoot)
    // Only asserts presence/count, never logs or inspects real titles —
    // this suite must not surface real session content, only prove the
    // isolation boundary around it.
    expect(realGraph.agents.length).toBeGreaterThan(0)
  })
})
