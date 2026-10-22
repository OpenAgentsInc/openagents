# Kiln edits itself

Admin Kiln can change its own code and reload into the result in place
(#12441). An admin, or the agent, edits Kiln's Rust in the tab; the runner
tests it, builds the page module and tools with the fast staging settings,
checks the build in a headless browser, and publishes it over the host's
copy through livehost. Every open Kiln page notices the new build, tests it
in a worker, and reloads into it with its log and task, while the workspace
stays in OPFS.

## The loop

```text
 tab (admin Kiln)                     relay           kiln-runner                       livehost store + pointer
 ┌─────────────────────────────┐                      ┌─────────────────────────────┐   ┌───────────────────────┐
 │ Load Kiln's source           │                      │ admin list check            │   │                       │
 │  crates/kiln, kiln-tools     │  25900 sealed job    │ stage files at the commit   │   │                       │
 │ edit, then Publish Kiln ─────┼─▶ action: publish ──▶│ cargo test -p kiln -p ...   │   │                       │
 │  (signed by the admin key)   │                      │ build-kiln-web.sh, staging  │   │                       │
 │                              │ ◀─ progress lines ───│  settings, in the sandbox   │   │                       │
 │                              │                      │ shape + browser self-test   │   │                       │
 │                              │ ◀─ result + phases ──│ overlay modules/kiln/ ──────┼──▶│ objects, manifest,    │
 │ poll tools/manifest.json ◀───┼──────────── host serves modules/kiln/* first ──────┼───│ signed 30440 pointer  │
 │ new build: smoke in a worker │                      └─────────────────────────────┘   └───────────────────────┘
 │ save log + task, reload      │
 └─────────────────────────────┘
```

1. **Load Kiln's own source.** **Load Kiln's source** loads
   `crates/kiln` and `crates/kiln-tools` at the commit the **Ref** field
   names (default `main`; set it to the deployed commit). The files are
   fetched in parallel from `raw.githubusercontent.com`.
2. **Edit, then test.** The agent or the admin edits the Rust. `cargo test
   -p kiln -p kiln-tools` runs on the runner as any test does.
3. **Publish Kiln** sends an `action: publish` job with the changed files
   and the commit, sealed to the runner and signed by the browser's admin
   key. The runner:
   1. checks the signer against its admin list (`--admins FILE`, read
      again for every job), and that every file is inside `crates/kiln/` or
      `crates/kiln-tools/`, never a `Cargo.toml` or `build.rs`;
   2. checks out the commit in its worktree and writes the files;
   3. runs `cargo test -p kiln -p kiln-tools` in `coder-boundary`, and
      refuses to publish on any failure;
   4. builds with `scripts/build-kiln-web.sh` in the boundary, with the
      staging settings of `scripts/deploy/staging-build-env.sh` (no LTO,
      16 codegen units, incremental, the brotli, gzip, and `wasm-opt`
      shims), offline, into its own build directory;
   5. names the build (the first 12 hex digits of the module's SHA-256) in
      `tools/manifest.json`'s `kiln` field;
   6. checks the build's shape (every file the page needs, the page's
      exports, tools matching their hashes), then serves it locally and
      runs the page's sandbox self-test in a headless browser through
      `openagents browser run`, and refuses to publish if either fails;
   7. overlays the build as `modules/kiln/...` on the tenant's current
      manifest (`livehost::deploy::overlay`), uploads the new objects,
      and signs the pointer with its publishing key;
   8. records the publish in `STATE/kiln-history.json`.
4. **Serve.** A host that follows the tenant answers Kiln's files from
   `modules/kiln/<file>` in the manifest before its own copy: on the site,
   `openagents-web`'s `serve_live` for files under `/srv/kiln`; locally,
   `kiln-dev --live-*`. `index.html` stays the host's own.
5. **Hot reload.** Every open Kiln page polls `tools/manifest.json` every
   three seconds while visible. When the build name changes, it runs a tool
   in a fresh worker, which loads the new page module (a failure keeps the
   page where it is and says so), saves the log and the task in session
   storage, and reloads. After the reload it restores them and reports the
   time from the click and from the reload. If the agent is busy, the page
   shows **Reload into it now** and waits.
6. **Revert.** **Revert to last good** points the channel back at the
   manifest before the last publish. **Reset to the site's build** removes
   `modules/kiln/` from the manifest, so the host serves the build it
   shipped with. Both are admin jobs. A runner operator can do the same
   without a working page (see below).

## Who can publish

- The site serves `/admin/kiln/` to site admins only, and refuses Kiln's
  files to anyone else.
- The runner publishes only for keys on its admin list. The page keeps a
  dedicated **admin key** in the browser's local storage (separate from the
  throwaway key it uses for Pylons) and shows its `npub` under
  **Self-edit**; an operator adds that `npub` to the runner's admin file.
  A key that isn't listed gets "only an admin's key may publish, revert,
  or reset Kiln".
- The publishing key and the store's credentials stay on the runner. The
  page never holds them, and the job carries only files and a commit.
- **Next step, not built:** the site, which knows who is an admin, signs a
  short-lived attestation for the page's admin key (`admin until T`) with a
  site key the runner trusts, so the admin list needn't name every
  browser. That needs an endpoint in `openagents-web`.

## Run it locally

```sh
S="$(openagents scratch)"
(umask 077; python3 -c 'import secrets; print(secrets.token_hex(32))' > "$S/kiln-dev-tenant.key")
touch "$S/kiln-admins"; mkdir -p "$S/livehost-store"
kiln-runner --state "$S/kiln-runner" --jobs 6 --open --admins "$S/kiln-admins" \
  --publish-store "$S/livehost-store" --publish-key "$S/kiln-dev-tenant.key" \
  --publish-tenant kiln-dev --publish-channel staging
# The runner prints the publishing key's public half; follow it:
kiln-dev --dir "$CARGO_TARGET_DIR/kiln-web" --port 8787 \
  --runner "$(cat "$S/kiln-runner/pubkey")" --repo "$(cat "$S/kiln-runner/repository")" \
  --live-store "$S/livehost-store" --live-pubkey PUBLISHING_KEY \
  --live-tenant kiln-dev --live-channel staging
```

Open `http://127.0.0.1:8787/`, add the **Admin key** `npub` to
`$S/kiln-admins`, click **Load Kiln's source**, edit, and click **Publish
Kiln**. The pointer travels over `wss://relay.openagents.com` under the
throwaway tenant `kiln-dev`; the store is a local directory.

## On staging

The runner publishes to the `openagents` tenant's `staging` channel with
`--publish-tenant openagents --publish-channel staging
--publish-store gs://openagentsgemini-livehost --publish-key
~/work/.secrets/livehost-openagents-staging.key` (with the bucket's
`CLOUDSDK_CONFIG`). Staging's `openagents-web` follows that pointer and
serves `modules/kiln/...` from it. Ask the coordinator before doing this:
staging is shared, and a full content deploy (`scripts/deploy/livehost.sh
deploy`) builds the manifest from scratch, which drops a published Kiln
build until the next publish. Production follows `live`, which only
`livehost.sh promote` moves, with the owner's approval.

## When a build is broken

- A failing `cargo test`, build, shape check, or browser self-test never
  publishes.
- A page that finds the new build won't run a tool stays on the build it
  has.
- If a published build still breaks the page, any admin page that loads
  can **Revert** or **Reset**, and an operator can do it from the
  terminal: `livehost rollback --tenant TENANT --channel CHANNEL --store
  STORE --relay RELAY --key KEY` (to the parent), or re-point to a manifest
  from `STATE/kiln-history.json` with `--to HASH`.

## Measurements

Measured on 2026-10-21 on the owner's Mac, locally: `kiln-runner --jobs 6`,
`kiln-dev --live-*` following the throwaway `kiln-dev` tenant over
`wss://relay.openagents.com`, and a local directory store. Each run edits
one log line in `crates/kiln/src/web/app.rs` (about 80 KiB) through
`kilnWrite`, then `kilnSelfEdit('publish')`, in headless Chrome.

| Phase | Warm (same commit as the last publish) | After a new commit |
| --- | --- | --- |
| Stage: check out, clean the crates, apply the diff | 0.28 s | 2.1 s |
| `cargo test -p kiln -p kiln-tools` | 4.2 s | 7.3 s |
| Build: page module, tools, `wasm-bindgen`, light compression | 4.2 s | 11.6 s |
| Smoke: shape, then the page's self-test in headless Chrome | 0.85 s | 1.1 s |
| Publish: read the pointer, upload, sign the new pointer | 0.53 s | 1.0 s |
| Runner total | 10.0 s | 23.1 s |
| The page sees the build (3-second poll), tests it in a worker, reloads | about 1 s | about 2 s |
| **Click to reloaded, measured by the page** | **11.0 s** | **25.1 s** |

- **Reset** to the host's own build: 0.4 s on the runner, 2.9 s from the
  click to the reloaded page.
- **Revert** to the build before the last publish: 2.9 s from the click to
  the reloaded page.
- The first publish after the runner started cold took 72 s to build
  (every dependency for `wasm32-unknown-unknown` and `wasm32-wasip1`).
- The reload itself is about 0.1 s; the worker smoke test of the new build
  is about 20 ms.
- On staging, expect more for the upload (Cloud Storage instead of a local
  directory) and for the follower's pickup of the pointer.

## Checked locally

- Publish, warm and after a new commit, with the timings above.
- Reset and revert, each reloading every open page.
- A key not on the admin list: the runner answers "only an admin's key
  may publish, revert, or reset Kiln", and nothing changes.
- An edit that doesn't compile for the browser: the native tests pass, the
  build fails, and the runner answers "not published: the build failed".

Handled in code but not yet exercised in a run: a new page module under an
older host `index.html` (the site serves its own `index.html`, not the
published one). The module skips the controls the older page lacks.
