# Trust stack

Date: 2026-07-08
Status: orientation
Sources: what-openagents-is, behavior contracts, promises alignment, QA
roadmap, Reactor plan, agent computers strategy (via fable references)

## Posture

> Don't trust us — check the receipt.

Trust is not a brand color. It is a set of systems that make false claims
expensive and silent failures visible.

## Layers

### 1. Exact-only accounting

- Every token, compute-minute, and charge traces to a receipt row.
- Counters are **projections of receipts**, never estimates presented as
  fact.
- If unmeasured: label `not_measured` (or equivalent), do not invent.

### 2. Public product promises

- Claims live in a machine-readable registry under `docs/promises/`.
- Green requires dereferenceable evidence + owner sign-off.
- Fable/analysis docs **must not** silently broaden public copy.

### 3. Behavior contracts

- Owner/customer sentences become typed contracts with oracle tests.
- New services ship with contracts from day one.
- Customer-facing invariant families include: indicator truthfulness,
  stated-flow availability, latency budgets, error honesty, dead
  controls, consistency, copy safety.

### 4. Isolation (Agent Computers)

- Per-work microVM, own kernel, wiped on reclaim, separately metered.
- Blast-radius sentence (product requirement): a fully compromised agent
  computer exposes that user's scoped work and credentials for that turn
  — **not** other users' worlds.
- Enforce by contract and test; never aspirational prose alone.

### 5. Bounded authority

- Compiled, deny-precedence toolsets — permissions enforced, not prompted.
- Send and spend sit behind approval receipts by default.
- Credentials brokered, short-lived, die with the machine.
- Employee promotion ladder: observe → draft → act-with-approval →
  (later) act-within-policy — each step receipted.

### 6. Verification culture

- Graders/oracles inform acceptance; they do not silently replace human
  or product authority where required.
- QA Swarm points agents at our product nightly; verdicts can auto-file
  issues.
- Research direction (verification by re-execution) is depth, not the
  default commercial claim.

### 7. Placement / model provenance (Reactor)

- Open weights inside customer boundary behind the same gateway shape.
- Typed model policy (e.g. origin/license constraints) enforced
  structurally — refuse nonconforming pulls with receipts.
- Dogfood: run production under own policy first; external claims stay
  gated until evidence says otherwise.

## Why this stack is the product

Operators already burned by skill supply chains settle on: *don't trust
anybody.* Enterprise sovereignty messaging raises the same demand without
serving the middle market. OpenAgents sells **always-on machines +
authority ladder + receipts** as the fix for babysitting and trust — the
two failure modes that kill field ROI patterns.

## Failure modes to watch

| Failure | Symptom |
| --- | --- |
| Soft accounting | Dashboard numbers without row lineage |
| Marketing ahead of registry | Public copy stronger than promise state |
| Authority creep | Sends/spends without approval receipts under volume pressure |
| Isolation theater | Shared hosts or long-lived credentials "for convenience" |
| Contract rot | UX changes without oracle updates |
| Dual spine | Surface-specific ledgers or claim systems |

## Dogfood rule

If we would not accept a claim from a vendor without a receipt, we do not
publish that claim about ourselves. The nightly swarm and the registry
exist to make that rule mechanical.
