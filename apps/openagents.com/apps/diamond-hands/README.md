# Operation Diamond Hands web surface

This standalone Rust crate builds the `/dh` GPUI/WebGPU document. The browser
owns its WebSocket to `wss://relay.openagents.com`; Immortal owns the bounded
subscription, event verification, EOSE snapshot boundary, live fold, and
reconnect state machine. There is no project-data API or server-side relay
proxy.

Build and stage the stable static assets manually:

```sh
./build-static.sh
```

The build pins Omega and Immortal by full Git commit and needs nightly Rust,
Trunk, and Zig. `build-static.sh` writes only the generated HTML, JavaScript,
and WebAssembly under `../start/public/dh/`; the ordinary Start build then
copies them into the Cloud Run client bundle.

`project-events.phase0.json` and `project-update.phase0-live.json` are operator
authoring inputs, not browser assets. The single Immortal binary signs them
through its bounded `sign-openagents-project-events` command using
`IMMORTAL_RELAY_SECRET_KEY` from the protected environment. Signed events are
then published over Nostr with
`../../scripts/publish-diamond-hands-events.mjs`; the browser reads their
content only from the relay. The publisher accepts at most 32 already-signed
events, waits for every relay `OK`, and fails if any event is refused.

Run the direct-network proof against a locally served release build:

```sh
trunk serve --release --open=false
node ../../scripts/diamond-hands-browser-proof.mjs http://127.0.0.1:8081/dh/
```

Set `DIAMOND_HANDS_EXPECT_LIVE_EVENT_ID` to a signed update ID before running
the proof to make it pause after EOSE and require that exact event on the live
subscription. Publish the event only after the proof prints its
`waitingForLiveEvent` record.

All checks and deployments are manual or run on OpenAgents-owned
infrastructure. GitHub workflows and GitHub-billed automation are forbidden.
