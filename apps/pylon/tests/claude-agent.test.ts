import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  CLAUDE_AGENT_CAPABILITY_REF,
  CLAUDE_AGENT_SDK_PACKAGE,
  claudeAgentCredentialSource,
  localClaudeSessionPresent,
  loadClaudeAgentConfig,
  loadClaudeDevConfig,
  probeClaudeAgentReadiness,
  withClaudeAgentCapability,
} from "../src/claude-agent"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

const sdkPresent = async (specifier: string) => {
  if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
  return {}
}

const sdkAbsent = async () => {
  throw new Error("Cannot find module")
}

describe("claude agent readiness probe", () => {
  test("ready when SDK imports and an API key is present", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
    })
    expect(probed.state).toBe("ready")
    expect(probed.capabilityRefs).toEqual([CLAUDE_AGENT_CAPABILITY_REF])
    expect(probed.blockerRefs).toEqual([])
    expect(probed.credentialSourceRef).toBe("credential.source.claude_agent.anthropic_api_key")
    expect(JSON.stringify(probed)).not.toContain("test-key-shape")
  })

  test("sdk_missing when the optional dependency cannot be imported", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "linux",
      importer: sdkAbsent,
    })
    expect(probed.state).toBe("sdk_missing")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.claude_agent.sdk_missing"])
  })

  test("credentials_missing when no BYOK source is configured", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: {},
      platform: "darwin",
      importer: sdkPresent,
      localSessionProbe: async () => false,
    })
    expect(probed.state).toBe("credentials_missing")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.claude_agent.credentials_missing"])
    expect(probed.credentialSourceRef).toBeNull()
  })

  test("ready via the local Claude session when no env source exists", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: {},
      platform: "darwin",
      importer: sdkPresent,
      localSessionProbe: async () => true,
    })
    expect(probed.state).toBe("ready")
    expect(probed.capabilityRefs).toEqual([CLAUDE_AGENT_CAPABILITY_REF])
    expect(probed.blockerRefs).toEqual([])
    expect(probed.credentialSourceRef).toBe(
      "credential.source.claude_agent.local_claude_session",
    )
  })

  test("local session detection honors CLAUDE_CONFIG_DIR without falling back globally", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-config-dir-"))
    try {
      expect(await localClaudeSessionPresent("linux", { CLAUDE_CONFIG_DIR: home })).toBe(false)
      await writeFile(join(home, ".credentials.json"), "{}\n")
      expect(await localClaudeSessionPresent("linux", { CLAUDE_CONFIG_DIR: home })).toBe(true)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("a per-account OAuth token counts as a present session regardless of config dir", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-oauth-token-"))
    try {
      // Empty config dir + no token: not present.
      expect(await localClaudeSessionPresent("darwin", { CLAUDE_CONFIG_DIR: home })).toBe(false)
      // Token present (even with an empty config dir and no .credentials.json): present.
      expect(
        await localClaudeSessionPresent("darwin", {
          CLAUDE_CONFIG_DIR: home,
          CLAUDE_CODE_OAUTH_TOKEN: "sk-ant-oat-test-token-value",
        }),
      ).toBe(true)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("env API key wins over the local session source", async () => {
    let detectorCalled = false
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
      localSessionProbe: async () => {
        detectorCalled = true
        return true
      },
    })
    expect(probed.state).toBe("ready")
    expect(probed.credentialSourceRef).toBe("credential.source.claude_agent.anthropic_api_key")
    expect(detectorCalled).toBe(false)
  })

  test("platform_unsupported outside the supported platform set", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "win32",
      importer: sdkPresent,
    })
    expect(probed.state).toBe("platform_unsupported")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.claude_agent.platform_unsupported"])
  })

  test("disabled_by_config wins even when the device is otherwise ready", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
      config: { enabled: false },
    })
    expect(probed.state).toBe("disabled_by_config")
    expect(probed.enabled).toBe(false)
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.claude_agent.disabled_by_config"])
  })

  test("provider env switches count as credential sources by presence only", () => {
    expect(claudeAgentCredentialSource({ CLAUDE_CODE_USE_BEDROCK: "1" })).toBe(
      "credential.source.claude_agent.amazon_bedrock",
    )
    expect(claudeAgentCredentialSource({ CLAUDE_CODE_USE_VERTEX: "1" })).toBe(
      "credential.source.claude_agent.google_vertex",
    )
    expect(claudeAgentCredentialSource({ CLAUDE_CODE_USE_FOUNDRY: "1" })).toBe(
      "credential.source.claude_agent.azure_foundry",
    )
    expect(
      claudeAgentCredentialSource({
        CLAUDE_CODE_USE_ANTHROPIC_AWS: "1",
        ANTHROPIC_AWS_WORKSPACE_ID: "workspace-shape",
      }),
    ).toBe("credential.source.claude_agent.anthropic_aws")
    expect(claudeAgentCredentialSource({ CLAUDE_CODE_USE_ANTHROPIC_AWS: "1" })).toBeNull()
    expect(claudeAgentCredentialSource({ ANTHROPIC_API_KEY: "   " })).toBeNull()
  })
})

describe("capability declaration", () => {
  test("ready probe adds the capability ref exactly once", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: { ANTHROPIC_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
    })
    const refs = withClaudeAgentCapability(
      ["capability.pylon.assignment_ready", CLAUDE_AGENT_CAPABILITY_REF],
      probed,
    )
    expect(refs.filter((ref) => ref === CLAUDE_AGENT_CAPABILITY_REF)).toHaveLength(1)
    expect(refs).toContain("capability.pylon.assignment_ready")
  })

  test("non-ready probe strips a stale capability declaration", async () => {
    const probed = await probeClaudeAgentReadiness({
      env: {},
      platform: "darwin",
      importer: sdkAbsent,
    })
    const refs = withClaudeAgentCapability(
      ["capability.pylon.assignment_ready", CLAUDE_AGENT_CAPABILITY_REF],
      probed,
    )
    expect(refs).not.toContain(CLAUDE_AGENT_CAPABILITY_REF)
    expect(refs).toContain("capability.pylon.assignment_ready")
  })
})

describe("claudeAgent config section", () => {
  test("reads bounded fields from the persisted config and ignores junk", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await mkdir(join(home), { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          claudeAgent: {
            enabled: false,
            model: "claude-fable-5",
            maxTurns: 12,
            timeoutSeconds: 600,
            apiKey: "must-be-ignored",
          },
        }),
      )
      const config = await loadClaudeAgentConfig(summary)
      expect(config).toEqual({
        enabled: false,
        model: "claude-fable-5",
        maxTurns: 12,
        timeoutSeconds: 600,
      })
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("missing config file means no overrides", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      expect(await loadClaudeAgentConfig(summary)).toEqual({})
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("assignment-safe config rejects permissive mode keys in claudeAgent", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await mkdir(join(home), { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          claudeAgent: {
            model: "claude-fable-5",
            permissionMode: "bypassPermissions",
            claudeExecutionMode: "local_supervised_danger",
            executionMode: "local_supervised_danger",
          },
        }),
      )
      expect(await loadClaudeAgentConfig(summary)).toEqual({ model: "claude-fable-5" })
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})

describe("local-only dev config section", () => {
  test("reads dev.claudeExecutionMode opt-in and ignores other values", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-claude-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await mkdir(join(home), { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({ dev: { claudeExecutionMode: "local_supervised_danger" } }),
      )
      expect(await loadClaudeDevConfig(summary)).toEqual({
        claudeExecutionMode: "local_supervised_danger",
      })
      await writeFile(
        summary.paths.config,
        JSON.stringify({ dev: { claudeExecutionMode: "anything_else" } }),
      )
      expect(await loadClaudeDevConfig(summary)).toEqual({})
      await writeFile(
        summary.paths.config,
        JSON.stringify({ claudeAgent: { claudeExecutionMode: "local_supervised_danger" } }),
      )
      expect(await loadClaudeDevConfig(summary)).toEqual({})
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})
