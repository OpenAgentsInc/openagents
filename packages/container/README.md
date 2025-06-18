# @openagentsinc/container

Container orchestration and Firecracker VM management for OpenAgents.

## Overview

This package provides Effect-based services for managing Firecracker microVMs to deploy OpenAgents at scale. The goal is to enable hosting millions of Psionic applications efficiently.

## Current Status

**MVP Implementation (Stub)**

This is a minimal stub implementation that demonstrates the API surface. The actual Firecracker integration is planned for future development.

## Features

- FirecrackerService for VM lifecycle management
- NetworkSetupService for TAP interface configuration  
- Schema-based configuration with @effect/schema
- Integration with SDK's Compute namespace
- CLI commands for container operations

## Usage

```typescript
import { FirecrackerService, makeDefaultFirecrackerConfig } from "@openagentsinc/container"

// Create VM configuration
const config = makeDefaultFirecrackerConfig({
  kernelPath: "/path/to/kernel",
  rootfsPath: "/path/to/rootfs",
  vcpus: 2,
  memoryMb: 512
})

// Use with Effect
Effect.gen(function* () {
  const service = yield* FirecrackerService
  const vm = yield* service.createVM({ id: "test-vm", config })
  console.log("VM started:", vm)
})
```

## Testing

See `docs/containers/firecracker-testing.md` for detailed testing instructions.

## Future Work

- Real Firecracker API integration
- CRIU checkpoint/restore for hibernation
- Resource monitoring and metrics
- Multi-VM orchestration
- Storage deduplication

## License

ISC