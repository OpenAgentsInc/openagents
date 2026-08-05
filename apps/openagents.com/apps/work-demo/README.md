# OpenAgents work items demo (`/work-demo`)

This standalone Rust crate builds the `/work-demo` GPUI/WebGPU document: a
read-only NIP-WK/NIP-PI work-items surface over the Immortal relay
(omega#245). It opens a real Nostr WebSocket subscription
(`{"kinds":[32170,32200],"authors":[<authority>]}` per NIP-01), verifies
every event's id (sha256 of the canonical serialization) and BIP-340
Schnorr signature through the immortal crate's transport-neutral domain
core, refuses events from any key other than the pinned authority, and
renders the Issue Projections as a board grouped by `state` with a detail
view and the kind-32171 Work Event timeline ordered by `seq`.

Honesty rules from the drafts are rendered, not summarized away: missing
sequence numbers appear as explicit gaps, a 32200/32170 revision mismatch
marks the lower record stale, unknown tags and unknown Work Event kinds
are preserved and labeled unknown, and counts carry a not-canonical
qualifier whenever any contributing record is stale or missing. There is
no command path: no NIP-WI intents, no writes, no keys beyond the
authority pubkey.

Configuration lives at the top of `main.rs`:

- `RELAY_URL` — default `wss://relay.openagents.com`, overridable with
  `?relay=wss://...`.
- `AUTHORITY_PUBKEY` — a clearly marked placeholder until the dev seed
  authority from OpenAgentsInc/immortal#33 lands; swap that one constant
  (or override with `?authority=<64-hex>`); nothing else changes.
- `?fixture=1` renders the checked-in `fixtures.json` — the exact WK.md
  1.4 and PI.md 1.5 spec examples plus constructed 32171 events with a
  deliberate seq gap, an unknown event kind, and a stale-revision pair —
  labeled FIXTURE throughout, so the page is verifiable without a relay.

One relay-honesty note: NIP-PI's rendering contract queries the timeline
with `{"kinds":[32171],"#work":[<work_ref>]}`. Immortal's filter grammar
currently accepts single-letter tag selectors only, so the app sends the
contract query verbatim and, if the relay refuses it, falls back to
kinds+authors and filters by the `work` tag client-side, saying so in the
verification panel.

It renders through Omega's design system (`ui`, `theme`, `gpui` via
`gpui_platform`) pinned by full Git commit, on the same build pipeline as
the market demo. Build and stage the static assets manually:

```sh
./build-static.sh
```

The build needs nightly Rust, Trunk, and Zig, and writes the generated
HTML, JavaScript, and WebAssembly under `../start/public/work-demo/`; the
ordinary Start build then copies them into the Cloud Run client bundle.
Serving is env-gated: the monolith refuses `/work-demo` unless
`OPENAGENTS_WORK_DEMO_ENABLED` is exactly `true`
(`workers/api/src/cloudrun/start-ui.ts`). The committed production
environment enables it per omega#245 (2026-08-04).

Local preview:

```sh
trunk serve --release --open=false   # http://127.0.0.1:8083/work-demo/
```

All checks and deployments are manual or run on OpenAgents-owned
infrastructure. GitHub workflows and GitHub-billed automation are
forbidden.
