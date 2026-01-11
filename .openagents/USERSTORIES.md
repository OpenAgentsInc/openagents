# User Stories by Directive

Comprehensive catalog of user stories that should be tested for each directive. Each story follows the format:

> **As a** [user type], **I want to** [action], **so that** [benefit].

Stories are prioritized: **P0** (critical), **P1** (high), **P2** (medium), **P3** (low).

---

## Table of Contents

1. [d-001: Breez Spark SDK Integration](#d-001-breez-spark-sdk-integration)
2. [d-002: Nostr Protocol Implementation](#d-002-nostr-protocol-implementation)
3. [d-003: OpenAgents Wallet](#d-003-openagents-wallet)
4. [d-004: Autopilot Improvement](#d-004-autopilot-improvement)
5. [d-005: GitAfter (Nostr GitHub Alternative)](#d-005-gitafter-nostr-github-alternative)
6. [d-006: NIP-SA (Sovereign Agents Protocol)](#d-006-nip-sa-sovereign-agents-protocol)
7. [d-007: FROSTR (Threshold Signatures)](#d-007-frostr-threshold-signatures)
8. [d-008: Unified Marketplace](#d-008-unified-marketplace)
9. [d-009: Autopilot GUI](#d-009-autopilot-gui)
10. [d-010: Unified Binary](#d-010-unified-binary)
11. [d-011: Storybook Coverage](#d-011-storybook-coverage)
12. [d-012: No Stubs Policy](#d-012-no-stubs-policy)
13. [d-013: Testing Framework](#d-013-testing-framework)
14. [d-014: NIP-SA/Bifrost Integration Tests](#d-014-nip-sabifrost-integration-tests)
15. [d-015: Marketplace E2E Tests](#d-015-marketplace-e2e-tests)
16. [d-016: APM Metrics](#d-016-apm-metrics)
17. [d-017: ACP Integration](#d-017-acp-integration)
18. [d-018: Parallel Container Isolation](#d-018-parallel-container-isolation)
19. [d-019: GPT-OSS Local Inference](#d-019-gpt-oss-local-inference)
20. [d-020: WGPUI Component Integration](#d-020-wgpui-component-integration)
21. [d-021: OpenCode SDK](#d-021-opencode-sdk)
22. [d-022: Agent Orchestration](#d-022-agent-orchestration)
23. [d-023: WGPUI Framework](#d-023-wgpui-framework)
24. [d-024: Arwes Parity](#d-024-arwes-parity)
25. [d-025: All-In WGPUI](#d-025-all-in-wgpui)
26. [d-026: E2E Test Live Viewer](#d-026-e2e-test-live-viewer)
27. [d-027: Autopilot Demo + Dogfooding Funnel](#d-027-autopilot-demo--dogfooding-funnel-launch-priority)

---

## Test Coverage

This section tracks which user stories have implemented tests.

### Covered Stories

| Story ID | Description | Test Type | File Path |
|----------|-------------|-----------|-----------|
| **d-001: Breez Spark SDK Integration** ||||
| 1.1.1 | Generate mnemonic on wallet init | Integration | `tests/cli_integration.rs` |
| 1.1.2 | Restore wallet from mnemonic | Integration | `tests/wallet_cli_integration.rs` |
| 1.1.3 | Store mnemonic in secure keychain | Integration | `tests/cli_integration.rs` |
| 1.1.4 | See my Nostr npub and Lightning address derived from the same seed | Integration | `tests/wallet_cli_integration.rs` |
| 1.1.5 | Set a wallet password for additional protection | Integration | `tests/wallet_cli_integration.rs` |
| 1.2.1 | See current balance in sats and USD | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.2.2 | See separate Lightning and Spark L2 balances | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.2.3 | View transaction history with timestamps | Unit | `crates/spark/src/wallet.rs` |
| 1.2.4 | Pending transactions are clearly marked | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.2.5 | Export transaction history as CSV | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.1 | Send Bitcoin to a Lightning invoice | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.2 | Send Bitcoin to an on-chain address | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.3 | Send to a Spark address | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.4 | Show confirmation with fee estimate before sending | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.5 | Cancel a pending transaction before it confirms | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.6 | Scan a QR code to populate payment details | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.3.7 | Save frequently-used addresses as contacts | Unit | `crates/wallet/src/storage/address_book.rs` |
| 1.4.1 | Generate a Lightning invoice for a specific amount | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.4.2 | Show a QR code for invoices | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.4.3 | Copy invoice string to clipboard | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.4.4 | Receive push notifications when a payment arrives | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.4.5 | Generate a reusable Lightning address | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.4.6 | Set an invoice expiry time | Unit | `crates/spark/src/wallet.rs` |
| 1.5.1 | Clear error messages when payment fails | Unit | `crates/spark/src/error.rs` |
| 1.5.2 | Failed transactions don't deduct balance | Unit | `crates/spark/src/error.rs` |
| 1.5.3 | Retry failed payments with one click | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.5.4 | See network status (connected/disconnected) | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 1.6.1 | Send HTLC escrow payment | Unit | `crates/spark/src/htlc.rs` |
| 1.6.2 | Claim HTLC payment with preimage | Unit | `crates/spark/src/htlc.rs` |
| 1.6.3 | HTLC timeout refund | Unit | `crates/spark/src/htlc.rs` |
| 3.1.5 | Manage multiple identities | Unit | `crates/wallet/src/storage/identities.rs` |
| 3.2.1 | Run `openagents wallet send <address> <amount>` | Integration | `tests/cli_integration.rs` |
| 3.2.2 | Run `openagents wallet receive <amount>` | Integration | `tests/cli_integration.rs` |
| 3.2.3 | Run `openagents wallet balance` | Integration | `tests/cli_integration.rs` |
| 3.2.4 | Run `openagents wallet history` | Integration | `tests/cli_integration.rs` |
| 3.3.1 | Show balance in GUI header | Unit | `crates/wallet/src/gui/view.rs` |
| 3.3.2 | Send payments via GUI form | Unit | `crates/wallet/src/gui/view.rs` |
| 3.3.3 | Display receive QR code in GUI | Unit | `crates/wallet/src/gui/view.rs` |
| 3.3.4 | Transaction list with infinite scroll | Unit | `crates/wallet/src/gui/view.rs` |
| 3.3.5 | Click transaction to see details | Unit | `crates/wallet/src/gui/view.rs` |
| 3.3.6 | Balance chart over time | Unit | `crates/wallet/src/gui/view.rs` |
| **d-002: Nostr Protocol Implementation** ||||
| 2.1.1 | Create and sign kind:1 text notes | Unit | `crates/nostr/core/src/tests/event_validation.rs` |
| 2.1.2 | Create and sign kind:0 metadata events | Unit | `crates/nostr/core/src/tests/event_validation.rs` |
| 2.1.3 | Verify event signatures | Unit | `crates/nostr/core/src/tests/event_validation.rs` |
| 2.1.4 | Create parameterized replaceable events (kind:30000-39999) | Unit | `crates/nostr/core/src/tests/event_validation.rs` |
| 2.1.5 | Create ephemeral events (kind:20000-29999) | Unit | `crates/nostr/core/src/tests/event_validation.rs` |
| 2.2.1 | Connect to a Nostr relay via WebSocket | Integration | `crates/nostr/tests/integration/client_relay.rs` |
| 2.2.2 | Publish events to multiple relays | Integration | `crates/nostr/tests/integration/pool.rs` |
| 2.2.3 | Subscribe to events with filters | Integration | `crates/nostr/tests/integration/client_relay.rs` |
| 2.2.4 | Receive EOSE signals | Integration | `crates/nostr/tests/integration/client_relay.rs` |
| 2.2.5 | Handle relay reconnection | Integration | `crates/nostr/tests/integration/client_relay.rs` |
| 2.2.6 | Manage multiple relay connections in a pool | Integration | `crates/nostr/tests/integration/pool.rs` |
| 2.3.1 | Encrypt messages with NIP-44 | Unit | `crates/nostr/core/src/nip44.rs` |
| 2.3.2 | Decrypt NIP-44 messages | Unit | `crates/nostr/core/src/nip44.rs` |
| 2.3.3 | Send encrypted DMs (kind:14 per NIP-17) | Unit | `crates/nostr/core/src/nip17.rs` |
| 2.3.4 | Receive and decrypt DMs from others | Unit | `crates/nostr/core/src/nip17.rs` |
| 2.4.1 | Publish a job request (kind:5000-5999) | Unit | `crates/nostr/core/src/tests/nip90_integration.rs` |
| 2.4.2 | Subscribe to job requests for supported kinds | Integration | `crates/nostr/tests/integration/subscriptions.rs` |
| 2.4.3 | Publish job results (kind:6000-6999) | Unit | `crates/nostr/core/src/tests/nip90_integration.rs` |
| 2.4.4 | Leave feedback on job quality | Unit | `crates/nostr/core/src/nip90.rs` |
| 2.4.5 | See provider pricing before accepting a bid | Unit | `crates/nostr/core/src/nip90.rs` |
| 2.5.1 | Send a zap to a note or profile | Unit | `crates/nostr/core/src/nip57.rs` |
| 2.5.2 | Receive zaps on content | Unit | `crates/nostr/core/src/nip57.rs` |
| 2.5.3 | See zap count on notes | Unit | `crates/nostr/core/src/nip57.rs` |
| 2.5.4 | Set default zap amount | Unit | `crates/nostr/core/src/nip57.rs` |
| 2.6.1 | Connect a signer app via NIP-46 | Unit | `crates/nostr/core/src/nip46.rs` |
| 2.6.2 | Approve or deny signing requests | Unit | `crates/nostr/core/src/nip46.rs` |
| 2.6.3 | See what I'm signing before approving | Unit | `crates/nostr/core/src/nip46.rs` |
| **d-005: GitAfter (Nostr GitHub Alternative)** ||||
| 5.1.1 | Create a repository on GitAfter | Integration | `crates/gitafter/tests/e2e_issue_flow.rs` |
| 5.1.2 | Clone a repository from GitAfter | Integration | `crates/gitafter/tests/clone_integration_test.rs` |
| 5.1.3 | Push commits to GitAfter | Unit | `crates/gitafter/src/git/remote.rs` |
| 5.1.4 | Browse repositories by topic/language | Unit | `crates/gitafter/src/server.rs` |
| 5.1.5 | Star/follow repositories | Integration | `crates/gitafter/tests/watch_repository_test.rs` |
| 5.2.1 | Create issues with titles and descriptions | Integration | `crates/gitafter/tests/e2e_issue_flow.rs` |
| 5.2.2 | View open issues | Integration | `crates/gitafter/tests/issue_listing_test.rs` |
| 5.2.3 | Attach bounties to issues | Integration | `crates/gitafter/tests/e2e_issue_flow.rs` |
| 5.2.4 | Claim an issue | Integration | `crates/gitafter/tests/e2e_issue_flow.rs` |
| 5.2.5 | Comment on issues | Integration | `crates/gitafter/tests/e2e_issue_flow.rs` |
| 5.2.6 | Add labels to issues | Integration | `crates/gitafter/tests/issue_listing_test.rs` |
| 5.3.1 | Create a pull request from a branch | Integration | `crates/gitafter/tests/pr_creation_test.rs` |
| 5.3.2 | See the diff of a pull request | Integration | `crates/gitafter/tests/diff_viewer_test.rs` |
| 5.3.3 | Approve or request changes on a PR | Integration | `crates/gitafter/src/server.rs` |
| 5.3.4 | Merge an approved PR | Integration | `crates/gitafter/tests/pr_creation_test.rs` |
| 5.3.5 | See the agent's trajectory linked to the PR | Integration | `crates/gitafter/tests/pr_creation_test.rs` |
| 5.3.6 | Update a PR with new commits | Integration | `crates/gitafter/tests/pr_creation_test.rs` |
| 5.4.1 | Release bounty payment when a PR is merged | Integration | `crates/gitafter/tests/full_workflow_test.rs` |
| 5.4.2 | Receive payment to a Lightning address | Integration | `crates/gitafter/tests/full_workflow_test.rs` |
| 5.4.3 | Split a bounty between multiple contributors | Integration | `crates/gitafter/tests/bounty_workflow_test.rs` |
| 5.4.4 | Set bounty tiers based on issue complexity | Integration | `crates/gitafter/tests/per_layer_bounties.rs` |
| 5.5.1 | Browse repositories in WGPUI interface | Unit | `crates/gitafter/src/gui/mod.rs` |
| 5.5.2 | View issues and bounties in GUI | Unit | `crates/gitafter/src/gui/mod.rs` |
| 5.5.3 | Review PRs with trajectory links in GUI | Unit | `crates/gitafter/src/gui/mod.rs` |
| 5.5.4 | Navigate with keyboard shortcuts | Unit | `crates/gitafter/src/gui/mod.rs` |
| **d-006: NIP-SA (Sovereign Agents Protocol)** ||||
| 6.1.1 | Publish an AgentProfile (kind:39200) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.1.2 | View an agent profile | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.1.3 | Update the agent profile | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.1.4 | See agent threshold key configuration | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.2.1 | Store encrypted state (kind:39201) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.2.2 | Retrieve encrypted state on startup | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.2.3 | Inspect (but not decrypt) state metadata | Unit | `crates/nostr/core/src/nip_sa/state.rs` |
| 6.2.4 | Compact old state events | Unit | `crates/nostr/core/src/nip_sa/state.rs` |
| 6.3.1 | Set a heartbeat interval | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.3.2 | Set event triggers for schedule | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.3.3 | Pause/resume schedule | Unit | `crates/nostr/core/src/nip_sa/schedule.rs` |
| 6.3.4 | Set business hours | Unit | `crates/nostr/core/src/nip_sa/schedule.rs` |
| 6.4.1 | Publish TickRequest (kind:39210) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.4.2 | Publish TickResult (kind:39211) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.4.3 | View an agent's tick history | Unit | `crates/nostr/core/src/nip_sa/tick.rs` |
| 6.4.4 | Include trajectory hash in TickResult | Unit | `crates/nostr/core/src/nip_sa/tick.rs` |
| 6.5.1 | Publish TrajectorySession (kind:39230) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.5.2 | Publish TrajectoryEvents (kind:39231) | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 6.5.3 | Fetch and verify an agent's trajectory | Unit | `crates/nostr/core/src/nip_sa/trajectory.rs` |
| 6.5.4 | Redact sensitive content from trajectories | Unit | `crates/nostr/core/src/nip_sa/trajectory.rs` |
| **d-007: FROSTR (Threshold Signatures)** ||||
| 7.1.1 | Generate a 2-of-3 threshold keypair | Unit | `crates/frostr/src/keygen.rs` |
| 7.1.2 | Distribute shares to designated holders | Unit | `crates/frostr/src/credential.rs` |
| 7.1.3 | Generate 3-of-5 (or other) configurations | Unit | `crates/frostr/src/keygen.rs` |
| 7.1.4 | Reshare a key to new holders | Unit | `crates/frostr/src/keygen.rs` |
| 7.2.1 | Participate in a signing round | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.2.2 | Initiate a signing request via Bifrost | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.2.3 | Validate threshold signature as a normal Schnorr signature | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.2.4 | See what I'm signing before contributing | Unit | `crates/frostr/src/bifrost/messages.rs` |
| 7.2.5 | Signing completes within a timeout | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.3.1 | Decrypt NIP-44 messages using threshold ECDH | Unit | `crates/frostr/src/ecdh.rs` |
| 7.3.2 | Contribute ECDH share for decryption | Unit | `crates/frostr/src/ecdh.rs` |
| 7.3.3 | Decrypt as fast as regular ECDH | Unit | `crates/frostr/src/ecdh.rs` |
| 7.4.1 | Discover Bifrost peers on Nostr relays | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.4.2 | Send and receive Bifrost messages | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 7.4.3 | Handle peer disconnection gracefully | Unit | `crates/frostr/src/bifrost/node.rs` |
| 7.4.4 | Retry failed requests automatically | Unit | `crates/frostr/src/bifrost/node.rs` |
| **d-008: Unified Marketplace** ||||
| 8.1.1 | Browse available compute providers | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 8.1.2 | Submit a job and receive a result | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 8.1.3 | Register capabilities and pricing | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 8.1.4 | See provider ratings | Integration | `crates/marketplace/tests/discovery.rs` |
| 8.1.5 | Set provider availability schedule | Unit | `crates/marketplace/src/compute/provider.rs` |
| 8.2.1 | Publish a skill with description and pricing | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 8.2.2 | Browse and search available skills | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 8.2.3 | Purchase a skill license | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 8.2.4 | See sales and revenue | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 8.2.5 | Rate skills used | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 8.2.6 | Set usage-based pricing | Unit | `crates/marketplace/src/types.rs` |
| 8.3.1 | Publish a dataset with metadata and price | Integration | `crates/marketplace/tests/data_e2e.rs` |
| 8.3.2 | Search for datasets by topic/format | Integration | `crates/marketplace/tests/data_e2e.rs` |
| 8.3.3 | Purchase and download a dataset | Integration | `crates/marketplace/tests/data_e2e.rs` |
| 8.3.4 | See download statistics | Integration | `crates/marketplace/tests/data_e2e.rs` |
| 8.3.5 | Offer dataset previews | Unit | `crates/marketplace/src/data/discover.rs` |
| 8.4.1 | Contribute anonymized coding trajectories | Integration | `crates/marketplace/tests/trajectory_e2e.rs` |
| 8.4.2 | See contributed trajectories | Integration | `crates/marketplace/tests/trajectory_e2e.rs` |
| 8.4.3 | Set redaction rules for contributions | Integration | `crates/marketplace/tests/trajectory_e2e.rs` |
| 8.4.4 | Purchase trajectory datasets | Integration | `crates/marketplace/tests/trajectory_e2e.rs` |
| 8.5.1 | Configure revenue splits | Unit | `crates/marketplace/src/core/payments.rs` |
| 8.5.2 | Receive split automatically on purchase | Unit | `crates/marketplace/src/core/payments.rs` |
| 8.5.3 | See pending and paid revenue shares | Unit | `crates/marketplace/src/core/payments.rs` |
| **d-013: Testing Framework** ||||
| 13.1.1 | Run cargo test for unit tests | Unit | `crates/testing/src/lib.rs` |
| 13.1.2 | Tests run in parallel | Unit | `crates/testing/src/lib.rs` |
| 13.1.3 | Code coverage reports | Integration | `tests/testing_framework.rs` |
| 13.1.4 | Property-based tests for encoders/validators | Unit | `crates/nostr/core/src/nip06.rs` |
| 13.2.1 | Integration tests use in-memory DB | Integration | `crates/testing/src/test_app.rs` |
| 13.2.2 | Integration tests run offline | Integration | `crates/testing/src/mock_relay.rs` |
| 13.2.3 | TestApp pattern for setting up test contexts | Unit | `crates/testing/src/test_app.rs` |
| 13.3.1 | Snapshot tests for WGPUI scenes | Unit | `crates/wgpui/src/testing/snapshot_tests.rs` |
| 13.3.2 | Update snapshots with a single command | Integration | `tests/testing_framework.rs` |
| 13.3.3 | Snapshot diffs in CI | Unit | `crates/wgpui/src/testing/snapshot_tests.rs` |
| **d-014: NIP-SA/Bifrost Integration Tests** ||||
| 14.1.1 | E2E tests for 2-of-3 threshold signing | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 14.1.2 | E2E tests for threshold ECDH decryption | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 14.1.3 | Tests for peer discovery over test relays | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 14.1.4 | Tests for timeout handling when peers are offline | Integration | `crates/frostr/tests/bifrost_e2e.rs` |
| 14.2.1 | E2E tests for agent profile publish/fetch | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 14.2.2 | E2E tests for encrypted state round-trips | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 14.2.3 | E2E tests for tick request/result lifecycle | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| 14.2.4 | E2E tests for trajectory publish/verify | Integration | `crates/nostr/tests/integration/nip_sa.rs` |
| **d-015: Marketplace E2E Tests** ||||
| 15.1.1 | E2E tests for NIP-90 job submission | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 15.1.2 | E2E tests for job result delivery | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 15.1.3 | E2E tests for job feedback flow | Integration | `crates/marketplace/tests/compute_e2e.rs` |
| 15.2.1 | E2E tests for skill browsing | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 15.2.2 | E2E tests for skill purchase with mock payment | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 15.2.3 | E2E tests for encrypted skill delivery | Integration | `crates/marketplace/tests/skill_e2e.rs` |
| 15.3.1 | E2E tests for agent-to-agent transactions | Integration | `crates/marketplace/tests/agent_commerce_e2e.rs` |
| 15.3.2 | E2E tests for budget constraint enforcement | Integration | `crates/marketplace/tests/agent_commerce_e2e.rs` |
| **d-009: Autopilot GUI** ||||
| 9.1.1 | Conversation thread display | Visual E2E | `crates/wgpui/examples/chat_streaming_test.rs` |
| 9.1.2 | Tool calls with output | Visual E2E | `crates/wgpui/examples/chat_streaming_test.rs` |
| 9.1.3 | Type and send prompts | Unit | `crates/wgpui/src/components/sections/message_editor.rs` |
| 9.1.4 | Expand/collapse thinking blocks | Unit | `crates/wgpui/src/components/molecules/thinking_block.rs` |
| 9.1.5 | Token usage gauge | Unit | `crates/wgpui/src/components/organisms/agent_state_inspector.rs` |
| 9.2.1 | Start a new session | Unit | `crates/wgpui/src/components/organisms/thread_controls.rs` |
| 9.2.2 | Stop a running session | Unit | `crates/wgpui/src/components/organisms/thread_controls.rs` |
| 9.2.3 | Switch between active sessions | Unit | `crates/wgpui/src/components/atoms/session_breadcrumb.rs` |
| 9.2.4 | Session history list | Unit | `crates/wgpui/src/components/molecules/session_card.rs` |
| 9.2.5 | Export session transcript | Unit | `crates/wgpui/src/components/molecules/entry_actions.rs` |
| 9.3.1 | See current APM in the GUI | Unit | `crates/wgpui/src/components/atoms/apm_gauge.rs` |
| 9.3.2 | See session error rate in the GUI | Unit | `crates/autopilot-gui/src/state/mod.rs` |
| 9.3.3 | See session cost estimate in the GUI | Unit | `crates/autopilot-gui/src/state/mod.rs` |
| 9.3.4 | See timeline of agent activity | Unit | `crates/autopilot-gui/src/state/mod.rs` |
| **d-003: OpenAgents Wallet** ||||
| 3.1.1 | Show identity via `openagents wallet whoami` | Integration | `tests/wallet_cli_integration.rs` |
| 3.1.2 | Update Nostr profile fields | Integration | `tests/wallet_cli_integration.rs` |
| 3.1.3 | Follow/unfollow contacts | Integration | `tests/wallet_cli_integration.rs` |
| 3.1.4 | See follower count and list | Integration | `tests/wallet_cli_integration.rs` |
| 3.2.5 | Post a Nostr note from CLI | Integration | `tests/wallet_cli_integration.rs` |
| 3.2.6 | Send encrypted DMs from CLI | Integration | `tests/wallet_cli_integration.rs` |
| 3.4.1 | Lock wallet with OS keychain protection | Unit | `crates/wallet/src/storage/keychain.rs` |
| 3.4.2 | Back up seed phrase | Integration | `tests/wallet_cli_integration.rs` |
| 3.4.3 | Set transaction limits | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| 3.4.4 | Require confirmation for large transactions | Unit | `crates/wallet/src/cli/bitcoin.rs` |
| **d-004: Autopilot Improvement** ||||
| 4.1.1 | See metrics for each autopilot run | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.1.2 | Compare runs against baselines | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.1.3 | Identify common tool errors | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.1.4 | See success vs failure patterns | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.1.5 | Receive anomaly detection alerts | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.2.1 | Auto-create issues for detected problems | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.2.2 | Suggest hook improvements | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.2.3 | Learn from successful runs | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.2.4 | Weekly improvement report | Integration | `crates/autopilot/tests/d004_story_tests.rs` |
| 4.3.1 | Start an autopilot run with a prompt | Integration | `tests/cli_integration.rs` |
| 4.3.2 | Stop a running autopilot session | Integration | `crates/autopilot/tests/daemon_control_tests.rs` |
| 4.3.3 | Resume a paused session | Integration | `tests/cli_integration.rs` |
| 4.3.4 | Replay a past session | Integration | `tests/cli_integration.rs` |
| **d-010: Unified Binary** ||||
| 10.1.1 | Launch GUI when running `openagents` with no args | Integration | `tests/cli_integration.rs` |
| 10.1.2 | Run `openagents wallet init` to initialize wallet | Integration | `tests/cli_integration.rs` |
| 10.1.3 | Run `openagents autopilot run "task"` to start autonomous run | Integration | `tests/cli_integration.rs` |
| 10.1.4 | Run `openagents daemon start` to launch daemon | Integration | `tests/cli_integration.rs` |
| 10.1.5 | Show all available commands with --help | Integration | `tests/cli_integration.rs` |
| 10.1.6 | Show subcommand help output | Integration | `tests/cli_integration.rs` |
| 10.2.1 | Deprecation warnings for legacy binary names | Integration | `tests/legacy_binaries.rs` |
| 10.2.2 | Symlinks keep legacy binaries working | Integration | `tests/legacy_binaries.rs` |
| **d-011: Storybook Coverage** ||||
| 11.1.1 | Button stories with all variants | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 11.1.2 | TextInput stories with different states | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 11.1.3 | Atom component gallery | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 11.1.4 | Copy-pasteable code snippets | Unit | `crates/wgpui/src/components/molecules/entry_actions.rs` |
| 11.1.5 | Interactive controls to tweak props | Unit | `crates/wgpui/src/components/molecules/mode_selector.rs` |
| 11.2.1 | Hierarchical navigation (atoms/molecules/organisms) | Unit | `crates/wgpui/src/components/atoms/session_breadcrumb.rs` |
| 11.2.2 | Search stories by name | Unit | `crates/wgpui/src/components/molecules/session_search_bar.rs` |
| 11.2.3 | Bookmark frequently used stories | Unit | `crates/wgpui/src/components/molecules/checkpoint_restore.rs` |
| **d-012: No Stubs Policy** ||||
| 12.1.1 | Pre-commit rejects `todo!()` stubs | Integration | `tests/no_stubs.rs` |
| 12.1.2 | Pre-commit rejects `unimplemented!()` stubs | Integration | `tests/no_stubs.rs` |
| 12.1.3 | CI scans for stub patterns | Integration | `tests/no_stubs.rs` |
| 12.1.4 | Allowed exceptions list with justification | Integration | `tests/no_stubs.rs` |
| **d-016: APM Metrics** ||||
| 16.1.1 | APM calculated from Codex Code JSONL logs | Unit | `crates/autopilot/src/apm_parser.rs` |
| 16.1.2 | APM calculated from autopilot trajectory logs | Unit | `crates/autopilot/src/apm_parser.rs` |
| 16.1.3 | APM tracked across multiple time windows | Unit | `crates/autopilot/src/apm_storage.rs` |
| 16.2.1 | See current APM in the CLI | Integration | `tests/cli_integration.rs` |
| 16.2.2 | See APM in the GUI dashboard | Unit | `crates/wgpui/src/components/atoms/apm_gauge.rs` |
| 16.2.3 | APM color-coded by tier | Unit | `crates/autopilot/src/apm.rs` |
| 16.2.4 | APM history charts | Unit | `crates/wgpui/src/components/molecules/apm_session_row.rs` |
| **d-017: ACP Integration** ||||
| 17.1.1 | Send ACP messages to Codex Code | Integration | `crates/acp-adapter/tests/integration_tests.rs` |
| 17.1.2 | ACP event streaming | Unit + Visual | `crates/wgpui/src/testing/chat_tests.rs` |
| 17.1.3 | Switch between Codex/Codex backends | Integration | `crates/acp-adapter/tests/integration_tests.rs` |
| 17.1.4 | Convert ACP events to rlog format | Unit | `crates/acp-adapter/src/converters/rlog.rs` |
| 17.2.1 | Start an ACP session | Integration | `crates/acp-adapter/tests/integration_tests.rs` |
| 17.2.2 | Send messages and receive responses | Integration | `crates/acp-adapter/tests/integration_tests.rs` |
| 17.2.3 | Replay old sessions from rlog files | Unit | `crates/acp-adapter/src/replay.rs` |
| **d-018: Parallel Container Isolation** ||||
| 18.1.1 | Start N autopilot containers in parallel | Unit | `crates/autopilot/src/parallel/docker.rs` |
| 18.1.2 | Each container uses its own git worktree | Integration | `crates/autopilot/tests/parallel_worktree.rs` |
| 18.1.3 | Containers share the issue database | Integration | `crates/autopilot/tests/parallel_compose_test.rs` |
| 18.1.4 | See status of all running agents | Unit | `crates/autopilot/src/parallel/docker.rs` |
| 18.1.5 | Stop individual agents | Unit | `crates/autopilot/src/parallel/docker.rs` |
| 18.2.1 | Agents respect memory limits | Integration | `crates/autopilot/tests/parallel_compose_test.rs` |
| 18.2.2 | Platform-aware defaults for agent counts | Unit | `crates/autopilot/src/parallel/mod.rs` |
| 18.2.3 | Customize resource limits per agent | Integration | `crates/autopilot/tests/parallel_compose_test.rs` |
| **d-019: GPT-OSS Local Inference** ||||
| 19.1.1 | Run inference with gpt-oss-120b | Integration | `crates/local-inference/tests/gpt_oss_backend_integration.rs` |
| 19.1.2 | Run inference with gpt-oss-20b | Integration | `crates/local-inference/tests/gpt_oss_backend_integration.rs` |
| 19.1.3 | Check if GPT-OSS is available locally | Integration | `crates/local-inference/tests/gpt_oss_backend_integration.rs` |
| 19.1.4 | Stream responses | Integration | `crates/local-inference/tests/gpt_oss_backend_integration.rs` |
| 19.2.1 | Run autopilot with GPT-OSS backend | Integration | `tests/cli_integration.rs` |
| 19.2.2 | Select GPT-OSS from model dropdown | Unit | `crates/wgpui/src/components/molecules/model_selector.rs` |
| 19.2.3 | GPT-OSS supports tool calls | Unit | `crates/gpt-oss-agent/tests/tool_tests.rs` |
| **d-020: WGPUI Component Integration** ||||
| 20.1.1 | Button components with all variants | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.1.2 | TextInput components | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.1.3 | Dropdown components | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.1.4 | Modal components | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.1.5 | ScrollView components | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.2.1 | ACP atoms ported to WGPUI | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.2.2 | ACP molecules ported to WGPUI | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.2.3 | ACP organisms ported to WGPUI | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 20.2.4 | HUD components (StatusBar, Notifications) | Unit | `crates/wgpui/src/components/hud/status_bar.rs`, `crates/wgpui/src/components/hud/notifications.rs` |
| **d-021: OpenCode SDK** ||||
| 21.1.1 | Connect to an OpenCode server | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| 21.1.2 | Send messages and receive responses | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| 21.1.3 | Receive SSE events | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| 21.1.4 | List available providers | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| 21.2.1 | Spawn an OpenCode server process | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| 21.2.2 | Stop the server gracefully | Integration | `crates/opencode-sdk/tests/opencode_sdk_integration.rs` |
| **d-022: Agent Orchestration** ||||
| 22.1.1 | Spawn specialized sub-agents | Unit | `crates/agent-orchestrator/src/background.rs` |
| 22.1.2 | Collect results from sub-agents | Unit | `crates/agent-orchestrator/src/background.rs` |
| 22.1.3 | Timeout slow agents | Unit | `crates/agent-orchestrator/src/background.rs` |
| 22.1.4 | Retry failed agents | Unit | `crates/agent-orchestrator/src/hooks/session.rs` |
| 22.2.1 | Session start hooks | Unit | `crates/agent-orchestrator/src/hooks/mod.rs` |
| 22.2.2 | Session end hooks | Unit | `crates/agent-orchestrator/src/hooks/mod.rs` |
| 22.2.3 | Message hooks | Unit | `crates/agent-orchestrator/src/hooks/tool.rs` |
| 22.2.4 | Error hooks | Unit | `crates/agent-orchestrator/src/hooks/session.rs` |
| **d-023: WGPUI Framework** ||||
| 23.1.1 | Quad rendering with colors/borders | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.1.2 | Text rendering | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.1.3 | 60fps GPU-accelerated rendering | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.1.4 | Web rendering via WebGPU | Integration | `tests/wgpui_platform_support.rs` |
| 23.1.5 | Desktop rendering via Vulkan/Metal/DX12 | Integration | `tests/wgpui_platform_support.rs` |
| 23.2.1 | Flexbox layout (bounds) | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.2.2 | Percentage-based sizing | Unit | `crates/wgpui/src/layout.rs` |
| 23.2.3 | Margin/padding/gap | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.3.1 | Mouse click handling | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.3.2 | Keyboard input handling | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.3.3 | Mouse hover handling | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 23.3.4 | Scroll event handling | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| **d-024: Arwes Parity** ||||
| 24.1.1 | All 6 frame styles | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 24.1.2 | 3 additional frame styles | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 24.1.3 | Animated frame corners | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 24.2.1 | All easing functions | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 24.2.2 | Additional easing functions | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 24.3.1 | Sequence text effect | Unit | `crates/wgpui/src/testing/framework_tests.rs` |
| 24.3.2 | Decipher text effect | Unit | `crates/wgpui/src/components/text_effects/decipher.rs` |
| 24.3.3 | Blinking cursor | Unit | `crates/wgpui/src/components/text_effects/mod.rs` |
| 24.4.1 | DotsGrid background | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 24.4.2 | GridLines background | Unit | `crates/wgpui/src/components/hud/backgrounds/grid_lines.rs` |
| 24.4.3 | MovingLines background | Unit | `crates/wgpui/src/components/hud/backgrounds/moving_lines.rs` |
| 24.4.4 | Puffs background | Unit | `crates/wgpui/src/components/hud/backgrounds/puffs.rs` |
| **d-025: All-In WGPUI** ||||
| 25.1.1 | Entity system for reactive state | Unit | `crates/wgpui/src/app/entity_map.rs` |
| 25.1.2 | Element lifecycle | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 25.1.3 | Window abstraction | Unit | `crates/wgpui/src/window/window.rs` |
| 25.1.4 | Styled trait for fluent builder DSL | Unit | `crates/wgpui/src/testing/component_tests.rs` |
| 25.1.5 | Async support via cx.spawn() | Unit | `crates/wgpui/src/async/executor.rs` |
| 25.2.1 | Archive HTML/Maud stack | Integration | `tests/gui_archive.rs` |
| 25.2.2 | Autopilot-GUI is WGPUI-only | Integration | `tests/autopilot_gui_stack.rs` |
| 25.2.3 | Examples are WGPUI-only | Integration | `tests/wgpui_examples_only.rs` |
| **d-026: E2E Test Live Viewer** ||||
| 26.1.1 | Fluent DSL for tests | Unit | `crates/wgpui/src/testing/dsl.rs` (module tests) |
| 26.1.2 | Click elements by selector | Unit | `crates/wgpui/src/testing/step.rs` |
| 26.1.3 | Type text into inputs | Unit | `crates/wgpui/src/testing/dsl.rs` (module tests) |
| 26.1.4 | Assert element existence | Unit | `crates/wgpui/src/testing/dsl.rs` (module tests) |
| 26.1.5 | Wait for elements to appear | Unit | `crates/wgpui/src/testing/dsl.rs` (module tests) |
| 26.2.1 | Real-time test execution | Visual E2E | `crates/wgpui/examples/test_viewer.rs` |
| 26.2.2 | Click ripples | Visual E2E | `crates/wgpui/examples/test_viewer.rs` |
| 26.2.3 | Key presses displayed | Unit | `crates/wgpui/src/testing/overlay.rs` |
| 26.2.4 | Pause/step through tests | Unit | `crates/wgpui/src/testing/runner.rs` |
| 26.2.5 | Playback speed control | Unit | `crates/wgpui/src/testing/runner.rs` |
| 26.3.1 | Component test harness | Unit | `crates/wgpui/src/testing/harness.rs` |
| 26.3.2 | Synthetic event injection | Unit | `crates/wgpui/src/testing/injection.rs` |
| 26.3.3 | Control bar with play/pause/step | Unit | `crates/wgpui/src/testing/harness.rs` |
| 26.3.4 | Record tests by performing actions | Unit | `crates/wgpui/src/testing/recorder.rs` |

### Test Files Summary

| File | Purpose | Stories Covered |
|------|---------|-----------------|
| `crates/wgpui/examples/chat_streaming_test.rs` | Visual demo of ACP chat streaming with assertions | 9.1.1, 9.1.2, 17.1.2 |
| `crates/wgpui/examples/test_viewer.rs` | Visual demo of E2E test framework | 26.2.1, 26.2.2 |
| `crates/wgpui/src/testing/chat_tests.rs` | 18 unit tests for streaming mechanics | 17.1.2 |
| `crates/wgpui/src/testing/component_tests.rs` | 46 unit tests for component integration | 11.1.1-11.1.3, 20.1.1-5, 20.2.1-3, 24.1.1-3, 24.4.1, 25.1.2, 25.1.4 |
| `crates/wgpui/src/testing/framework_tests.rs` | 45 unit tests for WGPUI framework | 23.1.1-23.1.3, 23.2.1, 23.2.3, 23.3.1-4, 24.2.1-2, 24.3.1 |
| `crates/wgpui/src/testing/snapshot_tests.rs` | Scene snapshot regression tests | 13.3.1, 13.3.3 |
| `crates/wgpui/src/layout.rs` | Layout helpers and percent sizing tests | 23.2.2 |
| `crates/wgpui/src/app/entity_map.rs` | Entity map reactive state tests | 25.1.1 |
| `crates/wgpui/src/async/executor.rs` | Foreground/background executor spawn tests | 25.1.5 |
| `crates/wgpui/src/window/window.rs` | Window render, hit testing, and focus tests | 25.1.3 |
| `crates/wgpui/src/testing/mod.rs` | Testing framework module exports | 26.1.1 |
| `crates/wgpui/src/testing/dsl.rs` | Fluent test builder API | 26.1.1, 26.1.3-26.1.5 |
| `crates/wgpui/src/testing/step.rs` | TestStep, ElementSelector types | 26.1.2 |
| `crates/wgpui/src/testing/harness.rs` | TestHarness wrapper component | 26.3.1, 26.3.3 |
| `crates/wgpui/src/testing/injection.rs` | EventSequence for synthetic events | 26.3.2 |
| `crates/wgpui/src/testing/recorder.rs` | TestRecorder input capture | 26.3.4 |
| `crates/wgpui/src/testing/overlay.rs` | InputOverlay for click ripples | 26.2.2, 26.2.3 |
| `crates/wgpui/src/testing/runner.rs` | TestRunner playback + step control | 26.2.4, 26.2.5 |
| `tests/cli_integration.rs` | Unified CLI headless launch, wallet init, and delegation coverage | 1.1.1, 1.1.3, 3.2.1-3.2.4, 4.3.1, 4.3.3-4.3.4, 10.1.1-10.1.6, 16.2.1, 19.2.1 |
| `tests/wallet_cli_integration.rs` | Wallet identity, profile, contacts, post, DM, and export flows | 1.1.2, 1.1.4-1.1.5, 3.1.1-3.1.4, 3.2.5-3.2.6, 3.4.2 |
| `crates/spark/src/wallet.rs` | Spark invoice request helpers and expiry tests | 1.2.3, 1.4.6 |
| `crates/wallet/src/cli/bitcoin.rs` | Wallet balance/send/receive/history formatting | 1.2.1-1.2.2, 1.2.4-1.2.5, 1.3.1-1.3.6, 1.4.1-1.4.5 |
| `crates/wallet/src/storage/address_book.rs` | Payee address book persistence tests | 1.3.7 |
| `tests/legacy_binaries.rs` | Legacy binary deprecation warnings + symlink installer coverage | 10.2.1-10.2.2 |
| `tests/no_stubs.rs` | Stub pattern scanning + exceptions documentation checks | 12.1.1-12.1.4 |
| `tests/testing_framework.rs` | Snapshot + coverage documentation checks | 13.1.3, 13.3.2 |
| `tests/gui_archive.rs` | HTML/Maud archive verification | 25.2.1 |
| `tests/autopilot_gui_stack.rs` | Autopilot GUI WGPUI-only dependency checks | 25.2.2 |
| `tests/wgpui_platform_support.rs` | WGPUI web/desktop backend feature checks | 23.1.4-23.1.5 |
| `tests/wgpui_examples_only.rs` | Example files guard against legacy web stack | 25.2.3 |
| `crates/autopilot-gui/src/state/mod.rs` | Autopilot GUI session stats + timeline helpers | 9.3.2-9.3.4 |
| `crates/marketplace/src/data/discover.rs` | Dataset discovery listing + preview mapping | 8.3.5 |
| `crates/marketplace/src/compute/provider.rs` | Provider schedule + handler info tags | 8.1.5 |
| `crates/opencode-sdk/tests/opencode_sdk_integration.rs` | OpenCode SDK mock server + SSE integration tests | 21.1.1-21.2.2 |
| `crates/testing/src/lib.rs` | Testing crate smoke and parallel task tests | 13.1.1-13.1.2 |
| `crates/testing/src/mock_relay.rs` | Mock relay loopback storage tests | 13.2.2 |
| `crates/testing/src/test_app.rs` | TestApp in-memory isolation tests | 13.2.1, 13.2.3 |
| `crates/agent-orchestrator/src/background.rs` | Background task manager lifecycle tests | 22.1.1-22.1.3 |
| `crates/agent-orchestrator/src/hooks/mod.rs` | HookManager session dispatch tests | 22.2.1-22.2.2 |
| `crates/agent-orchestrator/src/hooks/session.rs` | Session recovery and error hook tests | 22.1.4, 22.2.4 |
| `crates/agent-orchestrator/src/hooks/tool.rs` | Tool hook callback and blocker tests | 22.2.3 |
| `crates/gitafter/tests/e2e_issue_flow.rs` | GitAfter issue to PR and bounty claim flow | 5.1.1, 5.2.1, 5.2.3-5.2.5 |
| `crates/gitafter/tests/clone_integration_test.rs` | Git clone validation + local clone coverage | 5.1.2 |
| `crates/gitafter/src/git/remote.rs` | Git remote push and fetch tests | 5.1.3 |
| `crates/gitafter/src/server.rs` | Repository filters + PR review submission tests | 5.1.4, 5.3.3 |
| `crates/gitafter/tests/watch_repository_test.rs` | Watch/unwatch repository cache behavior | 5.1.5 |
| `crates/gitafter/tests/issue_listing_test.rs` | Issue listing + label search | 5.2.2, 5.2.6 |
| `crates/gitafter/tests/pr_creation_test.rs` | PR creation, status transitions, trajectory tags, updates | 5.3.1, 5.3.4-5.3.6 |
| `crates/gitafter/tests/diff_viewer_test.rs` | Diff rendering and inline comments | 5.3.2 |
| `crates/gitafter/tests/full_workflow_test.rs` | End-to-end PR merge + bounty payment flow | 5.4.1-5.4.2 |
| `crates/gitafter/tests/bounty_workflow_test.rs` | Multi-bounty issue flow | 5.4.3 |
| `crates/gitafter/tests/per_layer_bounties.rs` | Per-layer bounty stacking | 5.4.4 |
| `crates/acp-adapter/tests/integration_tests.rs` | ACP session lifecycle, permissions, and file operations | 17.1.1, 17.1.3, 17.2.1-17.2.2 |
| `crates/acp-adapter/src/converters/rlog.rs` | ACP to rlog conversion tests | 17.1.4 |
| `crates/acp-adapter/src/replay.rs` | Rlog replay tests | 17.2.3 |
| `crates/local-inference/tests/gpt_oss_backend_integration.rs` | GPT-OSS LocalModelBackend mock server tests | 19.1.1-19.1.4 |
| `crates/gpt-oss-agent/tests/tool_tests.rs` | GPT-OSS agent tool schema + validation tests | 19.2.3 |
| `crates/wgpui/src/components/text_effects/decipher.rs` | Decipher text effect tests | 24.3.2 |
| `crates/wgpui/src/components/text_effects/mod.rs` | Cursor blink timing tests | 24.3.3 |
| `crates/wgpui/src/components/hud/backgrounds/grid_lines.rs` | GridLines background tests | 24.4.2 |
| `crates/wgpui/src/components/hud/backgrounds/moving_lines.rs` | MovingLines background tests | 24.4.3 |
| `crates/wgpui/src/components/hud/backgrounds/puffs.rs` | Puffs background tests | 24.4.4 |
| `crates/wgpui/src/components/hud/status_bar.rs` | StatusBar HUD tests | 20.2.4 |
| `crates/wgpui/src/components/hud/notifications.rs` | Notifications HUD tests | 20.2.4 |
| `crates/wgpui/src/components/atoms/model_badge.rs` | Model label + color tests | 19.2.2 |
| `crates/wgpui/src/components/molecules/model_selector.rs` | Model dropdown selection tests | 19.2.2 |
| `crates/wgpui/src/components/molecules/mode_selector.rs` | Mode selector builder and selection tests | 11.1.5 |
| `crates/wgpui/src/components/molecules/entry_actions.rs` | Entry action copy tests | 9.2.5, 11.1.4 |
| `crates/wgpui/src/components/sections/message_editor.rs` | Prompt editor send and focus tests | 9.1.3 |
| `crates/wgpui/src/components/molecules/thinking_block.rs` | Thinking block toggle tests | 9.1.4 |
| `crates/wgpui/src/components/organisms/agent_state_inspector.rs` | Agent state + token usage inspector tests | 9.1.5 |
| `crates/wgpui/src/components/organisms/thread_controls.rs` | Thread controls run/stop callback tests | 9.2.1-9.2.2 |
| `crates/wgpui/src/components/atoms/session_breadcrumb.rs` | Session breadcrumb navigation tests | 9.2.3, 11.2.1 |
| `crates/wgpui/src/components/molecules/session_card.rs` | Session card status and metadata tests | 9.2.4 |
| `crates/wgpui/src/components/molecules/session_search_bar.rs` | Session search filter tests | 11.2.2 |
| `crates/wgpui/src/components/molecules/checkpoint_restore.rs` | Checkpoint restore selection tests | 11.2.3 |
| `crates/wgpui/src/components/molecules/apm_session_row.rs` | APM session row formatting tests | 16.2.4 |
| `crates/wgpui/src/components/atoms/apm_gauge.rs` | APM gauge tier and bar rendering tests | 9.3.1, 16.2.2 |
| `crates/autopilot/src/apm_parser.rs` | APM log parsing tests | 16.1.1-16.1.2 |
| `crates/autopilot/src/apm_storage.rs` | APM window snapshot tests | 16.1.3 |
| `crates/autopilot/src/apm.rs` | APM tier + stats tests | 16.2.3 |
| `crates/autopilot/src/parallel/mod.rs` | Parallel config + platform defaults tests | 18.2.2 |
| `crates/autopilot/src/parallel/docker.rs` | Parallel agent service naming + status parsing | 18.1.1, 18.1.4-18.1.5 |
| `crates/autopilot/tests/parallel_worktree.rs` | Parallel agent worktree isolation | 18.1.2 |
| `crates/autopilot/tests/parallel_compose_test.rs` | Parallel compose shared DB + resource overrides | 18.1.3, 18.2.1, 18.2.3 |
| `crates/autopilot/tests/d004_story_tests.rs` | Autopilot metrics analysis, alerts, learning, and reports | 4.1.1-4.2.4 |
| `crates/autopilot/tests/daemon_control_tests.rs` | Autopilot daemon control socket stop worker coverage | 4.3.2 |
| `crates/nostr/core/src/nip06.rs` | NIP-06 mnemonic and bech32 property tests | 13.1.4 |
| `crates/nostr/core/src/tests/event_validation.rs` | NIP-01 event validation and signing tests | 2.1.1-2.1.5 |
| `crates/nostr/core/src/nip17.rs` | NIP-17 DM creation and gift wrap tests | 2.3.3-2.3.4 |
| `crates/nostr/core/src/nip44.rs` | NIP-44 encryption/decryption tests | 2.3.1-2.3.2 |
| `crates/nostr/core/src/tests/nip90_integration.rs` | NIP-90 job request/result lifecycle tests | 2.4.1, 2.4.3 |
| `crates/nostr/core/src/nip90.rs` | NIP-90 feedback and kind validation tests | 2.4.4-2.4.5 |
| `crates/nostr/core/src/nip57.rs` | NIP-57 zap request/receipt tests | 2.5.1-2.5.2 |
| `crates/nostr/core/src/nip46.rs` | NIP-46 connect request/response tests | 2.6.1-2.6.3 |
| `crates/nostr/core/src/nip_sa/trajectory.rs` | NIP-SA trajectory hashing + verification tests | 6.5.3 |
| `crates/nostr/core/src/nip_sa/schedule.rs` | NIP-SA schedule active + business hours tests | 6.3.3-6.3.4 |
| `crates/nostr/core/src/nip_sa/tick.rs` | NIP-SA tick result trajectory hash tests | 6.4.4 |
| `crates/nostr/tests/integration/client_relay.rs` | Client/relay integration flows | 2.2.1, 2.2.3-2.2.5 |
| `crates/nostr/tests/integration/pool.rs` | RelayPool multi-relay publish tests | 2.2.2, 2.2.6 |
| `crates/nostr/tests/integration/subscriptions.rs` | Subscription behavior across kinds and filters | 2.4.2 |
| `crates/nostr/tests/integration/nip_sa.rs` | NIP-SA profile/state/tick/trajectory relay flows | 6.1.1-6.5.2, 14.2.1-14.2.4 |
| `crates/frostr/tests/bifrost_e2e.rs` | Bifrost signing/ECDH/peer/timeout relay flows | 7.2.1-7.2.3, 7.2.5, 7.4.1-7.4.2, 14.1.1-14.1.4 |
| `crates/frostr/src/keygen.rs` | Shamir + FROST key generation tests | 7.1.1, 7.1.3 |
| `crates/frostr/src/credential.rs` | Group/share credential encoding tests | 7.1.2 |
| `crates/frostr/src/bifrost/messages.rs` | Bifrost message serialization for signing context | 7.2.4 |
| `crates/frostr/src/ecdh.rs` | Threshold ECDH share/determinism tests | 7.3.1-7.3.2 |
| `crates/marketplace/tests/compute_e2e.rs` | NIP-90 job request/result/feedback E2E tests | 8.1.1-8.1.3, 8.2.5, 15.1.1-15.1.3 |
| `crates/marketplace/tests/skill_e2e.rs` | Skill browse/license/delivery/versioning E2E tests | 8.2.1-8.2.4, 15.2.1-15.2.3 |
| `crates/marketplace/tests/data_e2e.rs` | Data marketplace discovery/publish/purchase E2E tests | 8.3.1-8.3.4 |
| `crates/marketplace/tests/trajectory_e2e.rs` | Trajectory contribution/redaction/hash verification E2E tests | 8.4.1-8.4.4 |
| `crates/marketplace/tests/agent_commerce_e2e.rs` | Agent-to-agent transactions and budget constraint tests | 15.3.1-15.3.2 |
| `crates/marketplace/tests/discovery.rs` | Provider discovery and rating tests | 8.1.4 |
| `crates/marketplace/src/types.rs` | Skill pricing calculations and revenue split tests | 8.2.6 |
| `crates/spark/src/htlc.rs` | HTLC escrow payment creation and claiming tests | 1.6.1-1.6.3 |
| `crates/marketplace/src/core/payments.rs` | Revenue split distribution and payout tests | 8.5.1-8.5.3 |
| `crates/frostr/src/keygen.rs` | Key reshare protocol tests | 7.1.4 |
| `crates/nostr/core/src/nip_sa/state.rs` | State compaction and metadata tests | 6.2.3-6.2.4 |
| `crates/nostr/core/src/nip_sa/tick.rs` | Tick history and trajectory hash tests | 6.4.3-6.4.4 |
| `crates/nostr/core/src/nip_sa/trajectory.rs` | Trajectory redaction and verification tests | 6.5.3-6.5.4 |
| `crates/gitafter/src/gui/mod.rs` | GitAfter WGPUI GUI component tests | 5.5.1-5.5.4 |
| `crates/wallet/src/gui/view.rs` | Wallet WGPUI GUI component tests | 3.3.1-3.3.6 |

---

## d-001: Breez Spark SDK Integration

### Wallet Initialization

| ID | Priority | User Story |
|----|----------|------------|
| 1.1.1 | P0 | As a new user, I want to generate a BIP39 mnemonic when I run `wallet init`, so that I have a secure seed for both Nostr identity and Bitcoin wallet. |
| 1.1.2 | P0 | As a returning user, I want to restore my wallet from an existing 12/24-word mnemonic, so that I can recover my funds and identity on a new device. |
| 1.1.3 | P0 | As a user, I want my mnemonic stored securely in the OS keychain, so that it's protected by system-level encryption. |
| 1.1.4 | P1 | As a user, I want to see my Nostr npub and Lightning address derived from the same seed, so that I understand my unified identity. |
| 1.1.5 | P1 | As a user, I want to set a wallet password for additional protection, so that even if someone accesses my keychain, they can't use my funds. |

### Balance & Transactions

| ID | Priority | User Story |
|----|----------|------------|
| 1.2.1 | P0 | As a user, I want to see my current balance in sats and USD, so that I know how much I have available. |
| 1.2.2 | P0 | As a user, I want to see separate Lightning and Spark L2 balances, so that I understand where my funds are. |
| 1.2.3 | P0 | As a user, I want to view my transaction history with timestamps and amounts, so that I can track my spending. |
| 1.2.4 | P1 | As a user, I want to see pending transactions marked clearly, so that I know which payments are in-flight. |
| 1.2.5 | P2 | As a user, I want to export my transaction history as CSV, so that I can track for tax purposes. |

### Send Payments

| ID | Priority | User Story |
|----|----------|------------|
| 1.3.1 | P0 | As a user, I want to send Bitcoin to a Lightning invoice, so that I can pay for services instantly. |
| 1.3.2 | P0 | As a user, I want to send Bitcoin to an on-chain address, so that I can withdraw to cold storage. |
| 1.3.3 | P0 | As a user, I want to send to another Spark user by their Spark address, so that I can transfer cheaply. |
| 1.3.4 | P0 | As a user, I want to see a confirmation screen with fee estimate before sending, so that I can approve the transaction. |
| 1.3.5 | P1 | As a user, I want to cancel a pending transaction before it confirms, so that I can correct mistakes. |
| 1.3.6 | P1 | As a user, I want to scan a QR code to populate payment details, so that I don't have to type long addresses. |
| 1.3.7 | P2 | As a user, I want to save frequently-used addresses as contacts, so that I can pay them quickly. |

### Receive Payments

| ID | Priority | User Story |
|----|----------|------------|
| 1.4.1 | P0 | As a user, I want to generate a Lightning invoice for a specific amount, so that I can receive payments. |
| 1.4.2 | P0 | As a user, I want to see a QR code for my invoice, so that payers can scan it easily. |
| 1.4.3 | P0 | As a user, I want to copy my invoice string to clipboard, so that I can share it in chat. |
| 1.4.4 | P1 | As a user, I want to receive push notifications when a payment arrives, so that I know immediately. |
| 1.4.5 | P1 | As a user, I want to generate a reusable Lightning address, so that I don't need new invoices each time. |
| 1.4.6 | P2 | As a user, I want to set an invoice expiry time, so that stale invoices don't clutter my history. |

### Error Handling

| ID | Priority | User Story |
|----|----------|------------|
| 1.5.1 | P0 | As a user, I want to see clear error messages when a payment fails, so that I understand what went wrong. |
| 1.5.2 | P0 | As a user, I want failed transactions to not deduct my balance, so that I don't lose funds. |
| 1.5.3 | P1 | As a user, I want to retry failed payments with one click, so that I can complete the transaction. |
| 1.5.4 | P1 | As a user, I want to see network status (connected/disconnected), so that I know if payments will work. |

### HTLC Escrow Payments

| ID | Priority | User Story |
|----|----------|------------|
| 1.6.1 | P0 | As a marketplace buyer, I want to send an HTLC escrow payment, so that funds are locked until service is delivered. |
| 1.6.2 | P0 | As a marketplace seller, I want to claim an HTLC payment with a preimage, so that I receive funds after delivery. |
| 1.6.3 | P1 | As a buyer, I want HTLC payments to automatically refund after timeout, so that I'm protected from non-delivery. |

---

## d-002: Nostr Protocol Implementation

### Event Creation & Signing

| ID | Priority | User Story |
|----|----------|------------|
| 2.1.1 | P0 | As a developer, I want to create and sign kind:1 text notes, so that I can post to Nostr. |
| 2.1.2 | P0 | As a developer, I want to create and sign kind:0 metadata events, so that I can set my profile. |
| 2.1.3 | P0 | As a developer, I want to verify event signatures, so that I can trust event authenticity. |
| 2.1.4 | P1 | As a developer, I want to create parameterized replaceable events (kind:30000-39999), so that I can store mutable data. |
| 2.1.5 | P1 | As a developer, I want to create ephemeral events (kind:20000-29999), so that relays don't persist them. |

### Relay Communication

| ID | Priority | User Story |
|----|----------|------------|
| 2.2.1 | P0 | As a developer, I want to connect to a Nostr relay via WebSocket, so that I can send and receive events. |
| 2.2.2 | P0 | As a developer, I want to publish events to multiple relays, so that my content is distributed. |
| 2.2.3 | P0 | As a developer, I want to subscribe to events with filters (kinds, authors, tags), so that I get relevant data. |
| 2.2.4 | P0 | As a developer, I want to receive EOSE (end of stored events) signals, so that I know when historical data is complete. |
| 2.2.5 | P1 | As a developer, I want to handle relay disconnections with automatic reconnect, so that the app stays connected. |
| 2.2.6 | P1 | As a developer, I want to manage multiple relay connections in a pool, so that I have redundancy. |

### Encryption (NIP-44)

| ID | Priority | User Story |
|----|----------|------------|
| 2.3.1 | P0 | As a user, I want to encrypt messages with NIP-44, so that only the recipient can read them. |
| 2.3.2 | P0 | As a user, I want to decrypt NIP-44 messages I receive, so that I can read private content. |
| 2.3.3 | P1 | As a user, I want to send encrypted DMs (kind:14 per NIP-17), so that I can communicate privately. |
| 2.3.4 | P1 | As a user, I want to receive and decrypt DMs from others, so that I can read private messages. |

### NIP-90 Data Vending Machines

| ID | Priority | User Story |
|----|----------|------------|
| 2.4.1 | P0 | As a compute consumer, I want to publish a job request (kind:5000-5999), so that DVMs can bid on my work. |
| 2.4.2 | P0 | As a compute provider, I want to subscribe to job requests for my supported kinds, so that I can bid. |
| 2.4.3 | P0 | As a compute provider, I want to publish job results (kind:6000-6999), so that consumers get their output. |
| 2.4.4 | P1 | As a compute consumer, I want to leave feedback on job quality, so that providers build reputation. |
| 2.4.5 | P1 | As a compute consumer, I want to see provider pricing before accepting a bid, so that I can compare. |

### NIP-57 Zaps

| ID | Priority | User Story |
|----|----------|------------|
| 2.5.1 | P0 | As a user, I want to send a zap to a note or profile, so that I can tip content creators. |
| 2.5.2 | P0 | As a user, I want to receive zaps on my content, so that I can earn from my work. |
| 2.5.3 | P1 | As a user, I want to see a zap count on notes, so that I know what's popular. |
| 2.5.4 | P2 | As a user, I want to set a default zap amount, so that I can tip quickly. |

### NIP-46 Remote Signing

| ID | Priority | User Story |
|----|----------|------------|
| 2.6.1 | P0 | As a user, I want to connect a signer app via NIP-46, so that my keys stay on a secure device. |
| 2.6.2 | P0 | As a signer, I want to approve or deny signing requests, so that I control what gets signed. |
| 2.6.3 | P1 | As a user, I want to see what I'm signing before approving, so that I don't sign malicious events. |

---

## d-003: OpenAgents Wallet

### Identity Management

| ID | Priority | User Story |
|----|----------|------------|
| 3.1.1 | P0 | As a user, I want to run `openagents wallet whoami` to see my npub and balance, so that I can verify my identity. |
| 3.1.2 | P0 | As a user, I want to update my Nostr profile (name, picture, about), so that others can identify me. |
| 3.1.3 | P1 | As a user, I want to follow/unfollow other Nostr users, so that I can curate my feed. |
| 3.1.4 | P1 | As a user, I want to see my follower count and list, so that I know my audience. |
| 3.1.5 | P2 | As a user, I want to manage multiple identities, so that I can separate personal and work accounts. |

### Wallet CLI

| ID | Priority | User Story |
|----|----------|------------|
| 3.2.1 | P0 | As a user, I want to run `openagents wallet send <address> <amount>` to send payments, so that I can pay from the terminal. |
| 3.2.2 | P0 | As a user, I want to run `openagents wallet receive <amount>` to generate an invoice, so that I can receive payments. |
| 3.2.3 | P0 | As a user, I want to run `openagents wallet balance` to check my funds, so that I know my current state. |
| 3.2.4 | P1 | As a user, I want to run `openagents wallet history` to see past transactions, so that I can review activity. |
| 3.2.5 | P1 | As a user, I want to run `openagents wallet post "Hello Nostr"` to post a note, so that I can share content. |
| 3.2.6 | P2 | As a user, I want to run `openagents wallet dm <npub> "message"` to send encrypted DMs, so that I can chat privately. |

### Wallet GUI

| ID | Priority | User Story |
|----|----------|------------|
| 3.3.1 | P0 | As a user, I want to see my balance prominently in the GUI header, so that I always know my funds. |
| 3.3.2 | P0 | As a user, I want to click "Send" and fill out a payment form, so that I can send without CLI. |
| 3.3.3 | P0 | As a user, I want to click "Receive" and see a QR code, so that I can get paid easily. |
| 3.3.4 | P1 | As a user, I want to see my transaction list with infinite scroll, so that I can browse history. |
| 3.3.5 | P1 | As a user, I want to click on a transaction to see details, so that I can understand each payment. |
| 3.3.6 | P2 | As a user, I want to see a chart of my balance over time, so that I can visualize trends. |

### Security

| ID | Priority | User Story |
|----|----------|------------|
| 3.4.1 | P0 | As a user, I want to lock my wallet with a password/biometric, so that others can't access my funds. |
| 3.4.2 | P0 | As a user, I want to back up my seed phrase, so that I can recover from device loss. |
| 3.4.3 | P1 | As a user, I want to set transaction limits, so that a compromised device can't drain my wallet. |
| 3.4.4 | P1 | As a user, I want to require confirmation for large transactions, so that I prevent accidental sends. |

### Compute Pool Accounting (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 3.5.1 | P0 | As an Autopilot user, I want my compute purchases automatically deducted from my wallet, so that I don't have to manually fund each job. |
| 3.5.2 | P0 | As an Autopilot user, I want to see my compute spend separately from other wallet transactions, so that I can track costs. |
| 3.5.3 | P0 | As an Autopilot user, I want to set a compute budget cap, so that I don't overspend on automated runs. |
| 3.5.4 | P1 | As an Autopilot user, I want to receive warnings when my compute budget is running low, so that I can top up. |

---

## d-004: Autopilot Improvement

### Trajectory Analysis

| ID | Priority | User Story |
|----|----------|------------|
| 4.1.1 | P0 | As an operator, I want to see metrics (token usage, error rate, duration) for each autopilot run, so that I can assess performance. |
| 4.1.2 | P0 | As an operator, I want to compare runs against baselines, so that I can detect regressions. |
| 4.1.3 | P1 | As an operator, I want to identify the most common tool errors, so that I can prioritize fixes. |
| 4.1.4 | P1 | As an operator, I want to see which patterns lead to success vs failure, so that I can improve prompts. |
| 4.1.5 | P2 | As an operator, I want anomaly detection alerts, so that I'm notified of unusual behavior. |

### Self-Improvement

| ID | Priority | User Story |
|----|----------|------------|
| 4.2.1 | P0 | As an operator, I want autopilot to automatically create issues for detected problems, so that they get tracked. |
| 4.2.2 | P1 | As an operator, I want autopilot to suggest hook improvements based on failure patterns, so that I can tune behavior. |
| 4.2.3 | P1 | As an operator, I want autopilot to learn from successful runs, so that future runs are more efficient. |
| 4.2.4 | P2 | As an operator, I want to see a weekly improvement report, so that I can track progress over time. |

### Session Management

| ID | Priority | User Story |
|----|----------|------------|
| 4.3.1 | P0 | As an operator, I want to start an autopilot run with a prompt, so that it works on my task. |
| 4.3.2 | P0 | As an operator, I want to stop a running autopilot session, so that I can interrupt bad runs. |
| 4.3.3 | P1 | As an operator, I want to resume a paused session, so that I can continue interrupted work. |
| 4.3.4 | P1 | As an operator, I want to replay a past session, so that I can understand what happened. |

### Repo Onboarding (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 4.4.1 | P0 | As a user, I want to connect my GitHub repository to Autopilot, so that it can work on my codebase. |
| 4.4.2 | P0 | As a user, I want Autopilot to validate it has the required permissions (read code, write PRs), so that runs don't fail due to access issues. |
| 4.4.3 | P0 | As a user, I want Autopilot to auto-detect my project's language and tooling, so that it knows how to run tests. |
| 4.4.4 | P0 | As a user, I want Autopilot to run a health check on first connection, so that I know the setup is correct. |
| 4.4.5 | P1 | As a user, I want to see which repos I have connected to Autopilot, so that I can manage my setup. |
| 4.4.6 | P1 | As a user, I want to disconnect a repo from Autopilot, so that I can revoke access. |

### GitHub Issue → PR Flow (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 4.5.1 | P0 | As an agent, I want to detect issues labeled for autopilot (e.g., `autopilot`), so that I know which to work on. |
| 4.5.2 | P0 | As an agent, I want to post a "claiming" comment when I start an issue, so that humans know I'm working on it. |
| 4.5.3 | P0 | As an agent, I want to create a feature branch for each issue, so that work is isolated. |
| 4.5.4 | P0 | As an agent, I want to open a PR with a structured template when work is complete, so that humans can review. |
| 4.5.5 | P0 | As an agent, I want to link the PR to the original issue, so that GitHub closes it on merge. |
| 4.5.6 | P0 | As an agent, I want to include a replay link in my PR description, so that reviewers can see my reasoning. |
| 4.5.7 | P1 | As an agent, I want to handle review feedback by re-running and updating the PR, so that issues get resolved. |

### CI Integration (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 4.6.1 | P0 | As an agent, I want to run the local test suite before opening a PR, so that I don't submit broken code. |
| 4.6.2 | P0 | As an agent, I want to detect when CI fails after opening a PR, so that I can attempt to fix it. |
| 4.6.3 | P0 | As an agent, I want to auto-fix CI failures up to N attempts, so that PRs are more likely to pass. |
| 4.6.4 | P1 | As an agent, I want to escalate to a human after repeated CI failures, so that I don't spin forever. |
| 4.6.5 | P1 | As an agent, I want to include CI status in my receipts, so that outcomes are verifiable. |

### Replay Artifacts (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 4.7.1 | P0 | As an operator, I want Autopilot to produce a Run Bundle (rlog + metadata + diffs) on completion, so that I have a record. |
| 4.7.2 | P0 | As an operator, I want the Run Bundle to include a deterministic timeline, so that replays can be scrubbed. |
| 4.7.3 | P0 | As an operator, I want the Run Bundle to include receipts (test results, CI status), so that outcomes are verifiable. |
| 4.7.4 | P0 | As an operator, I want secrets redacted from Run Bundles before external publish, so that nothing sensitive leaks. |
| 4.7.5 | P1 | As an operator, I want to export a Run Bundle to share or archive, so that I can demo or review later. |

### Notifications & Escalation (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 4.8.1 | P0 | As a user, I want desktop notifications when Autopilot completes a run, so that I know to review. |
| 4.8.2 | P1 | As a user, I want email summaries for overnight runs, so that I wake up to a clear status. |
| 4.8.3 | P1 | As a user, I want Autopilot to escalate repeated failures to me via notification, so that I can intervene. |

---

## d-005: GitAfter (Nostr GitHub Alternative)

### Repository Management

| ID | Priority | User Story |
|----|----------|------------|
| 5.1.1 | P0 | As a developer, I want to create a repository on GitAfter, so that I can host my project. |
| 5.1.2 | P0 | As a developer, I want to clone a repository from GitAfter, so that I can work locally. |
| 5.1.3 | P0 | As a developer, I want to push commits to GitAfter, so that my changes are published. |
| 5.1.4 | P1 | As a developer, I want to browse repositories by topic/language, so that I can discover projects. |
| 5.1.5 | P1 | As a developer, I want to star/follow repositories, so that I can track favorites. |

### Issue Tracking

| ID | Priority | User Story |
|----|----------|------------|
| 5.2.1 | P0 | As a maintainer, I want to create issues with titles and descriptions, so that I can track work. |
| 5.2.2 | P0 | As a contributor, I want to view open issues, so that I can find work to do. |
| 5.2.3 | P0 | As a maintainer, I want to attach bounties to issues, so that I can incentivize contributions. |
| 5.2.4 | P1 | As an agent, I want to claim an issue, so that others know I'm working on it. |
| 5.2.5 | P1 | As a contributor, I want to comment on issues, so that I can discuss the work. |
| 5.2.6 | P2 | As a maintainer, I want to add labels to issues, so that I can categorize them. |

### Pull Requests

| ID | Priority | User Story |
|----|----------|------------|
| 5.3.1 | P0 | As a contributor, I want to create a pull request from my branch, so that I can submit changes. |
| 5.3.2 | P0 | As a reviewer, I want to see the diff of a pull request, so that I can review changes. |
| 5.3.3 | P0 | As a reviewer, I want to approve or request changes on a PR, so that I can gate merges. |
| 5.3.4 | P0 | As a maintainer, I want to merge an approved PR, so that changes go to main. |
| 5.3.5 | P1 | As a reviewer, I want to see the agent's trajectory linked to the PR, so that I understand reasoning. |
| 5.3.6 | P1 | As a contributor, I want to update my PR with new commits, so that I can address feedback. |

### Bounty Payments

| ID | Priority | User Story |
|----|----------|------------|
| 5.4.1 | P0 | As a maintainer, I want to release bounty payment when a PR is merged, so that contributors get paid. |
| 5.4.2 | P0 | As a contributor, I want to receive payment to my Lightning address, so that I earn Bitcoin. |
| 5.4.3 | P1 | As a maintainer, I want to split a bounty between multiple contributors, so that everyone gets credit. |
| 5.4.4 | P2 | As a maintainer, I want to set bounty tiers based on issue complexity, so that pricing is fair. |

### GitAfter GUI

| ID | Priority | User Story |
|----|----------|------------|
| 5.5.1 | P0 | As a user, I want to browse repositories in a native WGPUI interface, so that I have a fast desktop experience. |
| 5.5.2 | P0 | As a user, I want to view issues and their bounties in the GUI, so that I can find work visually. |
| 5.5.3 | P1 | As a user, I want to review PRs with trajectory links in the GUI, so that I can audit agent work. |
| 5.5.4 | P1 | As a user, I want to navigate between repos, issues, and PRs with keyboard shortcuts, so that I work efficiently. |

---

## d-006: NIP-SA (Sovereign Agents Protocol)

### Agent Profile

| ID | Priority | User Story |
|----|----------|------------|
| 6.1.1 | P0 | As an agent operator, I want to publish an AgentProfile (kind:39200), so that the agent is discoverable. |
| 6.1.2 | P0 | As a user, I want to view an agent's profile, so that I understand its capabilities. |
| 6.1.3 | P1 | As an agent operator, I want to update the agent's profile, so that I can change capabilities. |
| 6.1.4 | P1 | As a user, I want to see an agent's threshold key configuration, so that I know its security model. |

### Agent State

| ID | Priority | User Story |
|----|----------|------------|
| 6.2.1 | P0 | As an agent, I want to store my encrypted state (kind:39201), so that I persist between runs. |
| 6.2.2 | P0 | As an agent, I want to retrieve my encrypted state on startup, so that I resume where I left off. |
| 6.2.3 | P1 | As an agent operator, I want to inspect (but not decrypt) state metadata, so that I can monitor size/frequency. |
| 6.2.4 | P2 | As an agent, I want to compact old state events, so that I don't bloat the relay. |

### Agent Schedule

| ID | Priority | User Story |
|----|----------|------------|
| 6.3.1 | P0 | As an agent operator, I want to set a heartbeat interval, so that the agent wakes periodically. |
| 6.3.2 | P0 | As an agent operator, I want to set event triggers, so that the agent wakes on relevant events. |
| 6.3.3 | P1 | As an agent operator, I want to pause/resume the schedule, so that I can control activity. |
| 6.3.4 | P2 | As an agent operator, I want to set business hours, so that the agent only runs during work time. |

### Tick Lifecycle

| ID | Priority | User Story |
|----|----------|------------|
| 6.4.1 | P0 | As an agent, I want to publish a TickRequest (kind:39210) when I wake, so that my activity is logged. |
| 6.4.2 | P0 | As an agent, I want to publish a TickResult (kind:39211) when I complete, so that outcomes are recorded. |
| 6.4.3 | P1 | As an observer, I want to see an agent's tick history, so that I can monitor its activity. |
| 6.4.4 | P1 | As an agent, I want to include a trajectory hash in my TickResult, so that my reasoning is verifiable. |

### Trajectory Publishing

| ID | Priority | User Story |
|----|----------|------------|
| 6.5.1 | P0 | As an agent, I want to publish TrajectorySession (kind:39230) with my decision history, so that my work is transparent. |
| 6.5.2 | P0 | As an agent, I want to publish TrajectoryEvents (kind:39231) for each step, so that reasoning is traceable. |
| 6.5.3 | P1 | As a reviewer, I want to fetch and verify an agent's trajectory, so that I can audit its work. |
| 6.5.4 | P2 | As an agent, I want to redact sensitive content from trajectories, so that secrets aren't leaked. |

---

## d-007: FROSTR (Threshold Signatures)

### Key Generation

| ID | Priority | User Story |
|----|----------|------------|
| 7.1.1 | P0 | As an operator, I want to generate a 2-of-3 threshold keypair, so that no single party controls the key. |
| 7.1.2 | P0 | As an operator, I want to distribute shares to designated holders, so that the quorum is established. |
| 7.1.3 | P1 | As an operator, I want to generate 3-of-5 or other configurations, so that I can tune security. |
| 7.1.4 | P2 | As an operator, I want to reshare a key to new holders, so that I can rotate participants. |

### Threshold Signing

| ID | Priority | User Story |
|----|----------|------------|
| 7.2.1 | P0 | As a share holder, I want to participate in a signing round, so that we produce a valid signature. |
| 7.2.2 | P0 | As a requester, I want to initiate a signing request via Bifrost, so that holders are notified. |
| 7.2.3 | P0 | As a verifier, I want to validate a threshold signature as a normal Schnorr signature, so that compatibility is maintained. |
| 7.2.4 | P1 | As a share holder, I want to see what I'm signing before contributing, so that I can refuse bad requests. |
| 7.2.5 | P1 | As a requester, I want signing to complete within a timeout, so that I'm not blocked indefinitely. |

### Threshold ECDH (Decryption)

| ID | Priority | User Story |
|----|----------|------------|
| 7.3.1 | P0 | As an agent, I want to decrypt NIP-44 messages using threshold ECDH, so that no single party can read my DMs. |
| 7.3.2 | P0 | As a share holder, I want to contribute my ECDH share, so that decryption can complete. |
| 7.3.3 | P1 | As an agent, I want decryption to be as fast as regular ECDH, so that performance is acceptable. |

### Bifrost Coordination

| ID | Priority | User Story |
|----|----------|------------|
| 7.4.1 | P0 | As a node, I want to discover other Bifrost peers on Nostr relays, so that we can coordinate. |
| 7.4.2 | P0 | As a node, I want to send and receive Bifrost messages, so that protocols can execute. |
| 7.4.3 | P1 | As a node, I want to handle peer disconnection gracefully, so that the group remains functional. |
| 7.4.4 | P1 | As a requester, I want to retry failed requests automatically, so that transient failures don't block me. |

---

## d-008: Unified Marketplace

### Compute Marketplace

| ID | Priority | User Story |
|----|----------|------------|
| 8.1.1 | P0 | As a compute consumer, I want to browse available compute providers, so that I can find one for my job. |
| 8.1.2 | P0 | As a compute consumer, I want to submit a job and receive a result, so that my work gets done. |
| 8.1.3 | P0 | As a compute provider, I want to register my capabilities and pricing, so that consumers can find me. |
| 8.1.4 | P1 | As a compute consumer, I want to see provider ratings, so that I can choose quality providers. |
| 8.1.5 | P1 | As a compute provider, I want to set my availability schedule, so that I'm not disturbed off-hours. |

### Skills Marketplace

| ID | Priority | User Story |
|----|----------|------------|
| 8.2.1 | P0 | As a skill creator, I want to publish a skill with description and pricing, so that agents can purchase it. |
| 8.2.2 | P0 | As an agent, I want to browse and search available skills, so that I can extend my capabilities. |
| 8.2.3 | P0 | As an agent, I want to purchase a skill license, so that I can use it in my work. |
| 8.2.4 | P1 | As a skill creator, I want to see my sales and revenue, so that I can track earnings. |
| 8.2.5 | P1 | As an agent, I want to rate skills I've used, so that others benefit from my experience. |
| 8.2.6 | P2 | As a skill creator, I want to set usage-based pricing, so that I earn per invocation. |

### Data Marketplace

| ID | Priority | User Story |
|----|----------|------------|
| 8.3.1 | P0 | As a data provider, I want to publish a dataset with metadata and price, so that buyers can find it. |
| 8.3.2 | P0 | As a data consumer, I want to search for datasets by topic/format, so that I can find relevant data. |
| 8.3.3 | P0 | As a data consumer, I want to purchase and download a dataset, so that I can use it. |
| 8.3.4 | P1 | As a data provider, I want to see download statistics, so that I know my data's value. |
| 8.3.5 | P2 | As a data provider, I want to offer dataset previews, so that buyers can evaluate before purchasing. |

### Trajectory Contributions

| ID | Priority | User Story |
|----|----------|------------|
| 8.4.1 | P0 | As a developer, I want to contribute my anonymized coding trajectories, so that I earn Bitcoin. |
| 8.4.2 | P0 | As a developer, I want to see which trajectories I've contributed, so that I track my contributions. |
| 8.4.3 | P1 | As a developer, I want to set redaction rules for my contributions, so that secrets are protected. |
| 8.4.4 | P1 | As a researcher, I want to purchase trajectory datasets, so that I can train models. |

### Revenue Split Distribution

| ID | Priority | User Story |
|----|----------|------------|
| 8.5.1 | P0 | As a platform operator, I want to configure revenue splits between creators and providers, so that earnings are fairly distributed. |
| 8.5.2 | P0 | As a skill creator, I want to receive my split automatically when skills are purchased, so that I don't have to claim manually. |
| 8.5.3 | P1 | As a compute provider, I want to see my pending and paid revenue shares, so that I can track earnings. |

### Autopilot as Compute Buyer (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 8.6.1 | P0 | As Autopilot, I want to post SandboxRun job requests to run tests against a repo, so that I can verify my changes. |
| 8.6.2 | P0 | As Autopilot, I want to select a provider based on price, latency, and reputation, so that I get good value. |
| 8.6.3 | P0 | As Autopilot, I want to verify job results before releasing payment, so that I don't pay for bad work. |
| 8.6.4 | P0 | As Autopilot, I want to fall back to the reserve pool if no providers bid, so that jobs always run. |
| 8.6.5 | P1 | As Autopilot, I want to post RepoIndex jobs to generate embeddings, so that I can search code semantically. |

### Provider Onboarding (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 8.7.1 | P0 | As a new provider, I want to pass a qualification suite, so that I can start accepting jobs. |
| 8.7.2 | P0 | As a provider, I want to see my tier and quota, so that I know my limits. |
| 8.7.3 | P1 | As a provider, I want to be promoted to higher tiers based on success rate, so that I can handle more jobs. |

---

## d-009: Autopilot GUI

### Session View

| ID | Priority | User Story |
|----|----------|------------|
| 9.1.1 | P0 | As a user, I want to see the conversation thread between me and the agent, so that I can follow progress. |
| 9.1.2 | P0 | As a user, I want to see tool calls with their output, so that I understand what the agent is doing. |
| 9.1.3 | P0 | As a user, I want to type a prompt and send it to the agent, so that I can direct the work. |
| 9.1.4 | P1 | As a user, I want to expand/collapse thinking blocks, so that I can see or hide reasoning. |
| 9.1.5 | P1 | As a user, I want to see a token usage gauge, so that I know my consumption. |

### Control Panel

| ID | Priority | User Story |
|----|----------|------------|
| 9.2.1 | P0 | As a user, I want to start a new autopilot session, so that I can begin work. |
| 9.2.2 | P0 | As a user, I want to stop a running session, so that I can interrupt if needed. |
| 9.2.3 | P1 | As a user, I want to switch between multiple active sessions, so that I can multitask. |
| 9.2.4 | P1 | As a user, I want to see session history, so that I can resume past work. |
| 9.2.5 | P2 | As a user, I want to export a session transcript, so that I can share the conversation. |

### Metrics Display

| ID | Priority | User Story |
|----|----------|------------|
| 9.3.1 | P0 | As a user, I want to see my current APM, so that I know how fast the agent is working. |
| 9.3.2 | P1 | As a user, I want to see error rate for the session, so that I can assess quality. |
| 9.3.3 | P1 | As a user, I want to see cost estimate for the session, so that I can budget. |
| 9.3.4 | P2 | As a user, I want to see a timeline of agent activity, so that I can visualize the flow. |

### Replay Viewer (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 9.4.1 | P0 | As a user, I want to scrub through a completed run's timeline, so that I can review what happened. |
| 9.4.2 | P0 | As a user, I want to see diffs inline during replay, so that I understand the changes made. |
| 9.4.3 | P0 | As a user, I want to see receipts (tests run, CI status) during replay, so that I can verify outcomes. |
| 9.4.4 | P0 | As a user, I want a "skip to outcomes" button, so that I can jump to the final PR/result. |
| 9.4.5 | P1 | As a user, I want to share a replay link, so that others can view the run. |
| 9.4.6 | P1 | As a user, I want to download the replay bundle, so that I can archive or demo it. |

### Approval UX (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 9.5.1 | P0 | As a user, I want to see pending approvals in the GUI, so that I can respond to Autopilot requests. |
| 9.5.2 | P0 | As a user, I want to approve/deny actions with context visible (file, command), so that I make informed decisions. |
| 9.5.3 | P0 | As a user, I want approval options (once / repo / always), so that I can set my preference level. |

---

## d-010: Unified Binary

### Subcommand Structure

| ID | Priority | User Story |
|----|----------|------------|
| 10.1.1 | P0 | As a user, I want to run `openagents` without arguments to launch the GUI, so that I have a visual interface. |
| 10.1.2 | P0 | As a user, I want to run `openagents wallet init` to initialize my wallet, so that I can manage identity. |
| 10.1.3 | P0 | As a user, I want to run `openagents autopilot run "task"` to start an autonomous run, so that work happens. |
| 10.1.4 | P0 | As a user, I want to run `openagents daemon start` to launch the background daemon, so that agents run continuously. |
| 10.1.5 | P1 | As a user, I want to run `openagents --help` to see all available commands, so that I can discover features. |
| 10.1.6 | P1 | As a user, I want to run `openagents <subcommand> --help` for detailed help, so that I understand options. |

### Backward Compatibility

| ID | Priority | User Story |
|----|----------|------------|
| 10.2.1 | P1 | As a user with legacy scripts, I want deprecation warnings when using old binary names, so that I can migrate. |
| 10.2.2 | P2 | As a user, I want symlinks for old binary names to keep working temporarily, so that scripts don't break. |

### Install & Connect CLI (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 10.3.1 | P0 | As a new user, I want to run a simple install command (e.g., `curl | sh`), so that I can get started quickly. |
| 10.3.2 | P0 | As a user, I want to run `openagents connect <repo>` to link my GitHub repo, so that Autopilot can work on it. |
| 10.3.3 | P1 | As a user, I want to run `openagents status` to see my connected repos and Autopilot state, so that I know my setup. |

---

## d-011: Storybook Coverage

### Component Stories

| ID | Priority | User Story |
|----|----------|------------|
| 11.1.1 | P0 | As a developer, I want to see Button stories with all variants, so that I understand how to use it. |
| 11.1.2 | P0 | As a developer, I want to see TextInput stories with different states, so that I can use it correctly. |
| 11.1.3 | P1 | As a developer, I want to see all atom components in a gallery, so that I can browse available UI elements. |
| 11.1.4 | P1 | As a developer, I want copy-pasteable code snippets for each story, so that I can use components quickly. |
| 11.1.5 | P2 | As a developer, I want interactive controls to tweak component props, so that I can experiment. |

### Storybook Navigation

| ID | Priority | User Story |
|----|----------|------------|
| 11.2.1 | P0 | As a developer, I want hierarchical navigation (atoms/molecules/organisms), so that I can find components. |
| 11.2.2 | P1 | As a developer, I want to search stories by name, so that I can find specific components. |
| 11.2.3 | P2 | As a developer, I want to bookmark frequently used stories, so that I can access them quickly. |

---

## d-012: No Stubs Policy

### Code Quality

| ID | Priority | User Story |
|----|----------|------------|
| 12.1.1 | P0 | As a developer, I want pre-commit hooks to reject `todo!()`, so that stubs don't enter main. |
| 12.1.2 | P0 | As a developer, I want pre-commit hooks to reject `unimplemented!()`, so that incomplete code is blocked. |
| 12.1.3 | P1 | As a developer, I want CI to scan for stub patterns, so that nothing slips through. |
| 12.1.4 | P1 | As a developer, I want to see a list of allowed exceptions (with justification), so that I know what's permitted. |

---

## d-013: Testing Framework

### Unit Tests

| ID | Priority | User Story |
|----|----------|------------|
| 13.1.1 | P0 | As a developer, I want to run `cargo test` to execute all unit tests, so that I can verify correctness. |
| 13.1.2 | P0 | As a developer, I want tests to run in parallel, so that the suite finishes quickly. |
| 13.1.3 | P1 | As a developer, I want to see code coverage reports, so that I know what's untested. |
| 13.1.4 | P1 | As a developer, I want property-based tests for encoders/validators, so that edge cases are covered. |

### Integration Tests

| ID | Priority | User Story |
|----|----------|------------|
| 13.2.1 | P0 | As a developer, I want integration tests to use in-memory databases, so that they're isolated. |
| 13.2.2 | P0 | As a developer, I want integration tests to not require network access, so that they run offline. |
| 13.2.3 | P1 | As a developer, I want a TestApp pattern for setting up test contexts, so that boilerplate is minimal. |

### Snapshot Tests

| ID | Priority | User Story |
|----|----------|------------|
| 13.3.1 | P0 | As a developer, I want snapshot tests for WGPUI scenes, so that visual regressions are caught. |
| 13.3.2 | P1 | As a developer, I want to update snapshots with a single command, so that intentional changes are easy. |
| 13.3.3 | P1 | As a developer, I want snapshot diffs in CI, so that I can review changes in PRs. |

### Demo Testing (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 13.4.1 | P0 | As a developer, I want CI to render the demo replay headlessly, so that I know it still works. |
| 13.4.2 | P0 | As a developer, I want demo regression tests to catch breaks before deploy, so that the homepage stays functional. |
| 13.4.3 | P1 | As a developer, I want performance tests for demo loading (<3 seconds), so that user experience is fast. |
| 13.4.4 | P1 | As a developer, I want accessibility tests for the demo (captions, keyboard nav), so that it's inclusive. |

---

## d-014: NIP-SA/Bifrost Integration Tests

### Bifrost Tests

| ID | Priority | User Story |
|----|----------|------------|
| 14.1.1 | P0 | As a developer, I want E2E tests for 2-of-3 threshold signing, so that the common case is verified. |
| 14.1.2 | P0 | As a developer, I want E2E tests for threshold ECDH decryption, so that DM reading works. |
| 14.1.3 | P1 | As a developer, I want tests for peer discovery over test relays, so that coordination works. |
| 14.1.4 | P1 | As a developer, I want tests for timeout handling when peers are offline, so that failures are graceful. |

### NIP-SA Tests

| ID | Priority | User Story |
|----|----------|------------|
| 14.2.1 | P0 | As a developer, I want E2E tests for agent profile publish/fetch, so that discovery works. |
| 14.2.2 | P0 | As a developer, I want E2E tests for encrypted state round-trips, so that persistence works. |
| 14.2.3 | P0 | As a developer, I want E2E tests for tick request/result lifecycle, so that heartbeats work. |
| 14.2.4 | P1 | As a developer, I want E2E tests for trajectory publish/verify, so that transparency works. |

---

## d-015: Marketplace E2E Tests

### Compute Tests

| ID | Priority | User Story |
|----|----------|------------|
| 15.1.1 | P0 | As a developer, I want E2E tests for NIP-90 job submission, so that DVM requests work. |
| 15.1.2 | P0 | As a developer, I want E2E tests for job result delivery, so that outputs arrive correctly. |
| 15.1.3 | P1 | As a developer, I want E2E tests for job feedback flow, so that ratings work. |

### Skill Tests

| ID | Priority | User Story |
|----|----------|------------|
| 15.2.1 | P0 | As a developer, I want E2E tests for skill browsing, so that discovery works. |
| 15.2.2 | P0 | As a developer, I want E2E tests for skill purchase with mock payment, so that licensing works. |
| 15.2.3 | P1 | As a developer, I want E2E tests for encrypted skill delivery, so that content is protected. |

### Agent Commerce Tests

| ID | Priority | User Story |
|----|----------|------------|
| 15.3.1 | P0 | As a developer, I want E2E tests for agent-to-agent transactions, so that the economy works. |
| 15.3.2 | P1 | As a developer, I want E2E tests for budget constraint enforcement, so that agents don't overspend. |

---

## d-016: APM Metrics

### Data Collection

| ID | Priority | User Story |
|----|----------|------------|
| 16.1.1 | P0 | As an operator, I want APM calculated from Codex Code JSONL logs, so that interactive usage is tracked. |
| 16.1.2 | P0 | As an operator, I want APM calculated from autopilot trajectory logs, so that autonomous usage is tracked. |
| 16.1.3 | P1 | As an operator, I want APM tracked across multiple time windows, so that I can see trends. |

### Display

| ID | Priority | User Story |
|----|----------|------------|
| 16.2.1 | P0 | As a user, I want to see my current APM in the CLI, so that I know my velocity. |
| 16.2.2 | P0 | As a user, I want to see APM in the GUI dashboard, so that I have visual feedback. |
| 16.2.3 | P1 | As a user, I want APM color-coded by tier, so that I quickly understand performance level. |
| 16.2.4 | P2 | As a user, I want to see APM history charts, so that I can analyze trends. |

### APM as Marketing KPI (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 16.3.1 | P0 | As a marketing lead, I want to show Autopilot's APM in demos, so that prospects see agent velocity. |
| 16.3.2 | P0 | As a marketing lead, I want APM visible in replay bundles, so that demo viewers see the metric. |
| 16.3.3 | P1 | As a marketing lead, I want APM leaderboards for public Autopilot runs, so that performance is visible. |

---

## d-017: ACP Integration

### Protocol Support

| ID | Priority | User Story |
|----|----------|------------|
| 17.1.1 | P0 | As a developer, I want to send ACP messages to Codex Code, so that sessions work. |
| 17.1.2 | P0 | As a developer, I want to receive ACP events from Codex Code, so that responses stream. |
| 17.1.3 | P1 | As a developer, I want to switch between Codex/Codex backends, so that I can choose agents. |
| 17.1.4 | P1 | As a developer, I want to convert ACP events to rlog format, so that replay works. |

### Session Management

| ID | Priority | User Story |
|----|----------|------------|
| 17.2.1 | P0 | As a user, I want to start an ACP session, so that I can interact with an agent. |
| 17.2.2 | P0 | As a user, I want to send messages and receive responses, so that conversation works. |
| 17.2.3 | P1 | As a user, I want to replay old sessions from rlog files, so that I can review past work. |

---

## d-018: Parallel Container Isolation

### Container Management

| ID | Priority | User Story |
|----|----------|------------|
| 18.1.1 | P0 | As an operator, I want to start N autopilot containers in parallel, so that throughput increases. |
| 18.1.2 | P0 | As an operator, I want each container to have its own git worktree, so that agents don't conflict. |
| 18.1.3 | P0 | As an operator, I want containers to share the issue database, so that coordination works. |
| 18.1.4 | P1 | As an operator, I want to see status of all running agents, so that I can monitor progress. |
| 18.1.5 | P1 | As an operator, I want to stop individual agents, so that I can control resources. |

### Resource Management

| ID | Priority | User Story |
|----|----------|------------|
| 18.2.1 | P0 | As an operator, I want agents to respect memory limits, so that the host isn't overwhelmed. |
| 18.2.2 | P1 | As an operator, I want platform-aware defaults (10 agents on Linux, 5 on macOS), so that resources are tuned. |
| 18.2.3 | P2 | As an operator, I want to customize resource limits per agent, so that I can tune performance. |

### Fleet on One Repo (Launch Priority)

| ID | Priority | User Story |
|----|----------|------------|
| 18.3.1 | P0 | As an operator, I want to run a fleet of Autopilot agents on the OpenAgents repo, so that we dogfood the product. |
| 18.3.2 | P0 | As an operator, I want fleet runs to generate demo-worthy trajectories, so that we have marketing content. |
| 18.3.3 | P1 | As an operator, I want to select the best fleet runs for public demos, so that we showcase quality work. |

---

## d-019: GPT-OSS Local Inference

### Model Access

| ID | Priority | User Story |
|----|----------|------------|
| 19.1.1 | P0 | As a developer, I want to run inference with gpt-oss-120b, so that I can use the largest model. |
| 19.1.2 | P0 | As a developer, I want to run inference with gpt-oss-20b, so that I can use a faster model. |
| 19.1.3 | P0 | As a developer, I want to check if GPT-OSS is available locally, so that I know if I can use it. |
| 19.1.4 | P1 | As a developer, I want to stream responses, so that output appears incrementally. |

### Agent Integration

| ID | Priority | User Story |
|----|----------|------------|
| 19.2.1 | P0 | As an operator, I want to run autopilot with GPT-OSS as the backend, so that I use local inference. |
| 19.2.2 | P1 | As a GUI user, I want to select GPT-OSS from the model dropdown, so that I can switch models. |
| 19.2.3 | P1 | As a developer, I want GPT-OSS to support tool calls, so that full agent capabilities work. |

---

## d-020: WGPUI Component Integration

### Foundation Components

| ID | Priority | User Story |
|----|----------|------------|
| 20.1.1 | P0 | As a developer, I want Button components with all variants, so that I can build interactive UIs. |
| 20.1.2 | P0 | As a developer, I want TextInput components, so that users can enter text. |
| 20.1.3 | P0 | As a developer, I want Dropdown components, so that users can select options. |
| 20.1.4 | P0 | As a developer, I want Modal components, so that I can show dialogs. |
| 20.1.5 | P0 | As a developer, I want ScrollView components, so that content can scroll. |

### ACP Component Parity

| ID | Priority | User Story |
|----|----------|------------|
| 20.2.1 | P0 | As a developer, I want all ACP atoms ported to WGPUI, so that I have the same building blocks. |
| 20.2.2 | P0 | As a developer, I want all ACP molecules ported to WGPUI, so that I have composite components. |
| 20.2.3 | P0 | As a developer, I want all ACP organisms ported to WGPUI, so that I have complete features. |
| 20.2.4 | P1 | As a developer, I want HUD components (StatusBar, Notifications), so that I have overlays. |

---

## d-021: OpenCode SDK

### API Integration

| ID | Priority | User Story |
|----|----------|------------|
| 21.1.1 | P0 | As a developer, I want to connect to an OpenCode server, so that I can use its capabilities. |
| 21.1.2 | P0 | As a developer, I want to send messages and receive responses, so that conversation works. |
| 21.1.3 | P0 | As a developer, I want to receive SSE events, so that I get real-time updates. |
| 21.1.4 | P1 | As a developer, I want to list available providers, so that I can choose backends. |

### Server Management

| ID | Priority | User Story |
|----|----------|------------|
| 21.2.1 | P0 | As a developer, I want to spawn an OpenCode server process, so that I can use it locally. |
| 21.2.2 | P1 | As a developer, I want to stop the server gracefully, so that resources are freed. |

---

## d-022: Agent Orchestration

### Agent Management

| ID | Priority | User Story |
|----|----------|------------|
| 22.1.1 | P0 | As an orchestrator, I want to spawn specialized sub-agents, so that tasks are delegated. |
| 22.1.2 | P0 | As an orchestrator, I want to collect results from sub-agents, so that I can synthesize responses. |
| 22.1.3 | P1 | As an orchestrator, I want to timeout slow agents, so that I'm not blocked indefinitely. |
| 22.1.4 | P1 | As an orchestrator, I want to retry failed agents, so that transient failures are handled. |

### Lifecycle Hooks

| ID | Priority | User Story |
|----|----------|------------|
| 22.2.1 | P0 | As an orchestrator, I want session start hooks, so that I can set up context. |
| 22.2.2 | P0 | As an orchestrator, I want session end hooks, so that I can clean up. |
| 22.2.3 | P1 | As an orchestrator, I want message hooks, so that I can transform inputs/outputs. |
| 22.2.4 | P1 | As an orchestrator, I want error hooks, so that I can handle failures gracefully. |

---

## d-023: WGPUI Framework

### Rendering

| ID | Priority | User Story |
|----|----------|------------|
| 23.1.1 | P0 | As a developer, I want to render quads with colors and borders, so that I can draw rectangles. |
| 23.1.2 | P0 | As a developer, I want to render text with different fonts and sizes, so that I can display content. |
| 23.1.3 | P0 | As a developer, I want GPU-accelerated rendering at 60fps, so that the UI is smooth. |
| 23.1.4 | P1 | As a developer, I want to render on web via WebGPU, so that browser apps work. |
| 23.1.5 | P1 | As a developer, I want to render on desktop via Vulkan/Metal/DX12, so that native apps work. |

### Layout

| ID | Priority | User Story |
|----|----------|------------|
| 23.2.1 | P0 | As a developer, I want Flexbox layout, so that I can arrange components. |
| 23.2.2 | P1 | As a developer, I want percentage-based sizing, so that layouts are responsive. |
| 23.2.3 | P1 | As a developer, I want margin/padding/gap, so that I can space elements. |

### Input Handling

| ID | Priority | User Story |
|----|----------|------------|
| 23.3.1 | P0 | As a developer, I want to handle mouse clicks, so that buttons work. |
| 23.3.2 | P0 | As a developer, I want to handle keyboard input, so that text fields work. |
| 23.3.3 | P1 | As a developer, I want to handle mouse hover, so that hover states work. |
| 23.3.4 | P1 | As a developer, I want to handle scroll events, so that scrolling works. |

---

## d-024: Arwes Parity

### Frame Styles

| ID | Priority | User Story |
|----|----------|------------|
| 24.1.1 | P0 | As a developer, I want all 6 frame styles (Corners, Lines, Octagon, etc.), so that I can create sci-fi UI. |
| 24.1.2 | P1 | As a developer, I want 3 additional frame styles (Nero, Header, Circle), so that I have full parity. |
| 24.1.3 | P1 | As a developer, I want animated frame corners, so that UI feels alive. |

### Easing Functions

| ID | Priority | User Story |
|----|----------|------------|
| 24.2.1 | P0 | As a developer, I want all 13 current easing functions, so that animations are smooth. |
| 24.2.2 | P1 | As a developer, I want 18 additional easing functions (Quart, Bounce, etc.), so that I have full control. |

### Text Effects

| ID | Priority | User Story |
|----|----------|------------|
| 24.3.1 | P0 | As a developer, I want Sequence text effect (char-by-char reveal), so that text animates in. |
| 24.3.2 | P1 | As a developer, I want Decipher text effect (scramble reveal), so that text has a hacker feel. |
| 24.3.3 | P1 | As a developer, I want blinking cursor, so that inputs feel responsive. |

### Backgrounds

| ID | Priority | User Story |
|----|----------|------------|
| 24.4.1 | P0 | As a developer, I want DotsGrid background, so that I can create sci-fi panels. |
| 24.4.2 | P1 | As a developer, I want GridLines background, so that I have more options. |
| 24.4.3 | P1 | As a developer, I want MovingLines background, so that UI feels dynamic. |
| 24.4.4 | P2 | As a developer, I want Puffs background, so that I can add particle effects. |

---

## d-025: All-In WGPUI

### Framework Features

| ID | Priority | User Story |
|----|----------|------------|
| 25.1.1 | P0 | As a developer, I want Entity system for reactive state, so that UI updates automatically. |
| 25.1.2 | P0 | As a developer, I want Element lifecycle (layout/prepaint/paint), so that rendering is structured. |
| 25.1.3 | P0 | As a developer, I want Window abstraction, so that I can create native windows. |
| 25.1.4 | P1 | As a developer, I want Styled trait for fluent builder DSL, so that styling is ergonomic. |
| 25.1.5 | P1 | As a developer, I want async support via cx.spawn(), so that I can do background work. |

### Web Stack Removal

| ID | Priority | User Story |
|----|----------|------------|
| 25.2.1 | P0 | As a maintainer, I want the HTML/Maud stack archived, so that there's one UI path. |
| 25.2.2 | P0 | As a developer, I want autopilot-gui rebuilt in pure WGPUI, so that no web dependencies remain. |
| 25.2.3 | P1 | As a developer, I want all examples to be WGPUI-only, so that documentation is consistent. |

---

## d-026: E2E Test Live Viewer

### Test DSL

| ID | Priority | User Story |
|----|----------|------------|
| 26.1.1 | P0 | As a tester, I want to write tests with fluent DSL, so that test code is readable. |
| 26.1.2 | P0 | As a tester, I want to click elements by selector, so that I can simulate user interaction. |
| 26.1.3 | P0 | As a tester, I want to type text into inputs, so that I can test forms. |
| 26.1.4 | P0 | As a tester, I want to assert element existence, so that I can verify UI state. |
| 26.1.5 | P1 | As a tester, I want to wait for elements to appear, so that async UI works. |

### Live Viewer

| ID | Priority | User Story |
|----|----------|------------|
| 26.2.1 | P0 | As a tester, I want to see tests execute in real-time, so that I can watch what happens. |
| 26.2.2 | P0 | As a tester, I want to see click ripples, so that I know where clicks occurred. |
| 26.2.3 | P0 | As a tester, I want to see key presses displayed, so that I know what was typed. |
| 26.2.4 | P1 | As a tester, I want to pause/step through tests, so that I can debug failures. |
| 26.2.5 | P1 | As a tester, I want playback speed control, so that I can watch slowly or quickly. |

### Test Harness

| ID | Priority | User Story |
|----|----------|------------|
| 26.3.1 | P0 | As a tester, I want to wrap any component in a test harness, so that it can be tested. |
| 26.3.2 | P0 | As a tester, I want synthetic events injected into components, so that interaction is simulated. |
| 26.3.3 | P1 | As a tester, I want control bar with play/pause/step, so that I control execution. |
| 26.3.4 | P2 | As a tester, I want to record tests by performing actions, so that I don't write code. |

---

## d-027: Autopilot Demo + Dogfooding Funnel (Launch Priority)

### Public Demo

| ID | Priority | User Story |
|----|----------|------------|
| 27.1.1 | P0 | As a visitor, I want to see a real Autopilot replay on the homepage, so that I understand what the product does. |
| 27.1.2 | P0 | As a visitor, I want the demo to show issue→PR flow in 90-120 seconds, so that I don't lose interest. |
| 27.1.3 | P0 | As a visitor, I want to see receipts (tests passed, CI green), so that I trust the outcome is real. |
| 27.1.4 | P0 | As a visitor, I want a clear CTA ("Try It On Your Repo") after the demo, so that I can try it myself. |
| 27.1.5 | P1 | As a visitor, I want to scrub/replay the demo, so that I can examine details. |

### Free Repo Connection

| ID | Priority | User Story |
|----|----------|------------|
| 27.2.1 | P0 | As a visitor, I want to connect my GitHub repo without paying first, so that I can see value before committing. |
| 27.2.2 | P0 | As a visitor, I want GitHub OAuth to be fast and simple, so that I don't drop off. |
| 27.2.3 | P0 | As a new user, I want to select which repo to connect, so that I try Autopilot on my real code. |

### Free First Analysis

| ID | Priority | User Story |
|----|----------|------------|
| 27.3.1 | P0 | As a new user, I want Autopilot to scan my repo and show what it found, so that I see immediate value. |
| 27.3.2 | P0 | As a new user, I want to see which issues Autopilot can handle, so that I understand its capabilities. |
| 27.3.3 | P0 | As a new user, I want to see "this would take X hours manually", so that I understand the value proposition. |
| 27.3.4 | P1 | As a new user, I want suggested first actions, so that I know where to start. |

### Free Trial Run

| ID | Priority | User Story |
|----|----------|------------|
| 27.4.1 | P0 | As a new user, I want 1 free Autopilot run, so that I experience the product before paying. |
| 27.4.2 | P0 | As a new user, I want to see real results (PR created, tests run), so that I trust the product works. |
| 27.4.3 | P1 | As a new user, I want to pick which issue to run on, so that I try it on something relevant. |

### Upgrade to Paid

| ID | Priority | User Story |
|----|----------|------------|
| 27.5.1 | P0 | As a trial user, I want to see what I got for free vs what paid unlocks, so that I understand the upgrade value. |
| 27.5.2 | P0 | As a trial user, I want to pay with Stripe, so that I can use a credit card. |
| 27.5.3 | P0 | As a trial user, I want to pay with Lightning, so that I can use Bitcoin. |
| 27.5.4 | P0 | As a trial user, I want immediate access after payment, so that I can continue working. |

### Replay Publishing Pipeline

| ID | Priority | User Story |
|----|----------|------------|
| 27.6.1 | P0 | As an operator, I want to promote a run to public demo status, so that visitors can see it. |
| 27.6.2 | P0 | As an operator, I want secrets automatically redacted before publish, so that nothing sensitive leaks. |
| 27.6.3 | P0 | As an operator, I want replay bundles hosted on CDN, so that they load quickly. |
| 27.6.4 | P1 | As an operator, I want to rotate demos without breaking old links, so that versioning works. |

---

## Summary Statistics

| Directive | User Stories | P0 | P1 | P2 |
|-----------|-------------|----|----|-----|
| d-001 | 27 | 17 | 9 | 1 |
| d-002 | 22 | 14 | 7 | 1 |
| d-003 | 24 | 14 | 8 | 2 |
| d-004 | 38 | 23 | 13 | 2 |
| d-005 | 22 | 12 | 8 | 2 |
| d-006 | 16 | 9 | 6 | 1 |
| d-007 | 14 | 8 | 5 | 1 |
| d-008 | 29 | 18 | 10 | 1 |
| d-009 | 22 | 13 | 7 | 2 |
| d-010 | 10 | 7 | 3 | 0 |
| d-011 | 8 | 3 | 3 | 2 |
| d-012 | 4 | 2 | 2 | 0 |
| d-013 | 14 | 7 | 6 | 1 |
| d-014 | 8 | 5 | 3 | 0 |
| d-015 | 6 | 4 | 2 | 0 |
| d-016 | 10 | 5 | 4 | 1 |
| d-017 | 7 | 4 | 3 | 0 |
| d-018 | 11 | 6 | 4 | 1 |
| d-019 | 7 | 4 | 3 | 0 |
| d-020 | 9 | 7 | 2 | 0 |
| d-021 | 6 | 4 | 2 | 0 |
| d-022 | 8 | 4 | 4 | 0 |
| d-023 | 12 | 6 | 5 | 1 |
| d-024 | 11 | 4 | 6 | 1 |
| d-025 | 8 | 4 | 3 | 1 |
| d-026 | 13 | 7 | 5 | 1 |
| d-027 | 23 | 19 | 4 | 0 |
| **TOTAL** | **389** | **241** | **138** | **22** |

---

## Implementation Notes

### Test Priority Guidelines

- **P0 (Critical)**: Must work for the feature to be considered complete. Block release if failing.
- **P1 (High)**: Important for quality user experience. Should be fixed before release.
- **P2 (Medium)**: Nice to have. Can be deferred if time-constrained.
- **P3 (Low)**: Edge cases and polish. Implement when capacity allows.

### Mapping to Test Types

| User Story Type | Test Type |
|-----------------|-----------|
| "I want to see X" | Snapshot/visual test |
| "I want to click X and Y happens" | E2E interaction test |
| "I want to run `command`" | Integration test |
| "I want error message when..." | Error handling test |
| "I want to receive notification" | Async behavior test |

### Creating Tests from Stories

```rust
// From story 26.1.1: "As a tester, I want to write tests with fluent DSL"
test("DSL Fluent API Works")
    .click("#button")
    .type_text("hello")
    .expect("#result")
    .build();

// From story 1.3.1: "As a user, I want to send Bitcoin to a Lightning invoice"
test("Send to Lightning Invoice")
    .click("#send-button")
    .click("#invoice-input")
    .type_text("lnbc...")
    .click("#amount-input")
    .type_text("1000")
    .click("#confirm-button")
    .wait_for("#success-screen", 5000)
    .expect_text("#status", "Payment Sent")
    .build();
```

---

*Last updated: 2025-12-26*
