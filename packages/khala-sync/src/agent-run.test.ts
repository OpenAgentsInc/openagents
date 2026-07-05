import { describe, expect, test } from "bun:test"
import {
  AGENT_RUN_ENTITY_TYPE,
  canonicalJson,
  decodeAgentRunEntity,
  encodeAgentRunEntity,
} from "./index.js"

/**
 * Agent run + goal entity contract (KS-6.6, #8416). The load-bearing property
 * here is SPEC §7 invariant 9: ref-typed fields (runId, routeId, userId,
 * teamId, projectId, goalId) must structurally REFUSE emails, filesystem
 * paths, and whitespace, mirroring every other khala-sync entity contract.
 * `goal` (the user's free-text objective) and `repository.owner`/`repo`/
 * `ref` are deliberately content fields — bounded but otherwise
 * unconstrained, same exemption `KhalaCodeText`/chat `body` gets in
 * ./khala-code.ts — so a real repo/goal string is never rejected.
 */

const validRun = {
  backend: "shc_vm",
  canceledAt: null,
  completedAt: null,
  createdAt: "2026-07-05T12:00:00.000Z",
  failedAt: null,
  goal: "Run a bounded repo cleanup mission.",
  goalContext: {
    goalId: "goal.alpha",
    objective: "Run a bounded repo cleanup mission.",
    remainingTokens: 50_000,
    status: "active",
    timeUsedSeconds: 0,
    tokenBudget: 100_000,
    tokensUsed: 0,
    visibility: "private",
  },
  goalId: "goal.alpha",
  projectId: null,
  repository: {
    owner: "OpenAgentsInc",
    provider: "github",
    ref: "main",
    repo: "openagents",
  },
  routeId: "agent_run_abc123",
  runId: "run.alpha",
  runtime: "opencode_codex",
  startedAt: null,
  status: "queued",
  teamId: null,
  updatedAt: "2026-07-05T12:00:00.000Z",
  userId: "user.alice",
}

describe("agent run entity contract", () => {
  test("entity type is the expected constant", () => {
    expect(AGENT_RUN_ENTITY_TYPE).toBe("agent_run")
  })

  test("decodes and re-encodes a queued run with an attached goal", () => {
    const entity = decodeAgentRunEntity(validRun)
    expect(entity.runId).toBe("run.alpha")
    expect(entity.status).toBe("queued")
    expect(entity.goalContext?.goalId).toBe("goal.alpha")
    expect(encodeAgentRunEntity(entity).runId).toBe("run.alpha")
  })

  test("decodes a run with NO attached goal (goalContext omitted)", () => {
    const { goalContext, ...withoutGoalContext } = validRun
    const entity = decodeAgentRunEntity({ ...withoutGoalContext, goalId: null })
    expect(entity.goalId).toBeNull()
    expect("goalContext" in entity).toBe(false)
  })

  test("a real branch-name ref and repo/goal text survive the round trip", () => {
    const entity = decodeAgentRunEntity({
      ...validRun,
      goal: "Fix the /Users/alice reference in the README and email support@example.com if blocked.",
      repository: {
        ...validRun.repository,
        ref: "feature/fix-readme-refs",
      },
    })
    expect(entity.repository.ref).toBe("feature/fix-readme-refs")
    expect(entity.goal).toContain("support@example.com")
  })

  test("ref-typed fields structurally refuse emails, paths, and whitespace", () => {
    expect(() =>
      decodeAgentRunEntity({ ...validRun, runId: "/Users/alice/run" }),
    ).toThrow()
    expect(() =>
      decodeAgentRunEntity({ ...validRun, userId: "user@example.com" }),
    ).toThrow()
    expect(() =>
      decodeAgentRunEntity({ ...validRun, goalId: "goal with spaces" }),
    ).toThrow()
  })

  test("rejects an unknown status literal", () => {
    expect(() =>
      decodeAgentRunEntity({ ...validRun, status: "exploded" }),
    ).toThrow()
  })

  test("rejects a negative tokensUsed in goalContext", () => {
    expect(() =>
      decodeAgentRunEntity({
        ...validRun,
        goalContext: { ...validRun.goalContext, tokensUsed: -1 },
      }),
    ).toThrow()
  })

  test("canonicalJson of the encoded entity never spreads unknown fields", () => {
    const entity = decodeAgentRunEntity(validRun)
    const json = canonicalJson(encodeAgentRunEntity(entity))
    expect(json).not.toContain("hiddenSteering")
    expect(json).not.toContain("toolContract")
    expect(json).not.toContain("authGrantRef")
    expect(json).not.toContain("providerAccountRef")
  })
})
