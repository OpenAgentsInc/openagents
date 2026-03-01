# CAD Code Health Runbook

Related issues:

- [#2554](https://github.com/OpenAgentsInc/openagents/issues/2554)
- [#2555](https://github.com/OpenAgentsInc/openagents/issues/2555)

## Purpose

Provide a single repeatable workflow for CAD lint, formatting, tests, and strict hardening lanes.

This runbook is the canonical local quality gate for:

- `crates/cad`
- CAD integration paths in `apps/autopilot-desktop`
- CAD strict lanes in `scripts/lint/strict-production-hardening-check.sh`

## Ownership + Scope

- Product wiring and pane behavior live in `apps/autopilot-desktop`.
- Reusable CAD logic lives in `crates/cad`.
- Keep changes aligned with `docs/MVP.md` and `docs/OWNERSHIP.md`.

## Required Command Lanes

Run from repo root.

## 1) Formatting

```bash
cargo fmt --all -- --check
```

If this fails:

```bash
cargo fmt --all
```

## 2) CAD Clippy Policy Lane

```bash
scripts/lint/cad-clippy.sh --strict
```

Policy:

- `openagents-cad` lib/bin lane is strict (`-D warnings`, deny `unwrap/expect/panic`).
- `openagents-cad` test lane is transitional (allows `unwrap/expect/panic` while debt is retired).

Advisory mode (for triage only):

```bash
scripts/lint/cad-clippy.sh --advisory
```

## 3) CAD Release Gates (A-E)

```bash
scripts/cad/release-gate-checklist.sh
```

Covers deterministic rebuild/validity/history, viewport/pro UX invariants, variant determinism,
intent safety, and end-to-end reliability/performance checks.

## 4) CAD Reliability / Scripted CI Sub-Lanes

```bash
scripts/cad/step-checker-ci.sh
scripts/cad/headless-script-ci.sh
scripts/cad/reliability-20s-ci.sh
scripts/cad/perf-benchmark-ci.sh
```

## 5) Strict Production Hardening Lane

```bash
scripts/lint/strict-production-hardening-check.sh
```

This is the production hardening umbrella lane and must pass before merge for CAD-impacting work.

## 6) Workspace Clippy Regression Lane

```bash
scripts/lint/clippy-regression-check.sh
```

Use this to catch net-new warnings beyond CAD-specific checks.

## Panic / Expect Policy

## Production paths (strict)

Disallowed in production CAD paths:

- `unwrap()`
- `expect()`
- `panic!`

Applies to:

- `openagents-cad` library and binaries
- strict hardening lanes

## Transitional tests

Test-only lanes may temporarily allow `unwrap/expect/panic` when explicitly scoped and tracked.

Rules:

- Do not move transitional allowances into production code paths.
- Retire temporary allowances via tracked issues and follow-up refactors.

## Failure Triage Flow

1. Re-run only failing lane first.
2. Capture stable reproduction command + stderr snippet.
3. Classify failure:
   - formatting
   - lint policy
   - deterministic snapshot/golden drift
   - performance budget regression
   - release gate invariant regression
4. Fix at source; avoid suppressing failures without issue-backed rationale.
5. Re-run lane, then re-run strict hardening lane.

## Common Failure Classes

- `CAD-WARN-*` model validity warnings unexpectedly changed:
  - review `crates/cad/src/validity.rs`
  - update fixtures only when behavior change is intentional.
- STEP checker failures:
  - run `scripts/cad/step-checker-ci.sh` directly.
  - inspect checker artifacts and tolerance assumptions.
- Performance budget failures:
  - run `scripts/cad/perf-benchmark-ci.sh` directly.
  - compare rebuild/mesh/hit-test deltas and confirm deterministic fixture size.

## Recommended Pre-PR Sequence

```bash
cargo fmt --all -- --check
scripts/lint/cad-clippy.sh --strict
scripts/cad/release-gate-checklist.sh
scripts/lint/strict-production-hardening-check.sh
```

## References

- [`crates/cad/docs/PLAN.md`](/Users/christopherdavid/code/openagents/crates/cad/docs/PLAN.md)
- [`crates/cad/docs/CAD_DEMO_RELEASE_GATES.md`](/Users/christopherdavid/code/openagents/crates/cad/docs/CAD_DEMO_RELEASE_GATES.md)
- [`scripts/lint/strict-production-hardening-check.sh`](/Users/christopherdavid/code/openagents/scripts/lint/strict-production-hardening-check.sh)
- [`scripts/lint/cad-clippy.sh`](/Users/christopherdavid/code/openagents/scripts/lint/cad-clippy.sh)
