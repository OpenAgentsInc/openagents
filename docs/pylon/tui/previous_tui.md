# Previous Pylon TUI Evidence and Implementation Analysis

## Overview

Pylon TUI (`pylon-tui`) is implemented as a standalone Rust terminal user interface within the `openagents` repository. It provides contributor nodes with localized operator telemetry, resource tracking, and Money Dev Kit (MDK) wallet integration. It is located at `openagents/apps/pylon-tui/` and is defined as a workspace member crate.

## Implementation Architecture

The codebase in `openagents/apps/pylon-tui/` contains five core files that define its runtime and presentation layers:

1. **`main.rs`**: Handles command-line argument parsing, establishes the multi-threaded Tokio runtime, and executes the event-loop on a dedicated thread.
2. **`lib.rs`**: Establishes UI themes, wraps text to column widths, handles user keystrokes/mouse events, and coordinates worker threads.
3. **`bottom_pane.rs`**: Implements the user command bar and input area (composer) for executing slash commands or entering chat prompts.
4. **`slash_commands.rs`**: Parses slash commands (such as `/help`, `/download`, `/model`, `/uninstall`, `/announce`, `/provider`, `/job`, `/jobs`, `/earnings`, `/receipts`, `/payout`, `/relay`, and `/wallet`).
5. **`transcript.rs`**: Manages the scrolling historical log of past operations, system messages, and local model generation turns.

## Visual Design & Views

The UI is built using `ratatui` and `crossterm` and is designed as high-density engineering graphics. It supports three distinct sidebar panels:

* **Operate View**: Displays desired operation mode, current job intake telemetry, total earnings (lifetime and 24-hour), and recent market activities. It displays contributor progression using explicit thresholds defined in `STACKER_RANKS` (e.g., Pleb, Drifter, Runner, Courier, Operator, Captain, Sovereign, King).
* **Wallet View**: Tracks Money Dev Kit `agent-wallet` status, network details, current balances, bitcoin addresses, invoice records, and payment histories.
* **Inspect View**: Employs `sysinfo` to output high-density physical telemetry including CPU frequency/utilization, load average, physical and swap memory floor checks, disk reads/writes, GPU availability, and network interface rates.

## Scope Boundaries and Historical Context

The architectural boundaries of the Pylon TUI were established during the April 2026 codebase audits (specifically `docs/2026-04-04-pylon-tui-repo-boundary-audit.md`):

1. **Anti-Drift Discipline**: Historical archives in `backroom` showed prior Pylon iterations suffered from scope sprawl by attempting to absorb host-mode execution, browser/web socket bridges, Codex workrooms, and buyer-side task orchestration. 
2. **Monorepo Coherence**: The decision was made to build `pylon-tui` directly within the `openagents` repository rather than extracting it to a standalone repository. This structure keeps shared domain types in `openagents-provider-substrate` and uses a unified cargo workspace build and lint context.
3. **Narrow Resource Plane**: Pylon TUI operates strictly as a resource-selling dashboard for contributor nodes. Independent agent runtime capabilities (such as session lifecycle and tool authorization) are completely delegated to `probe`.

## Verification and Release Integration

The TUI is coupled directly to the MDK-default v0.2 release sequence. The operator workflow requires executing localized checks (`oa proof ...`) to verify configuration integrity, local wallet balances, and provider presence heartbeats prior to main branch integration and final production deployment.
