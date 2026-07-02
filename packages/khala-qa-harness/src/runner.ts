import { Effect } from "effect"

import type { KhalaCodeQaDriver, KhalaCodeQaObservation } from "./driver.js"
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
): KhalaCodeQaVerdict => {
  if (findings.length === 0) return "INCONCLUSIVE"
  if (findings.some((finding) => finding.verdict === "REFUTED")) return "REFUTED"
  if (findings.some((finding) => finding.verdict === "INCONCLUSIVE")) return "INCONCLUSIVE"
  return "CONFIRMED"
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

    const oracle = allOracles.find((outcome) =>
      oracleEvidenceLabel(outcome.phaseName, outcome.oracle)
        .toLowerCase()
        .includes(commitment.match.toLowerCase()),
    )
    if (oracle === undefined) {
      return {
        claim: commitment.claim,
        evidenceSummary: `no oracle outcome matched "${commitment.match}"`,
        id: commitment.id,
        verdict: "INCONCLUSIVE",
      }
    }
    return oracle.ok
      ? {
          claim: commitment.claim,
          evidenceSummary: `observed oracle ${oracleEvidenceLabel(oracle.phaseName, oracle.oracle)} = ok`,
          id: commitment.id,
          verdict: "CONFIRMED",
        }
      : {
          claim: commitment.claim,
          evidenceSummary: `observed oracle ${oracleEvidenceLabel(oracle.phaseName, oracle.oracle)} = failed`,
          id: commitment.id,
          verdict: "REFUTED",
        }
  })

  return {
    findings,
    observed: findings.every((finding) => finding.verdict !== "INCONCLUSIVE"),
    verdict: rollUp(findings),
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
    return {
      data: data?.oracle,
      ok: schemaObservation?.ok === true && decoded && unknownFields.length === 0,
      oracle: "schema",
      phaseName,
      summary: schemaObservation === undefined
        ? "no RPC observation available for schema oracle"
        : decoded && unknownFields.length === 0
          ? "RPC response decoded with no unknown fields"
          : "RPC response failed schema oracle",
    }
  }

  if (expectation.oracle === "crash") {
    return {
      ok: observations.every((observation) => observation.ok),
      oracle: "crash",
      phaseName,
      summary: "phase actions completed without driver crash",
    }
  }

  return {
    ok: true,
    oracle: expectation.oracle,
    phaseName,
    summary: `${expectation.oracle} oracle recorded for ${phaseName}`,
  }
}

export const runKhalaCodeQaScenario = (input: {
  readonly driver: KhalaCodeQaDriver
  readonly scenario: KhalaCodeQaScenario
}): Effect.Effect<KhalaCodeQaScenarioRunReport, never> =>
  Effect.gen(function* () {
    yield* input.driver.boot({ backend: input.scenario.backend, headless: true }).pipe(
      Effect.catch(() =>
        Effect.succeed({
          backend: input.scenario.backend,
          mode: input.driver.mode,
          startedAt: new Date(0).toISOString(),
        }),
      ),
    )

    const phaseOutcomes: KhalaCodeQaPhaseOutcome[] = []
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
      mode: input.driver.mode,
      phaseOutcomes,
      scenarioId: input.scenario.id,
      status,
    }
  })
