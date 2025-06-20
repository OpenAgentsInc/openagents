/**
 * Core Nostr schemas - aggregates all schemas from primitives and NIPs
 * @module
 */
import { Schema } from "effect";
export declare const EventId: Schema.brand<Schema.filter<typeof Schema.String>, "EventId">;
export type EventId = Schema.Schema.Type<typeof EventId>;
export declare const PublicKey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
export type PublicKey = Schema.Schema.Type<typeof PublicKey>;
export declare const PrivateKey: Schema.brand<Schema.filter<typeof Schema.String>, "PrivateKey">;
export type PrivateKey = Schema.Schema.Type<typeof PrivateKey>;
export declare const Signature: Schema.brand<Schema.filter<typeof Schema.String>, "Signature">;
export type Signature = Schema.Schema.Type<typeof Signature>;
export declare const UnixTimestamp: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
export type UnixTimestamp = Schema.Schema.Type<typeof UnixTimestamp>;
export declare const EventKind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
export type EventKind = Schema.Schema.Type<typeof EventKind>;
export declare const Tag: Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>;
export type Tag = Schema.Schema.Type<typeof Tag>;
declare const NostrEvent_base: Schema.Class<NostrEvent, {
    id: Schema.brand<Schema.filter<typeof Schema.String>, "EventId">;
    pubkey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    created_at: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
    sig: Schema.brand<Schema.filter<typeof Schema.String>, "Signature">;
}, Schema.Struct.Encoded<{
    id: Schema.brand<Schema.filter<typeof Schema.String>, "EventId">;
    pubkey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    created_at: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
    sig: Schema.brand<Schema.filter<typeof Schema.String>, "Signature">;
}>, never, {
    readonly id: string & import("effect/Brand").Brand<"EventId">;
} & {
    readonly pubkey: string & import("effect/Brand").Brand<"PublicKey">;
} & {
    readonly created_at: number;
} & {
    readonly kind: number;
} & {
    readonly content: string;
} & {
    readonly sig: string & import("effect/Brand").Brand<"Signature">;
} & {
    readonly tags: readonly (readonly string[])[];
}, {}, {}>;
export declare class NostrEvent extends NostrEvent_base {
}
declare const UnsignedEvent_base: Schema.Class<UnsignedEvent, {
    pubkey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    created_at: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
}, Schema.Struct.Encoded<{
    pubkey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    created_at: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
}>, never, {
    readonly pubkey: string & import("effect/Brand").Brand<"PublicKey">;
} & {
    readonly created_at: number;
} & {
    readonly kind: number;
} & {
    readonly content: string;
} & {
    readonly tags: readonly (readonly string[])[];
}, {}, {}>;
export declare class UnsignedEvent extends UnsignedEvent_base {
}
declare const EventParams_base: Schema.Class<EventParams, {
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
}, Schema.Struct.Encoded<{
    kind: Schema.refine<number, Schema.filter<typeof Schema.Number>>;
    tags: Schema.Array$<Schema.refine<readonly string[], Schema.Array$<typeof Schema.String>>>;
    content: typeof Schema.String;
}>, never, {
    readonly kind: number;
} & {
    readonly content: string;
} & {
    readonly tags: readonly (readonly string[])[];
}, {}, {}>;
export declare class EventParams extends EventParams_base {
}
declare const Filter_base: Schema.Class<Filter, {
    ids: Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "EventId">>>;
    authors: Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">>>;
    kinds: Schema.optional<Schema.Array$<Schema.refine<number, Schema.filter<typeof Schema.Number>>>>;
    since: Schema.optional<Schema.refine<number, Schema.filter<typeof Schema.Number>>>;
    until: Schema.optional<Schema.refine<number, Schema.filter<typeof Schema.Number>>>;
    limit: Schema.optional<Schema.filter<Schema.filter<typeof Schema.Number>>>;
    "#e": Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "EventId">>>;
    "#p": Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">>>;
    "#a": Schema.optional<Schema.Array$<typeof Schema.String>>;
    "#d": Schema.optional<Schema.Array$<typeof Schema.String>>;
}, Schema.Struct.Encoded<{
    ids: Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "EventId">>>;
    authors: Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">>>;
    kinds: Schema.optional<Schema.Array$<Schema.refine<number, Schema.filter<typeof Schema.Number>>>>;
    since: Schema.optional<Schema.refine<number, Schema.filter<typeof Schema.Number>>>;
    until: Schema.optional<Schema.refine<number, Schema.filter<typeof Schema.Number>>>;
    limit: Schema.optional<Schema.filter<Schema.filter<typeof Schema.Number>>>;
    "#e": Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "EventId">>>;
    "#p": Schema.optional<Schema.Array$<Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">>>;
    "#a": Schema.optional<Schema.Array$<typeof Schema.String>>;
    "#d": Schema.optional<Schema.Array$<typeof Schema.String>>;
}>, never, {
    readonly ids?: readonly (string & import("effect/Brand").Brand<"EventId">)[] | undefined;
} & {
    readonly authors?: readonly (string & import("effect/Brand").Brand<"PublicKey">)[] | undefined;
} & {
    readonly kinds?: readonly number[] | undefined;
} & {
    readonly since?: number | undefined;
} & {
    readonly until?: number | undefined;
} & {
    readonly limit?: number | undefined;
} & {
    readonly "#e"?: readonly (string & import("effect/Brand").Brand<"EventId">)[] | undefined;
} & {
    readonly "#p"?: readonly (string & import("effect/Brand").Brand<"PublicKey">)[] | undefined;
} & {
    readonly "#a"?: readonly string[] | undefined;
} & {
    readonly "#d"?: readonly string[] | undefined;
}, {}, {}>;
export declare class Filter extends Filter_base {
}
export declare const SubscriptionId: Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">;
export type SubscriptionId = Schema.Schema.Type<typeof SubscriptionId>;
export declare const EventMessage: Schema.Tuple2<Schema.Literal<["EVENT"]>, typeof NostrEvent>;
export declare const ReqMessage: Schema.refine<readonly unknown[] & ["REQ", string & import("effect/Brand").Brand<"SubscriptionId">, ...Filter[]], Schema.Schema<readonly unknown[], readonly unknown[], never>>;
export declare const CloseMessage: Schema.Tuple2<Schema.Literal<["CLOSE"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">>;
export declare const ClientMessage: Schema.Union<[Schema.Tuple2<Schema.Literal<["EVENT"]>, typeof NostrEvent>, Schema.refine<readonly unknown[] & ["REQ", string & import("effect/Brand").Brand<"SubscriptionId">, ...Filter[]], Schema.Schema<readonly unknown[], readonly unknown[], never>>, Schema.Tuple2<Schema.Literal<["CLOSE"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">>]>;
export type ClientMessage = Schema.Schema.Type<typeof ClientMessage>;
export declare const RelayEventMessage: Schema.Tuple<[Schema.Literal<["EVENT"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">, typeof NostrEvent]>;
export declare const OkMessage: Schema.Tuple<[Schema.Literal<["OK"]>, Schema.brand<Schema.filter<typeof Schema.String>, "EventId">, typeof Schema.Boolean, typeof Schema.String]>;
export declare const EoseMessage: Schema.Tuple2<Schema.Literal<["EOSE"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">>;
export declare const ClosedMessage: Schema.Tuple<[Schema.Literal<["CLOSED"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">, typeof Schema.String]>;
export declare const NoticeMessage: Schema.Tuple2<Schema.Literal<["NOTICE"]>, typeof Schema.String>;
export declare const RelayMessage: Schema.Union<[Schema.Tuple<[Schema.Literal<["EVENT"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">, typeof NostrEvent]>, Schema.Tuple<[Schema.Literal<["OK"]>, Schema.brand<Schema.filter<typeof Schema.String>, "EventId">, typeof Schema.Boolean, typeof Schema.String]>, Schema.Tuple2<Schema.Literal<["EOSE"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">>, Schema.Tuple<[Schema.Literal<["CLOSED"]>, Schema.brand<Schema.filter<Schema.filter<typeof Schema.String>>, "SubscriptionId">, typeof Schema.String]>, Schema.Tuple2<Schema.Literal<["NOTICE"]>, typeof Schema.String>]>;
export type RelayMessage = Schema.Schema.Type<typeof RelayMessage>;
export declare const OkPrefix: Schema.Literal<["duplicate", "pow", "blocked", "rate-limited", "invalid", "restricted", "error"]>;
export type OkPrefix = Schema.Schema.Type<typeof OkPrefix>;
export declare const Mnemonic: Schema.brand<Schema.filter<typeof Schema.String>, "Mnemonic">;
export type Mnemonic = Schema.Schema.Type<typeof Mnemonic>;
export declare const Nsec: Schema.brand<Schema.filter<typeof Schema.String>, "Nsec">;
export type Nsec = Schema.Schema.Type<typeof Nsec>;
export declare const Npub: Schema.brand<Schema.filter<typeof Schema.String>, "Npub">;
export type Npub = Schema.Schema.Type<typeof Npub>;
export declare const DerivationPath: Schema.brand<Schema.filter<typeof Schema.String>, "DerivationPath">;
export type DerivationPath = Schema.Schema.Type<typeof DerivationPath>;
declare const KeyDerivationResult_base: Schema.Class<KeyDerivationResult, {
    privateKey: Schema.brand<Schema.filter<typeof Schema.String>, "PrivateKey">;
    publicKey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    nsec: Schema.brand<Schema.filter<typeof Schema.String>, "Nsec">;
    npub: Schema.brand<Schema.filter<typeof Schema.String>, "Npub">;
}, Schema.Struct.Encoded<{
    privateKey: Schema.brand<Schema.filter<typeof Schema.String>, "PrivateKey">;
    publicKey: Schema.brand<Schema.filter<typeof Schema.String>, "PublicKey">;
    nsec: Schema.brand<Schema.filter<typeof Schema.String>, "Nsec">;
    npub: Schema.brand<Schema.filter<typeof Schema.String>, "Npub">;
}>, never, {
    readonly publicKey: string & import("effect/Brand").Brand<"PublicKey">;
} & {
    readonly privateKey: string & import("effect/Brand").Brand<"PrivateKey">;
} & {
    readonly nsec: string & import("effect/Brand").Brand<"Nsec">;
} & {
    readonly npub: string & import("effect/Brand").Brand<"Npub">;
}, {}, {}>;
export declare class KeyDerivationResult extends KeyDerivationResult_base {
}
export {};
//# sourceMappingURL=Schema.d.ts.map