import { Schema as S } from "effect"

export const AUDIO_PROTOCOL_VERSION = "openagents.audio.v1" as const
export const AUDIO_MEDIA_MAGIC = "OAA1" as const
export const MAX_AUDIO_PAYLOAD_BYTES = 24_000

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256))
const Text = S.String.check(S.isMaxLength(16_384))
const Seq = S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(9_007_199_254_740_991))
const Generation = S.Int.check(S.isGreaterThanOrEqualTo(1), S.isLessThanOrEqualTo(2_147_483_647))
const BoundedBytes = S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(MAX_AUDIO_PAYLOAD_BYTES))

export const VoiceIdentitySchema = S.Struct({
  ownerRef: Ref, deviceRef: Ref, threadRef: Ref, sessionRef: Ref,
  generation: Generation,
})
export type VoiceIdentity = typeof VoiceIdentitySchema.Type

export const VoiceSessionSchema = S.Struct({
  schema: S.Literal(AUDIO_PROTOCOL_VERSION),
  _tag: S.Literal("voice_session"),
  identity: VoiceIdentitySchema,
  disclosureRef: Ref,
  capture: S.Literals(["off", "on"]),
  egress: S.Literals(["off", "on"]),
  retention: S.Literals(["off", "on"]),
  playback: S.Literals(["off", "on"]),
})

export const RetentionReceiptSchema = S.Struct({
  schema: S.Literal(AUDIO_PROTOCOL_VERSION), _tag: S.Literal("retention_receipt"),
  identity: VoiceIdentitySchema, receiptRef: Ref, disclosureRef: Ref,
  policyRef: Ref, expiresAtMs: Seq, maxRetentionSeconds: Seq,
})

const Base = { schema: S.Literal(AUDIO_PROTOCOL_VERSION), identity: VoiceIdentitySchema, sequence: Seq }
export const ClientFrameSchema = S.Union([
  S.Struct({ ...Base, _tag: S.Literal("start"), disclosureRef: Ref }),
  S.Struct({ ...Base, _tag: S.Literal("audio_chunk"), payloadLength: BoundedBytes, sha256: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)) }),
  S.Struct({ ...Base, _tag: S.Literal("ack"), acknowledgedServerSequence: Seq }),
  S.Struct({ ...Base, _tag: S.Literal("mute") }),
  S.Struct({ ...Base, _tag: S.Literal("unmute") }),
  S.Struct({ ...Base, _tag: S.Literal("heartbeat") }),
  S.Struct({ ...Base, _tag: S.Literal("rekey"), keyRef: Ref }),
  S.Struct({ ...Base, _tag: S.Literal("close"), reason: S.Literals(["stop", "revoke", "transport_error"]) }),
])

export const ServerControlSchema = S.Union([
  S.Struct({ ...Base, _tag: S.Literal("ack"), acknowledgedClientSequence: Seq }),
  S.Struct({ ...Base, _tag: S.Literal("gap"), expectedClientSequence: Seq }),
  S.Struct({ ...Base, _tag: S.Literal("must_refetch"), afterServerSequence: Seq }),
  S.Struct({ ...Base, _tag: S.Literal("transcript_interim"), utteranceRef: Ref, text: Text }),
  S.Struct({ ...Base, _tag: S.Literal("transcript_final"), utteranceRef: Ref, text: Text }),
  S.Struct({ ...Base, _tag: S.Literal("assistant_text"), messageRef: Ref, text: Text }),
  S.Struct({ ...Base, _tag: S.Literal("tts_chunk"), speechRef: Ref, payloadLength: BoundedBytes, sha256: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)) }),
  S.Struct({ ...Base, _tag: S.Literal("command_proposal"), proposalRef: Ref }),
  S.Struct({ ...Base, _tag: S.Literal("command_outcome_ref"), proposalRef: Ref, outcomeRef: Ref }),
  S.Struct({ ...Base, _tag: S.Literal("retention_receipt"), receipt: RetentionReceiptSchema }),
  S.Struct({ ...Base, _tag: S.Literal("heartbeat") }),
  S.Struct({ ...Base, _tag: S.Literal("rekey"), keyRef: Ref }),
  S.Struct({ ...Base, _tag: S.Literal("close"), reason: Text }),
])

export const ClientAudioMediaHeaderSchema = S.Struct({
  schema: S.Literal(AUDIO_PROTOCOL_VERSION), kind: S.Literal("client_audio"),
  identity: VoiceIdentitySchema, sequence: Seq, codec: S.Literals(["pcm_s16le", "opus"]),
  sampleRateHz: S.Literals([16_000, 24_000, 48_000]), channels: S.Literal(1),
  payloadLength: BoundedBytes, sha256: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
})
export const ServerTtsMediaHeaderSchema = S.Struct({
  schema: S.Literal(AUDIO_PROTOCOL_VERSION), kind: S.Literal("server_tts"),
  identity: VoiceIdentitySchema, sequence: Seq, speechRef: Ref,
  codec: S.Literals(["pcm_s16le", "opus"]), sampleRateHz: S.Literals([24_000, 48_000]),
  channels: S.Literal(1), payloadLength: BoundedBytes,
  sha256: S.String.check(S.isPattern(/^[a-f0-9]{64}$/u)),
})
export const MediaHeaderSchema = S.Union([ClientAudioMediaHeaderSchema, ServerTtsMediaHeaderSchema])

export const decodeClientFrame = (value: unknown) => S.decodeUnknownSync(ClientFrameSchema)(value, { onExcessProperty: "error" })
export const decodeServerControl = (value: unknown) => S.decodeUnknownSync(ServerControlSchema)(value, { onExcessProperty: "error" })
export const decodeMediaHeader = (value: unknown) => S.decodeUnknownSync(MediaHeaderSchema)(value, { onExcessProperty: "error" })
