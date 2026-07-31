import { SQL } from "@openagentsinc/postgres-runtime";
import { copyFile, mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  SarahVoiceAdmissionRejectedError,
  SarahVoiceConcurrentSessionError,
  SarahVoiceDuplicateParticipantError,
  SarahVoiceLiveKitCapacityError,
  SarahVoiceSessionRejectedError,
  makeSarahRealtimeVoiceStore,
} from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const waitForBlockedApplication = async (sql: SQL, applicationName: string): Promise<void> => {
  const deadline = Date.now() + 2_000;
  while (Date.now() < deadline) {
    const rows = (await sql`
      SELECT wait_event_type
      FROM pg_stat_activity
      WHERE application_name = ${applicationName}
        AND wait_event_type = 'Lock'
    `) as ReadonlyArray<{ wait_event_type: string }>;
    if (rows.length > 0) return;
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  throw new Error(`${applicationName} did not block on the expected row lock`);
};

const completeWithin = <A>(promise: Promise<A>, timeoutMs: number, message: string): Promise<A> =>
  new Promise<A>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });

describe.skipIf(!hasLocalPostgres())("Sarah Realtime voice credit authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_sarah_voice");
    await admin.end();
    await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_sarah_voice"),
    });
    sql = SQL({ url: pg.urlFor("khala_sync_sarah_voice"), max: 5 });
    await sql`
        INSERT INTO users (
          id, kind, display_name, status, created_at, updated_at
        ) VALUES
          (
            'user-sarah-voice', 'human', 'Voice Tester', 'active',
            '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
          )
      `;
    await sql`
        INSERT INTO agent_balances (
          actor_ref, balance_msat, held_msat, usd_credit_msat,
          created_at, updated_at
        ) VALUES (
          'agent:user-sarah-voice', 10000, 0, 0,
          '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
        )
      `;
    await sql`
      INSERT INTO sarah_voice_alpha_memberships (
        membership_ref, cohort_ref, owner_user_id, state, admitted_at,
        admission_actor_ref, admission_reason, updated_at
      ) VALUES (
        'sarah_voice_alpha:user-sarah-voice',
        'sarah_voice_cohort:alpha_v1', 'user-sarah-voice', 'active',
        '2026-07-28T12:00:00.000Z', 'operator:test', 'Test admission',
        '2026-07-28T12:00:00.000Z'
      )
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("reserves, meters once, and settles the exact charge", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const common = {
      ownerUserId: "user-sarah-voice",
      ownerActorRef: "agent:user-sarah-voice",
      deviceRef: "omega-test",
      threadRef: "thread-test",
      generation: 1,
      disclosureRef: "disclosure-test",
      clientProfile: "mobile_voice_only",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 1_000,
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
      sessionExpiresAt: "2026-07-28T12:10:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    } as const;
    const reserved = await store.reserve({
      ...common,
      sessionRef: "voice-session-1",
      reservationRef: "voice-reservation-1",
      ticketDigest: "a".repeat(64),
    });
    expect(reserved.clientProfile).toBe("mobile_voice_only");
    await expect(
      store.reserve({
        ...common,
        sessionRef: "voice-session-2",
        reservationRef: "voice-reservation-2",
        ticketDigest: "b".repeat(64),
      }),
    ).rejects.toBeInstanceOf(SarahVoiceConcurrentSessionError);

    const connected = await store.connect({
      sessionRef: "voice-session-1",
      ticketDigest: "a".repeat(64),
      nowIso: "2026-07-28T12:00:30.000Z",
    });
    expect(connected.clientProfile).toBe("mobile_voice_only");
    const usage = {
      providerResponseRef: "response-1",
      inputTokens: 100,
      outputTokens: 50,
      cachedInputTokens: 10,
      audioInputTokens: 80,
      audioOutputTokens: 40,
      chargeMsat: 250,
      observedAt: "2026-07-28T12:01:00.000Z",
    } as const;
    await store.recordUsage({
      sessionRef: "voice-session-1",
      generation: 1,
      usage,
    });
    await store.recordUsage({
      sessionRef: "voice-session-1",
      generation: 1,
      usage,
    });
    const settled = await store.settle({
      sessionRef: "voice-session-1",
      closeReason: "user_stop",
      nowIso: "2026-07-28T12:02:00.000Z",
    });
    expect(settled).toMatchObject({
      state: "settled",
      clientProfile: "mobile_voice_only",
      chargedMsat: 15,
      reservedMsat: 1_000,
    });

    const [balance] = await sql`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = 'agent:user-sarah-voice'
      `;
    expect(Number(balance?.balance_msat)).toBe(9_985);
    expect(Number(balance?.held_msat)).toBe(0);
    const [receipt] = await sql`
        SELECT cost_msat, state
        FROM pay_ins
        WHERE idempotency_key = 'sarah:voice:settle:voice-session-1'
      `;
    expect(Number(receipt?.cost_msat)).toBe(15);
    expect(receipt?.state).toBe("paid");

    await store.settle({
      sessionRef: "voice-session-1",
      closeReason: "replay",
      nowIso: "2026-07-28T12:03:00.000Z",
    });
    const [afterReplay] = await sql`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = 'agent:user-sarah-voice'
      `;
    expect(Number(afterReplay?.balance_msat)).toBe(9_985);
    expect(Number(afterReplay?.held_msat)).toBe(0);
    expect(
      await store.readSettlement({
        sessionRef: "voice-session-1",
        ownerUserId: "user-sarah-voice",
      }),
    ).toMatchObject({
      finalChargeMsat: 15,
      spendableRemainingCreditMsat: 9_985,
      settlementReceiptRef: "sarah_voice_settlement:voice-session-1",
    });
  });

  test("releases an unconnected reservation when its ticket expires", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await store.reserve({
      sessionRef: "voice-ticket-expiry-session",
      reservationRef: "voice-ticket-expiry-reservation",
      ownerUserId: "user-sarah-voice",
      ownerActorRef: "agent:user-sarah-voice",
      deviceRef: "omega-ticket-expiry",
      threadRef: "thread-ticket-expiry",
      generation: 1,
      ticketDigest: "e".repeat(64),
      disclosureRef: "disclosure-ticket-expiry",
      clientProfile: "mobile_voice_only",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 1_000,
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
      sessionExpiresAt: "2026-07-28T12:10:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    });

    expect(await store.sweepExpired("2026-07-28T12:02:00.000Z")).toBe(1);
    const [session] = await sql`
      SELECT state, close_reason
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = 'voice-ticket-expiry-session'
    `;
    expect(session?.state).toBe("released");
    expect(session?.close_reason).toBe("ticket_expired");
    const [balance] = await sql`
      SELECT held_msat
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-voice'
    `;
    expect(Number(balance?.held_msat)).toBe(0);
  });

  test("continues an expiry batch before surfacing an isolated row failure", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO users (
        id, kind, display_name, status, created_at, updated_at
      ) VALUES
        (
          'user-sarah-expiry-bad', 'human', 'Bad Expiry', 'active',
          '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
        ),
        (
          'user-sarah-expiry-good', 'human', 'Good Expiry', 'active',
          '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
        )
    `;
    await sql`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, usd_credit_msat,
        created_at, updated_at
      ) VALUES
        (
          'agent:user-sarah-expiry-bad', 1000, 0, 0,
          '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
        ),
        (
          'agent:user-sarah-expiry-good', 1000, 0, 0,
          '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
        )
    `;
    await sql`
      INSERT INTO sarah_voice_alpha_memberships (
        membership_ref, cohort_ref, owner_user_id, state, admitted_at,
        admission_actor_ref, admission_reason, updated_at
      ) VALUES
        (
          'sarah_voice_alpha:user-sarah-expiry-bad',
          'sarah_voice_cohort:alpha_v1', 'user-sarah-expiry-bad',
          'active', '2026-07-28T12:00:00.000Z', 'operator:test',
          'Test admission', '2026-07-28T12:00:00.000Z'
        ),
        (
          'sarah_voice_alpha:user-sarah-expiry-good',
          'sarah_voice_cohort:alpha_v1', 'user-sarah-expiry-good',
          'active', '2026-07-28T12:00:00.000Z', 'operator:test',
          'Test admission', '2026-07-28T12:00:00.000Z'
        )
    `;
    const reservation = {
      deviceRef: "omega-expiry",
      threadRef: "thread-expiry",
      generation: 1,
      disclosureRef: "disclosure-expiry",
      clientProfile: "mobile_voice_only",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 100,
      sessionExpiresAt: "2026-07-28T12:10:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    } as const;
    await store.reserve({
      ...reservation,
      sessionRef: "voice-expiry-bad",
      reservationRef: "voice-expiry-bad-reservation",
      ownerUserId: "user-sarah-expiry-bad",
      ownerActorRef: "agent:user-sarah-expiry-bad",
      ticketDigest: "8".repeat(64),
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
    });
    await store.reserve({
      ...reservation,
      sessionRef: "voice-expiry-good",
      reservationRef: "voice-expiry-good-reservation",
      ownerUserId: "user-sarah-expiry-good",
      ownerActorRef: "agent:user-sarah-expiry-good",
      ticketDigest: "9".repeat(64),
      ticketExpiresAt: "2026-07-28T12:01:01.000Z",
    });
    await sql`
      UPDATE agent_balances
      SET held_msat = 0
      WHERE actor_ref = 'agent:user-sarah-expiry-bad'
    `;

    await expect(store.sweepExpired("2026-07-28T12:02:00.000Z")).rejects.toBeInstanceOf(
      AggregateError,
    );
    const sessions = await sql`
      SELECT session_ref, state
      FROM sarah_realtime_voice_sessions
      WHERE session_ref IN ('voice-expiry-bad', 'voice-expiry-good')
      ORDER BY session_ref
    `;
    expect(sessions).toMatchObject([
      { session_ref: "voice-expiry-bad", state: "reserved" },
      { session_ref: "voice-expiry-good", state: "released" },
    ]);
    await sql`
      UPDATE sarah_realtime_voice_sessions
      SET state = 'failed', ticket_digest = NULL,
          close_reason = 'test_isolated_failure'
      WHERE session_ref = 'voice-expiry-bad'
    `;
    await sql`
      DELETE FROM sarah_voice_alpha_memberships
      WHERE owner_user_id IN (
        'user-sarah-expiry-bad',
        'user-sarah-expiry-good'
      )
    `;
  });

  test("quarantines pre-rate active rows while enforcing frozen authority for new rows", async () => {
    const migrationsDir = path.join(import.meta.dirname, "..", "migrations");
    const stagedMigrationsDir = await mkdtemp(path.join(tmpdir(), "sarah-frozen-rate-migrations-"));
    const databaseName = "khala_sync_sarah_voice_legacy_rate";
    let legacySql: SQL | undefined;
    try {
      for (const filename of await readdir(migrationsDir)) {
        if (
          filename.endsWith(".sql") &&
          filename.localeCompare("0117_sarah_voice_frozen_accounting_authority.sql") < 0
        ) {
          await copyFile(
            path.join(migrationsDir, filename),
            path.join(stagedMigrationsDir, filename),
          );
        }
      }
      const admin = SQL({ url: pg.url, max: 1 });
      await admin.unsafe(`CREATE DATABASE ${databaseName}`);
      await admin.end();
      const databaseUrl = pg.urlFor(databaseName);
      await runMigrations({ databaseUrl, migrationsDir: stagedMigrationsDir });
      legacySql = SQL({ url: databaseUrl, max: 2 });
      await legacySql`
        INSERT INTO agent_balances (
          actor_ref, balance_msat, held_msat, usd_credit_msat,
          created_at, updated_at
        ) VALUES
          (
            'agent:legacy-reserved', 1000, 100, 0,
            '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
          ),
          (
            'agent:legacy-connected', 1000, 100, 0,
            '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
          )
      `;
      await legacySql`
        INSERT INTO sarah_realtime_voice_sessions (
          session_ref, reservation_ref, owner_user_id, owner_actor_ref,
          device_ref, thread_ref, generation, ticket_digest, disclosure_ref,
          client_profile, transport_kind, credit_mode, entitlement_ref,
          admission_cohort_ref, state, reserved_msat, charged_msat,
          ticket_expires_at, session_expires_at, created_at, updated_at,
          connected_at
        ) VALUES
          (
            'legacy-reserved', 'legacy-reserved-reservation',
            'legacy-reserved', 'agent:legacy-reserved', 'legacy-device',
            'legacy-thread', 1, ${"a".repeat(64)}, 'legacy-disclosure',
            'omega_editor', 'custom_wss_v1', 'metered', NULL,
            'sarah_voice_cohort:alpha_v1', 'reserved', 100, 0,
            '2026-07-28T12:01:00.000Z', '2026-07-28T12:10:00.000Z',
            '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z', NULL
          ),
          (
            'legacy-connected', 'legacy-connected-reservation',
            'legacy-connected', 'agent:legacy-connected', 'legacy-device',
            'legacy-thread', 1, ${"b".repeat(64)}, 'legacy-disclosure',
            'omega_editor', 'custom_wss_v1', 'metered', NULL,
            'sarah_voice_cohort:alpha_v1', 'connected', 100, 25,
            '2026-07-28T12:01:00.000Z', '2026-07-28T12:10:00.000Z',
            '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z',
            '2026-07-28T12:00:30.000Z'
          )
      `;
      await legacySql`
        INSERT INTO sarah_voice_admissions (
          admission_ref, owner_user_id, device_ref, thread_ref, session_ref,
          generation, disclosure_ref, client_profile, admission_cohort_ref,
          credit_mode, terms_digest, spendable_remaining_credit_msat, state,
          issued_at, expires_at, consumed_at
        ) VALUES (
          'legacy-admission', 'legacy-reserved', 'legacy-device',
          'legacy-thread', 'legacy-reserved', 1, 'legacy-disclosure',
          'omega_editor', 'sarah_voice_cohort:alpha_v1', 'metered',
          ${"c".repeat(64)}, 900, 'active', '2026-07-28T12:00:00.000Z',
          '2026-07-28T12:02:00.000Z', NULL
        )
      `;
      await legacySql.end();
      legacySql = undefined;
      // Stage the rest of the migration set, not only 0117. The rows above are
      // what a pre-0117 deployment left behind, but the store exercised below
      // is the current one, and a real deployment runs it against the current
      // schema. Stopping the fixture at 0117 made every later column the store
      // writes (most recently the EP263-LK cleanup convergence columns in 0125)
      // fail here for a reason that has nothing to do with frozen rate
      // authority.
      for (const filename of await readdir(migrationsDir)) {
        if (
          filename.endsWith(".sql") &&
          filename.localeCompare("0117_sarah_voice_frozen_accounting_authority.sql") >= 0
        ) {
          await copyFile(
            path.join(migrationsDir, filename),
            path.join(stagedMigrationsDir, filename),
          );
        }
      }
      await runMigrations({ databaseUrl, migrationsDir: stagedMigrationsDir });
      legacySql = SQL({ url: databaseUrl, max: 2 });

      const sessions = await legacySql`
        SELECT session_ref, state, ticket_digest, close_reason,
          credit_rate_msat_per_million_tokens, accounting_rate_authority
        FROM sarah_realtime_voice_sessions
        ORDER BY session_ref
      `;
      expect(sessions).toMatchObject([
        {
          session_ref: "legacy-connected",
          state: "connected",
          credit_rate_msat_per_million_tokens: null,
          accounting_rate_authority: "legacy_unresolved",
        },
        {
          session_ref: "legacy-reserved",
          state: "reserved",
          credit_rate_msat_per_million_tokens: null,
          accounting_rate_authority: "legacy_unresolved",
        },
      ]);
      const legacyStore = makeSarahRealtimeVoiceStore(legacySql as unknown as SyncSql);
      expect(await legacyStore.sweepExpired("2026-07-28T12:20:00.000Z")).toBe(2);
      const terminalSessions = await legacySql`
        SELECT session_ref, state, ticket_digest, close_reason,
          credit_rate_msat_per_million_tokens, accounting_rate_authority
        FROM sarah_realtime_voice_sessions
        ORDER BY session_ref
      `;
      expect(terminalSessions).toMatchObject([
        {
          session_ref: "legacy-connected",
          state: "accounting_uncertain",
          ticket_digest: null,
          close_reason: "legacy_accounting_authority_unavailable",
          credit_rate_msat_per_million_tokens: null,
          accounting_rate_authority: "legacy_unresolved",
        },
        {
          session_ref: "legacy-reserved",
          state: "released",
          ticket_digest: null,
          close_reason: "ticket_expired",
          credit_rate_msat_per_million_tokens: null,
          accounting_rate_authority: "legacy_unresolved",
        },
      ]);
      await expect(
        legacyStore.settle({
          sessionRef: "legacy-connected",
          closeReason: "must_not_release",
          nowIso: "2026-07-28T12:20:00.000Z",
        }),
      ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
      const balances = await legacySql`
        SELECT actor_ref, held_msat
        FROM agent_balances
        ORDER BY actor_ref
      `;
      expect(balances).toMatchObject([
        { actor_ref: "agent:legacy-connected", held_msat: "100" },
        { actor_ref: "agent:legacy-reserved", held_msat: "0" },
      ]);
      const [admission] = await legacySql`
        SELECT state, accounting_rate_authority,
          credit_rate_msat_per_million_tokens
        FROM sarah_voice_admissions
        WHERE admission_ref = 'legacy-admission'
      `;
      expect(admission).toMatchObject({
        state: "expired",
        accounting_rate_authority: "legacy_unresolved",
        credit_rate_msat_per_million_tokens: null,
      });
      await expect(
        legacySql`
          INSERT INTO sarah_realtime_voice_sessions (
            session_ref, reservation_ref, owner_user_id, owner_actor_ref,
            device_ref, thread_ref, generation, ticket_digest, disclosure_ref,
            client_profile, transport_kind, credit_mode, entitlement_ref,
            admission_cohort_ref, state, reserved_msat, charged_msat,
            ticket_expires_at, session_expires_at, created_at, updated_at
          ) VALUES (
            'new-missing-rate', 'new-missing-rate-reservation',
            'new-missing-rate', 'agent:new-missing-rate', 'new-device',
            'new-thread', 1, ${"d".repeat(64)}, 'new-disclosure',
            'omega_editor', 'custom_wss_v1', 'metered', NULL,
            'sarah_voice_cohort:alpha_v1', 'reserved', 100, 0,
            '2026-07-28T12:06:00.000Z', '2026-07-28T12:10:00.000Z',
            '2026-07-28T12:05:00.000Z', '2026-07-28T12:05:00.000Z'
          )
        `,
      ).rejects.toThrow();
    } finally {
      await legacySql?.end();
      await rm(stagedMigrationsDir, { recursive: true, force: true });
    }
  });

  test("records entitled usage without a credit hold or debit", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO users (
        id, kind, display_name, status, created_at, updated_at
      ) VALUES (
        'user-sarah-staging-owner', 'human', 'Staging Owner', 'active',
        '2026-07-28T12:00:00.000Z', '2026-07-28T12:00:00.000Z'
      )
    `;
    const entitlement = await store.ensureStagingOwnerEntitlement({
      ownerUserId: "user-sarah-staging-owner",
      entitlementRef: "sarah_voice_entitlement:staging_owner_v1",
      nowIso: "2026-07-28T12:00:00.000Z",
      expiresAt: "2026-08-04T12:00:00.000Z",
    });
    expect(entitlement).toMatchObject({
      entitlementRef: "sarah_voice_entitlement:staging_owner_v1",
      ownerUserId: "user-sarah-staging-owner",
    });
    expect(
      await store.readActiveStagingOwnerEntitlement({
        ownerUserId: "user-sarah-staging-owner",
        entitlementRef: "sarah_voice_entitlement:staging_owner_v1",
        nowIso: "2026-07-28T12:00:00.000Z",
      }),
    ).toEqual(entitlement);

    await store.reserve({
      sessionRef: "voice-entitled-session-1",
      reservationRef: "voice-entitled-reservation-1",
      ownerUserId: "user-sarah-staging-owner",
      ownerActorRef: "agent:user-sarah-staging-owner",
      deviceRef: "omega-entitled",
      threadRef: "thread-entitled",
      generation: 1,
      ticketDigest: "c".repeat(64),
      disclosureRef: "disclosure-entitled",
      clientProfile: "mobile_voice_only",
      creditMode: "staging_owner_entitlement",
      entitlementRef: entitlement?.entitlementRef ?? null,
      admissionCohortRef: "sarah_voice_cohort:staging_owner_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 0,
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
      sessionExpiresAt: "2026-07-28T12:05:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    });
    await store.connect({
      sessionRef: "voice-entitled-session-1",
      ticketDigest: "c".repeat(64),
      nowIso: "2026-07-28T12:00:30.000Z",
    });
    const usage = await store.recordUsage({
      sessionRef: "voice-entitled-session-1",
      generation: 1,
      usage: {
        providerResponseRef: "entitled-response-1",
        inputTokens: 100,
        outputTokens: 50,
        cachedInputTokens: 0,
        audioInputTokens: 100,
        audioOutputTokens: 50,
        chargeMsat: 500,
        observedAt: "2026-07-28T12:01:00.000Z",
      },
    });
    expect(usage).toEqual({
      chargedMsat: 15,
      reservedMsat: 0,
      creditLimitReached: false,
    });
    const settled = await store.settle({
      sessionRef: "voice-entitled-session-1",
      closeReason: "user_stop",
      nowIso: "2026-07-28T12:02:00.000Z",
    });
    expect(settled).toMatchObject({
      state: "settled",
      creditMode: "staging_owner_entitlement",
      reservedMsat: 0,
      chargedMsat: 15,
    });
    const [payment] = await sql`
      SELECT id FROM pay_ins
      WHERE idempotency_key = 'sarah:voice:settle:voice-entitled-session-1'
    `;
    expect(payment).toBeUndefined();

    await sql`
      UPDATE sarah_voice_credit_entitlements
      SET state = 'revoked',
          revoked_at = '2026-07-28T12:03:00.000Z',
          revocation_actor_ref = 'operator:test',
          revocation_reason = 'Test revocation',
          updated_at = '2026-07-28T12:03:00.000Z'
      WHERE entitlement_ref = 'sarah_voice_entitlement:staging_owner_v1'
    `;
    expect(
      await store.readActiveStagingOwnerEntitlement({
        ownerUserId: "user-sarah-staging-owner",
        entitlementRef: "sarah_voice_entitlement:staging_owner_v1",
        nowIso: "2026-07-28T12:04:00.000Z",
      }),
    ).toBeUndefined();

    await expect(
      store.reserve({
        sessionRef: "voice-unknown-session-1",
        reservationRef: "voice-unknown-reservation-1",
        ownerUserId: "user-sarah-staging-owner",
        ownerActorRef: "agent:user-sarah-staging-owner",
        deviceRef: "omega-unknown",
        threadRef: "thread-unknown",
        generation: 1,
        ticketDigest: "d".repeat(64),
        disclosureRef: "disclosure-unknown",
        clientProfile: "mobile_voice_only",
        creditMode: "metered",
        entitlementRef: null,
        admissionCohortRef: "sarah_voice_cohort:alpha_v1",
        creditRateMsatPerMillionTokens: 100_000,
        reservedMsat: 1,
        ticketExpiresAt: "2026-07-28T12:05:00.000Z",
        sessionExpiresAt: "2026-07-28T12:09:00.000Z",
        nowIso: "2026-07-28T12:04:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);

    const [unexpectedBalance] = await sql`
      SELECT actor_ref
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-staging-owner'
    `;
    expect(unexpectedBalance).toBeUndefined();
  });

  test("binds one Omega reservation to one unexpired exact admission", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const [balanceBefore] = await sql`
      SELECT balance_msat - held_msat AS spendable_msat
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-voice'
    `;
    const spendableMsat = Number(balanceBefore?.spendable_msat);
    const common = {
      sessionRef: "voice-bound-session-1",
      reservationRef: "voice-bound-reservation-1",
      ownerUserId: "user-sarah-voice",
      ownerActorRef: "agent:user-sarah-voice",
      deviceRef: "omega-bound",
      threadRef: "thread-bound",
      generation: 1,
      ticketDigest: "1".repeat(64),
      disclosureRef: "disclosure-bound",
      clientProfile: "omega_editor",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 1_000,
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
      sessionExpiresAt: "2026-07-28T12:10:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    } as const;
    await store.issueAdmission({
      admissionRef: "sarah_voice_admission:bound-1",
      ownerUserId: common.ownerUserId,
      deviceRef: common.deviceRef,
      threadRef: common.threadRef,
      sessionRef: common.sessionRef,
      generation: common.generation,
      disclosureRef: common.disclosureRef,
      clientProfile: common.clientProfile,
      admissionCohortRef: common.admissionCohortRef,
      creditRateMsatPerMillionTokens: 100_000,
      creditMode: common.creditMode,
      termsDigest: "a".repeat(64),
      spendableRemainingCreditMsat: spendableMsat,
      nowIso: common.nowIso,
      expiresAt: "2026-07-28T12:02:00.000Z",
    });

    await expect(
      store.reserve({
        ...common,
        creditRateMsatPerMillionTokens: 200_000,
        admissionBinding: {
          admissionRef: "sarah_voice_admission:bound-1",
          creditRateMsatPerMillionTokens: 200_000,
          termsDigest: "a".repeat(64),
          spendableRemainingCreditMsat: spendableMsat,
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceAdmissionRejectedError);

    await expect(
      store.reserve({
        ...common,
        admissionBinding: {
          admissionRef: "sarah_voice_admission:bound-1",
          creditRateMsatPerMillionTokens: 100_000,
          termsDigest: "b".repeat(64),
          spendableRemainingCreditMsat: spendableMsat,
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceAdmissionRejectedError);

    const reserved = await store.reserve({
      ...common,
      admissionBinding: {
        admissionRef: "sarah_voice_admission:bound-1",
        creditRateMsatPerMillionTokens: 100_000,
        termsDigest: "a".repeat(64),
        spendableRemainingCreditMsat: spendableMsat,
      },
    });
    expect(reserved.admissionExpiresAt).toBe("2026-07-28T12:02:00.000Z");
    await expect(
      store.reserve({
        ...common,
        reservationRef: "voice-bound-reservation-replay",
        ticketDigest: "2".repeat(64),
        admissionBinding: {
          admissionRef: "sarah_voice_admission:bound-1",
          creditRateMsatPerMillionTokens: 100_000,
          termsDigest: "a".repeat(64),
          spendableRemainingCreditMsat: spendableMsat,
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceAdmissionRejectedError);
    expect(await store.sweepExpired("2026-07-28T12:02:00.000Z")).toBe(1);

    await store.issueAdmission({
      admissionRef: "sarah_voice_admission:balance-change",
      ownerUserId: common.ownerUserId,
      deviceRef: common.deviceRef,
      threadRef: common.threadRef,
      sessionRef: "voice-bound-session-balance-change",
      generation: 2,
      disclosureRef: common.disclosureRef,
      clientProfile: common.clientProfile,
      admissionCohortRef: common.admissionCohortRef,
      creditRateMsatPerMillionTokens: 100_000,
      creditMode: common.creditMode,
      termsDigest: "c".repeat(64),
      spendableRemainingCreditMsat: spendableMsat,
      nowIso: "2026-07-28T12:03:00.000Z",
      expiresAt: "2026-07-28T12:05:00.000Z",
    });
    await sql`
      UPDATE agent_balances
      SET balance_msat = balance_msat - 1
      WHERE actor_ref = 'agent:user-sarah-voice'
    `;
    await expect(
      store.reserve({
        ...common,
        sessionRef: "voice-bound-session-balance-change",
        reservationRef: "voice-bound-reservation-balance-change",
        generation: 2,
        ticketDigest: "3".repeat(64),
        nowIso: "2026-07-28T12:03:00.000Z",
        admissionBinding: {
          admissionRef: "sarah_voice_admission:balance-change",
          creditRateMsatPerMillionTokens: 100_000,
          termsDigest: "c".repeat(64),
          spendableRemainingCreditMsat: spendableMsat,
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceAdmissionRejectedError);
    await sql`
      UPDATE agent_balances
      SET balance_msat = balance_msat + 1
      WHERE actor_ref = 'agent:user-sarah-voice'
    `;
  });

  test("revokes the alpha cohort before any new reservation can be created", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    expect(
      await store.readActiveAlphaMembership({
        ownerUserId: "user-sarah-voice",
        cohortRef: "sarah_voice_cohort:alpha_v1",
        nowIso: "2026-07-28T12:04:00.000Z",
      }),
    ).toMatchObject({ ownerUserId: "user-sarah-voice" });
    expect(
      await store.revokeAlphaCohort({
        cohortRef: "sarah_voice_cohort:alpha_v1",
        actorRef: "operator:test",
        reason: "End alpha access",
        nowIso: "2026-07-28T12:05:00.000Z",
      }),
    ).toBe(1);
    expect(
      await store.readActiveAlphaMembership({
        ownerUserId: "user-sarah-voice",
        cohortRef: "sarah_voice_cohort:alpha_v1",
        nowIso: "2026-07-28T12:06:00.000Z",
      }),
    ).toBeUndefined();
    await expect(
      store.reserve({
        sessionRef: "voice-after-revocation",
        reservationRef: "voice-after-revocation-reservation",
        ownerUserId: "user-sarah-voice",
        ownerActorRef: "agent:user-sarah-voice",
        deviceRef: "omega-revoked",
        threadRef: "thread-revoked",
        generation: 1,
        ticketDigest: "f".repeat(64),
        disclosureRef: "disclosure-revoked",
        clientProfile: "omega_editor",
        creditMode: "metered",
        entitlementRef: null,
        admissionCohortRef: "sarah_voice_cohort:alpha_v1",
        creditRateMsatPerMillionTokens: 100_000,
        reservedMsat: 1,
        ticketExpiresAt: "2026-07-28T12:07:00.000Z",
        sessionExpiresAt: "2026-07-28T12:10:00.000Z",
        nowIso: "2026-07-28T12:06:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
  });

  test("binds one LiveKit generation, rejects join replay, and gates cleanup on accounting", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO users (
        id, kind, display_name, status, created_at, updated_at
      ) VALUES (
        'user-sarah-livekit', 'human', 'LiveKit Tester', 'active',
        '2026-07-28T13:00:00.000Z', '2026-07-28T13:00:00.000Z'
      )
    `;
    await sql`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, usd_credit_msat,
        created_at, updated_at
      ) VALUES (
        'agent:user-sarah-livekit', 10000, 0, 0,
        '2026-07-28T13:00:00.000Z', '2026-07-28T13:00:00.000Z'
      )
    `;
    await sql`
      INSERT INTO sarah_voice_alpha_memberships (
        membership_ref, cohort_ref, owner_user_id, state, admitted_at,
        admission_actor_ref, admission_reason, updated_at
      ) VALUES (
        'sarah_voice_alpha:user-sarah-livekit',
        'sarah_voice_cohort:alpha_v1', 'user-sarah-livekit', 'active',
        '2026-07-28T13:00:00.000Z', 'operator:test', 'Test admission',
        '2026-07-28T13:00:00.000Z'
      )
    `;
    const reservation = {
      ownerUserId: "user-sarah-livekit",
      ownerActorRef: "agent:user-sarah-livekit",
      deviceRef: "omega-livekit",
      threadRef: "thread-livekit",
      disclosureRef: "disclosure-livekit",
      clientProfile: "mobile_voice_only",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditRateMsatPerMillionTokens: 100_000,
      reservedMsat: 1_000,
      nowIso: "2026-07-28T13:00:00.000Z",
    } as const;
    await store.issueAdmission({
      admissionRef: "sarah_voice_admission:livekit-1",
      ownerUserId: reservation.ownerUserId,
      deviceRef: reservation.deviceRef,
      threadRef: reservation.threadRef,
      sessionRef: "voice-livekit-1",
      generation: 1,
      disclosureRef: reservation.disclosureRef,
      clientProfile: reservation.clientProfile,
      admissionCohortRef: reservation.admissionCohortRef,
      creditRateMsatPerMillionTokens: 100_000,
      creditMode: reservation.creditMode,
      termsDigest: "8".repeat(64),
      spendableRemainingCreditMsat: 10_000,
      nowIso: reservation.nowIso,
      expiresAt: "2026-07-28T13:02:00.000Z",
    });
    const firstReservation = await store.reserve({
      ...reservation,
      sessionRef: "voice-livekit-1",
      reservationRef: "voice-livekit-reservation-1",
      generation: 1,
      transportKind: "livekit_room_v1",
      ticketDigest: "4".repeat(64),
      ticketExpiresAt: "2026-07-28T13:01:00.000Z",
      sessionExpiresAt: "2026-07-28T13:10:00.000Z",
      admissionBinding: {
        admissionRef: "sarah_voice_admission:livekit-1",
        creditRateMsatPerMillionTokens: 100_000,
        termsDigest: "8".repeat(64),
        spendableRemainingCreditMsat: 10_000,
      },
    });
    expect(firstReservation).toMatchObject({
      replayed: false,
      admissionTermsDigest: "8".repeat(64),
    });
    await expect(
      store.reserve({
        ...reservation,
        sessionRef: "voice-livekit-1",
        reservationRef: "voice-livekit-reservation-1",
        generation: 1,
        transportKind: "livekit_room_v1",
        ticketDigest: "4".repeat(64),
        ticketExpiresAt: "2026-07-28T13:01:30.000Z",
        sessionExpiresAt: "2026-07-28T13:11:00.000Z",
        nowIso: "2026-07-28T13:00:01.000Z",
        admissionBinding: {
          admissionRef: "sarah_voice_admission:livekit-1",
          creditRateMsatPerMillionTokens: 100_000,
          termsDigest: "9".repeat(64),
          spendableRemainingCreditMsat: 9_000,
        },
      }),
    ).resolves.toMatchObject({
      sessionRef: "voice-livekit-1",
      ticketExpiresAt: "2026-07-28T13:01:00.000Z",
      sessionExpiresAt: "2026-07-28T13:10:00.000Z",
      admissionExpiresAt: "2026-07-28T13:02:00.000Z",
      admissionTermsDigest: "8".repeat(64),
      replayed: true,
    });
    const [balanceAfterReplay] = await sql`
      SELECT held_msat
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-livekit'
    `;
    expect(Number(balanceAfterReplay?.held_msat)).toBe(1_000);
    const binding = {
      sessionRef: "voice-livekit-1",
      ownerUserId: reservation.ownerUserId,
      deviceRef: reservation.deviceRef,
      threadRef: reservation.threadRef,
      generation: 1,
      capabilityProfile: reservation.clientProfile,
      admissionRef: "sarah_voice_admission:livekit-1",
      admissionDigest: "8".repeat(64),
      roomContext: {
        kind: "community",
        communityRef: "community-livekit",
        channelRef: "channel-livekit",
        membershipRevision: "membership-revision-7",
      } as const,
      roomRef: "room-livekit-1",
      roomEpoch: 1,
      participantRef: "participant-owner-livekit-1",
      sarahParticipantRef: "principal.sarah",
      participantGrantDigest: "5".repeat(64),
      joinExpiresAt: "2026-07-28T13:01:00.000Z",
      dispatchRef: "dispatch-livekit-1",
      sarahPresenceLeaseRef: "presence-livekit-1",
      workerControlTokenDigest: "b".repeat(64),
      publishAllowed: false,
      subscribeAllowed: true,
      nowIso: reservation.nowIso,
    };
    await store.prepareLiveKitProvisioningIntent({
      sessionRef: binding.sessionRef,
      ownerUserId: binding.ownerUserId,
      deviceRef: binding.deviceRef,
      threadRef: binding.threadRef,
      generation: binding.generation,
      capabilityProfile: binding.capabilityProfile,
      admissionRef: binding.admissionRef,
      admissionDigest: binding.admissionDigest,
      idempotencyKey: "sarah-livekit:voice-livekit-1:1",
      workerControlTokenDigest: binding.workerControlTokenDigest,
      roomContext: binding.roomContext,
      nowIso: binding.nowIso,
    });
    const concurrentClaims = await Promise.all([
      store.claimLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:one",
        staleBeforeIso: "2026-07-28T12:59:30.000Z",
        nowIso: "2026-07-28T13:00:00.500Z",
      }),
      store.claimLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:two",
        staleBeforeIso: "2026-07-28T12:59:30.000Z",
        nowIso: "2026-07-28T13:00:00.500Z",
      }),
    ]);
    expect([...concurrentClaims].sort()).toEqual([false, true]);
    const [claimedIntent] = await sql`
      SELECT provisioning_owner_ref
      FROM sarah_livekit_provisioning_intents
      WHERE session_ref = ${binding.sessionRef}
    `;
    const losingOwnerRef =
      claimedIntent?.provisioning_owner_ref === "issuer:one" ? "issuer:two" : "issuer:one";
    const winningOwnerRef =
      claimedIntent?.provisioning_owner_ref === "issuer:one" ? "issuer:one" : "issuer:two";
    await expect(
      store.bindLiveKitRoom({
        ...binding,
        provisioningOwnerRef: losingOwnerRef,
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(
      store.settleLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: losingOwnerRef,
        closeReason: "loser_must_not_settle",
        nowIso: "2026-07-28T13:00:00.750Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    expect(
      await store.claimLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:takeover",
        staleBeforeIso: "2026-07-28T13:00:01.000Z",
        nowIso: "2026-07-28T13:00:31.000Z",
      }),
    ).toBe(true);
    await expect(
      store.settleLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: winningOwnerRef,
        closeReason: "superseded_owner_must_not_settle",
        nowIso: "2026-07-28T13:00:31.250Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    expect(
      await store.claimLiveKitProvisioningIntents({
        staleBeforeIso: "2026-07-28T12:59:00.000Z",
        nowIso: "2026-07-28T13:00:01.000Z",
        provisioningOwnerRef: "reconciler:not-stale",
      }),
    ).toEqual([]);
    await sql`
      UPDATE sarah_livekit_provisioning_intents
      SET state = 'reconciling',
          provisioning_owner_ref = NULL,
          provisioning_claimed_at = NULL,
          updated_at = '2026-07-28T12:58:00.000Z'
      WHERE session_ref = 'voice-livekit-1'
    `;
    expect(
      await store.claimLiveKitProvisioningIntents({
        staleBeforeIso: "2026-07-28T12:59:00.000Z",
        nowIso: "2026-07-28T13:00:02.000Z",
        provisioningOwnerRef: "reconciler:one",
      }),
    ).toEqual([
      {
        sessionRef: "voice-livekit-1",
        generation: 1,
        idempotencyKey: "sarah-livekit:voice-livekit-1:1",
        provisioningOwnerRef: "reconciler:one",
        // The claim spends one of the bounded attempts (#9282 follow-up).
        cleanupAttemptCount: 1,
      },
    ]);
    await sql`
      UPDATE sarah_livekit_provisioning_intents
      SET state = 'pending',
          provisioning_owner_ref = NULL,
          provisioning_claimed_at = NULL,
          updated_at = '2026-07-28T13:00:00.000Z'
      WHERE session_ref = 'voice-livekit-1'
    `;
    expect(
      await store.claimLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:binding",
        staleBeforeIso: "2026-07-28T12:59:30.000Z",
        nowIso: binding.nowIso,
      }),
    ).toBe(true);
    await expect(
      store.bindLiveKitRoom({
        ...binding,
        provisioningOwnerRef: "issuer:binding",
        roomContext: {
          ...binding.roomContext,
          membershipRevision: "membership-revision-8",
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await store.bindLiveKitRoom({
      ...binding,
      provisioningOwnerRef: "issuer:binding",
    });
    await expect(
      store.readLiveKitWorkerReadiness({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
      }),
    ).resolves.toBe("waiting");
    await expect(
      store.bindLiveKitRoom({
        ...binding,
        provisioningOwnerRef: "issuer:binding",
      }),
    ).resolves.toBeUndefined();
    expect(
      await store.readLiveKitCleanup({
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).toBeUndefined();

    const workerClaim = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerRefDigest: "c".repeat(64),
      workerJobRef: "job-livekit-1",
      workerRoomSid: "RM_livekit_1",
      sessionRef: binding.sessionRef,
      generation: binding.generation,
      roomRef: binding.roomRef,
      roomEpoch: binding.roomEpoch,
      dispatchRef: binding.dispatchRef,
      participantRef: binding.participantRef,
      sarahParticipantRef: binding.sarahParticipantRef,
      sarahPresenceLeaseRef: binding.sarahPresenceLeaseRef,
      capabilityProfile: binding.capabilityProfile,
      roomContext: binding.roomContext,
      nowIso: "2026-07-28T13:00:10.000Z",
    } as const;
    await expect(store.claimLiveKitWorkerJob(workerClaim)).resolves.toMatchObject({
      sessionRef: binding.sessionRef,
      generation: 1,
      ownerUserId: binding.ownerUserId,
      roomContext: binding.roomContext,
    });
    await expect(
      store.readLiveKitWorkerReadiness({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
      }),
    ).resolves.toBe("claimed");
    await expect(
      store.completeLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:binding",
        nowIso: "2026-07-28T13:00:10.000Z",
      }),
    ).resolves.toBeUndefined();
    const [completedIntent] = await sql`
      SELECT state, provisioning_owner_ref, provisioning_claimed_at
      FROM sarah_livekit_provisioning_intents
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(completedIntent).toMatchObject({
      state: "bound",
      provisioning_owner_ref: null,
      provisioning_claimed_at: null,
    });
    await expect(store.claimLiveKitWorkerJob(workerClaim)).resolves.toMatchObject({
      sessionRef: binding.sessionRef,
      generation: 1,
    });
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET state = 'active', updated_at = '2026-07-28T13:00:10.250Z'
      WHERE session_ref = ${binding.sessionRef}
    `;
    await expect(
      store.prepareLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        ownerUserId: binding.ownerUserId,
        deviceRef: binding.deviceRef,
        threadRef: binding.threadRef,
        generation: binding.generation,
        capabilityProfile: binding.capabilityProfile,
        admissionRef: binding.admissionRef,
        admissionDigest: binding.admissionDigest,
        idempotencyKey: "sarah-livekit:voice-livekit-1:1",
        workerControlTokenDigest: binding.workerControlTokenDigest,
        roomContext: binding.roomContext,
        nowIso: "2026-07-28T13:00:10.500Z",
      }),
    ).resolves.toBeUndefined();
    expect(
      await store.claimLiveKitProvisioningIntent({
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        provisioningOwnerRef: "issuer:active-replay",
        staleBeforeIso: "2026-07-28T12:59:40.000Z",
        nowIso: "2026-07-28T13:00:10.500Z",
      }),
    ).toBe(true);
    await expect(
      store.bindLiveKitRoom({
        ...binding,
        provisioningOwnerRef: "issuer:active-replay",
        participantGrantDigest: "6".repeat(64),
        joinExpiresAt: "2026-07-28T13:01:30.000Z",
        nowIso: "2026-07-28T13:00:10.500Z",
      }),
    ).resolves.toBeUndefined();
    const [activeReplayBinding] = await sql`
      SELECT state, participant_grant_digest, join_expires_at
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(activeReplayBinding).toMatchObject({
      state: "active",
      participant_grant_digest: "6".repeat(64),
      join_expires_at: "2026-07-28T13:01:30.000Z",
    });
    await expect(
      store.claimLiveKitWorkerJob({
        ...workerClaim,
        workerRefDigest: "d".repeat(64),
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(
      store.authorizeLiveKitWorkerEvent({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        workerRoomSid: workerClaim.workerRoomSid,
        sessionRef: binding.sessionRef,
        generation: 1,
        nowIso: "2026-07-28T13:00:11.000Z",
      }),
    ).resolves.toMatchObject({
      roomRef: binding.roomRef,
      sarahParticipantRef: binding.sarahParticipantRef,
    });
    await expect(
      store.readLiveKitMembershipLease({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toEqual({
      ownerUserId: binding.ownerUserId,
      sarahPresenceLeaseRef: binding.sarahPresenceLeaseRef,
      roomContext: binding.roomContext,
    });

    const ownerJoin = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      role: "owner",
    } as const;
    await store.recordLiveKitParticipantJoin({
      ...ownerJoin,
      nowIso: "2026-07-28T13:00:20.000Z",
    });
    const [ownerAdmitted] = await sql`
      SELECT state, owner_joined_at
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(ownerAdmitted).toMatchObject({
      state: "active",
      owner_joined_at: "2026-07-28T13:00:20.000Z",
    });
    // A second admission of the admitted identity is a duplicate participant,
    // not a resume: two live clients would hold one room seat.
    await expect(
      store.recordLiveKitParticipantJoin({
        ...ownerJoin,
        nowIso: "2026-07-28T13:00:21.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceDuplicateParticipantError);
    // A worker that never claimed this generation is refused, and is refused as
    // an unknown participant rather than as a duplicate.
    const foreignJoin = store.recordLiveKitParticipantJoin({
      ...ownerJoin,
      workerJobRef: "job-livekit-unclaimed",
      nowIso: "2026-07-28T13:00:21.000Z",
    });
    await expect(foreignJoin).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(foreignJoin).rejects.not.toBeInstanceOf(SarahVoiceDuplicateParticipantError);
    const connectedEvent = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: "connected:job-livekit-1",
      eventPayloadDigest: "9".repeat(64),
      eventKind: "worker_connected",
      workerRoomSid: workerClaim.workerRoomSid,
      nowIso: "2026-07-28T13:00:22.000Z",
    } as const;
    await expect(
      store.connect({
        sessionRef: binding.sessionRef,
        ticketDigest: "4".repeat(64),
        nowIso: "2026-07-28T13:00:21.500Z",
      }),
    ).resolves.toMatchObject({ state: "connected" });
    await expect(
      store.connect({
        sessionRef: binding.sessionRef,
        ticketDigest: "4".repeat(64),
        nowIso: "2026-07-28T13:00:21.600Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(store.applyLiveKitWorkerEvent(connectedEvent)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:22.000Z",
      replayed: false,
    });
    await expect(
      store.applyLiveKitWorkerEvent({
        ...connectedEvent,
        nowIso: "2026-07-28T13:00:30.000Z",
      }),
    ).resolves.toEqual({
      observedAt: "2026-07-28T13:00:22.000Z",
      replayed: true,
    });
    const providerAdmittedEvent = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: `provider:${"a".repeat(64)}`,
      eventPayloadDigest: "6".repeat(64),
      eventKind: "provider_admitted",
      providerSessionRefDigest: "a".repeat(64),
      providerConfigurationDigest: "b".repeat(64),
      nowIso: "2026-07-28T13:00:23.000Z",
    } as const;
    await expect(store.applyLiveKitWorkerEvent(providerAdmittedEvent)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.000Z",
      replayed: false,
    });
    await expect(
      store.readLiveKitProviderAdmission({
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toEqual({
      state: "admitted",
      providerSessionRefDigest: "a".repeat(64),
      providerConfigurationDigest: "b".repeat(64),
      admittedAt: "2026-07-28T13:00:23.000Z",
    });
    await expect(
      store.requestLiveKitProviderDisconnectFault({
        requestRef: "acceptance:provider-disconnect:wrong-generation",
        sessionRef: binding.sessionRef,
        generation: 2,
        providerSessionRefDigest: "a".repeat(64),
        operatorActorRef: "operator.sarah_livekit_acceptance",
        nowIso: "2026-07-28T13:00:23.125Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(
      store.requestLiveKitProviderDisconnectFault({
        requestRef: "acceptance:provider-disconnect:wrong-provider",
        sessionRef: binding.sessionRef,
        generation: 1,
        providerSessionRefDigest: "c".repeat(64),
        operatorActorRef: "operator.sarah_livekit_acceptance",
        nowIso: "2026-07-28T13:00:23.125Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    const providerDisconnectRequest = {
      requestRef: "acceptance:provider-disconnect:one",
      sessionRef: binding.sessionRef,
      generation: 1,
      providerSessionRefDigest: "a".repeat(64),
      operatorActorRef: "operator.sarah_livekit_acceptance",
      nowIso: "2026-07-28T13:00:23.125Z",
    } as const;
    await expect(
      store.requestLiveKitProviderDisconnectFault(providerDisconnectRequest),
    ).resolves.toEqual({
      requestRef: providerDisconnectRequest.requestRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      providerSessionRefDigest: "a".repeat(64),
      state: "requested",
      replayed: false,
    });
    await expect(
      store.requestLiveKitProviderDisconnectFault(providerDisconnectRequest),
    ).resolves.toEqual({
      requestRef: providerDisconnectRequest.requestRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      providerSessionRefDigest: "a".repeat(64),
      state: "requested",
      replayed: true,
    });
    await expect(
      store.requestLiveKitProviderDisconnectFault({
        ...providerDisconnectRequest,
        requestRef: "acceptance:provider-disconnect:overlap",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    const providerDisconnectLease = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: "lease:provider-disconnect",
      eventPayloadDigest: "7".repeat(64),
      eventKind: "lease_check",
      nowIso: "2026-07-28T13:00:23.250Z",
    } as const;
    await expect(store.applyLiveKitWorkerEvent(providerDisconnectLease)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.250Z",
      replayed: false,
      interruptSequence: 0,
      providerDisconnectFault: {
        requestRef: providerDisconnectRequest.requestRef,
        providerSessionRefDigest: "a".repeat(64),
      },
    });
    await expect(store.applyLiveKitWorkerEvent(providerDisconnectLease)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.250Z",
      replayed: true,
      interruptSequence: 0,
      providerDisconnectFault: {
        requestRef: providerDisconnectRequest.requestRef,
        providerSessionRefDigest: "a".repeat(64),
      },
    });
    const providerDisconnectApplied = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: "provider-disconnect:applied",
      eventPayloadDigest: "8".repeat(64),
      eventKind: "provider_disconnect_fault_applied",
      requestRef: providerDisconnectRequest.requestRef,
      providerSessionRefDigest: "a".repeat(64),
      nowIso: "2026-07-28T13:00:23.375Z",
    } as const;
    await expect(store.applyLiveKitWorkerEvent(providerDisconnectApplied)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.375Z",
      replayed: false,
    });
    await expect(store.applyLiveKitWorkerEvent(providerDisconnectApplied)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.375Z",
      replayed: true,
    });
    const [providerDisconnectRow] = await sql`
      SELECT applied_at, worker_job_ref
      FROM sarah_livekit_provider_disconnect_faults
      WHERE request_ref = ${providerDisconnectRequest.requestRef}
    `;
    expect(providerDisconnectRow).toMatchObject({
      applied_at: "2026-07-28T13:00:23.375Z",
      worker_job_ref: workerClaim.workerJobRef,
    });
    await expect(
      store.requestLiveKitWorkerInterrupt({
        sessionRef: binding.sessionRef,
        generation: 2,
        nowIso: "2026-07-28T13:00:23.250Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await expect(
      store.requestLiveKitWorkerInterrupt({
        sessionRef: binding.sessionRef,
        generation: 1,
        nowIso: "2026-07-28T13:00:23.500Z",
      }),
    ).resolves.toEqual({
      interruptSequence: 1,
      roomRef: binding.roomRef,
      roomEpoch: binding.roomEpoch,
      sarahParticipantRef: binding.sarahParticipantRef,
    });
    await expect(
      store.readLiveKitWorkerInterruptApplied({
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toBe(0);
    const interruptApplied = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: "interrupt:applied:1",
      eventPayloadDigest: "2".repeat(64),
      eventKind: "interrupt_applied",
      interruptSequence: 1,
      nowIso: "2026-07-28T13:00:23.750Z",
    } as const;
    await expect(store.applyLiveKitWorkerEvent(interruptApplied)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.750Z",
      replayed: false,
    });
    await expect(store.applyLiveKitWorkerEvent(interruptApplied)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:23.750Z",
      replayed: true,
    });
    await expect(
      store.readLiveKitWorkerInterruptApplied({
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toBe(1);
    await expect(
      store.applyLiveKitWorkerEvent({
        ...interruptApplied,
        eventRef: "interrupt:applied:2",
        eventPayloadDigest: "3".repeat(64),
        interruptSequence: 2,
        nowIso: "2026-07-28T13:00:23.875Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    const interruptLease = {
      workerControlTokenDigest: binding.workerControlTokenDigest,
      workerJobRef: workerClaim.workerJobRef,
      sessionRef: binding.sessionRef,
      generation: 1,
      eventRef: "lease:interrupt-1",
      eventPayloadDigest: "1".repeat(64),
      eventKind: "lease_check",
      nowIso: "2026-07-28T13:00:24.000Z",
    } as const;
    await expect(store.applyLiveKitWorkerEvent(interruptLease)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:24.000Z",
      replayed: false,
      interruptSequence: 1,
    });
    await expect(store.applyLiveKitWorkerEvent(interruptLease)).resolves.toEqual({
      observedAt: "2026-07-28T13:00:24.000Z",
      replayed: true,
      interruptSequence: 1,
    });
    await store.sweepExpired("2026-07-28T13:01:30.000Z");
    const [connectedSession] = await sql`
      SELECT state, ticket_digest, connected_at
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = 'voice-livekit-1'
    `;
    expect(connectedSession).toMatchObject({
      state: "connected",
      ticket_digest: null,
      connected_at: "2026-07-28T13:00:21.500Z",
    });
    const [staleWorker] = await sql`
      SELECT worker_stop_reason, worker_stop_close_reason,
        worker_stop_requested_at, worker_stop_deadline_at
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${binding.sessionRef}
        AND generation = ${binding.generation}
    `;
    expect(staleWorker).toMatchObject({
      worker_stop_reason: "worker_unavailable",
      worker_stop_close_reason: "livekit_worker_heartbeat_expired",
      worker_stop_requested_at: "2026-07-28T13:01:30.000Z",
    });
    expect(staleWorker?.worker_stop_deadline_at).toBe("2026-07-28T13:04:00.000Z");
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET worker_stop_reason = NULL, worker_stop_close_reason = NULL,
        worker_stop_requested_at = NULL, worker_stop_deadline_at = NULL
      WHERE session_ref = ${binding.sessionRef}
        AND generation = ${binding.generation}
    `;
    const usage = {
      providerResponseRef: "livekit-provider-response-1",
      inputTokens: 100,
      outputTokens: 50,
      cachedInputTokens: 10,
      audioInputTokens: 80,
      audioOutputTokens: 40,
      chargeMsat: 250,
      observedAt: "2026-07-28T13:00:40.000Z",
    } as const;
    await expect(
      store.recordUsage({
        sessionRef: binding.sessionRef,
        generation: 2,
        usage,
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await store.recordUsage({
      sessionRef: binding.sessionRef,
      generation: 1,
      usage,
    });
    await expect(
      store.recordUsage({
        sessionRef: binding.sessionRef,
        generation: 1,
        usage: { ...usage, inputTokens: usage.inputTokens + 1 },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await store.recordUsage({
      sessionRef: binding.sessionRef,
      generation: 1,
      usage: { ...usage, chargeMsat: 999_999 },
    });
    const applicationDatabaseUrl = (applicationName: string): string => {
      const databaseUrl = new URL(pg.urlFor("khala_sync_sarah_voice"));
      databaseUrl.searchParams.set("application_name", applicationName);
      return databaseUrl.toString();
    };
    const blockerSql = SQL({
      url: applicationDatabaseUrl("sarah-lock-blocker"),
      max: 1,
    });
    const eventSql = SQL({
      url: applicationDatabaseUrl("sarah-lock-event"),
      max: 1,
    });
    const revocationSql = SQL({
      url: applicationDatabaseUrl("sarah-lock-revocation"),
      max: 1,
    });
    let releaseBindingLock: (() => void) | undefined;
    let bindingLockReady: (() => void) | undefined;
    const bindingLockReleased = new Promise<void>((resolve) => {
      releaseBindingLock = resolve;
    });
    const bindingLocked = new Promise<void>((resolve) => {
      bindingLockReady = resolve;
    });
    const blocker = blockerSql.begin(async (tx) => {
      await tx`
        SELECT session_ref
        FROM sarah_livekit_room_bindings
        WHERE session_ref = ${binding.sessionRef}
          AND generation = ${binding.generation}
        FOR UPDATE
      `;
      bindingLockReady?.();
      await bindingLockReleased;
    });
    try {
      await bindingLocked;
      const eventStore = makeSarahRealtimeVoiceStore(eventSql as unknown as SyncSql);
      const event = eventStore.applyLiveKitWorkerEvent({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
        eventRef: "lease:concurrent-cohort-revocation",
        eventPayloadDigest: "0".repeat(64),
        eventKind: "lease_check",
        nowIso: "2026-07-28T13:00:58.000Z",
      });
      await waitForBlockedApplication(sql, "sarah-lock-event");

      const revocationStore = makeSarahRealtimeVoiceStore(revocationSql as unknown as SyncSql);
      const revocation = revocationStore.revokeAlphaCohort({
        cohortRef: reservation.admissionCohortRef,
        actorRef: "operator:test-lock-order",
        reason: "Concurrent lock-order regression",
        nowIso: "2026-07-28T13:00:58.500Z",
      });
      await waitForBlockedApplication(sql, "sarah-lock-revocation");
      releaseBindingLock?.();

      const completed = await completeWithin(
        Promise.all([event, revocation]),
        2_000,
        "Concurrent voice revocation did not complete",
      );
      expect(completed).toEqual([
        {
          observedAt: "2026-07-28T13:00:58.000Z",
          replayed: false,
          interruptSequence: 1,
        },
        1,
      ]);
    } finally {
      releaseBindingLock?.();
      await blocker;
      await Promise.all([blockerSql.end(), eventSql.end(), revocationSql.end()]);
    }
    await sql`
      UPDATE sarah_voice_alpha_memberships
      SET state = 'active', revoked_at = NULL, revocation_actor_ref = NULL,
        revocation_reason = NULL, updated_at = '2026-07-28T13:00:58.750Z'
      WHERE membership_ref = 'sarah_voice_alpha:user-sarah-livekit'
    `;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET worker_stop_reason = NULL, worker_stop_close_reason = NULL,
        worker_stop_requested_at = NULL, worker_stop_deadline_at = NULL
      WHERE session_ref = ${binding.sessionRef}
    `;
    await expect(
      store.revokeLiveKitRoom({
        sessionRef: binding.sessionRef,
        generation: 2,
        stopReason: "membership_revoked",
        reason: "operator_kill",
        nowIso: "2026-07-28T13:01:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    await sql`
      UPDATE sarah_voice_alpha_memberships
      SET state = 'revoked', revoked_at = '2026-07-28T13:00:59.000Z',
        revocation_actor_ref = 'operator:test',
        revocation_reason = 'Direct membership revocation',
        updated_at = '2026-07-28T13:00:59.000Z'
      WHERE membership_ref = 'sarah_voice_alpha:user-sarah-livekit'
    `;
    await expect(
      store.applyLiveKitWorkerEvent({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
        eventRef: "lease:membership-revoked",
        eventPayloadDigest: "6".repeat(64),
        eventKind: "lease_check",
        nowIso: "2026-07-28T13:00:59.500Z",
      }),
    ).resolves.toMatchObject({
      replayed: false,
      stopReason: "membership_revoked",
    });
    await sql`
      UPDATE sarah_voice_alpha_memberships
      SET state = 'active', revoked_at = NULL, revocation_actor_ref = NULL,
        revocation_reason = NULL, updated_at = '2026-07-28T13:00:59.750Z'
      WHERE membership_ref = 'sarah_voice_alpha:user-sarah-livekit'
    `;
    await sql`
      UPDATE sarah_livekit_room_bindings
      SET worker_stop_reason = NULL, worker_stop_close_reason = NULL,
        worker_stop_requested_at = NULL, worker_stop_deadline_at = NULL
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(
      await store.revokeAlphaCohort({
        cohortRef: reservation.admissionCohortRef,
        actorRef: "operator:test",
        reason: "membership_removed",
        nowIso: "2026-07-28T13:01:00.000Z",
      }),
    ).toBe(1);
    expect(
      await store.revokeAlphaCohort({
        cohortRef: reservation.admissionCohortRef,
        actorRef: "operator:test",
        reason: "duplicate_membership_removal",
        nowIso: "2026-07-28T13:01:00.250Z",
      }),
    ).toBe(0);
    const [draining] = await sql`
      SELECT state, charged_msat
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(draining?.state).toBe("connected");
    expect(Number(draining?.charged_msat)).toBe(15);
    const [stopRequested] = await sql`
      SELECT worker_stop_reason, worker_stop_close_reason
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${binding.sessionRef}
    `;
    expect(stopRequested).toMatchObject({
      worker_stop_reason: "membership_revoked",
      worker_stop_close_reason: "membership_removed",
    });
    await expect(
      store.revokeLiveKitRoom({
        sessionRef: binding.sessionRef,
        generation: 1,
        stopReason: "membership_revoked",
        reason: "membership_removed",
        nowIso: "2026-07-28T13:01:00.000Z",
      }),
    ).resolves.toMatchObject({
      state: "connected",
      chargedMsat: 15,
    });
    await expect(
      store.applyLiveKitWorkerEvent({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
        eventRef: "response:after-membership-removal",
        eventPayloadDigest: "8".repeat(64),
        eventKind: "response_usage",
        usage: {
          providerResponseRef: "response:after-membership-removal",
          providerStatus: "cancelled",
          inputTokens: 40,
          outputTokens: 10,
          cachedInputTokens: 0,
          audioInputTokens: 40,
          audioOutputTokens: 10,
        },
        nowIso: "2026-07-28T13:01:00.500Z",
      }),
    ).resolves.toMatchObject({
      replayed: false,
      stopReason: "membership_revoked",
    });
    const [terminalUsage] = await sql`
      SELECT provider_status
      FROM sarah_realtime_voice_usage
      WHERE session_ref = ${binding.sessionRef}
        AND provider_response_ref = 'response:after-membership-removal'
    `;
    expect(terminalUsage?.provider_status).toBe("cancelled");
    await expect(
      store.applyLiveKitWorkerEvent({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
        eventRef: "close:job-livekit-1",
        eventPayloadDigest: "7".repeat(64),
        eventKind: "close",
        closeReason: "livekit_worker_completed",
        accountingStatus: "exact",
        nowIso: "2026-07-28T13:01:01.000Z",
      }),
    ).resolves.toMatchObject({ replayed: false });
    await expect(
      store.readLiveKitMembershipLease({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toEqual({
      ownerUserId: binding.ownerUserId,
      sarahPresenceLeaseRef: binding.sarahPresenceLeaseRef,
      roomContext: binding.roomContext,
    });
    expect(
      await store.revokeLiveKitRoom({
        sessionRef: binding.sessionRef,
        generation: 1,
        stopReason: "operator_stop",
        reason: "operator_kill",
        nowIso: "2026-07-28T13:01:02.000Z",
      }),
    ).toMatchObject({ state: "settled", chargedMsat: 20 });
    const [revokedSession] = await sql`
      SELECT close_reason
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = 'voice-livekit-1'
    `;
    expect(revokedSession?.close_reason).toBe("membership_removed");
    const acceptanceEvidence = await store.readSettlement({
      sessionRef: binding.sessionRef,
      ownerUserId: binding.ownerUserId,
    });
    expect(acceptanceEvidence).toMatchObject({
      acceptanceEvidence: {
        principal: "principal.sarah",
        providerAccountingStatus: "exact",
        workerJobCount: 1,
        providerSessionCount: 1,
        usage: {
          responseCount: 2,
          cancelledResponseCount: 1,
          chargeMsat: 20,
        },
      },
    });
    const identityDigests = Object.values(
      acceptanceEvidence?.state === "settled"
        ? (acceptanceEvidence.acceptanceEvidence?.identityDigests ?? {})
        : {},
    );
    expect(identityDigests).toHaveLength(8);
    expect(new Set(identityDigests).size).toBe(8);
    const cleanup = await store.readLiveKitCleanup({
      sessionRef: binding.sessionRef,
      generation: 1,
    });
    expect(cleanup).toMatchObject({
      roomRef: binding.roomRef,
      dispatchRef: binding.dispatchRef,
    });
    const claimedCleanup = await store.claimLiveKitCleanups({
      staleBeforeIso: "2026-07-28T13:01:01.000Z",
      nowIso: "2026-07-28T13:01:10.000Z",
    });
    expect(claimedCleanup).toEqual([
      expect.objectContaining({
        sessionRef: binding.sessionRef,
        roomRef: binding.roomRef,
        cleanupAttemptedAt: "2026-07-28T13:01:10.000Z",
      }),
    ]);
    expect(
      await store.claimLiveKitCleanups({
        staleBeforeIso: "2026-07-28T13:01:09.999Z",
        nowIso: "2026-07-28T13:01:11.000Z",
      }),
    ).toEqual([]);
    // EP263-LK H4 (#9282): a failed cleanup earns a bounded, exponentially
    // later retry rather than an immediate re-claim. The first failure is
    // attempt one, so the next attempt is due 15 seconds later; a reconciler
    // pass before that must claim nothing, which is what stops the loop from
    // spinning on a room the broker will never delete.
    expect(
      await store.markLiveKitCleanup({
        sessionRef: binding.sessionRef,
        generation: 1,
        state: "cleanup_failed",
        nowIso: "2026-07-28T13:01:11.000Z",
      }),
    ).toEqual({ state: "cleanup_failed", cleanupAttemptCount: 1 });
    expect(
      await store.claimLiveKitCleanups({
        staleBeforeIso: "2026-07-28T13:01:11.000Z",
        nowIso: "2026-07-28T13:01:12.000Z",
      }),
    ).toEqual([]);
    expect(
      await store.claimLiveKitCleanups({
        staleBeforeIso: "2026-07-28T13:01:11.000Z",
        nowIso: "2026-07-28T13:01:26.000Z",
      }),
    ).toHaveLength(1);
    await store.markLiveKitCleanup({
      sessionRef: binding.sessionRef,
      generation: 1,
      state: "cleaned",
      nowIso: "2026-07-28T13:01:13.000Z",
    });
    await store.markLiveKitCleanup({
      sessionRef: binding.sessionRef,
      generation: 1,
      state: "cleaned",
      nowIso: "2026-07-28T13:01:14.000Z",
    });
    await expect(
      store.readLiveKitMembershipLease({
        workerControlTokenDigest: binding.workerControlTokenDigest,
        workerJobRef: workerClaim.workerJobRef,
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).resolves.toEqual({
      ownerUserId: binding.ownerUserId,
      sarahPresenceLeaseRef: binding.sarahPresenceLeaseRef,
      roomContext: binding.roomContext,
    });
    expect(
      await store.readLiveKitCleanup({
        sessionRef: binding.sessionRef,
        generation: 1,
      }),
    ).toBeUndefined();
    const [usageCount] = await sql`
      SELECT COUNT(*) AS count
      FROM sarah_realtime_voice_usage
      WHERE session_ref = 'voice-livekit-1'
    `;
    expect(Number(usageCount?.count)).toBe(2);

    await sql`
      UPDATE sarah_voice_alpha_memberships
      SET state = 'active', revoked_at = NULL, revocation_actor_ref = NULL,
        revocation_reason = NULL, updated_at = '2026-07-28T13:01:59.000Z'
      WHERE membership_ref = 'sarah_voice_alpha:user-sarah-livekit'
    `;
    await store.reserve({
      ...reservation,
      sessionRef: "voice-livekit-2",
      reservationRef: "voice-livekit-reservation-2",
      generation: 2,
      ticketDigest: "6".repeat(64),
      ticketExpiresAt: "2026-07-28T13:03:00.000Z",
      sessionExpiresAt: "2026-07-28T13:10:00.000Z",
      nowIso: "2026-07-28T13:02:00.000Z",
    });
    await sql.begin(async (tx) => {
      await tx`
        UPDATE sarah_realtime_voice_sessions
        SET state = 'failed', ticket_digest = NULL,
            updated_at = '2026-07-28T13:02:10.000Z'
        WHERE session_ref = 'voice-livekit-2'
      `;
      await tx`
        UPDATE agent_balances
        SET held_msat = held_msat - 1000
        WHERE actor_ref = 'agent:user-sarah-livekit'
      `;
    });
    await expect(
      store.reserve({
        ...reservation,
        sessionRef: "voice-livekit-3",
        reservationRef: "voice-livekit-reservation-3",
        generation: 3,
        ticketDigest: "7".repeat(64),
        ticketExpiresAt: "2026-07-28T13:04:00.000Z",
        sessionExpiresAt: "2026-07-28T13:10:00.000Z",
        nowIso: "2026-07-28T13:03:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceConcurrentSessionError);
  });

  test("reconciles uncertain LiveKit accounting exactly and idempotently", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO users (
        id, kind, display_name, status, created_at, updated_at
      ) VALUES (
        'user-sarah-reconciliation', 'human', 'Reconciliation Tester',
        'active', '2026-07-28T13:30:00.000Z', '2026-07-28T13:30:00.000Z'
      )
    `;
    await sql`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, usd_credit_msat,
        created_at, updated_at
      ) VALUES (
        'agent:user-sarah-reconciliation', 10000, 1000, 0,
        '2026-07-28T13:30:00.000Z', '2026-07-28T13:30:00.000Z'
      )
    `;
    await sql`
      INSERT INTO sarah_realtime_voice_sessions (
        session_ref, reservation_ref, owner_user_id, owner_actor_ref,
        device_ref, thread_ref, generation, disclosure_ref, state,
        reserved_msat, charged_msat, ticket_expires_at, session_expires_at,
        created_at, updated_at, connected_at, client_profile, credit_mode,
        transport_kind, credit_rate_msat_per_million_tokens,
        input_tokens, output_tokens, cached_input_tokens,
        audio_input_tokens, audio_output_tokens
      ) VALUES (
        'voice-livekit-reconciliation',
        'voice-livekit-reconciliation-reservation',
        'user-sarah-reconciliation', 'agent:user-sarah-reconciliation',
        'omega-reconciliation', 'thread-reconciliation', 1,
        'disclosure-reconciliation', 'accounting_uncertain',
        1000, 15, '2026-07-28T13:31:00.000Z',
        '2026-07-28T13:40:00.000Z', '2026-07-28T13:30:00.000Z',
        '2026-07-28T13:31:00.000Z', '2026-07-28T13:30:30.000Z',
        'omega_editor', 'metered', 'livekit_room_v1', 100000,
        100, 50, 10, 80, 40
      )
    `;
    await sql`
      INSERT INTO sarah_livekit_room_bindings (
        session_ref, owner_user_id, device_ref, thread_ref, generation,
        capability_profile, admission_ref, admission_digest,
        room_context_kind, room_ref, room_epoch, participant_ref,
        sarah_participant_ref, participant_grant_digest, join_expires_at,
        dispatch_ref, sarah_presence_lease_ref, publish_allowed,
        subscribe_allowed, state, created_at, updated_at,
        worker_job_ref, worker_last_seen_at,
        provider_session_ref_digest, provider_configuration_digest,
        provider_admitted_at, provider_accounting_status,
        provider_accounting_uncertain_at,
        provider_accounting_uncertain_reason
      ) VALUES (
        'voice-livekit-reconciliation', 'user-sarah-reconciliation',
        'omega-reconciliation', 'thread-reconciliation', 1,
        'omega_editor', 'admission-reconciliation', ${"a".repeat(64)},
        'private', 'room-reconciliation', 1, 'owner-reconciliation',
        'principal.sarah', ${"b".repeat(64)},
        '2026-07-28T13:35:00.000Z', 'dispatch-reconciliation',
        'presence-reconciliation', false, true, 'active',
        '2026-07-28T13:30:00.000Z', '2026-07-28T13:31:00.000Z',
        'worker-job-reconciliation', '2026-07-28T13:30:55.000Z',
        ${"c".repeat(64)}, ${"d".repeat(64)},
        '2026-07-28T13:30:30.000Z', 'uncertain',
        '2026-07-28T13:31:00.000Z', 'worker_disappeared'
      )
    `;
    await sql`
      INSERT INTO sarah_realtime_voice_usage (
        session_ref, provider_response_ref, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        charge_msat, observed_at, usage_kind, provider_status
      ) VALUES (
        'voice-livekit-reconciliation', 'response:provider-response-1',
        100, 50, 10, 80, 40, 15, '2026-07-28T13:30:40.000Z',
        'response', 'completed'
      )
    `;

    await expect(
      store.readSettlement({
        sessionRef: "voice-livekit-reconciliation",
        ownerUserId: "user-sarah-reconciliation",
      }),
    ).resolves.toMatchObject({
      state: "accounting_uncertain",
      recordedChargeMsat: 15,
      reservedMsat: 1_000,
      holdPreserved: true,
      failureEvidence: {
        principal: "principal.sarah",
        generation: 1,
        identityDigests: {
          job: expect.stringMatching(/^[0-9a-f]{64}$/u),
          providerSession: "c".repeat(64),
          providerConfiguration: "d".repeat(64),
          hold: expect.stringMatching(/^[0-9a-f]{64}$/u),
          usage: expect.stringMatching(/^[0-9a-f]{64}$/u),
        },
        usage: {
          inputTokens: 100,
          outputTokens: 50,
          recordedChargeMsat: 15,
          responseCount: 1,
          transcriptionCount: 0,
        },
        providerAccountingStatus: "uncertain",
        workerJobCount: 0,
        providerSessionCount: 1,
        holdPreserved: true,
      },
    });

    const reconciliation = {
      reconciliationRef: "operator-reconciliation-1",
      reconciliationPayloadDigest: "e".repeat(64),
      sessionRef: "voice-livekit-reconciliation",
      generation: 1,
      providerSessionRefDigest: "c".repeat(64),
      operatorActorRef: "operator:reconciliation-test",
      reason: "Verified against provider usage export",
      providerEvidenceRefs: ["provider-export:2026-07-28:voice-livekit-reconciliation"],
      usage: [
        {
          usageKind: "response" as const,
          providerResponseRef: "response:provider-response-1",
          providerStatus: "completed" as const,
          inputTokens: 100,
          outputTokens: 50,
          cachedInputTokens: 10,
          audioInputTokens: 80,
          audioOutputTokens: 40,
        },
        {
          usageKind: "transcription" as const,
          providerResponseRef: "transcription:provider-transcription-1",
          inputTokens: 25,
          outputTokens: 0,
          cachedInputTokens: 0,
          audioInputTokens: 25,
          audioOutputTokens: 0,
        },
      ],
      nowIso: "2026-07-28T13:32:00.000Z",
    } as const;
    await expect(
      store.reconcileLiveKitAccounting({
        ...reconciliation,
        reconciliationRef: "operator-reconciliation-wrong-provider-session",
        reconciliationPayloadDigest: "9".repeat(64),
        providerSessionRefDigest: "f".repeat(64),
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
    const [stillUncertain] = await sql`
      SELECT state
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${reconciliation.sessionRef}
    `;
    expect(stillUncertain?.state).toBe("accounting_uncertain");
    const firstReconciliation = await store.reconcileLiveKitAccounting(reconciliation);
    expect(firstReconciliation).toMatchObject({
      reconciliationRef: reconciliation.reconciliationRef,
      reconciliationReceiptRef: `sarah_voice_accounting_reconciliation:${reconciliation.reconciliationPayloadDigest}`,
      sessionRef: reconciliation.sessionRef,
      state: "settled",
      finalChargeMsat: 18,
      replayed: false,
    });
    await expect(
      store.reconcileLiveKitAccounting({
        ...reconciliation,
        nowIso: "2026-07-28T13:33:00.000Z",
      }),
    ).resolves.toEqual({
      ...firstReconciliation,
      replayed: true,
    });
    await expect(
      store.reconcileLiveKitAccounting({
        ...reconciliation,
        reconciliationPayloadDigest: "f".repeat(64),
        nowIso: "2026-07-28T13:34:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);

    const [settledSession] = await sql`
      SELECT state, charged_msat, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        settlement_receipt_ref
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${reconciliation.sessionRef}
    `;
    expect(settledSession).toMatchObject({
      state: "settled",
      charged_msat: "18",
      input_tokens: "125",
      output_tokens: "50",
      cached_input_tokens: "10",
      audio_input_tokens: "105",
      audio_output_tokens: "40",
    });
    expect(settledSession?.settlement_receipt_ref).toBe(firstReconciliation.settlementReceiptRef);
    const [balance] = await sql`
      SELECT balance_msat, held_msat
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-reconciliation'
    `;
    expect(balance).toMatchObject({
      balance_msat: "9982",
      held_msat: "0",
    });
    const [receipt] = await sql`
      SELECT reconciliation_ref, reconciliation_receipt_ref,
        reconciliation_payload_digest, operator_actor_ref,
        provider_evidence_refs_json
      FROM sarah_livekit_accounting_reconciliations
      WHERE session_ref = ${reconciliation.sessionRef}
    `;
    expect(receipt).toMatchObject({
      reconciliation_ref: reconciliation.reconciliationRef,
      reconciliation_receipt_ref: firstReconciliation.reconciliationReceiptRef,
      reconciliation_payload_digest: reconciliation.reconciliationPayloadDigest,
      operator_actor_ref: reconciliation.operatorActorRef,
      provider_evidence_refs_json: reconciliation.providerEvidenceRefs,
    });
    const [binding] = await sql`
      SELECT state, provider_accounting_status,
        provider_accounting_uncertain_at,
        provider_accounting_uncertain_reason
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${reconciliation.sessionRef}
    `;
    expect(binding).toMatchObject({
      state: "cleanup_ready",
      provider_accounting_status: "exact",
      provider_accounting_uncertain_at: null,
      provider_accounting_uncertain_reason: null,
    });

    await expect(sql`
      INSERT INTO sarah_realtime_voice_sessions (
        session_ref, reservation_ref, owner_user_id, owner_actor_ref,
        device_ref, thread_ref, generation, disclosure_ref, state,
        reserved_msat, charged_msat, ticket_expires_at, session_expires_at,
        created_at, updated_at, client_profile, credit_mode, transport_kind,
        credit_rate_msat_per_million_tokens
      ) VALUES (
        'voice-after-reconciliation', 'reservation-after-reconciliation',
        'user-sarah-reconciliation', 'agent:user-sarah-reconciliation',
        'omega-reconciliation', 'thread-after-reconciliation', 1,
        'disclosure-after-reconciliation', 'reserved', 1000, 0,
        '2026-07-28T13:36:00.000Z', '2026-07-28T13:45:00.000Z',
        '2026-07-28T13:35:00.000Z', '2026-07-28T13:35:00.000Z',
        'omega_editor', 'metered', 'livekit_room_v1', 100000
      )
    `).resolves.toBeDefined();
  });

  test("refuses room 21 in authoritative LiveKit provisioning state", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    try {
      await sql`
        INSERT INTO sarah_realtime_voice_sessions (
          session_ref, reservation_ref, owner_user_id, owner_actor_ref,
          device_ref, thread_ref, generation, disclosure_ref, state,
          reserved_msat, charged_msat, ticket_expires_at, session_expires_at,
          created_at, updated_at, client_profile, credit_mode, transport_kind,
          credit_rate_msat_per_million_tokens
        )
        SELECT
          'capacity-session-' || slot,
          'capacity-reservation-' || slot,
          'capacity-owner-' || slot,
          'agent:capacity-owner-' || slot,
          'capacity-device-' || slot,
          'capacity-thread-' || slot,
          1,
          'capacity-disclosure-' || slot,
          'reserved',
          1,
          0,
          '2026-07-28T14:01:00.000Z',
          '2026-07-28T14:10:00.000Z',
          '2026-07-28T14:00:00.000Z',
          '2026-07-28T14:00:00.000Z',
          'omega_editor',
          'metered',
          'livekit_room_v1',
          100000
        FROM generate_series(1, 21) AS slot
      `;
      await sql`
        INSERT INTO sarah_livekit_provisioning_intents (
          session_ref, generation, idempotency_key, owner_user_id,
          device_ref, thread_ref, capability_profile, admission_ref,
          admission_digest, room_context_kind, worker_control_token_digest,
          state, created_at, updated_at
        )
        SELECT
          'capacity-session-' || slot,
          1,
          'capacity-idempotency-' || slot,
          'capacity-owner-' || slot,
          'capacity-device-' || slot,
          'capacity-thread-' || slot,
          'omega_editor',
          'capacity-admission-' || slot,
          repeat('a', 64),
          'private',
          lpad(slot::text, 64, '0'),
          'pending',
          '2026-07-28T14:00:00.000Z',
          '2026-07-28T14:00:00.000Z'
        FROM generate_series(1, 20) AS slot
      `;
      await sql`
        INSERT INTO sarah_voice_admissions (
          admission_ref, owner_user_id, device_ref, thread_ref, session_ref,
          generation, disclosure_ref, client_profile, admission_cohort_ref,
          credit_mode, credit_rate_msat_per_million_tokens, terms_digest,
          spendable_remaining_credit_msat, state,
          issued_at, expires_at, consumed_at
        ) VALUES (
          'capacity-admission-21', 'capacity-owner-21',
          'capacity-device-21', 'capacity-thread-21',
          'capacity-session-21', 1, 'capacity-disclosure-21',
          'omega_editor', 'sarah_voice_cohort:alpha_v1', 'metered',
          100000, ${"e".repeat(64)}, 1000, 'consumed',
          '2026-07-28T14:00:00.000Z', '2026-07-28T14:02:00.000Z',
          '2026-07-28T14:00:00.000Z'
        )
      `;
      await expect(
        store.prepareLiveKitProvisioningIntent({
          sessionRef: "capacity-session-21",
          ownerUserId: "capacity-owner-21",
          deviceRef: "capacity-device-21",
          threadRef: "capacity-thread-21",
          generation: 1,
          capabilityProfile: "omega_editor",
          admissionRef: "capacity-admission-21",
          admissionDigest: "e".repeat(64),
          idempotencyKey: "capacity-idempotency-21",
          workerControlTokenDigest: "f".repeat(64),
          roomContext: { kind: "private" },
          nowIso: "2026-07-28T14:00:01.000Z",
        }),
      ).rejects.toBeInstanceOf(SarahVoiceLiveKitCapacityError);
    } finally {
      await sql`
        DELETE FROM sarah_voice_admissions
        WHERE admission_ref LIKE 'capacity-admission-%'
      `;
      await sql`
        DELETE FROM sarah_realtime_voice_sessions
        WHERE session_ref LIKE 'capacity-session-%'
      `;
    }
  });
});
