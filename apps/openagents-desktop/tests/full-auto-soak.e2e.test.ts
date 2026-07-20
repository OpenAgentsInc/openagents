import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  buildFullAutoSoakSm10Summary,
  classifySm10Termination,
  FULL_AUTO_SOAK_SCENARIOS,
  makeCompressedSoakClock,
  makeRealtimeSoakClock,
  runFullAutoSoakMatrix,
  SM10_GATE_TARGET,
  SM10_SUMMARY_SCHEMA,
  type FullAutoSoakRunResult,
} from "./full-auto-soak-harness.ts"
import { FULL_AUTO_MAX_CONTINUATIONS } from "../src/full-auto-reconcile.ts"

/**
 * FA-SOAK-01 (#8992): synthetic long-window soak of Full Auto against stub
 * lanes with an injected fault matrix, plus the SM-10 typed-termination
 * measurement computed from the REAL run-report data structures.
 *
 * The harness (tests/full-auto-soak-harness.ts) drives the actual production
 * machinery -- reconcileFullAutoThreads over the durable registry (lease/
 * cap/backoff/rotation, #8987), openFullAutoRunRegistry +
 * settleFullAutoRunLiveness (FA-RUN-01/03), the real 5-slot thread store,
 * the real local-turn journal, and openFullAutoRunReportStore.sync (#8988)
 * -- with only the provider scripted. Long windows are compressed by
 * injecting the harness clock as `now` everywhere the production modules
 * accept one, so multi-minute FA-H5 backoff sequences elapse instantly.
 *
 * SM-10 gate semantics (deliberate): this suite asserts that every run in
 * THIS synthetic matrix terminates typed (rate 1.0 on the fixture
 * population) and that the summary RECORDS the >= 99% target as the gate
 * value. It does NOT assert the product meets 99% -- that claim belongs to
 * SM-11 owner-AFK dogfood evidence over real lanes (see
 * scripts/full-auto-soak.ts --afk-prep).
 */

const expectedByScenario = new Map(FULL_AUTO_SOAK_SCENARIOS.map(s => [s.id, s]))
const resultFor = (
  results: ReadonlyArray<FullAutoSoakRunResult>,
  scenario: string,
): FullAutoSoakRunResult => {
  const found = results.find(result => result.scenario === scenario)
  if (found === undefined) throw new Error(`missing soak result for scenario ${scenario}`)
  return found
}

describe("FA-SOAK-01: long-window Full Auto soak matrix + SM-10 (#8992)", () => {
  test(
    "compressed-clock soak: the full 10-scenario fault matrix (account exhaustion with and without rotation, provider errors with disable and with recovery, app restart, cache pressure, workspace drift, owner stop, cap, clean completion) terminates typed on every run, and the SM-10 summary records rate 1.0 against the 0.99 gate value",
    async () => {
      const roots: Array<string> = []
      try {
        const { results, summary } = await runFullAutoSoakMatrix({
          makeRoot: scenarioId => {
            const root = mkdtempSync(path.join(tmpdir(), `oa-fa-soak-${scenarioId}-`))
            roots.push(root)
            return root
          },
          makeClock: () => makeCompressedSoakClock(),
        })

        // Machine-readable SM-10 summary -- one greppable stdout line.
        console.log(`[full-auto-soak] sm10-summary ${JSON.stringify(summary)}`)

        // The matrix is complete: every defined scenario ran exactly once.
        expect(results).toHaveLength(FULL_AUTO_SOAK_SCENARIOS.length)
        expect(results).toHaveLength(10)

        // Every soak run terminated TYPED, in the exact expected class --
        // asserting per-scenario means a regression names the scenario and
        // the classification drift in the diff.
        for (const result of results) {
          const scenario = expectedByScenario.get(result.scenario)!
          expect({ scenario: result.scenario, classification: result.classification })
            .toEqual({ scenario: result.scenario, classification: scenario.expected })
          expect(result.classification).not.toBe("untyped")
          // Terminal facts come from the run report's own typed fields
          // (#8988): terminal state, endedAt stamped, stop attributed, and
          // the local-only metrics counters marking the stop as attributed.
          expect(result.stopAttribution).not.toBeNull()
          expect(result.report.endedAt).toBeDefined()
          expect(result.report.metricsEnabled).toBe(true)
          expect(result.report.metrics?.stopAttributed).toBe(true)
        }

        // SM-10: on this fixture population the typed-termination rate is
        // exactly 1.0. The 0.99 target rides along as the recorded gate
        // VALUE (a product claim only SM-11 live evidence can make).
        expect(summary.schema).toBe(SM10_SUMMARY_SCHEMA)
        expect(summary.population).toBe(10)
        expect(summary.sm10.typedTerminations).toBe(10)
        expect(summary.sm10.untypedTerminations).toBe(0)
        expect(summary.sm10.typedTerminationRate).toBe(1)
        expect(summary.sm10.gate.target).toBe(SM10_GATE_TARGET)
        expect(summary.sm10.gate.comparator).toBe(">=")
        expect(summary.classCounts).toEqual({
          objective_complete: 5,
          owner_stop: 1,
          cap: 1,
          guardrail_policy_block: 1,
          fa_h5_disable: 2,
          untyped: 0,
        })

        // -- Scenario-specific durable evidence ------------------------------

        // Rotation (#8987): account exhaustion on the bound lane rotated to
        // the alternate candidate WITHOUT consuming FA-H5 budget, the durable
        // rotation fact reached the run report's passthrough section, and the
        // run completed on the rotated lane.
        const rotation = resultFor(results, "account_exhausted_rotation")
        expect(rotation.rotations).toBe(1)
        expect(rotation.dispatchFailures).toBe(0)
        expect(rotation.report.rotationHistory).toEqual([
          expect.objectContaining({
            fromLane: "codex-local",
            toLane: "claude-local",
            reason: "account_exhausted",
          }),
        ])
        expect(rotation.report.threadFailureHistory?.consecutiveFailures).toBe(0)
        expect(rotation.continuations).toBe(3)

        // FA-H5 disable paths: exactly 5 typed dispatch failures, the thread
        // record disabled by dispatch_failure_limit, and the run report
        // carrying the typed failure history.
        for (const scenarioId of ["account_exhausted_no_alternate", "provider_error_fa_h5_disable"]) {
          const result = resultFor(results, scenarioId)
          expect(result.state).toBe("failed")
          expect(result.stopAttribution).toBe("dispatch_failure_limit")
          expect(result.dispatchFailures).toBe(5)
          expect(result.events.dispatchFailures.at(-1)?.disabled).toBe(true)
          expect(result.report.threadFailureHistory?.consecutiveFailures).toBe(5)
          expect(result.report.threadFailureHistory?.disabledBy).toBe("dispatch_failure_limit")
        }
        expect(
          resultFor(results, "account_exhausted_no_alternate").report.threadFailureHistory
            ?.blockedReason,
        ).toBe("Codex account usage limit reached for this window.")

        // Transient provider errors: two failures entered backoff, the
        // provider recovered, and the run still completed typed.
        const transient = resultFor(results, "provider_error_transient_recovery")
        expect(transient.dispatchFailures).toBe(2)
        expect(transient.state).toBe("completed")
        expect(transient.report.threadFailureHistory?.consecutiveFailures).toBe(0)

        // Workspace drift: FA-H2 blocked typed -- workspace_guard
        // attribution, workspace_mismatch reason, no silent redirect.
        const drift = resultFor(results, "workspace_drift_block")
        expect(drift.stopAttribution).toBe("workspace_guard")
        expect(drift.events.workspaceBlocks).toEqual([
          expect.objectContaining({ reason: "workspace_mismatch" }),
        ])
        expect(drift.report.threadFailureHistory?.blockedReason).toBe("workspace_mismatch")
        expect(drift.report.threadFailureHistory?.disabledBy).toBe("workspace_guard")

        // Restart: one cold reopen of every durable store mid-run, the SAME
        // runRef resumed, and exactly one report spans the restart.
        const restart = resultFor(results, "app_restart_mid_run")
        expect(restart.restarts).toBe(1)
        expect(restart.continuations).toBe(3)
        expect(restart.reportRevision).toBeGreaterThanOrEqual(4)

        // Cap: the loop ran exactly the cap's worth of continuations and the
        // cap (not an error) stopped it.
        const cap = resultFor(results, "cap_exhaustion")
        expect(cap.continuations).toBe(FULL_AUTO_MAX_CONTINUATIONS)
        expect(cap.state).toBe("cap_reached")
        expect(cap.stopAttribution).toBe("continuation_cap")
        expect(cap.events.capStops).toEqual([cap.threadRef])

        // Cache pressure: the run survived real 5-slot eviction pressure and
        // completed all three continuations.
        const pressure = resultFor(results, "cache_pressure")
        expect(pressure.continuations).toBe(3)
        expect(pressure.state).toBe("completed")
        expect(pressure.dispatchFailures).toBe(0)

        // Owner stop: control-API attribution on the terminal edge.
        const stop = resultFor(results, "owner_stop_mid_run")
        expect(stop.state).toBe("stopped")
        expect(stop.stopAttribution).toBe("control_api")
        expect(stop.continuations).toBe(2)
      } finally {
        for (const root of roots) rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "realtime smoke path: the clean-completion scenario runs under the real wall clock (the scripts/full-auto-soak.ts --smoke clock) and terminates typed",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-fa-soak-realtime-"))
      try {
        const scenario = FULL_AUTO_SOAK_SCENARIOS.find(s => s.id === "clean_objective_completion")!
        const { results, summary } = await runFullAutoSoakMatrix({
          scenarios: [scenario],
          makeRoot: () => root,
          makeClock: () => makeRealtimeSoakClock(),
        })
        expect(results[0]!.clockMode).toBe("realtime")
        expect(results[0]!.classification).toBe("objective_complete")
        expect(summary.clockMode).toBe("realtime")
        expect(summary.sm10.typedTerminationRate).toBe(1)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "backoff-dependent scenarios refuse the realtime clock rather than silently fake compressed time",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-fa-soak-refuse-"))
      try {
        const scenario = FULL_AUTO_SOAK_SCENARIOS.find(
          s => s.id === "provider_error_fa_h5_disable",
        )!
        await expect(
          runFullAutoSoakMatrix({
            scenarios: [scenario],
            makeRoot: () => root,
            makeClock: () => makeRealtimeSoakClock(),
          }),
        ).rejects.toThrow(/requires the compressed clock/)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "SM-10 classifier honesty: the measurement can FAIL -- non-terminal runs, unattributed terminal runs, and unrecognized actors all classify untyped, and an untyped run drags the summary rate below the gate",
    () => {
      // A run that never terminated is untyped, whatever its shape.
      expect(classifySm10Termination({ state: "running", stopAttribution: null })).toBe("untyped")
      expect(classifySm10Termination({ state: "stalled", stopAttribution: "liveness_monitor" }))
        .toBe("untyped")
      // A terminal run without attribution is untyped -- the report's honest
      // absence (never guessed) must not count as a typed stop.
      expect(classifySm10Termination({ state: "failed", stopAttribution: null })).toBe("untyped")
      expect(classifySm10Termination({ state: "stopped", stopAttribution: undefined })).toBe("untyped")
      // A terminal state under an actor the vocabulary does not recognize
      // for that state is untyped, not coerced into the nearest class.
      expect(classifySm10Termination({ state: "failed", stopAttribution: "thread_state_sync" }))
        .toBe("untyped")
      expect(classifySm10Termination({ state: "stopped", stopAttribution: "workspace_guard" }))
        .toBe("untyped")
      expect(classifySm10Termination({ state: "cap_reached", stopAttribution: "owner_ui" }))
        .toBe("untyped")
      // The typed classes, for contrast.
      expect(classifySm10Termination({ state: "completed", stopAttribution: "owner_ui" }))
        .toBe("objective_complete")
      expect(classifySm10Termination({ state: "stopped", stopAttribution: "control_api" }))
        .toBe("owner_stop")
      expect(classifySm10Termination({ state: "cap_reached", stopAttribution: "continuation_cap" }))
        .toBe("cap")
      expect(classifySm10Termination({ state: "failed", stopAttribution: "workspace_guard" }))
        .toBe("guardrail_policy_block")
      expect(classifySm10Termination({ state: "failed", stopAttribution: "dispatch_failure_limit" }))
        .toBe("fa_h5_disable")

      const summary = buildFullAutoSoakSm10Summary(
        [
          {
            scenario: "synthetic_typed",
            runRef: "run.full-auto.synthetic.1",
            state: "completed",
            stopAttribution: "owner_ui",
            classification: "objective_complete",
          },
          {
            scenario: "synthetic_untyped",
            runRef: "run.full-auto.synthetic.2",
            state: "failed",
            stopAttribution: null,
            classification: "untyped",
          },
        ],
        { clockMode: "compressed", generatedAt: new Date().toISOString() },
      )
      expect(summary.sm10.typedTerminationRate).toBe(0.5)
      expect(summary.sm10.typedTerminationRate!).toBeLessThan(summary.sm10.gate.target)
      expect(summary.sm10.untypedTerminations).toBe(1)
      // And an empty population reports null, never a fabricated rate.
      const empty = buildFullAutoSoakSm10Summary([], {
        clockMode: "compressed",
        generatedAt: new Date().toISOString(),
      })
      expect(empty.sm10.typedTerminationRate).toBeNull()
    },
  )
})
