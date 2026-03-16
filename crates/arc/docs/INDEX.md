# ARC Docs Index
Status: compact navigation index for the ARC subtree
Date: 2026-03-15

This file is the compact entrypoint for `crates/arc/docs/*`.

Use it when you need to answer:

- which ARC doc is canonical for architecture?
- which ARC doc is canonical for roadmap sequencing?
- where is the frozen claim vocabulary?
- which audit explains the upstream port/source mapping?
- where should future acceptance matrices land?

## Canonical Docs

| Doc | Role | Use when you need... |
| --- | --- | --- |
| `crates/arc/docs/spec.md` | canonical architecture and ownership spec | crate boundaries, owner split, dependency direction, and normative ARC system shape |
| `crates/arc/docs/ROADMAP.md` | canonical dependency-ordered roadmap | execution order, epic boundaries, issue sequencing, and acceptance gating |
| `crates/arc/docs/CAPABILITY_MATRIX.md` | canonical current-state capability matrix | landed vs bounded vs blocked vs unknown status, solver/evidence coverage, and ARC-AGI-3 preview drift |
| `crates/arc/docs/CLAIMS.md` | canonical claim vocabulary | frozen claim names, minimum meanings, and claim-to-artifact expectations |
| `crates/arc/docs/UPSTREAM_TARGETS.md` | canonical first-pass upstream target freeze | exact upstream repo commits, protocol files, and benchmark/scoring source targets |
| `crates/arc/docs/PUBLIC_EVAL_HYGIENE.md` | canonical public-eval operator policy | mandatory labeling rules, operator posture, and validator entrypoint for public-eval artifacts |
| `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md` | canonical port-source and upstream-mapping audit | upstream source mapping, direct-port priorities, and Psionic-vs-ARC split rationale |

## Future Acceptance Surfaces

These are not all separate files yet, but they already have fixed roles.

| Future surface | Planned owner / anchor |
| --- | --- |
| claim vocabulary | `crates/arc/docs/CLAIMS.md` |
| capability matrices | `crates/arc/docs/CAPABILITY_MATRIX.md` |
| fixture corpora index | `ARC-602` |
| operator workflow docs | `ARC-603` |
| non-Python smoke coverage | `ARC-604` |
| compact acceptance index linking fixtures, reports, and gates | `ARC-606` |

Until those later docs exist, the rows above remain the canonical placeholders
for the future acceptance-matrix family.

## Reading Order

If you are new to the ARC subtree, read in this order:

1. `crates/arc/docs/INDEX.md`
2. `crates/arc/docs/spec.md`
3. `crates/arc/docs/CAPABILITY_MATRIX.md`
4. `crates/arc/docs/CLAIMS.md`
5. `crates/arc/docs/UPSTREAM_TARGETS.md`
6. `crates/arc/docs/PUBLIC_EVAL_HYGIENE.md`
7. `crates/arc/docs/ROADMAP.md`
8. `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md`

## Maintenance Rule

When a new ARC doc becomes canonical for one of the roles above, update this
index in the same change.
