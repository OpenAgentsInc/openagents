# Isolated delegation study tools

These Python tools run one native Claude attempt in a Linux bubblewrap namespace.
They are benchmark infrastructure, separate from the frozen briefing replay tools.
Use Python 3.12 or later. Preparation policy and study gates belong to the prospective
study protocol.

The [prospective protocol](../../docs/audits/2026-10-03-independent-efficiency/system-one-delegation/protocol.md)
is currently unsealed. The [audit and evidence](../../docs/audits/2026-10-03-independent-efficiency/system-one-delegation/README.md)
separate actual component measurements from the unrun 48-session panel. These
tools do not change product routing. Rust owns the existing `briefing-lab`
index and Tree-sitter extraction; this Python is benchmark infrastructure.

## Preview source context

Build `briefing-lab` on an approved build host with its existing external Cargo
target directory. Keep the index and output outside the inspected repository.
`COMMIT` is a complete 40-character source commit. The issue JSON contains
`title` and `body`, with optional `number` and `url`.

```sh
BRIEFING_LAB_BIN=/path/to/briefing-lab
"$BRIEFING_LAB_BIN" index --repo /path/to/repo --rev COMMIT \
  --syntax --output /path/to/artifacts/index.json

python3 bench/delegation-study/prepare.py \
  --repo /path/to/repo --rev COMMIT \
  --index /path/to/artifacts/index.json \
  --issue /path/to/artifacts/issue.json \
  --mode deterministic --output /path/to/artifacts/preview
```

Read `briefing.md`. `preparation.json` binds inputs, selected source, omissions,
timing, and output by digest. The command never executes instructions from the
issue or inspected source. Cold indexing is separate from warm preview time.

To use semantic ranking, select a new output directory and `--mode jev`. This
sends the public task and source candidates to TypeSafe using `TYPESAFE_API_KEY`
or the configured `~/.openagents/jev.json`. Credentials are absent from retained
requests and receipts. The command makes one paid call to pinned `jev-1.13.0`,
with no automatic retry. A failed or invalid response retains its error and
falls back to deterministic ordering. Inspect `jev-call.json`; a fallback does
not establish that System One ran successfully. The configured key returned
HTTP 402 during the retained capability check.

Both modes share the same candidate pool and 16 KiB output limit. Selection
uses complete Rust declarations from at most 24 files. It does not resolve
types, expand macros, recover every import or caller, or prove requirement
coverage. A relevance grade is not a probability that a patch is correct.
An executor must inspect additional source when the pack is incomplete. The
prior [explicit structural packer](../../crates/briefing-lab/README.md#explicit-structural-policy)
is a different policy with same-file dependency bundles.

## Executor boundary

`run_remote.py CONFIG OUTPUT` creates an exclusive output directory, checks the
source archive and CLI hashes, and exports the historical source at `/workspace`.
The repository has a deterministic, single-commit snapshot; no later Git history
is available. Each run gets a scratch home and a fixed-length UUID prefix in its
first user message. The native CLI has medium effort and the configured model.
The control keeps its native prompt and tool definitions; a treatment can supply
`system_file` and `tools`.

The namespace can read system binaries, the explicitly mounted dependency cache,
and its own source. It cannot read the shared repository, shared Cargo target,
acceptance checkers, credential transfer, or other attempts. Direct network access
is disabled. A loopback bridge reaches only the provider broker's Unix socket.
Server-side web search, MCP connectors, containers, fast mode, and priority or
region-specific inference are refused. Native web tool definitions remain present;
web access is unavailable in this common execution environment.

Required configuration fields are `run_id`, `source_commit`, `source_archive`,
`source_archive_sha256`, `binary`, `binary_sha256`, `binary_version`, `model`,
`effort`, `prompt_file`, `prompt_sha256`, `credential_file`, and `provider_meter`.
Optional `toolchain` contains explicit `read_only_mounts` pairs and `environment`
values. A seed requires `target_seed` and `target_seed_manifest_sha256`. The archive
must be the exact archive bound by that seed.

The credential file contains only the current provider access token. The broker
sets its mode to `0600`, disables process dumps, reads it once, and deletes it.
The executor receives a dummy token. Provision a fresh transfer for each run;
never provide a refresh token or an account configuration file.

## Provider accounting

The broker writes a durable admission receipt before each upstream request and a
terminal receipt afterward. This includes subagents, nested CLIs, and direct
socket calls. JSON and SSE usage are parsed without retaining response text in
the ledger. Streaming output counters are cumulative. Unknown usage retains its
reservation and closes further admission; it is never treated as zero cost.

`provider_meter` supplies the run and per-model request caps, concurrency cap,
maximum requested output tokens, admission target, and explicit model prices.
Each model has `max_requests` and `usd_per_million` fields named `input`, `output`,
`cache_write_5m`, `cache_write_1h`, and `cache_read`. Reservations use twice the
request bytes plus 16,384 input tokens at the highest input/cache-write rate, and
the declared output limit. This is a conservative admission estimate, not a
verified tokenizer bound or an absolute spending guarantee.

The provider ledger is the primary API-equivalent cost measure. Native cumulative
cost is retained once for reconciliation. The nominal native budget can be
exceeded by an admitted call; results and costs are never clipped. Incomplete or
unexpectedly priced responses remain unknown. `provider_accounting_complete` is
separate from `model_completed`, and independent acceptance is a later trusted
phase. The runner never sets `accepted` to true.

All source changes, mode changes, deleted paths, candidate files, native streams,
and errors remain in the private attempt directory. Candidate identity is the
canonical manifest digest, which binds the full change set and payload digest;
the payload digest alone does not bind deletions. Capture permits at most 100,000
entries, 128 MiB per source file, 2 GiB of source bytes per pass, and a 512 MiB
compressed candidate payload. A shared 120-second cooperative deadline covers
the final scan, archive, and validation. Runs can narrow these bounds.
`execution_closed` is set only after local executor and broker cleanup; it does
not convert unknown provider usage to a known cost. `total_retained_wall_s`
covers export through artifact capture and broker drain, before writing the final
result JSON. The standalone runner retains its workspace; the trial coordinator
performs the separately timed cleanup described below.

## Baseline Cargo seeds

`seed.py CONFIG OUTPUT` takes the shared agent target's exclusive calibration lock
and builds only named historical packages with `cargo test --no-run --locked
--offline --message-format=json`. Source and target appear at the same paths used
by attempts. Every exported source file gets a fresh mtime before compilation,
and every reported workspace unit must have `fresh: false`; older source mtimes
must not reuse a later revision's artifacts.

The `cargo-reported-libraries-v1` seed policy includes exact library artifacts
named by Cargo, corresponding dep-info, and fixed fingerprint companions. It
excludes final test and binary executables, all `profile.test` units, unreported
files, diagnostic output, incremental state, and build-script output directories
and fingerprints. Each attempt relinks its executables and regenerates missing
build-script data. Export is bounded to 8 GiB and requires 2 GiB of additional
free space before copying. The shared target is never copied wholesale. Native
execution and acceptance both validate the policy, exact file set, bytes, modes,
source commit, and source archive digest before copying a seed.

Seed configuration requires `repository`, a full `source_commit`, `packages`,
`shared_target`, and `toolchain`. An optional `timeout_s` bounds the baseline build.
Set `toolchain.environment.RUSTDOC` to the canonical absolute executable in the
pinned Rust toolchain, rather than a rustup proxy. The builder validates this
before exporting source and records its version and executable hash. Native
execution and acceptance check this binding; doctests remain enabled.
The trusted builder needs no provider credential and executes no model.

An optional `cargo_features` list names explicit package-qualified features,
for example `["jev/blocking"]`. The builder adds `--features jev/blocking`
and binds the list in the seed manifest. Native and acceptance configurations
must name the same list. Missing fields in retained manifests mean `[]`:
Cargo's default features, with no additional features. Wildcards, duplicate
names, unqualified names, and command flags such as `--all-features` are refused.
Acceptance features must belong to its checker package, which both ordinary
and independent compile/test commands select. Formatting receives no feature flags.

Run synthetic checks with:

```sh
python3 -m unittest discover -s bench/delegation-study -p 'test_*.py'
```

The retained infrastructure preflight and scrubbed provider receipts describe
unscored capability checks. They include a native budget overrun and preserve its
unsuccessful model-completion status.

## Validate a schedule and report evidence

`schedule.py preview` creates a draft sequence of eight complete six-arm blocks.
Supply one UUID as `--study-id`, four `--task` IDs, and a new `--output` file.
The default random seed is 20261003. A preview grants no execution authority.
The retained [48-run draft](../../docs/audits/2026-10-03-independent-efficiency/system-one-delegation/draft-schedule.json)
uses `reserved-a` through `reserved-d`; its
[provenance](../../docs/audits/2026-10-03-independent-efficiency/system-one-delegation/draft-schedule-provenance.json)
records the fresh study UUID, seed, generator hash, and output hash. This preview
is unsealed. Successful funded preflights and final artifact bindings are still
required before any scored run.

```sh
python3 bench/delegation-study/schedule.py validate \
  --registration /path/to/registration.json

python3 bench/delegation-study/report.py \
  --manifest /path/to/report-manifest.json --output /path/to/report
```

The validator requires sealed source, tool, model, price, prompt, checker, seed,
and schedule bindings; real provider and isolation preflights; and a reserve
for the full next block. It launches nothing. Paid dispatch and the final
acceptance coordinator are implemented through `trial.py` and
`check_candidate.py`. Scored use still requires successful provider and
integration preflights and a sealed registration. The current HTTP 402 receipt
does not satisfy the Jev prerequisite.

The reporter uses the sealed registration, validates candidate manifests and
payloads, and independently reprices provider receipts. It keeps unknown costs
as unknown or bounded intervals and withholds model and arm labels until the
bound blinded reviews are complete. Native completion is separate from patch
acceptance. Missing bindings, incomplete accounting, a demonstrated defect,
or an undelivered System One intervention cannot establish the combined thesis.
The tests include synthetic complete and incomplete report manifests; these
fixtures are not experimental results.

## Run one registered trial

Run the frozen copy named by `artifacts.dispatch_coordinator` in the sealed
registration. Its directory contains all 12 runtime modules bound by
`artifacts.harness_manifest`; their hashes are checked before each phase.
The example paths below place that copy under `harness/`. `RUN_UUID` must be the
next attempt in the registered schedule, and the output must be exactly
`runs/RUN_UUID` below the registration directory.

```sh
python3 /path/to/registration/harness/trial.py \
  --registration /path/to/registration/registration.json \
  --run-id RUN_UUID \
  --output /path/to/registration/runs/RUN_UUID \
  --credential-file /path/to/private/one-use-provider-token
```

The command holds one local execution slot. Before a new six-arm block, it
records a reserve of $48 plus preparation overhead against the $120 accounting
ceiling. Earlier attempts remain charged, including failed work. Later attempts
stop when execution or accounting is unresolved, or remaining reserves no longer
fit. It runs one trial; it does not automatically dispatch the panel.

The bound `trial_config` fixes native common settings, arm tools and system
prompt, source-repository and seed locations, acceptance templates, and outer
phase deadlines. Runtime identities come from registration. Native controls
receive the fixed base prompt. Prepared arms receive those same bytes, two
newline bytes, and the exact briefing. The native runner adds its UUID prefix.
Preparation continues to use the separately frozen public issue JSON.

Seed, native, and acceptance configurations must use identical toolchain
settings, including `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0`. Seeds use `cargo-reported-libraries-v1`; final
executables are relinked. The token path is supplied separately for this one
run. Jev's narrow credential stays outside the executor namespace. Neither
credential values nor paths appear in public trial receipts. Keep private
configuration and process logs out of a public evidence bundle until reviewed.

Each phase writes a durable launch intent before starting its subprocess.
Native execution is followed by independent acceptance of its final candidate;
there is no external repair. A budget-ended native session can still produce
an accepted patch when its bound final checks pass. The coordinator writes
`trial.json`, `launches.jsonl`, `endpoint.json`, and `report-entry.json`, with
references to preparation, provider accounting, candidate, and check artifacts.
Blinded source review remains separate and must be added before reporting a
complete quality gate.

An existing output directory is **inspect-only**, even if the earlier attempt
is incomplete. The same invocation never relaunches its paid phases. Inspect
without a credential file using:

```sh
python3 /path/to/registration/harness/trial.py --inspect \
  --output /path/to/registration/runs/RUN_UUID
```

A missing result does not establish that no request was charged. Retain the
directory and its unknown cost; do not delete or rename it to retry. Any
replacement must follow the prospective whole-block amendment rule.

### Timing and cleanup

One host's monotonic clock starts after the execution slot is acquired and
before per-trial registration and archive validation. Validation duration is
also recorded separately. The endpoint includes preparation, Jev, source
export, Git initialization, seed validation and copying, native execution,
capture, provider drain, native scratch removal, independent checks, and durable candidate/check
artifacts. Final receipt bookkeeping follows the endpoint timestamp. Queue
time, shared cold builds and indexing, and blinded review are separate.

Freeze executor and acceptance budgets separately from their outer watchdogs.
For example, a 600-second native execution budget needs additional outer time
for export, capture, and provider drain; a 240-second acceptance watchdog
includes its setup, compilation, and tests. The final registration must bind
feasible outer deadlines; the proposed budgets remain unsealed.

Before acceptance starts, the coordinator syncs the validated native candidate,
change manifest, native result, provider ledger, and private logs. It retains
their hashes in `native-retention.json`, then removes the confirmed-closed
native `workspace` and `home/target` copies. `native-scratch-release.json`
records removal outcomes, duration, and free disk bytes before and after.
This removal is inside the primary endpoint. Failure stops acceptance and later
trial admission, and is retained without an automatic cleanup retry. This prevents overlapping native
and acceptance source/build copies; it is not a disk quota or a reservation
against executor output growth or other processes.

After retaining the endpoint and trial receipts, the coordinator removes the
closed acceptance target and reconstructible workspace, recording this final
cleanup outside the primary endpoint. Source archives, shared seeds,
long-lived targets, logs, receipts, and candidate payloads remain. Cleanup
refuses symlinks or paths outside the attempt. Missing reconstruction evidence
preserves a workspace; unconfirmed process closure preserves that phase's
scratch directories. A failed cleanup is retained and does not cause a paid
retry.

## Final acceptance

`check_candidate.py CONFIG OUTPUT` checks one frozen candidate after native
execution ends. It creates an exclusive private output directory and writes
`checks.json` with schema `openagents.delegation.final-checks.v1`. The receipt
binds the run, canonical candidate manifest, source archive, checker, and baseline
seed. Raw command output and the injected checker remain private. No provider
socket, real home, or network route is mounted, and no repair follows a failure.

Required configuration includes the source and candidate identities from the
native attempt, `candidate_dir`, `allowed_paths`, `packages`, the frozen
`checker` (`path`, `sha256`, `injection`, `target`, and `package`), the verified
`target_seed` and its manifest digest, `toolchain`, and `total_timeout_s`.
`expected_snapshot_commit` checks agreement with the native Git snapshot.
The trial coordinator supplies the run-specific identities and the common
registered verification budget.
For a feature-bearing task, the registered task, native configuration, seed,
and acceptance template bind the same `cargo_features`. Its common base prompt
must contain the exact check command, such as
`cargo test --locked --offline -p jev --features jev/blocking`.
Registration and dispatch reject a missing command before any paid phase.
This exposes the same check policy to every native arm; it does not force the
model to run that command. Final acceptance independently applies the feature list.

Acceptance reconstructs a fresh historical tree and deterministic base Git
snapshot, verifies every candidate preimage, and applies the complete candidate,
including deletions and modes. Changes outside allowed paths, changes to the
checker injection path, and symlink candidates fail scope. The source stays
read-only during checks; the independent phase mounts a private copy of the
frozen checker read-only at its integration-test path. Formatting uses
`cargo fmt --check` without rewriting the candidate. Ordinary package tests and
the independent target each retain separate compile and test-command results.
The test-command time includes Cargo's freshness checks and startup.

The target starts as a separate copy of a digest-verified seed bound to the
exact baseline source and archive. It never copies the executor's modified
cache. On Linux, `cp -a --reflink=auto` uses copy-on-write when supported and
a full copy otherwise. The copy must have distinct writable inodes from the
baseline. Unchanged source keeps its archive timestamps so valid baseline artifacts
can be reused. Changed and newly added files, including non-Rust include inputs,
and the injected checker get fresh timestamps. This differs from seed generation
on a shared target, where every workspace source must be refreshed to exclude
artifacts from other revisions. Acceptance checks the namespace's Cargo and
rustc versions, the declared rustdoc version, and explicit build environment against the seed manifest. The
common pre-scoring profile sets `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` for seed, native, and acceptance work; optimization
and debug assertions remain unchanged. Original default-profile size and disk
failures remain in the infrastructure evidence.

One parent watchdog bounds setup and all checks. Each command gets only the
remaining common budget; runtime-only calibration limits do not silently cap
compilation. The watchdog stops the checking process group and worker before
retaining its final receipt and setting `execution_closed`. Only after that
receipt and logs are durable can the coordinator remove its reconstructible
workspace and target; cleanup time is recorded separately. A timeout after candidate identity and scope
validation records failed checks and non-acceptance. Earlier setup or provenance
failures remain incomplete infrastructure evidence. Completed subphase timings
survive later failures; a missing measurement is not reported as zero.
Final acceptance requires `checks.execution_closed=true`, written only after
worker and checking-process cleanup. A worker-only or premature receipt cannot
establish acceptance or authorize removal of its scratch workspace.

SIGTERM and SIGINT trigger the same cleanup path. The coordinator stops its
worker before loading the last durable check checkpoint, retaining completed
phase timings while marking the interrupted attempt incomplete.
