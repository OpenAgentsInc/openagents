import { describe, expect, test } from "bun:test"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import {
  khalaCodeProviderConnectionIntent,
  projectKhalaCodeProviderCatalog,
  validateKhalaCodeOpenAiCompatibleProvider,
} from "../src/shared/provider-catalog"

describe("Khala Code provider catalog", () => {
  test("projects Codex provider options into connected, paid, env, and auth states", () => {
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model_provider: "openai",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            provider: "openai",
            providerDisplayName: "OpenAI",
          },
          {
            id: "anthropic/claude-sonnet",
            model: "anthropic/claude-sonnet",
            provider: "anthropic",
            providerDisplayName: "Anthropic",
          },
          {
            id: "ollama/qwen",
            model: "ollama/qwen",
            provider: "ollama",
            providerDisplayName: "Ollama",
          },
          {
            id: "mistral/devstral",
            model: "mistral/devstral",
            provider: "mistral",
            providerDisplayName: "Mistral",
          },
        ],
      },
    })

    const catalog = projectKhalaCodeProviderCatalog(settings)
    expect(catalog.map(entry => [entry.id, entry.state, entry.selected])).toEqual([
      ["openai", "connected", true],
      ["anthropic", "paid", false],
      ["mistral", "missing_auth", false],
      ["ollama", "env_configured", false],
    ])
  })

  test("validates custom OpenAI-compatible providers without echoing secrets", () => {
    const result = validateKhalaCodeOpenAiCompatibleProvider({
      id: "Local_OpenAI",
      displayName: "Local OpenAI",
      baseUrl: "http://user:sk-private-value@localhost:8080/v1#debug",
      modelIds: [" qwen2.5 ", "qwen2.5", "llama"],
      apiKeyConfigured: true,
    })

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.provider).toEqual({
        id: "local_openai",
        displayName: "Local OpenAI",
        baseUrl: "http://localhost:8080/v1",
        modelIds: ["qwen2.5", "llama"],
        apiKeyConfigured: true,
      })
      expect(JSON.stringify(result)).not.toContain("sk-private-value")
    }
  })

  test("returns retryable connection intents with specific next steps", () => {
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model_provider: "openai",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            provider: "openai",
            providerDisplayName: "OpenAI",
          },
          {
            id: "openrouter/sonoma",
            model: "openrouter/sonoma",
            provider: "openrouter",
            providerDisplayName: "OpenRouter",
          },
          {
            id: "mistral/devstral",
            model: "mistral/devstral",
            provider: "mistral",
            providerDisplayName: "Mistral",
          },
        ],
      },
    })
    const catalog = projectKhalaCodeProviderCatalog(settings, [{
      id: "local-openai",
      displayName: "Local OpenAI",
      baseUrl: "http://localhost:8080/v1",
      modelIds: ["qwen"],
      apiKeyConfigured: false,
    }])

    expect(khalaCodeProviderConnectionIntent(catalog.find(entry => entry.id === "openrouter")!, "connect")).toMatchObject({
      ok: false,
      nextStep: "upgrade_plan",
      retryable: true,
    })
    expect(khalaCodeProviderConnectionIntent(catalog.find(entry => entry.id === "mistral")!, "connect")).toMatchObject({
      ok: false,
      nextStep: "configure_environment",
      retryable: true,
    })
    expect(khalaCodeProviderConnectionIntent(catalog.find(entry => entry.id === "local-openai")!, "connect")).toMatchObject({
      ok: true,
      nextStep: "custom_provider_pending_runtime",
      retryable: true,
    })
  })
})
