/**
 * Nostr relay connection and subscription management
 * @module
 */
import type { Scope } from "effect";
import { Context, Effect, Layer, Stream } from "effect";
import type { ConnectionError, MessageSendError, RelayError } from "../core/Errors.js";
import { SubscriptionError } from "../core/Errors.js";
import type { Filter, NostrEvent, SubscriptionId } from "../core/Schema.js";
import { WebSocketService } from "./WebSocketService.js";
export interface Subscription {
    readonly id: SubscriptionId;
    readonly filters: ReadonlyArray<Filter>;
    readonly events: Stream.Stream<NostrEvent, SubscriptionError | ConnectionError>;
}
export interface RelayConnection {
    readonly url: string;
    readonly subscribe: (id: SubscriptionId, filters: ReadonlyArray<Filter>) => Effect.Effect<Subscription, SubscriptionError | ConnectionError | MessageSendError, Scope.Scope>;
    readonly publish: (event: NostrEvent) => Effect.Effect<boolean, RelayError | ConnectionError | MessageSendError>;
    readonly close: (subscriptionId: SubscriptionId) => Effect.Effect<void, MessageSendError>;
    readonly disconnect: () => Effect.Effect<void>;
}
declare const RelayService_base: Context.TagClass<RelayService, "nostr/RelayService", {
    /**
     * Connect to a relay
     */
    readonly connect: (url: string) => Effect.Effect<RelayConnection, ConnectionError, Scope.Scope>;
}>;
/**
 * Service for relay operations
 */
export declare class RelayService extends RelayService_base {
}
/**
 * Live implementation of RelayService
 */
export declare const RelayServiceLive: Layer.Layer<RelayService, never, WebSocketService>;
export {};
//# sourceMappingURL=RelayService.d.ts.map