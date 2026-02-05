# ADR-0004: Lane Taxonomy and Naming

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents routes inference requests through different "lanes" based on cost, latency, privacy, and capability requirements. We need a canonical lane taxonomy that all components use consistently.

## Decision

**Lane classes are: `Local`, `Cloud`, `Swarm`**

### Lane Definitions

| Lane | Description | Examples |
|------|-------------|----------|
| `Local` | On-device inference | FM Bridge, Ollama, llama.cpp, MLX |
| `Cloud` | Remote API providers | Anthropic, OpenAI, Cerebras, Crusoe, Codex |
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

### Venue vs Lane

The `Venue` enum in code may include more specific provider identifiers. The mapping:

| Venue (code) | Lane (taxonomy) | Notes |
|--------------|-----------------|-------|
| `Local` | Local | On-device |
| `Datacenter` | Cloud | Legacy name, use Cloud in docs |
| `Cloud` | Cloud | Preferred |
| `Codex` | Cloud | Codex app-server routes through Cloud |
| `Swarm` | Swarm | NIP-90 distributed |
| `Unknown` | — | Fallback |

**Key clarification:** `Codex` is a **provider** (like Anthropic or OpenAI), not a separate lane class. For lane semantics, Codex routes as Cloud.

## Scope

What this ADR covers:
- Canonical lane class names
- Lane semantics (cost, latency, privacy, trust)
- Mapping from code enums to taxonomy

What this ADR does NOT cover:
- Routing algorithm implementation
- Provider-specific configuration
- Budget allocation logic

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Lane names | Stable: `Local`, `Cloud`, `Swarm` |
| Lane count | Stable: exactly 3 lane classes |

Backward compatibility:
- Internal code may use `Datacenter` enum variant; map to `Cloud` in docs/UI
- New providers may be added without changing lane taxonomy

### Deprecations

| Deprecated | Replacement | Notes |
|------------|-------------|-------|
| `Datacenter` | `Cloud` | Internal enum may retain; use Cloud in docs |
| `Remote` | `Cloud` | Too generic |
| `External` | `Cloud` or `Swarm` | Ambiguous |

## Consequences

**Positive:**
- Consistent terminology across codebase
- Clear mental model for routing decisions
- Simpler documentation

**Negative:**
- Internal code migration to prefer `Cloud` over `Datacenter`

**Neutral:**
- Internal enums may retain old names with mapping layer

## Alternatives Considered

1. **Keep Datacenter** — Technical but confusing alongside "Cloud" in user-facing docs.

2. **Use numbers/tiers** — Less intuitive, harder to remember.

3. **Provider-specific names** — Too granular, doesn't group by characteristics.

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `Lane` definition
- [crates/frlm/docs/README.md](../../crates/frlm/docs/README.md) — Execution venues
- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — Protocol routing
