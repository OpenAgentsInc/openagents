import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  SarahVoiceAdmissionRejectedError,
  SarahVoiceConcurrentSessionError,
  SarahVoiceSessionRejectedError,
  makeSarahRealtimeVoiceStore,
} from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

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
    await store.recordUsage({ sessionRef: "voice-session-1", usage });
    await store.recordUsage({ sessionRef: "voice-session-1", usage });
    const settled = await store.settle({
      sessionRef: "voice-session-1",
      closeReason: "user_stop",
      nowIso: "2026-07-28T12:02:00.000Z",
    });
    expect(settled).toMatchObject({
      state: "settled",
      clientProfile: "mobile_voice_only",
      chargedMsat: 250,
      reservedMsat: 1_000,
    });

    const [balance] = await sql`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = 'agent:user-sarah-voice'
      `;
    expect(Number(balance?.balance_msat)).toBe(9_750);
    expect(Number(balance?.held_msat)).toBe(0);
    const [receipt] = await sql`
        SELECT cost_msat, state
        FROM pay_ins
        WHERE idempotency_key = 'sarah:voice:settle:voice-session-1'
      `;
    expect(Number(receipt?.cost_msat)).toBe(250);
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
    expect(Number(afterReplay?.balance_msat)).toBe(9_750);
    expect(Number(afterReplay?.held_msat)).toBe(0);
    expect(
      await store.readSettlement({
        sessionRef: "voice-session-1",
        ownerUserId: "user-sarah-voice",
      }),
    ).toMatchObject({
      finalChargeMsat: 250,
      spendableRemainingCreditMsat: 9_750,
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
      chargedMsat: 500,
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
      chargedMsat: 500,
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
      creditMode: common.creditMode,
      termsDigest: "a".repeat(64),
      spendableRemainingCreditMsat: spendableMsat,
      nowIso: common.nowIso,
      expiresAt: "2026-07-28T12:02:00.000Z",
    });

    await expect(
      store.reserve({
        ...common,
        admissionBinding: {
          admissionRef: "sarah_voice_admission:bound-1",
          termsDigest: "b".repeat(64),
          spendableRemainingCreditMsat: spendableMsat,
        },
      }),
    ).rejects.toBeInstanceOf(SarahVoiceAdmissionRejectedError);

    const reserved = await store.reserve({
      ...common,
      admissionBinding: {
        admissionRef: "sarah_voice_admission:bound-1",
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
      generation: common.generation,
      disclosureRef: common.disclosureRef,
      clientProfile: common.clientProfile,
      admissionCohortRef: common.admissionCohortRef,
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
        ticketDigest: "3".repeat(64),
        nowIso: "2026-07-28T12:03:00.000Z",
        admissionBinding: {
          admissionRef: "sarah_voice_admission:balance-change",
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
        reservedMsat: 1,
        ticketExpiresAt: "2026-07-28T12:07:00.000Z",
        sessionExpiresAt: "2026-07-28T12:10:00.000Z",
        nowIso: "2026-07-28T12:06:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceSessionRejectedError);
  });
});
