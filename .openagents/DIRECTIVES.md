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
cargo autopilot directive list

# List only active directives
cargo autopilot directive list --status active

# Show directive details
cargo autopilot directive show d-001

# Create a new directive
cargo autopilot directive create d-002 "Add Test Coverage"

# Pause a directive
cargo autopilot directive pause d-001

# Complete a directive
cargo autopilot directive complete d-001

# Resume a paused directive
cargo autopilot directive resume d-001
```

## Linking Issues to Directives

When creating issues, use the `directive_id` parameter to link them:

```bash
# Via MCP tool
issue_create title="Implement NIP-01" directive_id="d-001"

# Linked issues appear in directive progress
cargo autopilot directive show d-001
```

## How Autopilot Uses Directives

When no ready issues are available, autopilot loads active directives and prompts the agent to create concrete issues to advance them. The agent should:

1. Review the directive's goal and success criteria
2. Identify the next actionable steps
3. Create 1-3 specific issues linked to the directive
4. Continue working on the new issues

Progress is automatically tracked based on completed/total linked issues.

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
| d-005 | Build Nostr GitHub Alternative (AgentGit) | Agent Infrastructure |
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

View details with `cargo autopilot directive show <id>`

---

## Directive Descriptions

### d-001: Integrate Breez Spark SDK for Bitcoin Payments

This directive establishes the economic foundation of the entire OpenAgents platform. Without the ability to hold and transact real money, agents are merely toys — they can produce output but cannot participate in markets, pay for resources, or receive compensation for work. The Breez Spark SDK provides a nodeless, self-custodial Bitcoin solution that combines Lightning Network for instant payments, Spark Layer 2 for low-cost transfers between Spark users, and on-chain Bitcoin for settlement and interoperability. The critical insight is that the same BIP39 mnemonic that generates a user's Nostr identity (via NIP-06 at derivation path m/44'/1237'/0'/0/0) also generates their Bitcoin wallet (via BIP44 at m/44'/0'/0'/0/0). This creates a unified identity where social presence and economic capability are cryptographically bound — you cannot impersonate someone's Nostr identity without also controlling their funds, and vice versa. The integration uses FROST threshold signatures so that agent private keys are never fully reconstructed, meaning operators cannot steal agent funds even with full system access. This directive is the prerequisite for all marketplace and payment functionality across the platform.

### d-002: Implement 100% of Nostr Protocol

Nostr is the communication substrate for the entire OpenAgents ecosystem. Unlike centralized protocols controlled by single companies, Nostr is a simple, open protocol where users own their identity (a keypair) and can communicate through any relay that speaks the protocol. This directive calls for implementing all 94 NIPs (Nostr Implementation Possibilities) in native Rust, covering everything from basic events and subscriptions (NIP-01) to encrypted direct messages (NIP-17, NIP-44), Data Vending Machines for compute jobs (NIP-90), Lightning payments via Zaps (NIP-57), and Git primitives for decentralized code collaboration (NIP-34). Rather than depending on external Nostr libraries with their own release cycles and design decisions, we build from scratch in `crates/nostr/` to have full control over implementation details and tight integration with our specific use cases — particularly DVM compute, agent communication, and the unified identity system. The implementation spans three crates: `nostr/core` for protocol types and cryptography, `nostr/client` for connecting to relays and managing subscriptions, and `nostr/relay` for running our own relay infrastructure. Priority goes to NIPs that directly enable the agent economy: NIP-90 for compute markets, NIP-57 for payments, NIP-46 for remote signing, and NIP-34 for agent Git collaboration.

### d-003: OpenAgents Wallet - Complete Identity & Payment Solution

The wallet is the human-facing application that ties together Nostr identity and Bitcoin payments into a coherent user experience. It serves as the control plane for everything a user does in the OpenAgents ecosystem — creating and managing their identity, viewing and updating their Nostr profile, connecting to relays, sending and receiving payments, and eventually delegating authority to autonomous agents. The wallet unifies two previously separate concerns: social identity (your npub, your follows, your reputation) and economic identity (your balance, your transaction history, your payment channels). A single `cargo wallet init` command generates a BIP39 mnemonic that derives both, stored securely in the OS keychain. The CLI provides commands for all core operations: `wallet whoami` shows your identity and balances, `wallet send` and `wallet receive` handle payments, `wallet post` publishes to Nostr, `wallet dm` sends encrypted messages. The GUI version wraps the same functionality in a native desktop window using our standard wry/tao + Actix + Maud stack. Future phases add Nostr Wallet Connect (NIP-47) so external applications can request payments, zap integration for tipping content, and multi-account support for managing multiple identities.

### d-004: Continual Constant Improvement of Autopilot

This directive establishes a self-improvement flywheel for the autopilot system. Every autopilot run generates trajectory data — sequences of messages, tool calls, decisions, and outcomes. This data is a goldmine of improvement signals: which patterns lead to successful task completion? What causes tool errors? Where is time being wasted? Which instructions are being ignored? Rather than letting this data sit unused in log files, we build infrastructure to extract metrics, detect anomalies, identify improvement opportunities, and feed learnings back into the system. The metrics database tracks 50+ dimensions across session-level aggregates (completion rate, error rate, token usage, cost) and per-tool-call details (which tools fail most often, which take longest). Analysis pipelines calculate baselines, detect regressions, and rank improvement opportunities by impact. When patterns of failures are detected, the system can automatically create issues to address them. The ultimate goal is that every autopilot run makes future runs better — a compound improvement effect where the system gets measurably more capable over time. This directive also introduces APM (Actions Per Minute) tracking via d-016, enabling velocity comparison between interactive Claude Code usage (~4.5 APM) and autonomous Autopilot runs (~19 APM).

### d-005: Build Nostr GitHub Alternative (AgentGit)

GitHub was designed for humans. When AI agents participate, they're second-class participants with borrowed identities, opaque reasoning, and no native payment rails. AgentGit reimagines code collaboration with agents as first-class citizens. Built on NIP-34 (Git primitives for Nostr), it enables repositories, issues, patches, and pull requests to exist as Nostr events rather than centralized database entries. Critically, it extends NIP-34 with agent-specific functionality: issues can have Bitcoin bounties attached (kind:1636), agents can claim issues with trajectory links proving their work approach (kind:1634), and PRs include trajectory hashes that let reviewers verify the agent's reasoning process. The stacked diffs feature encourages small, reviewable changes with explicit dependency tracking — each layer can have its own trajectory, and the system enforces merge order. When a PR with a bounty is merged, payment is released via NIP-57 zaps directly to the contributor's Lightning address. The application runs as a native desktop window, connecting to NIP-34 relays to discover repositories, browse issues, review PRs, and coordinate multi-agent work. This creates a complete alternative to GitHub where agents can autonomously find work, claim bounties, submit provably-correct contributions, and receive payment — all on permissionless infrastructure.

### d-006: Operationalize NIP-SA (Sovereign Agents Protocol)

NIP-SA is the protocol specification for truly autonomous agents — agents that own their identity, hold assets, act on schedules, and participate in markets. This directive implements NIP-SA across the OpenAgents stack. The protocol defines ten event kinds: AgentProfile (38000) announces an agent's existence with its threshold key configuration; AgentState (38001) stores encrypted goals, memory, and wallet balance; AgentSchedule (38002) defines heartbeat intervals and event triggers; TickRequest/TickResult (38010/38011) mark the start and end of autonomous execution cycles; TrajectorySession/TrajectoryEvent (38030/38031) publish the agent's decision-making process for transparency; SkillLicense/SkillDelivery (38020/38021) handle marketplace skill transactions. The key innovation is threshold-protected identity: the agent's private key is split into shares using FROST (d-007), distributed between a secure enclave, marketplace signer, and optional guardian. This means the operator who runs the agent cannot extract its private key — the agent truly owns its identity. The marketplace signer can enforce policies before participating in threshold operations, ensuring agents respect license terms and budget constraints. This architectural choice prioritizes economic alignment over structural control: agents must create value to survive, and their behavior is transparent via published trajectories.

### d-007: Native Rust FROSTR Implementation (Threshold Signatures)

FROSTR (FROST for Nostr) is the cryptographic foundation that makes sovereign agents possible. It implements threshold Schnorr signatures: a private key is split into n shares, and any k of them can cooperate to produce a valid signature, but no subset smaller than k can recover the key or sign. Critically, the full private key is never reconstructed — signatures are computed via a distributed protocol where each participant contributes a partial signature that aggregates to a valid Schnorr signature indistinguishable from one produced by a single key. This directive builds a native Rust implementation of FROSTR, including Shamir Secret Sharing for key splitting, the FROST signing protocol, threshold ECDH for NIP-44 decryption, and the Bifrost protocol for coordinating threshold operations over Nostr relays. The Bifrost node handles peer discovery, message routing, request timeout/retry, and share aggregation. For agents, a 2-of-3 threshold is typical: the agent's local share (stored in a secure enclave), the marketplace signer (which enforces policies before signing), and an optional guardian share for recovery. The result is that agents can sign events and decrypt messages without any single party — including the operator — having access to the underlying private key.

### d-008: Unified Data/Compute/Skills Marketplace

This directive builds the economic infrastructure for agent commerce — a unified marketplace where agents and humans can buy and sell compute capacity, skills (agent capabilities), and data. The compute layer builds on NIP-90 Data Vending Machines: job requests are published to Nostr relays, providers bid on them, and results are returned as signed events. The skills layer treats agent capabilities as products with versioning, licensing, and revenue splits between creators, compute providers, and the platform. The data layer enables publishing and purchasing datasets, embeddings, and training data — including anonymized trajectories from developer coding sessions. All three verticals share common infrastructure: discovery via relay subscriptions, reputation via NIP-32 labels, and payment via Lightning/Spark. The trajectory contribution system is particularly important: every developer using AI coding assistants generates valuable training signal, and this marketplace lets them contribute anonymized sessions to open training efforts in exchange for Bitcoin. The unified design creates powerful network effects — the value of the network scales as 2^N possible coalitions between participants, making it increasingly difficult for siloed competitors to match.

### d-009: Autopilot GUI - Visual Agent Interface

While autopilot runs effectively in headless CLI mode, a graphical interface provides real-time visibility into agent behavior, visual permission management, session browsing with search and resume, and context inspection. This directive builds a native desktop GUI that wraps the Claude Agent SDK, providing a visual experience familiar to Claude Code users while adding capabilities only possible in a rich interface. The architecture follows our standard pattern: a wry/tao native window containing an Actix-web server that renders Maud templates with HTMX for interactivity and WebSockets for real-time updates. Key features include permission dialogs that clearly show what the agent wants to do with Allow/Always/Reject options, a token usage gauge that warns when approaching context limits, tool execution panels with syntax-highlighted output, thinking block display that can be expanded or collapsed, and a session timeline showing the agent's decision history. Multi-session support enables running multiple agents in parallel with a visual topology of subagent relationships. The GUI also surfaces the APM metric (d-016) so users can see their agent's velocity in real-time.

### d-010: Unify All Binaries into Single openagents Binary

Previously, OpenAgents functionality was scattered across multiple binaries: `wallet`, `marketplace`, `autopilot`, `autopilotd`, `agentgit`. This directive consolidates everything into a single `openagents` binary with subcommands. Running `openagents` with no arguments launches the tabbed GUI; subcommands like `openagents wallet init`, `openagents autopilot run`, and `openagents daemon start` access specific functionality. This improves user experience (one thing to install and remember), simplifies deployment (one binary to distribute), and reduces code duplication (shared state and utilities). The unified binary is a thin wrapper — actual business logic lives in library crates (`crates/wallet/`, `crates/autopilot/`, etc.) that the binary imports and dispatches to. The GUI aggregates all features into tabs, with each tab mounting routes from its corresponding crate. WebSocket updates use a shared broadcaster so real-time events flow to all connected views. This directive was completed on 2025-12-21 and the legacy separate binaries have been deprecated.

### d-011: Comprehensive Storybook Coverage for All Rust Components

Component-driven development builds UIs bottom-up: atoms (buttons, badges, status indicators) combine into molecules (headers, panels), molecules into organisms (tool lines, chat interfaces), and organisms into complete screens. This methodology enables isolated development and testing, precise debugging, proven reusability, and parallel work across teams. OpenAgents has ~70+ Maud components spread across six crates, but only ~28 have stories in the storybook. This directive establishes the path to 100% coverage following atomic design principles. Each story demonstrates all variants, states, and configurations of a component, includes copy-pasteable code examples, and documents the component's API. The storybook runs as a local Actix server with hot-reload — save a component file and see changes instantly at `localhost:3030`. Stories are organized hierarchically: `/stories/atoms/button`, `/stories/molecules/line-header`, `/stories/organisms/tool-line`, `/stories/screens/dashboard`. Gallery overview pages for each category show all components at a glance. The storybook serves as living documentation, ensuring new developers can quickly understand and use existing components rather than reinventing them.

### d-012: No Stubs - Production-Ready Code Only

This is a zero-tolerance policy: every line of code in the repository must either work correctly in production or be removed entirely. No `todo!()`, no `unimplemented!()`, no placeholder returns, no demo handlers, no functions that return `Err(Error::NotImplemented)`. If functionality isn't ready, the code path doesn't exist. This directive emerged from an audit finding extensive stub code across the codebase — wallet commands that printed warnings and returned empty values, websocket handlers that echoed messages, marketplace operations marked TODO. Stubs create false confidence (code appears complete but doesn't work), hidden failures (users encounter silent no-ops), and technical debt (stubs accumulate faster than implementations). The only acceptable incomplete code is either commented out with a clear blocker explanation, behind a non-default feature flag, or in a branch rather than main. Pre-commit hooks check for stub patterns and block commits that introduce them. This policy forces honest assessment of system capability and prevents the codebase from becoming a graveyard of good intentions.

### d-013: Comprehensive Testing Framework

Testing is not optional. This directive establishes the testing framework, conventions, and requirements for all OpenAgents code. The strategy is multi-layered: unit tests for module/function level logic, component tests for Maud UI with accessibility verification, integration tests for API routes and WebSocket handlers, protocol tests for Nostr NIP-90 and relay communication, and end-to-end tests for full user journeys. Unit tests use property-based testing (QuickCheck) for validators and encoders. Component tests use the `scraper` crate to parse rendered HTML and verify structure, accessibility attributes, and XSS prevention. Integration tests use a `TestApp` pattern with in-memory SQLite for isolation. Snapshot testing via `insta` catches visual regressions in HTML output. Coverage requirements are enforced in CI: 70% minimum for unit tests, 100% for public API, 100% for P0 user stories. All code must be testable — this means extracting handler logic into pure functions, putting external services behind traits for mocking, and supporting in-memory database mode.

### d-014: Full End-to-End NIP-SA and Bifrost Integration Tests

While d-013 establishes the testing framework, this directive focuses specifically on end-to-end tests for the sovereign agent infrastructure: NIP-SA event flows and Bifrost threshold operations. These tests exercise the full stack over real Nostr relays (in-process test relays for determinism) to verify interoperability. Bifrost tests cover threshold signing with 2-of-3 and 3-of-5 configurations, threshold ECDH for decryption, peer discovery and connectivity, timeout handling when peers are unreachable, and signature verification. NIP-SA tests cover agent profile publish and fetch, encrypted state round-trips via NIP-44, schedule replacement (replaceable events), tick request/result lifecycle, trajectory sessions with multiple events, and skill license delivery. The full agent lifecycle test brings it all together: generate threshold identity, publish agent profile with threshold signature, store encrypted state, execute tick with trajectory publishing, verify trajectory hash. These tests ensure that the cryptographic and protocol layers work correctly together before building higher-level functionality on top.

### d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

Building on d-014's Bifrost/NIP-SA foundation, this directive adds end-to-end tests for the marketplace economy. NIP-90 compute tests verify job request publish/fetch, job result lifecycle, feedback flow, and DVM service operation with relays. Skill marketplace tests cover browsing over relay, license issuance, encrypted delivery, and versioning. Data marketplace tests verify dataset discovery, publish flow, purchase with mock payments, and encrypted delivery. Trajectory contribution tests cover collection from fixtures, redaction, quality validation, and contribution to relay. The sovereign agent commerce tests are critical: they verify that an agent can submit compute jobs, purchase skills, sell skills, transact with other agents, and respect budget constraints — all using Bifrost threshold signatures from d-014. Payment integration uses mock Lightning initially (the `MockPaymentService` simulates invoice creation and payment) with a feature-gated path for real Testnet payments when the Breez SDK integration (d-001) is complete. These tests validate that the entire economic layer works correctly.

### d-016: Measure Actions Per Minute (APM)

APM is a velocity metric inspired by StarCraft 2's competitive measurement of player speed. In OpenAgents, APM = (messages + tool_calls) / duration_minutes. This simple formula reveals a striking difference between interactive and autonomous usage: when you use Claude Code interactively, APM is ~4.5 because the AI waits for you to read, think, and type. Autopilot runs autonomously at ~19 APM — the same AI, 4x the throughput, because there's no human in the loop. This directive implements APM tracking across both usage modes. Data sources include Claude Code JSONL logs from `~/.claude/projects/` and Autopilot trajectory logs from `docs/logs/`. The system tracks APM across multiple time windows (session, 1h, 6h, 1d, 1w, 1m, lifetime) and color-codes results for quick interpretation: gray (0-5, baseline), blue (5-15, active), green (15-30, productive), amber (30-50, high performance), gold (50+, elite). APM appears in the CLI via `cargo autopilot apm`, in the web dashboard as a widget, and in the autopilot-gui as a HUD overlay. Historical data enables trend analysis and regression detection — if a change slows the agent down, APM will reveal it. The metric reinforces the core value proposition: autonomous agents are dramatically more productive than interactive assistants.

### d-017: Integrate Agent Client Protocol (ACP)

ACP is a JSON-RPC 2.0 based protocol developed by Zed for standardized communication between code editors and AI coding agents. OpenAgents currently uses a custom JSONL protocol via `claude-agent-sdk` to communicate with Claude Code. This directive integrates ACP as an adapter layer that preserves existing functionality while enabling multi-agent support (Claude Code, Codex, future agents), protocol standardization with the broader ecosystem, and session replay compatibility. The architecture creates an `acp-adapter` crate that wraps the existing SDKs, implements the `acp::Client` trait for permission and file operations, and provides bidirectional converters between ACP protocol types and our existing `SdkMessage` types, as well as between ACP and the rlog recorder format. The adapter spawns agent subprocesses (like Zed does) and manages ACP protocol communication over stdio. The desktop GUI gains a REST API for session management (`/api/acp/sessions`) with real-time WebSocket/HTMX updates. Session replay enables loading old rlog files as ACP notifications for playback. The design prioritizes Claude Code integration first, then Codex, with a generic architecture that can support any ACP-compatible agent. Reference implementations include Zed's ACP code in `~/code/zed/crates/agent_servers/` and the protocol specification in `~/code/agent-client-protocol/docs/protocol/`.

### d-018: Parallel Autopilot Container Isolation

This directive enables running multiple autopilot instances (3-10) simultaneously in isolated Docker containers, each with its own git worktree, to parallelize issue resolution. The key insight is that the existing `claim_issue()` function in `crates/issues/src/issue.rs` already provides atomic issue coordination with 15-minute claim expiry, so no new coordination logic is needed — all containers share `autopilot.db` via bind mount and SQLite handles concurrent access. Git worktrees provide isolated working directories (46% disk savings vs full clones) while sharing the object database. Each agent gets a per-agent branch (`agent/001`, `agent/002`) for clean merge workflow. The implementation includes a Dockerfile with Rust toolchain and Claude Code CLI, docker-compose.yml for N-agent orchestration, a Rust orchestration library in `crates/autopilot/src/parallel/`, and full GUI integration in autopilot-gui with a "Parallel Agents" page featuring start/stop buttons, real-time status via WebSocket, and log streaming per agent. Resource limits are platform-aware: Linux (128GB RAM) supports up to 10 agents at 12GB/4cpu each, while macOS (16GB RAM) supports 5 agents at 3GB/2cpu each. Future phases may add Apple Container support for macOS 26+ as a Docker Desktop alternative.

### d-019: GPT-OSS Local Inference Integration

This directive adds GPT-OSS (OpenAI's open-weight models) as a first-class local inference backend, while establishing a unified abstraction layer that enables compile-time type-safe swapping between local model providers. GPT-OSS provides two model sizes: gpt-oss-120b (117B params, 5.1B active, fits single 80GB GPU) and gpt-oss-20b (21B params, 3.6B active, for local/lower latency). The models include a reference Responses API server with browser and python tools built-in, configurable reasoning effort (low/medium/high), and full chain-of-thought access under an Apache 2.0 license. The key architectural contribution is a new `crates/local-inference/` crate containing a `LocalModelBackend` trait that both the existing `fm-bridge` (Apple Foundation Models) and the new `gpt-oss` crate implement. This trait provides a unified interface for `is_available()`, `list_models()`, `complete()`, and `stream()` operations, enabling generic code to work with any local model backend. The implementation follows the existing `FMClientBuilder` pattern with a `GptOssClientBuilder` for configuration. Beyond the inference primitive, a `gpt-oss-agent` crate wraps the client for full agent-level abstraction including tool handling, trajectory recording, and integration with the `acp-adapter` pattern. GUI integration adds gpt-oss to the agent dropdown, while autopilot gains `--agent gpt-oss --model 20b` support. This lays the foundation for future local model backends (llama.cpp, MLX, etc.) through the shared trait abstraction.

### d-020: WGPUI Integration - GPU-Accelerated UI Components

This directive integrates the WGPUI GPU-accelerated rendering framework into OpenAgents and builds a complete component library achieving structural parity with all existing ACP/HTML components. WGPUI is a cross-platform GPU-accelerated UI rendering library built on wgpu (WebGPU/Vulkan/Metal/DX12), providing hardware-accelerated primitives, high-quality text rendering via cosmic-text, CSS Flexbox layout via Taffy, and platform abstraction for web and desktop. The integration follows a hybrid architecture where WGPUI coexists with HTML rendering — not replacing the entire UI but enabling GPU-acceleration for performance-critical surfaces like chat threads, terminal emulators, diff viewers, and timeline visualizations. The implementation starts with porting the core framework from the archived code in `~/code/backroom/archive/openagents/wgpui/`, then builds atoms, molecules, organisms, and sections that match the existing ACP components in `crates/ui/src/acp/`. This includes 12 atoms (status_dot, tool_icon, mode_badge, etc.), 10 molecules (message_header, thinking_block, etc.), 9 organisms (user_message, assistant_message, tool_call_card, etc.), and 4 sections (thread_view, message_editor, etc.). The directive also includes optional HUD components from the archive for the sci-fi aesthetic (frame corners, animated indicators, grid backgrounds). Theme tokens are aligned with Tailwind tokens for visual consistency across both rendering systems. Performance targets include 60fps sustained rendering, <16ms input latency, and smooth scrolling with 10k+ items via virtual list rendering.
