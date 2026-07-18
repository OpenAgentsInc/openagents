import { Schema } from "effect";

import type { FullAutoProfile, FullAutoRecord } from "./full-auto-registry.ts";
import type { ProviderHandoffTransitionRecord } from "./full-auto-provider-handoff.ts";
import {
  FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
  FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
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
  objectiveSource: Schema.Literals(["user", "control_caller", "legacy_migration"]),
  workspaceRef: Schema.NullOr(Schema.String),
  currentLane: LaneRef,
  accountRef: Schema.NullOr(Ref),
  continuationOrdinal: Count,
  turnCap: Count,
  remainingTurnsIncludingThisOne: Count,
  priorAcceptedOutcome: Schema.NullOr(FullAutoMissionPriorAcceptedOutcomeSchema),
  previousHandoff: Schema.NullOr(FullAutoMissionPreviousHandoffSchema),
  responseObligations: Schema.Array(FullAutoMissionResponseObligationSchema),
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
  ].join("\n");

export const appendFullAutoQueuedInstruction = (mission: string, instruction: string): string =>
  `${mission}\n\nOWNER-QUEUED INSTRUCTION FOR THIS TURN\n${instruction}`;
