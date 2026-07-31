import {
  SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PATH,
  SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PROTOCOL_VERSION,
  decodeSarahLiveKitProviderDisconnectAcceptanceRequest,
} from "@openagentsinc/audio-contract";
import {
  SarahVoiceSessionRejectedError,
  type SarahRealtimeVoiceStore,
} from "@openagentsinc/khala-sync-server";

export { SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PATH };

export const SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE_HEADER =
  "x-openagents-livekit-owner-gate";
export const SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE =
  "I_ACCEPT_EXACT_SARAH_PROVIDER_DISCONNECT";

export type SarahLiveKitProviderDisconnectDependencies<Bindings> = Readonly<{
  enabled: (env: Bindings) => boolean;
  now?: (() => number) | undefined;
  openStore: (
    env: Bindings,
  ) => Promise<Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>>;
  requireOperator: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ actorRef: string }> | undefined>;
}>;

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json; charset=utf-8",
    },
  });

const parseBody = async (request: Request) => {
  const contentLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(contentLength) || contentLength > 4096) return undefined;
  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > 4096) return undefined;
    return decodeSarahLiveKitProviderDisconnectAcceptanceRequest(JSON.parse(text) as unknown);
  } catch {
    return undefined;
  }
};

export const handleSarahLiveKitProviderDisconnectAcceptance = async <Bindings>(
  dependencies: SarahLiveKitProviderDisconnectDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (!dependencies.enabled(env)) {
    return noStoreJson({ error: "not_found" }, 404);
  }
  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const operator = await dependencies.requireOperator(request, env, ctx);
  if (operator === undefined) return noStoreJson({ error: "forbidden" }, 403);
  if (
    request.headers.get(SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE_HEADER) !==
    SARAH_LIVEKIT_PROVIDER_DISCONNECT_OWNER_GATE
  ) {
    return noStoreJson({ error: "owner_gate_required" }, 428);
  }
  const body = await parseBody(request);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_sarah_provider_disconnect_acceptance_request" }, 400);
  }

  let opened: Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }> | undefined;
  try {
    opened = await dependencies.openStore(env);
    const result = await opened.store.requestLiveKitProviderDisconnectFault({
      requestRef: body.requestRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      providerSessionRefDigest: body.providerSessionRefDigest,
      operatorActorRef: operator.actorRef,
      nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
    });
    return noStoreJson(
      {
        schema: SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_PROTOCOL_VERSION,
        requestRef: result.requestRef,
        sessionRef: result.sessionRef,
        generation: result.generation,
        providerSessionRefDigest: result.providerSessionRefDigest,
        state: result.state,
        replayed: result.replayed,
        sharedInfrastructureMutated: false,
      },
      200,
    );
  } catch (error) {
    return error instanceof SarahVoiceSessionRejectedError
      ? noStoreJson({ error: "sarah_provider_disconnect_target_conflict" }, 409)
      : noStoreJson({ error: "sarah_provider_disconnect_control_unavailable" }, 503);
  } finally {
    try {
      await opened?.close();
    } catch {
      // The committed directive is unaffected by connection release failure.
    }
  }
};
