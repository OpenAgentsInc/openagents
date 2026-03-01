# CAD External Dependency License and Security Posture

Related issues: [#2455](https://github.com/OpenAgentsInc/openagents/issues/2455), [#2453](https://github.com/OpenAgentsInc/openagents/issues/2453)  
Decision record: `docs/cad/decisions/0001-kernel-strategy.md`

## 1) Scope

This document defines the external dependency posture for Wave 1 CAD kernel work in `crates/cad`.

In scope:

- license and attribution posture,
- vendor/fork/update policy,
- security scanning requirements for imported code paths.

Out of scope:

- final legal review for commercial distribution,
- runtime sandboxing details for all future CAD plugins.

## 2) Selected Kernel Direction

- Primary: Option A (VCAD subset via adapter boundary in `crates/cad`)
- Fallback: Option B (OpenCascade bindings)

All engine-specific integration must remain behind `cad::kernel` traits so switching remains possible.

## 3) Dependency and License Inventory

Current known candidates:

| Dependency candidate | Expected license posture | Notes |
|---|---|---|
| `vcad` kernel crates (subset) | MIT (per upstream repo metadata) | Primary Wave 1 direction |
| `opencascade` crate (`occt-sys`) | Validate on integration branch before adoption | Fallback path only |

License verification rule:

- Before enabling any external CAD kernel dependency in workspace manifests, capture exact upstream license files and SPDX identifiers in this document.
- If dual-license options exist, record chosen license path explicitly.

## 4) Attribution Requirements

For any imported external CAD engine code path:

1. Record upstream repository URL and revision/tag used.
2. Preserve upstream copyright and license notices.
3. Add attribution entry in repository legal/credits documentation before release.

## 5) Vendor vs Fork Policy

Wave 1 policy:

- Prefer pinned upstream source references with adapter isolation first.
- If reproducibility or patching requires source copy, use a vendor snapshot with explicit provenance notes.
- Only fork when:
  - required fixes are not mergeable upstream in needed timeframe, or
  - upstream policy/velocity is incompatible with MVP reliability goals.

Fork requirements:

- Document fork rationale and divergence scope.
- Track rebase/merge cadence and unresolved downstream patches.

## 6) Update Cadence

Minimum cadence for external CAD engine dependencies:

- security/advisory checks: weekly
- dependency patch review: monthly
- minor-version evaluation: quarterly

Emergency update trigger:

- any known critical vulnerability in active dependency graph,
- any determinism-breaking bug fixed upstream with direct CAD impact.

## 7) Security Scanning Requirements

For CAD dependency code paths, required checks before merge:

1. `cargo check` and `cargo test` on affected crates.
2. Advisory scan for Rust dependencies (`cargo audit` when available in environment).
3. Transitive dependency review (`cargo tree`) for newly introduced crates.
4. Review of build scripts (`build.rs`) for native code/toolchain dependencies.

For vendored/forked native components:

- record toolchain requirements and versions,
- track checksum/provenance of imported source snapshots,
- document any post-import local patches.

## 8) Determinism and Supply Chain Guardrails

- External engine usage must not bypass deterministic CAD receipts/validation paths.
- New external dependency adoption requires:
  - deterministic behavior check on seed corpus,
  - explicit failure-mode documentation,
  - rollback strategy if integration destabilizes CAD flow.

## 9) Ownership

- CAD dependency policy owner: CAD backlog owner
- Reviewers: runtime + security + legal stakeholders before release gating
