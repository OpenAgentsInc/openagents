import { describe, expect, test } from "bun:test"

import {
  disablePlugin,
  effectiveContributions,
  enablePlugin,
  type Plugin,
} from "../src/tas/plugin-system"

const plugin = (overrides: Partial<Plugin> & Pick<Plugin, "name">): Plugin => ({
  version: "1.0.0",
  contributes: {},
  enabled: true,
  ...overrides,
})

describe("tas plugin system core", () => {
  test("enabled contributions are merged", () => {
    const registry: readonly Plugin[] = [
      plugin({
        name: "repo-tools",
        contributes: {
          tools: ["repo.search"],
          commands: ["repo"],
        },
      }),
      plugin({
        name: "review-tools",
        contributes: {
          tools: ["diff.review"],
          hooks: ["tool.before"],
        },
      }),
    ]

    expect(effectiveContributions(registry)).toEqual({
      tools: ["repo.search", "diff.review"],
      commands: ["repo"],
      hooks: ["tool.before"],
      conflicts: [],
    })
  })

  test("disabled contributions are excluded", () => {
    const registry = disablePlugin(
      [
        plugin({
          name: "repo-tools",
          contributes: { tools: ["repo.search"] },
        }),
        plugin({
          name: "disabled-review",
          contributes: {
            tools: ["diff.review"],
            commands: ["review"],
            hooks: ["tool.before"],
          },
        }),
      ],
      "disabled-review",
    )

    expect(effectiveContributions(registry)).toEqual({
      tools: ["repo.search"],
      commands: [],
      hooks: [],
      conflicts: [],
    })
  })

  test("conflicting contribution is flagged", () => {
    const registry: readonly Plugin[] = [
      plugin({
        name: "repo-tools",
        contributes: { tools: ["repo.search"] },
      }),
      plugin({
        name: "alternate-repo-tools",
        contributes: { tools: ["repo.search"] },
      }),
    ]

    expect(effectiveContributions(registry)).toEqual({
      tools: ["repo.search"],
      commands: [],
      hooks: [],
      conflicts: [
        {
          kind: "tools",
          name: "repo.search",
          pluginNames: ["repo-tools", "alternate-repo-tools"],
        },
      ],
    })
  })

  test("duplicate plugin name is rejected", () => {
    const registry: readonly Plugin[] = [
      plugin({ name: "repo-tools" }),
      plugin({ name: "repo-tools" }),
    ]

    expect(() => effectiveContributions(registry)).toThrow(
      "Duplicate plugin name: repo-tools",
    )
    expect(() => enablePlugin(registry, "repo-tools")).toThrow(
      "Duplicate plugin name: repo-tools",
    )
  })
})
