# @openagentsinc/nostr

Effect-based Nostr protocol implementation with comprehensive NIP support.

## Installation

```bash
pnpm add @openagentsinc/nostr
```

## Features

- Type-safe Nostr event creation and validation
- WebSocket relay connections with automatic reconnection
- Effect-based error handling and resource management
- Comprehensive NIP implementations
- Built-in test utilities

## Usage

```typescript
import { EventService, RelayService } from "@openagentsinc/nostr"
import { Effect } from "effect"

// Create an event
const createEvent = Effect.gen(function* () {
  const eventService = yield* EventService
  const event = yield* eventService.create({
    kind: 1,
    content: "Hello Nostr!",
    tags: []
  })
  return event
})

// Connect to a relay
const connectToRelay = Effect.gen(function* () {
  const relayService = yield* RelayService
  const connection = yield* relayService.connect("wss://relay.nostr.example")
  // Use connection...
})
```

## License

CC0-1.0