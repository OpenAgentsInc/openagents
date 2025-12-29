# Plan: Wire Neobank into Pylon → Autopilot

## Goal
Integrate the completed neobank exchange into Pylon, then wire Pylon's financial capabilities into Autopilot so agents can autonomously manage their Bitcoin treasury.

## Pre-requisite
**Commit and push the neobank implementation first** (127 tests passing).

---

## Current State

### Neobank (COMPLETE - ready to integrate):
- `wallet.rs` - CashuWallet with proof transfer, P2PK
- `settlement.rs` - Mock, ReputationBased, AtomicP2PK modes
- `exchange.rs` - ExchangeClient with relay integration
- `relay.rs` - Nostr relay connectivity
- `reputation.rs` - WoT scoring, trust levels
- `rfq.rs` - RFQ market (NIP-90)
- `treasury_agent.rs` - Market making, bid/ask spreads
- `mint_trust.rs` - Mint discovery (NIP-87)
- `escrow.rs` - Bond/escrow system

### Pylon Architecture:
- Daemon-based (PID file, control socket, signal handling)
- `PylonProvider` - main service container
- `AgentRunner` - spawns agent subprocesses
- Uses `compute` crate for NIP-90 DVM
- Uses `spark` crate for Lightning wallet
- Service injection pattern in `provider.rs`

### Autopilot Architecture:
- `preflight.rs` - discovers environment capabilities
- `pylon_integration.rs` - queries Pylon daemon
- `startup.rs` - state machine with Pylon phases
- `claude.rs` - tool invocation via Claude Agent SDK
- Tools exposed via `McpToolDefinition` pattern

---

## Implementation Plan

### Phase 1: Add Neobank to Pylon

#### 1.1 Add neobank dependency to Pylon
**File:** `crates/pylon/Cargo.toml`
```toml
[dependencies]
neobank = { path = "../neobank" }
```

#### 1.2 Create NeobankService wrapper
**File:** `crates/pylon/src/neobank_service.rs` (NEW ~200 LOC)

```rust
pub struct NeobankService {
    btc_wallet: Option<Arc<CashuWallet>>,
    usd_wallet: Option<Arc<CashuWallet>>,
    treasury: Option<TreasuryAgent>,
    mint_trust: MintTrustService,
}

impl NeobankService {
    pub async fn new(identity: &UnifiedIdentity, config: &NeobankConfig) -> Result<Self>;
    pub async fn get_balance(&self, currency: Currency) -> Result<u64>;
    pub async fn send_payment(&self, bolt11: &str) -> Result<String>;
    pub async fn exchange(&self, from: Currency, to: Currency, amount: u64) -> Result<u64>;
    pub async fn get_treasury_status(&self) -> Result<TreasuryStatus>;
}
```

#### 1.3 Integrate into PylonProvider
**File:** `crates/pylon/src/provider.rs` (MODIFY)

```rust
pub struct PylonProvider {
    // ... existing fields
    neobank: Option<NeobankService>,  // ADD
}

impl PylonProvider {
    pub async fn init_neobank(&mut self) -> Result<()>;
    pub fn neobank(&self) -> Option<&NeobankService>;
}
```

#### 1.4 Add Daemon IPC commands for neobank
**File:** `crates/pylon/src/daemon/control.rs` (MODIFY)

```rust
pub enum DaemonCommand {
    // ... existing
    NeobankBalance { currency: String },
    NeobankPay { bolt11: String },
    NeobankExchange { from: String, to: String, amount: u64 },
    NeobankStatus,
}

pub enum DaemonResponse {
    // ... existing
    NeobankBalance { sats: u64 },
    NeobankPayment { payment_id: String },
    NeobankExchange { received: u64 },
    NeobankStatus { btc: u64, usd: u64 },
}
```

#### 1.5 Add CLI commands
**File:** `crates/pylon/src/cli/neobank.rs` (NEW ~150 LOC)

```bash
pylon neobank balance [--currency btc|usd]
pylon neobank pay <bolt11>
pylon neobank exchange <from> <to> <amount>
pylon neobank status
```

---

### Phase 2: Wire Pylon into Autopilot

#### 2.1 Extend pylon_integration.rs
**File:** `crates/autopilot/src/pylon_integration.rs` (MODIFY)

```rust
pub struct NeobankInfo {
    pub available: bool,
    pub btc_balance_sats: u64,
    pub usd_balance_cents: u64,
    pub treasury_active: bool,
}

pub async fn query_neobank_status() -> Option<NeobankInfo>;
```

#### 2.2 Extend preflight.rs
**File:** `crates/autopilot/src/preflight.rs` (MODIFY)

```rust
pub struct PreflightConfig {
    // ... existing
    pub neobank: Option<NeobankInfo>,  // ADD
}

// In detect_inference() or new detect_neobank():
pub async fn detect_neobank() -> Option<NeobankInfo>;
```

#### 2.3 Add neobank tools for Claude
**File:** `crates/autopilot/src/tools/neobank_tools.rs` (NEW ~200 LOC)

```rust
pub fn neobank_tool_definitions() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "neobank_check_balance",
            description: "Check wallet balance in BTC or USD",
            input_schema: json!({...}),
        },
        McpToolDefinition {
            name: "neobank_pay_lightning",
            description: "Pay a Lightning invoice",
            input_schema: json!({...}),
        },
        McpToolDefinition {
            name: "neobank_exchange",
            description: "Exchange between BTC and USD",
            input_schema: json!({...}),
        },
        McpToolDefinition {
            name: "neobank_treasury_status",
            description: "Get multi-currency treasury status",
            input_schema: json!({...}),
        },
    ]
}

pub async fn execute_neobank_tool(name: &str, params: Value) -> Result<Value>;
```

#### 2.4 Register tools in claude.rs
**File:** `crates/autopilot/src/claude.rs` (MODIFY)

Add neobank tools to the tool registry when in execution phase (BypassPermissions mode).

---

### Phase 3: Database & Persistence

#### 3.1 Add neobank tables to Pylon DB
**File:** `crates/pylon/src/db/mod.rs` (MODIFY)

```sql
-- Migration 004
CREATE TABLE neobank_wallets (
    id TEXT PRIMARY KEY,
    currency TEXT NOT NULL,
    mint_url TEXT NOT NULL,
    balance_sats INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE neobank_transactions (
    id TEXT PRIMARY KEY,
    wallet_id TEXT NOT NULL,
    type TEXT NOT NULL,  -- deposit, withdraw, exchange
    amount_sats INTEGER NOT NULL,
    counterparty TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (wallet_id) REFERENCES neobank_wallets(id)
);
```

---

### Phase 4: Tests

#### 4.1 Pylon neobank tests
**File:** `crates/pylon/tests/neobank_test.rs` (NEW)

- test_neobank_service_init
- test_balance_query
- test_daemon_neobank_commands
- test_cli_neobank_commands

#### 4.2 Autopilot integration tests
**File:** `crates/autopilot/tests/neobank_integration.rs` (NEW)

- test_preflight_detects_neobank
- test_claude_tools_available
- test_tool_execution

---

## Files to Modify/Create

### Pylon (5 files)
| File | Action | LOC |
|------|--------|-----|
| `Cargo.toml` | MODIFY | +2 |
| `src/neobank_service.rs` | NEW | ~200 |
| `src/provider.rs` | MODIFY | +50 |
| `src/daemon/control.rs` | MODIFY | +40 |
| `src/cli/neobank.rs` | NEW | ~150 |
| `src/db/mod.rs` | MODIFY | +30 |

### Autopilot (4 files)
| File | Action | LOC |
|------|--------|-----|
| `src/pylon_integration.rs` | MODIFY | +50 |
| `src/preflight.rs` | MODIFY | +30 |
| `src/tools/neobank_tools.rs` | NEW | ~200 |
| `src/claude.rs` | MODIFY | +20 |

### Tests (2 files)
| File | Action | LOC |
|------|--------|-----|
| `pylon/tests/neobank_test.rs` | NEW | ~100 |
| `autopilot/tests/neobank_integration.rs` | NEW | ~100 |

**Total: ~970 new LOC across 11 files**

---

## Implementation Order

1. **Commit neobank** - Push current implementation
2. **Pylon integration** - Add NeobankService, daemon commands, CLI
3. **Autopilot integration** - Preflight, tools, Claude registration
4. **Tests** - Both Pylon and Autopilot
5. **Documentation** - Update Pylon docs

---

## Success Criteria

1. `pylon neobank balance` returns wallet balance
2. `pylon neobank status` shows treasury state
3. Autopilot preflight shows neobank capabilities
4. Claude can call neobank_* tools in execution phase
5. All tests passing
