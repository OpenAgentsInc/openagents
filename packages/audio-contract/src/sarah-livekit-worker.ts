import { Schema as S } from "effect";
import { SarahEditorCommandSchema, type SarahEditorCommand } from "./sarah-realtime.js";
import { SarahLiveKitRoomPresenceLeaseSchema } from "./sarah-livekit-room-authority.js";

export const SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION = "openagents.sarah.livekit-worker.v1" as const;
export const SARAH_LIVEKIT_AGENT_NAME = "sarah-room-v1" as const;
export const SARAH_LIVEKIT_MODEL = "gpt-realtime-2.1" as const;
export const SARAH_LIVEKIT_VOICE = "marin" as const;
export const SARAH_LIVEKIT_TRANSCRIPTION_MODEL = "gpt-4o-mini-transcribe" as const;
export const SARAH_LIVEKIT_JOB_CLAIM_PATH = "/api/internal/sarah/livekit/job/claim" as const;
export const SARAH_LIVEKIT_JOB_EVENT_PATH = "/api/internal/sarah/livekit/job/event" as const;
export const SARAH_LIVEKIT_TOOL_PROPOSAL_PATH =
  "/api/internal/sarah/livekit/tool/proposal" as const;
export const SARAH_LIVEKIT_TOOL_STATE_PATH = "/api/internal/sarah/livekit/tool/state" as const;
export const SARAH_LIVEKIT_CONTROL_TOPIC = "openagents.sarah.control.v1" as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Digest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Seq = S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(9_007_199_254_740_991));
const AppliedInterruptSequence = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(9_007_199_254_740_991),
);

export const SarahLiveKitRoomContextSchema = S.Union([
  S.Struct({ kind: S.Literal("private") }),
  S.Struct({
    kind: S.Literal("community"),
    communityRef: Ref,
    channelRef: Ref,
    membershipRevision: Ref,
  }),
]);
export type SarahLiveKitRoomContext = typeof SarahLiveKitRoomContextSchema.Type;

export const SarahLiveKitCapabilityProfileSchema = S.Union([
  S.Struct({
    kind: S.Literal("private_owner_v1"),
    contextRead: S.Literal(true),
    editorProposals: S.Literal(true),
    agentThreadProposals: S.Literal(true),
    ownerMemory: S.Literal(false),
    workspace: S.Literal(false),
    payments: S.Literal(false),
    release: S.Literal(false),
    memberAdmin: S.Literal(false),
    shell: S.Literal(false),
    git: S.Literal(false),
    credentials: S.Literal(false),
  }),
  S.Struct({
    kind: S.Literal("community_member_v1"),
    contextRead: S.Literal(false),
    editorProposals: S.Literal(false),
    agentThreadProposals: S.Literal(false),
    ownerMemory: S.Literal(false),
    workspace: S.Literal(false),
    payments: S.Literal(false),
    release: S.Literal(false),
    memberAdmin: S.Literal(false),
    shell: S.Literal(false),
    git: S.Literal(false),
    credentials: S.Literal(false),
  }),
]);
export type SarahLiveKitCapabilityProfile = typeof SarahLiveKitCapabilityProfileSchema.Type;

export const SarahLiveKitDispatchMetadataSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  agentName: S.Literal(SARAH_LIVEKIT_AGENT_NAME),
  sessionRef: Ref,
  generation: Seq,
  roomRef: Ref,
  roomEpoch: Seq,
  participantRef: Ref,
  sarahParticipantRef: Ref,
  sarahPresenceLeaseRef: Ref,
  capabilityProfile: S.Literals(["omega_editor", "mobile_voice_only", "mobile_command_center"]),
  roomContext: SarahLiveKitRoomContextSchema,
});
export type SarahLiveKitDispatchMetadata = typeof SarahLiveKitDispatchMetadataSchema.Type;

export const SarahLiveKitJobClaimRequestSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  workerRef: Ref,
  jobRef: Ref,
  dispatchRef: Ref,
  roomSid: Ref,
  dispatch: S.Struct({
    sessionRef: Ref,
    generation: Seq,
    roomRef: Ref,
    roomEpoch: Seq,
    participantRef: Ref,
    sarahParticipantRef: Ref,
    sarahPresenceLeaseRef: Ref,
    capabilityProfile: S.Literals(["omega_editor", "mobile_voice_only", "mobile_command_center"]),
    roomContext: SarahLiveKitRoomContextSchema,
  }),
});
export type SarahLiveKitJobClaimRequest = typeof SarahLiveKitJobClaimRequestSchema.Type;

export const SarahLiveKitJobClaimResponseSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  admitted: S.Literal(true),
  sessionRef: Ref,
  generation: Seq,
  sessionExpiresAtMs: Seq,
  safetyIdentifier: Digest,
  capabilityProfile: SarahLiveKitCapabilityProfileSchema,
  presenceLease: S.optional(SarahLiveKitRoomPresenceLeaseSchema),
});
export type SarahLiveKitJobClaimResponse = typeof SarahLiveKitJobClaimResponseSchema.Type;

const Usage = {
  inputTokens: Seq,
  outputTokens: Seq,
  cachedInputTokens: Seq,
  audioInputTokens: Seq,
  audioOutputTokens: Seq,
};

export const SarahLiveKitJobEventSchema = S.Union([
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("worker_connected"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    roomSid: Ref,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("provider_admitted"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    providerSessionRefDigest: Digest,
    providerConfigurationDigest: Digest,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("lease_check"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("interrupt_applied"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    interruptSequence: AppliedInterruptSequence,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("response_usage"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    providerResponseRef: Ref,
    status: S.Literals(["completed", "cancelled", "failed", "incomplete"]),
    ...Usage,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("transcription_usage"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    providerTranscriptionRef: Ref,
    ...Usage,
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    _tag: S.Literal("close"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
    reason: S.Literals([
      "completed",
      "participant_left",
      "membership_revoked",
      "hold_exhausted",
      "operator_stop",
      "provider_disconnect",
      "provider_mismatch",
      "session_expired",
      "worker_shutdown",
      "worker_error",
    ]),
    accountingStatus: S.Literals(["exact", "uncertain"]),
  }),
]);
export type SarahLiveKitJobEvent = typeof SarahLiveKitJobEventSchema.Type;

export const SarahLiveKitInterruptControlSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  _tag: S.Literal("interrupt"),
  sessionRef: Ref,
  generation: Seq,
  roomRef: Ref,
  roomEpoch: Seq,
  interruptSequence: Seq,
  signature: S.String.check(S.isPattern(/^[A-Za-z0-9_-]{43}$/u)),
});
export type SarahLiveKitInterruptControl = typeof SarahLiveKitInterruptControlSchema.Type;

export const SarahLiveKitToolProposalRequestSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  sessionRef: Ref,
  generation: Seq,
  jobRef: Ref,
  eventRef: Ref,
  providerCallRef: Ref,
  command: SarahEditorCommandSchema,
});
export type SarahLiveKitToolProposalRequest = typeof SarahLiveKitToolProposalRequestSchema.Type;

export const SarahLiveKitToolProposalSchema = S.Struct({
  proposalRef: Ref,
  proposalDigest: Digest,
  command: SarahEditorCommandSchema,
  confirmationRequired: S.Boolean,
  expiresAtMs: Seq,
});
export type SarahLiveKitToolProposal = typeof SarahLiveKitToolProposalSchema.Type;

export const SarahLiveKitToolProposalResponseSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  accepted: S.Literal(true),
  proposal: SarahLiveKitToolProposalSchema,
});
export type SarahLiveKitToolProposalResponse = typeof SarahLiveKitToolProposalResponseSchema.Type;

export const SarahLiveKitToolStateRequestSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
  sessionRef: Ref,
  generation: Seq,
  jobRef: Ref,
  proposalRef: Ref,
  proposalDigest: Digest,
});
export type SarahLiveKitToolStateRequest = typeof SarahLiveKitToolStateRequestSchema.Type;

export const SarahLiveKitToolStateResponseSchema = S.Union([
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    state: S.Literal("waiting_decision"),
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    state: S.Literal("declined"),
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    state: S.Literal("execute_sent"),
  }),
  S.Struct({
    schema: S.Literal(SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION),
    state: S.Literal("outcome"),
    outcomeRef: Ref,
    ok: S.Boolean,
    summary: S.String.check(S.isMaxLength(2_048)),
  }),
]);
export type SarahLiveKitToolStateResponse = typeof SarahLiveKitToolStateResponseSchema.Type;

export type SarahLiveKitEditorCommand = Exclude<
  SarahEditorCommand,
  Readonly<{ _tag: "open_path" }>
>;

export const decodeSarahLiveKitDispatchMetadata = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitDispatchMetadataSchema)(value, {
    onExcessProperty: "error",
  });

export const canonicalSarahLiveKitDispatchAuthority = (
  value: SarahLiveKitDispatchMetadata,
): string => {
  const dispatch = decodeSarahLiveKitDispatchMetadata(value);
  const roomContext =
    dispatch.roomContext.kind === "private"
      ? ["private"]
      : [
          "community",
          dispatch.roomContext.communityRef,
          dispatch.roomContext.channelRef,
          dispatch.roomContext.membershipRevision,
        ];
  return JSON.stringify([
    "openagents.sarah.livekit-control-hmac.v1",
    dispatch.schema,
    dispatch.agentName,
    dispatch.sessionRef,
    dispatch.generation,
    dispatch.roomRef,
    dispatch.roomEpoch,
    dispatch.participantRef,
    dispatch.sarahParticipantRef,
    dispatch.sarahPresenceLeaseRef,
    dispatch.capabilityProfile,
    roomContext,
  ]);
};

export const canonicalSarahLiveKitInterruptControl = (
  value: Omit<SarahLiveKitInterruptControl, "signature">,
): string =>
  JSON.stringify([
    "openagents.sarah.livekit-interrupt-hmac.v1",
    value.schema,
    value._tag,
    value.sessionRef,
    value.generation,
    value.roomRef,
    value.roomEpoch,
    value.interruptSequence,
  ]);

export const decodeSarahLiveKitInterruptControl = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitInterruptControlSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitJobClaimRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitJobClaimRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitJobClaimResponse = (value: unknown) => {
  const response = S.decodeUnknownSync(SarahLiveKitJobClaimResponseSchema)(value, {
    onExcessProperty: "error",
  });
  if (
    (response.capabilityProfile.kind === "community_member_v1") !==
    (response.presenceLease !== undefined)
  ) {
    throw new Error("Sarah community claims require one persisted presence lease");
  }
  return response;
};

export const decodeSarahLiveKitJobEvent = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitJobEventSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitToolProposalRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitToolProposalRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitToolProposalResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitToolProposalResponseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitToolStateRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitToolStateRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitToolStateResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitToolStateResponseSchema)(value, {
    onExcessProperty: "error",
  });
