import { Schema } from "effect"

import {
  PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
  PROVIDER_HANDOFF_TRANSITION_SCHEMA,
  ProviderHandoffTransitionRecordSchema,
} from "./full-auto-provider-handoff.ts"
import { FULL_AUTO_REGISTRY_SCHEMA } from "./full-auto-registry.ts"
import { FULL_AUTO_RUN_ANALYSIS_SCHEMA } from "./full-auto-run-analyzer.ts"
import { FULL_AUTO_RUN_REGISTRY_SCHEMA } from "./full-auto-run-registry.ts"
import {
  FULL_AUTO_RUN_RECEIPT_SCHEMA,
  FULL_AUTO_RUN_REPORT_SCHEMA,
  sha256HexDigest,
} from "./full-auto-run-report.ts"
import { LOCAL_TURN_JOURNAL_SCHEMA } from "./local-turn-journal.ts"

/**
 * FA-QA-01 (#8976): the six named Full Auto / provider-handoff acceptance
 * tests as TYPED DATA, plus the pinned test-identity record and the
 * evidence->verdict evaluator.
 *
 * This module is deliberately execution-free: it defines WHAT each of the six
 * sidebar tests is (exact title, direction, marker, scripted intent) and WHAT
 * its pass rules are (typed predicates over observable evidence), so the same
 * definitions drive both the headless fixture harness
 * (full-auto-acceptance-driver.ts) and the later owner-armed real-provider
 * runs. Mock/fixture/headless results support diagnosis but can never replace
 * the visible real-provider passes the issue requires -- that boundary is
 * carried structurally by `profileClass` on the pinned identity: a verdict
 * whose identity says `fixture` is machinery proof, never release evidence.
 *
 * Title-prefix discipline (per the issue): a sidebar row's title gains its
 * `PASS`/`FAIL`/`BLOCKED` prefix only AFTER its evidence is evaluated --
 * `acceptanceTitleWithDisposition` is the only prefix mint, and the driver
 * applies it strictly after `evaluateFullAutoAcceptance` returns.
 */

export const FULL_AUTO_ACCEPTANCE_SCHEMA = "openagents.desktop.full_auto_acceptance.v1" as const
export const FULL_AUTO_ACCEPTANCE_IDENTITY_SCHEMA =
  "openagents.desktop.full_auto_acceptance_identity.v1" as const

/** The exact issue-pinned markers -- byte-for-byte, never paraphrased. */
export const FA_QA_MARKER_ORBIT = "ORBIT-17" as const
export const FA_QA_MARKER_LANTERN = "LANTERN-42" as const

/** The two built-in provider lanes the six tests exercise. */
export const FA_QA_CODEX_LANE = "codex-local" as const
export const FA_QA_CLAUDE_LANE = "claude-local" as const

// -----------------------------------------------------------------------
// Test identity (issue section "Pinned test identity").
// -----------------------------------------------------------------------

export const FullAutoAcceptancePackagingModeSchema = Schema.Literals(["dev", "packaged", "unknown"])
export type FullAutoAcceptancePackagingMode = typeof FullAutoAcceptancePackagingModeSchema.Type

/** `fixture` = headless stub-lane machinery proof; `owner_real` = the visible
 * real-provider sidebar run the issue's acceptance criteria require. */
export const FullAutoAcceptanceProfileClassSchema = Schema.Literals(["fixture", "owner_real"])
export type FullAutoAcceptanceProfileClass = typeof FullAutoAcceptanceProfileClassSchema.Type

export const FullAutoAcceptanceTelemetryStateSchema = Schema.Literals([
  "disabled",
  "opted_in",
  "unknown",
])
export type FullAutoAcceptanceTelemetryState = typeof FullAutoAcceptanceTelemetryStateSchema.Type

export const FullAutoAcceptanceProviderVersionSchema = Schema.Struct({
  laneRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  runtime: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  version: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  /** Auth readiness WITHOUT secrets -- a projection, never a credential. */
  authReadiness: Schema.Literals(["ready", "missing", "unknown"]),
})
export type FullAutoAcceptanceProviderVersion = typeof FullAutoAcceptanceProviderVersionSchema.Type

/** Every relevant durable-schema revision this build speaks, captured
 * mechanically from the modules' own exported constants (never typed in by
 * hand, so a schema bump can never silently desynchronize the identity). */
export const FullAutoAcceptanceSchemaRevisionsSchema = Schema.Struct({
  handoffEnvelope: Schema.String,
  handoffTransition: Schema.String,
  fullAutoRegistry: Schema.String,
  runRegistry: Schema.String,
  runReport: Schema.String,
  runReceipt: Schema.String,
  runAnalysis: Schema.String,
  localTurnJournal: Schema.String,
})
export type FullAutoAcceptanceSchemaRevisions = typeof FullAutoAcceptanceSchemaRevisionsSchema.Type

export const FullAutoAcceptanceIdentitySchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_ACCEPTANCE_IDENTITY_SCHEMA),
  /** OpenAgents revision (git SHA) and build/tag, supplied by the caller
   * from its own source of record (CI env, release manifest). */
  revision: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  build: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  packagingMode: FullAutoAcceptancePackagingModeSchema,
  os: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  arch: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  profileClass: FullAutoAcceptanceProfileClassSchema,
  providerVersions: Schema.Array(FullAutoAcceptanceProviderVersionSchema).check(
    Schema.isMaxLength(16),
  ),
  schemaRevisions: FullAutoAcceptanceSchemaRevisionsSchema,
  /** sha256 of the canonical serialized six-test definition set -- a changed
   * definition set is a changed test identity (the issue's rerun rule). */
  testDefinitionRevision: Schema.String.check(Schema.isMinLength(64), Schema.isMaxLength(64)),
  telemetry: FullAutoAcceptanceTelemetryStateSchema,
  startedAt: Schema.String,
  endedAt: Schema.NullOr(Schema.String),
})
export type FullAutoAcceptanceIdentity = typeof FullAutoAcceptanceIdentitySchema.Type

// -----------------------------------------------------------------------
// Pass rules: typed predicates over observable evidence, carried as data.
// -----------------------------------------------------------------------

export const FullAutoAcceptancePassRuleSchema = Schema.Union([
  /** Tests 01-03: the scripted steps stay in exactly one top-level thread. */
  Schema.Struct({ rule: Schema.Literal("single_top_level_thread") }),
  /** Tests 01/02: the target provider states the exact marker after the
   * switch, sourced only from the host-owned handoff. */
  Schema.Struct({
    rule: Schema.Literal("marker_retained"),
    marker: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  }),
  /** Tests 01/02: step two demonstrably uses the source provider's step-one
   * result, not a regenerated substitute. */
  Schema.Struct({ rule: Schema.Literal("step_two_uses_prior_result") }),
  /** Tests 01-03: exactly N visible, durable provider-transition events. */
  Schema.Struct({
    rule: Schema.Literal("visible_transition_count"),
    expected: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  }),
  /** Tests 01/02: no hidden out-of-band copy/paste repair anywhere. */
  Schema.Struct({ rule: Schema.Literal("no_hidden_repair") }),
  /** Test 03: the durable objective AND acceptance rule both reach the
   * target independent of recent-message truncation. */
  Schema.Struct({ rule: Schema.Literal("objective_and_acceptance_rule_durable") }),
  /** Test 03: if secondary context was truncated, Desktop said so and the
   * defined confirmation was recorded -- never a silent "complete" claim. */
  Schema.Struct({ rule: Schema.Literal("truncation_acknowledged_when_present") }),
  /** Tests 04-06: N useful terminal turns completed autonomously. */
  Schema.Struct({
    rule: Schema.Literal("autonomous_turns"),
    expected: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  }),
  /** Tests 04/05: no manual message was needed between autonomous turns. */
  Schema.Struct({ rule: Schema.Literal("no_manual_message_between_turns") }),
  /** Tests 04-06: the run report and analyzer result both exist. */
  Schema.Struct({ rule: Schema.Literal("report_and_analysis_complete") }),
  /** Tests 04/05: the run's final state carries an explicit reason. */
  Schema.Struct({ rule: Schema.Literal("final_reason_explicit") }),
  /** Test 05: the SAME run resumed across the restart boundary (same runRef,
   * objective, workspace, lane, cap). */
  Schema.Struct({ rule: Schema.Literal("resumed_same_run_across_restart") }),
  /** Tests 05/06: no duplicate dispatch anywhere in the run. */
  Schema.Struct({ rule: Schema.Literal("no_duplicate_dispatch") }),
  /** Test 05: one report identity spans the restart (revision advanced on
   * both sides of the boundary; never a second parallel report). */
  Schema.Struct({ rule: Schema.Literal("report_spans_restart") }),
  /** Test 06: the autonomous thread stays addressable while more than
   * `minOtherChats` other chats are created/opened. */
  Schema.Struct({
    rule: Schema.Literal("thread_addressable_under_pressure"),
    minOtherChats: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  }),
  /** Test 06: the next continuation started exactly once, with no manual
   * repair and clean incident classification. */
  Schema.Struct({ rule: Schema.Literal("continuation_started_exactly_once") }),
])
export type FullAutoAcceptancePassRule = typeof FullAutoAcceptancePassRuleSchema.Type

// -----------------------------------------------------------------------
// Observable evidence -- everything the pass rules may look at.
// -----------------------------------------------------------------------

export const FullAutoAcceptanceEvidenceSchema = Schema.Struct({
  /** The single sidebar thread the test ran in (null only when creation
   * itself failed, which is a BLOCKED shape). */
  threadRef: Schema.NullOr(Schema.String),
  /** Every top-level thread the SCRIPTED steps created for this test.
   * Deliberate pressure chats (Test 06) are counted separately below. */
  threadRefsTouched: Schema.Array(Schema.String),
  markerEstablishedInSource: Schema.Boolean,
  markerStatedByTarget: Schema.Boolean,
  /** The exact assistant text the target produced when asked to state the
   * marker -- retained so a human reviewer can audit the boolean above. */
  targetMarkerStatement: Schema.NullOr(Schema.String),
  stepTwoUsedPriorResult: Schema.Boolean,
  /** Count of out-of-band context injections outside the host-owned handoff
   * seam. The pass rule requires exactly zero. */
  hiddenRepairCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  /** Durable provider-transition receipts, read back from the handoff
   * registry (never synthesized by the driver). */
  transitions: Schema.Array(ProviderHandoffTransitionRecordSchema),
  objectiveDeliveredToTarget: Schema.Boolean,
  acceptanceRuleDeliveredToTarget: Schema.Boolean,
  contextTruncated: Schema.Boolean,
  truncationAcknowledged: Schema.Boolean,
  truncationConfirmationRecorded: Schema.Boolean,
  autonomousTurnsCompleted: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  manualMessagesBetweenTurns: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  restartBoundariesObserved: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  initialRunRef: Schema.NullOr(Schema.String),
  resumedRunRef: Schema.NullOr(Schema.String),
  runFieldsContinuous: Schema.Boolean,
  duplicateDispatchCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  otherChatsOpened: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  threadAddressableUnderPressure: Schema.Boolean,
  continuationDispatchCounts: Schema.Array(
    Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  ),
  reportPresent: Schema.Boolean,
  analysisPresent: Schema.Boolean,
  reportSpansRestart: Schema.Boolean,
  finalStateReason: Schema.NullOr(Schema.String),
  /** A provider/runtime outage (or unexecuted slice) marker. Non-null forces
   * disposition BLOCKED -- an outage is never PASS and never product failure
   * unless the product mishandled it (which shows up in other evidence). */
  blockedReason: Schema.NullOr(Schema.String),
})
export type FullAutoAcceptanceEvidence = typeof FullAutoAcceptanceEvidenceSchema.Type

/** Honest zero-evidence baseline: everything false/empty, nothing implied.
 * Drivers spread over this and set only what they actually observed. */
export const EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE: FullAutoAcceptanceEvidence = {
  threadRef: null,
  threadRefsTouched: [],
  markerEstablishedInSource: false,
  markerStatedByTarget: false,
  targetMarkerStatement: null,
  stepTwoUsedPriorResult: false,
  hiddenRepairCount: 0,
  transitions: [],
  objectiveDeliveredToTarget: false,
  acceptanceRuleDeliveredToTarget: false,
  contextTruncated: false,
  truncationAcknowledged: false,
  truncationConfirmationRecorded: false,
  autonomousTurnsCompleted: 0,
  manualMessagesBetweenTurns: 0,
  restartBoundariesObserved: 0,
  initialRunRef: null,
  resumedRunRef: null,
  runFieldsContinuous: false,
  duplicateDispatchCount: 0,
  otherChatsOpened: 0,
  threadAddressableUnderPressure: false,
  continuationDispatchCounts: [],
  reportPresent: false,
  analysisPresent: false,
  reportSpansRestart: false,
  finalStateReason: null,
  blockedReason: null,
}

// -----------------------------------------------------------------------
// The six test definitions.
// -----------------------------------------------------------------------

export const FullAutoAcceptanceTestIdSchema = Schema.Literals([
  "test-01",
  "test-02",
  "test-03",
  "test-04",
  "test-05",
  "test-06",
])
export type FullAutoAcceptanceTestId = typeof FullAutoAcceptanceTestIdSchema.Type

export const FullAutoAcceptanceTestKindSchema = Schema.Literals([
  "handoff_context",
  "handoff_objective",
  "full_auto_turns",
  "full_auto_restart",
  "thread_pressure",
])
export type FullAutoAcceptanceTestKind = typeof FullAutoAcceptanceTestKindSchema.Type

export const FullAutoAcceptanceTestDefinitionSchema = Schema.Struct({
  id: FullAutoAcceptanceTestIdSchema,
  /** The EXACT sidebar conversation title, verbatim from the issue. */
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  kind: FullAutoAcceptanceTestKindSchema,
  sourceLaneRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  targetLaneRef: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  marker: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40))),
  plannedTurns: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))),
  objective: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4000))),
  /** The explicit acceptance rule / done condition, when the test pins one. */
  acceptanceRule: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2000))),
  passRules: Schema.Array(FullAutoAcceptancePassRuleSchema).check(Schema.isMinLength(1)),
})
export type FullAutoAcceptanceTestDefinition = typeof FullAutoAcceptanceTestDefinitionSchema.Type

const decodeDefinition = Schema.decodeUnknownSync(FullAutoAcceptanceTestDefinitionSchema)

export const FULL_AUTO_ACCEPTANCE_TESTS: ReadonlyArray<FullAutoAcceptanceTestDefinition> = [
  decodeDefinition({
    id: "test-01",
    title: "TEST 01 · Codex → Claude · context",
    kind: "handoff_context",
    sourceLaneRef: FA_QA_CODEX_LANE,
    targetLaneRef: FA_QA_CLAUDE_LANE,
    marker: FA_QA_MARKER_ORBIT,
    plannedTurns: null,
    objective: null,
    acceptanceRule: null,
    passRules: [
      { rule: "single_top_level_thread" },
      { rule: "marker_retained", marker: FA_QA_MARKER_ORBIT },
      { rule: "step_two_uses_prior_result" },
      { rule: "visible_transition_count", expected: 1 },
      { rule: "no_hidden_repair" },
    ],
  }),
  decodeDefinition({
    id: "test-02",
    title: "TEST 02 · Claude → Codex · context",
    kind: "handoff_context",
    sourceLaneRef: FA_QA_CLAUDE_LANE,
    targetLaneRef: FA_QA_CODEX_LANE,
    marker: FA_QA_MARKER_LANTERN,
    plannedTurns: null,
    objective: null,
    acceptanceRule: null,
    passRules: [
      { rule: "single_top_level_thread" },
      { rule: "marker_retained", marker: FA_QA_MARKER_LANTERN },
      { rule: "step_two_uses_prior_result" },
      { rule: "visible_transition_count", expected: 1 },
      { rule: "no_hidden_repair" },
    ],
  }),
  decodeDefinition({
    id: "test-03",
    title: "TEST 03 · Codex → Claude · objective retention",
    kind: "handoff_objective",
    sourceLaneRef: FA_QA_CODEX_LANE,
    targetLaneRef: FA_QA_CLAUDE_LANE,
    marker: null,
    plannedTurns: null,
    objective:
      "Catalog every exported schema constant in the desktop Full Auto modules and produce a bounded summary table.",
    acceptanceRule:
      "The summary table lists every exported *_SCHEMA constant with its exact literal value; nothing invented.",
    passRules: [
      { rule: "single_top_level_thread" },
      { rule: "objective_and_acceptance_rule_durable" },
      { rule: "truncation_acknowledged_when_present" },
      { rule: "visible_transition_count", expected: 1 },
      { rule: "no_hidden_repair" },
    ],
  }),
  decodeDefinition({
    id: "test-04",
    title: "TEST 04 · Full Auto · Codex · 3 turns",
    kind: "full_auto_turns",
    sourceLaneRef: FA_QA_CODEX_LANE,
    targetLaneRef: null,
    marker: null,
    plannedTurns: 3,
    objective:
      "Complete three explicitly scoped work packets in this repository, one per autonomous turn, then stop.",
    acceptanceRule: "Three packets are individually complete and the run ends with an explicit final reason.",
    passRules: [
      { rule: "autonomous_turns", expected: 3 },
      { rule: "no_manual_message_between_turns" },
      { rule: "report_and_analysis_complete" },
      { rule: "final_reason_explicit" },
    ],
  }),
  decodeDefinition({
    id: "test-05",
    title: "TEST 05 · Full Auto · Claude · restart",
    kind: "full_auto_restart",
    sourceLaneRef: FA_QA_CLAUDE_LANE,
    targetLaneRef: null,
    marker: null,
    plannedTurns: 3,
    objective:
      "Complete three explicitly scoped work packets across a full Desktop quit/relaunch after the first turn.",
    acceptanceRule:
      "The same run resumes after relaunch with identical objective, workspace, lane, and cap; the report spans the restart.",
    passRules: [
      { rule: "autonomous_turns", expected: 3 },
      { rule: "no_manual_message_between_turns" },
      { rule: "resumed_same_run_across_restart" },
      { rule: "no_duplicate_dispatch" },
      { rule: "report_spans_restart" },
      { rule: "report_and_analysis_complete" },
      { rule: "final_reason_explicit" },
    ],
  }),
  decodeDefinition({
    id: "test-06",
    title: "TEST 06 · Full Auto · thread pressure",
    kind: "thread_pressure",
    sourceLaneRef: FA_QA_CODEX_LANE,
    targetLaneRef: null,
    marker: null,
    plannedTurns: 3,
    objective:
      "Complete three explicitly scoped work packets while more than five other chats are created and opened around the run.",
    acceptanceRule:
      "The autonomous thread remains addressable, each continuation starts exactly once, and incident classification stays clean.",
    passRules: [
      { rule: "autonomous_turns", expected: 3 },
      { rule: "thread_addressable_under_pressure", minOtherChats: 5 },
      { rule: "continuation_started_exactly_once" },
      { rule: "no_duplicate_dispatch" },
      { rule: "report_and_analysis_complete" },
    ],
  }),
]

export const fullAutoAcceptanceTest = (
  id: FullAutoAcceptanceTestId,
): FullAutoAcceptanceTestDefinition =>
  FULL_AUTO_ACCEPTANCE_TESTS.find(definition => definition.id === id)!

// -----------------------------------------------------------------------
// Evaluation: evidence -> verdict. The ONLY disposition mint.
// -----------------------------------------------------------------------

export const FullAutoAcceptanceDispositionSchema = Schema.Literals(["PASS", "FAIL", "BLOCKED"])
export type FullAutoAcceptanceDisposition = typeof FullAutoAcceptanceDispositionSchema.Type

export type FullAutoAcceptanceRuleResult = Readonly<{
  rule: FullAutoAcceptancePassRule["rule"]
  holds: boolean
  detail: string
}>

export type FullAutoAcceptanceVerdict = Readonly<{
  testId: FullAutoAcceptanceTestId
  disposition: FullAutoAcceptanceDisposition
  ruleResults: ReadonlyArray<FullAutoAcceptanceRuleResult>
  /** Human-readable reasons; empty exactly when disposition is PASS. */
  reasons: ReadonlyArray<string>
}>

export const evaluateFullAutoAcceptancePassRule = (
  rule: FullAutoAcceptancePassRule,
  evidence: FullAutoAcceptanceEvidence,
): FullAutoAcceptanceRuleResult => {
  switch (rule.rule) {
    case "single_top_level_thread": {
      const holds = evidence.threadRef !== null &&
        evidence.threadRefsTouched.length === 1 &&
        evidence.threadRefsTouched[0] === evidence.threadRef
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? `all scripted steps stayed in ${evidence.threadRef}`
          : `expected exactly one top-level thread; observed ${evidence.threadRefsTouched.length}`,
      }
    }
    case "marker_retained": {
      const holds = evidence.markerEstablishedInSource &&
        evidence.markerStatedByTarget &&
        (evidence.targetMarkerStatement?.includes(rule.marker) ?? false)
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? `target stated the exact marker ${rule.marker}`
          : evidence.markerEstablishedInSource
            ? `target failed to state marker ${rule.marker} (said: ${evidence.targetMarkerStatement ?? "<nothing>"})`
            : `source never established marker ${rule.marker}`,
      }
    }
    case "step_two_uses_prior_result":
      return {
        rule: rule.rule,
        holds: evidence.stepTwoUsedPriorResult,
        detail: evidence.stepTwoUsedPriorResult
          ? "step two demonstrably used the source provider's step-one result"
          : "step two did not use the prior result",
      }
    case "visible_transition_count": {
      const holds = evidence.transitions.length === rule.expected
      return {
        rule: rule.rule,
        holds,
        detail: `expected ${rule.expected} durable provider-transition event(s); observed ${evidence.transitions.length}`,
      }
    }
    case "no_hidden_repair": {
      const holds = evidence.hiddenRepairCount === 0
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? "no out-of-band context repair occurred"
          : `${evidence.hiddenRepairCount} hidden repair(s) detected`,
      }
    }
    case "objective_and_acceptance_rule_durable": {
      const holds = evidence.objectiveDeliveredToTarget && evidence.acceptanceRuleDeliveredToTarget
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? "objective and acceptance rule both reached the target via the durable priority channel"
          : `objective delivered: ${evidence.objectiveDeliveredToTarget}; acceptance rule delivered: ${evidence.acceptanceRuleDeliveredToTarget}`,
      }
    }
    case "truncation_acknowledged_when_present": {
      const holds = !evidence.contextTruncated ||
        (evidence.truncationAcknowledged && evidence.truncationConfirmationRecorded)
      return {
        rule: rule.rule,
        holds,
        detail: evidence.contextTruncated
          ? holds
            ? "truncation occurred, was surfaced, and the defined confirmation was recorded"
            : "context was truncated but the handoff did not surface it or record the confirmation"
          : "no truncation occurred",
      }
    }
    case "autonomous_turns": {
      const holds = evidence.autonomousTurnsCompleted >= rule.expected
      return {
        rule: rule.rule,
        holds,
        detail: `expected ${rule.expected} autonomous turn(s); observed ${evidence.autonomousTurnsCompleted}`,
      }
    }
    case "no_manual_message_between_turns": {
      const holds = evidence.manualMessagesBetweenTurns === 0
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? "no manual message was needed between turns"
          : `${evidence.manualMessagesBetweenTurns} manual message(s) between turns`,
      }
    }
    case "report_and_analysis_complete": {
      const holds = evidence.reportPresent && evidence.analysisPresent
      return {
        rule: rule.rule,
        holds,
        detail: `report present: ${evidence.reportPresent}; analysis present: ${evidence.analysisPresent}`,
      }
    }
    case "final_reason_explicit": {
      const holds = evidence.finalStateReason !== null && evidence.finalStateReason.trim() !== ""
      return {
        rule: rule.rule,
        holds,
        detail: holds ? `final reason: ${evidence.finalStateReason}` : "no explicit final state/reason",
      }
    }
    case "resumed_same_run_across_restart": {
      const holds = evidence.restartBoundariesObserved >= 1 &&
        evidence.initialRunRef !== null &&
        evidence.initialRunRef === evidence.resumedRunRef &&
        evidence.runFieldsContinuous
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? `run ${evidence.initialRunRef} resumed across ${evidence.restartBoundariesObserved} restart boundary(ies) with continuous fields`
          : `initial run ${evidence.initialRunRef ?? "<none>"} vs resumed ${evidence.resumedRunRef ?? "<none>"}; fields continuous: ${evidence.runFieldsContinuous}; restarts observed: ${evidence.restartBoundariesObserved}`,
      }
    }
    case "no_duplicate_dispatch": {
      const holds = evidence.duplicateDispatchCount === 0
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? "no duplicate dispatch observed"
          : `${evidence.duplicateDispatchCount} duplicate dispatch(es) observed`,
      }
    }
    case "report_spans_restart": {
      const holds = evidence.reportSpansRestart
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? "one report identity spans the restart boundary"
          : "the report does not span the restart boundary",
      }
    }
    case "thread_addressable_under_pressure": {
      const holds = evidence.threadAddressableUnderPressure &&
        evidence.otherChatsOpened > rule.minOtherChats
      return {
        rule: rule.rule,
        holds,
        detail: `thread addressable: ${evidence.threadAddressableUnderPressure}; other chats opened: ${evidence.otherChatsOpened} (required > ${rule.minOtherChats})`,
      }
    }
    case "continuation_started_exactly_once": {
      const holds = evidence.continuationDispatchCounts.length > 0 &&
        evidence.continuationDispatchCounts.every(count => count === 1)
      return {
        rule: rule.rule,
        holds,
        detail: holds
          ? `every continuation cycle dispatched exactly once (${evidence.continuationDispatchCounts.join(", ")})`
          : `continuation dispatch counts: [${evidence.continuationDispatchCounts.join(", ")}] (each must be exactly 1)`,
      }
    }
  }
}

export const evaluateFullAutoAcceptance = (
  definition: FullAutoAcceptanceTestDefinition,
  evidence: FullAutoAcceptanceEvidence,
): FullAutoAcceptanceVerdict => {
  const decoded = Schema.decodeUnknownSync(FullAutoAcceptanceEvidenceSchema)(evidence)
  const ruleResults = definition.passRules.map(rule =>
    evaluateFullAutoAcceptancePassRule(rule, decoded),
  )
  if (decoded.blockedReason !== null) {
    return {
      testId: definition.id,
      disposition: "BLOCKED",
      ruleResults,
      reasons: [`blocked: ${decoded.blockedReason}`],
    }
  }
  const failures = ruleResults.filter(result => !result.holds)
  return {
    testId: definition.id,
    disposition: failures.length === 0 ? "PASS" : "FAIL",
    ruleResults,
    reasons: failures.map(result => `${result.rule}: ${result.detail}`),
  }
}

// -----------------------------------------------------------------------
// Title-prefix discipline.
// -----------------------------------------------------------------------

/** The single prefix mint. Applied only AFTER evidence evaluation; a title
 * that already carries a disposition prefix is never double-prefixed. */
export const acceptanceTitleWithDisposition = (
  title: string,
  disposition: FullAutoAcceptanceDisposition,
): string => `${disposition} · ${stripAcceptanceDisposition(title)}`

export const stripAcceptanceDisposition = (title: string): string =>
  title.replace(/^(?:PASS|FAIL|BLOCKED) · /, "")

export const acceptanceTitleDisposition = (
  title: string,
): FullAutoAcceptanceDisposition | null => {
  const match = /^(PASS|FAIL|BLOCKED) · /.exec(title)
  return match === null ? null : (match[1] as FullAutoAcceptanceDisposition)
}

// -----------------------------------------------------------------------
// Pinned-identity capture.
// -----------------------------------------------------------------------

/** Canonical serialization of the six definitions -- the digest input. */
export const fullAutoAcceptanceDefinitionRevision = (): string =>
  sha256HexDigest(JSON.stringify(FULL_AUTO_ACCEPTANCE_TESTS))

export type CaptureFullAutoAcceptanceIdentityInput = Readonly<{
  revision: string
  build: string
  packagingMode: FullAutoAcceptancePackagingMode
  profileClass: FullAutoAcceptanceProfileClass
  providerVersions: ReadonlyArray<FullAutoAcceptanceProviderVersion>
  telemetry: FullAutoAcceptanceTelemetryState
  now?: () => Date
}>

/** Captures everything capturable mechanically (platform, arch, the exact
 * schema-revision constants each module exports, the definition digest) and
 * takes the rest -- revision/build/packaging/provider versions -- from the
 * caller's own source of record. */
export const captureFullAutoAcceptanceIdentity = (
  input: CaptureFullAutoAcceptanceIdentityInput,
): FullAutoAcceptanceIdentity => {
  const now = input.now ?? (() => new Date())
  return Schema.decodeUnknownSync(FullAutoAcceptanceIdentitySchema)({
    schema: FULL_AUTO_ACCEPTANCE_IDENTITY_SCHEMA,
    revision: input.revision,
    build: input.build,
    packagingMode: input.packagingMode,
    os: process.platform,
    arch: process.arch,
    profileClass: input.profileClass,
    providerVersions: input.providerVersions,
    schemaRevisions: {
      handoffEnvelope: PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
      handoffTransition: PROVIDER_HANDOFF_TRANSITION_SCHEMA,
      fullAutoRegistry: FULL_AUTO_REGISTRY_SCHEMA,
      runRegistry: FULL_AUTO_RUN_REGISTRY_SCHEMA,
      runReport: FULL_AUTO_RUN_REPORT_SCHEMA,
      runReceipt: FULL_AUTO_RUN_RECEIPT_SCHEMA,
      runAnalysis: FULL_AUTO_RUN_ANALYSIS_SCHEMA,
      localTurnJournal: LOCAL_TURN_JOURNAL_SCHEMA,
    },
    testDefinitionRevision: fullAutoAcceptanceDefinitionRevision(),
    telemetry: input.telemetry,
    startedAt: now().toISOString(),
    endedAt: null,
  })
}
