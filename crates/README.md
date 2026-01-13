# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a
single-paragraph overview of a crate to explain its role in the workspace.

## Workspace Layout Plan (Draft)
This plan keeps a single Cargo workspace but groups crates by purpose so product code stays
distinct from shared platform, core, and UI code. The UI group should live under `crates/ui/`
(not `crates/ux/`).

Proposed grouping (adjust as needed):
- `crates/products/`: autopilot, pylon, onyx
- `crates/core/`: adjutant, autopilot-core, dsrs, dsrs-macros, rlm, frlm
- `crates/platform/`: agent, protocol, runtime, compute, gateway, lm-router, local-inference, relay, issues
- `crates/ui/`: wgpui, vim, editor
- `crates/integrations/`: gpt-oss, nostr, spark, voice
- `crates/tools/`: arrow (Autopilot testing utilities)

Proposed steps:
1. Create the group folders under `crates/` and move the crate directories.
2. Update the root `Cargo.toml` workspace members to include the new paths (glob per group).
3. Update all path dependencies in crate `Cargo.toml` files to the new locations.
4. Refresh docs and tooling references (this file, any build scripts, CI paths).
5. Run `cargo check` and fix any path or feature fallout.

## adjutant
The adjutant crate is the autonomous task execution engine. It plans and routes work via DSPy
decision pipelines, delegates to Codex or RLM when needed, runs tool execution, and records session
outcomes for self-improvement and optimization.

## agent
The agent crate defines the core data model for sovereign agents: configs, lifecycle states, spawn
requests, and registry persistence. It owns identity and wallet metadata but does not execute
agents itself.

## autopilot
The autopilot crate is the WGPUI desktop UI for local-first agent sessions. It renders streaming
Markdown, tool cards, panels, and command palettes, manages workspace/session history, and drives
Codex app-server runs and Adjutant autopilot loops.

## autopilot-core
The autopilot-core crate provides shared execution logic for Autopilot, including DSPy workflows,
checkpointing, preflight checks, session logging, and streaming event normalization.

## codex-mcp
The codex-mcp crate provides minimal Rust helpers for implementing MCP (Model Context Protocol)
JSON-RPC servers over stdio in the style expected by Codex integrations. It focuses on the core
tool flow (`tools/list`, `tools/call`) with small protocol types and a server loop. See
`crates/codex-mcp/README.md` for usage and protocol details.

## compute
The compute crate implements a NIP-90 DVM provider that sells compute on Nostr. It bids on jobs,
executes inference or agent backends, and publishes results back to relays.

## dsrs
The dsrs crate is the Rust implementation of DSPy (Declarative Self-improving Programming). It
provides signatures, predictors, optimizers, retrieval, tracing, and evaluation for declarative
agent programming.

## dsrs-macros
The dsrs-macros crate provides procedural macros for declaring DSPy signatures and optimizable
modules in Rust.

## frlm
The frlm crate implements Federated Recursive Language Models, orchestrating distributed subqueries
across local and swarm backends with trace emission and verification.

## gateway
The gateway crate provides a unified abstraction layer between agents and external AI service
providers, including authentication, health checks, and capability discovery.

## gpt-oss
The gpt-oss crate is a Rust client for the GPT-OSS Responses API with streaming support and Harmony
prompt helpers.

## issues
The issues crate provides the SQLite-backed issue tracking core with migrations and lifecycle
transitions used by Autopilot and CLI tooling.

## lm-router
The lm-router crate routes LLM calls across multiple backends with usage tracking and model routing.

## local-inference
The local-inference crate defines shared request/response types and traits for local inference
engines.

## nostr (core/client/relay/tests)
The nostr workspace provides core Nostr protocol types, relay client APIs, a relay server, and
integration tests.

## protocol
The protocol crate provides typed job schemas with deterministic hashing, provenance, and
verification strategies for the OpenAgents swarm.

## pylon
The pylon crate is the local runtime for sovereign agents. It supports host mode for running agents
and provider mode for selling NIP-90 compute, with relay and wallet integration.

## relay
The relay crate defines the WebSocket protocol shared by tunnel clients and UI surfaces for session
control and streaming.

## rlm
The rlm crate is the Recursive Language Model execution engine, with tools for long-context analysis
and optional DSPy integration.

## runtime
The runtime crate provides a pluggable execution environment for agents, including filesystem-like
mounts for compute, containers, identity, and telemetry.

## spark
The spark crate integrates Breez Spark payments for OpenAgents wallets and compute flows.

## vim
The vim crate is an editor-agnostic Vim emulation layer with mode, motion, and operator handling.

## wgpui
The wgpui crate is the GPU-accelerated UI library used across desktop and web, providing layout,
text/Markdown rendering, and input handling.
