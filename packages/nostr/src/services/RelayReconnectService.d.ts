/**
 * Automatic reconnection service for Nostr relays
 * @module
 */
import type { Scope } from "effect";
import { Context, Duration, Effect, Layer, Ref } from "effect";
import type { ConnectionError } from "../core/Errors.js";
import { type RelayConnection, RelayService } from "./RelayService.js";
export interface ReconnectConfig {
    readonly initialDelay: Duration.Duration;
    readonly maxDelay: Duration.Duration;
    readonly maxAttempts: number;
    readonly factor: number;
}
export declare const defaultReconnectConfig: ReconnectConfig;
export interface ReconnectingRelay {
    readonly url: string;
    readonly connection: Ref.Ref<RelayConnection | null>;
    readonly isConnected: Ref.Ref<boolean>;
    readonly reconnectAttempts: Ref.Ref<number>;
    readonly stop: () => Effect.Effect<void, never, never>;
}
export interface RelayReconnectService {
    readonly createReconnectingRelay: (url: string, config?: Partial<ReconnectConfig>) => Effect.Effect<ReconnectingRelay, ConnectionError, Scope.Scope>;
}
export declare const RelayReconnectService: Context.Tag<RelayReconnectService, RelayReconnectService>;
/**
 * Live implementation of RelayReconnectService
 */
export declare const RelayReconnectServiceLive: Layer.Layer<RelayReconnectService, never, RelayService>;
//# sourceMappingURL=RelayReconnectService.d.ts.map