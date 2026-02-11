# HUD Integration

Runtime architecture for the Autopilot HUD. The HUD is a **pure viewer** over the agent filesystem—no bespoke UI backend logic per feature.

---

## Core Principle

```
watch files → render panes
```

The HUD renders whatever the filesystem emits. This makes "live fishbowl," "personal HUD," "timelapse," "APM gauge," and "proof it's real" all natural outputs of the runtime—not marketing boltons.

---

## Filesystem Layout

```
/agents/<id>/
├── status              # Current state (read)
├── logs/
│   ├── trace           # Live event stream (watch)
│   └── trajectory      # Append-only audit trail (read, replay)
├── hud/
│   ├── stream          # Redacted event stream for public viewers (watch)
│   └── settings        # Public/private toggle, embed_allowed (read/write)
├── metrics/
│   ├── apm             # Actions per minute (read)
│   ├── queue           # Issue queue depth (read)
│   └── last_pr         # Most recent PR info (read)
├── goals/              # Active goals summary (read)
├── identity/
│   └── pubkey          # Agent public key for verification (read)
└── deadletter/         # Failed envelopes for credibility (read)
```

---

## GTM Pillar Mappings

### 1. Live Fishbowl = Public Agent Filesystem

**What you need:** A session that's undeniably real *and* safely watchable.

| Runtime Path | HUD Feature |
|--------------|-------------|
| `/agents/<id>/status` | "Live: Autopilot is working on issue #847" |
| `/agents/<id>/logs/trace` (watch) | Panes lighting up, tools firing, code streaming |
| `/agents/<id>/logs/trajectory` | Audit trail / replay source |
| `/agents/<id>/deadletter` | Credibility: "we don't drop events; inspect failures" |
| `/agents/<id>/identity/pubkey` | Verify stream is from the Autopilot agent pubkey |

**Why this works:** The HUD literally just `watch()`es the filesystem. No "demo mode." It's the real session.

---

### 2. One-Click to Your Own HUD = Control Plane + Mounts

**Runtime mapping:**

`POST /agents` creates an agent with standard mounts:
- `/repo` mount (GitHub repo)
- `/compute` mount (LLM routing)
- `/containers` mount (safe build/test)
- `/nostr` mount (optional, for sharing/messaging)
- budgets/policy set at mount-level
- initial goals: "watch issues, pick next, implement, test, PR"

**Create agent response must include:**

```json
{
  "agent_id": "agent_abc123",
  "hud_stream_url": "wss://openagents.com/agents/abc123/hud/stream",
  "public_url": "https://openagents.com/repo/@username/repo",
  "status": "creating"
}
```

The frontend routes to `openagents.com/repo/@user/repo` which connects to `watch(/agents/<id>/hud/stream)`.

**Why it enables <30s setup:**
- Non-blocking job APIs (`/compute/new`, `/containers/new`)
- Consistent streaming via `watch()`
- Hibernation semantics (DO / local) so idle Autopilots are cheap

---

### 3. Shareable Personal HUDs = ACL at Mount Boundary

**Viral loop requires:**
- Public by default (opt-out)
- Embeddable
- Live-updating

**Runtime mapping:**

The *only* thing "public HUD" needs is **read-only access** to a narrow set of paths:

| Path | Access | Description |
|------|--------|-------------|
| `/status` | read | Current agent state |
| `/hud/stream` | watch | Redacted event stream |
| `/goals` | read | Active goals summary |
| `/metrics/*` | read | APM, queue, last_pr |

Because mounts are the security boundary, implement "public HUD" as:
- A read-only "public view" namespace (or token-based access to specific paths)
- With strict redaction

**ACL Modes:**

| Mode | Access |
|------|--------|
| Public | Can watch `/hud/stream`, read `/status`, `/goals`, `/metrics/*` |
| Owner | Can watch everything, including `/logs/trace` |
| Private | Only owner, no public access |

**Settings file (`/hud/settings`, read-only to agents; control plane updates):**

```json
{
  "public": true,
  "embed_allowed": true,
  "redaction_policy": "standard"
}
```

---

### 4. Demo Is The Product = Event Grammar

The HUD renders a deterministic projection of events. The runtime already enforces:
- Mandatory trajectories
- Job IDs + watchable streams for compute/containers
- Consistent causality via `envelope_id`, `tick_id`

**HUD Event Contract (Canonical Schema):**

| Event Type | Fields | Description |
|------------|--------|-------------|
| `session_start` | session_id, agent_id, started_at | Session begins |
| `session_end` | session_id, duration_ms, success | Session ends |
| `tick_start` | tick_id, envelope_id, cause, timestamp | Tick begins |
| `tick_end` | tick_id, duration_ms, success | Tick ends |
| `tool_start` | tick_id, tool_name, params | Tool execution starts |
| `tool_done` | tick_id, tool_name, result, duration_ms, success | Tool execution ends |
| `chunk` | tick_id, text, token_count | LLM streaming token |
| `file_diff` | tick_id, path, hunks, additions, deletions | Code change |
| `container_output` | session_id, exec_id, stream, data | Sandbox stdout/stderr |
| `usage` | tick_id, input_tokens, output_tokens, cost_usd | Billing update |
| `state_change` | field, operation, value | Agent state mutation |
| `error` | tick_id, code, message, recoverable | Error occurred |

**Unified contract:** Both the existing AutopilotContainer WebSocket event types AND the runtime `/logs/trace` emit this same schema. The HUD targets one stable contract.

---

### 5. APM Gauge + Queue Drain = Derived Metrics

No separate analytics system needed. Metrics are derived views over logs/state.

**Metrics files:**

| Path | Content | Source |
|------|---------|--------|
| `/metrics/apm` | `{"value": 19.2, "window_secs": 60}` | Count of action events per time window from trace |
| `/metrics/queue` | `{"depth": 5, "oldest_issue": "..."}` | From `/goals` or repo driver |
| `/metrics/last_pr` | `{"url": "...", "title": "...", "merged": false}` | From repo driver |

These make the HUD and screenshots instantly legible:
- APM gauge showing 19+ actions/minute
- Issue queue visibly shrinking

---

### 6. Overnight Timelapse = Trajectory Replay

The architecture requires trajectory capture. Replay is a natural output.

**Runtime mapping:**

- Persist append-only NDJSON per session (or per tick)
- Replay reads:
  - `/logs/trajectory?from=...&to=...`
  - `/logs/trajectory/replay?speed=20x`
- HUD runs in "replay mode" to produce timelapse *without special-casing the product*

**Event requirements for deterministic replay:**

| Field | Required | Purpose |
|-------|----------|---------|
| `timestamp` | Yes | Ordering |
| `session_id` | Yes | Grouping |
| `tick_id` | Yes | Causal grouping |
| `event_type` | Yes | Render decision |
| `payload` | Yes | Content |

Timelapse becomes *just* a client feature—8 hours compressed to 30 seconds.

---

### 7. Undeniably Real = Verifiable Links + Idempotency

The fishbowl only works if skeptics can't dismiss it as staged animation.

**Runtime mapping:**

- Idempotency journals prevent double-billing and duplicate external effects
- Event stream includes verifiable links:
  - GitHub issue/PR URLs
  - Commit SHAs
  - Container session IDs
  - Cost/usage (micro-USD)

**Optional verification (for high-trust fishbowl):**

- Sign session header and/or periodic checkpoints with agent identity
- Include signature in `/status` or `/hud/stream`
- Viewers can verify against `/identity/pubkey`

```json
{
  "session_id": "sess_abc123",
  "checkpoint": 42,
  "signature": "schnorr_sig_hex",
  "pubkey": "npub1..."
}
```

This creates "verifiable demo" as a product primitive.

---

### 8. Portability = Consistent Creator Demos

Creators use different setups. The runtime enables:

| Backend | Use Case |
|---------|----------|
| Local (Docker + local models) | Privacy, dev, comfort |
| Cloud (Cloudflare DO + containers) | Shareable public HUD URLs |
| Browser (WASM) | Zero-install demos |

Same UI surface, same semantics. Creator demos look consistent—critical for "product is the demo" strategy.

---

## Redaction for Public HUDs

Public streams must redact sensitive data while preserving "the motion."

**Redact:**
- Secrets (API keys, tokens, passwords)
- Environment variables
- Private repo paths/content (by policy)
- Credentials in tool params/results

**Keep:**
- Tool start/done events (names, timing)
- Diffs (with path sanitization if needed)
- Test output (pass/fail, durations)
- Container output (sanitized)
- Usage/cost data

**Implementation:**

`/agents/<id>/hud/stream` is a sanitized projection of `/logs/trace`:

```rust
pub struct RedactionPolicy {
    /// Patterns to redact from all string fields
    pub secret_patterns: Vec<Regex>,
    /// Paths to exclude entirely
    pub excluded_paths: Vec<String>,
    /// Fields to always redact
    pub redacted_fields: Vec<String>,
}

impl HudStreamFs {
    fn sanitize_event(&self, event: TraceEvent, policy: &RedactionPolicy) -> TraceEvent {
        // Apply redaction rules
        // Replace matched patterns with "[REDACTED]"
        // Preserve event structure and timing
    }
}
```

---

## Implementation per Backend

| Feature | Local | Cloudflare | Browser |
|---------|-------|------------|---------|
| `/logs/trace` | File watch + broadcast | DO WebSocket | BroadcastChannel |
| `/hud/stream` | Redaction filter | Redaction filter | Redaction filter |
| `/metrics/*` | Computed on read | Computed on read | Computed on read |
| Trajectory storage | SQLite | DO storage | IndexedDB |
| Signed checkpoints | Local keychain | DO + KMS | WebCrypto |

---

## Control Plane Additions

### Create Agent Response

```http
POST /agents
Content-Type: application/json

{
  "name": "my-autopilot",
  "repo": "https://github.com/user/repo",
  "config": { ... }
}
```

Response must include HUD URLs:

```json
{
  "id": "agent_abc123",
  "name": "my-autopilot",
  "pubkey": "npub1...",
  "status": "creating",

  "hud": {
    "stream_url": "wss://openagents.com/agents/abc123/hud/stream",
    "public_url": "https://openagents.com/repo/@username/repo",
    "embed_url": "https://openagents.com/repo/@username/repo/embed"
  }
}
```

### HUD Settings Endpoint

```http
GET /agents/{id}/hud/settings
PUT /agents/{id}/hud/settings  (owner/admin only)

{
  "public": true,
  "embed_allowed": true,
  "redaction_policy": "standard"
}
```

---

## The One-Liner

**The runtime turns Autopilot into a shareable "live filesystem" whose UI is just a projection of traces, jobs, and budgets.**

This makes "live fishbowl," "personal HUD," "timelapse," "APM gauge," and "proof it's real" all *natural outputs* of the core architecture—not marketing boltons.

---

## References

- [FILESYSTEM.md](FILESYSTEM.md) — FileService trait, watch semantics
- [CONTROL-PLANE.md](CONTROL-PLANE.md) — HTTP API, SSE streaming
- [DESIGN.md](DESIGN.md) — Tick model, trajectory logging
- [DRIVERS.md](DRIVERS.md) — Event sources (HTTP, WebSocket, Nostr)
- [../../docs/SYNTHESIS.md](../../docs/SYNTHESIS.md) — Product context (GTM notes archived)
