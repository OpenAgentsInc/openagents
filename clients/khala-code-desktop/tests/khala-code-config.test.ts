import { describe, expect, test } from "bun:test"
import { inspect } from "node:util"
import { Effect, Redacted } from "effect"

import {
  KhalaCodeConfig,
  KhalaCodePlainEnvKeys,
  KhalaCodeSecretEnvKeys,
  khalaCodeConfigFromEnv,
} from "../src/bun/khala-code-config"

describe("KhalaCodeConfig", () => {
  test("parses the Bun host env key surface through the schema", () => {
    const config = khalaCodeConfigFromEnv({
      CODEX_HOME: "/tmp/codex-home",
      KHALA_CODE_CODEX_BINARY: "/opt/bin/codex",
      KHALA_CODE_DESKTOP_PREVIEW_PORT: "6123",
      KHALA_CODE_HOSTED_BYOK_OPENROUTER: "1",
      KHALA_CODE_HOSTED_BYOK_OPENROUTER_API_KEY: "hosted-byok-secret",
      KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN: "token-secret",
      OPENAGENTS_AGENT_TOKEN: "agent-secret",
      PYLON_CONTROL_TOKEN: "control-secret",
    })

    expect(config.plain.CODEX_HOME).toBe("/tmp/codex-home")
    expect(config.plain.KHALA_CODE_CODEX_BINARY).toBe("/opt/bin/codex")
    expect(config.plain.KHALA_CODE_DESKTOP_PREVIEW_PORT).toBe("6123")
    expect(config.plain.KHALA_CODE_HOSTED_BYOK_OPENROUTER).toBe("1")
    expect(Redacted.value(config.secrets.KHALA_CODE_HOSTED_BYOK_OPENROUTER_API_KEY)).toBe("hosted-byok-secret")
    expect(Redacted.value(config.secrets.KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN)).toBe("token-secret")
    expect(Redacted.value(config.secrets.OPENAGENTS_AGENT_TOKEN)).toBe("agent-secret")
    expect(Redacted.value(config.secrets.PYLON_CONTROL_TOKEN)).toBe("control-secret")
    expect(config.env.KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN).toBe("token-secret")
  })

  test("covers every declared host env key with a parsed slot", () => {
    const config = khalaCodeConfigFromEnv({})

    for (const key of KhalaCodePlainEnvKeys) {
      expect(config.plain[key]).toBe("")
    }
    for (const key of KhalaCodeSecretEnvKeys) {
      expect(Redacted.value(config.secrets[key])).toBe("")
    }
  })

  test("keeps token-bearing values redacted in printable config output", () => {
    const config = khalaCodeConfigFromEnv({
      KHALA_CODE_HOSTED_BYOK_OPENROUTER_API_KEY: "hosted-byok-do-not-print",
      KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN: "bearer-do-not-print",
      KHALA_GPT_OSS_API_KEY: "gpt-key-do-not-print",
      OPENROUTER_API_KEY: "router-key-do-not-print",
      PYLON_CONTROL_TOKEN: "control-do-not-print",
    })

    const printed = [
      String(config.secrets.KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN),
      JSON.stringify(config),
      inspect(config),
    ].join("\n")

    expect(printed).toContain("<redacted>")
    expect(printed).not.toContain("hosted-byok-do-not-print")
    expect(printed).not.toContain("bearer-do-not-print")
    expect(printed).not.toContain("gpt-key-do-not-print")
    expect(printed).not.toContain("router-key-do-not-print")
    expect(printed).not.toContain("control-do-not-print")
  })

  test("supports Layer.succeed test-profile injection", async () => {
    const program = Effect.gen(function* () {
      const config = yield* KhalaCodeConfig
      return {
        codexHome: config.env.CODEX_HOME,
        token: Redacted.value(config.secrets.OPENAGENTS_API_KEY),
      }
    })

    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(
          KhalaCodeConfig.testProfile({
            CODEX_HOME: "/tmp/injected-codex",
            OPENAGENTS_API_KEY: "injected-secret",
          }),
        ),
      ),
    )

    expect(result).toEqual({
      codexHome: "/tmp/injected-codex",
      token: "injected-secret",
    })
  })
})

describe("spawn env interop", () => {
  test("undeclared runtime keys survive into config.env with declared overlay", () => {
    const config = khalaCodeConfigFromEnv({
      SSH_AUTH_SOCK: "/tmp/agent.sock",
      GH_TOKEN: "gh-secret",
      TMPDIR: "/var/tmp/x",
      KHALA_CODE_DESKTOP_PREVIEW_SERVER: "1",
      EMPTYVAL: "",
    })
    const env = config.env as Record<string, string | undefined>
    expect(env.SSH_AUTH_SOCK).toBe("/tmp/agent.sock")
    expect(env.GH_TOKEN).toBe("gh-secret")
    expect(env.TMPDIR).toBe("/var/tmp/x")
    expect(env.KHALA_CODE_DESKTOP_PREVIEW_SERVER).toBe("1")
    expect(env.EMPTYVAL).toBe("")
  })
})
