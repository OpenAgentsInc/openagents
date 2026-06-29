import { describe, expect, test } from "bun:test"

import {
  evaluatePolicy,
  type PolicyRequest,
  type PolicySnapshot,
} from "../src/tas/managed-policy"

const snapshot: PolicySnapshot = {
  allowedTeams: ["team.openagents"],
  allowedRepos: ["OpenAgentsInc/openagents"],
  approvedUsers: ["user.approved"],
  allowedProviders: ["codex"],
  blockedProviders: ["blocked-provider"],
  budget: {
    remainingTokens: 1_000,
    remainingCostUsd: 5,
  },
  retention: {
    allowedClasses: ["receipt"],
    maxDays: 30,
  },
  telemetry: {
    allowedModes: ["aggregate"],
  },
}

const request: PolicyRequest = {
  team: "team.openagents",
  repo: "OpenAgentsInc/openagents",
  user: "user.approved",
  provider: "codex",
  estimatedTokens: 500,
  estimatedCostUsd: 2,
  retentionClass: "receipt",
  retentionDays: 14,
  telemetryMode: "aggregate",
}

describe("managed policy core", () => {
  test("allows when all gates pass", () => {
    expect(evaluatePolicy(snapshot, request)).toEqual({
      decision: "allow",
      reason: "policy_allowed",
    })
  })

  test("denies with explicit reason when team is not allowed", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        team: "team.other",
      }),
    ).toEqual({
      decision: "deny",
      reason: "team_not_allowed",
    })
  })

  test("denies with explicit reason when repo is not allowed", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        repo: "OpenAgentsInc/private",
      }),
    ).toEqual({
      decision: "deny",
      reason: "repo_not_allowed",
    })
  })

  test("denies with explicit reason when user is not approved", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        user: "user.unapproved",
      }),
    ).toEqual({
      decision: "deny",
      reason: "user_not_approved",
    })
  })

  test("denies with explicit reason when provider is blocked", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        provider: "blocked-provider",
      }),
    ).toEqual({
      decision: "deny",
      reason: "provider_blocked",
    })
  })

  test("denies with explicit reason when provider is not allowed", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        provider: "gemini",
      }),
    ).toEqual({
      decision: "deny",
      reason: "provider_blocked",
    })
  })

  test("denies with explicit reason when budget is exceeded", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        estimatedTokens: 1_001,
      }),
    ).toEqual({
      decision: "deny",
      reason: "budget_exceeded",
    })
  })

  test("denies with explicit reason when retention is not allowed", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        retentionDays: 31,
      }),
    ).toEqual({
      decision: "deny",
      reason: "retention_not_allowed",
    })
  })

  test("denies with explicit reason when telemetry is not allowed", () => {
    expect(
      evaluatePolicy(snapshot, {
        ...request,
        telemetryMode: "raw",
      }),
    ).toEqual({
      decision: "deny",
      reason: "telemetry_not_allowed",
    })
  })

  test("approved-user gate denies an empty approved-user list", () => {
    expect(
      evaluatePolicy(
        {
          ...snapshot,
          approvedUsers: [],
        },
        request,
      ),
    ).toEqual({
      decision: "deny",
      reason: "user_not_approved",
    })
  })
})
