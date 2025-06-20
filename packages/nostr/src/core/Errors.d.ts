/**
 * Core error types for Nostr operations
 * @module
 */
import { Schema } from "effect";
declare const InvalidEventId_base: Schema.TaggedErrorClass<InvalidEventId, "InvalidEventId", {
    readonly _tag: Schema.tag<"InvalidEventId">;
} & {
    id: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class InvalidEventId extends InvalidEventId_base {
}
declare const InvalidSignature_base: Schema.TaggedErrorClass<InvalidSignature, "InvalidSignature", {
    readonly _tag: Schema.tag<"InvalidSignature">;
} & {
    eventId: typeof Schema.String;
    publicKey: typeof Schema.String;
    reason: Schema.optional<typeof Schema.String>;
}>;
export declare class InvalidSignature extends InvalidSignature_base {
}
declare const InvalidEventFormat_base: Schema.TaggedErrorClass<InvalidEventFormat, "InvalidEventFormat", {
    readonly _tag: Schema.tag<"InvalidEventFormat">;
} & {
    field: typeof Schema.String;
    value: typeof Schema.Unknown;
    reason: typeof Schema.String;
}>;
export declare class InvalidEventFormat extends InvalidEventFormat_base {
}
declare const EventValidationError_base: Schema.TaggedErrorClass<EventValidationError, "EventValidationError", {
    readonly _tag: Schema.tag<"EventValidationError">;
} & {
    eventId: Schema.optional<typeof Schema.String>;
    errors: Schema.Array$<typeof Schema.String>;
}>;
export declare class EventValidationError extends EventValidationError_base {
}
declare const CryptoError_base: Schema.TaggedErrorClass<CryptoError, "CryptoError", {
    readonly _tag: Schema.tag<"CryptoError">;
} & {
    operation: Schema.Literal<["sign", "verify", "hash", "generateKey"]>;
    reason: typeof Schema.String;
}>;
export declare class CryptoError extends CryptoError_base {
}
declare const InvalidPrivateKey_base: Schema.TaggedErrorClass<InvalidPrivateKey, "InvalidPrivateKey", {
    readonly _tag: Schema.tag<"InvalidPrivateKey">;
} & {
    reason: typeof Schema.String;
}>;
export declare class InvalidPrivateKey extends InvalidPrivateKey_base {
}
declare const InvalidPublicKey_base: Schema.TaggedErrorClass<InvalidPublicKey, "InvalidPublicKey", {
    readonly _tag: Schema.tag<"InvalidPublicKey">;
} & {
    publicKey: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class InvalidPublicKey extends InvalidPublicKey_base {
}
declare const ConnectionError_base: Schema.TaggedErrorClass<ConnectionError, "ConnectionError", {
    readonly _tag: Schema.tag<"ConnectionError">;
} & {
    url: typeof Schema.String;
    reason: typeof Schema.String;
    code: Schema.optional<Schema.Union<[typeof Schema.Number, typeof Schema.String]>>;
}>;
export declare class ConnectionError extends ConnectionError_base {
}
declare const ConnectionClosed_base: Schema.TaggedErrorClass<ConnectionClosed, "ConnectionClosed", {
    readonly _tag: Schema.tag<"ConnectionClosed">;
} & {
    url: typeof Schema.String;
    reason: Schema.optional<typeof Schema.String>;
}>;
export declare class ConnectionClosed extends ConnectionClosed_base {
}
declare const MessageSendError_base: Schema.TaggedErrorClass<MessageSendError, "MessageSendError", {
    readonly _tag: Schema.tag<"MessageSendError">;
} & {
    url: typeof Schema.String;
    message: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class MessageSendError extends MessageSendError_base {
}
declare const SubscriptionError_base: Schema.TaggedErrorClass<SubscriptionError, "SubscriptionError", {
    readonly _tag: Schema.tag<"SubscriptionError">;
} & {
    subscriptionId: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class SubscriptionError extends SubscriptionError_base {
}
declare const SubscriptionClosed_base: Schema.TaggedErrorClass<SubscriptionClosed, "SubscriptionClosed", {
    readonly _tag: Schema.tag<"SubscriptionClosed">;
} & {
    subscriptionId: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class SubscriptionClosed extends SubscriptionClosed_base {
}
declare const RelayError_base: Schema.TaggedErrorClass<RelayError, "RelayError", {
    readonly _tag: Schema.tag<"RelayError">;
} & {
    url: typeof Schema.String;
    code: Schema.Literal<["duplicate", "pow", "blocked", "rate-limited", "invalid", "restricted", "error"]>;
    message: typeof Schema.String;
}>;
export declare class RelayError extends RelayError_base {
}
declare const RelayNotice_base: Schema.TaggedErrorClass<RelayNotice, "RelayNotice", {
    readonly _tag: Schema.tag<"RelayNotice">;
} & {
    url: typeof Schema.String;
    message: typeof Schema.String;
}>;
export declare class RelayNotice extends RelayNotice_base {
}
declare const TimeoutError_base: Schema.TaggedErrorClass<TimeoutError, "TimeoutError", {
    readonly _tag: Schema.tag<"TimeoutError">;
} & {
    operation: typeof Schema.String;
    timeoutMs: typeof Schema.Number;
}>;
export declare class TimeoutError extends TimeoutError_base {
}
declare const Nip06Error_base: Schema.TaggedErrorClass<Nip06Error, "Nip06Error", {
    readonly _tag: Schema.tag<"Nip06Error">;
} & {
    operation: Schema.Literal<["generateMnemonic", "validateMnemonic", "deriveKey", "encodeKey", "decodeKey"]>;
    reason: typeof Schema.String;
}>;
export declare class Nip06Error extends Nip06Error_base {
}
declare const InvalidMnemonic_base: Schema.TaggedErrorClass<InvalidMnemonic, "InvalidMnemonic", {
    readonly _tag: Schema.tag<"InvalidMnemonic">;
} & {
    mnemonic: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class InvalidMnemonic extends InvalidMnemonic_base {
}
declare const KeyDerivationError_base: Schema.TaggedErrorClass<KeyDerivationError, "KeyDerivationError", {
    readonly _tag: Schema.tag<"KeyDerivationError">;
} & {
    path: typeof Schema.String;
    reason: typeof Schema.String;
}>;
export declare class KeyDerivationError extends KeyDerivationError_base {
}
export {};
//# sourceMappingURL=Errors.d.ts.map