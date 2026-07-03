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
          tui: {
            keymap: {
              global: {
                "ctrl-j": "move-down",
              },
            },
            vim_mode_default: true,
            status_line: ["model-with-reasoning", "current-dir"],
            status_line_use_colors: false,
            theme: "github-dark",
            pet: "spark",
            pet_anchor: "screen-bottom",
          },
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
      appearance: {
        pet: "spark",
        petAnchor: "screen-bottom",
        personality: "pragmatic",
        statusLine: ["model-with-reasoning", "current-dir"],
        statusLineUseColors: false,
        theme: "github-dark",
        vimModeDefault: true,
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
    expect(projection.modelRolePresets).toMatchObject({
      keyPath: "openagents.model_roles",
      activePreset: null,
      presets: [{
        id: "architect-coder-judge",
        promiseRef: "khala_code.architect_coder_judge.v1",
        noProxyRails: true,
        noResale: true,
        selected: false,
      }],
    })
    expect(projection.config.originKeys).toEqual(["model", "sandbox_mode"])
    expect(projection.appearance.keyPaths).toMatchObject({
      keymap: "tui.keymap",
      pet: "tui.pet",
      personality: "personality",
      statusLine: "tui.status_line",
      theme: "tui.theme",
      vimModeDefault: "tui.vim_mode_default",
    })
    expect(projection.appearance.keymap).toEqual({
      global: {
        "ctrl-j": "move-down",
      },
    })
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

  test("detects the architect-coder-judge role registry preset without proxy rails", () => {
    const projection = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          openagents: {
            model_roles: {
              schema: "openagents.khala_code.model_roles.v1",
              activePreset: "architect-coder-judge",
              noProxyRails: true,
              noResale: true,
              promiseRef: "khala_code.architect_coder_judge.v1",
            },
          },
        },
      },
    })

    expect(projection.modelRolePresets.activePreset).toBe("architect-coder-judge")
    expect(projection.modelRolePresets.presets[0]).toMatchObject({
      id: "architect-coder-judge",
      selected: true,
      registry: {
        noProxyRails: true,
        noResale: true,
        roles: [
          { role: "architect", harness: "claude", authRail: "user_anthropic_auth" },
          { role: "coder", harness: "codex", authRail: "user_codex_login" },
          { role: "judge", harness: "claude", authRail: "user_anthropic_auth" },
          { role: "advisor", enabled: false, optional: true },
        ],
      },
    })
  })
})
