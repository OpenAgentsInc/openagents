import {
  COMMUNITY_MEMBERSHIP_RECORD_KINDS,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  communityRoleFor,
  foldCommunityLedgerFromEvents,
} from "@openagentsinc/sarah/community";
import { NostrEvent, type NostrEvent as NostrEventType } from "@openagentsinc/public-nostr-chat";
import { Array as EffectArray, Option, Order, Schema as S } from "effect";
import { verifyEvent } from "nostr-effect/pure";
import { createHash } from "node:crypto";

import { parseJsonUnknown } from "./json-boundary";

export const SARAH_LIVEKIT_COMMUNITY_AUTHORITY_SCHEMA =
  "openagents.sarah.livekit.community-authority.v1" as const;

const MAX_EVENTS = 4_096;
const MAX_EVENT_BYTES = 65_536;
const MAX_TOTAL_BYTES = 16 * 1_024 * 1_024;
const DEFAULT_TIMEOUT_MS = 5_000;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/u));
const CommunityAuthority = S.Struct({
  communityRef: Ref,
  channelRefs: S.Array(Ref).check(S.isMinLength(1), S.isMaxLength(64)),
  relayUrl: S.String.check(S.isMinLength(1), S.isMaxLength(2_048)),
  adminPubkeys: S.Array(Hex64).check(S.isMinLength(1), S.isMaxLength(16)),
});

const SarahLiveKitCommunityAuthorityConfig = S.Struct({
  schema: S.Literal(SARAH_LIVEKIT_COMMUNITY_AUTHORITY_SCHEMA),
  communities: S.Array(CommunityAuthority).check(S.isMinLength(1), S.isMaxLength(32)),
});

type CommunityAuthority = typeof CommunityAuthority.Type;

export type SarahLiveKitCommunityAccess = Readonly<{
  communityRef: string;
  channelRef: string;
  membershipRevision: string;
  publishAllowed: boolean;
  subscribeAllowed: boolean;
}>;

type SocketLike = Readonly<{
  readyState: number;
  close: (code?: number, reason?: string) => void;
  send: (data: string) => void;
  addEventListener: (
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ) => void;
}>;

const decodeNostrEvent = S.decodeUnknownOption(NostrEvent);

const hasExactGroup = (event: Pick<NostrEventType, "tags">, communityRef: string): boolean =>
  event.tags.some((tag) => tag[0] === "h" && tag[1] === communityRef);

const isVerifiedEvent = (event: NostrEventType): boolean =>
  verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) });

const parseRelayUrl = (value: string): string | undefined => {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "wss:" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== ""
    ) {
      return undefined;
    }
    return url.toString();
  } catch {
    return undefined;
  }
};

const parseAuthorityConfig = (
  value: string | undefined,
): ReadonlyArray<CommunityAuthority> | undefined => {
  if (value === undefined || value.trim() === "") return undefined;
  try {
    const decoded = S.decodeUnknownSync(SarahLiveKitCommunityAuthorityConfig)(
      parseJsonUnknown(value),
    );
    const communityRefs = new Set<string>();
    for (const community of decoded.communities) {
      if (
        communityRefs.has(community.communityRef) ||
        parseRelayUrl(community.relayUrl) === undefined ||
        new Set(community.channelRefs).size !== community.channelRefs.length ||
        new Set(community.adminPubkeys).size !== community.adminPubkeys.length
      ) {
        return undefined;
      }
      communityRefs.add(community.communityRef);
    }
    return decoded.communities;
  } catch {
    return undefined;
  }
};

export const readSarahLiveKitCommunityEvents = (
  authority: CommunityAuthority,
  options: Readonly<{
    makeWebSocket?: (url: string) => SocketLike;
    timeoutMs?: number;
  }> = {},
): Promise<ReadonlyArray<NostrEventType>> => {
  const relayUrl = parseRelayUrl(authority.relayUrl);
  if (relayUrl === undefined) return Promise.reject(new Error("community relay is invalid"));
  const makeWebSocket =
    options.makeWebSocket ??
    ((url: string): SocketLike => {
      const webSocket = new WebSocket(url);
      return {
        get readyState() {
          return webSocket.readyState;
        },
        close: (code, reason) => webSocket.close(code, reason),
        send: (data) => webSocket.send(data),
        addEventListener: (type, listener) => {
          if (type === "message") {
            webSocket.addEventListener("message", (event) => listener({ data: event.data }));
          } else {
            webSocket.addEventListener(type, () => listener({}));
          }
        },
      };
    });

  return new Promise((resolve, reject) => {
    const subscriptionId = "sarah-livekit-community-membership-v1";
    const events = new Map<string, NostrEventType>();
    let totalBytes = 0;
    let settled = false;
    let socket: SocketLike | undefined;
    const timer = setTimeout(
      () => finish(new Error("community relay timed out")),
      options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    );

    const finish = (error?: Error): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      let cleanupError: Error | undefined;
      try {
        if (socket?.readyState === 1) {
          socket.send(JSON.stringify(["CLOSE", subscriptionId]));
        }
      } catch {
        cleanupError = new Error("community relay subscription cleanup failed");
      }
      try {
        socket?.close(1_000, "membership-query-complete");
      } catch {
        cleanupError ??= new Error("community relay socket cleanup failed");
      }
      const finalError = error ?? cleanupError;
      if (finalError !== undefined) {
        reject(finalError);
      } else if (events.size >= MAX_EVENTS) {
        reject(new Error("community membership history may be truncated"));
      } else {
        resolve([...events.values()]);
      }
    };

    try {
      socket = makeWebSocket(relayUrl);
    } catch {
      clearTimeout(timer);
      reject(new Error("community relay is unavailable"));
      return;
    }

    socket.addEventListener("open", () => {
      try {
        socket?.send(
          JSON.stringify([
            "REQ",
            subscriptionId,
            {
              "#h": [authority.communityRef],
              kinds: [...COMMUNITY_MEMBERSHIP_RECORD_KINDS],
              limit: MAX_EVENTS,
            },
          ]),
        );
      } catch {
        finish(new Error("community relay subscription failed"));
      }
    });
    socket.addEventListener("message", (event) => {
      const raw = String(event.data ?? "");
      const frameBytes = new TextEncoder().encode(raw).byteLength;
      totalBytes += frameBytes;
      if (frameBytes > MAX_EVENT_BYTES || totalBytes > MAX_TOTAL_BYTES) {
        finish(new Error("community relay response exceeded its bound"));
        return;
      }
      let frame: unknown;
      try {
        frame = parseJsonUnknown(raw);
      } catch {
        return;
      }
      if (!Array.isArray(frame) || typeof frame[0] !== "string") return;
      if (frame[0] === "EOSE" && frame[1] === subscriptionId) {
        finish();
        return;
      }
      if (frame[0] === "CLOSED" && frame[1] === subscriptionId) {
        finish(new Error("community relay closed the membership query"));
        return;
      }
      if (frame[0] !== "EVENT" || frame[1] !== subscriptionId) return;
      const maybeEvent = decodeNostrEvent(frame[2]);
      if (Option.isNone(maybeEvent)) return;
      const signedEvent = maybeEvent.value;
      if (!isVerifiedEvent(signedEvent) || !hasExactGroup(signedEvent, authority.communityRef)) {
        return;
      }
      events.set(signedEvent.id, signedEvent);
    });
    socket.addEventListener("error", () => {
      finish(new Error("community relay is unavailable"));
    });
    socket.addEventListener("close", () => {
      finish(new Error("community relay disconnected before EOSE"));
    });
  });
};

const membershipRevision = (
  authority: CommunityAuthority,
  channelRef: string,
  ownerPubkeys: ReadonlyArray<string>,
  fold: ReturnType<typeof foldCommunityLedgerFromEvents>,
  events: ReadonlyArray<NostrEventType>,
): string => {
  const ownerPubkeySet = new Set(ownerPubkeys);
  const membershipEventIds = new Set(
    events
      .filter(
        (event) =>
          (event.kind === NIP_29_PUT_USER_KIND || event.kind === NIP_29_REMOVE_USER_KIND) &&
          authority.adminPubkeys.includes(event.pubkey) &&
          hasExactGroup(event, authority.communityRef) &&
          event.tags.some((tag) => tag[0] === "p" && ownerPubkeySet.has(tag[1] ?? "")),
      )
      .map((event) => event.id),
  );
  const appliedMembershipEventIds = fold.appliedEventIds.filter((eventId) =>
    membershipEventIds.has(eventId),
  );
  return createHash("sha256")
    .update(SARAH_LIVEKIT_COMMUNITY_AUTHORITY_SCHEMA)
    .update("\n")
    .update(authority.communityRef)
    .update("\n")
    .update(channelRef)
    .update("\n")
    .update(EffectArray.sort(ownerPubkeys, Order.String).join("\n"))
    .update("\n")
    .update(EffectArray.sort(authority.adminPubkeys, Order.String).join("\n"))
    .update("\n")
    .update(appliedMembershipEventIds.join("\n"))
    .digest("hex");
};

export const makeSarahLiveKitCommunityAccessResolver =
  <Bindings>(
    dependencies: Readonly<{
      authorityConfig: (env: Bindings) => string | undefined;
      readEvents?: (authority: CommunityAuthority) => Promise<ReadonlyArray<NostrEventType>>;
      resolveOwnerPubkeys: (env: Bindings, ownerUserId: string) => Promise<ReadonlyArray<string>>;
    }>,
  ) =>
  async (
    env: Bindings,
    input: Readonly<{
      ownerUserId: string;
      communityRef: string;
      channelRef: string;
    }>,
  ): Promise<SarahLiveKitCommunityAccess | undefined> => {
    const authorities = parseAuthorityConfig(dependencies.authorityConfig(env));
    const authority = authorities?.find(
      (candidate) => candidate.communityRef === input.communityRef,
    );
    if (authority === undefined || !authority.channelRefs.includes(input.channelRef)) {
      return undefined;
    }

    const ownerPubkeys = [
      ...new Set(
        (await dependencies.resolveOwnerPubkeys(env, input.ownerUserId))
          .map((pubkey) => pubkey.trim().toLowerCase())
          .filter((pubkey) => /^[0-9a-f]{64}$/u.test(pubkey)),
      ),
    ];
    if (ownerPubkeys.length === 0) return undefined;

    let events: ReadonlyArray<NostrEventType>;
    try {
      events = await (dependencies.readEvents ?? readSarahLiveKitCommunityEvents)(authority);
    } catch {
      return undefined;
    }
    if (events.length >= MAX_EVENTS) return undefined;

    const verifiedEvents = events.filter((event) => {
      try {
        return isVerifiedEvent(event) && hasExactGroup(event, authority.communityRef);
      } catch {
        return false;
      }
    });
    const fold = foldCommunityLedgerFromEvents({
      groupId: authority.communityRef,
      adminPubkeys: authority.adminPubkeys,
      events: verifiedEvents,
    });
    const active = ownerPubkeys.some(
      (pubkey) => communityRoleFor(fold, pubkey).status === "active",
    );
    if (!active) return undefined;

    return {
      communityRef: input.communityRef,
      channelRef: input.channelRef,
      membershipRevision: membershipRevision(
        authority,
        input.channelRef,
        ownerPubkeys,
        fold,
        verifiedEvents,
      ),
      publishAllowed: true,
      subscribeAllowed: true,
    };
  };
