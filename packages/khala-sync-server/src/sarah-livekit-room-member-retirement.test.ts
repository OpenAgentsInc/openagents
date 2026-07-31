import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomAuthoritySnapshot,
  type SarahLiveKitRoomAuthoritySnapshot,
} from "@openagentsinc/audio-contract";
import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { PostgresSarahLiveKitRoomAuthorityStore } from "./sarah-livekit-room-authority-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

/**
 * EP263-LK H5 (#9282): shared-room membership must retire.
 *
 * `sarah_livekit_room_members` rows were never retired. `removeParticipant`
 * had no production caller, and the worker-close handler retired the community
 * rendezvous beside these rows without touching the rows themselves, so every
 * member of every room that ever existed stayed `state='active'` with a null
 * `removed_at` forever.
 *
 * `retireRoomMembers` closes the room-close path and `retireExpiredRoomMembers`
 * is the backstop for rooms that died before that path existed and have no
 * close event left to fire. The load-bearing predicate here is convergence:
 * seeded with the exact production shape, the sweep must drive the count of
 * stale active members back to zero.
 */

const base = "2026-07-31T12:00:00.000Z";
const baseMs = Date.parse(base);
/** Explicit offsets from one fixed instant keep every timestamp deterministic. */
const at = (seconds: number): string => new Date(baseMs + seconds * 1_000).toISOString();
const digest = (character: string): string => character.repeat(64);

/** The join window every stale member in this fixture already closed. */
const closedJoinWindow = at(60);
/** A join window still open at the sweep instant. */
const openJoinWindow = at(7_200);
const sweepNow = at(120);

const leaseA = "presence.retire.a";
const leaseB = "presence.retire.b";

const makeSnapshot = (suffix: string): SarahLiveKitRoomAuthoritySnapshot =>
  decodeSarahLiveKitRoomAuthoritySnapshot({
    revision: 1,
    presence: {
      schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
      principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
      sarahPubkey: digest("a"),
      leaseRef: `presence.retire.${suffix}`,
      communityRef: `community.retire.${suffix}`,
      channelRef: `channel.retire.${suffix}`,
      membershipRevision: digest("b"),
      e2eeKeyRevision: digest("c"),
      roomRef: `room.retire.${suffix}`,
      roomEpoch: 1,
      sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
      dispatchRef: `dispatch.retire.${suffix}`,
      sessionRef: `session.retire.${suffix}`,
      generation: 1,
      capabilityProfile: "community_member_v1",
      admissionDigest: digest("d"),
      processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
      cohortPolicy: "authenticated_allowlisted",
      issuedAtMs: baseMs,
      expiresAtMs: baseMs + 7_200_000,
    },
    presenceActive: true,
    floor: {
      state: "available",
      presenceLeaseRef: `presence.retire.${suffix}`,
      issuance: 0,
    },
    usedNonceDigests: [],
    rateBuckets: [],
    nextInterruptSequence: 1,
  });

const snapshotA = makeSnapshot("a");
const snapshotB = makeSnapshot("b");

/** The seeded members: two rooms, three of whose members are already stale. */
const seededMembers = [
  {
    snapshot: snapshotA,
    ownerUserId: "member.a1",
    userRefDigest: digest("1"),
    participantRef: "participant.retire.a1",
    joinExpiresAt: closedJoinWindow,
  },
  {
    snapshot: snapshotA,
    ownerUserId: "member.a2",
    userRefDigest: digest("2"),
    participantRef: "participant.retire.a2",
    joinExpiresAt: closedJoinWindow,
  },
  {
    snapshot: snapshotA,
    ownerUserId: "member.a3",
    userRefDigest: digest("3"),
    participantRef: "participant.retire.a3",
    joinExpiresAt: openJoinWindow,
  },
  {
    snapshot: snapshotB,
    ownerUserId: "member.b1",
    userRefDigest: digest("4"),
    participantRef: "participant.retire.b1",
    joinExpiresAt: closedJoinWindow,
  },
  {
    snapshot: snapshotB,
    ownerUserId: "member.b2",
    userRefDigest: digest("5"),
    participantRef: "participant.retire.b2",
    joinExpiresAt: openJoinWindow,
  },
] as const;

interface MemberRow {
  readonly presence_lease_ref: string;
  readonly owner_user_id: string;
  readonly state: string;
  readonly removed_at: string | null;
  readonly join_expires_at: string;
}

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit room member retirement", () => {
  let postgres: LocalPostgres;
  let sql: SQL;
  let store: PostgresSarahLiveKitRoomAuthorityStore;

  const readMembers = async (): Promise<ReadonlyArray<MemberRow>> =>
    (await sql`
      SELECT presence_lease_ref,owner_user_id,state,removed_at,join_expires_at
      FROM sarah_livekit_room_members
      ORDER BY owner_user_id`) as ReadonlyArray<MemberRow>;

  const activeOwners = async (presenceLeaseRef?: string): Promise<ReadonlyArray<string>> =>
    (await readMembers())
      .filter(
        (member) =>
          member.state === "active" &&
          (presenceLeaseRef === undefined || member.presence_lease_ref === presenceLeaseRef),
      )
      .map((member) => member.owner_user_id);

  /** Exactly the production defect: active, never removed, window long closed. */
  const staleActiveCount = async (now: string): Promise<number> =>
    (await readMembers()).filter(
      (member) =>
        member.state === "active" && member.removed_at === null && member.join_expires_at <= now,
    ).length;

  const seedRoom = async (snapshot: SarahLiveKitRoomAuthoritySnapshot): Promise<void> => {
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},
        ${`reservation.${snapshot.presence.sessionRef}`},
        ${`owner.${snapshot.presence.sessionRef}`},${`actor.${snapshot.presence.sessionRef}`},
        'device.retire','thread.retire',1,'disclosure.retire','connected',1000,
        ${at(7_200)},${at(7_200)},1000,${base},${base})`;
    await sql`
      INSERT INTO sarah_livekit_room_bindings
        (session_ref,owner_user_id,device_ref,thread_ref,generation,capability_profile,
         admission_ref,admission_digest,room_context_kind,community_ref,channel_ref,
         membership_revision,room_ref,room_epoch,participant_ref,sarah_participant_ref,
         participant_grant_digest,join_expires_at,dispatch_ref,sarah_presence_lease_ref,
         publish_allowed,subscribe_allowed,state,owner_joined_at,sarah_joined_at,
         created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},${`owner.${snapshot.presence.sessionRef}`},
        'device.retire','thread.retire',1,'community_member_v1',
        ${`admission.${snapshot.presence.sessionRef}`},${snapshot.presence.admissionDigest},
        'community',${snapshot.presence.communityRef},${snapshot.presence.channelRef},
        ${snapshot.presence.membershipRevision},${snapshot.presence.roomRef},1,
        ${`participant.owner.${snapshot.presence.sessionRef}`},
        ${snapshot.presence.sarahParticipantRef},${digest("e")},${at(7_200)},
        ${snapshot.presence.dispatchRef},${snapshot.presence.leaseRef},true,true,'active',
        ${base},${base},${base},${base})`;
    await store.create(snapshot, base);
  };

  beforeAll(async () => {
    postgres = await startLocalPostgres();
    const admin = SQL({ url: postgres.url, max: 1 });
    await admin.unsafe("CREATE DATABASE sarah_livekit_room_member_retirement");
    await admin.end();
    const databaseUrl = postgres.urlFor("sarah_livekit_room_member_retirement");
    await runMigrations({ databaseUrl });
    sql = SQL({ url: databaseUrl, max: 4 });
    store = new PostgresSarahLiveKitRoomAuthorityStore(sql as unknown as SyncSql);
    await seedRoom(snapshotA);
    await seedRoom(snapshotB);
    for (const member of seededMembers) {
      await store.bindParticipant({
        presenceLeaseRef: member.snapshot.presence.leaseRef,
        ownerUserId: member.ownerUserId,
        userRefDigest: member.userRefDigest,
        memberPubkey: member.userRefDigest,
        participantRef: member.participantRef,
        membershipRevision: member.snapshot.presence.membershipRevision,
        roomRef: member.snapshot.presence.roomRef,
        roomEpoch: member.snapshot.presence.roomEpoch,
        participantGrantDigest: digest("e"),
        joinExpiresAt: member.joinExpiresAt,
        now: base,
      });
    }
  });

  afterEach(async () => {
    await sql`
      UPDATE sarah_livekit_room_members
      SET state='active',removed_at=NULL,updated_at=${base}`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (postgres !== undefined) await postgres.stop();
  });

  test("retires every active member of one room and stays idempotent", async () => {
    expect(await store.retireRoomMembers({ presenceLeaseRef: leaseA, now: sweepNow })).toBe(3);
    expect(
      (await readMembers())
        .filter((member) => member.presence_lease_ref === leaseA)
        .map((member) => ({
          ownerUserId: member.owner_user_id,
          state: member.state,
          removedAt: member.removed_at,
        })),
    ).toEqual([
      { ownerUserId: "member.a1", state: "removed", removedAt: sweepNow },
      { ownerUserId: "member.a2", state: "removed", removedAt: sweepNow },
      // Retired with the rest: a room close retires its whole membership, not
      // only the members whose join window happened to lapse first.
      { ownerUserId: "member.a3", state: "removed", removedAt: sweepNow },
    ]);
    expect(await store.retireRoomMembers({ presenceLeaseRef: leaseA, now: sweepNow })).toBe(0);
    expect(await activeOwners(leaseB)).toEqual(["member.b1", "member.b2"]);
  });

  test("retires only members whose join window closed, bounded by the requested limit", async () => {
    expect(await store.retireExpiredRoomMembers({ now: sweepNow, limit: 2 })).toBe(2);
    expect(await store.retireExpiredRoomMembers({ now: sweepNow, limit: 2 })).toBe(1);
    expect(await store.retireExpiredRoomMembers({ now: sweepNow, limit: 2 })).toBe(0);
    expect(await activeOwners()).toEqual(["member.a3", "member.b2"]);
    expect(
      (await readMembers())
        .filter((member) => member.state === "removed")
        .map((member) => ({ ownerUserId: member.owner_user_id, removedAt: member.removed_at })),
    ).toEqual([
      { ownerUserId: "member.a1", removedAt: sweepNow },
      { ownerUserId: "member.a2", removedAt: sweepNow },
      { ownerUserId: "member.b1", removedAt: sweepNow },
    ]);
  });

  test("converges the stale active membership back to zero", async () => {
    // The seeded shape is the production shape: members left `active` with a
    // null `removed_at` long after their join window closed.
    expect(await staleActiveCount(sweepNow)).toBe(3);
    let retired = 0;
    // A generous bound: if the sweep does not converge the assertions below
    // fail rather than the test running forever.
    for (let pass = 0; pass < 16; pass += 1) {
      const swept = await store.retireExpiredRoomMembers({ now: sweepNow, limit: 1 });
      if (swept === 0) break;
      retired += swept;
    }
    expect(retired).toBe(3);
    expect(await staleActiveCount(sweepNow)).toBe(0);
    expect(await activeOwners()).toEqual(["member.a3", "member.b2"]);
  });

  test("holds the retired member shape the table enforces", async () => {
    await store.retireRoomMembers({ presenceLeaseRef: leaseA, now: sweepNow });
    const removed = (await readMembers()).filter((member) => member.state === "removed");
    expect(removed).toHaveLength(3);
    expect(removed.every((member) => member.removed_at !== null)).toBe(true);
    // The table, not only the writer, refuses a retirement with no moment.
    await expect(
      sql`
        UPDATE sarah_livekit_room_members
        SET state='removed',removed_at=NULL,updated_at=${sweepNow}
        WHERE presence_lease_ref=${leaseB} AND owner_user_id='member.b2'`,
    ).rejects.toThrow();
  });
});
