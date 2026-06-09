# Pylon v0.3: Proposed Bun/Effect/TypeScript/OpenTUI Port Specification

## Overview

Pylon v0.3 is a proposed complete port of the Pylon contributor app from Rust to TypeScript running on Bun, utilizing the `effect` concurrency ecosystem and the `@opentui/core` native Zig terminal rendering engine. The primary goal of Pylon v0.3 is to shift Pylon from an interactive CLI requiring manual operator execution to an automated, observational dashboard oriented around monitoring autonomous compute tasks and tracking real-time Bitcoin earnings.

---

## From Slash Commands to Autonomous Automation

The current Pylon implementation relies on interactive operator inputs (such as `/model`, `/download`, `/provider scan`, `/jobs`, `/payout`) to transition states. Pylon v0.3 deprecates manual command driving in favor of background automation:

1. **Auto-Warm Preflights**: On startup, Pylon v0.3 queries local GPU hardware resources via a platform discovery service, matches available capacities against recommended GGUF configurations, and automatically warms the target model without user intervention.
2. **Auto-Hydrate and Link**: The Money Dev Kit (MDK) `agent-wallet` is discovered, started, and hydrated automatically. Account-linking (NIP-98 authentication) and relay routing are negotiated as part of the initial launch sequence.
3. **Continuous Presence Heartbeats**: Instead of manual online toggles, presence heartbeats are managed as background fibers that continually publish provider status to configured relays with zero operator action.

---

## High-Density Observational UI Layout

The OpenTUI presentation layer is designed as an observational graphics console, divided into three automated panels. There is no active chat box; instead, the entire screen is optimized for monitoring autonomous execution:

```
+--------------------------------------------------------------------------------+
|                             Pylon v0.3 Earning Node                            |
+------------------------------------+-------------------------------------------+
| [Telemetry & Resource Plane]       | [Active Workroom Execution Logs]          |
|                                    |                                           |
| State: ACTIVE_IN_WORK              | 14:02:10 [Intake] New task received.       |
| Warm Model: Gemma-4-9B-SFT         | 14:02:12 [Sandbox] Isolated environment   |
| VRAM Load: 8.2 GB / 12.0 GB        |           warmup complete.                |
| CPU Usage: 14%                     | 14:02:25 [Inference] Generating patch.     |
| GPU Temp:  62C                     | 14:02:40 [Validator] Output matches       |
|                                    |           CS336 test parameters.          |
| Wallet Port: 3001 (OK)             | 14:02:45 [Settlement] Generating proof    |
| Relay Latency: 42ms                |           and publishing receipt.         |
|                                    |                                           |
+------------------------------------+-------------------------------------------+
| [Earnings & Transaction Ledger]                                                |
|                                    | Rank: COURIER  (Next: OPERATOR in 24k)    |
| Live Balance: 142,520 Sats         |                                           |
|                                    | [=====================>                  ] |
| Recent Payments:                   |                                           |
| - 14:02:45: +450 Sats  (Job #4931) |                                           |
| - 13:41:10: +200 Sats  (Job #4912) |                                           |
+------------------------------------+-------------------------------------------+
```

### 1. Telemetry & Resource Plane (Left Box)
* **Components**: `BoxRenderable` wrapping a `TextTableRenderable`.
* **Telemetry**: Tracks active model type, hardware thermals, active memory boundaries, wallet connection status, and network relay latency.

### 2. Active Workroom Execution Logs (Right Box)
* **Components**: `BoxRenderable` wrapping a `ScrollBoxRenderable` containing a streaming `MarkdownRenderable`.
* **Telemetry**: Displays real-time status output from the background workspace sidecar (`oa-workroomd`). It displays current task intake, sandbox isolation setup, active inference, test suite validation progress, and receipt publication.

### 3. Earnings & Transaction Ledger (Bottom Box)
* **Components**: Horizontal-split `BoxRenderable` displaying current MDK wallet balance, a scrolling list of recently settled Satoshis, and active rank progression (using the `STACKER_RANKS` scale) with a visual progress bar.

---

## Effect-Driven Architecture

The core of Pylon v0.3 is built as an Effect application consisting of four distinct layers:

### 1. State Machine Service
The node state is managed as a strictly typed Union of States (`State.Warming`, `State.Idle`, `State.Processing`, `State.Settling`) modeled through an Effect-based State service. Transitions are fully automated: incoming Nostr events automatically transition the state to `Processing`, triggering the isolated local sandbox environment.

### 2. Managed Sidecar Fiber
The background execution and communication with local MDK CLI processes or Rust sidecars run inside structured Effect Fibers. In the event of a crash, the main TUI remains completely responsive, while the crashed process is cleaned up and restarted with an exponential retry schedule.

### 3. Reactive UI Synchronizer
Instead of manual state merging, state changes publish to an Effect `PubSub` stream. The OpenTUI presentation loop subscribes to this stream, allowing incremental frame rendering to fire only when state changes are received.

```typescript
// Proposed Pylon v0.3 Main Entry Point
import { Effect, Schedule } from "effect"
import { createCliRenderer } from "@opentui/core"

const runPylonNode = Effect.gen(function* () {
  const renderer = yield* Effect.tryPromise(() => createCliRenderer({
    screenMode: "fullscreen",
    exitOnCtrlC: true,
  }))

  // Start Background Processes
  const walletFiber = yield* Effect.fork(startMdkWalletService)
  const heartbeatFiber = yield* Effect.fork(startPresenceHeartbeatLoop)
  const resourceFiber = yield* Effect.fork(startHardwareTelemetryLoop)

  yield* Effect.logInfo("Pylon v0.3 autonomous earning node initialized.")

  // Bounded execution loop
  yield* Effect.never
})
```
