import { Schema as S } from "effect";

import {
  makePublicChatRelayClient,
  type PublicChatRelayClient,
  type PublicChatRelaySnapshot,
} from "./client.js";
import type { PublicNostrChatManifest } from "./manifest.js";
import { PublicChatRelayState } from "./parity.js";
import {
  Hex64,
  NostrEvent,
  PUBLIC_CHAT_ACCEPTED_KINDS,
  PUBLIC_CHAT_GROUP_STATE_KINDS,
  PUBLIC_CHAT_MODERATION_KINDS,
  PublicChatCursor,
  PublicChatRejectionReason,
  SafeGroupId,
  nextCursor,
  stableChronological,
} from "./profile.js";

export const PUBLIC_CHANNEL_DESCRIPTOR_SCHEMA = "openagents.public_channel_descriptor.v1" as const;
export const PUBLIC_CHANNEL_REGISTRY_SCHEMA = "openagents.public_channel_registry.v1" as const;
export const PUBLIC_CHANNEL_SNAPSHOT_SCHEMA = "openagents.public_channel_snapshot.v1" as const;

const ChannelId = S.String.check(S.isPattern(/^[a-z0-9][a-z0-9-]{0,63}$/));
const DisplayName = S.String.check(S.isMinLength(1), S.isMaxLength(80));
const Kind = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(65_535));
const PositiveInt = S.Number.check(S.isInt(), S.isGreaterThan(0));
const RelayUrl = S.String.check(S.isPattern(/^wss:\/\/[^\s]+$/));

export const PublicChannelLimits = S.Struct({
  attachmentBytes: PositiveInt,
  attachmentCount: PositiveInt,
  contentBytes: PositiveInt,
  eventBytes: PositiveInt,
  futureSkewSeconds: PositiveInt,
  historyPageSize: PositiveInt,
  maxAgeSeconds: PositiveInt,
  tags: PositiveInt,
});
export type PublicChannelLimits = typeof PublicChannelLimits.Type;

export const PublicChannelDescriptor = S.Struct({
  acceptedKinds: S.Array(Kind),
  channelId: ChannelId,
  displayName: DisplayName,
  expectedRelaySelfPubkey: S.NullOr(Hex64),
  groupId: SafeGroupId,
  groupStateKinds: S.Array(Kind),
  limits: PublicChannelLimits,
  moderationKinds: S.Array(Kind),
  profileVersion: S.String.check(S.isMinLength(1), S.isMaxLength(120)),
  relayTrust: S.Literals(["pinned", "metadata-untrusted"]),
  relayUrl: RelayUrl,
  richContentProfileVersion: S.String.check(S.isMinLength(1), S.isMaxLength(120)),
  schemaVersion: S.Literal(PUBLIC_CHANNEL_DESCRIPTOR_SCHEMA),
});
export type PublicChannelDescriptor = typeof PublicChannelDescriptor.Type;

export const PublicChannelRegistry = S.Struct({
  channels: S.Array(PublicChannelDescriptor),
  schemaVersion: S.Literal(PUBLIC_CHANNEL_REGISTRY_SCHEMA),
});
export type PublicChannelRegistry = typeof PublicChannelRegistry.Type;

export const PublicChannelRelayIdentity = S.Struct({
  expectedRelaySelfPubkey: S.NullOr(Hex64),
  groupStateTrusted: S.Boolean,
  observedRelaySelfPubkey: S.NullOr(Hex64),
  reconnectAllowed: S.Boolean,
  status: S.Literals(["verified", "metadata-untrusted", "key-change-review"]),
});
export type PublicChannelRelayIdentity = typeof PublicChannelRelayIdentity.Type;

export const PublicChannelGapReason = S.NullOr(
  S.Union([
    PublicChatRejectionReason,
    S.Literals([
      "awaiting-eose",
      "disconnect-before-eose",
      "invalid-event",
      "invalid-relay-frame",
      "key-change-review",
      "relay-notice",
      "signer-refused",
      "unsafe-event-content",
    ]),
  ]),
);
export type PublicChannelGapReason = typeof PublicChannelGapReason.Type;

export const PublicChannelSnapshot = S.Struct({
  channelId: ChannelId,
  cursor: S.NullOr(PublicChatCursor),
  gapReason: PublicChannelGapReason,
  groupId: SafeGroupId,
  lastCurrentAt: S.NullOr(S.Int),
  relayIdentity: PublicChannelRelayIdentity,
  relayUrl: RelayUrl,
  schemaVersion: S.Literal(PUBLIC_CHANNEL_SNAPSHOT_SCHEMA),
  state: PublicChatRelayState,
  verifiedEvents: S.Array(NostrEvent),
});
export type PublicChannelSnapshot = typeof PublicChannelSnapshot.Type;

const strictlyIncreasingUniqueKinds = (values: ReadonlyArray<number>): boolean =>
  values.length > 0 && values.every((value, index) => index === 0 || values[index - 1]! < value);

export const normalizePublicChannelRelayUrl = (value: string): string => {
  const url = new URL(value);
  if (
    url.protocol !== "wss:" ||
    url.hostname.length === 0 ||
    url.username.length > 0 ||
    url.password.length > 0 ||
    url.search.length > 0 ||
    url.hash.length > 0
  ) {
    throw new Error(
      "The relay URL must be a public wss URL without credentials, a query, or a fragment.",
    );
  }
  const path = url.pathname.replace(/\/+$/, "");
  return path === "" ? url.origin : `${url.origin}${path}`;
};

const coordinateKey = (descriptor: Pick<PublicChannelDescriptor, "relayUrl" | "groupId">): string =>
  JSON.stringify([descriptor.relayUrl, descriptor.groupId]);

const validateDescriptor = (descriptor: PublicChannelDescriptor): PublicChannelDescriptor => {
  if (normalizePublicChannelRelayUrl(descriptor.relayUrl) !== descriptor.relayUrl) {
    throw new Error(`Channel ${descriptor.channelId} has a noncanonical relay URL.`);
  }
  for (const [name, values] of [
    ["acceptedKinds", descriptor.acceptedKinds],
    ["groupStateKinds", descriptor.groupStateKinds],
    ["moderationKinds", descriptor.moderationKinds],
  ] as const) {
    if (!strictlyIncreasingUniqueKinds(values)) {
      throw new Error(`Channel ${descriptor.channelId} has an invalid ${name} list.`);
    }
  }
  for (const [name, values, supported] of [
    ["acceptedKinds", descriptor.acceptedKinds, PUBLIC_CHAT_ACCEPTED_KINDS],
    ["groupStateKinds", descriptor.groupStateKinds, PUBLIC_CHAT_GROUP_STATE_KINDS],
    ["moderationKinds", descriptor.moderationKinds, PUBLIC_CHAT_MODERATION_KINDS],
  ] as const) {
    if (values.some((kind) => !supported.includes(kind as never))) {
      throw new Error(`Channel ${descriptor.channelId} has an unsupported ${name} value.`);
    }
  }
  if (
    (descriptor.expectedRelaySelfPubkey === null) !==
    (descriptor.relayTrust === "metadata-untrusted")
  ) {
    throw new Error(`Channel ${descriptor.channelId} has inconsistent relay trust.`);
  }
  return descriptor;
};

export const decodePublicChannelDescriptor = (value: unknown): PublicChannelDescriptor =>
  validateDescriptor(
    S.decodeUnknownSync(PublicChannelDescriptor, {
      onExcessProperty: "error",
    })(value),
  );

export const decodePublicChannelRegistry = (value: unknown): PublicChannelRegistry => {
  const registry = S.decodeUnknownSync(PublicChannelRegistry, {
    onExcessProperty: "error",
  })(value);
  if (registry.channels.length === 0 || registry.channels.length > 64) {
    throw new Error("A channel registry must contain between 1 and 64 channels.");
  }
  const channelIds = new Set<string>();
  const coordinates = new Set<string>();
  for (const entry of registry.channels) {
    const channel = validateDescriptor(entry);
    const coordinate = coordinateKey(channel);
    if (channelIds.has(channel.channelId)) {
      throw new Error(`The channel ID ${channel.channelId} is not unique.`);
    }
    if (coordinates.has(coordinate)) {
      throw new Error(`The channel coordinate ${coordinate} is not unique.`);
    }
    channelIds.add(channel.channelId);
    coordinates.add(coordinate);
  }
  return registry;
};

export const agentChatChannelDescriptor = (
  manifest: PublicNostrChatManifest,
): PublicChannelDescriptor =>
  decodePublicChannelDescriptor({
    acceptedKinds: [...manifest.acceptedKinds].sort((a, b) => a - b),
    channelId: "agent-chat",
    displayName: "Agent Chat",
    expectedRelaySelfPubkey: manifest.relay.selfPubkey,
    groupId: manifest.group.id,
    groupStateKinds: [...manifest.group.stateKinds].sort((a, b) => a - b),
    limits: { ...manifest.limits },
    moderationKinds: [...PUBLIC_CHAT_MODERATION_KINDS].sort((a, b) => a - b),
    profileVersion: manifest.profileVersion,
    relayTrust: manifest.relay.selfPubkey === null ? "metadata-untrusted" : "pinned",
    relayUrl: normalizePublicChannelRelayUrl(manifest.relay.websocketUrl),
    richContentProfileVersion: manifest.richContentProfileVersion,
    schemaVersion: PUBLIC_CHANNEL_DESCRIPTOR_SCHEMA,
  });

export const assessPublicChannelRelayIdentity = (
  expectedRelaySelfPubkey: string | null,
  observedRelaySelfPubkey: string | null,
): PublicChannelRelayIdentity => {
  if (
    expectedRelaySelfPubkey !== null &&
    observedRelaySelfPubkey !== null &&
    expectedRelaySelfPubkey !== observedRelaySelfPubkey
  ) {
    return {
      expectedRelaySelfPubkey,
      groupStateTrusted: false,
      observedRelaySelfPubkey,
      reconnectAllowed: false,
      status: "key-change-review",
    };
  }
  if (expectedRelaySelfPubkey !== null && observedRelaySelfPubkey === expectedRelaySelfPubkey) {
    return {
      expectedRelaySelfPubkey,
      groupStateTrusted: true,
      observedRelaySelfPubkey,
      reconnectAllowed: true,
      status: "verified",
    };
  }
  return {
    expectedRelaySelfPubkey,
    groupStateTrusted: false,
    observedRelaySelfPubkey,
    reconnectAllowed: true,
    status: "metadata-untrusted",
  };
};

export const publicChannelIdentityUpdateNeedsReview = (
  previous: string | null,
  next: string | null,
): boolean => previous !== next;

export const publicChannelSnapshotKey = (descriptor: PublicChannelDescriptor): string =>
  JSON.stringify([descriptor.channelId, descriptor.relayUrl, descriptor.groupId]);

const secretKeyName = /(?:^|[-_])(private|secret|seed|mnemonic|bearer|cookie|token)(?:$|[-_])/i;
const secretValue =
  /(?:\bnsec1[023456789acdefghjklmnpqrstuvwxyz]{20,}\b|\bBearer\s+[A-Za-z0-9._~+/=-]{12,}|\bOPENAGENTS_AGENT_TOKEN\s*=|\b(?:private|secret)[-_ ]?key\s*[:=]|\bmnemonic\s*[:=])/i;

const containsSecretShape = (value: unknown): boolean => {
  if (typeof value === "string") return secretValue.test(value);
  if (Array.isArray(value)) return value.some(containsSecretShape);
  if (typeof value !== "object" || value === null) return false;
  return Object.entries(value).some(
    ([key, entry]) => secretKeyName.test(key) || containsSecretShape(entry),
  );
};

const safeGapReason = (value: string | null): PublicChannelGapReason => {
  if (value === null) return null;
  if (S.is(PublicChannelGapReason)(value)) return value;
  return "relay-notice";
};

export const toPublicChannelSnapshot = (
  descriptor: PublicChannelDescriptor,
  relayIdentity: PublicChannelRelayIdentity,
  source: PublicChatRelaySnapshot,
): PublicChannelSnapshot => {
  const verifiedEvents = stableChronological(
    source.events.filter((event) => !containsSecretShape(event)),
  );
  const unsafe = verifiedEvents.length !== source.events.length;
  const cursor = nextCursor(verifiedEvents) ?? null;
  return {
    channelId: descriptor.channelId,
    cursor,
    gapReason: unsafe ? "unsafe-event-content" : safeGapReason(source.gapReason),
    groupId: descriptor.groupId,
    lastCurrentAt: source.lastCurrentAt,
    relayIdentity,
    relayUrl: descriptor.relayUrl,
    schemaVersion: PUBLIC_CHANNEL_SNAPSHOT_SCHEMA,
    state: source.state,
    verifiedEvents,
  };
};

export type PublicChannelRelayClient = Readonly<{
  close: PublicChatRelayClient["close"];
  connect: PublicChatRelayClient["connect"];
  loadOlder: PublicChatRelayClient["loadOlder"];
  snapshot: () => PublicChannelSnapshot;
  subscribe: (listener: (snapshot: PublicChannelSnapshot) => void) => () => void;
}>;

export const makePublicChannelRelayClient = (
  input: Readonly<{
    descriptor: PublicChannelDescriptor;
    now?: () => number;
    observedRelaySelfPubkey: string | null;
    reconnectMs?: number;
    webSocket?: Parameters<typeof makePublicChatRelayClient>[0]["webSocket"];
  }>,
):
  | Readonly<{ client: PublicChannelRelayClient; ok: true }>
  | Readonly<{ ok: false; reason: "key-change-review" }> => {
  const descriptor = decodePublicChannelDescriptor(input.descriptor);
  const relayIdentity = assessPublicChannelRelayIdentity(
    descriptor.expectedRelaySelfPubkey,
    input.observedRelaySelfPubkey,
  );
  if (!relayIdentity.reconnectAllowed) {
    return { ok: false, reason: "key-change-review" };
  }
  const relay = makePublicChatRelayClient({
    acceptedKinds: descriptor.acceptedKinds,
    groupId: descriptor.groupId,
    groupStateKinds: descriptor.groupStateKinds,
    limits: descriptor.limits,
    moderationKinds: descriptor.moderationKinds,
    relayUrl: descriptor.relayUrl,
    ...(input.now === undefined ? {} : { now: input.now }),
    ...(input.reconnectMs === undefined ? {} : { reconnectMs: input.reconnectMs }),
    ...(relayIdentity.groupStateTrusted && relayIdentity.observedRelaySelfPubkey !== null
      ? { relaySelfPubkey: relayIdentity.observedRelaySelfPubkey }
      : {}),
    ...(input.webSocket === undefined ? {} : { webSocket: input.webSocket }),
  });
  return {
    client: {
      close: relay.close,
      connect: relay.connect,
      loadOlder: relay.loadOlder,
      snapshot: () => toPublicChannelSnapshot(descriptor, relayIdentity, relay.snapshot()),
      subscribe: (listener) =>
        relay.subscribe((snapshot) =>
          listener(toPublicChannelSnapshot(descriptor, relayIdentity, snapshot)),
        ),
    },
    ok: true,
  };
};
