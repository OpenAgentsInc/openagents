import { describe, expect, test } from "vite-plus/test"

import { Schema } from "effect"

import { FullAutoControlRecordSchema } from "./full-auto-control-contract.ts"
import {
  FULL_AUTO_ROUTING_POLICY_LIMIT,
  type FullAutoRoutingCandidate,
} from "./full-auto-registry.ts"
import {
  makeFullAutoRoutingLaneGate,
  validateFullAutoRoutingPolicy,
  type FullAutoRoutingLaneGate,
} from "./full-auto-routing.ts"
import type { ProviderLaneCapabilityReport } from "./provider-lane-capabilities.ts"
import { gooseCapabilities } from "./goose-lane.ts"
import { opencodeCapabilities } from "./opencode-local-runtime.ts"
import { piCapabilities } from "./pi-local-runtime.ts"

/**
 * FA-RT-01 (#8987): the routing policy is admitted or refused as a WHOLE at
 * validation time. Unknown, unadmitted, or Full-Auto-ineligible lanes fail
 * closed here -- never at dispatch, and never by silently filtering the
 * candidate list.
 */

const admitAll: FullAutoRoutingLaneGate = () => ({ admitted: true, fullAuto: true })

const candidates = (...lanes: ReadonlyArray<string>): ReadonlyArray<FullAutoRoutingCandidate> =>
  lanes.map(lane => ({ lane }))

describe("Full Auto routing-policy validation (FA-RT-01 #8987)", () => {
  test("admits an ordered policy over the four existing Full Auto lanes, preserving order", () => {
    const policy = candidates("codex-local", "claude-local", "acp:grok-cli", "acp:cursor-agent")
    const verdict = validateFullAutoRoutingPolicy(policy, admitAll)
    expect(verdict).toEqual({ ok: true, policy })
  })

  test("admits per-lane account refs and treats same-lane different-account entries as distinct candidates", () => {
    const policy: ReadonlyArray<FullAutoRoutingCandidate> = [
      { lane: "codex-local", accountRef: "codex-1" },
      { lane: "codex-local", accountRef: "codex-2" },
      { lane: "claude-local" },
    ]
    expect(validateFullAutoRoutingPolicy(policy, admitAll)).toEqual({ ok: true, policy })
  })

  test("refuses an empty policy (fail closed, never treated as legacy single-lane by the validator)", () => {
    expect(validateFullAutoRoutingPolicy([], admitAll)).toEqual({ ok: false, reason: "policy_empty" })
  })

  test(`refuses a policy longer than the ${FULL_AUTO_ROUTING_POLICY_LIMIT}-candidate bound`, () => {
    const policy: ReadonlyArray<FullAutoRoutingCandidate> = Array.from(
      { length: FULL_AUTO_ROUTING_POLICY_LIMIT + 1 },
      (_, index) => ({ lane: "codex-local", accountRef: `codex-${index}` }),
    )
    expect(validateFullAutoRoutingPolicy(policy, admitAll)).toEqual({
      ok: false,
      reason: "policy_too_long",
      limit: FULL_AUTO_ROUTING_POLICY_LIMIT,
    })
  })

  test("refuses an exact duplicate candidate (same lane AND same account)", () => {
    expect(validateFullAutoRoutingPolicy(
      [{ lane: "codex-local", accountRef: "codex-1" }, { lane: "codex-local", accountRef: "codex-1" }],
      admitAll,
    )).toEqual({ ok: false, reason: "duplicate_candidate", lane: "codex-local" })
  })

  test("refuses an UNKNOWN lane (no live capability report) fail closed, naming the lane", () => {
    const gate: FullAutoRoutingLaneGate = laneRef =>
      laneRef === "codex-local" ? { admitted: true, fullAuto: true } : null
    expect(validateFullAutoRoutingPolicy(candidates("codex-local", "acp:ghost-cli"), gate)).toEqual({
      ok: false,
      reason: "lane_unknown",
      lane: "acp:ghost-cli",
    })
  })

  test("refuses a known-but-Full-Auto-ineligible lane (no lane policy) even when its report is admitted", () => {
    // The gate knows the lane (a report exists) but full-auto-lane.ts has no
    // policy entry for it -- exactly the fail-closed FULL_AUTO_LANE_POLICIES
    // contract for lanes that never proved background-question settlement.
    expect(validateFullAutoRoutingPolicy(candidates("acp:experimental-lane"), admitAll)).toEqual({
      ok: false,
      reason: "lane_not_full_auto_eligible",
      lane: "acp:experimental-lane",
    })
  })

  test("refuses a quarantined lane and a lane that does not advertise fullAuto (not admitted)", () => {
    const quarantinedGate: FullAutoRoutingLaneGate = () => ({ admitted: false, fullAuto: true })
    expect(validateFullAutoRoutingPolicy(candidates("codex-local"), quarantinedGate)).toEqual({
      ok: false,
      reason: "lane_not_admitted",
      lane: "codex-local",
    })
    const noFullAutoGate: FullAutoRoutingLaneGate = () => ({ admitted: true, fullAuto: false })
    expect(validateFullAutoRoutingPolicy(candidates("claude-local"), noFullAutoGate)).toEqual({
      ok: false,
      reason: "lane_not_admitted",
      lane: "claude-local",
    })
  })

  test("the first refusal wins: a policy is never silently narrowed to its admissible prefix", () => {
    const gate: FullAutoRoutingLaneGate = laneRef =>
      laneRef === "acp:grok-cli" ? { admitted: false, fullAuto: false } : { admitted: true, fullAuto: true }
    const verdict = validateFullAutoRoutingPolicy(
      candidates("codex-local", "acp:grok-cli", "claude-local"),
      gate,
    )
    expect(verdict.ok).toBe(false)
    expect(verdict).toMatchObject({ reason: "lane_not_admitted", lane: "acp:grok-cli" })
  })
})

describe("Full Auto routing lane gate over live capability reports (FA-RT-01 #8987)", () => {
  const report = (extra: Partial<ProviderLaneCapabilityReport> = {}): ProviderLaneCapabilityReport => ({
    laneRef: "codex-local",
    provider: "codex",
    models: ["gpt-5.5"],
    features: {
      skills: false,
      planOnly: false,
      reasoningEffort: true,
      images: true,
      fullAuto: true,
      interrupt: true,
      queueFollowup: true,
      steerTurn: true,
      steerChild: false,
      answerQuestion: true,
    },
    composer: {
      displayName: "Codex",
      reasoningEfforts: ["low", "medium", "high", "xhigh"],
      permissionModes: ["owner_full"],
      approvals: "host_mediated",
      extensions: [],
    },
    policy: {
      source: "native-static-declaration",
      profileRef: "native:codex-local:v1",
      evidence: "conformant",
      allowedModels: ["gpt-5.5"],
      allowedFeatures: ["reasoningEffort", "images", "fullAuto", "interrupt", "queueFollowup", "steerTurn", "answerQuestion"],
      allowedExtensions: [],
    },
    recovery: "provider_session_replay",
    ...extra,
  })

  test("derives admitted+fullAuto from the same L2 projection main's dispatch gate uses; unknown lanes are null", () => {
    const gate = makeFullAutoRoutingLaneGate(laneRef => laneRef === "codex-local" ? report() : null)
    expect(gate("codex-local")).toEqual({ admitted: true, fullAuto: true })
    expect(gate("acp:ghost-cli")).toBeNull()
  })

  test("the real host-run SDK-harness capability reports project admitted+fullAuto, so a harness fleet policy is admitted (#9187)", () => {
    // The exact production capability reports the three harness lanes ship,
    // routed through the SAME L2 projection main's dispatch gate uses. This is
    // the capability side of #9187: run-start's gate can no longer refuse a
    // harness lane as `lane_not_full_auto_eligible`/`lane_not_admitted`.
    const reports: ReadonlyArray<ProviderLaneCapabilityReport> = [
      opencodeCapabilities,
      piCapabilities,
      gooseCapabilities,
    ]
    const byRef = (laneRef: string): ProviderLaneCapabilityReport | null =>
      reports.find(report => report.laneRef === laneRef) ?? null
    const gate = makeFullAutoRoutingLaneGate(byRef)
    for (const report of reports) {
      // The report advertises fullAuto AND admits it in allowedFeatures, so the
      // projection is admitted (not an over-claim quarantine) and fullAuto=true.
      expect(gate(report.laneRef)).toEqual({ admitted: true, fullAuto: true })
    }
    // An ordered fleet policy across all three harness lanes is admitted whole,
    // order preserved — the run-start rotation priority the CLI `--lane` repeats
    // build (see full-auto-control-server.test.ts).
    const policy = candidates("harness:opencode", "harness:pi", "harness:goose")
    expect(validateFullAutoRoutingPolicy(policy, gate)).toEqual({ ok: true, policy })
  })

  test("an over-claiming (quarantined) report validates as not admitted, so the policy referencing it is refused", () => {
    // fullAuto claimed but not allowed by policy -> the L2 projection
    // quarantines the WHOLE lane.
    const lying = report({
      policy: {
        source: "native-static-declaration",
        profileRef: "native:codex-local:v1",
        evidence: "conformant",
        allowedModels: ["gpt-5.5"],
        allowedFeatures: ["reasoningEffort", "images", "interrupt", "queueFollowup", "steerTurn", "answerQuestion"],
        allowedExtensions: [],
      },
    })
    const gate = makeFullAutoRoutingLaneGate(() => lying)
    expect(gate("codex-local")).toEqual({ admitted: false, fullAuto: false })
    expect(validateFullAutoRoutingPolicy([{ lane: "codex-local" }], gate)).toEqual({
      ok: false,
      reason: "lane_not_admitted",
      lane: "codex-local",
    })
  })
})

describe("Full Auto control-record projection carries rotation history (FA-RT-01 #8987)", () => {
  test("the public-safe control record schema admits an OPTIONAL bounded rotationHistory and still decodes records without one", () => {
    const base = {
      threadRef: "thread-r",
      enabled: true,
      continuationCount: 2,
      updatedAt: "2026-07-17T00:00:00.000Z",
      workspaceRef: "/repo/a",
      lane: "codex-local",
      accountRef: null,
      blockedReason: null,
      disabledBy: null,
      disabledAt: null,
      live: { state: "idle", turnRef: null },
    }
    // Absent (every existing server projection): still decodes.
    expect(Schema.decodeUnknownSync(FullAutoControlRecordSchema)(base).rotationHistory).toBeUndefined()
    // Present: typed entries only.
    const decoded = Schema.decodeUnknownSync(FullAutoControlRecordSchema)({
      ...base,
      rotationHistory: [
        { fromLane: "codex-local", toLane: "claude-local", reason: "account_exhausted", at: "2026-07-17T01:00:00.000Z" },
      ],
    })
    expect(decoded.rotationHistory).toEqual([
      { fromLane: "codex-local", toLane: "claude-local", reason: "account_exhausted", at: "2026-07-17T01:00:00.000Z" },
    ])
    // A non-typed reason is refused -- the projection can never smuggle raw
    // provider detail through the reason field.
    expect(() => Schema.decodeUnknownSync(FullAutoControlRecordSchema)({
      ...base,
      rotationHistory: [
        { fromLane: "codex-local", toLane: "claude-local", reason: "raw provider stack trace", at: "t" },
      ],
    })).toThrow()
  })
})
