# 2026-03-16 Psionic Extraction Audit

## Scope

This audit answers a concrete near-term question:

> if we extract `crates/psionic/*` into a fresh `OpenAgentsInc/psionic` repo,
> what should move, what should stay here, and how should the new repo be set
> up so we do not create unnecessary churn?

This is a current-tree audit, not a greenfield wish list.

## Executive Summary

The short answer is:

- extract Psionic as a standalone multi-crate Rust workspace
- keep the `psionic-` prefix on all Rust packages
- do not mix the repo split with protocol or format renames
- preserve history with `git filter-repo`, not a copy-paste export
- move the reusable Psionic crates, docs, fixtures, and pure Psionic gates
- leave app-owned and product-owned acceptance flows in `openagents`
- rewire `openagents` back to Psionic through pinned git dependencies

The biggest practical risks are not inside the crate graph. They are:

- monorepo path assumptions baked into docs, fixtures, and some code strings
- the size of the checked-in fixture corpus
- release scripts that straddle Psionic and `autopilot-desktop`
- current license metadata inconsistency

## Current State Snapshot

As of this audit:

- Psionic is `32` crates under `crates/psionic/psionic-*`.
- The subtree is `563` files and about `281 MB`.
- `crates/psionic/fixtures` alone is about `268 MB` across `241` files.
- The largest checked-in artifacts are several `checkpoint_state.json` files at
  roughly `85 MB` each.
- Every Psionic crate currently path-depends only on sibling Psionic crates plus
  third-party workspace dependencies. There are no path dependencies from
  `crates/psionic/*` back into `apps/*`, `openagents-*`, `wgpui`, `spark`, or
  `nostr`.

That means the code graph is already close to extractable.

The real coupling is outward:

- `apps/autopilot-desktop`
- `crates/openagents-provider-substrate`
- `crates/openagents-validator-service`
- `crates/arc/benchmark`
- `crates/arc/datasets`
- `crates/arc/ml`
- `crates/arc/solvers`

Those seven consumers currently use direct path dependencies into
`crates/psionic/*`.

There is also script and docs coupling:

- `64` root-level `scripts/release/check-psionic-*` scripts
- `1` Psionic-specific root lint script:
  `scripts/lint/psionic-compiler-replay-gate.sh`
- many docs inside and outside `crates/psionic/` that refer to
  `crates/psionic/*`
- some absolute local paths, for example
  `/Users/christopherdavid/code/openagents/...`, already committed in Psionic
  docs and fixtures

There is also one cleanup item that should be treated as real extraction work:

- `Cargo.toml` workspace metadata currently says `license = "CC-0"`, while the
  repo `LICENSE` file is Apache 2.0

That mismatch should be resolved before treating the extracted repo as a clean
external dependency or publishable package set.

## Decision: Keep The `psionic-` Prefix

Yes: keep the `psionic-` prefix on the subpackages.

That is the right call for four reasons:

1. It matches the current reality across all `32` crates.
2. It keeps consumer churn low. Existing imports like `psionic_runtime` and
   packages like `psionic-runtime` do not need a mass rename.
3. It gives the crate family a stable ecosystem namespace if these crates are
   ever published outside this monorepo.
4. It keeps Psionic clearly separate from product crates such as
   `openagents-provider-substrate` and app-owned surfaces.

What I would not do:

- do not drop the prefix and rename crates to generic names like `runtime`,
  `models`, or `serve`
- do not rename them to `openagents-*` during the extraction

There is an older planning thread in `crates/psionic/docs/plan.md` that talked
about `openagents-*` names. The shipped codebase has already standardized on
`psionic-*`. The extraction should follow the code, not reopen that naming
question.

## Decision: Do Not Rename OpenAgents Protocol IDs In The First Split

There are many OpenAgents-branded identifiers inside Psionic today, including:

- `env.openagents.*`
- `dataset://openagents/*`
- `benchmark://openagents/*`
- `openagents.apple-fmadapter.v1`
- `https://openagents.dev/psionic/...`
- `.openagents-home` and related sandbox scratch directories

The first extraction should keep those as-is.

Reason:

- those are existing protocol, dataset, benchmark, schema, or runtime identity
  strings
- changing them at the same time as the repo split would combine repository
  packaging churn with contract churn
- some of them still correctly belong to the OpenAgents ecosystem even if the
  implementation lives in a standalone `psionic` repo

So the repo split should change repository ownership and dependency wiring, not
the semantic identifiers emitted by the current system.

## What Should Move To `OpenAgentsInc/psionic`

The new repo should own the reusable Psionic workspace itself:

- all `32` crates currently under `crates/psionic/psionic-*`
- the current Psionic README, promoted to the new repo root
- the current Psionic docs under `crates/psionic/docs/`
- the current Psionic fixtures under `crates/psionic/fixtures/`
- the current Psionic helper scripts under `crates/psionic/scripts/`
- pure Psionic release/lint gates that operate only on Psionic crates

Examples of root scripts that should move with Psionic:

- `scripts/release/check-psionic-framework-core-acceptance.sh`
- `scripts/release/check-psionic-product-class-acceptance.sh`
- `scripts/release/check-psionic-topology-acceptance-matrix.sh`
- `scripts/release/check-psionic-eval-runtime.sh`
- `scripts/release/check-psionic-data-contracts.sh`
- `scripts/release/check-psionic-compiler-hygiene-parity.sh`
- `scripts/lint/psionic-compiler-replay-gate.sh`

Those are Psionic-owned quality gates, not product UX or packaged-app flows.

## What Should Stay In `openagents`

The new repo should not absorb product-owned or app-owned surfaces.

These should stay here:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/headless-compute.md`
- `docs/v01.md`
- `apps/autopilot-desktop/*`
- `crates/openagents-provider-substrate/*`
- `crates/openagents-validator-service/*`
- `apps/nexus-control/*`
- product release scripts and packaged app checks

Examples of scripts that should stay in `openagents` or be split into an
OpenAgents overlay:

- `scripts/release/check-psionic-apple-architecture-explainer-acceptance.sh`
  because it runs `cargo run -p autopilot-desktop --bin apple_architecture_explainer_acceptance_harness`
- `scripts/release/check-psionic-apple-rust-only-gate.sh`
  because it scans `apps/autopilot-desktop`, `docs/headless-compute.md`, and
  Psionic together
- `scripts/release/check-v01-packaged-compute.sh`
- `scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`

Those are integration or product-release gates, not standalone library gates.

## Fresh Repo Shape

The best fresh-repo shape is a normal dedicated workspace, not a preserved
`crates/psionic/...` nesting layer.

I would set it up like this:

- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `LICENSE`
- `crates/psionic-core`
- `crates/psionic-ir`
- `crates/psionic-runtime`
- `crates/psionic-serve`
- `crates/...` for the rest of the Psionic crates
- `docs/`
- `docs/audits/`
- `fixtures/`
- `scripts/`

That is better than keeping `crates/psionic/psionic-*` in the new repo because:

- the repo itself is already named `psionic`
- `crates/psionic/psionic-runtime` is redundant once the code is no longer in a
  monorepo
- the flatter layout is easier for contributors and for repo-local docs

So the package names should stay `psionic-*`, but the filesystem layout should
be flattened one level.

## Workspace Setup Recommendations

The new repo should start with:

- one shared workspace version across all Psionic crates
- the current workspace lint posture copied over
- the current shared third-party dependency policy copied into
  `[workspace.dependencies]`
- `publish = false` at first
- edition `2024`
- a committed `Cargo.lock`

Why `publish = false` initially:

- the crate family is still moving fast
- the internal dependency graph is dense
- `openagents` can consume it through git dependencies first
- it avoids accidental crates.io publication before the API and license story
  are clean

Once the repo is stable, publication can be reconsidered. The extraction itself
should not depend on crates.io.

## History-Preserving Extraction Method

The best extraction path is:

1. Create the new repo from filtered history.
2. Then do one normalization commit in the new repo.
3. Then rewire `openagents` to consume the new repo.

I would not do a manual copy of files into a blank repo.

### Recommended Extraction Flow

Use `git filter-repo` on a fresh clone of `openagents`.

The simplest good starting point is to extract the Psionic subtree itself:

```bash
git clone git@github.com:OpenAgentsInc/openagents.git psionic-extract
cd psionic-extract
git filter-repo --path crates/psionic --path-rename crates/psionic/:
```

That gives a new repo whose root is the current Psionic subtree:

- `README.md`
- `docs/`
- `fixtures/`
- `scripts/`
- `psionic-*`

Then make one intentional follow-up commit that:

- creates `crates/`
- moves each `psionic-*` directory under `crates/`
- adds a new workspace `Cargo.toml`
- adds or copies the pure Psionic root scripts that should live with the new
  repo
- updates path strings and docs references

Why this is better than trying to filter many root scripts at the same time:

- the core Psionic history stays clean
- you do not accidentally import product-owned scripts and docs
- the first repo cut is mechanically obvious and auditable

`git subtree split` is acceptable as a fallback, but `git filter-repo` is the
better tool here because it handles path rewriting more cleanly.

## New Repo Cleanup Immediately After Extraction

There are a few cleanup items that should happen in the first repo-normalizing
commit.

### 1. Rewrite Path Assumptions

Many current docs and some code strings assume monorepo-relative paths like:

- `crates/psionic/docs/...`
- `crates/psionic/fixtures/...`
- `crates/psionic/scripts/...`

Those should be rewritten to the new repo shape:

- `docs/...`
- `fixtures/...`
- `scripts/...`
- `crates/psionic-*/...`

### 2. Remove Absolute Local Paths

There are already committed references to:

- `/Users/christopherdavid/code/openagents/...`

Those must be scrubbed. A standalone repo cannot keep machine-local absolute
paths in canonical docs or fixtures.

### 3. Split Integration Overlays From Core Psionic Docs

Some current Psionic docs and fixtures reference:

- `apps/autopilot-desktop`
- `autopilotctl`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `openagents-provider-substrate`

Those references are valid in the monorepo, but they are not good primary
dependencies for a standalone Psionic repo.

The best pattern is:

- keep Psionic system specs as Psionic-owned docs
- move OpenAgents-specific operator notes into a clearly named integration
  section, for example `docs/integrations/openagents/`
- keep product ownership docs in the OpenAgents repo, then link to them as
  external integration docs if still needed

### 4. Resolve License Metadata

Before external consumption is normalized, the new repo should make the license
story consistent:

- `Cargo.toml` workspace license
- individual crate metadata
- `LICENSE` file

Right now that is inconsistent in the monorepo.

### 5. Decide Fixture Policy Up Front

The current fixture corpus is not small. Most of the weight is in large JSON
run artifacts, especially Tassadar checkpoint state files.

My recommendation:

- keep small canonical fixtures in normal git
- keep the research run bundles that are genuinely part of the canonical
  evidence trail
- strongly consider moving the largest checkpoint artifacts to Git LFS or a
  release-artifact bucket if they are not needed for ordinary CI

Do not postpone this indefinitely. A fresh repo is the easiest time to choose a
fixture policy on purpose.

## How `openagents` Should Consume The Extracted Repo

After the new Psionic repo exists, `openagents` should stop using path
dependencies into `crates/psionic/*`.

The best near-term replacement is pinned git dependencies.

Example shape:

```toml
[workspace.dependencies]
psionic-runtime = { git = "https://github.com/OpenAgentsInc/psionic.git", rev = "<commit>" }
psionic-serve = { git = "https://github.com/OpenAgentsInc/psionic.git", rev = "<commit>" }
psionic-sandbox = { git = "https://github.com/OpenAgentsInc/psionic.git", rev = "<commit>" }
```

Then the consuming crates in `openagents` can use `workspace = true` instead of
repeating path dependencies.

That is better than:

- submodules
- sibling-checkout path dependencies
- publishing immediately to crates.io just to make the split work

For local cross-repo development, use temporary local patching or a local git
branch override, not permanent path coupling back into a sibling checkout.

## Recommended CI Baseline For The New Repo

The new repo should get its own CI quickly, but it does not need the whole
OpenAgents product matrix.

The minimum useful split is:

- Linux CPU job for `cargo test --workspace`
- Linux CPU job for the pure Psionic release gates
- macOS job for Metal / Apple-specific Psionic gates
- self-hosted Linux NVIDIA job for CUDA-specific gates

The important part is ownership clarity:

- pure library/runtime/conformance gates belong in `psionic`
- packaged app, desktop UX, and product acceptance gates stay in `openagents`

## Practical Extraction Sequence

If I were doing this soon, I would do it in this order:

1. Create `OpenAgentsInc/psionic` from filtered history of `crates/psionic`.
2. Make one normalization commit in the new repo:
   - flatten layout
   - add workspace root
   - port pure Psionic scripts
   - scrub paths
   - fix license metadata
3. Add basic CI in the new repo.
4. In `openagents`, replace the seven direct consumer path dependencies with
   pinned git dependencies.
5. Leave product-side docs and integration gates in `openagents`.
6. In a later follow-up, decide whether the very large run artifacts stay in
   git, move to LFS, or move to release assets.

## Final Recommendation

The best extraction is conservative on names and aggressive on ownership:

- keep `psionic-*` package names
- extract the whole Psionic subtree as its own workspace
- flatten the filesystem layout in the new repo
- keep OpenAgents protocol identifiers for now
- move pure Psionic docs, fixtures, and gates
- leave product and app integration flows in `openagents`
- consume Psionic from `openagents` through pinned git dependencies

That gets Psionic into a real standalone repo without reopening every naming,
protocol, and product-boundary question at once.
