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

## Critical Path (Must Complete First)

### 1. Spark SDK Integration (d-001) - ✅ COMPLETE

**SDK Location:** `/Users/christopherdavid/code/spark-sdk`

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

### d-001: Breez Spark SDK Integration ✅ COMPLETE

**Current:** Full SDK integration complete. All core wallet operations working.

- [x] SDK dependency enabled (`breez-sdk-spark`)
- [x] `SparkWallet` connects via `BreezSdk::connect()`
- [x] Balance queries via `get_info()`
- [x] Send payments via `prepare_send_payment()` + `send_payment()`
- [x] Create invoices via `receive_payment()`
- [x] Get Spark addresses
- [x] CLI commands wired
- [ ] E2E test with real sats (optional - can use regtest)
- [ ] NIP-47 Wallet Connect (future enhancement)

---

### d-002: Nostr Protocol (100% NIP Coverage) 🟡

**Current:** 86+ NIPs implemented. Some relay placeholders.

- [ ] **Audit all 94 NIPs** - Verify completeness vs spec
- [ ] **Fix relay placeholders** in:
  - [ ] `crates/nostr/core/src/nip52.rs` line 223
  - [ ] Other NIPs with "placeholder" comments
- [ ] **Priority NIPs to verify:**
  - [ ] NIP-01: Basic protocol ✅
  - [ ] NIP-06: Key derivation ✅
  - [ ] NIP-34: Git events - verify stacked diff support
  - [ ] NIP-44: Encryption ✅
  - [ ] NIP-46: Remote signing - verify Bifrost integration
  - [ ] NIP-57: Zaps - blocked on Spark
  - [ ] NIP-90: DVMs ✅
- [ ] **Relay client testing**
  - [ ] Test against public relays (damus.io, nos.lol)
  - [ ] Verify subscription handling
  - [ ] Test reconnection logic

**Files:** `crates/nostr/core/src/nip*.rs` (86 files)

---

### d-003: Wallet Application ✅ CORE COMPLETE

**Current:** Identity/Nostr works. Bitcoin payments via Spark SDK working.

- [x] **Nostr Operations** ✅ Complete
- [x] **Bitcoin Operations** ✅ Complete
  - [x] `openagents wallet balance` - queries Spark balance
  - [x] `openagents wallet send` - sends to invoice/address
  - [x] `openagents wallet receive` - creates invoice or shows address
  - [x] `openagents wallet pay` - pays Lightning invoice
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

### d-006: NIP-SA (Sovereign Agents Protocol) ✅ COMPLETE

**Current:** Full implementation. Wallet integration wired to Spark SDK.

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

### d-007: FROSTR (Threshold Signatures) ✅

**Current:** Production ready. Real cryptography, extensive tests.

- [x] FROST keygen with frost-secp256k1 ✅
- [x] Threshold signing protocol ✅
- [x] Threshold ECDH ✅
- [x] Bifrost coordination protocol ✅
- [x] E2E tests: `bifrost_e2e.rs`, `bifrost_concurrent.rs`, `bifrost_security.rs` ✅

**Files:** `crates/frostr/src/` ✅

---

### d-008: Unified Marketplace ✅ CORE COMPLETE

**Current:** All marketplace features implemented. Payments via Spark SDK working.

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

**Current:** Dashboard exists (Actix-web). Needs WGPUI native port.

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

### d-011: Storybook Coverage 🟡

**Current:** WGPUI has 60+ components. Storybook example exists.

- [ ] **Verify all components have stories**
  - [ ] 37 atoms
  - [ ] 25 molecules
  - [ ] 20 organisms
  - [ ] 4 sections
  - [ ] 9 HUD frames
- [ ] **Gallery overview pages**
  - [ ] `/stories/atoms/` gallery
  - [ ] `/stories/molecules/` gallery
  - [ ] `/stories/organisms/` gallery
  - [ ] `/stories/sections/` gallery
- [ ] **Hot reload support**
- [ ] **Copy-pasteable code examples**

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

### d-013: Testing Framework 🟡

**Current:** Substantial coverage. Needs verification of requirements.

- [ ] **Unit Tests** - 70% minimum coverage
  - [ ] Run `cargo tarpaulin` to measure
  - [ ] Add tests for uncovered modules
- [ ] **Component Tests** (WGPUI)
  - [ ] Scene inspection tests
  - [ ] Snapshot tests with `insta`
- [ ] **Integration Tests**
  - [ ] TestApp pattern with in-memory SQLite
  - [ ] All crates have integration tests
- [ ] **Protocol Tests**
  - [ ] NIP-90 relay communication
  - [ ] Bifrost threshold coordination
- [ ] **E2E Tests**
  - [ ] Full user journeys
  - [ ] d-014, d-015 specific tests

**Files:** `crates/*/tests/`

---

### d-014: NIP-SA & Bifrost E2E Tests 🟡

**Current:** Tests exist. Verify completeness.

- [ ] **Bifrost Tests**
  - [x] `bifrost_e2e.rs` ✅
  - [x] `bifrost_concurrent.rs` ✅
  - [x] `bifrost_security.rs` ✅
  - [ ] 2-of-3 threshold signing
  - [ ] 3-of-5 threshold signing
  - [ ] Timeout handling
  - [ ] Peer discovery
- [ ] **NIP-SA Tests**
  - [x] `nip_sa_e2e.rs` ✅
  - [ ] Agent profile publish/fetch
  - [ ] Encrypted state round-trip
  - [ ] Schedule replacement
  - [ ] Tick lifecycle
  - [ ] Trajectory sessions
- [ ] **Full Agent Lifecycle Test**
  - [ ] Generate threshold identity
  - [ ] Publish agent profile with threshold sig
  - [ ] Store encrypted state
  - [ ] Execute tick with trajectory
  - [ ] Verify trajectory hash

**Files:**
- `crates/frostr/tests/`
- `crates/nostr/core/tests/`

---

### d-015: Marketplace & Commerce E2E Tests 🟡

**Current:** Test files exist. Verify coverage.

- [ ] **NIP-90 Compute Tests**
  - [ ] Job request publish/fetch
  - [ ] Job result lifecycle
  - [ ] Feedback flow
  - [ ] DVM service operation
- [ ] **Skills Marketplace Tests**
  - [ ] Browse over relay
  - [ ] License issuance
  - [ ] Encrypted delivery
  - [ ] Versioning
- [ ] **Data Marketplace Tests**
  - [ ] Dataset discovery
  - [ ] Publish flow
  - [ ] Purchase with mock payments
  - [ ] Encrypted delivery
- [ ] **Trajectory Contribution Tests**
  - [ ] Collection from fixtures
  - [ ] Redaction verification
  - [ ] Quality validation
  - [ ] Contribution to relay
- [ ] **Agent Commerce Tests** (blocked on d-001)
  - [ ] Agent submits compute job
  - [ ] Agent purchases skill
  - [ ] Agent sells skill
  - [ ] Budget constraint enforcement

**Files:** `crates/marketplace/tests/`

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

### d-020: WGPUI Component Integration ✅

**Current:** Exceeds ACP parity targets.

- [x] 37 atoms (target: 12-13) ✅
- [x] 25 molecules (target: 10) ✅
- [x] 20 organisms (target: 10) ✅
- [x] 4 sections (target: 5) - **Missing 1 section**
- [x] HUD components ✅
- [x] Markdown/streaming ✅
- [x] Theme alignment ✅

**Remaining:**
- [ ] Add 5th section (TrajectoryView or similar)
- [ ] Verify Vera Mono font usage across all components
- [ ] WASM build verification

**Files:** `crates/wgpui/src/` ✅

---

### d-021: OpenCode SDK 🟡

**Current:** Needs verification.

- [ ] **SDK Generation**
  - [ ] Generate from OpenAPI spec
  - [ ] Type-safe Rust clients
- [ ] **OpencodeClient**
  - [ ] Session operations
  - [ ] Provider operations
- [ ] **OpencodeServer**
  - [ ] Process spawning
  - [ ] Management API
- [ ] **EventStream**
  - [ ] SSE consumption
  - [ ] Event parsing
- [ ] **ACP Integration**
  - [ ] Adapter for protocol unification

**Files:** `crates/opencode-sdk/` (if exists)

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

### d-023: WGPUI Framework ✅

**Current:** Complete GPU-accelerated UI framework.

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

### d-025: All-In WGPUI ✅

**Current:** Complete. Web stack archived.

- [x] Phase 1: Framework Foundation (Entity/Context/Element) ✅
- [x] Phase 2: Delete Web Stack (archived to backroom) ✅
- [x] Phase 3: Autopilot-GUI Native (4-pane layout) ✅
- [x] Phase 4: ACP Component Parity ✅

**Files:** `crates/wgpui/` ✅

---

### d-026: E2E Test Live Viewer 🟡

**Current:** Substantially implemented. Phase 7 needs verification.

- [x] Phase 1: Core Types (step.rs, assertion.rs, context.rs) ✅
- [x] Phase 2: Test Runner (runner.rs) ✅
- [x] Phase 3: DSL Builder (dsl.rs) ✅
- [x] Phase 4: Event Injection (injection.rs) ✅
- [x] Phase 5: Input Overlay (overlay.rs) ✅
- [x] Phase 6: Test Harness (harness.rs) ✅
- [ ] **Phase 7: Integration Checklist**
  - [ ] Verify module exports
  - [ ] Run test examples
  - [ ] Document usage

**Files:** `crates/wgpui/src/testing/`

---

## Priority Order for Production

### Phase 1: Unblock Critical Path ✅ COMPLETE
1. **d-001**: Spark SDK integration ✅
2. **d-003**: Wallet Bitcoin operations ✅
3. **d-005**: GitAfter NIP-34 implementation ✅
4. **d-006**: NIP-SA wallet integration ✅
5. **d-008**: Marketplace payment settlement ✅
6. **d-012**: No stubs policy (critical violations fixed) ✅

### Phase 2: Verification & Enhancement 🟡 IN PROGRESS
7. **d-017**: ACP integration ✅
8. **d-018**: Parallel containers ✅
9. **d-019**: Local inference ✅
10. **d-009**: Autopilot GUI native port (remaining work)
11. **d-021**: OpenCode SDK verification

### Phase 3: Testing & Quality
12. **d-013**: Testing framework coverage
13. **d-014**: NIP-SA/Bifrost E2E tests
14. **d-015**: Marketplace E2E tests
15. **d-026**: E2E test viewer Phase 7

### Phase 4: Polish
16. **d-002**: NIP audit and relay fixes
17. **d-011**: Storybook completeness
18. **d-020**: Add 5th section

---

## Verification Checklist

Before declaring production-ready:

- [ ] All Spark SDK payments working on Testnet
- [ ] All marketplace flows E2E tested with real sats
- [ ] NIP-SA agent can execute tick with real payments
- [ ] GitAfter bounty claim → merge → payout flow works
- [ ] Autopilot GUI runs natively (no Actix)
- [ ] All 94 NIPs verified against spec
- [ ] Coverage >70% with tarpaulin
- [ ] No remaining todo!() or unimplemented!()
- [ ] All examples compile and run
- [ ] Storybook shows all components

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
