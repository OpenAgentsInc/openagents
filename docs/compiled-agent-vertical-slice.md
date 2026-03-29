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

## What It Emits

The harness writes a JSON receipt with:

- prompt lineage
- selected route
- tool calls and tool results
- public outcome
- authority manifest ids
- shadow manifest ids
- internal phase traces
- runtime state used to answer the prompt

This is the narrow handoff seam to `psionic` for receipt normalization, replay,
and later bounded XTRAIN.

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
