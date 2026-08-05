# Immortal Infrastructure explainer (`/infra`)

This standalone Rust crate builds the `/infra` GPUI/WebGPU document: the
Episode 267 visual companion walking through the architecture of the
decentralized Boltz replacement that the Immortal repo
(`OpenAgentsInc/immortal`) ships. Six sections — the centralized failure,
the decentralized shape, the hardened-binary monorepo, one swap end to
end, the inside of a liquidity provider, and the die-safely failure
matrix — each pairing tight prose with a mermaid diagram rendered in the
Aiur theme. The page performs one live NIP-11 probe of
`relay.openagents.com`; everything else is static.

It renders through Omega's design system (`ui`, `theme`, `gpui` via
`gpui_platform`) pinned by full Git commit, on the same build pipeline as
the market demo (`../market-demo`).

## Diagrams

The mermaid sources live in `diagrams/*.mmd`. They are rendered at build
time — never in the browser — by `diagrams-gen/`, a small native tool
that drives merman (the Rust mermaid engine, `zed-industries/merman`
tag `v0.6.2-with-patches`, MIT OR Apache-2.0) with an Aiur-mapped theme,
overrides merman's hardcoded light sequence-diagram CSS, flattens all
text to paths with usvg (so the wasm document needs no fonts to
rasterize), and writes the committed SVGs into `assets/diagrams/`.
The document embeds those SVG bytes and rasterizes them with gpui's
resvg-backed full-color image pipeline.

Regenerate after editing a `.mmd` source or the theme:

```sh
cd diagrams-gen && cargo run --release
```

## Build and stage

```sh
./build-static.sh
```

The build needs nightly Rust, Trunk, and Zig, and writes the generated
HTML, JavaScript, and WebAssembly under `../start/public/infra/`; the
ordinary Start build then copies them into the Cloud Run client bundle.
Serving is env-gated: the monolith refuses `/infra` unless
`OPENAGENTS_INFRA_EXPLAINER_ENABLED` is exactly `true`
(`workers/api/src/cloudrun/start-ui.ts`). The committed production
environment enables it by owner direction (2026-08-04).

Local preview:

```sh
trunk serve --release --open=false   # http://127.0.0.1:8083/infra/
```

All checks and deployments are manual or run on OpenAgents-owned
infrastructure. GitHub workflows and GitHub-billed automation are
forbidden.
