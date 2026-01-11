# Provider Mode

Provider mode enables Pylon to earn Bitcoin by processing compute jobs for other agents on the network.

## Overview

In provider mode, Pylon:
- Listens on Nostr relays for NIP-90 job requests
- Routes jobs to local inference backends
- Returns results and receives Lightning payments
- Persists job history and earnings

## NIP-90 Data Vending Machines

Pylon implements NIP-90 (Data Vending Machines), a protocol for requesting and providing compute services over Nostr.

### Job Request Flow

```
Customer                    Nostr Relays                    Provider (Pylon)
    │                            │                               │
    │── Job Request (kind:5xxx) ─▶│                               │
    │                            │◀── Subscribe to kind:5xxx ─────│
    │                            │                               │
    │                            │─── Job Request Event ─────────▶│
    │                            │                               │
    │                            │◀── Job Status (kind:7000) ─────│
    │◀─ Status: processing ──────│                               │
    │                            │                               │
    │                            │         (processing...)        │
    │                            │                               │
    │                            │◀── Job Result (kind:6xxx) ─────│
    │◀─ Result ──────────────────│                               │
    │                            │                               │
    │── Payment (Lightning) ─────────────────────────────────────▶│
    │                            │                               │
```

### Event Kinds

| Kind | Type | Description |
|------|------|-------------|
| 5000-5999 | Request | Job request events |
| 6000-6999 | Result | Job result events |
| 7000 | Status | Job status/feedback |

### Job Targeting

- If a job request includes a `#p` tag, Pylon only processes it when the pubkey matches the provider identity.
- Jobs without a `#p` tag are treated as broadcast and are eligible for processing.
- Pylon subscribes broadly to kind:5xxx on its configured relays to catch both cases.

### Supported Job Kinds

**Inference Jobs:**

| Kind | Description | Backend |
|------|-------------|---------|
| 5050 | Text generation | Ollama, llama.cpp, Apple FM |

**Bazaar Jobs (Agent Backends):**

| Kind | Description | Backend |
|------|-------------|---------|
| 5930 | SandboxRun | Codex Code |
| 5931 | RepoIndex | Codex Code |
| 5932 | PatchGen | Codex Code |
| 5933 | CodeReview | Codex Code |

## Inference Backends

Pylon auto-detects available inference backends:

### Ollama

Most common backend, supports many models.

```bash
# Install
brew install ollama  # macOS
curl -fsSL https://ollama.com/install.sh | sh  # Linux

# Start
ollama serve

# Pull models
ollama pull llama3.2
ollama pull mistral
```

**Detection**: Pylon checks `http://localhost:11434/api/tags`

### llama.cpp / GPT-OSS

Direct llama.cpp server or compatible API.

```bash
# Start server
./server -m model.gguf --host 0.0.0.0 --port 8080
```

**Detection**: Pylon checks `http://localhost:8080/health`

### Apple Foundation Models

macOS-only, uses Apple Silicon Neural Engine.

**Detection**: Pylon checks `http://localhost:11435/health` (requires separate server)

## Agent Backends (Bazaar Jobs)

Agent backends handle complex, multi-step tasks that require tool execution, repository access, and sandboxed environments.

### Codex Code

Primary agent backend for Bazaar jobs. Uses Codex with sandbox isolation.

**Detection**: Pylon checks for:
- `OPENAI_API_KEY` environment variable, OR
- `codex` CLI in PATH

**Capabilities:**
- PatchGen (kind 5932): Generate patches from issues/requirements
- CodeReview (kind 5933): Structured code review with issues
- SandboxRun (kind 5930): Execute code in isolated sandbox

**Configuration:**

```toml
# ~/.config/pylon/config.toml

[codex]
enabled = true
max_workers = 3
isolation = "container"  # local | container | gvisor
model_pattern = "codex-sonnet-4-*"
default_time_limit = 900

[codex.pricing]
patch_gen_base_msats = 10000
patch_gen_per_1k_tokens = 100
code_review_base_msats = 5000
```

**Isolation Modes:**
- `local`: Run Codex in current environment (development only)
- `container`: Run in Docker container (recommended)
- `gvisor`: Run in gVisor sandbox (most secure)

### Job Flow for Bazaar

```
1. BUYER publishes kind:5932 job request to Nostr
2. PYLON DvmService receives job
   → Validates kind supported
   → Routes to AgentRegistry
3. CodexCodeBackend executes:
   a. Clone repo (filtered paths)
   b. Start sandbox
   c. Run Codex with system prompt
   d. Extract result (patch/review)
   e. Verify: apply patch, run tests
4. DvmService creates Lightning invoice
5. DvmService publishes kind:6932 result
   → content = patch diff / review
   → tags = hashes, verification, invoice
6. BUYER verifies locally, pays if valid
7. PYLON records earnings
```

## Configuration

### Provider Identity

```bash
# Initialize creates identity
pylon init

# Identity stored at:
# ~/.config/pylon/identity.mnemonic
```

The provider identity is used for:
- Signing NIP-90 responses
- Receiving Lightning payments
- Appearing in the DVM marketplace

### Relay Configuration

```toml
# ~/.config/pylon/config.toml

relays = [
    "wss://nexus.openagents.com",
    "wss://relay.damus.io",
    "wss://nos.lol"
]
```

## Job Processing

### Job Lifecycle

```
                    ┌─────────────────────────────────────────────┐
                    │                                             │
                    ▼                                             │
┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐        │
│ Received │──▶│Processing│──▶│Completed │   │  Failed  │        │
└──────────┘   └──────────┘   └──────────┘   └──────────┘        │
                    │              │              │               │
                    │              │              │               │
                    │              ▼              ▼               │
                    │         ┌──────────┐  ┌──────────┐          │
                    └────────▶│ Payment  │  │  Error   │──────────┘
                              │ Received │  │ Response │
                              └──────────┘  └──────────┘
```

### Processing Steps

1. **Job Received**:
   - Parse job request event
   - Validate inputs
   - Check if backend available
   - Create job record in database

2. **Processing**:
   - Select appropriate backend
   - Forward request to backend
   - Stream response if supported

3. **Completed**:
   - Sign result event
   - Publish to relays
   - Update job status in database

4. **Payment**:
   - Receive Lightning payment
   - Record earning in database

### Error Handling

| Error | Response |
|-------|----------|
| Invalid request | kind:7000 with error status |
| Backend unavailable | kind:7000 with "no backend" |
| Timeout | kind:7000 with "timeout" |
| Processing error | kind:7000 with error details |

## Earnings Tracking

### Viewing Earnings

```bash
# Summary
pylon earnings

# JSON output
pylon earnings --json

# Last N earnings
pylon earnings --limit 20
```

### Example Output

```
Pylon Earnings
==============

Summary:
  Total earned: 1234 sats (1234000 msats)
  Jobs completed: 42

By Source:
  job: 1200 sats
  tip: 34 sats

Recent Earnings (last 10):
SATS     SOURCE     TIME AGO
50       job        2 hours
25       job        5 hours
100      job        1 days
...
```

### Database Queries

```bash
# Total earnings
sqlite3 ~/.openagents/pylon/pylon.db "SELECT SUM(amount_msats)/1000 FROM earnings"

# Earnings by day
sqlite3 ~/.openagents/pylon/pylon.db "
  SELECT date(earned_at, 'unixepoch') as day, SUM(amount_msats)/1000 as sats
  FROM earnings
  GROUP BY day
  ORDER BY day DESC
  LIMIT 7
"

# Job success rate
sqlite3 ~/.openagents/pylon/pylon.db "
  SELECT status, COUNT(*) as count
  FROM jobs
  GROUP BY status
"
```

## Domain Events

Provider mode emits events that can be monitored:

### Event Types

```rust
enum DomainEvent {
    // Job lifecycle
    JobReceived { job_id, kind, customer_pubkey, timestamp },
    JobStarted { job_id, model, timestamp },
    JobProgress { job_id, progress, timestamp },
    JobCompleted { job_id, amount_msats, duration_ms, timestamp },
    JobFailed { job_id, error, timestamp },

    // Payments
    PaymentReceived { job_id, amount_msats, timestamp },
    InvoiceCreated { job_id, bolt11, amount_msats, timestamp },

    // Network
    RelayConnected { url, timestamp },
    RelayDisconnected { url, reason, timestamp },

    // Backends
    BackendAvailable { backend_id, timestamp },
    BackendUnavailable { backend_id, timestamp },
    BackendsRegistered { backend_ids, timestamp },
    ModelsRefreshed { backend_id, models, timestamp },
}
```

### Event Handling in Daemon

The daemon subscribes to these events and:
- Logs them (`tracing::info!`)
- Persists jobs and earnings
- Updates running stats (for `pylon status`)

## Pricing

### Current Implementation

Jobs are priced based on:
- Token count (input + output)
- Model complexity
- Provider configuration

### Future: Dynamic Pricing

Planned features:
- Market-based pricing
- Reputation-based premiums
- Volume discounts

## Monitoring

### Status Check

```bash
pylon status

# Output:
# Daemon: Running (PID: 12345)
# Uptime: 2h 30m 15s
# Modes:  provider, host
#
# Session Stats:
#   Jobs completed: 42
#   Earnings: 1234 sats (1234000 msats)
#
# Inference Backends:
#   Available: ollama (default)
#
# Agent Backends:
#   Available: codex_code
#   Supported Bazaar Kinds: 5930, 5932, 5933
#
# Relays:
#   wss://nexus.openagents.com
```

### Diagnostics

```bash
pylon doctor

# Checks:
# - Identity configured
# - Backends available
# - Relay connectivity
# - Wallet status
```

### Logs

```bash
# Run in foreground with debug logging
RUST_LOG=debug pylon start -f

# Trace all DVM messages
RUST_LOG=pylon=debug,compute=trace pylon start -f
```

## Performance Tuning

### Concurrent Jobs

By default, jobs are processed serially. Future versions may support:
- Parallel job processing
- Job queuing with priorities
- Rate limiting per customer

### Backend Selection

When multiple backends are available:
1. Use configured default
2. Fall back to first available
3. Model-specific routing (not yet implemented)

### Relay Management

```toml
# config.toml
relays = [
    "wss://nexus.openagents.com", # Primary
    "wss://relay.damus.io",       # Backup
    "wss://nos.lol",              # Additional coverage
]

# More relays = more visibility but more bandwidth
# Fewer relays = less visibility but lower overhead
```

## Troubleshooting

### No Jobs Received

1. Check relay connectivity:
   ```bash
   pylon doctor
   ```

2. Verify identity is published:
   ```bash
   # Your provider should be discoverable
   ```

3. Check backend is available:
   ```bash
   pylon status
   # Should show "ollama" or other backend
   ```

### Jobs Failing

```bash
# Check job history
sqlite3 ~/.openagents/pylon/pylon.db "SELECT * FROM jobs WHERE status = 'failed' LIMIT 5"

# Check backend directly
curl http://localhost:11434/api/tags  # Ollama

# Check logs
RUST_LOG=debug pylon start -f
```

### No Earnings

Possible causes:
- Customer didn't pay (zap-optional jobs)
- Payment failed
- Wrong Lightning configuration

```bash
# Check earnings table
sqlite3 ~/.openagents/pylon/pylon.db "SELECT * FROM earnings ORDER BY earned_at DESC LIMIT 5"
```

## Security

### Identity Security

- Keep `identity.mnemonic` private
- Controls your provider identity
- Used for signing all responses

### Network Security

- Only connects to configured relays
- Validates all incoming events
- Rejects malformed requests

### Compute Security

- Jobs run through local backends
- No arbitrary code execution
- Sandboxed by backend (Ollama, etc.)
