import { type SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { Schema as S } from "effect";
import { createHash } from "node:crypto";

import type { SarahLiveKitCommunityAccess } from "./sarah-livekit-community-access.js";
import {
  handleSarahLiveKitRoomMemberFloorRequest,
  handleSarahLiveKitRoomModeratorFloorRequest,
  handleSarahLiveKitRoomSnapshotRequest,
} from "./sarah-livekit-room-authority-routes.js";
import {
  initialSarahLiveKitRoomAuthoritySnapshot,
  issueSarahLiveKitRoomPresenceLease,
  removeSarahLiveKitRoomPresence,
  type SarahLiveKitRoomAuthoritySnapshot,
  type SarahLiveKitRoomMemberAccess,
} from "./sarah-livekit-room-authority.js";
import type { SarahVoiceLiveKitRoomBroker } from "./sarah-realtime-voice-routes.js";

export const SARAH_LIVEKIT_ROOM_MEMBER_FLOOR_PATH = "/api/sarah/livekit/room/floor/member" as const;
export const SARAH_LIVEKIT_ROOM_MODERATOR_FLOOR_PATH =
  "/api/sarah/livekit/room/floor/moderator" as const;
export const SARAH_LIVEKIT_ROOM_SNAPSHOT_PATH = "/api/sarah/livekit/room/snapshot" as const;
export const SARAH_LIVEKIT_ROOM_JOIN_PATH = "/api/sarah/livekit/room/join" as const;
export const SARAH_LIVEKIT_ROOM_SUMMON_PATH = "/api/sarah/livekit/room/summon" as const;
export const SARAH_LIVEKIT_ROOM_REMOVE_PATH = "/api/sarah/livekit/room/remove" as const;

type OpenedAuthorityStore = Readonly<{
  store: SarahLiveKitRoomAuthorityStore;
  close: () => Promise<void>;
}>;

export type SarahLiveKitRoomAuthorityProductionDependencies<Environment, Context> = Readonly<{
  openStore: (environment: Environment) => Promise<OpenedAuthorityStore>;
  requireUser: (
    request: Request,
    environment: Environment,
    context: Context,
  ) => Promise<Readonly<{ userId: string }> | undefined>;
  resolveCommunityAccess: (
    environment: Environment,
    input: Readonly<{
      ownerUserId: string;
      communityRef: string;
      channelRef: string;
    }>,
  ) => Promise<SarahLiveKitCommunityAccess | undefined>;
  onAuthorityChanged?: (
    environment: Environment,
    previous: SarahLiveKitRoomAuthoritySnapshot,
    current: SarahLiveKitRoomAuthoritySnapshot,
  ) => Promise<void>;
  now?: (() => number) | undefined;
}>;

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");
const SharedRoomRequest = S.Struct({
  presenceLeaseRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(256)),
  expectedRevision: S.optional(S.Int.check(S.isGreaterThanOrEqualTo(1))),
});
const JoinRoomRequest = S.Struct({
  communityRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(256)),
  channelRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(256)),
});

const noStoreJson = (status: number, error: string): Response =>
  Response.json(
    { error },
    {
      status,
      headers: {
        "cache-control": "no-store",
      },
    },
  );

export const handleSarahLiveKitRoomAuthorityProductionRequest = async <Environment, Context>(
  dependencies: SarahLiveKitRoomAuthorityProductionDependencies<Environment, Context>,
  mode: "member" | "moderator" | "snapshot",
  request: Request,
  environment: Environment,
  context: Context,
): Promise<Response> => {
  let authenticated: Readonly<{ userId: string }> | undefined;
  try {
    authenticated = await dependencies.requireUser(request, environment, context);
  } catch {
    return noStoreJson(503, "authentication_unavailable");
  }
  if (authenticated === undefined) {
    return noStoreJson(401, "authentication_required");
  }

  let opened: OpenedAuthorityStore | undefined;
  try {
    opened = await dependencies.openStore(environment);
    const authorityStore = opened.store;
    const userRefDigest = digest(`sarah-livekit-room-user\n${authenticated.userId}`);
    const now = () => new Date((dependencies.now ?? Date.now)());
    const routeDependencies = {
      store: authorityStore,
      authenticate: async () => ({
        userId: authenticated.userId,
        userRefDigest,
      }),
      resolveMember: async (
        memberEnvironment: Environment,
        input: Readonly<{
          userId: string;
          presenceLeaseRef: string;
          targetUserRefDigest?: string;
        }>,
      ): Promise<SarahLiveKitRoomMemberAccess | undefined> => {
        if (input.userId !== authenticated.userId) {
          return undefined;
        }
        const binding = await authorityStore.readParticipantBinding({
          presenceLeaseRef: input.presenceLeaseRef,
          ...(input.targetUserRefDigest === undefined
            ? { ownerUserId: input.userId }
            : { userRefDigest: input.targetUserRefDigest }),
          now: now().toISOString(),
        });
        if (binding === undefined) return undefined;
        const access = await dependencies.resolveCommunityAccess(memberEnvironment, {
          ownerUserId: binding.ownerUserId,
          communityRef: binding.communityRef,
          channelRef: binding.channelRef,
        });
        if (
          access === undefined ||
          access.communityRef !== binding.communityRef ||
          access.channelRef !== binding.channelRef ||
          access.membershipRevision !== binding.membershipRevision ||
          access.memberPubkey !== binding.memberPubkey ||
          !access.publishAllowed ||
          !access.subscribeAllowed
        ) {
          return undefined;
        }
        return {
          authenticated: true,
          allowlisted: true,
          active: true,
          role: access.role,
          userRefDigest: binding.userRefDigest,
          pubkey: binding.memberPubkey,
          participantRef: binding.participantRef,
          mappedParticipantRef: binding.participantRef,
          membershipRevision: binding.membershipRevision,
          roomRef: binding.roomRef,
          roomEpoch: binding.roomEpoch,
          safetyIdentifier: digest(
            `sarah-livekit-room-safety\n${binding.roomRef}\n${binding.ownerUserId}`,
          ),
        };
      },
      now,
      ...(dependencies.onAuthorityChanged === undefined
        ? {}
        : {
            onAuthorityChanged: (
              previous: SarahLiveKitRoomAuthoritySnapshot,
              current: SarahLiveKitRoomAuthoritySnapshot,
            ) => dependencies.onAuthorityChanged!(environment, previous, current),
          }),
    } as const;
    if (mode === "member") {
      return await handleSarahLiveKitRoomMemberFloorRequest(
        request,
        environment,
        routeDependencies,
      );
    }
    if (mode === "moderator") {
      return await handleSarahLiveKitRoomModeratorFloorRequest(
        request,
        environment,
        routeDependencies,
      );
    }
    return await handleSarahLiveKitRoomSnapshotRequest(request, environment, routeDependencies);
  } catch {
    return noStoreJson(503, "authority_unavailable");
  } finally {
    try {
      await opened?.close();
    } catch (error) {
      console.error("Sarah LiveKit room authority store close failed", error);
    }
  }
};

export type SarahLiveKitSharedRoomProductionDependencies<Environment, Context> =
  SarahLiveKitRoomAuthorityProductionDependencies<Environment, Context> &
    Readonly<{
      broker: (environment: Environment) => SarahVoiceLiveKitRoomBroker | undefined;
      liveKitUrl: (environment: Environment) => string | undefined;
      sarahPubkey: (environment: Environment) => string | undefined;
      e2eeKeyRevision: (environment: Environment) => string | undefined;
      stopWorker: (
        environment: Environment,
        input: Readonly<{
          sessionRef: string;
          generation: number;
          reason: "operator_stop";
        }>,
      ) => Promise<void>;
    }>;

const decodeSharedRoomRequest = async (
  request: Request,
): Promise<typeof SharedRoomRequest.Type | undefined> => {
  const declaredLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(declaredLength) || declaredLength > 4_096) return undefined;
  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > 4_096) return undefined;
    return S.decodeUnknownSync(SharedRoomRequest)(JSON.parse(text), {
      onExcessProperty: "error",
    });
  } catch {
    return undefined;
  }
};

const decodeJoinRoomRequest = async (
  request: Request,
): Promise<typeof JoinRoomRequest.Type | undefined> => {
  const declaredLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(declaredLength) || declaredLength > 4_096) return undefined;
  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > 4_096) return undefined;
    return S.decodeUnknownSync(JoinRoomRequest)(JSON.parse(text), {
      onExcessProperty: "error",
    });
  } catch {
    return undefined;
  }
};

const sharedParticipantRef = (presenceLeaseRef: string, ownerUserId: string): string =>
  `member-${digest(`sarah-livekit-room-member\n${presenceLeaseRef}\n${ownerUserId}`).slice(0, 40)}`;

const floorProjection = (snapshot: SarahLiveKitRoomAuthoritySnapshot, nowMs: number) =>
  snapshot.floor.state === "held" && snapshot.floor.lease.expiresAtMs <= nowMs
    ? {
        state: "stopped" as const,
        presenceLeaseRef: snapshot.presence.leaseRef,
        issuance: snapshot.floor.lease.issuance,
        reason: "timeout" as const,
      }
    : snapshot.floor;

export const handleSarahLiveKitCommunityRoomJoinRequest = async <Environment, Context>(
  dependencies: SarahLiveKitSharedRoomProductionDependencies<Environment, Context>,
  request: Request,
  environment: Environment,
  context: Context,
): Promise<Response> => {
  if (request.method !== "POST") return noStoreJson(405, "method_not_allowed");
  let authenticated: Readonly<{ userId: string }> | undefined;
  try {
    authenticated = await dependencies.requireUser(request, environment, context);
  } catch {
    return noStoreJson(503, "authentication_unavailable");
  }
  if (authenticated === undefined) return noStoreJson(401, "authentication_required");
  const body = await decodeJoinRoomRequest(request);
  if (body === undefined) return noStoreJson(400, "invalid_request");

  let opened: OpenedAuthorityStore | undefined;
  try {
    opened = await dependencies.openStore(environment);
    const nowMs = (dependencies.now ?? Date.now)();
    const now = new Date(nowMs).toISOString();
    const access = await dependencies.resolveCommunityAccess(environment, {
      ownerUserId: authenticated.userId,
      communityRef: body.communityRef,
      channelRef: body.channelRef,
    });
    if (
      access === undefined ||
      access.communityRef !== body.communityRef ||
      access.channelRef !== body.channelRef ||
      !access.publishAllowed ||
      !access.subscribeAllowed
    ) {
      return noStoreJson(403, "community_membership_required");
    }
    const rendezvous = await opened.store.readActiveCommunityRoomRendezvous({
      communityRef: body.communityRef,
      channelRef: body.channelRef,
      now,
    });
    if (rendezvous === undefined) return noStoreJson(404, "community_room_not_active");
    if (rendezvous.membershipRevision !== access.membershipRevision) {
      const staleSnapshot = await opened.store.read(rendezvous.presenceLeaseRef);
      if (staleSnapshot !== undefined && staleSnapshot.presenceActive) {
        const removed = removeSarahLiveKitRoomPresence(staleSnapshot);
        await opened.store.compareAndSwap({
          presenceLeaseRef: rendezvous.presenceLeaseRef,
          expectedRevision: staleSnapshot.revision,
          snapshot: removed,
          now,
        });
        await opened.store.retireCommunityRoomRendezvous({
          presenceLeaseRef: rendezvous.presenceLeaseRef,
          now,
        });
        await dependencies.stopWorker(environment, {
          sessionRef: staleSnapshot.presence.sessionRef,
          generation: staleSnapshot.presence.generation,
          reason: "operator_stop",
        });
      } else {
        await opened.store.retireCommunityRoomRendezvous({
          presenceLeaseRef: rendezvous.presenceLeaseRef,
          now,
        });
      }
      return noStoreJson(409, "community_membership_changed");
    }
    const snapshot = await opened.store.read(rendezvous.presenceLeaseRef);
    if (
      snapshot === undefined ||
      !snapshot.presenceActive ||
      snapshot.presence.expiresAtMs <= nowMs
    ) {
      return noStoreJson(410, "sarah_presence_inactive");
    }
    const broker = dependencies.broker(environment);
    const livekitUrl = dependencies.liveKitUrl(environment);
    if (broker?.grantParticipant === undefined || livekitUrl === undefined) {
      return noStoreJson(503, "shared_room_grant_unavailable");
    }
    const existing = await opened.store.readParticipantBinding({
      presenceLeaseRef: rendezvous.presenceLeaseRef,
      ownerUserId: authenticated.userId,
      now,
    });
    const participantRef =
      existing?.participantRef ??
      sharedParticipantRef(rendezvous.presenceLeaseRef, authenticated.userId);
    const grant = await broker.grantParticipant({
      roomRef: snapshot.presence.roomRef,
      participantRef,
      expiresAtMs: snapshot.presence.expiresAtMs,
      publishAllowed: access.publishAllowed,
      subscribeAllowed: access.subscribeAllowed,
    });
    await opened.store.bindParticipant({
      presenceLeaseRef: rendezvous.presenceLeaseRef,
      ownerUserId: authenticated.userId,
      userRefDigest: digest(`sarah-livekit-room-user\n${authenticated.userId}`),
      memberPubkey: access.memberPubkey,
      participantRef,
      membershipRevision: access.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      participantGrantDigest: digest(grant.participantGrant),
      joinExpiresAt: new Date(grant.joinExpiresAtMs).toISOString(),
      now,
    });
    const participants = await opened.store.listActiveCommunityRoomParticipants({
      presenceLeaseRef: rendezvous.presenceLeaseRef,
      now,
    });
    const localParticipant = participants.find(
      (participant) => participant.ownerUserId === authenticated.userId,
    );
    if (localParticipant === undefined) return noStoreJson(503, "participant_binding_unavailable");
    return Response.json(
      {
        schema: snapshot.presence.schema,
        livekitUrl,
        roomRef: snapshot.presence.roomRef,
        roomEpoch: snapshot.presence.roomEpoch,
        role: access.role,
        participantRef,
        sarahParticipantRef: snapshot.presence.sarahParticipantRef,
        participantGrant: grant.participantGrant,
        joinExpiresAtMs: grant.joinExpiresAtMs,
        presenceLeaseRef: snapshot.presence.leaseRef,
        authority: {
          ...snapshot.presence,
          revision: snapshot.revision,
          presenceActive: snapshot.presenceActive,
          localParticipant,
          verifiedParticipants: participants,
          floor: floorProjection(snapshot, nowMs),
        },
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  } catch {
    return noStoreJson(503, "authority_unavailable");
  } finally {
    try {
      await opened?.close();
    } catch (error) {
      console.error("Sarah shared-room join store close failed", error);
    }
  }
};

export const handleSarahLiveKitSharedRoomProductionRequest = async <Environment, Context>(
  dependencies: SarahLiveKitSharedRoomProductionDependencies<Environment, Context>,
  mode: "summon" | "remove",
  request: Request,
  environment: Environment,
  context: Context,
): Promise<Response> => {
  if (request.method !== "POST") return noStoreJson(405, "method_not_allowed");
  let authenticated: Readonly<{ userId: string }> | undefined;
  try {
    authenticated = await dependencies.requireUser(request, environment, context);
  } catch {
    return noStoreJson(503, "authentication_unavailable");
  }
  if (authenticated === undefined) return noStoreJson(401, "authentication_required");
  const body = await decodeSharedRoomRequest(request);
  if (body === undefined) return noStoreJson(400, "invalid_request");

  let opened: OpenedAuthorityStore | undefined;
  try {
    opened = await dependencies.openStore(environment);
    const nowMs = (dependencies.now ?? Date.now)();
    const now = new Date(nowMs).toISOString();
    let snapshot = await opened.store.read(body.presenceLeaseRef);
    if (snapshot === undefined) {
      if (mode === "remove") return noStoreJson(404, "authority_not_found");
      const binding = await opened.store.readCommunityRoomBinding({
        presenceLeaseRef: body.presenceLeaseRef,
        ownerUserId: authenticated.userId,
        now,
      });
      if (binding === undefined) return noStoreJson(404, "community_room_not_found");
      const access = await dependencies.resolveCommunityAccess(environment, {
        ownerUserId: authenticated.userId,
        communityRef: binding.communityRef,
        channelRef: binding.channelRef,
      });
      const sarahPubkey = dependencies.sarahPubkey(environment)?.trim().toLowerCase();
      const e2eeKeyRevision = dependencies.e2eeKeyRevision(environment)?.trim().toLowerCase();
      if (
        access === undefined ||
        access.communityRef !== binding.communityRef ||
        access.channelRef !== binding.channelRef ||
        access.membershipRevision !== binding.membershipRevision ||
        !access.publishAllowed ||
        !access.subscribeAllowed ||
        sarahPubkey === undefined ||
        !/^[a-f0-9]{64}$/u.test(sarahPubkey) ||
        e2eeKeyRevision === undefined ||
        !/^[a-f0-9]{64}$/u.test(e2eeKeyRevision)
      ) {
        return noStoreJson(403, "community_membership_required");
      }
      const presence = issueSarahLiveKitRoomPresenceLease({
        sarahPubkey,
        presenceLeaseRef: body.presenceLeaseRef,
        communityRef: binding.communityRef,
        channelRef: binding.channelRef,
        membershipRevision: binding.membershipRevision,
        currentMembershipRevision: access.membershipRevision,
        e2eeKeyRevision,
        roomRef: binding.roomRef,
        roomEpoch: binding.roomEpoch,
        sarahParticipantRef: binding.sarahParticipantRef,
        dispatchRef: binding.dispatchRef,
        sessionRef: binding.sessionRef,
        generation: binding.generation,
        admissionDigest: binding.admissionDigest,
        issuedAtMs: nowMs,
        sessionExpiresAtMs: Date.parse(binding.sessionExpiresAt),
      });
      snapshot = await opened.store.create(initialSarahLiveKitRoomAuthoritySnapshot(presence), now);
      const rendezvous = await opened.store.claimCommunityRoomRendezvous(snapshot, now);
      if (!rendezvous.claimed || rendezvous.presenceLeaseRef !== body.presenceLeaseRef) {
        return noStoreJson(409, "community_room_already_active");
      }
      await opened.store.bindParticipant({
        presenceLeaseRef: body.presenceLeaseRef,
        ownerUserId: authenticated.userId,
        userRefDigest: digest(`sarah-livekit-room-user\n${authenticated.userId}`),
        memberPubkey: access.memberPubkey,
        participantRef: binding.participantRef,
        membershipRevision: binding.membershipRevision,
        roomRef: binding.roomRef,
        roomEpoch: binding.roomEpoch,
        participantGrantDigest: binding.participantGrantDigest,
        joinExpiresAt: binding.joinExpiresAt,
        now,
      });
    }

    const access = await dependencies.resolveCommunityAccess(environment, {
      ownerUserId: authenticated.userId,
      communityRef: snapshot.presence.communityRef,
      channelRef: snapshot.presence.channelRef,
    });
    if (
      access === undefined ||
      access.membershipRevision !== snapshot.presence.membershipRevision ||
      access.communityRef !== snapshot.presence.communityRef ||
      access.channelRef !== snapshot.presence.channelRef ||
      !access.subscribeAllowed
    ) {
      return noStoreJson(403, "community_membership_required");
    }
    if (!snapshot.presenceActive || snapshot.presence.expiresAtMs <= nowMs) {
      return noStoreJson(410, "sarah_presence_inactive");
    }

    if (mode === "remove") {
      if (access.role !== "moderator") return noStoreJson(403, "moderator_required");
      if (body.expectedRevision !== snapshot.revision) {
        return noStoreJson(409, "stale_revision");
      }
      const removed = removeSarahLiveKitRoomPresence(snapshot);
      const persisted = await opened.store.compareAndSwap({
        presenceLeaseRef: body.presenceLeaseRef,
        expectedRevision: snapshot.revision,
        snapshot: removed,
        now,
      });
      await opened.store.retireCommunityRoomRendezvous({
        presenceLeaseRef: body.presenceLeaseRef,
        now,
      });
      await Promise.all([
        dependencies.onAuthorityChanged?.(environment, snapshot, persisted) ?? Promise.resolve(),
        dependencies.stopWorker(environment, {
          sessionRef: snapshot.presence.sessionRef,
          generation: snapshot.presence.generation,
          reason: "operator_stop",
        }),
      ]);
      return Response.json(
        { removed: true, revision: persisted.revision },
        { status: 200, headers: { "cache-control": "no-store" } },
      );
    }

    const broker = dependencies.broker(environment);
    const livekitUrl = dependencies.liveKitUrl(environment);
    if (broker?.grantParticipant === undefined || livekitUrl === undefined) {
      return noStoreJson(503, "shared_room_grant_unavailable");
    }
    const existing = await opened.store.readParticipantBinding({
      presenceLeaseRef: body.presenceLeaseRef,
      ownerUserId: authenticated.userId,
      now,
    });
    const participantRef =
      existing?.participantRef ?? sharedParticipantRef(body.presenceLeaseRef, authenticated.userId);
    const grant = await broker.grantParticipant({
      roomRef: snapshot.presence.roomRef,
      participantRef,
      expiresAtMs: snapshot.presence.expiresAtMs,
      publishAllowed: access.publishAllowed,
      subscribeAllowed: access.subscribeAllowed,
    });
    await opened.store.bindParticipant({
      presenceLeaseRef: body.presenceLeaseRef,
      ownerUserId: authenticated.userId,
      userRefDigest: digest(`sarah-livekit-room-user\n${authenticated.userId}`),
      memberPubkey: access.memberPubkey,
      participantRef,
      membershipRevision: access.membershipRevision,
      roomRef: snapshot.presence.roomRef,
      roomEpoch: snapshot.presence.roomEpoch,
      participantGrantDigest: digest(grant.participantGrant),
      joinExpiresAt: new Date(grant.joinExpiresAtMs).toISOString(),
      now,
    });
    return Response.json(
      {
        schema: snapshot.presence.schema,
        presenceLeaseRef: body.presenceLeaseRef,
        revision: snapshot.revision,
        livekitUrl,
        roomRef: snapshot.presence.roomRef,
        roomEpoch: snapshot.presence.roomEpoch,
        role: access.role,
        participantRef,
        sarahParticipantRef: snapshot.presence.sarahParticipantRef,
        participantGrant: grant.participantGrant,
        joinExpiresAtMs: grant.joinExpiresAtMs,
        processorDisclosure: snapshot.presence.processorDisclosure,
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  } catch {
    return noStoreJson(503, "authority_unavailable");
  } finally {
    try {
      await opened?.close();
    } catch (error) {
      console.error("Sarah shared-room authority store close failed", error);
    }
  }
};
