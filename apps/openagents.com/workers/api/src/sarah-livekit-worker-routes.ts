import {
  SARAH_LIVEKIT_JOB_CLAIM_PATH,
  SARAH_LIVEKIT_JOB_EVENT_PATH,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  decodeSarahLiveKitJobClaimRequest,
  decodeSarahLiveKitJobEvent,
} from "@openagentsinc/audio-contract";
import type { SarahRealtimeVoiceStore } from "@openagentsinc/khala-sync-server";
import { createHash } from "node:crypto";

export const SARAH_LIVEKIT_WORKER_CLAIM_PATH = SARAH_LIVEKIT_JOB_CLAIM_PATH;
export const SARAH_LIVEKIT_WORKER_EVENT_PATH = SARAH_LIVEKIT_JOB_EVENT_PATH;

export type SarahLiveKitWorkerRouteDependencies<Bindings> = Readonly<{
  creditMsatPerMillionTokens: (env: Bindings) => number | undefined;
  now?: (() => number) | undefined;
  openStore: (
    env: Bindings,
  ) => Promise<Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>>;
  cleanup: (
    env: Bindings,
    input: Readonly<{ sessionRef: string; generation: number }>,
  ) => Promise<void>;
}>;

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json; charset=utf-8",
    },
  });

const tokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get("authorization");
  if (
    authorization === null ||
    !authorization.startsWith("Bearer ") ||
    !/^oa_sarah_lk_[A-Za-z0-9_-]{43,256}$/u.test(authorization.slice(7))
  ) {
    return undefined;
  }
  return authorization.slice(7);
};

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  const record = value as Readonly<Record<string, unknown>>;
  return `{${Object.keys(record)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
    .join(",")}}`;
};

const parseBody = async <A>(
  request: Request,
  decode: (value: unknown) => A,
): Promise<A | undefined> => {
  const contentLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(contentLength) || contentLength > 16_384) {
    return undefined;
  }
  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > 16_384) return undefined;
    return decode(JSON.parse(text) as unknown);
  } catch {
    return undefined;
  }
};

const capabilityProfile = (
  kind: "private" | "community",
):
  | Readonly<{
      kind: "private_owner_v1";
      contextRead: true;
      editorProposals: true;
      ownerMemory: true;
      workspace: true;
      payments: false;
      release: false;
      memberAdmin: false;
      shell: false;
      git: false;
      credentials: false;
    }>
  | Readonly<{
      kind: "community_member_v1";
      contextRead: false;
      editorProposals: false;
      ownerMemory: false;
      workspace: false;
      payments: false;
      release: false;
      memberAdmin: false;
      shell: false;
      git: false;
      credentials: false;
    }> =>
  kind === "private"
    ? {
        kind: "private_owner_v1",
        contextRead: true,
        editorProposals: true,
        ownerMemory: true,
        workspace: true,
        payments: false,
        release: false,
        memberAdmin: false,
        shell: false,
        git: false,
        credentials: false,
      }
    : {
        kind: "community_member_v1",
        contextRead: false,
        editorProposals: false,
        ownerMemory: false,
        workspace: false,
        payments: false,
        release: false,
        memberAdmin: false,
        shell: false,
        git: false,
        credentials: false,
      };

export const handleSarahLiveKitWorkerClaim = async <Bindings>(
  dependencies: SarahLiveKitWorkerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<Response> => {
  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const token = tokenFromRequest(request);
  if (token === undefined) return noStoreJson({ error: "unauthorized" }, 401);
  const body = await parseBody(request, decodeSarahLiveKitJobClaimRequest);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_sarah_livekit_worker_claim" }, 400);
  }
  let opened: Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }> | undefined;
  try {
    opened = await dependencies.openStore(env);
    const nowIso = new Date((dependencies.now ?? Date.now)()).toISOString();
    const claimed = await opened.store.claimLiveKitWorkerJob({
      workerControlTokenDigest: sha256(token),
      workerRefDigest: sha256(body.workerRef),
      workerJobRef: body.jobRef,
      workerRoomSid: body.roomSid,
      sessionRef: body.dispatch.sessionRef,
      generation: body.dispatch.generation,
      roomRef: body.dispatch.roomRef,
      roomEpoch: body.dispatch.roomEpoch,
      dispatchRef: body.dispatchRef,
      participantRef: body.dispatch.participantRef,
      sarahParticipantRef: body.dispatch.sarahParticipantRef,
      sarahPresenceLeaseRef: body.dispatch.sarahPresenceLeaseRef,
      capabilityProfile: body.dispatch.capabilityProfile,
      roomContext: body.dispatch.roomContext,
      nowIso,
    });
    const sessionExpiresAtMs = Date.parse(claimed.sessionExpiresAt);
    if (
      !Number.isSafeInteger(sessionExpiresAtMs) ||
      sessionExpiresAtMs <= (dependencies.now ?? Date.now)()
    ) {
      return noStoreJson({ error: "sarah_livekit_generation_expired" }, 409);
    }
    return noStoreJson(
      {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        admitted: true,
        sessionRef: claimed.sessionRef,
        generation: claimed.generation,
        sessionExpiresAtMs,
        safetyIdentifier: sha256(claimed.ownerUserId),
        capabilityProfile: capabilityProfile(claimed.roomContext.kind),
      },
      200,
    );
  } catch {
    return noStoreJson({ error: "sarah_livekit_worker_claim_refused" }, 409);
  } finally {
    try {
      await opened?.close();
    } catch {
      // The response outcome is already fixed and must not expose storage details.
    }
  }
};

export const handleSarahLiveKitWorkerEvent = async <Bindings>(
  dependencies: SarahLiveKitWorkerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<Response> => {
  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const token = tokenFromRequest(request);
  if (token === undefined) return noStoreJson({ error: "unauthorized" }, 401);
  const body = await parseBody(request, decodeSarahLiveKitJobEvent);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_sarah_livekit_worker_event" }, 400);
  }
  let opened: Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }> | undefined;
  try {
    opened = await dependencies.openStore(env);
    const now = (dependencies.now ?? Date.now)();
    const nowIso = new Date(now).toISOString();
    const common = {
      workerControlTokenDigest: sha256(token),
      workerJobRef: body.jobRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      eventRef: body.eventRef,
      eventPayloadDigest: sha256(canonicalJson(body)),
      nowIso,
    } as const;
    const result =
      body._tag === "worker_connected"
        ? await opened.store.applyLiveKitWorkerEvent({
            ...common,
            eventKind: body._tag,
            workerRoomSid: body.roomSid,
          })
        : body._tag === "lease_check"
          ? await opened.store.applyLiveKitWorkerEvent({
              ...common,
              eventKind: body._tag,
            })
          : body._tag === "close"
            ? await opened.store.applyLiveKitWorkerEvent({
                ...common,
                eventKind: body._tag,
                closeReason: `livekit_worker_${body.reason}`,
              })
            : await (async () => {
                const rate = dependencies.creditMsatPerMillionTokens(env);
                if (rate === undefined || !Number.isSafeInteger(rate) || rate <= 0) {
                  return undefined;
                }
                return opened.store.applyLiveKitWorkerEvent({
                  ...common,
                  eventKind: body._tag,
                  usage: {
                    usageKind: body._tag === "response_usage" ? "response" : "transcription",
                    providerResponseRef:
                      body._tag === "response_usage"
                        ? `response:${body.providerResponseRef}`
                        : `transcription:${body.providerTranscriptionRef}`,
                    inputTokens: body.inputTokens,
                    outputTokens: body.outputTokens,
                    cachedInputTokens: body.cachedInputTokens,
                    audioInputTokens: body.audioInputTokens,
                    audioOutputTokens: body.audioOutputTokens,
                    chargeMsat: Math.ceil(
                      ((body.inputTokens + body.outputTokens) * rate) / 1_000_000,
                    ),
                  },
                });
              })();
    if (result === undefined) {
      return noStoreJson({ error: "sarah_livekit_usage_rate_invalid" }, 503);
    }
    if (body._tag === "close") {
      await dependencies.cleanup(env, {
        sessionRef: body.sessionRef,
        generation: body.generation,
      });
    }
    return noStoreJson(
      result.stopReason === undefined
        ? { accepted: true }
        : { accepted: true, stopReason: result.stopReason },
      200,
    );
  } catch {
    return noStoreJson({ error: "sarah_livekit_worker_event_refused" }, 409);
  } finally {
    try {
      await opened?.close();
    } catch {
      // The response outcome is already fixed and must not expose storage details.
    }
  }
};
