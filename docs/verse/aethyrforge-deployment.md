# Aethyrforge browser deployment

The Kiki routes embed pinned engine releases: the universe map at
`/new-lander/verse`, the Haunted House at `/new-lander/house`, personal
planets, and the Medieval Town demo at `/demo/medieval-town`. The website is
the Rust host in `crates/openagents-web`; it embeds an engine release through
the host protocol in `crates/aethyrforge-host`. Since the Aethyr merge
([plan](../aethyr/merge.md), [provenance](../aethyr/provenance.md)), the
engine is built from this repository too: the web package `apps/aethyr`, the
Rust workspace `crates/aethyr`, and the deploy tools in
`scripts/deploy/aethyr/`. One openagents commit now produces every part of a
release. The root homepage is unchanged.

## Source and artifact boundary

- The repository builds and tests without private package credentials. Pins
  in `crates/openagents-web/content/demos/` hold URLs and compatibility
  values only.
- Engine containers hold compiled browser JS/Wasm and dependency notices.
  Visitors can inspect those bytes; their delivery is not a source license.
  Source maps, the source tree, secrets, and purchased source exports are
  never in a container.
- Licensed assets are never in Git ([private assets](private-assets.md)).
  Each release's compiled scene closure lives in a private bucket with
  uniform access and public access prevention, under
  `assetsets/ASSET_SET_SHA256/`. Only the preview runtime account reads it.
  The editor account has no project roles. The browser receives no account,
  wallet, model, or publishing credentials.
- Editor and preview use separate HTTPS origins, sandboxed frames, exact host
  CORS and frame ancestors, and release-specific immutable URLs. Camera,
  microphone, geolocation, and payment are disabled.

## Release from one commit

`scripts/deploy/aethyr/release.sh` builds, stages, checks, and pins one
release from one pushed openagents commit. Each step reads and extends the
release directory the build writes (default
`~/work/aethyr-releases/COMMIT10`, outside the checkout).

```sh
scripts/deploy/aethyr/release.sh build origin/main   # local; no GCP
scripts/deploy/aethyr/release.sh stage DIR           # images, no-traffic tags
scripts/deploy/aethyr/release.sh check DIR           # candidate browser checks
scripts/deploy/aethyr/release.sh pin DIR             # new pins; old ones kept
```

`DRY_RUN=1` prints every `gcloud` write instead of making it. Reads still
run; when a read is refused, a placeholder stands in for its answer.

### Profiles

One build makes four releases, one per profile. `AETHYR_PROFILES` selects a
subset.

| Profile | Route | Asset closure | Pins |
| --- | --- | --- | --- |
| `universe` | `/new-lander/verse` | none | `demos/universe.json` |
| `house` | `/new-lander/house` | `haunt-profile.json` | `demos/haunted-house.json`, `demos/runtimes/haunted-house.json` |
| `planet` | personal planets | `planet-profile.json` | `demos/runtimes/personal-planet.json` |
| `town` | `/demo/medieval-town` | `scene.json`, `scene-night.json` | `demos/medieval-town.json` |

A profile's release ID and Cloud Run tag are `a`, the profile letter
(`u`, `h`, `p`, `m`), the commit's first ten hex digits, and the optional
`AETHYR_RELEASE_SUFFIX`; for example, `ah9b4e07506e`. Tags are never reused:
`stage` refuses a tag either service already has, so rebuild a commit with a
new suffix.

### Prerequisites

- A pushed commit. Pins come only from commits on `origin/main`;
  `AETHYR_ALLOW_BRANCH=1` builds a pushed branch commit as a candidate that
  `pin` refuses.
- Node 22 and npm; Rust 1.97.1 (the root toolchain) and 1.98.1
  (`crates/aethyr`), both with `wasm32-unknown-unknown`; `wasm-bindgen-cli`
  at the `Cargo.lock` version; Binaryen 132 (`wasm-opt`, or
  `AETHYRFORGE_WASM_OPT`); and a clang with the WebAssembly target for Kiln
  (`CC_wasm32_unknown_unknown` overrides the search).
- The compiled private asset source in `AETHYR_PRIVATE_SOURCE` (default
  `~/.openagents/verse/private/aethyrforge`) for every profile except
  `universe`.
- For `stage`, the automation account's gcloud configuration
  (`CLOUDSDK_CONFIG`, default `~/work/.secrets/gcloud-sa-config`). The script
  checks that its active account is a service account and never falls back to
  a person's login ([automation account](../deployment/automation-account.md)).
- For `check`, the `openagents` CLI (each check runs in its own browser
  through `openagents browser run`) and Playwright.

### Build

`build` makes no GCP call and runs on any machine with the toolchains:

1. Checks the commit out into a detached worktree, `DIR/src`, so no local
   edit reaches the release.
2. Builds the host SDK with Kiln from `crates/aethyrforge-host`
   (`scripts/build-aethyrforge-host.sh`). The SDK's identity is the SHA-256 of
   `aethyrforge_host_bg.wasm`.
3. Builds `kernel.wasm` from `crates/aethyr` (`apps/aethyr/tools/build-kernel.sh`)
   after `npm ci`, and type-checks `apps/aethyr`.
4. For each profile, packages the asset closure with `package-assets.py`,
   runs one Vite build with that profile's environment, and runs `render.py`
   to make the editor and preview container contexts with their Nginx
   configuration, CSP, notices, and `release.json` hash inventory.
5. Writes `build.json` (every hash, the toolchain versions, and each context's
   tree hash) and `candidates/`, the pin JSON each profile would publish.

The SDK is linked at build time. The script sets
`VITE_OPENAGENTS_PROTOCOL_URL` to `/sdk/SDK_SHA256/aethyrforge_host.js` on the
release's own editor origin, using the SDK it built in step 2, and refuses a
bundle that names any other SDK. `render.py` places those exact files at that
path. There is no operator-pinned cross-origin SDK URL to supply any more. The
host protocol stays the security boundary between the website and the engine.
Releases pinned before this change keep their own SDK URLs and tags, and
still load.

The Vite environment for each profile:

| Variable | Value |
| --- | --- |
| `VITE_PUBLIC_DEMO` | `1` |
| `VITE_PREVIEW_URL` | `PREVIEW/preview.html` |
| `VITE_EDITOR_ORIGIN` | `EDITOR` |
| `VITE_OPENAGENTS_HOST_ORIGIN` | `https://openagents.com` |
| `VITE_OPENAGENTS_ENGINE_RELEASE` | the release ID |
| `VITE_OPENAGENTS_ASSET_SET` | the asset set SHA-256 |
| `VITE_OPENAGENTS_PROTOCOL_URL` | `EDITOR/sdk/SDK_SHA256/aethyrforge_host.js` |
| `VITE_WORLD_PROFILE` | `haunted-house` (house), `personal-planet` (planet), unset otherwise |

`EDITOR` and `PREVIEW` are the release's tag URLs,
`https://TAG---aethyrforge-editor-ezxz4mgdsq-uc.a.run.app` and
`https://TAG---aethyrforge-preview-ezxz4mgdsq-uc.a.run.app`.

`engine_artifact_sha256` in a publishing runtime is the SHA-256 of the
profile's release manifest, the `release.json` that `render.py` writes. The
manifest embeds the SDK URL, so the value changes whenever the SDK does, even
for the same engine release; earlier pins follow the same rule.

The publishing runtimes (`demos/runtimes/*.json`) also name the scene, its
artifacts, and its materials. When a profile's asset set is unchanged, the
candidate carries them over. When the asset set changed, `check` and `pin`
need the reviewed runtime JSON in `AETHYR_RUNTIME_HOUSE` or
`AETHYR_RUNTIME_PLANET`; the script checks its identity fields and that every
artifact it lists is in the new asset set.

### Stage

`stage` deploys the recorded bytes and nothing else. It first rehashes every
context against `build.json`. Then, for each profile:

1. Uploads the asset set once to the bucket the live `aethyrforge-preview`
   service already mounts, `openagentsgemini-aethyrforge-releases`
   (`AETHYR_ASSET_BUCKET` overrides it). An existing
   `assetsets/ASSET_SET_SHA256/assets-manifest.json` means the set is already
   there; nothing under that prefix is overwritten (`--no-clobber`), and the
   manifest is written last.
2. Builds the editor and preview contexts with
   `scripts/deploy/aethyr/cloudbuild.yaml` on the automation account, not
   GitHub automation, into
   `us-central1-docker.pkg.dev/openagentsgemini/openagents/aethyrforge-ROLE:TAG`.
3. Deploys each image by digest, keeping prior tags:

   ```sh
   gcloud run deploy aethyrforge-editor --image EDITOR_IMAGE@DIGEST \
     --use-http2 --no-traffic --tag TAG
   gcloud run deploy aethyrforge-preview --image PREVIEW_IMAGE@DIGEST \
     --use-http2 --no-traffic --tag TAG \
     --add-volume name=runtime,type=cloud-storage,bucket=BUCKET,readonly=true,mount-options=only-dir=assetsets/ASSET_SET_SHA256 \
     --add-volume-mount volume=runtime,mount-path=/runtime
   ```

   The preview service already has the read-only gcsfuse volume `runtime` at
   `/runtime`. Every release has been deployed with these same flags against
   it, which point the volume at the new asset set; no `--remove-volume` is
   needed.

4. Checks that both tag URLs answer `/health` with the release's
   `X-Aethyrforge-Release` header over HTTP/2, that the preview serves its
   largest runtime file with the recorded hash, and that the editor does not
   serve private assets.

It writes `stage.json` with each image digest and revision. The engine
services keep no default traffic for these revisions: the pins name tag URLs,
so a new release never moves traffic for an old one.

The generated Nginx configuration accepts cleartext HTTP/2 (`h2c`) on port
8080. Cloud Run terminates TLS and needs both container support and
`--use-http2` for [HTTP/2 through to the container](https://docs.cloud.google.com/run/docs/configuring/http2).
Without it, Cloud Run sends HTTP/1, and its
[32 MiB HTTP/1 response limit](https://docs.cloud.google.com/run/quotas#request_limits_for_cloud_run)
rejects larger static files, such as the town's 45.68 MB PNG. Both services
scale from zero to at most two instances and use the generated CSP, exact host
CORS, MIME types, immutable caching, and the sandboxed preview.

### Check

`check` builds the native website consumer
(`openagents-web`) from the release's worktree and starts it with the
candidate pins (`OPENAGENTS_UNIVERSE_RELEASE`, `OPENAGENTS_HAUNT_RELEASE`,
`OPENAGENTS_MEDIEVAL_RELEASE`, `OPENAGENTS_VERSE_PUBLISH_RUNTIME`,
`OPENAGENTS_VERSE_PERSONAL_RUNTIME`). It then runs, in candidate mode against
the tag URLs, `scripts/check-medieval-town.mjs`,
`scripts/check-haunted-house.mjs`, `scripts/check-verse-project.mjs`,
`-publish`, `-material`, `-access`, `-voice`, `-workspace`, and
`-universe`. Candidate mode loads the host's responses at the real host origin
to exercise the exact origin protocol. To check against a website candidate
instead, set `AETHYRFORGE_HOST_RESPONSE_URL` to its tag URL. Results go to
`checks.json` and each check's output to `DIR/checks/NAME/`. Any failure
stops the release.

### Pin and ship

`pin` refuses a release whose checks did not all pass or whose commit is not
on `origin/main`. It copies the current pins to
`crates/openagents-web/content/demos/rollback/`, writes the candidates in their
place, and writes the release's qualification entry (below). It commits
nothing. Commit those files and ship them as one website release with
`scripts/deploy/web.sh`: stage, `promote`, and `shift`. Then rerun the browser
checks without candidate mode against the public routes, and check `/`,
health, script and Wasm MIME types, CSP, denied source paths, and the serving
revision.

## Rollback and evidence

The previous pins are in `demos/rollback/`. To roll back without a rebuild,
copy them back and ship the website, or promote with
`WEB_MEDIEVAL_RELEASE_JSON` set to the previous town pin. A website rollback
uses `scripts/deploy/web.sh rollback REVISION`; check that it can't overwrite
a newer unrelated deployment. Don't remove release tags or asset sets while
sessions can still refer to them.

The release directory keeps `build.json`, `stage.json`, `checks.json`, and the
browser output. Licensed scene captures stay in scratch storage, not in Git.
`aethyrforge-host-protocol.md` is the public host contract.

## Qualification entries

Each release pinned from this repository gets one new file,
`docs/verse/aethyr-releases/COMMIT10.json`, that `pin` writes. Existing
qualification records (`aethyrforge-*-qualification.json`,
`haunted-house-qualification.json`, and `aethyrforge-medieval-deployment.json`)
describe Aethyrforge-built releases and are not edited.

```json
{
  "schema": "openagents.aethyr.release-qualification.v1",
  "issue": 12590,
  "commit": "FULL_OPENAGENTS_COMMIT",
  "protocol_version": 7,
  "toolchain": { "node": "...", "rustc_root": "...", "rustc_aethyr": "...", "wasm_bindgen": "...", "wasm_opt": "..." },
  "sdk": { "wasm_sha256": "...", "closure_sha256": "..." },
  "kernel_sha256": "...",
  "profiles": {
    "house": {
      "release": "ahCOMMIT10",
      "editor": "https://ahCOMMIT10---aethyrforge-editor-ezxz4mgdsq-uc.a.run.app",
      "preview": "https://ahCOMMIT10---aethyrforge-preview-ezxz4mgdsq-uc.a.run.app",
      "asset_set_sha256": "...", "asset_files": 0, "asset_bytes": 0,
      "release_json_sha256": "...",
      "context_sha256": { "editor": "...", "preview": "..." },
      "editor": { "image_digest": "sha256:...", "revision": "aethyrforge-editor-NNNNN-xxx" },
      "preview": { "image_digest": "sha256:...", "revision": "aethyrforge-preview-NNNNN-xxx" }
    }
  },
  "candidate_checks": [{ "profile": "house", "check": "check-haunted-house", "result": "pass" }],
  "pins": ["haunted-house.json", "runtimes/haunted-house.json"],
  "rollback": ["rollback/haunted-house.json", "rollback/runtimes/haunted-house.json"],
  "production_checks": "pending: ..."
}
```

The one `commit` field names every source: SDK, kernel, bundles, render
scripts, and pins. After the website shift, add the production check results
to the entry in the same change that records them; never edit an entry for a
later release.

## Haunted House and the first-planet tutorial

The Haunted House route `/new-lander/house` uses its own pin,
`demos/haunted-house.json`; `OPENAGENTS_HAUNT_RELEASE` selects an explicit
override. Build the native consumer with `cargo build --locked -p
openagents-web-site --bin openagents-web`. The house adds map return, walking
mode, and an honest unsaved status. Its
[qualification record](haunted-house-qualification.json) records the isolated
Aethyrforge release; it does not authorize a homepage or production traffic
shift.

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
A pre-award `404` from project creation can mean that an older account service
lacks the ensure endpoint. The host reports creation unavailable and keeps the
tutorial ready; it never substitutes ordinary project creation. Only a missing
project for an already-recorded planet is reported as removed.

## Aethyrforge-built releases

**Historical.** The releases below were built in the Aethyrforge repository
before the merge, with its `tools/deploy/README.md` and an operator-pinned
SDK URL. Their records are kept as published. "Engine source" names an
Aethyrforge commit and "Public SDK source" an openagents commit.

### Published character release, 2026-10-23

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

### Published Druid destruction release, 2026-10-23

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
For the opt-in entry/full-town package, use the [cell cooking guide](../aethyr/engine/cell-streaming.md).
It extends this dependency closure and keeps the original full-quality scene.

### Published castle barrage release, 2026-10-23

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

### Published surface targeting and finer rubble release, 2026-10-23

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

### Published knight patrols release, 2026-10-23

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
