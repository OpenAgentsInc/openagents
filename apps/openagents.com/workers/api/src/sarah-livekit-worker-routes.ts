import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_JOB_CLAIM_PATH,
  SARAH_LIVEKIT_JOB_EVENT_PATH,
  SARAH_LIVEKIT_TOOL_PROPOSAL_PATH,
  SARAH_LIVEKIT_TOOL_STATE_PATH,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  decodeSarahLiveKitJobClaimRequest,
  decodeSarahLiveKitJobEvent,
  decodeSarahLiveKitToolProposalRequest,
  decodeSarahLiveKitToolStateRequest,
  sarahEditorCommandRequiresConfirmation,
  validateSarahEditorCommandTarget,
} from "@openagentsinc/audio-contract";
import type { SarahRealtimeVoiceStore } from "@openagentsinc/khala-sync-server";
import { createHash, timingSafeEqual } from "node:crypto";

import {
  deriveSarahLiveKitControlToken,
  parseSarahLiveKitControlRoot,
} from "./sarah-livekit-room-broker";

export const SARAH_LIVEKIT_WORKER_CLAIM_PATH = SARAH_LIVEKIT_JOB_CLAIM_PATH;
export const SARAH_LIVEKIT_WORKER_EVENT_PATH = SARAH_LIVEKIT_JOB_EVENT_PATH;
export const SARAH_LIVEKIT_WORKER_TOOL_PROPOSAL_PATH = SARAH_LIVEKIT_TOOL_PROPOSAL_PATH;
export const SARAH_LIVEKIT_WORKER_TOOL_STATE_PATH = SARAH_LIVEKIT_TOOL_STATE_PATH;

export type SarahLiveKitWorkerRouteDependencies<Bindings> = Readonly<{
  controlRoot: (env: Bindings) => string | undefined;
  creditMsatPerMillionTokens: (env: Bindings) => number | undefined;
  now?: (() => number) | undefined;
  openStore: (
    env: Bindings,
  ) => Promise<Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>>;
  cleanup: (
    env: Bindings,
    input: Readonly<{ sessionRef: string; generation: number }>,
  ) => Promise<void>;
  resolveCommunityAccess?: (
    env: Bindings,
    input: Readonly<{
      ownerUserId: string;
      communityRef: string;
      channelRef: string;
    }>,
  ) => Promise<
    | Readonly<{
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        subscribeAllowed: boolean;
      }>
    | undefined
  >;
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

const tokensMatch = (left: string, right: string): boolean => {
  const leftBytes = Buffer.from(left);
  const rightBytes = Buffer.from(right);
  return leftBytes.length === rightBytes.length && timingSafeEqual(leftBytes, rightBytes);
};

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
      agentThreadProposals: true;
      ownerMemory: false;
      workspace: false;
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
      agentThreadProposals: false;
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
        agentThreadProposals: true,
        ownerMemory: false,
        workspace: false,
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
        agentThreadProposals: false,
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
  const controlRoot = parseSarahLiveKitControlRoot(dependencies.controlRoot(env));
  if (controlRoot === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
  let expectedToken: string;
  try {
    expectedToken = deriveSarahLiveKitControlToken(controlRoot, {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      agentName: SARAH_LIVEKIT_AGENT_NAME,
      ...body.dispatch,
    });
  } catch {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
  if (!tokensMatch(token, expectedToken)) {
    return noStoreJson({ error: "unauthorized" }, 401);
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
  if (parseSarahLiveKitControlRoot(dependencies.controlRoot(env)) === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
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
    if (body._tag === "worker_connected" || body._tag === "lease_check") {
      const membershipLease = await opened.store.readLiveKitMembershipLease({
        workerControlTokenDigest: sha256(token),
        workerJobRef: body.jobRef,
        sessionRef: body.sessionRef,
        generation: body.generation,
      });
      if (membershipLease?.roomContext.kind === "community") {
        let currentAccess:
          | Readonly<{
              communityRef: string;
              channelRef: string;
              membershipRevision: string;
              subscribeAllowed: boolean;
            }>
          | undefined;
        try {
          currentAccess = await dependencies.resolveCommunityAccess?.(env, {
            ownerUserId: membershipLease.ownerUserId,
            communityRef: membershipLease.roomContext.communityRef,
            channelRef: membershipLease.roomContext.channelRef,
          });
        } catch {
          currentAccess = undefined;
        }
        if (
          currentAccess === undefined ||
          !currentAccess.subscribeAllowed ||
          currentAccess.communityRef !== membershipLease.roomContext.communityRef ||
          currentAccess.channelRef !== membershipLease.roomContext.channelRef ||
          currentAccess.membershipRevision !== membershipLease.roomContext.membershipRevision
        ) {
          await opened.store.revokeLiveKitRoom({
            sessionRef: body.sessionRef,
            generation: body.generation,
            stopReason: "membership_revoked",
            reason: "community_membership_changed",
            nowIso,
          });
        }
      }
    }
    const result =
      body._tag === "worker_connected"
        ? await opened.store.applyLiveKitWorkerEvent({
            ...common,
            eventKind: body._tag,
            workerRoomSid: body.roomSid,
          })
        : body._tag === "provider_admitted"
          ? await opened.store.applyLiveKitWorkerEvent({
              ...common,
              eventKind: body._tag,
              providerSessionRefDigest: body.providerSessionRefDigest,
              providerConfigurationDigest: body.providerConfigurationDigest,
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
                  const usage = {
                    inputTokens: body.inputTokens,
                    outputTokens: body.outputTokens,
                    cachedInputTokens: body.cachedInputTokens,
                    audioInputTokens: body.audioInputTokens,
                    audioOutputTokens: body.audioOutputTokens,
                    chargeMsat: Math.ceil(
                      ((body.inputTokens + body.outputTokens) * rate) / 1_000_000,
                    ),
                  } as const;
                  return body._tag === "response_usage"
                    ? opened.store.applyLiveKitWorkerEvent({
                        ...common,
                        eventKind: body._tag,
                        usage: {
                          ...usage,
                          usageKind: "response",
                          providerResponseRef: `response:${body.providerResponseRef}`,
                          providerStatus: body.status,
                        },
                      })
                    : opened.store.applyLiveKitWorkerEvent({
                        ...common,
                        eventKind: body._tag,
                        usage: {
                          ...usage,
                          usageKind: "transcription",
                          providerResponseRef: `transcription:${body.providerTranscriptionRef}`,
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

export const handleSarahLiveKitWorkerToolProposal = async <Bindings>(
  dependencies: SarahLiveKitWorkerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<Response> => {
  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const token = tokenFromRequest(request);
  if (token === undefined) return noStoreJson({ error: "unauthorized" }, 401);
  if (parseSarahLiveKitControlRoot(dependencies.controlRoot(env)) === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
  const body = await parseBody(request, decodeSarahLiveKitToolProposalRequest);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_sarah_livekit_tool_proposal" }, 400);
  }
  if (body.command._tag === "open_path") {
    return noStoreJson({ error: "invalid_sarah_livekit_tool_proposal" }, 400);
  }
  let opened: Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }> | undefined;
  try {
    const command = validateSarahEditorCommandTarget(body.command);
    if (command._tag === "open_path") {
      throw new Error("editor_open_path_not_allowed");
    }
    opened = await dependencies.openStore(env);
    const now = (dependencies.now ?? Date.now)();
    const nowIso = new Date(now).toISOString();
    const expiresAt = new Date(now + 60_000).toISOString();
    const commandPayload = canonicalJson(command);
    const proposalRef = `sarah_lk_tool_${sha256(
      canonicalJson([body.sessionRef, body.generation, body.eventRef]),
    ).slice(0, 40)}`;
    const proposalDigest = sha256(
      canonicalJson({
        protocol: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        sessionRef: body.sessionRef,
        generation: body.generation,
        proposalRef,
        command,
        expiresAt,
      }),
    );
    const proposal = await opened.store.proposeLiveKitTool({
      workerControlTokenDigest: sha256(token),
      workerJobRef: body.jobRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      workerEventRef: body.eventRef,
      providerCallRef: body.providerCallRef,
      commandPayloadDigest: sha256(commandPayload),
      proposalRef,
      proposalDigest,
      command,
      confirmationRequired: sarahEditorCommandRequiresConfirmation(command),
      nowIso,
      expiresAt,
    });
    return noStoreJson(
      {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        accepted: true,
        proposal,
      },
      200,
    );
  } catch {
    return noStoreJson({ error: "sarah_livekit_tool_proposal_refused" }, 409);
  } finally {
    try {
      await opened?.close();
    } catch {
      // The response outcome is already fixed and must not expose storage details.
    }
  }
};

export const handleSarahLiveKitWorkerToolState = async <Bindings>(
  dependencies: SarahLiveKitWorkerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<Response> => {
  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const token = tokenFromRequest(request);
  if (token === undefined) return noStoreJson({ error: "unauthorized" }, 401);
  if (parseSarahLiveKitControlRoot(dependencies.controlRoot(env)) === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
  const body = await parseBody(request, decodeSarahLiveKitToolStateRequest);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_sarah_livekit_tool_state" }, 400);
  }
  let opened: Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }> | undefined;
  try {
    opened = await dependencies.openStore(env);
    const state = await opened.store.readLiveKitToolState({
      workerControlTokenDigest: sha256(token),
      workerJobRef: body.jobRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      proposalRef: body.proposalRef,
      proposalDigest: body.proposalDigest,
      nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
    });
    return noStoreJson(
      {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        ...state,
      },
      200,
    );
  } catch {
    return noStoreJson({ error: "sarah_livekit_tool_state_refused" }, 409);
  } finally {
    try {
      await opened?.close();
    } catch {
      // The response outcome is already fixed and must not expose storage details.
    }
  }
};
