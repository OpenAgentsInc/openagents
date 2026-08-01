import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { makeSarahRealtimeVoiceStore } from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

describe.skipIf(!hasLocalPostgres())("Sarah voice owner-waived unmetered accounting", () => {
  let pg: LocalPostgres;
  let sql: SQL;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_sarah_unmetered");
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_sarah_unmetered") });
    sql = SQL({ url: pg.urlFor("khala_sync_sarah_unmetered"), max: 3 });
  }, 120_000);

  afterAll(async () => {
    await sql?.end();
    await pg?.stop();
  });

  test("reserves zero without a balance and records tokens with zero ledger charge", async () => {
    const nowIso = "2026-08-01T12:00:00.000Z";
    await sql`
      INSERT INTO users (id, kind, display_name, status, created_at, updated_at)
      VALUES ('user-unmetered', 'human', 'Unmetered', 'active', ${nowIso}, ${nowIso})
    `;
    await sql`
      INSERT INTO sarah_voice_alpha_memberships (
        membership_ref, owner_user_id, cohort_ref, state, admitted_at,
        admission_actor_ref, admission_reason, updated_at
      ) VALUES (
        'membership-unmetered', 'user-unmetered',
        'sarah_voice_cohort:alpha_v1', 'active', ${nowIso},
        'operator:test', 'test', ${nowIso}
      )
    `;
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    await expect(
      store.reserve({
        sessionRef: "voice-unmetered",
        reservationRef: "reservation-unmetered",
        ownerUserId: "user-unmetered",
        ownerActorRef: "agent:user-unmetered",
        deviceRef: "device-unmetered",
        threadRef: "thread-unmetered",
        generation: 1,
        ticketDigest: "a".repeat(64),
        disclosureRef: "disclosure-unmetered",
        clientProfile: "omega_editor",
        transportKind: "custom_wss_v1",
        creditMode: "owner_waived_unmetered",
        entitlementRef: null,
        admissionCohortRef: "sarah_voice_cohort:alpha_v1",
        creditRateMsatPerMillionTokens: 64_000_000,
        reservedMsat: 0,
        ticketExpiresAt: "2026-08-01T12:01:00.000Z",
        sessionExpiresAt: "2026-08-01T12:05:00.000Z",
        nowIso,
      }),
    ).resolves.toMatchObject({
      creditMode: "owner_waived_unmetered",
      reservedMsat: 0,
      chargedMsat: 0,
    });
    await store.connect({
      sessionRef: "voice-unmetered",
      ticketDigest: "a".repeat(64),
      nowIso: "2026-08-01T12:00:10.000Z",
    });
    await expect(
      store.recordUsage({
        sessionRef: "voice-unmetered",
        generation: 1,
        usage: {
          providerResponseRef: "response:unmetered",
          providerStatus: "completed",
          inputTokens: 100,
          outputTokens: 50,
          cachedInputTokens: 10,
          audioInputTokens: 80,
          audioOutputTokens: 40,
          observedAt: "2026-08-01T12:00:20.000Z",
        },
      }),
    ).resolves.toEqual({ chargedMsat: 0, reservedMsat: 0, creditLimitReached: false });
    await expect(
      store.settle({
        sessionRef: "voice-unmetered",
        closeReason: "completed",
        nowIso: "2026-08-01T12:00:30.000Z",
      }),
    ).resolves.toMatchObject({ state: "released", chargedMsat: 0, reservedMsat: 0 });
    const [usage] = await sql`
      SELECT input_tokens, output_tokens, charge_msat
      FROM sarah_realtime_voice_usage
      WHERE session_ref = 'voice-unmetered'
    `;
    expect(usage).toMatchObject({ input_tokens: "100", output_tokens: "50", charge_msat: "0" });
    const [balance] = await sql`
      SELECT actor_ref FROM agent_balances WHERE actor_ref = 'agent:user-unmetered'
    `;
    expect(balance).toBeUndefined();
  }, 120_000);

  test("waives an uncertain hold idempotently without changing provider evidence", async () => {
    const connectedAt = "2026-08-01T13:00:00.000Z";
    const uncertainAt = "2026-08-01T13:01:00.000Z";
    await sql`
      INSERT INTO users (id, kind, display_name, status, created_at, updated_at)
      VALUES ('user-waiver', 'human', 'Waiver', 'active', ${connectedAt}, ${connectedAt})
    `;
    await sql`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, usd_credit_msat, created_at, updated_at
      ) VALUES ('agent:user-waiver', 10000, 1000, 0, ${connectedAt}, ${connectedAt})
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
        'voice-waiver', 'reservation-waiver', 'user-waiver', 'agent:user-waiver',
        'device-waiver', 'thread-waiver', 1, 'disclosure-waiver',
        'accounting_uncertain', 1000, 15, ${uncertainAt},
        '2026-08-01T13:05:00.000Z', ${connectedAt}, ${uncertainAt}, ${connectedAt},
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
        worker_job_ref, worker_last_seen_at, provider_session_ref_digest,
        provider_configuration_digest, provider_admitted_at,
        provider_accounting_status, provider_accounting_uncertain_at,
        provider_accounting_uncertain_reason
      ) VALUES (
        'voice-waiver', 'user-waiver', 'device-waiver', 'thread-waiver', 1,
        'omega_editor', 'admission-waiver', ${"b".repeat(64)}, 'private',
        'room-waiver', 1, 'owner-waiver', 'principal.sarah', ${"c".repeat(64)},
        '2026-08-01T13:05:00.000Z', 'dispatch-waiver', 'presence-waiver',
        false, true, 'active', ${connectedAt}, ${uncertainAt}, 'job-waiver',
        ${connectedAt}, ${"d".repeat(64)}, ${"e".repeat(64)}, ${connectedAt},
        'uncertain', ${uncertainAt}, 'worker_disappeared'
      )
    `;
    await sql`
      INSERT INTO sarah_realtime_voice_usage (
        session_ref, provider_response_ref, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        charge_msat, observed_at, usage_kind, provider_status
      ) VALUES (
        'voice-waiver', 'response:waiver', 100, 50, 10, 80, 40, 15,
        ${connectedAt}, 'response', 'completed'
      )
    `;
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const input = {
      waiverRef: "waiver:voice-waiver",
      waiverPayloadDigest: "f".repeat(64),
      sessionRef: "voice-waiver",
      generation: 1,
      providerSessionRefDigest: "d".repeat(64),
      operatorActorRef: "operator:owner_sarah_unmetered_waiver",
      reason: "Owner waived platform credit accounting; provider usage remains uncertain",
      providerEvidenceRefs: ["issue:9285", "worker-log:voice-waiver"],
      nowIso: "2026-08-01T14:00:00.000Z",
    } as const;
    await expect(store.waiveLiveKitAccounting(input)).resolves.toMatchObject({
      releasedHoldMsat: 1000,
      recordedChargeWaivedMsat: 15,
      providerAccountingStatus: "uncertain",
      authority: "owner_waived_unmetered_v1",
      replayed: false,
    });
    await expect(
      store.waiveLiveKitAccounting({ ...input, nowIso: "2026-08-01T15:00:00.000Z" }),
    ).resolves.toMatchObject({ replayed: true, releasedHoldMsat: 1000 });

    const [balance] = await sql`
      SELECT balance_msat, held_msat FROM agent_balances
      WHERE actor_ref = 'agent:user-waiver'
    `;
    expect(balance).toMatchObject({ balance_msat: "10000", held_msat: "0" });
    const [session] = await sql`
      SELECT state, credit_mode, reserved_msat, charged_msat
      FROM sarah_realtime_voice_sessions WHERE session_ref = 'voice-waiver'
    `;
    expect(session).toMatchObject({
      state: "released",
      credit_mode: "owner_waived_unmetered",
      reserved_msat: "0",
      charged_msat: "0",
    });
    const [usage] = await sql`
      SELECT input_tokens, output_tokens, charge_msat, observed_at
      FROM sarah_realtime_voice_usage WHERE session_ref = 'voice-waiver'
    `;
    expect(usage).toMatchObject({
      input_tokens: "100",
      output_tokens: "50",
      charge_msat: "15",
      observed_at: connectedAt,
    });
    const [binding] = await sql`
      SELECT provider_accounting_status, provider_accounting_terminal_at,
        provider_accounting_uncertain_at, provider_accounting_uncertain_reason
      FROM sarah_livekit_room_bindings WHERE session_ref = 'voice-waiver'
    `;
    expect(binding).toMatchObject({
      provider_accounting_status: "uncertain",
      provider_accounting_terminal_at: null,
      provider_accounting_uncertain_at: uncertainAt,
      provider_accounting_uncertain_reason: "worker_disappeared",
    });
    const [waiver] = await sql`
      SELECT prior_reserved_msat, prior_recorded_charge_msat,
        provider_accounting_status, authority
      FROM sarah_voice_accounting_waivers WHERE session_ref = 'voice-waiver'
    `;
    expect(waiver).toMatchObject({
      prior_reserved_msat: "1000",
      prior_recorded_charge_msat: "15",
      provider_accounting_status: "uncertain",
      authority: "owner_waived_unmetered_v1",
    });
  }, 120_000);
});
