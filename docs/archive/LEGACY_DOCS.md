# Legacy Documentation Mapping

This document maps legacy/archived documentation to their canonical replacements.

## Canonical Sources of Truth

| Concern | Source |
|---------|--------|
| Terminology | [GLOSSARY.md](../GLOSSARY.md) |
| Architecture decisions | [docs/adr/](../adr/) (ADRs for contracts, invariants, interfaces) |
| Agent architecture | [AGENTS.md](../../AGENTS.md) |
| Execution flow | [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) |
| Protocol/Transport | [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) |
| Roadmap | [ROADMAP.md](../ROADMAP.md) |
| DSPy modules | [crates/dsrs/docs/](../crates/dsrs/docs/) |

## Authority Hierarchy

See [ADR-0001](./adr/ADR-0001-adoption-of-adrs.md) for the full specification.

| Priority | Concern | Authoritative Source |
|----------|---------|---------------------|
| 1 | Behavior | Code wins |
| 2 | Terminology | GLOSSARY.md wins |
| 3 | Architecture intent | ADRs win |
| 4 | Implementation status | Crate sources + SYNTHESIS_EXECUTION.md |
| 5 | Priorities/sequencing | ROADMAP.md |

## Archived Documents

| Archived File | Replaced By | Notes |
|---------------|-------------|-------|
| `.openagents/TODO.md` | [ROADMAP.md](../ROADMAP.md), [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) | Planning docs archived to `.openagents/archive/2025-12/` |
| `.openagents/DIRECTIVES.md` | [ROADMAP.md](../ROADMAP.md), [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) | Planning docs archived to `.openagents/archive/2025-12/` |
| `.openagents/USERSTORIES.md` | [ROADMAP.md](../ROADMAP.md), [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) | Planning docs archived to `.openagents/archive/2025-12/` |

## Terminology Changes

When reviewing older documents, note these terminology updates:

| Old Term | Current Term | Reference |
|----------|--------------|-----------|
| `codex_code` | `codex` | GLOSSARY.md |
| `policy_version` | `policy_bundle_id` | GLOSSARY.md |
| `step_utility` (0-1) | `step_utility_norm` | GLOSSARY.md (raw is -1.0..+1.0) |
| `Verified PR Bundle` | `Verified Patch Bundle` | GLOSSARY.md |
| `Datacenter` lane | `Cloud` lane | GLOSSARY.md |
| `proof` (Cashu) | `Cashu Proof` | GLOSSARY.md |
| `adjutant` CLI | `autopilot` CLI | adjutant is internal library only |

## CLI Names

| Binary | Purpose |
|--------|---------|
| `autopilot` | User-facing CLI for sessions, runs, exports, replay, policy |
| `pylon` | Network node CLI for jobs, wallet, provider mode |
| `adjutant` | **Internal library** - not a CLI. DSPy decision pipelines. |

## Storage Paths

| Path | Purpose |
|------|---------|
| `~/.openagents/` | Global config, identity |
| `.autopilot/` | Repo-local session data, policies |

## Kind Numbers

Kind numbers in documentation are **illustrative only**. See [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) for the canonical mapping between `schema_id` values and NIP-90 transport kinds.

## Document Authority Banners

All crate docs should include an authority banner:

```markdown
- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** terminology → [GLOSSARY.md](../../GLOSSARY.md), behavior → code
- **If this doc conflicts with code, code wins.**
```

## Updated Documents (2026-01)

The following documents have been updated with authority banners and terminology fixes:

- `crates/autopilot-core/docs/EXECUTION_FLOW.md`
- `crates/adjutant/docs/README.md`
- `crates/protocol/docs/README.md`
- `crates/pylon/docs/README.md`
- `crates/rlm/docs/README.md`
- `crates/frlm/docs/README.md`
- `crates/gateway/docs/README.md`
- `crates/spark/docs/README.md`
- `crates/issues/docs/README.md`
