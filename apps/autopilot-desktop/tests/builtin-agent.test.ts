import { describe, expect, test } from "bun:test"

import {
  builtInAgentObjective,
  resolveBuiltInAgentSettings,
} from "../src/shared/builtin-agent"

describe("built-in agent preset (#5063)", () => {
  test("defaults to bounded hosted Google compute without a user key", () => {
    const settings = resolveBuiltInAgentSettings({
      OA_CLOUD_CONTROL_URL: "https://cloud.openagents.test",
      OA_CLOUD_CONTROL_TOKEN: "present",
    })

    expect(settings).toMatchObject({
      enabled: true,
      hostedComputeConfigured: true,
      lane: "cloud-gcp",
      modelSet: "openagents-hosted-gemini",
      maxSessionSeconds: 600,
      dailySessionCap: 3,
    })
    expect(settings.meteringLabel).toBe(
      "3 sessions/day · 600s/session · openagents-hosted-gemini",
    )
  })

  test("bounds configured caps and keeps the objective no-spend/no-key", () => {
    const settings = resolveBuiltInAgentSettings({
      OA_CLOUD_CONTROL_URL: "https://cloud.openagents.test",
      OA_CLOUD_CONTROL_TOKEN: "present",
      OPENAGENTS_BUILTIN_AGENT_LANE: "cloud-shc",
      OPENAGENTS_BUILTIN_AGENT_MAX_SESSION_SECONDS: "9999",
      OPENAGENTS_BUILTIN_AGENT_DAILY_SESSION_CAP: "99",
      OPENAGENTS_BUILTIN_AGENT_MODEL_SET: "hosted-gemini-free",
    })

    expect(settings.lane).toBe("cloud-shc")
    expect(settings.maxSessionSeconds).toBe(1200)
    expect(settings.dailySessionCap).toBe(20)
    expect(builtInAgentObjective(settings)).toContain(
      "do not ask the user for a provider API key",
    )
    expect(builtInAgentObjective(settings)).toContain("Do not modify code")
  })
})
