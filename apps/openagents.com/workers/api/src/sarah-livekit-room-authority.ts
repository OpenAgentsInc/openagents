import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRESENCE_KIND,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  SARAH_LIVEKIT_ROOM_TEXT_PROJECTION_KIND,
  canonicalSarahLiveKitRoomFloorAuthority,
  canonicalSarahLiveKitRoomPresenceAuthority,
  decodeSarahLiveKitRoomFloorLease,
  decodeSarahLiveKitRoomPresenceLease,
  type SarahLiveKitRoomFloorLease,
  type SarahLiveKitRoomFloorState,
  type SarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";
import {
  verifySignedEvent,
  type SarahNostrEventTemplate,
  type SarahNostrSignedEvent,
} from "@openagentsinc/sarah";
import { createHash } from "node:crypto";

export const SARAH_LIVEKIT_FLOOR_MAX_LEASE_MS = 30_000;
export const SARAH_LIVEKIT_FLOOR_RATE_WINDOW_MS = 10_000;
export const SARAH_LIVEKIT_FLOOR_RATE_LIMIT = 4;
export const SARAH_LIVEKIT_PRESENCE_MAX_LEASE_MS = 30 * 60_000;
const MAX_NONCE_HISTORY = 512;
const MAX_RATE_BUCKETS = 128;
const MAX_PROJECTION_BYTES = 4_096;

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const isDigest = (value: string): boolean => /^[a-f0-9]{64}$/u.test(value);
const isRef = (value: string): boolean =>
  value.trim() === value && value.length > 0 && value.length <= 256;

export type SarahLiveKitRoomSigningPort = Readonly<{
  publicKey: () => Promise<string>;
  sign: (template: SarahNostrEventTemplate) => Promise<SarahNostrSignedEvent>;
}>;

export type SarahLiveKitSignedPresence = Readonly<{
  lease: SarahLiveKitRoomPresenceLease;
  authorityDigest: string;
  event: SarahNostrSignedEvent;
}>;

export type SarahLiveKitSignedTextProjection = Readonly<{
  authority: "projection_only";
  presenceLeaseRef: string;
  event: SarahNostrSignedEvent;
}>;

export const issueSarahLiveKitRoomPresenceLease = (
  input: Readonly<{
    sarahPubkey: string;
    presenceLeaseRef: string;
    communityRef: string;
    channelRef: string;
    membershipRevision: string;
    currentMembershipRevision: string;
    e2eeKeyRevision: string;
    roomRef: string;
    roomEpoch: number;
    sarahParticipantRef: string;
    dispatchRef: string;
    sessionRef: string;
    generation: number;
    admissionDigest: string;
    issuedAtMs: number;
    sessionExpiresAtMs: number;
  }>,
): SarahLiveKitRoomPresenceLease => {
  if (
    input.membershipRevision !== input.currentMembershipRevision ||
    input.sarahParticipantRef !== SARAH_LIVEKIT_ROOM_PRINCIPAL
  ) {
    throw new Error("Sarah room presence authority does not match the current room");
  }
  const expiresAtMs = Math.min(
    input.sessionExpiresAtMs,
    input.issuedAtMs + SARAH_LIVEKIT_PRESENCE_MAX_LEASE_MS,
  );
  const lease = decodeSarahLiveKitRoomPresenceLease({
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    sarahPubkey: input.sarahPubkey,
    leaseRef: input.presenceLeaseRef,
    communityRef: input.communityRef,
    channelRef: input.channelRef,
    membershipRevision: input.membershipRevision,
    e2eeKeyRevision: input.e2eeKeyRevision,
    roomRef: input.roomRef,
    roomEpoch: input.roomEpoch,
    sarahParticipantRef: input.sarahParticipantRef,
    dispatchRef: input.dispatchRef,
    sessionRef: input.sessionRef,
    generation: input.generation,
    capabilityProfile: "community_member_v1",
    admissionDigest: input.admissionDigest,
    processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    cohortPolicy: "authenticated_allowlisted",
    issuedAtMs: input.issuedAtMs,
    expiresAtMs,
  });
  assertPresenceLease(lease);
  return lease;
};

const exactTemplate = (
  signed: SarahNostrSignedEvent,
  template: SarahNostrEventTemplate,
  expectedPubkey: string,
): boolean =>
  signed.pubkey === expectedPubkey &&
  signed.kind === template.kind &&
  signed.created_at === template.created_at &&
  signed.content === template.content &&
  JSON.stringify(signed.tags) === JSON.stringify(template.tags) &&
  verifySignedEvent(signed);

const assertPresenceLease = (lease: SarahLiveKitRoomPresenceLease): void => {
  const decoded = decodeSarahLiveKitRoomPresenceLease(lease);
  if (
    decoded.expiresAtMs <= decoded.issuedAtMs ||
    decoded.expiresAtMs - decoded.issuedAtMs > SARAH_LIVEKIT_PRESENCE_MAX_LEASE_MS
  ) {
    throw new Error("Sarah room presence lease duration is invalid");
  }
};

const presenceTemplate = (
  lease: SarahLiveKitRoomPresenceLease,
  authorityDigest: string,
): SarahNostrEventTemplate => ({
  kind: SARAH_LIVEKIT_ROOM_PRESENCE_KIND,
  created_at: Math.floor(lease.issuedAtMs / 1_000),
  tags: [
    ["d", lease.leaseRef],
    ["h", lease.communityRef],
    ["channel", lease.channelRef],
    ["principal", SARAH_LIVEKIT_ROOM_PRINCIPAL],
    ["participant", lease.sarahParticipantRef],
    ["room_epoch", sha256(`${lease.roomRef}\n${lease.roomEpoch}`)],
    ["session", sha256(lease.sessionRef)],
    ["generation", String(lease.generation)],
    ["membership", lease.membershipRevision],
    ["e2ee_key_revision", lease.e2eeKeyRevision],
    ["capability", lease.capabilityProfile],
    ["admission", lease.admissionDigest],
    ["processors", SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE],
    ["expires", String(Math.floor(lease.expiresAtMs / 1_000))],
    ["alt", "OpenAgents verified Sarah room presence"],
  ],
  content: JSON.stringify({
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    authority: "presence_only",
    authorityDigest,
  }),
});

export const makeSarahLiveKitRoomSigner = (
  signingPort: SarahLiveKitRoomSigningPort,
): Readonly<{
  signPresence: (lease: SarahLiveKitRoomPresenceLease) => Promise<SarahLiveKitSignedPresence>;
  signTextProjection: (
    input: Readonly<{
      lease: SarahLiveKitRoomPresenceLease;
      messageRef: string;
      text: string;
      createdAtMs: number;
    }>,
  ) => Promise<SarahLiveKitSignedTextProjection>;
}> => ({
  signPresence: async (lease) => {
    assertPresenceLease(lease);
    const expectedPubkey = await signingPort.publicKey();
    if (!isDigest(expectedPubkey) || expectedPubkey !== lease.sarahPubkey) {
      throw new Error("Sarah room signer identity does not match the presence lease");
    }
    const authorityDigest = sha256(canonicalSarahLiveKitRoomPresenceAuthority(lease));
    const template = presenceTemplate(lease, authorityDigest);
    const event = await signingPort.sign(template);
    if (!exactTemplate(event, template, expectedPubkey)) {
      throw new Error("Sarah room signer returned a changed or invalid presence event");
    }
    return { lease, authorityDigest, event };
  },
  signTextProjection: async ({ lease, messageRef, text, createdAtMs }) => {
    assertPresenceLease(lease);
    const bytes = new TextEncoder().encode(text).byteLength;
    if (
      !isRef(messageRef) ||
      text.trim() === "" ||
      bytes > MAX_PROJECTION_BYTES ||
      createdAtMs < lease.issuedAtMs ||
      createdAtMs >= lease.expiresAtMs
    ) {
      throw new Error("Sarah room text projection is invalid");
    }
    const expectedPubkey = await signingPort.publicKey();
    if (!isDigest(expectedPubkey) || expectedPubkey !== lease.sarahPubkey) {
      throw new Error("Sarah room signer identity does not match the projection lease");
    }
    const template: SarahNostrEventTemplate = {
      kind: SARAH_LIVEKIT_ROOM_TEXT_PROJECTION_KIND,
      created_at: Math.floor(createdAtMs / 1_000),
      tags: [
        ["h", lease.communityRef],
        ["channel", lease.channelRef],
        ["message", messageRef],
        ["presence", lease.leaseRef],
        ["principal", SARAH_LIVEKIT_ROOM_PRINCIPAL],
        ["generation", String(lease.generation)],
        ["authority", "projection_only"],
        ["alt", "Sarah text projection; not command, audio, membership, or settlement authority"],
      ],
      content: text,
    };
    const event = await signingPort.sign(template);
    if (!exactTemplate(event, template, expectedPubkey)) {
      throw new Error("Sarah room signer returned a changed or invalid text projection");
    }
    return {
      authority: "projection_only",
      presenceLeaseRef: lease.leaseRef,
      event,
    };
  },
});

export type SarahLiveKitRoomMemberAccess = Readonly<{
  authenticated: boolean;
  allowlisted: boolean;
  active: boolean;
  role: "member" | "moderator";
  userRefDigest: string;
  pubkey: string;
  participantRef: string;
  mappedParticipantRef: string;
  membershipRevision: string;
  roomRef: string;
  roomEpoch: number;
  safetyIdentifier: string;
}>;

type RateBucket = Readonly<{
  userRefDigest: string;
  windowStartedAtMs: number;
  requestCount: number;
}>;

export type SarahLiveKitRoomAuthoritySnapshot = Readonly<{
  revision: number;
  presence: SarahLiveKitRoomPresenceLease;
  presenceActive: boolean;
  floor: SarahLiveKitRoomFloorState;
  usedNonceDigests: ReadonlyArray<string>;
  rateBuckets: ReadonlyArray<RateBucket>;
  nextInterruptSequence: number;
}>;

export type SarahLiveKitFloorRefusal =
  | "presence_expired"
  | "presence_mismatch"
  | "member_not_authenticated"
  | "member_not_allowlisted"
  | "member_removed"
  | "participant_mismatch"
  | "membership_changed"
  | "nonce_invalid"
  | "nonce_replayed"
  | "nonce_history_full"
  | "rate_limited"
  | "rate_history_full"
  | "floor_busy"
  | "floor_not_held"
  | "not_floor_holder"
  | "moderator_required"
  | "transfer_target_invalid";

export type SarahLiveKitFloorDecision<Value> =
  | Readonly<{
      accepted: true;
      value: Value;
      snapshot: SarahLiveKitRoomAuthoritySnapshot;
    }>
  | Readonly<{ accepted: false; reason: SarahLiveKitFloorRefusal }>;

const activeFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  nowMs: number,
): SarahLiveKitRoomFloorLease | undefined =>
  snapshot.floor.state === "held" && snapshot.floor.lease.expiresAtMs > nowMs
    ? snapshot.floor.lease
    : undefined;

const validateMember = (
  presence: SarahLiveKitRoomPresenceLease,
  member: SarahLiveKitRoomMemberAccess,
): SarahLiveKitFloorRefusal | undefined => {
  if (!member.authenticated) return "member_not_authenticated";
  if (!member.allowlisted) return "member_not_allowlisted";
  if (!member.active) return "member_removed";
  if (member.participantRef !== member.mappedParticipantRef || !isRef(member.participantRef)) {
    return "participant_mismatch";
  }
  if (member.membershipRevision !== presence.membershipRevision) {
    return "membership_changed";
  }
  if (member.roomRef !== presence.roomRef || member.roomEpoch !== presence.roomEpoch) {
    return "presence_mismatch";
  }
  if (
    !isDigest(member.userRefDigest) ||
    !isDigest(member.pubkey) ||
    !isDigest(member.safetyIdentifier)
  ) {
    return "member_not_authenticated";
  }
  return undefined;
};

const consumeRequest = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  member: SarahLiveKitRoomMemberAccess,
  nonce: string,
  nowMs: number,
):
  | Readonly<{
      nonceDigest: string;
      usedNonceDigests: ReadonlyArray<string>;
      rateBuckets: ReadonlyArray<RateBucket>;
    }>
  | SarahLiveKitFloorRefusal => {
  if (!/^[A-Za-z0-9_-]{32,256}$/u.test(nonce)) return "nonce_invalid";
  const nonceDigest = sha256(nonce);
  if (snapshot.usedNonceDigests.includes(nonceDigest)) return "nonce_replayed";
  if (snapshot.usedNonceDigests.length >= MAX_NONCE_HISTORY) return "nonce_history_full";

  const freshBuckets = snapshot.rateBuckets.filter(
    (bucket) => nowMs - bucket.windowStartedAtMs < SARAH_LIVEKIT_FLOOR_RATE_WINDOW_MS,
  );
  const current = freshBuckets.find((bucket) => bucket.userRefDigest === member.userRefDigest);
  if (current !== undefined && current.requestCount >= SARAH_LIVEKIT_FLOOR_RATE_LIMIT) {
    return "rate_limited";
  }
  if (current === undefined && freshBuckets.length >= MAX_RATE_BUCKETS) {
    return "rate_history_full";
  }
  const rateBuckets =
    current === undefined
      ? [
          ...freshBuckets,
          {
            userRefDigest: member.userRefDigest,
            windowStartedAtMs: nowMs,
            requestCount: 1,
          },
        ]
      : freshBuckets.map((bucket) =>
          bucket.userRefDigest === member.userRefDigest
            ? { ...bucket, requestCount: bucket.requestCount + 1 }
            : bucket,
        );
  return {
    nonceDigest,
    usedNonceDigests: [...snapshot.usedNonceDigests, nonceDigest],
    rateBuckets,
  };
};

const validatePresence = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  nowMs: number,
): SarahLiveKitFloorRefusal | undefined => {
  if (!snapshot.presenceActive) return "presence_mismatch";
  try {
    assertPresenceLease(snapshot.presence);
  } catch {
    return "presence_mismatch";
  }
  return snapshot.presence.expiresAtMs <= nowMs ? "presence_expired" : undefined;
};

const floorLease = (
  presence: SarahLiveKitRoomPresenceLease,
  member: SarahLiveKitRoomMemberAccess,
  nonceDigest: string,
  issuance: number,
  nowMs: number,
  requestedLeaseMs: number,
): SarahLiveKitRoomFloorLease => {
  const expiresAtMs =
    nowMs + Math.min(Math.max(1, requestedLeaseMs), SARAH_LIVEKIT_FLOOR_MAX_LEASE_MS);
  const lease = decodeSarahLiveKitRoomFloorLease({
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    leaseRef: `sarah-floor-${sha256(
      `${presence.leaseRef}\n${issuance}\n${member.userRefDigest}\n${nonceDigest}`,
    ).slice(0, 40)}`,
    presenceLeaseRef: presence.leaseRef,
    communityRef: presence.communityRef,
    channelRef: presence.channelRef,
    membershipRevision: presence.membershipRevision,
    roomRef: presence.roomRef,
    roomEpoch: presence.roomEpoch,
    sessionRef: presence.sessionRef,
    generation: presence.generation,
    issuance,
    holderUserRefDigest: member.userRefDigest,
    holderPubkey: member.pubkey,
    holderParticipantRef: member.participantRef,
    holderSafetyIdentifier: member.safetyIdentifier,
    nonceDigest,
    issuedAtMs: nowMs,
    expiresAtMs: Math.min(expiresAtMs, presence.expiresAtMs),
  });
  canonicalSarahLiveKitRoomFloorAuthority(lease);
  return lease;
};

export const initialSarahLiveKitRoomAuthoritySnapshot = (
  presence: SarahLiveKitRoomPresenceLease,
): SarahLiveKitRoomAuthoritySnapshot => {
  assertPresenceLease(presence);
  return {
    revision: 1,
    presence,
    presenceActive: true,
    floor: { state: "available", presenceLeaseRef: presence.leaseRef, issuance: 0 },
    usedNonceDigests: [],
    rateBuckets: [],
    nextInterruptSequence: 1,
  };
};

export const requestSarahLiveKitFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  input: Readonly<{
    member: SarahLiveKitRoomMemberAccess;
    nonce: string;
    requestedLeaseMs: number;
    nowMs: number;
  }>,
): SarahLiveKitFloorDecision<SarahLiveKitRoomFloorLease> => {
  const presenceRefusal = validatePresence(snapshot, input.nowMs);
  if (presenceRefusal !== undefined) return { accepted: false, reason: presenceRefusal };
  const memberRefusal = validateMember(snapshot.presence, input.member);
  if (memberRefusal !== undefined) return { accepted: false, reason: memberRefusal };
  if (activeFloor(snapshot, input.nowMs) !== undefined) {
    return { accepted: false, reason: "floor_busy" };
  }
  const consumed = consumeRequest(snapshot, input.member, input.nonce, input.nowMs);
  if (typeof consumed === "string") return { accepted: false, reason: consumed };
  const priorIssuance =
    snapshot.floor.state === "held" ? snapshot.floor.lease.issuance : snapshot.floor.issuance;
  const lease = floorLease(
    snapshot.presence,
    input.member,
    consumed.nonceDigest,
    priorIssuance + 1,
    input.nowMs,
    input.requestedLeaseMs,
  );
  return {
    accepted: true,
    value: lease,
    snapshot: {
      ...snapshot,
      revision: snapshot.revision + 1,
      floor: { state: "held", lease },
      usedNonceDigests: consumed.usedNonceDigests,
      rateBuckets: consumed.rateBuckets,
    },
  };
};

export const transferSarahLiveKitFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  input: Readonly<{
    actor: SarahLiveKitRoomMemberAccess;
    target: SarahLiveKitRoomMemberAccess;
    nonce: string;
    requestedLeaseMs: number;
    nowMs: number;
  }>,
): SarahLiveKitFloorDecision<SarahLiveKitRoomFloorLease> => {
  const presenceRefusal = validatePresence(snapshot, input.nowMs);
  if (presenceRefusal !== undefined) return { accepted: false, reason: presenceRefusal };
  const held = activeFloor(snapshot, input.nowMs);
  if (held === undefined) return { accepted: false, reason: "floor_not_held" };
  const actorRefusal = validateMember(snapshot.presence, input.actor);
  if (actorRefusal !== undefined) return { accepted: false, reason: actorRefusal };
  if (held.holderUserRefDigest !== input.actor.userRefDigest && input.actor.role !== "moderator") {
    return { accepted: false, reason: "not_floor_holder" };
  }
  const targetRefusal = validateMember(snapshot.presence, input.target);
  if (targetRefusal !== undefined) return { accepted: false, reason: "transfer_target_invalid" };
  const consumed = consumeRequest(snapshot, input.actor, input.nonce, input.nowMs);
  if (typeof consumed === "string") return { accepted: false, reason: consumed };
  const lease = floorLease(
    snapshot.presence,
    input.target,
    consumed.nonceDigest,
    held.issuance + 1,
    input.nowMs,
    input.requestedLeaseMs,
  );
  return {
    accepted: true,
    value: lease,
    snapshot: {
      ...snapshot,
      revision: snapshot.revision + 1,
      floor: { state: "held", lease },
      usedNonceDigests: consumed.usedNonceDigests,
      rateBuckets: consumed.rateBuckets,
    },
  };
};

export const bargeInSarahLiveKitFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  input: Readonly<{
    member: SarahLiveKitRoomMemberAccess;
    nonce: string;
    nowMs: number;
  }>,
): SarahLiveKitFloorDecision<Readonly<{ interruptSequence: number }>> => {
  const presenceRefusal = validatePresence(snapshot, input.nowMs);
  if (presenceRefusal !== undefined) return { accepted: false, reason: presenceRefusal };
  const held = activeFloor(snapshot, input.nowMs);
  if (held === undefined) return { accepted: false, reason: "floor_not_held" };
  const memberRefusal = validateMember(snapshot.presence, input.member);
  if (memberRefusal !== undefined) return { accepted: false, reason: memberRefusal };
  if (held.holderUserRefDigest !== input.member.userRefDigest) {
    return { accepted: false, reason: "not_floor_holder" };
  }
  const consumed = consumeRequest(snapshot, input.member, input.nonce, input.nowMs);
  if (typeof consumed === "string") return { accepted: false, reason: consumed };
  const interruptSequence = snapshot.nextInterruptSequence;
  return {
    accepted: true,
    value: { interruptSequence },
    snapshot: {
      ...snapshot,
      revision: snapshot.revision + 1,
      nextInterruptSequence: interruptSequence + 1,
      usedNonceDigests: consumed.usedNonceDigests,
      rateBuckets: consumed.rateBuckets,
    },
  };
};

export const stopSarahLiveKitFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  input: Readonly<{
    moderator: SarahLiveKitRoomMemberAccess;
    nonce: string;
    nowMs: number;
  }>,
): SarahLiveKitFloorDecision<SarahLiveKitRoomFloorState> => {
  const presenceRefusal = validatePresence(snapshot, input.nowMs);
  if (presenceRefusal !== undefined) return { accepted: false, reason: presenceRefusal };
  const memberRefusal = validateMember(snapshot.presence, input.moderator);
  if (memberRefusal !== undefined) return { accepted: false, reason: memberRefusal };
  if (input.moderator.role !== "moderator") {
    return { accepted: false, reason: "moderator_required" };
  }
  const consumed = consumeRequest(snapshot, input.moderator, input.nonce, input.nowMs);
  if (typeof consumed === "string") return { accepted: false, reason: consumed };
  const issuance =
    snapshot.floor.state === "held" ? snapshot.floor.lease.issuance : snapshot.floor.issuance;
  const floor: SarahLiveKitRoomFloorState = {
    state: "stopped",
    presenceLeaseRef: snapshot.presence.leaseRef,
    issuance,
    reason: "moderator_stop",
  };
  return {
    accepted: true,
    value: floor,
    snapshot: {
      ...snapshot,
      revision: snapshot.revision + 1,
      floor,
      usedNonceDigests: consumed.usedNonceDigests,
      rateBuckets: consumed.rateBuckets,
    },
  };
};

export const expireSarahLiveKitFloor = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  nowMs: number,
): SarahLiveKitRoomAuthoritySnapshot => {
  const held = snapshot.floor.state === "held" ? snapshot.floor.lease : undefined;
  if (held === undefined || held.expiresAtMs > nowMs) return snapshot;
  return {
    ...snapshot,
    revision: snapshot.revision + 1,
    floor: {
      state: "stopped",
      presenceLeaseRef: snapshot.presence.leaseRef,
      issuance: held.issuance,
      reason: "timeout",
    },
  };
};

export const removeSarahLiveKitRoomMember = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  input: Readonly<{
    memberUserRefDigest: string;
    nextMembershipRevision: string;
    nextE2eeKeyRevision: string;
  }>,
): SarahLiveKitRoomAuthoritySnapshot => {
  if (
    !isDigest(input.memberUserRefDigest) ||
    !isDigest(input.nextMembershipRevision) ||
    !isDigest(input.nextE2eeKeyRevision) ||
    input.nextMembershipRevision === snapshot.presence.membershipRevision ||
    input.nextE2eeKeyRevision === snapshot.presence.e2eeKeyRevision
  ) {
    throw new Error("Sarah room membership removal did not rotate authority");
  }
  const held = snapshot.floor.state === "held" ? snapshot.floor.lease : undefined;
  return {
    ...snapshot,
    revision: snapshot.revision + 1,
    presenceActive: false,
    floor: {
      state: "stopped",
      presenceLeaseRef: snapshot.presence.leaseRef,
      issuance:
        snapshot.floor.state === "held" ? snapshot.floor.lease.issuance : snapshot.floor.issuance,
      reason:
        held?.holderUserRefDigest === input.memberUserRefDigest
          ? "member_removed"
          : "membership_changed",
    },
  };
};

export const removeSarahLiveKitRoomPresence = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
): SarahLiveKitRoomAuthoritySnapshot => ({
  ...snapshot,
  revision: snapshot.revision + 1,
  presenceActive: false,
  floor: {
    state: "stopped",
    presenceLeaseRef: snapshot.presence.leaseRef,
    issuance:
      snapshot.floor.state === "held" ? snapshot.floor.lease.issuance : snapshot.floor.issuance,
    reason: "sarah_removed",
  },
});
