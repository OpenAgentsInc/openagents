import { describe, expect, test } from "bun:test"

import {
  DEFAULT_EFFECTIVE_CONFIG,
  resolveEffectiveConfig,
  type EffectiveConfigLayer,
} from "../src/tas/effective-config"

describe("tas effective config core", () => {
  test("later layers override earlier layers by key", () => {
    const layers: readonly EffectiveConfigLayer[] = [
      {
        layer: "defaults",
        config: {
          provider: {
            providerId: "chatgpt_codex",
            model: "codex-default",
          },
          routing: {
            mode: "local_only",
          },
        },
      },
      {
        layer: "project",
        config: {
          provider: {
            model: "project-model",
          },
          telemetry: {
            level: "errors",
          },
        },
      },
      {
        layer: "user",
        config: {
          provider: {
            model: "user-model",
          },
          approval: {
            mode: "auto",
          },
        },
      },
      {
        layer: "runtime",
        config: {
          provider: {
            providerId: "anthropic_claude",
          },
          routing: {
            mode: "provider_peers",
            allowedProviderIds: ["anthropic_claude"],
          },
        },
      },
    ]

    expect(resolveEffectiveConfig(layers).config).toMatchObject({
      provider: {
        providerId: "anthropic_claude",
        model: "user-model",
      },
      approval: {
        mode: "auto",
      },
      telemetry: {
        level: "errors",
      },
      routing: {
        mode: "provider_peers",
        allowedProviderIds: ["anthropic_claude"],
      },
    })
  })

  test("provenance records the winning layer for each resolved key", () => {
    const snapshot = resolveEffectiveConfig([
      {
        layer: "project",
        config: {
          budget: {
            maxTokens: 10_000,
            maxCostUsd: 5,
          },
          retention: {
            class: "short",
          },
        },
      },
      {
        layer: "runtime",
        config: {
          budget: {
            maxCostUsd: 1,
          },
        },
      },
    ])

    expect(snapshot.provenance.budget.maxTokens).toEqual({ layer: "project" })
    expect(snapshot.provenance.budget.maxCostUsd).toEqual({ layer: "runtime" })
    expect(snapshot.provenance.retention.class).toEqual({ layer: "project" })
  })

  test("missing keys fall back to defaults with default provenance", () => {
    const snapshot = resolveEffectiveConfig([
      {
        layer: "user",
        config: {
          telemetry: {
            enabled: false,
          },
        },
      },
    ])

    expect(snapshot.config.provider).toEqual(DEFAULT_EFFECTIVE_CONFIG.provider)
    expect(snapshot.config.budget).toEqual(DEFAULT_EFFECTIVE_CONFIG.budget)
    expect(snapshot.config.telemetry).toEqual({
      ...DEFAULT_EFFECTIVE_CONFIG.telemetry,
      enabled: false,
    })
    expect(snapshot.provenance.provider.providerId).toEqual({
      layer: "defaults",
    })
    expect(snapshot.provenance.telemetry.enabled).toEqual({ layer: "user" })
    expect(snapshot.provenance.telemetry.level).toEqual({ layer: "defaults" })
  })
})
