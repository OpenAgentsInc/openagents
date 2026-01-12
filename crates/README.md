# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a
single-paragraph overview of a crate to explain its role in the workspace.

## acp-adapter
The acp-adapter crate implements the Agent Client Protocol (ACP) adapter used to talk to external
coding agents over JSON-RPC. It manages sessions and permissions, converts ACP notifications into
OpenAgents events, and provides rlog replay/streaming utilities.

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

## compute
The compute crate implements a NIP-90 DVM provider that sells compute on Nostr. It bids on jobs,
executes inference or agent backends, and publishes results back to relays.

## daytona
The daytona crate is a Rust SDK for the Daytona sandbox API, covering sandbox lifecycle, file
operations, git actions, and command execution.

## dsrs
The dsrs crate is the Rust implementation of DSPy (Declarative Self-improving Programming). It
provides signatures, predictors, optimizers, retrieval, tracing, and evaluation for declarative
agent programming.

## dsrs-macros
The dsrs-macros crate provides procedural macros for declaring DSPy signatures and optimizable
modules in Rust.

## fm-bridge
The fm-bridge crate is a Rust client for the Apple Foundation Models HTTP bridge with an
OpenAI-compatible API surface.

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

## neobank
The neobank crate defines treasury and payment routing primitives for agent budgets across multiple
rails, including exchange and settlement flows.

## nostr (core/client/relay/tests)
The nostr workspace provides core Nostr protocol types, relay client APIs, a relay server, and
integration tests.

## oanix
The oanix crate is the agent OS runtime for environment discovery, capability manifests, and
situation assessment during boot.

## protocol
The protocol crate provides typed job schemas with deterministic hashing, provenance, and
verification strategies for the OpenAgents swarm.

## pylon
The pylon crate is the local runtime for sovereign agents. It supports host mode for running agents
and provider mode for selling NIP-90 compute, with relay and wallet integration.

## recorder
The recorder crate parses, validates, and repairs rlog session logs, and ships a CLI for inspection
and conversion.

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

## testing
The testing crate centralizes shared fixtures and helpers for workspace integration tests.

## vim
The vim crate is an editor-agnostic Vim emulation layer with mode, motion, and operator handling.

## wgpui
The wgpui crate is the GPU-accelerated UI library used across desktop and web, providing layout,
text/Markdown rendering, and input handling.
