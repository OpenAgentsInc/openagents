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
import {
  SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS,
  SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS,
  SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS,
  SarahVoiceSessionRejectedError,
  makeSarahRealtimeVoiceStore,
} from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

/**
 * EP263-LK H4 (#9282): LiveKit room cleanup must converge.
 *
 * `claimLiveKitCleanups` re-selected every `cleanup_failed` binding behind a
 * flat staleness gate with no attempt count and no terminal state, so a room
 * the broker can never delete was retried for as long as the service ran.
 * Production carried fifteen such bindings, re-attempted continuously for
 * hours.
 *
 * The load-bearing predicate in this file is convergence: a cleanup that
 * always fails reaches `cleanup_abandoned` after exactly
 * `SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS` attempts and is never claimed again.
 * The retry ladder between the first failure and that give-up is asserted
 * against the stored `cleanup_next_attempt_at`, never by sleeping.
 *
 * `cleanup_abandoned` is a resource state, not an authority state. The last
 * test pins that it stays outside the `binding.state IN ('prepared','active')`
 * predicates exactly as `cleanup_failed` already did, so giving up on a room
 * can never widen what a member is allowed to join.
 */

const base = "2026-07-31T12:00:00.000Z";
const baseMs = Date.parse(base);
/** Explicit offsets from one fixed instant keep every timestamp deterministic. */
const at = (seconds: number): string => new Date(baseMs + seconds * 1_000).toISOString();
const digest = (character: string): string => character.repeat(64);

/** The binding under cleanup: terminal accounting, no live authority. */
const cleanupSessionRef = "session.cleanup.convergence";
/** A live community binding used only to probe the authority predicates. */
const authoritySessionRef = "session.authority.convergence";

const snapshot = decodeSarahLiveKitRoomAuthoritySnapshot({
  revision: 1,
  presence: {
    schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
    principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    sarahPubkey: digest("a"),
    leaseRef: "presence.authority.convergence",
    communityRef: "community.convergence",
    channelRef: "channel.convergence",
    membershipRevision: digest("b"),
    e2eeKeyRevision: digest("c"),
    roomRef: "room.authority.convergence",
    roomEpoch: 1,
    sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
    dispatchRef: "dispatch.authority.convergence",
    sessionRef: authoritySessionRef,
    generation: 1,
    capabilityProfile: "community_member_v1",
    admissionDigest: digest("d"),
    processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
    cohortPolicy: "authenticated_allowlisted",
    issuedAtMs: baseMs,
    expiresAtMs: baseMs + 3_600_000,
  },
  presenceActive: true,
  floor: {
    state: "available",
    presenceLeaseRef: "presence.authority.convergence",
    issuance: 0,
  },
  usedNonceDigests: [],
  rateBuckets: [],
  nextInterruptSequence: 1,
});

interface CleanupBindingRow {
  readonly state: string;
  readonly cleanup_attempt_count: number | string;
  readonly cleanup_attempted_at: string | null;
  readonly cleanup_next_attempt_at: string | null;
  readonly cleanup_abandoned_at: string | null;
  readonly cleaned_at: string | null;
}

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit cleanup convergence", () => {
  let postgres: LocalPostgres;
  let sql: SQL;
  let store: ReturnType<typeof makeSarahRealtimeVoiceStore>;
  let authorityStore: PostgresSarahLiveKitRoomAuthorityStore;

  const readCleanupBinding = async (): Promise<CleanupBindingRow> => {
    const rows = (await sql`
      SELECT state,cleanup_attempt_count,cleanup_attempted_at,cleanup_next_attempt_at,
        cleanup_abandoned_at,cleaned_at
      FROM sarah_livekit_room_bindings
      WHERE session_ref=${cleanupSessionRef}`) as ReadonlyArray<CleanupBindingRow>;
    const row = rows[0];
    if (row === undefined) throw new Error("missing cleanup binding");
    return row;
  };

  /** The reconciler's only claim shape: one pass at one instant. */
  const claimAt = async (instant: string) =>
    store.claimLiveKitCleanups({ staleBeforeIso: instant, nowIso: instant });

  const markFailedAt = async (instant: string) =>
    store.markLiveKitCleanup({
      sessionRef: cleanupSessionRef,
      generation: 1,
      state: "cleanup_failed",
      nowIso: instant,
    });

  beforeAll(async () => {
    postgres = await startLocalPostgres();
    const admin = SQL({ url: postgres.url, max: 1 });
    await admin.unsafe("CREATE DATABASE sarah_livekit_cleanup_convergence");
    await admin.end();
    const databaseUrl = postgres.urlFor("sarah_livekit_cleanup_convergence");
    await runMigrations({ databaseUrl });
    sql = SQL({ url: databaseUrl, max: 4 });

    // The exact production shape of a stuck cleanup: accounting is terminal
    // and receipted, so the binding is eligible for cleanup, and the room the
    // broker refuses to delete is waiting in `cleanup_ready`.
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,settlement_receipt_ref,settled_at,
         created_at,updated_at)
      VALUES (${cleanupSessionRef},'reservation.cleanup.convergence','owner.cleanup',
        'actor.cleanup','device.cleanup','thread.cleanup',1,'disclosure.cleanup',
        'settled',1000,${at(3_600)},${at(3_600)},1000,'receipt.cleanup.convergence',
        ${base},${base},${base})`;
    await sql`
      INSERT INTO sarah_livekit_room_bindings
        (session_ref,owner_user_id,device_ref,thread_ref,generation,capability_profile,
         admission_ref,admission_digest,room_context_kind,room_ref,room_epoch,participant_ref,
         sarah_participant_ref,participant_grant_digest,join_expires_at,dispatch_ref,
         sarah_presence_lease_ref,publish_allowed,subscribe_allowed,state,created_at,updated_at)
      VALUES (${cleanupSessionRef},'owner.cleanup','device.cleanup','thread.cleanup',1,
        'mobile_voice_only','admission.cleanup.convergence',${digest("d")},'private',
        'room.cleanup.convergence',1,'participant.cleanup','sarah.participant.cleanup',
        ${digest("e")},${at(3_600)},'dispatch.cleanup.convergence',
        'presence.cleanup.convergence',true,true,'cleanup_ready',${base},${base})`;

    // A separate live community room, used only to prove that
    // `cleanup_abandoned` fails closed in the authority predicates.
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,created_at,updated_at)
      VALUES (${authoritySessionRef},'reservation.authority.convergence','owner.authority',
        'actor.authority','device.authority','thread.authority',1,'disclosure.authority',
        'connected',1000,${at(3_600)},${at(3_600)},1000,${base},${base})`;
    await sql`
      INSERT INTO sarah_livekit_room_bindings
        (session_ref,owner_user_id,device_ref,thread_ref,generation,capability_profile,
         admission_ref,admission_digest,room_context_kind,community_ref,channel_ref,
         membership_revision,room_ref,room_epoch,participant_ref,sarah_participant_ref,
         participant_grant_digest,join_expires_at,dispatch_ref,sarah_presence_lease_ref,
         publish_allowed,subscribe_allowed,state,owner_joined_at,sarah_joined_at,
         created_at,updated_at)
      VALUES (${authoritySessionRef},'owner.authority','device.authority','thread.authority',1,
        'community_member_v1','admission.authority.convergence',
        ${snapshot.presence.admissionDigest},'community',${snapshot.presence.communityRef},
        ${snapshot.presence.channelRef},${snapshot.presence.membershipRevision},
        ${snapshot.presence.roomRef},1,'participant.authority',
        ${snapshot.presence.sarahParticipantRef},${digest("e")},${at(3_600)},
        ${snapshot.presence.dispatchRef},${snapshot.presence.leaseRef},true,true,'active',
        ${base},${base},${base},${base})`;

    store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    authorityStore = new PostgresSarahLiveKitRoomAuthorityStore(sql as unknown as SyncSql);
    await authorityStore.create(snapshot, base);
    await authorityStore.bindParticipant({
      presenceLeaseRef: snapshot.presence.leaseRef,
      ownerUserId: "owner.authority",
      userRefDigest: digest("1"),
      memberPubkey: digest("2"),
      deviceRefDigest: digest("d"),
      participantRef: "participant.authority",
      membershipRevision: snapshot.presence.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      participantGrantDigest: digest("e"),
      joinExpiresAt: at(3_600),
      now: base,
    });
  });

  afterEach(async () => {
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleanup_ready',cleanup_attempt_count=0,cleanup_attempted_at=NULL,
          cleanup_next_attempt_at=NULL,cleanup_abandoned_at=NULL,cleaned_at=NULL,
          updated_at=${base}
      WHERE session_ref=${cleanupSessionRef}`;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='active',cleanup_attempt_count=0,cleanup_attempted_at=NULL,
          cleanup_next_attempt_at=NULL,cleanup_abandoned_at=NULL,cleaned_at=NULL,
          updated_at=${base}
      WHERE session_ref=${authoritySessionRef}`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (postgres !== undefined) await postgres.stop();
  });

  test("counts every cleanup claim and returns the attempt on the claim", async () => {
    expect(await claimAt(at(0))).toEqual([
      expect.objectContaining({
        sessionRef: cleanupSessionRef,
        cleanupAttemptedAt: at(0),
        cleanupAttemptCount: 1,
      }),
    ]);
    await markFailedAt(at(0));
    expect(await claimAt(at(15))).toEqual([
      expect.objectContaining({
        sessionRef: cleanupSessionRef,
        cleanupAttemptedAt: at(15),
        cleanupAttemptCount: 2,
      }),
    ]);
    expect(Number((await readCleanupBinding()).cleanup_attempt_count)).toBe(2);
  });

  test("grows the retry backoff exponentially to its bounded ceiling", async () => {
    const backoffSeconds: Array<number> = [];
    let cursor = at(0);
    for (let attempt = 1; attempt < SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS; attempt += 1) {
      expect(await claimAt(cursor)).toHaveLength(1);
      expect(await markFailedAt(cursor)).toEqual({
        state: "cleanup_failed",
        cleanupAttemptCount: attempt,
      });
      const row = await readCleanupBinding();
      const next = row.cleanup_next_attempt_at;
      if (next === null) throw new Error("failed cleanup left no next attempt");
      backoffSeconds.push((Date.parse(next) - Date.parse(cursor)) / 1_000);
      // Advancing the clock to the scheduled attempt is what makes this
      // deterministic: the schedule is read from the row, never waited out.
      cursor = next;
    }
    expect(backoffSeconds).toEqual([15, 30, 60, 120, 240, 480, 900]);
    expect(backoffSeconds[0]).toBe(SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS);
    expect(backoffSeconds.at(-1)).toBe(SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS);
  });

  test("abandons a cleanup that never succeeds and never claims it again", async () => {
    let cursor = at(0);
    let attempts = 0;
    let outcome: Awaited<ReturnType<typeof markFailedAt>> | undefined;
    // A generous bound: if the loop does not converge on its own the count
    // assertion below fails rather than the test running forever.
    for (let pass = 0; pass < 64; pass += 1) {
      const claimed = await claimAt(cursor);
      if (claimed.length === 0) break;
      attempts += 1;
      outcome = await markFailedAt(cursor);
      if (outcome.state === "cleanup_abandoned") break;
      const next = (await readCleanupBinding()).cleanup_next_attempt_at;
      if (next === null) throw new Error("failed cleanup left no next attempt");
      cursor = next;
    }
    expect(attempts).toBe(SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS);
    expect(outcome).toEqual({
      state: "cleanup_abandoned",
      cleanupAttemptCount: SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS,
    });
    const abandoned = await readCleanupBinding();
    expect(abandoned.state).toBe("cleanup_abandoned");
    expect(abandoned.cleanup_abandoned_at).toBe(cursor);
    expect(abandoned.cleanup_next_attempt_at).toBeNull();
    expect(abandoned.cleaned_at).toBeNull();
    // The whole point: no later reconciler pass, at any distance, picks it up.
    expect(await claimAt(cursor)).toEqual([]);
    expect(await claimAt(at(86_400))).toEqual([]);
    expect(await claimAt(at(86_400 * 365))).toEqual([]);
    // And the terminal row refuses further marks rather than re-arming itself.
    await expect(markFailedAt(at(86_400))).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
  });

  test("lets no binding sit at the attempt cap in a retryable state", async () => {
    // A worker that dies between the claim, which spends the attempt, and the
    // mark, which records the outcome, leaves the row AT the cap in a
    // retryable state: never claimed again, never visibly given up on. The
    // claim pass retires it lazily instead.
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state='cleanup_failed',
          cleanup_attempt_count=${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS},
          cleanup_attempted_at=${at(0)},cleanup_next_attempt_at=${at(15)},
          updated_at=${at(0)}
      WHERE session_ref=${cleanupSessionRef}`;
    expect(await claimAt(at(3_600))).toEqual([]);
    const row = await readCleanupBinding();
    expect(row.state).toBe("cleanup_abandoned");
    expect(row.cleanup_abandoned_at).toBe(at(3_600));
    expect(row.cleanup_next_attempt_at).toBeNull();
    expect(Number(row.cleanup_attempt_count)).toBe(SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS);
    expect(await claimAt(at(86_400))).toEqual([]);
  });

  test("returns the resulting cleanup state and attempt count from every mark", async () => {
    expect(await claimAt(at(0))).toHaveLength(1);
    expect(await markFailedAt(at(0))).toEqual({
      state: "cleanup_failed",
      cleanupAttemptCount: 1,
    });
    expect(
      await store.markLiveKitCleanup({
        sessionRef: cleanupSessionRef,
        generation: 1,
        state: "cleaned",
        nowIso: at(1),
      }),
    ).toEqual({ state: "cleaned", cleanupAttemptCount: 1 });
  });

  test("clears the retry schedule once the cleanup finally succeeds", async () => {
    expect(await claimAt(at(0))).toHaveLength(1);
    await markFailedAt(at(0));
    expect((await readCleanupBinding()).cleanup_next_attempt_at).toBe(at(15));
    await store.markLiveKitCleanup({
      sessionRef: cleanupSessionRef,
      generation: 1,
      state: "cleaned",
      nowIso: at(20),
    });
    const cleaned = await readCleanupBinding();
    expect(cleaned.state).toBe("cleaned");
    expect(cleaned.cleanup_next_attempt_at).toBeNull();
    expect(cleaned.cleanup_abandoned_at).toBeNull();
    expect(cleaned.cleaned_at).toBe(at(20));
    expect(await claimAt(at(3_600))).toEqual([]);
  });

  test("keeps an abandoned binding outside the room authority predicates", async () => {
    // This is a resource fix, not an authority change. `cleanup_abandoned` must
    // be exactly as invisible to the join predicates as `cleanup_failed`, so
    // giving up on a room can never widen what a member may join.
    await expect(
      authorityStore.readCommunityRoomBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        ownerUserId: "owner.authority",
        now: base,
      }),
    ).resolves.toMatchObject({ sessionRef: authoritySessionRef });
    await expect(
      authorityStore.readParticipantBinding({
        presenceLeaseRef: snapshot.presence.leaseRef,
        ownerUserId: "owner.authority",
        now: base,
      }),
    ).resolves.toMatchObject({ participantRef: "participant.authority" });

    for (const state of ["cleanup_failed", "cleanup_abandoned"] as const) {
      await sql`
        UPDATE sarah_livekit_room_bindings
        SET state=${state},
            cleanup_abandoned_at=${state === "cleanup_abandoned" ? base : null},
            updated_at=${base}
        WHERE session_ref=${authoritySessionRef}`;
      await expect(
        authorityStore.readCommunityRoomBinding({
          presenceLeaseRef: snapshot.presence.leaseRef,
          ownerUserId: "owner.authority",
          now: base,
        }),
      ).resolves.toBeUndefined();
      await expect(
        authorityStore.readParticipantBinding({
          presenceLeaseRef: snapshot.presence.leaseRef,
          ownerUserId: "owner.authority",
          now: base,
        }),
      ).resolves.toBeUndefined();
    }
  });
});
