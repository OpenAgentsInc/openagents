# Audit Issues List

This document tracks all issues to be created from the 2025-12-22 codebase audit.
Issues are organized by priority then by subsystem.

---

## CRITICAL / HIGH PRIORITY

### Security - Path Traversal

- [ ] **SEC-1**: Fix GitAfter path traversal vulnerability - repository identifiers from HTTP paths used directly in filesystem joins allow `../` to escape workspace
  - Files: `crates/gitafter/src/server.rs:1725`, `crates/gitafter/src/git/clone.rs:109`
  - Ref: E-H-3

- [ ] **SEC-2**: Fix plan mode path traversal vulnerability - MCP slug parameter used in join without sanitization allows arbitrary file write
  - Files: `crates/issues-mcp/src/main.rs:700`, `crates/autopilot/src/planmode.rs:83`
  - Ref: D-H-1

- [ ] **SEC-2b**: Fix get_repository_path path traversal - repo_identifier joined directly without validation
  - Files: `crates/gitafter/src/git/clone.rs:103-110`
  - Ref: Deep audit pass 2

### Security - Injection

- [ ] **SEC-5**: Fix shell injection in notifications - only double quotes escaped, backticks and $() can be exploited
  - Files: `crates/gitafter/src/notifications.rs:157-202`
  - Ref: Deep audit pass 2

### Security - Authentication

- [ ] **SEC-6**: Add authentication to API routes - all routes unprotected, anyone on localhost can start/stop autopilot, clone repos, claim bounties
  - Files: `src/gui/routes/autopilot.rs:83-119`, `src/gui/routes/mod.rs`, `crates/gitafter/src/server.rs`, `crates/issues-mcp/src/main.rs`
  - Ref: Deep audit pass 2

- [ ] **SEC-7**: Add WebSocket authentication - no auth check before accepting connections
  - Files: `src/gui/ws.rs:30-35`
  - Ref: Deep audit pass 2

### Security - Secret Exposure

- [ ] **SEC-3**: Fix plaintext mnemonic storage - `.seed` files stored without permission hardening or automatic deletion
  - Files: `crates/compute/src/storage/secure_store.rs:92-121`
  - Ref: E-H-2

- [ ] **SEC-4**: Fix recorder PII/secret leakage - default embeds raw Claude JSONL as comments, leaking sensitive data when logs are shared
  - Files: `crates/recorder/src/convert.rs:26,244`, `crates/recorder/src/main.rs:121`
  - Ref: D6-M-2

- [ ] **SEC-8**: Remove mnemonic from environment variables - GITAFTER_MNEMONIC readable by all processes as same user
  - Files: `crates/gitafter/src/main.rs:46-69`
  - Ref: Deep audit pass 2

- [ ] **SEC-9**: Harden workspace directory permissions - created with default perms, readable by other users on multi-user systems
  - Files: `crates/gitafter/src/git/clone.rs` (create_dir_all calls)
  - Ref: Deep audit pass 2

### Security - Rate Limiting

- [ ] **SEC-10**: Add rate limiting to API endpoints - no protection against DoS via issue spam, clone spam, etc.
  - Files: `src/gui/routes/`, `crates/gitafter/src/server.rs`
  - Ref: Deep audit pass 2

### Security - Input Validation

- [ ] **SEC-11**: Add input length limits on issue operations - no max length on titles, descriptions, directive IDs
  - Files: `crates/issues-mcp/src/main.rs:502-539`
  - Ref: Deep audit pass 2

### Security - SQL Injection

- [ ] **SEC-12**: Fix SQL injection in session queries - project_id interpolated directly into SQL string
  - Files: `crates/issues/src/session.rs:136-143`
  - Ref: Deep audit pass 3 - CRITICAL

### Data Integrity - Critical

- [ ] **DATA-1**: Fix issue creation bypassing issues API - auto_issues.rs omits required columns and double-increments counter
  - Files: `crates/autopilot/src/auto_issues.rs:412-449`, `crates/issues/src/db.rs:110,224`
  - Ref: H-1

- [ ] **DATA-2**: Fix log cleanup protection - expects JSON in integer column, session IDs don't match filenames
  - Files: `crates/autopilot/src/logs.rs:219-242`, `crates/issues/src/db.rs:262-274`
  - Ref: H-2

- [ ] **DATA-3**: Fix relay queue infinite loop - dequeue never marks items sent/failed, loop spins on same item
  - Files: `crates/nostr/client/src/relay.rs:607-656`, `crates/nostr/client/src/queue.rs:170-208`
  - Ref: D6-H-1, E-M-4

- [ ] **DATA-4**: Add ON DELETE CASCADE to issue_events foreign key - orphaned events when issues deleted
  - Files: `crates/issues/src/db.rs:131-139`
  - Ref: Deep audit pass 3

- [ ] **DATA-5**: Add ON DELETE CASCADE to metrics tables - tool_calls and anomalies orphaned when sessions deleted
  - Files: `crates/autopilot/src/metrics/mod.rs:325,337`
  - Ref: Deep audit pass 3

- [ ] **DATA-6**: Add logging to migration v2 deletions - silently deletes rows with NULL/empty IDs
  - Files: `crates/issues/src/db.rs:164,168`
  - Ref: Deep audit pass 3

### Database Indexes

- [ ] **DB-1**: Add missing index on issues.agent column - used in WHERE clauses
  - Files: `crates/issues/src/db.rs`
  - Ref: Deep audit pass 3

- [ ] **DB-2**: Add missing index on issues.claimed_by column - used for expiry queries
  - Files: `crates/issues/src/db.rs`
  - Ref: Deep audit pass 3

- [ ] **DB-3**: Use exact match instead of LIKE for event ID lookups - full 64-char hex strings
  - Files: `crates/nostr/relay/src/db.rs:317-333`
  - Ref: Deep audit pass 3

### Safety - Process Management

- [ ] **SAFE-1**: Fix memory cleanup killing unrelated processes - kills any "node" process above threshold, not just autopilot children
  - Files: `crates/autopilot/src/main.rs:51,99`, `crates/autopilot/src/daemon/memory.rs:95`
  - Ref: H-3

- [ ] **SAFE-2**: Fix compute identity orphaning - can generate new identity when encrypted one exists but no password provided
  - Files: `crates/compute/src/app.rs:112-143`
  - Ref: E-H-1

---

## MEDIUM PRIORITY

### Stability - Panic Prevention

- [ ] **PANIC-1**: Fix metrics DB parse failures causing panic - unwrap on timestamps, status enums, anomaly severities
  - Files: `crates/autopilot/src/metrics/mod.rs:496-672`
  - Ref: M-1

- [ ] **PANIC-2**: Fix baseline NaN panic - partial_cmp().unwrap() on floats
  - Files: `crates/autopilot/src/metrics/baseline.rs:157`
  - Ref: M-2

- [ ] **PANIC-3**: Fix dashboard API NaN panic and worker blocking - loads all sessions in memory, partial_cmp().unwrap()
  - Files: `crates/autopilot/src/dashboard.rs:1157-1169`
  - Ref: M-3

- [ ] **PANIC-4**: Fix FROSTR Shamir panic on duplicate indices - GF(256) division panics on zero
  - Files: `crates/frostr/src/keygen.rs:95-235`
  - Ref: D-M-6

- [ ] **PANIC-5**: Fix FM bridge client expect panic - HTTP client init failure panics instead of returning error
  - Files: `crates/fm-bridge/src/client.rs:28-31`
  - Ref: D-L-2

- [ ] **PANIC-6**: Fix Storybook WebSocket handshake panic - unwrap on actix_ws::handle
  - Files: `crates/storybook/src/main.rs:145`
  - Ref: D3-L-3

- [ ] **PANIC-7**: Fix planmode RwLock poisoning panic - write().unwrap() on PLAN_FILE_PATH
  - Files: `crates/autopilot/src/planmode.rs:630`
  - Ref: Deep audit pass 2

- [ ] **PANIC-8**: Fix outbox cache lock poisoning panics - 7+ locations with .unwrap() on cache locks
  - Files: `crates/nostr/client/src/outbox.rs:96,108,130,185,198,204,210`
  - Ref: Deep audit pass 2

- [ ] **PANIC-9**: Fix marketplace compute consumer lock panics - 20+ nested lock().unwrap() calls
  - Files: `crates/marketplace/src/compute/consumer.rs:212-412`
  - Ref: Deep audit pass 2

- [ ] **PANIC-10**: Fix SystemTime unwrap panics - duration_since can fail if clock goes backward
  - Files: `crates/marketplace/src/compute/consumer.rs:74,114,126,138`
  - Ref: Deep audit pass 2

- [ ] **PANIC-11**: Fix guardrails File::create unwrap - panics on file creation failure
  - Files: `crates/autopilot/src/guardrails.rs:155,185`
  - Ref: Deep audit pass 2

- [ ] **PANIC-12**: Fix home_dir expect panic - panics if home directory unavailable
  - Files: `crates/autopilot/src/planmode.rs:84`
  - Ref: Deep audit pass 2

- [ ] **PANIC-13**: Fix admin UTF-8 unwrap - panics on invalid UTF-8 in response body
  - Files: `crates/nostr/relay/src/admin.rs:284`
  - Ref: Deep audit pass 2

- [ ] **PANIC-14**: Fix GitAfter index write unwraps - multiple locations
  - Files: `crates/gitafter/src/git/patch.rs:59`, `crates/gitafter/src/git/rebase.rs:154`, `crates/gitafter/src/git/diff.rs:204`
  - Ref: Deep audit pass 2

- [ ] **PANIC-15**: Fix string slicing without bounds check - panics if pubkey < 16 chars
  - Files: `crates/wallet/src/cli/identity.rs:635`
  - Ref: Deep audit pass 2

### Async/Concurrency Issues

- [ ] **ASYNC-1**: Fix OutboxModel using std::sync::RwLock in async code - can block and panic on poisoning
  - Files: `crates/nostr/client/src/outbox.rs:16,96`
  - Ref: E-M-5

- [ ] **ASYNC-2**: Fix relay DB sync calls in async tasks - rusqlite blocks Tokio executor
  - Files: `crates/nostr/relay/src/server.rs:363,446`, `crates/nostr/relay/src/db.rs:168`
  - Ref: D2-M-2

- [ ] **ASYNC-3**: Fix GitAfter cache sync calls in async tasks - Mutex<EventCache> blocks runtime
  - Files: `crates/gitafter/src/nostr/client.rs:41,153`, `crates/gitafter/src/nostr/cache.rs:10,95`
  - Ref: D4-M-1

- [ ] **ASYNC-4**: Fix Claude status route holding lock across await - blocks writers during network call
  - Files: `src/gui/routes/claude.rs:15-43`
  - Ref: D3-M-2

- [ ] **ASYNC-5**: Fix blocking sleeps in daemon control paths - stalls async tasks
  - Files: `crates/autopilot/src/daemon/supervisor.rs:340,414`
  - Ref: L-5

### Process Lifecycle

- [ ] **PROC-1**: Fix Codex ProcessTransport missing Drop impl - spawned processes can outlive caller
  - Files: `crates/codex-agent-sdk/src/transport/process.rs:46,117`
  - Ref: D2-M-4

- [ ] **PROC-2**: Fix GUI autopilot child surviving stop - only kills cargo process, not actual autopilot
  - Files: `src/gui/routes/autopilot.rs:83,207`
  - Ref: D2-M-5

- [ ] **PROC-3**: Fix Codex SDK output schema temp file race - files dropped before CLI can open them
  - Files: `crates/codex-agent-sdk/src/thread.rs:227,258`
  - Ref: D2-M-3

### Stubbed/Unimplemented Features

- [ ] **STUB-1**: Implement DVM relay subscriptions - always errors, DvmService::start cannot succeed
  - Files: `crates/compute/src/services/dvm_service.rs:120`, `crates/compute/src/services/relay_service.rs:81`
  - Ref: E-M-1

- [ ] **STUB-2**: Implement relay connection logic - marks relays "connected" without network I/O
  - Files: `crates/compute/src/services/relay_service.rs:61`
  - Ref: E-M-2

- [ ] **STUB-3**: Implement or remove Ollama integration - always returns NotAvailable
  - Files: `crates/compute/src/services/ollama_service.rs`
  - Ref: E-M-3

- [ ] **STUB-4**: Implement marketplace payments/pricing/install modules - empty placeholders
  - Files: `crates/marketplace/src/core/payments.rs`, `crates/marketplace/src/compute/pricing.rs`, `crates/marketplace/src/skills/install.rs`
  - Ref: E-M-6

- [ ] **STUB-5**: Fix claude-mcp missing binary source - Cargo.toml declares binary but src/main.rs missing
  - Files: `crates/claude-mcp/Cargo.toml:10-12`
  - Ref: D-M-1

- [ ] **STUB-6**: Implement wallet Bitcoin CLI commands - balance/send/receive/history all bail
  - Files: `crates/wallet/src/cli/bitcoin.rs:5,29`
  - Ref: D4-M-3

- [ ] **STUB-7**: Implement FROSTR wallet CLI - keygen stores only marker, import/export/sign not implemented
  - Files: `crates/wallet/src/cli/frostr.rs:34-96`
  - Ref: D4-L-3

- [ ] **STUB-8**: Fix unified CLI stubbed commands - print "coming soon" or suggest invalid cargo commands
  - Files: `src/cli/autopilot.rs:129,152`, `src/cli/daemon.rs:74`, `src/cli/marketplace.rs:44`, `src/cli/gitafter.rs:24`
  - Ref: D3-M-1

- [ ] **STUB-9**: Wire GUI routes for wallet/marketplace/gitafter/daemon - currently static placeholders
  - Files: `src/gui/routes/wallet.rs:13`, `src/gui/routes/marketplace.rs:14`, `src/gui/routes/gitafter.rs:12`, `src/gui/routes/daemon.rs:9`
  - Ref: D3-L-1

### Identity/Nostr Issues

- [ ] **NOSTR-1**: Fix UnifiedIdentity Nostr key derivation - uses 33-byte compressed key, returns placeholder npub, hardcodes profile to None
  - Files: `crates/wallet/src/core/identity.rs:85-157`
  - Ref: D-M-2

- [ ] **NOSTR-2**: Fix Spark network mapping - collapses Testnet/Signet/Regtest, get_spark_address returns hex not address
  - Files: `crates/spark/src/wallet.rs:39-194`
  - Ref: D-M-3

- [ ] **NOSTR-3**: Fix wallet Nostr fetch only querying first relay - ignores additional configured relays
  - Files: `crates/wallet/src/core/client.rs:79-81`
  - Ref: D-L-1

- [ ] **NOSTR-4**: Fix relay pool publish silently succeeding on partial failure - can return Ok with fewer than min confirmations
  - Files: `crates/nostr/client/src/pool.rs:351,367`
  - Ref: D6-M-3

### Permissions/Guardrails

- [ ] **PERM-1**: Implement autopilot GUI permissions edit/add - actions are stubbed, routes reopen ad-hoc DB
  - Files: `crates/autopilot-gui/src/views/permissions.rs:218,223`, `crates/autopilot-gui/src/server/routes.rs:246,281`
  - Ref: D-M-4

- [ ] **PERM-2**: Fix permission wildcard patterns not matching - check_pattern only does exact match and `tool:*`
  - Files: `crates/autopilot-gui/src/storage/permissions.rs:18,92,113`
  - Ref: D-M-5

- [ ] **PERM-3**: Integrate or remove guardrails module - defined but not enforced in runtime hooks
  - Files: `crates/autopilot/src/guardrails.rs`
  - Ref: M-6

### Logging/Observability

- [ ] **LOG-1**: Fix trajectory streaming silently ignoring errors - JSONL and rlog update errors not reported
  - Files: `crates/autopilot/src/lib.rs:121,176,205`
  - Ref: M-4

- [ ] **LOG-2**: Fix recorder dropping user text in block messages - only extracts ToolResult, ignores Text blocks
  - Files: `crates/recorder/src/convert.rs:103,266`
  - Ref: D6-M-1

- [ ] **LOG-3**: Fix APM CLI placeholder output - shows TODO data, misleading users
  - Files: `crates/autopilot/src/main.rs:4862`
  - Ref: L-2

### Query/Cache Issues

- [ ] **QUERY-1**: Fix GitAfter search LIMIT before filter - returns empty when matches exist beyond first N rows
  - Files: `crates/gitafter/src/nostr/cache.rs:1062-1122`
  - Ref: D4-M-2

- [ ] **QUERY-2**: Enable GitAfter cache foreign keys and fix cleanup - orphan rows left in metadata tables
  - Files: `crates/gitafter/src/nostr/cache.rs:52,546`
  - Ref: D4-L-1

### Network/Protocol

- [ ] **NET-1**: Enforce relay max_message_size - defined but not checked for WebSocket frames
  - Files: `crates/nostr/relay/src/server.rs:33,47`
  - Ref: D2-M-1

- [ ] **NET-2**: Add timeout to Claude usage-limit fetches - default reqwest client can hang indefinitely
  - Files: `src/gui/state.rs:350,356`
  - Ref: D2-L-2

- [ ] **NET-3**: Add caching/backoff to Claude status polling - 5-second refresh with live OAuth calls risks rate limits
  - Files: `crates/ui/src/claude_status.rs:241`, `src/gui/routes/claude.rs:41`
  - Ref: D3-M-3

### Testing

- [ ] **TEST-1**: Fix MockRelay port mismatch - start() returns URL with wrong port, clients can't connect
  - Files: `crates/testing/src/mock_relay.rs:69,82`
  - Ref: D5-M-1

- [ ] **TEST-2**: Implement MockRelay shutdown - currently no-op, leaves servers running
  - Files: `crates/testing/src/mock_relay.rs:153,159`
  - Ref: D5-L-1

- [ ] **TEST-3**: Fix test fixtures using placeholder values - can hide encoding/validation bugs
  - Files: `crates/testing/src/fixtures.rs:7-34`
  - Ref: D5-L-2

- [ ] **TEST-4**: Fix nondeterministic random-text test - can flake in CI
  - Files: `crates/testing/src/fixtures.rs:114-119`
  - Ref: D5-L-3

- [ ] **TEST-5**: Fix autopilot state tests inconsistent with implementation - mock flow vs identity requirement
  - Files: `crates/autopilot/src/state.rs:128,376`
  - Ref: M-5

### Data Validation

- [ ] **VALID-1**: Fix trajectory hash concatenation ambiguity - no delimiter/length prefix weakens integrity
  - Files: `crates/nostr/core/src/nip_sa/trajectory.rs:176,187`
  - Ref: D5-M-2

- [ ] **VALID-2**: Fix PullRequestBuilder not validating required fields - allows invalid NIP-34 events
  - Files: `crates/gitafter/src/nostr/events.rs:503,581`
  - Ref: D4-L-2

- [ ] **VALID-3**: Fix recorder repo_sha validation - only checks length, allows non-hex like "unknown"
  - Files: `crates/recorder/src/lib.rs:834`
  - Ref: D6-L-1

- [ ] **VALID-4**: Fix config merge_with_defaults skipping validation - invalid overrides slip through
  - Files: `crates/config/src/loader.rs:185,207`
  - Ref: D2-L-1

---

## LOW PRIORITY

### Memory/Performance

- [ ] **MEM-1**: Stream log archiving compression - currently reads entire files into memory
  - Files: `crates/autopilot/src/logs.rs:133`
  - Ref: L-1

- [ ] **MEM-2**: Stream recorder file parsing - reads entire files into memory
  - Files: `crates/recorder/src/lib.rs:355,411`, `crates/recorder/src/convert.rs:210`
  - Ref: E-L-1, D6-L-2

- [ ] **MEM-3**: Add backpressure to Nostr subscription channels - unbounded_channel can grow indefinitely
  - Files: `crates/nostr/client/src/subscription.rs:50`, `crates/nostr/client/src/pool.rs:388`
  - Ref: D6-L-3

### Portability/Defaults

- [ ] **PORT-1**: Fix Claude CLI hardcoded zsh - use $SHELL or platform discovery
  - Files: `crates/claude-agent-sdk/src/transport/process.rs:33`
  - Ref: L-3

- [ ] **PORT-2**: Persist SecureStore Argon2 parameters - future changes can make stored data unreadable
  - Files: `crates/compute/src/storage/secure_store.rs:221,261`
  - Ref: E-L-2

### Configuration Issues

- [ ] **CFG-1**: Make storybook port configurable - hardcoded to 3030
  - Files: `crates/storybook/src/main.rs:45`
  - Ref: Deep audit pass 3

- [ ] **CFG-2**: Make autopilot-gui port configurable - hardcoded to 3847
  - Files: `crates/autopilot-gui/src/main.rs:7`
  - Ref: Deep audit pass 3

- [ ] **CFG-3**: Make relay/admin ports configurable - hardcoded to 7000/7001 with unwrap
  - Files: `crates/nostr/relay/src/server.rs:47`, `crates/nostr/relay/src/admin.rs:25`
  - Ref: Deep audit pass 3

- [ ] **CFG-4**: Make FM bridge URL configurable - hardcoded to localhost:3030
  - Files: `crates/fm-bridge/src/client.rs:8`
  - Ref: Deep audit pass 3

- [ ] **CFG-5**: Make daemon timeouts configurable - stall_timeout, recovery_cooldown hardcoded
  - Files: `crates/autopilot/src/daemon/config.rs:100`
  - Ref: Deep audit pass 3

- [ ] **CFG-6**: Make memory thresholds configurable - MIN_AVAILABLE_MEMORY, CLEANUP_THRESHOLD hardcoded
  - Files: `crates/autopilot/src/main.rs:31,35`
  - Ref: Deep audit pass 3

- [ ] **CFG-7**: Make default relays configurable - marketplace uses hardcoded relay list
  - Files: `crates/marketplace/src/relay.rs:14-17`
  - Ref: Deep audit pass 3

- [ ] **CFG-8**: Fix test port conflicts - multiple test files use overlapping hardcoded port ranges
  - Files: `crates/nostr/tests/integration/` (multiple files)
  - Ref: Deep audit pass 3

### Error Handling

- [ ] **ERR-1**: Fix issue timestamp parsing defaulting to now - masks data corruption
  - Files: `crates/issues/src/issue.rs:137,140`
  - Ref: L-4

- [ ] **ERR-2**: Fix directive progress masking DB errors - unwrap_or(0) hides failures
  - Files: `crates/issues/src/directive.rs:309,317`
  - Ref: D2-L-3

- [ ] **ERR-3**: Surface daemon config parse errors - silently ignored via unwrap_or_default
  - Files: `src/cli/daemon.rs:51`
  - Ref: D3-L-2

- [ ] **ERR-4**: Fix silent .ok() error swallowing - errors discarded without logging in planmode, wallet client
  - Files: `crates/autopilot/src/planmode.rs:88`, `crates/wallet/src/core/client.rs:49,116`, `crates/wallet/src/core/nip05.rs:196`
  - Ref: Deep audit pass 2

- [ ] **ERR-5**: Fix dashboard silently returning empty on DB error - unwrap_or_default hides failures
  - Files: `crates/autopilot/src/dashboard.rs:165-166`
  - Ref: Deep audit pass 2

- [ ] **ERR-6**: Add error context to queue lock operations - actual PoisonError discarded
  - Files: `crates/nostr/client/src/queue.rs:150,177,228,248,287,323,359,376,388,410,425`
  - Ref: Deep audit pass 2

- [ ] **ERR-7**: Add error context to contacts lock operations - RwLock poisoning errors discarded
  - Files: `crates/nostr/client/src/contacts.rs:60,141,150,159,171,183,199,210,220,229`
  - Ref: Deep audit pass 2

- [ ] **ERR-8**: Fix relay message parsing with silent defaults - empty string defaults mask protocol errors
  - Files: `crates/nostr/client/src/relay.rs:513,522-524,532,540,548`
  - Ref: Deep audit pass 2

### Integer/Bounds Safety

- [ ] **INT-1**: Fix unchecked u64 to i64 cast in queue - milliseconds can overflow
  - Files: `crates/nostr/client/src/queue.rs:145-148`
  - Ref: Deep audit pass 2

- [ ] **INT-2**: Fix unchecked subtraction in replay - can underflow
  - Files: `crates/autopilot/src/replay.rs:452`
  - Ref: Deep audit pass 2

### Unsafe Code

- [ ] **UNSAFE-1**: Validate pgid in killpg call - no validation of argument before unsafe libc call
  - Files: `crates/autopilot/src/daemon/memory.rs:143-145`
  - Ref: Deep audit pass 2

### Resource Management

- [ ] **RES-1**: Track WebSocket task handles in GitAfter - spawned tasks not stored, can't cancel or await
  - Files: `crates/gitafter/src/ws.rs:43-63`
  - Ref: Deep audit pass 4

- [ ] **RES-2**: Track HTTP server handle for graceful shutdown - tokio::spawn without storing handle
  - Files: `crates/gitafter/src/server.rs:100`
  - Ref: Deep audit pass 4

- [ ] **RES-3**: Add Drop impl to RlogWriter for guaranteed flush - BufWriter may lose data on crash
  - Files: `crates/autopilot/src/rlog.rs:11-38`
  - Ref: Deep audit pass 4

- [ ] **RES-4**: Track Nostr event handler task - spawned without handle, panics are silent
  - Files: `crates/gitafter/src/nostr/client.rs:148-161`
  - Ref: Deep audit pass 4

- [ ] **RES-5**: Add Drop impl to MetricsDb - no explicit cleanup for transactions on panic
  - Files: `crates/autopilot/src/metrics/mod.rs:263-277`
  - Ref: Deep audit pass 4

- [ ] **RES-6**: Add close() method to EventCache - database handles may linger
  - Files: `crates/gitafter/src/nostr/cache.rs:10-31`
  - Ref: Deep audit pass 4

- [ ] **RES-7**: Store and await daemon signal handler task - not awaited on shutdown
  - Files: `crates/autopilot/src/bin/autopilotd.rs:144-160`
  - Ref: Deep audit pass 4

- [ ] **RES-8**: Add graceful shutdown to control socket server - infinite loop with no break signal
  - Files: `crates/autopilot/src/daemon/control.rs:95-111`
  - Ref: Deep audit pass 4

- [ ] **RES-9**: Fix ProcessTransport stdout task cleanup - may not complete on drop
  - Files: `crates/codex-agent-sdk/src/transport/process.rs:86-102`
  - Ref: Deep audit pass 4

- [ ] **RES-10**: Clean up PID file on error paths - stale files left if daemon crashes during init
  - Files: `crates/autopilot/src/bin/autopilotd.rs:125-181`
  - Ref: Deep audit pass 4

- [ ] **RES-11**: Handle broadcast channel capacity limits - can drop messages silently
  - Files: `crates/gitafter/src/ws.rs:10-18`
  - Ref: Deep audit pass 4

---

## Summary

| Priority | Count |
|----------|-------|
| Critical/High (Security, Data Integrity, Safety) | 23 |
| Medium (Panic, Async, Stubs, Nostr, Permissions) | 55 |
| Low (Memory, Config, Error Handling, Resources) | 37 |
| **Total** | **115** |

### By Category

| Category | Count |
|----------|-------|
| Security - Path Traversal | 3 |
| Security - Injection | 2 |
| Security - Authentication | 2 |
| Security - Secret Exposure | 4 |
| Security - Rate Limiting | 1 |
| Security - Input Validation | 1 |
| Data Integrity | 6 |
| Database Indexes | 3 |
| Safety - Process Management | 2 |
| Panic Prevention | 15 |
| Async/Concurrency | 5 |
| Process Lifecycle | 3 |
| Stubbed Features | 9 |
| Identity/Nostr | 4 |
| Permissions | 3 |
| Logging | 3 |
| Query/Cache | 2 |
| Network | 3 |
| Testing | 5 |
| Validation | 4 |
| Memory/Performance | 3 |
| Portability | 2 |
| Configuration | 8 |
| Error Handling | 8 |
| Integer/Bounds Safety | 2 |
| Unsafe Code | 1 |
| Resource Management | 11 |

---

## Recommended Priority Order

### Phase 1: Critical Security (Do First)
1. SEC-12: SQL injection in session queries
2. SEC-1, SEC-2, SEC-2b: Path traversal vulnerabilities
3. SEC-5: Shell injection in notifications
4. SEC-3, SEC-8: Secret exposure (mnemonics)
5. SEC-6, SEC-7: Authentication on routes

### Phase 2: Data Integrity
6. DATA-1 through DATA-6: Database integrity issues
7. DB-1, DB-2, DB-3: Missing indexes

### Phase 3: Stability (Panic Prevention)
8. PANIC-1 through PANIC-15: All panic-inducing code paths

### Phase 4: Reliability
9. ASYNC-1 through ASYNC-5: Concurrency issues
10. RES-1 through RES-11: Resource management
11. ERR-1 through ERR-8: Error handling

### Phase 5: Completeness
12. STUB-1 through STUB-9: Implement stubs
13. NOSTR-1 through NOSTR-4: Nostr integration fixes

### Phase 6: Polish
14. CFG-1 through CFG-8: Configuration
15. TEST-1 through TEST-5: Test reliability
16. MEM-1 through MEM-3: Performance
