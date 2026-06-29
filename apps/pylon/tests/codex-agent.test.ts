import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  CODEX_AGENT_CAPABILITY_REF,
  CODEX_AGENT_SDK_PACKAGE,
  codexAgentCredentialSource,
  detectCodexCliLogin,
  loadCodexAgentConfig,
  loadCodexDevConfig,
  probeCodexAgentReadiness,
  withCodexAgentCapability,
} from "../src/codex-agent"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

const sdkPresent = async (specifier: string) => {
  if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
  return {}
}

const sdkAbsent = async () => {
  throw new Error("Cannot find module")
}

describe("codex agent readiness probe", () => {
  test("ready when SDK imports and a Codex API key is present", async () => {
    const probed = await probeCodexAgentReadiness({
      env: { CODEX_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
      codexCliLoginPresent: false,
    })
    expect(probed.state).toBe("ready")
    expect(probed.capabilityRefs).toEqual([CODEX_AGENT_CAPABILITY_REF])
    expect(probed.blockerRefs).toEqual([])
    expect(probed.credentialSourceRef).toBe("credential.source.codex_agent.codex_api_key")
    expect(JSON.stringify(probed)).not.toContain("test-key-shape")
  })

  test("sdk_missing when the optional dependency cannot be imported", async () => {
    const probed = await probeCodexAgentReadiness({
      env: { CODEX_API_KEY: "test-key-shape" },
      platform: "linux",
      importer: sdkAbsent,
      codexCliLoginPresent: false,
    })
    expect(probed.state).toBe("sdk_missing")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.codex_agent.sdk_missing"])
  })

  test("credentials_missing when no BYOK source is configured", async () => {
    const probed = await probeCodexAgentReadiness({
      env: {},
      platform: "darwin",
      importer: sdkPresent,
      codexCliLoginPresent: false,
    })
    expect(probed.state).toBe("credentials_missing")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.codex_agent.credentials_missing"])
    expect(probed.credentialSourceRef).toBeNull()
  })

  test("platform_unsupported outside the supported platform set", async () => {
    const probed = await probeCodexAgentReadiness({
      env: { CODEX_API_KEY: "test-key-shape" },
      platform: "win32",
      importer: sdkPresent,
      codexCliLoginPresent: false,
    })
    expect(probed.state).toBe("platform_unsupported")
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.codex_agent.platform_unsupported"])
  })

  test("disabled_by_config wins even when the device is otherwise ready", async () => {
    const probed = await probeCodexAgentReadiness({
      env: { CODEX_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
      config: { enabled: false },
      codexCliLoginPresent: false,
    })
    expect(probed.state).toBe("disabled_by_config")
    expect(probed.enabled).toBe(false)
    expect(probed.capabilityRefs).toEqual([])
    expect(probed.blockerRefs).toEqual(["blocker.codex_agent.disabled_by_config"])
  })

  test("credential source order: codex key, openai key, cli login, none", () => {
    expect(
      codexAgentCredentialSource({ CODEX_API_KEY: "a", OPENAI_API_KEY: "b" }, true),
    ).toBe("credential.source.codex_agent.codex_api_key")
    expect(codexAgentCredentialSource({ OPENAI_API_KEY: "b" }, true)).toBe(
      "credential.source.codex_agent.openai_api_key",
    )
    expect(codexAgentCredentialSource({}, true)).toBe(
      "credential.source.codex_agent.codex_cli_login",
    )
    expect(codexAgentCredentialSource({ CODEX_API_KEY: "   " }, false)).toBeNull()
  })

  test("cli login detection is by auth-file presence only", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-home-"))
    try {
      expect(await detectCodexCliLogin({ CODEX_HOME: home })).toBe(false)
      await writeFile(join(home, "auth.json"), `${JSON.stringify({ tokens: "never-read" })}\n`)
      expect(await detectCodexCliLogin({ CODEX_HOME: home })).toBe(true)
      const probed = await probeCodexAgentReadiness({
        env: { CODEX_HOME: home },
        platform: "darwin",
        importer: sdkPresent,
      })
      expect(probed.state).toBe("ready")
      expect(probed.credentialSourceRef).toBe("credential.source.codex_agent.codex_cli_login")
      expect(JSON.stringify(probed)).not.toContain("never-read")
      expect(JSON.stringify(probed)).not.toContain(home)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  // #6331: a Codex account connected via `accounts connect codex
  // --openagents-link` writes auth.json into an isolated per-account home, never
  // ~/.codex. The probe must still report ready (and advertise codex capacity)
  // so the heartbeat publishes capacity.coding.codex.* and the dispatch gate
  // admits the work.
  test("ready when a connected per-account codex home carries a login (empty ~/.codex)", async () => {
    const emptyDefaultHome = await mkdtemp(join(tmpdir(), "pylon-codex-default-"))
    const accountHome = await mkdtemp(join(tmpdir(), "pylon-codex-account-"))
    try {
      await writeFile(
        join(accountHome, "auth.json"),
        `${JSON.stringify({ tokens: "never-read-account" })}\n`,
      )
      // The default ~/.codex (via CODEX_HOME) has no login at all.
      expect(await detectCodexCliLogin({ CODEX_HOME: emptyDefaultHome })).toBe(false)
      const probed = await probeCodexAgentReadiness({
        env: { CODEX_HOME: emptyDefaultHome },
        platform: "darwin",
        importer: sdkPresent,
        codexAccountHomes: [accountHome],
      })
      expect(probed.state).toBe("ready")
      expect(probed.capabilityRefs).toEqual([CODEX_AGENT_CAPABILITY_REF])
      expect(probed.credentialSourceRef).toBe(
        "credential.source.codex_agent.pylon_account_login",
      )
      expect(JSON.stringify(probed)).not.toContain("never-read-account")
      expect(JSON.stringify(probed)).not.toContain(accountHome)
    } finally {
      await rm(emptyDefaultHome, { recursive: true, force: true })
      await rm(accountHome, { recursive: true, force: true })
    }
  })

  test("credentials_missing when no account home carries a login", async () => {
    const emptyDefaultHome = await mkdtemp(join(tmpdir(), "pylon-codex-default-"))
    const emptyAccountHome = await mkdtemp(join(tmpdir(), "pylon-codex-account-"))
    try {
      const probed = await probeCodexAgentReadiness({
        env: { CODEX_HOME: emptyDefaultHome },
        platform: "darwin",
        importer: sdkPresent,
        codexAccountHomes: [emptyAccountHome],
      })
      expect(probed.state).toBe("credentials_missing")
      expect(probed.capabilityRefs).toEqual([])
      expect(probed.credentialSourceRef).toBeNull()
    } finally {
      await rm(emptyDefaultHome, { recursive: true, force: true })
      await rm(emptyAccountHome, { recursive: true, force: true })
    }
  })
})

describe("capability declaration", () => {
  test("ready probe adds the capability ref exactly once", async () => {
    const probed = await probeCodexAgentReadiness({
      env: { CODEX_API_KEY: "test-key-shape" },
      platform: "darwin",
      importer: sdkPresent,
      codexCliLoginPresent: false,
    })
    const refs = withCodexAgentCapability(
      ["capability.pylon.assignment_ready", CODEX_AGENT_CAPABILITY_REF],
      probed,
    )
    expect(refs.filter((ref) => ref === CODEX_AGENT_CAPABILITY_REF)).toHaveLength(1)
    expect(refs).toContain("capability.pylon.assignment_ready")
  })

  test("non-ready probe strips a stale capability declaration", async () => {
    const probed = await probeCodexAgentReadiness({
      env: {},
      platform: "darwin",
      importer: sdkAbsent,
      codexCliLoginPresent: false,
    })
    const refs = withCodexAgentCapability(
      ["capability.pylon.assignment_ready", CODEX_AGENT_CAPABILITY_REF],
      probed,
    )
    expect(refs).not.toContain(CODEX_AGENT_CAPABILITY_REF)
    expect(refs).toContain("capability.pylon.assignment_ready")
  })
})

describe("codex config section", () => {
  test("reads bounded fields from the persisted config and ignores junk", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await mkdir(join(home), { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          codex: {
            enabled: false,
            model: "gpt-5.4-codex",
            maxTurns: 12,
            timeoutSeconds: 600,
            sandboxMode: "workspace-write",
            apiKey: "must-be-ignored",
          },
        }),
      )
      const config = await loadCodexAgentConfig(summary)
      expect(config).toEqual({
        enabled: false,
        model: "gpt-5.4-codex",
        maxTurns: 12,
        timeoutSeconds: 600,
        sandboxMode: "workspace-write",
      })
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("assignment sandboxMode rejects danger-full-access", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await writeFile(
        summary.paths.config,
        JSON.stringify({ codex: { sandboxMode: "danger-full-access" } }),
      )
      expect(await loadCodexAgentConfig(summary)).toEqual({})
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("dev config accepts only the local supervised dangerous mode", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-dev-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          dev: {
            codexExecutionMode: "local_supervised_danger",
            defaultAdapter: "claude_agent",
            apiKey: "must-be-ignored",
          },
          codex: { sandboxMode: "danger-full-access" },
        }),
      )
      expect(await loadCodexDevConfig(summary)).toEqual({
        codexExecutionMode: "local_supervised_danger",
        defaultAdapter: "claude_agent",
      })
      expect(JSON.stringify(await loadCodexDevConfig(summary))).not.toContain("must-be-ignored")
      expect(await loadCodexAgentConfig(summary)).toEqual({})
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("missing config file means no overrides", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-agent-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      expect(await loadCodexAgentConfig(summary)).toEqual({})
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})
