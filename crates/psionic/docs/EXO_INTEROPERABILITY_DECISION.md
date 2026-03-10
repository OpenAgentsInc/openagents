# Exo Interoperability Decision

Status: decided 2026-03-10 after landing `#3329` on `main`

## Decision

Discard broad Exo interoperability as an active roadmap target.

Keep only the bounded `ExoPlacementHint` seam landed in `#3329` as an optional,
Psionic-owned import surface for experimentation. Do not pursue an Exo
orchestrator bridge, Exo-owned discovery/election authority, or any execution
delegation path.

## Why

The cluster roadmap has already landed the Rust-native substrate Exo was meant
to inform:

- Psionic owns cluster identity, ordered state, catchup, topology, placement,
  scheduling, replication, sharding evidence, and validation
- the former CUDA local-runtime gate `#3276` -> `#3288` -> `#3248` is closed on
  `main`
- the Exo-derived spike in `#3329` proved the only currently useful interop
  seam is a bounded placement hint that cannot widen eligibility or override
  Psionic-owned truth

The remaining broad Exo bridge options do not justify their cost:

- they introduce avoidable Python/MLX-shaped dependency pressure into a
  Rust-owned control plane
- they risk splitting placement truth between Exo and Psionic
- they weaken the repo's requirement that Psionic receipts and evidence remain
  the source of truth for final placement and execution
- they do not solve the active honest blocker for future Apple clustered
  execution, which is still the Metal GPT-OSS queue

## What We Keep

- Exo remains a design reference for cluster-control ideas already absorbed into
  Psionic docs and tests
- `ExoPlacementHint` remains available as an optional, bounded input to
  whole-request remote scheduling
- any future Exo-shaped import must preserve these constraints:
  - Psionic keeps final placement authority
  - Psionic executes on each worker node
  - Psionic receipts and runtime evidence remain authoritative
  - the import may bias only among already-eligible candidates

## What We Discard

- no Exo-owned control-plane authority in front of Psionic
- no Exo discovery, election, or placement service as a required runtime peer
- no FFI, proxy, or subprocess execution path that makes Exo necessary for
  Psionic cluster execution
- no roadmap work to make Exo a first-class shipped dependency

## Revisit Boundary

Revisit this decision only if a future GitHub-backed issue can show a narrower
interop seam than `#3329` that reduces real operator cost without weakening
Psionic-owned evidence truth.

Until that happens, the effective decision is:

- keep the bounded hint seam
- discard the broader Exo bridge
