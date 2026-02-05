# Protocol Surface

- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** This document is the canonical protocol reference
- **Doc owner:** protocol
- **If this doc conflicts with code, code wins.**

High-level protocol surface for OpenAgents. This document enumerates event kinds, job schemas, receipt formats, and trajectory specifications at a level suitable for implementers and auditors.

For canonical vocabulary, see [GLOSSARY.md](../GLOSSARY.md).

---

## Event Kinds

OpenAgents uses Nostr event kinds for job coordination and agent lifecycle.

### NIP-90 Job Events (Implemented)

> **Note:** Kind numbers shown are current defaults per the NIP-90 specification. Schema IDs (e.g., `oa.code_chunk_analysis.v1`) are the canonical identifiers for job types and will remain stable even if kind numbers change.

| Kind | Name | Purpose | Status |
|------|------|---------|--------|
| 5050 | Job Request | Buyer submits job with typed schema | 🟡 Partial |
| 6050 | Job Result | Provider returns result with provenance | 🟡 Partial |
| 7000 | Job Feedback | Invoice, status updates, completion signals | 🟡 Partial |

### NIP-SA Agent Lifecycle Events (Proposed)

| Kind | Name | Purpose | Status |
|------|------|---------|--------|
| TBD | AgentProfile | Agent capabilities, identity, reputation | ⚪ Planned |
| TBD | AgentState | Current agent status and availability | ⚪ Planned |
| TBD | TickRequest | Request for agent to process a tick | ⚪ Planned |
| TBD | TickResult | Result of agent tick processing | ⚪ Planned |
| TBD | TrajectorySession | Session metadata and summary | ⚪ Planned |
| TBD | TrajectoryEvent | Individual events within a session | ⚪ Planned |

### Handler Discovery (NIP-89)

| Kind | Name | Purpose | Status |
|------|------|---------|--------|
| 31990 | Handler Announcement | Provider advertises capabilities, pricing, capacity | 🟢 Implemented |

- **Note:** Kind numbers marked TBD are under design. See [GLOSSARY.md](../GLOSSARY.md) for status terminology.

---

## Local UI Bridge (Pylon)

The Pylon UI bridge is **local-only** and is used by browser-based UIs to
discover capabilities. It is not a Nostr protocol surface.

**Endpoint:**
- `wss://127.0.0.1:8081/app/{app_key}`
- Default `app_key`: `local-key`

**Channels:**
- `pylon.system`
- `pylon.codex`

**Events (system):**
- `pylon.capabilities` — Capability snapshot (JSON payload)
- `pylon.system.pong` — Ping response (JSON payload)
- `client-pylon.discover` — Request a capability refresh
- `client-pylon.ping` — Request a pong

**Events (codex):**
- `pylon.codex.event` — Codex app-server notifications (JSON-RPC envelope)
- `pylon.codex.response` — Response to client requests (`request_id`, `ok`, `result`/`error`)
- `pylon.codex.status` — Bridge/app-server status updates
- `pylon.codex.error` — Bridge/app-server error
- `client-codex.connect` — Register a workspace (requires `workspaceId` + `cwd`)
- `client-codex.disconnect` — Disconnect a workspace
- `client-codex.request` — JSON-RPC request envelope
- `client-codex.respond` — Approval response envelope

**Codex envelope example:**
```json
{
  "workspace_id": "ws_123",
  "message": {
    "method": "turn/started",
    "params": { "thread_id": "thread_1", "turn": { "id": "turn_1" } }
  }
}
```

The bridge uses the Pusher protocol so standard Echo/Pusher clients can connect.

---

## Job Schema Surface

Every job has a typed schema specifying inputs, outputs, and verification mode.

### Schema Structure

```json
{
  "schema_id": "oa.<domain>.<operation>.v<version>",
  "inputs": {
    "<field_name>": {
      "type": "<json_type>",
      "required": true,
      "description": "..."
    }
  },
  "outputs": {
    "<field_name>": {
      "type": "<json_type>",
      "description": "..."
    }
  },
  "verification_mode": "objective | subjective",
  "hash_rules": {
    "algorithm": "sha256",
    "canonicalization": "json_canonical",
    "scope": "full_output"
  },
  "provenance": {
    "model": "required",
    "provider_id": "required",
    "token_counts": "recommended",
    "sampling_params": "optional"
  }
}
```

### Current Job Types

| Schema ID | Verification | Purpose | Status |
|-----------|--------------|---------|--------|
| `oa.code_chunk_analysis.v1` | Subjective | Parallel file/chunk analysis, hypothesis generation | 🔵 Specified |
| `oa.retrieval_rerank.v1` | Subjective | LLM-based candidate reranking | 🔵 Specified |
| `oa.sandbox_run.v1` | Objective | Build/test/lint in isolated sandbox | 🔵 Specified |
| `oa.embedding.v1` | Objective | Text embedding generation | 🔵 Specified |

### Verification Modes

- **Objective**: Verification via exit code and artifact hashes. Payment releases only on correct output. Deterministic.
- **Subjective**: Requires judgment. Uses redundancy (run on N providers), adjudication, or judge models. Pay for consensus.

---

## Receipt Schema

Receipts bind payments to execution state and policy decisions.

### Minimum Receipt Fields

```json
{
  "receipt_id": "string (unique)",
  "session_id": "string",
  "trajectory_hash": "sha256:...",
  "policy_bundle_id": "string",
  "job_hash": "sha256:...",
  "payment_proof": {
    "type": "lightning_preimage | cashu_proof | onchain_txid",
    "value": "..."
  },
  "rail": "lightning | cashu | onchain | taproot_assets",
  "asset_id": "BTC_LN | BTC_CASHU(<mint>) | USD_CASHU(<mint>) | ...",
  "amount_msats": "number",
  "provider_id": "npub...",
  "approval_rule_id": "string | null",
  "timestamp": "ISO 8601"
}
```

### Receipt Lifecycle

1. Job submitted → Quote created with idempotency key
2. Quote approved → Payment initiated
3. Payment confirmed → Job executed
4. Job verified → Receipt emitted
5. Receipt links: payment → job → trajectory → policy

---

## Trajectory Format

Trajectories provide full execution audit trails.

### Current: ReplayBundle

Current implementation in `crates/autopilot-core/src/replay.rs`.

```rust
pub struct ReplayBundle {
    pub session_id: String,
    pub steps: Vec<ReplayStep>,
    pub metadata: ReplayMetadata,
}
```

### Target: REPLAY.jsonl v1

Interoperable format specified in `crates/dsrs/docs/REPLAY.md`.

```jsonl
{"event":"ReplayHeader","replay_version":1,"producer":"adjutant@1.2.3","created_at":"..."}
{"event":"SessionStart","session_id":"...","issue_number":42,"policy_bundle_id":"...","t":"..."}
{"event":"PlanStart","plan_hash":"sha256:...","step_count":3,"t":"..."}
{"event":"ToolCall","id":"tc_001","tool":"file_read","params":{...},"params_hash":"sha256:...","step_id":"step-1","t":"..."}
{"event":"ToolResult","id":"tc_001","output_hash":"sha256:...","step_utility":0.8,"latency_ms":45,"t":"..."}
{"event":"StepComplete","step_id":"step-1","status":"Success","iterations":1,"t":"..."}
{"event":"Verification","commands":["cargo check","cargo test"],"exit_codes":[0,0],"verification_delta":3,"t":"..."}
{"event":"SessionEnd","status":"Success","confidence":0.92,"total_tool_calls":15,"total_latency_ms":12500,"t":"..."}
```

### Compatibility Plan

**MVP acceptance:** Either:
- Native emission of REPLAY.jsonl v1, OR
- Emission of ReplayBundle + working exporter to REPLAY.jsonl v1

---

## Hashing Rules

All hashes use deterministic canonicalization.

### Algorithm

- **Hash function**: SHA-256
- **Canonicalization**: JSON Canonical form (sorted keys, no whitespace)
- **Scope**: Full output, never truncated

### Canonical JSON Serialization

```rust
fn canonical_serialize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by_key(|(k, _)| *k);
            let pairs: Vec<String> = sorted
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, canonical_serialize(v)))
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_serialize).collect();
            format!("[{}]", items.join(","))
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}

pub fn canonical_hash(value: &Value) -> String {
    let canonical = canonical_serialize(value);
    format!("sha256:{}", sha256::digest(canonical.as_bytes()))
}
```

### What Gets Hashed

| Artifact | Hash Source | Notes |
|----------|-------------|-------|
| Plan | Canonical JSON of PlanIR | Before execution |
| Tool params | Canonical JSON of params object | Per tool call |
| Tool output | Full output (never truncated) | Even if display is truncated |
| Trajectory | Canonical JSON of full session | At session end |
| Policy bundle | Canonical JSON of bundle manifest | At compilation |

---

## Provider Announcements

Providers register via NIP-89 kind 31990 events.

### Announcement Content

```json
{
  "supply_class": "SingleNode | BundleLAN | BundleRack | InstanceMarket | ReservePool",
  "capabilities": {
    "supported_jobs": ["oa.code_chunk_analysis.v1", "oa.sandbox_run.v1"],
    "models": ["gpt-4", "codex-3-sonnet"],
    "hardware": {
      "memory_gb": 64,
      "gpu_type": "M4 Max",
      "vram_gb": 128
    }
  },
  "pricing": {
    "input_tokens_per_1k_msats": 10,
    "output_tokens_per_1k_msats": 30,
    "sandbox_per_minute_msats": 100
  },
  "capacity": {
    "max_concurrent_jobs": 4,
    "max_context_tokens": 128000
  },
  "stability": {
    "success_rate": 0.95,
    "avg_latency_ms": 250,
    "jobs_completed": 1523
  }
}
```

---

## Cross-References

- [GLOSSARY.md](../GLOSSARY.md) - Canonical terminology
- [crates/dsrs/docs/REPLAY.md](../crates/dsrs/docs/REPLAY.md) - REPLAY.jsonl specification
- [crates/dsrs/docs/ARTIFACTS.md](../crates/dsrs/docs/ARTIFACTS.md) - Verified Patch Bundle schemas
- [PAPER.md](../PAPER.md) Appendix G - Protocol surface summary
- [crates/protocol/](../crates/protocol/) - Protocol implementation
