# Psionic System Architecture

The canonical architecture document defines Psionic as the Rust-native
execution substrate for compute workloads inside OpenAgents.

The main architectural split is:

- Psionic owns reusable execution substrate.
- OpenAgents app code owns product workflows and UI.
- Kernel or hosted authority owns final market truth and settlement.

The substrate Psionic owns includes:

- runtime execution
- backend discovery and planning
- clustered topology and ordered state
- artifact staging and resumable transport
- runtime and environment manifest binding
- session-bound execution identity
- sandbox execution
- serving contracts
- training recovery and collective planning
- execution evidence and proof bundles

The architecture doc also uses a strict status vocabulary:

- `implemented`
- `implemented_early`
- `partial`
- `partial_outside_psionic`
- `planned`

That matters because Psionic docs try to say exactly what is real today instead
of collapsing everything into one optimistic roadmap.

Two especially important themes show up throughout the spec:

1. Truthful boundaries

Psionic explains what happened at execution time. It should not silently absorb
UI, wallet, or settlement concerns that belong elsewhere.

2. Evidence-bearing execution

Psionic is supposed to emit machine-legible receipts, manifests, topology
facts, proof bundles, and lineage records so downstream systems can reason
about real execution instead of vague status strings.

The architecture also highlights `Tassadar`, an executor-class reference lane
inside the Psionic workspace. For a new reader, the main point is not every
phase detail. The point is that Psionic is already more than a thin inference
wrapper. It includes a growing execution and evaluation substrate with explicit
capability reporting and proof posture.
