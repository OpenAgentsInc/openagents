import { describe, expect, test } from "bun:test"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"

describe("Codex settings projection", () => {
  test("projects Codex model, config, requirements, usage, and collaboration state", () => {
    const projection = projectKhalaCodeDesktopCodexSettings({
      cwd: "/repo",
      observedAt: "2026-07-01T18:00:00.000Z",
      configRead: {
        config: {
          model: "gpt-5.1-codex",
          model_provider: "openai",
          model_reasoning_effort: "high",
          model_reasoning_summary: "auto",
          model_verbosity: "medium",
          service_tier: "priority",
          approval_policy: "on-request",
          approvals_reviewer: "user",
          sandbox_mode: "workspace-write",
          default_permissions: ":workspace",
          personality: "pragmatic",
        },
        origins: {
          model: { source: "user" },
          sandbox_mode: { source: "system" },
        },
        layers: [],
      },
      modelList: {
        data: [
          {
            id: "gpt-5.1-codex",
            model: "gpt-5.1-codex",
            displayName: "GPT-5.1 Codex",
            description: "Codex default",
            hidden: false,
            isDefault: true,
            supportsPersonality: true,
            defaultReasoningEffort: "medium",
            supportedReasoningEfforts: [
              { reasoningEffort: "medium", description: "balanced" },
              { reasoningEffort: "high", description: "deeper" },
            ],
            serviceTiers: [
              { id: "priority", name: "Priority", description: "faster" },
            ],
            defaultServiceTier: "priority",
          },
        ],
      },
      providerCapabilities: {
        namespaceTools: true,
        imageGeneration: false,
        webSearch: true,
      },
      permissionProfileList: {
        data: [
          { id: ":workspace", description: "Workspace", allowed: true },
          { id: ":danger-full-access", description: "Full access", allowed: false },
        ],
      },
      requirementsRead: {
        requirements: {
          allowedSandboxModes: ["read-only", "workspace-write"],
          allowedPermissionProfiles: {
            ":workspace": true,
            ":danger-full-access": false,
          },
          defaultPermissions: ":workspace",
        },
      },
      usageRead: {
        summary: {
          lifetimeTokens: 12345,
          peakDailyTokens: 678,
          currentStreakDays: 3,
        },
        dailyUsageBuckets: [{ startDate: "2026-07-01", tokens: 678 }],
      },
      collaborationModeList: {
        data: [
          { name: "Default", mode: "default", model: null, reasoning_effort: null },
          { name: "Plan", mode: "plan", model: null, reasoning_effort: "medium" },
        ],
      },
    })

    expect(projection).toMatchObject({
      ok: true,
      cwd: "/repo",
      config: {
        model: "gpt-5.1-codex",
        modelProvider: "openai",
        reasoningEffort: "high",
        serviceTier: "priority",
        defaultPermissions: ":workspace",
        personality: "pragmatic",
      },
      models: {
        selected: {
          id: "gpt-5.1-codex",
          displayName: "GPT-5.1 Codex",
        },
        serviceTierCommands: ["priority"],
      },
      providerCapabilities: {
        namespaceTools: true,
        imageGeneration: false,
        webSearch: true,
      },
      permissions: {
        selectedProfile: ":workspace",
        blockedProfileIds: [":danger-full-access"],
      },
      requirements: {
        managed: true,
        blockers: [],
      },
      usage: {
        available: true,
      },
    })
    expect(projection.models.selected?.supportedReasoningEfforts.map(option => option.value))
      .toEqual(["medium", "high"])
    expect(projection.collaboration.modes.map(mode => mode.name)).toEqual(["Default", "Plan"])
    expect(projection.config.originKeys).toEqual(["model", "sandbox_mode"])
  })

  test("carries endpoint errors and managed blockers without leaking raw config", () => {
    const projection = projectKhalaCodeDesktopCodexSettings({
      errors: ["model/list: unavailable"],
      configRead: {
        config: {
          default_permissions: ":danger-full-access",
          sandbox_mode: "danger-full-access",
          api_key: "sk-local-secret",
        },
      },
      requirementsRead: {
        requirements: {
          allowedSandboxModes: ["read-only"],
          allowedPermissionProfiles: {
            ":danger-full-access": false,
          },
        },
      },
    })

    expect(projection.ok).toBe(false)
    expect(projection.errors).toEqual(["model/list: unavailable"])
    expect(projection.requirements.blockers.map(blocker => blocker.key)).toEqual([
      "codex.settings.endpoint.0",
      "codex.settings.default_permissions.managed",
      "codex.settings.sandbox_mode.managed",
    ])
    expect(JSON.stringify(projection)).not.toContain("sk-local-secret")
  })
})
