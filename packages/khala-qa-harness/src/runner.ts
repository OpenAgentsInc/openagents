import { Effect } from "effect"
import {
  evaluateKhalaCodeQaMetricBudget,
  type KhalaCodeQaMetricsSnapshot,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

import { collectKhalaCodeQaCoverageLedger, type KhalaCodeQaCoverageLedger } from "./coverage-ledger.js"
import type { KhalaCodeQaDriver, KhalaCodeQaObservation } from "./driver.js"
import { compareKhalaCodeRpcConsistency } from "./rpc-client.js"
import type {
  KhalaCodeQaCommitment,
  KhalaCodeQaOracleExpectation,
  KhalaCodeQaScenario,
  KhalaCodeQaVerdict,
} from "./scenario.js"

export type KhalaCodeQaOracleOutcome = {
  readonly ok: boolean
  readonly oracle: KhalaCodeQaOracleExpectation["oracle"]
  readonly phaseName: string
  readonly summary: string
  readonly verdict: KhalaCodeQaVerdict
  readonly data?: unknown
}

export type KhalaCodeQaPhaseOutcome = {
  readonly name: string
  readonly observations: ReadonlyArray<KhalaCodeQaObservation>
  readonly oracles: ReadonlyArray<KhalaCodeQaOracleOutcome>
  readonly status: "pass" | "fail"
}

export type KhalaCodeQaCommitmentFinding = {
  readonly id: string
  readonly claim: string
  readonly verdict: KhalaCodeQaVerdict
  readonly evidenceSummary: string
}

export type KhalaCodeQaCommitmentReport = {
  readonly verdict: KhalaCodeQaVerdict
  readonly observed: boolean
  readonly findings: ReadonlyArray<KhalaCodeQaCommitmentFinding>
}

export type KhalaCodeQaScenarioRunReport = {
  readonly backend: KhalaCodeQaScenario["backend"]
  readonly commitments: KhalaCodeQaCommitmentReport
  readonly coverageLedger: KhalaCodeQaCoverageLedger
  readonly mode: KhalaCodeQaDriver["mode"]
  readonly phaseOutcomes: ReadonlyArray<KhalaCodeQaPhaseOutcome>
  readonly scenarioId: string
  readonly status: "pass" | "fail"
}

const oracleEvidenceLabel = (phaseName: string, oracle: string): string =>
  `${phaseName}:${oracle}`

const runStatus = (phaseOutcomes: ReadonlyArray<KhalaCodeQaPhaseOutcome>): "pass" | "fail" =>
  phaseOutcomes.every((phase) => phase.status === "pass") ? "pass" : "fail"

const rollUp = (
  findings: ReadonlyArray<KhalaCodeQaCommitmentFinding>,
  status: "pass" | "fail",
): KhalaCodeQaVerdict => {
  if (findings.length === 0) return "INCONCLUSIVE"
  if (findings.some((finding) => finding.verdict === "REFUTED")) return "REFUTED"
  if (findings.some((finding) => finding.verdict === "INCONCLUSIVE")) return "INCONCLUSIVE"
  if (status === "fail") return "REFUTED"
  return "CONFIRMED"
}

const summarizeOracleLabels = (
  oracles: ReadonlyArray<KhalaCodeQaOracleOutcome>,
): string =>
  oracles
    .map((oracle) =>
      `${oracleEvidenceLabel(oracle.phaseName, oracle.oracle)}=${oracle.verdict.toLowerCase()}`
    )
    .join(", ")

const observationMatchesQuery = (
  observation: KhalaCodeQaObservation,
  query: string,
): boolean => {
  if (observation.label === query || observation.label === `read:${query}`) return true
  if (observation.action.kind === "rpc_call") {
    return observation.action.method === query || `rpc:${observation.action.method}` === query
  }
  if (observation.action.kind === "read") {
    return observation.action.query === query || `read:${observation.action.query}` === query
  }
  return false
}

const observationValue = (observation: KhalaCodeQaObservation): unknown => {
  const data = observation.data as
    | { readonly value?: unknown }
    | undefined
  return data?.value ?? observation.data
}

const findObservationValue = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
  query: string,
): unknown | undefined => {
  const observation = [...observations].reverse().find((item) =>
    observationMatchesQuery(item, query)
  )
  return observation === undefined ? undefined : observationValue(observation)
}

const isQaMetricsSnapshot = (value: unknown): value is KhalaCodeQaMetricsSnapshot => {
  if (value === null || typeof value !== "object") return false
  const record = value as Record<string, unknown>
  return record.ok === true &&
    record.schema === "openagents.khala_code.qa_metrics.v1" &&
    Array.isArray(record.budgets) &&
    Array.isArray(record.samples)
}

const findQaMetricsSnapshot = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaMetricsSnapshot | null => {
  for (const observation of [...observations].reverse()) {
    const value = observationValue(observation)
    if (isQaMetricsSnapshot(value)) return value
  }
  return null
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const safeFailureDetail = (failure: unknown): string => {
  if (failure instanceof Error) return failure.message
  if (typeof failure === "string") return failure
  try {
    return JSON.stringify(failure)
  } catch {
    return String(failure)
  }
}

const observedValues = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): readonly unknown[] =>
  observations.map(observationValue)

const fleetRunProjectionValues = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): readonly Record<string, unknown>[] =>
  observedValues(observations).flatMap((value) => {
    if (!isRecord(value)) return []
    const runs = Array.isArray(value.runs) ? value.runs : []
    const candidates = [
      value.run,
      value.projection,
      ...runs,
    ]
    return candidates.filter(isRecord)
  })

type ActiveAssignmentClaim = {
  readonly assignmentRef: string
  readonly claimantRef: string | null
  readonly observationIndex: number
}

const claimantRefFor = (entry: Record<string, unknown>): string | null => {
  const candidates = [
    entry.workerAccountRef,
    entry.accountRef,
    entry.claimingWorkerRef,
    entry.workerRef,
    entry.workerRefHash,
    entry.pylonRef,
  ]
  const candidate = candidates.find((value): value is string =>
    typeof value === "string" && value.length > 0
  )
  return candidate ?? null
}

const activeAssignmentClaimsForValue = (
  value: unknown,
  observationIndex: number,
): readonly ActiveAssignmentClaim[] => {
    if (!isRecord(value)) return []
    const activeAssignments = Array.isArray(value.activeAssignments) ? value.activeAssignments : []
    const results = Array.isArray(value.results) ? value.results : []
    return [...activeAssignments, ...results].flatMap((entry) => {
      if (!isRecord(entry) || typeof entry.assignmentRef !== "string") return []
      return [{
        assignmentRef: entry.assignmentRef,
        claimantRef: claimantRefFor(entry),
        observationIndex,
      }]
    })
}

const activeAssignmentClaimsByObservation = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): readonly (readonly ActiveAssignmentClaim[])[] =>
  observations.map((observation, observationIndex) =>
    activeAssignmentClaimsForValue(observationValue(observation), observationIndex)
  )

const evaluateClaimInvariant = (
  phaseName: string,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  const runs = fleetRunProjectionValues(observations)
  const oversubscribedRuns = runs.flatMap((run) => {
    const counters = isRecord(run.counters) ? run.counters : {}
    const activeAssignments = typeof counters.activeAssignments === "number" ? counters.activeAssignments : 0
    const targetConcurrency = typeof run.targetConcurrency === "number" ? run.targetConcurrency : 0
    return activeAssignments > targetConcurrency
      ? [{ activeAssignments, runRef: run.runRef, targetConcurrency }]
      : []
  })
  const claimsByObservation = activeAssignmentClaimsByObservation(observations)
  const duplicateRefs = [...new Set(claimsByObservation.flatMap((claims) => {
    const refs = claims.map((claim) => claim.assignmentRef)
    return refs.filter((ref, index) => refs.indexOf(ref) !== index)
  }))].sort()
  const claimantsByAssignment = new Map<string, Set<string>>()
  for (const claim of claimsByObservation.flat()) {
    if (claim.claimantRef === null) continue
    const claimants = claimantsByAssignment.get(claim.assignmentRef) ?? new Set<string>()
    claimants.add(claim.claimantRef)
    claimantsByAssignment.set(claim.assignmentRef, claimants)
  }
  const conflictingClaimants = [...claimantsByAssignment.entries()]
    .flatMap(([assignmentRef, claimants]) =>
      claimants.size > 1 ? [{ assignmentRef, claimants: [...claimants].sort() }] : []
    )
    .sort((left, right) => left.assignmentRef.localeCompare(right.assignmentRef))
  const ok = oversubscribedRuns.length === 0 && duplicateRefs.length === 0 && conflictingClaimants.length === 0
  return {
    data: { conflictingClaimants, duplicateRefs, oversubscribedRuns },
    ok,
    oracle: "invariant",
    phaseName,
    summary: ok
      ? "fleet claim invariant held: no duplicate assignment refs, conflicting claimants, or oversubscribed FleetRun counters"
      : "fleet claim invariant failed",
    verdict: ok ? "CONFIRMED" : "REFUTED",
  }
}

const verifyCommitments = (input: {
  readonly commitments: ReadonlyArray<KhalaCodeQaCommitment>
  readonly phaseOutcomes: ReadonlyArray<KhalaCodeQaPhaseOutcome>
  readonly runStatus: "pass" | "fail"
}): KhalaCodeQaCommitmentReport => {
  const allOracles = input.phaseOutcomes.flatMap((phase) => phase.oracles)
  const findings = input.commitments.map((commitment): KhalaCodeQaCommitmentFinding => {
    if (commitment.evidence === "run-pass") {
      return input.runStatus === "pass"
        ? {
            claim: commitment.claim,
            evidenceSummary: "run completed with status=pass",
            id: commitment.id,
            verdict: "CONFIRMED",
          }
        : {
            claim: commitment.claim,
            evidenceSummary: "run completed with status=fail",
            id: commitment.id,
            verdict: "REFUTED",
          }
    }

    const matchingOracles = allOracles.filter((outcome) =>
      oracleEvidenceLabel(outcome.phaseName, outcome.oracle)
        .toLowerCase()
        .includes(commitment.match.toLowerCase()),
    )
    if (matchingOracles.length === 0) {
      return {
        claim: commitment.claim,
        evidenceSummary: `no oracle outcome matched "${commitment.match}"`,
        id: commitment.id,
        verdict: "INCONCLUSIVE",
      }
    }
    const evidenceSummary = `observed oracles: ${summarizeOracleLabels(matchingOracles)}`
    if (matchingOracles.some((oracle) => oracle.verdict === "REFUTED")) {
      return {
        claim: commitment.claim,
        evidenceSummary,
        id: commitment.id,
        verdict: "REFUTED",
      }
    }
    if (matchingOracles.some((oracle) => oracle.verdict === "INCONCLUSIVE")) {
      return {
        claim: commitment.claim,
        evidenceSummary,
        id: commitment.id,
        verdict: "INCONCLUSIVE",
      }
    }
    return {
      claim: commitment.claim,
      evidenceSummary,
      id: commitment.id,
      verdict: "CONFIRMED",
    }
  })

  return {
    findings,
    observed: findings.every((finding) => finding.verdict !== "INCONCLUSIVE"),
    verdict: rollUp(findings, input.runStatus),
  }
}

const evaluateOracle = (
  phaseName: string,
  expectation: KhalaCodeQaOracleExpectation,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  if (expectation.oracle === "schema") {
    const schemaObservation = [...observations].reverse().find((observation) => {
      if (observation.action.kind !== "rpc_call") return false
      if (expectation.query === undefined) return true
      return observation.action.method === expectation.query || `rpc:${observation.action.method}` === expectation.query
    })
    const data = schemaObservation?.data as
      | { readonly oracle?: { readonly decoded?: boolean; readonly unknownFields?: ReadonlyArray<unknown> } }
      | undefined
    const decoded = data?.oracle?.decoded === true
    const unknownFields = data?.oracle?.unknownFields ?? []
    const ok = schemaObservation?.ok === true && decoded && unknownFields.length === 0
    return {
      data: data?.oracle,
      ok,
      oracle: "schema",
      phaseName,
      summary: schemaObservation === undefined
        ? "no RPC observation available for schema oracle"
        : decoded && unknownFields.length === 0
          ? "RPC response decoded with no unknown fields"
          : "RPC response failed schema oracle",
      verdict: ok ? "CONFIRMED" : "REFUTED",
    }
  }

  if (expectation.oracle === "crash") {
    const ok = observations.every((observation) => observation.ok)
    return {
      ok,
      oracle: "crash",
      phaseName,
      summary: "phase actions completed without driver crash",
      verdict: ok ? "CONFIRMED" : "REFUTED",
    }
  }

  if (expectation.oracle === "invariant" && expectation.id === "claim-invariant") {
    return evaluateClaimInvariant(phaseName, observations)
  }

  if (expectation.oracle === "consistency") {
    if (expectation.left === undefined || expectation.right === undefined) {
      return {
        data: { expectation },
        ok: false,
        oracle: "consistency",
        phaseName,
        summary: "consistency oracle requires left and right query labels",
        verdict: "REFUTED",
      }
    }
    const left = findObservationValue(observations, expectation.left)
    const right = findObservationValue(observations, expectation.right)
    if (left === undefined || right === undefined) {
      return {
        data: { leftFound: left !== undefined, rightFound: right !== undefined },
        ok: false,
        oracle: "consistency",
        phaseName,
        summary: "consistency oracle could not find both observed query values",
        verdict: "REFUTED",
      }
    }
    const result = compareKhalaCodeRpcConsistency({
      left,
      leftLabel: expectation.left,
      right,
      rightLabel: expectation.right,
    })
    return {
      data: result,
      ok: result.ok,
      oracle: "consistency",
      phaseName,
      summary: result.ok
        ? "observed values are consistent"
        : `observed values differ at ${result.mismatches.map((mismatch) => mismatch.path).join(", ")}`,
      verdict: result.ok ? "CONFIRMED" : "REFUTED",
    }
  }

  if (expectation.oracle === "perf") {
    const snapshot = expectation.query === undefined
      ? findQaMetricsSnapshot(observations)
      : findObservationValue(observations, expectation.query)
    if (!isQaMetricsSnapshot(snapshot)) {
      return {
        data: { expectation },
        ok: false,
        oracle: "perf",
        phaseName,
        summary: "perf oracle requires a qaMetrics snapshot observation",
        verdict: "INCONCLUSIVE",
      }
    }
    const budget = snapshot.budgets.find((candidate) =>
      candidate.budgetId === expectation.match ||
      candidate.metric === expectation.metric
    )
    if (budget === undefined) {
      return {
        data: { expectation, availableBudgets: snapshot.budgets.map((candidate) => candidate.budgetId) },
        ok: false,
        oracle: "perf",
        phaseName,
        summary: "perf oracle could not find a matching metric budget",
        verdict: "INCONCLUSIVE",
      }
    }
    const evaluation = evaluateKhalaCodeQaMetricBudget(
      expectation.budget === undefined ? budget : { ...budget, threshold: expectation.budget },
      snapshot.samples,
    )
    return {
      data: evaluation,
      ok: evaluation.status === "pass",
      oracle: "perf",
      phaseName,
      summary: evaluation.actual === null
        ? `no samples for ${budget.metric}`
        : `${budget.metric}=${evaluation.actual}${evaluation.unit} budget=${evaluation.threshold}${evaluation.unit}`,
      verdict: evaluation.status === "pass"
        ? "CONFIRMED"
        : evaluation.status === "fail"
          ? "REFUTED"
          : "INCONCLUSIVE",
    }
  }

  return {
    data: { expectation },
    ok: false,
    oracle: expectation.oracle,
    phaseName,
    summary: `${expectation.oracle} oracle is not evaluated by this runner`,
    verdict: "INCONCLUSIVE",
  }
}

export const runKhalaCodeQaScenario = (input: {
  readonly driver: KhalaCodeQaDriver
  readonly scenario: KhalaCodeQaScenario
}): Effect.Effect<KhalaCodeQaScenarioRunReport, never> =>
  Effect.gen(function* () {
    const bootFailure = yield* input.driver.boot({ backend: input.scenario.backend, headless: true }).pipe(
      Effect.match({
        onFailure: (failure) => failure,
        onSuccess: () => undefined,
      }),
    )

    const phaseOutcomes: KhalaCodeQaPhaseOutcome[] = []
    if (bootFailure !== undefined) {
      phaseOutcomes.push({
        name: "boot",
        observations: [
          {
            action: {
              backend: input.scenario.backend,
              headless: true,
              kind: "boot",
            },
            error: safeFailureDetail(bootFailure),
            label: "boot",
            ok: false,
          },
        ],
        oracles: [],
        status: "fail",
      })
    }
    for (const phase of input.scenario.phases) {
      const observations: KhalaCodeQaObservation[] = []
      for (const action of phase.act) {
        const observation = yield* input.driver.act(action).pipe(
          Effect.catch((cause) =>
            Effect.succeed({
              action,
              error: safeFailureDetail(cause),
              label: action.kind,
              ok: false,
            } satisfies KhalaCodeQaObservation),
          ),
        )
        observations.push(observation)
      }
      if (phase.expect.some((expectation) => expectation.oracle === "perf")) {
        const metricsObservation = yield* input.driver.metrics().pipe(
          Effect.match({
            onFailure: (failure) => ({
              action: { kind: "read", query: "qaMetrics" } as const,
              error: safeFailureDetail(failure),
              label: "read:qaMetrics",
              ok: false,
            } satisfies KhalaCodeQaObservation),
            onSuccess: (snapshot) => ({
              action: { kind: "read", query: "qaMetrics" } as const,
              data: { value: snapshot },
              label: "read:qaMetrics",
              ok: true,
            } satisfies KhalaCodeQaObservation),
          }),
        )
        observations.push(metricsObservation)
      }
      const oracles = phase.expect.map((expectation) =>
        evaluateOracle(phase.name, expectation, observations),
      )
      phaseOutcomes.push({
        name: phase.name,
        observations,
        oracles,
        status: observations.every((observation) => observation.ok) &&
          oracles.every((oracle) => oracle.ok)
          ? "pass"
          : "fail",
      })
    }

    const status = runStatus(phaseOutcomes)
    const commitments = verifyCommitments({
      commitments: input.scenario.commitments,
      phaseOutcomes,
      runStatus: status,
    })
    yield* input.driver.shutdown().pipe(Effect.catch(() => Effect.succeed({ refs: [] })))
    return {
      backend: input.scenario.backend,
      commitments,
      coverageLedger: collectKhalaCodeQaCoverageLedger({
        observations: phaseOutcomes.flatMap((phase) => phase.observations),
        runId: input.scenario.id,
      }),
      mode: input.driver.mode,
      phaseOutcomes,
      scenarioId: input.scenario.id,
      status,
    }
  })
