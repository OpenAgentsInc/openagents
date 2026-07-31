import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  canonicalSarahLiveKitRoomFloorAuthority,
  decodeSarahLiveKitRoomPresenceLease,
  type SarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";
import { generateSarahNostrSigner } from "@openagentsinc/sarah";
import { describe, expect, test } from "vitest";

import {
  SARAH_LIVEKIT_FLOOR_MAX_LEASE_MS,
  SARAH_LIVEKIT_FLOOR_RATE_LIMIT,
  bargeInSarahLiveKitFloor,
  expireSarahLiveKitFloor,
  initialSarahLiveKitRoomAuthoritySnapshot,
  issueSarahLiveKitRoomPresenceLease,
  makeSarahLiveKitRoomSigner,
  removeSarahLiveKitRoomMember,
  removeSarahLiveKitRoomPresence,
  requestSarahLiveKitFloor,
  stopSarahLiveKitFloor,
  transferSarahLiveKitFloor,
  type SarahLiveKitRoomAuthoritySnapshot,
  type SarahLiveKitRoomMemberAccess,
} from "./sarah-livekit-room-authority";

const digest = (character: string): string => character.repeat(64);
const issuedAtMs = 1_000_000;

const makeLease = (
  overrides: Partial<SarahLiveKitRoomPresenceLease> = {},
): SarahLiveKitRoomPresenceLease =>
  decodeSarahLiveKitRoomPresenceLease({
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    sarahPubkey: digest("a"),
    leaseRef: "presence:community-one:1",
    communityRef: "community-one",
    channelRef: "agent-chat",
    membershipRevision: digest("b"),
    e2eeKeyRevision: digest("c"),
    roomRef: "room:community-one:1",
    roomEpoch: 1,
    sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    dispatchRef: "dispatch:one",
    sessionRef: "session:one",
    generation: 1,
    capabilityProfile: "community_member_v1",
    admissionDigest: digest("d"),
    processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    cohortPolicy: "authenticated_allowlisted",
    issuedAtMs,
    expiresAtMs: issuedAtMs + 60_000,
    ...overrides,
  });

const makeMember = (
  identity: "one" | "two" | "three",
  overrides: Partial<SarahLiveKitRoomMemberAccess> = {},
): SarahLiveKitRoomMemberAccess => {
  const characters = { one: "1", two: "2", three: "3" } as const;
  const character = characters[identity];
  const participantRef = `participant:${identity}`;
  return {
    authenticated: true,
    allowlisted: true,
    active: true,
    role: "member",
    userRefDigest: digest(character),
    pubkey: digest(character),
    participantRef,
    mappedParticipantRef: participantRef,
    membershipRevision: digest("b"),
    roomRef: "room:community-one:1",
    roomEpoch: 1,
    safetyIdentifier: digest(character),
    ...overrides,
  };
};

const request = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  member: SarahLiveKitRoomMemberAccess,
  nonce: string,
  nowMs = issuedAtMs + 1_000,
) =>
  requestSarahLiveKitFloor(snapshot, {
    member,
    nonce,
    requestedLeaseMs: 10_000,
    nowMs,
  });

describe("Sarah LiveKit room signing boundary", () => {
  test("constructs a bounded community-only lease from current server authority", () => {
    const lease = issueSarahLiveKitRoomPresenceLease({
      sarahPubkey: digest("a"),
      presenceLeaseRef: "presence:server-issued",
      communityRef: "community-one",
      channelRef: "agent-chat",
      membershipRevision: digest("b"),
      currentMembershipRevision: digest("b"),
      e2eeKeyRevision: digest("c"),
      roomRef: "room:community-one:1",
      roomEpoch: 1,
      sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
      dispatchRef: "dispatch:one",
      sessionRef: "session:one",
      generation: 1,
      admissionDigest: digest("d"),
      issuedAtMs,
      sessionExpiresAtMs: issuedAtMs + 60 * 60_000,
    });

    expect(lease.expiresAtMs - lease.issuedAtMs).toBe(30 * 60_000);
    expect(lease.capabilityProfile).toBe("community_member_v1");
    expect(lease.cohortPolicy).toBe("authenticated_allowlisted");
    expect(() =>
      issueSarahLiveKitRoomPresenceLease({
        ...lease,
        presenceLeaseRef: lease.leaseRef,
        currentMembershipRevision: digest("e"),
        sessionExpiresAtMs: lease.expiresAtMs,
      }),
    ).toThrow("does not match");
  });

  test("signs only a public-safe presence binding under Sarah's stable key", async () => {
    const sealed = generateSarahNostrSigner();
    const lease = makeLease({ sarahPubkey: sealed.getPublicKey() });
    const signer = makeSarahLiveKitRoomSigner({
      publicKey: async () => sealed.getPublicKey(),
      sign: async (template) => sealed.signEvent(template),
    });

    const signed = await signer.signPresence(lease);

    expect(signed.event.pubkey).toBe(lease.sarahPubkey);
    expect(signed.event.kind).toBe(30_382);
    expect(signed.event.tags).toContainEqual(["h", "community-one"]);
    expect(signed.event.tags).toContainEqual(["participant", SARAH_LIVEKIT_ROOM_PRINCIPAL]);
    expect(signed.event.tags).toContainEqual([
      "processors",
      SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    ]);
    expect(JSON.stringify(signed)).not.toMatch(
      /secret|nsec|workspace|transcript|settlement|ownerMemory/u,
    );
    expect(signer).not.toHaveProperty("sign");
  });

  test("rejects a signer that changes an allowlisted template or identity", async () => {
    const sealed = generateSarahNostrSigner();
    const other = generateSarahNostrSigner();
    const lease = makeLease({ sarahPubkey: sealed.getPublicKey() });
    const changedTemplate = makeSarahLiveKitRoomSigner({
      publicKey: async () => sealed.getPublicKey(),
      sign: async (template) =>
        sealed.signEvent({ ...template, content: `${template.content} changed` }),
    });
    const changedIdentity = makeSarahLiveKitRoomSigner({
      publicKey: async () => other.getPublicKey(),
      sign: async (template) => other.signEvent(template),
    });

    await expect(changedTemplate.signPresence(lease)).rejects.toThrow("changed or invalid");
    await expect(changedIdentity.signPresence(lease)).rejects.toThrow("does not match");
  });

  test("marks kind-9 text as a non-authoritative projection in the exact group", async () => {
    const sealed = generateSarahNostrSigner();
    const lease = makeLease({ sarahPubkey: sealed.getPublicKey() });
    const signer = makeSarahLiveKitRoomSigner({
      publicKey: async () => sealed.getPublicKey(),
      sign: async (template) => sealed.signEvent(template),
    });

    const projection = await signer.signTextProjection({
      lease,
      messageRef: "message:one",
      text: "The shared answer.",
      createdAtMs: issuedAtMs + 2_000,
    });

    expect(projection.authority).toBe("projection_only");
    expect(projection.event.kind).toBe(9);
    expect(projection.event.tags).toContainEqual(["h", "community-one"]);
    expect(projection.event.tags).toContainEqual(["authority", "projection_only"]);
    expect(projection.event.tags.join(" ")).toContain(
      "not command, audio, membership, or settlement authority",
    );
    await expect(
      signer.signTextProjection({
        lease,
        messageRef: "message:private",
        text: "x".repeat(4_097),
        createdAtMs: issuedAtMs + 2_000,
      }),
    ).rejects.toThrow("projection is invalid");
  });
});

describe("Sarah LiveKit room floor authority", () => {
  test("transfers one floor between three authenticated members without changing Sarah identity", () => {
    const initial = initialSarahLiveKitRoomAuthoritySnapshot(makeLease());
    const memberOne = makeMember("one");
    const memberTwo = makeMember("two");
    const memberThree = makeMember("three");
    const first = request(initial, memberOne, "nonce_member_one_0000000000000001");
    expect(first.accepted).toBe(true);
    if (!first.accepted) return;
    const transfer = transferSarahLiveKitFloor(first.snapshot, {
      actor: memberOne,
      target: memberTwo,
      nonce: "nonce_transfer_to_two_000000000001",
      requestedLeaseMs: 10_000,
      nowMs: issuedAtMs + 2_000,
    });
    expect(transfer.accepted).toBe(true);
    if (!transfer.accepted) return;

    expect(transfer.value.holderParticipantRef).toBe(memberTwo.participantRef);
    expect(transfer.value.issuance).toBe(2);
    expect(transfer.snapshot.revision).toBe(initial.revision + 2);
    expect(transfer.snapshot.presence.sarahPubkey).toBe(initial.presence.sarahPubkey);
    expect(request(transfer.snapshot, memberThree, "nonce_member_three_0000000000001")).toEqual({
      accepted: false,
      reason: "floor_busy",
    });
  });

  test.each([
    [
      "forged participant mapping",
      makeMember("one", { mappedParticipantRef: "participant:forged" }),
      "participant_mismatch",
    ],
    [
      "stale membership",
      makeMember("one", { membershipRevision: digest("f") }),
      "membership_changed",
    ],
    ["removed member", makeMember("one", { active: false }), "member_removed"],
    [
      "unauthenticated member",
      makeMember("one", { authenticated: false }),
      "member_not_authenticated",
    ],
    [
      "public untrusted member",
      makeMember("one", { allowlisted: false }),
      "member_not_allowlisted",
    ],
  ] as const)("rejects %s input", (_label, member, reason) => {
    const result = request(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      member,
      "nonce_rejection_fixture_000000000001",
    );
    expect(result).toEqual({ accepted: false, reason });
  });

  test("rejects a replayed nonce even after a floor timeout", () => {
    const initial = initialSarahLiveKitRoomAuthoritySnapshot(makeLease());
    const member = makeMember("one");
    const nonce = "nonce_replay_fixture_00000000000001";
    const first = request(initial, member, nonce);
    expect(first.accepted).toBe(true);
    if (!first.accepted) return;
    const expired = expireSarahLiveKitFloor(first.snapshot, issuedAtMs + 20_000);

    expect(request(expired, member, nonce, issuedAtMs + 21_000)).toEqual({
      accepted: false,
      reason: "nonce_replayed",
    });
  });

  test("lets only the current floor holder barge in and advances an interrupt sequence", () => {
    const memberOne = makeMember("one");
    const held = request(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      memberOne,
      "nonce_floor_holder_0000000000000001",
    );
    expect(held.accepted).toBe(true);
    if (!held.accepted) return;

    expect(
      bargeInSarahLiveKitFloor(held.snapshot, {
        member: makeMember("two"),
        nonce: "nonce_bad_barge_00000000000000001",
        nowMs: issuedAtMs + 2_000,
      }),
    ).toEqual({ accepted: false, reason: "not_floor_holder" });
    const barge = bargeInSarahLiveKitFloor(held.snapshot, {
      member: memberOne,
      nonce: "nonce_good_barge_0000000000000001",
      nowMs: issuedAtMs + 2_000,
    });
    expect(barge.accepted).toBe(true);
    if (!barge.accepted) return;
    expect(barge.value.interruptSequence).toBe(1);
    expect(barge.snapshot.nextInterruptSequence).toBe(2);
  });

  test("requires a moderator to stop the floor", () => {
    const held = request(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      makeMember("one"),
      "nonce_floor_for_stop_00000000000001",
    );
    expect(held.accepted).toBe(true);
    if (!held.accepted) return;
    const ordinary = stopSarahLiveKitFloor(held.snapshot, {
      moderator: makeMember("two"),
      nonce: "nonce_ordinary_stop_0000000000001",
      nowMs: issuedAtMs + 2_000,
    });
    expect(ordinary).toEqual({ accepted: false, reason: "moderator_required" });
    const stopped = stopSarahLiveKitFloor(held.snapshot, {
      moderator: makeMember("two", { role: "moderator" }),
      nonce: "nonce_moderator_stop_000000000000",
      nowMs: issuedAtMs + 2_000,
    });
    expect(stopped.accepted).toBe(true);
    if (!stopped.accepted) return;
    expect(stopped.value).toMatchObject({ state: "stopped", reason: "moderator_stop" });
  });

  test("bounds lease duration and rate limits repeated floor attempts", () => {
    const member = makeMember("one");
    const bounded = requestSarahLiveKitFloor(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      {
        member,
        nonce: "nonce_bounded_00000000000000000001",
        requestedLeaseMs: Number.MAX_SAFE_INTEGER,
        nowMs: issuedAtMs + 1_000,
      },
    );
    expect(bounded.accepted).toBe(true);
    if (!bounded.accepted) return;
    expect(bounded.value.expiresAtMs - bounded.value.issuedAtMs).toBe(
      SARAH_LIVEKIT_FLOOR_MAX_LEASE_MS,
    );

    let snapshot = initialSarahLiveKitRoomAuthoritySnapshot(makeLease());
    for (let index = 0; index < SARAH_LIVEKIT_FLOOR_RATE_LIMIT; index += 1) {
      const nowMs = issuedAtMs + 1_000 + index * 2;
      const next = requestSarahLiveKitFloor(snapshot, {
        member,
        nonce: `nonce_rate_${String(index).padStart(28, "0")}`,
        requestedLeaseMs: 1,
        nowMs,
      });
      expect(next.accepted).toBe(true);
      if (!next.accepted) return;
      snapshot = expireSarahLiveKitFloor(next.snapshot, nowMs + 1);
    }
    const limited = requestSarahLiveKitFloor(snapshot, {
      member,
      nonce: "nonce_rate_limit_00000000000000001",
      requestedLeaseMs: 1,
      nowMs: issuedAtMs + 1_100,
    });
    expect(limited).toEqual({ accepted: false, reason: "rate_limited" });
  });

  test("rotates membership and E2EE authority on removal and invalidates the floor", () => {
    const held = request(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      makeMember("one"),
      "nonce_member_removal_00000000000001",
    );
    expect(held.accepted).toBe(true);
    if (!held.accepted) return;
    const removed = removeSarahLiveKitRoomMember(held.snapshot, {
      memberUserRefDigest: makeMember("one").userRefDigest,
      nextMembershipRevision: digest("e"),
      nextE2eeKeyRevision: digest("f"),
    });
    expect(removed.floor).toMatchObject({ state: "stopped", reason: "member_removed" });
    expect(removed.presenceActive).toBe(false);
    expect(() =>
      removeSarahLiveKitRoomMember(held.snapshot, {
        memberUserRefDigest: makeMember("one").userRefDigest,
        nextMembershipRevision: held.snapshot.presence.membershipRevision,
        nextE2eeKeyRevision: digest("f"),
      }),
    ).toThrow("did not rotate authority");
  });

  test("Sarah removal immediately invalidates the speaking floor", () => {
    const held = request(
      initialSarahLiveKitRoomAuthoritySnapshot(makeLease()),
      makeMember("one"),
      "nonce_sarah_removal_000000000000001",
    );
    expect(held.accepted).toBe(true);
    if (!held.accepted) return;
    const removed = removeSarahLiveKitRoomPresence(held.snapshot);
    expect(removed.presenceActive).toBe(false);
    expect(removed.floor).toMatchObject({
      state: "stopped",
      reason: "sarah_removed",
    });
  });

  test("two groups cannot share presence, context, floor, generation, or settlement authority", () => {
    const groupOne = initialSarahLiveKitRoomAuthoritySnapshot(makeLease());
    const groupTwo = initialSarahLiveKitRoomAuthoritySnapshot(
      makeLease({
        leaseRef: "presence:community-two:1",
        communityRef: "community-two",
        channelRef: "other-channel",
        membershipRevision: digest("e"),
        e2eeKeyRevision: digest("f"),
        roomRef: "room:community-two:1",
        dispatchRef: "dispatch:two",
        sessionRef: "session:two",
        generation: 2,
        admissionDigest: digest("9"),
      }),
    );
    const first = request(groupOne, makeMember("one"), "nonce_group_one_00000000000000001");
    expect(first.accepted).toBe(true);
    expect(
      request(
        groupTwo,
        makeMember("two", {
          membershipRevision: digest("e"),
          roomRef: "room:community-two:1",
        }),
        "nonce_group_two_00000000000000002",
      ).accepted,
    ).toBe(true);
    if (!first.accepted) return;
    expect(
      transferSarahLiveKitFloor(first.snapshot, {
        actor: makeMember("one"),
        target: makeMember("two", {
          membershipRevision: digest("e"),
          roomRef: "room:community-two:1",
        }),
        nonce: "nonce_cross_group_000000000000001",
        requestedLeaseMs: 10_000,
        nowMs: issuedAtMs + 2_000,
      }),
    ).toEqual({ accepted: false, reason: "transfer_target_invalid" });
    expect(canonicalSarahLiveKitRoomFloorAuthority(first.value)).not.toContain("session:two");
  });

  test("schema rejects private capability and owner-context additions", () => {
    expect(() =>
      decodeSarahLiveKitRoomPresenceLease({
        ...makeLease(),
        capabilityProfile: "private_owner_v1",
      }),
    ).toThrow();
    expect(() =>
      decodeSarahLiveKitRoomPresenceLease({
        ...makeLease(),
        ownerMemory: "private",
      }),
    ).toThrow();
  });
});
