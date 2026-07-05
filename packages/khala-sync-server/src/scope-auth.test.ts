import {
  agentRunScope,
  fleetRunScope,
  personalScope,
  publicScope,
  SyncScope,
  teamScope,
  threadScope,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import {
  isAnonymousReadableScope,
  type KhalaSyncScopeAuthCapabilities,
  resolveScopeRead,
  type ScopeReadDecision,
} from "./scope-auth.js"

/**
 * KS-7.1 (#8305) auth matrix per scope kind (SPEC §7 invariant 7): the
 * resolver grants exactly the taxonomy's read policy, denies everything
 * else, gates unknown scope kinds CLOSED with the typed `unknown_scope`
 * reason, and turns EVERY capability failure into a typed `unavailable`
 * decision — a throwing capability can never grant.
 */

const USER = "user-a"
const OTHER = "user-b"
const TEAM = "team-1"
const RUN = "run-1"
const THREAD = "thread-1"
const FLEET = "fleet-1"

const unusable = (name: string) => async (): Promise<never> => {
  throw new Error(`${name} capability must not be consulted for this scope`)
}

/** Deterministic in-memory capabilities for the matrix. */
const capabilities = (
  overrides: Partial<KhalaSyncScopeAuthCapabilities> = {},
): KhalaSyncScopeAuthCapabilities => ({
  isTeamMember: async (userId, teamId) => teamId === TEAM && userId === USER,
  canReadAgentRun: async (userId, runId) => runId === RUN && userId === USER,
  canReadThread: async (userId, threadId) =>
    threadId === THREAD && userId === USER,
  readFleetScopeOwner: async (scope) =>
    scope === fleetRunScope(FLEET) ? USER : null,
  ...overrides,
})

const decide = (
  userId: string,
  scope: SyncScope,
  overrides: Partial<KhalaSyncScopeAuthCapabilities> = {},
): Promise<ScopeReadDecision> =>
  resolveScopeRead(capabilities(overrides), userId, scope)

describe("resolveScopeRead auth matrix", () => {
  test("scope.user: self is allowed, a foreign user is denied", async () => {
    expect(await decide(USER, personalScope(USER))).toEqual({
      kind: "allowed",
    })
    expect(await decide(OTHER, personalScope(USER))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("scope.user consults NO capability (pure comparison)", async () => {
    const decision = await resolveScopeRead(
      {
        isTeamMember: unusable("isTeamMember"),
        canReadAgentRun: unusable("canReadAgentRun"),
        canReadThread: unusable("canReadThread"),
        readFleetScopeOwner: unusable("readFleetScopeOwner"),
      },
      USER,
      personalScope(USER),
    )
    expect(decision).toEqual({ kind: "allowed" })
  })

  test("scope.public: every authenticated user is allowed", async () => {
    expect(await decide(USER, publicScope("tokens-served"))).toEqual({
      kind: "allowed",
    })
    expect(await decide(OTHER, publicScope("tokens-served"))).toEqual({
      kind: "allowed",
    })
  })

  test("scope.public: an ANONYMOUS caller (userId undefined) is allowed — consults NO capability", async () => {
    const decision = await resolveScopeRead(
      {
        isTeamMember: unusable("isTeamMember"),
        canReadAgentRun: unusable("canReadAgentRun"),
        canReadThread: unusable("canReadThread"),
        readFleetScopeOwner: unusable("readFleetScopeOwner"),
      },
      undefined,
      publicScope("tokens-served"),
    )
    expect(decision).toEqual({ kind: "allowed" })
  })

  test.each([
    ["scope.user", personalScope(USER)],
    ["scope.team", teamScope(TEAM)],
    ["scope.agent_run", agentRunScope(RUN)],
    ["scope.thread", threadScope(THREAD)],
    ["scope.fleet_run", fleetRunScope(FLEET)],
    ["an unknown taxonomy kind", SyncScope.make("scope.workspace.w-1")],
  ] as const)(
    "SECURITY: an ANONYMOUS caller is denied %s — the ONLY anonymous-readable kind is scope.public (never a grant, never a capability call)",
    async (_label, scope) => {
      const decision = await resolveScopeRead(
        {
          isTeamMember: unusable("isTeamMember"),
          canReadAgentRun: unusable("canReadAgentRun"),
          canReadThread: unusable("canReadThread"),
          readFleetScopeOwner: unusable("readFleetScopeOwner"),
        },
        undefined,
        scope,
      )
      expect(decision.kind).not.toBe("allowed")
    },
  )

  test("scope.team: LIVE member is allowed, non-member is denied", async () => {
    expect(await decide(USER, teamScope(TEAM))).toEqual({ kind: "allowed" })
    expect(await decide(OTHER, teamScope(TEAM))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("scope.team: membership is re-read on every call (revocation bites immediately)", async () => {
    const members = new Set([USER])
    const caps = capabilities({
      isTeamMember: async (userId, teamId) =>
        teamId === TEAM && members.has(userId),
    })
    expect(await resolveScopeRead(caps, USER, teamScope(TEAM))).toEqual({
      kind: "allowed",
    })
    members.delete(USER)
    expect(await resolveScopeRead(caps, USER, teamScope(TEAM))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("scope.agent_run: owner is allowed, foreign user is denied", async () => {
    expect(await decide(USER, agentRunScope(RUN))).toEqual({ kind: "allowed" })
    expect(await decide(OTHER, agentRunScope(RUN))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("scope.thread: owner is allowed, foreign user is denied", async () => {
    expect(await decide(USER, threadScope(THREAD))).toEqual({
      kind: "allowed",
    })
    expect(await decide(OTHER, threadScope(THREAD))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("scope.fleet_run: the khala_sync_scope_owners owner is allowed; a foreign user and an UNOWNED scope are denied", async () => {
    expect(await decide(USER, fleetRunScope(FLEET))).toEqual({
      kind: "allowed",
    })
    expect(await decide(OTHER, fleetRunScope(FLEET))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
    expect(await decide(USER, fleetRunScope("fleet-unowned"))).toEqual({
      kind: "denied",
      reason: "unauthorized_scope",
    })
  })

  test("unknown taxonomy members are gated CLOSED with the typed unknown_scope reason", async () => {
    expect(
      await decide(USER, SyncScope.make("scope.workspace.w-1")),
    ).toEqual({ kind: "denied", reason: "unknown_scope" })
  })

  test.each([
    ["isTeamMember", teamScope(TEAM)],
    ["canReadAgentRun", agentRunScope(RUN)],
    ["canReadThread", threadScope(THREAD)],
    ["readFleetScopeOwner", fleetRunScope(FLEET)],
  ] as const)(
    "a throwing %s capability fails CLOSED as typed unavailable (never a grant, never a silent 403)",
    async (capability, scope) => {
      const decision = await decide(USER, scope, {
        [capability]: async () => {
          throw new Error("connection refused — raw driver text must not leak")
        },
      })
      expect(decision.kind).toBe("unavailable")
      if (decision.kind === "unavailable") {
        expect(decision.messageSafe).not.toContain("connection refused")
        expect(decision.messageSafe).toContain("retry")
      }
    },
  )
})

describe("isAnonymousReadableScope (KS-8.x anonymous-read exception)", () => {
  test("true for every scope.public.* channel", () => {
    expect(isAnonymousReadableScope(publicScope("tokens-served"))).toBe(true)
    expect(isAnonymousReadableScope(publicScope("gym-run-progress"))).toBe(
      true,
    )
    expect(isAnonymousReadableScope(publicScope("settled-feed"))).toBe(true)
  })

  test("false for every other taxonomy kind", () => {
    expect(isAnonymousReadableScope(personalScope(USER))).toBe(false)
    expect(isAnonymousReadableScope(teamScope(TEAM))).toBe(false)
    expect(isAnonymousReadableScope(agentRunScope(RUN))).toBe(false)
    expect(isAnonymousReadableScope(threadScope(THREAD))).toBe(false)
    expect(isAnonymousReadableScope(fleetRunScope(FLEET))).toBe(false)
    expect(
      isAnonymousReadableScope(SyncScope.make("scope.workspace.w-1")),
    ).toBe(false)
  })

  test("SECURITY: a crafted id segment can never be mistaken for the public kind — kind is captured up to the FIRST dot only", () => {
    // "public_evil" is a DIFFERENT kind than "public" (exact match, not a
    // prefix/startsWith/includes check).
    expect(
      isAnonymousReadableScope(SyncScope.make("scope.public_evil.x")),
    ).toBe(false)
    // A "public" SEGMENT nested inside a non-public kind's id must not leak
    // through: the kind is "team", not "public", regardless of what the id
    // portion (after the second dot) contains.
    expect(
      isAnonymousReadableScope(SyncScope.make("scope.team.public.evil")),
    ).toBe(false)
  })
})
