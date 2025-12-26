# OpenAgents Production Readiness TODO

**Generated:** 2025-12-25
**Goal:** Bring all 26 directives to full production readiness with WGPUI, real integrations, and comprehensive testing.

---

## Status Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Complete - Production ready |
| 🟡 | In Progress - Partially implemented |
| ⚠️ | Blocked - External dependency required |
| ❌ | Not Started - Needs implementation |

---

## Outstanding User Stories (No Test Coverage Yet) — 32 total

These story IDs are defined in `.openagents/USERSTORIES.md` but are not listed in the Covered Stories table yet.

Priority breakdown: P0 = 6, P1 = 18, P2 = 8.
P0 focus IDs: 3.3.1, 3.3.2, 3.3.3, 23.1.3, 25.2.1, 25.2.2.

### d-001: Breez Spark SDK Integration (1)
- [ ] 1.5.4 (P1) As a user, I want to see network status (connected/disconnected), so that I know if payments will work.

### d-003: OpenAgents Wallet (9)
- [ ] 3.1.5 (P2) As a user, I want to manage multiple identities, so that I can separate personal and work accounts.
- [ ] 3.3.1 (P0) As a user, I want to see my balance prominently in the GUI header, so that I always know my funds.
- [ ] 3.3.2 (P0) As a user, I want to click "Send" and fill out a payment form, so that I can send without CLI.
- [ ] 3.3.3 (P0) As a user, I want to click "Receive" and see a QR code, so that I can get paid easily.
- [ ] 3.3.4 (P1) As a user, I want to see my transaction list with infinite scroll, so that I can browse history.
- [ ] 3.3.5 (P1) As a user, I want to click on a transaction to see details, so that I can understand each payment.
- [ ] 3.3.6 (P2) As a user, I want to see a chart of my balance over time, so that I can visualize trends.
- [ ] 3.4.3 (P1) As a user, I want to set transaction limits, so that a compromised device can't drain my wallet.
- [ ] 3.4.4 (P1) As a user, I want to require confirmation for large transactions, so that I prevent accidental sends.

### d-006: NIP-SA (Sovereign Agents Protocol) (7)
- [ ] 6.2.3 (P1) As an agent operator, I want to inspect (but not decrypt) state metadata, so that I can monitor size/frequency.
- [ ] 6.2.4 (P2) As an agent, I want to compact old state events, so that I don't bloat the relay.
- [ ] 6.3.3 (P1) As an agent operator, I want to pause/resume the schedule, so that I can control activity.
- [ ] 6.3.4 (P2) As an agent operator, I want to set business hours, so that the agent only runs during work time.
- [ ] 6.4.3 (P1) As an observer, I want to see an agent's tick history, so that I can monitor its activity.
- [ ] 6.4.4 (P1) As an agent, I want to include a trajectory hash in my TickResult, so that my reasoning is verifiable.
- [ ] 6.5.4 (P2) As an agent, I want to redact sensitive content from trajectories, so that secrets aren't leaked.

### d-007: FROSTR (Threshold Signatures) (4)
- [ ] 7.1.4 (P2) As an operator, I want to reshare a key to new holders, so that I can rotate participants.
- [ ] 7.3.3 (P1) As an agent, I want decryption to be as fast as regular ECDH, so that performance is acceptable.
- [ ] 7.4.3 (P1) As a node, I want to handle peer disconnection gracefully, so that the group remains functional.
- [ ] 7.4.4 (P1) As a requester, I want to retry failed requests automatically, so that transient failures don't block me.

### d-008: Unified Marketplace (2)
- [ ] 8.1.5 (P1) As a compute provider, I want to set my availability schedule, so that I'm not disturbed off-hours.
- [ ] 8.3.5 (P2) As a data provider, I want to offer dataset previews, so that buyers can evaluate before purchasing.

### d-009: Autopilot GUI (3)
- [ ] 9.3.2 (P1) As a user, I want to see error rate for the session, so that I can assess quality.
- [ ] 9.3.3 (P1) As a user, I want to see cost estimate for the session, so that I can budget.
- [ ] 9.3.4 (P2) As a user, I want to see a timeline of agent activity, so that I can visualize the flow.

### d-023: WGPUI Framework (3)
- [ ] 23.1.3 (P0) As a developer, I want GPU-accelerated rendering at 60fps, so that the UI is smooth.
- [ ] 23.1.4 (P1) As a developer, I want to render on web via WebGPU, so that browser apps work.
- [ ] 23.1.5 (P1) As a developer, I want to render on desktop via Vulkan/Metal/DX12, so that native apps work.

### d-025: All-In WGPUI (3)
- [ ] 25.2.1 (P0) As a maintainer, I want the HTML/Maud stack archived, so that there's one UI path.
- [ ] 25.2.2 (P0) As a developer, I want autopilot-gui rebuilt in pure WGPUI, so that no web dependencies remain.
- [ ] 25.2.3 (P1) As a developer, I want all examples to be WGPUI-only, so that documentation is consistent.

---

## Critical Path (Must Complete First)

### 1. Spark SDK Integration (d-001) - 🟡 IN PROGRESS

**SDK Location:** `/Users/christopherdavid/code/spark-sdk`
**Remaining User Stories:** See "Outstanding User Stories" (d-001).

- [x] Breez SDK dependency enabled in `crates/spark/Cargo.toml`
- [x] `SparkWallet` implemented with real SDK calls:
  - [x] `SparkWallet::new()` → `BreezSdk::connect()`
  - [x] `get_balance()` → `sdk.get_info()`
  - [x] `send_payment()` → `sdk.send_payment()`
  - [x] `create_invoice()` → `sdk.receive_payment()`
- [x] Wallet CLI commands wired up
- [x] Marketplace payments updated (using standard invoices, HTLC available for escrow)

**API Key:** Required for Mainnet, optional for Regtest. Set `BREEZ_API_KEY` env var.

**Files (updated):**
- `crates/spark/src/wallet.rs` - Real SDK integration
- `crates/spark/Cargo.toml` - SDK enabled
- `crates/wallet/src/cli/bitcoin.rs` - Commands wired
- `crates/marketplace/src/core/payments.rs` - HODL TODOs removed

---

## Directive Status & Tasks

### d-001: Breez Spark SDK Integration 🟡 IN PROGRESS

**Current:** Core SDK integration complete; wallet UX and transaction flow stories still need coverage (see Outstanding User Stories).

- [x] SDK dependency enabled (`breez-sdk-spark`)
- [x] `SparkWallet` connects via `BreezSdk::connect()`
- [x] Balance queries via `get_info()`
- [x] Send payments via `prepare_send_payment()` + `send_payment()`
- [x] Create invoices via `receive_payment()`
- [x] Get Spark addresses
- [x] CLI commands wired
- [x] **Transaction history** via `list_payments()` (story 1.2.3) ✅
- [x] **User-friendly error messages** with `SparkError::user_friendly_message()` (story 1.5.1) ✅
- [x] **Balance protection** - `SparkError::balance_unaffected()` confirms no deduction on failure (story 1.5.2) ✅
- [ ] E2E test with real sats (optional - can use regtest)
- [ ] NIP-47 Wallet Connect (future enhancement)

---

### d-002: Nostr Protocol (100% NIP Coverage) ✅ COMPLETE

**Current:** 86 NIPs implemented. All core functionality working.

- [x] **86 NIP files implemented** ✅
  - [x] NIP-01: Basic protocol ✅
  - [x] NIP-06: Key derivation ✅
  - [x] NIP-34: Git events with stacked diff support ✅
  - [x] NIP-44: Encryption ✅
  - [x] NIP-46: Remote signing with Bifrost ✅
  - [x] NIP-57: Zaps with Spark ✅
  - [x] NIP-90: DVMs ✅
- [x] **NIP-52 "placeholders"** - Intentional empty strings per spec ✅
- [ ] **Relay client testing** (optional enhancement)
  - [ ] Test against public relays (damus.io, nos.lol)
  - [ ] Verify subscription handling

**Files:** `crates/nostr/core/src/nip*.rs` (86 files)

---

### d-003: Wallet Application 🟡 IN PROGRESS

**Current:** Identity/Nostr works; core Bitcoin flows exist, but CLI + GUI wallet stories still need coverage (see Outstanding User Stories).

- [x] **Nostr Operations** ✅ Complete
- [x] **Bitcoin Operations** ✅ Complete
  - [x] `openagents wallet balance` - queries Spark balance
  - [x] `openagents wallet send` - sends to invoice/address
  - [x] `openagents wallet receive` - creates invoice or shows address
  - [x] `openagents wallet pay` - pays Lightning invoice
  - [x] `openagents wallet history` - shows transaction history
- [x] **Wallet Security** ✅ Complete
  - [x] OS keychain protection (macOS Keychain, Linux Secret Service, Windows Credential Manager) (story 3.4.1)
  - [x] Seed phrase backup via `openagents wallet export`
- [ ] **NIP-47 Wallet Connect** (future)
- [ ] **NIP-57 Zap Support** (future)
- [ ] **WGPUI Wallet GUI** (future)

**Files:**
- `crates/wallet/src/cli/bitcoin.rs` ✅ Wired to SparkWallet
- `crates/wallet/src/core/identity.rs` ✅

---

### d-004: Autopilot Continuous Improvement ✅

**Current:** Production ready. APM, trajectory, learning system complete.

- [ ] **Verify all subsystems:**
  - [x] APM tracking with database ✅
  - [x] Trajectory collection (JSONL/rlog) ✅
  - [x] Learning system (instruction adherence) ✅
  - [x] Memory management ✅
  - [x] Dashboard with WebSocket ✅
- [ ] **Minor TODO:** `autopilot/src/memory.rs` line 48 - Make memory hog killing opt-in (non-blocking)
- [ ] **Enhancement:** Add automatic issue creation from failure patterns

**Files:** `crates/autopilot/src/` ✅

---

### d-005: GitAfter (Nostr GitHub Alternative) ✅ CORE COMPLETE

**Current:** Full NIP-34 implementation. Event builders, caching, relay client working.

- [x] **NIP-34 Git Events** ✅
  - [x] Repository announcements (kind:30617) via `RepositoryAnnouncementBuilder`
  - [x] Issue events with bounties (kind:1636) via `BountyOfferBuilder`
  - [x] PR events with trajectory hashes (kind:1618) via `PullRequestBuilder`
  - [x] Patch events (kind:1617) via `PatchBuilder`
  - [x] Status events (kinds:1630-1633) via `StatusEventBuilder`
- [x] **Agent Claims** (kind:1634) ✅
  - [x] `IssueClaimBuilder` with trajectory link, estimate
  - [x] `WorkAssignmentBuilder` for maintainer assignments
  - [x] `BountyClaimBuilder` for claiming on merge
- [x] **Stacked Diffs** ✅
  - [x] `depends_on` tag for dependencies
  - [x] `stack` tag for grouping PRs
  - [x] `layer` tag for position (e.g., "2 of 4")
  - [x] `is_pr_mergeable()` checks dependency status
  - [x] `get_dependent_prs()` finds later layers
- [x] **NIP-57 Zap Support** ✅
  - [x] `ZapRequestBuilder` for bounty payments
- [ ] **On-Demand Trajectory Fetch** (enhancement)
  - [ ] Wire `trajectory/fetch.rs` to NostrClient
- [ ] **WGPUI GitAfter GUI** (future)
  - [ ] Repository browser
  - [ ] Issue list with bounty display
  - [ ] PR review interface

**Files:** `crates/gitafter/src/`

---

### d-006: NIP-SA (Sovereign Agents Protocol) 🟡 IN PROGRESS

**Current:** Core protocol and wallet integration are implemented; schedule/state/tick history stories still need coverage (see Outstanding User Stories).

- [x] **Core Protocol** ✅
  - [x] AgentProfile (kind:38000) ✅
  - [x] AgentState (kind:38001) ✅
  - [x] AgentSchedule (kind:38002) ✅
  - [x] TickRequest/Result (kinds:38010/38011) ✅
  - [x] TrajectorySession/Event (kinds:38030/38031) ✅
  - [x] SkillLicense/Delivery (kinds:38020/38021) ✅
  - [x] Budget constraints ✅
- [x] **Wallet Integration** ✅
  - [x] Fixed `crates/nostr/core/src/nip_sa/wallet_integration.rs`
  - [x] Real SparkWallet initialization via `init_wallet()`
  - [x] Global wallet singleton with OnceCell
  - [x] Budget enforcement queries real Spark balance
- [ ] **E2E Agent Lifecycle Test**
  - [ ] Generate threshold identity
  - [ ] Publish agent profile
  - [ ] Execute tick with trajectory
  - [ ] Verify trajectory hash

**Files:**
- `crates/nostr/core/src/nip_sa/` ✅
- `crates/nostr/core/src/nip_sa/wallet_integration.rs` ✅

---

### d-007: FROSTR (Threshold Signatures) 🟡 IN PROGRESS

**Current:** Core cryptography and tests are strong; resilience/performance stories still need coverage (see Outstanding User Stories).

- [x] FROST keygen with frost-secp256k1 ✅
- [x] Threshold signing protocol ✅
- [x] Threshold ECDH ✅
- [x] Bifrost coordination protocol ✅
- [x] E2E tests: `bifrost_e2e.rs`, `bifrost_concurrent.rs`, `bifrost_security.rs` ✅

**Files:** `crates/frostr/src/` ✅

---

### d-008: Unified Marketplace 🟡 IN PROGRESS

**Current:** Core marketplace features implemented; remaining provider schedule + dataset preview stories still need coverage (see Outstanding User Stories).

#### Skills Marketplace ✅
- [x] Browse, publish, install, invoke ✅
- [x] License management ✅
- [x] Version control ✅
- [x] NIP-SA integration ✅

#### Compute Marketplace ✅
- [x] NIP-90 DVM infrastructure ✅
- [x] Provider advertising (NIP-89) ✅
- [x] Job tracking ✅
- [x] Pricing models ✅

#### Data Marketplace ✅
- [x] NIP-94/95 file metadata ✅
- [x] Dataset publishing/discovery ✅
- [x] Trajectory contribution ✅
- [x] Redaction engine ✅

#### Payment Settlement ✅ WORKING
- [x] Standard Lightning payments via Spark SDK
- [x] Invoice creation for receiving payments
- [x] Preimage verification for settlement
- [ ] HTLC escrow flows (available via SDK, not yet integrated)
- [ ] Revenue split distribution (future)

**Files:**
- `crates/marketplace/src/` ✅
- `crates/marketplace/src/core/payments.rs` ✅ Updated

---

### d-009: Autopilot GUI 🟡

**Current:** Dashboard exists (Actix-web). Needs WGPUI native port. Story gaps: 9.3.2-9.3.4 (see Outstanding User Stories).

- [ ] **Port to Native WGPUI**
  - [ ] Remove Actix-web dependency
  - [ ] winit/wgpu event loop
  - [ ] In-process backend channels
- [ ] **Core Panes**
  - [ ] Dashboard pane (APM, metrics)
  - [ ] Chat pane (agent conversation)
  - [ ] Context pane (token usage, tools)
  - [ ] Parallel agents pane
- [ ] **Features**
  - [ ] Real-time APM gauge
  - [ ] Token usage visualization
  - [ ] Tool execution timeline
  - [ ] Thinking block toggle
  - [ ] Session browser with search
  - [ ] Multi-session tabs

**Files:**
- `crates/autopilot/src/dashboard.rs` (current Actix)
- `crates/wgpui/src/sections/` (WGPUI components ready)

---

### d-010: Unified Binary ✅

**Current:** Complete. All functionality via `openagents` subcommands.

- [x] `openagents` launches GUI ✅
- [x] `openagents wallet *` subcommands ✅
- [x] `openagents autopilot *` subcommands ✅
- [x] `openagents daemon *` subcommands ✅
- [x] `openagents marketplace *` subcommands ✅
- [x] Legacy binaries deprecated ✅

**Files:** `src/main.rs`, `src/cli/`

---

### d-011: Storybook Coverage ✅ COMPLETE

**Current:** 115+ components with 9193-line storybook example.

- [x] **All component types represented** ✅
  - [x] 43 atoms (exceeded 37 target)
  - [x] 35 molecules (exceeded 25 target)
  - [x] 20 organisms (matches target)
  - [x] 4 sections (ThreadView, ThreadHeader, MessageEditor, ThreadFeedback)
  - [x] 12 HUD frames (exceeded 9 target)
- [x] **Storybook example** ✅
  - [x] `examples/storybook.rs` - 9193 lines
  - [x] Keyboard navigation (Left/Right/Up/Down)
  - [x] All atoms, molecules, organisms showcased
- [ ] **Hot reload support** (enhancement)
**Files:** `crates/wgpui/examples/storybook.rs`

---

### d-012: No Stubs Policy ✅ CORE COMPLETE

**Current:** All critical violations fixed. Only minor enhancements remain.

#### Critical Violations - FIXED ✅
- [x] `crates/marketplace/src/core/payments.rs` - HODL TODOs removed, standard payments working ✅
- [x] `crates/spark/src/wallet.rs` - Real Breez SDK integration ✅
- [x] `crates/wallet/src/cli/bitcoin.rs` - All commands wired to SparkWallet ✅
- [x] `crates/nostr/core/src/nip_sa/wallet_integration.rs` - Real wallet singleton ✅

#### Non-Blocking TODOs (Enhancements)
- [ ] `crates/autopilot/src/memory.rs:48` - Make memory hog killing opt-in (config)
- [ ] `crates/autopilot/src/daemon/nostr_trigger.rs` - NIP-SA fetch (optional feature)

#### Acceptable (Error Variants, Not Stubs)
- [x] `gitafter/src/review/checklist.rs` - NotImplemented is error type ✅
- [x] `nostr/core/src/nip07.rs` - NotImplemented is error type ✅

---

### d-013: Testing Framework ✅ CORE COMPLETE

**Current:** 700+ tests across workspace. TestApp, mocks, fixtures all working.

- [x] **Unit Tests** ✅
  - [x] All crates have unit tests
  - [x] autopilot: 258 tests
  - [x] frostr: 127 tests
  - [x] acp-adapter: 33 tests
  - [x] gitafter, wallet, compute: extensive coverage
- [x] **Component Tests** (WGPUI) ✅
  - [x] `testing/harness.rs` - Test harness
  - [x] `testing/runner.rs` - Test runner
  - [x] `testing/dsl.rs` - DSL for tests
  - [x] `testing/assertion.rs` - Assertions
  - [x] insta snapshots configured
- [x] **Integration Tests** ✅
  - [x] `crates/testing/src/test_app.rs` - TestApp pattern
  - [x] In-memory SQLite support
  - [x] All major crates have integration tests
- [x] **Protocol Tests** ✅
  - [x] `crates/testing/src/mock_relay.rs` - Mock Nostr relay
  - [x] Bifrost threshold tests in frostr
- [ ] **Coverage Measurement** (enhancement)
  - [ ] Run `cargo tarpaulin` to measure exact %

**Files:** `crates/testing/src/` ✅

---

### d-014: NIP-SA & Bifrost E2E Tests ✅ COMPLETE

**Current:** 127+ lib tests + 32 integration tests. All pass.

- [x] **Bifrost Tests** ✅
  - [x] `bifrost_e2e.rs` ✅
  - [x] `bifrost_concurrent.rs` ✅
  - [x] `bifrost_security.rs` ✅
  - [x] 2-of-3 threshold signing (`test_2_of_3_threshold_signing`) ✅
  - [x] 3-of-5 threshold signing (`test_3_of_5_threshold_signing`) ✅
  - [x] Timeout handling (`test_timeout_when_peers_dont_respond`) ✅
  - [x] Concurrent requests (`test_concurrent_signing_requests`) ✅
  - [x] Node lifecycle (`test_node_lifecycle_during_signing`) ✅
- [x] **NIP-SA Tests** ✅
  - [x] `nip_sa_e2e.rs` ✅
  - [x] Agent profile creation (`test_sovereign_agent_lifecycle`) ✅
  - [x] Encrypted state round-trip (`test_state_encryption_with_threshold_ecdh`) ✅
  - [x] Schedule creation (`test_agent_schedule_creation`) ✅
  - [x] Agent DM decryption (`test_agent_decrypts_dm_with_threshold_ecdh`) ✅
  - [x] Trajectory hash verification (`test_trajectory_hash_verification`) ✅
- [x] **Full Agent Lifecycle Test** ✅
  - [x] `test_sovereign_agent_lifecycle` covers end-to-end
  - [x] `test_agent_signs_with_bifrost` verifies threshold signing

**Files:**
- `crates/frostr/tests/` (32 integration tests)
- `crates/frostr/src/` (127 lib tests)

---

### d-015: Marketplace & Commerce E2E Tests ✅ COMPLETE

**Current:** 658 lib tests + 21 integration tests. All pass.

- [x] **NIP-90 Compute Tests** ✅
  - [x] Job request/result lifecycle (lib tests)
  - [x] DVM service operation (lib tests)
  - [x] Handler info event kind (`test_handler_info_event_kind`)
- [x] **Skills Marketplace Tests** ✅
  - [x] Browse over relay (`test_skill_browse_over_relay`) ✅
  - [x] License issuance (`test_skill_license_issuance`) ✅
  - [x] Encrypted delivery (`test_skill_delivery_encrypted`) ✅
  - [x] Versioning (`test_skill_versioning`) ✅
  - [x] Complete purchase flow (`test_complete_skill_purchase_flow`) ✅
- [x] **Trajectory Contribution Tests** ✅
  - [x] Session creation (`test_trajectory_session_creation`) ✅
  - [x] Publish to relay (`test_trajectory_session_publish_to_relay`) ✅
  - [x] Hash verification (`test_trajectory_with_hash_verification`) ✅
  - [x] Redaction verification (lib tests) ✅
  - [x] Contribution submission (`test_contribution_submission`) ✅
- [x] **Agent Commerce Tests** ✅
  - [x] Budget constraint (lib tests: `test_budget_enforcement*`)
  - [x] Revenue split calculation (`test_revenue_split_*` - 8 tests)

**Files:**
- `crates/marketplace/tests/` (21 integration tests)
- `crates/marketplace/src/` (658 lib tests)

---

### d-016: APM Tracking ✅

**Current:** Complete and production ready.

- [x] APM calculation (messages + tool_calls) / duration ✅
- [x] Database storage ✅
- [x] Time windows (session, 1h, 6h, 1d, 1w, 1m, lifetime) ✅
- [x] Color coding (gray, blue, green, amber, gold) ✅
- [x] CLI: `openagents autopilot apm` ✅
- [x] Dashboard display ✅
- [ ] **WGPUI HUD overlay** - Port from dashboard to native

**Files:** `crates/autopilot/src/apm*.rs` ✅

---

### d-017: Agent Client Protocol (ACP) ✅ COMPLETE

**Current:** Full ACP adapter implementation with multi-agent support.

- [x] **acp-adapter crate** ✅
  - [x] Bidirectional converters (`acp_to_sdk.rs`, `sdk_to_acp.rs`)
  - [x] ACP ↔ rlog converters (`rlog.rs`)
  - [x] Session management (`session.rs`)
  - [x] Permission handling (`permissions.rs`)
  - [x] Streaming/telemetry (`streaming.rs`, `telemetry.rs`)
- [x] **Claude Code Integration** ✅
  - [x] `agents/claude.rs` - Full Claude subprocess support
  - [x] stdio protocol communication
  - [x] Permission handling via UiPermissionHandler
- [x] **Codex Integration** ✅
  - [x] `agents/codex.rs` - Full Codex subprocess support
  - [x] `converters/codex.rs` - Codex-specific converters
- [x] **OpenCode & GPT-OSS Integration** ✅
  - [x] `agents/opencode.rs`, `agents/gpt_oss.rs`
- [x] **Session Replay** ✅
  - [x] `replay.rs` - RlogReplay with playback support

**Files:** `crates/acp-adapter/src/` ✅

---

### d-018: Parallel Container Isolation ✅ CORE COMPLETE

**Current:** Git worktree isolation implemented. Docker optional for enhanced isolation.

- [x] **Git Worktrees** ✅
  - [x] `parallel/worktree.rs` - Worktree creation per agent
  - [x] Per-agent branch naming
  - [x] Shared object database (disk savings)
- [x] **Issue Coordination** ✅
  - [x] Atomic claim via `claim_issue()` ✅
  - [x] 15-minute claim expiry ✅
  - [x] SQLite with WAL mode for concurrent access
- [x] **Docker Support** ✅
  - [x] `parallel/docker.rs` - Container orchestration
- [ ] **Docker Infrastructure Files** (optional enhancement)
  - [ ] Dockerfile with Rust toolchain
  - [ ] docker-compose.yml for N-agent orchestration
- [ ] **GUI Integration** (future)
  - [ ] "Parallel Agents" page

**Files:** `crates/autopilot/src/parallel/` ✅

---

### d-019: GPT-OSS Local Inference ✅ COMPLETE

**Current:** Full LocalModelBackend implementation with streaming support.

- [x] **LocalModelBackend trait** ✅
  - [x] `initialize()` - Server health check
  - [x] `list_models()` - Query available models
  - [x] `get_model_info()` - Model metadata
  - [x] `complete()` - Synchronous completion
  - [x] `complete_stream()` - Streaming completion
  - [x] `is_ready()` - Readiness check
  - [x] `shutdown()` - Clean shutdown
- [x] **GPT-OSS Backend** (`crates/gpt-oss/`) ✅
  - [x] `GptOssClient` with full API support
  - [x] Model listing and selection
  - [x] Streaming response handling
  - [x] Harmony integration
- [x] **Agent Wrapper** (`crates/gpt-oss-agent/`) ✅
  - [x] Tool handling (`tools/` module)
  - [x] Session management
  - [x] Python, browser, UI pane tools
- [ ] **FM-Bridge Backend** (future - Apple Silicon)
  - [ ] macOS 15.1+ Foundation Models integration
- [ ] **GUI Integration** (future)
  - [ ] Agent dropdown with gpt-oss option

**Files:**
- `crates/gpt-oss/src/` ✅
- `crates/gpt-oss-agent/src/` ✅

---

### d-020: WGPUI Component Integration ✅ COMPLETE

**Current:** Far exceeds ACP parity targets. 115+ components total.

- [x] 43 atoms (target: 12-13) ✅
- [x] 35 molecules (target: 10) ✅
- [x] 20 organisms (target: 10) ✅
- [x] 4 sections (target: 5) - ThreadView, ThreadHeader, MessageEditor, ThreadFeedback
- [x] 12 HUD components ✅
- [x] Markdown/streaming ✅
- [x] Theme alignment ✅

**Optional Enhancements:**
- [ ] Add 5th section (TrajectoryView or similar)
- [ ] Verify Vera Mono font usage across all components

**Files:** `crates/wgpui/src/` ✅

---

### d-021: OpenCode SDK ✅ COMPLETE

**Current:** Full REST + SSE client implementation with progenitor codegen.

- [x] **SDK Generation** ✅
  - [x] Generate from OpenAPI spec (progenitor)
  - [x] Type-safe Rust clients
- [x] **OpencodeClient** ✅
  - [x] Session operations (create, list, get, delete, fork)
  - [x] Prompt handling (sync, async)
  - [x] Message history, todos, children
  - [x] Permission responses
  - [x] Summarize, revert, unrevert
  - [x] Share and diff
- [x] **Provider operations** ✅
  - [x] Provider list, auth
  - [x] Agent list, MCP list
  - [x] Config get/update
- [x] **File operations** ✅
  - [x] File list, content, status
  - [x] Find text, file, symbol
  - [x] VCS status
- [x] **OpencodeServer** ✅
  - [x] ServerOptions builder
  - [x] Port, hostname, timeout, directory config
- [x] **EventStream** ✅
  - [x] SSE consumption via reqwest-eventsource
  - [x] Event parsing
- [x] **ACP Integration** ✅
  - [x] `acp-adapter/agents/opencode.rs` adapter

**Files:** `crates/opencode-sdk/src/` ✅

---

### d-022: Agent Orchestration Framework ✅

**Current:** Fully implemented with real integrations.

- [x] Agent registry (7 types: Sisyphus, Oracle, Librarian, Explore, Frontend, DocWriter, Multimodal) ✅
- [x] Background task manager ✅
- [x] 21 lifecycle hooks ✅
- [x] Permission levels ✅
- [x] FROSTR bridge integration ✅
- [x] Spark bridge integration ✅
- [x] Multi-backend router ✅
- [x] Cost tracking hooks ✅
- [x] Budget enforcement ✅
- [x] Solver agent coordinator ✅

**Files:** `crates/agent-orchestrator/src/` ✅

---

### d-023: WGPUI Framework 🟡 IN PROGRESS

**Current:** Core framework exists; platform/perf validation stories still need coverage (see Outstanding User Stories).

- [x] wgpu rendering ✅
- [x] Scene-based API ✅
- [x] Layout primitives ✅
- [x] Theme system ✅
- [x] Animation framework ✅
- [x] 60+ components ✅
- [x] 377+ tests ✅

**Files:** `crates/wgpui/` ✅

---

### d-024: Arwes Parity ✅

**Current:** Complete. All phases implemented.

- [x] Phase 1: Animation Foundation (34+ easing functions) ✅
- [x] Phase 2: Frame Styles (9 styles) ✅
- [x] Phase 3: Text Effects (Sequence, Decipher) ✅
- [x] Phase 4: Background Effects (Puffs, GridLines, MovingLines) ✅
- [x] Phase 5: Visual Effects (Illuminator) ✅
- [x] Phase 6: Dynamic Theme ✅

**Files:** `crates/wgpui/src/animation/` ✅

---

### d-025: All-In WGPUI 🟡 IN PROGRESS

**Current:** Core migration done; story-level verification still outstanding (see Outstanding User Stories).

- [x] Phase 1: Framework Foundation (Entity/Context/Element) ✅
- [x] Phase 2: Delete Web Stack (archived to backroom) ✅
- [x] Phase 3: Autopilot-GUI Native (4-pane layout) ✅
- [x] Phase 4: ACP Component Parity ✅

**Files:** `crates/wgpui/` ✅

---

### d-026: E2E Test Live Viewer ✅ COMPLETE

**Current:** Full implementation with comprehensive DSL and documentation.

- [x] Phase 1: Core Types (step.rs, assertion.rs, context.rs) ✅
- [x] Phase 2: Test Runner (runner.rs) ✅
- [x] Phase 3: DSL Builder (dsl.rs) ✅
- [x] Phase 4: Event Injection (injection.rs) ✅
- [x] Phase 5: Input Overlay (overlay.rs) ✅
- [x] Phase 6: Test Harness (harness.rs) ✅
- [x] **Phase 7: Integration Complete** ✅
  - [x] All modules exported in mod.rs
  - [x] chat_tests.rs, component_tests.rs, framework_tests.rs
  - [x] Comprehensive doc comments with examples

**Files:** `crates/wgpui/src/testing/` ✅

---

## Priority Order for Production

### Phase 1: Unblock Critical Path 🟡 IN PROGRESS
1. **d-001**: Spark SDK integration 🟡
2. **d-003**: Wallet Bitcoin operations 🟡
3. **d-005**: GitAfter NIP-34 implementation ✅
4. **d-006**: NIP-SA wallet integration 🟡
5. **d-008**: Marketplace payment settlement 🟡
6. **d-012**: No stubs policy (critical violations fixed) ✅

### Phase 2: Verification & Enhancement 🟡 IN PROGRESS
7. **d-017**: ACP integration ✅
8. **d-018**: Parallel containers ✅
9. **d-019**: Local inference ✅
10. **d-021**: OpenCode SDK ✅
11. **d-009**: Autopilot GUI native port 🟡 (future - WGPUI port)

### Phase 3: Testing & Quality 🟡 IN PROGRESS

Story coverage is incomplete (32 outstanding stories; see Outstanding User Stories).
12. **d-013**: Testing framework coverage ✅ (700+ tests)
13. **d-026**: E2E test viewer ✅
14. **d-014**: NIP-SA/Bifrost E2E tests ✅ (159 tests verified)
15. **d-015**: Marketplace E2E tests ✅ (679 tests verified)

### Phase 4: Polish ✅ COMPLETE
16. **d-002**: NIP audit ✅ (86 NIPs implemented)
17. **d-011**: Storybook ✅ (115+ components, 9193-line example)
18. **d-020**: WGPUI components ✅ (exceeds all targets)

---

## Verification Checklist

Before declaring production-ready:

- [x] Spark SDK integrated and compiling ✅
- [x] Wallet CLI wired to SparkWallet ✅
- [x] NIP-SA wallet singleton working ✅
- [x] 86 NIPs implemented ✅
- [x] 1500+ tests passing ✅
- [ ] 100% user story coverage (see Outstanding User Stories)
- [x] No critical stubs remaining ✅
- [x] 115+ WGPUI components ✅
- [x] Storybook example (9193 lines) ✅
- [ ] **E2E with real sats** (optional - can use regtest)
  - [ ] Spark payments on testnet
  - [ ] Marketplace flow with real payments
  - [ ] GitAfter bounty claim → payout
- [ ] **Coverage measurement**
  - [ ] Run `cargo tarpaulin` for exact %

---

## Key Files Reference

| Area | Critical Files |
|------|---------------|
| Spark (blocked) | `crates/spark/src/wallet.rs`, `crates/spark/Cargo.toml` |
| Wallet (blocked) | `crates/wallet/src/cli/bitcoin.rs` |
| Marketplace payments | `crates/marketplace/src/core/payments.rs` |
| NIP-SA wallet | `crates/nostr/core/src/nip_sa/wallet_integration.rs` |
| WGPUI | `crates/wgpui/src/` |
| Autopilot | `crates/autopilot/src/` |
| FROSTR | `crates/frostr/src/` |
| Agent Orchestration | `crates/agent-orchestrator/src/` |
