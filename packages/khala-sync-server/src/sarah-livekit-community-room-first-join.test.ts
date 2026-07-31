import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomAuthoritySnapshot,
} from "@openagentsinc/audio-contract";
import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { PostgresSarahLiveKitRoomAuthorityStore } from "./sarah-livekit-room-authority-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

/**
 * EP263-LK regression: the first canonical community-room join.
 *
 * The 2026-07-31 production acceptance failed at
 * `POST /api/sarah/livekit/room/join` with `404 community_room_not_active`
 * while the community accounting session was still `reserved`, the binding
 * was still `prepared`, and the rendezvous was `active`.
 *
 * That is the deadlock this file pins down. The Sarah LiveKit worker only
 * emits `provider_admitted` (which is what moves the accounting session from
 * `reserved` to `connected` and the binding from `prepared` to `active`)
 * after the dispatched member participant actually appears in the LiveKit
 * room, and a member can only appear in the room after this join route mints
 * their participant grant. Requiring `session.state='connected'` to discover
 * the rendezvous therefore requires the join that the discovery gates.
 *
 * The corrected predicate admits the bounded pre-admission window and nothing
 * else: stopped, closed, cleaned, released, expired, retired, and
 * epoch-stale rooms must stay undiscoverable.
 */

const now = "2026-07-31T12:00:00.000Z";
const nowMs = Date.parse(now);
const later = new Date(nowMs + 60_000).toISOString();
const digest = (character: string): string => character.repeat(64);

const snapshot = decodeSarahLiveKitRoomAuthoritySnapshot({
  revision: 1,
  presence: {
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    sarahPubkey: digest("a"),
    leaseRef: "presence.room.first-join",
    communityRef: "community.first-join",
    channelRef: "channel.first-join",
    membershipRevision: digest("b"),
    e2eeKeyRevision: digest("c"),
    roomRef: "room.first-join",
    roomEpoch: 1,
    sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    dispatchRef: "dispatch.first-join",
    sessionRef: "session.first-join",
    generation: 1,
    capabilityProfile: "community_member_v1",
    admissionDigest: digest("d"),
    processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    cohortPolicy: "authenticated_allowlisted",
    issuedAtMs: nowMs,
    expiresAtMs: nowMs + 60_000,
  },
  presenceActive: true,
  floor: {
    state: "available",
    presenceLeaseRef: "presence.room.first-join",
    issuance: 0,
  },
  usedNonceDigests: [],
  rateBuckets: [],
  nextInterruptSequence: 1,
});

const activeRendezvous = {
  presenceLeaseRef: snapshot.presence.leaseRef,
  membershipRevision: snapshot.presence.membershipRevision,
  roomRef: snapshot.presence.roomRef,
  roomEpoch: snapshot.presence.roomEpoch,
  sessionRef: snapshot.presence.sessionRef,
  generation: snapshot.presence.generation,
} as const;

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit community room first join", () => {
  let postgres: LocalPostgres;
  let sql: SQL;
  let store: PostgresSarahLiveKitRoomAuthorityStore;

  const readRendezvous = async () =>
    store.readActiveCommunityRoomRendezvous({
      communityRef: snapshot.presence.communityRef,
      channelRef: snapshot.presence.channelRef,
      now,
    });

  beforeAll(async () => {
    postgres = await startLocalPostgres();
    const admin = SQL({ url: postgres.url, max: 1 });
    await admin.unsafe("CREATE DATABASE sarah_livekit_community_first_join");
    await admin.end();
    const databaseUrl = postgres.urlFor("sarah_livekit_community_first_join");
    await runMigrations({ databaseUrl });
    sql = SQL({ url: databaseUrl, max: 4 });

    // The exact state observed at the failed production join boundary: the
    // accounting session is reserved, the room binding is prepared, and the
    // Sarah worker has been dispatched but has not admitted its provider.
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},'reservation.first-join','owner.first-join',
        'actor.first-join','device.first-join','thread.first-join',1,'disclosure.first-join',
        'reserved',1000,${later},${later},1000,${now},${now})`;
    await sql`
      INSERT INTO sarah_livekit_room_bindings
        (session_ref,owner_user_id,device_ref,thread_ref,generation,capability_profile,
         admission_ref,admission_digest,room_context_kind,community_ref,channel_ref,
         membership_revision,room_ref,room_epoch,participant_ref,sarah_participant_ref,
         participant_grant_digest,join_expires_at,dispatch_ref,sarah_presence_lease_ref,
         publish_allowed,subscribe_allowed,state,created_at,updated_at)
      VALUES (${snapshot.presence.sessionRef},'owner.first-join','device.first-join',
        'thread.first-join',1,'community_member_v1','admission.first-join',
        ${snapshot.presence.admissionDigest},'community',${snapshot.presence.communityRef},
        ${snapshot.presence.channelRef},${snapshot.presence.membershipRevision},
        ${snapshot.presence.roomRef},1,'participant.owner',${snapshot.presence.sarahParticipantRef},
        ${digest("e")},${later},${snapshot.presence.dispatchRef},${snapshot.presence.leaseRef},
        true,true,'prepared',${now},${now})`;

    store = new PostgresSarahLiveKitRoomAuthorityStore(sql as unknown as SyncSql);
    await store.create(snapshot, now);
    await store.claimCommunityRoomRendezvous(snapshot, now);
  });

  afterEach(async () => {
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET state='reserved',session_expires_at=${later},updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='prepared',worker_stop_reason=NULL,worker_stop_close_reason=NULL,
          worker_stop_requested_at=NULL,worker_stop_deadline_at=NULL,
          worker_closed_at=NULL,updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await sql`
      UPDATE sarah_livekit_community_room_rendezvous
      SET state='active',retired_at=NULL,expires_at=${later},updated_at=${now}
      WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (postgres !== undefined) await postgres.stop();
  });

  test("resolves the claimed rendezvous before the worker admits its provider", async () => {
    // Reproduces the production `404 community_room_not_active`: without this
    // the first member can never join, so the worker can never connect the
    // accounting session that the read was demanding.
    await expect(readRendezvous()).resolves.toEqual(activeRendezvous);
  });

  test("still resolves once the worker has connected the accounting session", async () => {
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET state='connected',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='active',sarah_joined_at=${now},updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toEqual(activeRendezvous);
  });

  test("refuses a room whose worker reported a stop reason", async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET worker_stop_reason='operator_stop',worker_stop_close_reason='operator_stop',
          worker_stop_requested_at=${now},worker_stop_deadline_at=${later},updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses a room whose worker closed", async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET worker_closed_at=${now},updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses a binding that entered cleanup", async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleanup_ready',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleaned',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses a released accounting session", async () => {
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET state='released',updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses an expired accounting session", async () => {
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET session_expires_at=${new Date(nowMs - 1_000).toISOString()},updated_at=${now}
      WHERE session_ref=${snapshot.presence.sessionRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses a retired or expired rendezvous", async () => {
    await store.retireCommunityRoomRendezvous({
      presenceLeaseRef: snapshot.presence.leaseRef,
      now,
    });
    await expect(readRendezvous()).resolves.toBeUndefined();
    await sql`
      UPDATE sarah_livekit_community_room_rendezvous
      SET state='active',retired_at=NULL,
          expires_at=${new Date(nowMs - 1_000).toISOString()},updated_at=${now}
      WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    await expect(readRendezvous()).resolves.toBeUndefined();
  });

  test("refuses a rendezvous whose room epoch no longer matches its authority", async () => {
    await sql`
      UPDATE sarah_livekit_community_room_rendezvous
      SET room_epoch=2,updated_at=${now}
      WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    try {
      await expect(readRendezvous()).resolves.toBeUndefined();
    } finally {
      await sql`
        UPDATE sarah_livekit_community_room_rendezvous
        SET room_epoch=1,updated_at=${now}
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    }
  });

  test("refuses a rendezvous whose membership revision no longer matches its authority", async () => {
    await sql`
      UPDATE sarah_livekit_community_room_rendezvous
      SET membership_revision=${digest("9")},updated_at=${now}
      WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    try {
      await expect(readRendezvous()).resolves.toBeUndefined();
    } finally {
      await sql`
        UPDATE sarah_livekit_community_room_rendezvous
        SET membership_revision=${snapshot.presence.membershipRevision},updated_at=${now}
        WHERE presence_lease_ref=${snapshot.presence.leaseRef}`;
    }
  });
});
