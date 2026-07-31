import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomAuthoritySnapshot,
} from "@openagentsinc/audio-contract";
import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  PostgresSarahLiveKitRoomAuthorityStore,
  SarahLiveKitRoomAuthorityStoreError,
} from "./sarah-livekit-room-authority-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const now = "2026-07-31T12:00:00.000Z";
const digest = (character: string): string => character.repeat(64);

const snapshot = decodeSarahLiveKitRoomAuthoritySnapshot({
  revision: 1,
  presence: {
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    sarahPubkey: digest("a"),
    leaseRef: "presence.room.authority",
    communityRef: "community.authority",
    channelRef: "channel.authority",
    membershipRevision: digest("b"),
    e2eeKeyRevision: digest("c"),
    roomRef: "room.authority",
    roomEpoch: 1,
    sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    dispatchRef: "dispatch.authority",
    sessionRef: "session.authority",
    generation: 1,
    capabilityProfile: "community_member_v1",
    admissionDigest: digest("d"),
    processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    cohortPolicy: "authenticated_allowlisted",
    issuedAtMs: Date.parse(now),
    expiresAtMs: Date.parse(now) + 60_000,
  },
  presenceActive: true,
  floor: {
    state: "available",
    presenceLeaseRef: "presence.room.authority",
    issuance: 0,
  },
  usedNonceDigests: [],
  rateBuckets: [],
  nextInterruptSequence: 1,
});

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit room authority store", () => {
  let postgres: LocalPostgres;
  let sql: SQL;
  let store: PostgresSarahLiveKitRoomAuthorityStore;

  beforeAll(async () => {
    postgres = await startLocalPostgres();
    const admin = SQL({ url: postgres.url, max: 1 });
    await admin.unsafe("CREATE DATABASE sarah_livekit_room_authority");
    await admin.end();
    const databaseUrl = postgres.urlFor("sarah_livekit_room_authority");
    await runMigrations({ databaseUrl });
    sql = SQL({ url: databaseUrl, max: 4 });
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},'reservation.authority','owner.authority',
        'actor.authority','device.authority','thread.authority',1,'disclosure.authority',
        'connected',1000,${new Date(Date.parse(now) + 60_000).toISOString()},
        ${new Date(Date.parse(now) + 60_000).toISOString()},1000,${now},${now})`;
    await sql`
      INSERT INTO sarah_livekit_room_bindings
        (session_ref,owner_user_id,device_ref,thread_ref,generation,capability_profile,
         admission_ref,admission_digest,room_context_kind,community_ref,channel_ref,
         membership_revision,room_ref,room_epoch,participant_ref,sarah_participant_ref,
         participant_grant_digest,join_expires_at,dispatch_ref,sarah_presence_lease_ref,
         publish_allowed,subscribe_allowed,state,owner_joined_at,sarah_joined_at,
         created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},'owner.authority','device.authority',
        'thread.authority',1,'community_member_v1','admission.authority',
        ${snapshot.presence.admissionDigest},'community',${snapshot.presence.communityRef},
        ${snapshot.presence.channelRef},${snapshot.presence.membershipRevision},
        ${snapshot.presence.roomRef},1,'participant.owner',${snapshot.presence.sarahParticipantRef},
        ${digest("e")},${new Date(Date.parse(now) + 60_000).toISOString()},
        ${snapshot.presence.dispatchRef},${snapshot.presence.leaseRef},true,true,'active',
        ${now},${now},${now},${now})`;
    store = new PostgresSarahLiveKitRoomAuthorityStore(sql as unknown as SyncSql);
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (postgres !== undefined) await postgres.stop();
  });

  test("reads a prepared community room while its worker connection is pending", async () => {
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET state='reserved',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='prepared',owner_joined_at=NULL,sarah_joined_at=NULL,updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    try {
      await expect(
        store.readCommunityRoomBinding({
          presenceLeaseRef: snapshot.presence.leaseRef,
          ownerUserId: "owner.authority",
          now,
        }),
      ).resolves.toMatchObject({
        ownerUserId: "owner.authority",
        sessionRef: snapshot.presence.sessionRef,
        generation: snapshot.presence.generation,
        roomRef: snapshot.presence.roomRef,
        roomEpoch: snapshot.presence.roomEpoch,
      });
      await expect(store.create(snapshot, now)).resolves.toEqual(snapshot);
    } finally {
      await sql`
        DELETE FROM sarah_livekit_room_authorities
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
      await sql`
        UPDATE sarah_realtime_voice_sessions
        SET state='connected',updated_at=${now}
        WHERE session_ref=${snapshot.presence.sessionRef}`;
      await sql`
        UPDATE sarah_livekit_room_bindings
        SET state='active',owner_joined_at=${now},sarah_joined_at=${now},updated_at=${now}
        WHERE session_ref=${snapshot.presence.sessionRef}`;
    }
  });

  test("creates from live binding and applies replay state with revision CAS", async () => {
    await expect(store.create(snapshot, now)).resolves.toEqual(snapshot);
    await store.bindParticipant({
      presenceLeaseRef: snapshot.presence.leaseRef,
      ownerUserId: "owner.authority",
      userRefDigest: digest("1"),
      memberPubkey: digest("2"),
      deviceRefDigest: digest("d"),
      participantRef: "participant.owner",
      membershipRevision: snapshot.presence.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      participantGrantDigest: digest("e"),
      joinExpiresAt: new Date(Date.parse(now) + 60_000).toISOString(),
      now,
    });
    const seat = {
      presenceLeaseRef: snapshot.presence.leaseRef,
      ownerUserId: "owner.authority",
      userRefDigest: digest("1"),
      memberPubkey: digest("2"),
      participantRef: "participant.owner",
      membershipRevision: snapshot.presence.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      participantGrantDigest: digest("e"),
      joinExpiresAt: new Date(Date.parse(now) + 60_000).toISOString(),
    } as const;
    try {
      // The member's own client resuming its seat refreshes the grant.
      await store.bindParticipant({
        ...seat,
        deviceRefDigest: digest("d"),
        participantGrantDigest: digest("f"),
        now: new Date(Date.parse(now) + 1_000).toISOString(),
      });
      const [resumed] = await sql`
        SELECT participant_grant_digest, device_ref_digest
        FROM sarah_livekit_room_members
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
      expect(resumed).toMatchObject({
        participant_grant_digest: digest("f"),
        device_ref_digest: digest("d"),
      });
      // A second client of the same member would share one LiveKit identity.
      await expect(
        store.bindParticipant({
          ...seat,
          deviceRefDigest: digest("c"),
          now: new Date(Date.parse(now) + 2_000).toISOString(),
        }),
      ).rejects.toMatchObject({ reason: "duplicate_participant" });
      // Once the seat's join window closes, the same member may be re-admitted
      // from another client: that is a reconnect, not a duplicate.
      const afterJoinWindow = new Date(Date.parse(seat.joinExpiresAt) + 1_000).toISOString();
      await store.bindParticipant({
        ...seat,
        deviceRefDigest: digest("c"),
        joinExpiresAt: new Date(Date.parse(afterJoinWindow) + 60_000).toISOString(),
        now: afterJoinWindow,
      });
      const [reconnected] = await sql`
        SELECT device_ref_digest
        FROM sarah_livekit_room_members
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
      expect(reconnected).toMatchObject({ device_ref_digest: digest("c") });
    } finally {
      await sql`
        UPDATE sarah_livekit_room_members
        SET device_ref_digest=${digest("d")},
            participant_grant_digest=${digest("e")},
            join_expires_at=${seat.joinExpiresAt},
            updated_at=${now}
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    }
    await expect(store.claimCommunityRoomRendezvous(snapshot, now)).resolves.toEqual({
      claimed: true,
      presenceLeaseRef: snapshot.presence.leaseRef,
    });
    await expect(store.claimCommunityRoomRendezvous(snapshot, now)).resolves.toEqual({
      claimed: true,
      presenceLeaseRef: snapshot.presence.leaseRef,
    });
    await expect(
      store.readActiveCommunityRoomRendezvous({
        communityRef: snapshot.presence.communityRef,
        channelRef: snapshot.presence.channelRef,
        now,
      }),
    ).resolves.toEqual({
      presenceLeaseRef: snapshot.presence.leaseRef,
      membershipRevision: snapshot.presence.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      sessionRef: snapshot.presence.sessionRef,
      generation: snapshot.presence.generation,
    });
    await expect(
      store.listActiveCommunityRoomParticipants({
        presenceLeaseRef: snapshot.presence.leaseRef,
        now,
      }),
    ).resolves.toEqual([
      {
        ownerUserId: "owner.authority",
        userRefDigest: digest("1"),
        memberPubkey: digest("2"),
        participantRef: "participant.owner",
        membershipRevision: snapshot.presence.membershipRevision,
        roomRef: snapshot.presence.roomRef,
        roomEpoch: snapshot.presence.roomEpoch,
      },
    ]);
    const next = decodeSarahLiveKitRoomAuthoritySnapshot({
      ...snapshot,
      revision: 2,
      usedNonceDigests: [digest("f")],
      rateBuckets: [
        {
          userRefDigest: digest("1"),
          windowStartedAtMs: Date.parse(now),
          requestCount: 1,
        },
      ],
    });
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 1,
        snapshot: next,
        now,
      }),
    ).resolves.toEqual(next);
    await expect(store.read(snapshot.presence.leaseRef)).resolves.toEqual(next);
    await expect(
      store.readParticipantBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        ownerUserId: "owner.authority",
        now,
      }),
    ).resolves.toEqual({
      ownerUserId: "owner.authority",
      userRefDigest: digest("1"),
      memberPubkey: digest("2"),
      participantRef: "participant.owner",
      communityRef: snapshot.presence.communityRef,
      channelRef: snapshot.presence.channelRef,
      membershipRevision: snapshot.presence.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
    });
    await expect(
      store.readParticipantBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        userRefDigest: digest("9"),
        now,
      }),
    ).resolves.toBeUndefined();
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 1,
        snapshot: next,
        now,
      }),
    ).rejects.toMatchObject({ reason: "stale_revision" });
  });

  test("rejects an authority identity change", async () => {
    const current = await store.read(snapshot.presence.leaseRef);
    expect(current).toBeDefined();
    if (current === undefined) throw new Error("missing room authority");
    const changed = decodeSarahLiveKitRoomAuthoritySnapshot({
      ...current,
      revision: 3,
      presence: { ...snapshot.presence, e2eeKeyRevision: digest("9") },
    });
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 2,
        snapshot: changed,
        now,
      }),
    ).rejects.toBeInstanceOf(SarahLiveKitRoomAuthorityStoreError);
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 2,
        snapshot: changed,
        now,
      }),
    ).rejects.toMatchObject({ reason: "authority_mismatch" });
  });

  test("permits only terminal presence removal after the room enters cleanup", async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleanup_ready',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    const current = await store.read(snapshot.presence.leaseRef);
    expect(current).toBeDefined();
    if (current === undefined) throw new Error("missing room authority");
    const next = decodeSarahLiveKitRoomAuthoritySnapshot({
      ...current,
      revision: 3,
      nextInterruptSequence: 2,
    });
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 2,
        snapshot: next,
        now,
      }),
    ).rejects.toMatchObject({ reason: "authority_mismatch" });
    const removed = decodeSarahLiveKitRoomAuthoritySnapshot({
      ...current,
      revision: 3,
      presenceActive: false,
      nextInterruptSequence: 2,
      floor: {
        state: "stopped",
        presenceLeaseRef: snapshot.presence.leaseRef,
        issuance:
          current.floor.state === "held"
            ? current.floor.lease.issuance
            : current.floor.issuance,
        reason: "sarah_removed",
      },
    });
    await expect(
      store.compareAndSwap({
        presenceLeaseRef: snapshot.presence.leaseRef,
        expectedRevision: 2,
        snapshot: removed,
        now,
      }),
    ).resolves.toEqual(removed);
    await expect(
      store.readParticipantBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        ownerUserId: "owner.authority",
        now,
      }),
    ).resolves.toBeUndefined();
    await expect(
      store.retireCommunityRoomRendezvous({
        presenceLeaseRef: snapshot.presence.leaseRef,
        now,
      }),
    ).resolves.toBe(true);
  });
});
