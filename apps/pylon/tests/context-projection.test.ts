import { describe, expect, test } from "bun:test"

import {
  PYLON_CONTEXT_SCHEMA,
  contextProjectionFromDevDoctor,
  emptyPylonContextProjection,
} from "../src/context-projection"
import type { PylonDevDoctorProjection } from "../src/dev-doctor"

function devDoctorFixture(): PylonDevDoctorProjection {
  return {
    schema: "openagents.pylon.dev_doctor.v0.3",
    observedAt: "2026-06-12T12:00:00.000Z",
    package: {
      name: "@openagentsinc/pylon",
      version: "1.0.0-rc.2",
      sourceCommit: "f".repeat(40),
    },
    repo: {
      state: "ready",
      provider: "github",
      fullName: "OpenAgentsInc/openagents",
      branch: "main",
      commit: "a".repeat(40),
      dirty: { state: "clean", changedCount: 0 },
      blockerRefs: [],
    },
    instructions: {
      refs: [
        {
          sourceRef: "instruction.workspace.agents",
          state: "present",
          relativePath: "AGENTS.md",
          digestRef: "file.digest.workspace",
        },
        {
          sourceRef: "instruction.repo.invariants",
          state: "present",
          relativePath: "openagents/INVARIANTS.md",
          digestRef: "file.digest.repo",
        },
      ],
      blockerRefs: [],
    },
    pylonConfig: {
      state: "present",
      configRef: "config.pylon.local",
      digestRef: "file.digest.config",
      devOverlayRef: "config.pylon.dev.local_supervised_danger",
      claudeDevOverlayRef: null,
      defaultAdapter: "codex",
    },
    codex: {
      cli: "present",
      sdkReadiness: {
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "ready",
        enabled: true,
        capabilityRefs: ["capability.pylon.local_codex"],
        blockerRefs: [],
        credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
      },
      credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
      configuredModel: "gpt-5-codex",
      executionMode: "local_supervised_danger",
      sandboxMode: "danger-full-access",
      blockerRefs: [],
    },
    claudeAgent: {
      readiness: {
        schema: "openagents.pylon.claude_agent_readiness.v0.3",
        state: "ready",
        enabled: true,
        capabilityRefs: ["capability.pylon.local_claude_agent"],
        blockerRefs: [],
        credentialSourceRef: "credential.source.claude_agent.local_claude_session",
      },
      configuredModel: "claude-fable-5",
      fableReviewAvailable: true,
      executionMode: "local_bounded",
      permissionMode: "acceptEdits",
      dangerPublicPathBlockerRef: null,
      blockerRefs: [],
    },
    backends: {
      refs: [
        { backendRef: "backend.opencode.cli", state: "ready", modelRef: "model.opencode.default", blockerRefs: [] },
        { backendRef: "backend.apple_fm", state: "ready", modelRef: "model.apple_foundation_model", blockerRefs: [] },
        { backendRef: "backend.gemini", state: "missing", modelRef: null, blockerRefs: ["blocker.backend.gemini_auth_missing"] },
        { backendRef: "backend.psionic.qwen35", state: "missing", modelRef: null, blockerRefs: ["blocker.psionic_qwen35.connector_unconfigured"] },
      ],
      blockerRefs: ["blocker.backend.gemini_auth_missing"],
    },
    blockerRefs: ["blocker.backend.gemini_auth_missing"],
  }
}

describe("pylon context projection", () => {
  test("maps dev doctor into repo, instruction, adapter, and job context", () => {
    const projection = contextProjectionFromDevDoctor(devDoctorFixture())

    expect(projection.schema).toBe(PYLON_CONTEXT_SCHEMA)
    expect(projection.repo).toMatchObject({
      fullName: "OpenAgentsInc/openagents",
      branch: "main",
      commitRef: "commit.aaaaaaaaaaaa",
      dirtyState: "clean",
    })
    expect(projection.instructions.refs.map((ref) => ref.sourceRef)).toContain("instruction.workspace.agents")
    expect(projection.instructions.configRefs).toContain("config.pylon.dev.local_supervised_danger")
    expect(projection.instructions.configRefs).toContain("config.pylon.dev.default_adapter.codex")
    expect(projection.adapters.codex).toMatchObject({
      state: "ready",
      danger: true,
      sandboxMode: "danger-full-access",
      modelRef: "model.codex.gpt-5-codex",
    })
    expect(projection.adapters.openai.sourceRefs).toEqual(["credential.source.codex_agent.codex_cli_login"])
    expect(projection.adapters.claudeAgent).toMatchObject({
      fableReviewAvailable: true,
      executionMode: "local_bounded",
      permissionMode: "acceptEdits",
      danger: false,
    })
    expect(projection.adapters.primaryAdapter).toBe("codex")
    expect(projection.adapters.reviewerAdapter).toBe("fable")
    expect(projection.currentJob.requiredCapabilityRefs).toEqual([
      "capability.pylon.local_codex",
      "capability.pylon.local_claude_agent",
    ])
  })

  test("default projection is public-safe and unknown", () => {
    const projection = emptyPylonContextProjection()
    expect(projection.repo.state).toBe("unknown")
    expect(projection.adapters.primaryAdapter).toBe("unknown")
    expect(projection.blockerRefs).toContain("blocker.context.repo_unknown")
  })

  test("honors Claude as the configured primary adapter", () => {
    const fixture = devDoctorFixture()
    fixture.pylonConfig.defaultAdapter = "claude_agent"
    const projection = contextProjectionFromDevDoctor(fixture)
    expect(projection.adapters.primaryAdapter).toBe("claude_agent")
    expect(projection.adapters.reviewerAdapter).toBe("codex")
    expect(projection.currentJob.requiredCapabilityRefs).toEqual([
      "capability.pylon.local_codex",
      "capability.pylon.local_claude_agent",
    ])
  })


  test("projects the Claude supervised danger mode with dev-mode flag", () => {
    const fixture = devDoctorFixture()
    fixture.codex.executionMode = "local_bounded"
    fixture.codex.sandboxMode = "workspace-write"
    fixture.pylonConfig.devOverlayRef = null
    fixture.pylonConfig.claudeDevOverlayRef = "config.pylon.dev.claude_local_supervised_danger"
    fixture.claudeAgent.executionMode = "local_supervised_danger"
    fixture.claudeAgent.permissionMode = "bypassPermissions"
    fixture.claudeAgent.dangerPublicPathBlockerRef =
      "blocker.claude.local_supervised_danger_public_path"
    const projection = contextProjectionFromDevDoctor(fixture)
    expect(projection.adapters.mode).toBe("dev")
    expect(projection.adapters.claudeAgent).toMatchObject({
      executionMode: "local_supervised_danger",
      permissionMode: "bypassPermissions",
      danger: true,
    })
    expect(projection.adapters.codex.danger).toBe(false)
    expect(projection.instructions.configRefs).toContain(
      "config.pylon.dev.claude_local_supervised_danger",
    )
  })

  test("rejects emails and local auth paths in status output", () => {
    const fixture = devDoctorFixture()
    fixture.codex.credentialSourceRef = "/Users/operator/.codex/auth.json"
    fixture.codex.sdkReadiness.credentialSourceRef = "operator@example.com"
    expect(() => contextProjectionFromDevDoctor(fixture)).toThrow(/private context text/)
  })
})
