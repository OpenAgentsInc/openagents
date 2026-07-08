import { Database } from "bun:sqlite"
import {
  createPylonOrchestrationStore,
  type PylonOrchestrationStore,
  type WorkClaim,
} from "./orchestration/store.js"
import {
  runClaudeAgentTaskCiSmoke,
  type ClaudeAgentTaskSmokeResult,
} from "./claude-agent-task-smoke.js"
import {
  runCodexAgentTaskCiSmoke,
  type CodexAgentTaskSmokeResult,
} from "./codex-agent-task-smoke.js"

/**
 * The mixed two-harness FleetRun exit receipt (MH-2, issue #8583).
 *
 * This is the receipt that proves the multi-harness abstraction BEFORE a third
 * (Grok) adapter lands: one FleetRun (`workerKind: "auto"`) dispatches two work
 * units to two DIFFERENT concrete harnesses — Codex on one, Claude on the
 * other — under ONE claim registry. It asserts the two properties the
 * three-harness plan depends on:
 *
 *   1. Claim uniqueness holds under MIXED kinds. A worker of one harness kind
 *      cannot steal a work unit already live-claimed by a worker of another
 *      kind (the partial unique index on `work_unit_ref` for live claim states
 *      is harness-agnostic). Zero collisions.
 *   2. BOTH closeouts are receipted. Each harness runs its real CI-safe worker
 *      loop (the same executor code the live leg uses, with a mock SDK runner
 *      standing in for the agent, no key/network/spend) and returns an
 *      `accepted`, redacted, no-spend closeout with a closeout ref.
 *
 * CI-safe by construction: the claim registry is a real in-memory
 * `PylonOrchestrationStore`; the two executor runs are the existing
 * `runCodexAgentTaskCiSmoke` / `runClaudeAgentTaskCiSmoke` fixture legs. It is
 * honest about being fixture-simulated — no live device, account, or spend is
 * exercised — while still driving the REAL executor and claim-registry code.
 */

export const MIXED_HARNESS_FLEET_RUN_SMOKE_SCHEMA =
  "openagents.pylon.mixed_harness_fleet_run_smoke.v1"

const FLEET_RUN_REF = "run.public.mixed_harness.ci_smoke"
const WORK_UNIT_CODEX = "work_unit.public.mixed_harness.codex.fixture"
const WORK_UNIT_CLAUDE = "work_unit.public.mixed_harness.claude.fixture"
const CLAIM_TTL_MS = 15 * 60 * 1000

export type MixedHarnessFleetRunHarnessLeg = {
  harnessKind: "codex" | "claude"
  workUnitRef: string
  claimRef: string
  ok: boolean
  closeoutStatus: string | null
  closeoutRef: string | null
  paymentMode: string | null
  settlementState: string | null
  payoutClaimAllowed: boolean | null
  redacted: boolean | null
  redactionViolations: readonly string[]
}

export type MixedHarnessFleetRunSmokeResult = {
  schema: typeof MIXED_HARNESS_FLEET_RUN_SMOKE_SCHEMA
  ok: boolean
  runRef: string
  workerKind: "auto"
  distinctHarnessKinds: readonly string[]
  claimUniqueness: {
    liveClaimsAtPeak: number
    crossKindCollisionsPrevented: number
    doubleLiveClaims: number
  }
  legs: readonly MixedHarnessFleetRunHarnessLeg[]
  blockerRefs: readonly string[]
}

const claimRefFor = (harnessKind: "codex" | "claude"): string =>
  `claim.public.mixed_harness.${harnessKind}.ci_smoke`

const workerAccountRefFor = (harnessKind: "codex" | "claude"): string =>
  `account.pylon.${harnessKind === "claude" ? "claude_agent" : "codex"}.ci_smoke`

/**
 * Register the mixed FleetRun and claim one work unit per harness, then prove
 * that a worker of the OTHER kind cannot steal an already-live-claimed unit.
 * Returns the two live claims plus the uniqueness tallies.
 */
function claimMixedWorkUnits(
  store: PylonOrchestrationStore,
  now: Date,
): {
  codexClaim: WorkClaim
  claudeClaim: WorkClaim
  crossKindCollisionsPrevented: number
  doubleLiveClaims: number
} {
  store.createFleetRun({
    runRef: FLEET_RUN_REF,
    objective: "Prove the mixed two-harness FleetRun abstraction (MH-2).",
    workSource: "fixture",
    targetConcurrency: 2,
    // A mixed run selects `auto`; the typed dumb policy picks a concrete
    // harness per unit. The concrete harness is carried on the per-unit
    // work claim / assignment, not on the run-level worker kind.
    workerKind: "auto",
    state: "running",
    now,
  })

  const codexClaim = store.tryClaimWorkUnit({
    claimRef: claimRefFor("codex"),
    workUnitRef: WORK_UNIT_CODEX,
    runRef: FLEET_RUN_REF,
    workerAccountRef: workerAccountRefFor("codex"),
    ttl: CLAIM_TTL_MS,
    now,
  })
  const claudeClaim = store.tryClaimWorkUnit({
    claimRef: claimRefFor("claude"),
    workUnitRef: WORK_UNIT_CLAUDE,
    runRef: FLEET_RUN_REF,
    workerAccountRef: workerAccountRefFor("claude"),
    ttl: CLAIM_TTL_MS,
    now,
  })
  if (codexClaim === null || claudeClaim === null) {
    throw new Error("mixed-harness smoke: initial per-harness claim unexpectedly rejected")
  }

  // Cross-kind steal attempts: a Claude worker races the Codex-held unit and a
  // Codex worker races the Claude-held unit. Both must be rejected (null).
  let crossKindCollisionsPrevented = 0
  const claudeStealsCodexUnit = store.tryClaimWorkUnit({
    claimRef: "claim.public.mixed_harness.claude.steal_codex",
    workUnitRef: WORK_UNIT_CODEX,
    runRef: FLEET_RUN_REF,
    workerAccountRef: workerAccountRefFor("claude"),
    ttl: CLAIM_TTL_MS,
    now,
  })
  if (claudeStealsCodexUnit === null) crossKindCollisionsPrevented += 1
  const codexStealsClaudeUnit = store.tryClaimWorkUnit({
    claimRef: "claim.public.mixed_harness.codex.steal_claude",
    workUnitRef: WORK_UNIT_CLAUDE,
    runRef: FLEET_RUN_REF,
    workerAccountRef: workerAccountRefFor("codex"),
    ttl: CLAIM_TTL_MS,
    now,
  })
  if (codexStealsClaudeUnit === null) crossKindCollisionsPrevented += 1

  // A double-live-claim would be any work unit holding more than one live
  // claim after the steal attempts. The registry must show exactly zero.
  const doubleLiveClaims = [WORK_UNIT_CODEX, WORK_UNIT_CLAUDE].filter((workUnitRef) => {
    const live = store
      .listWorkClaims({})
      .filter(
        (claim) =>
          claim.workUnitRef === workUnitRef &&
          (claim.state === "claimed" ||
            claim.state === "in_progress" ||
            claim.state === "closeout"),
      )
    return live.length > 1
  }).length

  return { codexClaim, claudeClaim, crossKindCollisionsPrevented, doubleLiveClaims }
}

function legFrom(
  harnessKind: "codex" | "claude",
  workUnitRef: string,
  claimRef: string,
  smoke: CodexAgentTaskSmokeResult | ClaudeAgentTaskSmokeResult,
): MixedHarnessFleetRunHarnessLeg {
  return {
    harnessKind,
    workUnitRef,
    claimRef,
    ok: smoke.ok,
    closeoutStatus: smoke.closeoutStatus,
    closeoutRef: smoke.closeoutRef,
    paymentMode: smoke.boundaryChecks.paymentMode,
    settlementState: smoke.boundaryChecks.settlementState,
    payoutClaimAllowed: smoke.boundaryChecks.payoutClaimAllowed,
    redacted: smoke.boundaryChecks.redacted,
    redactionViolations: smoke.redactionScan.violations,
  }
}

const legReceipted = (leg: MixedHarnessFleetRunHarnessLeg): boolean =>
  leg.ok &&
  leg.closeoutStatus === "accepted" &&
  leg.closeoutRef !== null &&
  leg.paymentMode === "no-spend" &&
  leg.settlementState === "not_applicable" &&
  leg.payoutClaimAllowed === false &&
  leg.redactionViolations.length === 0

export async function runMixedHarnessFleetRunCiSmoke(): Promise<MixedHarnessFleetRunSmokeResult> {
  const now = new Date()
  const store = createPylonOrchestrationStore(new Database(":memory:"))

  const { codexClaim, claudeClaim, crossKindCollisionsPrevented, doubleLiveClaims } =
    claimMixedWorkUnits(store, now)

  const liveClaimsAtPeak = store.listLiveWorkClaims(now).length

  // Both concrete harness worker loops run their CI-safe leg. These are the
  // REAL executors (mock SDK runner) — the same code the live leg uses.
  store.updateWorkClaimState(codexClaim.claimRef, "in_progress", now)
  store.updateWorkClaimState(claudeClaim.claimRef, "in_progress", now)

  const [codexSmoke, claudeSmoke] = await Promise.all([
    runCodexAgentTaskCiSmoke(),
    runClaudeAgentTaskCiSmoke(),
  ])

  // Advance each claim through closeout and release it as the worker loop
  // finishes, mirroring the supervised-dispatch lifecycle.
  for (const [claim, smoke] of [
    [codexClaim, codexSmoke],
    [claudeClaim, claudeSmoke],
  ] as const) {
    if (smoke.assignmentRef !== null) {
      store.updateWorkClaimAssignmentRef(claim.claimRef, smoke.assignmentRef, now)
    }
    store.updateWorkClaimState(claim.claimRef, "closeout", now)
    store.releaseWorkClaim(claim.claimRef, now)
  }

  const legs: MixedHarnessFleetRunHarnessLeg[] = [
    legFrom("codex", WORK_UNIT_CODEX, codexClaim.claimRef, codexSmoke),
    legFrom("claude", WORK_UNIT_CLAUDE, claudeClaim.claimRef, claudeSmoke),
  ]

  const distinctHarnessKinds = [...new Set(legs.map((leg) => leg.harnessKind))]
  const blockerRefs: string[] = []
  if (crossKindCollisionsPrevented !== 2) {
    blockerRefs.push("blocker.mixed_harness.claim_uniqueness_not_proven")
  }
  if (doubleLiveClaims !== 0) {
    blockerRefs.push("blocker.mixed_harness.double_live_claim_detected")
  }
  if (distinctHarnessKinds.length !== 2) {
    blockerRefs.push("blocker.mixed_harness.not_two_distinct_harnesses")
  }
  for (const leg of legs) {
    if (!legReceipted(leg)) {
      blockerRefs.push(`blocker.mixed_harness.${leg.harnessKind}_closeout_not_receipted`)
    }
  }

  return {
    schema: MIXED_HARNESS_FLEET_RUN_SMOKE_SCHEMA,
    ok: blockerRefs.length === 0,
    runRef: FLEET_RUN_REF,
    workerKind: "auto",
    distinctHarnessKinds,
    claimUniqueness: {
      liveClaimsAtPeak,
      crossKindCollisionsPrevented,
      doubleLiveClaims,
    },
    legs,
    blockerRefs,
  }
}
