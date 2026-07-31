import { JobRestartPolicy, TrackSource } from "@livekit/protocol";
import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitDispatchMetadata,
} from "@openagentsinc/audio-contract";
import {
  AccessToken,
  AgentDispatchClient,
  RoomServiceClient,
  ServerError,
} from "livekit-server-sdk";
import { createHash } from "node:crypto";

import type {
  SarahVoiceLiveKitProvision,
  SarahVoiceLiveKitRoomBroker,
} from "./sarah-realtime-voice-routes";

export type SarahLiveKitRoomBrokerConfig = Readonly<{
  livekitUrl: string;
  apiKey: string;
  apiSecret: string;
}>;

export type SarahLiveKitRoomBrokerClients = Readonly<{
  createRoom: (input: {
    name: string;
    emptyTimeout: number;
    departureTimeout: number;
    maxParticipants: number;
    metadata: string;
  }) => Promise<unknown>;
  deleteRoom: (roomRef: string) => Promise<unknown>;
  createDispatch: (
    roomRef: string,
    agentName: string,
    options: Readonly<{ metadata: string; restartPolicy: JobRestartPolicy }>,
  ) => Promise<Readonly<{ id: string }>>;
  listDispatch: (roomRef: string) => Promise<ReadonlyArray<Readonly<{ id: string }>>>;
  deleteDispatch: (dispatchRef: string, roomRef: string) => Promise<unknown>;
}>;

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const controlOrigin = (livekitUrl: string): string => {
  const url = new URL(livekitUrl);
  if (
    url.protocol !== "wss:" ||
    url.username !== "" ||
    url.password !== "" ||
    url.search !== "" ||
    url.hash !== ""
  ) {
    throw new Error("The Sarah LiveKit URL must be a credential-free WSS URL");
  }
  url.protocol = "https:";
  return url.toString().replace(/\/$/u, "");
};

const roomContextForDispatch = (
  context: Parameters<SarahVoiceLiveKitRoomBroker["provision"]>[0]["roomContext"],
): SarahLiveKitDispatchMetadata["roomContext"] =>
  context.kind === "private"
    ? { kind: "private" }
    : {
        kind: "community",
        communityRef: context.communityRef,
        channelRef: context.channelRef,
        membershipRevision: context.membershipRevision,
      };

const isSarahClientCapabilityProfile = (
  value: string,
): value is SarahLiveKitDispatchMetadata["capabilityProfile"] =>
  value === "omega_editor" || value === "mobile_voice_only" || value === "mobile_command_center";

const cleanAll = async (operations: ReadonlyArray<Promise<unknown>>): Promise<void> => {
  const outcomes = await Promise.allSettled(operations);
  const failures = outcomes.flatMap((outcome) =>
    outcome.status === "rejected" &&
    !(
      outcome.reason instanceof ServerError &&
      (outcome.reason.status === 404 || outcome.reason.code === "not_found")
    )
      ? [outcome.reason]
      : [],
  );
  if (failures.length > 0) {
    throw new AggregateError(failures, "Sarah LiveKit cleanup was incomplete");
  }
};

export const makeSarahLiveKitRoomBroker = (
  config: SarahLiveKitRoomBrokerConfig,
  now: () => number = Date.now,
  suppliedClients?: SarahLiveKitRoomBrokerClients,
): SarahVoiceLiveKitRoomBroker => {
  const origin = controlOrigin(config.livekitUrl);
  const roomService = new RoomServiceClient(origin, config.apiKey, config.apiSecret);
  const dispatchService = new AgentDispatchClient(origin, config.apiKey, config.apiSecret);
  const clients: SarahLiveKitRoomBrokerClients = suppliedClients ?? {
    createRoom: (input) => roomService.createRoom(input),
    deleteRoom: (roomRef) => roomService.deleteRoom(roomRef),
    createDispatch: (roomRef, agentName, options) =>
      dispatchService.createDispatch(roomRef, agentName, options),
    listDispatch: (roomRef) => dispatchService.listDispatch(roomRef),
    deleteDispatch: (dispatchRef, roomRef) => dispatchService.deleteDispatch(dispatchRef, roomRef),
  };

  const provision: SarahVoiceLiveKitRoomBroker["provision"] = async (input) => {
    if (!isSarahClientCapabilityProfile(input.capabilityProfile)) {
      throw new Error("The Sarah LiveKit capability profile is unsupported");
    }
    const roomRef = `oa-sarah-${sha256(input.idempotencyKey).slice(0, 40)}`;
    const participantRef = `owner-${sha256(input.ownerUserId).slice(0, 40)}`;
    const sarahParticipantRef = "principal.sarah";
    const sarahPresenceLeaseRef = `sarah-presence-${sha256(
      `${input.idempotencyKey}:presence`,
    ).slice(0, 40)}`;
    const joinExpiresAtMs = Math.min(input.expiresAtMs, now() + 60_000);
    const joinTtlSeconds = Math.max(1, Math.floor((joinExpiresAtMs - now()) / 1_000));
    if (joinTtlSeconds < 1) {
      throw new Error("The Sarah LiveKit join grant has already expired");
    }

    await clients.createRoom({
      name: roomRef,
      emptyTimeout: 60,
      departureTimeout: 30,
      maxParticipants: input.roomContext.kind === "private" ? 2 : 128,
      metadata: JSON.stringify({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        roomEpoch: 1,
      }),
    });

    const participantToken = new AccessToken(config.apiKey, config.apiSecret, {
      identity: participantRef,
      ttl: joinTtlSeconds,
    });
    participantToken.addGrant({
      room: roomRef,
      roomJoin: true,
      canPublish: input.publishAllowed,
      canSubscribe: input.subscribeAllowed,
      canPublishData: false,
      canUpdateOwnMetadata: false,
      canPublishSources: [TrackSource.MICROPHONE],
      roomAdmin: false,
      roomCreate: false,
      roomList: false,
    });
    const participantGrant = await participantToken.toJwt();

    const metadata: SarahLiveKitDispatchMetadata = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      agentName: SARAH_LIVEKIT_AGENT_NAME,
      controlToken: input.workerControlToken,
      sessionRef: input.sessionRef,
      generation: input.generation,
      roomRef,
      roomEpoch: 1,
      participantRef,
      sarahParticipantRef,
      sarahPresenceLeaseRef,
      capabilityProfile: input.capabilityProfile,
      roomContext: roomContextForDispatch(input.roomContext),
    };
    const dispatch = await clients.createDispatch(roomRef, SARAH_LIVEKIT_AGENT_NAME, {
      metadata: JSON.stringify(metadata),
      restartPolicy: JobRestartPolicy.JRP_NEVER,
    });
    if (dispatch.id.trim() === "") {
      await clients.deleteRoom(roomRef);
      throw new Error("LiveKit returned an empty Sarah dispatch reference");
    }

    const grantClaims = {
      roomRef,
      participantRef,
      expiresAtMs: joinExpiresAtMs,
      roomJoin: true as const,
      canPublish: input.publishAllowed,
      canSubscribe: input.subscribeAllowed,
      canPublishData: false as const,
      canUpdateOwnMetadata: false as const,
      canPublishSources: ["microphone"] as const,
      roomAdmin: false as const,
      roomCreate: false as const,
      roomList: false as const,
    };
    return {
      livekitUrl: config.livekitUrl,
      roomRef,
      roomEpoch: 1,
      participantRef,
      sarahParticipantRef,
      participantGrant,
      joinExpiresAtMs,
      dispatchRef: dispatch.id,
      sarahPresenceLeaseRef,
      grantClaims,
    } satisfies SarahVoiceLiveKitProvision;
  };

  const cleanupRoom = async (
    room: Parameters<SarahVoiceLiveKitRoomBroker["cleanupRoom"]>[0],
  ): Promise<void> =>
    cleanAll([
      clients.deleteDispatch(room.dispatchRef, room.roomRef),
      clients.deleteRoom(room.roomRef),
    ]);

  return {
    provision,
    cleanup: (provisioned) =>
      cleanupRoom({
        sessionRef: "cleanup-by-provision",
        generation: 1,
        roomRef: provisioned.roomRef,
        roomEpoch: provisioned.roomEpoch,
        dispatchRef: provisioned.dispatchRef,
        sarahPresenceLeaseRef: provisioned.sarahPresenceLeaseRef,
      }),
    cleanupByIdempotencyKey: async (idempotencyKey) => {
      const roomRef = `oa-sarah-${sha256(idempotencyKey).slice(0, 40)}`;
      const dispatches = await clients.listDispatch(roomRef);
      await cleanAll([
        ...dispatches.map((dispatch) => clients.deleteDispatch(dispatch.id, roomRef)),
        clients.deleteRoom(roomRef),
      ]);
    },
    cleanupRoom,
  };
};

export const parseSarahLiveKitRoomBrokerConfig = (
  env: Readonly<{
    SARAH_LIVEKIT_URL?: string | undefined;
    SARAH_LIVEKIT_API_KEY?: string | undefined;
    SARAH_LIVEKIT_API_SECRET?: string | undefined;
  }>,
): SarahLiveKitRoomBrokerConfig | undefined => {
  const livekitUrl = env.SARAH_LIVEKIT_URL?.trim();
  const apiKey = env.SARAH_LIVEKIT_API_KEY?.trim();
  const apiSecret = env.SARAH_LIVEKIT_API_SECRET?.trim();
  if (
    livekitUrl === undefined ||
    apiKey === undefined ||
    apiSecret === undefined ||
    !/^API[23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{12}$/u.test(apiKey) ||
    !/^[A-Za-z0-9]{43,52}$/u.test(apiSecret)
  ) {
    return undefined;
  }
  try {
    controlOrigin(livekitUrl);
  } catch {
    return undefined;
  }
  return { livekitUrl, apiKey, apiSecret };
};
