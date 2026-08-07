# OpenAgents Markets swap demo (`/demo`)

This standalone Rust crate builds the `/demo` GPUI/WebGPU document. It has
two separate lanes:

- verified live discovery reads bounded 39600/39601 heads directly from the
  configured relay, validates their structure, signature, timestamps, MKT
  shape, and replacement ordering, and publishes the snapshot only after
  EOSE;
- the scripted DEMO lane walks through synthetic quotes, verification,
  status gaps, Close, and a redacted receipt. Every value in that lane is
  labeled DEMO.

When a discovered provider advertises `mode=no_spend`, the live lane can use
an in-browser throwaway identity to exchange a signed NIP-59
RFQ→Quote→Order→Contract→Status→Cancel→Close session. It cancels before
funding and succeeds only when the provider-signed Close reports
`outcome=cancelled`, `external_spend_effects=0`, and
`loss_accounting.input_committed=0`. The relay remains transport only. The
throwaway key exists only in browser memory; the page does not persist or log
it, and it has no funding or custody capability.

It renders through Omega's design system (`ui`, `theme`, `gpui` via
`gpui_platform`) pinned by full Git commit, on the same build pipeline as
the Diamond Hands document. See
`docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` for the rollout
plan this implements. The public relay is the default and may return a
verified empty snapshot.

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

Local browser-owned proof, from the Immortal repository:

```sh
IMMORTAL_DEV_RELAY_PORT=18080 scripts/dev-relay.sh
```

In a second terminal:

```sh
cargo build --locked -p immortal-provider --bin immortal-provider \
  --features no-spend
IMMORTAL_PROVIDER_IDENTITY_SECRET="$(openssl rand -hex 32)" \
  IMMORTAL_PROVIDER_RELAY_URL=ws://127.0.0.1:18080 \
  target/debug/immortal-provider --no-spend
```

Then serve this crate:

```sh
trunk serve --release --open=false
```

Open
`http://127.0.0.1:8082/demo/?relay=ws%3A%2F%2F127.0.0.1%3A18080`.
Only the public relay or a loopback relay is accepted by the query parameter.

The headless proof checks the exact bounded requests, EOSE, the signed local
provider and offering addresses, five accepted requester wraps, six provider
wraps, the absence of OpenAgents API calls, and browser errors:

```sh
MARKET_DEMO_EXPECT_PROVIDER_PUBKEY=<provider-ready-pubkey> \
  node ../../scripts/market-demo-browser-proof.mjs \
  'http://127.0.0.1:8082/demo/?relay=ws%3A%2F%2F127.0.0.1%3A18080'
```

All checks and deployments are manual or run on OpenAgents-owned
infrastructure. GitHub workflows and GitHub-billed automation are
forbidden.
