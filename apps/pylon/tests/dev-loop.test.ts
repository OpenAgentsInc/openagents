import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  collectPylonDevChangeSummary,
  runPylonDevCheck,
  runPylonDevReload,
  type PylonDevCommandResult,
  type PylonDevCommandSpec,
} from "../src/dev-loop"
import { assertPublicProjectionSafe } from "../src/state"

async function run(args: string[], cwd: string) {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new Error(`${args.join(" ")} failed: ${stderr || stdout}`)
  return stdout.trim()
}

async function createRepoFixture() {
  const root = await mkdtemp(join(tmpdir(), "pylon-dev-loop-"))
  const repo = join(root, "repo")
  await mkdir(join(repo, "apps/pylon/src"), { recursive: true })
  await writeFile(join(repo, "apps/pylon/src/dev-loop.ts"), "export const value = 1\n")
  await run(["git", "init"], repo)
  await run(["git", "config", "user.email", "dev-loop@example.test"], repo)
  await run(["git", "config", "user.name", "Dev Loop"], repo)
  await run(["git", "add", "."], repo)
  await run(["git", "commit", "-m", "initial"], repo)
  await run(["git", "branch", "-M", "main"], repo)
  const home = join(root, "pylon-home")
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
  return { home, repo, root, summary }
}

function commandResult(command: PylonDevCommandSpec, status: "passed" | "failed", exitCode: number): PylonDevCommandResult {
  return {
    argvRef: "command.argv.test",
    commandRef: "command.dev_check.test",
    cwdRef: "command.cwd.test",
    durationMs: 12,
    exitCode,
    reasonRef: command.reasonRef,
    status,
    stderrBytes: 0,
    stderrDigestRef: null,
    stdoutBytes: 8,
    stdoutDigestRef: "command.stdout.test",
  }
}

describe("pylon dev loop", () => {
  test("dev check returns typed refs for a successful focused command", async () => {
    const { repo, root, summary } = await createRepoFixture()
    try {
      await writeFile(join(repo, "apps/pylon/src/dev-loop.ts"), "export const value = 2\n")
      const projection = await runPylonDevCheck({
        allowDirty: true,
        commands: [{ argv: ["bun", "--version"], cwd: repo, reasonRef: "check.test" }],
        commandRunner: async (command) => commandResult(command, "passed", 0),
        cwd: repo,
        now: new Date("2026-06-12T12:00:00.000Z"),
        persist: false,
        summary,
      })

      expect(projection.schema).toBe("openagents.pylon.dev_check.v0.3")
      expect(projection.state).toBe("passed")
      expect(projection.changeSummary.dirty.state).toBe("dirty")
      expect(projection.changeSummary.changedFileRefs[0]?.area).toBe("pylon.dev")
      expect(projection.commandResults).toHaveLength(1)
      expect(projection.commandResults[0]?.exitCode).toBe(0)
      expect(projection.branchUntouched).toBe(true)
      expect(projection.commitUntouched).toBe(true)
      expect(projection.pushPerformed).toBe(false)
      expect(JSON.stringify(projection)).not.toContain(root)
      assertPublicProjectionSafe(projection)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("dev check returns failed when a focused command fails", async () => {
    const { repo, root, summary } = await createRepoFixture()
    try {
      await writeFile(join(repo, "apps/pylon/src/dev-loop.ts"), "export const value = 3\n")
      const projection = await runPylonDevCheck({
        allowDirty: true,
        commands: [{ argv: ["bun", "test"], cwd: repo, reasonRef: "check.test" }],
        commandRunner: async (command) => commandResult(command, "failed", 7),
        cwd: repo,
        persist: false,
        summary,
      })

      expect(projection.state).toBe("failed")
      expect(projection.commandResults[0]?.status).toBe("failed")
      expect(projection.commandResults[0]?.exitCode).toBe(7)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("dev check blocks untracked dirty pre-state unless explicitly allowed", async () => {
    const { repo, root, summary } = await createRepoFixture()
    try {
      await writeFile(join(repo, "untracked-secret-filename.ts"), "secret local change\n")
      let ran = false
      const projection = await runPylonDevCheck({
        commands: [{ argv: ["bun", "--version"], cwd: repo, reasonRef: "check.test" }],
        commandRunner: async (command) => {
          ran = true
          return commandResult(command, "passed", 0)
        },
        cwd: repo,
        persist: false,
        summary,
      })

      expect(projection.state).toBe("blocked")
      expect(projection.blockerRefs).toContain("blocker.dev_check.dirty_prestate_requires_allow_dirty")
      expect(projection.commandResults).toHaveLength(0)
      expect(ran).toBe(false)
      expect(JSON.stringify(projection)).not.toContain("untracked-secret-filename")
      expect(JSON.stringify(projection)).not.toContain(root)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("dev check blocks a detached HEAD by default but runs when allowDetached is set", async () => {
    const { repo, root, summary } = await createRepoFixture()
    try {
      // Detach HEAD the way a worktree materialized from a pinned commit is.
      const head = await run(["git", "rev-parse", "HEAD"], repo)
      await run(["git", "checkout", "--detach", head], repo)

      let blockedRan = false
      const blocked = await runPylonDevCheck({
        allowDirty: true,
        commands: [{ argv: ["bun", "--version"], cwd: repo, reasonRef: "check.test" }],
        commandRunner: async (command) => {
          blockedRan = true
          return commandResult(command, "passed", 0)
        },
        cwd: repo,
        persist: false,
        summary,
      })
      expect(blocked.state).toBe("blocked")
      expect(blocked.blockerRefs).toContain("blocker.dev_loop.branch_unknown_or_detached")
      expect(blocked.commandResults).toHaveLength(0)
      expect(blockedRan).toBe(false)

      let allowedRan = false
      const allowed = await runPylonDevCheck({
        allowDirty: true,
        allowDetached: true,
        commands: [{ argv: ["bun", "--version"], cwd: repo, reasonRef: "check.test" }],
        commandRunner: async (command) => {
          allowedRan = true
          return commandResult(command, "passed", 0)
        },
        cwd: repo,
        persist: false,
        summary,
      })
      expect(allowed.state).toBe("passed")
      expect(allowedRan).toBe(true)
      expect(allowed.commandResults).toHaveLength(1)
      // The detached state stays honestly visible in the change summary even
      // though it no longer gates command execution.
      expect(allowed.changeSummary.repo.branch).toBeNull()
      expect(allowed.changeSummary.blockerRefs).toContain("blocker.dev_loop.branch_unknown_or_detached")
      assertPublicProjectionSafe(allowed)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("dev reload is explicit and no-op when no controlled process exists", async () => {
    const { repo, root, summary } = await createRepoFixture()
    try {
      const projection = await runPylonDevReload({
        cwd: repo,
        now: new Date("2026-06-12T12:00:00.000Z"),
        persist: false,
        summary,
      })

      expect(projection.schema).toBe("openagents.pylon.dev_reload.v0.3")
      expect(projection.state).toBe("noop")
      expect(projection.reasonRef).toBe("dev_reload.no_controlled_process")
      expect(projection.branchUntouched).toBe(true)
      expect(projection.commitUntouched).toBe(true)
      expect(projection.pushPerformed).toBe(false)
      expect(projection.destructiveGitPerformed).toBe(false)
      assertPublicProjectionSafe(projection)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("change summary does not expose local changed filenames", async () => {
    const { repo, root } = await createRepoFixture()
    try {
      await writeFile(join(repo, "apps/pylon/src/dev-loop.ts"), "export const value = 4\n")
      const summary = await collectPylonDevChangeSummary({ cwd: repo })
      expect(summary.changedFileRefs[0]?.fileRef.startsWith("file.local_change.")).toBe(true)
      expect(JSON.stringify(summary)).not.toContain("dev-loop.ts")
      expect(JSON.stringify(summary)).not.toContain(root)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
