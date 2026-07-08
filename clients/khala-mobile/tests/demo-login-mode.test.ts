import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  RUNTIME_EVENT_ENTITY_TYPE,
  personalScope,
  threadScope,
} from "@openagentsinc/khala-sync"

import {
  initialKhalaAuthMachineState,
  reduceKhalaAuthMachine,
} from "../src/auth/khala-auth-state-machine"
import {
  DEMO_REVIEWER_CREDENTIALS,
  DEMO_REVIEWER_OWNER_USER_ID,
  DEMO_REVIEWER_TOKEN,
  DEMO_CREDITS_BALANCE_USD_CENTS,
  DEMO_MODEL_ID,
  demoChatMessagesByThread,
  demoChatThreads,
  demoCreditsTransactions,
  demoModelPreference,
  demoRepositories,
  demoSyncScopeEntities,
  isDemoToken,
} from "../src/demo/demo-fixtures"

// Behavior contract: khala_mobile.auth.demo_login_example_data.v1
// Owner statement (verbatim): "long-press the Sign in with GitHub button →
// demo login with example data".
//
// This suite is the enforced oracle for that contract. It exercises the
// long-press → demo-session transition and proves every product data source
// serves hardcoded, offline, public-safe example data in demo mode.
//
// NOTE: it asserts the credits/repos/model-preference DEMO GATE at the source
// level (each client short-circuits on `isDemoToken`) rather than by calling
// those functions, because other test files globally `mock.module` those
// clients — a call here would resolve to another test's mock, not the real
// gate.

const mobileRoot = new URL("../", import.meta.url).pathname
const read = (rel: string): string => readFileSync(join(mobileRoot, rel), "utf8")

describe("contract khala_mobile.auth.demo_login_example_data.v1 — reviewer demo login", () => {
  test("the GitHub sign-in button wires a deliberate long-press to demo mode (normal tap unchanged)", () => {
    const source = readFileSync(join(mobileRoot, "src/components/sign-in-screen.tsx"), "utf8")
    // Long-press enters demo mode; a plain tap still starts real GitHub OAuth.
    expect(source).toContain("onLongPress={enterDemoMode}")
    expect(source).toContain("onPress={signInWithGitHub}")
    // A deliberate ~1s hold so an accidental tap never triggers it.
    expect(source).toContain("delayLongPress={1000}")
  })

  test("entering demo mode establishes a signed-in synthetic reviewer session", () => {
    const state = reduceKhalaAuthMachine(initialKhalaAuthMachineState, {
      credentials: DEMO_REVIEWER_CREDENTIALS,
      type: "demo_sign_in_started",
    })
    expect(state.status).toBe("signed_in")
    expect(state.credentials?.token).toBe(DEMO_REVIEWER_TOKEN)
    expect(state.credentials?.ownerUserId).toBe(DEMO_REVIEWER_OWNER_USER_ID)
    expect(isDemoToken(state.credentials?.token ?? "")).toBe(true)
    // A real GitHub token is never treated as demo.
    expect(isDemoToken("gho_realtoken")).toBe(false)
    expect(isDemoToken("")).toBe(false)
  })

  test("thread list + thread messages render from hardcoded fixtures via the scope-entity gate", () => {
    const personalThreads = demoSyncScopeEntities(
      CHAT_THREAD_ENTITY_TYPE,
      String(personalScope(DEMO_REVIEWER_OWNER_USER_ID)),
    )
    expect(personalThreads).toEqual(demoChatThreads)
    expect(personalThreads.length).toBeGreaterThanOrEqual(3)

    const firstThreadId = demoChatThreads[0]!.threadId
    const messages = demoSyncScopeEntities(CHAT_MESSAGE_ENTITY_TYPE, String(threadScope(firstThreadId)))
    expect(messages).toEqual(demoChatMessagesByThread[firstThreadId] ?? [])
    expect(messages.length).toBeGreaterThan(0)

    // The thread entity is served on the thread scope (for the repo chip);
    // runtime events/turns are empty so the plain-chat transcript renders.
    const threadEntity = demoSyncScopeEntities(CHAT_THREAD_ENTITY_TYPE, String(threadScope(firstThreadId)))
    expect(threadEntity).toHaveLength(1)
    expect(demoSyncScopeEntities(RUNTIME_EVENT_ENTITY_TYPE, String(threadScope(firstThreadId)))).toEqual([])
  })

  test("the credits/repos/model-preference clients short-circuit to demo data on the demo token", () => {
    // Each API client gates on isDemoToken(token) before any network call.
    expect(read("src/sync/khala-mobile-credits-api.ts")).toContain("isDemoToken(token)")
    expect(read("src/sync/khala-mobile-repos-api.ts")).toContain("isDemoToken(token)")
    expect(read("src/sync/khala-mobile-model-preference-api.ts")).toContain("isDemoToken(token)")
    // The runtime provider serves an offline demo runtime for the demo token
    // instead of opening a real Khala Sync runtime.
    expect(read("src/sync/khala-mobile-sync-runtime-context.tsx")).toContain("isDemoToken(token)")
    // The sync scope-entity hook (the single gate for threads + messages) reads
    // demoMode and returns fixtures.
    expect(read("src/sync/use-khala-sync-scope-entities.ts")).toContain("demoSyncScopeEntities")
  })

  test("the demo fixtures are realistic, complete example data", () => {
    // Credits: an example balance ($10.00) + a couple of example transactions.
    expect(DEMO_CREDITS_BALANCE_USD_CENTS).toBe(1_000)
    expect(demoCreditsTransactions.length).toBeGreaterThanOrEqual(2)
    // Repos: public + private badges both represented.
    expect(demoRepositories.some(repo => repo.private)).toBe(true)
    expect(demoRepositories.some(repo => !repo.private)).toBe(true)
    // Model config: the single Khala model.
    expect(demoModelPreference()).toEqual({
      availableModelIds: [DEMO_MODEL_ID],
      effectiveModelId: DEMO_MODEL_ID,
      fallback: "none",
      preferredModelId: DEMO_MODEL_ID,
      updatedAt: "2026-07-06T14:00:00Z",
      usedPreference: true,
    })
  })

  test("fixtures are clearly generic and public-safe (no real accounts, repos, tokens, or balances)", () => {
    const blob = JSON.stringify({
      messages: demoChatMessagesByThread,
      repos: demoRepositories,
      threads: demoChatThreads,
    })
    // Everything is namespaced under the obviously-generic demo-user/example-*.
    for (const repo of demoRepositories) {
      expect(repo.owner).toBe("demo-user")
      expect(repo.name.startsWith("example-")).toBe(true)
    }
    // The demo token is an obviously-fake sentinel, never a real bearer token.
    expect(DEMO_REVIEWER_TOKEN.includes("example")).toBe(true)
    // No real OpenAgents owner handles leaked into the fixtures.
    expect(blob).not.toContain("AtlantisPleb")
    expect(blob).not.toContain("openagents/khala-mobile")
  })
})
