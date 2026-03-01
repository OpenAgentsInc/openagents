# CAD Kernel Strategy Decision Record

Status: Accepted  
Decision Date: 2026-03-01  
Related Issue: [#2453](https://github.com/OpenAgentsInc/openagents/issues/2453)  
Supersedes: none

## Context

Wave 1 CAD requires a kernel strategy that supports:

- deterministic parametric rebuilds,
- reliable boolean operations for rack-like solids,
- deterministic STEP export path,
- integration with Rust desktop runtime and `crates/cad`,
- acceptable maintenance burden for MVP velocity.

Options considered:

- A: Depend on/vend constrained subset of VCAD kernel crates.
- B: Use OpenCascade bindings directly.
- C: Build a minimal in-house B-Rep subset for generator-only solids.
- D: Use CSG tree now and exact solid eval later.

## In Scope

- Choose primary Wave 1 kernel direction.
- Define fallback direction.
- Define kill criteria for switching off primary.
- Attach weighted scorecard and short spike artifacts.

## Out of Scope

- Full production integration of selected kernel.
- Full benchmark matrix on complete rack corpus.
- Wave 2 sketch/constraint implementation.

## Weighted Scorecard

Weights:

- Robustness / boolean reliability: 30%
- STEP fidelity / export viability: 25%
- Integration cost in current repo: 20%
- Runtime performance potential: 15%
- Maintenance risk: 10%

Scoring scale: 1 (poor) to 5 (strong)

| Option | Robustness (30) | STEP Fidelity (25) | Integration Cost (20) | Runtime Perf (15) | Maintenance (10) | Weighted Score |
|---|---:|---:|---:|---:|---:|---:|
| A: VCAD subset | 4 | 4 | 3 | 4 | 3 | 3.65 |
| B: OpenCascade bindings | 4 | 4 | 2 | 3 | 2 | 3.20 |
| C: Minimal in-house B-Rep | 2 | 1 | 4 | 4 | 3 | 2.45 |
| D: CSG tree then exact later | 1 | 1 | 3 | 3 | 3 | 1.80 |

## Spike Summary

Short spike artifacts are stored under:

`docs/cad/spikes/2026-03-01-kernel-strategy/`

Key observed blockers:

- VCAD local compile path currently blocked by missing local `tang` workspace dependency in `/Users/christopherdavid/code/vcad`.
- OpenCascade crate compile path currently blocked by `occt-sys` toolchain/CMake failure in this environment.

Despite blockers, option C and D remain higher long-term risk for robustness and STEP credibility; option A remains best primary direction provided adapter isolation and kill criteria.

## Decision

Choose **Option A (VCAD subset)** as Wave 1 primary kernel direction via a strict adapter boundary in `crates/cad`.

Fallback direction: **Option B (OpenCascade bindings)**.

Rationale:

- Best expected blend of robustness, STEP viability, and local-first kernel behavior.
- Better alignment with existing CAD-oriented Rust crate ecosystem than in-house B-Rep from scratch.
- Maintains deterministic architecture if wrapped behind adapter boundary.

## Kill Criteria (Switch A -> B)

Switch off option A if any of these remain unresolved after the agreed spike window:

1. Cannot produce deterministic boolean outputs across repeated rack corpus runs.
2. STEP export cannot pass checker/import sanity for the Wave 1 corpus.
3. Integration requires unstable cross-repo coupling that breaks `openagents` build reproducibility.
4. Memory/perf regressions exceed Wave 1 budgets without realistic remediation.

## Required Follow-On Actions

1. Implement `cad::kernel` adapter boundary before higher-level feature work.
2. Track dependency/license/security posture in CAD docs.
3. Run full rack corpus once kernel adapter work is in place and attach expanded artifacts.

## Sign-Off

- Decision owner: OpenAgents CAD backlog owner
- Recorded by: Codex implementation pass for issue #2453
