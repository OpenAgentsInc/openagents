# Replay Serialization

> **Status:** Spec (implementation differs)
> **Last verified:** 634f5b627
> **Source of truth:**
> - Spec: This document defines the target format
> - Current impl: `crates/autopilot-core/src/replay.rs` (uses `ReplayBundle` format)
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

Canonical JSONL event format for REPLAY.jsonl files.

## Overview

REPLAY.jsonl files capture a complete event stream of a session for:
- CLI replay viewer (`adjutant replay sess_abc123`)
- Counterfactual analysis
- Shadow mode comparison
- Training data generation
- Debugging and audit

---

## Event Types

```rust
// File: crates/dsrs/src/adapter/replay.rs (SPEC - not yet implemented)

#[derive(Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum ReplayEvent {
    /// Format version header - MUST be first line
    ReplayHeader {
        replay_version: u8,
        producer: String,
        created_at: DateTime<Utc>,
    },

    /// Session started
    SessionStart {
        t: DateTime<Utc>,
        session_id: String,
        issue_number: Option<i64>,
        policy_version: String,
    },

    /// Plan generated
    PlanStart {
        t: DateTime<Utc>,
        plan_hash: String,
        step_count: usize,
    },

    /// Tool call initiated
    ToolCall {
        t: DateTime<Utc>,
        id: String,
        tool: String,
        params: Value,
        params_hash: String,
        step_id: String,
    },

    /// Tool result received
    ToolResult {
        t: DateTime<Utc>,
        id: String,
        output_hash: String,
        exit_code: Option<i32>,
        step_utility: f32,  // -1.0..+1.0 (THE LEARNING SIGNAL)
        latency_ms: u64,
    },

    /// Step completed
    StepComplete {
        t: DateTime<Utc>,
        step_id: String,
        status: StepStatus,
        iterations: u8,
    },

    /// Verification run
    Verification {
        t: DateTime<Utc>,
        commands: Vec<String>,
        exit_codes: Vec<i32>,
        verification_delta: i32,
    },

    /// Session ended
    SessionEnd {
        t: DateTime<Utc>,
        status: SessionStatus,
        confidence: f32,
        total_tool_calls: usize,
        total_latency_ms: u64,
    },
}

#[derive(Serialize, Deserialize)]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
    MaxIterationsReached,
}

#[derive(Serialize, Deserialize)]
pub enum SessionStatus {
    Success,
    Failed,
    Cancelled,
    Timeout,
}
```

---

## Field Requirements

| Field | Required | Description |
|-------|----------|-------------|
| `t` | Yes | ISO 8601 timestamp |
| `event` | Yes | Event type discriminator |
| `session_id` | SessionStart only | Unique session identifier |
| `id` | ToolCall/ToolResult | Correlation ID for tool calls |
| `step_id` | ToolCall/StepComplete | Links to PlanIR step |
| `params_hash` | ToolCall | SHA256 of canonical params |
| `output_hash` | ToolResult | SHA256 of canonical output |

---

## Canonical Serialization

For deterministic hashing and reproducibility:

```rust
impl ReplayEvent {
    /// Serialize to canonical JSONL line.
    /// - Keys sorted alphabetically
    /// - No extra whitespace
    /// - Deterministic output
    pub fn to_canonical_json(&self) -> String {
        let value = serde_json::to_value(self).unwrap();
        canonical_serialize(&value)
    }
}

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
```

### Hashing Rules

```rust
pub fn canonical_hash(value: &Value) -> String {
    let canonical = canonical_serialize(value);
    format!("sha256:{}", sha256::digest(canonical.as_bytes()))
}
```

---

## Truncation Rules

**Important:** Hashes are ALWAYS computed from full output, never truncated previews. Truncation is only for display/storage of preview fields.

| Field | Max Size | Truncation Method |
|-------|----------|-------------------|
| `output_preview` | 2000 chars | First 1000 + "..." + Last 1000 |
| `params` | No limit | Full params always stored |
| `stdout/stderr` | 10000 chars | First 5000 + "..." + Last 5000 |

---

## Versioning

Use a `ReplayHeader` event as the first line to specify format version (avoids per-line bloat):

```rust
/// Header event - MUST be first line of REPLAY.jsonl
ReplayHeader {
    replay_version: u8,      // Currently: 1
    producer: String,        // e.g., "adjutant@1.2.3"
    created_at: DateTime<Utc>,
}
```

**Compatibility note:** Fields can be added but existing fields must preserve backward parsing. Consumers should ignore unknown events/fields.

---

## Example REPLAY.jsonl

```jsonl
{"event":"ReplayHeader","created_at":"2026-01-13T10:00:00Z","producer":"adjutant@1.2.3","replay_version":1}
{"event":"SessionStart","issue_number":42,"policy_version":"v1.2.3","session_id":"sess_abc123","t":"2026-01-13T10:00:00Z"}
{"event":"PlanStart","plan_hash":"sha256:abc123...","step_count":3,"t":"2026-01-13T10:00:01Z"}
{"event":"ToolCall","id":"tc_001","params":{"path":"src/auth.rs"},"params_hash":"sha256:def456...","step_id":"step-1","t":"2026-01-13T10:00:02Z","tool":"file_read"}
{"event":"ToolResult","exit_code":null,"id":"tc_001","latency_ms":45,"output_hash":"sha256:ghi789...","step_utility":0.8,"t":"2026-01-13T10:00:02Z"}
{"event":"StepComplete","iterations":1,"status":"Success","step_id":"step-1","t":"2026-01-13T10:00:03Z"}
{"event":"Verification","commands":["cargo check","cargo test"],"exit_codes":[0,0],"t":"2026-01-13T10:05:30Z","verification_delta":3}
{"event":"SessionEnd","confidence":0.92,"status":"Success","t":"2026-01-13T10:05:32Z","total_latency_ms":5320,"total_tool_calls":5}
```

---

## Replay Reader

```rust
// File: crates/dsrs/src/adapter/replay.rs

pub struct ReplayReader {
    events: Vec<ReplayEvent>,
}

impl ReplayReader {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let events: Vec<ReplayEvent> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();
        Ok(Self { events })
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ReplayEvent> {
        self.events.iter().filter(|e| matches!(e, ReplayEvent::ToolCall { .. }))
    }

    pub fn total_latency_ms(&self) -> u64 {
        self.events.iter()
            .filter_map(|e| match e {
                ReplayEvent::ToolResult { latency_ms, .. } => Some(latency_ms),
                _ => None,
            })
            .sum()
    }

    pub fn average_step_utility(&self) -> f32 {
        let utilities: Vec<f32> = self.events.iter()
            .filter_map(|e| match e {
                ReplayEvent::ToolResult { step_utility, .. } => Some(*step_utility),
                _ => None,
            })
            .collect();
        if utilities.is_empty() { 0.0 } else {
            utilities.iter().sum::<f32>() / utilities.len() as f32
        }
    }
}
```

---

## Replay Writer

```rust
pub struct ReplayWriter {
    file: BufWriter<File>,
}

impl ReplayWriter {
    pub fn new(path: &Path) -> Result<Self> {
        let file = BufWriter::new(File::create(path)?);
        Ok(Self { file })
    }

    pub fn emit(&mut self, event: &ReplayEvent) -> Result<()> {
        let line = event.to_canonical_json();
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        Ok(())
    }
}
```

---

## Replay Verification

Verify a replay matches the original RECEIPT.json:

```rust
pub fn verify_replay(
    replay_events: &[ReplayEvent],
    original_receipt: &SessionReceipt,
) -> VerificationResult {
    let mut mismatches = Vec::new();

    for (event, receipt) in replay_events.iter()
        .filter(|e| matches!(e, ReplayEvent::ToolResult { .. }))
        .zip(&original_receipt.tool_calls)
    {
        if let ReplayEvent::ToolResult { output_hash, .. } = event {
            if output_hash != &receipt.output_hash {
                mismatches.push(format!(
                    "Tool {} output mismatch: expected {}, got {}",
                    receipt.tool, receipt.output_hash, output_hash
                ));
            }
        }
    }

    VerificationResult {
        success: mismatches.is_empty(),
        mismatches,
    }
}
```

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **ARTIFACTS.md** | REPLAY.jsonl is one of three MVP artifacts |
| **MODULES.md** | Execution flow emits replay events |
| **TOOLS.md** | Tool receipts feed into replay events |
| **METRICS.md** | step_utility in ToolResult enables outcome-coupled scoring |
