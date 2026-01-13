# ADR-0014: CLI Surface Ownership and Naming

## Status

**Proposed**

## Date

2026-01-13

## Context

Docs and contributors often drift on "what CLI exists" and "which binary owns which command."
Examples of recurring problems:
- Docs reference `adjutant replay ...` even when `adjutant` is dev-only or not shipped.
- Older docs reference `openagents ...` wrappers or legacy binary names inconsistently.
- Command semantics (inputs/outputs/artifacts) get duplicated across docs and diverge.

We need a stable rule set so:
- user-facing docs are consistent,
- agent contributors know where to add/modify CLI behavior,
- commands can evolve without breaking users silently.

## Decision

**CLI surfaces are owned by product binaries. User-facing docs MUST reference the owning binary's CLI.**

### Canonical CLI owners

- **`autopilot`** — product CLI for sessions, runs, export, replay, policy management (user-facing).
- **`pylon`** — node CLI for provider/host operations (user-facing).
- **`adjutant`** — internal/dev CLI only (if present). Not a user-facing contract.

If a doc references a command that is not present in a shipped binary, it MUST be labeled as **Planned** and point to the roadmap or crate sources.

### Optional wrapper binaries

If an umbrella/wrapper CLI exists (now or later), it MUST be a **thin dispatcher**:
- it maps subcommands 1:1 onto the owning binary's semantics,
- it does not define new contract formats,
- it must not fork storage layouts or artifact formats.

Docs should still reference the owning binary as the source of truth for behavior.

### Documentation rule (normative)

When documenting a CLI command:
1. Use the **owning binary name** in examples (`autopilot …`, `pylon …`).
2. Link to the owning crate docs or `--help` output location.
3. If the command is planned/spec-only, explicitly label it as **Planned**.

## Scope

What this ADR covers:
- Which binary owns which CLI surface
- What is considered "user-facing" vs "dev-only"
- How docs must reference CLI commands

What this ADR does NOT cover:
- The full command list for each binary (that changes over time)
- Artifact schemas (see ADR-0002/ADR-0003 and canonical specs)
- Routing semantics (see ADR-0004)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| User-facing CLIs | `autopilot` and `pylon` are the canonical user surfaces |
| Dev-only CLI | `adjutant` is not a user-facing contract |
| Docs | Must reference owning binary; planned commands labeled "Planned" |

Backward compatibility expectations:
- If a command is renamed or removed, it requires a superseding ADR + migration note (or a compatibility shim period).
- New commands can be added freely, but must be documented under the owning binary.

## Consequences

**Positive:**
- Stops "CLI drift" across docs
- Makes it clear where to implement new commands
- Keeps tutorials aligned with what users can actually run

**Negative:**
- Requires updating older docs that reference `adjutant` commands as if they exist
- Slight overhead: docs must label planned commands explicitly

**Neutral:**
- Wrapper CLIs can exist, but cannot redefine contracts

## Alternatives Considered

1. **Single unified CLI as the only surface** — rejected for now; increases coordination cost and tends to rot unless already shipped and enforced.
2. **Allow docs to reference any internal binary** — rejected; causes user confusion and broken tutorials.
3. **Only document behavior, never commands** — rejected; users need executable examples.

## References

- [ADR-0001](./ADR-0001-adoption-of-adrs.md) — authority hierarchy
- [GLOSSARY.md](../../GLOSSARY.md) — canonical terminology
- [ROADMAP.md](../../ROADMAP.md) — MVP gates and planned surfaces
- [AGENTS.md](../../AGENTS.md) — "CLI surfaces" section and status notes
- `crates/autopilot/` — product CLI implementation
- `crates/pylon/` — node CLI implementation
