import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  decodeSarahLiveKitJobClaimResponse,
} from "@openagentsinc/audio-contract";
import {
  makeSarahRealtimeVoiceStore,
  type SarahRealtimeVoiceStore,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";
import { runMigrations } from "@openagentsinc/khala-sync-server/migrate";
import {
  hasLocalPostgres,
  startLocalPostgres,
  type LocalPostgres,
} from "@openagentsinc/khala-sync-server/test/local-postgres";
import { createHash } from "node:crypto";
import postgres, { type Sql } from "postgres";
import { afterAll, beforeAll, describe, expect, test, vi } from "vitest";

import {
  handleSarahLiveKitWorkerClaim,
  handleSarahLiveKitWorkerEvent,
  handleSarahLiveKitWorkerToolProposal,
  handleSarahLiveKitWorkerToolState,
} from "./sarah-livekit-worker-routes";
import { deriveSarahLiveKitControlToken } from "./sarah-livekit-room-broker";

const controlRoot = "R".repeat(64);
const claimDispatch = {
  sessionRef: "voice-livekit-route-1",
  generation: 1,
  roomRef: "room-livekit-route-1",
  roomEpoch: 1,
  participantRef: "participant-owner-livekit-route-1",
  sarahParticipantRef: "principal.sarah",
  sarahPresenceLeaseRef: "presence-livekit-route-1",
  capabilityProfile: "omega_editor",
  roomContext: { kind: "private" },
} as const;
const token = deriveSarahLiveKitControlToken(controlRoot, {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
  ...claimDispatch,
});
const tokenDigest = createHash("sha256").update(token).digest("hex");

const authorizedRequest = (path: string, body: unknown) =>
  new Request(`https://openagents.com${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });

describe.skipIf(!hasLocalPostgres())("Sarah LiveKit production worker route lifecycle", () => {
  let pg: LocalPostgres;
  let sql: Sql;
  let store: SarahRealtimeVoiceStore;
  let nowMs = Date.parse("2026-07-28T13:00:10.000Z");
  const cleanup = vi.fn(async () => undefined);

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = postgres(pg.url, { max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_sarah_livekit_worker");
    await admin.end();
    const databaseUrl = pg.urlFor("khala_sync_sarah_livekit_worker");
    await runMigrations({ databaseUrl });
    sql = postgres(databaseUrl, { max: 5 });
    store = makeSarahRealtimeVoiceStore(sql as unknown as SyncSql);

    await sql`
        INSERT INTO users (
          id, kind, display_name, status, created_at, updated_at
        ) VALUES (
          'user-livekit-route', 'human', 'LiveKit Route Tester', 'active',
          '2026-07-28T13:00:00.000Z', '2026-07-28T13:00:00.000Z'
        )
        `;
    await sql`
        INSERT INTO agent_balances (
          actor_ref, balance_msat, held_msat, usd_credit_msat,
          created_at, updated_at
        ) VALUES (
          'agent:user-livekit-route', 10000, 0, 0,
          '2026-07-28T13:00:00.000Z', '2026-07-28T13:00:00.000Z'
        )
        `;
    await sql`
        INSERT INTO sarah_voice_alpha_memberships (
          membership_ref, cohort_ref, owner_user_id, state, admitted_at,
          admission_actor_ref, admission_reason, updated_at
        ) VALUES (
          'sarah_voice_alpha:user-livekit-route',
          'sarah_voice_cohort:alpha_v1', 'user-livekit-route', 'active',
          '2026-07-28T13:00:00.000Z', 'operator:test', 'Test admission',
          '2026-07-28T13:00:00.000Z'
        )
        `;
    await store.issueAdmission({
      admissionRef: "sarah_voice_admission:route-1",
      ownerUserId: "user-livekit-route",
      deviceRef: "omega-livekit-route",
      threadRef: "thread-livekit-route",
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      disclosureRef: "disclosure-livekit-route",
      clientProfile: "omega_editor",
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditMode: "metered",
      termsDigest: "8".repeat(64),
      spendableRemainingCreditMsat: 10_000,
      nowIso: "2026-07-28T13:00:00.000Z",
      expiresAt: "2026-07-28T13:02:00.000Z",
    });
    await store.reserve({
      sessionRef: "voice-livekit-route-1",
      reservationRef: "voice-livekit-route-reservation-1",
      ownerUserId: "user-livekit-route",
      ownerActorRef: "agent:user-livekit-route",
      deviceRef: "omega-livekit-route",
      threadRef: "thread-livekit-route",
      generation: 1,
      ticketDigest: "4".repeat(64),
      disclosureRef: "disclosure-livekit-route",
      clientProfile: "omega_editor",
      transportKind: "livekit_room_v1",
      creditMode: "metered",
      entitlementRef: null,
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      reservedMsat: 1_000,
      ticketExpiresAt: "2026-07-28T13:01:00.000Z",
      sessionExpiresAt: "2026-07-28T13:02:00.000Z",
      nowIso: "2026-07-28T13:00:00.000Z",
      admissionBinding: {
        admissionRef: "sarah_voice_admission:route-1",
        termsDigest: "8".repeat(64),
        spendableRemainingCreditMsat: 10_000,
      },
    });
    await store.prepareLiveKitProvisioningIntent({
      sessionRef: "voice-livekit-route-1",
      ownerUserId: "user-livekit-route",
      deviceRef: "omega-livekit-route",
      threadRef: "thread-livekit-route",
      generation: 1,
      capabilityProfile: "omega_editor",
      admissionRef: "sarah_voice_admission:route-1",
      admissionDigest: "8".repeat(64),
      idempotencyKey: "sarah-livekit:voice-livekit-route-1:1",
      workerControlTokenDigest: tokenDigest,
      roomContext: { kind: "private" },
      nowIso: "2026-07-28T13:00:00.000Z",
    });
    await store.bindLiveKitRoom({
      sessionRef: "voice-livekit-route-1",
      ownerUserId: "user-livekit-route",
      deviceRef: "omega-livekit-route",
      threadRef: "thread-livekit-route",
      generation: 1,
      capabilityProfile: "omega_editor",
      admissionRef: "sarah_voice_admission:route-1",
      admissionDigest: "8".repeat(64),
      roomContext: { kind: "private" },
      roomRef: "room-livekit-route-1",
      roomEpoch: 1,
      participantRef: "participant-owner-livekit-route-1",
      sarahParticipantRef: "principal.sarah",
      participantGrantDigest: "5".repeat(64),
      joinExpiresAt: "2026-07-28T13:01:00.000Z",
      dispatchRef: "dispatch-livekit-route-1",
      sarahPresenceLeaseRef: "presence-livekit-route-1",
      workerControlTokenDigest: tokenDigest,
      publishAllowed: true,
      subscribeAllowed: true,
      nowIso: "2026-07-28T13:00:00.000Z",
    });
  }, 30_000);

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("connects, meters, leases, and closes idempotently without the legacy ticket path", async () => {
    const dependencies = {
      controlRoot: () => controlRoot,
      creditMsatPerMillionTokens: () => 1_000_000,
      now: () => nowMs,
      openStore: async () => ({
        store,
        close: async () => undefined,
      }),
      cleanup,
    };
    const claim = await handleSarahLiveKitWorkerClaim(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/claim", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        workerRef: "worker:route-one",
        jobRef: "job:route-one",
        dispatchRef: "dispatch-livekit-route-1",
        roomSid: "RM_livekit_route_1",
        dispatch: claimDispatch,
      }),
      {},
    );
    expect(claim.status).toBe(200);
    expect(decodeSarahLiveKitJobClaimResponse(await claim.json())).toMatchObject({
      sessionRef: "voice-livekit-route-1",
      generation: 1,
    });

    const connectedEvent = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      _tag: "worker_connected",
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      jobRef: "job:route-one",
      eventRef: "connected:job:route-one",
      roomSid: "RM_livekit_route_1",
    } as const;
    nowMs = Date.parse("2026-07-28T13:00:22.000Z");
    const connected = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", connectedEvent),
      {},
    );
    expect(connected.status).toBe(200);
    nowMs = Date.parse("2026-07-28T13:00:30.000Z");
    const connectedReplay = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", connectedEvent),
      {},
    );
    expect(connectedReplay.status).toBe(200);

    const [activeSession] = await sql`
        SELECT state, ticket_digest, connected_at
        FROM sarah_realtime_voice_sessions
        WHERE session_ref = 'voice-livekit-route-1'
      `;
    expect(activeSession).toMatchObject({
      state: "connected",
      ticket_digest: "4".repeat(64),
      connected_at: "2026-07-28T13:00:22.000Z",
    });
    await expect(
      store.connect({
        sessionRef: "voice-livekit-route-1",
        ticketDigest: "4".repeat(64),
        nowIso: "2026-07-28T13:00:30.500Z",
      }),
    ).resolves.toMatchObject({ state: "connected" });
    await expect(
      store.connect({
        sessionRef: "voice-livekit-route-1",
        ticketDigest: "4".repeat(64),
        nowIso: "2026-07-28T13:00:30.600Z",
      }),
    ).rejects.toThrow("invalid or expired");
    await store.sweepExpired("2026-07-28T13:01:30.000Z");

    const leaseEvent = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      _tag: "lease_check",
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      jobRef: "job:route-one",
      eventRef: "lease:job:route-one:1",
    } as const;
    nowMs = Date.parse("2026-07-28T13:01:31.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", leaseEvent),
          {},
        )
      ).status,
    ).toBe(200);
    nowMs = Date.parse("2026-07-28T13:01:32.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", leaseEvent),
          {},
        )
      ).status,
    ).toBe(200);

    const usageEvent = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      _tag: "response_usage",
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      jobRef: "job:route-one",
      eventRef: "response:resp_route_one",
      providerResponseRef: "resp_route_one",
      status: "completed",
      inputTokens: 100,
      outputTokens: 50,
      cachedInputTokens: 10,
      audioInputTokens: 80,
      audioOutputTokens: 40,
    } as const;
    nowMs = Date.parse("2026-07-28T13:01:40.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", usageEvent),
          {},
        )
      ).status,
    ).toBe(200);
    nowMs = Date.parse("2026-07-28T13:01:45.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", usageEvent),
          {},
        )
      ).status,
    ).toBe(200);
    const changedUsage = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        ...usageEvent,
        outputTokens: 51,
      }),
      {},
    );
    expect(changedUsage.status).toBe(409);

    const [usage] = await sql`
        SELECT input_tokens, output_tokens, charge_msat, observed_at
        FROM sarah_realtime_voice_usage
        WHERE session_ref = 'voice-livekit-route-1'
      `;
    expect({
      inputTokens: Number(usage?.input_tokens),
      outputTokens: Number(usage?.output_tokens),
      chargeMsat: Number(usage?.charge_msat),
      observedAt: usage?.observed_at,
    }).toEqual({
      inputTokens: 100,
      outputTokens: 50,
      chargeMsat: 150,
      observedAt: "2026-07-28T13:01:40.000Z",
    });

    nowMs = Date.parse("2026-07-28T13:01:46.000Z");
    const toolProposalRequest = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      jobRef: "job:route-one",
      eventRef: "tool:event:route-one",
      providerCallRef: "call:route-one",
      command: {
        _tag: "start_agent_thread",
        message: "Inspect the current test failure.",
        presentation: "foreground",
      },
    } as const;
    const toolProposalResponse = await handleSarahLiveKitWorkerToolProposal(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/tool/proposal", toolProposalRequest),
      {},
    );
    expect(toolProposalResponse.status).toBe(200);
    const toolProposal = (await toolProposalResponse.json()) as {
      proposal: { proposalRef: string; proposalDigest: string };
    };

    const finalUsageEvent = {
      ...usageEvent,
      eventRef: "response:resp_route_final",
      providerResponseRef: "resp_route_final",
      inputTokens: 20,
      outputTokens: 5,
      cachedInputTokens: 0,
      audioInputTokens: 20,
      audioOutputTokens: 5,
    } as const;
    nowMs = Date.parse("2026-07-28T13:02:00.000Z");
    const [swept, finalUsageResponse] = await Promise.all([
      store.sweepExpired("2026-07-28T13:02:00.000Z"),
      handleSarahLiveKitWorkerEvent(
        dependencies,
        authorizedRequest("/api/internal/sarah/livekit/job/event", finalUsageEvent),
        {},
      ),
    ]);
    expect(swept).toBe(1);
    expect(finalUsageResponse.status).toBe(200);
    expect(await finalUsageResponse.json()).toEqual({
      accepted: true,
      stopReason: "session_expired",
    });
    nowMs = Date.parse("2026-07-28T13:02:01.000Z");
    const stoppedProposalState = await handleSarahLiveKitWorkerToolState(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/tool/state", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        sessionRef: "voice-livekit-route-1",
        generation: 1,
        jobRef: "job:route-one",
        proposalRef: toolProposal.proposal.proposalRef,
        proposalDigest: toolProposal.proposal.proposalDigest,
      }),
      {},
    );
    expect(stoppedProposalState.status).toBe(200);
    expect(await stoppedProposalState.json()).toMatchObject({
      state: "declined",
    });
    const proposalAfterStop = await handleSarahLiveKitWorkerToolProposal(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/tool/proposal", {
        ...toolProposalRequest,
        eventRef: "tool:event:route-two",
        providerCallRef: "call:route-two",
      }),
      {},
    );
    expect(proposalAfterStop.status).toBe(409);
    const [draining] = await sql`
      SELECT state, charged_msat
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = 'voice-livekit-route-1'
    `;
    expect(draining).toMatchObject({ state: "connected" });
    expect(Number(draining?.charged_msat)).toBe(175);
    expect(await store.sweepExpired("2026-07-28T13:02:14.999Z")).toBe(0);
    expect(await store.sweepExpired("2026-07-28T13:02:15.000Z")).toBe(1);

    const closeEvent = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      _tag: "close",
      sessionRef: "voice-livekit-route-1",
      generation: 1,
      jobRef: "job:route-one",
      eventRef: "close:job:route-one",
      reason: "completed",
    } as const;
    nowMs = Date.parse("2026-07-28T13:02:16.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", closeEvent),
          {},
        )
      ).status,
    ).toBe(200);
    nowMs = Date.parse("2026-07-28T13:02:20.000Z");
    expect(
      (
        await handleSarahLiveKitWorkerEvent(
          dependencies,
          authorizedRequest("/api/internal/sarah/livekit/job/event", closeEvent),
          {},
        )
      ).status,
    ).toBe(200);

    const [settled] = await sql`
        SELECT state, charged_msat, close_reason
        FROM sarah_realtime_voice_sessions
        WHERE session_ref = 'voice-livekit-route-1'
      `;
    expect({
      state: settled?.state,
      chargedMsat: Number(settled?.charged_msat),
      closeReason: settled?.close_reason,
    }).toEqual({
      state: "settled",
      chargedMsat: 175,
      closeReason: "session_expired",
    });
    const [eventCount] = await sql`
        SELECT COUNT(*) AS count
        FROM sarah_livekit_worker_events
        WHERE session_ref = 'voice-livekit-route-1'
      `;
    expect(Number(eventCount?.count)).toBe(5);
    const eventObservations = await sql`
        SELECT event_ref, observed_at
        FROM sarah_livekit_worker_events
        WHERE session_ref = 'voice-livekit-route-1'
        ORDER BY event_ref
      `;
    expect(eventObservations).toEqual([
      {
        event_ref: "close:job:route-one",
        observed_at: "2026-07-28T13:02:16.000Z",
      },
      {
        event_ref: "connected:job:route-one",
        observed_at: "2026-07-28T13:00:22.000Z",
      },
      {
        event_ref: "lease:job:route-one:1",
        observed_at: "2026-07-28T13:01:31.000Z",
      },
      {
        event_ref: "response:resp_route_final",
        observed_at: "2026-07-28T13:02:00.000Z",
      },
      {
        event_ref: "response:resp_route_one",
        observed_at: "2026-07-28T13:01:40.000Z",
      },
    ]);
    expect(cleanup).toHaveBeenCalledTimes(2);
  });
});
