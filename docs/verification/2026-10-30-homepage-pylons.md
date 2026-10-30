# Pylons on the homepage — 2026-10-30

The PYLONS control on `/` and its `/new-land` alias opens the network board
inside the homepage. It provides the exact audience join command, the agent
guide, current online/free advertisements, machine inspection and full model
identities/public keys. Project updates, the interactive Grid and the realm
galaxy keep their existing paths. The standalone `/pylon` link still works.

The board uses the homepage's existing persistent watch and Coax original-event
verification. No second Nostr client, signing key, public test publication or
DOM overlay was introduced. C adds read-only UI probes. Full npubs and model
identities survive the bounded projection instead of being shortened at `@`.
Only current online advertisements contribute free capacity; draining is shown
separately. Answer/status values remain observed provider events, not completed
jobs or globally complete accounting. Accessible HTML/Markdown twins include
the command and guide.

The board fits desktop and phone screens, wraps identities, and scrolls with
wheel or touch. Header actions stay fixed. A scroll or cancelled press cannot
launch a command or pan/zoom the Grid. Joining emits the generic host's existing
clipboard action with the complete 96-byte shell command.

## Focused qualification

- Coax renderer/projection checks: 67 passed before the isolated gesture fix
  below. No admission or crypto policy was changed.
- Pure row-format extraction: 22 checks passed, covering exact npubs/identities,
  the 128-byte model boundary, explicit fifth-model ellipsis and output refusal.
- Actual homepage Wasm: 5 initial UI cases, 205 assertions. Desktop, phone and
  DPR 2 layouts; exact command, guide and close; drag/cancel refusal; ten signed
  machine rows; signed capacity changes and expiry.
- Review found a remaining-finger scroll jump and incorrect Retina wheel units.
  Two new regressions failed before the fix, then passed with 29 assertions
  (both finger lift orders included). The original five cases were filtered
  out of that rerun.
- The linked renderer's nine frames across three viewports retained finite
  geometry and output guards; peak 48,846 vertices of 400,000. Pause expiry passed.
- Muted headless Chrome used real sockets on desktop and phone, genuine CDP
  mouse/touch input, actual clipboard readback and guide navigation/HTTP 200.
  Both views saw 3 fresh Pylons / 10 advertised free slots, with all six projects
  caught up and no JS exceptions. This local browser proof preceded the
  gesture-only fix; the changed gesture paths have their separate regression.
- Desktop/phone screenshots were inspected. Only `home` and `home-phone`
  goldens were approved. GPU-hidden markup checks and whitespace checks passed.

The unchanged shared reader, BIP-340, digest/freshness and rollback gates retain
[their prior proof](2026-10-30-live-network-homepage.md). No unrelated model,
compiler bootstrap or full application mutation suite was repeated. The existing
qualified release compiler was installed from its matching provenance cache.
[Inputs, local browser results and screenshots](data/homepage-pylons/)
identify the scope. Test commands:

```sh
coax/bin/coax test apps/openagents-web/newland.coax --steps 400000000
coax/web/project/build-home.sh apps/openagents-web/build/newland.wasm
bun test coax/web/tests/home_pylon_ui.test.ts
bun test coax/web/tests/home_pylon_ui.test.ts -t 'Pylon scroll regression'
bun coax/web/tests/home_pylon_browser.ts http://127.0.0.1:18843
node coax/web/tests/newland_wasm.cjs apps/openagents-web/build/newland.wasm
```

The first UI run and the later regression run are distinct records, not a
request to repeat the first run after every unrelated main commit.

## Deployment

Pending the normal committed-code staging→same-byte production release.
