# Compiled Agent Vertical Slice

This document records the first app-owned compiled-agent path that now exists in
`openagents`.

## Scope

The slice is intentionally narrow. It handles only:

- provider readiness questions
- wallet balance / recent earnings questions
- clean refusal for anything else

It uses the reusable `openagents-compiled-agent` crate for:

- typed signatures
- phase-separated graph execution
- promoted vs candidate manifests
- shadow mode
- confidence-gated fallback
- replay-friendly lineage

It now consumes promoted and candidate module artifacts from the retained
`psionic` compiled-agent promoted-artifact contract instead of hardcoding the
route, grounded-answer, and verify behavior inside `openagents`.

## Entry Point

Run the harness from the repo root:

```bash
cargo run -p autopilot-desktop --bin autopilot-compiled-agent-harness -- \
  --prompt "Can I go online right now?" \
  --receipt-out target/compiled-agent/provider-ready.json
```

Wallet example:

```bash
cargo run -p autopilot-desktop --bin autopilot-compiled-agent-harness -- \
  --prompt "How many sats are in the wallet?" \
  --wallet-balance-sats 3400 \
  --recent-earnings-sats 180 \
  --receipt-out target/compiled-agent/wallet.json
```

Candidate shadow example:

```bash
cargo run -p autopilot-desktop --bin autopilot-compiled-agent-harness -- \
  --prompt "How many sats are in the wallet?" \
  --shadow-mode evaluate-candidate \
  --show-trace
```

Rollback example using the retained `last_known_good` route artifact:

```bash
cargo run -p autopilot-desktop --bin autopilot-compiled-agent-harness -- \
  --prompt "Can I go online right now?" \
  --shadow-mode candidate-authority \
  --candidate-label last_known_good \
  --show-trace
```

## What It Emits

The harness writes a JSON receipt with:

- prompt lineage
- selected route
- tool calls and tool results
- public outcome
- authority manifest ids
- authority artifact ids and digests
- shadow manifest ids
- shadow artifact ids and digests
- internal phase traces
- runtime state used to answer the prompt

This is now the narrow runtime-adoption seam to `psionic` for:

- promoted artifact authority
- candidate shadow comparison
- receipt normalization
- replay
- bounded XTRAIN

## Why This Exists

This slice is not meant to be broad or clever. It exists to make one honest
compiled-agent path real:

- route
- tool policy
- tool arguments
- grounded answer
- verify or refuse

That path now produces structured lineage instead of a giant improvised prompt
loop.

## Current Authority Posture

The current retained runtime contract keeps the admitted task family narrow:

- provider readiness
- wallet balance and recent earnings
- unsupported refusal

Within that boundary:

- promoted route authority comes from the learned artifact
  `compiled_agent.route.multinomial_nb_v1`
- `last_known_good` is available as a rollback-safe route candidate
- `psionic_candidate` is available for grounded-answer and verify shadow runs
