import { describe, expect, test } from "bun:test"

import {
  MobileExecutionTargetCatalogError,
  decodeMobileExecutionTargetCatalog,
  fetchMobileExecutionTargetCatalog,
  resolveMobileExecutionTargetOption,
} from "../src/coding/mobile-execution-targets"

const codexRef = "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa"
const claudeRef = "account.pylon.claude.bbbbbbbbbbbbbbbbbbbbbbbb"

const catalogFixture = () => ({
  availableModelIds: ["gemini", "openagents/khala", "some-future-model"],
  availableTargetIds: [
    "gemini",
    "auto",
    "khala",
    "agent-computer",
    `codex:${codexRef}`,
    `claude:${claudeRef}`,
  ],
  autoResolution: {
    effectiveTargetId: `codex:${codexRef}`,
    usedFallback: false,
    events: [],
  },
  claudeAccounts: [{
    accountRefHash: claudeRef,
    label: "Your Claude",
    ready: false,
    reason: "account_requires_reauth",
  }],
  codexAccounts: [{
    accountRefHash: codexRef,
    label: "Your Codex",
    ready: true,
  }],
  effectiveModelId: "auto",
  effectiveTargetId: "auto",
  fallback: "none",
  preferredModelId: "auto",
  preferredTargetId: "auto",
  updatedAt: "2026-07-12T12:00:00.000Z",
  usedPreference: true,
})

describe("mobile execution-target catalog", () => {
  test("strictly decodes the catalog and lowers every concrete target exactly", () => {
    const catalog = decodeMobileExecutionTargetCatalog(catalogFixture())

    expect(catalog.options.map(option => option.targetId)).toEqual([
      "khala",
      "gemini",
      "agent-computer",
      `codex:${codexRef}`,
      `claude:${claudeRef}`,
    ])
    expect(catalog.options.find(option => option.targetId === "khala")).toMatchObject({
      providerRef: "provider.openagents.hosted",
      modelRef: "model.gemini-3.5-flash",
      runtimeTarget: { lane: "hosted_khala", executionTargetId: "khala" },
    })
    expect(catalog.options.find(option => option.targetId === "agent-computer")).toMatchObject({
      accessibilityLabel: "Agent Computer, OpenAgents, ready",
      modelRef: "model.gpt-5.6-sol",
      providerRef: "provider.openagents.agent-computer",
      runtimeTarget: { lane: "managed_cloud", executionTargetId: "agent-computer" },
    })
    expect(catalog.options.find(option => option.targetId.startsWith("codex:"))).toMatchObject({
      accountRef: codexRef,
      providerRef: "provider.openai.codex",
      modelRef: "model.gpt-5.6-sol",
      runtimeTarget: {
        lane: "codex_app_server",
        executionTargetId: `codex:${codexRef}`,
      },
    })
    expect(catalog.options.find(option => option.targetId.startsWith("claude:"))).toMatchObject({
      accountRef: claudeRef,
      accessibilityLabel: "Your Claude, Claude, revoked",
      providerRef: "provider.anthropic.claude",
      modelRef: "model.claude-fable-5",
      readiness: "revoked",
      reasonRef: "reason.account_requires_reauth",
      runtimeTarget: {
        lane: "claude_pylon",
        executionTargetId: `claude:${claudeRef}`,
      },
    })
  })

  test("rejects malformed and excess response properties", () => {
    expect(() => decodeMobileExecutionTargetCatalog({
      ...catalogFixture(),
      bearerToken: "must-never-be-accepted",
    })).toThrow(MobileExecutionTargetCatalogError)

    expect(() => decodeMobileExecutionTargetCatalog({
      ...catalogFixture(),
      codexAccounts: [{ label: "missing ref", ready: true }],
    })).toThrow(MobileExecutionTargetCatalogError)
  })

  test("maps readiness to accessible labels and typed reasons", () => {
    const fixture = catalogFixture()
    const catalog = decodeMobileExecutionTargetCatalog({
      ...fixture,
      codexAccounts: [{
        ...fixture.codexAccounts[0],
        ready: false,
        reason: "account_rate_limited",
      }],
      claudeAccounts: [{
        ...fixture.claudeAccounts[0],
        reason: "account_unavailable",
      }],
    })
    const codex = catalog.options.find(option => option.providerLabel === "Codex")!
    const claude = catalog.options.find(option => option.providerLabel === "Claude")!

    expect(codex).toMatchObject({
      accessibilityLabel: "Your Codex, Codex, unavailable",
      readiness: "unavailable",
      reasonRef: "reason.account_rate_limited",
    })
    expect(claude).toMatchObject({
      accessibilityLabel: "Your Claude, Claude, offline",
      readiness: "offline",
      reasonRef: "reason.account_unavailable",
    })
    expect(resolveMobileExecutionTargetOption(catalog, codex.targetId)).toMatchObject({
      state: "refused",
      reason: "account_rate_limited",
    })
  })

  test("fails closed for unknown and unadvertised targets", () => {
    const catalog = decodeMobileExecutionTargetCatalog(catalogFixture())
    expect(resolveMobileExecutionTargetOption(catalog, "codex:unknown")).toEqual({
      state: "refused",
      reason: "target_not_advertised",
    })

    const fixture = catalogFixture()
    expect(() => decodeMobileExecutionTargetCatalog({
      ...fixture,
      availableTargetIds: fixture.availableTargetIds.filter(
        targetId => !targetId.startsWith("codex:"),
      ),
    })).toThrow("Codex account is not present")
  })

  test("resolves Auto to its concrete target and never returns literal auto", () => {
    const catalog = decodeMobileExecutionTargetCatalog(catalogFixture())

    expect(catalog.effectiveTargetId).toBe(`codex:${codexRef}`)
    expect(catalog.options.some(option => option.targetId === "auto")).toBe(false)
    expect(resolveMobileExecutionTargetOption(catalog, "auto")).toMatchObject({
      state: "ready",
      option: { targetId: `codex:${codexRef}` },
    })

    const fixture = catalogFixture()
    const unresolved = decodeMobileExecutionTargetCatalog({
      ...fixture,
      autoResolution: {
        effectiveTargetId: null,
        usedFallback: true,
        events: [{
          type: "account_unavailable",
          targetId: `codex:${codexRef}`,
          nextTargetId: null,
        }],
      },
    })
    expect(resolveMobileExecutionTargetOption(unresolved, "auto")).toEqual({
      state: "refused",
      reason: "auto_unresolved",
    })
  })

  test("uses the bearer only on the request and redacts it from failures", async () => {
    const token = "native-secret-token-that-must-not-escape"
    let authorization = ""
    const catalog = await fetchMobileExecutionTargetCatalog({
      baseUrl: "https://openagents.example",
      token,
      fetch: async (_request, init) => {
        authorization = new Headers(init?.headers).get("authorization") ?? ""
        return Response.json(catalogFixture())
      },
    })

    expect(authorization).toBe(`Bearer ${token}`)
    expect(JSON.stringify(catalog)).not.toContain(token)

    const failure = await fetchMobileExecutionTargetCatalog({
      baseUrl: "https://openagents.example",
      token,
      fetch: async () => new Response(token, { status: 500 }),
    }).catch(error => error)
    expect(String(failure)).not.toContain(token)
    expect(JSON.stringify(failure)).not.toContain(token)
  })
})
