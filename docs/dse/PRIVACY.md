# Privacy Presets (Swarm Dispatch Defaults)

This document defines the canonical privacy policy presets referenced by `docs/plans/archived/adr-legacy-2026-02-21/ADR-0016-privacy-defaults-swarm-dispatch.md`.

Terminology:
- `PrivacyPolicy`, redaction modes, and trust tiers are described at the ADR level.
- Protocol-level receipt/publication redaction rules are covered by `docs/plans/archived/adr-legacy-2026-02-21/ADR-0017-telemetry-trace-contract.md`.

## Presets (Canonical Names)

Preset names are stable:
- `open_source`
- `private_repo`
- `paranoid`

## Preset Parameters (Canonical Defaults)

These defaults are conservative and are intended as a **spec**. Implementations may be stricter. Code behavior remains authoritative.

### open_source

Purpose: public repositories where leaking paths/code is acceptable.

- Redaction mode: `None`
- Allowed job types: all
- Auto-redact: disabled (no redaction needed)
- Max content size: unlimited (bounded only by lane/provider limits)

### private_repo (recommended swarm default)

Purpose: typical private repos.

- Redaction mode: `PathsOnly`
- Allowed job types: allowlisted only (deny by default)
- Auto-redact: enabled (paths-only)
- Max content size (per job): 512 KB (after redaction)
- Verification required: true for objective jobs (lint/test/build)
- Disallow raw secrets patterns: true (reject on detection)

### paranoid

Purpose: sensitive codebases (keys, proprietary IP, regulated environments).

- Redaction mode: `Full`
- Allowed job types: minimal allowlist only
- Auto-redact: enabled (full redaction)
- Max content size (per job): 64 KB (after redaction)
- Verification required: true
- Require trusted provider tier: true (no untrusted providers)

## Notes

- This doc intentionally avoids listing every job type id; job type allowlists should be maintained alongside the dispatcher/tooling that consumes them.
- Tightening defaults (more restrictive) is allowed without a superseding ADR. Loosening requires a superseding ADR.

