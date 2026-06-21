// VCODE-13 (#5930): deterministic multi-Codex account routing.

import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

import {
  nextCodeModeAccountOverride,
  projectCodeModeAccountRoute,
} from "../src/ui/code-mode-account-routing"
import type { CodeModeSyncAccountRow } from "../src/ui/code-mode-sync"

const workHash = "account.pylon.codex.work.abcdef0123456789abcdef0123456789"
const altHash = "account.pylon.codex.alt.11111111111111111111111111111111"
const longRef = "very-long-production-codex-account-reference-that-should-be-redacted"
const longHash = "account.pylon.codex.long.99999999999999999999999999999999"

const account = (
  input: Partial<CodeModeSyncAccountRow> & Pick<CodeModeSyncAccountRow, "accountRef">,
): CodeModeSyncAccountRow => ({
  key: `codex:${input.accountRef ?? "default"}`,
  provider: input.provider ?? "codex",
  accountRef: input.accountRef,
  accountRefHash: input.accountRefHash ?? `account.pylon.codex.${input.accountRef ?? "default"}`,
  label: input.label ?? `codex ${input.accountRef ?? "default"}`,
  selector: input.selector ?? (input.accountRef === null ? "default_home" : "registry_ref"),
  ready: input.ready ?? true,
  managed: input.managed ?? true,
  live: input.live ?? true,
  homePresent: input.homePresent ?? true,
  priority: input.priority ?? null,
  blockerRefs: input.blockerRefs ?? [],
  source: input.source ?? (input.accountRef === null ? "default_home" : "managed_live"),
})

const session = (input: Partial<SessionSummary>): SessionSummary => ({
  sessionRef: input.sessionRef ?? "session.pylon.codex.route",
  adapter: input.adapter ?? "codex",
  state: input.state ?? "completed",
  objectiveRef: input.objectiveRef ?? "objective.route",
  workspaceRef: input.workspaceRef ?? "workspace.repo",
  accountRefHash: input.accountRefHash ?? workHash,
  latestActivity: input.latestActivity ?? "completed route test",
  updatedAt: input.updatedAt ?? "2026-06-21T23:00:00.000Z",
})

describe("code mode account route projection", () => {
  test("explicit blocked account does not fall back to another account", () => {
    const route = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: "work",
      accounts: [
        account({ accountRef: "work", ready: false, priority: 0 }),
        account({ accountRef: "personal", ready: true, priority: 1 }),
      ],
      sessions: [],
      workspaceRef: null,
      allowDefaultHome: true,
    })
    expect(route.source).toBe("blocked")
    expect(route.accountRef).toBeNull()
    expect(route.blocker).toContain("blocked")
  })

  test("last-used workspace route beats priority deterministically", () => {
    const route = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: null,
      accounts: [
        account({ accountRef: "priority", accountRefHash: altHash, priority: 0 }),
        account({ accountRef: "work", accountRefHash: workHash, priority: 5 }),
      ],
      sessions: [
        session({
          sessionRef: "session.old",
          accountRefHash: altHash,
          workspaceRef: "workspace.repo",
          updatedAt: "2026-06-21T22:00:00.000Z",
        }),
        session({
          sessionRef: "session.new",
          accountRefHash: workHash,
          workspaceRef: "workspace.repo",
          updatedAt: "2026-06-21T23:00:00.000Z",
        }),
      ],
      workspaceRef: "workspace.repo",
      allowDefaultHome: true,
    })
    expect(route.source).toBe("last_used")
    expect(route.accountRef).toBe("work")
  })

  test("priority route is stable when no workspace account was last used", () => {
    const route = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: null,
      accounts: [
        account({ accountRef: "zeta", priority: 2 }),
        account({ accountRef: "alpha", priority: 1 }),
      ],
      sessions: [],
      workspaceRef: "workspace.repo",
      allowDefaultHome: true,
    })
    expect(route.source).toBe("priority")
    expect(route.accountRef).toBe("alpha")
  })

  test("default home only wins when default-home routing is allowed", () => {
    const allowed = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: null,
      accounts: [account({ accountRef: null, accountRefHash: "account.default" })],
      sessions: [],
      workspaceRef: null,
      allowDefaultHome: true,
    })
    expect(allowed.source).toBe("default_home")
    expect(allowed.accountRef).toBeNull()

    const blocked = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: null,
      accounts: [account({ accountRef: null, accountRefHash: "account.default" })],
      sessions: [],
      workspaceRef: null,
      allowDefaultHome: false,
    })
    expect(blocked.source).toBe("blocked")
  })

  test("route evidence redacts long account refs and hashes", () => {
    const route = projectCodeModeAccountRoute({
      adapter: "codex",
      selectedAccountRef: longRef,
      accounts: [account({ accountRef: longRef, accountRefHash: longHash })],
      sessions: [],
      workspaceRef: "workspace.repo",
      allowDefaultHome: true,
    })
    const evidence = JSON.stringify(route.evidence)
    expect(evidence).not.toContain(longRef)
    expect(evidence).not.toContain(longHash)
    expect(route.evidence.accountHash).toBe("#99999999")
  })

  test("override cycles ready named accounts and then default home", () => {
    const input = {
      adapter: "codex" as const,
      selectedAccountRef: "alpha",
      accounts: [
        account({ accountRef: "alpha", priority: 0 }),
        account({ accountRef: "beta", priority: 1 }),
      ],
      sessions: [],
      workspaceRef: null,
      allowDefaultHome: true,
    }
    expect(nextCodeModeAccountOverride(input)?.accountRef).toBe("beta")
    expect(nextCodeModeAccountOverride({ ...input, selectedAccountRef: "beta" })?.accountRef).toBeNull()
    expect(nextCodeModeAccountOverride({ ...input, selectedAccountRef: null })?.accountRef).toBe("beta")
  })
})
