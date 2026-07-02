import { describe, expect, test } from "bun:test"

import { buildPylonKhalaBurndownPlan } from "./khala-burndown.js"
import type { PylonAccountsListProjection } from "./account-usage.js"

const accounts: PylonAccountsListProjection = {
  accounts: [
    {
      accountRef: null,
      accountRefHash: "account.pylon.codex.default",
      blockerRefs: [],
      homeRef: "home.pylon.codex.default",
      homeState: "present",
      provider: "codex",
      readiness: {
        blockerRefs: [],
        capabilityRefs: ["capability.pylon.local_codex"],
        credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
        enabled: true,
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "ready",
      },
      selector: "default_home",
    },
  ],
  blockerRefs: [],
  observedAt: "2026-06-27T13:30:00.000Z",
  schema: "openagents.pylon.accounts_list.v0.3",
}

const claudeAccounts: PylonAccountsListProjection = {
  ...accounts,
  accounts: [
    {
      accountRef: "claude-a",
      accountRefHash: "account.pylon.claude.a",
      blockerRefs: [],
      homeRef: "home.pylon.claude.a",
      homeState: "present",
      provider: "claude_agent",
      readiness: {
        blockerRefs: [],
        capabilityRefs: ["capability.pylon.local_claude"],
        credentialSourceRef: "credential.source.claude_agent.setup_token",
        enabled: true,
        schema: "openagents.pylon.claude_agent_readiness.v0.3",
        state: "ready",
      },
      selector: "registry_ref",
    },
  ],
}

describe("Khala burndown plan", () => {
  test("keeps requested issue count visible when capacity is unavailable", () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts,
      advertisedCodexAvailability: 0,
      baseUrl: "https://openagents.example",
      commit: "2e5937497ec2d7a6b5256e7265042b0e78cd294f",
      issueNumbers: [6359, 6366],
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.33afd48282a649047e3a",
      verificationCommand: "bun run --cwd apps/openagents.com check:public-projection-freshness",
    })

    expect(plan.issueCount).toBe(2)
    expect(plan.slots).toHaveLength(0)
    expect(plan.blockerRefs).toContain(
      "blocker.khala_burndown.no_advertised_codex_availability",
    )
    expect(plan.blockerRefs).not.toContain("blocker.khala_burndown.no_issue_numbers")
  })

  test("uses advertised per-account capacity beyond one slot per account", () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: {
        ...accounts,
        accounts: [
          {
            ...accounts.accounts[0]!,
            accountRef: "codex-a",
            accountRefHash: "account.pylon.codex.a",
          },
          {
            ...accounts.accounts[0]!,
            accountRef: "codex-b",
            accountRefHash: "account.pylon.codex.b",
          },
        ],
      },
      advertisedCodexAccounts: [
        {
          accountKey: "a",
          accountRefHash: "account.pylon.codex.a",
          available: 3,
          busy: 0,
          queued: 0,
          ready: 3,
        },
        {
          accountKey: "b",
          accountRefHash: "account.pylon.codex.b",
          available: 2,
          busy: 0,
          queued: 0,
          ready: 2,
        },
      ],
      baseUrl: "https://openagents.example",
      commit: "2e5937497ec2d7a6b5256e7265042b0e78cd294f",
      issueNumbers: [6323, 6311, 6320, 6318, 6317],
      maxParallel: 5,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.33afd48282a649047e3a",
      verificationCommand: "bun test apps/pylon/src/khala-burndown.test.ts",
    })

    expect(plan.maxParallel).toBe(5)
    expect(plan.slots).toHaveLength(5)
    expect(plan.slots.map((slot) => slot.account.accountRef)).toEqual([
      "codex-a",
      "codex-b",
      "codex-a",
      "codex-b",
      "codex-a",
    ])
    expect(plan.slots.map((slot) => slot.requestInput.targetAccountRefHash)).toEqual([
      "account.pylon.codex.a",
      "account.pylon.codex.b",
      "account.pylon.codex.a",
      "account.pylon.codex.b",
      "account.pylon.codex.a",
    ])
  })

  test("uses claude_agent_task when worker kind is Claude", () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: claudeAccounts,
      advertisedCodexAccounts: [
        {
          accountKey: "claude-a",
          accountRefHash: "account.pylon.claude.a",
          available: 2,
          busy: 0,
          queued: 0,
          ready: 2,
        },
      ],
      baseUrl: "https://openagents.example",
      commit: "2e5937497ec2d7a6b5256e7265042b0e78cd294f",
      issueNumbers: [6323, 6311],
      maxParallel: 2,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.33afd48282a649047e3a",
      verificationCommand: "bun test apps/pylon/src/khala-burndown.test.ts",
      workerKind: "claude",
    })

    expect(plan.workerKind).toBe("claude")
    expect(plan.workflow).toBe("claude_agent_task")
    expect(plan.readyWorkerAccountCount).toBe(1)
    expect(plan.slots).toHaveLength(2)
    expect(plan.slots[0]?.requestInput.workflow).toBe("claude_agent_task")
    expect(plan.slots[0]?.commands.request).toContain("--workflow claude_agent_task")
    expect(plan.slots.map((slot) => slot.account.accountRef)).toEqual(["claude-a", "claude-a"])
  })
})
