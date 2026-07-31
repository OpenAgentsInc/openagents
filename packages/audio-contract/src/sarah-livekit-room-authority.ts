import { Schema as S } from "effect";

export const SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA =
  "openagents.sarah.livekit-room-authority.v1" as const;
export const SARAH_LIVEKIT_ROOM_PRESENCE_KIND = 30_382 as const;
export const SARAH_LIVEKIT_ROOM_TEXT_PROJECTION_KIND = 9 as const;
export const SARAH_LIVEKIT_ROOM_PRINCIPAL = "principal.sarah" as const;
export const SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE = "sarah_openagents_openai_v1" as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Digest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Pubkey = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Sequence = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);

export const SarahLiveKitRoomPresenceLeaseSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA),
  principal: S.Literal(SARAH_LIVEKIT_ROOM_PRINCIPAL),
  sarahPubkey: Pubkey,
  leaseRef: Ref,
  communityRef: Ref,
  channelRef: Ref,
  membershipRevision: Digest,
  e2eeKeyRevision: Digest,
  roomRef: Ref,
  roomEpoch: Sequence,
  sarahParticipantRef: S.Literal(SARAH_LIVEKIT_ROOM_PRINCIPAL),
  dispatchRef: Ref,
  sessionRef: Ref,
  generation: Sequence,
  capabilityProfile: S.Literal("community_member_v1"),
  admissionDigest: Digest,
  processorDisclosure: S.Literal(SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE),
  cohortPolicy: S.Literal("authenticated_allowlisted"),
  issuedAtMs: Sequence,
  expiresAtMs: Sequence,
});
export type SarahLiveKitRoomPresenceLease = typeof SarahLiveKitRoomPresenceLeaseSchema.Type;

export const SarahLiveKitRoomFloorLeaseSchema = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA),
  leaseRef: Ref,
  presenceLeaseRef: Ref,
  communityRef: Ref,
  channelRef: Ref,
  membershipRevision: Digest,
  roomRef: Ref,
  roomEpoch: Sequence,
  sessionRef: Ref,
  generation: Sequence,
  issuance: Sequence,
  holderUserRefDigest: Digest,
  holderPubkey: Pubkey,
  holderParticipantRef: Ref,
  holderSafetyIdentifier: Digest,
  nonceDigest: Digest,
  issuedAtMs: Sequence,
  expiresAtMs: Sequence,
});
export type SarahLiveKitRoomFloorLease = typeof SarahLiveKitRoomFloorLeaseSchema.Type;

export const SarahLiveKitRoomFloorStateSchema = S.Union([
  S.Struct({
    state: S.Literal("available"),
    presenceLeaseRef: Ref,
    issuance: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  }),
  S.Struct({
    state: S.Literal("held"),
    lease: SarahLiveKitRoomFloorLeaseSchema,
  }),
  S.Struct({
    state: S.Literal("stopped"),
    presenceLeaseRef: Ref,
    issuance: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    reason: S.Literals([
      "moderator_stop",
      "timeout",
      "member_removed",
      "sarah_removed",
      "membership_changed",
      "presence_expired",
    ]),
  }),
]);
export type SarahLiveKitRoomFloorState = typeof SarahLiveKitRoomFloorStateSchema.Type;

export const SarahLiveKitRoomRateBucketSchema = S.Struct({
  userRefDigest: Digest,
  windowStartedAtMs: Sequence,
  requestCount: Sequence,
});
export type SarahLiveKitRoomRateBucket = typeof SarahLiveKitRoomRateBucketSchema.Type;

export const SarahLiveKitRoomAuthoritySnapshotSchema = S.Struct({
  revision: Sequence,
  presence: SarahLiveKitRoomPresenceLeaseSchema,
  presenceActive: S.Boolean,
  floor: SarahLiveKitRoomFloorStateSchema,
  usedNonceDigests: S.Array(Digest).check(S.isMaxLength(512)),
  rateBuckets: S.Array(SarahLiveKitRoomRateBucketSchema).check(S.isMaxLength(128)),
  nextInterruptSequence: Sequence,
});
export type SarahLiveKitRoomAuthoritySnapshot = typeof SarahLiveKitRoomAuthoritySnapshotSchema.Type;

export const decodeSarahLiveKitRoomPresenceLease = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitRoomPresenceLeaseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitRoomFloorLease = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitRoomFloorLeaseSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitRoomFloorState = (value: unknown) =>
  S.decodeUnknownSync(SarahLiveKitRoomFloorStateSchema)(value, {
    onExcessProperty: "error",
  });

export const decodeSarahLiveKitRoomAuthoritySnapshot = (
  value: unknown,
): SarahLiveKitRoomAuthoritySnapshot => {
  const snapshot = S.decodeUnknownSync(SarahLiveKitRoomAuthoritySnapshotSchema)(value, {
    onExcessProperty: "error",
  });
  if (
    new Set(snapshot.usedNonceDigests).size !== snapshot.usedNonceDigests.length ||
    new Set(snapshot.rateBuckets.map((bucket) => bucket.userRefDigest)).size !==
      snapshot.rateBuckets.length
  ) {
    throw new Error("Sarah room authority snapshot contains duplicate replay state");
  }
  return snapshot;
};

export const canonicalSarahLiveKitRoomPresenceAuthority = (
  value: SarahLiveKitRoomPresenceLease,
): string => {
  const lease = decodeSarahLiveKitRoomPresenceLease(value);
  return JSON.stringify([
    lease.schema,
    lease.principal,
    lease.sarahPubkey,
    lease.leaseRef,
    lease.communityRef,
    lease.channelRef,
    lease.membershipRevision,
    lease.e2eeKeyRevision,
    lease.roomRef,
    lease.roomEpoch,
    lease.sarahParticipantRef,
    lease.dispatchRef,
    lease.sessionRef,
    lease.generation,
    lease.capabilityProfile,
    lease.admissionDigest,
    lease.processorDisclosure,
    lease.cohortPolicy,
    lease.issuedAtMs,
    lease.expiresAtMs,
  ]);
};

export const canonicalSarahLiveKitRoomFloorAuthority = (
  value: SarahLiveKitRoomFloorLease,
): string => {
  const lease = decodeSarahLiveKitRoomFloorLease(value);
  return JSON.stringify([
    lease.schema,
    lease.leaseRef,
    lease.presenceLeaseRef,
    lease.communityRef,
    lease.channelRef,
    lease.membershipRevision,
    lease.roomRef,
    lease.roomEpoch,
    lease.sessionRef,
    lease.generation,
    lease.issuance,
    lease.holderUserRefDigest,
    lease.holderPubkey,
    lease.holderParticipantRef,
    lease.holderSafetyIdentifier,
    lease.nonceDigest,
    lease.issuedAtMs,
    lease.expiresAtMs,
  ]);
};
