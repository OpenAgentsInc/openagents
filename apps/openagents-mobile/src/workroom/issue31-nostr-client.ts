import { makeAuthEvent } from "nostr-effect/nip42";
import { verifyEvent } from "nostr-effect/pure";

import {
  ISSUE31_HOST_ANNOUNCEMENT_KIND,
  ISSUE31_PAIRING_SCHEMA,
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  decodeIssue31AnyHostAnnouncement,
  isIssue31DeliveredAdjunct,
  unwrapIssue31PrivateGiftWrap,
  type Issue31AnyHostAnnouncement,
  type Issue31NostrSigner,
  type Issue31PrivateRecord,
  type Issue31SignedNostrEvent,
} from "@openagentsinc/sarah/issue31-nostr";
import {
  SARAH_AUTHORITY_RECEIPT_KIND,
  SARAH_TURN_RECORD_KIND,
} from "@openagentsinc/sarah/nostr-turn";
import {
  SARAH_ENGRAM_KIND,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
} from "@openagentsinc/sarah/nostr-memory";
import {
  NIP_29_GROUP_ADMINS_KIND,
  NIP_29_GROUP_CHAT_KIND,
  NIP_29_GROUP_MEMBERS_KIND,
  NIP_29_GROUP_METADATA_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  NIP_AP_MANAGED_INSTANCE_KIND,
  NIP_AP_PERSONA_KIND,
} from "@openagentsinc/sarah/community";
import { COMMUNITY_ARBITRATION_FEEDBACK_KIND } from "@openagentsinc/sarah/community-arbitration";
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
} from "@openagentsinc/sarah/lbr-request-quote";
import {
  XP_AWARD_KIND,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_PROFILE_BADGES_KIND,
  XP_RANK_KIND,
} from "@openagentsinc/sarah/xp";

export type Issue31NostrRoom = "discovery" | "owner_private" | "community";
export type Issue31RelayState =
  | "disconnected"
  | "connecting"
  | "replaying"
  | "live"
  | "retrying"
  | "exhausted";

export interface Issue31RelayCursor {
  readonly since: number;
  readonly eventIdsAtSince: ReadonlyArray<string>;
}

export interface Issue31RelayCursorStore {
  readonly load: (relayUrl: string, room: Issue31NostrRoom) => Promise<Issue31RelayCursor | null>;
  readonly save: (
    relayUrl: string,
    room: Issue31NostrRoom,
    cursor: Issue31RelayCursor,
  ) => Promise<void>;
}

export interface Issue31OutboundEventStore {
  readonly load: () => ReadonlyArray<Issue31SignedNostrEvent>;
  readonly put: (event: Issue31SignedNostrEvent) => void;
  readonly delete: (eventId: string) => void;
  readonly close: () => void;
}

export interface Issue31WebSocketLike {
  onopen: ((event: unknown) => void) | null;
  onmessage: ((event: Readonly<{ data: unknown }>) => void) | null;
  onerror: ((event: unknown) => void) | null;
  onclose: ((event: Readonly<{ code?: number; reason?: string }>) => void) | null;
  send(data: string): void;
  close(code?: number, reason?: string): void;
}

export interface Issue31NostrFilter {
  readonly kinds: ReadonlyArray<number>;
  readonly authors?: ReadonlyArray<string>;
  readonly since?: number;
  readonly "#p"?: ReadonlyArray<string>;
  readonly "#t"?: ReadonlyArray<string>;
  readonly "#h"?: ReadonlyArray<string>;
  readonly "#d"?: ReadonlyArray<string>;
}

export interface Issue31ConfirmedEvent {
  readonly relayUrl: string;
  readonly room: Issue31NostrRoom;
  readonly event: Issue31SignedNostrEvent;
  readonly canonicalRecordId: string;
  readonly privateRumorId: string | null;
  readonly privateRecord: Issue31PrivateRecord | null;
  readonly hostAnnouncement: Issue31AnyHostAnnouncement | null;
}

export interface Issue31RelaySnapshot {
  readonly relayUrl: string;
  readonly state: Issue31RelayState;
  readonly reconnectAttempt: number;
  /**
   * The oldest room's replay point, kept for compatibility.
   *
   * Do not read this as one room's freshness: it is `min` across rooms, so a
   * community room that has never synced drags the owner-private number
   * backwards and vice versa. Read {@link roomReplaySince} for a room.
   */
  readonly replaySince: number;
  /** Per-room replay point. The two rooms do not share a freshness number. */
  readonly roomReplaySince: Readonly<Record<Issue31NostrRoom, number>>;
  readonly gapReason:
    | "awaiting_eose"
    | "disconnect_before_eose"
    | "relay_unavailable"
    | "decode_failure"
    | "auth_failure"
    | "discovery_conflict"
    | "relay_notice"
    | null;
  readonly rejectedEventCount: number;
}

export interface Issue31NostrClientSnapshot {
  readonly devicePublicKeyHex: string | null;
  readonly admittedHostPublicKeys: ReadonlyArray<string>;
  readonly selectedHostPublicKeys: ReadonlyArray<string>;
  readonly ownerPrivateAuthors: ReadonlyArray<string>;
  readonly ownerRecipientPublicKeys: ReadonlyArray<string>;
  readonly relays: ReadonlyArray<Issue31RelaySnapshot>;
  readonly confirmedEvents: ReadonlyArray<Issue31ConfirmedEvent>;
  readonly storedEventIds: Readonly<Record<string, ReadonlyArray<string>>>;
  readonly publishRefusals: Readonly<Record<string, Readonly<Record<string, string>>>>;
}

export interface Issue31PublishReceipt {
  readonly eventId: string;
  readonly sentRelayUrls: ReadonlyArray<string>;
  readonly transportState: "queued" | "sent";
  readonly relayAcknowledgement: "pending";
  readonly commandCompletion: "pending_terminal_record";
}

export const ISSUE31_OWNER_PRIVATE_KINDS = [
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  SARAH_TURN_RECORD_KIND,
  SARAH_AUTHORITY_RECEIPT_KIND,
  SARAH_ENGRAM_KIND,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
] as const;

/**
 * Kinds scoped by the group's `h` tag rather than by author.
 *
 * Membership admin records and room chat are addressed to the group, not to a
 * key this device knows in advance — the whole point of the roster is to learn
 * who the members are. Admission authority still comes from the out-of-band
 * admin key set at fold time; subscribing to them is not admitting them.
 */
export const ISSUE31_COMMUNITY_GROUP_SCOPED_KINDS = [
  NIP_29_GROUP_CHAT_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  NIP_29_GROUP_METADATA_KIND,
  NIP_29_GROUP_ADMINS_KIND,
  NIP_29_GROUP_MEMBERS_KIND,
  // An agent's persona carries the attestation binding it to its operator. The
  // author is the agent key, which is exactly what the roster is for.
  NIP_AP_PERSONA_KIND,
  NIP_AP_MANAGED_INSTANCE_KIND,
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  // Arbitration decisions, dispute appeals, and owner rulings. Without this the
  // client filtered out the very events that carry a typed rejection reason and
  // its appeal destination.
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
] as const;

/** Scorer-published only. Author-scoped, so a self-labelled score never lands. */
export const ISSUE31_COMMUNITY_SCORER_KINDS = [
  XP_AWARD_KIND,
  XP_RANK_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_BADGE_AWARD_KIND,
  XP_PROFILE_BADGES_KIND,
] as const;

export const ISSUE31_COMMUNITY_KINDS = [
  ...ISSUE31_COMMUNITY_GROUP_SCOPED_KINDS,
  ...ISSUE31_COMMUNITY_SCORER_KINDS,
] as const;

const COMMUNITY_GROUP_SCOPED_KIND_SET: ReadonlyArray<number> = [
  ...ISSUE31_COMMUNITY_GROUP_SCOPED_KINDS,
];

const isCommunityGroupScopedKind = (kind: number): boolean =>
  COMMUNITY_GROUP_SCOPED_KIND_SET.includes(kind);

const HEX_64 = /^[0-9a-f]{64}$/;

const normalizeRelayUrl = (value: string): string => {
  const parsed = new URL(value);
  if (
    (parsed.protocol !== "wss:" && parsed.protocol !== "ws:") ||
    parsed.hostname === "" ||
    parsed.username !== "" ||
    parsed.password !== "" ||
    parsed.search !== "" ||
    parsed.hash !== ""
  ) {
    throw new Error("Issue 31 relay URL is unsafe.");
  }
  parsed.hash = "";
  return parsed.toString().replace(/\/$/, "");
};

const eventShape = (value: unknown): value is Issue31SignedNostrEvent => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const event = value as Readonly<Record<string, unknown>>;
  return (
    typeof event["id"] === "string" &&
    HEX_64.test(event["id"]) &&
    typeof event["pubkey"] === "string" &&
    HEX_64.test(event["pubkey"]) &&
    Number.isSafeInteger(event["created_at"]) &&
    (event["created_at"] as number) >= 0 &&
    (event["created_at"] as number) <= 8_640_000_000_000 &&
    typeof event["kind"] === "number" &&
    Number.isSafeInteger(event["kind"]) &&
    Array.isArray(event["tags"]) &&
    event["tags"].length <= 128 &&
    event["tags"].every(
      (tag) =>
        Array.isArray(tag) &&
        tag.length <= 16 &&
        tag.every((value) => typeof value === "string" && value.length <= 4_096),
    ) &&
    typeof event["content"] === "string" &&
    event["content"].length <= 524_288 &&
    typeof event["sig"] === "string" &&
    /^[0-9a-f]{128}$/.test(event["sig"])
  );
};

const tagValue = (event: Issue31SignedNostrEvent, name: string): string | null =>
  event.tags.find((tag) => tag[0] === name)?.[1] ?? null;

const replaceableCoordinate = (event: Issue31SignedNostrEvent): string | null => {
  if ((event.kind >= 10_000 && event.kind < 20_000) || event.kind === 0 || event.kind === 3) {
    return `${event.pubkey}:${event.kind}`;
  }
  if (event.kind >= 30_000 && event.kind < 40_000) {
    const dTag = tagValue(event, "d");
    return dTag === null ? null : `${event.pubkey}:${event.kind}:${dTag}`;
  }
  return null;
};

const newerEvent = (
  left: Issue31SignedNostrEvent,
  right: Issue31SignedNostrEvent,
): Issue31SignedNostrEvent =>
  left.created_at === right.created_at
    ? left.id > right.id
      ? left
      : right
    : left.created_at > right.created_at
      ? left
      : right;

const hostAnnouncementBindingFingerprint = (announcement: Issue31AnyHostAnnouncement): string =>
  JSON.stringify({
    hostRef: announcement.hostRef,
    hostPublicKeyHex: announcement.hostPublicKeyHex,
    sarahPublicKeyHex: announcement.sarahPublicKeyHex,
    displayName: announcement.displayName,
    protocols: [...announcement.protocols].sort(),
    relayUrls: [...announcement.relayUrls].sort(),
    conversation:
      announcement.schema === "openagents.omega.issue31.host_discovery.v2"
        ? announcement.conversation
        : null,
  });

const hostAnnouncementRecordFingerprint = (announcement: Issue31AnyHostAnnouncement): string =>
  JSON.stringify({
    binding: hostAnnouncementBindingFingerprint(announcement),
    generation: announcement.generation,
    issuedAt: announcement.issuedAt,
    expiresAt: announcement.expiresAt,
  });

const memoryCursorStore = (): Issue31RelayCursorStore => {
  const cursors = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: Issue31NostrRoom): string => `${relayUrl}\n${room}`;
  return {
    load: async (relayUrl, room) => cursors.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      cursors.set(key(relayUrl, room), cursor);
    },
  };
};

interface RelayRuntime {
  readonly relayUrl: string;
  socket: Issue31WebSocketLike | null;
  state: Issue31RelayState;
  reconnectAttempt: number;
  replaySince: number;
  gapReason: Issue31RelaySnapshot["gapReason"];
  stickyGapReason: "discovery_conflict" | "relay_notice" | null;
  locallyClosed: boolean;
  subscriptionRooms: Map<string, Issue31NostrRoom>;
  retiredSubscriptionIds: Set<string>;
  pendingEose: Set<string>;
  subscriptionEpoch: number;
  pendingAuthEventId: string | null;
  pendingPublishEventIds: Set<string>;
  publishRefusals: Map<string, string>;
  roomReplaySince: Map<Issue31NostrRoom, number>;
  cursorAdvances: Map<Issue31NostrRoom, Promise<void>>;
  rejectedEventIds: Set<string>;
}

interface OutboundPublish {
  readonly event: Issue31SignedNostrEvent;
  readonly serializedFrame: string;
  /**
   * Which room this pending publish belongs to.
   *
   * The queue bound used to be one number across both rooms, which is shared
   * optimistic state: a community room that queues its bound worth of messages
   * while offline would refuse the owner's next private message with a full
   * queue. Each room now gets its own budget.
   */
  readonly room: Issue31NostrRoom;
}

/**
 * The room an outbound event belongs to, from the event alone.
 *
 * Used when re-loading the persisted queue, which stores signed events rather
 * than our own envelope, so there is no room field to read.
 */
const outboundRoomForKind = (kind: number): Issue31NostrRoom =>
  (ISSUE31_COMMUNITY_KINDS as ReadonlyArray<number>).includes(kind)
    ? "community"
    : "owner_private";

export interface Issue31NostrClient {
  readonly start: () => Promise<void>;
  readonly close: () => void;
  readonly publish: (
    event: Issue31SignedNostrEvent,
    room?: Issue31NostrRoom,
  ) => Issue31PublishReceipt;
  readonly retryPublish: (eventId: string) => boolean;
  readonly discardPublish: (eventId: string) => boolean;
  readonly updateSubscriptionScope: (
    scope: Readonly<{
      selectedHostPublicKeys?: ReadonlyArray<string>;
      ownerAuthors?: ReadonlyArray<string>;
      ownerRecipientPublicKeys?: ReadonlyArray<string>;
      communityAuthors?: ReadonlyArray<string>;
      communityGroupIds?: ReadonlyArray<string>;
    }>,
  ) => Promise<void>;
  readonly snapshot: () => Issue31NostrClientSnapshot;
}

export const createIssue31NostrClient = (
  input: Readonly<{
    relayUrls: ReadonlyArray<string>;
    signer: Issue31NostrSigner;
    webSocket: new (url: string) => Issue31WebSocketLike;
    cursorStore?: Issue31RelayCursorStore;
    outboundStore?: Issue31OutboundEventStore;
    admittedHostPublicKeys?: ReadonlyArray<string>;
    selectedHostPublicKeys?: ReadonlyArray<string>;
    ownerRecipientPublicKeys?: ReadonlyArray<string>;
    communityGroupIds?: ReadonlyArray<string>;
    ownerAuthors?: ReadonlyArray<string>;
    communityAuthors?: ReadonlyArray<string>;
    initialReconnectDelayMs?: number;
    maxReconnectDelayMs?: number;
    maxReconnectAttempts?: number;
    eoseTimeoutMs?: number;
    maxQueuedEvents?: number;
    setTimer?: (callback: () => void, delayMs: number) => unknown;
    onSnapshot?: (snapshot: Issue31NostrClientSnapshot) => void;
  }>,
): Issue31NostrClient => {
  const relayUrls = [...new Set(input.relayUrls.map(normalizeRelayUrl))];
  if (relayUrls.length === 0 || relayUrls.length > 8) {
    throw new Error("Issue 31 client needs between one and eight relays.");
  }
  for (const authors of [
    input.admittedHostPublicKeys,
    input.selectedHostPublicKeys,
    input.ownerAuthors,
    input.ownerRecipientPublicKeys,
    input.communityAuthors,
  ]) {
    if (
      authors !== undefined &&
      (authors.length > 16 || authors.some((value) => !HEX_64.test(value)))
    ) {
      throw new Error("Issue 31 subscription authors are invalid.");
    }
  }
  if (
    input.communityGroupIds !== undefined &&
    (input.communityGroupIds.length > 16 ||
      input.communityGroupIds.some(
        (value) => value.length < 1 || value.length > 128 || /\s/.test(value),
      ))
  ) {
    throw new Error("Issue 31 community group filters are invalid.");
  }
  const cursorStore = input.cursorStore ?? memoryCursorStore();
  const admittedHostPublicKeys = new Set(input.admittedHostPublicKeys ?? []);
  const ownerAuthors = new Set(input.ownerAuthors ?? []);
  const ownerRecipientPublicKeys = new Set(input.ownerRecipientPublicKeys ?? []);
  const selectedHostPublicKeys = new Set(input.selectedHostPublicKeys ?? input.ownerAuthors ?? []);
  const communityAuthors = new Set(input.communityAuthors ?? []);
  const communityGroupIds = new Set(input.communityGroupIds ?? []);
  const initialReconnectDelayMs = input.initialReconnectDelayMs ?? 500;
  const maxReconnectDelayMs = input.maxReconnectDelayMs ?? 15_000;
  const maxReconnectAttempts = input.maxReconnectAttempts ?? 8;
  const eoseTimeoutMs = input.eoseTimeoutMs ?? 10_000;
  const maxQueuedEvents = input.maxQueuedEvents ?? 256;
  if (!Number.isSafeInteger(maxQueuedEvents) || maxQueuedEvents < 1 || maxQueuedEvents > 1_024) {
    throw new Error("Issue 31 outbound queue bound is invalid.");
  }
  const setTimer = input.setTimer ?? ((callback, delayMs) => setTimeout(callback, delayMs));
  const relays = new Map<string, RelayRuntime>();
  const eventsById = new Map<string, Issue31ConfirmedEvent>();
  const replaceableByCoordinate = new Map<string, string>();
  const privateEventByRumorId = new Map<string, string>();
  /**
   * Which `(grantRef, hostRef)` pairs each host key has actually granted to
   * THIS device, learned from the confirmed pairing chain (omega#49).
   *
   * The envelope binds a delivered omega#47 adjunct to the host key that
   * signed the seal and to the device that unwrapped it. It cannot bind the
   * `hostRef` *inside* the body: a seal proves who signed, not which host the
   * body describes. So an admitted host could otherwise deliver a snapshot
   * labelled with a second host's `hostRef`, and a device paired to both would
   * bind it to the wrong grant.
   *
   * Only the pairing chain states that relation, so it is the authority here.
   */
  const grantedRefsByHostKey = new Map<string, Set<string>>();
  const grantBindingKey = (grantRef: string, hostRef: string): string =>
    `${grantRef} ${hostRef}`;
  /**
   * True when a delivered adjunct does not contradict the pairing chain.
   *
   * Absence of evidence is deliberately not a refusal. A relay replay resumes
   * from a cursor already past the grant records, so after a restart a device
   * legitimately holds a live grant it will not be served again; refusing every
   * adjunct in that state would turn a working pairing into a permanently blank
   * Full Auto section. What is refused is a *contradiction*: this host has
   * granted this device something, and the adjunct names a different grant or
   * labels itself with a different host.
   */
  const adjunctDeliveryContradictsGrant = (record: Issue31PrivateRecord): boolean => {
    if (!isIssue31DeliveredAdjunct(record)) return false;
    const granted = grantedRefsByHostKey.get(record.hostPublicKeyHex);
    if (granted === undefined || granted.size === 0) return false;
    return !granted.has(grantBindingKey(record.grantRef, record.hostRef));
  };
  const rememberGrantBinding = (record: Issue31PrivateRecord): void => {
    if (
      record.schema !== ISSUE31_PAIRING_SCHEMA ||
      record.devicePublicKeyHex !== localPublicKeyHex ||
      (record.recordType !== "scoped_grant" &&
        record.recordType !== "grant_renewal" &&
        record.recordType !== "grant_revocation")
    ) {
      return;
    }
    const granted = grantedRefsByHostKey.get(record.hostPublicKeyHex) ?? new Set<string>();
    granted.add(grantBindingKey(record.grantRef, record.hostRef));
    grantedRefsByHostKey.set(record.hostPublicKeyHex, granted);
  };
  const storedByRelay = new Map<string, Set<string>>();
  const outboundPublishes = new Map<string, OutboundPublish>();
  for (const event of input.outboundStore?.load() ?? []) {
    if (!eventShape(event) || !verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
      throw new Error("Issue 31 persisted outbound event is invalid.");
    }
    outboundPublishes.set(event.id, {
      event,
      serializedFrame: JSON.stringify(["EVENT", event]),
      room: outboundRoomForKind(event.kind),
    });
  }
  const queuedInRoom = (room: Issue31NostrRoom): number =>
    [...outboundPublishes.values()].filter((pending) => pending.room === room).length;
  for (const room of ["owner_private", "community"] as const) {
    if (queuedInRoom(room) > maxQueuedEvents) {
      throw new Error("Issue 31 persisted outbound queue exceeds its bound.");
    }
  }
  let closed = false;
  let localPublicKeyHex = "";

  const snapshot = (): Issue31NostrClientSnapshot => ({
    devicePublicKeyHex: localPublicKeyHex === "" ? null : localPublicKeyHex,
    admittedHostPublicKeys: [...admittedHostPublicKeys].sort(),
    selectedHostPublicKeys: [...selectedHostPublicKeys].sort(),
    ownerPrivateAuthors: [...ownerAuthors].sort(),
    ownerRecipientPublicKeys: [...ownerRecipientPublicKeys].sort(),
    relays: [...relays.values()]
      .map((relay) => ({
        relayUrl: relay.relayUrl,
        state: relay.state,
        reconnectAttempt: relay.reconnectAttempt,
        replaySince: relay.replaySince,
        roomReplaySince: {
          discovery: relay.roomReplaySince.get("discovery") ?? 0,
          owner_private: relay.roomReplaySince.get("owner_private") ?? 0,
          community: relay.roomReplaySince.get("community") ?? 0,
        },
        gapReason: relay.gapReason,
        rejectedEventCount: relay.rejectedEventIds.size,
      }))
      .sort((left, right) => left.relayUrl.localeCompare(right.relayUrl)),
    // Filtered here rather than at admission on purpose: a relay may serve the
    // adjunct before the grant that entitles it, so the judgement has to be
    // re-made against everything the device knows *now* instead of frozen at
    // the moment one event happened to arrive.
    confirmedEvents: [...eventsById.values()]
      .filter(
        (confirmed) =>
          confirmed.privateRecord === null ||
          !adjunctDeliveryContradictsGrant(confirmed.privateRecord),
      )
      .sort(
        (left, right) =>
          left.event.created_at - right.event.created_at ||
          left.event.id.localeCompare(right.event.id),
      ),
    storedEventIds: Object.fromEntries(
      [...storedByRelay]
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([relayUrl, ids]) => [relayUrl, [...ids].sort()]),
    ),
    publishRefusals: Object.fromEntries(
      [...relays.values()]
        .filter((relay) => relay.publishRefusals.size > 0)
        .sort((left, right) => left.relayUrl.localeCompare(right.relayUrl))
        .map((relay) => [relay.relayUrl, Object.fromEntries(relay.publishRefusals)]),
    ),
  });

  const notify = (): void => input.onSnapshot?.(snapshot());

  const markGap = (relay: RelayRuntime, reason: NonNullable<RelayRuntime["gapReason"]>): void => {
    relay.gapReason = reason;
    if (reason === "discovery_conflict" || reason === "relay_notice") {
      relay.stickyGapReason = reason;
    }
    notify();
  };

  const closeForProtocolError = (
    relay: RelayRuntime,
    reason: "decode_failure" | "auth_failure",
  ): void => {
    markGap(relay, reason);
    relay.socket?.close(1002, `issue31_${reason}`);
  };

  const persistCursor = async (
    relay: RelayRuntime,
    room: Issue31NostrRoom,
    event: Issue31SignedNostrEvent,
  ): Promise<void> => {
    const prior = relay.cursorAdvances.get(room) ?? Promise.resolve();
    const advance = prior.then(async () => {
      const current = await cursorStore.load(relay.relayUrl, room);
      const cursor =
        current === null || event.created_at > current.since
          ? { since: event.created_at, eventIdsAtSince: [event.id] }
          : event.created_at === current.since
            ? {
                since: current.since,
                eventIdsAtSince: [...new Set([...current.eventIdsAtSince, event.id])]
                  .sort()
                  .slice(-4),
              }
            : current;
      await cursorStore.save(relay.relayUrl, room, cursor);
    });
    relay.cursorAdvances.set(room, advance);
    try {
      await advance;
    } finally {
      if (relay.cursorAdvances.get(room) === advance) relay.cursorAdvances.delete(room);
    }
  };

  const rejectEvent = (relay: RelayRuntime, eventId: string): void => {
    if (!relay.rejectedEventIds.has(eventId)) {
      if (relay.rejectedEventIds.size >= 512) {
        const oldest = relay.rejectedEventIds.values().next().value;
        if (typeof oldest === "string") relay.rejectedEventIds.delete(oldest);
      }
      relay.rejectedEventIds.add(eventId);
    }
    notify();
  };

  const admitEvent = async (
    relay: RelayRuntime,
    room: Issue31NostrRoom,
    event: Issue31SignedNostrEvent,
  ): Promise<void> => {
    if (!verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
      rejectEvent(relay, event.id);
      return;
    }
    let hostAnnouncement: Issue31AnyHostAnnouncement | null = null;
    let privateRecord: Issue31PrivateRecord | null = null;
    let privateRumorId: string | null = null;
    if (room === "discovery") {
      if (event.kind !== ISSUE31_HOST_ANNOUNCEMENT_KIND) throw new Error("wrong discovery kind");
      if (!admittedHostPublicKeys.has(event.pubkey)) {
        rejectEvent(relay, event.id);
        return;
      }
      hostAnnouncement = decodeIssue31AnyHostAnnouncement(JSON.parse(event.content) as unknown);
      if (
        event.pubkey !== hostAnnouncement.hostPublicKeyHex ||
        tagValue(event, "d") !== hostAnnouncement.hostRef ||
        tagValue(event, "t") !== "omega-issue31-host" ||
        !event.tags.some((tag) => tag[0] === "k" && tag[1] === "1059")
      ) {
        throw new Error("host discovery identity mismatch");
      }
    } else if (event.kind === ISSUE31_PRIVATE_GIFT_WRAP_KIND) {
      if (room !== "owner_private") throw new Error("private event crossed room boundary");
      if (!event.tags.some((tag) => tag[0] === "p" && tag[1] === localPublicKeyHex)) {
        rejectEvent(relay, event.id);
        return;
      }
      let unwrapped: Awaited<ReturnType<typeof unwrapIssue31PrivateGiftWrap>>;
      try {
        unwrapped = await unwrapIssue31PrivateGiftWrap({
          signer: input.signer,
          giftWrap: event,
          requireIssue31Record: false,
        });
      } catch {
        rejectEvent(relay, event.id);
        return;
      }
      privateRecord = unwrapped.record;
      privateRumorId = unwrapped.rumor.id;
      if (
        (privateRecord === null && !ownerAuthors.has(unwrapped.rumor.pubkey)) ||
        (privateRecord !== null &&
          (!admittedHostPublicKeys.has(privateRecord.hostPublicKeyHex) ||
            !selectedHostPublicKeys.has(privateRecord.hostPublicKeyHex)))
      ) {
        rejectEvent(relay, event.id);
        return;
      }
      if (privateEventByRumorId.has(privateRumorId)) {
        await persistCursor(relay, room, event);
        return;
      }
    } else if (room === "owner_private") {
      const ownerRecipientTags = event.tags.filter((tag) => tag[0] === "p");
      if (
        !(ISSUE31_OWNER_PRIVATE_KINDS as ReadonlyArray<number>).includes(event.kind) ||
        !ownerAuthors.has(event.pubkey) ||
        ownerRecipientTags.length !== 1 ||
        !ownerRecipientPublicKeys.has(ownerRecipientTags[0]?.[1] ?? "")
      ) {
        rejectEvent(relay, event.id);
        return;
      }
    } else if (room === "community") {
      const correctKind = (ISSUE31_COMMUNITY_KINDS as ReadonlyArray<number>).includes(event.kind);
      // Group-scoped kinds are admitted by the group they name, because the
      // authors are exactly what the roster is for: a device cannot know every
      // member key, agent key, or provider key in advance. Admission authority
      // is still the out-of-band admin set, applied when the record is folded —
      // reading a record is not admitting its claim.
      const namesThisGroup = event.tags.some(
        (tag) => (tag[0] === "h" || tag[0] === "d") && communityGroupIds.has(tag[1] ?? ""),
      );
      const correctAuthority = isCommunityGroupScopedKind(event.kind)
        ? namesThisGroup
        : // Scorer-published kinds stay author-scoped: only OpenAgents scorer
          // keys publish awards and rank, and a member self-labelling their own
          // score should not even be stored.
          communityAuthors.has(event.pubkey);
      if (!correctKind || !correctAuthority) {
        rejectEvent(relay, event.id);
        return;
      }
    }
    if (eventsById.has(event.id)) {
      await persistCursor(relay, room, event);
      return;
    }

    const coordinate = replaceableCoordinate(event);
    if (room === "discovery" && coordinate !== null && hostAnnouncement !== null) {
      const currentId = replaceableByCoordinate.get(coordinate);
      const current = currentId === undefined ? undefined : eventsById.get(currentId);
      if (current !== undefined) {
        const currentAnnouncement = current.hostAnnouncement;
        if (currentAnnouncement === null) throw new Error("discovery coordinate is corrupted");
        if (
          current.event.id === event.id ||
          hostAnnouncement.generation < currentAnnouncement.generation
        ) {
          await persistCursor(relay, room, event);
          return;
        }
        if (hostAnnouncement.generation === currentAnnouncement.generation) {
          const sameBinding =
            hostAnnouncementBindingFingerprint(hostAnnouncement) ===
            hostAnnouncementBindingFingerprint(currentAnnouncement);
          if (!sameBinding) {
            markGap(relay, "discovery_conflict");
            await persistCursor(relay, room, event);
            return;
          }
          if (hostAnnouncement.issuedAt < currentAnnouncement.issuedAt) {
            await persistCursor(relay, room, event);
            return;
          }
          if (hostAnnouncement.issuedAt === currentAnnouncement.issuedAt) {
            if (
              hostAnnouncementRecordFingerprint(hostAnnouncement) !==
              hostAnnouncementRecordFingerprint(currentAnnouncement)
            ) {
              markGap(relay, "discovery_conflict");
              await persistCursor(relay, room, event);
              return;
            }
            if (newerEvent(current.event, event).id === current.event.id) {
              await persistCursor(relay, room, event);
              return;
            }
          }
        }
        eventsById.delete(current.event.id);
        if (
          hostAnnouncement.generation > currentAnnouncement.generation &&
          relay.stickyGapReason === "discovery_conflict"
        ) {
          relay.stickyGapReason = null;
          relay.gapReason = relay.pendingEose.size > 0 ? "awaiting_eose" : null;
        }
      }
      replaceableByCoordinate.set(coordinate, event.id);
    } else if (coordinate !== null) {
      const currentId = replaceableByCoordinate.get(coordinate);
      const current = currentId === undefined ? undefined : eventsById.get(currentId);
      if (current !== undefined && newerEvent(current.event, event).id === current.event.id) {
        await persistCursor(relay, room, event);
        return;
      }
      if (currentId !== undefined) eventsById.delete(currentId);
      replaceableByCoordinate.set(coordinate, event.id);
    }
    if (privateRecord !== null) rememberGrantBinding(privateRecord);
    eventsById.set(event.id, {
      relayUrl: relay.relayUrl,
      room,
      event,
      canonicalRecordId: privateRumorId ?? event.id,
      privateRumorId,
      privateRecord,
      hostAnnouncement,
    });
    if (privateRumorId !== null) privateEventByRumorId.set(privateRumorId, event.id);
    await persistCursor(relay, room, event);
    notify();
  };

  /**
   * How far below the cursor a gift wrap may legitimately land.
   *
   * NIP-59 *requires* the wrap's `created_at` to be randomized into the past —
   * that is the point of it, so a relay operator cannot use timestamps to infer
   * who is talking to whom and when. The relay filters on the wrap's timestamp,
   * not the sealed rumor's, so a strictly monotonic `since` silently drops any
   * wrap whose randomized time falls below the newest one already admitted.
   *
   * That failure is invisible on both sides. The host publishes and gets an
   * `OK`; the device's subscription simply never mentions the event; there is no
   * gap to report and nothing to quarantine. It is also nondeterministic — the
   * same code pairs successfully whenever the roll happens to land above the
   * cursor — so it presents as a flaky device rather than a protocol error, and
   * it gets *worse* the longer a device runs, because the cursor only rises.
   *
   * Asking for the full backdating window and relying on id-level idempotence
   * is the standard NIP-59 subscription shape. It costs a bounded replay on
   * reconnect and buys delivery that does not depend on a dice roll.
   */
  const NIP59_MAX_BACKDATE_SECONDS = 2 * 24 * 60 * 60;

  /**
   * Widen any gift-wrap-bearing filter to cover the backdating window.
   *
   * Applied by kind rather than by room so a future private lane inherits it
   * instead of reintroducing the same silent hole.
   */
  const allowGiftWrapBackdating = (
    filter: Issue31NostrFilter,
  ): Issue31NostrFilter =>
    (filter.kinds ?? []).includes(ISSUE31_PRIVATE_GIFT_WRAP_KIND)
      ? { ...filter, since: Math.max(0, (filter.since ?? 0) - NIP59_MAX_BACKDATE_SECONDS) }
      : filter;

  const filtersFor = (room: Issue31NostrRoom, since: number): ReadonlyArray<Issue31NostrFilter> => {
    if (room === "discovery") {
      return [
        {
          kinds: [ISSUE31_HOST_ANNOUNCEMENT_KIND],
          authors: [...admittedHostPublicKeys],
          since,
          "#t": ["omega-issue31-host"],
        },
      ];
    }
    if (room === "owner_private") {
      const privateFilter: Issue31NostrFilter = {
        kinds: [ISSUE31_PRIVATE_GIFT_WRAP_KIND],
        since,
        "#p": [localPublicKeyHex],
      };
      if (ownerAuthors.size === 0 || ownerRecipientPublicKeys.size === 0) {
        return [privateFilter];
      }
      return [
        privateFilter,
        {
          kinds: ISSUE31_OWNER_PRIVATE_KINDS.filter(
            (kind) => kind !== ISSUE31_PRIVATE_GIFT_WRAP_KIND,
          ),
          authors: [...ownerAuthors],
          since,
          "#p": [...ownerRecipientPublicKeys],
        },
      ];
    }
    const filters: Issue31NostrFilter[] = [];
    if (communityGroupIds.size > 0) {
      // The transcript, the roster, the membership admin records, the work-unit
      // lane, and the arbitration lane all address the group rather than a key
      // this device already knows. One `#h` filter carries all of them.
      filters.push({
        kinds: COMMUNITY_GROUP_SCOPED_KIND_SET,
        since,
        "#h": [...communityGroupIds],
      });
      // Relay-signed group state is addressable by `d`, not `h`.
      filters.push({
        kinds: [
          NIP_29_GROUP_METADATA_KIND,
          NIP_29_GROUP_ADMINS_KIND,
          NIP_29_GROUP_MEMBERS_KIND,
        ],
        since,
        "#d": [...communityGroupIds],
      });
    }
    if (communityAuthors.size > 0) {
      filters.push({
        kinds: ISSUE31_COMMUNITY_KINDS.filter((kind) => !isCommunityGroupScopedKind(kind)),
        authors: [...communityAuthors],
        since,
      });
    }
    return filters;
  };

  const retireSubscription = (relay: RelayRuntime, subscriptionId: string): void => {
    relay.retiredSubscriptionIds.add(subscriptionId);
    while (relay.retiredSubscriptionIds.size > 128) {
      const oldest = relay.retiredSubscriptionIds.values().next().value;
      if (typeof oldest === "string") relay.retiredSubscriptionIds.delete(oldest);
    }
  };

  const sendSubscriptions = async (relay: RelayRuntime): Promise<void> => {
    const socket = relay.socket;
    if (socket === null) return;
    for (const subscriptionId of relay.subscriptionRooms.keys()) {
      socket.send(JSON.stringify(["CLOSE", subscriptionId]));
      retireSubscription(relay, subscriptionId);
    }
    relay.subscriptionRooms.clear();
    relay.pendingEose.clear();
    relay.subscriptionEpoch += 1;
    const subscriptionEpoch = relay.subscriptionEpoch;
    for (const room of ["discovery", "owner_private", "community"] as const) {
      const cursor = await cursorStore.load(relay.relayUrl, room);
      if (relay.socket !== socket || relay.subscriptionEpoch !== subscriptionEpoch) return;
      const replaySince = Math.max(0, (cursor?.since ?? 0) - 1);
      const filters = filtersFor(room, replaySince).map(allowGiftWrapBackdating);
      // Report the point actually asked for, not the cursor. A room carrying
      // gift wraps genuinely replays from further back, and saying otherwise
      // would make the widened window invisible to anyone reading freshness.
      relay.roomReplaySince.set(
        room,
        filters.length === 0
          ? replaySince
          : Math.min(...filters.map((filter) => filter.since ?? 0)),
      );
      filters.forEach((filter, index) => {
        const subscriptionId = `issue31-${subscriptionEpoch}-${room}-${index}`;
        relay.subscriptionRooms.set(subscriptionId, room);
        relay.pendingEose.add(subscriptionId);
        socket.send(JSON.stringify(["REQ", subscriptionId, filter]));
      });
    }
    relay.replaySince = Math.min(...relay.roomReplaySince.values());
    if (relay.pendingEose.size === 0) {
      relay.state = "live";
      relay.gapReason = null;
      notify();
      return;
    }
    const expectedSocket = socket;
    setTimer(() => {
      if (
        relay.socket === expectedSocket &&
        relay.subscriptionEpoch === subscriptionEpoch &&
        relay.pendingEose.size > 0 &&
        !relay.locallyClosed &&
        !closed
      ) {
        relay.gapReason = "disconnect_before_eose";
        relay.socket.close(1002, "issue31_eose_timeout");
      }
    }, eoseTimeoutMs);
  };

  const sendQueuedPublishes = (relay: RelayRuntime): void => {
    const socket = relay.socket;
    if (socket === null) return;
    const stored = storedByRelay.get(relay.relayUrl);
    for (const [eventId, outbound] of outboundPublishes) {
      if (stored?.has(eventId) === true || relay.publishRefusals.has(eventId)) continue;
      socket.send(outbound.serializedFrame);
      relay.pendingPublishEventIds.add(eventId);
    }
  };

  const removeStoredPublish = (eventId: string): void => {
    const storedEverywhere = [...relays.values()].every(
      (relay) => storedByRelay.get(relay.relayUrl)?.has(eventId) === true,
    );
    if (!storedEverywhere) return;
    outboundPublishes.delete(eventId);
    input.outboundStore?.delete(eventId);
    for (const relay of relays.values()) relay.pendingPublishEventIds.delete(eventId);
  };

  const scheduleReconnect = (relay: RelayRuntime): void => {
    if (closed || relay.locallyClosed) return;
    if (relay.reconnectAttempt >= maxReconnectAttempts) {
      relay.state = "exhausted";
      relay.gapReason = "relay_unavailable";
      notify();
      return;
    }
    const delay = Math.min(
      initialReconnectDelayMs * 2 ** relay.reconnectAttempt,
      maxReconnectDelayMs,
    );
    relay.reconnectAttempt += 1;
    relay.state = "retrying";
    notify();
    setTimer(() => {
      if (!closed && !relay.locallyClosed) void connectRelay(relay);
    }, delay);
  };

  const handleFrame = async (relay: RelayRuntime, data: unknown): Promise<void> => {
    let frame: unknown;
    try {
      const serialized = String(data);
      if (serialized.length > 1_048_576) {
        closeForProtocolError(relay, "decode_failure");
        return;
      }
      frame = JSON.parse(serialized) as unknown;
    } catch {
      closeForProtocolError(relay, "decode_failure");
      return;
    }
    if (!Array.isArray(frame) || typeof frame[0] !== "string") {
      closeForProtocolError(relay, "decode_failure");
      return;
    }
    if (frame[0] === "AUTH") {
      if (typeof frame[1] !== "string" || frame[1].length < 1 || frame[1].length > 512) {
        closeForProtocolError(relay, "auth_failure");
        return;
      }
      try {
        const authTemplate = makeAuthEvent(relay.relayUrl, frame[1]);
        const auth = await input.signer.signEvent({
          ...authTemplate,
          created_at: Math.floor(Date.now() / 1_000),
        });
        relay.pendingAuthEventId = auth.id;
        relay.socket?.send(JSON.stringify(["AUTH", auth]));
      } catch {
        closeForProtocolError(relay, "auth_failure");
      }
      return;
    }
    if (frame[0] === "EOSE") {
      if (typeof frame[1] === "string" && relay.retiredSubscriptionIds.has(frame[1])) return;
      if (typeof frame[1] !== "string" || !relay.subscriptionRooms.has(frame[1])) {
        closeForProtocolError(relay, "decode_failure");
        return;
      }
      relay.pendingEose.delete(frame[1]);
      if (relay.pendingEose.size === 0) {
        relay.state = "live";
        relay.gapReason = relay.stickyGapReason;
        relay.reconnectAttempt = 0;
      }
      notify();
      return;
    }
    if (frame[0] === "OK") {
      if (typeof frame[1] !== "string" || !HEX_64.test(frame[1]) || typeof frame[2] !== "boolean") {
        closeForProtocolError(relay, "decode_failure");
        return;
      }
      const eventId = frame[1];
      const accepted = frame[2];
      const message = typeof frame[3] === "string" ? frame[3] : "relay_refused";
      if (relay.pendingAuthEventId === eventId) {
        relay.pendingAuthEventId = null;
        if (!accepted) {
          closeForProtocolError(relay, "auth_failure");
          return;
        }
        await sendSubscriptions(relay);
        sendQueuedPublishes(relay);
        return;
      }
      if (!relay.pendingPublishEventIds.has(eventId)) return;
      relay.pendingPublishEventIds.delete(eventId);
      if (accepted) {
        const stored = storedByRelay.get(relay.relayUrl) ?? new Set<string>();
        stored.add(eventId);
        storedByRelay.set(relay.relayUrl, stored);
      } else {
        relay.publishRefusals.set(eventId, message.slice(0, 256));
      }
      removeStoredPublish(eventId);
      notify();
      return;
    }
    if (frame[0] === "EVENT") {
      const subscriptionId = frame[1];
      const event = frame[2];
      if (typeof subscriptionId === "string" && relay.retiredSubscriptionIds.has(subscriptionId)) {
        return;
      }
      const room =
        typeof subscriptionId === "string"
          ? relay.subscriptionRooms.get(subscriptionId)
          : undefined;
      if (room === undefined || !eventShape(event)) {
        closeForProtocolError(relay, "decode_failure");
        return;
      }
      try {
        await admitEvent(relay, room, event);
      } catch {
        rejectEvent(relay, event.id);
      }
      return;
    }
    if (frame[0] === "CLOSED") {
      const subscriptionId = frame[1];
      if (typeof subscriptionId === "string" && relay.retiredSubscriptionIds.has(subscriptionId)) {
        return;
      }
      if (typeof subscriptionId === "string" && relay.subscriptionRooms.has(subscriptionId)) {
        markGap(relay, "relay_unavailable");
        relay.socket?.close(1002, "issue31_subscription_closed");
      }
      return;
    }
    if (frame[0] === "NOTICE") {
      if (typeof frame[1] !== "string" || frame[1].length > 512) {
        closeForProtocolError(relay, "decode_failure");
        return;
      }
      markGap(relay, "relay_notice");
    }
  };

  const connectRelay = async (relay: RelayRuntime): Promise<void> => {
    relay.state = "connecting";
    if (relay.stickyGapReason === "relay_notice") relay.stickyGapReason = null;
    relay.gapReason = "awaiting_eose";
    notify();
    let socket: Issue31WebSocketLike;
    try {
      socket = new input.webSocket(relay.relayUrl);
    } catch {
      relay.state = "disconnected";
      relay.gapReason = "relay_unavailable";
      scheduleReconnect(relay);
      return;
    }
    relay.socket = socket;
    socket.onopen = () => {
      relay.state = "replaying";
      relay.gapReason = "awaiting_eose";
      void sendSubscriptions(relay)
        .then(() => sendQueuedPublishes(relay))
        .catch(() => closeForProtocolError(relay, "decode_failure"));
      notify();
    };
    socket.onmessage = (event) => {
      void handleFrame(relay, event.data);
    };
    socket.onerror = () => {
      if (relay.gapReason === "awaiting_eose") relay.gapReason = "disconnect_before_eose";
      notify();
    };
    socket.onclose = () => {
      relay.socket = null;
      if (relay.locallyClosed || closed) return;
      relay.state = "disconnected";
      if (relay.gapReason === null || relay.gapReason === "awaiting_eose") {
        relay.gapReason =
          relay.gapReason === "awaiting_eose" ? "disconnect_before_eose" : "relay_unavailable";
      }
      notify();
      scheduleReconnect(relay);
    };
  };

  const start = async (): Promise<void> => {
    if (relays.size > 0) return;
    localPublicKeyHex = await input.signer.getPublicKey();
    if (!HEX_64.test(localPublicKeyHex)) throw new Error("Issue 31 signer public key is invalid.");
    for (const relayUrl of relayUrls) {
      const relay: RelayRuntime = {
        relayUrl,
        socket: null,
        state: "disconnected",
        reconnectAttempt: 0,
        replaySince: 0,
        gapReason: "awaiting_eose",
        stickyGapReason: null,
        locallyClosed: false,
        subscriptionRooms: new Map(),
        retiredSubscriptionIds: new Set(),
        pendingEose: new Set(),
        subscriptionEpoch: 0,
        pendingAuthEventId: null,
        pendingPublishEventIds: new Set(),
        publishRefusals: new Map(),
        roomReplaySince: new Map(),
        cursorAdvances: new Map(),
        rejectedEventIds: new Set(),
      };
      relays.set(relayUrl, relay);
      await connectRelay(relay);
    }
  };

  const close = (): void => {
    closed = true;
    for (const relay of relays.values()) {
      relay.locallyClosed = true;
      for (const subscriptionId of relay.subscriptionRooms.keys()) {
        relay.socket?.send(JSON.stringify(["CLOSE", subscriptionId]));
      }
      relay.socket?.close(1000, "issue31_client_close");
      relay.socket = null;
      relay.state = "disconnected";
    }
    notify();
  };

  const publish = (
    event: Issue31SignedNostrEvent,
    room: Issue31NostrRoom = outboundRoomForKind(event.kind),
  ): Issue31PublishReceipt => {
    if (!eventShape(event) || !verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
      throw new Error("Issue 31 publish event is invalid.");
    }
    const serializedFrame = JSON.stringify(["EVENT", event]);
    const existing = outboundPublishes.get(event.id);
    if (existing !== undefined && existing.serializedFrame !== serializedFrame) {
      throw new Error("Issue 31 publish identifier has conflicting signed event bytes.");
    }
    if (existing === undefined) {
      // Per-room bound: one room filling its queue never refuses the other's
      // next publish.
      if (queuedInRoom(room) >= maxQueuedEvents) {
        throw new Error("Issue 31 outbound publish queue is full.");
      }
      input.outboundStore?.put(event);
      outboundPublishes.set(event.id, { event, serializedFrame, room });
    }
    const sentRelayUrls: string[] = [];
    for (const relay of relays.values()) {
      if (relay.socket !== null && (relay.state === "live" || relay.state === "replaying")) {
        relay.socket.send(serializedFrame);
        relay.pendingPublishEventIds.add(event.id);
        sentRelayUrls.push(relay.relayUrl);
      }
    }
    return {
      eventId: event.id,
      sentRelayUrls,
      transportState: sentRelayUrls.length === 0 ? "queued" : "sent",
      relayAcknowledgement: "pending",
      commandCompletion: "pending_terminal_record",
    };
  };

  const retryPublish = (eventId: string): boolean => {
    if (!outboundPublishes.has(eventId)) return false;
    for (const relay of relays.values()) {
      relay.publishRefusals.delete(eventId);
      if (relay.socket !== null && (relay.state === "live" || relay.state === "replaying")) {
        sendQueuedPublishes(relay);
      }
    }
    notify();
    return true;
  };

  const discardPublish = (eventId: string): boolean => {
    if (!outboundPublishes.has(eventId)) return false;
    input.outboundStore?.delete(eventId);
    outboundPublishes.delete(eventId);
    for (const relay of relays.values()) {
      relay.pendingPublishEventIds.delete(eventId);
      relay.publishRefusals.delete(eventId);
    }
    notify();
    return true;
  };

  const updateSubscriptionScope: Issue31NostrClient["updateSubscriptionScope"] = async (scope) => {
    for (const authors of [
      scope.selectedHostPublicKeys,
      scope.ownerAuthors,
      scope.ownerRecipientPublicKeys,
      scope.communityAuthors,
    ]) {
      if (
        authors !== undefined &&
        (authors.length > 16 || authors.some((value) => !HEX_64.test(value)))
      ) {
        throw new Error("Issue 31 subscription authors are invalid.");
      }
    }
    if (
      scope.communityGroupIds !== undefined &&
      (scope.communityGroupIds.length > 16 ||
        scope.communityGroupIds.some(
          (value) => value.length < 1 || value.length > 128 || /\s/.test(value),
        ))
    ) {
      throw new Error("Issue 31 community group filters are invalid.");
    }
    if (scope.ownerAuthors !== undefined) {
      ownerAuthors.clear();
      for (const author of scope.ownerAuthors) ownerAuthors.add(author);
    }
    if (scope.ownerRecipientPublicKeys !== undefined) {
      ownerRecipientPublicKeys.clear();
      for (const publicKey of scope.ownerRecipientPublicKeys) {
        ownerRecipientPublicKeys.add(publicKey);
      }
    }
    if (scope.selectedHostPublicKeys !== undefined) {
      selectedHostPublicKeys.clear();
      for (const publicKey of scope.selectedHostPublicKeys) {
        selectedHostPublicKeys.add(publicKey);
      }
    }
    if (scope.communityAuthors !== undefined) {
      communityAuthors.clear();
      for (const author of scope.communityAuthors) communityAuthors.add(author);
    }
    if (scope.communityGroupIds !== undefined) {
      communityGroupIds.clear();
      for (const groupId of scope.communityGroupIds) communityGroupIds.add(groupId);
    }
    await Promise.all(
      [...relays.values()].flatMap((relay) => {
        if (relay.socket === null || (relay.state !== "live" && relay.state !== "replaying")) {
          return [];
        }
        relay.state = "replaying";
        relay.gapReason = "awaiting_eose";
        return [sendSubscriptions(relay)];
      }),
    );
    notify();
  };

  return {
    start,
    close,
    publish,
    retryPublish,
    discardPublish,
    updateSubscriptionScope,
    snapshot,
  };
};
