# OpenAgents Markets swap demo (`/demo`)

This standalone Rust crate builds the `/demo` GPUI/WebGPU document: an
interactive walkthrough of one NIP-MKT negotiated swap session — public
discovery, private RFQ, competing quotes (firm/hard versus
indicative/soft), the verify-before-fund checklist, the per-signer status
timeline with an explicit sequence gap, and the terminal Close plus
redacted public receipt. Every value is synthetic and labeled DEMO; no
keys, funds, or custody exist in the browser. The page performs one live
NIP-11 probe of `relay.openagents.com` to show the relay's real status.

It renders through Omega's design system (`ui`, `theme`, `gpui` via
`gpui_platform`) pinned by full Git commit, on the same build pipeline as
the Diamond Hands document. See
`docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` for the rollout
plan this implements (phase P0/P1).

Build and stage the static assets manually:

```sh
./build-static.sh
```

The build needs nightly Rust, Trunk, and Zig, and writes the generated
HTML, JavaScript, and WebAssembly under `../start/public/demo/`; the
ordinary Start build then copies them into the Cloud Run client bundle.
Serving is env-gated: the monolith refuses `/demo` unless
`OPENAGENTS_MARKET_DEMO_ENABLED` is exactly `true`
(`workers/api/src/cloudrun/start-ui.ts`). The committed production
environment enables it by owner direction (2026-08-04).

Local preview:

```sh
trunk serve --release --open=false   # http://127.0.0.1:8082/demo/
```

All checks and deployments are manual or run on OpenAgents-owned
infrastructure. GitHub workflows and GitHub-billed automation are
forbidden.
