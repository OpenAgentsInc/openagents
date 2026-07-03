import { Effect } from "effect"
import {
  evaluateKhalaCodeQaMetricBudget,
  type KhalaCodeQaMetricsSnapshot,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

import { collectKhalaCodeQaCoverageLedger, type KhalaCodeQaCoverageLedger } from "./coverage-ledger.js"
import type { KhalaCodeQaDriver, KhalaCodeQaObservation } from "./driver.js"
import {
  buildKhalaCodeQaShutdownOracle,
  evaluateKhalaCodeQaShutdownOracle,
  type KhalaCodeQaShutdownOracle,
} from "./memory-oracle.js"
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
  readonly shutdownOracle: KhalaCodeQaShutdownOracle
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

const collectStrings = (value: unknown): readonly string[] => {
  if (typeof value === "string") return [value]
  if (Array.isArray(value)) return value.flatMap(collectStrings)
  if (!isRecord(value)) return []
  return Object.values(value).flatMap(collectStrings)
}

const collectRecords = (value: unknown): readonly Record<string, unknown>[] => {
  if (Array.isArray(value)) return value.flatMap(collectRecords)
  if (!isRecord(value)) return []
  return [value, ...Object.values(value).flatMap(collectRecords)]
}

const armedErrorStateCases = (
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): ReadonlySet<string> =>
  new Set(observations.flatMap((observation) => {
    if (observation.action.kind !== "rpc_call" || observation.action.method !== "qaMetricSample") return []
    const sample = observation.action.args?.[0]
    if (!isRecord(sample) || !isRecord(sample.context)) return []
    return typeof sample.context.errorStateCase === "string" ? [sample.context.errorStateCase] : []
  }))

const hasErrorStateCaseString = (value: unknown, caseId: string): boolean =>
  collectStrings(value).some((text) =>
    text.includes(`qa.error_state.${caseId}`) ||
    text.includes(`error_state.${caseId}`) ||
    text.includes(caseId)
  )

const hasExplicitDegradedRecord = (value: unknown, caseId: string): boolean =>
  collectRecords(value).some((record) => {
    if (isRecord(record.degradedState) && record.degradedState.caseId === caseId) return true
    if (record.kind === "khala_code_qa_error_state" && record.caseId === caseId) return true
    return false
  })

const hasGenericDegradedShape = (value: unknown): boolean =>
  collectRecords(value).some((record) => {
    if (record.ok === false) return true
    if (record.available === false && typeof record.reason === "string") return true
    if (record.status === "error" || record.status === "unavailable") return true
    if (record.state === "errored") return true
    if (isRecord(record.binary) && record.binary.available === false) return true
    if (isRecord(record.auth) && record.auth.state !== "ready") return true
    if (isRecord(record.pylon) && record.pylon.status === "unavailable") return true
    if (Array.isArray(record.errors) && record.errors.length > 0) return true
    if (Array.isArray(record.diagnostics) && record.diagnostics.length > 0) return true
    return false
  })

const hasDataPreservedEvidence = (value: unknown, caseId: string): boolean =>
  collectRecords(value).some((record) => {
    if (isRecord(record.degradedState) && record.degradedState.caseId === caseId) {
      return record.degradedState.dataLoss === false || record.degradedState.preservesData === true
    }
    if (record.kind === "khala_code_qa_error_state" && record.caseId === caseId) {
      return record.dataLoss === false || record.preservesData === true
    }
    return false
  }) ||
  collectStrings(value).some((text) =>
    text.includes(`qa.error_state.${caseId}.data_preserved`) ||
    text.includes("data preserved")
  )

const hasDataLossEvidence = (value: unknown): boolean =>
  collectRecords(value).some((record) =>
    record.dataLoss === true ||
    isRecord(record.degradedState) && record.degradedState.dataLoss === true
  )

const collectConsoleErrorEvidence = (value: unknown): readonly unknown[] =>
  collectRecords(value).flatMap((record) => {
    const consoleErrors = record.consoleErrors
    if (Array.isArray(consoleErrors)) return consoleErrors
    return []
  })

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

const evaluateTypedDegradedStateInvariant = (
  phaseName: string,
  expectation: KhalaCodeQaOracleExpectation,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  const caseId = expectation.match
  if (caseId === undefined) {
    return {
      ok: false,
      oracle: "invariant",
      phaseName,
      summary: "typed degraded-state invariant requires match=<error-state-case-id>",
      verdict: "REFUTED",
    }
  }
  const armedCases = armedErrorStateCases(observations)
  const values = observedValues(observations)
  const typedEvidence = values.some((value) =>
    hasExplicitDegradedRecord(value, caseId) ||
    hasErrorStateCaseString(value, caseId) ||
    armedCases.has(caseId) && hasGenericDegradedShape(value)
  )
  const ok = armedCases.has(caseId) && typedEvidence
  return {
    data: { armedCases: [...armedCases].sort(), caseId, typedEvidence },
    ok,
    oracle: "invariant",
    phaseName,
    summary: ok
      ? `${caseId} produced a typed degraded-state projection`
      : `${caseId} did not produce a typed degraded-state projection`,
    verdict: ok ? "CONFIRMED" : "REFUTED",
  }
}

const evaluateNoConsoleErrorsInvariant = (
  phaseName: string,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  const driverErrors = observations.flatMap((observation) =>
    observation.ok ? [] : [observation.error ?? `${observation.label} failed`]
  )
  const consoleErrors = observedValues(observations).flatMap(collectConsoleErrorEvidence)
  const ok = driverErrors.length === 0 && consoleErrors.length === 0
  return {
    data: { consoleErrors, driverErrors },
    ok,
    oracle: "invariant",
    phaseName,
    summary: ok
      ? "no driver or console errors were observed"
      : "driver or console errors were observed",
    verdict: ok ? "CONFIRMED" : "REFUTED",
  }
}

const evaluateNoDataLossInvariant = (
  phaseName: string,
  expectation: KhalaCodeQaOracleExpectation,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  const caseId = expectation.match
  if (caseId === undefined) {
    return {
      ok: false,
      oracle: "invariant",
      phaseName,
      summary: "no-data-loss invariant requires match=<error-state-case-id>",
      verdict: "REFUTED",
    }
  }
  const armedCases = armedErrorStateCases(observations)
  const values = observedValues(observations)
  const dataLoss = values.some(hasDataLossEvidence)
  const preserved = values.some((value) => hasDataPreservedEvidence(value, caseId))
  const ok = armedCases.has(caseId) && preserved && !dataLoss && observations.every((observation) => observation.ok)
  return {
    data: { armedCases: [...armedCases].sort(), caseId, dataLoss, preserved },
    ok,
    oracle: "invariant",
    phaseName,
    summary: ok
      ? `${caseId} preserved fixture data while degraded`
      : `${caseId} did not prove data preservation`,
    verdict: ok ? "CONFIRMED" : "REFUTED",
  }
}

const stringifyPublicFixtureValue = (value: unknown): string => {
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

const evaluateFixtureEvidenceInvariant = (
  phaseName: string,
  expectation: KhalaCodeQaOracleExpectation,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): KhalaCodeQaOracleOutcome => {
  const match = expectation.match
  const haystack = stringifyPublicFixtureValue(observedValues(observations))
  const ok = match === undefined ? observations.every((observation) => observation.ok) : haystack.includes(match)
  return {
    data: { id: expectation.id, match, observed: ok },
    ok,
    oracle: "invariant",
    phaseName,
    summary: ok
      ? `${expectation.id ?? "fixture evidence"} was observed`
      : `${expectation.id ?? "fixture evidence"} was not observed`,
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
    const schemaObservations = observations.filter((observation) => {
      if (observation.action.kind !== "rpc_call") return false
      if (expectation.query === undefined) return true
      return observation.action.method === expectation.query || `rpc:${observation.action.method}` === expectation.query
    })
    const oracleReports = schemaObservations.map((observation) => {
      const data = observation.data as
        | { readonly oracle?: { readonly decoded?: boolean; readonly unknownFields?: ReadonlyArray<unknown> } }
        | undefined
      return {
        decoded: data?.oracle?.decoded === true,
        label: observation.label,
        method: observation.action.kind === "rpc_call" ? observation.action.method : "unknown",
        observationOk: observation.ok,
        unknownFields: data?.oracle?.unknownFields ?? [],
      }
    })
    const ok = schemaObservations.length > 0 &&
      oracleReports.every((report) =>
        report.observationOk === true &&
        report.decoded &&
        report.unknownFields.length === 0
      )
    const failedReports = oracleReports.filter((report) =>
      report.observationOk !== true ||
      !report.decoded ||
      report.unknownFields.length > 0
    )
    return {
      data: {
        query: expectation.query,
        checkedResponses: oracleReports.length,
        failures: failedReports,
      },
      ok,
      oracle: "schema",
      phaseName,
      summary: schemaObservations.length === 0
        ? "no RPC observation available for schema oracle"
        : failedReports.length === 0
          ? `${oracleReports.length} RPC response(s) decoded with no unknown fields`
          : `RPC response schema oracle failed for ${failedReports.map((report) => report.label).join(", ")}`,
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

  if (expectation.oracle === "invariant" && expectation.id === "typed-degraded-state") {
    return evaluateTypedDegradedStateInvariant(phaseName, expectation, observations)
  }

  if (expectation.oracle === "invariant" && expectation.id === "no-console-errors") {
    return evaluateNoConsoleErrorsInvariant(phaseName, observations)
  }

  if (expectation.oracle === "invariant" && expectation.id === "no-data-loss") {
    return evaluateNoDataLossInvariant(phaseName, expectation, observations)
  }

  if (
    expectation.oracle === "invariant" &&
    (expectation.id === "advisor-advisory-severity" ||
      expectation.id === "advisor-dedupe-guard" ||
      expectation.id === "advisor-interrupt-budget" ||
      expectation.id === "judge-verdict-card" ||
      expectation.id === "role-economics-exact-rows")
  ) {
    return evaluateFixtureEvidenceInvariant(phaseName, expectation, observations)
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

const schemaGateExpectationsForFixturePhase = (
  explicitExpectations: ReadonlyArray<KhalaCodeQaOracleExpectation>,
  observations: ReadonlyArray<KhalaCodeQaObservation>,
): ReadonlyArray<KhalaCodeQaOracleExpectation> => {
  const rpcMethods = [...new Set(observations.flatMap((observation) =>
    observation.action.kind === "rpc_call" ? [observation.action.method] : []
  ))]
  if (rpcMethods.length === 0) return []

  const explicitSchemaExpectations = explicitExpectations.filter((expectation) =>
    expectation.oracle === "schema"
  )
  if (explicitSchemaExpectations.some((expectation) => expectation.query === undefined)) {
    return []
  }

  const coveredQueries = new Set(explicitSchemaExpectations.flatMap((expectation) =>
    expectation.query === undefined ? [] : [expectation.query]
  ))
  return rpcMethods
    .filter((method) =>
      !coveredQueries.has(method) &&
      !coveredQueries.has(`rpc:${method}`)
    )
    .map((method) => ({
      id: "fixture-schema-gate",
      oracle: "schema" as const,
      query: method,
    }))
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
      const expectations = input.scenario.backend === "fixture"
        ? [
            ...phase.expect,
            ...schemaGateExpectationsForFixturePhase(phase.expect, observations),
          ]
        : phase.expect
      const oracles = expectations.map((expectation) =>
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

    const phaseStatus = runStatus(phaseOutcomes)
    const shutdownOracle = yield* input.driver.shutdown().pipe(
      Effect.match({
        onFailure: (failure) =>
          buildKhalaCodeQaShutdownOracle({
            observedAt: new Date().toISOString(),
            orphanProcesses: [{
              pid: -1,
              reason: `driver shutdown failed: ${safeFailureDetail(failure)}`,
            }],
          }),
        onSuccess: (artifacts) =>
          evaluateKhalaCodeQaShutdownOracle({
            artifacts,
            observedAt: new Date().toISOString(),
          }),
      }),
    )
    const status = phaseStatus === "pass" && shutdownOracle.status === "pass" ? "pass" : "fail"
    const commitments = verifyCommitments({
      commitments: input.scenario.commitments,
      phaseOutcomes,
      runStatus: status,
    })
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
      shutdownOracle,
      status,
    }
  })
