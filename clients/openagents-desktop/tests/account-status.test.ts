import { describe, expect, test } from "bun:test"

import {
  accountStatusFromPayload,
  fleetAccountFromEntry,
  parseFleetAccounts,
} from "../src/shared/account-status.js"

const payload = {
  schema: "openagents.pylon.accounts_list.v0.3",
  accounts: [
    {
      provider: "codex",
      accountRef: "codex-2",
      readiness: {
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "ready",
        enabled: true,
        capabilityRefs: ["capability.pylon.local_codex"],
        blockerRefs: [],
        credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
      },
      blockerRefs: [],
    },
    {
      provider: "codex",
      accountRef: null,
      readiness: {
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "ready",
        enabled: true,
        capabilityRefs: ["capability.pylon.local_codex"],
        blockerRefs: [],
        credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
      },
      blockerRefs: [],
    },
    {
      provider: "claude_agent",
      accountRef: null,
      readiness: {
        schema: "openagents.pylon.claude_agent_readiness.v0.3",
        state: "ready",
        enabled: true,
        capabilityRefs: ["capability.pylon.local_claude_agent"],
        blockerRefs: [],
        credentialSourceRef: "credential.source.claude_agent.local_claude_session",
      },
      blockerRefs: [],
    },
    {
      provider: "codex",
      accountRef: "codex-supervisor",
      readiness: {
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "credentials_missing",
        enabled: true,
        capabilityRefs: [],
        blockerRefs: ["blocker.codex_agent.credentials_missing"],
        credentialSourceRef: null,
      },
      blockerRefs: ["blocker.codex_agent.credentials_missing"],
    },
  ],
}

describe("openagents desktop fleet account status", () => {
  test("maps codex and claude providers from readiness schema", () => {
    const accounts = parseFleetAccounts(payload)
    expect(accounts.map(account => account.provider)).toEqual([
      "codex",
      "codex",
      "claude",
      "codex",
    ])
  })

  test("labels null accountRefs with the credential source, ready reflects state", () => {
    const accounts = parseFleetAccounts(payload)
    expect(accounts[0].label).toBe("codex-2")
    expect(accounts[1].label).toBe(
      "credential.source.codex_agent.codex_cli_login",
    )
    expect(accounts[1].accountRef).toBeNull()
    expect(accounts[0].ready).toBe(true)
    expect(accounts[3].ready).toBe(false)
    expect(accounts[3].credentialsMissing).toBe(true)
    expect(accounts[3].blockerRefs).toEqual([
      "blocker.codex_agent.credentials_missing",
    ])
  })

  test("summary counts ready vs needs-reconnect accounts", () => {
    const result = accountStatusFromPayload(payload, "2026-06-29T00:00:00.000Z")
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.readyCount).toBe(3)
    expect(result.needsReconnectCount).toBe(1)
    expect(result.observedAt).toBe("2026-06-29T00:00:00.000Z")
  })

  test("falls back to unnamed and degraded for unknown shapes", () => {
    const account = fleetAccountFromEntry({
      readiness: { state: "usage_limited" },
    })
    expect(account.label).toBe("unnamed")
    expect(account.provider).toBe("codex")
    expect(account.state).toBe("usage_limited")
    expect(account.ready).toBe(false)
    expect(account.credentialsMissing).toBe(false)
  })

  test("handles non-object payloads gracefully", () => {
    expect(parseFleetAccounts(null)).toEqual([])
    expect(parseFleetAccounts({ accounts: "nope" })).toEqual([])
  })
})
