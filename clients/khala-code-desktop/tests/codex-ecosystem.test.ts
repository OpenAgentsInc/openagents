import { describe, expect, test } from "bun:test"

import {
  projectKhalaCodeDesktopCodexEcosystem,
} from "../src/shared/codex-ecosystem"

const observedAt = "2026-07-01T12:00:00.000Z"

describe("Khala Code Codex ecosystem projection", () => {
  test("projects Codex skills, hooks, plugins, apps, MCP, and notifications", () => {
    const projection = projectKhalaCodeDesktopCodexEcosystem({
      cwd: "/workspace/openagents",
      observedAt,
      skillsList: {
        data: [{
          cwd: "/workspace/openagents",
          skills: [
            {
              name: "github",
              description: "GitHub workflow",
              path: "/home/user/.codex/skills/github/SKILL.md",
              scope: "user",
              enabled: true,
            },
            {
              name: "admin-skill",
              description: "Managed skill",
              path: "/etc/codex/skills/admin/SKILL.md",
              scope: "admin",
              enabled: false,
            },
          ],
          errors: [],
        }],
      },
      hooksList: {
        data: [{
          cwd: "/workspace/openagents",
          hooks: [{
            key: "format",
            eventName: "postToolUse",
            handlerType: "command",
            matcher: null,
            command: "bun fmt",
            timeoutSec: 10,
            statusMessage: null,
            sourcePath: "/workspace/.codex/hooks.json",
            source: "project",
            pluginId: null,
            displayOrder: 0,
            enabled: true,
            isManaged: true,
            currentHash: "hash",
            trustStatus: "managed",
          }],
          warnings: [],
          errors: [],
        }],
      },
      pluginList: {
        marketplaces: [{
          name: "curated",
          path: null,
          interface: null,
          plugins: [
            {
              id: "github@curated",
              remotePluginId: null,
              localVersion: "1.0.0",
              name: "github",
              shareContext: null,
              source: { type: "remote" },
              installed: true,
              enabled: true,
              installPolicy: "AVAILABLE",
              authPolicy: "ON_USE",
              availability: "AVAILABLE",
              interface: null,
              keywords: [],
            },
            {
              id: "admin-disabled@curated",
              remotePluginId: null,
              localVersion: null,
              name: "admin-disabled",
              shareContext: null,
              source: { type: "remote" },
              installed: false,
              enabled: false,
              installPolicy: "NOT_AVAILABLE",
              authPolicy: "ON_INSTALL",
              availability: "DISABLED_BY_ADMIN",
              interface: null,
              keywords: [],
            },
            {
              id: "install-me@curated",
              remotePluginId: null,
              localVersion: null,
              name: "install-me",
              shareContext: null,
              source: { type: "remote" },
              installed: false,
              enabled: true,
              installPolicy: "AVAILABLE",
              authPolicy: "ON_INSTALL",
              availability: "AVAILABLE",
              interface: null,
              keywords: [],
            },
          ],
        }],
        marketplaceLoadErrors: [],
        featuredPluginIds: [],
      },
      pluginInstalled: {
        marketplaces: [],
        marketplaceLoadErrors: [],
      },
      appsList: {
        data: [
          {
            id: "linear",
            name: "Linear",
            description: null,
            logoUrl: null,
            logoUrlDark: null,
            iconAssets: null,
            iconDarkAssets: null,
            distributionChannel: null,
            branding: null,
            appMetadata: null,
            labels: null,
            installUrl: null,
            isAccessible: false,
            isEnabled: true,
            pluginDisplayNames: ["github"],
          },
          {
            id: "notion",
            name: "Notion",
            description: null,
            logoUrl: null,
            logoUrlDark: null,
            iconAssets: null,
            iconDarkAssets: null,
            distributionChannel: null,
            branding: null,
            appMetadata: null,
            labels: null,
            installUrl: null,
            isAccessible: true,
            isEnabled: false,
            pluginDisplayNames: [],
          },
        ],
        nextCursor: null,
      },
      mcpServerStatusList: {
        data: [{
          name: "github",
          serverInfo: null,
          tools: { list_issues: { name: "list_issues" } },
          resources: [],
          resourceTemplates: [],
          authStatus: "notLoggedIn",
        }],
        nextCursor: null,
      },
      notifications: [
        {
          method: "skills/changed",
          params: {},
          receivedAt: "2026-07-01T12:01:00.000Z",
        },
        {
          method: "mcpServer/startupStatus/updated",
          params: {
            threadId: null,
            name: "github",
            status: "failed",
            error: "OAuth token expired",
            failureReason: "reauthenticationRequired",
          },
          receivedAt: "2026-07-01T12:02:00.000Z",
        },
      ],
    })

    expect(projection.sections.skills.count).toBe(2)
    expect(projection.sections.hooks.managedCount).toBe(1)
    expect(projection.sections.plugins.authRequiredCount).toBe(2)
    expect(projection.sections.plugins.installRequiredCount).toBe(1)
    expect(projection.sections.plugins.disabledCount).toBe(1)
    expect(projection.sections.apps.authRequiredCount).toBe(1)
    expect(projection.sections.apps.disabledCount).toBe(1)
    expect(projection.sections.mcp.authRequiredCount).toBe(1)
    expect(projection.sections.khala.count).toBe(2)
    expect(projection.notifications.map(notification => notification.method)).toContain("skills/changed")
    expect(projection.diagnostics.map(diagnostic => diagnostic.title)).toContain("Codex skills changed")
    expect(projection.diagnostics.some(diagnostic =>
      diagnostic.title === "github MCP server needs login"
    )).toBe(true)
  })

  test("keeps unknown app-server states safe and summarized", () => {
    const projection = projectKhalaCodeDesktopCodexEcosystem({
      observedAt,
      skillsList: {
        data: [{
          cwd: "/workspace",
          skills: [{
            name: "mystery",
            description: "Mystery skill",
            path: "/workspace/SKILL.md",
            scope: "user",
            unusedSecret: "SECRET_VALUE",
          }],
          errors: [],
        }],
      },
      pluginList: {
        marketplaces: [{
          name: "local",
          path: "/workspace/.codex/plugins.json",
          interface: null,
          plugins: [{
            id: "mystery@local",
            name: "mystery",
            installed: true,
            enabled: true,
            installPolicy: "AVAILABLE",
            authPolicy: "ON_USE",
            availability: "SOMETHING_NEW",
            source: { type: "local", path: "/workspace/plugin" },
            secret: "SECRET_VALUE",
            keywords: [],
          }],
        }],
        marketplaceLoadErrors: [],
        featuredPluginIds: [],
      },
      appsList: {
        data: [{
          id: "mystery-app",
          name: "Mystery app",
          labels: { token: "SECRET_VALUE" },
          pluginDisplayNames: [],
        }],
        nextCursor: null,
      },
      mcpServerStatusList: {
        data: [{
          name: "mystery-mcp",
          tools: {},
          resources: [],
          resourceTemplates: [],
          authStatus: "brandNewAuthState",
          secret: "SECRET_VALUE",
        }],
        nextCursor: null,
      },
    })

    expect(projection.sections.skills.unknownCount).toBe(1)
    expect(projection.sections.apps.unknownCount).toBe(1)
    expect(projection.sections.mcp.unknownCount).toBe(1)
    expect(projection.diagnostics.some(diagnostic =>
      diagnostic.title.includes("Unknown")
    )).toBe(true)
    expect(JSON.stringify(projection)).not.toContain("SECRET_VALUE")
  })
})
