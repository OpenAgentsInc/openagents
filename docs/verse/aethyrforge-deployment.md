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
