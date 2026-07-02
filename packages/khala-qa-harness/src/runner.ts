import { Effect } from "effect"

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
            error: bootFailure.message,
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
              error: cause.message,
              label: action.kind,
              ok: false,
            } satisfies KhalaCodeQaObservation),
          ),
        )
        observations.push(observation)
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
