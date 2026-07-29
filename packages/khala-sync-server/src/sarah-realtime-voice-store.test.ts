import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  SarahVoiceConcurrentSessionError,
  SarahVoiceInsufficientCreditError,
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
          ),
          (
            'user-sarah-voice-new', 'human', 'New Voice Tester', 'active',
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
    expect(Number(balance?.balance_msat)).toBe(1_000_000_750);
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
    expect(Number(afterReplay?.balance_msat)).toBe(1_000_000_750);
    expect(Number(afterReplay?.held_msat)).toBe(0);
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
        reservedMsat: 1,
        ticketExpiresAt: "2026-07-28T12:05:00.000Z",
        sessionExpiresAt: "2026-07-28T12:09:00.000Z",
        nowIso: "2026-07-28T12:04:00.000Z",
      }),
    ).rejects.toBeInstanceOf(SarahVoiceInsufficientCreditError);

  test("provisions credit when an active user has no balance", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await store.reserve({
      sessionRef: "voice-session-new-user",
      reservationRef: "voice-reservation-new-user",
      ownerUserId: "user-sarah-voice-new",
      ownerActorRef: "agent:user-sarah-voice-new",
      deviceRef: "omega-test",
      threadRef: "thread-new-user",
      generation: 1,
      ticketDigest: "c".repeat(64),
      disclosureRef: "disclosure-test",
      clientProfile: "mobile_voice_only",
      reservedMsat: 1_000,
      ticketExpiresAt: "2026-07-28T12:01:00.000Z",
      sessionExpiresAt: "2026-07-28T12:10:00.000Z",
      nowIso: "2026-07-28T12:00:00.000Z",
    });

    const [balance] = await sql`
      SELECT balance_msat, held_msat, usd_credit_msat
      FROM agent_balances
      WHERE actor_ref = 'agent:user-sarah-voice-new'
    `;
    expect(Number(balance?.balance_msat)).toBe(1_000_000_000);
    expect(Number(balance?.held_msat)).toBe(1_000);
    expect(Number(balance?.usd_credit_msat)).toBe(1_000_000_000);
  });
});

});
