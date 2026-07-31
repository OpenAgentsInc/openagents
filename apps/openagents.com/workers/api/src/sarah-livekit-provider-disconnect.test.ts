import {
  SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PROTOCOL_VERSION,
  type SarahLiveKitProviderDisconnectAcceptanceRequest,
} from "@openagentsinc/audio-contract";
import {
  SarahVoiceSessionRejectedError,
  type SarahRealtimeVoiceStore,
} from "@openagentsinc/khala-sync-server";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE,
  SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE_HEADER,
  handleSarahLiveKitProviderDisconnectAcceptance,
} from "./sarah-livekit-provider-disconnect";

const body = {
  schema: SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PROTOCOL_VERSION,
  requestRef: "acceptance:provider-disconnect:one",
  sessionRef: "session:one",
  generation: 3,
  providerSessionRefDigest: "a".repeat(64),
  acknowledgement: "disconnect_exact_provider_socket",
} as const satisfies SarahLiveKitProviderDisconnectAcceptanceRequest;

const requestLiveKitProviderDisconnectFault = vi.fn(async () => ({
  requestRef: body.requestRef,
  sessionRef: body.sessionRef,
  generation: body.generation,
  providerSessionRefDigest: body.providerSessionRefDigest,
  state: "requested" as const,
  replayed: false,
}));
const close = vi.fn(async () => undefined);
const openStore = vi.fn(async () => ({
  store: {
    requestLiveKitProviderDisconnectFault,
  } as unknown as SarahRealtimeVoiceStore,
  close,
}));
const requireOperator = vi.fn(async () => ({
  actorRef: "operator.sarah_livekit_acceptance",
}));
const dependencies = {
  enabled: () => true,
  now: () => 2_000_000_000_000,
  openStore,
  requireOperator,
};
const request = (
  requestBody: unknown = body,
  ownerGate = SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE,
) =>
  new Request("https://openagents.com/api/operator/sarah/livekit/provider-disconnect", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      [SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE_HEADER]: ownerGate,
    },
    body: JSON.stringify(requestBody),
  });

describe("Sarah LiveKit provider-disconnect acceptance", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test("is undiscoverable when the runtime acceptance gate is disabled", async () => {
    const response = await handleSarahLiveKitProviderDisconnectAcceptance(
      { ...dependencies, enabled: () => false },
      request(),
      {},
      {} as ExecutionContext,
    );

    expect(response.status).toBe(404);
    expect(requireOperator).not.toHaveBeenCalled();
    expect(openStore).not.toHaveBeenCalled();
  });

  test("requires the exact operator and owner acceptance controls", async () => {
    const forbidden = await handleSarahLiveKitProviderDisconnectAcceptance(
      {
        ...dependencies,
        requireOperator: async () => undefined,
      },
      request(),
      {},
      {} as ExecutionContext,
    );
    expect(forbidden.status).toBe(403);
    expect(openStore).not.toHaveBeenCalled();

    const missingOwnerGate = await handleSarahLiveKitProviderDisconnectAcceptance(
      dependencies,
      request(body, "disconnect_provider"),
      {},
      {} as ExecutionContext,
    );
    expect(missingOwnerGate.status).toBe(428);
    expect(openStore).not.toHaveBeenCalled();

    const invalidAcknowledgement = await handleSarahLiveKitProviderDisconnectAcceptance(
      dependencies,
      request({
        ...body,
        acknowledgement: "disconnect_every_provider_socket",
      }),
      {},
      {} as ExecutionContext,
    );
    expect(invalidAcknowledgement.status).toBe(400);
    expect(openStore).not.toHaveBeenCalled();
  });

  test("persists only the exact generation and provider socket target", async () => {
    const response = await handleSarahLiveKitProviderDisconnectAcceptance(
      dependencies,
      request(),
      {},
      {} as ExecutionContext,
    );

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(requestLiveKitProviderDisconnectFault).toHaveBeenCalledWith({
      requestRef: body.requestRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      providerSessionRefDigest: body.providerSessionRefDigest,
      operatorActorRef: "operator.sarah_livekit_acceptance",
      nowIso: "2033-05-18T03:33:20.000Z",
    });
    expect(await response.json()).toEqual({
      schema: SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PROTOCOL_VERSION,
      requestRef: body.requestRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      providerSessionRefDigest: body.providerSessionRefDigest,
      state: "requested",
      replayed: false,
      sharedInfrastructureMutated: false,
    });
    expect(close).toHaveBeenCalledTimes(1);
  });

  test("reports a rejected target as a conflict and still closes storage", async () => {
    requestLiveKitProviderDisconnectFault.mockRejectedValueOnce(
      new SarahVoiceSessionRejectedError("provider mismatch"),
    );

    const response = await handleSarahLiveKitProviderDisconnectAcceptance(
      dependencies,
      request(),
      {},
      {} as ExecutionContext,
    );

    expect(response.status).toBe(409);
    expect(await response.json()).toEqual({
      error: "sarah_provider_disconnect_target_conflict",
    });
    expect(close).toHaveBeenCalledTimes(1);
  });
});
