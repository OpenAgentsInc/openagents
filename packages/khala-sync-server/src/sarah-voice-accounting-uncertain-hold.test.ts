import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS,
  makeSarahRealtimeVoiceStore,
} from "./sarah-realtime-voice-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

/**
 * `accounting_uncertain` is a hold, not a settlement.
 *
 * Migration 0115 widened `sarah_realtime_voice_owner_active_idx` to cover
 * `accounting_uncertain` while exact provider accounting is still inside its
 * bounded reconciliation window. Migration 0128 removes only that concurrency
 * lock after a durable escalation. The owner's hold remains preserved until
 * exact provider accounting is reconciled. `sweepExpired` therefore selects
 * only `reserved` and `connected`, and cannot terminate an uncertain row.
 *
 * That asymmetry looks like a bug, and it has already been misread as one. The
 * liveness symptom is real — an unreconciled hold occupied the owner's single
 * concurrency slot forever — but "make the sweep's selected set match the
 * index's blocking set" is not its fix. A sweep that terminated this state
 * would invent the exact charge the design refuses to invent, trading a
 * liveness bug for a money bug.
 *
 * This file exists so that trade cannot be made quietly. It was written after a
 * coordinating agent issued exactly that instruction and it was stopped only
 * because a second reviewer checked the claim instead of accepting it.
 *
 * The repair keeps the money invariant and separates it from voice liveness:
 * after a bounded window, maintenance records a durable escalation and removes
 * only the concurrency lock. The full hold and uncertain state remain until an
 * operator supplies complete provider evidence through exact reconciliation.
 */
describe.skipIf(!hasLocalPostgres())("Sarah voice accounting-uncertain holds", () => {
  let pg: LocalPostgres;
  let sql: SQL;

  const ownerUserId = "user-uncertain-hold";
  const sessionRef = "voice-uncertain-hold";
  const connectedAt = "2026-07-28T13:00:00.000Z";
  const uncertainAt = "2026-07-28T13:01:00.000Z";

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_uncertain_hold");
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_uncertain_hold") });
    sql = SQL({ url: pg.urlFor("khala_sync_uncertain_hold"), max: 3 });
    await sql`
      INSERT INTO users (id, kind, display_name, status, created_at, updated_at)
      VALUES (
        ${ownerUserId}, 'human', 'Uncertain Hold Tester', 'active',
        ${connectedAt}, ${connectedAt}
      )
    `;
    await sql`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, usd_credit_msat, created_at, updated_at
      ) VALUES (
        ${`agent:${ownerUserId}`}, 10000, 1000, 0, ${connectedAt}, ${connectedAt}
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
        ${sessionRef}, ${`${sessionRef}-reservation`}, ${ownerUserId},
        ${`agent:${ownerUserId}`}, 'omega-uncertain', 'thread-uncertain', 1,
        'disclosure-uncertain', 'accounting_uncertain',
        1000, 15, '2026-07-28T13:01:00.000Z', '2026-07-28T13:10:00.000Z',
        ${connectedAt}, ${uncertainAt}, ${connectedAt},
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
        provider_accounting_uncertain_at, provider_accounting_uncertain_reason
      ) VALUES (
        ${sessionRef}, ${ownerUserId}, 'omega-uncertain', 'thread-uncertain', 1,
        'omega_editor', 'admission-uncertain', ${"a".repeat(64)},
        'private', 'room-uncertain', 1, 'owner-uncertain',
        'principal.sarah', ${"b".repeat(64)}, '2026-07-28T13:05:00.000Z',
        'dispatch-uncertain', 'presence-uncertain', false, true, 'active',
        ${connectedAt}, ${uncertainAt},
        'worker-job-uncertain', '2026-07-28T13:00:30.000Z',
        ${"c".repeat(64)}, ${"d".repeat(64)},
        ${connectedAt}, 'uncertain',
        ${uncertainAt}, 'worker_disappeared'
      )
    `;
    await sql`
      INSERT INTO sarah_realtime_voice_usage (
        session_ref, provider_response_ref, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        charge_msat, observed_at, usage_kind, provider_status
      ) VALUES (
        ${sessionRef}, 'response:provider-response-uncertain',
        100, 50, 10, 80, 40, 15, ${connectedAt}, 'response', 'completed'
      )
    `;
  }, 120_000);

  afterAll(async () => {
    await sql?.end();
    await pg?.stop();
  });

  const readHold = async (): Promise<
    Readonly<{ state: string; charged_msat: number | string }> | undefined
  > => {
    const rows = (await sql`
      SELECT state, charged_msat
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${sessionRef}
    `) as ReadonlyArray<{ state: string; charged_msat: number | string }>;
    return rows[0];
  };

  test("the sweep never terminates an unreconciled hold, however long it waits", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);

    const before = await readHold();
    expect(before?.state).toBe("accounting_uncertain");
    const [balanceBefore] = (await sql`
      SELECT balance_msat, held_msat FROM agent_balances
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `) as ReadonlyArray<{ balance_msat: number | string; held_msat: number | string }>;

    // Far past every deadline the row carries: ticket, session, join, and the
    // worker heartbeat. If any sweep branch could reach this state, one of
    // these instants would trip it.
    for (const nowIso of [
      "2026-07-28T13:11:00.000Z",
      "2026-07-28T14:00:00.000Z",
      "2026-07-29T13:00:00.000Z",
      "2027-07-28T13:00:00.000Z",
    ]) {
      // eslint-disable-next-line no-await-in-loop
      expect(await store.sweepExpired(nowIso)).toBe(0);
      // eslint-disable-next-line no-await-in-loop
      const after = await readHold();
      expect(after?.state).toBe("accounting_uncertain");
    }

    const [balanceAfter] = (await sql`
      SELECT balance_msat, held_msat FROM agent_balances
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `) as ReadonlyArray<{ balance_msat: number | string; held_msat: number | string }>;

    // The hold is the point. A sweep that released or charged it would have
    // invented a settlement figure from partial usage.
    expect(String(balanceAfter?.balance_msat)).toBe(String(balanceBefore?.balance_msat));
    expect(String(balanceAfter?.held_msat)).toBe(String(balanceBefore?.held_msat));
    expect(String((await readHold())?.charged_msat)).toBe(String(before?.charged_msat));
  }, 120_000);

  test("reports the hold as stuck once it outlives the bound", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const uncertainMs = Date.parse(uncertainAt);

    // One millisecond before the bound elapses: not yet an operator's problem.
    await expect(
      store.readStuckAccountingUncertainHolds({
        nowIso: new Date(uncertainMs + SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS - 1).toISOString(),
      }),
    ).resolves.toMatchObject({ stuck: 0, owners: 0, oldestAgeMs: 0 });

    // Exactly at the bound: reported.
    await expect(
      store.readStuckAccountingUncertainHolds({
        nowIso: new Date(uncertainMs + SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS).toISOString(),
      }),
    ).resolves.toMatchObject({
      stuck: 1,
      owners: 1,
      oldestAgeMs: SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS,
    });

    // The age is measured, not assumed: a day later it reads a day.
    await expect(
      store.readStuckAccountingUncertainHolds({
        nowIso: new Date(uncertainMs + 86_400_000).toISOString(),
      }),
    ).resolves.toMatchObject({ stuck: 1, owners: 1, oldestAgeMs: 86_400_000 });
  }, 120_000);

  test("escalates at the bound without moving money and releases only the voice slot", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const uncertainMs = Date.parse(uncertainAt);
    const balanceBefore = (
      await sql`
      SELECT balance_msat, held_msat FROM agent_balances
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `
    )[0];

    await expect(
      store.escalateStuckAccountingUncertainHolds({
        nowIso: new Date(uncertainMs + SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS - 1).toISOString(),
      }),
    ).resolves.toEqual({ escalated: 0, owners: 0, oldestAgeMs: 0 });

    const escalatedAt = new Date(
      uncertainMs + SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS,
    ).toISOString();
    await expect(
      store.escalateStuckAccountingUncertainHolds({ nowIso: escalatedAt }),
    ).resolves.toEqual({
      escalated: 1,
      owners: 1,
      oldestAgeMs: SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS,
    });
    await expect(
      store.escalateStuckAccountingUncertainHolds({ nowIso: escalatedAt }),
    ).resolves.toEqual({ escalated: 0, owners: 0, oldestAgeMs: 0 });

    const [escalated] = await sql`
      SELECT state, charged_msat, accounting_escalated_at,
        accounting_escalation_ref
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${sessionRef}
    `;
    expect(escalated).toMatchObject({
      state: "accounting_uncertain",
      charged_msat: "15",
      accounting_escalated_at: escalatedAt,
      accounting_escalation_ref: expect.stringMatching(
        /^sarah_voice_accounting_escalation:[0-9a-f]{64}$/u,
      ),
    });
    const balanceAfter = (
      await sql`
      SELECT balance_msat, held_msat FROM agent_balances
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `
    )[0];
    expect(balanceAfter).toEqual(balanceBefore);

    await expect(store.readSettlement({ sessionRef, ownerUserId })).resolves.toMatchObject({
      state: "accounting_uncertain",
      holdPreserved: true,
      noHoldCreated: false,
      accountingEscalation: {
        escalationRef: escalated.accounting_escalation_ref,
        escalatedAt,
      },
    });

    // The old full hold still reduces spendable credit, but it is no longer a
    // voice-concurrency mutex. The normal credit reservation remains the bound.
    await expect(sql`
      INSERT INTO sarah_realtime_voice_sessions (
        session_ref, reservation_ref, owner_user_id, owner_actor_ref,
        device_ref, thread_ref, generation, disclosure_ref, state,
        reserved_msat, charged_msat, ticket_expires_at, session_expires_at,
        created_at, updated_at, client_profile, credit_mode, transport_kind,
        credit_rate_msat_per_million_tokens
      ) VALUES (
        'voice-after-accounting-escalation',
        'reservation-after-accounting-escalation', ${ownerUserId},
        ${`agent:${ownerUserId}`}, 'omega-after-escalation',
        'thread-after-escalation', 1, 'disclosure-after-escalation',
        'connected', 1000, 0, '2026-07-28T13:20:00.000Z',
        '2026-07-28T13:30:00.000Z', ${escalatedAt}, ${escalatedAt},
        'omega_editor', 'metered', 'livekit_room_v1', 100000
      )
    `).resolves.toBeDefined();
    await sql`
      UPDATE agent_balances
      SET held_msat = held_msat + 1000
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `;
  }, 120_000);

  /**
   * The second half of the same lock, and the more important one.
   *
   * Terminating this state takes two changes, not one: the sweep has to select
   * the row, and `settle` has to accept it. Proving the regression showed that
   * widening the sweep alone makes `settle` refuse and the sweep throw — which
   * is loud, and therefore survivable. The quiet failure is an agent who, having
   * widened the sweep, then removes the refusal below to stop the noise. At that
   * point the charge is computed from whatever partial usage was recorded before
   * the worker died, and nothing anywhere says so.
   *
   * So both halves are asserted here, together, where the reason is written
   * down.
   */
  test("settlement refuses an unreconciled hold outright", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);

    await expect(
      store.settle({
        sessionRef,
        closeReason: "must_not_settle_partial_usage",
        nowIso: "2027-07-28T13:00:00.000Z",
      }),
    ).rejects.toThrow(/uncertain/iu);

    const after = await readHold();
    expect(after?.state).toBe("accounting_uncertain");
    expect(String(after?.charged_msat)).toBe("15");
  }, 120_000);

  // Observing a stuck hold must not be a way to clear one.
  test("scanning for stuck holds changes nothing", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);
    const before = await readHold();

    await store.readStuckAccountingUncertainHolds({
      nowIso: "2027-07-28T13:00:00.000Z",
      stuckAfterMs: 0,
    });

    const after = await readHold();
    expect(after?.state).toBe("accounting_uncertain");
    expect(String(after?.charged_msat)).toBe(String(before?.charged_msat));
  }, 120_000);

  test("reconciles an escalated hold while the owner's newer voice session stays active", async () => {
    const store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);

    await expect(
      store.reconcileLiveKitAccounting({
        reconciliationRef: "reconciliation-after-escalation",
        reconciliationPayloadDigest: "e".repeat(64),
        sessionRef,
        generation: 1,
        providerSessionRefDigest: "c".repeat(64),
        operatorActorRef: "operator:accounting-test",
        reason: "Complete provider export matched the recorded response",
        providerEvidenceRefs: ["provider-export:uncertain-hold"],
        usage: [
          {
            usageKind: "response",
            providerResponseRef: "response:provider-response-uncertain",
            providerStatus: "completed",
            inputTokens: 100,
            outputTokens: 50,
            cachedInputTokens: 10,
            audioInputTokens: 80,
            audioOutputTokens: 40,
          },
        ],
        nowIso: "2026-07-29T14:00:00.000Z",
      }),
    ).resolves.toMatchObject({
      state: "settled",
      finalChargeMsat: 15,
      replayed: false,
    });

    const [oldSession] = await sql`
      SELECT state, charged_msat, accounting_escalation_ref
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${sessionRef}
    `;
    expect(oldSession).toMatchObject({
      state: "settled",
      charged_msat: "15",
      accounting_escalation_ref: expect.stringMatching(
        /^sarah_voice_accounting_escalation:[0-9a-f]{64}$/u,
      ),
    });
    const [newSession] = await sql`
      SELECT state, charged_msat
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = 'voice-after-accounting-escalation'
    `;
    expect(newSession).toMatchObject({ state: "connected", charged_msat: "0" });
    const [balance] = await sql`
      SELECT balance_msat, held_msat
      FROM agent_balances
      WHERE actor_ref = ${`agent:${ownerUserId}`}
    `;
    expect(balance).toMatchObject({ balance_msat: "9985", held_msat: "1000" });
  }, 120_000);
});
