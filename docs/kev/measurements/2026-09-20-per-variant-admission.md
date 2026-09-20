# Per-variant forward admission: the estimate beside a real forward

[openagents#9426](https://github.com/OpenAgentsInc/openagents/issues/9426)
asked that `kev-serve` bound concurrent work per model rather than with one
slot count shared across every loaded variant, and that the bound come from
the model's cost and the host's measured budget. This record is the check
that the cost the door computes covers what a real forward takes, on the one
box available, with the commands beside each number.

## The machine

CPU only, fp32, the `kev-serve` default.

```text
cpu: INTEL(R) XEON(R) PLATINUM 8559C, 8 cores, KVM guest
mem: 31 GiB
kernel: 5.15.200 (Linux)
toolchain: rust-toolchain.toml as pinned
weights: ./scripts/fetch-kev-artifacts.sh kev-0.5b
```

`kev-0.5b` on `Qwen/Qwen2.5-0.5B`: 14 attention heads of width 64, hidden
896, intermediate 4,864, served at fp32. Those are the numbers
`Variant::forward_bytes` reads from the loaded backbone; nothing below was
typed in from the model card.

## What the door computes

Started with the defaults, after the weights loaded:

```text
KEV_ARTIFACT_DIR=…/kev-artifacts/kev-0.5b KEV_BASE_DIR=…/kev-artifacts/qwen2.5-0.5b \
  kev-serve --port 8009
kev-serve: memory budget 28771 MiB, 2 forwards at once across variants
kev-serve: kev-latest: one forward at 8192 tokens needs 11480 MiB, 2 at once
```

The budget is `MemAvailable` read after the 2 GiB of fp32 weights were
resident. At the 8,192-token bound one forward is estimated at 11,480 MiB,
so two fit the budget and the variant's slots equal the host's two. On a
16 GiB host the same variant would get one slot at this token bound, and
on an 8 GiB host `kev-serve` would refuse to start until `--max-tokens`
brought one forward inside the budget.

## The estimate against a resident-set peak

One request of 1,814 packed tokens (a `noul` question over a 100-sentence
state), against a server started with `--max-tokens 1814
--memory-budget-mib 4096` so the estimate is stated at that length:

```text
kev-serve: kev-latest: one forward at 1814 tokens needs 645 MiB, 2 at once
```

`VmHWM` from `/proc/<pid>/status`, the process's peak resident set, before
and after:

| | VmHWM | Growth |
| --- | --- | --- |
| Idle, weights resident | 2,111,084 kB | |
| After one forward at 1,814 tokens (9.3 s) | 2,715,792 kB | 604,708 kB = 590.5 MiB |
| After two forwards at once | 3,335,340 kB | 1,224,256 kB = 1,195.6 MiB |

The door reserved 645 MiB for one forward and 1,290 MiB for two; the
process peaked at 590.5 MiB and 1,195.6 MiB. The estimate covers the
measurement with 9% and 8% to spare at this length. At 8,192 tokens the
`tokens²` terms dominate and the estimate is not measured here: a
full-length forward on this box would hold about 11 GiB and was not run,
because the acceptance asks for instrumentation without provoking a
machine-wide exhaustion.

## The refusals, on real weights

Three requests at once against `concurrency 2`; `/api/info` read while two
were in flight:

```text
req1 status=503
{"detail":"busy: the host's forward slots at its limit, 2 of 2 in use","error":{"code":"busy",…}}
in_use_mib 1290 in_flight 2
req2 status=200
req3 status=200
in_use_mib 0 in_flight 0
```

Two requests at once against `--memory-budget-mib 1000`, which gives the
variant one slot (1000 / 645) under a host bound of two:

```text
kev-serve: kev-latest: one forward at 1814 tokens needs 645 MiB, 1 at once
req1 status=503
{"detail":"busy: the `kev-latest` forward slots at its limit, 1 of 1 in use",…}
req2 status=200
```

Both refusals arrived at once, before the admitted forward finished, and
every counter read zero after the forwards ended.

## The synthetic checks

`crates/kev/tests/refusals.rs` builds three one-layer variants from
safetensors it writes itself: `kev-test` and `kev-broken` with two heads,
and `kev-wide` with eight, at a 200-token bound and a 12 MiB budget. There
`kev-test` costs 2 MiB a forward and `kev-wide` 4 MiB, so under a host
bound of four `kev-test` gets four slots and `kev-wide` three. The tests
hold `kev-wide`'s slots and show `kev-test` still answers; spend the memory
budget and show a variant with free slots is refused naming the budget;
fire eight requests at a one-slot door held shut and show every one is
refused at once with no permit left behind; and check each variant's
`forward_mib` and `limit` against `Variant::forward_bytes` on its own
config rather than a number in the test.

```text
cargo test -p kev --features serve
```

## What is not measured here

Metal and macOS. `host_memory_budget` reads `vm_stat` there and the
parser is unit-tested against a captured line format, but no Apple
machine ran this. The bf16 path (`--dtype bf16`) halves the dtype term in
the estimate and was not measured against a resident set either.
