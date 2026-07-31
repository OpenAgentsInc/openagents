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
import {
  type SarahLiveKitRoomAuthorityStore,
  type SarahRealtimeVoiceStore,
} from "@openagentsinc/khala-sync-server";
import { createHash, timingSafeEqual } from "node:crypto";

import {
  initialSarahLiveKitRoomAuthoritySnapshot,
  issueSarahLiveKitRoomPresenceLease,
} from "./sarah-livekit-room-authority";
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
  now?: (() => number) | undefined;
  openStore: (env: Bindings) => Promise<
    Readonly<{
      store: SarahRealtimeVoiceStore;
      authorityStore?: SarahLiveKitRoomAuthorityStore | undefined;
      close: () => Promise<void>;
    }>
  >;
  sarahNostrPublicKey?: ((env: Bindings) => string | undefined) | undefined;
  e2eeKeyRevision?: ((env: Bindings) => string | undefined) | undefined;
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
  resolveRoomFloor?: (
    env: Bindings,
    input: Readonly<{
      presenceLeaseRef: string;
      nowMs: number;
    }>,
  ) => Promise<
    | Readonly<{
        authorityRevision: number;
        interruptSequence: number;
        participantRef: string | null;
        expiresAtMs: number | null;
        presenceActive: boolean;
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
const digest = (value: string | undefined): string | undefined =>
  value !== undefined && /^[a-f0-9]{64}$/u.test(value) ? value : undefined;

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
  let opened:
    | Readonly<{
        store: SarahRealtimeVoiceStore;
        authorityStore?: SarahLiveKitRoomAuthorityStore | undefined;
        close: () => Promise<void>;
      }>
    | undefined;
  try {
    const nowMs = (dependencies.now ?? Date.now)();
    if (body.dispatch.roomContext.kind === "community") {
      const authority = await dependencies.resolveRoomFloor?.(env, {
        presenceLeaseRef: body.dispatch.sarahPresenceLeaseRef,
        nowMs,
      });
      if (authority === undefined || !authority.presenceActive) {
        return noStoreJson({ error: "sarah_livekit_room_authority_unavailable" }, 409);
      }
    }
    opened = await dependencies.openStore(env);
    const nowIso = new Date(nowMs).toISOString();
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
    let presenceLease;
    if (claimed.roomContext.kind === "community") {
      const sarahPubkey = digest(dependencies.sarahNostrPublicKey?.(env));
      const e2eeKeyRevision = digest(dependencies.e2eeKeyRevision?.(env));
      if (
        sarahPubkey === undefined ||
        e2eeKeyRevision === undefined ||
        opened.authorityStore === undefined
      ) {
        return noStoreJson({ error: "sarah_livekit_presence_authority_unavailable" }, 503);
      }
      const existing = await opened.authorityStore.read(body.dispatch.sarahPresenceLeaseRef);
      if (existing !== undefined) {
        presenceLease = existing.presence;
        if (
          !existing.presenceActive ||
          presenceLease.sarahPubkey !== sarahPubkey ||
          presenceLease.communityRef !== claimed.roomContext.communityRef ||
          presenceLease.channelRef !== claimed.roomContext.channelRef ||
          presenceLease.membershipRevision !== claimed.roomContext.membershipRevision ||
          presenceLease.e2eeKeyRevision !== e2eeKeyRevision ||
          presenceLease.roomRef !== body.dispatch.roomRef ||
          presenceLease.roomEpoch !== body.dispatch.roomEpoch ||
          presenceLease.sarahParticipantRef !== body.dispatch.sarahParticipantRef ||
          presenceLease.dispatchRef !== body.dispatchRef ||
          presenceLease.sessionRef !== claimed.sessionRef ||
          presenceLease.generation !== claimed.generation ||
          presenceLease.admissionDigest !== claimed.admissionDigest ||
          presenceLease.expiresAtMs <= (dependencies.now ?? Date.now)()
        ) {
          return noStoreJson({ error: "sarah_livekit_presence_authority_conflict" }, 409);
        }
      } else {
        presenceLease = issueSarahLiveKitRoomPresenceLease({
          sarahPubkey,
          presenceLeaseRef: body.dispatch.sarahPresenceLeaseRef,
          communityRef: claimed.roomContext.communityRef,
          channelRef: claimed.roomContext.channelRef,
          membershipRevision: claimed.roomContext.membershipRevision,
          currentMembershipRevision: claimed.roomContext.membershipRevision,
          e2eeKeyRevision,
          roomRef: body.dispatch.roomRef,
          roomEpoch: body.dispatch.roomEpoch,
          sarahParticipantRef: body.dispatch.sarahParticipantRef,
          dispatchRef: body.dispatchRef,
          sessionRef: claimed.sessionRef,
          generation: claimed.generation,
          admissionDigest: claimed.admissionDigest,
          issuedAtMs: (dependencies.now ?? Date.now)(),
          sessionExpiresAtMs,
        });
        const persisted = await opened.authorityStore.create(
          initialSarahLiveKitRoomAuthoritySnapshot(presenceLease),
          nowIso,
        );
        presenceLease = persisted.presence;
      }
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
        ...(presenceLease === undefined ? {} : { presenceLease }),
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
    let communityPresenceLeaseRef: string | undefined;
    let membershipRevoked = false;
    if (body._tag === "worker_connected" || body._tag === "lease_check") {
      const membershipLease = await opened.store.readLiveKitMembershipLease({
        workerControlTokenDigest: sha256(token),
        workerJobRef: body.jobRef,
        sessionRef: body.sessionRef,
        generation: body.generation,
      });
      if (membershipLease?.roomContext.kind === "community") {
        communityPresenceLeaseRef = membershipLease.sarahPresenceLeaseRef;
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
          membershipRevoked = true;
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
                  accountingStatus: body.accountingStatus,
                })
              : await (async () => {
                  if (body._tag === "interrupt_applied") {
                    return opened.store.applyLiveKitWorkerEvent({
                      ...common,
                      eventKind: body._tag,
                      interruptSequence: body.interruptSequence,
                    });
                  }
                  if (body._tag === "provider_disconnect_fault_applied") {
                    return opened.store.applyLiveKitWorkerEvent({
                      ...common,
                      eventKind: body._tag,
                      requestRef: body.requestRef,
                      providerSessionRefDigest: body.providerSessionRefDigest,
                    });
                  }
                  const usage = {
                    inputTokens: body.inputTokens,
                    outputTokens: body.outputTokens,
                    cachedInputTokens: body.cachedInputTokens,
                    audioInputTokens: body.audioInputTokens,
                    audioOutputTokens: body.audioOutputTokens,
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
    if (body._tag === "close" && body.accountingStatus === "exact") {
      await dependencies.cleanup(env, {
        sessionRef: body.sessionRef,
        generation: body.generation,
      });
    }
    if (membershipRevoked) {
      return noStoreJson(
        {
          accepted: true,
          stopReason: "membership_revoked",
          ...(result.interruptSequence === undefined
            ? {}
            : { interruptSequence: result.interruptSequence }),
        },
        200,
      );
    }
    const roomFloor =
      communityPresenceLeaseRef === undefined
        ? undefined
        : await dependencies.resolveRoomFloor?.(env, {
            presenceLeaseRef: communityPresenceLeaseRef,
            nowMs: now,
          });
    if (communityPresenceLeaseRef !== undefined && roomFloor === undefined) {
      await opened.store.revokeLiveKitRoom({
        sessionRef: body.sessionRef,
        generation: body.generation,
        stopReason: "membership_revoked",
        reason: "community_room_authority_unavailable",
        nowIso,
      });
      return noStoreJson({ accepted: true, stopReason: "membership_revoked" }, 200);
    }
    return noStoreJson(
      {
        accepted: true,
        ...(result.interruptSequence === undefined
          ? {}
          : { interruptSequence: result.interruptSequence }),
        ...(result.providerDisconnectFault === undefined
          ? {}
          : { providerDisconnectFault: result.providerDisconnectFault }),
        ...(result.stopReason === undefined ? {} : { stopReason: result.stopReason }),
        ...(roomFloor === undefined
          ? {}
          : {
              authorityRevision: roomFloor.authorityRevision,
              floorParticipantRef: roomFloor.participantRef,
              floorExpiresAtMs: roomFloor.expiresAtMs,
              presenceActive: roomFloor.presenceActive,
              interruptSequence: Math.max(
                result.interruptSequence ?? 0,
                roomFloor.interruptSequence,
              ),
            }),
      },
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
