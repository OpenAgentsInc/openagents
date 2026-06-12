import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { collectPylonDevDoctor } from "../src/dev-doctor"
import { projectHostInventoryFixture } from "../src/inventory"
import { assertPublicProjectionSafe } from "../src/state"

const sdkPresent = (expected: string) => async (specifier: string) => {
  if (specifier !== expected) throw new Error(`unexpected import: ${specifier}`)
  return {}
}

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
  const root = await mkdtemp(join(tmpdir(), "pylon-dev-doctor-"))
  const workspace = join(root, "workspace")
  const repo = join(workspace, "repo")
  await mkdir(repo, { recursive: true })
  await writeFile(join(workspace, "AGENTS.md"), "# workspace instructions\n")
  await writeFile(join(workspace, "INVARIANTS.md"), "# workspace invariants\n")
  await writeFile(join(repo, "AGENTS.md"), "# repo instructions\n")
  await writeFile(join(repo, "INVARIANTS.md"), "# repo invariants\n")
  await writeFile(join(repo, "README.md"), "fixture\n")
  await run(["git", "init"], repo)
  await run(["git", "config", "user.email", "dev-doctor@example.test"], repo)
  await run(["git", "config", "user.name", "Dev Doctor"], repo)
  await run(["git", "add", "."], repo)
  await run(["git", "commit", "-m", "initial"], repo)
  await run(["git", "branch", "-M", "main"], repo)
  await run(["git", "remote", "add", "origin", "git@github.com:OpenAgentsInc/dev-doctor-fixture.git"], repo)
  return { repo, root }
}

function inventoryFixture() {
  return projectHostInventoryFixture({
    platform: "darwin",
    arch: "arm64",
    cpuCores: 12,
    cpuModel: "Apple M3 Max",
    totalMemoryBytes: 36 * 1024 * 1024 * 1024,
    freeMemoryBytes: 20 * 1024 * 1024 * 1024,
    homeFreeBytes: 100 * 1024 * 1024 * 1024,
    networkInterfaceCount: 4,
    externalNetworkInterfaceCount: 1,
    opencodeInstalled: false,
    geminiConfigured: false,
    appleFmReady: true,
    now: "2026-06-12T12:00:00.000Z",
  })
}

async function writeConfig(home: string, config: Record<string, unknown>) {
  await mkdir(home, { recursive: true })
  await writeFile(join(home, "config.json"), `${JSON.stringify(config, null, 2)}\n`)
}

describe("pylon dev doctor projection", () => {
  test("projects repo, instructions, Codex, Fable, and execution mode without local paths", async () => {
    const { repo, root } = await createRepoFixture()
    const home = join(root, "pylon-home")
    try {
      await writeConfig(home, {
        codex: { enabled: true, model: "gpt-5-codex", sandboxMode: "workspace-write" },
        dev: { codexExecutionMode: "local_supervised_danger" },
        claudeAgent: { enabled: true, model: "claude-fable-5" },
      })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonDevDoctor({
        claudeImporter: sdkPresent(CLAUDE_AGENT_SDK_PACKAGE),
        codexCliLoginPresent: false,
        codexCliPath: "/usr/local/bin/codex",
        codexImporter: sdkPresent(CODEX_AGENT_SDK_PACKAGE),
        cwd: repo,
        env: {
          ANTHROPIC_API_KEY: "anthropic-test-key",
          CODEX_API_KEY: "codex-test-key",
          PYLON_HOME: home,
        },
        inventory: inventoryFixture(),
        localClaudeSessionProbe: async () => false,
        now: new Date("2026-06-12T12:00:00.000Z"),
        summary,
      })

      expect(projection.schema).toBe("openagents.pylon.dev_doctor.v0.3")
      expect(projection.repo).toMatchObject({
        provider: "github",
        fullName: "OpenAgentsInc/dev-doctor-fixture",
        branch: "main",
        dirty: { state: "clean", changedCount: 0 },
      })
      expect(projection.instructions.refs.every((ref) => ref.state === "present")).toBe(true)
      expect(projection.codex).toMatchObject({
        cli: "present",
        configuredModel: "gpt-5-codex",
        executionMode: "local_supervised_danger",
        sandboxMode: "danger-full-access",
      })
      expect(projection.codex.credentialSourceRef).toBe("credential.source.codex_agent.codex_api_key")
      expect(projection.claudeAgent.configuredModel).toBe("claude-fable-5")
      expect(projection.claudeAgent.fableReviewAvailable).toBe(true)
      expect(projection.claudeAgent).toMatchObject({
        executionMode: "local_bounded",
        permissionMode: "acceptEdits",
        dangerPublicPathBlockerRef: null,
      })
      expect(projection.pylonConfig.devOverlayRef).toBe("config.pylon.dev.local_supervised_danger")
      expect(projection.pylonConfig.claudeDevOverlayRef).toBeNull()
      expect(projection.pylonConfig.defaultAdapter).toBe("codex")
      expect(JSON.stringify(projection)).not.toContain(root)
      expect(JSON.stringify(projection)).not.toContain("codex-test-key")
      expect(JSON.stringify(projection)).not.toContain("anthropic-test-key")
      assertPublicProjectionSafe(projection)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("reports missing auth and missing Codex CLI as explicit blockers", async () => {
    const { repo, root } = await createRepoFixture()
    const home = join(root, "pylon-home")
    try {
      await writeConfig(home, { codex: { enabled: true } })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonDevDoctor({
        claudeImporter: sdkPresent(CLAUDE_AGENT_SDK_PACKAGE),
        codexCliLoginPresent: false,
        codexCliPath: null,
        codexImporter: sdkPresent(CODEX_AGENT_SDK_PACKAGE),
        cwd: repo,
        env: { PYLON_HOME: home },
        inventory: inventoryFixture(),
        localClaudeSessionProbe: async () => false,
        summary,
      })

      expect(projection.codex.cli).toBe("missing")
      expect(projection.codex.sdkReadiness.state).toBe("credentials_missing")
      expect(projection.codex.blockerRefs).toContain("blocker.dev_doctor.codex_cli_missing")
      expect(projection.blockerRefs).toContain("blocker.codex_agent.credentials_missing")
      expect(projection.claudeAgent.readiness.state).toBe("credentials_missing")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("reports dirty repository state without leaking changed filenames", async () => {
    const { repo, root } = await createRepoFixture()
    const home = join(root, "pylon-home")
    try {
      await writeConfig(home, { codex: { enabled: true } })
      await writeFile(join(repo, "secret-local-file.txt"), "changed\n")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonDevDoctor({
        claudeImporter: sdkPresent(CLAUDE_AGENT_SDK_PACKAGE),
        codexCliLoginPresent: false,
        codexCliPath: "/usr/local/bin/codex",
        codexImporter: sdkPresent(CODEX_AGENT_SDK_PACKAGE),
        cwd: repo,
        env: { CODEX_API_KEY: "codex-test-key", PYLON_HOME: home },
        inventory: inventoryFixture(),
        localClaudeSessionProbe: async () => false,
        summary,
      })

      expect(projection.repo.dirty.state).toBe("dirty")
      expect(projection.repo.dirty.changedCount).toBeGreaterThan(0)
      expect(projection.repo.blockerRefs).toContain("blocker.dev_doctor.repo_dirty")
      expect(JSON.stringify(projection)).not.toContain("secret-local-file")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("projects the Claude supervised danger mode from dev config or flag", async () => {
    const { repo, root } = await createRepoFixture()
    const home = join(root, "pylon-home")
    try {
      await writeConfig(home, {
        claudeAgent: { enabled: true, model: "claude-fable-5" },
        dev: { claudeExecutionMode: "local_supervised_danger" },
      })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const baseOptions = {
        claudeImporter: sdkPresent(CLAUDE_AGENT_SDK_PACKAGE),
        codexCliLoginPresent: false,
        codexCliPath: "/usr/local/bin/codex",
        codexImporter: sdkPresent(CODEX_AGENT_SDK_PACKAGE),
        cwd: repo,
        env: { ANTHROPIC_API_KEY: "anthropic-test-key", PYLON_HOME: home },
        inventory: inventoryFixture(),
        localClaudeSessionProbe: async () => false,
        summary,
      }
      const fromConfig = await collectPylonDevDoctor(baseOptions)
      expect(fromConfig.claudeAgent).toMatchObject({
        executionMode: "local_supervised_danger",
        permissionMode: "bypassPermissions",
        dangerPublicPathBlockerRef: "blocker.claude.local_supervised_danger_public_path",
      })
      expect(fromConfig.pylonConfig.claudeDevOverlayRef).toBe(
        "config.pylon.dev.claude_local_supervised_danger",
      )
      // The Claude overlay must not claim the Codex mode and vice versa.
      expect(fromConfig.codex.executionMode).toBe("local_bounded")
      expect(fromConfig.pylonConfig.devOverlayRef).toBeNull()

      await writeConfig(home, { claudeAgent: { enabled: true, model: "claude-fable-5" } })
      const fromFlag = await collectPylonDevDoctor({ ...baseOptions, claudeDangerFlag: true })
      expect(fromFlag.claudeAgent.executionMode).toBe("local_supervised_danger")
      expect(fromFlag.claudeAgent.permissionMode).toBe("bypassPermissions")
      expect(fromFlag.pylonConfig.claudeDevOverlayRef).toBeNull()
      expect(JSON.stringify(fromFlag)).not.toContain("anthropic-test-key")
      assertPublicProjectionSafe(fromFlag)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("unknown repo and missing instruction files produce blockers", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-dev-doctor-nongit-"))
    const home = join(root, "pylon-home")
    try {
      await mkdir(root, { recursive: true })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonDevDoctor({
        claudeImporter: sdkPresent(CLAUDE_AGENT_SDK_PACKAGE),
        codexCliLoginPresent: false,
        codexCliPath: null,
        codexImporter: sdkPresent(CODEX_AGENT_SDK_PACKAGE),
        cwd: root,
        env: { PYLON_HOME: home },
        inventory: inventoryFixture(),
        localClaudeSessionProbe: async () => false,
        summary,
      })

      expect(projection.repo.state).toBe("not_git")
      expect(projection.blockerRefs).toContain("blocker.dev_doctor.repo_unknown")
      expect(projection.instructions.refs.every((ref) => ref.state === "missing")).toBe(true)
      expect(projection.instructions.blockerRefs).toContain("blocker.dev_doctor.repo_agents_missing")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
