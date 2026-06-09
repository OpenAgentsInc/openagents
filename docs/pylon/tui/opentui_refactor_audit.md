# OpenTUI & Effect Refactor Audit for Pylon TUI

## Overview

Pylon TUI (`pylon-tui`) is currently implemented in Rust using `ratatui` and `crossterm`. It manages background operations and UI rendering via OS threads, manual word-wrap calculations, and `mpsc` channel message passing. This document audits a proposed refactor to migrate Pylon TUI to `opentui` and the `effect` TypeScript ecosystem, aligning its architecture with `probe`.

---

## Current Architecture Bottlenecks

1. **Manual Layout Calculations**: `pylon-tui` requires manual calculation of wrapped line counts (`wrapped_line_total` and `wrapped_rows` in `lib.rs`) to prevent panel clipping on narrow terminals. This adds cognitive overhead and rendering instability.
2. **Coarse Concurrency Management**: Telemetry refreshes, Money Dev Kit (MDK) wallet queries, and presence heartbeats are spawned as unstructured OS threads (`std::thread::spawn`). If an API query stalls, threads can pile up without timeout boundaries.
3. **Manual State Synchronization**: State returned from background threads must be manually merged into the application state using a custom stabilizer (`stabilize_operator_panel_stats`), leading to complex state reconciliation code.

---

## The OpenTUI & Effect Blueprint

### 1. OpenTUI Rendering Layer

OpenTUI (`@opentui/core`) leverages a native Zig core with Yoga-based flexbox layout, eliminating the need for manual height or wrap calculations.

* **Layout Structure**:
  * Root container: `BoxRenderable` set to flex-direction column.
  * Header/Title: `TextRenderable` formatted with Berkeley Mono styling.
  * Central Workspace: Split-pane `BoxRenderable` with row layout:
    * Left Pane (Operator Stats): `BoxRenderable` enclosing a `TextTableRenderable` for dense, multi-column key-value metrics.
    * Right Pane (System Logs/Transcript): `ScrollBoxRenderable` encapsulating a `MarkdownRenderable` for rich streaming events.
  * Footer: `TextareaRenderable` for slash commands.

### 2. Concurrency with Effect

The `effect` ecosystem provides highly typed, fiber-based concurrency, enabling declarative management of asynchronous tasks.

* **Fiber-Based Telemetry**: Telemetry querying is split into separate fibers using `Effect.fork`. Stalled calls to the MDK wallet API or presence heartbeats can be bounded using `Effect.timeout`.
* **Resilient Polling**: Telemetry loops can be structured using `Effect.repeat(Schedule.spaced("10 seconds"))`, providing clean error recovery, backing-off on API failures, and clean interrupt semantics upon shutdown.
* **Schema Validation**: OpenTUI and Effect schemas (`Schema.Struct`) allow for immediate runtime parsing of JSON outputs from MDK CLI queries, preventing raw parsing exceptions in the render loop.

---

## Comparison of Refactoring Paths

### Path A: TypeScript / Bun Port (Recommended)
This approach rewrites `pylon-tui` as a TypeScript application executed via Bun, matching the stack of `probe`.

* **Causal Impact**:
  * Unifies the terminal UI dependency tree across `openagents` and `probe` to `@opentui/core`.
  * Integrates direct node package linking for `@opentui/core` using `bun link`.
  * Enables natural event streaming (`onEvent` handlers) from MDK wallet queries to the active UI layer.
* **Drawback**: Requires re-authoring the sysinfo gathering layers to use JS bindings or parsing shell tools, and porting Rust pylon types to TypeScript.

### Path B: Rust Bindings to OpenTUI
This approach keeps `pylon-tui` in Rust but integrates with OpenTUI via its native C ABI.

* **Causal Impact**:
  * Retains the existing Rust-based sysinfo and substrate integrations inside `openagents`.
  * Avoids a complete language rewrite.
* **Drawback**: Requires authoring manual Rust FFI bindings to the C ABI exposed by `@opentui/core`'s Zig core, adding significant integration debt.

---

## Implementation Recommendation

We recommend executing **Path A (TypeScript / Bun Port)** for the following reasons:
1. **Developer Parity**: Unified developer tooling with `probe` makes UI debugging and custom renderable assembly uniform across the platform.
2. **Robust Concurrency**: Effect's concurrent scheduler natively solves the unstructured background polling issues observed in the current Rust implementation.
3. **Yoga Flexbox**: Using the flexbox engine inside OpenTUI completely deletes the manual cell and row wrap calculation code currently maintained in `pylon-tui`'s Rust core.
