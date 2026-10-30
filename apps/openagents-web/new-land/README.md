# New-land development capture

The route remains development-only. Its projects, work and Pylons are public
Nostr observations admitted by the shared Coax libraries. People walking the
Grid are simulated and labelled as such.

Refresh and build only this page:

```sh
bun coax/web/project/landing.ts
COAXIAL_MODE=dev coax/web/wasm.sh apps/openagents-web/newland.coax apps/openagents-web/newland-face.c apps/openagents-web/build/newland.wasm
```

A normal development build refreshes the capture too. A refused or interrupted
read preserves the previous complete file. Release builds skip this route.

`coax/web/project/landing-reader.ts` provides the Wasm and socket effects;
`landing.ts` handles files and git. `landing-nostr.coax` owns message reading, public verification,
filter matching, bounded pagination, NIP-19, beacon digest/freshness and signed
activity counts. `landing-fold.coax` reuses the existing Coax project fold;
the private C bridge verifies the same immutable original immediately before
copying it into the fold's owned storage. No public ABI accepts a verification
assertion from the caller. `landing.ts` and `pylons.ts` format its projections
for the scene; neither imports the Nostr SDK or keyed publisher.

The visible footer identifies a captured view. Expired beacons become unknown
and stop contributing live slots. Observed age grows honestly across pauses;
it is never wrapped to make old data look recent. Activity is provider-wide
signed answer/status events, not proof of completed work or failures. Reply
delay is unknown without a verified referenced request. Optional owner auth
and project-authored worker names do not establish independently authenticated
ownership or worker identity.

Focused checks:

```sh
coax/bin/coax test coax/web/project/landing-nostr.coax --library --steps 250000000
bun test coax/web/tests/newland_reader.test.ts
coax/bin/coax test apps/openagents-web/newland.coax --steps 400000000
node coax/web/tests/newland_wasm.cjs apps/openagents-web/build/newland.wasm
```

Scope, limits and observed results are in the
[migration record](../../../docs/verification/2026-10-30-newland-coax-nostr.md).
