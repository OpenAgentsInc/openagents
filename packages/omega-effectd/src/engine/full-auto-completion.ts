import {
  admitFullAutoCompletion,
  deriveFullAutoVerificationSpec,
  fullAutoCompletionBlockReason,
  runFullAutoVerification,
  type FullAutoVerificationEvidencePresent,
  type FullAutoVerificationExec,
  type FullAutoVerificationResult,
  type FullAutoVerificationSpec,
} from "./full-auto-verification.ts"
import {
  asHostExecutedVerification,
  buildFullAutoRunEvidence,
  type FullAutoRunEvidence,
  type FullAutoWorkspaceProbe,
} from "./full-auto-evidence.ts"
import {
  isFullAutoRunAutonomyEnabled,
  type FullAutoRun,
  type FullAutoRunRegistry,
} from "./full-auto-run-registry.ts"

/**
 * HANDS-2 (#9173): host-executed done-condition verification WIRED into the
 * run-completion transition. `full-auto-verification.ts` holds the pure engine
 * (spec derivation, execution, the pass-only admission gate); this module is
 * the thin host seam that ties it to the durable `FullAutoRunRegistry`:
 *
 *  1. Resolve the run's verification spec (an explicit typed spec on the
 *     autonomy block wins; otherwise the structured `verify:` marker in the
 *     done condition is extracted -- bounded-field parsing, never a keyword
 *     guess at intent). No structured check -> spec `none` -> `absent`.
 *  2. Execute it in the run's BOUND workspace via the injected executor (the
 *     real host binds `makeNodeVerificationExec`).
 *  3. Record the host verdict on the run (SEPARATE from the provider
 *     self-report, via `recordVerification`), then admit completion ONLY on a
 *     PASSED verdict; a failed/absent/error verdict keeps the run active with a
 *     typed block reason.
 *
 * A provider "self-report of done" is only EVIDENCE that the host should run
 * this gate (see `detectFullAutoSelfReportedCompletion`); it never completes a
 * run on its own. The gate runs only for an autonomy-enabled run.
 */

export type FullAutoCompletionAdmission =
  | Readonly<{
      /** The host verified the done condition and transitioned the run to
       * `completed`. */
      outcome: "admitted"
      result: FullAutoVerificationResult
      run: FullAutoRun
      /**
       * OMEGA-MOB-31-03 (omega#47): the chain the host stamped for this finished
       * unit, or null when it could not measure one honestly (no turn ref, no
       * workspace probe, an unmeasurable workspace, or a hop that failed the
       * public-safety bound). Null is an honest absence: the projection reports
       * the chain as unavailable rather than showing an invented hop.
       */
      evidence: FullAutoRunEvidence | null
    }>
  | Readonly<{
      /** The host ran (or attempted) verification, recorded the verdict, and
       * kept the run active. `blockReason` is the typed reason completion was
       * not admitted. */
      outcome: "blocked"
      blockReason: string
      result: FullAutoVerificationResult
      run: FullAutoRun
    }>
  | Readonly<{
      /** No verification ran: the run is not autonomy-enabled, is missing, or
       * was already terminal. The run is untouched. */
      outcome: "skipped"
      reason: "autonomy_disabled" | "run_not_found" | "already_terminal" | "transition_refused"
      run: FullAutoRun | null
    }>

/**
 * Resolve the verification spec for a run: an explicit typed spec on the
 * autonomy block takes precedence over a structured marker extracted from the
 * done condition. The workspace ref becomes the command cwd so the check runs
 * in the run's bound workspace.
 */
export const resolveFullAutoRunVerificationSpec = (
  run: FullAutoRun,
  workspaceRef?: string,
): FullAutoVerificationSpec => {
  if (run.autonomy?.verification !== undefined) return run.autonomy.verification
  const cwd = workspaceRef ?? run.workspaceRef
  return deriveFullAutoVerificationSpec(run.doneCondition, cwd === undefined ? {} : { cwd })
}

export type AdmitFullAutoRunCompletionInput = Readonly<{
  registry: FullAutoRunRegistry
  run: FullAutoRun
  /** Overrides the run's own workspaceRef as the verification cwd. */
  workspaceRef?: string
  /** Required for a `command` spec; the host binds `makeNodeVerificationExec`. */
  exec?: FullAutoVerificationExec
  /** Required for an `evidence_ref` spec. */
  evidencePresent?: FullAutoVerificationEvidencePresent
  timeoutMs?: number
  now?: () => Date
  /** Reason recorded on the completion transition. */
  reason?: string
  /**
   * OMEGA-MOB-31-03 (omega#47): the HOST's own journal ref for the turn whose
   * self-reported completion triggered this verification. The caller reads it
   * from the turn journal it already owns -- it is host state, and there is no
   * path from a control-API body, CLI flag, MCP call, or mobile intent to it.
   * Absent means no evidence chain is stamped rather than a guessed turn.
   */
  turnRef?: string
  /**
   * OMEGA-MOB-31-03 (omega#47): the host's own workspace reader. Production
   * binds `makeNodeWorkspaceProbe()`; tests inject a deterministic stub.
   * Absent means no evidence chain is stamped.
   */
  workspaceProbe?: FullAutoWorkspaceProbe
}>

/**
 * Run the host verification for an autonomy run's claimed completion and only
 * admit completion on a PASSED verdict. Always records the verdict (except when
 * skipped), never throws (a spawn failure is an `error` verdict), and never
 * transitions on anything but a pass. The `blocked` and `admitted` outcomes both
 * carry the freshly-read run so callers see the recorded verdict.
 */
export const admitFullAutoRunCompletion = async (
  input: AdmitFullAutoRunCompletionInput,
): Promise<FullAutoCompletionAdmission> => {
  const { registry } = input
  if (!isFullAutoRunAutonomyEnabled(input.run)) {
    return { outcome: "skipped", reason: "autonomy_disabled", run: input.run }
  }
  // Re-read fresh: another pass may have already terminated this run.
  const current = registry.get(input.run.runRef) ?? input.run
  if (current.state === "completed" || current.state === "failed" || current.state === "stopped" || current.state === "cap_reached") {
    return { outcome: "skipped", reason: "already_terminal", run: current }
  }

  const spec = resolveFullAutoRunVerificationSpec(current, input.workspaceRef)
  const workspaceRef = input.workspaceRef ?? current.workspaceRef
  const result = await runFullAutoVerification({
    spec,
    ...(input.exec === undefined ? {} : { exec: input.exec }),
    ...(input.evidencePresent === undefined ? {} : { evidencePresent: input.evidencePresent }),
    ...(workspaceRef === undefined ? {} : { workspaceRef }),
    ...(input.timeoutMs === undefined ? {} : { timeoutMs: input.timeoutMs }),
    ...(input.now === undefined ? {} : { now: input.now }),
  })

  const recorded = registry.recordVerification(current.runRef, result) ?? current

  if (!admitFullAutoCompletion(result)) {
    return {
      outcome: "blocked",
      blockReason: fullAutoCompletionBlockReason(result) ?? "host_verification_not_passed",
      result,
      run: recorded,
    }
  }

  // Attributed to the host control layer (`control_api`): a host verification
  // admitted this completion programmatically, never an owner click. The real,
  // distinct evidence is the durable `autonomy.lastVerification` verdict
  // (recorded above), which the grading lane reads -- not the transition actor.
  const transition = registry.transition(current.runRef, {
    to: "completed",
    actor: "control_api",
    reason: input.reason ?? "host verified the done condition (verification passed)",
  })
  if (!transition.ok) {
    // A legal-transition race (e.g. the run was concurrently paused/stopped):
    // the verdict is still recorded; do not force completion.
    return { outcome: "skipped", reason: "transition_refused", run: recorded }
  }

  // OMEGA-MOB-31-03 (omega#47): the finished unit exists exactly here, and only
  // here does the host hold every hop at once -- the objective it verified
  // against, the turn that triggered it, the verdict its own child process
  // produced, and the workspace it produced that verdict in. Stamping anywhere
  // else would mean reconstructing at least one hop after the fact.
  const evidence = await stampFullAutoRunEvidence({
    registry,
    run: transition.run,
    result,
    ...(workspaceRef === undefined ? {} : { workspaceRef }),
    ...(input.turnRef === undefined ? {} : { turnRef: input.turnRef }),
    ...(input.workspaceProbe === undefined ? {} : { workspaceProbe: input.workspaceProbe }),
    ...(input.now === undefined ? {} : { now: input.now }),
  })
  return {
    outcome: "admitted",
    result,
    run: registry.get(transition.run.runRef) ?? transition.run,
    evidence,
  }
}

/**
 * Build and durably stamp the finished unit's chain, or record nothing.
 *
 * Every early return is an honest absence rather than a fabricated hop:
 * `asHostExecutedVerification` refuses a verdict the host did not itself
 * execute, a missing turn ref or workspace probe means the host cannot name
 * which turn or measure which tree, and `buildFullAutoRunEvidence` returns null
 * when any hop fails the public-safety bound the phone-side reader applies.
 */
const stampFullAutoRunEvidence = async (
  input: Readonly<{
    registry: FullAutoRunRegistry
    run: FullAutoRun
    result: FullAutoVerificationResult
    workspaceRef?: string
    turnRef?: string
    workspaceProbe?: FullAutoWorkspaceProbe
    now?: () => Date
  }>,
): Promise<FullAutoRunEvidence | null> => {
  const executed = asHostExecutedVerification(input.result)
  if (executed === null) return null
  if (input.turnRef === undefined || input.workspaceProbe === undefined) return null
  const workspaceRef = input.workspaceRef ?? input.run.workspaceRef
  if (workspaceRef === undefined) return null
  const baseline = input.run.workspaceBaseline ?? null
  const measurement = await input.workspaceProbe({
    workspaceRef,
    ...(baseline === null ? {} : { baselineRef: baseline.headRef }),
  })
  if (measurement === null) return null
  const evidence = buildFullAutoRunEvidence({
    runRef: input.run.runRef,
    objective: input.run.objective,
    turnRef: input.turnRef,
    measurement,
    baseline,
    verification: executed,
    recordedAtMs: (input.now ?? (() => new Date()))().getTime(),
  })
  if (evidence === null) return null
  input.registry.recordEvidence(input.run.runRef, evidence)
  return evidence
}
