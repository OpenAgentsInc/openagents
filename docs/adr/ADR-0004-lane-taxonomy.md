# ADR-0004: Lane Taxonomy and Naming

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents routes inference requests through different "lanes" based on cost, latency, privacy, and capability requirements. This ADR establishes the canonical lane taxonomy.

## Decision

**Lane classes are: `Local`, `Cloud`, `Swarm`**

### Lane Definitions

| Lane | Description | Examples |
|------|-------------|----------|
| `Local` | On-device inference | FM Bridge, Ollama, llama.cpp, MLX |
| `Cloud` | Remote API providers | Anthropic, OpenAI, Cerebras, Crusoe |
| `Swarm` | Distributed NIP-90 network | Pylon providers, NIP-90 DVMs |

### Routing Semantics

| Lane | Latency | Cost | Privacy | Trust |
|------|---------|------|---------|-------|
| `Local` | Lowest | Free | Maximum | Self |
| `Cloud` | Medium | Per-token | Depends on provider | Provider |
| `Swarm` | Variable | Sats/job | Encrypted payloads | Verification |

### When to Use Each Lane

- **Local**: Quick operations, privacy-sensitive, offline capable
- **Cloud**: High capability models, guaranteed availability
- **Swarm**: Cost optimization, decentralization, earning sats

### Deprecations

| Deprecated | Replacement | Notes |
|------------|-------------|-------|
| `Datacenter` | `Cloud` | Internal enum may still use `Datacenter` |
| `Remote` | `Cloud` | Too generic |
| `External` | `Cloud` or `Swarm` | Ambiguous |

### Code Alignment

The `Venue` enum in `crates/frlm/src/types.rs` should align:

```rust
pub enum Venue {
    Local,   // On-device
    Cloud,   // Remote API (may internally be "Datacenter")
    Swarm,   // NIP-90 distributed
    Codex,   // Codex app-server
    Unknown, // Fallback
}
```

Note: `Codex` is a special case that routes through the Codex app-server.

## Consequences

**Positive:**
- Consistent terminology across codebase
- Clear mental model for routing decisions
- Simpler documentation

**Negative:**
- Requires updating existing code that uses `Datacenter`
- Brief confusion during transition

**Neutral:**
- Internal enums may retain old names with mapping layer

## Alternatives Considered

1. **Keep Datacenter** — Technical but confusing alongside "Cloud" in user-facing docs.

2. **Use numbers/tiers** — Less intuitive, harder to remember.

3. **Provider-specific names** — Too granular, doesn't group by characteristics.

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `Lane` definition
- [crates/frlm/docs/README.md](../../crates/frlm/docs/README.md) — Execution venues
- [docs/PROTOCOL_SURFACE.md](../PROTOCOL_SURFACE.md) — Protocol routing
