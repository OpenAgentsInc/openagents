import type { KhalaCodeQaObservation } from "./driver.js"
import type {
  KhalaCodeQaAction,
  KhalaCodeQaBackendTier,
  KhalaCodeQaDriverMode,
  KhalaCodeQaOracleExpectation,
  KhalaCodeQaScenario,
  KhalaCodeQaVerdict,
} from "./scenario.js"

export type KhalaCodeQaExploreSessionActionEntry = {
  readonly action: KhalaCodeQaAction
  readonly index: number
  readonly frontierRef?: string
  readonly observation?: KhalaCodeQaObservation
  readonly rationale?: string
}

export type KhalaCodeQaExploreSession = {
  readonly schema: "khala_code_qa_explore_session.v1"
  readonly actionLog: readonly KhalaCodeQaExploreSessionActionEntry[]
  readonly backend: KhalaCodeQaBackendTier
  readonly explorer: "seeded_monkey" | "llm"
  readonly mode: KhalaCodeQaDriverMode
  readonly oracleExpectations: readonly KhalaCodeQaOracleExpectation[]
  readonly runId: string
  readonly status: "pass" | "fail"
}

export type KhalaCodeQaDistilledRegression = {
  readonly schema: "khala_code_qa_distilled_regression.v1"
  readonly scenario: KhalaCodeQaScenario
  readonly sourceRunId: string
  readonly sourceStatus: KhalaCodeQaExploreSession["status"]
}

export type KhalaCodeQaExploreDistillationResult =
  | {
      readonly schema: "khala_code_qa_explore_distillation_result.v1"
      readonly distilled: KhalaCodeQaDistilledRegression
      readonly reason: "distilled"
      readonly verdict: "CONFIRMED"
    }
  | {
      readonly schema: "khala_code_qa_explore_distillation_result.v1"
      readonly distilled?: undefined
      readonly reason:
        | "missing_oracle_expectations"
        | "missing_replayable_actions"
      readonly verdict: "INCONCLUSIVE"
    }

const slugFor = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 72) || "explore_session"

const stableExpectationKey = (expectation: KhalaCodeQaOracleExpectation): string =>
  JSON.stringify(Object.entries(expectation).sort(([left], [right]) => left.localeCompare(right)))

const dedupeExpectations = (
  expectations: readonly KhalaCodeQaOracleExpectation[],
): readonly KhalaCodeQaOracleExpectation[] => {
  const seen = new Set<string>()
  const deduped: KhalaCodeQaOracleExpectation[] = []
  for (const expectation of expectations) {
    const key = stableExpectationKey(expectation)
    if (seen.has(key)) continue
    seen.add(key)
    deduped.push(expectation)
  }
  return deduped
}

const replayableActions = (
  actionLog: readonly KhalaCodeQaExploreSessionActionEntry[],
): readonly KhalaCodeQaAction[] =>
  actionLog
    .filter((entry) => entry.action.kind !== "boot")
    .sort((left, right) => left.index - right.index)
    .map((entry) => entry.action)

export const distillKhalaCodeQaExploreSessionToRegression = (
  session: KhalaCodeQaExploreSession,
): KhalaCodeQaExploreDistillationResult => {
  const actions = replayableActions(session.actionLog)
  if (actions.length === 0) {
    return {
      reason: "missing_replayable_actions",
      schema: "khala_code_qa_explore_distillation_result.v1",
      verdict: "INCONCLUSIVE",
    }
  }

  const expectations = dedupeExpectations(session.oracleExpectations)
  if (expectations.length === 0) {
    return {
      reason: "missing_oracle_expectations",
      schema: "khala_code_qa_explore_distillation_result.v1",
      verdict: "INCONCLUSIVE",
    }
  }

  const slug = slugFor(session.runId)
  return {
    distilled: {
      scenario: {
        backend: session.backend,
        commitments: [
          {
            claim: "distilled explore session replays deterministically",
            evidence: "run-pass",
            id: "distilled_explore.replay_pass",
          },
          {
            claim: "distilled explore session preserved oracle evidence",
            evidence: "phase-oracle",
            id: "distilled_explore.oracle_evidence",
            match: "distilled-regression:crash",
          },
        ],
        id: `scenario.khala_code.distilled.${slug}.v1`,
        modes: [session.mode],
        phases: [{
          act: actions,
          expect: expectations,
          name: "distilled-regression",
        }],
      },
      schema: "khala_code_qa_distilled_regression.v1",
      sourceRunId: session.runId,
      sourceStatus: session.status,
    },
    reason: "distilled",
    schema: "khala_code_qa_explore_distillation_result.v1",
    verdict: "CONFIRMED",
  }
}

export const khalaCodeQaExploreDistillationVerdict = (
  result: KhalaCodeQaExploreDistillationResult,
): KhalaCodeQaVerdict => result.verdict
