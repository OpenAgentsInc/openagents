import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS,
  SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS,
  SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS,
  SarahVoiceSessionRejectedError,
  makeSarahRealtimeVoiceStore,
} from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

/**
 * EP263-LK H4 follow-up (#9282): LiveKit provisioning-intent reconciliation
 * must converge.
 *
 * 0125 bounded the sibling room-binding cleanup and left this table alone.
 * `claimLiveKitProvisioningIntents` re-selected every `pending`,
 * `reconciling`, and `cleanup_failed` intent behind a flat staleness gate with
 * no attempt count and no terminal state, and `markLiveKitProvisioningIntent`
 * wrote `cleanup_failed` straight back into that claim pool, so an intent
 * whose broker key can never be cleaned was retried for as long as the service
 * ran. This table was failing every tick in production.
 *
 * The load-bearing predicate here is the same one the room-binding suite pins:
 * an intent that always fails reaches `cleanup_abandoned` after exactly
 * `SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS` attempts and is never claimed
 * again. The ladder between the first failure and that give-up is asserted
 * against the stored `cleanup_next_attempt_at`, never by sleeping.
 */

const base = "2026-07-31T12:00:00.000Z";
const baseMs = Date.parse(base);
/** Explicit offsets from one fixed instant keep every timestamp deterministic. */
const at = (seconds: number): string => new Date(baseMs + seconds * 1_000).toISOString();
const digest = (character: string): string => character.repeat(64);

const intentSessionRef = "session.provisioning.convergence";

interface IntentRow {
  readonly state: string;
  readonly cleanup_attempt_count: number | string;
  readonly cleanup_next_attempt_at: string | null;
  readonly cleanup_abandoned_at: string | null;
  readonly provisioning_owner_ref: string | null;
}

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit provisioning convergence", () => {
  let postgres: LocalPostgres;
  let sql: SQL;
  let store: ReturnType<typeof makeSarahRealtimeVoiceStore>;

  const readIntent = async (): Promise<IntentRow> => {
    const rows = (await sql`
      SELECT state,cleanup_attempt_count,cleanup_next_attempt_at,cleanup_abandoned_at,
        provisioning_owner_ref
      FROM sarah_livekit_provisioning_intents
      WHERE session_ref=${intentSessionRef}`) as ReadonlyArray<IntentRow>;
    const row = rows[0];
    if (row === undefined) throw new Error("missing provisioning intent");
    return row;
  };

  /** The reconciler's only claim shape: one pass at one instant. */
  const claimAt = async (instant: string) =>
    store.claimLiveKitProvisioningIntents({
      staleBeforeIso: instant,
      nowIso: instant,
      provisioningOwnerRef: `sarah-livekit-reconciler:${instant}`,
    });

  const markFailedAt = async (instant: string, provisioningOwnerRef: string) =>
    store.markLiveKitProvisioningIntent({
      sessionRef: intentSessionRef,
      generation: 1,
      provisioningOwnerRef,
      state: "cleanup_failed",
      nowIso: instant,
    });

  /** One full reconciler tick that always fails, returning the new state. */
  const failOnceAt = async (instant: string): Promise<string | undefined> => {
    const claimed = await claimAt(instant);
    const intent = claimed[0];
    if (intent === undefined) return undefined;
    const outcome = await markFailedAt(instant, intent.provisioningOwnerRef);
    return outcome.state;
  };

  beforeAll(async () => {
    postgres = await startLocalPostgres();
    const admin = SQL({ url: postgres.url, max: 1 });
    await admin.unsafe("CREATE DATABASE sarah_livekit_provisioning_convergence");
    await admin.end();
    const databaseUrl = postgres.urlFor("sarah_livekit_provisioning_convergence");
    await runMigrations({ databaseUrl });
    sql = SQL({ url: databaseUrl, max: 4 });

    // The exact production shape of a stuck intent: a session whose request
    // path died after the intent was written, leaving a broker key nobody can
    // clean.
    await sql`
      INSERT INTO sarah_realtime_voice_sessions
        (session_ref,reservation_ref,owner_user_id,owner_actor_ref,device_ref,thread_ref,
         generation,disclosure_ref,state,reserved_msat,ticket_expires_at,session_expires_at,
         credit_rate_msat_per_million_tokens,created_at,updated_at)
      VALUES (${intentSessionRef},'reservation.provisioning.convergence','owner.provisioning',
        'actor.provisioning','device.provisioning','thread.provisioning',1,
        'disclosure.provisioning','reserved',1000,${at(3_600)},${at(3_600)},1000,
        ${base},${base})`;
    await sql`
      INSERT INTO sarah_livekit_provisioning_intents
        (session_ref,generation,idempotency_key,owner_user_id,device_ref,thread_ref,
         capability_profile,admission_ref,admission_digest,room_context_kind,
         worker_control_token_digest,state,created_at,updated_at)
      VALUES (${intentSessionRef},1,'idempotency.provisioning.convergence',
        'owner.provisioning','device.provisioning','thread.provisioning','mobile_voice_only',
        'admission.provisioning.convergence',${digest("d")},'private',${digest("f")},
        'pending',${base},${base})`;

    store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
  });

  afterEach(async () => {
    await sql`
      UPDATE sarah_livekit_provisioning_intents
      SET state='pending',cleanup_attempt_count=0,cleanup_next_attempt_at=NULL,
          cleanup_abandoned_at=NULL,provisioning_owner_ref=NULL,
          provisioning_claimed_at=NULL,updated_at=${base}
      WHERE session_ref=${intentSessionRef}`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (postgres !== undefined) await postgres.stop();
  });

  test("counts every claim and returns the attempt on the claim", async () => {
    const firstClaim = await claimAt(at(0));
    expect(firstClaim).toEqual([
      expect.objectContaining({ sessionRef: intentSessionRef, cleanupAttemptCount: 1 }),
    ]);
    const firstIntent = firstClaim[0];
    if (firstIntent === undefined) throw new Error("expected a first claim");
    await markFailedAt(at(0), firstIntent.provisioningOwnerRef);

    expect(await claimAt(at(15))).toEqual([
      expect.objectContaining({ sessionRef: intentSessionRef, cleanupAttemptCount: 2 }),
    ]);
  });

  test("counts an attempt whose process died before it could mark", async () => {
    // The bound must be on attempts, not on marks. A claim that never reaches
    // a mark still spent an attempt; if it did not count, a process crashing
    // in the same place every tick would retry forever, which is the defect
    // this suite exists to prevent.
    await claimAt(at(0));
    expect((await readIntent()).cleanup_attempt_count).toBe(1);
    await claimAt(at(120));
    expect((await readIntent()).cleanup_attempt_count).toBe(2);
  });

  test("grows the retry backoff exponentially to its bounded ceiling", async () => {
    const backoffSeconds: Array<number> = [];
    let instant = 0;
    for (let attempt = 0; attempt < SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS - 1; attempt += 1) {
      // eslint-disable-next-line no-await-in-loop
      const state = await failOnceAt(at(instant));
      expect(state).toBe("cleanup_failed");
      // eslint-disable-next-line no-await-in-loop
      const row = await readIntent();
      const next = row.cleanup_next_attempt_at;
      if (next === null) throw new Error("expected a scheduled retry");
      backoffSeconds.push((Date.parse(next) - (baseMs + instant * 1_000)) / 1_000);
      // The next claim happens exactly when the row says it may.
      instant = (Date.parse(next) - baseMs) / 1_000;
    }
    expect(backoffSeconds).toEqual([15, 30, 60, 120, 240, 480, 900]);
    expect(backoffSeconds[0]).toBe(SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS);
    expect(backoffSeconds[backoffSeconds.length - 1]).toBe(
      SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS,
    );
  });

  test("refuses to claim before the scheduled next attempt", async () => {
    const state = await failOnceAt(at(0));
    expect(state).toBe("cleanup_failed");
    // One second before the ladder allows it, the row is invisible.
    expect(await claimAt(at(14))).toEqual([]);
    expect(await claimAt(at(15))).toHaveLength(1);
  });

  test("abandons an intent that never succeeds and never claims it again", async () => {
    let instant = 0;
    let attempts = 0;
    let finalState: string | undefined;
    // Walk the ladder to its end, always arriving exactly when allowed.
    for (let pass = 0; pass < SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS + 4; pass += 1) {
      // eslint-disable-next-line no-await-in-loop
      const claimed = await claimAt(at(instant));
      const intent = claimed[0];
      if (intent === undefined) break;
      attempts += 1;
      // eslint-disable-next-line no-await-in-loop
      const outcome = await markFailedAt(at(instant), intent.provisioningOwnerRef);
      finalState = outcome.state;
      // eslint-disable-next-line no-await-in-loop
      const row = await readIntent();
      instant =
        row.cleanup_next_attempt_at === null
          ? instant + 86_400
          : (Date.parse(row.cleanup_next_attempt_at) - baseMs) / 1_000;
    }
    expect(attempts).toBe(SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS);
    expect(finalState).toBe("cleanup_abandoned");

    const row = await readIntent();
    expect(row.state).toBe("cleanup_abandoned");
    expect(row.cleanup_abandoned_at).not.toBeNull();
    expect(row.cleanup_next_attempt_at).toBeNull();
    expect(row.provisioning_owner_ref).toBeNull();

    // The give-up is permanent: no later tick, however far in the future,
    // resurrects the row.
    expect(await claimAt(at(86_400))).toEqual([]);
    expect(await claimAt(at(31_536_000))).toEqual([]);
    await expect(markFailedAt(at(86_400), "sarah-livekit-reconciler:later")).rejects.toBeInstanceOf(
      SarahVoiceSessionRejectedError,
    );
  });

  test("lets no intent sit at the attempt cap in a retryable state", async () => {
    // The crash window: a process that claimed the last attempt and died
    // before marking it leaves the row at the cap, unclaimed but not given up
    // on. The next claim pass must retire it rather than ignore it forever.
    await sql`
      UPDATE sarah_livekit_provisioning_intents
      SET state='cleanup_failed',
          cleanup_attempt_count=${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS},
          cleanup_next_attempt_at=NULL,provisioning_owner_ref=NULL,
          provisioning_claimed_at=NULL,updated_at=${base}
      WHERE session_ref=${intentSessionRef}`;

    expect(await claimAt(at(60))).toEqual([]);

    const row = await readIntent();
    expect(row.state).toBe("cleanup_abandoned");
    expect(row.cleanup_abandoned_at).toBe(at(60));
    expect(row.cleanup_next_attempt_at).toBeNull();
  });

  test("clears the retry schedule once the reconciliation finally succeeds", async () => {
    const failed = await failOnceAt(at(0));
    expect(failed).toBe("cleanup_failed");
    expect((await readIntent()).cleanup_next_attempt_at).not.toBeNull();

    const claimed = (await claimAt(at(15)))[0];
    if (claimed === undefined) throw new Error("expected a retry claim");
    const outcome = await store.markLiveKitProvisioningIntent({
      sessionRef: intentSessionRef,
      generation: 1,
      provisioningOwnerRef: claimed.provisioningOwnerRef,
      state: "cleaned",
      nowIso: at(15),
    });
    expect(outcome.state).toBe("cleaned");

    const row = await readIntent();
    expect(row.state).toBe("cleaned");
    expect(row.cleanup_next_attempt_at).toBeNull();
    expect(row.cleanup_abandoned_at).toBeNull();
    // A cleaned intent leaves the claim pool for good.
    expect(await claimAt(at(3_600))).toEqual([]);
  });
});
