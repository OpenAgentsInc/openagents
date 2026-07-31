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

  test("creates from live binding and applies replay state with revision CAS", async () => {
    await expect(store.create(snapshot, now)).resolves.toEqual(snapshot);
    await store.bindParticipant({
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
      now,
    });
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

  test("fails closed when the durable room binding is no longer active", async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleanup_ready',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    const current = await store.read(snapshot.presence.leaseRef);
    expect(current).toBeDefined();
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
    await expect(
      store.readParticipantBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        ownerUserId: "owner.authority",
        now,
      }),
    ).resolves.toBeUndefined();
  });
});
