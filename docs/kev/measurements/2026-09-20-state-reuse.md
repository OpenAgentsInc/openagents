# Exact-state reuse and the cache decision

Defer a prefix KV cache for the measured Coder workload. All 461 open
requests in the replacement 4B evaluation have different encoded states.
The 76 Coder requests also have no exact repeats. Shared transcript text
is insufficient: a changed encoded state would miss an exact-prefix cache.
This is an audit of archived evaluation inputs, not a production hit rate.

## Measurement

The [candidate evaluation](2026-09-20-candidate-4b.md) pins the adapter,
base, tokenizer, question sets, and suites. `workload_shapes` uses the real
Rust renderer and encoder without model inference. It hashes little-endian
`u32` state token IDs, including the state delimiter. The retained audit
contains IDs, hashes, and counts, without raw state. No locked examples
were encoded or scored.

The simulated policy is four LRU entries, a minimum state length of 384
tokens, and the evaluation's 4,096 packed-token bound. Refused requests do
not enter the cache. The threshold and entry count are an experimental
control, not a new serving policy.

| Open suite | Requests / unique states | State tokens: min / median / p95 / max | Eligible | Refused | Simulated hits |
| --- | ---: | ---: | ---: | ---: | ---: |
| Coder turns v2 | 76 / 76 | 100 / 1649.5 / 5783 / 6001 | 44 | 14 | 0 |
| Program selection v2 | 68 / 68 | 4 / 21 / 106 / 757 | 2 | 0 | 0 |
| Support v2 | 157 / 157 | 7 / 12 / 15 / 21 | 0 | 0 | 0 |
| External labels v1 | 160 / 160 | 14 / 69.5 / 246 / 555 | 2 | 0 | 0 |
| Combined | 461 / 461 | 4 / 32 / 3196 / 6001 | 48 | 14 | 0 |

The observed reuse-distance list is empty because no state repeats. It
cannot estimate a future distribution of distances. Concatenating the
suites in another order cannot create a repeat in these inputs.

## Fixed-document control

A separate synthetic control alternates a routing question and an exchange
question over the same document. Each case has 20 requests. It verifies
that changing question branches does not change the encoded state key.
Changing a ticket reference does change that key.

| Control | State tokens | Unique states | Eligible | Simulated hits |
| --- | ---: | ---: | ---: | ---: |
| Short, repeated | 26 | 1 | 0 | 0 |
| Short, new reference | 35 | 20 | 0 | 0 |
| Long, repeated | 578 | 1 | 20 | 19 |
| Long, new reference | 587 | 20 | 20 | 0 |

The repeated controls have a reuse distance of zero distinct intervening
states. Separate LRU checks verify a hit after three intervening distinct
states and eviction after four. The long repeated control shows a possible
95% hit rate for this deliberately repetitive input, not a measured KV
cache speedup. The short repeated control is deliberately bypassed.

`build-kev-reuse-control.py` creates 80 unscored open inputs and one synthetic
locked sentinel because Gym requires every partition to exist. The sentinel
is never encoded and is not a held-out quality example. The generated suite
is separate from every scored workload.

## Why defer implementation

The current 4B has 36 layers, eight KV heads, and 128 values per head.
At bf16, K and V alone cost `2 × 36 × 8 × 128 × 2 = 147456` bytes per state
token. Four 8,192-token entries would retain 4.5 GiB before final hidden
states, temporary copies, or in-flight references. An entry count alone
would exceed the evaluation's 4 GiB forward budget. A useful implementation
needs byte accounting and ownership limits as well as eviction.

No serving cache was added. Cache concurrency, invalidation, and numerical
parity acceptance tests are therefore conditional work, not tests reported
as passing. Repeated versus new HTTP requests in the baseline are uncached;
their different lengths and timings do not measure a cache benefit.

Revisit this decision for a concrete fixed-document feature or a separately
authorized sample of live exact-state hashes showing enough eligible reuse.
Before implementation, freeze its byte budget, reuse workload, and latency
criterion. Namespace entries by artifact, tokenizer/configuration, dtype,
device/backend, encoding/isolation mode, and exact encoded state. Then test
changed questions, concurrency, eviction, failures, and uncached parity.

## Reproduce

```sh
cargo build --release -p kev --example workload_shapes
# Repeat for each of the four open suites; use their pinned tokenizer.
target/release/examples/workload_shapes ADAPTER SUITE > shapes.json
python3 scripts/report-kev-state-reuse.py shapes.json
python3 scripts/build-kev-reuse-control.py > /tmp/reuse-control.json
target/release/examples/workload_shapes ADAPTER /tmp/reuse-control.json > control.json
python3 scripts/report-kev-state-reuse.py control.json
```

The complete workload and control reports are in
[`data/state-reuse/`](data/state-reuse/). This measured deferral completes
[#9460](https://github.com/OpenAgentsInc/openagents/issues/9460); it makes
no claim about upstream's implementation or repeated-document speedup.
