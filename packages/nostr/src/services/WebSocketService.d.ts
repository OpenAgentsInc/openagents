/**
 * WebSocket connection management service
 * @module
 */
import type { Scope } from "effect";
import { Context, Effect, Layer, Stream } from "effect";
import { WebSocket } from "ws";
import { ConnectionError, MessageSendError } from "../core/Errors.js";
export interface WebSocketConnection {
    readonly url: string;
    readonly ws: WebSocket;
    readonly readyState: () => number;
    readonly send: (message: string) => Effect.Effect<void, MessageSendError>;
    readonly close: (code?: number, reason?: string) => Effect.Effect<void>;
    readonly messages: Stream.Stream<string, ConnectionError>;
}
declare const WebSocketService_base: Context.TagClass<WebSocketService, "nostr/WebSocketService", {
    /**
     * Connect to a WebSocket URL
     */
    readonly connect: (url: string) => Effect.Effect<WebSocketConnection, ConnectionError, Scope.Scope>;
    /**
     * Check if a connection is open
     */
    readonly isOpen: (connection: WebSocketConnection) => boolean;
}>;
/**
 * Service for WebSocket operations
 */
export declare class WebSocketService extends WebSocketService_base {
}
/**
 * Live implementation of WebSocketService
 */
export declare const WebSocketServiceLive: Layer.Layer<WebSocketService, never, never>;
export {};
//# sourceMappingURL=WebSocketService.d.ts.map