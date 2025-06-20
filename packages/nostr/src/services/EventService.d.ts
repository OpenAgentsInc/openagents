/**
 * Service for creating and validating Nostr events
 * @module
 */
import type { ParseResult } from "effect";
import { Context, Effect, Layer } from "effect";
import { EventValidationError, InvalidEventId, InvalidSignature } from "../core/Errors.js";
import { type EventId, type EventParams, NostrEvent, type PrivateKey, type Signature, type UnsignedEvent } from "../core/Schema.js";
import { CryptoService } from "./CryptoService.js";
declare const EventService_base: Context.TagClass<EventService, "nostr/EventService", {
    /**
     * Create a new event from parameters
     */
    readonly create: (params: EventParams, privateKey: PrivateKey) => Effect.Effect<NostrEvent, EventValidationError | InvalidEventId | InvalidSignature | ParseResult.ParseError>;
    /**
     * Verify an event's signature and ID
     */
    readonly verify: (event: NostrEvent) => Effect.Effect<NostrEvent, InvalidEventId | InvalidSignature>;
    /**
     * Calculate event ID
     */
    readonly calculateId: (event: UnsignedEvent) => Effect.Effect<EventId, InvalidEventId>;
    /**
     * Sign an event
     */
    readonly sign: (event: UnsignedEvent & {
        id: EventId;
    }, privateKey: PrivateKey) => Effect.Effect<Signature, InvalidSignature>;
    /**
     * Serialize event for hashing (as per NIP-01)
     */
    readonly serialize: (event: UnsignedEvent) => string;
}>;
/**
 * Service for event operations
 */
export declare class EventService extends EventService_base {
}
/**
 * Live implementation of EventService
 */
export declare const EventServiceLive: Layer.Layer<EventService, never, CryptoService>;
export {};
//# sourceMappingURL=EventService.d.ts.map