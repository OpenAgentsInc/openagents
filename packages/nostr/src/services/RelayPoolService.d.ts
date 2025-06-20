/**
 * Relay connection pool for managing multiple Nostr relays
 * @module
 */
import type { Scope } from "effect";
import { Context, Effect, HashMap, Layer, Ref, Stream } from "effect";
import type { ConnectionError, MessageSendError } from "../core/Errors.js";
import { SubscriptionError } from "../core/Errors.js";
import type { EventId, Filter, NostrEvent, SubscriptionId } from "../core/Schema.js";
import { type RelayConnection, RelayService } from "./RelayService.js";
export interface PoolSubscription {
    readonly id: SubscriptionId;
    readonly filters: ReadonlyArray<Filter>;
    readonly events: Stream.Stream<NostrEvent, SubscriptionError | ConnectionError>;
    readonly seenOn: Ref.Ref<HashMap.HashMap<EventId, ReadonlyArray<string>>>;
}
export interface RelayPoolConnection {
    readonly urls: ReadonlyArray<string>;
    readonly connections: Ref.Ref<HashMap.HashMap<string, RelayConnection>>;
    readonly subscriptions: Ref.Ref<HashMap.HashMap<SubscriptionId, PoolSubscription>>;
    readonly subscribe: (id: SubscriptionId, filters: ReadonlyArray<Filter>) => Effect.Effect<PoolSubscription, SubscriptionError | ConnectionError, Scope.Scope>;
    readonly unsubscribe: (id: SubscriptionId) => Effect.Effect<void, SubscriptionError>;
    readonly publish: (event: NostrEvent) => Effect.Effect<HashMap.HashMap<string, boolean>, MessageSendError | ConnectionError>;
    readonly close: () => Effect.Effect<void>;
    readonly getConnectionStatus: () => Effect.Effect<HashMap.HashMap<string, "connected" | "disconnected">>;
}
export interface RelayPoolService {
    readonly connect: (urls: ReadonlyArray<string>) => Effect.Effect<RelayPoolConnection, ConnectionError, Scope.Scope>;
}
export declare const RelayPoolService: Context.Tag<RelayPoolService, RelayPoolService>;
/**
 * Live implementation of RelayPoolService
 */
export declare const RelayPoolServiceLive: Layer.Layer<RelayPoolService, never, RelayService>;
//# sourceMappingURL=RelayPoolService.d.ts.map