import { Schema as S } from "effect";
import { VoiceIdentitySchema } from "./voice-identity.js";

export const SARAH_VOICE_PROTOCOL_VERSION =
  "openagents.sarah.voice.v1" as const;
export const SARAH_VOICE_SESSION_PATH =
  "/api/omega/sarah/voice/session" as const;
export const SARAH_VOICE_CONNECT_PATH =
  "/api/omega/sarah/voice/connect" as const;
export const SARAH_VOICE_NOSTR_CHALLENGE_PATH =
  "/api/omega/sarah/voice/auth/challenge" as const;
export const OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH =
  "/api/omega/auth/nostr-device-link/challenge" as const;
export const OMEGA_NOSTR_DEVICE_LINK_PATH =
  "/api/omega/auth/nostr-device-link" as const;
export const SARAH_VOICE_MODEL = "gpt-realtime-2.1" as const;
export const SARAH_VOICE_NOSTR_AUTH_METHOD = "nostr_nip98" as const;
export const SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION =
  "openagents.sarah.voice.auth-challenge.v1" as const;
export const OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION =
  "openagents.omega.nostr-device-link-challenge.v1" as const;
export const OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION =
  "openagents.omega.nostr-device-link.v1" as const;
export const SARAH_VOICE_CLIENT_PROFILES = [
  "omega_editor",
  "mobile_voice_only",
] as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Text = S.String.check(S.isMaxLength(16_384));
const SmallText = S.String.check(S.isMaxLength(2_048));
const Seq = S.Int.check(
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(9_007_199_254_740_991),
);

export const SarahVoiceSessionRequestSchema = S.Struct({
  schema: S.Literal(SARAH_VOICE_PROTOCOL_VERSION),
  identity: VoiceIdentitySchema,
  disclosureRef: Ref,
  clientProfile: S.optional(S.Literals(SARAH_VOICE_CLIENT_PROFILES)),
  auth: S.optional(
    S.Struct({
      method: S.Literal(SARAH_VOICE_NOSTR_AUTH_METHOD),
      challenge: S.String.check(S.isPattern(/^[A-Za-z0-9_-]{32,256}$/u)),
    }),
  ),
});
export type SarahVoiceSessionRequest =
  typeof SarahVoiceSessionRequestSchema.Type;

export const SarahVoiceNostrChallengeRequestSchema = S.Struct({
  schema: S.Literal(SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION),
  deviceRef: Ref,
  pubkey: S.String.check(S.isPattern(/^[0-9A-Fa-f]{64}$/u)),
});
export type SarahVoiceNostrChallengeRequest =
  typeof SarahVoiceNostrChallengeRequestSchema.Type;

export const SarahVoiceNostrChallengeResponseSchema = S.Struct({
  schema: S.Literal(SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION),
  challenge: S.String.check(S.isPattern(/^[A-Za-z0-9_-]{32,256}$/u)),
  expiresAtMs: Seq,
  ownerRef: Ref,
});
export type SarahVoiceNostrChallengeResponse =
  typeof SarahVoiceNostrChallengeResponseSchema.Type;

const NostrPubkey = S.String.check(S.isPattern(/^[0-9a-f]{64}$/u));
const NostrChallenge = S.String.check(S.isPattern(/^[A-Za-z0-9_-]{32,256}$/u));

export const OmegaNostrDeviceLinkChallengeRequestSchema = S.Struct({
  schema: S.Literal(OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION),
  deviceRef: Ref,
  pubkey: NostrPubkey,
});
export type OmegaNostrDeviceLinkChallengeRequest =
  typeof OmegaNostrDeviceLinkChallengeRequestSchema.Type;

export const OmegaNostrDeviceLinkChallengeResponseSchema = S.Struct({
  schema: S.Literal(OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION),
  challenge: NostrChallenge,
  expiresAtMs: Seq,
  ownerRef: Ref,
});
export type OmegaNostrDeviceLinkChallengeResponse =
  typeof OmegaNostrDeviceLinkChallengeResponseSchema.Type;

export const OmegaNostrDeviceLinkRequestSchema = S.Struct({
  schema: S.Literal(OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION),
  challenge: NostrChallenge,
  ownerRef: Ref,
  deviceRef: Ref,
});
export type OmegaNostrDeviceLinkRequest =
  typeof OmegaNostrDeviceLinkRequestSchema.Type;

export const OmegaNostrDeviceLinkResponseSchema = S.Struct({
  schema: S.Literal(OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION),
  linked: S.Literal(true),
  ownerRef: Ref,
});
export type OmegaNostrDeviceLinkResponse =
  typeof OmegaNostrDeviceLinkResponseSchema.Type;

export const SarahVoiceSessionResponseSchema = S.Struct({
  schema: S.Literal(SARAH_VOICE_PROTOCOL_VERSION),
  sessionRef: Ref,
  model: S.Literal(SARAH_VOICE_MODEL),
  gatewayUrl: S.String.check(S.isMaxLength(2_048)),
  ticket: S.String.check(S.isPattern(/^[A-Za-z0-9_-]{32,256}$/u)),
  ticketExpiresAtMs: Seq,
  sessionExpiresAtMs: Seq,
  reservedCreditMsat: Seq,
  maxDurationSeconds: S.Int.check(
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(3_600),
  ),
  clientProfile: S.Literals(SARAH_VOICE_CLIENT_PROFILES),
  inputAudio: S.Struct({
    codec: S.Literal("pcm_s16le"),
    sampleRateHz: S.Literal(24_000),
    channels: S.Literal(1),
  }),
  outputAudio: S.Struct({
    codec: S.Literal("pcm_s16le"),
    sampleRateHz: S.Literal(24_000),
    channels: S.Literal(1),
  }),
  auth: S.optional(
    S.Struct({
      method: S.Literal(SARAH_VOICE_NOSTR_AUTH_METHOD),
      accessToken: S.String.check(
        S.isPattern(/^oa_omega_[A-Za-z0-9_-]{32,256}$/u),
      ),
      expiresIn: S.Int.check(
        S.isGreaterThanOrEqualTo(1),
        S.isLessThanOrEqualTo(3_600),
      ),
    }),
  ),
});
export type SarahVoiceSessionResponse =
  typeof SarahVoiceSessionResponseSchema.Type;

const EditorTargetSchema = S.Struct({
  workspaceRef: Ref,
  path: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
  documentVersion: S.optional(SmallText),
});
export type SarahEditorTarget = typeof EditorTargetSchema.Type;

export const SarahEditorCommandSchema = S.Union([
  S.Struct({
    _tag: S.Literal("context_read"),
    target: EditorTargetSchema,
    startLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
    endLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  }),
  S.Struct({
    _tag: S.Literal("open_path"),
    target: EditorTargetSchema,
  }),
  S.Struct({
    _tag: S.Literal("reveal_range"),
    target: EditorTargetSchema,
    startLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
    endLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  }),
  S.Struct({
    _tag: S.Literal("replace_selection"),
    target: EditorTargetSchema,
    replacement: S.String.check(S.isMaxLength(16_384)),
  }),
  S.Struct({
    _tag: S.Literal("save_document"),
    target: EditorTargetSchema,
  }),
  S.Struct({
    _tag: S.Literal("start_agent_thread"),
    message: S.String.check(S.isMinLength(1), S.isMaxLength(16_384)),
    presentation: S.Literals(["foreground", "background"]),
  }),
]);
export type SarahEditorCommand = typeof SarahEditorCommandSchema.Type;

const BaseClient = {
  schema: S.Literal(SARAH_VOICE_PROTOCOL_VERSION),
  identity: VoiceIdentitySchema,
  sequence: Seq,
};

export const SarahVoiceClientControlSchema = S.Union([
  S.Struct({
    ...BaseClient,
    _tag: S.Literal("session_hello"),
    disclosureRef: Ref,
  }),
  S.Struct({
    ...BaseClient,
    _tag: S.Literal("interrupt"),
    providerItemRef: S.optional(Ref),
    playedAudioMs: S.optional(Seq),
  }),
  S.Struct({
    ...BaseClient,
    _tag: S.Literal("tool_decision"),
    proposalRef: Ref,
    proposalDigest: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
    decision: S.Literals(["confirm", "decline"]),
  }),
  S.Struct({
    ...BaseClient,
    _tag: S.Literal("tool_outcome"),
    proposalRef: Ref,
    proposalDigest: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
    outcomeRef: Ref,
    ok: S.Boolean,
    summary: SmallText,
  }),
  S.Struct({ ...BaseClient, _tag: S.Literal("heartbeat") }),
  S.Struct({
    ...BaseClient,
    _tag: S.Literal("close"),
    reason: S.Literals(["user_stop", "app_backgrounded", "transport_error"]),
  }),
]);
export type SarahVoiceClientControl = typeof SarahVoiceClientControlSchema.Type;

const BaseServer = {
  schema: S.Literal(SARAH_VOICE_PROTOCOL_VERSION),
  identity: VoiceIdentitySchema,
  sequence: Seq,
};

export const SarahVoiceServerControlSchema = S.Union([
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("session_ready"),
    model: S.Literal(SARAH_VOICE_MODEL),
    expiresAtMs: Seq,
    reservedCreditMsat: Seq,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("lifecycle"),
    state: S.Literals([
      "connecting",
      "listening",
      "thinking",
      "speaking",
      "interrupted",
      "closing",
    ]),
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("transcript_delta"),
    source: S.Literals(["user", "assistant"]),
    utteranceRef: Ref,
    text: Text,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("transcript_final"),
    source: S.Literals(["user", "assistant"]),
    utteranceRef: Ref,
    text: Text,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("interrupt_ack"),
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("tool_proposal"),
    proposalRef: Ref,
    proposalDigest: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
    command: SarahEditorCommandSchema,
    confirmationRequired: S.Boolean,
    expiresAtMs: Seq,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("tool_execute"),
    proposalRef: Ref,
    proposalDigest: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
    command: SarahEditorCommandSchema,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("tool_outcome_ref"),
    proposalRef: Ref,
    outcomeRef: Ref,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("audio_ack"),
    acknowledgedClientSequence: Seq,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("heartbeat"),
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("error"),
    code: S.Literals([
      "invalid_frame",
      "sequence_gap",
      "tool_not_allowed",
      "confirmation_required",
      "provider_unavailable",
      "credit_limit",
      "session_expired",
      "internal",
    ]),
    retryable: S.Boolean,
  }),
  S.Struct({
    ...BaseServer,
    _tag: S.Literal("closing"),
    reason: S.Literals([
      "user_stop",
      "session_expired",
      "credit_limit",
      "provider_error",
      "transport_error",
      "server_shutdown",
    ]),
  }),
]);
export type SarahVoiceServerControl = typeof SarahVoiceServerControlSchema.Type;

export const decodeSarahVoiceSessionRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceSessionRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahVoiceNostrChallengeRequest = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceNostrChallengeRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeOmegaNostrDeviceLinkChallengeRequest = (value: unknown) =>
  S.decodeUnknownSync(OmegaNostrDeviceLinkChallengeRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeOmegaNostrDeviceLinkRequest = (value: unknown) =>
  S.decodeUnknownSync(OmegaNostrDeviceLinkRequestSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahVoiceClientControl = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceClientControlSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahEditorCommand = (value: unknown) =>
  S.decodeUnknownSync(SarahEditorCommandSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahVoiceNostrChallengeResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceNostrChallengeResponseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahVoiceSessionResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceSessionResponseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahVoiceServerControl = (value: unknown) =>
  S.decodeUnknownSync(SarahVoiceServerControlSchema)(value, {
    onExcessProperty: "error",
  });
