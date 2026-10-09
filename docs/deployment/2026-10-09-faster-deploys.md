# Faster web deploys: audit and plan

Recorded 2026-10-09. A planning document: how `crates/openagents-web` and the
staging stack are built and deployed today, where the time goes, and a phased
plan to cut a code deploy from about 15 minutes to under 3, and a content
change to under a minute with no Rust build.

All numbers come from Cloud Build and Cloud Run in `openagentsgemini`, read
with the automation account on 2026-10-09. Nothing was deployed or changed.

## Summary

- A staging web build takes **12 to 13 minutes** inside Cloud Build, plus
  about **1.5 minutes** of upload and **1 minute** of queue. Recent web
  builds ranged from 7 to 28 minutes; on 2026-10-02 they took 4 to 6.
- Three causes account for most of it:
  1. Docker's **legacy builder** runs the native stage and the WebAssembly
     stages one after another. They don't depend on each other.
  2. **Nothing is cached** between builds: every build downloads about 550
     crates twice, reinstalls apt packages three times, recompiles
     `wasm-bindgen-cli`, and compiles 603 crates from scratch.
  3. The web binary depends on the **`coder` crate** for four small modules,
     which pulls in 662 of its 735 dependencies, including `wasmtime`.
- Production **rebuilds** the image that staging already built and tested.
- Content (docs Markdown, legal text, CSS, JavaScript, images) is compiled
  into the binary with `include_str!`, so a typo fix needs a full build.

## 1. Today's pipeline

### Staging (`openagents-web-1-staging`)

From [deploy/staging/README.md](../../deploy/staging/README.md):

1. `gcloud builds submit` with `crates/openagents-web/cloudbuild.yaml` and
   `web.gcloudignore` (tag `stg-<sha>`).
2. In parallel, `gcloud builds submit` with `deploy/staging/cloudbuild.yaml`
   (the gateway and chat worker image, `openagents-stack:<sha>`).
3. Read both digests, run `deploy/staging/render.py`, then
   `gcloud run services replace`.
4. `scripts/smoke/staging.sh`.

### Production (service `coder`, container `web`)

From [openagents-web.md](openagents-web.md) and
[the 1.0 operations guide](../launch/1.0/operations.md):

1. Build the image again, usually from GitHub
   (`gcloud builds submit https://github.com/... --git-source-revision`).
2. `gcloud run services describe coder --format export`, edit the `web`
   image by hand (and quote `CODER_CHAT_SYNC: 'on'`), then
   `gcloud run services replace` as a no-traffic revision tagged `new`.
   The automation account is refused `actAs`, so `chris@` applies it.
3. Check the tag URL, then `update-traffic`.

### Timing, staging build `b2b67216` (commit `2a81ae3887`, E2_HIGHCPU_32)

| Phase | Time | Notes |
| --- | ---: | --- |
| Tar and upload the context | 86 s | 255 MiB compressed (620 MB uncompressed) from the Mac |
| Queue for a worker | 54 s | 17 to 177 s across 30 builds; median about 55 s |
| Fetch source, fetch kit packs | 24 s | 12 s each |
| Base image and apt (builder) | 21 s | |
| `COPY . .` | 12 s | 620 MB into a layer |
| Crate downloads (native) | 48 s | about 550 crates from crates.io |
| Third-party crates (native) | ~85 s | |
| Workspace crates (native) | ~90 s | 75 workspace crates; `coder`, `coder-host`, `tenancy`, `gym`, `pylon` |
| `openagents-web` crate and link | ~73 s | One crate, mostly Maud templates |
| Strip and commit the builder layer | 29 s | The whole `target/` is committed into a layer |
| apt (WebAssembly stage) | 20 s | clang, llvm, binaryen |
| Install `wasm-bindgen-cli` | 31 s | Compiled from source every build |
| `COPY . .` again | 12 s | |
| Everglade WebAssembly build | 173 s | 48 s crate downloads again, 89 s compile, gzip, layer commit |
| Component, Cloud, chat WebAssembly | 67 s | 60 s compile |
| Grow Little Bunny | 28 s | |
| Runtime stage | 14 s | |
| Push | 20 s | Image is 217 MB |
| **Build total (Cloud Build)** | **760 s** | 12.7 min |
| **Wall time, tar to pushed** | **~15 min** | 18:18:06 to 18:33:07 |
| `services replace` to healthy | ~15 s | Image import 4.4 s, containers healthy 4.7 s |
| Smoke suite | not measured | `web.py` prints its own duration |

The stack image (`5dbc4d25`) ran beside it: 87 s upload, 74 s queue, 372 s
build, of which 274 s is `cargo build` of 583 crates (64 workspace crates).
Most of those crates are the same ones the web build compiles.

### Trend across 50 builds (successful web builds only)

| Dates | Cloud Build time | Source |
| --- | ---: | --- |
| 2026-10-02 | 227 to 379 s | Upload |
| 2026-10-04 | 540 to 616 s | GitHub |
| 2026-10-05 to 10-08 | 347 to 1,549 s | Mostly GitHub |
| 2026-10-09 | 709 to 720 s | Upload |

- Fetching from GitHub took 55 to 252 s, compared with 10 to 13 s for an
  uploaded context; the upload itself takes about 86 s on the Mac.
- Failed builds take 6 to 7 minutes to fail (`a50987e5`, `6c0c73ab`).
- The same commit was built twice on 2026-10-08 (`88cb5f7599`) and twice on
  2026-10-06 (`0a15818d70`): staging and production builds are separate.
- Cost: E2_HIGHCPU_32 is roughly $0.06 a build-minute, so about $0.75 a web
  build. The money is small; the time is the problem.

### Compiled in, versus read at runtime

| What | How it reaches the site | Change needs |
| --- | --- | --- |
| Pages, layout, routes, headers, redirects | Maud and Rust in `openagents-web` | Rust build |
| Docs, `/docs/*` (over 40 Markdown files) | `include_str!` in `src/pages/content.rs`, list in `SECTIONS` | Rust build |
| Legal text (`/terms`, `/privacy`) | `include_str!` | Rust build |
| Agent docs and skill (`content/agents`, `content/skills`) | `include_str!` | Rust build |
| CSS, JavaScript, fonts CSS, htmx, favicon, images (`static/`, `static/bluerush/`) | `include_str!` and `include_bytes!` | Rust build |
| Installer scripts (`scripts/install/coder.sh`, `.ps1`) | `include_str!` | Rust build |
| Efficiency study rows (`/efficiency`) | `include_str!` in `coder::efficiency` | Rust build |
| Chat answer bank (`crates/coder/answers/chat-answers-v1.toml`) | `include_str!` in `coder::router::bank` | Rust build (web, stack, worker VM) |
| Chat goldens (`bench/web-chat/goldens-v1.json`) | `include_str!` in `coder::chat_goldens` | Rust build |
| Product knowledge (`knowledge/openagents/`) | Files, copied to `/srv/knowledge` in the stack image | Stack image rebuild (no Rust change, but the Dockerfile rebuilds anyway) |
| Everglade, components, Cloud, chat, Bunny WebAssembly | Files under `/srv/*`, passed by `--everglade`, `--components-build`, `--cloud-build`, `--chat-build`, `--bunny` | Image rebuild |
| Settings such as `CODER_VERSION`, pay host, upstream | Environment and flags in the service spec | New revision only (about 15 s) |

The site already serves several directories from disk at runtime. Docs,
static files, and legal text are the obvious next ones.

## 2. Options to cut Rust build time

Savings are for the 760 s staging web build above unless noted.

| Option | Expected saving | Cost | Risk |
| --- | --- | --- | --- |
| **BuildKit** (`DOCKER_BUILDKIT=1`, or `docker buildx build`) so the native and WebAssembly stages run in parallel | 760 s to about 420 to 480 s (the longer of the 358 s native and 335 s WebAssembly chains, plus runtime and push; the two share 32 cores) | One line in each `cloudbuild.yaml` | Low. Output is the same image |
| **Builder base image** in Artifact Registry with apt packages, `wasm32` target, `wasm-bindgen-cli`, and `cargo-chef` preinstalled | About 90 s (3 apt runs, the `wasm-bindgen-cli` compile) | A small image, rebuilt when the toolchain or `wasm-bindgen` pin changes | Low. Pin it by digest |
| **cargo-chef** dependency layer, with `--cache-from` an Artifact Registry image | Third-party crates and downloads: about 130 s native and about 100 s WebAssembly, when `Cargo.lock` is unchanged | Registry cache storage; Dockerfile restructure | Low to medium. Cache misses whenever `Cargo.lock` changes; workspace crates still rebuild |
| **BuildKit cache mounts** (`--mount=type=cache` for `~/.cargo/registry` and `target/`) | Avoids the 29 s and 36 s layer commits. On fresh Cloud Build workers the mount starts empty, so it only pays off on a persistent builder | None | Low |
| **sccache with a GCS bucket** (`SCCACHE_GCS_BUCKET`), shared by Cloud Build and Linux builders | Unchanged crates come from the bucket: native compile about 300 s to 100 to 150 s; WebAssembly similar | A bucket and a service account; network pulls of about 1 GB a build | Medium. Doesn't cache the final link or crates whose inputs changed; needs a stable `/build` path |
| **kache with a remote** (it supports S3 and filesystem remotes; GCS through its S3 interoperability API) | Same order as sccache | HMAC keys for GCS, not yet tried | Medium. Remote on GCS is untested. Mac caches don't help Linux builds: macOS compiles `aarch64-apple-darwin`, Cloud Run needs `x86_64-unknown-linux-gnu` |
| **Persistent builder VM** in us-central1 (warm `target/`, registry, kache local store, Docker layer cache) | Change only in `openagents-web`: about 80 s compile, 10 s push. Cold paths vanish: no upload, no queue, no downloads | e2-highcpu-32 is about $0.80 an hour; stop it when idle, or a c3/c4 machine for faster single-thread | Medium. A machine to keep patched; builds must start from a clean `origin/main` checkout |
| **CoderOS box** as the builder | Same as above | Free | High. It runs Coder batches and has tight disk; uploads from the home network |
| **Bigger Cloud Build machine** | Little. E2_HIGHCPU_32 is already the largest default-pool type; a private pool can use other machine families. More cores don't shorten the serial parts (downloads, final crate, link, layer commits) | Private pool fee | Low |
| **Split crates**: move what `openagents-web` uses from `coder` (`task::sales`, `customer::plugins`, `efficiency`, `builtin_plugins`) into small crates | Cold native build from 603 crates to roughly 250 (no `wasmtime`, `coder-host`, `gym`, `pylon` if their uses are similar); estimated 300 s to about 120 s | Engineering time; touches `coder` | Medium. Needs care so the chat worker and web agree on shared types |
| **Release profile**: `strip = true`; `incremental = true` in a deploy profile used only on a persistent builder | `strip`: a few seconds. Incremental: the `openagents-web` crate rebuild drops from about 73 s to about 30 s on a small change | None | Low. LTO is already off (no `[profile.release]` section; defaults are `lto = false`, `codegen-units = 16`) |
| **Separate faster staging profile** (`opt-level = 1`, more codegen units) | 30 to 40 % of compile | None | High. Staging would test a different binary from production; this breaks promote by digest. Not recommended |
| **mold or lld** | Small. Rust 1.90 and later already link `x86_64-unknown-linux-gnu` with `rust-lld` by default | None | Low |
| **Build the binary outside Docker, copy it into a thin runtime image** (`FROM debian:13-slim` plus `COPY`; or `crane append` a layer onto a base by digest) | Image step drops to seconds; the binary build uses whatever cache the builder has | None | Low on a Linux builder. On the Mac it needs `cargo zigbuild` (the chat worker already ships this way) and weakens build provenance |
| **Promote by digest**: deploy the exact staging digest to production, never rebuild | Removes a full 12 to 15 minute build from every production deploy | None | Low. The image already takes its host from flags, so one image fits both |
| **One build for web and stack** (`cargo build -p openagents-web -p gateway -p coder --bin coder-worker` in one `target/`) | Wall time unchanged (they run in parallel today); saves a 6 minute build and about 64 shared workspace crates compiled twice | None | Low. Feature unification can change; check binaries still pass smoke |
| **Skip unchanged WebAssembly** (a separate `openagents-web-assets` image, rebuilt only when its crates or `assets/verse` change; the web image copies from it by digest) | With BuildKit, about 0 s off the critical path; without BuildKit, about 335 s | A second image and a change check | Low. `Dockerfile.native-overlay` already does this by hand |
| **Smaller context** (assets image above; `assets/verse` is 263 MB of the 620 MB) | Upload 86 s to about 40 s | None | Low |

## 3. Content updates without a Rust rebuild

### What can move out of the binary

| Content | Move to runtime files? | Why |
| --- | --- | --- |
| Docs Markdown, `SECTIONS` order and titles | Yes | Pure text; most frequent edits |
| Agent docs, the API skill page | Yes | Text |
| Landing and marketing copy (strings inside Maud) | Yes, as data: keep the Maud layout, read the strings from a TOML file | Layout stays reviewed code |
| Images, fonts CSS, `bluerush` CSS | Yes | Static |
| JavaScript (`chat.js`, `flow.js`, `webmcp.js`, htmx) | Later, with care | It's code: needs the full test suite and the same CSP review |
| Efficiency rows, answer bank, knowledge | Yes, with a compiled-in fallback | Data; the answer bank changes what chat says, so it needs its eval |
| Legal text | Keep compiled for now | Legal sign-off; rare changes |
| Installer scripts | Keep compiled, or serve only by a pinned digest | A swapped installer is a supply-chain risk |
| Routes, auth, sessions, CSRF, CSP and other headers, redirects | Keep compiled | Security |

### Ways to ship content

| Option | Deploy time | Pros | Cons |
| --- | ---: | --- | --- |
| **Content layer on the live image**: `crane append` a tar of `content/` onto the live image digest, then `services replace` with the new digest | about 30 s (seconds to append, 15 s to roll out) | Atomic, versioned, rollback by revision; no Docker or Rust on the build path; every revision is still one immutable image | One new revision per content change |
| Content-only Dockerfile (`FROM openagents-web@sha256:…` plus `COPY`) in Cloud Build | about 1.5 to 2 min (upload, queue, a 20 s build, as `a5c1c97c` did in 19 s) | Uses today's tools | Queue and upload dominate |
| Cloud Storage bucket mounted as a read-only volume (Cloud Storage FUSE) | Seconds after upload; the site rereads | No revision needed | Not atomic across files; a bucket write becomes a production change with no review; caching and consistency in FUSE |
| Serve docs straight from a bucket with cache headers | Seconds | Simplest for big media | Same review gap; split origins for links and CSP |
| Hot-reloadable templates (replace Maud with a runtime engine) | Seconds | Full page edits | Rewrite of every page; loses compile-time checks. Not recommended |

**Recommended**: a content layer by digest. The binary gains one flag,
`--content DIR` (default: the compiled-in copy, so local runs and tests keep
working). The service spec pins the image digest, so content is reviewed,
versioned, and rolled back exactly like code.

### Keeping content-only deploys safe

- Move the content checks out of `openagents-web` into a small crate (for
  example `openagents-web-content`) that depends only on `oa-copy` and
  `pulldown-cmark`: the machine-talk guard, the bare-instruction lint in
  `content.rs`, "every section lists every doc", and internal links resolve.
  The web crate and a `content-check` binary both use it. It compiles in
  seconds with kache.
- Run `content-check` in CI and in the content deploy script; refuse to
  append a layer that fails.
- Render Markdown with raw HTML off, as the compiled path does, so a content
  file can't add a script.
- After the rollout, the deploy script fetches each changed page from the tag
  URL and checks the status code.

## 4. Recommended plan

### This week (quick wins)

| Step | Saving | Owner decision |
| --- | --- | --- |
| Turn on BuildKit in both `cloudbuild.yaml` files | About 5 min a web build | None |
| Promote by digest: production uses the staging digest; write `scripts/deploy/web.sh` (`stage`, `promote`, `rollback`) that copies the live spec, swaps only the image digest, quotes `CODER_CHAT_SYNC`, and deploys to the `new` tag | One full build per production deploy; fewer hand edits | Agree that production only ever runs a staged digest |
| Fix the `actAs` refusal for the automation account on the runtime account, so the script runs end to end | Removes the hand-off to `chris@` | Owner IAM change |
| Builder base image with apt, `wasm32`, `wasm-bindgen-cli`, `cargo-chef` | About 1.5 min | None |
| Add `strip = true` to the release profile and drop the `strip` calls | Seconds | None |

Target: staging code deploy about **8 minutes** end to end; production
promotion about **1 minute** after staging passes.

**Done 2026-10-09** (#11094): BuildKit in both `cloudbuild.yaml` files and
`scripts/deploy/web.sh` (`stage`, `promote`, `shift`, `rollback`). First
run, commit `dba9b59ff8`, build `3b9081ea`:

| Phase | Before (`b2b67216`) | After (`3b9081ea`) |
| --- | ---: | ---: |
| Export and upload from the Mac | 86 s | ~180 s (`git archive` of the commit, then the upload) |
| Queue | 54 s | 58 s |
| Cloud Build (fetch, kit, docker, push) | 760 s | 604 s |
| `docker build` step | ~700 s | 556 s |
| Staging deploy, smoke | ~15 s, not measured | 42 s, 41 s |
| Production | a second full build, then hand edits | promote 26 s, candidate smoke 51 s, traffic 10 s |

BuildKit saved about 2.5 minutes, not 5: the native and WebAssembly
stages now overlap, but each still compiles from nothing. Promoting by
digest removed the second build. The upload grew because the script
exports the whole commit before `gcloud` applies the ignore file; the
next saving there is exporting only what `web.gcloudignore` lets through,
or the persistent builder below. The `actAs` refusal still holds: the
script retries production writes as `chris@`.

### Next (one to two weeks)

| Step | Saving | Owner decision |
| --- | --- | --- |
| Persistent builder VM in us-central1 (warm `target/`, kache, Docker cache), building from a clean `origin/main` worktree, pushing to Artifact Registry. Build the binaries outside Docker and `COPY` them into a thin runtime image | Web-only change: about 2 min build. No upload or queue | VM size and budget; stop-when-idle schedule |
| One build for web and stack binaries in the same `target/` | About 6 min of duplicate compile | None |
| Separate assets image for the WebAssembly builds, rebuilt only on change | 3 to 6 min when unchanged | None |
| `--content DIR` flag, `content-check` crate, and `crane append` content deploy for docs and agent docs | Docs change: **under 1 minute**, no Rust | Which content may ship without a code review (docs only to start) |
| Keep Cloud Build as a fallback, with sccache on a GCS bucket | Fallback build about 6 to 7 min | None |

Target: code deploy **under 3 minutes** for a change in `openagents-web`;
content deploy **under 60 seconds**.

### Later

| Step | Saving | Owner decision |
| --- | --- | --- |
| Split the parts of `coder` that the web uses into small crates, so the web binary doesn't compile `wasmtime`, `coder-host`, `gym`, or `pylon` | Cold build about 5 min to about 2 min; smaller binary | Priority against launch work |
| Split `openagents-web` itself (pages, chat, cloud, environments) so a page edit recompiles a small crate | The 73 s final crate to about 20 s | None |
| Move landing copy, efficiency rows, and the answer bank to runtime data with compiled fallbacks; the answer bank behind its eval | Copy and data changes become content deploys | Which data needs an eval gate |
| Automatic staging deploy on push to `main` from the builder, with smoke, then a one-command promote | No operator steps for staging | Owner go for automatic staging |

Target: content deploy **under 30 seconds**; cold code build **under 4
minutes**; incremental code deploy **under 2 minutes**.

## Top five, by expected saving

1. **Promote by digest**: removes a 12 to 15 minute rebuild from every
   production deploy.
2. **BuildKit parallel stages**: about 5 minutes off every web build, one
   line of configuration.
3. **Persistent warm builder, binary built outside Docker**: about 15
   minutes to about 2 to 3 minutes for typical web changes; removes upload
   (86 s) and queue (55 s).
4. **Content layer by digest with a fast `content-check`**: docs and copy
   fixes in under a minute instead of 15.
5. **Stop compiling `coder` into the web binary**: the cold build drops by
   more than half, which helps every path above, including the fallback.
