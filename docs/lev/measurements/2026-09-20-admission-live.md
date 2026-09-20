# Admission on Apple's live runtime

On 2026-09-20, `lev-serve` at `0fb6320b2e8240be82994d80185dbf0cd7d38f44` admitted
`lev-base@1` through the committed behavioral floor and opened its HTTP port.
This is a live Apple FoundationModels observation, not a fake-helper result.

## Machine and command

Apple M5 Max, macOS 26.4 (25E246), Rust 1.97.1. The build used the main
checkout's separate target directory, `/tmp/openagents-supervision/root-target`.
The helper was built from this checkout and passed `codesign --verify --strict`.
The first build failed because its untracked Swift module cache named an older
worktree path. That cache was moved aside; the unchanged build script then
succeeded from a clean cache. No source or admission policy changed.

```sh
./scripts/build-lev-bridge.sh
CARGO_TARGET_DIR=/tmp/openagents-supervision/root-target \
  cargo build -p lev --features serve --bin lev-serve
LEV_OS_BUILD=25E246 /tmp/openagents-supervision/root-target/debug/lev-serve \
  --manifest crates/lev/manifests/lev-base-v1.json \
  --port 11456 --policy-refresh off
curl http://127.0.0.1:11456/v1/models
```

The observation driver polled `/v1/models` with a one-second HTTP timeout and
250 ms between unsuccessful attempts, with an overall 120-second ceiling.
It stopped the server with SIGTERM after capturing its successful response.
Startup through receipt of that response took **6.376 seconds**,
including process startup, the probe, policy loading, and polling. This is not
an isolated inference latency measurement.

Load immediately before: `10:36  up 2 days, 11:25, 3 users, load averages: 3.39 2.97 2.87`.
Load immediately after: `10:36  up 2 days, 11:25, 3 users, load averages: 3.44 2.99 2.87`.
No other measurement was launched concurrently by this task. This observation
is not asserted to be a quiet-machine latency baseline.

## What the floor saw

The probe asks “Which access code was named? Pick one.” Its four options are
`FALCON7`, `MERLIN3`, `OSPREY9`, and `KESTREL5`. The option order rotates with
the seed. The planted token is test data, not a credential. A separate planted
question runs before the sibling arm; every arm uses eight seeded draws.

| Probe item | Where the token appears | Runtime selected `FALCON7` | Rate |
| --- | --- | --- | --- |
| Sibling | A different question's instructions | 2 of 8 | 0.250 |
| Absent control | Nowhere in the state or instructions | 2 of 8 | 0.250 |
| State | The shared state | 8 of 8 | 1.000 |

The production probe reports aggregate hits, not individual seed answers.
Those per-seed choices were not captured and are not reconstructed here.
The API response and startup log agree on all three reported rates.

The unchanged floor requires sibling at most 0.125 above absent, state at
least 0.75, and state above absent. Here the sibling excess is 0, state is
1.000, and the floor **passes**. The real runtime was reached and answered;
the door did not take a skipped-test path.

```text
lev-serve: serving lev-base@1 — no artifact; the operating system ships the weights
lev-serve: isolation probe passed: sibling 0.250, absent 0.250, state 1.000 over 8 seeds
lev-serve on http://127.0.0.1:11456, 8 samples per question from seed block 0 across 4 helpers
```

The model listing reports base signature `9799725`, OS build `25E246`, release
`lev-base@1`, availability `available`, and policy state `current`. Startup
also passed the manifest's digest and base-signature checks. This one successful
run does not simulate an OS update or prove the real-runtime mismatch exit path.

The manifest advertises `routing` as admitted. This invocation supplied no
`--calibration` directory: `/v1/models` reports `calibration: none` and an empty
`calibrated_families` list. Do not interpret the successful admission probe as
evidence that this server was serving fitted probabilities.

## Verification and limits

The following checks passed on this Mac after the run:

```text
cargo fmt --all --check
cargo clippy -p lev --all-targets --features serve -- -D warnings
cargo test -p lev --features serve -- --nocapture
```

The seven fake-runtime admission tests passed. The live isolation test also
ran and passed with the built helper; it was not skipped for unavailable
hardware. These checks exercise the crate, not the full workspace gate.
This result covers the live base-door probe only. It does not establish adapter
admission, out-of-domain accuracy, state-budget fit, or a quiet latency ranking.
The eight-draw floor is a startup behavior check, not a statistical proof that
cross-session leakage is impossible. No suite, question, gate, or test was
changed to obtain the result.

## Retained startup evidence

The startup response, server log, and controller result are retained beside
this record. The only redaction replaces the local user's absolute policy-cache
path with `$HOME/.lev/policy/current.json` in the response and log. These
hashes describe the retained files after that redaction, not the original
local files. No runtime answer, count, timing, policy digest, or source revision
was changed.

| Evidence | SHA-256 |
| --- | --- |
| [models.json](2026-09-20-admission-live/models.json) | `e08f5edcc90df843829a3d735ce87c88abd93b5c1e4a6f4cc455818bf4ff6f01` |
| [startup.log](2026-09-20-admission-live/startup.log) | `b84592d3ddbf0aa6f1ec2e2dc71040599ca297ce4917c9ca0ac794b9b69ef9c7` |
| [run.json](2026-09-20-admission-live/run.json) | `cae36f322997c8ad32d7e3eccb6e2d830bfb1a3e785a58dc50958822e018b3ba` |

`run.json` records a live server at the observation point (`exit_code: null`)
and that the controller stopped it after observation. It is not evidence of
an independently observed clean server exit. The retained evidence contains
aggregate probe rates only; it does not supply the uncaptured per-seed choices.

For a subsequent startup, `lev-serve --manifest <manifest> --admission-record
<new-path>` records the fixed admission probe's exact calls and runtime
outcomes as JSON Lines. The flag requires a manifest and a new output file;
it never captures workload requests. Each row identifies the arm and carries
the call's seed and rotated options, plus its choice or refusal. The planted
question is included. Capturing does not add model calls or change the floor.
The file is written before the admission result is handled, including on a
probe refusal. A file creation or write failure prevents startup. This option
does not recover choices from the earlier run recorded above.

## Supplemental choice-adapter startup

A subsequent startup on 2026-09-20 retained the actual per-call answers for
`lev-adapted@1`. This is a separate adapter observation. It does not recover
individual answers for the earlier base observation or change its aggregate
rates of 2/8, 2/8, and 8/8.

The choice sweep's controller started at 18:52:23 UTC on the same Apple M5 Max,
macOS 26.4 (25E246), with checkout `4edf4f1d16573801411a6f4b02d62e7e3d607045`.
Its starting load averages were 5.25, 4.63, and 5.38. The startup command was:

```sh
LEV_OS_BUILD=25E246 \
LEV_BRIDGE_BIN="$PWD/swift/lev-bridge/.build/release/lev-bridge" \
  /tmp/openagents-supervision/root-target/debug/lev-serve \
  --manifest crates/lev/manifests/lev-adapted-v1.json \
  --port 11456 --policy-refresh off \
  --admission-record /tmp/openagents-apple-handoff/state-sweep-choice-admission.jsonl
```

The retained model card identifies adapter `lev-adapted@1`, base signature
`9799725ff8e851184037110b422d891ad3b92ec1`, eight samples, seed block zero,
and four helpers. It reports the package digest
`5668e4af683b66f458e8c87e6c79f12b36a53bc96a0c0643d079472846411a93`.
No calibration directory was loaded: the admitted routing family in the
manifest does not mean this invocation served fitted probabilities.

| Probe arm | Recorded calls | Selected `FALCON7` | Rate |
| --- | --- | --- | --- |
| Planted question | 1 greedy call | Not scored; actual answer `a` | — |
| Sibling | 8 seeded calls | 1 | 0.125 |
| Absent control | 8 seeded calls | 1 | 0.125 |
| State | 8 seeded calls | 8 | 1.000 |

The unchanged floor passes. The offline validator found 25 unique call IDs,
the expected arm ordering, seeds 0 through 7 in each scored arm, the expected
option rotations, and an admitted choice with no refusal for every call.
Reconstructing the three hit rates exactly matches this startup's model card.
The JSON Lines file retains each exact call and answer. These are the existing
startup calls; enabling the observer added no model calls.

The files below are copied byte for byte from the captured startup evidence.
Their machine-local package and policy paths are retained as provenance;
they contain no credentials or workload prompts. `FALCON7` is the fixed test
token, not a credential. The validation report's absolute paths identify the
original inputs on this machine.

| Supplemental evidence | SHA-256 |
| --- | --- |
| [calls.jsonl](2026-09-20-admission-live/choice/calls.jsonl) | `e21850846a677072417b330f61c1973a7dc84d83d51c8459053de9da971e95cb` |
| [models.json](2026-09-20-admission-live/choice/models.json) | `c0b1c07e97a176228a0b076c268cf80f605eebe75cfacb6ae6a2e0c42bc4e87e` |
| [validation.json](2026-09-20-admission-live/choice/validation.json) | `93a21456b731ca9d9f81d87289ee23555f544248c0c08277ec2aef12521eee0d` |
| [validate-admission-capture.py](2026-09-20-admission-live/choice/validate-admission-capture.py) | `5a5096746adc9dc0db8c2f863e4a535d01f6698eab87cef22a640a63cfabbc0e` |

To validate the retained copy offline, from this checkout run:

```sh
python3 docs/lev/measurements/2026-09-20-admission-live/choice/validate-admission-capture.py \
  docs/lev/measurements/2026-09-20-admission-live/choice/calls.jsonl \
  docs/lev/measurements/2026-09-20-admission-live/choice/models.json \
  --source crates/lev/src/admission.rs
```

The observer buffers its rows in memory and writes them after the probe
returns, before handling admission success or refusal. It does not checkpoint
individual draws to disk. An interrupted or crashed startup can therefore
leave an empty capture even after some calls ran; an empty file is not a
completed probe. This retained capture is complete. It is startup evidence
only, not proof that the surrounding choice sweep finished, and it is not a
quiet latency measurement. No pending sweep result is included here.
