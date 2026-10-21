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
