import { type SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { createHash } from "node:crypto";

import type { SarahLiveKitCommunityAccess } from "./sarah-livekit-community-access.js";
import {
  handleSarahLiveKitRoomMemberFloorRequest,
  handleSarahLiveKitRoomModeratorFloorRequest,
} from "./sarah-livekit-room-authority-routes.js";
import type { SarahLiveKitRoomMemberAccess } from "./sarah-livekit-room-authority.js";

export const SARAH_LIVEKIT_ROOM_MEMBER_FLOOR_PATH = "/api/sarah/livekit/room/floor/member" as const;
export const SARAH_LIVEKIT_ROOM_MODERATOR_FLOOR_PATH =
  "/api/sarah/livekit/room/floor/moderator" as const;

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
  now?: (() => number) | undefined;
}>;

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");

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
  mode: "member" | "moderator",
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
        if (
          input.userId !== authenticated.userId ||
          (input.targetUserRefDigest !== undefined && input.targetUserRefDigest !== userRefDigest)
        ) {
          return undefined;
        }
        const binding = await authorityStore.readParticipantBinding({
          presenceLeaseRef: input.presenceLeaseRef,
          ownerUserId: input.userId,
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
          userRefDigest,
          pubkey: access.memberPubkey,
          participantRef: binding.participantRef,
          mappedParticipantRef: binding.participantRef,
          membershipRevision: binding.membershipRevision,
          roomRef: binding.roomRef,
          roomEpoch: binding.roomEpoch,
          safetyIdentifier: digest(
            `sarah-livekit-room-safety\n${binding.roomRef}\n${authenticated.userId}`,
          ),
        };
      },
      now,
    } as const;
    return mode === "member"
      ? await handleSarahLiveKitRoomMemberFloorRequest(request, environment, routeDependencies)
      : await handleSarahLiveKitRoomModeratorFloorRequest(request, environment, routeDependencies);
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
