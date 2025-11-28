/**
 * Core error types for Nostr operations
 * @module
 */

import { Schema } from "effect"

// Validation errors
export class InvalidEventId extends Schema.TaggedError<InvalidEventId>()("InvalidEventId", {
  id: Schema.String,
  reason: Schema.String
}) {}

export class InvalidSignature extends Schema.TaggedError<InvalidSignature>()("InvalidSignature", {
  eventId: Schema.String,
  publicKey: Schema.String,
  reason: Schema.optional(Schema.String)
}) {}

export class InvalidEventFormat extends Schema.TaggedError<InvalidEventFormat>()("InvalidEventFormat", {
  field: Schema.String,
  value: Schema.Unknown,
  reason: Schema.String
}) {}

export class EventValidationError extends Schema.TaggedError<EventValidationError>()("EventValidationError", {
  eventId: Schema.optional(Schema.String),
  errors: Schema.Array(Schema.String)
}) {}

// Crypto errors
export class CryptoError extends Schema.TaggedError<CryptoError>()("CryptoError", {
  operation: Schema.Literal("sign", "verify", "hash", "generateKey"),
  reason: Schema.String
}) {}

export class InvalidPrivateKey extends Schema.TaggedError<InvalidPrivateKey>()("InvalidPrivateKey", {
  reason: Schema.String
}) {}

export class InvalidPublicKey extends Schema.TaggedError<InvalidPublicKey>()("InvalidPublicKey", {
  publicKey: Schema.String,
  reason: Schema.String
}) {}

// Connection errors
export class ConnectionError extends Schema.TaggedError<ConnectionError>()("ConnectionError", {
  url: Schema.String,
  reason: Schema.String,
  code: Schema.optional(Schema.Union(Schema.Number, Schema.String))
}) {}

export class ConnectionClosed extends Schema.TaggedError<ConnectionClosed>()("ConnectionClosed", {
  url: Schema.String,
  reason: Schema.optional(Schema.String)
}) {}

export class MessageSendError extends Schema.TaggedError<MessageSendError>()("MessageSendError", {
  url: Schema.String,
  message: Schema.String,
  reason: Schema.String
}) {}

// Subscription errors
export class SubscriptionError extends Schema.TaggedError<SubscriptionError>()("SubscriptionError", {
  subscriptionId: Schema.String,
  reason: Schema.String
}) {}

export class SubscriptionClosed extends Schema.TaggedError<SubscriptionClosed>()("SubscriptionClosed", {
  subscriptionId: Schema.String,
  reason: Schema.String
}) {}

// Relay errors
export class RelayError extends Schema.TaggedError<RelayError>()("RelayError", {
  url: Schema.String,
  code: Schema.Literal("duplicate", "pow", "blocked", "rate-limited", "invalid", "restricted", "error"),
  message: Schema.String
}) {}

export class RelayNotice extends Schema.TaggedError<RelayNotice>()("RelayNotice", {
  url: Schema.String,
  message: Schema.String
}) {}

// Timeout error
export class TimeoutError extends Schema.TaggedError<TimeoutError>()("TimeoutError", {
  operation: Schema.String,
  timeoutMs: Schema.Number
}) {}
