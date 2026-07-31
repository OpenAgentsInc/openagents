import type { SarahLiveKitCommunityAccess } from "./sarah-livekit-community-access.js";
import {
  handleSarahLiveKitSharedRoomProductionRequest,
  SARAH_LIVEKIT_ROOM_SUMMON_PATH,
  SARAH_ROOM_DEVICE_REF_HEADER,
  type SarahLiveKitSharedRoomProductionDependencies,
} from "./sarah-livekit-room-authority-production.js";

/**
 * Opens the durable shared-room authority for a community voice session.
 *
 * This is the *internal caller* of the shared-room summon handler. It exists as
 * its own module, rather than as a closure inside the worker entrypoint, for one
 * reason: a caller that lives inside a 17k-line entrypoint cannot be tested
 * against the contract of the thing it calls, and that is exactly how it broke.
 *
 * The defect worth remembering: `94d49d8bab` added a `device_ref_required` gate
 * to the summon handler. Its tests exercised the handler directly and stayed
 * green, while this caller — which synthesized its request with only a
 * content-type header — could no longer satisfy the contract. Every community
 * session 503'd in production.
 *
 * The fix is to pass the device ref this request already authenticated, not to
 * synthesize one. A synthetic ref would not merely disable the gate for one
 * caller; it would bind the member's seat under a device that does not exist,
 * so the member's own later join would be refused `duplicate_participant`.
 */
export type SarahLiveKitCommunityRoomBootstrapInput = Readonly<{
  /** The authenticated owner this session belongs to. */
  ownerUserId: string;
  /**
   * The client device that already authenticated this request, verified
   * against `body.identity.deviceRef` by the voice-session route before the
   * bootstrap runs. Never synthesized here.
   */
  deviceRef: string;
  presenceLeaseRef: string;
  communityAccess: SarahLiveKitCommunityAccess;
}>;

export class SarahLiveKitCommunityRoomBootstrapError extends Error {
  readonly status: number;

  constructor(status: number, body: string) {
    super(`Sarah shared-room authority bootstrap failed (${status}): ${body}`);
    this.name = "SarahLiveKitCommunityRoomBootstrapError";
    this.status = status;
  }
}

/**
 * `requireUser` and `resolveCommunityAccess` are omitted deliberately: this
 * caller supplies both from the identity the session route already
 * authenticated, so accepting them would let a caller pass a subject that was
 * never checked.
 */
export type SarahLiveKitCommunityRoomBootstrapDependencies<Environment> = Omit<
  SarahLiveKitSharedRoomProductionDependencies<Environment, undefined>,
  "requireUser" | "resolveCommunityAccess"
>;

export const bootstrapSarahLiveKitCommunityRoom = async <Environment>(
  dependencies: SarahLiveKitCommunityRoomBootstrapDependencies<Environment>,
  environment: Environment,
  input: SarahLiveKitCommunityRoomBootstrapInput,
): Promise<void> => {
  const response = await handleSarahLiveKitSharedRoomProductionRequest(
    {
      ...dependencies,
      // The session route authenticated this owner already; re-deriving the
      // subject from a synthetic request would prove nothing.
      requireUser: async () => ({ userId: input.ownerUserId }),
      resolveCommunityAccess: async (
        _environment: Environment,
        requested: Readonly<{
          ownerUserId: string;
          communityRef: string;
          channelRef: string;
        }>,
      ) =>
        requested.ownerUserId === input.ownerUserId &&
        requested.communityRef === input.communityAccess.communityRef &&
        requested.channelRef === input.communityAccess.channelRef
          ? input.communityAccess
          : undefined,
    },
    "summon",
    new Request(`https://api.openagents.com${SARAH_LIVEKIT_ROOM_SUMMON_PATH}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        [SARAH_ROOM_DEVICE_REF_HEADER]: input.deviceRef,
      },
      body: JSON.stringify({ presenceLeaseRef: input.presenceLeaseRef }),
    }),
    environment,
    undefined,
  );
  if (!response.ok) {
    throw new SarahLiveKitCommunityRoomBootstrapError(
      response.status,
      (await response.text()).slice(0, 256),
    );
  }
};
