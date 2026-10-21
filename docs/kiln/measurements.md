# Kiln demo measurements

Measured on 2026-10-21 on the owner's Mac (Apple silicon) in headless Chrome
through `openagents browser run`, against `wss://relay.openagents.com`.
The page reports these numbers in its **Measurements** panel and in
`window.kilnMetrics`.

## Page load

| What | Size or time |
| --- | --- |
| `kiln_bg.wasm` (page and worker module, `kiln-web` profile, no `wasm-opt`) | 2.2 MB |
| `kiln.js` (`wasm-bindgen` glue) | 40 KB |
| Seven tools, served as `tools/<sha256>.wasm` | 3.0 MB (`rhai` and `test` are 1.4 MB each; the others are 59 to 92 KB) |
| Everything the page downloads | 5.5 MB, uncompressed (`kiln-dev` doesn't compress) |
| Fetch, hash-check, and compile all seven tools | 24 to 47 ms (from the local server) |
| Page ready (tools loaded, OPFS open) | 340 ms after navigation |
| OPFS open and load on a new profile | 30 to 160 ms |
| OPFS load of a saved workspace after a reload | 4 ms |
| OPFS save (snapshot of about 300 bytes) | 1.5 ms |

## Tool startup

Times are wall-clock from the page posting the job to applying the result,
for the self-test's ten runs in one worker.

| What | Time |
| --- | --- |
| A run in a warm worker (`write`, `cat`, `ls`, `grep`, `patch`) | 0.7 to 2.9 ms |
| Instantiating a precompiled tool in the worker | 0.07 to 0.19 ms |
| `test` with Rhai on two test functions | 1 to 9 ms |
| `rhai` running a script that computes 12 Fibonacci numbers | 9.6 ms |
| WASI host calls a run makes | 14 to 34 |

## Pylon round trips

From the page's first agent run (fib task), all on DeepSeek V4.1 group
`DeepSeek-V4.1 w16-g5c` (`deepseek-v4.1-flash@sha256:74b0686a…`):

| What | Time |
| --- | --- |
| Relay connection with NIP-42 sign-in | 244 to 255 ms |
| Beacon query to `EOSE` (11 beacons, 7 usable) | 72 to 101 ms |
| First contact (`processing` feedback) | 299 to 391 ms |
| Answer, small prompt | 23.7, 24.4, and 28.9 s |
| Answer, the Roman-numeral task's first and later steps | 44.9 to 61.4 s |
| Answer from a `qwen3.5-0.8b-q8_0` Pylon (for comparison; unusable output) | 4.2 to 11.0 s |

## Agent steps

| Run | Steps | Total | Model share | Tool share |
| --- | --- | --- | --- | --- |
| Fib task, headless check | 3 | 77 s | 24.1, 24.4, 28.9 s | 56, 2, and 0 ms |
| Fib task, the owner's run on the Mac | 3 | 78 s | about 25 s a step | under 0.1 s |
| Roman numerals, 300-number round trip (before the wait-for-slot fix) | 12 | 230 s | 9 steps went to the small model | under 0.1 s a step |

Almost all of an agent step is model time. The sandbox costs milliseconds.

## Rust tests on the runner (#12437)

Measured on 2026-10-21 with `kiln-runner --jobs 4` on the owner's Mac,
under load from other builds (load average 7 to 35).

| What | Time |
| --- | --- |
| Load `crates/memory-stream` (2 files, 12 KiB): commit and tree API calls | 0.63 to 1.08 s |
| The same, with every raw file fetched and saved to OPFS | 1.0 to 2.2 s |
| Runner setup: shallow bare clone and one worktree | about 3 GB on disk, one time |
| `cargo test -p memory-stream`, cold target | 320 s on the runner (5 min 11 s of compiling) |
| `cargo test -p memory-stream`, warm target, one changed file | 7.7 s on the runner (4.7 s of Cargo) |
| Page to runner and back, beyond the runner's own time | under 1 s |

The cold build is slow because `memory-stream`, like every crate here,
depends on `openagents-workspace-hack`, which unifies the features of about
400 crates for the whole workspace. A runner should keep its target warm.

## The Rust agent run

The `half_life_hours` task on `memory-stream`, every step on
`DeepSeek-V4.1 w16-g5c`:

| What | Value |
| --- | --- |
| Steps | 8, plus two replies in the model's own tool-call markup that the loop skipped |
| Total | 630 s |
| DeepSeek answer per step (4 to 6 KiB prompts) | 85 to 95 s |
| The group's prompt limit | 2,872 tokens; a 13 KiB prompt was refused |
| Remote `cargo test` | 1 run, 6 passed, 7.7 s on the runner |

## The publish run

The same task with publishing, end to end on 2026-10-21 (commit
`e21ed111f1` of `main`):

| What | Value |
| --- | --- |
| Load `crates/memory-stream` | 1.03 s (0.70 s for the commit and tree) |
| Steps | 8, every one on `DeepSeek-V4.1 w16-g5c` |
| DeepSeek answers | 24.5, 32.5, 74.1, 72.0, 104.9, 94.5, 84.6, 91.4 s |
| Remote `cargo test` on the edited file | 6.9 s from the page, 6.5 s on the runner |
| The runner's check of the published patch (`git apply`, then `cargo test`) | 5.7 s on the runner |
| Total, task to published and checked patch | 614 s |

The patch, event `3b06062ede4f3d0c097f2c21cd6e92896113d923e96bacb69f54a24578dd80a6`
on `wss://relay.openagents.com`, adds `half_life_hours()` with a doc
comment and the test `half_life_is_where_recency_reaches_one_half` to
`crates/memory-stream/src/lib.rs` (14 insertions). `review.html` reads it
back with its diff, "open", and the runner's "tests pass" check.
