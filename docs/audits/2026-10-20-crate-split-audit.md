# Rust crate split audit

Recorded on 2026-10-20 on `coderos-4080`. Compile samples use `1947313448`;
the source inventory uses `11b6369e40`, through `2026-10-20T20:58:27-05:00`.
This is a docs-only audit. It covers every workspace member
package and separately lists nonmember support and fixture manifests that standalone Cargo metadata rejects.
It proposes smaller Rust crates, not more product implementations or more
workspaces.

## Recommendation

Make **20,000 checked-in Rust source lines a reporting threshold**, with a
**5,000–15,000-line target for new implementation crates** and **under 5,000
lines for shared contract crates**. Treat 40,000 lines as a required split plan
or an explicit, reviewed exception. Count source separately from integration
tests, examples, and generated code. A 400-line crate does not need splitting.
An unchanged facade that still imports every implementation does not reduce
consumer rebuilds by itself.

Start with frequently edited implementation crates: `coder-new`, remaining web
route families, the still-large `coder-task`, CLI command families, and the
app's engine and view areas.
Continue with runtime/router refinements and high-fanout domain contracts. Start an early ML track with train-core and one Parameter Golf family, then
split the remaining very large training, serving, evaluation, and runtime families; its recent import makes raw commit counts a poor indication of
future demand. Retained legacy agents and old desktop/mobile shells come later,
with retirement preferred where they duplicate the current app.

The useful unit of progress is an independently testable implementation crate
with a narrow dependency direction. Moving files to another package while every
caller still imports one large facade saves that package's local test loop, but
can leave the desktop or Coder binary waiting on the same chain. Measure both.

## What is already done or in progress

Read [#12184 and its comments](https://github.com/OpenAgentsInc/openagents/issues/12184)
and the [compile-time audit](2026-10-19-rust-compile-times.md) before applying
this plan. The older audit includes different machines, compilers, profiles,
and pre-split source sizes; its rows are historical baselines, not today's
measurements.

| Work | State at audit start | Evidence and implication |
| --- | --- | --- |
| Screens, agent pages, run commands, web executable → `openagents-web-site` | Landed, `b0b1ce1708` | Same-host edit build 43.1 → 4.8 s; test build 88.7 → 4.5 s. Keep this boundary; do not propose it again. The executable is still named `openagents-web`. |
| Coder → `coder-runtime`, `coder-router`, `coder-task`, compatibility facade | Landed, `8c5facc913` | Package build 65.0 → 14.0 / 7.5 / 22.0 / 4.0 s; test build 86.6 → 11.4 / 11.5 / 31.3 / 14.6 s. These are first cuts: task still holds 156k source lines. |
| Edits in runtime/router/task → `coder` consumer | Measured by #12184 | Build 34.9 / 63.0 / 43.8 s; test build 75.3 / 78.8 / 74.6 s. Do not substitute child-only times for consumer times. |
| Native media, Lab cards/models, voice orbs, rounded clipping, running pills, notification cues, font statics → `openagents-media-view` | In progress at audit start; **landed during the audit as `cc84ee3d7c`**, owned by chat `150a40a9` | Reserved scope. UI keeps old paths through reexports. Media local build 14.3 → 1.9 s; test build 21.1 → 2.6 s. Remaining UI 18.7 / 27.0 s, versus 14.3 / 21.1 before, is not an improvement. Its no-op was 17.2 s, so load is a substantial confounder. Media → UI build 23.0 s; consumer test build 18.7 s versus 21.1 s before; media → desktop build 9.8 s versus the UI → desktop baseline 10.9 s. Warm-ups were 795.9 s and 63.9 s after its required target trim, and are excluded. |
| Earlier `f40eed6e9a` mentioning #12184 | Benchmark script change only | It is not evidence that any crate was split. `git log --oneline --grep=12184` found the two landed splits above and this script change at the audited revision. |

Coordination used the requested `send_message` to chat `150a40a9` on this
machine. The chat confirmed that Foreman lifted the build hold, then reported the media
landing and no next split beyond media. All three #12184 splits are on main.
The remaining UI proposals below start after that work. Its measurements use eight jobs, no compiler wrapper,
Rust 1.97.1 for root and 1.98.1 for the app, on this i7-14700K shared host.

## Scope and measurement definitions

The inventory comes from tracked manifests and `cargo metadata --no-deps
--format-version 1` for all 25 explicit `[workspace]` roots. There are **420
distinct member packages**, plus **9 nonmember support/fixture manifests**.
The tables key packages by name **and directory**: root and app both contain
`openagents-ui`, for example. Names alone are not unique across workspaces.
Historical files named `something.Cargo.toml` are not Cargo manifests and are
not counted as workspace crates.

- **Rust lines, all/src:** `wc -l`-equivalent physical lines in tracked `.rs`
  files owned by the nearest package manifest; `src` counts only `src/**`.
  Both include comments, blank lines, inline tests, and checked-in generated
  Rust. `all` also includes build scripts, integration tests, and examples.
  Nested crates are not double-counted. `include!` and `#[path]` can compile
  additional files owned elsewhere, so a small wrapper is not proof of a small
  compiler unit. Generated files are called out where
  they affect the recommendation.
- **Files/mods:** tracked Rust file count and syntactic `mod name;` or `mod
  name {` declarations, including test and conditional modules. These counts
  describe the checked-in surface, not just modules enabled on Linux.
- **API P/U/R:** approximate unrestricted `pub` item/function/method
  declarations, `pub use` statements, and restricted `pub(...)` declarations.
  Public struct fields, implicit trait methods, and enum variants are not counted. This is a source-level exposure proxy,
  not a rustdoc reachability or semver API count: binary-only packages (which have no importable Rust library), private parent
  modules, macros,
  multiline declarations, cfgs, and reexports affect the actual exposed API.
  The cached Code Graph at `0b17782540` was queried first for module/public-item
  structure; it predates the Coder move, so current files and manifests supply
  the inventory rather than silently treating that graph as current.
- **Edits:** distinct commits from
  `git log --since=2026-10-06T20:58:27-05:00
  --until=2026-10-20T20:58:27-05:00 -- CRATE_DIR`, at the audited revision.
  This counts every change under a crate, including docs/tests. Newly extracted
  Coder crates have one move commit; their parent's history demonstrates the
  workload. Do not rank them as inactive because Git's directory history resets.
- **Dependents D/T (+dev):** distinct direct and transitive **other local
  packages** reachable through normal/build local dependencies, including local
  Cargo patches; the final number
  also admits dev edges. Each workspace resolves its local patches separately before counts are
  combined; paths from different patch contexts are not chained together. Counts
  include all declared target and optional-feature edges. They are a conservative impact envelope, not a promise
  that one `cargo -p` command builds every package in it. The changed crate is
  additional. Cargo feature resolution, selected targets, conditional code,
  compiler reuse, and caches determine actual work. Registry/git dependencies
  that are not local packages do not inflate reverse counts.
- **Heavy dependency candidates:** selected direct local dependencies with the
  largest source bodies, plus known compiler/native/GPU/network/database stacks.
  `?` means optional; the feature column in the split table explains important
  gates. These are dependency risks, not timings assigned to unmeasured crates.
  A workspace-hack dependency can add cost beyond the abbreviated list.
- **Savings:** future percentages are planning estimates of the edited
  implementation's own warm compile/build time, assuming an edit stays within
  the moved family and stable contracts avoid rebuilding other families.
  Consumer savings can be zero. A context/types crate that imports every old
  implementation defeats the boundary even if the files are smaller. Multiply a measured edit time by the percentage
  to get an estimated seconds range; do not report it as an observed A/B result.
  A facade or shared-types edit still invalidates its consumers. Link time and
  Cargo overhead limit the saving even after a successful split.
- **Effort/risk:** S = up to a day; M = roughly 1–3 engineering days;
  L = roughly 3–7 days; XL = several staged weeks. These include boundary design,
  caller migration, and focused validation, not elapsed agent wall time.
  Low/medium/high risk describes compatibility, platform, persistence, and
  authority behavior. New splits still need before/after measurement.

The audit worktree is `~/wt/crate-audit`. Every compile uses only
`~/wt/target-crate-audit/{root,app,psionic,nightly}` and source-keyed state from
`scripts/dev/cargo-build-env.sh`, through the normal Cargo shim, with
`OPENAGENTS_KEEP_TARGET_DIR=1`, eight jobs, no compiler wrapper, and the
repository's dev profiles. `CARGO_INCREMENTAL` stays unset: some leaf crates
are incremental and others intentionally are not. Calling these all
“incremental edits” describes the warm edit loop, not a claim that every crate
has `-C incremental` enabled.

`df -h /` initially reported 167 GiB free. The sampler checks free space before
each command and while it runs, refusing a command below 65 GiB and terminating
its own command below 60 GiB to leave a margin over the owner's 50 GiB floor.
It never cleans or builds in the owner's release target or another agent's
target. Dependencies are warmed once and reused within each workspace; this is
not a full clean build of every crate. Only a private dead function is appended
temporarily to the selected library root, then restored byte-for-byte after
each sample. No source, manifest, or lockfile edits are part of the commit.

## All workspace roots

Member counts are Cargo metadata counts at the audited revision. Package
paths in the inventory below enumerate every member. Standalone workspaces
with one package are included.

| Workspace manifest | Member packages | Purpose / build context |
| --- | --- | --- |
| `Cargo.toml` | 285 | Root product/domain workspace; Rust 1.97.1 |
| `apps/openagents/Cargo.toml` | 34 | Desktop/mobile app; Rust 1.98.1; own profiles and SQLite version |
| `apps/openagents/web/Cargo.toml` | 41 | GPUI browser clients, views, and first-frame tooling; separate Wasm feature graph |
| `apps/openagents/web/card/webgpu/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `apps/openagents/web/graph-card/webgpu/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `apps/openagents/web/lander-chat/webgpu/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `apps/openagents/web/pylon-card/webgpu/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/layertree-tee-lab/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/layertree-tee-lab/client/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/layertree-tee-lab/fuzz/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/layertree-tee-lab/server/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/lnd-sdk/signet-lab/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `crates/psionic/Cargo.toml` | 35 | ML/inference/training, including four vendored DeepSeek packages; separate profiles |
| `crates/robot-sim/tools/bindings/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `deploy/coder-sync/Cargo.toml` | 2 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `deploy/iroh-relay/live-test/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `deploy/space-credentials/fixture/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `deploy/space-host/fixtures/environment/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `docs/decision-models/2026-09-20-lev-domain-gap/analysis/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `examples/sdk-history/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `examples/sdk-labs/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `mc-bridge/Cargo.toml` | 1 | Minecraft bridge helper; own nightly toolchain |
| `quests/bridge-planner/fixture/Cargo.toml` | 1 | Isolated lab, fixture, deployment package, or SDK example; do not merge it into the root build by splitting crates |
| `support/workspace-hack/Cargo.toml` | 1 | Generated feature-unification support; no implementation to split |
| `vendor/boltz-client/Cargo.toml` | 4 | Vendored wallet integration; follow upstream |

## Compile samples on coderos

Selection at `1947313448`: the ten largest packages by all Rust lines, plus the five
most-edited remaining packages (`coder`, app mobile, `verse`, gateway, and
SDK), plus an extra Everglade sample selected before tightening the activity
window to exactly 14 days. These are 16 distinct packages across root, app, and Psionic.
The table reports **Cargo elapsed seconds**, parsed from its `Finished` line;
it excludes shim admission and process-start overhead. Warm-up is excluded
from edit and no-op columns. One sample per command is sufficient to find
large units, not to prove small percentage differences on a shared host.

Each check and build is warmed separately. The private probe changes the
library root without changing its public API. It tests a modest invalidation;
a real signature or derive-heavy edit can cost more. Package builds include
any default binaries: the remaining web core still has three helper binaries.
`cargo build --timings` records the library rustc unit separately. Its front-end
section is the interval labelled frontend by Cargo. Binary-only packages
can lack that section; their whole rustc unit is shown without inventing a share.
This is not a sum of overlapping
`-Ztime-passes` counters and not the percentage of whole-command wall time.

| Crate / workspace | No-op check / build (s) | Edit check (s) | Edit build (s) | Primary library/bin front end / rustc unit (s) | Warm check / build (s) |
| --- | --- | --- | --- | --- | --- |
| `openagents-web` (root) | 1.00 / 1.12 | 5.81 | 15.99 | 7.81 / 9.94 (79%) | 146.00 / 288.00 |
| `coder-task` (root) | 0.65 / 0.76 | 13.01 | 23.08 | 16.74 / 22.38 (75%) | 38.64 / 71.00 |
| `coder-new` (root) | 0.67 / 5.75 | 2.77 | 10.22 | 3.61 / 5.17 (70%) | 40.59 / 73.00 |
| `coder-one` (root) | 0.67 / 0.63 | 12.38 | 28.42 | 18.89 / 25.69 (74%) | 23.76 / 33.45 |
| `openagents-cli` (root) | 1.05 / 2.90 | 2.46 | 6.94 | unit 6.24; no front-end sections | 92.00 / 172.00 |
| `coder` (root) | 0.63 / 0.58 | 1.19 | 4.15 | 0.05 / 0.06 (83%) | 13.07 / 26.36 |
| `verse` (root) | 0.66 / 0.61 | 3.77 | 25.70 | 4.47 / 21.66 (21%) | 27.81 / 99.00 |
| `verse-zone-everglade` (root) | 0.56 / 0.60 | 3.03 | 13.62 | 3.58 / 11.11 (32%) | 14.18 / 58.31 |
| `gateway` (root) | 0.54 / 0.57 | 1.80 | 4.02 | 1.67 / 2.02 (83%) | 18.40 / 41.90 |
| `openagents-sdk` (root) | 0.53 / 0.53 | 3.37 | 4.01 | 3.28 / 3.49 (94%) | 0.54 / 5.07 |
| `openagents-ui` (app) | 0.68 / 1.05 | 3.87 | 5.76 | 3.93 / 5.14 (76%) | 231.00 / 689.00 |
| `openagents-engine` (app) | 0.56 / 0.74 | 2.66 | 4.00 | 2.78 / 3.48 (80%) | 97.00 / 432.00 |
| `openagents-mobile` (app) | 0.55 / 0.98 | 2.61 | 6.19 | unit 5.62; no front-end sections | 116.00 / 376.00 |
| `psionic-train` (psionic) | 0.23 / 14.87 | 18.32 | 76.00 | 33.67 / 42.73 (79%) | 166.00 / 369.00 |
| `psionic-serve` (psionic) | 0.55 / 0.41 | 3.52 | 5.74 | 3.39 / 4.21 (81%) | 197.00 / 362.00 |
| `psionic-eval` (psionic) | 0.22 / 0.30 | 8.31 | 12.31 | 9.72 / 12.09 (80%) | 35.56 / 124.00 |

The root samples support a second cut to task and the web: their libraries
spent 16.74/22.38 s and 7.81/9.94 s in the front end. Coder One rebuilt in
28.42 s but has much less recent activity. The Coder facade is already tiny: its
83% share is only 0.05 s of front-end work, so percentage alone is a poor rank.
Verse spent only 21% in the front end; most of its 21.66 s library unit was
codegen under its optimized dev profile. Its split benefit comes mainly from
isolating optimized implementation families. The SDK is 94% front end but only
3.49 s per library unit. Keep those small absolute costs below larger hot units.


Training is the largest measured edit command: 76.00 s, including a 42.73 s
library unit with 33.67 s in the front end. Its no-op took 14.87 s despite no
newly compiled units in the timing report, so command-level precision is limited.
Start its core/family split early as a separate ML track; do not interpret a
recent import as an inactive crate. Most of the remaining edit-command time
comes after the library, including its many default binaries. A child-only
measurement cannot stand in for those consumers.


### What the timing reports identify

Largest newly compiled library units from each workspace's first build
warm-up, with wall time per rustc unit. Units overlap, so do not add their
seconds to predict command time. These are actual heavy dependencies for the
sampled build, complementing the static per-crate dependency candidates.

| Workspace / first sampled package | Ten slowest library units, seconds |
| --- | --- |
| root / `openagents-web` | `openagents-web` 105.39; `coder-task` 34.52; `physics` 33.97; `gym` 31.97; `solidcraft-kernel` 29.44; `solidcraft-doc` 22.28; `openagents-harness` 22.06; `cranelift-codegen` 21.87; `wasmtime` 17.17; `tenancy` 16.49 |
| app / `openagents-ui` | `fff-search` 181.49; `coder-host` 127.61; `uniffi_bindgen` 98.07; `cranelift-codegen` 92.93; `openagents-chat` 90.33; `loro-internal` 71.84; `coder-access` 68.18; `pylon` 67.10; `nostr` 64.83; `gpui` 63.79 |
| psionic / `psionic-train` | `psionic-train` 164.72; `psionic-eval` 97.14; `psionic-runtime` 84.18; `candle-core` 76.14; `candle-transformers` 61.77; `psionic-data` 42.23; `psionic-models` 42.19; `image` 29.45; `tokenizers` 29.15; `ravif` 28.96 |

### Nightly pass sample

The installed `nightly-2026-08-21` is Rust **1.100.0-nightly
(`8925ea358`, 2026-08-20)**. The repository's opt-in pinned
`nightly-2026-08-03` is not installed. This diagnostic uses the installed
compiler without changing repository settings. To avoid warming another full
application graph, it profiles `psionic-ir`, a shared compiler/IR library.

After warming its dependencies, a full, non-incremental library recompile uses
eight Cargo jobs, one front-end thread, the Psionic dev profile (`opt-level=1`,
16 codegen units), and a command-local incremental override for this package:

```sh
# From crates/psionic, with the audit's nightly target and build helper loaded:
export RUSTUP_TOOLCHAIN=nightly-2026-08-21
unset RUSTC
. ../../scripts/dev/cargo-build-env.sh
cargo rustc -p psionic-ir --lib --timings \
  --config 'profile.dev.package.psionic-ir.incremental=false' \
  -- -Zthreads=1 -Ztime-passes
```

| Measurement | Seconds |
| --- | ---: |
| Cargo command | 6.93 |
| Library rustc unit | 6.69 |
| Cargo front-end interval | 3.67 (55% of the unit) |
| `type_check_crate` | 0.887 |
| `MIR_borrow_checking` | 1.337 |
| `coherence_checking` | 0.127 |
| `generate_crate_metadata` | 0.702 |
| Monomorphization graph walk | 0.267 |
| `LLVM_passes` | 0.954 |
| `LLVM_thinlto` | 1.823 |
| rustc `total` | 6.675 |

These pass counters nest and overlap; do not add them. Dependency warm-up was
about 36 s and is excluded. This is a separate compiler/profile diagnostic, not
an A/B comparison against the stable incremental samples. Some binary and
multi-output staticlib/cdylib units lack front-end sections in the stable Cargo
reports; their table cells retain the complete unit time without inventing a
front-end share.


## Inventory by crate

The 429 manifests own 5,269,679 tracked Rust lines,
including 4,831,177 under `src/**`. Every row uses the
metric definitions above. All/src is deliberately separate: app engine
contains substantial integration tests; LND source is mostly generated.

### `Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `acp-client`<br>`crates/acp-client` | 5,597 / 5,597 | 10 / 21 | 254 / 4 / 7 | 19 | 10 / 51 (118) | `coder-lease`, `tokio` |
| `actors`<br>`crates/actors` | 9,564 / 8,098 | 20 / 16 | 178 / 6 / 8 | 5 | 4 / 6 (6) | `tokio-postgres`?, `reqwest`?, `tokio`?, `axum`? |
| `agent-contract`<br>`crates/agent-contract` | 636 / 636 | 1 / 1 | 38 / 0 / 0 | 11 | 9 / 209 (213) | none measured; small direct dependency set |
| `agent-fleet`<br>`crates/agent-fleet` | 1,839 / 1,839 | 3 / 5 | 89 / 0 / 0 | 7 | 5 / 5 (118) | `coder-lease` |
| `agent-graph`<br>`crates/agent-graph` | 2,905 / 2,827 | 9 / 18 | 115 / 1 / 1 | 5 | 7 / 9 (9) | `agent-lab`? |
| `agent-lab`<br>`crates/agent-lab` | 5,407 / 3,934 | 13 / 10 | 147 / 2 / 0 | 41 | 8 / 99 (122) | `rust-native`?, `lab-kit` |
| `archil`<br>`crates/archil` | 12,019 / 6,710 | 34 / 32 | 379 / 17 / 44 | 3 | 2 / 24 (24) | `reqwest`, `tokio`, `tokio-tungstenite` |
| `atif`<br>`crates/atif` | 6,046 / 5,343 | 13 / 11 | 121 / 5 / 0 | 9 | 35 / 120 (160) | none measured; small direct dependency set |
| `atif-nostr`<br>`crates/atif-nostr` | 1,755 / 1,624 | 8 / 5 | 38 / 1 / 2 | 2 | 1 / 1 (1) | `nostr`, `atif`, `reqwest`, `tokio`, `secp256k1` |
| `att-platforms`<br>`crates/att-platforms` | 1,691 / 1,691 | 6 / 5 | 45 / 0 / 0 | 3 | 1 / 1 (3) | `nostr`, `oa-att` |
| `att-release`<br>`crates/att-release` | 2,144 / 1,921 | 9 / 8 | 52 / 0 / 0 | 6 | 2 / 4 (5) | `nostr`, `secp256k1` |
| `att-seal`<br>`crates/att-seal` | 1,759 / 1,759 | 5 / 4 | 51 / 0 / 11 | 3 | 0 / 0 (1) | `nostr`, `oa-att`, `secp256k1` |
| `att-web`<br>`crates/att-web` | 5,870 / 5,775 | 13 / 18 | 171 / 2 / 2 | 11 | 0 / 0 (0) | `oa-att`, `secp256k1` |
| `automation-host`<br>`crates/automation-host` | 6,473 / 6,473 | 18 / 17 | 67 / 0 / 42 | 7 | 0 / 0 (0) | `nostr`, `coordination-host`, `workspace-host`, `secp256k1` |
| `background`<br>`crates/background` | 17,870 / 17,837 | 38 / 51 | 426 / 19 / 18 | 13 | 8 / 73 (134) | `coder-lease`, `coder-boundary`, `atif` |
| `bitcoin-amount`<br>`crates/bitcoin-amount` | 483 / 483 | 2 / 1 | 25 / 0 / 0 | 14 | 6 / 11 (11) | none measured; small direct dependency set |
| `boat`<br>`crates/boat` | 14,462 / 10,175 | 239 / 235 | 402 / 217 / 15 | 7 | 16 / 39 (126) | `reqwest`, `tokio` |
| `boat-template`<br>`crates/boat-template` | 884 / 884 | 1 / 1 | 0 / 0 / 0 | 4 | 0 / 0 (0) | `boat`, `tokio` |
| `brainstorm-client`<br>`crates/brainstorm-client` | 2,270 / 2,270 | 4 / 3 | 45 / 1 / 10 | 2 | 2 / 2 (2) | `reqwest`, `tokio` |
| `briefed-agent`<br>`crates/briefed-agent` | 4,615 / 4,526 | 9 / 11 | 49 / 0 / 0 | 27 | 1 / 2 (2) | `coder-sdk`, `jev`, `claude_agent_sdk`, `squeeze`, `tokio` |
| `briefing-lab`<br>`crates/briefing-lab` | 7,592 / 5,657 | 11 / 11 | 66 / 0 / 7 | 1 | 0 / 0 (0) | `tree-sitter`, `tree-sitter-rust` |
| `bunny-rules`<br>`crates/bunny-rules` | 5,252 / 5,252 | 10 / 19 | 172 / 3 / 2 | 5 | 1 / 1 (1) | none measured; small direct dependency set |
| `bunny-web`<br>`crates/bunny-web` | 7,354 / 7,354 | 13 / 21 | 212 / 0 / 1 | 13 | 0 / 0 (0) | `nostr`, `verse-net`, `bunny-rules` |
| `cad`<br>`crates/cad` | 15,243 / 15,111 | 28 / 43 | 230 / 1 / 5 | 31 | 5 / 95 (120) | `physics`, `lab-kit`, `solidcraft-engine` |
| `cad-lab`<br>`crates/cad-lab` | 3,332 / 3,332 | 4 / 4 | 25 / 0 / 0 | 25 | 0 / 0 (0) | `cad`, `robot-lab`, `tokio`, `axum` |
| `cad-wasm`<br>`crates/cad-wasm` | 370 / 370 | 1 / 2 | 12 / 0 / 0 | 7 | 0 / 0 (0) | `cad` |
| `cad-world`<br>`crates/cad-world` | 9,048 / 9,048 | 14 / 18 | 351 / 1 / 0 | 2 | 6 / 35 (134) | `physics` |
| `capability`<br>`crates/capability` | 4,180 / 3,091 | 8 / 6 | 91 / 4 / 5 | 2 | 5 / 33 (118) | `nostr`, `atif`, `supervise`, `tokio` |
| `chat-home`<br>`crates/chat-home` | 476 / 476 | 2 / 3 | 27 / 0 / 0 | 5 | 10 / 112 (135) | `rust-native`? |
| `chat-load-bench`<br>`crates/chat-load-bench` | 2,660 / 2,618 | 9 / 7 | 51 / 0 / 0 | 2 | 0 / 0 (0) | `nostr`, `rust-native`, `coder-history`, `coder-connect`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `claude_agent_sdk`<br>`crates/claude_agent_sdk` | 9,270 / 9,108 | 15 / 17 | 327 / 12 / 4 | 9 | 5 / 49 (118) | `tokio` |
| `code-graph`<br>`crates/code-graph` | 3,629 / 3,281 | 9 / 7 | 84 / 2 / 0 | 3 | 2 / 3 (3) | `tree-sitter`, `tree-sitter-rust` |
| `code-highlight`<br>`crates/code-highlight` | 2,466 / 2,427 | 8 / 14 | 74 / 6 / 1 | 1 | 4 / 168 (179) | `tree-sitter-rust` |
| `code-index`<br>`crates/code-index` | 7,467 / 6,090 | 25 / 20 | 150 / 0 / 5 | 10 | 1 / 2 (2) | `coder-history`, `atif`, `code-graph`, `reqwest` |
| `coder`<br>`crates/coder` | 31,152 / 11,565 | 54 / 47 | 12 / 3 / 0 | 349 | 15 / 28 (117) | `coder-task`, `gym`, `nostr`, `coder-runtime`, `image`, `reqwest`, `tokio`, `tokio-tungstenite` |
| `coder-access`<br>`crates/coder-access` | 25,845 / 25,111 | 50 / 69 | 873 / 10 / 52 | 42 | 29 / 92 (153) | `nostr`, `coder-connect`, `robot-lab`, `tokio`, `secp256k1` |
| `coder-binds`<br>`crates/coder-binds` | 2,535 / 2,332 | 5 / 5 | 65 / 2 / 0 | 1 | 2 / 76 (136) | none measured; small direct dependency set |
| `coder-boundary`<br>`crates/coder-boundary` | 8,374 / 6,540 | 19 / 23 | 120 / 4 / 37 | 3 | 15 / 87 (144) | none measured; small direct dependency set |
| `coder-browser`<br>`crates/coder-browser` | 2,778 / 2,778 | 5 / 7 | 72 / 2 / 8 | 5 | 2 / 2 (2) | `nostr`, `coder-access`, `terminal-core`, `coder-pty`, `tokio`, `secp256k1` |
| `coder-browser-web`<br>`crates/coder-browser-web` | 2,472 / 2,472 | 3 / 3 | 7 / 1 / 0 | 7 | 0 / 0 (0) | `coder-access`, `terminal-core`, `rust-native`, `coder-ui`, `wgpu` |
| `coder-chat-web`<br>`crates/coder-chat-web` | 1,004 / 1,004 | 2 / 2 | 3 / 1 / 0 | 6 | 0 / 0 (0) | none measured; small direct dependency set |
| `coder-cloud`<br>`crates/coder-cloud` | 17,800 / 16,776 | 26 / 41 | 339 / 2 / 18 | 63 | 9 / 14 (14) | `coder-access`, `coder-host`, `openagents-sdk`, `microcoder-loop`, `reqwest`, `tokio` |
| `coder-cloud-web`<br>`crates/coder-cloud-web` | 845 / 845 | 3 / 4 | 16 / 1 / 0 | 6 | 0 / 0 (0) | none measured; small direct dependency set |
| `coder-components-web`<br>`crates/coder-components-web` | 1,813 / 1,813 | 4 / 4 | 6 / 2 / 3 | 5 | 0 / 0 (0) | `rust-native`, `coder-ui`, `coder-demo-ui`, `rust-native-web` |
| `coder-compositor`<br>`crates/coder-compositor` | 15,057 / 15,057 | 39 / 60 | 319 / 0 / 3 | 2 | 0 / 0 (0) | `coder-hands`, `coder-desk`, `coder-binds`, `coder-wm`, `smithay` |
| `coder-computers`<br>`crates/coder-computers` | 19,333 / 16,673 | 30 / 38 | 254 / 9 / 23 | 7 | 5 / 23 (125) | `nostr`, `openagents-chat`, `coder-access`, `coder-host`?, `tokio`?, `secp256k1` |
| `coder-connect`<br>`crates/coder-connect` | 8,067 / 7,900 | 19 / 22 | 168 / 4 / 44 | 5 | 10 / 95 (155) | `nostr`, `coder-history`, `tokio`, `secp256k1` |
| `coder-control`<br>`crates/coder-control` | 4,056 / 4,056 | 13 / 12 | 54 / 2 / 20 | 2 | 0 / 0 (0) | `nostr`, `coder`?, `tokio`, `secp256k1` |
| `coder-delegate`<br>`crates/coder-delegate` | 39,453 / 39,058 | 54 / 92 | 1074 / 9 / 4 | 40 | 10 / 50 (117) | `openagents-harness`, `coder-lease`, `knowledge`, `jev`, `reqwest`, `tokio` |
| `coder-demo-ui`<br>`crates/coder-demo-ui` | 2,883 / 2,650 | 11 / 10 | 33 / 7 / 5 | 12 | 4 / 5 (5) | `rust-native`, `coder-ui` |
| `coder-desk`<br>`crates/coder-desk` | 5,318 / 5,318 | 9 / 11 | 186 / 4 / 6 | 2 | 3 / 75 (135) | `coder-binds`, `tokio` |
| `coder-desk-cli`<br>`crates/coder-desk-cli` | 985 / 594 | 2 / 0 | 0 / 0 / 0 | 1 | 0 / 0 (0) | `coder-desk` |
| `coder-engine-status`<br>`crates/coder-engine-status` | 1,555 / 1,555 | 4 / 7 | 50 / 1 / 0 | 6 | 7 / 84 (143) | none measured; small direct dependency set |
| `coder-environment`<br>`crates/coder-environment` | 7,256 / 7,256 | 13 / 13 | 207 / 5 / 2 | 12 | 9 / 15 (15) | `boat`, `computer-driver`, `atif` |
| `coder-environment-build`<br>`crates/coder-environment-build` | 3,410 / 3,170 | 6 / 4 | 68 / 0 / 1 | 10 | 2 / 5 (5) | `coder-working-computer`, `coder-environment`, `coder-environment-setup` |
| `coder-environment-operator`<br>`crates/coder-environment-operator` | 15,650 / 14,278 | 25 / 32 | 310 / 3 / 44 | 42 | 3 / 3 (3) | `coder-access`, `coder-cloud`, `openagents-sdk`, `boat`, `reqwest`, `tokio` |
| `coder-environment-setup`<br>`crates/coder-environment-setup` | 4,985 / 4,325 | 9 / 6 | 121 / 1 / 3 | 11 | 3 / 6 (6) | `coder-access`, `coder-cloud`, `coder-working-computer`, `coder-environment`, `tokio` |
| `coder-environment-verify`<br>`crates/coder-environment-verify` | 3,610 / 3,516 | 6 / 4 | 81 / 0 / 1 | 7 | 1 / 4 (4) | `coder-working-computer`, `coder-environment`, `coder-environment-setup`, `coder-environment-build` |
| `coder-hands`<br>`crates/coder-hands` | 5,385 / 5,385 | 13 / 21 | 234 / 1 / 1 | 2 | 4 / 7 (7) | `jev`?, `jev-hosted`?, `coder-wm`, `ort`?, `tokio`? |
| `coder-hands-measure`<br>`crates/coder-hands-measure` | 2,981 / 2,981 | 13 / 12 | 69 / 0 / 0 | 2 | 0 / 0 (0) | `coder-hands` |
| `coder-history`<br>`crates/coder-history` | 8,256 / 8,256 | 14 / 17 | 97 / 2 / 14 | 2 | 11 / 97 (157) | `rusqlite`? |
| `coder-host`<br>`crates/coder-host` | 33,179 / 24,662 | 85 / 122 | 472 / 17 / 151 | 40 | 17 / 72 (133) | `nostr`, `pylon`?, `openagents-chat`?, `coder-access`, `reqwest`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `coder-host-wire`<br>`crates/coder-host-wire` | 582 / 582 | 1 / 1 | 23 / 0 / 0 | 2 | 4 / 74 (135) | `nostr`, `coder-pty`, `coder-reach` |
| `coder-labor`<br>`crates/coder-labor` | 8,269 / 8,269 | 16 / 15 | 103 / 2 / 24 | 7 | 1 / 1 (1) | `nostr`, `pay-ledger`, `coder`, `receipts`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `coder-lease`<br>`crates/coder-lease` | 18,555 / 17,275 | 43 / 51 | 382 / 12 / 21 | 41 | 18 / 79 (141) | none measured; small direct dependency set |
| `coder-link`<br>`crates/coder-link` | 2,135 / 2,135 | 7 / 6 | 62 / 5 / 4 | 1 | 4 / 73 (134) | none measured; small direct dependency set |
| `coder-mobile`<br>`crates/coder-mobile` | 20,098 / 20,098 | 24 / 35 | 118 / 4 / 135 | 17 | 1 / 1 (117) | `nostr`, `verse-world`, `verse`, `openagents-chat-app`, `image`, `tokio`, `secp256k1` |
| `coder-mobile-probe`<br>`crates/coder-mobile-probe` | 652 / 652 | 6 / 5 | 7 / 0 / 2 | 3 | 0 / 0 (0) | `atif` |
| `coder-new`<br>`crates/coder-new` | 144,395 / 135,868 | 204 / 297 | 2232 / 10 / 534 | 484 | 1 / 1 (1) | `nostr`, `coder-delegate`, `inference`, `coder-sdk`, `reqwest`, `tokio` |
| `coder-one`<br>`crates/coder-one` | 132,869 / 132,606 | 195 / 274 | 2580 / 29 / 73 | 10 | 0 / 0 (0) | `coder-delegate`, `coder-sdk`, `jev`, `coder-history`, `reqwest`, `tokio` |
| `coder-project`<br>`crates/coder-project` | 8,914 / 8,197 | 15 / 19 | 131 / 0 / 0 | 2 | 1 / 1 (3) | `gym`, `coder-access`, `coder-host`, `jev`, `tokio` |
| `coder-pty`<br>`crates/coder-pty` | 13,383 / 9,970 | 24 / 26 | 393 / 5 / 65 | 6 | 15 / 83 (142) | `coder-boundary`?, `supervise`?, `coder-engine-status` |
| `coder-reach`<br>`crates/coder-reach` | 5,305 / 4,401 | 15 / 18 | 155 / 1 / 15 | 1 | 9 / 91 (151) | `nostr`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `coder-router`<br>`crates/coder-router` | 33,551 / 33,551 | 42 / 70 | 1003 / 11 / 7 | 1 | 2 / 30 (117) | `gym`, `nostr`, `coder-runtime`, `pylon`, `reqwest`, `tokio`, `tokio-tungstenite` |
| `coder-runtime`<br>`crates/coder-runtime` | 59,990 / 59,923 | 61 / 116 | 1288 / 23 / 12 | 1 | 3 / 31 (117) | `gym`, `nostr`, `coder-delegate`, `inference`, `reqwest`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `coder-scheduler`<br>`crates/coder-scheduler` | 4,516 / 4,182 | 8 / 11 | 75 / 0 / 0 | 3 | 1 / 2 (4) | none measured; small direct dependency set |
| `coder-sdk`<br>`crates/coder-sdk` | 19,764 / 19,764 | 46 / 71 | 590 / 20 / 11 | 53 | 10 / 48 (117) | `coder-delegate`, `jev`, `route-contract`, `claude_agent_sdk`, `reqwest`, `tokio` |
| `coder-service`<br>`crates/coder-service` | 6,452 / 6,452 | 14 / 16 | 146 / 0 / 1 | 4 | 3 / 73 (134) | `supervise`, `secp256k1` |
| `coder-sharing`<br>`crates/coder-sharing` | 2,392 / 2,075 | 5 / 4 | 43 / 0 / 8 | 3 | 1 / 1 (1) | `coder-access`, `coder-host`, `rust-native`, `computer-driver`, `tokio`, `secp256k1` |
| `coder-ssh`<br>`crates/coder-ssh` | 3,459 / 2,221 | 11 / 13 | 58 / 6 / 25 | 2 | 3 / 26 (126) | `supervise` |
| `coder-sync`<br>`crates/coder-sync` | 11,368 / 11,368 | 26 / 35 | 282 / 2 / 9 | 57 | 2 / 2 (2) | `openagents-sdk`, `openagents-doc`, `openagents-sync`, `atif`, `loro`, `reqwest`, `tokio` |
| `coder-task`<br>`crates/coder-task` | 156,617 / 156,617 | 210 / 263 | 2853 / 29 / 367 | 1 | 1 / 29 (117) | `gym`, `nostr`, `coder-runtime`, `tenancy`, `image`, `reqwest`, `tokio`, `secp256k1` |
| `coder-terminal`<br>`crates/coder-terminal` | 12,196 / 11,476 | 28 / 49 | 252 / 17 / 12 | 13 | 6 / 47 (126) | `rust-native`, `coder-ui`, `code-highlight`, `tokio` |
| `coder-ui`<br>`crates/coder-ui` | 15,749 / 15,749 | 35 / 49 | 376 / 3 / 10 | 21 | 22 / 70 (142) | `nostr`, `rust-native`, `atif`, `oa-tokens` |
| `coder-vt`<br>`crates/coder-vt` | 8,326 / 5,809 | 20 / 14 | 125 / 7 / 16 | 1 | 11 / 80 (142) | `coder-pty` |
| `coder-wm`<br>`crates/coder-wm` | 1,927 / 1,927 | 3 / 3 | 53 / 1 / 12 | 1 | 2 / 8 (8) | none measured; small direct dependency set |
| `coder-working-computer`<br>`crates/coder-working-computer` | 9,270 / 7,828 | 14 / 12 | 202 / 4 / 3 | 30 | 5 / 8 (8) | `coder-cloud`, `boat`, `coder-environment`, `computer-driver`, `tokio` |
| `coderbench`<br>`crates/coderbench` | 6,054 / 3,985 | 13 / 11 | 87 / 2 / 0 | 15 | 0 / 0 (117) | `gym`, `coder-boundary`, `atif`, `supervise` |
| `coderos-camera`<br>`crates/coderos-camera` | 2,815 / 2,815 | 13 / 28 | 92 / 2 / 0 | 2 | 0 / 0 (0) | `coder-hands` |
| `codex-transport`<br>`crates/codex-transport` | 2,275 / 2,275 | 6 / 8 | 75 / 1 / 0 | 3 | 9 / 65 (120) | `reqwest`, `tokio` |
| `commercial-accounts`<br>`crates/commercial-accounts` | 2,441 / 2,441 | 3 / 2 | 25 / 0 / 0 | 5 | 4 / 4 (117) | `nostr`?, `tenancy`, `pay-ledger`, `openagents-x402`? |
| `commercial-spend`<br>`crates/commercial-spend` | 3,421 / 2,035 | 10 / 7 | 32 / 2 / 27 | 5 | 1 / 1 (117) | `nostr`, `tenancy`, `pay-ledger`, `openagents-x402` |
| `compute-workbench`<br>`crates/compute-workbench` | 3,437 / 2,391 | 6 / 9 | 69 / 0 / 4 | 10 | 5 / 14 (117) | `terminal-core`?, `pay-ledger`?, `route-contract`, `receipts`, `reqwest`? |
| `computer-conformance`<br>`crates/computer-conformance` | 1,754 / 602 | 5 / 0 | 7 / 0 / 0 | 15 | 0 / 0 (4) | `boat`, `computer-driver`, `tokio` |
| `computer-credentials`<br>`crates/computer-credentials` | 3,116 / 3,074 | 10 / 10 | 98 / 0 / 1 | 5 | 3 / 25 (25) | `reqwest`, `tokio`, `axum` |
| `computer-driver`<br>`crates/computer-driver` | 6,903 / 6,619 | 12 / 18 | 109 / 2 / 6 | 43 | 9 / 23 (23) | `boat`?, `archil`?, `computer-credentials`?, `reqwest`?, `tokio`, `tokio-tungstenite`? |
| `computer-enrolled`<br>`crates/computer-enrolled` | 1,022 / 605 | 2 / 1 | 10 / 0 / 0 | 3 | 2 / 3 (3) | `coder-access`, `coder-host`, `computer-driver`, `tokio` |
| `computer-files`<br>`crates/computer-files` | 811 / 502 | 5 / 3 | 16 / 0 / 1 | 3 | 8 / 40 (127) | none measured; small direct dependency set |
| `contribution-service`<br>`crates/contribution-service` | 3,737 / 3,670 | 8 / 6 | 38 / 0 / 3 | 3 | 0 / 0 (0) | `gym`, `nostr`, `tenancy`, `pay-ledger`, `rusqlite` |
| `contribution-workbench`<br>`crates/contribution-workbench` | 2,692 / 2,692 | 5 / 6 | 18 / 0 / 2 | 1 | 3 / 11 (118) | `nostr`, `openagents-chat`?, `terminal-core`?, `knowledge`? |
| `coordination-host`<br>`crates/coordination-host` | 7,734 / 7,602 | 27 / 25 | 85 / 1 / 53 | 15 | 3 / 3 (3) | `nostr`, `workspace-host`, `secp256k1` |
| `decision-panel`<br>`crates/decision-panel` | 1,089 / 1,089 | 1 / 1 | 40 / 0 / 0 | 1 | 3 / 20 (122) | `rust-native`, `route-contract` |
| `design-lab`<br>`crates/design-lab` | 3,299 / 3,299 | 4 / 5 | 59 / 0 / 0 | 15 | 0 / 0 (0) | `lab-kit`, `tokio`, `axum` |
| `dhruv-conformance`<br>`crates/dhruv-conformance` | 1,496 / 3 | 6 / 1 | 1 / 0 / 0 | 5 | 0 / 0 (0) | none measured; small direct dependency set |
| `discovery`<br>`crates/discovery` | 2,825 / 2,106 | 7 / 8 | 73 / 0 / 0 | 5 | 5 / 5 (118) | `nostr` |
| `env-host`<br>`crates/env-host` | 10,046 / 9,165 | 18 / 14 | 96 / 0 / 34 | 14 | 1 / 1 (1) | `nostr`, `tenancy`, `coordination-host`, `oa-boat`, `tokio`, `secp256k1` |
| `eval-runner`<br>`crates/eval-runner` | 6,812 / 4,346 | 18 / 18 | 132 / 0 / 0 | 4 | 0 / 0 (0) | `nostr`, `ext-eval`, `jev`, `coder`, `reqwest`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `everglade-web`<br>`crates/everglade-web` | 5,588 / 5,588 | 13 / 15 | 67 / 1 / 8 | 32 | 0 / 0 (0) | `nostr`, `verse-world`, `verse`, `coder-access`, `wgpu`, `secp256k1` |
| `ext-eval`<br>`crates/ext-eval` | 20,846 / 16,760 | 51 / 68 | 570 / 15 / 9 | 4 | 5 / 31 (117) | `gym`, `nostr`, `jev`, `coder-boundary`?, `reqwest`?, `tokio`?, `axum`? |
| `game-lab`<br>`crates/game-lab` | 2,940 / 2,940 | 7 / 11 | 67 / 0 / 3 | 11 | 2 / 2 (2) | `icecream-web`, `icecream-rules`, `lab-kit`, `tokio`, `axum` |
| `gateway`<br>`crates/gateway` | 91,744 / 59,832 | 116 / 143 | 567 / 4 / 334 | 122 | 0 / 0 (117) | `gym`, `nostr`, `tenancy`, `pylon`, `reqwest`, `tokio`, `axum`, `tokio-tungstenite` |
| `github-actions`<br>`crates/github-actions` | 3,586 / 3,586 | 8 / 7 | 94 / 3 / 11 | 4 | 6 / 76 (137) | `reqwest`? |
| `gym`<br>`crates/gym` | 112,988 / 108,457 | 111 / 172 | 1913 / 3 / 96 | 26 | 17 / 44 (117) | `knowledge`, `pay-ledger`, `jev`, `coder-terminal`?, `tokio` |
| `gym-bridge`<br>`crates/gym-bridge` | 3,239 / 3,239 | 10 / 10 | 72 / 6 / 18 | 1 | 3 / 10 (117) | `gym`?, `nostr`, `coder-connect`, `supervise`?, `tokio`, `secp256k1` |
| `gym-leaderboard`<br>`crates/gym-leaderboard` | 12,057 / 10,376 | 24 / 27 | 262 / 4 / 4 | 2 | 5 / 12 (117) | `gym`?, `nostr`, `knowledge`?, `atif`, `reqwest`?, `tokio`?, `tokio-tungstenite`? |
| `hook-log`<br>`crates/hook-log` | 544 / 544 | 3 / 3 | 20 / 0 / 0 | 5 | 4 / 49 (118) | none measured; small direct dependency set |
| `icecream-rules`<br>`crates/icecream-rules` | 6,092 / 6,092 | 9 / 16 | 293 / 6 / 0 | 8 | 2 / 4 (4) | none measured; small direct dependency set |
| `icecream-web`<br>`crates/icecream-web` | 8,582 / 8,582 | 11 / 17 | 272 / 0 / 0 | 14 | 1 / 3 (3) | `icecream-rules` |
| `inference`<br>`crates/inference` | 27,264 / 22,262 | 67 / 82 | 832 / 15 / 12 | 33 | 9 / 65 (120) | `openrouter`, `reqwest`, `tokio` |
| `jev`<br>`crates/jev` | 18,675 / 11,954 | 49 / 35 | 506 / 22 / 37 | 34 | 27 / 104 (146) | `receipts`, `reqwest`, `tokio` |
| `jev-hosted`<br>`crates/jev-hosted` | 2,818 / 2,138 | 4 / 1 | 53 / 0 / 0 | 11 | 13 / 71 (117) | `nostr`, `jev`, `atif`, `model-access`, `reqwest`, `tokio`, `secp256k1` |
| `kev`<br>`crates/kev` | 6,484 / 3,773 | 21 / 18 | 119 / 8 / 14 | 2 | 0 / 0 (0) | `receipts`?, `candle-core`, `tokenizers`, `tokio`?, `axum`? |
| `knowledge`<br>`crates/knowledge` | 14,288 / 14,261 | 32 / 33 | 349 / 7 / 6 | 19 | 13 / 64 (119) | `nostr`, `inference`, `xp-ledger`, `openui-lang`, `secp256k1` |
| `lab-api`<br>`crates/lab-api` | 4,260 / 2,109 | 9 / 4 | 33 / 2 / 6 | 8 | 2 / 2 (117) | `lab-store`, `lab-jobs`, `lab-kit`, `tokio`, `axum` |
| `lab-jobs`<br>`crates/lab-jobs` | 3,928 / 2,062 | 14 / 7 | 52 / 4 / 0 | 3 | 4 / 4 (117) | `openagents-sdk`?, `lab-store`, `lab-kit`, `tokio` |
| `lab-kit`<br>`crates/lab-kit` | 2,081 / 1,774 | 12 / 12 | 94 / 5 / 1 | 6 | 20 / 168 (180) | none measured; small direct dependency set |
| `lab-store`<br>`crates/lab-store` | 2,930 / 2,230 | 8 / 3 | 62 / 2 / 7 | 5 | 6 / 110 (158) | `lab-kit`, `sqlx-postgres`, `reqwest`, `tokio` |
| `lab-terminal`<br>`crates/lab-terminal` | 714 / 714 | 1 / 1 | 15 / 1 / 0 | 4 | 2 / 2 (2) | `rust-native`, `openagents-sdk`, `tokio` |
| `lab-ui`<br>`crates/lab-ui` | 233 / 233 | 1 / 1 | 6 / 0 / 0 | 4 | 7 / 47 (47) | `rust-native`, `lab-kit` |
| `labor-roles`<br>`crates/labor-roles` | 4,011 / 4,011 | 10 / 9 | 85 / 0 / 6 | 6 | 1 / 2 (3) | `nostr`, `pay-ledger`, `workspace-host`, `secp256k1` |
| `lander-gl`<br>`crates/lander-gl` | 1,041 / 907 | 5 / 4 | 9 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `lander5-web`<br>`crates/lander5-web` | 785 / 785 | 4 / 4 | 24 / 0 / 0 | 5 | 2 / 2 (2) | none measured; small direct dependency set |
| `laya`<br>`crates/laya` | 3,985 / 3,618 | 15 / 15 | 102 / 8 / 3 | 1 | 0 / 0 (0) | `candle-core`, `candle-transformers`, `tokenizers`, `tokio`? |
| `layertree-web`<br>`crates/layertree-web` | 2,960 / 2,960 | 8 / 15 | 89 / 0 / 0 | 3 | 0 / 0 (0) | `verse`, `verse-engine`, `wgpu` |
| `lev`<br>`crates/lev` | 13,573 / 11,045 | 32 / 25 | 264 / 9 / 0 | 2 | 0 / 0 (0) | `gym`, `jev`, `tokio`?, `axum`? |
| `live-host`<br>`crates/live-host` | 6,634 / 6,447 | 18 / 16 | 79 / 1 / 21 | 10 | 0 / 0 (0) | `nostr`, `workspace-host`, `secp256k1` |
| `lnd-sdk`<br>`crates/lnd-sdk` | 63,326 / 61,948 | 62 / 188 | 2471 / 2 / 15 | 27 | 2 / 2 (2) | `nostr`, `pay-ledger`, `openagents-wallet`, `supervise`?, `tonic`, `prost`, `bitcoin`, `lightning-invoice` |
| `mac-jobs`<br>`crates/mac-jobs` | 3,121 / 2,419 | 7 / 5 | 74 / 3 / 0 | 5 | 4 / 5 (5) | `actors` |
| `markdown-stream`<br>`crates/markdown-stream` | 942 / 942 | 3 / 3 | 7 / 0 / 0 | 5 | 5 / 169 (180) | none measured; small direct dependency set |
| `memory-stream`<br>`crates/memory-stream` | 378 / 378 | 1 / 1 | 24 / 0 / 0 | 3 | 3 / 31 (119) | none measured; small direct dependency set |
| `microcoder`<br>`crates/microcoder` | 23,207 / 23,040 | 32 / 48 | 138 / 7 / 90 | 54 | 1 / 1 (1) | `nostr`, `coder-delegate`, `coder-sdk`, `microcoder-loop`, `reqwest`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `microcoder-loop`<br>`crates/microcoder-loop` | 14,744 / 14,744 | 19 / 35 | 415 / 1 / 6 | 21 | 8 / 43 (117) | `nostr`, `coder-sdk`, `coder-lease`, `knowledge`, `reqwest`, `tokio` |
| `microluna`<br>`crates/microluna` | 5,080 / 4,074 | 10 / 11 | 107 / 9 / 0 | 1 | 1 / 1 (1) | `coder-delegate`, `coder-boundary`, `atif`, `supervise`, `reqwest`, `tokio` |
| `model-access`<br>`crates/model-access` | 2,574 / 2,574 | 5 / 9 | 131 / 1 / 0 | 4 | 12 / 102 (145) | `jev`, `reqwest`? |
| `nostr`<br>`crates/nostr` | 93,190 / 91,673 | 180 / 292 | 2222 / 94 / 90 | 54 | 91 / 203 (207) | `secp256k1` |
| `nostr-relay`<br>`crates/nostr-relay` | 27,994 / 17,364 | 44 / 37 | 263 / 9 / 15 | 2 | 0 / 0 (119) | `nostr`, `tokio-postgres`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `nostr-transport`<br>`crates/nostr-transport` | 397 / 397 | 2 / 2 | 13 / 0 / 0 | 3 | 21 / 131 (171) | `nostr`, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `oa-att`<br>`crates/oa-att` | 2,343 / 2,053 | 7 / 5 | 48 / 3 / 2 | 15 | 7 / 9 (10) | `nostr`, `receipts`, `tokio`?, `secp256k1` |
| `oa-auth`<br>`crates/oa-auth` | 22,074 / 18,336 | 40 / 46 | 436 / 12 / 79 | 48 | 3 / 3 (117) | `nostr`, `tenancy`, `reqwest`, `tokio`, `axum`? |
| `oa-boat`<br>`crates/oa-boat` | 6,706 / 5,541 | 16 / 13 | 92 / 3 / 1 | 16 | 1 / 2 (3) | `boat`, `computer-driver`, `reqwest`, `tokio`, `axum` |
| `oa-computers`<br>`crates/oa-computers` | 19,492 / 12,418 | 39 / 42 | 206 / 7 / 35 | 54 | 0 / 0 (0) | `coder-access`, `coder-host`, `boat`, `coder-working-computer`, `tokio-postgres`, `reqwest`, `tokio`, `axum` |
| `oa-connections`<br>`crates/oa-connections` | 5,282 / 4,927 | 11 / 15 | 206 / 1 / 4 | 4 | 3 / 4 (4) | `reqwest`, `tokio`?, `axum`? |
| `oa-copy`<br>`crates/oa-copy` | 669 / 669 | 1 / 2 | 11 / 0 / 0 | 7 | 6 / 93 (185) | none measured; small direct dependency set |
| `oa-seal`<br>`crates/oa-seal` | 424 / 424 | 1 / 1 | 12 / 0 / 0 | 2 | 3 / 3 (118) | none measured; small direct dependency set |
| `oa-tokens`<br>`crates/oa-tokens` | 2,699 / 2,699 | 7 / 12 | 155 / 1 / 0 | 4 | 5 / 73 (145) | none measured; small direct dependency set |
| `oa-vault`<br>`crates/oa-vault` | 2,751 / 2,751 | 10 / 9 | 137 / 4 / 5 | 6 | 6 / 6 (7) | none measured; small direct dependency set |
| `oa-vault-web`<br>`crates/oa-vault-web` | 727 / 727 | 3 / 2 | 60 / 0 / 0 | 2 | 0 / 0 (2) | `oa-vault` |
| `oak`<br>`crates/oak` | 6,243 / 3,903 | 9 / 2 | 46 / 3 / 3 | 2 | 0 / 0 (0) | `jev`, `discovery`, `reqwest`, `tokio`, `axum`?, `secp256k1`? |
| `openagents-chat`<br>`crates/openagents-chat` | 27,679 / 27,505 | 34 / 55 | 572 / 4 / 9 | 38 | 16 / 75 (136) | `nostr`, `coder-access`, `rust-native`, `route-contract`, `tokio`, `secp256k1` |
| `openagents-chat-app`<br>`crates/openagents-chat-app` | 45,908 / 45,908 | 57 / 103 | 1117 / 7 / 16 | 69 | 7 / 17 (120) | `nostr`, `openagents-chat`, `coder-host`, `rust-native`, `image`, `tokio`, `secp256k1` |
| `openagents-cli`<br>`crates/openagents-cli` | 118,489 / 104,756 | 221 / 346 | 276 / 4 / 715 | 270 | 0 / 0 (0) | `coder-new`, `gym`, `nostr`, `verse-world`?, `lightning-invoice`, `reqwest`, `tokio`, `axum` |
| `openagents-connect`<br>`crates/openagents-connect` | 6,150 / 4,113 | 19 / 17 | 209 / 2 / 3 | 5 | 11 / 73 (134) | `nostr`, `openagents-chat`, `coder-access`, `coder-connect`, `iroh`, `tokio` |
| `openagents-dashboard`<br>`crates/openagents-dashboard` | 643 / 643 | 1 / 1 | 43 / 0 / 0 | 5 | 3 / 93 (118) | none measured; small direct dependency set |
| `openagents-deck`<br>`crates/openagents-deck` | 3,173 / 3,173 | 9 / 14 | 104 / 7 / 0 | 2 | 3 / 31 (118) | `rust-native`?, `rust-native-desktop`? |
| `openagents-desktop`<br>`crates/openagents-desktop` | 59,963 / 59,946 | 80 / 142 | 1023 / 12 / 41 | 59 | 0 / 0 (117) | `nostr`, `verse`?, `openagents-chat-app`, `openagents-chat`, `resvg`?, `reqwest`?, `tokio`, `secp256k1` |
| `openagents-login`<br>`crates/openagents-login` | 1,004 / 1,004 | 3 / 3 | 37 / 0 / 0 | 10 | 4 / 93 (118) | `reqwest`, `tokio` |
| `openagents-sdk`<br>`crates/openagents-sdk` | 23,244 / 15,192 | 79 / 81 | 838 / 29 / 39 | 110 | 16 / 92 (117) | `nostr`?, `cad`?, `agent-lab`?, `lab-kit`?, `reqwest`?, `tokio`? |
| `openagents-spark`<br>`crates/spark-wallet` | 4,021 / 3,772 | 11 / 15 | 89 / 2 / 2 | 7 | 3 / 3 (3) | `nostr`, `breez-sdk-spark`, `tokio`, `secp256k1` |
| `openagents-terminal`<br>`crates/openagents-terminal` | 9,504 / 8,502 | 16 / 18 | 137 / 1 / 11 | 19 | 1 / 1 (1) | `nostr`, `openagents-chat-app`, `openagents-chat`, `coder-terminal`, `tokio` |
| `openagents-ui`<br>`crates/openagents-ui` | 24,649 / 24,500 | 75 / 91 | 951 / 49 / 83 | 96 | 2 / 2 (2) | `oa-tokens` |
| `openagents-wallet`<br>`crates/wallet` | 4,172 / 3,988 | 10 / 13 | 123 / 2 / 6 | 20 | 11 / 23 (121) | `ldk-node`?, `rusqlite` |
| `openagents-web`<br>`crates/openagents-web` | 201,424 / 200,578 | 370 / 520 | 798 / 6 / 2530 | 713 | 1 / 1 (1) | `nostr`, `tenancy`, `pylon`, `openagents-chat`, `sqlx-postgres`, `resvg`, `image`, `reqwest` |
| `openagents-web-site`<br>`crates/openagents-web-site` | 7,449 / 7,449 | 13 / 14 | 30 / 1 / 0 | 2 | 0 / 0 (0) | `openagents-web`, `nostr`, `tenancy`, `pylon`, `sqlx-postgres`, `resvg`, `image`, `reqwest` |
| `openagents-workspace-hack`<br>`crates/workspace-hack` | 3 / 1 | 2 / 0 | 0 / 0 / 0 | 2 | 287 / 290 (293) | `iroh-relay`, `tonic`, `prost`, `tokio-postgres` |
| `openagents-x402`<br>`crates/x402` | 14,135 / 11,681 | 21 / 31 | 327 / 4 / 10 | 27 | 9 / 77 (140) | `nostr`, `model-access` |
| `openrouter`<br>`crates/openrouter` | 2,907 / 2,514 | 3 / 1 | 62 / 0 / 0 | 10 | 9 / 66 (121) | `reqwest`, `tokio` |
| `openui-lang`<br>`crates/openui-lang` | 2,715 / 2,715 | 9 / 8 | 53 / 5 / 10 | 4 | 7 / 169 (180) | none measured; small direct dependency set |
| `paper-mono`<br>`crates/paper-mono` | 128 / 128 | 1 / 2 | 14 / 0 / 0 | 17 | 11 / 191 (198) | none measured; small direct dependency set |
| `pay-host`<br>`crates/pay-host` | 2,692 / 2,187 | 6 / 4 | 29 / 0 / 2 | 2 | 0 / 0 (0) | `nostr`, `openagents-chat-app`, `route-contract`, `rusqlite`, `tokio`, `axum` |
| `pay-ledger`<br>`crates/pay-ledger` | 19,125 / 14,175 | 48 / 32 | 412 / 3 / 43 | 22 | 18 / 94 (145) | `nostr`, `receipts`, `rusqlite`? |
| `physics`<br>`crates/physics` | 29,638 / 28,230 | 69 / 96 | 709 / 33 / 58 | 22 | 15 / 117 (143) | none measured; small direct dependency set |
| `playtest`<br>`crates/playtest` | 3,337 / 3,337 | 12 / 12 | 111 / 0 / 0 | 1 | 4 / 76 (137) | `nostr`, `secp256k1` |
| `plugin`<br>`crates/plugin` | 3,791 / 2,328 | 12 / 11 | 56 / 4 / 8 | 5 | 10 / 52 (118) | `nostr`, `wasmtime` |
| `plugin-action-items`<br>`crates/plugin-action-items` | 547 / 547 | 1 / 1 | 0 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-code-search`<br>`crates/plugin-code-search` | 465 / 451 | 5 / 3 | 5 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-dependency-check`<br>`crates/plugin-dependency-check` | 1,836 / 1,836 | 1 / 1 | 0 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-explain-error`<br>`crates/plugin-explain-error` | 1,838 / 1,821 | 2 / 1 | 3 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-hook-guard`<br>`crates/plugin-hook-guard` | 155 / 155 | 1 / 1 | 1 / 0 / 0 | 2 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-outline`<br>`crates/plugin-outline` | 205 / 205 | 1 / 0 | 3 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-pdk`<br>`crates/plugin-pdk` | 765 / 765 | 2 / 3 | 43 / 0 / 0 | 1 | 9 / 9 (9) | none measured; small direct dependency set |
| `plugin-registry`<br>`crates/plugin-registry` | 3,651 / 3,573 | 10 / 7 | 84 / 0 / 8 | 3 | 3 / 3 (3) | `nostr`, `reqwest`, `tokio`, `secp256k1` |
| `plugin-release-notes`<br>`crates/plugin-release-notes` | 803 / 803 | 1 / 1 | 0 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-repo-map`<br>`crates/plugin-repo-map` | 343 / 340 | 2 / 1 | 1 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `plugin-test-report`<br>`crates/plugin-test-report` | 718 / 715 | 2 / 1 | 1 / 0 / 0 | 1 | 0 / 0 (0) | none measured; small direct dependency set |
| `private-fs`<br>`crates/private-fs` | 545 / 545 | 2 / 3 | 13 / 1 / 0 | 1 | 13 / 109 (158) | none measured; small direct dependency set |
| `program-launch`<br>`crates/program-launch` | 567 / 567 | 1 / 1 | 10 / 0 / 0 | 3 | 2 / 53 (120) | none measured; small direct dependency set |
| `push-gateway`<br>`crates/push-gateway` | 4,882 / 3,764 | 14 / 19 | 146 / 1 / 2 | 2 | 4 / 5 (118) | `nostr`, `reqwest`, `tokio`, `axum`?, `secp256k1` |
| `pylon`<br>`crates/pylon` | 34,370 / 28,621 | 65 / 80 | 757 / 5 / 44 | 68 | 10 / 74 (136) | `nostr`, `coder-lease`, `pay-ledger`?, `openagents-x402`, `reqwest`, `tokio`, `tokio-tungstenite`?, `secp256k1` |
| `pylons-view`<br>`crates/pylons-view` | 601 / 601 | 2 / 3 | 23 / 0 / 0 | 2 | 5 / 6 (6) | none measured; small direct dependency set |
| `pylons-web`<br>`crates/pylons-web` | 1,169 / 1,169 | 5 / 6 | 25 / 1 / 0 | 6 | 2 / 2 (2) | none measured; small direct dependency set |
| `receipts`<br>`crates/receipts` | 9,553 / 9,339 | 18 / 25 | 263 / 3 / 3 | 13 | 21 / 121 (160) | none measured; small direct dependency set |
| `receipts-nostr`<br>`crates/receipts-nostr` | 204 / 182 | 2 / 1 | 6 / 0 / 0 | 1 | 1 / 2 (2) | `nostr`, `receipts` |
| `retail-cloud`<br>`crates/retail-cloud` | 11,343 / 7,755 | 33 / 28 | 319 / 0 / 15 | 9 | 6 / 16 (118) | `nostr`, `pay-ledger`, `boat`?, `route-contract`, `rusqlite`, `tokio`? |
| `retail-qualify`<br>`crates/retail-qualify` | 4,084 / 3,683 | 13 / 9 | 98 / 0 / 2 | 2 | 1 / 1 (117) | `pay-ledger`, `boat`, `route-contract`, `retail-cloud`, `rusqlite` |
| `retail-service`<br>`crates/retail-service` | 7,039 / 6,246 | 16 / 11 | 73 / 1 / 24 | 10 | 0 / 0 (117) | `nostr`, `tenancy`, `pay-ledger`, `boat`, `rusqlite`, `tokio`, `axum` |
| `robot-data`<br>`crates/robot-data` | 3,566 / 3,014 | 13 / 10 | 62 / 0 / 13 | 4 | 2 / 5 (119) | `supervise`?, `reqwest`? |
| `robot-description`<br>`crates/robot-description` | 2,281 / 1,731 | 6 / 5 | 49 / 3 / 0 | 5 | 6 / 108 (162) | none measured; small direct dependency set |
| `robot-lab`<br>`crates/robot-lab` | 4,718 / 3,531 | 14 / 10 | 129 / 5 / 5 | 13 | 12 / 107 (157) | `lab-store`?, `lab-kit`, `robot-description` |
| `robot-policy`<br>`crates/robot-policy` | 4,022 / 3,541 | 11 / 9 | 73 / 0 / 14 | 3 | 1 / 4 (4) | `robot-sim`, `robot-lab`, `robot-data`, `candle-core`, `tokenizers`, `reqwest`, `tokio` |
| `robot-sim`<br>`crates/robot-sim` | 6,208 / 4,267 | 14 / 7 | 138 / 0 / 1 | 13 | 3 / 11 (120) | `robot-lab`, `robot-description` |
| `robot-web`<br>`crates/robot-web` | 2,204 / 2,204 | 2 / 1 | 2 / 1 / 0 | 6 | 0 / 0 (0) | `verse`, `robot-workbench`, `robot-lab`, `robot-description`, `wgpu` |
| `robot-workbench`<br>`crates/robot-workbench` | 6,031 / 4,774 | 22 / 12 | 120 / 4 / 0 | 14 | 2 / 3 (3) | `gym`?, `verse`?, `rust-native`, `robot-sim`?, `tokio`?, `axum`? |
| `route-contract`<br>`crates/route-contract` | 9,833 / 9,833 | 31 / 41 | 437 / 6 / 3 | 13 | 22 / 86 (144) | `workbench` |
| `rust-native`<br>`crates/rust-native` | 17,381 / 17,333 | 32 / 46 | 408 / 9 / 46 | 30 | 49 / 167 (178) | `openui-lang`, `code-highlight`? |
| `rust-native-desktop`<br>`crates/rust-native-desktop` | 15,549 / 15,284 | 24 / 44 | 289 / 7 / 19 | 17 | 3 / 32 (122) | `rust-native`, `wgpu`?, `resvg`, `winit`? |
| `rust-native-web`<br>`crates/rust-native-web` | 1,303 / 1,303 | 4 / 5 | 6 / 2 / 1 | 4 | 4 / 4 (4) | `rust-native` |
| `sand-garden`<br>`crates/sand-garden` | 2,265 / 2,265 | 6 / 8 | 54 / 0 / 0 | 3 | 2 / 2 (2) | none measured; small direct dependency set |
| `secret-screen`<br>`crates/secret-screen` | 780 / 780 | 1 / 1 | 25 / 0 / 0 | 7 | 13 / 118 (139) | none measured; small direct dependency set |
| `session-host`<br>`crates/session-host` | 14,995 / 14,750 | 17 / 16 | 89 / 2 / 53 | 26 | 0 / 0 (0) | `nostr`, `coder-sdk`?, `env-host`, `coordination-host`, `tokio`?, `secp256k1` |
| `ship-lab`<br>`crates/ship-lab` | 953 / 953 | 3 / 3 | 14 / 0 / 0 | 4 | 2 / 2 (2) | `verse-lagrange2`, `lab-kit`, `tokio`, `axum` |
| `snap-rules`<br>`crates/snap-rules` | 5,211 / 5,211 | 13 / 22 | 195 / 4 / 0 | 4 | 5 / 13 (120) | none measured; small direct dependency set |
| `snap-share`<br>`crates/snap-share` | 169 / 169 | 1 / 1 | 10 / 0 / 0 | 2 | 3 / 12 (119) | `snap-rules` |
| `snap-web`<br>`crates/snap-web` | 7,107 / 7,107 | 15 / 24 | 211 / 0 / 0 | 8 | 1 / 9 (118) | `snap-rules` |
| `solver-box`<br>`crates/solver-box` | 2,694 / 2,694 | 6 / 5 | 49 / 0 / 0 | 8 | 0 / 0 (0) | `openagents-sdk`, `boat`, `reqwest`, `tokio` |
| `sov-custody`<br>`crates/sov-custody` | 1,907 / 1,832 | 7 / 4 | 34 / 0 / 0 | 4 | 0 / 0 (0) | `nostr`, `workspace-host`, `secp256k1` |
| `sov-lifecycle`<br>`crates/sov-lifecycle` | 1,923 / 1,859 | 6 / 3 | 19 / 0 / 0 | 3 | 0 / 0 (0) | `nostr`, `workspace-host`, `secp256k1` |
| `sov-treasury`<br>`crates/sov-treasury` | 2,325 / 2,285 | 7 / 4 | 37 / 0 / 0 | 4 | 0 / 0 (1) | `nostr`, `pay-ledger`, `openagents-x402`, `workspace-host`, `secp256k1` |
| `space-key-broker`<br>`crates/space-key-broker` | 1,011 / 874 | 6 / 4 | 11 / 0 / 0 | 3 | 0 / 0 (0) | `tonic`, `prost`, `reqwest`, `tokio` |
| `spark-wallet-web`<br>`crates/spark-wallet-web` | 242 / 242 | 2 / 1 | 1 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-spark` |
| `squeeze`<br>`crates/squeeze` | 1,133 / 1,133 | 3 / 3 | 29 / 0 / 0 | 6 | 3 / 3 (3) | `coder-sdk`, `jev`, `claude_agent_sdk`, `tokio` |
| `standing-jobs`<br>`crates/standing-jobs` | 256 / 256 | 2 / 3 | 15 / 0 / 0 | 2 | 3 / 31 (119) | none measured; small direct dependency set |
| `supervise`<br>`crates/supervise` | 4,315 / 3,249 | 17 / 14 | 61 / 5 / 52 | 4 | 24 / 104 (152) | `tokio`? |
| `tenancy`<br>`crates/tenancy` | 44,807 / 43,983 | 70 / 90 | 997 / 13 / 78 | 64 | 12 / 34 (117) | `gym`, `pay-ledger`, `receipts`, `tokio-postgres`?, `tokio`?, `secp256k1` |
| `terminal-app`<br>`crates/terminal-app` | 1,562 / 1,562 | 3 / 3 | 3 / 0 / 0 | 16 | 0 / 0 (0) | `openagents-chat`, `terminal-core`, `knowledge`, `terminal-gfx`, `wgpu`, `winit` |
| `terminal-control`<br>`crates/terminal-control` | 485 / 485 | 3 / 3 | 16 / 4 / 0 | 2 | 2 / 12 (120) | `terminal-core`, `tokio` |
| `terminal-core`<br>`crates/terminal-core` | 19,539 / 19,470 | 41 / 56 | 526 / 5 / 11 | 6 | 14 / 79 (138) | `coder-pty`, `route-contract`, `coder-vt`, `workbench` |
| `terminal-gfx`<br>`crates/terminal-gfx` | 6,311 / 6,311 | 16 / 20 | 124 / 7 / 4 | 5 | 5 / 11 (119) | `terminal-core`, `coder-ui`, `coder-pty`, `coder-vt`, `winit`? |
| `terminal-mux`<br>`crates/terminal-mux` | 1,656 / 817 | 4 / 0 | 12 / 0 / 0 | 1 | 0 / 0 (0) | `terminal-core`, `coder-terminal`, `coder-pty`, `coder-vt` |
| `terminal-remote`<br>`crates/terminal-remote` | 1,267 / 446 | 3 / 2 | 7 / 0 / 0 | 1 | 2 / 13 (121) | `coder-access`, `coder-host`, `terminal-core`, `coder-computers`, `tokio` |
| `terminal-studio`<br>`crates/terminal-studio` | 3,520 / 2,981 | 10 / 12 | 51 / 0 / 2 | 2 | 2 / 10 (117) | `openagents-chat`, `coder-access`, `coder-host`?, `terminal-core`, `tokio`? |
| `terminal-tty`<br>`crates/terminal-tty` | 1,077 / 835 | 4 / 3 | 5 / 0 / 0 | 2 | 1 / 1 (1) | `terminal-core`, `coder-pty`, `coder-vt` |
| `town-clock`<br>`crates/town-clock` | 723 / 723 | 1 / 1 | 38 / 0 / 0 | 5 | 4 / 16 (119) | none measured; small direct dependency set |
| `townsfolk`<br>`crates/townsfolk` | 4,830 / 4,830 | 11 / 10 | 206 / 3 / 10 | 4 | 3 / 15 (118) | `world-tree` |
| `user-home`<br>`crates/user-home` | 299 / 201 | 2 / 1 | 10 / 0 / 0 | 2 | 68 / 168 (206) | none measured; small direct dependency set |
| `vault-operator`<br>`crates/vault-operator` | 1,051 / 921 | 7 / 4 | 22 / 0 / 0 | 2 | 0 / 0 (0) | `nostr`, `oa-vault`, `vault-sealed`, `secp256k1` |
| `vault-sealed`<br>`crates/vault-sealed` | 1,504 / 1,162 | 6 / 5 | 42 / 0 / 0 | 4 | 1 / 1 (2) | `nostr`, `oa-vault`, `oa-att`, `secp256k1` |
| `verse`<br>`crates/verse` | 91,687 / 55,353 | 178 / 158 | 1279 / 70 / 80 | 188 | 7 / 8 (117) | `gym`?, `nostr`, `verse-world`, `verse-zone-everglade`, `wgpu`, `resvg`, `winit`?, `cpal`? |
| `verse-bake`<br>`crates/verse-bake` | 4,112 / 4,112 | 10 / 10 | 69 / 5 / 30 | 9 | 0 / 0 (0) | `verse-zone-everglade`?, `verse-pbr`, `wgpu`? |
| `verse-content`<br>`crates/verse-content` | 9,103 / 8,727 | 25 / 27 | 138 / 6 / 13 | 12 | 8 / 17 (129) | `verse-world`, `physics`, `verse-engine`, `resvg`?, `secp256k1`? |
| `verse-core`<br>`crates/verse-core` | 5,810 / 5,810 | 17 / 28 | 246 / 6 / 0 | 27 | 15 / 24 (126) | `verse-world`, `verse-pbr`, `verse-engine`, `coder-ui` |
| `verse-engine`<br>`crates/verse-engine` | 17,800 / 17,800 | 35 / 68 | 594 / 2 / 16 | 11 | 14 / 33 (135) | none measured; small direct dependency set |
| `verse-film`<br>`crates/verse-film` | 2,230 / 2,230 | 6 / 7 | 52 / 2 / 0 | 10 | 1 / 1 (1) | `video-lab`, `oa-tokens`?, `wgpu`?, `image`? |
| `verse-game`<br>`crates/verse-game` | 475 / 475 | 1 / 1 | 21 / 0 / 0 | 4 | 3 / 11 (120) | none measured; small direct dependency set |
| `verse-gfx`<br>`crates/verse-gfx` | 2,675 / 2,675 | 9 / 14 | 114 / 2 / 0 | 4 | 13 / 29 (130) | `verse-world`?, `verse-engine`, `coder-ui`, `wgpu` |
| `verse-gym`<br>`crates/verse-gym` | 4,475 / 4,475 | 12 / 16 | 139 / 2 / 1 | 4 | 1 / 9 (117) | `nostr`, `verse-pbr`, `pylon`, `coder-ui`, `tokio`, `secp256k1` |
| `verse-host`<br>`crates/verse-host` | 1,287 / 782 | 5 / 3 | 12 / 0 / 0 | 2 | 0 / 0 (118) | `verse-world`, `verse-engine`, `verse-content`, `tokio` |
| `verse-imported`<br>`crates/verse-imported` | 14,365 / 12,857 | 29 / 43 | 222 / 15 / 5 | 9 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `physics`, `verse-engine`, `wgpu`, `winit`?, `tokio`?, `secp256k1` |
| `verse-lagrange`<br>`crates/verse-lagrange` | 5,563 / 5,493 | 8 / 8 | 247 / 5 / 0 | 3 | 5 / 15 (125) | `physics` |
| `verse-lagrange2`<br>`crates/verse-lagrange2` | 5,385 / 5,385 | 10 / 9 | 231 / 4 / 3 | 5 | 2 / 13 (123) | `physics`, `verse-lagrange` |
| `verse-net`<br>`crates/verse-net` | 6,601 / 6,601 | 14 / 21 | 206 / 4 / 8 | 8 | 6 / 26 (128) | `nostr`, `coder-ui`, `xp-ledger`?, `world-tree`, `reqwest`?, `tokio`, `tokio-tungstenite`, `secp256k1` |
| `verse-pbr`<br>`crates/verse-pbr` | 38,341 / 35,559 | 65 / 104 | 933 / 17 / 43 | 73 | 17 / 25 (127) | `physics`, `verse-engine`, `coder-ui`, `cad-world`, `wgpu`, `winit`? |
| `verse-private`<br>`crates/verse-private` | 2,014 / 2,014 | 9 / 15 | 90 / 0 / 2 | 6 | 3 / 73 (134) | `nostr`, `reqwest`?, `tokio`?, `axum`? |
| `verse-water-spells`<br>`crates/verse-water-spells` | 6,756 / 6,756 | 12 / 13 | 237 / 6 / 17 | 4 | 2 / 11 (117) | `verse-world`, `verse-zone-everglade`, `verse-pbr`, `physics` |
| `verse-world`<br>`crates/verse-world` | 90,235 / 90,167 | 143 / 221 | 1952 / 28 / 364 | 23 | 21 / 32 (133) | `physics`, `coder-access`?, `verse-engine`, `cad-world`, `tokio`?, `secp256k1`? |
| `verse-zone-arena`<br>`crates/verse-zone-arena` | 2,791 / 2,791 | 6 / 5 | 141 / 1 / 3 | 2 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `coder-ui`, `verse-core` |
| `verse-zone-coast`<br>`crates/verse-zone-coast` | 1,338 / 1,338 | 7 / 12 | 51 / 3 / 0 | 4 | 1 / 9 (117) | `verse-world`, `verse-zone-everglade`, `verse-pbr`, `physics` |
| `verse-zone-crypt`<br>`crates/verse-zone-crypt` | 995 / 995 | 3 / 2 | 41 / 0 / 2 | 1 | 2 / 10 (119) | `verse-world`, `verse-pbr`, `verse-core` |
| `verse-zone-duel`<br>`crates/verse-zone-duel` | 5,490 / 5,490 | 8 / 7 | 209 / 1 / 3 | 1 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `physics`, `verse-core` |
| `verse-zone-everglade`<br>`crates/verse-zone-everglade` | 57,078 / 56,351 | 108 / 141 | 1441 / 16 / 64 | 101 | 6 / 14 (117) | `nostr`, `verse-world`, `openagents-chat-app`?, `verse-pbr`, `reqwest`, `tokio`? |
| `verse-zone-grove`<br>`crates/verse-zone-grove` | 8,223 / 8,223 | 16 / 19 | 233 / 5 / 27 | 8 | 3 / 12 (117) | `verse-world`, `verse-zone-everglade`, `verse-pbr`, `verse-engine` |
| `verse-zone-lab`<br>`crates/verse-zone-lab` | 2,764 / 2,764 | 4 / 3 | 44 / 2 / 0 | 3 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `physics`, `coder-ui` |
| `verse-zone-lagrange`<br>`crates/verse-zone-lagrange` | 1,640 / 1,640 | 3 / 4 | 33 / 0 / 0 | 2 | 2 / 10 (119) | `verse-world`, `verse-pbr`, `verse-core`, `verse-lagrange` |
| `verse-zone-lagrange2`<br>`crates/verse-zone-lagrange2` | 1,370 / 1,370 | 3 / 3 | 35 / 1 / 0 | 5 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `verse-core`, `verse-lagrange` |
| `verse-zone-robotlab`<br>`crates/verse-zone-robotlab` | 2,346 / 2,113 | 4 / 3 | 49 / 0 / 4 | 2 | 1 / 9 (118) | `verse-world`, `verse-pbr`, `verse-core`, `robot-sim`, `reqwest` |
| `verse-zone-water`<br>`crates/verse-zone-water` | 1,169 / 483 | 4 / 2 | 30 / 1 / 0 | 19 | 2 / 10 (117) | `verse-world`, `verse-zone-everglade`, `verse-pbr`, `physics` |
| `video-lab`<br>`crates/video-lab` | 6,233 / 6,233 | 13 / 21 | 96 / 2 / 3 | 5 | 5 / 11 (11) | `lab-kit` |
| `video-lab-serve`<br>`crates/video-lab-serve` | 717 / 717 | 2 / 1 | 9 / 0 / 0 | 2 | 0 / 0 (0) | `video-lab` |
| `voyager`<br>`crates/voyager` | 9,765 / 9,765 | 20 / 29 | 185 / 1 / 3 | 2 | 0 / 0 (0) | `nostr`, `jev`, `atif`, `supervise`, `mlua`, `reqwest` |
| `work-host`<br>`crates/work-host` | 4,808 / 4,721 | 14 / 11 | 45 / 0 / 39 | 2 | 2 / 2 (2) | `nostr`, `workspace-host`, `secp256k1` |
| `workbench`<br>`crates/workbench` | 2,306 / 1,367 | 4 / 1 | 72 / 0 / 0 | 2 | 15 / 110 (157) | none measured; small direct dependency set |
| `workbench-session`<br>`crates/workbench-session` | 1,194 / 570 | 4 / 1 | 24 / 0 / 0 | 2 | 2 / 2 (2) | `coder-pty`, `workbench` |
| `workflow-run`<br>`crates/workflow-run` | 2,518 / 2,518 | 4 / 3 | 28 / 1 / 34 | 2 | 1 / 2 (2) | `tokio` |
| `workspace-host`<br>`crates/workspace-host` | 4,348 / 4,202 | 12 / 9 | 91 / 0 / 32 | 5 | 12 / 12 (13) | `nostr`, `workbench`, `secp256k1` |
| `world-tree`<br>`crates/world-tree` | 1,790 / 1,790 | 6 / 5 | 82 / 3 / 0 | 11 | 6 / 43 (130) | none measured; small direct dependency set |
| `xp-ledger`<br>`crates/xp-ledger` | 4,588 / 4,588 | 11 / 13 | 118 / 1 / 0 | 3 | 8 / 79 (134) | `nostr` |

### `apps/openagents/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents`<br>`apps/openagents/apps/openagents` | 2,062 / 2,016 | 9 / 11 | 15 / 0 / 0 | 13 | 0 / 0 (0) | `openagents-ui`?, `openagents-engine`, `openagents-account`, `openagents-mcp`, `tokio` |
| `openagents-account`<br>`apps/openagents/crates/account` | 16,096 / 12,725 | 34 / 47 | 426 / 4 / 21 | 59 | 5 / 5 (5) | `nostr`, `rust-native`, `openagents-sdk`, `openagents-doc`, `tokio`, `secp256k1` |
| `openagents-agent-graph-view`<br>`apps/openagents/crates/agent-graph-view` | 1,384 / 1,384 | 3 / 4 | 27 / 1 / 0 | 1 | 2 / 4 (4) | `agent-graph`, `gpui` |
| `openagents-audio`<br>`apps/openagents/crates/audio` | 588 / 588 | 5 / 7 | 27 / 0 / 0 | 3 | 0 / 0 (0) | `openagents-proto`, `cpal`? |
| `openagents-background`<br>`apps/openagents/crates/background` | 1,545 / 1,545 | 8 / 7 | 53 / 4 / 0 | 5 | 2 / 2 (2) | `openagents-proto` |
| `openagents-brand`<br>`apps/openagents/crates/brand` | 204 / 204 | 1 / 2 | 29 / 0 / 0 | 8 | 23 / 110 (138) | none measured; small direct dependency set |
| `openagents-chat-view`<br>`apps/openagents/crates/chat-view` | 52,778 / 52,778 | 74 / 115 | 1382 / 15 / 42 | 30 | 24 / 38 (38) | `rust-native`, `openagents-doc`, `openagents-proto`, `openagents-visual`?, `gpui`, `image`? |
| `openagents-client`<br>`apps/openagents/crates/client` | 16,643 / 14,144 | 35 / 53 | 282 / 15 / 295 | 42 | 2 / 2 (2) | `openagents-doc`, `openagents-sync`, `openagents-proto`, `openagents-rpc`, `loro`, `reqwest`, `tokio` |
| `openagents-doc`<br>`apps/openagents/crates/doc` | 12,684 / 12,493 | 22 / 34 | 275 / 9 / 12 | 16 | 19 / 102 (130) | `openagents-proto`, `loro` |
| `openagents-engine`<br>`apps/openagents/crates/engine` | 124,458 / 90,469 | 144 / 212 | 838 / 28 / 414 | 133 | 2 / 2 (2) | `openagents-harness`, `coder-access`, `coder-host`, `coder-lease`, `fff-search`, `loro`, `rusqlite`, `reqwest` |
| `openagents-gpui-test`<br>`apps/openagents/crates/gpui-test` | 246 / 135 | 2 / 1 | 9 / 0 / 0 | 2 | 0 / 0 (39) | `gpui`, `gpui_platform`, `gpui_wgpu`, `image` |
| `openagents-harness`<br>`apps/openagents/crates/harness` | 63,035 / 47,291 | 108 / 134 | 302 / 20 / 327 | 40 | 3 / 51 (117) | `coder-lease`, `openagents-sdk`, `openagents-proto`, `supervise`?, `reqwest`, `tokio` |
| `openagents-host-upgrade`<br>`apps/openagents/crates/host-upgrade` | 2,908 / 2,360 | 11 / 14 | 92 / 4 / 0 | 7 | 3 / 3 (3) | `coder-lease` |
| `openagents-markdown`<br>`apps/openagents/crates/markdown` | 2,263 / 2,263 | 3 / 7 | 26 / 1 / 0 | 4 | 4 / 40 (40) | none measured; small direct dependency set |
| `openagents-mcp`<br>`apps/openagents/crates/mcp` | 10,552 / 10,552 | 20 / 37 | 160 / 6 / 5 | 46 | 3 / 4 (4) | `coder-lease`, `coder-cloud`, `openagents-sdk`, `microcoder-loop`, `reqwest`, `tokio` |
| `openagents-media-view`<br>`apps/openagents/crates/media-view` | 4,073 / 4,073 | 14 / 23 | 121 / 4 / 5 | 1 | 1 / 2 (2) | `openagents-chat-view`, `openagents-account`, `openagents-proto`, `openagents-visual`, `gpui`, `usvg`, `image` |
| `openagents-mobile`<br>`apps/openagents/crates/mobile` | 41,006 / 41,006 | 84 / 141 | 960 / 43 / 220 | 192 | 0 / 0 (0) | `coder-access`, `rust-native`, `coder-ui`, `openagents-client`, `breez-sdk-spark`, `uniffi`, `reqwest`, `tokio` |
| `openagents-orb`<br>`apps/openagents/crates/orb` | 2,788 / 2,788 | 15 / 16 | 102 / 6 / 0 | 3 | 4 / 59 (121) | none measured; small direct dependency set |
| `openagents-preview`<br>`apps/openagents/crates/preview` | 4,880 / 3,660 | 15 / 14 | 89 / 1 / 6 | 5 | 1 / 3 (3) | `openagents-proto`, `webrtc`, `reqwest`, `tokio`, `tokio-tungstenite` |
| `openagents-proto`<br>`apps/openagents/crates/proto` | 8,279 / 8,279 | 20 / 46 | 466 / 5 / 1 | 29 | 31 / 106 (134) | none measured; small direct dependency set |
| `openagents-rpc`<br>`apps/openagents/crates/rpc` | 5,026 / 3,544 | 13 / 17 | 299 / 4 / 0 | 29 | 9 / 59 (121) | `openagents-sync`, `openagents-proto`, `tokio`, `tokio-tungstenite` |
| `openagents-sync`<br>`apps/openagents/crates/sync` | 12,163 / 10,502 | 24 / 29 | 177 / 10 / 18 | 22 | 6 / 61 (122) | `nostr`, `coder-access`, `coder-host`, `openagents-doc`, `loro`, `rusqlite`?, `tokio`, `tokio-tungstenite` |
| `openagents-syntax`<br>`apps/openagents/crates/syntax` | 1,726 / 1,390 | 3 / 3 | 17 / 0 / 14 | 4 | 3 / 40 (40) | `tree-sitter`?, `tree-sitter-rust`? |
| `openagents-text`<br>`apps/openagents/crates/text` | 7,569 / 4,070 | 16 / 13 | 79 / 4 / 76 | 3 | 1 / 1 (1) | none measured; small direct dependency set |
| `openagents-theme`<br>`apps/openagents/crates/theme` | 5,147 / 5,147 | 5 / 7 | 86 / 2 / 0 | 6 | 5 / 39 (39) | none measured; small direct dependency set |
| `openagents-ui`<br>`apps/openagents/crates/ui` | 194,481 / 188,637 | 200 / 320 | 1453 / 54 / 1083 | 100 | 1 / 1 (1) | `openagents-engine`, `openagents-chat-view`, `openagents-harness`, `rust-native`, `gpui`, `gpui_platform`, `wry`, `usvg` |
| `openagents-update`<br>`apps/openagents/crates/update` | 2,583 / 2,583 | 2 / 3 | 47 / 0 / 2 | 5 | 3 / 3 (3) | `reqwest`, `tokio` |
| `openagents-veil`<br>`apps/openagents/crates/veil` | 606 / 606 | 2 / 3 | 25 / 1 / 0 | 4 | 3 / 40 (40) | none measured; small direct dependency set |
| `openagents-visual`<br>`apps/openagents/crates/visual` | 6,572 / 6,471 | 13 / 19 | 140 / 8 / 81 | 4 | 7 / 42 (46) | `naga`, `mlua`, `image` |
| `openagents-visual-gpu`<br>`apps/openagents/crates/visual-gpu` | 3,452 / 3,380 | 5 / 6 | 8 / 2 / 22 | 3 | 4 / 40 (40) | `openagents-visual`, `wgpu` |
| `openagents-voice`<br>`apps/openagents/crates/voice` | 1,011 / 981 | 4 / 4 | 25 / 0 / 2 | 5 | 2 / 3 (3) | `cpal`, `reqwest`, `tokio` |
| `openagents-voice-media`<br>`apps/openagents/crates/voice-media` | 674 / 674 | 1 / 1 | 14 / 0 / 0 | 3 | 2 / 52 (118) | `openagents-proto`, `openagents-voice-session`, `tokio` |
| `openagents-voice-session`<br>`apps/openagents/crates/voice-session` | 1,285 / 1,285 | 3 / 4 | 30 / 2 / 0 | 5 | 5 / 57 (120) | `openagents-proto`, `openagents-rpc`?, `openagents-orb`, `tokio`? |
| `openagents-workspace-hack`<br>`apps/openagents/crates/workspace-hack` | 3 / 1 | 2 / 0 | 0 / 0 / 0 | 2 | 118 / 118 (195) | `wgpu`, `uniffi`, `naga`, `image` |

### `apps/openagents/web/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-first-frame`<br>`apps/openagents/web/first-frame` | 1,211 / 1,211 | 1 / 1 | 24 / 0 / 0 | 3 | 4 / 40 (40) | none |
| `openagents-first-frame-capture`<br>`apps/openagents/web/first-frame-capture` | 730 / 730 | 4 / 3 | 8 / 1 / 0 | 3 | 0 / 0 (0) | `agent-graph`, `openagents-first-frame`, `gpui`, `gpui_wgpu`, `image` |
| `openagents-first-frame-paint`<br>`apps/openagents/web/first-frame-paint` | 954 / 954 | 3 / 3 | 26 / 1 / 0 | 6 | 1 / 38 (38) | `openagents-first-frame` |
| `openagents-first-frame-record`<br>`apps/openagents/web/first-frame-record` | 639 / 639 | 1 / 1 | 5 / 0 / 0 | 1 | 2 / 38 (38) | `openagents-first-frame`, `gpui` |
| `openagents-gpui-web`<br>`apps/openagents/web/gpui-web` | 5,434 / 5,182 | 16 / 17 | 44 / 11 / 57 | 13 | 31 / 37 (37) | `openagents-first-frame`?, `gpui`, `gpui_wgpu`?, `image` |
| `openagents-web-admin-analytics`<br>`apps/openagents/web/admin-analytics` | 25 / 25 | 1 / 1 | 2 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-admin-attribution`<br>`apps/openagents/web/admin-attribution` | 25 / 25 | 1 / 1 | 2 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-admin-drive-access`<br>`apps/openagents/web/admin-drive-access` | 25 / 25 | 1 / 1 | 2 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-agent-graph`<br>`apps/openagents/web/agent-graph` | 278 / 278 | 3 / 3 | 5 / 1 / 2 | 12 | 1 / 1 (1) | `openagents-gpui-web`, `agent-graph`, `openagents-agent-graph-view`, `gpui` |
| `openagents-web-agent-lab`<br>`apps/openagents/web/agent-lab` | 1,240 / 1,240 | 4 / 4 | 22 / 2 / 0 | 3 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-agents`<br>`apps/openagents/web/agents` | 181 / 181 | 2 / 1 | 3 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-cad`<br>`apps/openagents/web/cad` | 458 / 458 | 2 / 1 | 3 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-card`<br>`apps/openagents/web/card` | 510 / 510 | 3 / 6 | 6 / 0 / 5 | 13 | 1 / 1 (1) | `openagents-doc`, `openagents-proto`, `openagents-gpui-web`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-chat`<br>`apps/openagents/web/chat` | 10,368 / 10,368 | 22 / 32 | 108 / 1 / 65 | 62 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-doc`, `openagents-proto`, `gpui` |
| `openagents-web-chat-view`<br>`apps/openagents/web/chat-view` | 4,227 / 4,179 | 14 / 19 | 151 / 3 / 5 | 24 | 14 / 34 (34) | `openagents-chat-view`, `openagents-doc`, `openagents-proto`, `openagents-visual`?, `gpui`, `resvg` |
| `openagents-web-claude`<br>`apps/openagents/web/claude` | 18 / 18 | 1 / 0 | 2 / 0 / 0 | 2 | 0 / 0 (0) | `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-client`<br>`apps/openagents/web/client` | 4,253 / 4,253 | 17 / 25 | 213 / 4 / 7 | 19 | 28 / 29 (29) | `openagents-sdk`, `openagents-doc`, `openagents-proto`, `loro` |
| `openagents-web-cloud-settings`<br>`apps/openagents/web/cloud-settings` | 231 / 231 | 2 / 1 | 1 / 1 / 0 | 1 | 2 / 2 (2) | `openagents-chat-view`, `rust-native`, `openagents-web-client`, `gpui` |
| `openagents-web-connection-pages`<br>`apps/openagents/web/connection-pages` | 212 / 212 | 2 / 1 | 4 / 1 / 0 | 1 | 3 / 3 (3) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-connections`<br>`apps/openagents/web/connections` | 20 / 20 | 1 / 0 | 3 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-data`<br>`apps/openagents/web/data` | 18 / 18 | 1 / 0 | 2 / 0 / 0 | 2 | 0 / 0 (0) | `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-demo`<br>`apps/openagents/web/demo` | 1,606 / 1,606 | 5 / 14 | 15 / 0 / 11 | 17 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-doc`, `openagents-proto`, `gpui` |
| `openagents-web-dictation`<br>`apps/openagents/web/dictation` | 357 / 357 | 1 / 2 | 11 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-web-client` |
| `openagents-web-files`<br>`apps/openagents/web/files` | 2,585 / 2,585 | 4 / 6 | 39 / 0 / 17 | 6 | 0 / 0 (0) | `openagents-proto`, `openagents-gpui-web`, `openagents-web-client`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-graph-card`<br>`apps/openagents/web/graph-card` | 4,314 / 4,314 | 9 / 16 | 120 / 1 / 4 | 13 | 1 / 1 (1) | `openagents-doc`, `openagents-proto`, `openagents-gpui-web`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-hands`<br>`apps/openagents/web/hands` | 1,184 / 1,184 | 4 / 6 | 52 / 2 / 0 | 1 | 2 / 3 (3) | `coder-hands` |
| `openagents-web-keys`<br>`apps/openagents/web/keys` | 362 / 362 | 2 / 1 | 4 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-lab`<br>`apps/openagents/web/lab` | 211 / 211 | 2 / 1 | 3 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-lander-chat`<br>`apps/openagents/web/lander-chat` | 1,498 / 1,498 | 3 / 3 | 43 / 0 / 1 | 18 | 1 / 1 (1) | `openagents-chat-view`, `openagents-doc`, `openagents-proto`, `openagents-gpui-web`, `gpui` |
| `openagents-web-page-host`<br>`apps/openagents/web/page-host` | 449 / 449 | 2 / 1 | 14 / 2 / 0 | 5 | 16 / 16 (16) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-projects`<br>`apps/openagents/web/projects` | 441 / 441 | 2 / 1 | 3 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-pylon-card`<br>`apps/openagents/web/pylon-card` | 372 / 372 | 2 / 2 | 6 / 0 / 2 | 9 | 1 / 1 (1) | `openagents-gpui-web`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-room`<br>`apps/openagents/web/room` | 76 / 76 | 1 / 1 | 2 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-web-client` |
| `openagents-web-settings`<br>`apps/openagents/web/settings` | 2,192 / 2,192 | 4 / 4 | 25 / 0 / 9 | 9 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-proto`, `openagents-gpui-web`, `gpui` |
| `openagents-web-terminal`<br>`apps/openagents/web/terminal` | 20 / 20 | 1 / 0 | 3 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-traces`<br>`apps/openagents/web/traces` | 210 / 210 | 2 / 1 | 3 / 0 / 0 | 2 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-transcript-extras`<br>`apps/openagents/web/transcript-extras` | 702 / 702 | 3 / 3 | 14 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `openagents-proto`, `openagents-gpui-web`, `openagents-theme`, `gpui` |
| `openagents-web-vault`<br>`apps/openagents/web/vault` | 20 / 20 | 1 / 0 | 3 / 0 / 0 | 1 | 0 / 0 (0) | `openagents-chat-view`, `rust-native`, `openagents-gpui-web`, `openagents-web-client`, `gpui` |
| `openagents-web-visuals`<br>`apps/openagents/web/visuals` | 282 / 282 | 2 / 2 | 3 / 1 / 0 | 2 | 0 / 0 (0) | `openagents-doc`, `openagents-gpui-web`, `openagents-web-client`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-voice`<br>`apps/openagents/web/voice` | 379 / 379 | 2 / 1 | 8 / 0 / 0 | 3 | 0 / 0 (0) | `openagents-proto`, `openagents-gpui-web`, `openagents-web-client`, `openagents-web-chat-view`, `gpui` |
| `openagents-web-work`<br>`apps/openagents/web/work` | 1,350 / 1,350 | 3 / 3 | 27 / 0 / 0 | 4 | 0 / 0 (0) | `openagents-gpui-web`, `openagents-web-client`, `openagents-web-chat-view`, `gpui` |

### `apps/openagents/web/card/webgpu/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-web-card-webgpu`<br>`apps/openagents/web/card/webgpu` | 12 / 12 | 1 / 0 | 1 / 0 / 0 | 10 | 0 / 0 (0) | none measured; small direct dependency set |

### `apps/openagents/web/graph-card/webgpu/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-web-graph-card-webgpu`<br>`apps/openagents/web/graph-card/webgpu` | 12 / 12 | 1 / 0 | 1 / 0 / 0 | 11 | 0 / 0 (0) | `openagents-web-graph-card` |

### `apps/openagents/web/lander-chat/webgpu/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-web-lander-chat-webgpu`<br>`apps/openagents/web/lander-chat/webgpu` | 12 / 12 | 1 / 0 | 1 / 0 / 0 | 13 | 0 / 0 (0) | `openagents-web-lander-chat` |

### `apps/openagents/web/pylon-card/webgpu/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-web-pylon-card-webgpu`<br>`apps/openagents/web/pylon-card/webgpu` | 19 / 19 | 1 / 0 | 2 / 0 / 0 | 9 | 0 / 0 (0) | none measured; small direct dependency set |

### `crates/layertree-tee-lab/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `layertree-tee-lab`<br>`crates/layertree-tee-lab` | 14,739 / 9,121 | 46 / 41 | 319 / 1 / 5 | 33 | 3 / 3 (3) | `nostr`?, `bitcoin`, `reqwest`?, `tokio`?, `tokio-tungstenite`? |

### `crates/layertree-tee-lab/client/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `layertree-attest`<br>`crates/layertree-tee-lab/client` | 3,005 / 2,565 | 7 / 3 | 32 / 0 / 0 | 9 | 1 / 1 (1) | `nostr`, `layertree-tee-lab`, `oa-att`, `att-release`, `bitcoin`, `reqwest` |

### `crates/layertree-tee-lab/fuzz/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `layertree-tee-lab-fuzz`<br>`crates/layertree-tee-lab/fuzz` | 115 / 0 | 7 / 0 | 0 / 0 / 0 | 5 | 0 / 0 (0) | `layertree-tee-lab`, `layertree-attest`, `att-platforms`, `bitcoin` |

### `crates/layertree-tee-lab/server/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `layertree-signet-service`<br>`crates/layertree-tee-lab/server` | 931 / 931 | 3 / 1 | 11 / 0 / 0 | 4 | 1 / 2 (2) | `layertree-tee-lab`, `bitcoin`, `reqwest` |

### `crates/lnd-sdk/signet-lab/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `lnd-signet-lab`<br>`crates/lnd-sdk/signet-lab` | 1,801 / 1,801 | 3 / 0 | 0 / 0 / 0 | 8 | 0 / 0 (0) | `nostr`, `lnd-sdk`, `pay-ledger`, `openagents-x402`, `tonic`, `reqwest`, `tokio` |

### `crates/psionic/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `deepseek-recipe`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe` | 3,421 / 3,421 | 12 / 15 | 58 / 8 / 21 | 1 | 1 / 9 (13) | `deepseek-recipe-core`, `tokenizers` |
| `deepseek-recipe-core`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-core` | 1,534 / 1,534 | 9 / 13 | 63 / 2 / 0 | 1 | 3 / 11 (15) | none measured; small direct dependency set |
| `deepseek-recipe-encoding`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-encoding` | 1,644 / 1,644 | 5 / 5 | 18 / 2 / 0 | 1 | 1 / 9 (13) | `deepseek-recipe-core`, `tokenizers` |
| `deepseek-recipe-macros`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-macros` | 683 / 683 | 3 / 2 | 2 / 0 / 3 | 1 | 1 / 12 (16) | `syn` |
| `psionic-adapters`<br>`crates/psionic/crates/psionic-adapters` | 1,707 / 1,707 | 1 / 1 | 48 / 0 / 0 | 1 | 3 / 5 (5) | `psionic-core`, `psionic-datastream` |
| `psionic-apple-fm`<br>`crates/psionic/crates/psionic-apple-fm` | 6,770 / 6,755 | 10 / 15 | 205 / 8 / 0 | 1 | 1 / 5 (5) | `psionic-runtime`, `reqwest`, `tokio` |
| `psionic-array`<br>`crates/psionic/crates/psionic-array` | 6,230 / 6,137 | 2 / 1 | 184 / 0 / 0 | 1 | 2 / 10 (12) | `psionic-runtime`, `psionic-backend-cuda`, `psionic-compiler`, `psionic-ir` |
| `psionic-backend-cpu`<br>`crates/psionic/crates/psionic-backend-cpu` | 3,227 / 3,227 | 1 / 1 | 17 / 0 / 0 | 1 | 1 / 2 (14) | `psionic-runtime`, `psionic-compiler`, `psionic-ir`, `psionic-core` |
| `psionic-backend-cuda`<br>`crates/psionic/crates/psionic-backend-cuda` | 30,386 / 29,902 | 8 / 9 | 260 / 0 / 253 | 18 | 4 / 12 (12) | `psionic-runtime`, `psionic-compiler`, `psionic-ir`, `psionic-core`, `rayon` |
| `psionic-backend-metal`<br>`crates/psionic/crates/psionic-backend-metal` | 21,685 / 21,685 | 16 / 21 | 274 / 0 / 139 | 10 | 4 / 12 (13) | `psionic-runtime`, `psionic-compiler`, `psionic-ir`, `psionic-core` |
| `psionic-backend-tests`<br>`crates/psionic/crates/psionic-backend-tests` | 469 / 469 | 1 / 0 | 11 / 0 / 0 | 1 | 0 / 0 (15) | `psionic-runtime`, `psionic-ir`, `psionic-core` |
| `psionic-catalog`<br>`crates/psionic/crates/psionic-catalog` | 7,489 / 7,448 | 9 / 11 | 166 / 5 / 2 | 2 | 4 / 9 (13) | `reqwest` |
| `psionic-cluster`<br>`crates/psionic/crates/psionic-cluster` | 24,339 / 21,020 | 18 / 24 | 477 / 12 / 0 | 1 | 3 / 5 (5) | `psionic-runtime`, `psionic-net`, `psionic-datastream`, `tokio` |
| `psionic-collectives`<br>`crates/psionic/crates/psionic-collectives` | 3,858 / 3,858 | 3 / 3 | 77 / 1 / 0 | 1 | 1 / 4 (4) | `psionic-runtime` |
| `psionic-compiler`<br>`crates/psionic/crates/psionic-compiler` | 26,599 / 26,302 | 47 / 86 | 474 / 40 / 0 | 1 | 10 / 15 (15) | `psionic-runtime`, `psionic-ir`, `psionic-core` |
| `psionic-core`<br>`crates/psionic/crates/psionic-core` | 5,514 / 5,514 | 5 / 7 | 216 / 0 / 0 | 1 | 17 / 27 (27) | none measured; small direct dependency set |
| `psionic-data`<br>`crates/psionic/crates/psionic-data` | 37,406 / 36,895 | 55 / 83 | 830 / 41 / 0 | 1 | 5 / 7 (7) | `psionic-runtime`, `psionic-ir`, `psionic-datastream` |
| `psionic-datastream`<br>`crates/psionic/crates/psionic-datastream` | 3,818 / 3,818 | 1 / 1 | 115 / 0 / 0 | 1 | 5 / 10 (10) | `psionic-runtime` |
| `psionic-environments`<br>`crates/psionic/crates/psionic-environments` | 8,486 / 8,486 | 7 / 13 | 216 / 6 / 0 | 1 | 4 / 6 (6) | `psionic-models`, `psionic-data` |
| `psionic-eval`<br>`crates/psionic/crates/psionic-eval` | 162,211 / 156,513 | 421 / 450 | 2680 / 225 / 2 | 1 | 4 / 4 (4) | `psionic-runtime`?, `psionic-models`?, `psionic-data`?, `psionic-compiler`?, `reqwest` |
| `psionic-inference`<br>`crates/psionic/crates/psionic-inference` | 7,942 / 6,897 | 29 / 30 | 97 / 3 / 43 | 25 | 1 / 2 (2) | `psionic-backend-cuda`?, `psionic-backend-metal`?, `psionic-core`?, `ort`?, `tokenizers`, `tokio`?, `axum`? |
| `psionic-ir`<br>`crates/psionic/crates/psionic-ir` | 25,802 / 25,788 | 22 / 41 | 567 / 20 / 13 | 1 | 15 / 25 (25) | `psionic-core` |
| `psionic-models`<br>`crates/psionic/crates/psionic-models` | 89,353 / 87,566 | 111 / 191 | 2225 / 82 / 20 | 26 | 8 / 8 (12) | `psionic-runtime`, `psionic-ir`, `psionic-catalog`, `psionic-transformer`, `candle-core`, `candle-transformers`, `moshi`, `tokenizers` |
| `psionic-net`<br>`crates/psionic/crates/psionic-net` | 10,308 / 10,308 | 2 / 3 | 241 / 2 / 0 | 1 | 2 / 6 (6) | `psionic-runtime`, `tokio` |
| `psionic-nn`<br>`crates/psionic/crates/psionic-nn` | 5,542 / 5,542 | 4 / 7 | 163 / 3 / 0 | 1 | 4 / 10 (13) | `psionic-core` |
| `psionic-observe`<br>`crates/psionic/crates/psionic-observe` | 529 / 529 | 1 / 1 | 19 / 0 / 0 | 1 | 2 / 7 (7) | `tokio` |
| `psionic-optimize`<br>`crates/psionic/crates/psionic-optimize` | 4,622 / 4,622 | 1 / 1 | 137 / 0 / 0 | 1 | 1 / 5 (5) | none measured; small direct dependency set |
| `psionic-provider`<br>`crates/psionic/crates/psionic-provider` | 21,448 / 21,390 | 117 / 225 | 361 / 112 / 0 | 1 | 0 / 0 (1) | `psionic-eval`, `psionic-serve`, `psionic-runtime`, `psionic-models` |
| `psionic-research`<br>`crates/psionic/crates/psionic-research` | 49,757 / 47,478 | 281 / 275 | 1104 / 138 / 0 | 1 | 2 / 2 (2) | `psionic-train`, `psionic-eval`, `psionic-runtime`, `psionic-models` |
| `psionic-router`<br>`crates/psionic/crates/psionic-router` | 16,630 / 16,328 | 52 / 59 | 423 / 29 / 0 | 1 | 3 / 5 (5) | `psionic-runtime`, `psionic-models`, `psionic-compiler`, `psionic-ir` |
| `psionic-runtime`<br>`crates/psionic/crates/psionic-runtime` | 97,595 / 96,481 | 167 / 197 | 2419 / 98 / 2 | 1 | 20 / 24 (24) | `psionic-ir`, `psionic-core`, `reqwest` |
| `psionic-sandbox`<br>`crates/psionic/crates/psionic-sandbox` | 15,027 / 14,835 | 31 / 38 | 259 / 19 / 0 | 1 | 5 / 6 (6) | `psionic-runtime`, `psionic-models`, `psionic-compiler`, `psionic-core` |
| `psionic-serve`<br>`crates/psionic/crates/psionic-serve` | 167,490 / 152,299 | 167 / 180 | 1387 / 60 / 200 | 43 | 1 / 1 (1) | `psionic-train`, `psionic-eval`, `psionic-runtime`, `psionic-models`, `image`, `reqwest`, `tokio`, `axum` |
| `psionic-train`<br>`crates/psionic/crates/psionic-train` | 418,076 / 390,156 | 666 / 706 | 7007 / 361 / 128 | 2 | 2 / 3 (3) | `psionic-eval`, `psionic-runtime`, `psionic-models`, `psionic-data`, `reqwest`, `rayon`, `zstd` |
| `psionic-transformer`<br>`crates/psionic/crates/psionic-transformer` | 6,315 / 6,315 | 16 / 31 | 238 / 16 / 0 | 1 | 4 / 9 (12) | `psionic-array`, `psionic-nn`, `psionic-core` |

### `crates/robot-sim/tools/bindings/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `robot-mujoco-bindings`<br>`crates/robot-sim/tools/bindings` | 34 / 34 | 1 / 0 | 0 / 0 / 0 | 1 | 0 / 0 (0) | `bindgen` |

### `deploy/coder-sync/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `coder-sync-conformance`<br>`deploy/coder-sync/conformance` | 186 / 186 | 2 / 0 | 0 / 0 / 0 | 7 | 0 / 0 (0) | `nostr`, `coder-access`, `openagents-sdk`, `openagents-client`, `loro`, `image`, `reqwest`, `tokio` |
| `coder-sync-service`<br>`deploy/coder-sync` | 5,122 / 0 | 12 / 2 | 1 / 0 / 0 | 35 | 1 / 1 (1) | `nostr`, `coder-access`, `openagents-sdk`, `openagents-doc`, `tokio-postgres`, `reqwest`, `tokio`, `axum` |

### `deploy/iroh-relay/live-test/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `iroh-relay-live-test`<br>`deploy/iroh-relay/live-test` | 126 / 1 | 2 / 0 | 0 / 0 / 0 | 13 | 0 / 0 (0) | none measured; small direct dependency set |

### `deploy/space-credentials/fixture/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `space-credential-fixture`<br>`deploy/space-credentials/fixture` | 297 / 297 | 1 / 0 | 0 / 0 / 0 | 5 | 0 / 0 (0) | `computer-credentials`, `tokio`, `axum` |

### `deploy/space-host/fixtures/environment/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `environment-fixture`<br>`deploy/space-host/fixtures/environment` | 36 / 36 | 1 / 1 | 1 / 0 / 0 | 1 | 0 / 0 (0) | none |

### `docs/decision-models/2026-09-20-lev-domain-gap/analysis/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `lev-domain-gap-analysis`<br>`docs/decision-models/2026-09-20-lev-domain-gap/analysis` | 187 / 0 | 1 / 0 | 0 / 0 / 0 | 1 | 0 / 0 (0) | `gym` |

### `examples/sdk-history/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `sdk-history-example`<br>`examples/sdk-history` | 281 / 281 | 1 / 0 | 0 / 0 / 0 | 1 | 0 / 0 (0) | `tokio` |

### `examples/sdk-labs/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `sdk-labs-example`<br>`examples/sdk-labs` | 275 / 275 | 1 / 0 | 0 / 0 / 0 | 1 | 0 / 0 (0) | `tokio` |

### `mc-bridge/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `mc-bridge`<br>`mc-bridge` | 1,078 / 1,078 | 1 / 0 | 0 / 0 / 0 | 13 | 0 / 0 (0) | `tokio` |

### `quests/bridge-planner/fixture/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `bridge-planner`<br>`quests/bridge-planner/fixture` | 36 / 28 | 2 / 0 | 3 / 0 / 0 | 0 | 0 / 0 (0) | none |

### `support/workspace-hack/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `openagents-workspace-hack`<br>`support/workspace-hack` | 2 / 2 | 1 / 0 | 0 / 0 / 0 | 1 | 79 / 88 (175) | none |

### `vendor/boltz-client/Cargo.toml`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `boltz-cli`<br>`vendor/boltz-client/crates/cli` | 805 / 805 | 1 / 1 | 0 / 0 / 0 | 0 | 0 / 0 (0) | `boltz-client`, `tokio` |
| `boltz-client`<br>`vendor/boltz-client/crates/lib` | 17,577 / 16,827 | 34 / 59 | 296 / 9 / 45 | 0 | 1 / 1 (1) | `alloy-primitives`, `alloy-sol-types`, `lightning-invoice`, `tokio` |
| `macros`<br>`vendor/boltz-client/crates/macros` | 987 / 987 | 6 / 5 | 20 / 0 / 0 | 0 | 3 / 3 (3) | `syn` |
| `platform-utils`<br>`vendor/boltz-client/crates/platform-utils` | 977 / 977 | 5 / 6 | 28 / 8 / 0 | 0 | 1 / 2 (2) | `reqwest`, `tokio` |

### `nonmember support/fixture manifests`

| Crate and directory | Rust all / src | Files / mods | API P / U / R | 14d edits | Dependents D / T (+dev) | Selected heavy direct deps |
| --- | --- | --- | --- | --- | --- | --- |
| `ingest`<br>`crates/plugin-dependency-check/evals/loose-ranges/fixtures` | 0 / 0 | 0 / 0 | 0 / 0 / 0 | 0 | N/A: nonmember | none |
| `ledger`<br>`crates/plugin-dependency-check/fixtures/tree` | 0 / 0 | 0 / 0 | 0 / 0 / 0 | 0 | N/A: nonmember | none |
| `ledger`<br>`crates/plugin-explain-error/evals/rust-mismatch/fixtures` | 12 / 12 | 1 / 0 | 2 / 0 / 0 | 0 | N/A: nonmember | none |
| `lev-state-sweep-choice`<br>`docs/decision-models/2026-09-20-state-budget-choice/original-runner` | 463 / 0 | 1 / 0 | 1 / 0 / 0 | 0 | N/A: nonmember | none |
| `lev-state-sweep-choice-recovery`<br>`docs/decision-models/2026-09-20-state-budget-choice/recovery-runner` | 472 / 0 | 1 / 0 | 1 / 0 / 0 | 0 | N/A: nonmember | none |
| `openagents-workspace-hack`<br>`apps/openagents/web/hack-stub` | 1 / 1 | 1 / 0 | 0 / 0 / 0 | 1 | 76 / 117 (214) | none |
| `orders-api`<br>`crates/plugin-repo-map/evals/largest-file/fixtures` | 11 / 11 | 2 / 1 | 1 / 0 / 0 | 0 | N/A: nonmember | none |
| `relay-bridge`<br>`crates/plugin-dependency-check/evals/cargo-duplicates/fixtures` | 0 / 0 | 0 / 0 | 0 / 0 / 0 | 0 | N/A: nonmember | none |
| `tally`<br>`crates/ext-eval/fixtures/repo-map-brief/evals/overview/fixtures` | 284 / 280 | 4 / 1 | 2 / 0 / 0 | 0 | N/A: nonmember | none |

## Concrete splits and blockers

These are proposed crate names, not created packages. Reuse an existing
contract crate wherever possible; each new contract must stay small. The
root route proposal `openagents-web-chat-routes` is distinct from the existing
Wasm `openagents-web-chat` package. Every crate above 20k raw source lines
has an explicit proposal below. Other sizable crates have a staged seam or
a reason to retain their current boundary. Family names are staging boundaries:
measure each proposed child before landing it. If a family still exceeds 20k,
extract its named subfamilies in separate commits rather than treating another
100k-line crate as the end state. A temporary >40k child needs the same explicit
split plan as an existing oversize crate. Savings are estimates as defined
above, not additions to the measured #12184 savings.

### Phase 1 candidate boundaries

| Crate | Modules → proposed crates and dependency direction | Shared values / ports | Cycles, features, and other blockers | Expected local edit-build saving | Effort / risk |
| --- | --- | --- | --- | --- | --- |
| `psionic-train`<br>`crates/psionic/crates/psionic-train` | `parameter_golf_single_h100_training*` → `psionic-train-golf-single`; distributed 8xH100 train-step/mesh modules → `psionic-train-golf-distributed`; parameter_golf_reference/baseline_graph → `psionic-train-golf-reference`; `apple_adapter*`, `adapter_*`, `qwen_legal*` → adapter trainers (separate Apple and legal crates if >20k); `psion_*` → `psionic-train-psion`; `compiled_agent*`, coordinator/distributed contracts → `psionic-train-distributed`; cs336/reference studies → dedicated experiment crates. Thin train facade → trainers → train-core. | `psionic-train-core`: optimizer/checkpoint/rollout values, including `TrainingSessionState`, `TrainingRecoveryPlan`, and `TrainingSessionError`, plus trainer ports; retain tensor/runtime primitives in their existing lower crates. | Extensive root glob reexports and sibling imports tie experiments to lib.rs. Do not let eval depend on trainers; trainers may depend on eval-core. Forward legacy-apple-toolkit-oracle only to its adapter crate. Declared feature gates: `legacy-apple-toolkit-oracle`. | 50–80%; about 38.0–60.8 s of the sampled 76.00 s command if the split reaches that fraction | XL/high |
| `openagents-web`<br>`crates/openagents-web` | `pages/chat*`, `chat_api*`, `chat_store`, `session_*`, `continuation*` → `openagents-web-chat-routes`; `projects`, `cloud`, `computers_api`, `environments` → `openagents-web-computers`; `auth`, `account*`, `connections` → `openagents-web-account`; `cad_*`, `lab_*` → `openagents-web-labs`. Site → route crates → context/domain crates. Keep router wiring in the existing core until each family moves. | `openagents-web-context`: narrow backend ports, authenticated identity, error/response helpers, and per-family config; reuse SDK and route-contract wire types. | Handlers import core-private modules and the large Config. Extract ports first to prevent route → core → route cycles; return non-generic routers, preserving middleware order. Do not redo the landed site split. Declared feature gates: `test-support`. | 35–65%; about 5.6–10.4 s of the sampled 15.99 s command if the split reaches that fraction | L/medium |
| `openagents-ui`<br>`apps/openagents/crates/ui` | AFTER the active media-view split: `files`, `changes`, `pull_request_detail` → `openagents-workspace-view`; `settings`, `pickers` → `openagents-settings-view`; composer and transcript orchestration → `openagents-conversation-view`; shell/spaces/navigation → `openagents-shell-view`. Desktop → shell → view crates → shared state. | `openagents-ui-state` owns the EngineHandle interface and selected-chat/sidebar DTOs; reuse proto/doc/chat-view values. Keep prepared transcript/render caches in the conversation view. Move embedded-engine bootstrap above the views into app assembly; ui-state must not import concrete Engine. | `state.rs` and the 18.6k-line shell own GPUI entities and callbacks into child views. Move state interfaces first, inject actions upward, and forward the existing fixture features. Media, orbs, Lab cards, clipping, pills, cues, and fonts are RESERVED to #12184. Declared feature gates: `upstream-workos`, `account-fixture`, `voice-experimental`, `project-palette-fixture`, `compact-picker-fixture`, `browser-fixture`, `windows-render-fixture`, `appshots-fixture`, `pull-request-fixture`, `fork-chat-fixture`, `resource-profile`. | 25–55%; about 1.4–3.2 s of the sampled 5.76 s command if the split reaches that fraction | L/high |
| `coder-task`<br>`crates/coder-task` | Second stage after landed #12184: `task/sales` → `coder-task-sales`; agent key/profile/hiring/lifecycle → `coder-agent-registry`; memory/recall/reflect/engrams/consolidate → `coder-agent-memory`; agent host/queue/remote/plan → `coder-agent-runner`; crew/control → `coder-agent-crew`; land/issue/worktree orchestration → `coder-task-work`; inbox/storage/owner/checks remain `coder-task-store`. Facade → orchestration → store → values. | `coder-task-types`: task IDs, command/receipt/store schemas, journal events, and explicit store/runner ports; reuse runtime/route-contract types. | Agent and sales modules call siblings through crate::task, and store-private helpers expose persistence. Move the inert store contract first; no store dependency back to agents. One recent commit is a move, not low activity. Declared feature gates: none declared. | 35–65%; about 8.1–15.0 s of the sampled 23.08 s command if the split reaches that fraction | L/high |
| `coder-new`<br>`crates/coder-new` | `issue_run`, `solver`, `brainstorm` → `coder-issue-flow`; `provider`, `claude*`, `codex*`, `harness*` → `coder-turn-adapters`; tool/plugin registration (`*_plugin`, `*_tool*`, `bundled_runtime`) → `coder-tool-runtime`; `ui`, composer, pickers → `coder-terminal-app`. Binary → adapters/tools/UI → contracts. | Small `coder-session-types`: turn events, tool outcomes, session selection, and injected host traits; keep existing protocol values in route-contract and coder-sdk. | `lib.rs`, live state, and callbacks cross all families. Tools must accept host ports rather than import the terminal. Keep one registration list in the facade. Declared feature gates: none declared. | 40–70%; about 4.1–7.2 s of the sampled 10.22 s command if the split reaches that fraction | L/medium |
| `openagents-engine`<br>`apps/openagents/crates/engine` | `repos`, `source_control`, `diff_sync`, `workspace_files`, `workspace_search`, `worktree_settings` → `openagents-workspace-engine`; `doc_host`, `sessions`, `run_journal`, persistence → `openagents-session-engine`; account logins/auth → `openagents-engine-auth`. Existing Engine assembles them and keeps RPC dispatch thin. | Reuse openagents-proto/doc/rpc. A small engine-ports crate holds session control, file/watch events, and assembly interfaces, without concrete Engine fields. | RPC and DocHost currently reach the assembled engine; invert those calls through ports. Assemble concrete harness registries above engine/UI so an adapter edit does not keep rebuilding both. Preserve upstream-workos behavior and journal/recovery ownership. Declared feature gates: `upstream-workos`. | 25–55%; about 1.0–2.2 s of the sampled 4.00 s command if the split reaches that fraction | L/high |
| `openagents-cli`<br>`crates/openagents-cli` | `projects`, `connect`, computers/host commands → `openagents-cli-computers`; chat/agent/work commands → `openagents-cli-work`; wallet/x402/plugin purchase → `openagents-cli-payments`; Verse/chamber commands → `openagents-cli-verse`. CLI binary → command crates → existing domain clients. | `openagents-cli-common`: parsed global options, output/error format, credential access trait. Keep generated command tree and dispatch at the top. | Avoid helpers calling the top-level dispatcher. Forward the optional verse feature; generated help/tree paths and executable packaging must remain stable. Declared feature gates: `default`, `verse`. | 25–55%; about 1.7–3.8 s of the sampled 6.94 s command if the split reaches that fraction | L/medium |
| `gateway`<br>`crates/gateway` | `accounts`, billing/card funding/usage/earnings → `gateway-accounts`; classify/decision dispatch/relay_worker/inference_* → `gateway-inference`; playground/dashboard → `gateway-pages`. Binary/router → route families → context/domain. | Gateway-context holds admission context, config slices, errors, and backend traits; money/admission values stay in tenancy/pay-ledger/receipts. | serve.rs mixes wiring and admission. Preserve reserve/settle order and per-route auth; shared context cannot import route crates. Declared feature gates: none declared. | 25–50%; about 1.0–2.0 s of the sampled 4.02 s command if the split reaches that fraction | L/high |
| `verse`<br>`crates/verse` | `app`, `render`, window/input glue → `verse-desktop`; `panels`, workshop, terminal integrations → `verse-panels`; `film`, captures → `verse-capture`; retain portable session/simulation facade below. | Existing verse-core/engine/gfx and rust-native own portable values; use a narrow scene/runtime view trait for panels. | Default desktop enables many host features. Keep web/browser-chamber/mobile paths below desktop, and forward capture/model/xp/studio/terminal features without a panels → app cycle. Declared feature gates: `default`, `desktop`, `pylon-relay`, `model-host`, `xp-host`, `replay-host`, `capture`, `crypt`, `crypt-fight`, `panels`, `terminal`, `studio-host`, `imported-desktop`, `imported-surface`, `native-audio`, `hosted-social`, `remote-chamber`, `chamber-loopback`, `web`, `browser-chamber`, `dev-destruction`. | 20–50%; about 5.1–12.8 s of the sampled 25.70 s command if the split reaches that fraction | L/medium |
| `verse-zone-everglade`<br>`crates/verse-zone-everglade` | `zones/everglade/demolition` → `everglade-demolition`; pack format/loading and admission → `everglade-content`; studio/townsfolk/world_tree → `everglade-town`. Zone assembles these over existing world/core/cad-world. | Existing cad-world owns shared physics; only Everglade layout/spawn/interaction values need a zone-types crate. | Demolition and town currently reference the zone player/layout. Inject world/player interfaces. Preserve model-host, studio-host, desktop, destruction, pylon-relay forwarding and pack digests. Declared feature gates: `model-host`, `studio-host`, `desktop`, `dev-destruction`, `pylon-relay`. | 25–55%; about 3.4–7.5 s of the sampled 13.62 s command if the split reaches that fraction | L/medium |
| `openagents-mobile`<br>`apps/openagents/crates/mobile` | `layout`, `visual` → `openagents-mobile-view`; wallet → `openagents-mobile-wallet`; `client_ffi/android_host`, sessions/account adapters → smaller internal bridge crates; keep one cdylib/UniFFI entry package. | Reuse proto/doc/client types; FFI boundary owns the public UniFFI records and converts to portable types. | UniFFI scaffolding/library names, callback ownership, and JNI exports must stay compatible. Internal crates cannot require the final cdylib to resolve their types. Declared feature gates: `account-fixture`, `bindgen`. | 20–45%; about 1.2–2.8 s of the sampled 6.19 s command if the split reaches that fraction | L/high |

### Phase 2 candidate boundaries

| Crate | Modules → proposed crates and dependency direction | Shared values / ports | Cycles, features, and other blockers | Expected local edit-build saving | Effort / risk |
| --- | --- | --- | --- | --- | --- |
| `psionic-serve`<br>`crates/psionic/crates/psionic-serve` | `openai_http` → `psionic-serve-http`; `gguf`, `qwen35`, `gpt_oss`, `clef`, speech/model adapters → separate `psionic-serve-*` implementation crates. Serve binary → HTTP/router → model trait implementations → inference/runtime. | `psionic-serve-types`: request/response and model engine traits; reuse existing inference/router contracts before adding types. | HTTP file reaches model implementations and training/research helpers. HTTP must accept injected engines; forward router-ort/cuda/metal and model feature switches to the right leaf only. Declared feature gates: `default`, `router-ort`, `router-cuda`, `router-metal`, `tokio-runtime-telemetry`, `tokio-runtime-telemetry-cpu`, `csm-cuda`, `csm-metal`, `medpsy-cuda`, `qwen38-vision-cuda`. | 35–65%; about 2.0–3.7 s of the sampled 5.74 s command if the split reaches that fraction | L/high |
| `psionic-eval`<br>`crates/psionic/crates/psionic-eval` | `legal_benchmark*` → `psionic-eval-legal`; `tassadar*` → `psionic-eval-tassadar`; Apple adapter and Psion evaluations → separate families; keep small eval facade/core. Trainers/research → evaluator families → eval-core. | Eval-core owns `EvalRunMode`, `EvalRunStatus`, `EvalSampleStatus`, `EvalMetric`, `EvalArtifact`, case/gate/report values, and evaluator ports, with no model execution dependency. | Default full enables most of the stack. Retain no-default-features support and replace full with explicit family forwarding; evaluation must not depend on train/serve. Declared feature gates: `default`, `full`. | 40–70%; about 4.9–8.6 s of the sampled 12.31 s command if the split reaches that fraction | L/medium |
| `gym`<br>`crates/gym` | `runs_*`, terminal_bench, replay/analysis → `gym-runs`; suite/admission/gate → `gym-eval-core`; TUI files → `gym-tui`; binaries → those libraries. | Suite, score, run receipt, and gate evidence values in gym-core; use atif/receipts for existing wire shapes. | Gate and run analysis share stored data and private helpers. TUI remains optional and depends downward, so headless consumers do not compile terminal widgets. Declared feature gates: `default`, `tui`. | 30–60% | L/medium |
| `psionic-runtime`<br>`crates/psionic/crates/psionic-runtime` | `tassadar*`, plugin execution/ABI → `psionic-runtime-tassadar`; structured_output → `psionic-structured-output`; local_multi_device → `psionic-runtime-multi-device`. Consumers → family crates → runtime-core. | Runtime work/capability/checkpoint values stay in core over psionic-core/ir. Plugin ABI structs move with their protocol, below execution. | lib.rs has 14.9k lines and many cross-family reexports; do not place generic runtime values in the Tassadar child. Core must shed its wasmi/parser/network dependencies if only the children need them. Declared feature gates: none declared. | 30–60% | L/high |
| `nostr`<br>`crates/nostr` | Move application NIP families: `domain`, contracts/eval/ext, XP and market payloads into `nostr-domain-*` crates; keep event/key/signature/encryption/tag primitives in `nostr-core`. Reexport through nostr facade during migration. New direct consumers → family crates → core. | Event, tags, keys, canonical serialization, signing/error types in core; reuse agent-contract rather than duplicate domain schemas. | The facade has 201 potential reverse dependents. Old imports through the facade still invalidate them: migrate consumers to core/families before claiming a fanout win. Do not move cryptographic behavior in the mechanical commit. Declared feature gates: `default`, `server`, `test-invoice`. | 25–60% per family; consumer win requires import migration | L/high |
| `verse-world`<br>`crates/verse-world` | `service` → `verse-world-service`; spells/combat → `verse-world-combat`; prediction/play simulation → `verse-world-sim`. Service → sim/combat → world types. | Entity IDs, world state, snapshots, spell inputs/results in world-types; wire DTOs in service-protocol. | Service currently uses simulation private state; simulate through owned snapshots and commands. Preserve all service-*/reach-*/studio feature combinations and wasm support. Declared feature gates: `service-client`, `reach-client`, `service-net`, `service-auth`, `service-reach`, `studio`. | 25–55% | L/high |
| `psionic-models`<br>`crates/psionic/crates/psionic-models` | Model-family modules (Tassadar, parameter_golf, csm, DeepSeek, Qwen) → model-family crates; tokenizers/weight metadata → model-core. Existing facade → family implementations → model-core/runtime. | Model descriptors, weight manifests, tokenizer interfaces, and admission errors in model-core; avoid tensor backend types in portable descriptors. | lib.rs contains shared implementations, and csm/vision features activate Candle/Moshi backends. Preserve CUDA/Metal forwarding and fixture-only deepseek-v41-fixtures. Declared feature gates: `default`, `csm-cuda`, `csm-metal`, `medpsy-cuda`, `qwen38-vision-cuda`, `deepseek-v41-fixtures`. | 30–60% | L/medium |
| `lnd-sdk`<br>`crates/lnd-sdk` | `generated/*rpc.rs` → `lnd-sdk-proto` first; then independently feature-gate generated service families if they stay >20k. SDK transport/automation → proto; generated crate has no SDK dependency. | Prost messages and tonic service definitions are the contract; no second hand-written copy. | Generated cross-service message references can require a common proto package; regenerate include paths and keep supervisor optional. Generation is a justified size exception until measured. Declared feature gates: `default`, `supervisor`. | local handwritten edits 15–40%; proto edit may save 0% | M/medium |
| `openagents-harness`<br>`apps/openagents/crates/harness` | `acp`, `codex`, `claude`, `opencode`, `cursor`, `pi` → adapter crates; process/keeper/install code → `openagents-harness-process`. Registry/facade → adapters → process and proto. | Harness trait/events stay in a small harness-core or existing proto; installer/executable descriptors in process layer. | Adapters use common process, notification, and executable helpers. Registry cannot live in core; construct it in app assembly rather than force concrete adapters into every engine consumer. Preserve voice/pty/supervised and native/installer fixtures. Declared feature gates: `default`, `voice`, `pty`, `supervised`, `native-fixture`, `installer-fixture`. | 25–55% | L/medium |
| `coder-runtime`<br>`crates/coder-runtime` | `runtime`, program/package/capability readers → `coder-program-runtime`; `generate`, `delegate_door`, `delegate`, relay → `coder-turn-runtime`. Existing runtime facade → both. | `coder-runtime-types`: invocation, program-step, child result, and policy values; existing capability/atif own their contracts. | The two families share runstate/source/tracker and private helpers. Inject execution and tracing ports; do not make contracts depend on coder-sdk/delegate. This follows, rather than repeats, the landed split. Declared feature gates: none declared. | 20–45% | M/medium |
| `openagents-chat-view`<br>`apps/openagents/crates/chat-view` | `markdown` → `openagents-markdown-view`; `settings_forms` → `openagents-form-view`; composer_input → `openagents-composer-view`; transcript uses those over theme/doc/proto. | Reuse existing theme, doc, and proto; render intents and generic action callbacks stay in a small view interface. | Theme/popover helpers are shared. Feature composer-input/visuals and the web workspace GPUI patch/dependency declarations must follow the new crates; this excludes #12184 native media scope. Declared feature gates: `composer-input`, `visuals`, `browser-fixture`, `test-support`. | 20–45% | M/medium |
| `openagents-chat-app`<br>`crates/openagents-chat-app` | `coder_tab`, `coder_run`, host_threads → `openagents-chat-work-view`; `gym`, eval_cards → `openagents-chat-gym-view`; conversation/cards/navigation stay in small chat shell. | Existing openagents-chat and rust-native values; small action/result interfaces below views. | View modules call each other through shared chat state. Inject actions rather than allowing a child to depend on the chat shell. Declared feature gates: `test-support`. | 20–45% | M/medium |
| `tenancy`<br>`crates/tenancy` | `accounts`, sessions/workspaces → `tenancy-accounts`; money/billing/quota → `tenancy-billing`; training/skills → separate books. Keep registry/admission lower. | Account/workspace IDs and registry/admission snapshots below the books; pay-ledger remains the money contract. | Cross-book atomic/replay behavior and postgres persistence make this high risk. Extract storage ports and preserve journal/recovery boundaries before moving handlers. Declared feature gates: `postgres`. | 20–45% | L/high |
| `coder-delegate`<br>`crates/coder-delegate` | `data_profile`, component/requirements/recipe/pack → `coder-delegate-plan`; delegate/session/monitor/stream/environment adapters → `coder-delegate-executor`; issue flow/terminal above both. | Worker, briefing, and execution-result interfaces below implementations; retain ATIF/route-contract values. | The existing crate feeds coder-sdk; child contracts must not import coder-sdk. Preserve the explicit no-microluna dependency. Declared feature gates: none declared. | 20–45% | M/high |
| `verse-pbr`<br>`crates/verse-pbr` | `water`, `fluid`, liquid film → `verse-water-render`; imported surface/desktop → `verse-imported-render`; keep generic PBR GPU pipelines below them. | Mesh/material/bake descriptors in existing cad-world/verse-engine; render-resource interfaces in PBR core. | Both child renderers use private GPU resources. Expose a narrow render context and forward imported-surface/desktop; avoid making generic PBR depend on water. Declared feature gates: `diagnostics`, `imported-surface`, `imported-desktop`. | 20–45% | M/medium |
| `pylon`<br>`crates/pylon` | `paid`, market/broker/earnings → `pylon-market`; online/provider/router/runtime → `pylon-provider`; CLI/setup above both. | Keep request/envelope/provider descriptors in pylon-core or existing Nostr/pay-ledger values. | Broker is a default feature. Preserve settlement authority and fixture forwarding while removing provider → top-level CLI calls. Declared feature gates: `default`, `broker`, `fixture`. | 15–40% | M/high |
| `coder-router`<br>`crates/coder-router` | `router` and policy/judge → `coder-turn-router`; `cli_route`, `eval_author`, `gym_kb`, product/codebase readers → `coder-route-tools`. Facade → tools/router → runtime. | Reuse route-contract; extract only route decisions and knowledge-reader ports that would otherwise point back to the facade. | Router policy imports runtime and knowledge helpers; tools may depend on router, never the reverse. Golden fixtures must retain their source-relative paths. Declared feature gates: none declared. | 15–35% | M/medium |
| `coder-host`<br>`crates/coder-host` | `client` → `coder-host-client`; `serve`, control/task/process dispatch → `coder-host-server`. Facade → both → coder-host-wire/access. | Use existing coder-host-wire; rights and device grants stay in coder-access. | Default host currently enables PTY, storage, and process stacks. Client crate must select default-features=false on host-dependent dependencies, including when linked by the app. Declared feature gates: `default`, `host`, `verse-assets`. | 15–40% | M/high |
| `coder`<br>`crates/coder` | Keep the newly thin library facade. Move `src/bin/coder-worker.rs` (7.4k lines) → `coder-worker-app`, leaving a tiny binary wrapper; retain old exports from runtime/router/task. | Use existing coder-runtime/task/route-contract APIs, not a new common crate. | Worker must not import the facade it is reexported from; relocate binary-only tests and assets together. The 362 commits largely predate the landed split. Declared feature gates: none declared. | worker edits 30–60%; facade 0% | M/low |
| `physics`<br>`crates/physics` | `water`, `fluid` → `physics-fluid`; cloth/rope → `physics-deformable`; collision/queries/rigid world stay in physics-core. Renderers/simulation → children → core. | Body/shape IDs, forces, contacts, and query traits stay in core. | Fluid/rope code mutates world internals; expose narrow access rather than duplicate World. Changes to core still reach 115 potential consumers. Declared feature gates: none declared. | 15–40% | M/high |
| `openagents-chat`<br>`crates/openagents-chat` | `client`, coder_events → `openagents-chat-client`; service/router/basic chats → `openagents-chat-service`. Views/facade → client/service → shared chat contract. | Thread/message/observation and tool-group values in chat-core; reuse ATIF and route-contract. | Current router and client both know view helpers. Strip rust-native presentation dependencies from chat-core to avoid app → service cycles. Declared feature gates: none declared. | 15–40% | M/medium |
| `inference`<br>`crates/inference` | `request`, item/event/response/stream and chat translation → `inference-protocol`; upstreams/run/session/router → `inference-client`; meter → `inference-meter`. Clients → protocol; metering stays independent. | Open Responses/SSE request and event values in protocol; backend credentials never in it. | Translation/stream validation share helpers. Protocol must shed reqwest/tokio; preserve exact streaming behavior and price accounting. Declared feature gates: none declared. | 15–40% | M/medium |
| `coder-access`<br>`crates/coder-access` | `protocol` and rights/grant/studio/spend request values → `coder-access-protocol`; host enrollment/store → `coder-access-host`; client → `coder-access-client`. Both depend on protocol. | Protocol owns wire DTOs, rights, signed grant validation; reuse existing Nostr primitives. | host defaults on and forwards coder-connect/host. Keep portable verification usable with default-features=false; authority checks cannot migrate to clients only. Declared feature gates: `default`, `host`. | 15–35% | M/high |
| `openagents-ui`<br>`crates/openagents-ui` | This is the ROOT web UI crate: `icons/generated` → `openagents-web-icons`; forms/actions/overlays → `openagents-web-controls`; shell/content → higher web components. Facade → controls/icons → tokens. | Keep oa-tokens and semantic action/view values below rendered HTML. | Do not confuse it with apps/openagents/crates/ui or duplicate the media split. Generated SVGs are an exception candidate; build.rs include paths follow them. Declared feature gates: none declared. | 10–30% | M/low |
| `microcoder`<br>`crates/microcoder` | `repository` → `microcoder-repository`; xpnet/kbnet/studies → `microcoder-knowledge`; binary/facade → both → microcoder-loop. | Existing loop state/ATIF/knowledge types; repository worker/checkout interface below adapters. | Repository still depends on coder; no dependency from microcoder-loop back to the facade or repository. Declared feature gates: none declared. | 15–35% | M/medium |

### Phase 3 candidate boundaries

| Crate | Modules → proposed crates and dependency direction | Shared values / ports | Cycles, features, and other blockers | Expected local edit-build saving | Effort / risk |
| --- | --- | --- | --- | --- | --- |
| `coder-one`<br>`crates/coder-one` | `checks`, `accept`, grade → `coder-one-checks`; `micro`, `compose`, component/localize/study → dedicated evaluation/policy crates. Binary → experiment crates → small policy contracts. | Existing ATIF and a small policy-input/result interface. | Legacy Microluna studies must remain reproducible. Recent edits are sparse: schedule behind current product work; do not expand this retained agent. It has no normal reverse dependents, so benchmark leaf-only incremental compilation before a large extraction. Declared feature gates: none declared. | 30–60%; about 8.5–17.1 s of the sampled 28.42 s command if the split reaches that fraction | L/medium |
| `openagents-desktop`<br>`crates/openagents-desktop` | Prefer retirement/migration into apps/openagents, consistent with the one-app rule. If still required, move `chat`, route_map, slides/settings into view crates below the legacy shell. | Reuse existing chat-app/rust-native and app protocols; no new parallel desktop state model. | The app feature links Verse and platform code. Establish retained callers before spending on another shell; preserve current exports until replacement consumers move. Declared feature gates: `default`, `app`. | 0% through retirement; 15–40% if retained | L/medium |
| `psionic-research`<br>`crates/psionic/crates/psionic-research` | `tassadar_*`, `parameter_golf_*`, attnres experiment runners → separate research crates; retain a small command/runner facade. Research family → eval/train/models. | Experiment descriptors and report values in a small research-contract crate, or existing eval-core. | burn-import must belong only to the attnres importer. Never pull the research facade into lower model/eval code to reuse one report. Declared feature gates: `default`, `burn-import`. | 20–50% | M/medium |
| `psionic-data`<br>`crates/psionic/crates/psionic-data` | `tassadar*`, `psion_*`, `legal_benchmark*`/`qwen_legal*`, and adapter datasets → dataset-family crates over data-core. | Dataset manifest, shard/token references, and corpus validation errors remain in data-core/datastream. | Current root reexports and runtime-specific descriptors; keep tokenization and runtime adapters above inert manifests. Declared feature gates: none declared. | 20–45% | M/medium |
| `psionic-backend-cuda`<br>`crates/psionic/crates/psionic-backend-cuda` | Mechanically subdivide lib.rs first; then CUDA allocation/stream/device lifetime → `psionic-cuda-device`, CLEF/kernel launch families → `psionic-cuda-kernels`; backend → kernels/device → core/runtime. | Use existing core/runtime tensor/device descriptors; add a small CUDA handle/lifetime interface only where needed. | Unsafe device ownership and compiled-kernel lifetimes block a blind file split; build.rs kernel assets must follow the leaf. Declared feature gates: none declared. | 15–40% | L/high |
| `nostr-relay`<br>`crates/nostr-relay` | `store` → `nostr-relay-store`; `gateway` → `nostr-relay-gateway`; binary → gateway → store trait. | Event/filter values stay in Nostr core; store query/result trait in a small relay interface. | Postgres integration fixtures and gateway config/media policy share helpers. Move only storage implementation behind the trait; retain transaction semantics. Declared feature gates: none declared. | 10–30% | M/high |
| `psionic-compiler`<br>`crates/psionic/crates/psionic-compiler` | `tassadar_alm_*`, Wasm/module linking → `psionic-compiler-tassadar`; preserve generic lowering in compiler-core. | IR and ABI values belong in psionic-ir or a dedicated Tassadar IR crate below both runtime and compiler. | Compiler already depends on runtime; moving runtime-independent IR into a compiler child would form a reverse edge. Declared feature gates: none declared. | 20–45% | M/medium |
| `psionic-ir`<br>`crates/psionic/crates/psionic-ir` | `autodiff` → `psionic-autodiff`; `tassadar_*` ABI/Wasm/manifest modules → `psionic-tassadar-ir`; keep tensor/graph IR core. | Core graph/value IDs remain in psionic-ir; new IR families depend on it and psionic-core. | AD must not be required to construct base IR. Root type references and serialization compatibility need a mechanical first pass. Declared feature gates: none declared. | 20–45% | M/medium |
| `psionic-cluster`<br>`crates/psionic/crates/psionic-cluster` | `ordered_state` → `psionic-cluster-state`; scheduler/replicated/layer/tensor/pipeline sharding → `psionic-cluster-serving`. Facade → serving → state/runtime. | Membership/state IDs and event/version envelopes in cluster-state; transport ports in psionic-net. | Ordered-state persistence and scheduler mutations share private types. Keep network effects outside deterministic state transitions. Declared feature gates: none declared. | 15–40% | M/medium |
| `oa-auth`<br>`crates/oa-auth` | `workos`, GitHub/OAuth adapters → `oa-auth-providers`; repos membership flows → `oa-auth-repos`; retain auth/session contract below. | Account/session identities and provider verification result values in auth-core or tenancy-accounts. | Feature matrix fake/local/postgres/workos/fake-workos and encrypted token storage need equivalent tests; adapters cannot call app/router assembly. Declared feature gates: `fake`, `local`, `postgres`, `workos`, `fake-workos`. | 10–30% | M/high |
| `psionic-backend-metal`<br>`crates/psionic/crates/psionic-backend-metal` | Mechanically split lib.rs, then device/buffer management → `psionic-metal-device`; CLEF and small-model kernels → `psionic-metal-kernels`. Backend → kernel/device layers. | Existing core/runtime descriptors; retain Metal object ownership in device layer. | Apple target cfg and Objective-C/Metal lifetimes need a Mac validation pass; Linux stub measurements cannot establish the win. Declared feature gates: none declared. | 15–40% | L/high |
| `psionic-provider`<br>`crates/psionic/crates/psionic-provider` | Tassadar market/accepted-outcome/gate modules → `psionic-provider-tassadar`; leave general provider registry in provider-core. | Accepted-outcome and provider capability descriptors below both provider implementations and serve. | Current provider imports serve/research/eval. Extract contracts downward; do not move a serve implementation into provider-core. Declared feature gates: none declared. | 15–35% | M/medium |
| `ext-eval`<br>`crates/ext-eval` | `author` → `ext-eval-author`; runner/grader/sandbox execution → `ext-eval-runner`; case/report/score/replay values stay in core. | Core owns case/verdict/report schemas; runner and author depend on core. | Default runner and plugin feature coupling must remain explicit. Keep fixture authoring out of runtime consumers. Declared feature gates: `default`, `runner`, `plugin`. | 10–30% | M/medium |
| `coder-mobile`<br>`crates/coder-mobile` | Prefer migration/retirement under the one-mobile-app rule. If retained, `verse_app`, verse_ffi/surface → `coder-verse-mobile`; computer/studio views stay separate. | Reuse existing mobile proto, rust-native, and Verse types; no parallel account/session model. | JNI/FFI symbols and old app consumers must be enumerated first. Do not grow a second phone implementation. Declared feature gates: none declared. | 0% by retirement; 15–35% if retained | L/high |
| `coder-sdk`<br>`crates/coder-sdk` | Keep; next extract checkout_guard/floors into `coder-checkout-policy`, and individual Codex/Claude/ACP adapters into leaf adapters below the SDK facade. | Tool/delegation/session request and result contracts stay below adapters, using route-contract. | SDK currently depends on coder-delegate. A delegate contract cannot point back into this SDK. Declared feature gates: none declared. | 10–30% | M/medium |
| `terminal-core`<br>`crates/terminal-core` | `paper` → `terminal-document`; application/run/smart/Gym/workbench adapters above a small terminal state core. | Terminal input/layout/state values stay in core; use coder-pty/vt rather than new terminal wire types. | application.rs currently coordinates many domains; expose commands/events instead of child references to Application. Declared feature gates: none declared. | 10–30% | M/medium |
| `oa-computers`<br>`crates/oa-computers` | Keep; registry/store/credential broker → `oa-computers-registry`; lifecycle/turns/provider effects → `oa-computers-operator` if growth continues. | Existing computer-driver/enrolled/environment types plus a registry port. | space/archil feature forwards and credential disclosure boundaries must remain at provider calls. Declared feature gates: `space`, `archil`. | 0% now; 10–25% later | M/high |
| `coder-computers`<br>`crates/coder-computers` | `terminal` → `coder-computer-terminal`; live/connect/SSH → `coder-computer-client`; keep model/controller presentation above protocol clients. | Existing host-wire/access plus portable terminal state and view action types. | Default live and ssh feature forward must remain optional; pure views should not import concrete host engines. Declared feature gates: `default`, `live`, `ssh`. | 10–30% | M/medium |
| `pay-ledger`<br>`crates/pay-ledger` | Keep; at growth move sqlite/shared storage → `pay-ledger-sqlite`, commission/market/payout policy above a small ledger contract. | Ledger entry, money, idempotency, and reconciliation values stay in core. | Default sqlite and concurrent replay semantics block a casual move; both app SQLite 0.30 and root 0.28 must still work. Declared feature gates: `default`, `sqlite`. | 0% now; 10–25% later | M/high |
| `jev`<br>`crates/jev` | Keep; future seam: questions/answers/decision/schema → `jev-types`; HTTP client/retry/account/doors stay above it. | Question and answer DTOs and typed refusals in jev-types. | blocking/live feature forwards must not make types depend on tokio; preserve the retry/idempotency contract. Declared feature gates: `blocking`, `live`. | 0% now; 10–25% later | M/medium |
| `coder-lease`<br>`crates/coder-lease` | `agent_builds`, shim, sweep → `coder-build-state`; machine grants/actions → `coder-machine-policy`; broker/table → small lease core. | Lease requests/receipts, process identity, and grant values below implementations. | Cleanup and grants are authority-sensitive. Move pure classification before filesystem/process actions; never add a lease-core dependency back to the CLI. Declared feature gates: none declared. | 10–30% | M/high |
| `background`<br>`crates/background` | Keep; compile/plan/rule → `background-plan`; runner/run/cleanup → `background-host` when it grows. | Plan, rule, phase, and result values in the plan layer; inject execution ports. | Plan compiler currently reaches boundary/lease helpers; remove those effects before extracting inert planning. Declared feature gates: none declared. | 0% now; 10–30% later | M/medium |
| `coder-cloud`<br>`crates/coder-cloud` | Keep; pool/operator/provider backends → `coder-cloud-operator`; workspace/runtime/budget policy → cloud-core. | Work request, budget, pool status and provider interfaces in core. | Boat/Archil and operator calls cross runtime helpers. Keep credentials/execution above policy. Declared feature gates: none declared. | 10–30% | M/medium |
| `verse-engine`<br>`crates/verse-engine` | Keep; `animation_graph`, animation/locomotion/hair → `verse-animation` at the next growth point; audio/banks → `verse-audio`. Engine facade → these over value-only core. | Pose, clip and audio event values remain in stable low-level modules, not renderer types. | Asset-io must remain optional; simulation consumers must not acquire audio/GPU dependencies. Declared feature gates: `default`, `asset-io`. | 0% now; 10–30% later | M/medium |
| `boltz-client`<br>`vendor/boltz-client/crates/lib` | Retain vendored shape. If downstream build measurements justify it, propose upstream EVM (`evm`) and swap implementation crates over common API models. | Existing models/error/config remain the upstream contract. | An independent downstream split makes upstream updates harder; browser/regtest/getrandom/tokio target behavior needs upstream agreement. Declared feature gates: `default`, `browser-tests`, `regtest`. | 0% recommended | L/high |
| `rust-native`<br>`crates/rust-native` | Keep semantic view/style as the reusable core; `layout`, markdown/edit shaping → `rust-native-layout` / `rust-native-text` at growth. Adapters → layout/text → view contract. | View/style/intent and surface lifetime types stay in core; no product palette. | Default syntax and ffi/shaping feature relationships; layout currently uses view internals. Do not add platform or app dependencies to core. Declared feature gates: `default`, `syntax`, `ffi`, `shaping`. | 10–30% | M/medium |
| `openagents-client`<br>`apps/openagents/crates/client` | Keep; `demo` → `openagents-client-fixtures`; live/session/workspace transport implementations below a client facade if they grow. | Use proto/doc/rpc for session and workspace state; no new duplicate wire model. | Fixture catalogs currently share client private constructors; inject fixture factories and preserve upstream-workos. Declared feature gates: `upstream-workos`. | 0% now; 10–25% later | S/low |
| `psionic-router`<br>`crates/psionic/crates/psionic-router` | Keep for now; next seam: `tassadar_*` policy/routing → `psionic-router-tassadar`, leaving response_state/tool_loop and generic routing in router-core. | Route input/output and engine selection traits in existing router core. | Tassadar policy must depend on generic routing, without generic router importing the policy implementation. Declared feature gates: none declared. | 10–30% | M/medium |
| `openagents-account`<br>`apps/openagents/crates/account` | Keep; desktop identity/Keychain adapter → `openagents-account-platform`; reader/history/video-editor adapters above a portable account core. | Reuse SDK/proto/doc/lab types; account identity/credential-provider trait below the platform adapter. | Apple target cfg and Keychain access must remain isolated from tests; avoid moving platform state into proto. Declared feature gates: none declared. | 10–25% | M/high |
| `coder-ui`<br>`crates/coder-ui` | `catalog`, `demo` → `coder-ui-catalog`; keep theme/components/portable controls in coder-ui. Catalog → UI → rust-native/tokens. | Existing oa-tokens/rust-native semantics; no new types crate. | Catalog exports must not be required by production components. Preserve syntax forwarding and existing compatibility paths. Declared feature gates: `default`, `syntax`. | 10–30% | S/low |
| `coder-environment-operator`<br>`crates/coder-environment-operator` | `studio` and agent sign-in adapters → `coder-studio-operator`; provider/computer qualification → separate operator leaf. Existing facade → adapters → environment/driver contracts. | Reuse environment/setup/verify/working-computer types; provider actions behind existing driver interfaces. | space feature and sign-in handoff callbacks cross root helpers; keep credentials out of shared values. Declared feature gates: `space`. | 10–30% | M/high |
| `rust-native-desktop`<br>`crates/rust-native-desktop` | Keep; window/Wayland/accessibility → `rust-native-window`; text/layout/paint → `rust-native-desktop-render`. Shell adapter → render/window → rust-native. | Semantic view and input events remain in rust-native; GPU/window handles stay platform-local. | Default window feature and native accessibility cfgs require platform checks. Do not force a window dependency into offscreen layout tests. Declared feature gates: `default`, `window`. | 10–30% | M/medium |
| `cad`<br>`crates/cad` | Keep; drawing/DXF/manufacture → `cad-drawing`; solver/build/clash → `cad-engine`; schema/doc/tree below both if growth demands it. | Document, stable refs, and feature definitions in cad-core; existing lab-kit/physics own shared values. | Drawing reads solver internals; expose solved geometry values. Keep solidcraft out of schema-only consumers. Declared feature gates: none declared. | 10–30% | M/medium |
| `coder-compositor`<br>`crates/coder-compositor` | Keep; pure layout/binds/state policy → `coder-compositor-policy`; Smithay/udev/screencopy/render stay in host binary. | Window/output IDs and placement/input action values in policy, without Smithay handles. | Unsafe compositor/native resource ownership and desk callbacks need hardware validation. Declared feature gates: none declared. | 0% now; 10–25% later | M/high |
| `psionic-sandbox`<br>`crates/psionic/crates/psionic-sandbox` | Plugin charter/capability/world-mount/packet-ABI modules → `psionic-plugin-contract`; execution/pool/jobs/podman → `psionic-sandbox-host`. Host → plugin contract/runtime. | Packet ABI, mount policies, and capability refusal values in plugin-contract. | Preserve all authority checks at the host call site; model-specific benchmarks belong above the contract. Declared feature gates: none declared. | 10–30% | M/high |
| `session-host`<br>`crates/session-host` | Keep; durable host state/commands → `session-host-core`; Codex/resident/SSH/transport adapters above it. | Use coordination-host/workspace-host protocols; session command/result interface in core. | codex feature stays optional; persistence and callback ownership currently sit together in host.rs. Declared feature gates: `codex`. | 10–25% | M/medium |
| `microcoder-loop`<br>`crates/microcoder-loop` | Keep; capacity/account/usage → `provider-capacity`; Claude/Vertex/model transport adapters → separate leaves around the one shared loop. | Loop state/command/reply and capacity records below executors. | Keep a single Microcoder loop and current failover semantics; adapters must not require a crate that imports them back. Declared feature gates: none declared. | 10–30% | M/medium |
| `layertree-tee-lab`<br>`crates/layertree-tee-lab` | Keep research workspace; pure tree/rules/predicate/codec → `layertree-lab-core`; follow-live/RPC/enclave effects remain adapters. | Tree/event/codec values in lab-core; events serde stays optional. | Research client/server/fuzz are already separate workspaces. Preserve events/follow-live/gcp-sealing/regtest combinations; no product dependency on this lab. Declared feature gates: `stage-attacks`, `follow-relay-tests`, `events`, `follow-live`, `gcp-sealing`, `regtest`. | 0% now; 10–25% later | M/high |
| `boat`<br>`crates/boat` | Keep; generated/models layer could become `boat-types` below client/api/follow if schema edits become costly. | Existing models/error/transport request values, with no reqwest in types. | Model helper methods call transport conveniences; remove those upward edges first. Declared feature gates: none declared. | 0% now; 10–25% later | S/low |
| `verse-imported`<br>`crates/verse-imported` | Keep; chamber/session/remote-window → `verse-chamber-client`, leaving asset/rig presentation in imported. Facade → chamber client → world-service/net. | Chamber session/wire values stay in world-service protocol; use existing core scene values. | Forward remote-chamber/chamber-loopback/chamber-client separately from imported-desktop and crypt-fight. Declared feature gates: `chamber-client`, `remote-chamber`, `chamber-loopback`, `imported-desktop`, `crypt-fight`. | 10–30% | M/medium |
| `knowledge`<br>`crates/knowledge` | Keep; evidence/product/search-index schemas → `knowledge-core`; search/harvest/remote/codebase model adapters → `knowledge-host`. | Existing entry/evidence/index types; XP remains in xp-ledger. | Avoid dragging inference/model clients into schemas; preserve bounded reads and evidence rules. Declared feature gates: none declared. | 10–30% | M/medium |
| `openagents-x402`<br>`crates/x402` | Keep; front/router/hosted/native adapters → `openagents-x402-host`; policy/payment-scheme/outcome/receipt values stay in core. | Existing payment and receipt contract values, with no network/host execution. | Native effects and refusal/settlement behavior must remain at the host boundary; facade reexports preserve client imports. Declared feature gates: none declared. | 10–30% | M/high |
| `lev`<br>`crates/lev` | Keep; serve and operator binaries → `lev-serve` above admission/policy/manifest/bridge library. | Existing Jev decision values; bridge message DTOs stay in the portable library. | serve feature is already optional and use is infrequent; retain Swift helper supervision and policy checks. Declared feature gates: `serve`. | 0% now; 10–25% later | S/low |
| `coder-pty`<br>`crates/coder-pty` | Keep; host platform PTY/process-group implementation → `coder-pty-host`; client/wire/replay stay portable. | Frame, gap, terminal identity and rights-check interfaces stay in the protocol layer. | Default host and Windows/Unix ownership semantics; preserve process-group cleanup and narrow rights at every call. Declared feature gates: `default`, `host`. | 0% now; 10–25% later | M/high |
| `openagents-doc`<br>`apps/openagents/crates/doc` | Keep; schema/parts/commands/transcript_delta → `openagents-doc-types`; Loro registry/workspace/queue → doc-store if growth demands it. | Portable transcript and command DTOs stay below Loro adapters, with existing proto values. | Do not serialize internal Loro handles into types; preserve document operation IDs and replay. Declared feature gates: none declared. | 10–25% | M/medium |
| `coder-terminal`<br>`crates/coder-terminal` | Keep; markdown and reusable ratatui components → `coder-terminal-widgets`, leaving editor/event/shell glue above. | Existing coder-ui palette and rust-native semantics, no new common crate. | Widgets currently share progress/selection state; split only after they accept explicit view models. Declared feature gates: none declared. | 0% now; 10–25% later | S/low |
| `openagents-sync`<br>`apps/openagents/crates/sync` | Keep; store/registry persistence → `openagents-sync-store`; chat/account/device transport → `openagents-sync-client`. Both use doc/proto values. | Existing Loro protocol and doc types; transport messages below storage/network. | Default store/SQLite version split and Apple credential code; preserve no-store clients and pairing authority. Declared feature gates: `default`, `store`, `mock-server`. | 10–25% | M/high |
| `gym-leaderboard`<br>`crates/gym-leaderboard` | `generator`, study/tb* adapters → `gym-leaderboard-generator`; client/view/contract remain small portable crates. Generator → contract; client → contract. | Versioned contract/bundle/signature values in leaderboard-core. | Feature generate defaults on; client-only users already disable it. Move publish forwarding without reintroducing Gym/knowledge into readers. Declared feature gates: `default`, `generate`, `client`, `publish`. | 10–30% | S/low |
| `archil`<br>`crates/archil` | Keep. If needed, models/types → `archil-types`, process/disk/file/S3 implementations stay in the SDK. | Existing models and error values, no additional shared state. | Most counted lines here are tests; splitting the 6.7k source crate now adds overhead. Declared feature gates: none declared. | 0% | S/low |
| `coder-sync`<br>`crates/coder-sync` | Keep; server routes/account exports → `coder-sync-server`; rooms/Loro/client helpers remain below it when growth justifies the move. | Reuse existing doc/sync/SDK values; server auth/config and room store ports in a narrow interface. | Server handlers use global room state; extract store ownership before router code. Do not confuse deploy/coder-sync standalone packaging workspace. Declared feature gates: none declared. | 0% now; 10–25% later | M/medium |
| `retail-cloud`<br>`crates/retail-cloud` | Keep; Boat/provision/dispatch → `retail-cloud-operator`; offer/meter/settle policy stays portable. | Existing contract/authority/offer/payment values; provider port below Boat implementation. | Boat is optional; preserve recovery and spending authority without exposing provider credentials. Declared feature gates: `boat`. | 0% now; 10–25% later | M/high |
| `openagents-mcp`<br>`apps/openagents/crates/mcp` | Keep; `tools` schema list/JSON-RPC → `openagents-mcp-protocol`; work/machine/video handlers → tool-family adapters above it. | Existing SDK/proto types and MCP request/result/error values; engine ports below handlers. | Tool metadata/dispatch must stay in sync; engine callbacks cannot make protocol import engine. Declared feature gates: none declared. | 10–30% | M/medium |
| `openagents-web-chat`<br>`apps/openagents/web/chat` | Keep; portable compose/model/check-in/work state → `openagents-web-chat-state`; GPUI/Wasm app shell stays above it. | Reuse proto/doc and chat-view values; no duplicate desktop chat state. | Default webgpu and optional webgl must forward together with wasm cfgs; GPUI entities cannot enter portable state. Declared feature gates: `default`, `webgpu`, `webgl`. | 0% now; 10–25% later | M/medium |
| `psionic-net`<br>`crates/psionic/crates/psionic-net` | Keep; first split lib.rs into private transport, protocol, and peer modules. At growth, transport → `psionic-net-transport` over existing net protocol values. | Peer/frame/operator-manifest values stay below transport. | Keep both tokio-runtime-telemetry feature forwards intact; no new crate until a private module boundary exists. Declared feature gates: `default`, `tokio-runtime-telemetry`, `tokio-runtime-telemetry-cpu`. | 0% now; 10–25% later | S/low |
| `env-host`<br>`crates/env-host` | Keep; allocator/claims/provider policy → `env-host-core`; GCE/SSH/local transport implementations above it. | Use coordination/workspace host protocol values and a provider trait. | Allocator currently owns bootstrap side effects; keep credentials and process execution in adapters. Declared feature gates: none declared. | 0% now; 10–25% later | M/high |

### Small crates and fixture decisions

Keep the following boundaries now. For each row, **expected split saving
is 0 s, effort is none, and split risk is avoided**. Shared values remain in
the listed crate or its existing dependencies; no new shared-types crate or
dependency edge is proposed, so there is no proposed cycle to break. The
feature list records gates a later extraction must preserve. This is a
per-crate decision to spend effort on larger units first, not an instruction
to create a new crate for each file. Modules named below are existing review
points if growth later makes a split useful.

| Crate | Decision and existing review points | Feature gates to retain |
| --- | --- | --- |
| `acp-client`<br>`crates/acp-client` | Keep 5,597-line source unit within the target range. Review points if it grows: `client.rs`, `cursor.rs`, `devin.rs`. | none declared |
| `actors`<br>`crates/actors` | Keep 8,098-line source unit within the target range. Review points if it grows: `client.rs`, `core.rs`, `example.rs`. | `default`, `server`, `net` |
| `agent-contract`<br>`crates/agent-contract` | Keep 636-line source unit; already below the target range. | none declared |
| `agent-fleet`<br>`crates/agent-fleet` | Keep 1,839-line source unit; already below the target range. Review points if it grows: `board.rs`. | none declared |
| `agent-graph`<br>`crates/agent-graph` | Keep 2,827-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `definition.rs`, `edit.rs`. | `default`, `lab` |
| `agent-lab`<br>`crates/agent-lab` | Keep 3,934-line source unit; already below the target range. Review points if it grows: `api.rs`, `autonomy.rs`, `components.rs`. | `views` |
| `atif`<br>`crates/atif` | Keep 5,343-line source unit within the target range. Review points if it grows: `activity.rs`, `analysis.rs`, `content.rs`. | none declared |
| `atif-nostr`<br>`crates/atif-nostr` | Keep 1,624-line source unit; already below the target range. Review points if it grows: `carriage.rs`, `cli.rs`, `relay.rs`. | none declared |
| `att-platforms`<br>`crates/att-platforms` | Keep 1,691-line source unit; already below the target range. Review points if it grows: `nras.rs`, `pki.rs`, `snp.rs`. | none declared |
| `att-release`<br>`crates/att-release` | Keep 1,921-line source unit; already below the target range. Review points if it grows: `approvals.rs`, `rebuild.rs`, `receipt.rs`. | none declared |
| `att-seal`<br>`crates/att-seal` | Keep 1,759-line source unit; already below the target range. Review points if it grows: `keys.rs`, `request.rs`, `stream.rs`. | none declared |
| `att-web`<br>`crates/att-web` | Keep 5,775-line source unit within the target range. Review points if it grows: `bubble.rs`, `copy.rs`, `demo.rs`. | `demo` |
| `automation-host`<br>`crates/automation-host` | Keep 6,473-line source unit within the target range. Review points if it grows: `cli.rs`, `controls.rs`, `evaluator.rs`. | none declared |
| `bitcoin-amount`<br>`crates/bitcoin-amount` | Keep 483-line source unit; already below the target range. | none declared |
| `boat-template`<br>`crates/boat-template` | Keep 884-line source unit; already below the target range. | none declared |
| `brainstorm-client`<br>`crates/brainstorm-client` | Keep 2,270-line source unit; already below the target range. Review points if it grows: `client.rs`, `transport.rs`. | none declared |
| `briefed-agent`<br>`crates/briefed-agent` | Keep 4,526-line source unit; already below the target range. Review points if it grows: `codex.rs`, `opencode.rs`, `stop_gate.rs`. | none declared |
| `briefing-lab`<br>`crates/briefing-lab` | Keep 5,657-line source unit within the target range. Review points if it grows: `attempt.rs`, `environment.rs`, `execution.rs`. | none declared |
| `bunny-rules`<br>`crates/bunny-rules` | Keep 5,252-line source unit within the target range. Review points if it grows: `bot.rs`, `game.rs`, `garden.rs`. | none declared |
| `bunny-web`<br>`crates/bunny-web` | Keep 7,354-line source unit within the target range. Review points if it grows: `app.rs`, `copy.rs`, `hud.rs`. | `autoplay` |
| `cad-lab`<br>`crates/cad-lab` | Keep 3,332-line source unit; already below the target range. Review points if it grows: `lab.rs`, `mcp.rs`. | none declared |
| `cad-wasm`<br>`crates/cad-wasm` | Keep 370-line source unit; already below the target range. | none declared |
| `cad-world`<br>`crates/cad-world` | Keep 9,048-line source unit within the target range. Review points if it grows: `cad_physics.rs`, `cad_physics`, `cad_place.rs`. | none declared |
| `capability`<br>`crates/capability` | Keep 3,091-line source unit; already below the target range. Review points if it grows: `bounded.rs`, `manifest.rs`, `probe.rs`. | none declared |
| `chat-home`<br>`crates/chat-home` | Keep 476-line source unit; already below the target range. Review points if it grows: `view.rs`. | `ui` |
| `chat-load-bench`<br>`crates/chat-load-bench` | Keep 2,618-line source unit; already below the target range. Review points if it grows: `bench.rs`, `cj.rs`, `dataset.rs`. | none declared |
| `claude_agent_sdk`<br>`crates/claude_agent_sdk` | Keep 9,108-line source unit within the target range. Review points if it grows: `callbacks.rs`, `error.rs`, `mcp.rs`. | none declared |
| `code-graph`<br>`crates/code-graph` | Keep 3,281-line source unit; already below the target range. Review points if it grows: `build.rs`, `extract.rs`, `model.rs`. | none declared |
| `code-highlight`<br>`crates/code-highlight` | Keep 2,427-line source unit; already below the target range. Review points if it grows: `grok`. | `grok` |
| `code-index`<br>`crates/code-index` | Keep 6,090-line source unit within the target range. Review points if it grows: `builder.rs`, `demand.rs`, `episodes.rs`. | none declared |
| `coder-binds`<br>`crates/coder-binds` | Keep 2,332-line source unit; already below the target range. Review points if it grows: `modifier.rs`, `rules.rs`. | none declared |
| `coder-boundary`<br>`crates/coder-boundary` | Keep 6,540-line source unit within the target range. Review points if it grows: `boundary.rs`, `cmdline.rs`, `privacy.rs`. | none declared |
| `coder-browser`<br>`crates/coder-browser` | Keep 2,778-line source unit; already below the target range. Review points if it grows: `browser.rs`, `pairing.rs`, `workbench.rs`. | none declared |
| `coder-browser-web`<br>`crates/coder-browser-web` | Keep 2,472-line source unit; already below the target range. Review points if it grows: `gpu.rs`, `mount.rs`. | none declared |
| `coder-chat-web`<br>`crates/coder-chat-web` | Keep 1,004-line source unit; already below the target range. Review points if it grows: `browser.rs`. | none declared |
| `coder-cloud-web`<br>`crates/coder-cloud-web` | Keep 845-line source unit; already below the target range. Review points if it grows: `browser.rs`, `resource.rs`. | none declared |
| `coder-components-web`<br>`crates/coder-components-web` | Keep 1,813-line source unit; already below the target range. Review points if it grows: `browser.rs`, `demo.rs`, `demo`. | none declared |
| `coder-connect`<br>`crates/coder-connect` | Keep 7,900-line source unit within the target range. Review points if it grows: `cli.rs`, `client.rs`, `direct.rs`. | `default`, `host`, `qr` |
| `coder-control`<br>`crates/coder-control` | Keep 4,056-line source unit; already below the target range. Review points if it grows: `client.rs`, `client`, `host.rs`. | `default`, `host` |
| `coder-demo-ui`<br>`crates/coder-demo-ui` | Keep 2,650-line source unit; already below the target range. Review points if it grows: `html.rs`, `slash.rs`, `svg.rs`. | none declared |
| `coder-desk`<br>`crates/coder-desk` | Keep 5,318-line source unit within the target range. Review points if it grows: `fake.rs`, `hyprland.rs`, `native.rs`. | `test-support` |
| `coder-desk-cli`<br>`crates/coder-desk-cli` | Keep 594-line source unit; already below the target range. | none declared |
| `coder-engine-status`<br>`crates/coder-engine-status` | Keep 1,555-line source unit; already below the target range. Review points if it grows: `claude.rs`, `limit.rs`, `plan.rs`. | none declared |
| `coder-environment`<br>`crates/coder-environment` | Keep 7,256-line source unit within the target range. Review points if it grows: `artifacts.rs`, `capture.rs`, `evidence`. | none declared |
| `coder-environment-build`<br>`crates/coder-environment-build` | Keep 3,170-line source unit; already below the target range. Review points if it grows: `sanitize.rs`, `service.rs`, `store.rs`. | none declared |
| `coder-environment-setup`<br>`crates/coder-environment-setup` | Keep 4,325-line source unit; already below the target range. Review points if it grows: `panel.rs`, `service.rs`, `source.rs`. | none declared |
| `coder-environment-verify`<br>`crates/coder-environment-verify` | Keep 3,516-line source unit; already below the target range. Review points if it grows: `plan.rs`, `service.rs`, `store.rs`. | none declared |
| `coder-hands`<br>`crates/coder-hands` | Keep 5,385-line source unit within the target range. Review points if it grows: `gestures.rs`, `judge.rs`, `landmarker.rs`. | `default`, `landmarker`, `judge` |
| `coder-hands-measure`<br>`crates/coder-hands-measure` | Keep 2,981-line source unit; already below the target range. Review points if it grows: `answers.rs`, `ask.rs`, `camera.rs`. | none declared |
| `coder-history`<br>`crates/coder-history` | Keep 8,256-line source unit within the target range. Review points if it grows: `devin.rs`, `devin`, `host.rs`. | `default`, `host`, `opencode`, `devin` |
| `coder-host-wire`<br>`crates/coder-host-wire` | Keep 582-line source unit; already below the target range. | none declared |
| `coder-labor`<br>`crates/coder-labor` | Keep 8,269-line source unit within the target range. Review points if it grows: `admission.rs`, `agreement.rs`, `book.rs`. | none declared |
| `coder-link`<br>`crates/coder-link` | Keep 2,135-line source unit; already below the target range. Review points if it grows: `clock.rs`, `ids.rs`, `policy.rs`. | none declared |
| `coder-mobile-probe`<br>`crates/coder-mobile-probe` | Keep 652-line source unit; already below the target range. Review points if it grows: `android.rs`, `android_keystore.rs`, `ios.rs`. | none declared |
| `coder-project`<br>`crates/coder-project` | Keep 8,197-line source unit within the target range. Review points if it grows: `artifact.rs`, `claims.rs`, `controller.rs`. | none declared |
| `coder-reach`<br>`crates/coder-reach` | Keep 4,401-line source unit; already below the target range. Review points if it grows: `artifact.rs`, `browser.rs`, `channel.rs`. | none declared |
| `coder-scheduler`<br>`crates/coder-scheduler` | Keep 4,182-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `ledger.rs`, `plan.rs`. | none declared |
| `coder-service`<br>`crates/coder-service` | Keep 6,452-line source unit within the target range. Review points if it grows: `adopt.rs`, `adopt`, `bundle.rs`. | none declared |
| `coder-sharing`<br>`crates/coder-sharing` | Keep 2,075-line source unit; already below the target range. Review points if it grows: `model.rs`, `store.rs`. | none declared |
| `coder-ssh`<br>`crates/coder-ssh` | Keep 2,221-line source unit; already below the target range. Review points if it grows: `askpass.rs`, `error.rs`, `job.rs`. | none declared |
| `coder-vt`<br>`crates/coder-vt` | Keep 5,809-line source unit within the target range. Review points if it grows: `authority.rs`, `cell.rs`, `continuation.rs`. | none declared |
| `coder-wm`<br>`crates/coder-wm` | Keep 1,927-line source unit; already below the target range. Review points if it grows: `screen.rs`. | none declared |
| `coder-working-computer`<br>`crates/coder-working-computer` | Keep 7,828-line source unit within the target range. Review points if it grows: `boat.rs`, `decide.rs`, `driver.rs`. | none declared |
| `coderbench`<br>`crates/coderbench` | Keep 3,985-line source unit; already below the target range. Review points if it grows: `drive.rs`, `preflight.rs`, `tune.rs`. | none declared |
| `coderos-camera`<br>`crates/coderos-camera` | Keep 2,815-line source unit; already below the target range. Review points if it grows: `capture.rs`, `config.rs`, `control.rs`. | none declared |
| `codex-transport`<br>`crates/codex-transport` | Keep 2,275-line source unit; already below the target range. Review points if it grows: `codex.rs`, `fake.rs`, `oneshot.rs`. | none declared |
| `commercial-accounts`<br>`crates/commercial-accounts` | Keep 2,441-line source unit; already below the target range. Review points if it grows: `commission.rs`. | `default`, `merchant-commissions` |
| `commercial-spend`<br>`crates/commercial-spend` | Keep 2,035-line source unit; already below the target range. Review points if it grows: `authority.rs`, `private.rs`, `protocol.rs`. | none declared |
| `compute-workbench`<br>`crates/compute-workbench` | Keep 2,391-line source unit; already below the target range. Review points if it grows: `host.rs`, `retail`. | `default`, `client`, `host` |
| `computer-conformance`<br>`crates/computer-conformance` | Keep 602-line source unit; already below the target range. | `space`, `archil` |
| `computer-credentials`<br>`crates/computer-credentials` | Keep 3,074-line source unit; already below the target range. Review points if it grows: `certificate_authority.rs`, `gateway.rs`, `host_transport.rs`. | none declared |
| `computer-driver`<br>`crates/computer-driver` | Keep 6,619-line source unit within the target range. Review points if it grows: `archil.rs`, `archil`, `boat.rs`. | `default`, `boat`, `archil`, `space` |
| `computer-enrolled`<br>`crates/computer-enrolled` | Keep 605-line source unit; already below the target range. | none declared |
| `computer-files`<br>`crates/computer-files` | Keep 502-line source unit; already below the target range. Review points if it grows: `linux.rs`. | none declared |
| `contribution-service`<br>`crates/contribution-service` | Keep 3,670-line source unit; already below the target range. Review points if it grows: `evaluate.rs`, `finance.rs`, `source.rs`. | none declared |
| `contribution-workbench`<br>`crates/contribution-workbench` | Keep 2,692-line source unit; already below the target range. Review points if it grows: `host.rs`, `host`, `quests.rs`. | `default`, `host` |
| `coordination-host`<br>`crates/coordination-host` | Keep 7,602-line source unit within the target range. Review points if it grows: `admin.rs`, `automation.rs`, `automation_cleanup.rs`. | none declared |
| `decision-panel`<br>`crates/decision-panel` | Keep 1,089-line source unit; already below the target range. | none declared |
| `design-lab`<br>`crates/design-lab` | Keep 3,299-line source unit; already below the target range. Review points if it grows: `kind.rs`, `mock.rs`. | none declared |
| `dhruv-conformance`<br>`crates/dhruv-conformance` | Keep 3-line source unit; already below the target range. | none declared |
| `discovery`<br>`crates/discovery` | Keep 2,106-line source unit; already below the target range. Review points if it grows: `corpus.rs`, `curated.rs`, `plugins.rs`. | none declared |
| `eval-runner`<br>`crates/eval-runner` | Keep 4,346-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `config.rs`, `defaults.rs`. | none declared |
| `everglade-web`<br>`crates/everglade-web` | Keep 5,588-line source unit within the target range. Review points if it grows: `cad_sync.rs`, `chamber.rs`, `grid.rs`. | none declared |
| `game-lab`<br>`crates/game-lab` | Keep 2,940-line source unit; already below the target range. Review points if it grows: `doc.rs`, `kind.rs`, `lab.rs`. | none declared |
| `github-actions`<br>`crates/github-actions` | Keep 3,586-line source unit; already below the target range. Review points if it grows: `action.rs`, `argv.rs`, `issues.rs`. | `default`, `http` |
| `gym-bridge`<br>`crates/gym-bridge` | Keep 3,239-line source unit; already below the target range. Review points if it grows: `client.rs`, `confined.rs`, `host.rs`. | `default`, `host` |
| `hook-log`<br>`crates/hook-log` | Keep 544-line source unit; already below the target range. Review points if it grows: `report.rs`. | none declared |
| `icecream-rules`<br>`crates/icecream-rules` | Keep 6,092-line source unit within the target range. Review points if it grows: `bot.rs`, `game.rs`, `geom.rs`. | none declared |
| `icecream-web`<br>`crates/icecream-web` | Keep 8,582-line source unit within the target range. Review points if it grows: `app.rs`, `camera.rs`, `copy.rs`. | none declared |
| `jev-hosted`<br>`crates/jev-hosted` | Keep 2,138-line source unit; already below the target range. | none declared |
| `kev`<br>`crates/kev` | Keep 3,773-line source unit; already below the target range. Review points if it grows: `api.rs`, `artifacts.rs`, `decision.rs`. | `serve`, `metal` |
| `lab-api`<br>`crates/lab-api` | Keep 2,109-line source unit; already below the target range. Review points if it grows: `contract.rs`, `handlers.rs`, `server.rs`. | none declared |
| `lab-jobs`<br>`crates/lab-jobs` | Keep 2,062-line source unit; already below the target range. Review points if it grows: `authority.rs`, `cloud.rs`, `coordinator.rs`. | `default`, `worker`, `schema`, `cloud` |
| `lab-kit`<br>`crates/lab-kit` | Keep 1,774-line source unit; already below the target range. Review points if it grows: `api.rs`, `card.rs`, `change.rs`. | `schema` |
| `lab-store`<br>`crates/lab-store` | Keep 2,230-line source unit; already below the target range. Review points if it grows: `backend.rs`, `cad_legacy.rs`, `state.rs`. | none declared |
| `lab-terminal`<br>`crates/lab-terminal` | Keep 714-line source unit; already below the target range. | none declared |
| `lab-ui`<br>`crates/lab-ui` | Keep 233-line source unit; already below the target range. | none declared |
| `labor-roles`<br>`crates/labor-roles` | Keep 4,011-line source unit; already below the target range. Review points if it grows: `book.rs`, `context.rs`, `host.rs`. | none declared |
| `lander-gl`<br>`crates/lander-gl` | Keep 907-line source unit; already below the target range. Review points if it grows: `glyphs.rs`, `heap.rs`, `text.rs`. | none declared |
| `lander5-web`<br>`crates/lander5-web` | Keep 785-line source unit; already below the target range. Review points if it grows: `bump.rs`, `shaders.rs`, `web.rs`. | `worker` |
| `laya`<br>`crates/laya` | Keep 3,618-line source unit; already below the target range. Review points if it grows: `api.rs`, `artifacts.rs`, `config.rs`. | `serve`, `metal` |
| `layertree-web`<br>`crates/layertree-web` | Keep 2,960-line source unit; already below the target range. Review points if it grows: `demo.rs`, `layertree.rs`, `layout.rs`. | none declared |
| `live-host`<br>`crates/live-host` | Keep 6,447-line source unit within the target range. Review points if it grows: `cli.rs`, `controller.rs`, `controller`. | none declared |
| `mac-jobs`<br>`crates/mac-jobs` | Keep 2,419-line source unit; already below the target range. Review points if it grows: `actor.rs`, `capabilities.rs`, `plan.rs`. | none declared |
| `markdown-stream`<br>`crates/markdown-stream` | Keep 942-line source unit; already below the target range. Review points if it grows: `autolink.rs`. | none declared |
| `memory-stream`<br>`crates/memory-stream` | Keep 378-line source unit; already below the target range. | none declared |
| `microluna`<br>`crates/microluna` | Keep 4,074-line source unit; already below the target range. Review points if it grows: `finish.rs`, `openrouter.rs`, `patch.rs`. | none declared |
| `model-access`<br>`crates/model-access` | Keep 2,574-line source unit; already below the target range. Review points if it grows: `access.rs`, `check.rs`, `connect.rs`. | `default`, `http`, `keychain` |
| `nostr-transport`<br>`crates/nostr-transport` | Keep 397-line source unit; already below the target range. Review points if it grows: `artifacts.rs`. | none declared |
| `oa-att`<br>`crates/oa-att` | Keep 2,053-line source unit; already below the target range. Review points if it grows: `net.rs`, `open.rs`, `token.rs`. | `net` |
| `oa-boat`<br>`crates/oa-boat` | Keep 5,541-line source unit within the target range. Review points if it grows: `api.rs`, `gce.rs`, `gce`. | none declared |
| `oa-connections`<br>`crates/oa-connections` | Keep 4,927-line source unit; already below the target range. Review points if it grows: `core.rs`, `discovery.rs`, `fake.rs`. | `fake` |
| `oa-copy`<br>`crates/oa-copy` | Keep 669-line source unit; already below the target range. | none declared |
| `oa-seal`<br>`crates/oa-seal` | Keep 424-line source unit; already below the target range. | none declared |
| `oa-tokens`<br>`crates/oa-tokens` | Keep 2,699-line source unit; already below the target range. Review points if it grows: `apps_sdk.rs`, `noir.rs`, `palette.rs`. | `serde` |
| `oa-vault`<br>`crates/oa-vault` | Keep 2,751-line source unit; already below the target range. Review points if it grows: `index.rs`, `jcs.rs`, `keys.rs`. | none declared |
| `oa-vault-web`<br>`crates/oa-vault-web` | Keep 727-line source unit; already below the target range. Review points if it grows: `wasm.rs`. | none declared |
| `oak`<br>`crates/oak` | Keep 3,903-line source unit; already below the target range. Review points if it grows: `mcp_http.rs`. | `mcp-http` |
| `openagents-connect`<br>`crates/openagents-connect` | Keep 4,113-line source unit; already below the target range. Review points if it grows: `code.rs`, `control.rs`, `control`. | none declared |
| `openagents-dashboard`<br>`crates/openagents-dashboard` | Keep 643-line source unit; already below the target range. | `schema` |
| `openagents-deck`<br>`crates/openagents-deck` | Keep 3,173-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `compose.rs`, `present.rs`. | `default`, `viewer` |
| `openagents-login`<br>`crates/openagents-login` | Keep 1,004-line source unit; already below the target range. Review points if it grows: `protocol.rs`. | `default`, `system` |
| `openagents-sdk`<br>`crates/openagents-sdk` | Keep 15,192-line source unit within the target range. Review points if it grows: `agent_lab.rs`, `agents.rs`, `agents`. | `default`, `transport`, `schema`, `labs-cad` |
| `openagents-spark`<br>`crates/spark-wallet` | Keep 3,772-line source unit; already below the target range. Review points if it grows: `browser.rs`, `computer.rs`, `config.rs`. | none declared |
| `openagents-terminal`<br>`crates/openagents-terminal` | Keep 8,502-line source unit within the target range. Review points if it grows: `app.rs`, `copy.rs`, `draw.rs`. | none declared |
| `openagents-wallet`<br>`crates/wallet` | Keep 3,988-line source unit; already below the target range. Review points if it grows: `backup.rs`, `config.rs`, `custody.rs`. | `default`, `ldk` |
| `openagents-web-site`<br>`crates/openagents-web-site` | Keep 7,449-line source unit within the target range. Review points if it grows: `agent_card_page.rs`, `agent_lab_page.rs`, `run_commands.rs`. | none declared |
| `openagents-workspace-hack`<br>`crates/workspace-hack` | Keep generated feature-unification glue; splitting it would recreate feature drift. | none declared |
| `openrouter`<br>`crates/openrouter` | Keep 2,514-line source unit; already below the target range. | none declared |
| `openui-lang`<br>`crates/openui-lang` | Keep 2,715-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `edit.rs`, `embed.rs`. | none declared |
| `paper-mono`<br>`crates/paper-mono` | Keep 128-line source unit; already below the target range. | none declared |
| `pay-host`<br>`crates/pay-host` | Keep 2,187-line source unit; already below the target range. Review points if it grows: `ingest.rs`, `usage.rs`. | none declared |
| `playtest`<br>`crates/playtest` | Keep 3,337-line source unit; already below the target range. Review points if it grows: `award.rs`, `award`, `feedback.rs`. | none declared |
| `plugin`<br>`crates/plugin` | Keep 2,328-line source unit; already below the target range. Review points if it grows: `engine.rs`, `memory.rs`, `modules.rs`. | none declared |
| `plugin-action-items`<br>`crates/plugin-action-items` | Keep 547-line source unit; already below the target range. | none declared |
| `plugin-code-search`<br>`crates/plugin-code-search` | Keep 451-line source unit; already below the target range. | none declared |
| `plugin-dependency-check`<br>`crates/plugin-dependency-check` | Keep 1,836-line source unit; already below the target range. | none declared |
| `plugin-explain-error`<br>`crates/plugin-explain-error` | Keep 1,821-line source unit; already below the target range. | none declared |
| `plugin-hook-guard`<br>`crates/plugin-hook-guard` | Keep 155-line source unit; already below the target range. | none declared |
| `plugin-outline`<br>`crates/plugin-outline` | Keep 205-line source unit; already below the target range. | `default`, `snapshot` |
| `plugin-pdk`<br>`crates/plugin-pdk` | Keep 765-line source unit; already below the target range. Review points if it grows: `guest.rs`. | `default`, `guest` |
| `plugin-registry`<br>`crates/plugin-registry` | Keep 3,573-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `cli.rs`, `listings.rs`. | none declared |
| `plugin-release-notes`<br>`crates/plugin-release-notes` | Keep 803-line source unit; already below the target range. | none declared |
| `plugin-repo-map`<br>`crates/plugin-repo-map` | Keep 340-line source unit; already below the target range. | none declared |
| `plugin-test-report`<br>`crates/plugin-test-report` | Keep 715-line source unit; already below the target range. | none declared |
| `private-fs`<br>`crates/private-fs` | Keep 545-line source unit; already below the target range. Review points if it grows: `imp.rs`. | none declared |
| `program-launch`<br>`crates/program-launch` | Keep 567-line source unit; already below the target range. | none declared |
| `push-gateway`<br>`crates/push-gateway` | Keep 3,764-line source unit; already below the target range. Review points if it grows: `client.rs`, `nip98.rs`, `server`. | `default`, `server`, `client` |
| `pylons-view`<br>`crates/pylons-view` | Keep 601-line source unit; already below the target range. Review points if it grows: `terminal.rs`. | none declared |
| `pylons-web`<br>`crates/pylons-web` | Keep 1,169-line source unit; already below the target range. Review points if it grows: `camera.rs`, `terminal_web.rs`, `web.rs`. | none declared |
| `receipts`<br>`crates/receipts` | Keep 9,339-line source unit within the target range. Review points if it grows: `brainstorm_pilot.rs`, `brainstorm_pilot`, `decision_metering.rs`. | none declared |
| `receipts-nostr`<br>`crates/receipts-nostr` | Keep 182-line source unit; already below the target range. | none declared |
| `retail-qualify`<br>`crates/retail-qualify` | Keep 3,683-line source unit; already below the target range. Review points if it grows: `acceptance.rs`, `bindings.rs`, `bound.rs`. | none declared |
| `retail-service`<br>`crates/retail-service` | Keep 6,246-line source unit within the target range. Review points if it grows: `backup.rs`, `commercial.rs`, `custody.rs`. | none declared |
| `robot-data`<br>`crates/robot-data` | Keep 3,014-line source unit; already below the target range. Review points if it grows: `dataset.rs`, `export.rs`, `mapping.rs`. | `default`, `video`, `export`, `publish` |
| `robot-description`<br>`crates/robot-description` | Keep 1,731-line source unit; already below the target range. Review points if it grows: `error.rs`, `pin.rs`, `stl.rs`. | none declared |
| `robot-lab`<br>`crates/robot-lab` | Keep 3,531-line source unit; already below the target range. Review points if it grows: `document.rs`, `import.rs`, `kind.rs`. | `platform` |
| `robot-policy`<br>`crates/robot-policy` | Keep 3,541-line source unit; already below the target range. Review points if it grows: `act.rs`, `artifacts.rs`, `controller.rs`. | none declared |
| `robot-sim`<br>`crates/robot-sim` | Keep 4,267-line source unit; already below the target range. Review points if it grows: `import.rs`, `jobs.rs`, `mujoco.rs`. | `default`, `mujoco` |
| `robot-web`<br>`crates/robot-web` | Keep 2,204-line source unit; already below the target range. Review points if it grows: `app.rs`. | none declared |
| `robot-workbench`<br>`crates/robot-workbench` | Keep 4,774-line source unit; already below the target range. Review points if it grows: `device.rs`, `experiments.rs`, `host.rs`. | `default`, `host` |
| `route-contract`<br>`crates/route-contract` | Keep 9,833-line source unit within the target range. Review points if it grows: `binding.rs`, `children.rs`, `decision.rs`. | none declared |
| `rust-native-web`<br>`crates/rust-native-web` | Keep 1,303-line source unit; already below the target range. Review points if it grows: `browser.rs`, `editing.rs`, `html.rs`. | `default`, `browser` |
| `sand-garden`<br>`crates/sand-garden` | Keep 2,265-line source unit; already below the target range. Review points if it grows: `browser.rs`, `creatures.rs`, `paint.rs`. | none declared |
| `secret-screen`<br>`crates/secret-screen` | Keep 780-line source unit; already below the target range. | none declared |
| `ship-lab`<br>`crates/ship-lab` | Keep 953-line source unit; already below the target range. Review points if it grows: `kind.rs`. | none declared |
| `snap-rules`<br>`crates/snap-rules` | Keep 5,211-line source unit within the target range. Review points if it grows: `bot.rs`, `build.rs`, `game.rs`. | none declared |
| `snap-share`<br>`crates/snap-share` | Keep 169-line source unit; already below the target range. | none declared |
| `snap-web`<br>`crates/snap-web` | Keep 7,107-line source unit within the target range. Review points if it grows: `access.rs`, `app.rs`, `copy.rs`. | `default`, `browser`, `autoplay` |
| `solver-box`<br>`crates/solver-box` | Keep 2,694-line source unit; already below the target range. Review points if it grows: `computers.rs`, `computers`, `dispatch.rs`. | none declared |
| `sov-custody`<br>`crates/sov-custody` | Keep 1,832-line source unit; already below the target range. Review points if it grows: `cli.rs`, `custodian.rs`, `signer.rs`. | none declared |
| `sov-lifecycle`<br>`crates/sov-lifecycle` | Keep 1,859-line source unit; already below the target range. Review points if it grows: `cli.rs`, `coordinator.rs`. | none declared |
| `sov-treasury`<br>`crates/sov-treasury` | Keep 2,285-line source unit; already below the target range. Review points if it grows: `adapters.rs`, `cli.rs`, `treasury.rs`. | none declared |
| `space-key-broker`<br>`crates/space-key-broker` | Keep 874-line source unit; already below the target range. Review points if it grows: `kms.rs`. | none declared |
| `spark-wallet-web`<br>`crates/spark-wallet-web` | Keep 242-line source unit; already below the target range. Review points if it grows: `browser.rs`. | none declared |
| `squeeze`<br>`crates/squeeze` | Keep 1,133-line source unit; already below the target range. Review points if it grows: `shell.rs`. | none declared |
| `standing-jobs`<br>`crates/standing-jobs` | Keep 256-line source unit; already below the target range. Review points if it grows: `ledger.rs`. | none declared |
| `supervise`<br>`crates/supervise` | Keep 3,249-line source unit; already below the target range. Review points if it grows: `blocking.rs`, `group.rs`, `job.rs`. | `default`, `job` |
| `terminal-app`<br>`crates/terminal-app` | Keep 1,562-line source unit; already below the target range. Review points if it grows: `window.rs`, `window`. | none declared |
| `terminal-control`<br>`crates/terminal-control` | Keep 485-line source unit; already below the target range. Review points if it grows: `unix.rs`, `windows.rs`. | none declared |
| `terminal-gfx`<br>`crates/terminal-gfx` | Keep 6,311-line source unit within the target range. Review points if it grows: `draw.rs`, `glyphs.rs`, `helpers.rs`. | `default`, `native`, `fonts` |
| `terminal-mux`<br>`crates/terminal-mux` | Keep 817-line source unit; already below the target range. | `host` |
| `terminal-remote`<br>`crates/terminal-remote` | Keep 446-line source unit; already below the target range. | none declared |
| `terminal-studio`<br>`crates/terminal-studio` | Keep 2,981-line source unit; already below the target range. Review points if it grows: `native.rs`, `onboarding.rs`, `onboarding`. | `onboarding-host`, `acceptance` |
| `terminal-tty`<br>`crates/terminal-tty` | Keep 835-line source unit; already below the target range. Review points if it grows: `runner.rs`. | none declared |
| `town-clock`<br>`crates/town-clock` | Keep 723-line source unit; already below the target range. | none declared |
| `townsfolk`<br>`crates/townsfolk` | Keep 4,830-line source unit; already below the target range. Review points if it grows: `diffusion.rs`, `files.rs`, `quest.rs`. | none declared |
| `user-home`<br>`crates/user-home` | Keep 201-line source unit; already below the target range. | none declared |
| `vault-operator`<br>`crates/vault-operator` | Keep 921-line source unit; already below the target range. Review points if it grows: `audit.rs`, `cli.rs`, `service.rs`. | none declared |
| `vault-sealed`<br>`crates/vault-sealed` | Keep 1,162-line source unit; already below the target range. Review points if it grows: `client.rs`, `kms.rs`, `workload.rs`. | none declared |
| `verse-bake`<br>`crates/verse-bake` | Keep 4,112-line source unit; already below the target range. Review points if it grows: `backend.rs`, `bake.rs`, `fixture.rs`. | `gpu`, `everglade` |
| `verse-content`<br>`crates/verse-content` | Keep 8,727-line source unit within the target range. Review points if it grows: `authoring`, `collision.rs`, `compiler`. | `default`, `compiler`, `authoring` |
| `verse-core`<br>`crates/verse-core` | Keep 5,810-line source unit within the target range. Review points if it grows: `agent.rs`, `avatar.rs`, `crowd.rs`. | none declared |
| `verse-film`<br>`crates/verse-film` | Keep 2,230-line source unit; already below the target range. Review points if it grows: `export.rs`, `layers.rs`, `render.rs`. | `default`, `gpu`, `layers` |
| `verse-game`<br>`crates/verse-game` | Keep 475-line source unit; already below the target range. | none declared |
| `verse-gfx`<br>`crates/verse-gfx` | Keep 2,675-line source unit; already below the target range. Review points if it grows: `camera.rs`, `gles.rs`, `gpu_lifecycle.rs`. | `default`, `world-camera` |
| `verse-gym`<br>`crates/verse-gym` | Keep 4,475-line source unit; already below the target range. Review points if it grows: `gym.rs`, `gym_evals.rs`, `gym_evals`. | none declared |
| `verse-host`<br>`crates/verse-host` | Keep 782-line source unit; already below the target range. Review points if it grows: `operations.rs`. | none declared |
| `verse-lagrange`<br>`crates/verse-lagrange` | Keep 5,493-line source unit within the target range. Review points if it grows: `arrays.rs`, `earth_moon.rs`, `orbit.rs`. | none declared |
| `verse-lagrange2`<br>`crates/verse-lagrange2` | Keep 5,385-line source unit within the target range. Review points if it grows: `arm.rs`, `assembly.rs`, `burn.rs`. | none declared |
| `verse-net`<br>`crates/verse-net` | Keep 6,601-line source unit within the target range. Review points if it grows: `blocklist.rs`, `chat.rs`, `feed.rs`. | `xp`, `test-support` |
| `verse-private`<br>`crates/verse-private` | Keep 2,014-line source unit; already below the target range. Review points if it grows: `auth.rs`, `broker.rs`, `client.rs`. | `default`, `client`, `broker` |
| `verse-water-spells`<br>`crates/verse-water-spells` | Keep 6,756-line source unit within the target range. Review points if it grows: `bolt.rs`, `floats.rs`, `hotbar.rs`. | none declared |
| `verse-zone-arena`<br>`crates/verse-zone-arena` | Keep 2,791-line source unit; already below the target range. Review points if it grows: `ai.rs`, `draw.rs`, `hud.rs`. | none declared |
| `verse-zone-coast`<br>`crates/verse-zone-coast` | Keep 1,338-line source unit; already below the target range. Review points if it grows: `kit.rs`, `pack.rs`, `scene.rs`. | none declared |
| `verse-zone-crypt`<br>`crates/verse-zone-crypt` | Keep 995-line source unit; already below the target range. Review points if it grows: `embedded.rs`. | `embedded`, `great-crypt` |
| `verse-zone-duel`<br>`crates/verse-zone-duel` | Keep 5,490-line source unit within the target range. Review points if it grows: `ai.rs`, `draw.rs`, `hits.rs`. | none declared |
| `verse-zone-grove`<br>`crates/verse-zone-grove` | Keep 8,223-line source unit within the target range. Review points if it grows: `zones`. | none declared |
| `verse-zone-lab`<br>`crates/verse-zone-lab` | Keep 2,764-line source unit; already below the target range. Review points if it grows: `draw.rs`, `scenes.rs`. | none declared |
| `verse-zone-lagrange`<br>`crates/verse-zone-lagrange` | Keep 1,640-line source unit; already below the target range. Review points if it grows: `draw.rs`, `light.rs`. | none declared |
| `verse-zone-lagrange2`<br>`crates/verse-zone-lagrange2` | Keep 1,370-line source unit; already below the target range. Review points if it grows: `light.rs`, `scene.rs`. | none declared |
| `verse-zone-robotlab`<br>`crates/verse-zone-robotlab` | Keep 2,113-line source unit; already below the target range. Review points if it grows: `draw.rs`, `local.rs`. | none declared |
| `verse-zone-water`<br>`crates/verse-zone-water` | Keep 483-line source unit; already below the target range. Review points if it grows: `coast.rs`. | none declared |
| `video-lab`<br>`crates/video-lab` | Keep 6,233-line source unit within the target range. Review points if it grows: `check.rs`, `edit.rs`, `fmt.rs`. | none declared |
| `video-lab-serve`<br>`crates/video-lab-serve` | Keep 717-line source unit; already below the target range. | none declared |
| `voyager`<br>`crates/voyager` | Keep 9,765-line source unit within the target range. Review points if it grows: `bridge.rs`, `critic.rs`, `curriculum.rs`. | none declared |
| `work-host`<br>`crates/work-host` | Keep 4,721-line source unit; already below the target range. Review points if it grows: `access.rs`, `authority.rs`, `checks.rs`. | none declared |
| `workbench`<br>`crates/workbench` | Keep 1,367-line source unit; already below the target range. Review points if it grows: `pane.rs`. | none declared |
| `workbench-session`<br>`crates/workbench-session` | Keep 570-line source unit; already below the target range. Review points if it grows: `projection.rs`. | none declared |
| `workflow-run`<br>`crates/workflow-run` | Keep 2,518-line source unit; already below the target range. Review points if it grows: `engine.rs`, `script.rs`. | none declared |
| `workspace-host`<br>`crates/workspace-host` | Keep 4,202-line source unit; already below the target range. Review points if it grows: `artifact.rs`, `bridge.rs`, `checkpoint.rs`. | none declared |
| `world-tree`<br>`crates/world-tree` | Keep 1,790-line source unit; already below the target range. Review points if it grows: `choose.rs`, `known.rs`, `state.rs`. | none declared |
| `xp-ledger`<br>`crates/xp-ledger` | Keep 4,588-line source unit; already below the target range. Review points if it grows: `adopt.rs`, `defaults.rs`, `entry.rs`. | `fixtures` |
| `openagents`<br>`apps/openagents/apps/openagents` | Keep 2,016-line source unit; already below the target range. Review points if it grows: `auth_cli.rs`, `daemon.rs`, `host_upgrade_cli.rs`. | `default`, `ui` |
| `openagents-agent-graph-view`<br>`apps/openagents/crates/agent-graph-view` | Keep 1,384-line source unit; already below the target range. Review points if it grows: `theme.rs`, `view.rs`. | none declared |
| `openagents-audio`<br>`apps/openagents/crates/audio` | Keep 588-line source unit; already below the target range. Review points if it grows: `aec.rs`, `dsp.rs`, `native.rs`. | `default`, `native`, `aec` |
| `openagents-background`<br>`apps/openagents/crates/background` | Keep 1,545-line source unit; already below the target range. Review points if it grows: `cli.rs`, `decide.rs`, `fresh.rs`. | none declared |
| `openagents-brand`<br>`apps/openagents/crates/brand` | Keep 204-line source unit; already below the target range. | none declared |
| `openagents-gpui-test`<br>`apps/openagents/crates/gpui-test` | Keep 135-line source unit; already below the target range. | none declared |
| `openagents-host-upgrade`<br>`apps/openagents/crates/host-upgrade` | Keep 2,360-line source unit; already below the target range. Review points if it grows: `app.rs`, `grant.rs`, `layout.rs`. | none declared |
| `openagents-markdown`<br>`apps/openagents/crates/markdown` | Keep 2,263-line source unit; already below the target range. Review points if it grows: `mend.rs`, `parser.rs`. | none declared |
| `openagents-media-view`<br>`apps/openagents/crates/media-view` | Keep 4,073-line source unit; already below the target range. Review points if it grows: `fonts.rs`, `image_media.rs`, `lab_card.rs`. | `test-support` |
| `openagents-orb`<br>`apps/openagents/crates/orb` | Keep 2,788-line source unit; already below the target range. Review points if it grows: `animator.rs`, `engine`, `motion.rs`. | none declared |
| `openagents-preview`<br>`apps/openagents/crates/preview` | Keep 3,660-line source unit; already below the target range. Review points if it grows: `catalog.rs`, `discovery.rs`, `login.rs`. | none declared |
| `openagents-proto`<br>`apps/openagents/crates/proto` | Keep 8,279-line source unit within the target range. Review points if it grows: `agent.rs`, `agent_wakes.rs`, `attachment_mentions.rs`. | none declared |
| `openagents-rpc`<br>`apps/openagents/crates/rpc` | Keep 3,544-line source unit; already below the target range. Review points if it grows: `browsers.rs`, `client.rs`, `connect_phone.rs`. | none declared |
| `openagents-syntax`<br>`apps/openagents/crates/syntax` | Keep 1,390-line source unit; already below the target range. Review points if it grows: `engine.rs`. | `default`, `highlight` |
| `openagents-text`<br>`apps/openagents/crates/text` | Keep 4,070-line source unit; already below the target range. Review points if it grows: `analysis.rs`, `cache.rs`, `chars.rs`. | none declared |
| `openagents-theme`<br>`apps/openagents/crates/theme` | Keep 5,147-line source unit within the target range. Review points if it grows: `builtins.rs`, `library.rs`, `vscode.rs`. | none declared |
| `openagents-update`<br>`apps/openagents/crates/update` | Keep 2,583-line source unit; already below the target range. Review points if it grows: `windows.rs`. | none declared |
| `openagents-veil`<br>`apps/openagents/crates/veil` | Keep 606-line source unit; already below the target range. Review points if it grows: `caption.rs`. | none declared |
| `openagents-visual`<br>`apps/openagents/crates/visual` | Keep 6,471-line source unit within the target range. Review points if it grows: `assets.rs`, `draw.rs`, `image.rs`. | `portable-runtime` |
| `openagents-visual-gpu`<br>`apps/openagents/crates/visual-gpu` | Keep 3,380-line source unit; already below the target range. Review points if it grows: `scene.rs`, `shader.rs`, `textures.rs`. | none declared |
| `openagents-voice`<br>`apps/openagents/crates/voice` | Keep 981-line source unit; already below the target range. Review points if it grows: `resample.rs`. | none declared |
| `openagents-voice-media`<br>`apps/openagents/crates/voice-media` | Keep 674-line source unit; already below the target range. | none declared |
| `openagents-voice-session`<br>`apps/openagents/crates/voice-session` | Keep 1,285-line source unit; already below the target range. Review points if it grows: `runtime.rs`, `view.rs`. | `default`, `runtime` |
| `openagents-workspace-hack`<br>`apps/openagents/crates/workspace-hack` | Keep generated feature-unification glue; splitting it would recreate feature drift. | none declared |
| `openagents-first-frame`<br>`apps/openagents/web/first-frame` | Keep 1,211-line source unit; already below the target range. | none declared |
| `openagents-first-frame-capture`<br>`apps/openagents/web/first-frame-capture` | Keep 730-line source unit; already below the target range. Review points if it grows: `capture.rs`, `diff.rs`, `platform.rs`. | none declared |
| `openagents-first-frame-paint`<br>`apps/openagents/web/first-frame-paint` | Keep 954-line source unit; already below the target range. Review points if it grows: `gl.rs`, `shaders.rs`. | `default`, `painter` |
| `openagents-first-frame-record`<br>`apps/openagents/web/first-frame-record` | Keep 639-line source unit; already below the target range. | none declared |
| `openagents-gpui-web`<br>`apps/openagents/web/gpui-web` | Keep 5,182-line source unit within the target range. Review points if it grows: `dispatcher.rs`, `display.rs`, `events.rs`. | `default`, `webgpu`, `webgl`, `multithreaded`, `first-frame`, `browser-text` |
| `openagents-web-admin-analytics`<br>`apps/openagents/web/admin-analytics` | Keep 25-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-admin-attribution`<br>`apps/openagents/web/admin-attribution` | Keep 25-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-admin-drive-access`<br>`apps/openagents/web/admin-drive-access` | Keep 25-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-agent-graph`<br>`apps/openagents/web/agent-graph` | Keep 278-line source unit; already below the target range. Review points if it grows: `accessibility.rs`, `assets.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-agent-lab`<br>`apps/openagents/web/agent-lab` | Keep 1,240-line source unit; already below the target range. Review points if it grows: `agent_lab`, `lab.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-agents`<br>`apps/openagents/web/agents` | Keep 181-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-cad`<br>`apps/openagents/web/cad` | Keep 458-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-card`<br>`apps/openagents/web/card` | Keep 510-line source unit; already below the target range. Review points if it grows: `app.rs`, `demo.rs`. | `webgpu`, `webgl`, `default`, `export` |
| `openagents-web-chat-view`<br>`apps/openagents/web/chat-view` | Keep 4,179-line source unit; already below the target range. Review points if it grows: `assets.rs`, `composer.rs`, `desktop_transcript.rs`. | `visuals` |
| `openagents-web-claude`<br>`apps/openagents/web/claude` | Keep 18-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-client`<br>`apps/openagents/web/client` | Keep 4,253-line source unit; already below the target range. Review points if it grows: `chat_replica.rs`, `chat_room.rs`, `cloud_computers.rs`. | none declared |
| `openagents-web-cloud-settings`<br>`apps/openagents/web/cloud-settings` | Keep 231-line source unit; already below the target range. Review points if it grows: `page.rs`. | none declared |
| `openagents-web-connection-pages`<br>`apps/openagents/web/connection-pages` | Keep 212-line source unit; already below the target range. Review points if it grows: `page.rs`. | none declared |
| `openagents-web-connections`<br>`apps/openagents/web/connections` | Keep 20-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-data`<br>`apps/openagents/web/data` | Keep 18-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-demo`<br>`apps/openagents/web/demo` | Keep 1,606-line source unit; already below the target range. Review points if it grows: `app.rs`, `approval.rs`, `demo.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-dictation`<br>`apps/openagents/web/dictation` | Keep 357-line source unit; already below the target range. | none declared |
| `openagents-web-files`<br>`apps/openagents/web/files` | Keep 2,585-line source unit; already below the target range. Review points if it grows: `app.rs`, `app`, `model.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-graph-card`<br>`apps/openagents/web/graph-card` | Keep 4,314-line source unit; already below the target range. Review points if it grows: `app.rs`, `chat.rs`, `graph.rs`. | `webgpu`, `webgl`, `default`, `export`, `hands` |
| `openagents-web-hands`<br>`apps/openagents/web/hands` | Keep 1,184-line source unit; already below the target range. Review points if it grows: `filter.rs`, `synth.rs`, `track.rs`. | none declared |
| `openagents-web-keys`<br>`apps/openagents/web/keys` | Keep 362-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-lab`<br>`apps/openagents/web/lab` | Keep 211-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-lander-chat`<br>`apps/openagents/web/lander-chat` | Keep 1,498-line source unit; already below the target range. Review points if it grows: `app.rs`, `model.rs`. | `webgpu`, `webgl`, `default`, `export` |
| `openagents-web-page-host`<br>`apps/openagents/web/page-host` | Keep 449-line source unit; already below the target range. Review points if it grows: `host.rs`. | none declared |
| `openagents-web-projects`<br>`apps/openagents/web/projects` | Keep 441-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-pylon-card`<br>`apps/openagents/web/pylon-card` | Keep 372-line source unit; already below the target range. Review points if it grows: `app.rs`. | `webgpu`, `webgl`, `default`, `export` |
| `openagents-web-room`<br>`apps/openagents/web/room` | Keep 76-line source unit; already below the target range. | none declared |
| `openagents-web-settings`<br>`apps/openagents/web/settings` | Keep 2,192-line source unit; already below the target range. Review points if it grows: `app.rs`, `app`. | `default`, `webgpu`, `webgl` |
| `openagents-web-terminal`<br>`apps/openagents/web/terminal` | Keep 20-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-traces`<br>`apps/openagents/web/traces` | Keep 210-line source unit; already below the target range. Review points if it grows: `page.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-transcript-extras`<br>`apps/openagents/web/transcript-extras` | Keep 702-line source unit; already below the target range. Review points if it grows: `app.rs`, `model.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-vault`<br>`apps/openagents/web/vault` | Keep 20-line source unit; already below the target range. | `default`, `webgpu`, `webgl` |
| `openagents-web-visuals`<br>`apps/openagents/web/visuals` | Keep 282-line source unit; already below the target range. Review points if it grows: `browser.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-voice`<br>`apps/openagents/web/voice` | Keep 379-line source unit; already below the target range. Review points if it grows: `stage.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-work`<br>`apps/openagents/web/work` | Keep 1,350-line source unit; already below the target range. Review points if it grows: `app.rs`, `model.rs`. | `default`, `webgpu`, `webgl` |
| `openagents-web-card-webgpu`<br>`apps/openagents/web/card/webgpu` | Keep 12-line source unit; already below the target range. | none declared |
| `openagents-web-graph-card-webgpu`<br>`apps/openagents/web/graph-card/webgpu` | Keep 12-line source unit; already below the target range. | `hands` |
| `openagents-web-lander-chat-webgpu`<br>`apps/openagents/web/lander-chat/webgpu` | Keep 12-line source unit; already below the target range. | none declared |
| `openagents-web-pylon-card-webgpu`<br>`apps/openagents/web/pylon-card/webgpu` | Keep 19-line source unit; already below the target range. | none declared |
| `layertree-attest`<br>`crates/layertree-tee-lab/client` | Keep 2,565-line source unit; already below the target range. Review points if it grows: `channel.rs`, `nostr_mirror.rs`. | none declared |
| `layertree-tee-lab-fuzz`<br>`crates/layertree-tee-lab/fuzz` | Keep packaging/build/test glue; no source implementation to extract. | none declared |
| `layertree-signet-service`<br>`crates/layertree-tee-lab/server` | Keep 931-line source unit; already below the target range. | none declared |
| `lnd-signet-lab`<br>`crates/lnd-sdk/signet-lab` | Keep 1,801-line source unit; already below the target range. | none declared |
| `deepseek-recipe`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `protocol`, `request`, `stream`. | `upstream-tokenizer-tests` |
| `deepseek-recipe-core`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-core` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `conversation.rs`, `json.rs`, `messages.rs`. | none declared |
| `deepseek-recipe-encoding`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-encoding` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `tokenizer.rs`, `v4`. | `upstream-tokenizer-tests` |
| `deepseek-recipe-macros`<br>`crates/psionic/vendor/deepseek-recipe/deepseek-recipe-macros` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `prompt.rs`, `untagged.rs`. | none declared |
| `psionic-adapters`<br>`crates/psionic/crates/psionic-adapters` | Keep 1,707-line source unit; already below the target range. | none declared |
| `psionic-apple-fm`<br>`crates/psionic/crates/psionic-apple-fm` | Keep 6,755-line source unit within the target range. Review points if it grows: `client.rs`, `client`, `contract.rs`. | none declared |
| `psionic-array`<br>`crates/psionic/crates/psionic-array` | Keep 6,137-line source unit within the target range. | none declared |
| `psionic-backend-cpu`<br>`crates/psionic/crates/psionic-backend-cpu` | Keep 3,227-line source unit; already below the target range. | none declared |
| `psionic-backend-tests`<br>`crates/psionic/crates/psionic-backend-tests` | Keep 469-line source unit; already below the target range. | none declared |
| `psionic-catalog`<br>`crates/psionic/crates/psionic-catalog` | Keep 7,448-line source unit within the target range. Review points if it grows: `ollama.rs`, `registry.rs`, `tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate.rs`. | none declared |
| `psionic-collectives`<br>`crates/psionic/crates/psionic-collectives` | Keep 3,858-line source unit; already below the target range. Review points if it grows: `failure_semantics.rs`. | none declared |
| `psionic-core`<br>`crates/psionic/crates/psionic-core` | Keep 5,514-line source unit within the target range. Review points if it grows: `ggml_quantization.rs`, `philox.rs`, `ternary.rs`. | none declared |
| `psionic-datastream`<br>`crates/psionic/crates/psionic-datastream` | Keep 3,818-line source unit; already below the target range. | none declared |
| `psionic-environments`<br>`crates/psionic/crates/psionic-environments` | Keep 8,486-line source unit within the target range. Review points if it grows: `apple_adapter.rs`, `runtime_service.rs`, `tassadar.rs`. | none declared |
| `psionic-inference`<br>`crates/psionic/crates/psionic-inference` | Keep 6,897-line source unit within the target range. Review points if it grows: `artifacts.rs`, `buffers.rs`, `cpu.rs`. | `default`, `native-cuda`, `native-metal`, `ort`, `http` |
| `psionic-nn`<br>`crates/psionic/crates/psionic-nn` | Keep 5,542-line source unit within the target range. Review points if it grows: `layers.rs`, `quantized.rs`, `training.rs`. | none declared |
| `psionic-observe`<br>`crates/psionic/crates/psionic-observe` | Keep 529-line source unit; already below the target range. | `default`, `tokio-runtime-telemetry`, `tokio-runtime-telemetry-cpu` |
| `psionic-optimize`<br>`crates/psionic/crates/psionic-optimize` | Keep 4,622-line source unit; already below the target range. | none declared |
| `psionic-transformer`<br>`crates/psionic/crates/psionic-transformer` | Keep 6,315-line source unit within the target range. Review points if it grows: `attention.rs`, `blocks.rs`, `encoder_decoder.rs`. | none declared |
| `robot-mujoco-bindings`<br>`crates/robot-sim/tools/bindings` | Keep 34-line source unit; already below the target range. | none declared |
| `coder-sync-conformance`<br>`deploy/coder-sync/conformance` | Keep 186-line source unit; already below the target range. | none declared |
| `coder-sync-service`<br>`deploy/coder-sync` | Keep packaging/build/test glue; no source implementation to extract. | `server` |
| `iroh-relay-live-test`<br>`deploy/iroh-relay/live-test` | Keep 1-line source unit; already below the target range. | none declared |
| `space-credential-fixture`<br>`deploy/space-credentials/fixture` | Keep 297-line source unit; already below the target range. | none declared |
| `environment-fixture`<br>`deploy/space-host/fixtures/environment` | Keep 36-line source unit; already below the target range. | none declared |
| `lev-domain-gap-analysis`<br>`docs/decision-models/2026-09-20-lev-domain-gap/analysis` | Keep packaging/build/test glue; no source implementation to extract. | none declared |
| `sdk-history-example`<br>`examples/sdk-history` | Keep 281-line source unit; already below the target range. | none declared |
| `sdk-labs-example`<br>`examples/sdk-labs` | Keep 275-line source unit; already below the target range. | none declared |
| `mc-bridge`<br>`mc-bridge` | Keep 1,078-line source unit; already below the target range. | none declared |
| `ingest`<br>`crates/plugin-dependency-check/evals/loose-ranges/fixtures` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. | none declared |
| `ledger`<br>`crates/plugin-dependency-check/fixtures/tree` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. | none declared |
| `ledger`<br>`crates/plugin-explain-error/evals/rust-mismatch/fixtures` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. Review points if it grows: `ledger.rs`. | none declared |
| `lev-state-sweep-choice`<br>`docs/decision-models/2026-09-20-state-budget-choice/original-runner` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. | none declared |
| `lev-state-sweep-choice-recovery`<br>`docs/decision-models/2026-09-20-state-budget-choice/recovery-runner` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. | none declared |
| `openagents-workspace-hack`<br>`apps/openagents/web/hack-stub` | Keep the browser workspace-hack patch stub. Standalone metadata rejects its workspace membership; there is no implementation to split. | none declared |
| `orders-api`<br>`crates/plugin-repo-map/evals/largest-file/fixtures` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. Review points if it grows: `routes.rs`. | none declared |
| `relay-bridge`<br>`crates/plugin-dependency-check/evals/cargo-duplicates/fixtures` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. | none declared |
| `tally`<br>`crates/ext-eval/fixtures/repo-map-brief/evals/overview/fixtures` | Retain as frozen fixture input; Cargo metadata rejects it. Do not turn it into a product crate. Review points if it grows: `tables.rs`. | none declared |
| `bridge-planner`<br>`quests/bridge-planner/fixture` | Keep 28-line source unit; already below the target range. | none declared |
| `openagents-workspace-hack`<br>`support/workspace-hack` | Keep generated feature-unification glue; splitting it would recreate feature drift. | none declared |
| `boltz-cli`<br>`vendor/boltz-client/crates/cli` | Keep vendored package shape; coordinate any split upstream. | none declared |
| `macros`<br>`vendor/boltz-client/crates/macros` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `async_trait.rs`, `derive_from.rs`, `derive_into.rs`. | none declared |
| `platform-utils`<br>`vendor/boltz-client/crates/platform-utils` | Keep vendored package shape; coordinate any split upstream. Review points if it grows: `auth.rs`, `http`, `proxy.rs`. | none declared |

## Phased plan

The order combines edit frequency, size, exposed surface, and measured unit
time. It does not rank cold dependency warm-up time as though every edit paid
it. Stable rustc's serial front end makes a large, frequently edited unit a
better first target than a large, rarely edited fixture. The tables below are
the implementation backlog; the original three #12184 splits are not new work.

| Order | Phase and crates | First reviewable extraction | Exit condition |
| ---: | --- | --- | --- |
| 0 | Completed #12184 media work | `openagents-media-view`, landed as `cc84ee3d7c` by chat `150a40a9` | Consumer samples are recorded above, including the slower UI build. Do not assign another agent this completed scope. |
| 1 | Hot product units: `openagents-web`, `coder-task`, `coder-new`, `openagents-cli` | Web context then one route family; task store/types then one agent family; Coder tool/provider contracts; CLI common options then one command family | Each new family has its own focused tests; editing it avoids compiling unrelated families. Measure final command/binary consumers. |
| 1a | Early ML track: `psionic-train` core and a first family | Eval/train contracts, then single-device versus distributed Parameter Golf | Training's 76 s edit command and 33.67 s library front end justify starting now; import history understates churn. Keep each new family below the size target. |
| 2 | App `openagents-engine`, remaining app UI, `openagents-mobile` | Workspace files/source-control engine; file/settings views; portable mobile layout | Independent pure state/logic tests run without the full app shell. Keep one desktop app and one mobile app. Follow, rather than overlap, the media split. |
| 3 | `gateway`, `verse-zone-everglade`, `verse` | Account/admission boundary, demolition/content boundary, desktop/panels boundary | Preserve auth/settlement, deterministic pack checks, and browser/mobile feature builds; no new mandatory host dependencies. |
| 4 | Remaining Coder second cuts: runtime/router, then worker binary | Program runtime versus turn runtime; routing tools; worker executable | Moved child and consumer both improve on repeat samples; keep compatibility facade. Avoid using one post-move commit as its workload estimate. |
| 5 | High-fanout contracts: Nostr, inference, access/host, chat, tenancy, Gym | Nostr core and one NIP family; inference protocol; host client/server; book/store boundaries | Migrate actual consumers to narrow crates. A facade that still imports every child is an interim compatibility layer. Preserve authority/storage invariants. |
| 6 | Large Psionic families | Eval-core/train-core, model interfaces, then training/eval/serving families | Lower layers no longer import model execution or experiment implementations just for a type. Measure CPU-only and enabled GPU paths separately; preserve independent eval. |
| 7 | General size work and retained code | Remaining >20k units, generated proto separation, optional adapters, legacy retirement | Every oversize crate has an owner, next extraction, or dated exception. Small coherent crates stay intact. |

Do one extraction at a time within each chain. Independent chains can proceed
concurrently after their contracts are settled, but a shared types change should
land before several agents start depending on it. Add newly separated leaves
to incremental compilation only when the rule in `AGENTS.md` permits it: their
workspace dependents must also use incremental compilation. Do not enable it
for a high-fanout core by default and lose cross-worktree compiler-cache reuse.

### A size report in CI

Propose `scripts/dev/report-crate-size.py`, invoked by the existing local or
non-GitHub verification service, not a GitHub workflow. No script is implemented
in this docs-only change. It should:

1. Discover tracked files whose basename is exactly `Cargo.toml`, find every
   `[workspace]`, run no-build metadata, and deduplicate members by physical
   manifest path. Report malformed fixtures separately. Do not assume `crates/*`
   is the whole monorepo, or count path members repeatedly in several workspaces.
2. Assign each tracked Rust file to its nearest package. Report raw physical
   source lines, generated source lines, test/example/build-script lines, module
   count, and the previous report's delta. Ignore all Cargo target directories.
   Generated Rust must have an explicit header/path classification; a growing
   generated file must not silently hide handwritten logic. Report Rust
   `include!` and external `#[path]` inclusions too; a macro or include wrapper
   must not pass the limit while retaining the same large compiler unit.
3. Sort by authored source size and report >20k. Require a reviewed split plan
   or an exception above 40k. Start as a report so existing oversize crates do not
   block unrelated work. Once the baseline is recorded, reject a new oversize
   crate or an unexplained growth beyond its approved ceiling. Add a separate
   report for source files above 2k lines to reveal extraction preparation work.
4. Keep exceptions in a reviewed data file, keyed by **workspace + package path**,
   with owner, reason, maximum size, expiry/review date, and the last measured
   edit build. Generated LND bindings are a plausible exception; a 150k-line
   facade is not. Vendor code follows upstream unless downstream measurements
   justify a maintained fork.
5. Emit both a Markdown table and JSON for trend tracking. Run a small metadata
   and counting fixture check when implementing the reporter; no full Rust build
   is needed for a source-size check. This proposal changes no current CI gate.

The initial raw `src/**` inventory has 46 packages over 20k and 87 over 10k.
These include inline tests and generated code; use the authored/generated split
before setting enforcement ceilings. A crate with thousands of unrelated tiny
modules can still be expensive, and a stable generated types crate can be cheap
to edit because nobody edits it directly. Size is a prompt for review, not the
only priority score.

### Test-time wins

The earlier audit's **384.5 s** app UI `cargo test -p` measurement was the
**build of the tests** on the Mac with kache, under that audit's settings. It
was not 384.5 s of test execution on coderos. Keep it as the original motivation,
but do not claim today's Linux checks have reproduced it. #12184's later Linux
UI baseline was 21.1 s and media-local test build 2.6 s; these measure a different
host/configuration and a smaller selected target.

Move pure view models, transcript transformations, file diff state, settings
validation, and task policy tests with their implementations. `cargo test -p
NEW_CRATE` should avoid linking GPUI, every engine adapter, or a whole website
when its code needs only DTOs. Keep integration tests that exercise assembly in
the final consumer. Move test fixtures and dev dependencies too; a dev-dependency
back to the old facade can recreate the full build and even a package cycle.

#12199 already merged many integration test binaries in Coder, CLI, SDK, host,
and engine, and removed SDK → website dev dependency. The earlier audit measured Coder
test builds at 117.2 → 45.6 s on the Mac after harness consolidation. Keep that
win. Prefer one
integration harness per package, with modules for ordinary cases and separate
binaries only for genuine process-global/environment isolation. Small packages
should not each gain dozens of fixture binaries. Avoid moving every test into a
new catch-all test crate that depends on the whole monorepo. A full affected-chain
test build can get slower when new libraries add test binaries, so record that
aggregate cost when changing what CI selects.

After each split, record separately:

| Measurement | What it answers |
| --- | --- |
| Warm edit `cargo check -p` | Parsing/type/borrow-check loop, without linking |
| Warm edit `cargo build -p` | Package artifacts, including its binaries |
| Warm edit `cargo test -p --no-run` | Unit/integration harness compile and link cost |
| `cargo test -p` after `--no-run` | Actual test execution cost and isolation |
| Same edited child → app/CLI/server consumer build and test build | User-visible consumer benefit and any new serialized chain |
| Native fixture/example check and relevant no-default-features/browser/mobile build | Feature forwarding and platform compatibility |

This audit samples check/build, not all test execution. It does not promise a
runtime speedup. Keep performance tests' required optimization settings: the
split chat reports chat-view's 5 ms rail assertion needs its existing optimized
test configuration, rather than a weaker assertion.

## How an agent splits a crate safely

1. **Choose one seam and record a baseline.** Claim its work, coordinate with
   active owners, identify the real edited modules and final consumers, and
   write down current feature/default/platform combinations. Warm each command;
   record no-op, edit check/build, and test-build times. Use an isolated scratch
   edit and always restore it. Check disk space and use the approved target.
2. **Move mechanically first.** Create one crate, move one coherent family
   byte-for-byte, preserve module names, tests, assets, generated inputs,
   `include_*` paths, and fixture-relative paths. Keep behavior changes out of
   this commit. Private module preparation can be a preceding small commit.
3. **Move shared values downward before implementations.** Prefer an existing
   protocol/types crate. Add a small contract crate only for genuinely shared
   DTOs and ports. It must not import the implementation, binary, UI, database,
   or transport whose dependencies it is meant to remove. A type's inherent impl must move
   with it, or remain behind an implementation wrapper; it cannot stay as an
   inherent impl of a foreign type in the parent crate. Replace callbacks to
   the parent with explicit traits/events, not a new globally shared context.
4. **Preserve imports with reexports.** The old crate depends on the new child
   and reexports its old paths. The child must not depend back on the facade.
   Forward features deliberately, keep defaults unchanged initially, and check
   Cargo's normal and dev dependency trees for cycles. Additive features must
   not accidentally enable GPU/host/storage for portable clients. Inspect the resolved dependency tree and
   each workspace-hack patch: feature unification can retain a native dependency
   even after the direct manifest looks smaller. Preserve Hakari's measured
   feature-reuse benefit; change traversal exclusions only with measurements.
5. **Validate one new implementation crate per commit.** Run formatting only on
   changed files, focused child tests, and the relevant consumer/fixture checks.
   A tiny contract extraction may need its own preceding commit; do not batch
   several unrelated implementation crates into one large landing. Check package
   selection in Dockerfiles, launchers, generated references, and release inputs.
6. **Measure both sides, then migrate consumers.** Repeat identical edits and
   flags in the child and through the final consumer, preferably three rounds
   under comparable load. Record no-op overhead and unit front-end/codegen time.
   If only the child's tests improve, say so. Change consumers to direct narrow
   dependencies in a later mechanical commit before removing compatibility
   reexports. Rebase, rerun checks affected by upstream changes, and land.

## Reproduction notes

The inventory uses tracked files, not whatever another agent has left in an
untracked directory. The essential commands are:

```sh
df -h /
git fetch origin main
git worktree add -b codex/crate-audit ~/wt/crate-audit origin/main
cd ~/wt/crate-audit
gh issue view 12184 -R OpenAgentsInc/openagents --comments
git log --oneline --grep=12184
git ls-files '*Cargo.toml'  # Filter basename == Cargo.toml in the collector.
cargo metadata --no-deps --format-version 1
cargo metadata --no-deps --format-version 1 \
  --manifest-path apps/openagents/Cargo.toml
cargo metadata --no-deps --format-version 1 \
  --manifest-path crates/psionic/Cargo.toml
git log --format=%H --since=2026-10-06T20:58:27-05:00 \
  --until=2026-10-20T20:58:27-05:00 -- crates/openagents-web
```

Repeat metadata for every workspace listed above, including the web/fixture
workspaces. Build inside `nix-shell apps/openagents/shell-linux.nix` on this
NixOS host to supply the native dependencies. Set the target for the selected
workspace before loading the build helper:

```sh
export CARGO_TARGET_DIR="$HOME/wt/target-crate-audit/root"
export OPENAGENTS_KEEP_TARGET_DIR=1
export CARGO_BUILD_JOBS=8
export RUSTC_WRAPPER=
export CARGO_BUILD_RUSTC_WRAPPER=
unset CARGO_INCREMENTAL
. scripts/dev/cargo-build-env.sh
cargo check -p openagents-web --timings  # warm, then no-op, then private edit
cargo build -p openagents-web --timings # warm, then no-op, then private edit
```

For each command, append the following only during its edit pass, and restore
the original bytes even if Cargo fails:

```rust
#[allow(dead_code)]
fn __crate_audit_private_edit_probe_20261020() {}
```

Use a fresh restore/edit for check and build separately. For app and Psionic,
change working directory, select their compiler/profile, and use their target
subdirectory. Do not run `cargo clean` on any shared target. The audit's temporary
target is removed after extracting the timing data; the committed tables retain
the package-level evidence needed to rerun the samples.

## Final audit checks

The final inventory is at `11b6369e40`, after the media landing and the new
`openagents-web-keys` package. It includes 420 workspace members and nine
nonmember support/fixture manifests across 25 workspace roots. Every one of
the 429 package entries has both a metric row and a split/keep decision.
The independent `wc -l` total is 5,269,679 Rust lines, of which 4,831,177 are
under `src/**`. All 46 packages above 20k raw source lines have an explicit
candidate boundary or retained-code recommendation. The dependency model
also covers all 2,231 local normal/build edges returned by the root and app
resolved metadata (including local patches); declared optional/target edges
make the reported impact envelope wider than a single selected build.

All 96 warm-up/no-op/edit check and build commands passed for the 16 selected
packages. The two nightly IR commands passed. These are compilation samples;
this audit does not claim to have run every crate's tests. Documentation-only
changes do not require the Rust verification gate under `AGENTS.md`.

Every temporary source probe was restored. The entire
`~/wt/target-crate-audit` directory was removed after extracting the timing
data. Minimum observed free space during the samples was **111.0 GiB**; no
disk guard fired. The owner's release target and other agents' targets were
not used or cleaned. The final commit changes only this Markdown document.
