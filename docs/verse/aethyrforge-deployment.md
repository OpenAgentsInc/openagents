# Aethyrforge browser deployment

The Medieval Town demo at `/demo/medieval-town` embeds a pinned Aethyrforge
release. OpenAgents owns the Rust page, public Rust/Wasm SDK, and host protocol.
The private Aethyrforge repository owns the renderer, editor, and build tools.
The root homepage is unchanged.

## Source and artifact boundary

- The public repository builds and tests without a private checkout or private
  package credentials. Its release JSON contains URLs and compatibility values.
- Engine containers contain compiled browser JS/Wasm and dependency notices.
  Visitors can inspect those bytes. Private source, source maps, secrets, and
  purchased source exports are excluded from public Git and runtime containers.
- Compiled scene dependencies live in a private bucket with public access
  prevention. Only the preview runtime account reads that bucket. The editor
  account has no project roles. The browser receives no account, wallet, model,
  or publishing credentials.
- Editor and preview use separate HTTPS origins, sandboxed frames, exact host
  CORS and frame ancestors, and release-specific immutable URLs. Camera,
  microphone, geolocation, and payment authority are disabled.
- This release supports signed-out exploration, movement, fullscreen, reload
  with the camera position restored, and cleanup on leaving. Authenticated
  editing and publishing remain separate MVP issues.

## Build and promote

1. Build a clean private revision with the private repository's
   `tools/deploy/README.md`. Freeze the recursive compiled asset dependencies,
   matching public SDK, release manifest, and image digests. Keep old release
   tags so existing sessions retain their selected engine.
2. Set the public configuration in
   `crates/openagents-web/content/demos/medieval-town.json`. It pins the editor,
   matching SDK, release, ABI, project format, and project identity. The server
   validates HTTPS and requires the SDK and editor to share an origin.
3. Build the ordinary website image from a pushed public commit using
   `crates/openagents-web/cloudbuild.yaml`. Promote by digest with
   `scripts/deploy/web.sh`. `WEB_MEDIEVAL_RELEASE_JSON` can override the compiled
   default when promoting, including selecting a previous engine release.
4. Use a separate `WEB_DEPLOY_TAG` and `XDG_STATE_HOME` when other deployments
   might be in progress. Run GCP writes with the automation account's isolated
   `CLOUDSDK_CONFIG`; never fall back to an owner's login.
5. Before shifting traffic, run `scripts/check-medieval-town.mjs` through
   `openagents browser run -- node scripts/check-medieval-town.mjs`. Supply
   `AETHYRFORGE_DEPLOY_SCRATCH` and `AETHYRFORGE_RELEASE_CONFIG` (release, editor,
   preview, and SDK SHA-256). Candidate mode also takes
   `AETHYRFORGE_CHECK_MODE=candidate` and `AETHYRFORGE_HOST_RESPONSE_URL`.
   It loads native candidate responses at the real host origin to exercise
   the exact origin protocol, then checks movement, fullscreen, checkpoint
   reload, renderer disposal, back navigation, and unavailable releases.
6. Shift with `scripts/deploy/web.sh shift REVISION`; rerun the browser check
   without candidate mode against the real public URL. Check `/`, health,
   script/Wasm MIME, CSP, denied source paths, and the serving revision.

## Rollback and evidence

Promote the previous tested release JSON to select its immutable engine URLs.
A website rollback uses `scripts/deploy/web.sh rollback REVISION`; check that
it cannot overwrite a newer unrelated deployment. Do not remove release tags
or asset sets while sessions can still refer to them.

Keep source revisions, image digests, manifest hashes, and signed-out browser
results with each release. Licensed scene captures stay in scratch storage,
not public Git. See `aethyrforge-medieval-deployment.json` for the deployment
record and `aethyrforge-host-protocol.md` for the public contract.

The Haunted House review route `/new-lander/house` uses its own pinned release
in `crates/openagents-web/content/demos/haunted-house.json`;
`OPENAGENTS_HAUNT_RELEASE` selects an explicit override. Build the native
consumer with `cargo build --locked -p openagents-web-site --bin openagents-web`.
Run `scripts/check-haunted-house.mjs` with the same candidate environment as
the town check. The house adds map return, walking mode, and an honest unsaved
status. Its [qualification record](haunted-house-qualification.json) records the
isolated release; it does not authorize a homepage or production traffic shift.

The first-planet tutorial uses this minimum completion rule: the authenticated
person currently owns the house project, personally published a release that
passes the native playable-content contract, chooses **Play** or **Share** for
that release, and returns to the universe map. The map records the return and
offers explicit planet creation with finite capacity. This is publication,
choice, and return evidence; it does not attest to server-observed engine
Compile or Inspect actions. A recipient visit, guest, member without ownership,
or local completion flag cannot allocate a planet. Allocation uses one durable
account-project creation receipt per person and tutorial version. Temporary
membership or storage failures preserve progress and require retry; deletion
keeps the receipt and does not resurrect a removed project.
