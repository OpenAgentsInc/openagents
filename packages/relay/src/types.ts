/**
 * Local type definitions to avoid circular dependencies
 * @module
 */

// Basic types from Nostr
export type EventId = string & { readonly EventId: unique symbol }
export type PublicKey = string & { readonly PublicKey: unique symbol }
export type PrivateKey = string & { readonly PrivateKey: unique symbol }
export type SubscriptionId = string & { readonly SubscriptionId: unique symbol }

export interface NostrEvent {
  readonly id: EventId
  readonly pubkey: PublicKey
  readonly created_at: number
  readonly kind: number
  readonly tags: ReadonlyArray<ReadonlyArray<string>>
  readonly content: string
  readonly sig: string
}

export interface Filter {
  readonly ids?: ReadonlyArray<EventId>
  readonly authors?: ReadonlyArray<PublicKey>
  readonly kinds?: ReadonlyArray<number>
  readonly since?: number
  readonly until?: number
  readonly limit?: number
  readonly "#e"?: ReadonlyArray<EventId>
  readonly "#p"?: ReadonlyArray<PublicKey>
  readonly [key: string]: any
}

export type ClientMessage =
  | readonly ["EVENT", NostrEvent]
  | readonly ["REQ", SubscriptionId, ...Array<Filter>]
  | readonly ["CLOSE", SubscriptionId]

export type RelayMessage =
  | readonly ["EVENT", SubscriptionId, NostrEvent]
  | readonly ["EOSE", SubscriptionId]
  | readonly ["OK", EventId, boolean, string]
  | readonly ["NOTICE", string]
