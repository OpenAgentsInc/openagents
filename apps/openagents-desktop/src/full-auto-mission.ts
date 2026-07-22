import { Schema } from "effect";

import { renderFullAutoPlanBrief } from "./full-auto-plan.ts";
import type { FullAutoProfile, FullAutoRecord } from "./full-auto-registry.ts";
import type { ProviderHandoffTransitionRecord } from "./full-auto-provider-handoff.ts";
import {
  FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
  FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
  isFullAutoRunAutonomyEnabled,
  type FullAutoRun,
} from "./full-auto-run-registry.ts";
import type { LocalTurnRecord } from "./local-turn-journal.ts";

export const FULL_AUTO_MISSION_SCHEMA = "openagents.desktop.full_auto_mission.v1" as const;

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180));
const Count = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));
const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64));

export const FullAutoMissionPriorAcceptedOutcomeSchema = Schema.Struct({
  turnRef: Ref,
  lane: LaneRef,
  disposition: Schema.Literal("completed"),
  updatedAt: Schema.String,
});

export const FullAutoMissionPreviousHandoffSchema = Schema.Struct({
  handoffRef: Ref,
  from: LaneRef,
  to: LaneRef,
  at: Schema.String,
  disposition: Schema.Literals(["complete_within_bounds", "truncated_with_confirmation"]),
  truncated: Schema.Boolean,
});

export const FullAutoMissionResponseObligationSchema = Schema.Literals([
  "preserve_owner_objective",
  "perform_one_concrete_useful_step",
  "report_evidence_refs",
  "treat_provider_completion_as_unverified",
]);

/**
 * HANDS-3 (#9174): the bounded plan brief carried into an AUTONOMY-enabled
 * run's mission packet -- the current (next unblocking) step plus a compact
 * prior-progress summary. Absent entirely for a non-autonomy run, so the
 * packet and rendered prompt are byte-for-byte unchanged for existing runs.
 */
export const FullAutoMissionPlanBriefSchema = Schema.Struct({
  currentStepRef: Schema.NullOr(Ref),
  currentStepTitle: Schema.NullOr(Schema.String),
  done: Count,
  total: Count,
  text: Schema.String,
});

/**
 * HANDS-6 (#9184): the autonomy INITIATIVE block. Present ONLY for an
 * autonomy-enabled run, so a non-autonomy packet (and its rendered prompt) is
 * byte-for-byte unchanged. It tells the provider, IN THE MISSION ITSELF, that
 * the absence of an open GitHub issue/claim is not a reason to stop: a
 * self-selected owner-priority host-verifiable action is a valid basis to act,
 * and the run has recorded (or will record) a self-claim for it. `selfClaimRef`
 * names the durable claim already recorded on the run's autonomy block, or null
 * when the run has not yet self-claimed (the provider still acts and the host
 * records the claim). The block grants no authority -- it is framing that
 * counters the passive default at the exact place the provider reads intent.
 */
export const FullAutoMissionAutonomyInitiativeSchema = Schema.Struct({
  selfClaimRef: Schema.NullOr(Ref),
  claimBasis: Schema.Literal("self_selected"),
  claimLedger: Schema.Literals(["local", "relay"]),
})
export type FullAutoMissionAutonomyInitiative = typeof FullAutoMissionAutonomyInitiativeSchema.Type

/**
 * Private provider prompt authority for one Full Auto attempt. The packet is
 * deliberately not a public receipt: objective and doneCondition are owner
 * content and may leave this boundary only for the selected provider turn.
 */
export const FullAutoMissionPacketSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_MISSION_SCHEMA),
  runRef: Schema.NullOr(Ref),
  threadRef: Ref,
  objective: Schema.String,
  doneCondition: Schema.String,
  objectiveSource: Schema.Literals(["user", "control_caller", "legacy_migration", "system_selected"]),
  workspaceRef: Schema.NullOr(Schema.String),
  currentLane: LaneRef,
  accountRef: Schema.NullOr(Ref),
  continuationOrdinal: Count,
  turnCap: Count,
  remainingTurnsIncludingThisOne: Count,
  priorAcceptedOutcome: Schema.NullOr(FullAutoMissionPriorAcceptedOutcomeSchema),
  previousHandoff: Schema.NullOr(FullAutoMissionPreviousHandoffSchema),
  responseObligations: Schema.Array(FullAutoMissionResponseObligationSchema),
  /** HANDS-3 (#9174): present ONLY for an autonomy-enabled run carrying a
   * plan; omitted for every other run. */
  planBrief: Schema.optional(FullAutoMissionPlanBriefSchema),
  /** HANDS-6 (#9184): present ONLY for an autonomy-enabled run; omitted for
   * every non-autonomy run so its packet is byte-for-byte unchanged. */
  autonomyInitiative: Schema.optional(FullAutoMissionAutonomyInitiativeSchema),
  completionAuthority: Schema.Literal(
    "provider completion is self-reported evidence only; the host or owner verifies the done condition",
  ),
});
export type FullAutoMissionPacket = typeof FullAutoMissionPacketSchema.Type;

export type CompileFullAutoMissionPacketInput = Readonly<{
  run: FullAutoRun | null;
  record: FullAutoRecord;
  threadRef: string;
  profile: FullAutoProfile | undefined;
  turnCap: number;
  priorAcceptedOutcome: LocalTurnRecord | null;
  previousHandoff: ProviderHandoffTransitionRecord | null;
}>;

export const compileFullAutoMissionPacket = (
  input: CompileFullAutoMissionPacketInput,
): FullAutoMissionPacket => {
  const completed =
    input.priorAcceptedOutcome?.disposition === "completed" ? input.priorAcceptedOutcome : null;
  const handoff = input.previousHandoff;
  const autonomyEnabled = input.run !== null && isFullAutoRunAutonomyEnabled(input.run);
  // HANDS-6 (#9184): include the initiative block ONLY for an autonomy-enabled
  // run. Every non-autonomy run omits the key entirely (byte-identical packet).
  const autonomyInitiative =
    autonomyEnabled && input.run !== null
      ? {
          selfClaimRef: input.run.autonomy?.selfClaim?.claimRef ?? null,
          claimBasis: "self_selected" as const,
          claimLedger: input.run.autonomy?.selfClaim?.ledger ?? ("local" as const),
        }
      : null;
  // HANDS-3 (#9174): include the plan brief ONLY for an autonomy-enabled run
  // that carries a plan. Every other run omits the key entirely.
  const planBrief =
    autonomyEnabled && input.run !== null && input.run.autonomy?.plan !== undefined
      ? (() => {
          const brief = renderFullAutoPlanBrief(input.run.autonomy.plan);
          return {
            currentStepRef: brief.currentStepRef,
            currentStepTitle: brief.currentStepTitle,
            done: brief.done,
            total: brief.total,
            text: brief.text,
          };
        })()
      : null;
  return Schema.decodeUnknownSync(FullAutoMissionPacketSchema)({
    schema: FULL_AUTO_MISSION_SCHEMA,
    runRef: input.run?.runRef ?? null,
    threadRef: input.threadRef,
    objective: input.run?.objective ?? FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
    doneCondition: input.run?.doneCondition ?? FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
    objectiveSource: input.run?.objectiveSource ?? "legacy_migration",
    workspaceRef: input.run?.workspaceRef ?? input.record.workspaceRef ?? null,
    currentLane: input.profile?.lane ?? input.record.profile?.lane ?? "codex-local",
    accountRef: input.profile?.accountRef ?? input.record.profile?.accountRef ?? null,
    continuationOrdinal: input.record.continuationCount + 1,
    turnCap: input.turnCap,
    remainingTurnsIncludingThisOne: Math.max(0, input.turnCap - input.record.continuationCount),
    priorAcceptedOutcome:
      completed === null
        ? null
        : {
            turnRef: completed.turnRef,
            lane: completed.lane,
            disposition: "completed",
            updatedAt: completed.updatedAt,
          },
    previousHandoff:
      handoff === null
        ? null
        : {
            handoffRef: handoff.handoffRef,
            from: handoff.from,
            to: handoff.to,
            at: handoff.at,
            disposition: handoff.disposition,
            truncated: handoff.truncated,
          },
    responseObligations: [
      "preserve_owner_objective",
      "perform_one_concrete_useful_step",
      "report_evidence_refs",
      "treat_provider_completion_as_unverified",
    ],
    ...(planBrief === null ? {} : { planBrief }),
    ...(autonomyInitiative === null ? {} : { autonomyInitiative }),
    completionAuthority:
      "provider completion is self-reported evidence only; the host or owner verifies the done condition",
  });
};

/** Deterministic private prompt rendering shared by first, continuation,
 * rotated, resumed, and restart-recovered attempts. */
export const renderFullAutoMissionPrompt = (packet: FullAutoMissionPacket): string =>
  [
    "Execute this host-authoritative Full Auto mission packet.",
    "The owner objective and done condition below are verbatim. Preserve them across providers and turns.",
    "Do one concrete, useful next step now; do not replace the mission with a generic repository task.",
    "Report the useful result and concrete evidence references. Do not declare the run complete merely because this turn completed.",
    "",
    JSON.stringify(packet, null, 2),
    "",
    "OWNER OBJECTIVE (VERBATIM)",
    packet.objective,
    "",
    "DONE CONDITION (VERBATIM)",
    packet.doneCondition,
    // HANDS-3 (#9174): the plan brief section appears ONLY for an
    // autonomy-enabled run carrying a plan; a non-autonomy prompt is
    // byte-for-byte unchanged. HANDS-2/3/4 (#9173/#9174/#9175): the same
    // autonomy-only section tells the provider how to report structured plan
    // progress and a done-condition self-report the host will VERIFY (never
    // trust) -- so host-side plan advancement, churn reset, and verified
    // completion have a bounded, machine-readable signal.
    ...(packet.planBrief === undefined
      ? []
      : [
          "",
          "PERSISTENT PLAN (host-tracked)",
          packet.planBrief.text,
          "",
          "STRUCTURED PROGRESS REPORTING (host-parsed)",
          "When you finish a plan step, emit a line: STEP-DONE: <stepRef>. When you start one, emit: STEP-START: <stepRef>.",
          "When (and only when) you believe the DONE CONDITION is fully met, emit a line: FULL-AUTO-COMPLETE. This is a request for the host to run the named verification; it is self-reported evidence only and never completes the run by itself.",
        ]),
    // HANDS-6 (#9184): the INITIATIVE directive appears ONLY for an
    // autonomy-enabled run; a non-autonomy prompt is byte-for-byte unchanged.
    // It counters the passive default -- "no open GitHub claim, so I stopped" --
    // at the exact place the provider reads intent.
    ...(packet.autonomyInitiative === undefined
      ? []
      : [
          "",
          "AUTONOMY INITIATIVE (take the next valuable action)",
          "You are operating in AUTONOMY mode. The absence of an open GitHub issue or claim is NOT a reason to stop. A self-selected, owner-priority-aligned, host-verifiable next action is a valid basis to act.",
          "Do not sit idle waiting for a pre-existing GitHub issue. Record a self-claim (a local or relay work-packet claim, never a new GitHub issue) and PROCEED to one bounded, verified unit of the next valuable work.",
          packet.autonomyInitiative.selfClaimRef === null
            ? "No self-claim has been recorded yet: act on the bounded unit and the host will record the self-claim; do not stop for lack of a claim."
            : `A self-claim is already recorded (ref: ${packet.autonomyInitiative.selfClaimRef}, basis: ${packet.autonomyInitiative.claimBasis}, ledger: ${packet.autonomyInitiative.claimLedger}). Proceed under it.`,
          "Initiative is BOUNDED, not \"do anything\": stay within the owner-priority objective; the host verifies the DONE CONDITION (a provider self-report never completes the run); respect owner Stop/override and the plan and churn limits. Do NOT self-amplify authority or take any reserved action (secrets, custody, release, public claims, spend).",
        ]),
  ].join("\n");

export const appendFullAutoQueuedInstruction = (mission: string, instruction: string): string =>
  `${mission}\n\nOWNER-QUEUED INSTRUCTION FOR THIS TURN\n${instruction}`;
