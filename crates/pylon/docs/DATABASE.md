# Database Schema

Pylon uses SQLite for persistent storage. This document describes the complete database schema.

## Overview

The database is located at `~/.pylon/pylon.db` and uses WAL (Write-Ahead Logging) mode for better concurrency.

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
```

## Schema

### migrations

Tracks applied database migrations.

```sql
CREATE TABLE migrations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    applied_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Auto-incrementing primary key |
| `name` | TEXT | Migration identifier (e.g., "001_initial_schema") |
| `applied_at` | INTEGER | Unix timestamp when applied |

### jobs

Stores job records for provider mode.

```sql
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    kind INTEGER NOT NULL,
    customer_pubkey TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    price_msats INTEGER NOT NULL DEFAULT 0,
    input_hash TEXT,
    output_hash TEXT,
    error_message TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_jobs_status ON jobs(status);
CREATE INDEX idx_jobs_customer ON jobs(customer_pubkey);
CREATE INDEX idx_jobs_created ON jobs(created_at);
```

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT | Unique job identifier (usually event ID) |
| `kind` | INTEGER | NIP-90 job kind (e.g., 5100 for text generation) |
| `customer_pubkey` | TEXT | Requester's Nostr public key (hex) |
| `status` | TEXT | Job status (see below) |
| `price_msats` | INTEGER | Price paid in millisatoshis |
| `input_hash` | TEXT | SHA-256 hash of input (optional) |
| `output_hash` | TEXT | SHA-256 hash of output (optional) |
| `error_message` | TEXT | Error message if failed |
| `started_at` | INTEGER | Unix timestamp when job started |
| `completed_at` | INTEGER | Unix timestamp when job finished |
| `created_at` | INTEGER | Unix timestamp when record created |

**Job Statuses:**

| Status | Description |
|--------|-------------|
| `pending` | Job received, not yet started |
| `processing` | Job in progress |
| `completed` | Job finished successfully |
| `failed` | Job failed with error |
| `cancelled` | Job cancelled by requester or provider |

### earnings

Tracks all earnings from provider mode.

```sql
CREATE TABLE earnings (
    id TEXT PRIMARY KEY,
    job_id TEXT REFERENCES jobs(id),
    amount_msats INTEGER NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('job', 'tip', 'other')),
    payment_hash TEXT,
    preimage TEXT,
    earned_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_earnings_job ON earnings(job_id);
CREATE INDEX idx_earnings_date ON earnings(earned_at);
```

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT | Unique earning identifier |
| `job_id` | TEXT | Related job ID (nullable for tips) |
| `amount_msats` | INTEGER | Amount in millisatoshis |
| `source` | TEXT | Earning source type |
| `payment_hash` | TEXT | Lightning payment hash |
| `preimage` | TEXT | Lightning preimage (proof of payment) |
| `earned_at` | INTEGER | Unix timestamp when earned |

**Earning Sources:**

| Source | Description |
|--------|-------------|
| `job` | Payment for completed job |
| `tip` | Tip/donation (no associated job) |
| `other` | Other income source |

### agents

Stores agent state for host mode.

```sql
CREATE TABLE agents (
    npub TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL CHECK(lifecycle_state IN ('embryonic', 'active', 'dormant', 'terminated')),
    balance_sats INTEGER NOT NULL DEFAULT 0,
    tick_count INTEGER NOT NULL DEFAULT 0,
    last_tick_at INTEGER,
    memory_json TEXT,
    goals_json TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_agents_state ON agents(lifecycle_state);
```

| Column | Type | Description |
|--------|------|-------------|
| `npub` | TEXT | Agent's Nostr public key (bech32) |
| `name` | TEXT | Agent display name |
| `lifecycle_state` | TEXT | Current lifecycle state |
| `balance_sats` | INTEGER | Current wallet balance in satoshis |
| `tick_count` | INTEGER | Total ticks executed |
| `last_tick_at` | INTEGER | Unix timestamp of last tick |
| `memory_json` | TEXT | Agent memory (JSON blob) |
| `goals_json` | TEXT | Agent goals (JSON blob) |
| `created_at` | INTEGER | Unix timestamp when created |
| `updated_at` | INTEGER | Unix timestamp when last updated |

**Lifecycle States:**

| State | Description |
|-------|-------------|
| `embryonic` | Agent created, awaiting initial funding |
| `active` | Agent running normally |
| `dormant` | Agent paused due to zero balance |
| `terminated` | Agent permanently stopped |

### tick_history

Records tick execution history for agents.

```sql
CREATE TABLE tick_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_npub TEXT NOT NULL REFERENCES agents(npub),
    tick_number INTEGER NOT NULL,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    actions_json TEXT,
    cost_sats INTEGER,
    duration_ms INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_tick_history_agent ON tick_history(agent_npub);
CREATE INDEX idx_tick_history_date ON tick_history(created_at);
```

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Auto-incrementing primary key |
| `agent_npub` | TEXT | Agent's npub (foreign key) |
| `tick_number` | INTEGER | Sequential tick number |
| `prompt_tokens` | INTEGER | Tokens in prompt |
| `completion_tokens` | INTEGER | Tokens in completion |
| `actions_json` | TEXT | Actions taken (JSON array) |
| `cost_sats` | INTEGER | Cost of tick in satoshis |
| `duration_ms` | INTEGER | Tick duration in milliseconds |
| `created_at` | INTEGER | Unix timestamp when executed |

## API Reference

### Database Connection

```rust
use pylon::db::PylonDb;

// Open database (creates if doesn't exist)
let db = PylonDb::open("~/.pylon/pylon.db")?;

// Open in-memory database (for testing)
let db = PylonDb::open_in_memory()?;
```

### Jobs API

```rust
use pylon::db::jobs::{Job, JobStatus};

// Create a job
let job = Job {
    id: "event123".to_string(),
    kind: 5100,
    customer_pubkey: "abc123...".to_string(),
    status: JobStatus::Processing,
    price_msats: 0,
    input_hash: None,
    output_hash: None,
    error_message: None,
    started_at: now(),
    completed_at: None,
    created_at: now(),
};
db.create_job(&job)?;

// Get a job
let job = db.get_job("event123")?;

// Update status
db.update_job_status("event123", JobStatus::Completed)?;

// Complete with payment
db.complete_job("event123", Some("output_hash"), 5000)?;

// Mark as failed
db.fail_job("event123", "Inference error")?;

// List by status
let pending = db.list_jobs_by_status(JobStatus::Pending, 100)?;

// Count by status
let counts = db.count_jobs_by_status()?;
// Returns HashMap<JobStatus, u64>
```

### Earnings API

```rust
use pylon::db::earnings::{Earning, EarningSource};

// Record an earning
let earning = Earning {
    id: "earn-123".to_string(),
    job_id: Some("event123".to_string()),
    amount_msats: 5000,
    source: EarningSource::Job,
    payment_hash: Some("hash...".to_string()),
    preimage: None,
    earned_at: now(),
};
db.record_earning(&earning)?;

// Shorthand for job earnings
let id = db.record_job_earning("event123", 5000, Some("hash"), None)?;

// Get total earnings
let total_msats = db.get_total_earnings()?;

// Get earnings summary
let summary = db.get_earnings_summary()?;
// summary.total_msats, summary.total_sats, summary.job_count, summary.by_source

// Get today's earnings
let today_msats = db.get_today_earnings()?;

// Get recent earnings
let recent = db.get_recent_earnings(10)?;

// Get earnings in time range
let range = db.get_earnings_in_range(start_ts, end_ts)?;
```

### Agents API

```rust
use pylon::db::agents::{Agent, LifecycleState};

// Create or update agent
let agent = Agent {
    npub: "npub1...".to_string(),
    name: "MyAgent".to_string(),
    lifecycle_state: LifecycleState::Active,
    balance_sats: 1000,
    tick_count: 0,
    last_tick_at: None,
    memory_json: None,
    goals_json: None,
    created_at: now(),
    updated_at: now(),
};
db.upsert_agent(&agent)?;

// Get agent
let agent = db.get_agent("npub1...")?;

// List by state
let active = db.list_agents_by_state(LifecycleState::Active)?;

// List all
let all = db.list_all_agents()?;

// Update state
db.update_agent_state("npub1...", LifecycleState::Dormant)?;

// Update balance
db.update_agent_balance("npub1...", 500)?;

// Record a tick
db.record_tick(
    "npub1...",
    42,                           // tick_number
    Some(100),                    // prompt_tokens
    Some(50),                     // completion_tokens
    Some("[\"Post\"]"),           // actions_json
    Some(5),                      // cost_sats
    Some(1200),                   // duration_ms
)?;

// Get tick history
let ticks = db.get_tick_history("npub1...", 10)?;

// Delete agent (and tick history)
db.delete_agent("npub1...")?;
```

## Migrations

### Adding New Migrations

1. Define migration SQL in `src/db/mod.rs`:

```rust
const MIGRATION_002: &str = r#"
ALTER TABLE jobs ADD COLUMN priority INTEGER DEFAULT 0;
"#;
```

2. Call in migrate function:

```rust
fn migrate(&self) -> anyhow::Result<()> {
    // ... existing migrations ...
    self.run_migration("002_add_priority", MIGRATION_002)?;
    Ok(())
}
```

### Migration Safety

- Migrations are idempotent (safe to run multiple times)
- Each migration tracked in `migrations` table
- Failed migrations leave database in unknown state (manual recovery needed)

## Backup and Recovery

### Backup

```bash
# While daemon is running (WAL mode is safe for this)
cp ~/.pylon/pylon.db ~/.pylon/pylon.db.backup
cp ~/.pylon/pylon.db-wal ~/.pylon/pylon.db-wal.backup
cp ~/.pylon/pylon.db-shm ~/.pylon/pylon.db-shm.backup

# Or using sqlite3
sqlite3 ~/.pylon/pylon.db ".backup backup.db"
```

### Recovery

```bash
# Stop daemon first
pylon stop

# Restore
cp backup.db ~/.pylon/pylon.db

# Restart
pylon start
```

## Performance Tuning

### WAL Mode

The database uses WAL mode by default:
- Better concurrency (readers don't block writers)
- Better performance for typical workloads
- Creates additional files: `.db-wal` and `.db-shm`

### Indexes

Current indexes optimize for:
- Job lookup by status
- Job lookup by customer
- Earnings lookup by job
- Earnings lookup by date
- Agent lookup by state
- Tick history by agent and date

### Vacuuming

SQLite doesn't automatically reclaim space. For long-running instances:

```bash
# While daemon is stopped
sqlite3 ~/.pylon/pylon.db "VACUUM;"
```

## Querying the Database

### Direct SQLite Access

```bash
# Open database
sqlite3 ~/.pylon/pylon.db

# Example queries
.tables                          -- List tables
.schema jobs                     -- Show table schema
SELECT * FROM jobs LIMIT 10;     -- Query jobs
SELECT SUM(amount_msats) FROM earnings;  -- Total earnings
```

### Export Data

```bash
# Export as CSV
sqlite3 -header -csv ~/.pylon/pylon.db "SELECT * FROM earnings" > earnings.csv

# Export as JSON
sqlite3 ~/.pylon/pylon.db "SELECT json_group_array(json_object(
    'id', id,
    'amount_msats', amount_msats,
    'source', source
)) FROM earnings" > earnings.json
```
