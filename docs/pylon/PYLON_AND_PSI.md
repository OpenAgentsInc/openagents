# Pylon and Psionic: How They Relate

Status: summary
Date: 2026-04-05

This document summarizes the current public-safe relationship between **Pylon** and **Psionic** for the OpenAgents Compute Market.

## Short version

- **Pylon** is the standalone provider program. It brings a machine online, publishes honest supply, accepts work, and tracks settlement.
- **Psionic** is the runtime repo that owns the Gemma benchmark and deeper execution substrate.
- The current standalone operator flow in this repo is **Gemma-first**. A node should only look sellable when local Gemma supply is actually present.

## What Pylon does

Pylon turns one machine into market-visible supply. It owns:

- provider identity
- local health and inventory truth
- relay intake and result publication
- wallet and payout tracking
- operator-facing status, jobs, earnings, receipts, and activity views

For the current standalone flow, the honest launch product is:

- `psionic.local.inference.gemma.single_node`

## What Psionic does

Psionic is the runtime repo and benchmark lane behind the broader execution roadmap. In practice that means:

- curated Gemma 4 model support
- benchmark and capability validation
- the sibling checkout used by `cargo pylon-headless gemma ...`
- the longer-term substrate for deeper execution and routing work

Pylon already shells into the sibling `psionic` checkout for the retained Gemma benchmark path. That is the current repo-truth bridge between the two systems.

## Current operator truth

Do not describe the current standalone `Pylon` bring-up as a generic multi-backend marketplace.

Describe it this way instead:

- the node is sellable only when local Gemma supply is ready
- non-Gemma local models must not make the node look online
- the standalone benchmark and validation path runs through the sibling `psionic` checkout
- sandbox execution is a separate bounded lane and not part of the Gemma inference identity

## Dependency flow

1. `openagents-provider-substrate` owns shared provider lifecycle, product derivation, receipts, and inventory truth.
2. `Pylon` is the standalone operator shell on top of that substrate.
3. `Psionic` is the runtime repo used for the retained Gemma benchmark and the deeper execution roadmap.
4. The current repo truth is: Pylon operator flow is Gemma-first, and Psionic is the benchmark/runtime lane that validates and deepens that flow.

## Summary diagram

```text
[machine]
   |
   +-- Pylon
   |     |
   |     +-- status / inventory / relay / wallet / receipts
   |     +-- honest local Gemma sellability checks
   |     +-- provider intake and result publication
   |
   +-- Psionic sibling checkout
         |
         +-- Gemma benchmark lane
         +-- runtime and execution substrate roadmap
```

## References

- `docs/pylon/README.md`
- `docs/pylon/PYLON_PLAN.md`
- `https://github.com/OpenAgentsInc/psionic`
