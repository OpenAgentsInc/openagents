/**
 * Core Nostr schemas for NIP-01 protocol
 * @module
 */

import { pipe, Schema } from "effect"

// Branded types for type safety
export const EventId = pipe(
  Schema.String,
  Schema.pattern(/^[0-9a-f]{64}$/),
  Schema.brand("EventId"),
  Schema.annotations({
    title: "EventId",
    description: "32-bytes lowercase hex-encoded sha256"
  })
)
export type EventId = Schema.Schema.Type<typeof EventId>

export const PublicKey = pipe(
  Schema.String,
  Schema.pattern(/^[0-9a-f]{64}$/),
  Schema.brand("PublicKey"),
  Schema.annotations({
    title: "PublicKey",
    description: "32-bytes lowercase hex-encoded public key"
  })
)
export type PublicKey = Schema.Schema.Type<typeof PublicKey>

export const PrivateKey = pipe(
  Schema.String,
  Schema.pattern(/^[0-9a-f]{64}$/),
  Schema.brand("PrivateKey"),
  Schema.annotations({
    title: "PrivateKey",
    description: "32-bytes lowercase hex-encoded private key"
  })
)
export type PrivateKey = Schema.Schema.Type<typeof PrivateKey>

export const Signature = pipe(
  Schema.String,
  Schema.pattern(/^[0-9a-f]{128}$/),
  Schema.brand("Signature"),
  Schema.annotations({
    title: "Signature",
    description: "64-bytes lowercase hex-encoded schnorr signature"
  })
)
export type Signature = Schema.Schema.Type<typeof Signature>

export const UnixTimestamp = pipe(
  Schema.Number,
  Schema.int(),
  Schema.positive(),
  Schema.annotations({
    title: "UnixTimestamp",
    description: "Unix timestamp in seconds"
  })
)
export type UnixTimestamp = Schema.Schema.Type<typeof UnixTimestamp>

export const EventKind = pipe(
  Schema.Number,
  Schema.int(),
  Schema.between(0, 65535),
  Schema.annotations({
    title: "EventKind",
    description: "Event kind (0-65535)"
  })
)
export type EventKind = Schema.Schema.Type<typeof EventKind>

// Tag schema - array of strings with at least one element (tag name)
export const Tag = pipe(
  Schema.Array(Schema.String),
  Schema.minItems(1),
  Schema.annotations({
    title: "Tag",
    description: "Event tag - array of strings with tag name as first element"
  })
)
export type Tag = Schema.Schema.Type<typeof Tag>

// Core event schema
export class NostrEvent extends Schema.Class<NostrEvent>("NostrEvent")({
  id: EventId,
  pubkey: PublicKey,
  created_at: UnixTimestamp,
  kind: EventKind,
  tags: Schema.Array(Tag),
  content: Schema.String,
  sig: Signature
}, {
  title: "NostrEvent",
  description: "Nostr event as defined in NIP-01"
}) {}

// Unsigned event (before id and signature)
export class UnsignedEvent extends Schema.Class<UnsignedEvent>("UnsignedEvent")({
  pubkey: PublicKey,
  created_at: UnixTimestamp,
  kind: EventKind,
  tags: Schema.Array(Tag),
  content: Schema.String
}, {
  title: "UnsignedEvent",
  description: "Nostr event before ID calculation and signing"
}) {}

// Event parameters for creation (without pubkey and created_at)
export class EventParams extends Schema.Class<EventParams>("EventParams")({
  kind: EventKind,
  tags: Schema.Array(Tag),
  content: Schema.String
}, {
  title: "EventParams",
  description: "Parameters for creating a new event"
}) {}

// Filter schema for querying events
export class Filter extends Schema.Class<Filter>("Filter")({
  ids: Schema.optional(Schema.Array(EventId)),
  authors: Schema.optional(Schema.Array(PublicKey)),
  kinds: Schema.optional(Schema.Array(EventKind)),
  since: Schema.optional(UnixTimestamp),
  until: Schema.optional(UnixTimestamp),
  limit: Schema.optional(pipe(Schema.Number, Schema.int(), Schema.positive())),
  // Tag filters - single letter tags
  "#e": Schema.optional(Schema.Array(EventId)),
  "#p": Schema.optional(Schema.Array(PublicKey)),
  "#a": Schema.optional(Schema.Array(Schema.String)),
  "#d": Schema.optional(Schema.Array(Schema.String))
}, {
  title: "Filter",
  description: "Filter for querying events as defined in NIP-01"
}) {}

// Subscription ID
export const SubscriptionId = pipe(
  Schema.String,
  Schema.minLength(1),
  Schema.maxLength(64),
  Schema.brand("SubscriptionId"),
  Schema.annotations({
    title: "SubscriptionId",
    description: "Arbitrary, non-empty string of max length 64 chars"
  })
)
export type SubscriptionId = Schema.Schema.Type<typeof SubscriptionId>

// Client to Relay messages
export const EventMessage = Schema.Tuple(
  Schema.Literal("EVENT"),
  NostrEvent
)

// Custom schema for variable-length REQ message
export const ReqMessage = Schema.Array(Schema.Unknown).pipe(
  Schema.filter((arr): arr is ["REQ", SubscriptionId, ...Filter[]] => {
    if (arr.length < 2) return false
    if (arr[0] !== "REQ") return false
    return true
  })
)

export const CloseMessage = Schema.Tuple(
  Schema.Literal("CLOSE"),
  SubscriptionId
)

export const ClientMessage = Schema.Union(
  EventMessage,
  ReqMessage,
  CloseMessage
)
export type ClientMessage = Schema.Schema.Type<typeof ClientMessage>

// Relay to Client messages
export const RelayEventMessage = Schema.Tuple(
  Schema.Literal("EVENT"),
  SubscriptionId,
  NostrEvent
)

export const OkMessage = Schema.Tuple(
  Schema.Literal("OK"),
  EventId,
  Schema.Boolean,
  Schema.String
)

export const EoseMessage = Schema.Tuple(
  Schema.Literal("EOSE"),
  SubscriptionId
)

export const ClosedMessage = Schema.Tuple(
  Schema.Literal("CLOSED"),
  SubscriptionId,
  Schema.String
)

export const NoticeMessage = Schema.Tuple(
  Schema.Literal("NOTICE"),
  Schema.String
)

export const RelayMessage = Schema.Union(
  RelayEventMessage,
  OkMessage,
  EoseMessage,
  ClosedMessage,
  NoticeMessage
)
export type RelayMessage = Schema.Schema.Type<typeof RelayMessage>

// Standard OK/CLOSED message prefixes
export const OkPrefix = Schema.Literal(
  "duplicate",
  "pow",
  "blocked",
  "rate-limited",
  "invalid",
  "restricted",
  "error"
)
export type OkPrefix = Schema.Schema.Type<typeof OkPrefix>
