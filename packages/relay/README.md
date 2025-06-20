# @openagentsinc/relay

Nostr relay with NIP-01 support and Effect.js architecture.

## Features

- Full NIP-01 protocol implementation
- PlanetScale database backend with Drizzle ORM
- Effect.js for type-safe functional programming
- Psionic framework integration
- WebSocket connection management
- Real-time event broadcasting

## Usage

```typescript
import { createRelayPlugin } from '@openagentsinc/relay'

app.elysia.use(createRelayPlugin({
  path: '/relay',
  maxConnections: 1000,
  enableCors: true
}))
```