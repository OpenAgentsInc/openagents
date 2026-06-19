// #5471 (EPIC #5461): repo / worktree picker reducer + helper tests.
//
// The picker gives the composer two workspace modes on the EXISTING control
// protocol (session.spawn): an existing worktree path (`worktreePath`) and a
// Pylon-managed worktree (`repoRef`, resolved node-side via git ls-remote).
// These tests drive the pure helpers (composer-workspace.ts) and the pure
// reducer (update.ts) through the whole managed flow — switch mode → validate →
// resolve → spawn — plus the error branches, without a DOM, a runtime, or git.

import { describe, expect, test } from "bun:test"

import {
  DEFAULT_MANAGED_BASE_REF,
  managedWorktreeLabel,
  normalizeGitHubFullName,
  parseManagedWorktreeRequest,
  worktreePathLabel,
} from "../src/ui/composer-workspace"
import { initialModel, Model } from "../src/ui/model"
import {
  ChangedComposerManagedBaseRef,
  ChangedComposerManagedRepo,
  ChangedComposerWorkspaceMode,
  ClickedComposerReply,
  ClickedComposerSpawn,
  ResolvedComposerManagedWorktree,
} from "../src/ui/message"
import { update } from "../src/ui/update"

const COMMIT = "a".repeat(40)

describe("composer-workspace helpers (#5471)", () => {
  test("normalizeGitHubFullName: accepts owner/name, URL, and SSH; rejects junk", () => {
    expect(normalizeGitHubFullName("OpenAgentsInc/openagents")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(normalizeGitHubFullName("https://github.com/OpenAgentsInc/openagents")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(normalizeGitHubFullName("https://github.com/OpenAgentsInc/openagents.git")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(normalizeGitHubFullName("git@github.com:OpenAgentsInc/openagents.git")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(normalizeGitHubFullName("OpenAgentsInc/openagents/")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(normalizeGitHubFullName("")).toBeNull()
    expect(normalizeGitHubFullName("not a repo")).toBeNull()
    expect(normalizeGitHubFullName("https://gitlab.com/a/b")).toBeNull()
    expect(normalizeGitHubFullName("owneronly")).toBeNull()
  })

  test("parseManagedWorktreeRequest: defaults the base ref and strips origin/ for branch", () => {
    const parsed = parseManagedWorktreeRequest({ repo: "OpenAgentsInc/openagents", baseRef: "" })
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) throw new Error("expected ok")
    expect(parsed.request.fullName).toBe("OpenAgentsInc/openagents")
    expect(parsed.request.baseRef).toBe(DEFAULT_MANAGED_BASE_REF)
    expect(parsed.request.branch).toBe("main")
  })

  test("parseManagedWorktreeRequest: keeps a custom base ref and derives its branch", () => {
    const parsed = parseManagedWorktreeRequest({
      repo: "OpenAgentsInc/openagents",
      baseRef: "origin/feature-x",
    })
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) throw new Error("expected ok")
    expect(parsed.request.baseRef).toBe("origin/feature-x")
    expect(parsed.request.branch).toBe("feature-x")
  })

  test("parseManagedWorktreeRequest: rejects a bad repo and a dangerous ref", () => {
    expect(parseManagedWorktreeRequest({ repo: "nope", baseRef: "main" }).ok).toBe(false)
    expect(
      parseManagedWorktreeRequest({ repo: "a/b", baseRef: "../escape" }).ok,
    ).toBe(false)
    expect(parseManagedWorktreeRequest({ repo: "a/b", baseRef: "-x" }).ok).toBe(false)
  })

  test("labels: managed shows repo @ ref; worktree shows path or default", () => {
    const parsed = parseManagedWorktreeRequest({ repo: "a/b", baseRef: "origin/main" })
    if (!parsed.ok) throw new Error("expected ok")
    expect(managedWorktreeLabel(parsed.request)).toBe("a/b @ origin/main")
    expect(worktreePathLabel("/Users/me/code/x")).toBe("/Users/me/code/x")
    expect(worktreePathLabel("   ")).toBe("node default worktree")
  })
})

describe("composer-workspace reducer (#5471)", () => {
  test("ChangedComposerWorkspaceMode toggles the picker mode", () => {
    const [model] = update(
      Model.make({ ...initialModel }),
      ChangedComposerWorkspaceMode({ mode: "managed" }),
    )
    expect(model.composerWorkspaceMode).toBe("managed")
  })

  test("ChangedComposerManagedRepo / BaseRef record the inputs", () => {
    const [a] = update(
      Model.make({ ...initialModel }),
      ChangedComposerManagedRepo({ value: "OpenAgentsInc/openagents" }),
    )
    expect(a.composerManagedRepo).toBe("OpenAgentsInc/openagents")
    const [b] = update(a, ChangedComposerManagedBaseRef({ value: "origin/main" }))
    expect(b.composerManagedBaseRef).toBe("origin/main")
  })

  test("worktree mode still spawns directly with worktreePath (no resolve step)", () => {
    const start = Model.make({
      ...initialModel,
      spawnObjective: "do the thing",
      composerWorkspaceMode: "worktree",
      composerRepoPath: "/Users/me/code/repo",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)
    expect(commands).toHaveLength(1)
    const cmd = commands[0] as unknown as {
      args?: { worktreePath?: string | null; repoRef?: unknown }
    }
    // It is a SpawnComposerTurn (worktreePath set, repoRef null) — not a resolve.
    expect(cmd.args?.worktreePath).toBe("/Users/me/code/repo")
    expect(cmd.args?.repoRef).toBeNull()
  })

  test("managed mode validates the repo before resolving", () => {
    const start = Model.make({
      ...initialModel,
      spawnObjective: "do the thing",
      composerWorkspaceMode: "managed",
      composerManagedRepo: "not a repo",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
    expect(model.composerPending).toBe(false)
  })

  test("managed mode fires a resolve step that defers the spawn", () => {
    const start = Model.make({
      ...initialModel,
      spawnObjective: "do the thing",
      composerWorkspaceMode: "managed",
      composerManagedRepo: "OpenAgentsInc/openagents",
      composerManagedBaseRef: "origin/main",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)
    // The full objective is stashed so the resolve→spawn handoff can fire it.
    expect(model.composerPendingObjective).toBe("do the thing")
    expect(model.composerTurns).toEqual(["do the thing"])
    expect(commands).toHaveLength(1)
    // The command is ResolveManagedWorktree (repo + base ref + stripped branch).
    const cmd = commands[0] as unknown as {
      args?: { fullName?: string; baseRef?: string; branch?: string }
    }
    expect(cmd.args?.fullName).toBe("OpenAgentsInc/openagents")
    expect(cmd.args?.baseRef).toBe("origin/main")
    expect(cmd.args?.branch).toBe("main")
  })

  test("ResolvedComposerManagedWorktree(ok) fires the deferred spawn with the repoRef", () => {
    const waiting = Model.make({
      ...initialModel,
      spawnObjective: "",
      composerWorkspaceMode: "managed",
      composerManagedRepo: "OpenAgentsInc/openagents",
      composerPending: true,
      composerPendingObjective: "do the thing",
      composerTurns: ["do the thing"],
    })
    const [model, commands] = update(
      waiting,
      ResolvedComposerManagedWorktree({
        result: {
          ok: true,
          repoRef: {
            provider: "github",
            visibility: "public",
            fullName: "OpenAgentsInc/openagents",
            branch: "main",
            commitSha: COMMIT,
          },
        },
      }),
    )
    expect(model.composerPendingObjective).toBeNull()
    expect(commands).toHaveLength(1)
    const cmd = commands[0] as unknown as {
      args?: {
        objective?: string
        worktreePath?: string | null
        repoRef?: { commitSha?: string; fullName?: string } | null
      }
    }
    expect(cmd.args?.objective).toBe("do the thing")
    expect(cmd.args?.worktreePath).toBeNull()
    expect(cmd.args?.repoRef?.commitSha).toBe(COMMIT)
    expect(cmd.args?.repoRef?.fullName).toBe("OpenAgentsInc/openagents")
  })

  test("ResolvedComposerManagedWorktree(fail) settles the loop with the error", () => {
    const waiting = Model.make({
      ...initialModel,
      composerPending: true,
      composerPendingObjective: "do the thing",
    })
    const [model, commands] = update(
      waiting,
      ResolvedComposerManagedWorktree({
        result: { ok: false, error: "base ref 'origin/nope' did not resolve to a commit" },
      }),
    )
    expect(model.composerPending).toBe(false)
    expect(model.composerPendingObjective).toBeNull()
    expect(model.composerStatus.tone).toBe("error")
    expect(model.composerStatus.text).toContain("did not resolve")
    expect(commands).toHaveLength(0)
  })

  test("managed reply turn also resolves first then spawns the continuation", () => {
    const active = Model.make({
      ...initialModel,
      composerSessionRef: "session.pylon.codex.abc",
      composerWorkspaceMode: "managed",
      composerManagedRepo: "OpenAgentsInc/openagents",
      composerManagedBaseRef: "origin/main",
      composerReply: "now add a test",
      composerTurns: ["first turn"],
    })
    const [model, commands] = update(active, ClickedComposerReply())
    expect(model.composerPending).toBe(true)
    expect(model.composerPendingObjective).toContain("now add a test")
    expect(model.composerTurns).toEqual(["first turn", "now add a test"])
    expect(commands).toHaveLength(1)
    const cmd = commands[0] as unknown as { args?: { fullName?: string } }
    expect(cmd.args?.fullName).toBe("OpenAgentsInc/openagents")
  })
})
