# R0 frame-tail budget, written before optimizing

## Budget

[`verse/benchmarks/frame-budget.json`](../../verse/benchmarks/frame-budget.json)
fixes, against a 120 Hz frame (8.33 ms), the p99 and max for each captured
scope:

| Scope | p99 | max |
| --- | --- | --- |
| Entire CPU frame | 4.0 ms | 8.33 ms |
| Robot poses and body capsules (Coax) | 500 µs | 2 ms |
| Player update (Coax) | 250 µs | 1 ms |
| Player pose plus palette copy (Coax) | 250 µs | 1 ms |
| Host buffer copy (upload) | 100 µs | 500 µs |
| Render encode and submission | 2 ms | 8.33 ms |
| Completed-command GPU time | 6 ms | 8.33 ms |

At least 540 samples per scope, zero GPU errors, every command completed and
zero observed kernel heap allocations (from `verse/benchmarks/run.py`). It is
the agent's written proposal; owner approval is listed in `NEEDS_OWNER.md`.
The 5% median gate in `contract.json` is unchanged.

## Retained Simulator result

`tails.py` on the retained 2026-10-28 capture (iPhone 17 Pro Max Simulator,
iOS 26.5, build 1056) gives `simulator-only`: every scope is inside budget,
and it is not a phone result.

| Scope | median | p95 | p99 | max | CV |
| --- | --- | --- | --- | --- | --- |
| Entire CPU frame | 214.0 µs | 250.6 µs | 268.5 µs | 351.2 µs | 0.141 |
| Robots and bodies | 14.1 µs | 16.2 µs | 24.1 µs | 43.4 µs | 0.153 |
| Player update | 6.0 µs | 8.6 µs | 10.6 µs | 20.5 µs | 0.246 |
| Pose plus palette copy | 5.1 µs | 6.9 µs | 8.2 µs | 14.4 µs | 0.186 |
| Buffer copy | 0.8 µs | 1.1 µs | 1.2 µs | 1.3 µs | 0.172 |
| Render encode | 93.6 µs | 108.5 µs | 114.5 µs | 149.2 µs | 0.134 |
| GPU | 127.4 µs | 205.2 µs | 278.1 µs | 863.7 µs | 0.421 |

Report: [coax-r0-tails-simulator-2026-10-31.json.gz](data/coax-r0-tails-simulator-2026-10-31.json.gz).

## Physical device

The owner's iPhone 17 Pro Max is paired with this Mac, but a UI test takes
over the phone's screen, so it is not run without the owner present. No device
verdict exists yet; `tails.py --enforce` refuses the Simulator capture. The
owner step (unlock the phone and say go, so an agent runs three captures) is
in `NEEDS_OWNER.md`; the commands are in the
[benchmark README](../../verse/benchmarks/README.md).

```sh
python3 verse/benchmarks/test_tails.py
python3 verse/benchmarks/tails.py docs/verification/data/coax-r0-pose-ios-2026-10-28.json.gz
```
