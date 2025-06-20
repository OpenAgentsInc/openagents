/**
 * NIP-28: Public Chat Channel Service
 * Implements channel creation, messaging, and subscription functionality
 * @module
 */
import { Context, Effect, Layer, Schema, Stream } from "effect";
import type { EventId, Filter, NostrEvent, PrivateKey, PublicKey } from "../core/Schema.js";
import { EventService } from "../services/EventService.js";
import { RelayService } from "../services/RelayService.js";
declare const Nip28InvalidInputError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip28InvalidInputError";
} & Readonly<A>;
export declare class Nip28InvalidInputError extends Nip28InvalidInputError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip28PublishError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip28PublishError";
} & Readonly<A>;
export declare class Nip28PublishError extends Nip28PublishError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip28FetchError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip28FetchError";
} & Readonly<A>;
export declare class Nip28FetchError extends Nip28FetchError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip28ChannelNotFoundError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip28ChannelNotFoundError";
} & Readonly<A>;
export declare class Nip28ChannelNotFoundError extends Nip28ChannelNotFoundError_base<{
    channelId: string;
}> {
}
export declare const ChannelMetadataContentSchema: Schema.Struct<{
    name: typeof Schema.String;
    about: Schema.optional<typeof Schema.String>;
    picture: Schema.optional<typeof Schema.String>;
    relays: Schema.optional<Schema.Array$<typeof Schema.String>>;
}>;
export type ChannelMetadataContent = Schema.Schema.Type<typeof ChannelMetadataContentSchema>;
export declare const ChannelMessageContentSchema: typeof Schema.String;
export declare const ModerationReasonContentSchema: Schema.Struct<{
    reason: typeof Schema.String;
}>;
export type ModerationReasonContent = Schema.Schema.Type<typeof ModerationReasonContentSchema>;
export interface CreateChannelParams {
    name: string;
    about?: string;
    picture?: string;
    relays?: Array<string>;
    privateKey: PrivateKey;
}
export interface ChannelMetadata {
    name: string;
    about?: string;
    picture?: string;
    creatorPubkey: PublicKey;
    channelId: EventId;
    relays?: Array<string>;
}
export interface SendChannelMessageParams {
    channelId: EventId;
    content: string;
    privateKey: PrivateKey;
    replyToEventId?: EventId;
    replyToPubkey?: PublicKey;
    relayHint?: string;
}
export interface ChannelMessage extends NostrEvent {
    channelId: EventId;
    replyToEventId?: EventId;
    replyToPubkey?: PublicKey;
}
export interface HideMessageParams {
    messageEventId: EventId;
    reason?: string;
    privateKey: PrivateKey;
}
export interface MuteUserParams {
    userPubkey: PublicKey;
    reason?: string;
    privateKey: PrivateKey;
}
declare const Nip28Service_base: Context.TagClass<Nip28Service, "nostr/Nip28Service", {
    /**
     * Creates a new public chat channel (Kind 40).
     */
    readonly createChannel: (params: CreateChannelParams) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>;
    /**
     * Gets metadata for a channel from its creation event (Kind 40).
     */
    readonly getChannelMetadata: (channelId: EventId) => Effect.Effect<ChannelMetadata, Nip28FetchError | Nip28ChannelNotFoundError>;
    /**
     * Updates metadata for a channel (Kind 41).
     */
    readonly setChannelMetadata: (params: {
        channelId: EventId;
        name?: string;
        about?: string;
        picture?: string;
        privateKey: PrivateKey;
    }) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>;
    /**
     * Sends a message to a channel (Kind 42).
     * Messages are public and not encrypted.
     */
    readonly sendChannelMessage: (params: SendChannelMessageParams) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>;
    /**
     * Fetches messages for a channel (Kind 42).
     * Messages are sorted by created_at ascending (oldest first).
     */
    readonly getChannelMessages: (channelId: EventId, filterOptions?: Partial<Filter>) => Effect.Effect<Array<ChannelMessage>, Nip28FetchError>;
    /**
     * Subscribes to new messages for a channel.
     */
    readonly subscribeToChannelMessages: (channelId: EventId) => Stream.Stream<ChannelMessage, Nip28FetchError>;
    /**
     * Hide a message (Kind 43) - client-side moderation
     */
    readonly hideMessage: (params: HideMessageParams) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>;
    /**
     * Mute a user (Kind 44) - client-side moderation
     */
    readonly muteUser: (params: MuteUserParams) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>;
}>;
export declare class Nip28Service extends Nip28Service_base {
}
export declare const Nip28ServiceLive: Layer.Layer<Nip28Service, never, EventService | RelayService>;
export {};
//# sourceMappingURL=Nip28Service.d.ts.map