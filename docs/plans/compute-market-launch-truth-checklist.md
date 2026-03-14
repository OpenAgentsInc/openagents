# Compute Market Launch Truth Checklist

Status: active  
Date: 2026-03-07

This checklist exists to keep launch language, operator behavior, and actual repository state aligned. The OpenAgents Compute Market should be described as a compute market, not reduced to a single narrow backend loop, but it also should not be marketed as a generic raw accelerator exchange before the product can defend that claim in the desktop, in the authority layer, and in public operator-facing stats. The right launch posture is therefore specific: `compute` is the umbrella market, `inference` and `embeddings` are the first standardized compute product families inside that umbrella, and accelerator or hardware traits refine supply through the capability envelope rather than replacing the primary product identity.

That distinction matters operationally as much as editorially. A provider, buyer, operator, and auditor should all be able to answer the same questions from the live system without reading source code. Which compute families are live? Which backends are allowed? Which phases are enabled in the authority? Which advanced phases are still backend-only or advanced-only? Which parts of the truth are legacy or transitional rather than canonical compute-market state? If those answers cannot be recovered from the product, `/stats`, receipts, and snapshots, the market is not launch-honest yet even if the underlying types already exist.

## Launch Position

- The market name is the `OpenAgents Compute Market`.
- `Compute` remains the umbrella market category.
- The first live compute product families are `inference` and `embeddings`.
- Launch is not framed as raw accelerator trading.
- Accelerator, runtime, latency, memory, and backend traits live in the capability envelope.
- Buyers request compute products plus capability constraints rather than buying undifferentiated hardware.

## Backend Truth

- `Ollama` is a launch backend for `inference` and `embeddings`.
- `Apple Foundation Models` is a launch backend for `inference` only until a truthfully supported embedding path exists.
- A provider must not be shown as offering a launch family unless the desktop can prove the backend is healthy and policy-allowed.
- Capability envelopes may include hardware-sensitive facts such as accelerator vendor, accelerator family, and memory, but those fields refine offers rather than redefining the launch taxonomy as a raw accelerator market.

## What Must Be Live Before Claiming Market Availability

- Canonical compute products, lots, instruments, delivery proofs, and indices are backed by authoritative mutations and receipts.
- `/stats` and snapshot output expose compute-specific market health, not only generic earnings counters.
- Rollout gates are visible for forward physical, future cash, structured products, and reconciliation diagnostics.
- Breaker rows are visible for delivery integrity, provider concentration, index quality, buyer concentration, and paper-to-physical posture.
- Historical truth labels distinguish `legacy`, `transitional`, and `canonical` compute-market activity.
- Policy bundle identifiers and versions are visible so operator policy posture is auditable.

## What Can Be Described As Live Today

- The authority layer supports standardized compute objects, delivery proofs, indices, forward physical instruments, future-cash settlement logic, and structured instrument composition.
- Public stats and runtime snapshots publish compute inventory, delivery integrity, price and fill behavior, forward exposure, hedging posture, provider concentration, rollout gates, breaker state, policy bundle metadata, and truth-label diagnostics.
- Advanced compute phases can be hard-gated at runtime through `NEXUS_CONTROL_COMPUTE_ENABLE_FORWARD_PHYSICAL`, `NEXUS_CONTROL_COMPUTE_ENABLE_FUTURE_CASH`, `NEXUS_CONTROL_COMPUTE_ENABLE_STRUCTURED_PRODUCTS`, and `NEXUS_CONTROL_COMPUTE_ENABLE_RECONCILIATION_DIAGNOSTICS`.

## What Must Still Be Described More Narrowly

- Desktop productization is still narrower than the full authority surface.
- The retained desktop earn loop is still inference-led even though the launch taxonomy now includes `embeddings`.
- Advanced instruments are available in the authority and observability surface, but they should still be described as advanced or backend-facing until the desktop exposes them as an intentional operator or buyer workflow.
- Raw accelerator spot or futures markets are not launch truth and should not be implied by marketing copy or product IDs.

## Operator Pre-Launch Checks

- Run `scripts/release/check-compute-launch-program.sh` and inspect
  `SUMMARY.md` plus `summary.json`.
- Add `--soak-iterations 3` and the relevant optional platform legs before
  widening production claims beyond the default local gate.
- Confirm `/stats` exposes the expected `compute_rollout_gates` values for the intended environment.
- Confirm `/stats` publishes the expected `compute_policy_bundle_id` and `compute_policy_version`.
- Confirm `compute_truth_labels` are enabled in environments that need migration diagnostics, and intentionally disabled where operators do not want transitional labeling noise.
- Confirm breaker rows are present and their actions are understandable before widening product claims or enabling advanced phases.
- Confirm provider concentration, delivery rejection rate, and index quality are within policy before enabling future cash or structured products in production.

## Copy And Messaging Checks

- Say: `We are launching the OpenAgents Compute Market with inference and embeddings as the first live compute product families.`
- Do not say: `the compute market is really just an inference market`.
- Do not say: `raw H100 trading is live` unless a real top-level hardware market exists with its own product identity, settlement logic, and buyer UX.
- When discussing hardware-sensitive procurement, say that buyers request compute products with capability-envelope constraints.

## Launch-Honesty Failure Modes

- A backend advertises a family it cannot actually serve.
- Desktop copy claims a broader market than the authority or UI can substantiate.
- Docs describe raw accelerator trading when the actual market identity is still backend-mediated compute products.
- Operators cannot tell which advanced compute phases are enabled from `/stats`.
- Historical dashboards mix legacy labor truth and canonical compute truth without labeling the difference.
