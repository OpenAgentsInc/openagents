# Browser release deployment

These tools package built browser artifacts from `apps/aethyr`; purchased asset
exports stay outside the OpenAgents repository. The current public demo
is read-only exploration. No account, wallet, model, command, or publishing token
is present in these containers or browser bundles.

1. Start from a clean revision on current `origin/main`. Build the public
   Rust/Wasm SDK from a clean public revision. Pin its Wasm SHA-256 and keep the
   matching generated JS beside it.
2. Run `python3 scripts/deploy/aethyr/package-assets.py SOURCE OUTPUT/private-assets`.
   Only `scene.json`, `scene-night.json`, and their recursive compiled runtime
   dependencies are copied. The parent directory receives `assets-manifest.json`.
   Source exports, unrelated debug files, and stale builds are excluded. For the
   house, append `haunted-house`; its accepted profile selects the dependency
   closure instead. Set `VITE_WORLD_PROFILE=haunted-house` for that build.
3. Pick a fresh release ID and never reuse its Cloud Run tag. Use separate editor
   and preview origins. Build with `VITE_PUBLIC_DEMO=1`, the configured
   `VITE_PREVIEW_URL`, `VITE_EDITOR_ORIGIN`, `VITE_OPENAGENTS_HOST_ORIGIN`,
   `VITE_OPENAGENTS_ENGINE_RELEASE`, `VITE_OPENAGENTS_ASSET_SET` for editable wire-v5 worlds, and the editor origin's immutable
   `/sdk/WASM_SHA256/aethyrforge_host.js` as `VITE_OPENAGENTS_PROTOCOL_URL`.
4. Run `npm run build` in `apps/aethyr`, then `scripts/deploy/aethyr/render.py --help` to prepare the two
   container contexts and the hash inventory in `release.json`. The renderer
   is built from `apps/aethyr`; the SDK inputs are the public SDK files.
   Contexts contain built JS/Wasm and notices, with no source maps or source tree.
5. Upload the compiled asset directory and manifest once to a private, uniform
   access, public-access-prevention bucket under `assetsets/ASSET_SET_SHA256/`.
   Do not overwrite that prefix. Build each context with `scripts/deploy/aethyr/cloudbuild.yaml`
   on the project's authorized Cloud Build service account, not GitHub automation.
6. Deploy each image **by digest** with `--use-http2 --no-traffic` and a fresh
   release tag. Apply these flags to both editor and preview services. Keep prior tags.
   The editor runtime account has no project roles. Only the preview account
   reads the compiled-assets bucket. Mount it read-only at `/runtime`, using
   `only-dir=assetsets/ASSET_SET_SHA256`. The installed CLI uses a named volume:

   ```sh
   gcloud run deploy aethyrforge-editor --image EDITOR_IMAGE_DIGEST \
     --use-http2 --no-traffic --tag RELEASE_ID
   gcloud run deploy aethyrforge-preview --image PREVIEW_IMAGE_DIGEST \
     --use-http2 --no-traffic --tag RELEASE_ID \
     --add-volume name=runtime,type=cloud-storage,bucket=BUCKET,readonly=true,mount-options=only-dir=assetsets/ASSET_SET_SHA256 \
     --add-volume-mount volume=runtime,mount-path=/runtime
   ```

   The generated Nginx configuration accepts cleartext HTTP/2 (`h2c`) on port
   8080. Cloud Run terminates TLS and requires both container support and
   `--use-http2` for [HTTP/2 through to the container](https://docs.cloud.google.com/run/docs/configuring/http2).
   Without that setting, Cloud Run sends HTTP/1 to the container. Its
   [32 MiB HTTP/1 response limit](https://docs.cloud.google.com/run/quotas#request_limits_for_cloud_run)
   rejects larger static resources, including the town's 45.68 MB PNG, when
   served without chunked transfer or streaming. Keep the asset bytes unchanged
   and use HTTP/2 for these releases. Confirm the candidate serves its largest
   runtime resource successfully before shifting traffic.

   Both services scale from zero to at most two instances and
   use the generated CSP, exact host CORS, MIME types, immutable caching, and
   sandboxed preview. The preview receives no general application credentials.
7. Publish the approved editor/SDK URLs and compatibility identities through the
   public Rust host configuration. OpenAgents' promotion script accepts
   `WEB_MEDIEVAL_RELEASE_JSON` as a tested override of the compiled default;
   switching back to the prior JSON selects the previous release without a rebuild.
   Deploy a no-traffic candidate, check it in an isolated browser, then shift traffic.
8. Check ready/open, movement, frame continuity, checkpoint reload, close, denied
   raw/source-map paths, headers and network failures. Keep a player on the prior
   tag while deploying a newer release. Verify the new host selection and rollback
   before removing any old release. Retain the manifests, source revisions, image
   digests, browser results, and deployed revision identities with the release.

Run GCP commands using the automation account's isolated `CLOUDSDK_CONFIG`.
Runtime browser artifacts are intentionally inspectable by visitors; their delivery
is not a source license. Never upload raw vendor source directories or secrets.

## Published character release, 2026-10-23

[Medieval Town](https://openagents.com/demo/medieval-town) previously selected immutable
character release `r308ca999c8bbc714c5`. The public surface shows the animated Everglade/Grove
player, first-/third-person controls, and movement hints without editor panels.

| Component | Recorded identity |
| --- | --- |
| Engine source | `e20bd2a6f9a4fc059bb00f46b28d477ba266853a` |
| Public SDK source / protocol | `a77d7aac2304fa050d946df33af77c3f6bf62d8b` / version 7 |
| Host SDK Wasm SHA-256 | `c3aa78cc16e49da94d8719b16d0460fad9569efce2be4e501120ebf04bcffef8` |
| Compiled asset set | `86a1d3bea3b8e5849b887c3412f497c6e53a8aba79a76446e76f43394c570f9c` |
| Editor revision / image digest | `aethyrforge-editor-00016-zot` / `sha256:62175b45a4cc8c949e83c9e072bc2f3b363012956e5b1032e22637d526eed5de` |
| Preview revision / image digest | `aethyrforge-preview-00016-jem` / `sha256:00e0cb031162667156fcd5e2125cf970f9b84650b1e778a5da1911319e7dc4e7` |
| Public host revision | `coder-web-b3afdd3d3b-20261023170558`, 100% traffic |
| Unchanged host image digest | `sha256:b3afdd3d3b470400f0ea1d46057589588007136806120546ecd5994e2f113637` |

Both runtime services use h2c. The 45,680,260-byte PNG that failed under the
previous HTTP/1 container configuration now downloads with its expected hash;
the source art and compiled asset set were not changed for this fix. The tagged
candidate and the real public route each pass nine browser checks covering
loading, first-/third-person controls, zoom, movement, checkpoint reload, worker
disposal, and returning to the world. All 41 HTTP audit checks pass. In isolated
Chrome 155 at 1280 × 720,
the production route became ready after 35.17 seconds and submitted 44 frames
over 3.04 seconds, about 14.5 fps. This single-machine observation is not GPU
execution timing or frame-rate qualification; performance work remains.

The prior host revision is `coder-web-b3afdd3d3b-20261023043410`; its selected
release is `rd4a6dcbc64ecc48e3d`. The private release evidence retains that
selection in `rollback-host-release.json`, alongside the new release manifest,
image digests, asset inventory, and browser reports. Keep both immutable release
tags available for rollback.

## Published Druid destruction release, 2026-10-23

**Historical release.** The initial Druid release, `d97b1a501684c175224`, added
Meteor Shower (1), Lightning Bolt (2), and Thunderbolt (3) to
[Medieval Town](https://openagents.com/demo/medieval-town). These abilities use
local Rust/Wasm damage, support collapse, owned collision, and textured WebGPU
fragments. Aim and click or press Enter; **Restore town** clears local play damage.
Authored scene revisions stay unchanged. The identities and results below record
that release at publication.

| Component | Recorded identity |
| --- | --- |
| Engine source | `97b1a501684cf5e6b60ecb35e490c6af076fd5cc` |
| Public SDK source / protocol | `a77d7aac2304fa050d946df33af77c3f6bf62d8b` / version 7, unchanged |
| Host SDK Wasm SHA-256 | `c3aa78cc16e49da94d8719b16d0460fad9569efce2be4e501120ebf04bcffef8` |
| Compiled asset set | `b4dda1271c9aea2e7d0f597cb9a01b922feb42e409747a76877e8e09fecacf64` |
| Editor revision / image digest | `aethyrforge-editor-00022-rek` / `sha256:9bb991cb0273dfb5d35e49fef64aed3f23ba1a88e24590df62ac0c45da6303c1` |
| Preview revision / image digest | `aethyrforge-preview-00022-lig` / `sha256:6c8da6e7ee228ddcb99fb236e35752251ba02bc6aa4b736e5ed17042f4b2756d` |
| Public host revision | `coder-web-b3afdd3d3b-20261023175314`, 100% traffic |
| Unchanged host image digest | `sha256:b3afdd3d3b470400f0ea1d46057589588007136806120546ecd5994e2f113637` |
| Prior host / release for rollback | `coder-web-b3afdd3d3b-20261023170558` / `r308ca999c8bbc714c5` |

Both runtime services retain h2c, their previous tags, and zero default traffic for
the new revisions; the host selects their immutable release URLs. The host spec
changes only `OPENAGENTS_MEDIEVAL_RELEASE`. Its images, sidecar, command arguments,
ports, mounts, and remaining environment are identical to the previous serving
revision. The preview mounts only this compiled asset set, read-only. The editor
has no runtime-asset mount and returns 404 for its private-asset path. The 27
read-only deployment checks pass. Most assets reuse existing immutable bytes;
only changed prepared manifests and collision/cell buffers were uploaded.

The 46 Rust kernel tests, 553 TypeScript/runtime tests, type check, and spec check
pass. Local, hosted-candidate, and real-production browsers each pass 11 Druid
checks: effects, textured debris, surviving cells, broken-wall rays, real capsule
traversal, movement while casting, failed-shader retention, and restoration. A wall
blocks the intact walking route at z=30.76; the cleared opening admits the same
route. Settled rubble remains solid until its 20-second lifetime ends. Live Meteor
Shower reaches 256 fragments without overflowing its geometry allocation.

In isolated Chrome 155 at 1280 × 720, the public route became ready after 43.69
seconds. Its short restored-arch sample submits 129 frames over 2.20 seconds,
about 58.5 fps. This excludes GPU execution timing, is not a whole-town benchmark,
and does not resolve slow asset startup. The separate nine-check production character
regression also passes model loading, real perspective controls, zoom, movement,
checkpoint reload, worker disposal, and returning to the world. Its third-person
starting view submits 87 frames over 3.04 seconds, about 28.6 fps; frame rate varies
with the view. The public health, docs, installer,
OpenAPI, demo selection, and SDK boot checks pass. Private reports and licensed
captures remain outside Git; the private release inventory retains the exact
source, SDK, asset, build, and image identities. Keep the previous release tags
available for rollback.
For the opt-in entry/full-town package, use the [cell cooking guide](../../../docs/aethyr/engine/cell-streaming.md).
It extends this dependency closure and keeps the original full-quality scene.

## Published castle barrage release, 2026-10-23

The first castle release at [Medieval Town](https://openagents.com/demo/medieval-town),
`c1f78f2b939bb184438`, adds **Destroy the Castle**. Its compiled castle
group includes the stone structures and their attached banners and props, so
fixtures fall with the castle. The barrage targets surviving castle cells until
none remain, while movement continues.

1. Press `4` or select **Destroy the Castle** to target the distant castle.
2. Choose **Cast** or press `Enter` to start the sustained meteor and lightning
   barrage. You can keep walking, jumping, and changing perspective.
3. Choose **Stop barrage** or press `Esc` to stop new strikes. Strikes already
   falling still land.
4. Choose **Restore town** to clear the damage and rebuild the castle.

| Component | Recorded identity |
| --- | --- |
| Engine source | `1f78f2b939bbfeedd26fe2252139827990faa288` |
| Simulation kernel SHA-256 | `4d6e2ffd953f171100d05108284ab720243eae38ccd0e66e582ebf62ef3223f8` |
| Public SDK source / protocol | `a77d7aac2304fa050d946df33af77c3f6bf62d8b` / version 7, unchanged |
| Host SDK Wasm SHA-256 | `c3aa78cc16e49da94d8719b16d0460fad9569efce2be4e501120ebf04bcffef8` |
| Compiled asset set | `89c7d01d68662df61adf514350d3d7bb9ff2fade40487708b76ea226df8e700b` |
| Castle group | 6,954 cells / 23 sources by day; 6,834 cells / 21 sources by night |
| Editor revision / image digest | `aethyrforge-editor-00027-nac` / `sha256:91e7f6a198327fee681726c026373fe34a4d2a1cc0d2f705d38b53645c60ce12` |
| Preview revision / image digest | `aethyrforge-preview-00027-fuf` / `sha256:5325a290808fe4aa6542c6a1f0a92bc2554486f4286723efc88a569c2357a723` |
| Public host revision | `coder-web-b3afdd3d3b-20261023184541`, 100% traffic |
| Unchanged host image digest | `sha256:b3afdd3d3b470400f0ea1d46057589588007136806120546ecd5994e2f113637` |
| Previous published host / release | `coder-web-b3afdd3d3b-20261023175314` / `d97b1a501684c175224` |

Local and hosted-candidate browser runs each pass 10 checks; the public route
passes seven focused checks. Exact-ID CPU/Wasm checks destroy
all 6,954 day cells and 6,834 night cells, including the attached fixtures, with no
damage outside the castle group. Completion takes 24.7 and 29.9 simulation
seconds; these durations do not measure browser startup or frame rate. The
runtime retains 128 visible, stationary castle fragments after moving debris
expires. The unchanged kernel passes 60 Rust tests; 603 TypeScript tests pass.
Private release evidence retains the full IDs, source and asset bindings, image
digests, and captures outside Git.

The 48 read-only deployment checks pass. The website image and sidecar stay the
same; only its Medieval Town release selection changes. Existing runtime tags
and percentage routing remain available, and the preview mounts the new compiled
asset set read-only. The old Druid selection remains available for rollback.

The public browser verifies automatic targeting without teleporting, complete
source removal, lasting textured rubble, cleared source collision, failed-shader
retention, and restoration without changing the scene revision. Local and
candidate runs also exercise immediate cancellation, the visible Stop control,
and real movement during the barrage. The focused production verifier waits for
queued camera-turn input before placing its fixture camera.

In isolated Chrome 155 at 1280 × 720, the public route becomes ready after 47.87
seconds. Its restored-castle view submits 116 frames over 2.01 seconds, about
57.8 fps. This short observation does not measure GPU execution or qualify
whole-world performance. Asset startup remains slow. Runtime fracture surfaces
use the source exterior geometry without newly generated interior caps.


## Published surface targeting and finer rubble release, 2026-10-23

[Medieval Town](https://openagents.com/demo/medieval-town) selects
`ccbccd92a2c6c194419`. Selected spells show a ring on walls and the ground.
Unlocked aiming follows the pointer; locked third-person aiming clears the avatar,
and first-person aiming uses the view center. Clicking preserves pointer lock.
Meteor placement stays near the chosen surface. The old body crosshairs and the
public host's top bar are removed.

Every building now produces finer textured fragments. A shared pool retains up
to 1,024 stationary pieces and 256 moving pieces. Castle rubble lands on the
visible mountain terrain. Transient asset failures retry the complete download,
including interrupted bodies and known length or hash checks. Closing the world
cancels its downloads.

| Component | Recorded identity |
| --- | --- |
| Engine source | `cbccd92a2c6c5aa0ef13ffdbcacbff8c900ae3b7` |
| Simulation kernel SHA-256 | `a6410504b0706dcc84e35209946c00ed405e7766f76ad23e0f2020f31e8ab782` |
| Public SDK source / protocol | `a77d7aac2304fa050d946df33af77c3f6bf62d8b` / version 7, unchanged |
| Host SDK Wasm SHA-256 | `c3aa78cc16e49da94d8719b16d0460fad9569efce2be4e501120ebf04bcffef8` |
| Compiled asset set | `cab03b068eb19f4e52c394e0358bc801cf1b21a8860bdd0477c8e1c9f45d9ab9` |
| Editor revision / image digest | `aethyrforge-editor-00036-xez` / `sha256:9bb8e883ae7f6f846668f41a8ce3af22db6a0aea5be86e10acc60a90c2edf95b` |
| Preview revision / image digest | `aethyrforge-preview-00036-jux` / `sha256:088700536a3e9d59a02a8fa1569c17508e6643dbe6b6e8b7b283df7252533868` |
| Native host source | `ed398ef895fac3b6afa25b9265209b46cc0a6e98` |
| Public host revision | `coder-web-bf4641ba3a-20261023194511` |
| Host image digest | `sha256:bf4641ba3a104fbe9a340ae5167245c7934717734067a6eb63b4d6d0acd181ff` |
| Rollback host / release | `coder-web-b3afdd3d3b-20261023184541` / `c1f78f2b939bb184438` |

The candidate passes nine targeting and startup checks and ten full castle checks.
The startup test interrupts the first source-normal geometry request, observes a
visible retry message, and requires a completed response and a working world.
The targeting tests check real world-ring pixels, both camera modes, cursor
projection, spell impacts, preserved player position, and invalid sky rejection.
The castle test covers cancellation, movement, complete destruction, lasting
textured rubble, failed-shader retention, and restoration. All 50 deployment
checks pass before promotion. The kernel passes 65 Rust tests; 697 TypeScript and
JavaScript tests, type checking, and the spec check pass. The native header change
passes its two targeted Rust tests.

Exact-ID Wasm proofs destroy 6,954 day castle cells and 6,834 night cells without
outside-group damage. Each retains 1,024 nonempty fragments on visible terrain
after the debris lifetime. Ordinary Lightning Bolt, Thunderbolt, and Meteor
Shower retain 65, 620, and 1,024 fragments in their source-bound probes. Pieces
have half the previous width. These are bounded tests, not whole-world
performance qualification. New fracture interiors remain uncapped, and asset
startup still needs improvement. Licensed captures and raw IDs remain in private
release evidence outside Git.


On the public route, all 15 ordinary-spell checks and all nine targeting/startup
checks pass. These include real movement through a broken wall, persistent rubble
for all three spells, both camera perspectives, and automatic recovery from an
intentionally interrupted source-normal download. No unexpected browser errors
or failed asset requests remain in either run. The ordinary-spell cold start
reaches ready after 43.99 seconds in isolated Chrome 155 at 1280 × 720. Its short
restored-arch sample submits 142 frames over 2.20 seconds. This is a view-specific
observation without GPU execution timing, not a whole-town performance claim.


After the listener handoff, all 33 public-route and service checks pass. The new
host receives 100% of production traffic, prior unrelated tags remain available,
and temporary drain tags are removed. The gateway sidecar and host configuration
stay unchanged apart from the verified web image and Medieval Town selection.


## Published knight patrols release, 2026-10-23

[Medieval Town](https://openagents.com/demo/medieval-town) selects
`c149b1fa675c1201903`. Eight armored knights run through the town square and
nearby streets. Meteor Shower, Lightning Bolt, and Thunderbolt kill them on
impact; cover blocks damage. The original death animation plays once and holds.
**Restore town** revives them and clears building damage. The existing targeting
ring can pick a living knight directly.

| Component | Recorded identity |
| --- | --- |
| Engine source | `149b1fa675c1b723a781d126efc540b467276eb2` |
| Simulation kernel SHA-256 | `01d81a3ff0a37e8e49484ca549761e23dc90b0aa4a976b08b84cef67a28784e4` |
| Compiled asset set | `fe8309dd23e55b8e3ed7a0943792c47813e5f5d83b648e0efc5e2560c78f5484` |
| Knight manifest SHA-256 | `78b97171cb9bad788dbdad067572a0ab74be5115364a9683ce3ad56d25b06020` |
| Patrol manifest SHA-256 | `0a9ce26842ee48d313efff0f6a9b834b93db39ff709ebbcd17036145f2c92841` |
| Editor revision / image digest | `aethyrforge-editor-00037-bap` / `sha256:5800dae506901d76c0851a73b748e23e9d75561e3a1b70c987a375873322dda5` |
| Preview revision / image digest | `aethyrforge-preview-00037-jot` / `sha256:8274587fece609d1bb5ba577e8d52c35036dcfa891612dbd20d1910a0edb8477` |
| Public host revision | `coder-web-bf4641ba3a-20261023202058` |
| Unchanged host image digest | `sha256:bf4641ba3a104fbe9a340ae5167245c7934717734067a6eb63b4d6d0acd181ff` |
| Rollback host / release | `coder-web-bf4641ba3a-20261023194511` / `ccbccd92a2c6c194419` |

The public SDK remains protocol version 7 at source
`a77d7aac2304fa050d946df33af77c3f6bf62d8b`; its Wasm hash remains
`c3aa78cc16e49da94d8719b16d0460fad9569efce2be4e501120ebf04bcffef8`.
The asset publication reuses 463 existing files and uploads seven changed or new
files. The native host image and gateway sidecar stay unchanged; the host selects
the new immutable editor and preview.

The kernel passes 74 Rust tests; 709 JavaScript/TypeScript tests, type checking,
seven Knight compiler tests, and the spec check pass. Local and hosted-candidate
browsers each pass nine checks: actual animated patrols, missed shots, intact-wall
cover, Lightning deaths, stopped corpses, Meteor deaths, shader failure retention,
Restore town, and night movement. All 50 deployment checks pass before promotion.
Actual day/night CPU proofs also run every patrol for 61 simulated seconds and
commit a real six-cell Lightning forecast after a moving-target preparation gap.

The knight is the original CC0 KayKit model at source commit
`672074b73ba276876a19e8816ecdc5241817ab47`. Its private preparation keeps the original
run/death clips and records source rights; no source art or licensed captures
enter Git. The current patrols are bounded local NPC behavior, without shared
multiplayer state. CPU timing and skinning observations remain separate from
browser frame-rate qualification.


The public route also passes all nine combat checks, with no unexpected browser
errors or failed asset requests. The isolated browser verifies real running
models, misses and cover, actual Lightning and Meteor deaths, held death poses,
shader failure retention, restoration, and night movement. Raw actor poses,
source bindings, CPU timings, and licensed captures remain in private release
evidence outside Git.


After the listener handoff, all 33 public-route and service checks pass. The new
host receives 100% of production traffic, previous unrelated tags remain, and
no temporary drain tags remain. The previous host/release above is retained for
rollback.
