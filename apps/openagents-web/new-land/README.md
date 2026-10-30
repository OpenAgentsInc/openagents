# The live OpenAgents homepage

`/` and `/new-land` run `newland.coax`. The browser's generic host moves socket
bytes and supplies its clock; shared Coax verifies original events and Pylon
beacon digests. `newland-network.coax` owns watches, bounded project catch-up,
known-root repairs, atomic publication and reconnects. No Nostr SDK or keys are
used by the homepage runtime. The C faces bridge byte buffers and effects.

Pylon announcements update immediately over a persistent watch. Verified
project changes schedule bounded refreshes; incomplete histories preserve the
last view. Slots expire with the signed beacon. Provider answer/status counts
are observations during this tab's watch, with unknown reply timing. Worker
labels are project-authored claims. Git trees are the deployment's real history.
No simulated people are drawn.

The homepage's PYLONS control opens a scrollable board in the same canvas.
It shows online machines, free slots and current advertised capacity; machine
inspection keeps full npub and model identities available. Provider answers
and statuses remain observation counts, not completed-job claims. COPY COMMAND
copies the portable join command below; GUIDE opens `/pylon/index.md`.
The HTML and Markdown twins also carry both for agents and non-WebGPU readers.

```sh
curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh
```

Build only this page:

```sh
bun coax/web/project/landing.ts
coax/web/project/build-home.sh apps/openagents-web/build/newland.wasm
```

`landing.ts` refreshes the real warm start. Runtime reads are independent of
that snapshot. `newland-golden-data.coax` is an explicitly frozen public fixture
for deterministic visual checks; production visitors use the live path.

Focused checks:

```sh
coax/bin/coax test apps/openagents-web/newland.coax --steps 400000000
coax/bin/coax test coax/web/project/landing-browser.coax --library --steps 250000000
bun test coax/web/tests/home_live_wasm.test.ts
bun coax/web/tests/home_runtime.ts apps/openagents-web/build/newland.wasm
node coax/web/tests/newland_wasm.cjs apps/openagents-web/build/newland.wasm
```
