import { describe, expect, test } from "bun:test"

import { projectKhalaCodeDesktopCodexEcosystem } from "../src/shared/codex-ecosystem"
import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import {
  filterKhalaCodeModelManagerEntries,
  khalaCodeMcpManagerIntent,
  projectKhalaCodeModelMcpPermissionManager,
} from "../src/shared/model-mcp-permission-manager"

describe("Khala Code model/MCP/permission manager projection", () => {
  test("projects grouped model visibility and searchable hidden-model state", () => {
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model: "gpt-5.5-codex",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            displayName: "GPT-5.5 Codex",
            provider: "openai",
            providerDisplayName: "OpenAI",
          },
          {
            id: "codex-auto-review",
            model: "codex-auto-review",
            displayName: "Codex Auto Review",
            provider: "openai",
            providerDisplayName: "OpenAI",
            hidden: true,
          },
          {
            id: "anthropic/opus",
            model: "anthropic/opus",
            displayName: "Opus",
            provider: "anthropic",
            providerDisplayName: "Anthropic",
            serviceTiers: [{ id: "paid", name: "Paid" }],
          },
        ],
      },
    })

    const projection = projectKhalaCodeModelMcpPermissionManager({
      ecosystem: null,
      hiddenModelIds: new Set(["gpt-5.5-codex"]),
      settings,
    })

    expect(projection.models.map(model => [model.id, model.providerDisplayName, model.state, model.visible])).toEqual([
      ["anthropic/opus", "Anthropic", "unpaid", true],
      ["codex-auto-review", "OpenAI", "hidden", false],
      ["gpt-5.5-codex", "OpenAI", "hidden", false],
    ])
    expect(filterKhalaCodeModelManagerEntries(projection.models, "opus").map(model => model.id)).toEqual([
      "anthropic/opus",
    ])
  })

  test("projects MCP status and enable/authentication intents without raw payloads", () => {
    const settings = projectKhalaCodeDesktopCodexSettings({})
    const ecosystem = projectKhalaCodeDesktopCodexEcosystem({
      mcpServerStatusList: {
        data: [
          {
            name: "khala_fleet",
            authStatus: "notRequired",
            tools: { fleet_run_start: { description: "start" } },
          },
          {
            name: "private_oauth",
            authStatus: "notLoggedIn",
            tools: { secret_tool: { token: "raw-mcp-token" } },
          },
        ],
      },
      notifications: [{
        method: "mcpServer/startupStatus/updated",
        receivedAt: "2026-07-05T00:00:00.000Z",
        params: {
          name: "broken_server",
          status: "failed",
          error: "spawn failed",
        },
      }],
    })
    const projection = projectKhalaCodeModelMcpPermissionManager({ ecosystem, settings })

    expect(projection.mcp.map(entry => [entry.name, entry.state])).toEqual([
      ["khala_fleet", "connected"],
      ["private_oauth", "needs_auth"],
    ])
    expect(khalaCodeMcpManagerIntent(projection.mcp[0]!, "disable")).toMatchObject({
      ok: true,
      nextStep: "reload_mcp",
    })
    expect(khalaCodeMcpManagerIntent(projection.mcp[1]!, "authenticate")).toMatchObject({
      ok: false,
      nextStep: "oauth_login",
      retryable: true,
    })
    expect(JSON.stringify(projection)).not.toContain("raw-mcp-token")
  })

  test("disables auto-accept when Codex requirements are managed", () => {
    const settings = projectKhalaCodeDesktopCodexSettings({
      requirementsRead: {
        requirements: {
          allowedSandboxModes: ["read-only"],
        },
      },
    })
    const projection = projectKhalaCodeModelMcpPermissionManager({
      ecosystem: null,
      permissionAutoAcceptMode: "session",
      settings,
    })

    expect(projection.permissions.autoAccept).toMatchObject({
      mode: "session",
      allowed: false,
    })
    expect(projection.permissions.autoAccept.detail).toContain("Managed Codex requirements")
  })
})
