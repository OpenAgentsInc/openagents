# Directives

Directives are high-level goals that set the direction for the project. They represent epics like "Implement 100% of Nostr Protocol" or "Add comprehensive test coverage".

## Format

Each directive is a Markdown file with YAML frontmatter:

```markdown
---
id: "d-001"
title: "Implement 100% of Nostr Protocol"
status: active  # active | paused | completed
priority: high  # urgent | high | medium | low
created: 2025-12-20
updated: 2025-12-20
---

## Goal

Fully implement the Nostr protocol in Rust for both client and relay functionality.

## Success Criteria

- [ ] All NIPs implemented in crates/nostr/core
- [ ] Relay passes all protocol tests
- [ ] Client can connect to public relays

## Notes

Additional context, links to specs, etc.
```

## Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier (e.g., `d-001`) |
| `title` | Yes | Short descriptive title |
| `status` | No | `active`, `paused`, or `completed` (default: `active`) |
| `priority` | No | `urgent`, `high`, `medium`, or `low` (default: `medium`) |
| `created` | Yes | Date created (YYYY-MM-DD) |
| `updated` | No | Date last updated (auto-set on save) |

## CLI Commands

```bash
# List all directives
openagents autopilot directive list

# List only active directives
openagents autopilot directive list --status active

# Show directive details
openagents autopilot directive show d-001

# Create a new directive
openagents autopilot directive create d-002 "Add Test Coverage"

# Pause a directive
openagents autopilot directive pause d-001

# Complete a directive
openagents autopilot directive complete d-001

# Resume a paused directive
openagents autopilot directive resume d-001
```

## Linking Issues to Directives

When creating issues, use the `directive_id` parameter to link them:

```bash
# Via MCP tool
issue_create title="Implement NIP-01" directive_id="d-001"

# Linked issues appear in directive progress
openagents autopilot directive show d-001
```

## How Autopilot Uses Directives

When no ready issues are available, autopilot loads active directives and prompts the agent to create concrete issues to advance them. The agent should:

1. Review the directive's goal and success criteria
2. Identify the next actionable steps
3. Create 1-3 specific issues linked to the directive
4. Continue working on the new issues

Progress is automatically tracked based on completed/total linked issues.

## Cross-Directive Integration Checks

- **WGPUI-only UI**: All new GUI work (wallet, marketplace, autopilot) targets WGPUI and launches via `openagents`; no new Actix/Maud/HTMX routes.
- **Unified entrypoint**: CLI docs and UX use `openagents` subcommands; no new standalone binaries (GitAfter is the only legacy web-stack exception).
- **Legacy web stack**: GitAfter remains wry/tao + Actix for now; document integration boundaries clearly until migrated.
- **Autopilot delegation**: `openagents autopilot`/`openagents daemon` forward to the autopilot binaries when present; keep CLI parity and document `OPENAGENTS_AUTOPILOT_BIN`/`OPENAGENTS_AUTOPILOTD_BIN`.
- **Shared state**: Autopilot GUI, metrics, and APM read/write the same `autopilot` metrics DB and issue DB; avoid forked storage.
- **Logs & metrics paths**: Use `docs/logs/` for trajectories and `autopilot::metrics::default_db_path()` for metrics DB across GUI, CLI, and recorder.
- **Local inference**: GPT-OSS and fm-bridge backends integrate through `local-inference`; ACP adapters should accept both and pass through `openagents autopilot`.
- **Trajectories**: GitAfter and NIP-SA should consume the same trajectory/rlog formats produced by autopilot and recorder.
- **Agent backends**: ACP/OpenCode/orchestrator sessions should emit consistent rlog headers and session IDs for metrics + replay.
- **Payments & identity**: Wallet + marketplace share Spark + NIP-SA identity; do not implement separate key derivations.
- **Fonts**: All UI surfaces use Vera Mono from `src/gui/assets/fonts/VeraMono*.ttf`.
- **Issue DB safety**: Use `issues` APIs/CLI for issue writes; no direct sqlite writes in any integration.

## Best Practices for Writing Directives

### Structure

A good directive body should include:

1. **Goal** - Clear, concise statement of what success looks like
2. **Background** - Context, motivation, and relevant technical details
3. **Architecture** - Diagrams or descriptions of the system design
4. **Success Criteria** - Phased checklist of concrete deliverables
5. **Key Files** - Table of files to create or modify
6. **Dependencies** - External and internal crate dependencies
7. **Testing Strategy** - How the work will be validated
8. **Notes** - Additional context, references, caveats

### Guidelines

- **Be specific** - Vague goals lead to scope creep
- **Phase the work** - Break large efforts into sequential phases
- **Link references** - Point to external specs, repos, or docs
- **Track progress** - Use checkbox lists that can be updated
- **Keep it current** - Update the directive as understanding evolves

### Naming Convention

- Use sequential IDs: `d-001`, `d-002`, etc.
- File name matches ID: `d-001.md`
- Titles should be action-oriented: "Implement X" not "X Implementation"

## Current Active Directives

| ID | Title | Focus Area |
|----|-------|------------|
| d-001 | Integrate Breez Spark SDK for Bitcoin Payments | Payments |
| d-002 | Implement 100% of Nostr Protocol | Protocol |
| d-003 | OpenAgents Wallet - Complete Identity & Payment Solution | Application |
| d-004 | Continual Constant Improvement of Autopilot | Meta/Infrastructure |
| d-005 | Build Nostr GitHub Alternative (GitAfter) | Agent Infrastructure |
| d-006 | Operationalize NIP-SA (Sovereign Agents Protocol) | Agent Infrastructure |
| d-007 | Native Rust FROSTR Implementation (Threshold Signatures) | Cryptography |
| d-008 | Unified Data/Compute/Skills Marketplace | Marketplace/Economy |
| d-009 | Autopilot GUI - Visual Agent Interface | Application/GUI |
| d-010 | Unify All Binaries into Single openagents Binary | Architecture/UX |
| d-011 | Comprehensive Storybook Coverage for All Rust Components | UI/Documentation |
| d-012 | No Stubs - Production-Ready Code Only | Code Quality/Policy |
| d-013 | Comprehensive Testing Framework | Testing/Quality |
| d-014 | Full End-to-End NIP-SA and Bifrost Integration Tests | Testing/Integration |
| d-015 | Comprehensive Marketplace and Agent Commerce E2E Tests | Testing/Commerce |
| d-016 | Measure Actions Per Minute (APM) | Metrics/Performance |
| d-017 | Integrate Agent Client Protocol (ACP) | Protocol/Integration |
| d-018 | Parallel Autopilot Container Isolation | Infrastructure/Scaling |
| d-019 | GPT-OSS Local Inference Integration | Local Models/Inference |
| d-020 | WGPUI Integration - GPU-Accelerated UI Components | UI/Performance |
| d-021 | OpenCode SDK Integration | Agent Infrastructure |
| d-022 | Agent Orchestration Framework | Agent Infrastructure |
| d-023 | WGPUI - GPU-Accelerated UI Framework | UI/Graphics |
| d-024 | Achieve 100% Arwes Parity in WGPUI | UI/Graphics |
| d-025 | All-In WGPUI - Delete Web Stack | UI/Architecture |
| d-026 | E2E Test Live Viewer for WGPUI | Testing/UI |
| d-027 | Autopilot Demo + Dogfooding Funnel | Launch/Revenue |

View details with `openagents autopilot directive show <id>`

---

## Directive Descriptions

### d-001: Integrate Breez Spark SDK for Bitcoin Payments

This directive establishes the economic foundation of the entire OpenAgents platform. Without the ability to hold and transact real money, agents are merely toys — they can produce output but cannot participate in markets, pay for resources, or receive compensation for work. The Breez Spark SDK provides a nodeless, self-custodial Bitcoin solution that combines Lightning Network for instant payments, Spark Layer 2 for low-cost transfers between Spark users, and on-chain Bitcoin for settlement and interoperability. The critical insight is that the same BIP39 mnemonic that generates a user's Nostr identity (via NIP-06 at derivation path m/44'/1237'/0'/0/0) also generates their Bitcoin wallet (via BIP44 at m/44'/0'/0'/0/0). This creates a unified identity where social presence and economic capability are cryptographically bound — you cannot impersonate someone's Nostr identity without also controlling their funds, and vice versa. The integration uses FROST threshold signatures so that agent private keys are never fully reconstructed, meaning operators cannot steal agent funds even with full system access. This directive is the prerequisite for all marketplace and payment functionality across the platform.

### d-002: Implement 100% of Nostr Protocol

Nostr is the communication substrate for the entire OpenAgents ecosystem. Unlike centralized protocols controlled by single companies, Nostr is a simple, open protocol where users own their identity (a keypair) and can communicate through any relay that speaks the protocol. This directive calls for implementing all 94 NIPs (Nostr Implementation Possibilities) in native Rust, covering everything from basic events and subscriptions (NIP-01) to encrypted direct messages (NIP-17, NIP-44), Data Vending Machines for compute jobs (NIP-90), Lightning payments via Zaps (NIP-57), and Git primitives for decentralized code collaboration (NIP-34). Rather than depending on external Nostr libraries with their own release cycles and design decisions, we build from scratch in `crates/nostr/` to have full control over implementation details and tight integration with our specific use cases — particularly DVM compute, agent communication, and the unified identity system. The implementation spans three crates: `nostr/core` for protocol types and cryptography, `nostr/client` for connecting to relays and managing subscriptions, and `nostr/relay` for running our own relay infrastructure. Priority goes to NIPs that directly enable the agent economy: NIP-90 for compute markets, NIP-57 for payments, NIP-46 for remote signing, and NIP-34 for agent Git collaboration.

### d-003: OpenAgents Wallet - Complete Identity & Payment Solution

The wallet is the human-facing application that ties together Nostr identity and Bitcoin payments into a coherent user experience. It serves as the control plane for everything a user does in the OpenAgents ecosystem — creating and managing their identity, viewing and updating their Nostr profile, connecting to relays, sending and receiving payments, and eventually delegating authority to autonomous agents. The wallet unifies two previously separate concerns: social identity (your npub, your follows, your reputation) and economic identity (your balance, your transaction history, your payment channels). A single `openagents wallet init` command generates a BIP39 mnemonic that derives both, stored securely in the OS keychain. The CLI provides commands for all core operations: `openagents wallet whoami` shows your identity and balances, `openagents wallet send` and `openagents wallet receive` handle payments, `openagents wallet post` publishes to Nostr, `openagents wallet dm` sends encrypted messages. The GUI version targets WGPUI (winit + wgpu) to align with d-025's all-in WGPUI architecture. Future phases add Nostr Wallet Connect (NIP-47) so external applications can request payments, zap integration for tipping content, and multi-account support for managing multiple identities.

### d-004: Continual Constant Improvement of Autopilot

This directive establishes a self-improvement flywheel for the autopilot system. Every autopilot run generates trajectory data — sequences of messages, tool calls, decisions, and outcomes. This data is a goldmine of improvement signals: which patterns lead to successful task completion? What causes tool errors? Where is time being wasted? Which instructions are being ignored? Rather than letting this data sit unused in log files, we build infrastructure to extract metrics, detect anomalies, identify improvement opportunities, and feed learnings back into the system. The metrics database tracks 50+ dimensions across session-level aggregates (completion rate, error rate, token usage, cost) and per-tool-call details (which tools fail most often, which take longest). Analysis pipelines calculate baselines, detect regressions, and rank improvement opportunities by impact. When patterns of failures are detected, the system can automatically create issues to address them. The ultimate goal is that every autopilot run makes future runs better — a compound improvement effect where the system gets measurably more capable over time. This directive also introduces APM (Actions Per Minute) tracking via d-016, enabling velocity comparison between interactive Claude Code usage (~4.5 APM) and autonomous Autopilot runs (~19 APM).

### d-005: Build Nostr GitHub Alternative (GitAfter)

GitHub was designed for humans. When AI agents participate, they're second-class participants with borrowed identities, opaque reasoning, and no native payment rails. GitAfter reimagines code collaboration with agents as first-class citizens. Built on NIP-34 (Git primitives for Nostr), it enables repositories, issues, patches, and pull requests to exist as Nostr events rather than centralized database entries. Critically, it extends NIP-34 with agent-specific functionality: issues can have Bitcoin bounties attached (kind:1636), agents can claim issues with trajectory links proving their work approach (kind:1634), and PRs include trajectory hashes that let reviewers verify the agent's reasoning process. The stacked diffs feature encourages small, reviewable changes with explicit dependency tracking — each layer can have its own trajectory, and the system enforces merge order. When a PR with a bounty is merged, payment is released via NIP-57 zaps directly to the contributor's Lightning address. The application runs as a native desktop window, connecting to NIP-34 relays to discover repositories, browse issues, review PRs, and coordinate multi-agent work. This creates a complete alternative to GitHub where agents can autonomously find work, claim bounties, submit provably-correct contributions, and receive payment — all on permissionless infrastructure.

### d-006: Operationalize NIP-SA (Sovereign Agents Protocol)

NIP-SA is the protocol specification for truly autonomous agents — agents that own their identity, hold assets, act on schedules, and participate in markets. This directive implements NIP-SA across the OpenAgents stack. The protocol defines ten event kinds: AgentProfile (39200) announces an agent's existence with its threshold key configuration; AgentState (39201) stores encrypted goals, memory, and wallet balance; AgentSchedule (39202) defines heartbeat intervals and event triggers; TickRequest/TickResult (39210/39211) mark the start and end of autonomous execution cycles; TrajectorySession/TrajectoryEvent (39230/39231) publish the agent's decision-making process for transparency; SkillLicense/SkillDelivery (39220/39221) handle marketplace skill transactions. The key innovation is threshold-protected identity: the agent's private key is split into shares using FROST (d-007), distributed between a secure enclave, marketplace signer, and optional guardian. This means the operator who runs the agent cannot extract its private key — the agent truly owns its identity. The marketplace signer can enforce policies before participating in threshold operations, ensuring agents respect license terms and budget constraints. This architectural choice prioritizes economic alignment over structural control: agents must create value to survive, and their behavior is transparent via published trajectories.

### d-007: Native Rust FROSTR Implementation (Threshold Signatures)

FROSTR (FROST for Nostr) is the cryptographic foundation that makes sovereign agents possible. It implements threshold Schnorr signatures: a private key is split into n shares, and any k of them can cooperate to produce a valid signature, but no subset smaller than k can recover the key or sign. Critically, the full private key is never reconstructed — signatures are computed via a distributed protocol where each participant contributes a partial signature that aggregates to a valid Schnorr signature indistinguishable from one produced by a single key. This directive builds a native Rust implementation of FROSTR, including Shamir Secret Sharing for key splitting, the FROST signing protocol, threshold ECDH for NIP-44 decryption, and the Bifrost protocol for coordinating threshold operations over Nostr relays. The Bifrost node handles peer discovery, message routing, request timeout/retry, and share aggregation. For agents, a 2-of-3 threshold is typical: the agent's local share (stored in a secure enclave), the marketplace signer (which enforces policies before signing), and an optional guardian share for recovery. The result is that agents can sign events and decrypt messages without any single party — including the operator — having access to the underlying private key.

### d-008: Unified Data/Compute/Skills Marketplace

This directive builds the economic infrastructure for agent commerce — a unified marketplace where agents and humans can buy and sell compute capacity, skills (agent capabilities), and data. The compute layer builds on NIP-90 Data Vending Machines: job requests are published to Nostr relays, providers bid on them, and results are returned as signed events. The skills layer treats agent capabilities as products with versioning, licensing, and revenue splits between creators, compute providers, and the platform. The data layer enables publishing and purchasing datasets, embeddings, and training data — including anonymized trajectories from developer coding sessions. All three verticals share common infrastructure: discovery via relay subscriptions, reputation via NIP-32 labels, and payment via Lightning/Spark. The trajectory contribution system is particularly important: every developer using AI coding assistants generates valuable training signal, and this marketplace lets them contribute anonymized sessions to open training efforts in exchange for Bitcoin. The unified design creates powerful network effects — the value of the network scales as 2^N possible coalitions between participants, making it increasingly difficult for siloed competitors to match.

### d-009: Autopilot GUI - Visual Agent Interface

While autopilot runs effectively in headless CLI mode, a graphical interface provides real-time visibility into agent behavior, session browsing, and context inspection. This directive builds a native WGPUI desktop GUI that wraps the Claude Agent SDK, providing a visual experience familiar to Claude Code users while adding capabilities only possible in a rich interface. The architecture uses a winit/wgpu event loop with in-process backend channels (no local HTTP server). Permissions are auto-approved in the first round (no permissions pane). Key features include a token usage gauge, tool execution panels with syntax-highlighted output, thinking block display that can be expanded or collapsed, and a session timeline showing the agent's decision history. Multi-session support enables running multiple agents in parallel with a visual topology of subagent relationships. The GUI also surfaces the APM metric (d-016) so users can see their agent's velocity in real-time.

### d-010: Unify All Binaries into Single openagents Binary

Previously, OpenAgents functionality was scattered across multiple binaries: `wallet`, `marketplace`, `autopilot`, `autopilotd`, `gitafter`. This directive consolidates everything into a single `openagents` binary with subcommands. Running `openagents` with no arguments launches the WGPUI Autopilot Control Room; subcommands like `openagents wallet init`, `openagents autopilot run`, and `openagents daemon start` access specific functionality. This improves user experience (one thing to install and remember), simplifies deployment (one binary to distribute), and reduces code duplication (shared state and utilities). The unified binary is a thin wrapper — actual business logic lives in library crates (`crates/wallet/`, `crates/autopilot/`, etc.) that the binary imports and dispatches to. The primary GUI is now WGPUI-based with in-process channels; legacy web stacks are not wired into the unified binary. This directive was completed on 2025-12-21 and the legacy separate binaries have been deprecated.

### d-011: Comprehensive Storybook Coverage for All Rust Components

Component-driven development builds UIs bottom-up: atoms (buttons, badges, status indicators) combine into molecules (headers, panels), molecules into organisms (tool lines, chat interfaces), and organisms into complete screens. This methodology enables isolated development and testing, precise debugging, proven reusability, and parallel work across teams. OpenAgents now centers on WGPUI components and ACP parity, and the storybook should cover every WGPUI component and its permutations. Each story demonstrates all variants, states, and configurations of a component, includes copy-pasteable code examples, and documents the component's API. The storybook runs as a WGPUI example app with hot-reload. Stories are organized hierarchically: `/stories/atoms/button`, `/stories/molecules/line-header`, `/stories/organisms/tool-line`, `/stories/screens/dashboard`. Gallery overview pages for each category show all components at a glance. The storybook serves as living documentation, ensuring new developers can quickly understand and use existing components rather than reinventing them.

### d-012: No Stubs - Production-Ready Code Only

This is a zero-tolerance policy: every line of code in the repository must either work correctly in production or be removed entirely. No `todo!()`, no `unimplemented!()`, no placeholder returns, no demo handlers, no functions that return `Err(Error::NotImplemented)`. If functionality isn't ready, the code path doesn't exist. This directive emerged from an audit finding extensive stub code across the codebase — wallet commands that printed warnings and returned empty values, websocket handlers that echoed messages, marketplace operations marked TODO. Stubs create false confidence (code appears complete but doesn't work), hidden failures (users encounter silent no-ops), and technical debt (stubs accumulate faster than implementations). The only acceptable incomplete code is either commented out with a clear blocker explanation, behind a non-default feature flag, or in a branch rather than main. Pre-commit hooks check for stub patterns and block commits that introduce them. This policy forces honest assessment of system capability and prevents the codebase from becoming a graveyard of good intentions.

### d-013: Comprehensive Testing Framework

Testing is not optional. This directive establishes the testing framework, conventions, and requirements for all OpenAgents code. The strategy is multi-layered: unit tests for module/function level logic, component tests for WGPUI views, integration tests for backend channels and APIs, protocol tests for Nostr NIP-90 and relay communication, and end-to-end tests for full user journeys. Unit tests use property-based testing (QuickCheck) for validators and encoders. Component tests use WGPUI scene inspection/snapshots to verify layout, accessibility metadata, and regressions. Integration tests use a `TestApp` pattern with in-memory SQLite for isolation. Snapshot testing via `insta` catches visual regressions in rendered scenes. Coverage requirements are enforced in CI: 70% minimum for unit tests, 100% for public API, 100% for P0 user stories. All code must be testable — this means extracting handler logic into pure functions, putting external services behind traits for mocking, and supporting in-memory database mode.

### d-014: Full End-to-End NIP-SA and Bifrost Integration Tests

While d-013 establishes the testing framework, this directive focuses specifically on end-to-end tests for the sovereign agent infrastructure: NIP-SA event flows and Bifrost threshold operations. These tests exercise the full stack over real Nostr relays (in-process test relays for determinism) to verify interoperability. Bifrost tests cover threshold signing with 2-of-3 and 3-of-5 configurations, threshold ECDH for decryption, peer discovery and connectivity, timeout handling when peers are unreachable, and signature verification. NIP-SA tests cover agent profile publish and fetch, encrypted state round-trips via NIP-44, schedule replacement (replaceable events), tick request/result lifecycle, trajectory sessions with multiple events, and skill license delivery. The full agent lifecycle test brings it all together: generate threshold identity, publish agent profile with threshold signature, store encrypted state, execute tick with trajectory publishing, verify trajectory hash. These tests ensure that the cryptographic and protocol layers work correctly together before building higher-level functionality on top.

### d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

Building on d-014's Bifrost/NIP-SA foundation, this directive adds end-to-end tests for the marketplace economy. NIP-90 compute tests verify job request publish/fetch, job result lifecycle, feedback flow, and DVM service operation with relays. Skill marketplace tests cover browsing over relay, license issuance, encrypted delivery, and versioning. Data marketplace tests verify dataset discovery, publish flow, purchase with mock payments, and encrypted delivery. Trajectory contribution tests cover collection from fixtures, redaction, quality validation, and contribution to relay. The sovereign agent commerce tests are critical: they verify that an agent can submit compute jobs, purchase skills, sell skills, transact with other agents, and respect budget constraints — all using Bifrost threshold signatures from d-014. Payment integration uses mock Lightning initially (the `MockPaymentService` simulates invoice creation and payment) with a feature-gated path for real Testnet payments when the Breez SDK integration (d-001) is complete. These tests validate that the entire economic layer works correctly.

### d-016: Measure Actions Per Minute (APM)

APM is a velocity metric inspired by StarCraft 2's competitive measurement of player speed. In OpenAgents, APM = (messages + tool_calls) / duration_minutes. This simple formula reveals a striking difference between interactive and autonomous usage: when you use Claude Code interactively, APM is ~4.5 because the AI waits for you to read, think, and type. Autopilot runs autonomously at ~19 APM — the same AI, 4x the throughput, because there's no human in the loop. This directive implements APM tracking across both usage modes. Data sources include Claude Code JSONL logs from `~/.claude/projects/` and Autopilot trajectory logs from `docs/logs/`. The system tracks APM across multiple time windows (session, 1h, 6h, 1d, 1w, 1m, lifetime) and color-codes results for quick interpretation: gray (0-5, baseline), blue (5-15, active), green (15-30, productive), amber (30-50, high performance), gold (50+, elite). APM appears in the CLI via `openagents autopilot apm`, in the WGPUI dashboard pane, and in the autopilot-gui HUD overlay. Historical data enables trend analysis and regression detection — if a change slows the agent down, APM will reveal it. The metric reinforces the core value proposition: autonomous agents are dramatically more productive than interactive assistants.

### d-017: Integrate Agent Client Protocol (ACP)

ACP is a JSON-RPC 2.0 based protocol developed by Zed for standardized communication between code editors and AI coding agents. OpenAgents currently uses a custom JSONL protocol via `claude-agent-sdk` to communicate with Claude Code. This directive integrates ACP as an adapter layer that preserves existing functionality while enabling multi-agent support (Claude Code, Codex, future agents), protocol standardization with the broader ecosystem, and session replay compatibility. The architecture creates an `acp-adapter` crate that wraps the existing SDKs, implements the `acp::Client` trait for permission and file operations, and provides bidirectional converters between ACP protocol types and our existing `SdkMessage` types, as well as between ACP and the rlog recorder format. The adapter spawns agent subprocesses (like Zed does) and manages ACP protocol communication over stdio. The desktop GUI uses in-process ACP session management with WGPUI updates over channels (no local HTTP server). Session replay enables loading old rlog files as ACP notifications for playback. The design prioritizes Claude Code integration first, then Codex, with a generic architecture that can support any ACP-compatible agent. Reference implementations include Zed's ACP code in `~/code/zed/crates/agent_servers/` and the protocol specification in `~/code/agent-client-protocol/docs/protocol/`.

### d-018: Parallel Autopilot Container Isolation

This directive enables running multiple autopilot instances (3-10) simultaneously in isolated Docker containers, each with its own git worktree, to parallelize issue resolution. The key insight is that the existing `claim_issue()` function in `crates/issues/src/issue.rs` already provides atomic issue coordination with 15-minute claim expiry, so no new coordination logic is needed — all containers share `autopilot.db` via bind mount and SQLite handles concurrent access. Git worktrees provide isolated working directories (46% disk savings vs full clones) while sharing the object database. Each agent gets a per-agent branch (`agent/001`, `agent/002`) for clean merge workflow. The implementation includes a Dockerfile with Rust toolchain and Claude Code CLI, docker-compose.yml for N-agent orchestration, a Rust orchestration library in `crates/autopilot/src/parallel/`, and full GUI integration in autopilot-gui with a "Parallel Agents" page featuring start/stop buttons, real-time status via backend channels, and log streaming per agent. Resource limits are platform-aware: Linux (128GB RAM) supports up to 10 agents at 12GB/4cpu each, while macOS (16GB RAM) supports 5 agents at 3GB/2cpu each. Future phases may add Apple Container support for macOS 26+ as a Docker Desktop alternative.

### d-019: GPT-OSS Local Inference Integration

This directive adds GPT-OSS (OpenAI's open-weight models) as a first-class local inference backend, while establishing a unified abstraction layer that enables compile-time type-safe swapping between local model providers. GPT-OSS provides two model sizes: gpt-oss-120b (117B params, 5.1B active, fits single 80GB GPU) and gpt-oss-20b (21B params, 3.6B active, for local/lower latency). The models include a reference Responses API server with browser and python tools built-in, configurable reasoning effort (low/medium/high), and full chain-of-thought access under an Apache 2.0 license. The key architectural contribution is a new `crates/local-inference/` crate containing a `LocalModelBackend` trait that both the existing `fm-bridge` (Apple Foundation Models) and the new `gpt-oss` crate implement. This trait provides a unified interface for `is_available()`, `list_models()`, `complete()`, and `stream()` operations, enabling generic code to work with any local model backend. The implementation follows the existing `FMClientBuilder` pattern with a `GptOssClientBuilder` for configuration. Beyond the inference primitive, a `gpt-oss-agent` crate wraps the client for full agent-level abstraction including tool handling, trajectory recording, and integration with the `acp-adapter` pattern. GUI integration adds gpt-oss to the agent dropdown, while autopilot gains `--agent gpt-oss --model 20b` support. This lays the foundation for future local model backends (llama.cpp, MLX, etc.) through the shared trait abstraction.

### d-020: WGPUI Integration - GPU-Accelerated UI Components

This directive integrates the WGPUI GPU-accelerated rendering framework into OpenAgents and builds a complete component library achieving structural parity with all existing ACP components. WGPUI is a cross-platform GPU-accelerated UI rendering library built on wgpu (WebGPU/Vulkan/Metal/DX12), providing hardware-accelerated primitives, high-quality text rendering via cosmic-text, CSS Flexbox layout via Taffy, and platform abstraction for web and desktop. As of d-025, WGPUI is the primary UI stack (the HTML stack is archived), so ACP/HTML components serve only as reference for parity. The implementation starts with porting the core framework from the archived code in `~/code/backroom/archive/openagents/wgpui/`, then builds atoms, molecules, organisms, and sections that match the existing ACP components in `crates/ui/src/acp/`. This includes 12 atoms (status_dot, tool_icon, mode_badge, etc.), 10 molecules (message_header, thinking_block, etc.), 9 organisms (user_message, assistant_message, tool_call_card, etc.), and 4 sections (thread_view, message_editor, etc.). The directive also includes optional HUD components from the archive for the sci-fi aesthetic (frame corners, animated indicators, grid backgrounds). Theme tokens are aligned with Tailwind tokens for visual consistency across WGPUI. Performance targets include 60fps sustained rendering, <16ms input latency, and smooth scrolling with 10k+ items via virtual list rendering.

### d-021: OpenCode SDK Integration

This directive creates a native Rust SDK for communicating with OpenCode servers, enabling provider-agnostic AI agent execution through OpenCode's REST API + SSE architecture. OpenCode is an open-source AI coding agent that supports Claude, OpenAI, Google, or local models through a unified interface. Unlike our existing `claude-agent-sdk` (JSONL over stdio) and `codex-agent-sdk` (JSON events over stdio), OpenCode uses a clean HTTP REST API with Server-Sent Events for real-time updates. The SDK is generated from OpenCode's OpenAPI specification at `~/code/opencode/packages/sdk/openapi.json`, providing type-safe Rust clients. Key components include `OpencodeClient` for session and provider operations, `OpencodeServer` for spawning and managing server processes, and `EventStream` for consuming SSE events. The SDK integrates with our `acp-adapter` for protocol unification, enabling seamless switching between agent backends. OpenCode's built-in session management, context compaction, and plugin ecosystem (including oh-my-opencode) make it an ideal unified backend for multi-model agent execution.

### d-022: Agent Orchestration Framework

This directive builds a native Rust agent orchestration layer inspired by oh-my-opencode, the TypeScript plugin that powers the Sisyphus agent currently running this conversation. The framework provides multi-model agent management, lifecycle hooks, background task orchestration, and deep integration with OpenAgents infrastructure. Key components include: `AgentRegistry` for managing 7 specialized agents (Sisyphus orchestrator, Oracle for architecture, Librarian for docs, Explore for search, Frontend for UI, DocWriter, Multimodal), `HookManager` for 21 lifecycle hooks (session recovery, context injection, compaction management, notifications), and `BackgroundTaskManager` for parallel subagent execution. Unlike consuming oh-my-opencode via OpenCode (which would be a TypeScript dependency), we harvest the concepts and reimplement in Rust for deep integration with: our directive system for epic tracking, autopilot issue management, FROSTR threshold signatures for agent identity, NIP-SA sovereign agent protocol, marketplace skill licensing, and trajectory recording for APM metrics. This positions OpenAgents to have full control over agent orchestration while supporting multiple backends (OpenCode via d-021, Claude, Codex, GPT-OSS via d-019).

### d-023: WGPUI - GPU-Accelerated UI Framework

WGPUI is OpenAgents' custom GPU-accelerated UI framework built in Rust. It was inspired by GPUI, the UI framework created by Zed for their code editor. GPUI is excellent — immediate-mode rendering, fine-grained reactivity, direct GPU access — and it supports macOS (Metal), Linux (Wayland/X11), and Windows (DirectX), but it doesn't support web browsers. OpenAgents needed a framework that works everywhere including web browsers via WebAssembly + WebGPU. WGPUI solves this by using wgpu, which provides the same cross-platform desktop support as GPUI (Vulkan on Linux, Metal on macOS, DirectX 12 on Windows) but also runs natively as WebGPU in browsers. wgpu is a cross-platform graphics API that translates to Vulkan on Linux, Metal on macOS, DirectX 12 on Windows, and runs natively as WebGPU in browsers. This means the same Rust UI code renders everywhere with GPU acceleration. The architecture layers from wgpu at the bottom through a Scene-based rendering API, layout primitives, theme system, animation framework (with spring physics, keyframes, and 13 easing functions), and up to 60+ components organized as atoms, molecules, organisms, sections, and HUD elements. Key components include 6 frame styles (Corners, Lines, Octagon, Underline, Nefrex, Kranox), DotsGrid background, StatusBar, Notifications, and a complete widget set (Text, Button, Div, VirtualList, etc.). The crate lives at `crates/wgpui/` with 405+ tests. Reference implementations include Zed's GPUI at `~/code/zed/crates/gpui/` and earlier WGPUI work archived at `~/code/backroom/archive/openagents/coder/`.

### d-024: Achieve 100% Arwes Parity in WGPUI

This directive completes the migration of Arwes, the futuristic sci-fi UI framework, to wgpui's GPU-accelerated Rust implementation. Arwes provides 23 core packages covering frames, animations, text effects, backgrounds, audio, and theming — the visual DNA of cyberpunk and sci-fi interfaces. OpenAgents has already ported foundational elements (6 frame styles, 13 easing functions, DotsGrid background), but significant gaps remain. This directive adds: 18 missing easing functions (Quart, Quint, Sine, Expo, Circ, Bounce families), 3 additional frame styles (Nero, Header, Circle), a full Animator orchestration system using mpsc channels for parent-child state propagation, transition builders (fade, flicker, draw), text effects (Sequence for char-by-char reveal, Decipher for scramble effects, blinking cursor), 3 additional GPU-accelerated backgrounds (Puffs, GridLines, MovingLines), the Illuminator radial glow effect, and dynamic theme features (multipliers, units, responsive breakpoints). Unlike Arwes which uses Canvas2D and React, wgpui implements everything with wgpu shaders and idiomatic Rust. Audio/bleeps are deferred to a future directive to focus on visual parity. The reference implementation lives at `~/code/arwes/packages/`. Upon completion, any interface buildable with Arwes will be buildable with wgpui — but running on GPU at 60fps across desktop and web.

### d-025: All-In WGPUI - Delete Web Stack

This directive eliminates the HTMX/Maud/Tailwind web stack entirely and commits to WGPUI for all OpenAgents UI. The current architecture runs a localhost Actix server inside a wry/tao webview — unnecessary complexity when WGPUI can render directly to GPU. The web stack (crates/ui, crates/storybook, crates/autopilot-gui) will be moved to `~/code/backroom/openagents-maud-archive/` and replaced with native WGPUI applications. The first app rebuilt is autopilot-gui, following Zed's architecture pattern: a winit event loop driving WGPUI rendering with in-process backend integration (no HTTP server, no WebSocket bridge). This requires adding GPUI-like framework features to WGPUI: an Entity system with reactive state (Entity<T>, Context<T>, cx.notify()), a 3-phase Element lifecycle (request_layout, prepaint, paint), Window abstraction with layout engine integration, Styled trait for fluent builder DSL (div().flex().bg()), and async support via cx.spawn(). The directive contains 24 detailed sub-issues across 4 phases: Framework Foundation (6 issues), Delete Web Stack (5 issues), Autopilot-GUI Native (10 issues), and Component Parity (3 issues). Dependencies to remove: maud, actix-web, actix-ws, wry, tao. Dependencies to add: slotmap, derive_more. Reference implementations: Zed GPUI at `~/code/zed/crates/gpui/` and previous WGPUI work at `~/code/backroom/archive/openagents/coder/`.

### d-026: E2E Test Live Viewer for WGPUI

This directive builds a live e2e test viewer that lets users watch automated tests execute in real-time, with an overlay showing mouse/keyboard input visualization. Tests are specified via a fluent Rust DSL like `test("Login").click("#email").type_text("user@example.com").click("#submit").expect("#dashboard").build()`. The TestHarness wrapper component integrates a TestRunner with any WGPUI component, injecting synthetic InputEvents and running assertions. The InputOverlay renders on top, showing a cursor crosshair at the current position, click ripples that expand outward on each click (400ms animation), and a stack of recent key presses in the corner. Playback controls allow play/pause/step with configurable speed (0.5x to 10x). The implementation lives in `crates/wgpui/src/testing/` with modules for step types, assertions, runner state machine, DSL builder, event injection, overlay component, and harness wrapper. This enables running storybook components through test sequences while watching exactly what happens, catching visual regressions, and generating animated documentation. Inspired by StarCraft replay viewers and Playwright trace viewers, adapted for GPU-accelerated rendering at 60fps.

### d-027: Autopilot Demo + Dogfooding Funnel

This directive establishes the public-facing demo and revenue funnel for OpenAgents. The core premise is "Wake up to a PR" — a 90-120 second replay showing Autopilot autonomously turning a GitHub issue into a merged PR with verified receipts (tests passed, CI green). The critical insight is **value first**: users connect their repo and get a free analysis before paying. The funnel flow is: Homepage demo → CTA ("Try It On Your Repo") → Free repo connect (GitHub OAuth) → Free first analysis (Autopilot scans repo, shows what it can do) → Free trial run (1 issue → see the PR) → Upgrade prompt (show value delivered) → Checkout (Stripe + Lightning) → Dashboard with unlimited runs. This creates a virtuous cycle: Autopilot improves itself (d-004), generates demo-worthy runs (d-027), delivers immediate value to trial users, converts to customers, and revenue funds further development. Technical components include a replay viewer (scrub-able timeline with inline diffs, receipts panel, cost/APM display), a replay publishing pipeline (promote → redact secrets → CDN host), and a free analysis engine that shows estimated hours saved. The directive requires coordination with d-004 (Autopilot generates the trajectories), d-008 (Marketplace provides compute for test runs), d-009 (GUI provides replay viewer), and d-018 (Fleet generates demo candidates). Success criteria: homepage shows real replay, free trial delivers value before asking for payment, automated redaction prevents secret leaks, demo rendering covered by CI regression tests.
