# @openagentsinc/nostr

Effect-based Nostr protocol implementation for the OpenAgents SDK.

## Overview

This package provides a comprehensive, type-safe implementation of the Nostr protocol using Effect. It includes support for multiple NIPs (Nostr Implementation Possibilities) and provides a robust foundation for building Nostr-enabled applications.

## Installation

```bash
pnpm add @openagentsinc/nostr
```

## Architecture

The package is organized into several modules:

- **Core**: Base types and schemas shared across all implementations
- **Services**: Core Nostr services (WebSocket, Relay, Crypto, Event handling)
- **NIPs**: Individual NIP implementations
- **Agent Profile**: OpenAgents-specific agent identity management

## Implemented Features

### Core Services

#### WebSocketService
Manages WebSocket connections with automatic reconnection and error handling.

```typescript
import { WebSocketService } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const ws = yield* WebSocketService
  const socket = yield* ws.connect("wss://relay.example.com")
  
  // Send messages
  yield* ws.send(socket, JSON.stringify(["REQ", "sub1", { kinds: [1] }]))
  
  // Receive messages
  yield* ws.receive(socket).pipe(
    Effect.tap((message) => Console.log("Received:", message))
  )
})
```

#### RelayService
High-level Nostr relay connection management.

```typescript
import { RelayService } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const relay = yield* RelayService
  
  // Connect to relay
  yield* relay.connect("wss://relay.example.com")
  
  // Subscribe to events
  const subscription = yield* relay.subscribe({
    kinds: [1],
    authors: ["pubkey..."],
    limit: 10
  })
  
  // Publish event
  yield* relay.publish({
    kind: 1,
    content: "Hello Nostr!",
    tags: [],
    created_at: Math.floor(Date.now() / 1000)
  })
})
```

#### CryptoService
Cryptographic operations for Nostr.

```typescript
import { CryptoService } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const crypto = yield* CryptoService
  
  // Generate keypair
  const { privateKey, publicKey } = yield* crypto.generateKeyPair()
  
  // Sign event
  const signature = yield* crypto.signEvent(eventId, privateKey)
  
  // Verify signature
  const isValid = yield* crypto.verifySignature(eventId, signature, publicKey)
})
```

### Implemented NIPs

#### NIP-01: Basic Protocol Flow
Core event handling and subscription management (in core services).

#### NIP-02: Contact Lists and Petname System
Manage following lists and local naming.

```typescript
import { nip02 } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const service = yield* nip02.Nip02Service
  
  // Create contact list
  const contactList = yield* service.createContactList([
    {
      pubkey: "pubkey1...",
      mainRelay: "wss://relay1.com",
      petname: "alice"
    },
    {
      pubkey: "pubkey2...",
      petname: "bob"
    }
  ], privateKey)
  
  // Set petname
  yield* service.setPetname(pubkey, "my-friend")
})
```

#### NIP-04: Encrypted Direct Messages
Legacy encrypted messaging (use NIP-44 for new implementations).

```typescript
import { nip04 } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const service = yield* nip04.Nip04Service
  
  // Encrypt message
  const encrypted = yield* service.encryptMessage(
    "Secret message",
    recipientPubkey,
    senderPrivkey
  )
  
  // Decrypt message
  const decrypted = yield* service.decryptMessage(
    encrypted,
    senderPubkey,
    recipientPrivkey
  )
})
```

#### NIP-05: DNS-based Internet Identifiers
Resolve human-readable identifiers.

```typescript
import { nip05 } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const service = yield* nip05.Nip05Service
  
  // Resolve identifier
  const profile = yield* service.resolve("alice@example.com")
  console.log(profile.pubkey) // The resolved public key
  
  // Verify identifier
  const isValid = yield* service.verify(pubkey, "alice@example.com")
})
```

#### NIP-06: Basic Key Derivation from Mnemonic
Deterministic key generation from seed phrases.

```typescript
import { Nip06Service } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const nip06 = yield* Nip06Service
  
  // Generate mnemonic
  const mnemonic = yield* nip06.generateMnemonic()
  
  // Derive keys
  const privateKey = yield* nip06.derivePrivateKey(mnemonic, 0)
  const publicKey = yield* nip06.derivePublicKey(mnemonic, 0)
})
```

#### NIP-09: Event Deletion
Request deletion of events.

```typescript
import { nip09 } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const service = yield* nip09.Nip09Service
  
  // Create deletion request
  const deletion = yield* service.createDeletionEvent({
    eventIds: ["event1", "event2"],
    reason: "spam"
  }, authorPrivkey)
  
  // Validate deletion request
  yield* service.validateDeletion(
    deletionRequest,
    requestedBy,
    originalEvents,
    policy
  )
})
```

#### NIP-19: bech32-encoded Entities
Encode/decode Nostr entities.

```typescript
import { nip19 } from "@openagentsinc/nostr"

// Encode public key
const npub = nip19.npubEncode("pubkey...")

// Decode npub
const pubkey = nip19.npubDecode("npub1...")

// Encode event reference with relay hints
const nevent = nip19.neventEncode({
  id: "eventid...",
  relays: ["wss://relay1.com", "wss://relay2.com"],
  author: "pubkey..."
})

// Decode nevent
const eventPointer = nip19.neventDecode("nevent1...")
```

#### NIP-28: Public Chat
Channel-based public chat.

```typescript
import { Nip28Service } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const chat = yield* Nip28Service
  
  // Create channel
  const channel = yield* chat.createChannel({
    name: "openagents",
    about: "OpenAgents development chat",
    picture: "https://example.com/icon.png"
  })
  
  // Send message
  yield* chat.sendMessage(channel.id, "Hello channel!")
  
  // Subscribe to messages
  yield* chat.subscribeToChannel(channel.id)
})
```

#### NIP-42: Authentication of clients to relays
Challenge-response authentication for relay connections.

```typescript
import { Nip42Service } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const auth = yield* Nip42Service
  
  // Generate challenge (relay side)
  const challenge = yield* auth.generateChallenge()
  
  // Create auth event (client side)
  const authEvent = yield* auth.createAuthEvent({
    challenge: challengeFromRelay,
    relayUrl: "wss://relay.example.com",
    privateKey: yourPrivateKey
  })
  
  // Send AUTH message to relay
  // ["AUTH", authEvent]
  
  // Verify auth event (relay side)
  const isValid = yield* auth.verifyAuthEvent({
    event: authEvent,
    challenge: challenge,
    relayUrl: "wss://relay.example.com"
  })
  console.log(isValid) // true if valid
})
```

#### NIP-44: Versioned Encryption
Modern encryption standard (recommended over NIP-04).

```typescript
import { nip44 } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const service = yield* nip44.Nip44Service
  
  // Encrypt with versioning
  const encrypted = yield* service.encrypt(
    "Secret message",
    recipientPubkey,
    senderPrivkey
  )
  
  // Decrypt
  const decrypted = yield* service.decrypt(
    encrypted,
    senderPubkey,
    recipientPrivkey
  )
  
  // Derive conversation key for multiple messages
  const convKey = yield* service.deriveConversationKey(
    privateKey,
    publicKey
  )
})
```

#### NIP-90: Data Vending Machine
AI service marketplace protocol.

```typescript
import { Nip90Service } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const dvm = yield* Nip90Service
  
  // Request AI service
  const jobRequest = yield* dvm.createJobRequest({
    kind: 65001, // Text generation
    input: "Write a haiku about Nostr",
    bid: 1000 // satoshis
  })
  
  // Process job (as service provider)
  const result = yield* dvm.processJob(jobRequest)
  
  // Submit job result
  yield* dvm.submitJobResult(jobRequest.id, result)
})
```

### Agent Profile Service

OpenAgents-specific extension for agent identity management.

```typescript
import { AgentProfileService } from "@openagentsinc/nostr"

const program = Effect.gen(function* () {
  const profiles = yield* AgentProfileService
  
  // Create agent profile
  const profile = yield* profiles.createProfile({
    name: "Agent Smith",
    about: "I help with coding tasks",
    capabilities: ["code", "debug", "review"],
    picture: "https://example.com/avatar.png"
  })
  
  // Update profile
  yield* profiles.updateProfile(profile.id, {
    status: "available"
  })
})
```

## Testing

All implementations include comprehensive tests. Run tests with:

```bash
pnpm test
```

## Development

### Adding a New NIP

1. Create implementation in `src/nips/nipXX.ts`
2. Export from `src/index.ts`
3. Add tests in `test/nips/nipXX.test.ts`
4. Update this README

### Code Organization

```
src/
├── core/           # Core types and utilities
│   ├── Errors.ts   # Tagged error types
│   └── Schema.ts   # Shared schemas
├── services/       # Core Nostr services
│   ├── WebSocketService.ts
│   ├── RelayService.ts
│   ├── CryptoService.ts
│   └── EventService.ts
├── nips/          # Individual NIP implementations
│   ├── nip02.ts   # Contact lists
│   ├── nip04.ts   # Encrypted DMs
│   ├── nip05.ts   # DNS identifiers
│   ├── nip09.ts   # Event deletion
│   ├── nip19.ts   # bech32 encoding
│   └── nip44.ts   # Versioned encryption
├── nip06/         # Key derivation
├── nip28/         # Public chat
├── nip90/         # Data vending machine
└── agent-profile/ # Agent identity management
```

## Error Handling

All services use Effect's typed error system:

```typescript
const program = Effect.gen(function* () {
  const service = yield* nip05.Nip05Service
  
  yield* service.resolve("invalid@identifier").pipe(
    Effect.catchTag("Nip05Error", (error) => {
      switch (error.reason) {
        case "dns_error":
          return Console.error("DNS lookup failed")
        case "invalid_format":
          return Console.error("Invalid identifier format")
        case "not_found":
          return Console.error("Identifier not found")
      }
    })
  )
})
```

## Best Practices

1. **Always use branded types** for keys, IDs, and other identifiers
2. **Handle errors explicitly** using Effect's error handling
3. **Clean up resources** using Effect's resource management
4. **Test thoroughly** including edge cases and error conditions
5. **Document your code** with examples and type annotations

## License

CC0-1.0