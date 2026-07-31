import { Schema as S } from "effect";

export const SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION = "openagents.sarah.livekit-worker.v1" as const;
export const SARAH_LIVEKIT_AGENT_NAME = "sarah-room-v1" as const;
export const SARAH_LIVEKIT_MODEL = "gpt-realtime-2.1" as const;
export const SARAH_LIVEKIT_VOICE = "marin" as const;
export const SARAH_LIVEKIT_TRANSCRIPTION_MODEL = "gpt-4o-mini-transcribe" as const;
export const SARAH_LIVEKIT_JOB_CLAIM_PATH = "/api/internal/sarah/livekit/job/claim" as const;
export const SARAH_LIVEKIT_JOB_EVENT_PATH = "/api/internal/sarah/livekit/job/event" as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Digest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Seq = S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(9_007_199_254_740_991));
const ControlToken = S.String.check(S.isPattern(/^oa_sarah_lk_[A-Za-z0-9_-]{43,256}$/u));

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
    ownerMemory: S.Literal(true),
    workspace: S.Literal(true),
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
  controlToken: ControlToken,
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
    _tag: S.Literal("lease_check"),
    sessionRef: Ref,
    generation: Seq,
    jobRef: Ref,
    eventRef: Ref,
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
  }),
]);
export type SarahLiveKitJobEvent = typeof SarahLiveKitJobEventSchema.Type;

export const decodeSarahLiveKitDispatchMetadata = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitDispatchMetadataSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitJobClaimRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitJobClaimRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitJobClaimResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitJobClaimResponseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitJobEvent = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitJobEventSchema)(value, {
    onExcessProperty: "error",
  });
