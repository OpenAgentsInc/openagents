# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a single-paragraph overview of a crate to explain its role in the workspace. Descriptions call out core responsibilities, primary interfaces, and key dependencies so overlap is easy to spot. Nested crates use paths like `nostr/core` or `web/worker`.

## acp-adapter
The acp-adapter crate implements the Agent Client Protocol (ACP) adapter used to talk to external coding agents over JSON-RPC 2.0. It wraps SDKs like claude-agent-sdk and codex-agent-sdk, manages sessions and permission handling, and converts ACP notifications to and from rlog streams for recorder and UI replay.

## adjutant
The adjutant crate is the autonomous task execution agent, named after StarCraft's command & control AI. It prioritizes Claude Pro/Max via claude-agent-sdk, falls back to Cerebras TieredExecutor, and uses tools directly (Read, Edit, Bash, Glob, Grep). For complex analysis it integrates RLM for large context processing, and can delegate to Claude Code for very complex work. It owns task planning, complexity assessment, and execution routing logic.

## agent
The agent crate defines the core data model for sovereign agents: configs, lifecycle states, spawn requests, and registry persistence. It owns identity and wallet metadata but does not schedule or execute agents itself; runtimes like Pylon and Nexus build on these types.

## agent-orchestrator
The agent-orchestrator crate provides a control plane for coordinating multiple agents and backends. It defines agent registry/config types, lifecycle hooks, and background task management for parallel sessions, and is intended to integrate with Autopilot and marketplace policies for routing.

## auth
The auth crate handles token-based authentication for local services by generating or loading a per-user token, storing it with restrictive permissions, and validating with constant-time comparison.

## autopilot
The autopilot crate is the core autonomous runner for code tasks. It drives plan, execute, review, and fix phases using agent SDKs, emits structured `StartupState` and `ClaudeEvent` streams, and persists `SessionCheckpoint` data for resume/replay. It also runs preflight checks, integrates with issues storage, and writes rlog trajectories.

## autopilot-container
The autopilot-container crate wraps Autopilot in an HTTP service designed for Cloudflare Containers. It handles repo cloning, run startup, and WebSocket streaming, powering the paid web execution path.

## autopilot-service
The autopilot-service crate is the service layer for shells and CLIs. It exposes `AutopilotRuntime` snapshots, groups startup logs into UI sections, and tracks per-phase session IDs and events for presentation layers.

## autopilot-shell
The autopilot-shell crate is the WGPUI desktop shell that renders the docked HUD layout, panels, and keymap for Autopilot. It consumes autopilot-service snapshots and provides UI components like the full-auto toggle and rate limit display.

## autopilot-wasm
The autopilot-wasm crate provides WASM bindings for replay viewing, including JSONL parsing, replay bundle construction, secret redaction, and timeline querying.

## bench-datasets
The bench-datasets crate provides async loaders and configuration for benchmark datasets. It implements dataset-specific loaders (S-NIAH, BrowseComp-Plus, OOLONG, CodeQA), exposes a shared Dataset trait, and re-exports bench-harness task types for consistent evaluation.

## bench-harness
The bench-harness crate is the experiment backbone for benchmark replication. It defines task and method traits, experiment runners with checkpointing, trajectory logging in JSONL, and metrics/statistics helpers for aggregating results and usage.

## bench-runner
The bench-runner crate is a CLI for running RLM paper replication experiments. It wires bench-harness, bench-datasets, and rlm-methods together with lm-router backends, supports dataset/method selection, and can emit tables or ablation analyses from stored results.

## claude-agent-sdk
The claude-agent-sdk crate is a Rust SDK for the Claude Code CLI. It manages sessions and streaming, exposes permission and budget controls, and supports interrupt/abort flows for UI-driven stop behavior.

## claude-mcp
The claude-mcp crate is an MCP server that exposes Claude Code as JSON-RPC stdio tools. It wraps claude-agent-sdk to provide query execution, session management, and permission configuration for MCP-aware clients.

## codex-agent-sdk
The codex-agent-sdk crate is a Rust SDK for the Codex CLI agent. It provides thread/session management, streaming events, and configurable sandbox, model, and approval settings.

## compute
The compute crate implements a NIP-90 DVM provider that sells compute on Nostr. It listens for job requests, bids/invoices, executes inference or agent backends, and publishes results back to relays. It also defines UnifiedIdentity (Nostr plus Spark) and backend registries for job handling.

## config
The config crate loads, validates, and writes `.openagents/project.json`. It centralizes defaults for models, safety, sandboxing, Claude settings, and other Autopilot runtime constraints.

## daytona
The daytona crate is a Rust SDK for the Daytona sandbox API, covering sandbox lifecycle, file operations, git actions, and command execution models.

## editor
The editor crate provides text editor primitives used by WGPUI surfaces: a Ropey-backed text buffer, caret and selection logic, undo/redo history, and optional tree-sitter syntax highlighting on native targets.

## fm-bridge
The fm-bridge crate is a Rust client for the Apple Foundation Models HTTP bridge with an OpenAI-compatible API surface. It includes model listing, health checks, and a CLI for local inference testing.

## fm-bridge-agent
The fm-bridge-agent crate wraps fm-bridge in an agent-style interface. It adds session state, tool execution plumbing, and error handling so FM-backed sessions can participate like other agent backends.

## frlm
The frlm crate implements Federated Recursive Language Models, orchestrating distributed sub-queries across local, swarm, and datacenter backends. It provides a conductor, scheduler, policy, verification, and trace emission to coordinate fanout execution and aggregate results.

## frostr
The frostr crate implements FROSTR threshold Schnorr signing for Nostr identities, including key sharing and signing flows for k-of-n setups.

## gitafter
The gitafter crate powers the GitAfter desktop app, a Nostr-native GitHub alternative. It ties local git operations to NIP-34 events and provides a UI for repos, issues, and patches.

## gpt-oss
The gpt-oss crate is a Rust client for the GPT-OSS Responses API. It defines request/response types, streaming chunks, and Harmony prompt rendering utilities used by higher-level agents.

## gpt-oss-agent
The gpt-oss-agent crate wraps gpt-oss in an agent abstraction compatible with ACP tooling. It manages sessions, tool calls, and rlog recording, and includes builtin tools like browser, python, and apply_patch implementations.

## gpt-oss-metal
The gpt-oss-metal crate provides Metal-backed bindings and an inference engine for GPT-OSS models. It loads local model binaries, renders Harmony prompts, streams token callbacks, and exposes configuration via environment variables.

## issue-tool
The issue-tool crate is a lightweight CLI wrapper over the issues database for create, list, claim, complete, and block workflows.

## issues
The issues crate provides the SQLite-backed issue tracking core with migrations and lifecycle transitions. It stores projects, sessions, and issue state used by Autopilot and CLI tooling.

## lm-router
The lm-router crate routes LLM calls across multiple backends. It exposes a router/builder, backend traits, usage tracking, and built-in backends including Ollama (localhost inference), FM Bridge (Apple Foundation Models), OpenAI, OpenRouter, swarm simulation, and mocks for benchmarking.

## local-inference
The local-inference crate defines the `LocalModelBackend` trait and shared request/response types for local inference engines. It standardizes streaming, model metadata, and readiness checks across backends like gpt-oss and fm-bridge.

## marketplace
The marketplace crate implements the agent economy layer: skills listings, compute provider profiles, reputations, data marketplace primitives, and payment/credit accounting. It is the shared domain model for routing and pricing agent work.

## ml
The ml crate is a unified inference library built on Candle with browser-first support. It handles GGUF/GPT-OSS model loading, sampling, tokenizer utilities, optional WebGPU paths, and NIP-90 DVM provider plumbing for native and WASM targets.

## ml/candle-wgpu
The ml/candle-wgpu crate implements a WebGPU backend for Candle. It provides a WGPU device/storage bridge with CPU fallback, shader-backed ops, and pipeline caching for browser and native targets.

## neobank
The neobank crate defines treasury and payment routing primitives for agent budgets across multiple rails. It models amounts, assets, quotes, policy checks, and exchange flows (including NIP-69 order concepts) and serves as the foundation for higher-level wallet and marketplace integrations.

## nexus
The nexus crate holds the planned cloud runtime for sovereign agents. It is the design counterpart to Pylon and captures the intended hosting model and APIs.

## nexus/client
The nexus/client crate is a WASM WGPUI dashboard for Nexus relay stats. It renders the HUD, polls `/api/stats`, and surfaces event/job/rlm metrics in a browser canvas.

## nexus/worker
The nexus/worker crate is the Cloudflare Worker implementation of the Nexus relay. It handles NIP-01/11/42/89/90 flows, serves the HUD assets and stats API, and uses Durable Objects plus D1 storage for relay state.

## nostr/core
The nostr/core crate implements core Nostr protocol types and cryptography, including events, filters, signing, and key derivation.

## nostr/client
The nostr/client crate provides a WebSocket relay client, subscription management, and publish flows for Nostr communication.

## nostr/relay
The nostr/relay crate is a Nostr relay server with WebSocket endpoints, filtering, and SQLite-backed storage.

## nostr/tests
The nostr/tests crate contains integration tests that exercise core, client, and relay behavior together.

## oanix
The oanix crate is the agent operating system runtime (OpenAgents NIX). It handles environment discovery during boot, including hardware detection (CPU, GPU, memory), inference backend discovery (Ollama, FM Bridge, GPT-OSS), network probing (relay connectivity), and workspace/project config loading. It produces an OanixManifest summarizing the agent's capabilities and provides situation assessment with recommended actions.

## onyx
The onyx crate is the local-first Markdown editor app. It uses WGPUI and the editor primitives to render inline formatting, manages vault/config persistence and file watching, and includes update checks plus optional voice transcription.

## opencode-sdk
The opencode-sdk crate is a generated Rust SDK for OpenCode servers. It provides REST and SSE clients for session control, event streaming, and file operations.

## orderbook
The orderbook crate is a NIP-69 orderbook viewer library with an optional GUI. It parses order events leniently, deduplicates by coordinate, groups markets, and renders terminal or GUI views.

## pylon
The pylon crate is the local runtime for sovereign agents. It supports host mode for running agents and provider mode for selling NIP-90 compute, manages relays and wallets, and exposes CLI commands including tunnel connect for the web UI.

## pylon-desktop
The pylon-desktop crate is the desktop GUI and CLI wrapper around Pylon. It embeds WGPUI and viz for FM Bridge visualization, runs a NIP-90 provider runtime, and can operate in headless CLI mode.

## recorder
The recorder crate parses, validates, and repairs rlog session logs. It also ships a CLI for stats, parsing, and formatting so trajectories remain auditable.

## relay
The relay crate defines the WebSocket protocol shared by the browser, worker, and tunnel client. It includes session registration/status structs and message envelopes for StartTask, Autopilot streaming, and Claude tunnel session control.

## relay-worker
The relay-worker crate is a Cloudflare Worker Nostr relay focused on the inference network. It implements NIP-01/11/28/32/42/90 routing with Durable Objects and D1 storage, and serves a minimal HTTP info page.

## rlm
The rlm crate is the Recursive Language Model execution engine. It provides the orchestration loop, command parsing, executor interfaces, span provenance tracking, and optional DSPy integration for recursive tool-driven reasoning over documents. It includes an MCP server binary (`rlm-mcp-server`) exposing `rlm_query` and `rlm_fanout` tools for Claude integration, and supports Claude as an LlmClient backend for RLM execution.

## rlm-methods
The rlm-methods crate implements the method variants used in the RLM paper (Base, Summary Agent, CodeAct+BM25, full RLM, and ablations). It adapts lm-router clients to bench-harness Method traits and bundles prompts/retrieval helpers.

## runtime
The runtime crate provides a pluggable execution environment for autonomous agents. It defines the tick model, identity/storage abstractions, filesystem-style mounts for compute/containers/Claude, and HUD event streaming, and ships adapters like `SparkWalletService` plus the NIP-90 `DvmProvider` for decentralized compute. Backends target local, browser, and cloud deployments.

## spark
The spark crate integrates Breez Spark payments for OpenAgents. It derives Bitcoin keys from the shared mnemonic, provides wallet configuration, and wraps Spark payment flows (Lightning, LNURL, and on-chain primitives) used by wallet and compute components.

## testing
The testing crate centralizes shared fixtures and helpers for integration tests across the workspace.

## vim
The vim crate is an editor-agnostic Vim emulation layer. It defines the VimEditor trait plus handlers for modes, motions, operators, and key parsing so UI surfaces can plug in Vim behavior.

## voice
The voice crate provides voice recording and transcription functionality using whisper.cpp via whisper-rs bindings. It includes audio capture, model management (auto-download of Whisper models), and a VoiceSession API with event callbacks for transcription progress and completion.

## voice-daemon
The voice-daemon crate is a macOS menu bar daemon for system-wide voice transcription. It registers global hotkeys (hold Right Command to record), transcribes via the voice crate, and pastes the result into any application. It supports start/stop/status subcommands and runs as a background daemon.

## viz
The viz crate defines the visualization grammar for execution HUDs. It provides primitives for fill/pulse/flow/heat/topology, trace event rendering, and FRLM-specific panels for budget/timeline visualizations.

## wallet
The wallet crate provides the unified OpenAgents wallet types tying Nostr identity to Bitcoin/Spark payments. It handles key derivation, storage helpers, and CLI-facing wallet models used by runtimes and web services.

## web
The web crate is a standalone WGPUI web demo showcasing GPU-accelerated text and Markdown rendering in the browser.

## web/client
The web/client crate is the WGPUI WASM client for the OpenAgents web app. It owns app state and view routing, renders repo selection and Autopilot chat UI, and speaks the relay protocol for tunnel sessions.

## web/worker
The web/worker crate is the Cloudflare Worker API for the web app, built with Axum on workers-rs. It implements OAuth, billing, Stripe, wallet, HUD, tunnel registration, and container orchestration routes backed by D1 and KV, and hosts the tunnel relay Durable Object.

## web/wallet-worker
The web/wallet-worker crate is a separate Cloudflare Worker for Spark wallet operations to stay under WASM size limits. It authenticates users via KV sessions, decrypts identity material from D1, and serves balance, receive, and send endpoints.

## wgpui
The wgpui crate is the GPU-accelerated UI library used across desktop and web. It provides a component/layout system, text rendering and Markdown views, input event handling, and wgpu-backed rendering for Winit and WebGPU/WebGL targets.
