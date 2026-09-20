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
