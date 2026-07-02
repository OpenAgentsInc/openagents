import { Effect } from "effect"

import {
  type KhalaCodeQaDriver,
  type KhalaCodeQaObservation,
} from "./driver.js"
import { compareKhalaCodeRpcConsistency, type KhalaCodeRpcConsistencyResult } from "./rpc-client.js"
import {
  runKhalaCodeQaScenario,
  type KhalaCodeQaOracleOutcome,
  type KhalaCodeQaScenarioRunReport,
} from "./runner.js"
import type {
  KhalaCodeQaDriverMode,
  KhalaCodeQaOracleExpectation,
  KhalaCodeQaScenario,
} from "./scenario.js"

export type KhalaCodeQaCrossModeFiledIssue = {
  readonly url: string
}

export type KhalaCodeQaCrossModeDisagreementBug = {
  readonly schema: "khala_code_qa_cross_mode_disagreement_bug.v1"
  readonly body: string
  readonly leftLabel: string
  readonly leftState: unknown
  readonly mismatches: KhalaCodeRpcConsistencyResult["mismatches"]
  readonly phaseName: string
  readonly rightLabel: string
  readonly rightState: unknown
  readonly scenarioId: string
  readonly title: string
}

export type KhalaCodeQaCrossModeReport = {
  readonly bugIssue?: KhalaCodeQaCrossModeFiledIssue
  readonly consistencyOutcomes: ReadonlyArray<KhalaCodeQaOracleOutcome>
  readonly firstDisagreementBug?: KhalaCodeQaCrossModeDisagreementBug
  readonly modeReports: Readonly<Record<"dom" | "rpc", KhalaCodeQaScenarioRunReport>>
  readonly scenarioId: string
  readonly status: "pass" | "fail"
}

const isCrossModeLabel = (label: string | undefined): label is `${"dom" | "rpc"}:${string}` =>
  label?.startsWith("rpc:") === true || label?.startsWith("dom:") === true

const parseModeLabel = (
  label: `${"dom" | "rpc"}:${string}`,
): { readonly mode: "dom" | "rpc"; readonly query: string } => {
  const separator = label.indexOf(":")
  return {
    mode: label.slice(0, separator) as "dom" | "rpc",
    query: label.slice(separator + 1),
  }
}

const observationValue = (observation: KhalaCodeQaObservation): unknown => {
  const data = observation.data as
    | { readonly value?: unknown }
    | undefined
  return data?.value ?? observation.data
}

const observationMatchesQuery = (
  observation: KhalaCodeQaObservation,
  query: string,
): boolean => {
  if (observation.label === query || observation.label === `read:${query}`) return true
  if (observation.action.kind === "read") {
    return observation.action.query === query || `read:${observation.action.query}` === query
  }
  if (observation.action.kind === "rpc_call") {
    return observation.action.method === query || `rpc:${observation.action.method}` === query
  }
  return false
}

const findModeValue = (
  reports: Readonly<Record<"dom" | "rpc", KhalaCodeQaScenarioRunReport>>,
  phaseName: string,
  label: string,
): unknown | undefined => {
  if (!isCrossModeLabel(label)) return undefined
  const parsed = parseModeLabel(label)
  const phase = reports[parsed.mode].phaseOutcomes.find((candidate) => candidate.name === phaseName)
  if (phase === undefined) return undefined
  const observation = [...phase.observations].reverse().find((candidate) =>
    observationMatchesQuery(candidate, parsed.query)
  )
  return observation === undefined ? undefined : observationValue(observation)
}

const stripCrossModeConsistency = (
  scenario: KhalaCodeQaScenario,
): KhalaCodeQaScenario => ({
  ...scenario,
  commitments: scenario.commitments.filter((commitment) =>
    commitment.evidence === "run-pass" ||
    !commitment.match.toLowerCase().includes("consistency")
  ),
  phases: scenario.phases.map((phase) => ({
    ...phase,
    expect: phase.expect.filter((expectation) =>
      expectation.oracle !== "consistency" ||
      !isCrossModeLabel(expectation.left) ||
      !isCrossModeLabel(expectation.right)
    ),
  })),
})

const crossModeExpectations = (
  scenario: KhalaCodeQaScenario,
): ReadonlyArray<{ readonly expectation: KhalaCodeQaOracleExpectation; readonly phaseName: string }> =>
  scenario.phases.flatMap((phase) =>
    phase.expect.flatMap((expectation) =>
      expectation.oracle === "consistency" &&
        isCrossModeLabel(expectation.left) &&
        isCrossModeLabel(expectation.right)
        ? [{ expectation, phaseName: phase.name }]
        : []
    )
  )

const evaluateCrossModeConsistency = (
  reports: Readonly<Record<"dom" | "rpc", KhalaCodeQaScenarioRunReport>>,
  phaseName: string,
  expectation: KhalaCodeQaOracleExpectation,
): KhalaCodeQaOracleOutcome => {
  const left = expectation.left === undefined ? undefined : findModeValue(reports, phaseName, expectation.left)
  const right = expectation.right === undefined ? undefined : findModeValue(reports, phaseName, expectation.right)
  if (expectation.left === undefined || expectation.right === undefined || left === undefined || right === undefined) {
    return {
      data: {
        leftFound: left !== undefined,
        rightFound: right !== undefined,
        expectation,
      },
      ok: false,
      oracle: "consistency",
      phaseName,
      summary: "cross-mode consistency oracle could not find both mode states",
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
      ? "Mode P and Mode D projections are consistent"
      : `Mode P and Mode D differ at ${result.mismatches.map((mismatch) => mismatch.path).join(", ")}`,
    verdict: result.ok ? "CONFIRMED" : "REFUTED",
  }
}

const formatJson = (value: unknown): string =>
  JSON.stringify(value, null, 2) ?? "undefined"

const bugBody = (
  scenarioId: string,
  phaseName: string,
  leftLabel: string,
  rightLabel: string,
  result: KhalaCodeRpcConsistencyResult,
  leftState: unknown,
  rightState: unknown,
): string => [
  `Cross-mode consistency disagreement in \`${scenarioId}\` / \`${phaseName}\`.`,
  "",
  `Left: \`${leftLabel}\``,
  `Right: \`${rightLabel}\``,
  "",
  "Mismatches:",
  "```json",
  formatJson(result.mismatches),
  "```",
  "",
  "Left state:",
  "```json",
  formatJson(leftState),
  "```",
  "",
  "Right state:",
  "```json",
  formatJson(rightState),
  "```",
].join("\n")

const firstDisagreementBug = (
  scenarioId: string,
  reports: Readonly<Record<"dom" | "rpc", KhalaCodeQaScenarioRunReport>>,
  entries: ReadonlyArray<{ readonly expectation: KhalaCodeQaOracleExpectation; readonly outcome: KhalaCodeQaOracleOutcome; readonly phaseName: string }>,
): KhalaCodeQaCrossModeDisagreementBug | undefined => {
  const disagreement = entries.find((entry) => entry.outcome.ok === false)
  if (disagreement === undefined || disagreement.expectation.left === undefined || disagreement.expectation.right === undefined) {
    return undefined
  }
  const data = disagreement.outcome.data as KhalaCodeRpcConsistencyResult | undefined
  const leftState = findModeValue(reports, disagreement.phaseName, disagreement.expectation.left)
  const rightState = findModeValue(reports, disagreement.phaseName, disagreement.expectation.right)
  const mismatches = data?.mismatches ?? []
  return {
    schema: "khala_code_qa_cross_mode_disagreement_bug.v1",
    body: bugBody(
      scenarioId,
      disagreement.phaseName,
      disagreement.expectation.left,
      disagreement.expectation.right,
      {
        leftLabel: disagreement.expectation.left,
        mismatches,
        ok: false,
        rightLabel: disagreement.expectation.right,
      },
      leftState,
      rightState,
    ),
    leftLabel: disagreement.expectation.left,
    leftState,
    mismatches,
    phaseName: disagreement.phaseName,
    rightLabel: disagreement.expectation.right,
    rightState,
    scenarioId,
    title: `[QA] Cross-mode consistency mismatch: ${scenarioId} ${disagreement.phaseName}`,
  }
}

export const runKhalaCodeQaCrossModeScenario = (input: {
  readonly fileDisagreement?: (
    bug: KhalaCodeQaCrossModeDisagreementBug,
  ) => Effect.Effect<KhalaCodeQaCrossModeFiledIssue, never>
  readonly makeDriver: (mode: "dom" | "rpc") => KhalaCodeQaDriver
  readonly scenario: KhalaCodeQaScenario
}): Effect.Effect<KhalaCodeQaCrossModeReport, never> =>
  Effect.gen(function* () {
    const strippedScenario = stripCrossModeConsistency(input.scenario)
    const rpcReport = yield* runKhalaCodeQaScenario({
      driver: input.makeDriver("rpc"),
      scenario: { ...strippedScenario, modes: ["rpc" satisfies KhalaCodeQaDriverMode] },
    })
    const domReport = yield* runKhalaCodeQaScenario({
      driver: input.makeDriver("dom"),
      scenario: { ...strippedScenario, modes: ["dom" satisfies KhalaCodeQaDriverMode] },
    })
    const reports = {
      dom: domReport,
      rpc: rpcReport,
    } as const
    const crossEntries = crossModeExpectations(input.scenario).map((entry) => ({
      ...entry,
      outcome: evaluateCrossModeConsistency(reports, entry.phaseName, entry.expectation),
    }))
    const bug = firstDisagreementBug(input.scenario.id, reports, crossEntries)
    const bugIssue = bug === undefined || input.fileDisagreement === undefined
      ? undefined
      : yield* input.fileDisagreement(bug)
    const status = rpcReport.status === "pass" &&
      domReport.status === "pass" &&
      crossEntries.every((entry) => entry.outcome.ok)
      ? "pass"
      : "fail"
    return {
      ...(bugIssue === undefined ? {} : { bugIssue }),
      consistencyOutcomes: crossEntries.map((entry) => entry.outcome),
      ...(bug === undefined ? {} : { firstDisagreementBug: bug }),
      modeReports: reports,
      scenarioId: input.scenario.id,
      status,
    }
  })
